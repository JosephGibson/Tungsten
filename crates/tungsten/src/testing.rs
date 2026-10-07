//! Headless test harness (`D-110`): steps an [`App`] through the frame body
//! the window loop runs, with no window, renderer, audio device or watcher.
//!
//! Compiled under the umbrella's `testing` feature, off by default. A game
//! turns it on for its tests only:
//!
//! ```toml
//! [dev-dependencies]
//! tungsten = { workspace = true, features = ["testing"] }
//! ```
//!
//! Each frame [`Harness::step`] runs has every stage of a window frame, in
//! the same order: the schedule's stages (`Startup` once, `PreUpdate`,
//! `FixedUpdate`, `Update`, `PostUpdate`), the command flush, event
//! rotation, extract and audio. Render draws nothing: [`Harness::draw`]
//! holds what the frame would have drawn and [`Harness::audio`] what it would
//! have played. Input goes in between steps, as winit events do between
//! redraws. The startup hook does not run and no manifest loads, since both
//! need the renderer; seed resources and assets through
//! [`Harness::world_mut`].
//!
//! The harness's dt is the real clock's: each frame advances
//! [`Time`](tungsten_core::Time) by it, and systems read it through `Time`,
//! times the scale and zero while paused. Pause or scale the game clock
//! through [`Harness::world_mut`], as a game does through its `World`.

#![deny(missing_docs)]

use std::time::Instant;

use tungsten_core::{
    ActionMap, AudioCommand, Binding, EventQueue, InputState, ScrollDirection, World,
};
use tungsten_render::{
    DebugLineInstance, LightUbo, MeshParticleBatch, QuadInstance, SpriteBatch, TextSection,
};

use crate::app::{App, FrameClock, FrameEnd, FrameExtract};

mod ui;

pub use ui::UiHarness;

/// The dt each frame gets until [`Harness::set_dt`] changes it: 60 Hz, as
/// smoke mode pins it.
const DEFAULT_DT_SECS: f32 = 1.0 / 60.0;

/// Runs an [`App`]'s frames one call at a time, with a pinned dt and no
/// window.
pub struct Harness {
    app: App,
    dt: f32,
    draw: Option<FrameDraw>,
    audio: Vec<AudioCommand>,
    frames: u64,
    exit_requested: bool,
}

impl Harness {
    /// Wraps `app` with its schedule resolved, as [`App::run`] does.
    /// Nothing else from startup happens: no window, renderer, audio device,
    /// watcher, startup hook or manifest roots. The `Startup` stage runs on
    /// the first step.
    ///
    /// # Panics
    ///
    /// When the schedule does not resolve: an unknown name, a duplicate or
    /// a cycle, as `App::run` would fail.
    #[must_use]
    pub fn new(mut app: App) -> Self {
        app.resolve_schedule()
            .unwrap_or_else(|err| panic!("Harness::new: {err}"));
        Self {
            app,
            dt: DEFAULT_DT_SECS,
            draw: None,
            audio: Vec::new(),
            frames: 0,
            exit_requested: false,
        }
    }

    /// Sets the real dt, in seconds, that each later frame advances
    /// [`Time`](tungsten_core::Time) by; systems read it through `Time`,
    /// times the scale and zero while paused. It is taken as given: the
    /// window loop's cap does not apply (`D-110`). Zero is allowed.
    ///
    /// # Panics
    ///
    /// When `dt` is NaN, infinite or negative.
    pub fn set_dt(&mut self, dt: f32) {
        assert!(
            dt.is_finite() && dt >= 0.0,
            "Harness::set_dt: dt must be finite and not negative, got {dt}"
        );
        self.dt = dt;
    }

    /// Runs up to `frames` frames and returns how many completed. A frame
    /// whose update leaves `engine_exit` just pressed stops there, before
    /// its later stages, as the window loop exits; after that, no frame
    /// runs.
    pub fn step(&mut self, frames: u32) -> u32 {
        for completed in 0..frames {
            if self.exit_requested {
                return completed;
            }
            self.app.audio_capture = Some(Vec::new());
            let mut draw = None;
            let end = self
                .app
                .run_frame(Instant::now(), FrameClock::Pinned(self.dt), |extract| {
                    draw = Some(FrameDraw::copy_of(extract));
                });
            let audio = self.app.audio_capture.take().unwrap_or_default();
            if end == FrameEnd::ExitRequested {
                self.exit_requested = true;
                return completed;
            }
            self.draw = draw;
            self.audio = audio;
            self.frames += 1;
        }
        frames
    }

    /// Presses `action`'s first binding, as the key, button or wheel event
    /// would between redraws: the next frame reads it as just pressed, and
    /// later frames as held until [`Harness::release_action`]. A scroll
    /// binding lasts one frame, as a wheel notch does.
    ///
    /// # Panics
    ///
    /// When the action map has no binding for `action`.
    pub fn press_action(&mut self, action: &str) {
        let binding = self.first_binding(action);
        let input = self.input_mut();
        match binding {
            Binding::Key { code } => input.key_down(code),
            Binding::Mouse { button } => input.mouse_down(button),
            Binding::Scroll { direction } => {
                let lines = match direction {
                    ScrollDirection::Up => 1.0,
                    ScrollDirection::Down => -1.0,
                };
                input.add_scroll_line_delta(0.0, lines);
            }
        }
    }

    /// Releases `action`'s first binding: the next frame reads it as just
    /// released. A scroll binding has nothing to release.
    ///
    /// # Panics
    ///
    /// When the action map has no binding for `action`.
    pub fn release_action(&mut self, action: &str) {
        let binding = self.first_binding(action);
        let input = self.input_mut();
        match binding {
            Binding::Key { code } => input.key_up(code),
            Binding::Mouse { button } => input.mouse_up(button),
            Binding::Scroll { .. } => {}
        }
    }

    /// Moves the cursor to (`x`, `y`) in window pixels, as a cursor event
    /// would between redraws.
    pub fn set_cursor(&mut self, x: f32, y: f32) {
        self.input_mut().update_cursor_position(x, y);
    }

    /// The app's world.
    #[must_use]
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// The app's world, to seed or change between frames.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// The `T` events readable after the last frame: those sent during it,
    /// which its event flush rotated into the previous window, then any
    /// sent since.
    ///
    /// # Panics
    ///
    /// When `T` is not a registered event type ([`App::register_event`]).
    pub fn events<T: 'static>(&self) -> impl Iterator<Item = &T> {
        self.app
            .world()
            .get_resource::<EventQueue<T>>()
            .unwrap_or_else(|| {
                panic!(
                    "Harness::events: {} is not a registered event type",
                    std::any::type_name::<T>()
                )
            })
            .iter()
    }

    /// What the last completed frame would have drawn; `None` before the
    /// first.
    #[must_use]
    pub fn draw(&self) -> Option<&FrameDraw> {
        self.draw.as_ref()
    }

    /// The audio commands the last completed frame would have played, in
    /// the order its systems queued them.
    #[must_use]
    pub fn audio(&self) -> &[AudioCommand] {
        &self.audio
    }

    /// Whether a frame stopped at an `engine_exit` press.
    #[must_use]
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// The number of frames completed.
    #[must_use]
    pub fn frame(&self) -> u64 {
        self.frames
    }

    fn first_binding(&self, action: &str) -> Binding {
        self.world()
            .get_resource::<ActionMap>()
            .and_then(|actions| actions.bindings(action).first().copied())
            .unwrap_or_else(|| panic!("Harness: action '{action}' has no binding"))
    }

    fn input_mut(&mut self) -> &mut InputState {
        self.world_mut()
            .get_resource_mut::<InputState>()
            .expect("InputState resource missing")
    }
}

/// What a frame would have drawn: the plain data its extract handed the
/// renderer.
#[non_exhaustive]
pub struct FrameDraw {
    /// The quad channel of [`Extracts`](crate::Extracts): the app's
    /// replacement, if it set one, then the contributions.
    pub quads: Vec<QuadInstance>,
    /// The sprite channel's batches: the default (the tilemaps, then the
    /// `Sprite` entities) or the app's replacement, then the contributions.
    pub sprites: Vec<SpriteBatch>,
    /// The text channel's sections, then the HUD, systems overlay and
    /// inspector sections when shown.
    pub text: Vec<TextSection>,
    /// Debug-draw boxes, as outline quads.
    pub debug_quads: Vec<QuadInstance>,
    /// Debug-draw lines and circles.
    pub debug_lines: Vec<DebugLineInstance>,
    /// The frame's lights and ambient color.
    pub light_ubo: LightUbo,
    /// Mesh particles, one batch per mesh.
    pub mesh_particles: Vec<MeshParticleBatch>,
}

impl FrameDraw {
    fn copy_of(extract: &FrameExtract) -> Self {
        Self {
            quads: extract.quads.clone(),
            sprites: extract
                .sprites
                .iter()
                .map(|batch| SpriteBatch {
                    texture: batch.texture,
                    filter: batch.filter,
                    instances: batch.instances.clone(),
                    material_id: batch.material_id,
                    uniform_overrides: batch.uniform_overrides,
                    lit: batch.lit,
                })
                .collect(),
            text: extract.text.clone(),
            debug_quads: extract.debug_quads.clone(),
            debug_lines: extract.debug_lines.clone(),
            light_ubo: extract.light_ubo,
            mesh_particles: extract.mesh_particles.clone(),
        }
    }
}

#[cfg(test)]
#[path = "tests/testing.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/fixed_step.rs"]
mod fixed_step;
