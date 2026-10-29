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
//! redraws. The startup hook does not run and no manifest loads at
//! startup, since both need the renderer; [`Harness::load_manifest`] loads
//! a manifest's headless sections, and the rest seeds through
//! [`Harness::world_mut`].
//!
//! The harness's dt is the real clock's: each frame advances
//! [`Time`](tungsten_core::Time) by it, and systems read it through `Time`,
//! times the scale and zero while paused. Pause or scale the game clock
//! through [`Harness::world_mut`], as a game does through its `World`.
//!
//! A counter guard ([`CounterGuard`]) pins a window's allocations, ECS
//! counts and sprite batches, so a test fails when a change adds work.

#![deny(missing_docs)]

use std::path::Path;
use std::time::Instant;

use tungsten_core::assets::{ResolvedManifest, TextureHandle};
use tungsten_core::script::{Ending, Report, Script};
use tungsten_core::ui::PaintList;
use tungsten_core::{AudioCommand, Binding, EventQueue, InputState, MaterialRegistry, World};
use tungsten_render::{
    DebugLineInstance, LightUbo, MeshParticleBatch, QuadInstance, SpriteBatch, TextSection,
    WorldLabelRun,
};

use crate::app::{App, ExitReason, Exiting, FrameClock, FrameEnd, FrameExtract, WindowSize};
use crate::asset_loader;
use crate::input_bridge;
use crate::script::ScriptRunner;

mod guard;
mod ui;

pub use guard::{Allocations, CounterGuard, CountingAlloc, allocations};
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
    /// Whether [`step`](Self::step) only counts the sprite channel
    /// ([`CounterGuard`]) instead of copying the frame's draw.
    count_only: bool,
    /// The last counted frame's sprite batches and instances.
    sprite_counts: (u64, u64),
    next_texture_handle: u32,
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
            count_only: false,
            sprite_counts: (0, 0),
            next_texture_handle: 0,
        }
    }

    /// A texture handle to register sprites on headless, as a renderer's
    /// [`allocate_texture_handle`](tungsten_render::Renderer::allocate_texture_handle)
    /// gives one: the harness's own counter from 0, with no texture behind
    /// it, since no renderer runs (`D-048`). Sprites registered on one
    /// handle share a batch, as on one atlas page.
    pub fn allocate_texture_handle(&mut self) -> TextureHandle {
        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle += 1;
        handle
    }

    /// Loads the manifest at `path` without the renderer: its animations,
    /// UI themes, sounds, tilemaps and particle configs, in the order a
    /// window's startup loads them; each `materials` entry's metadata (its
    /// ID, source manifest, shader ID and uniform defaults) in the
    /// `MaterialRegistry`, with no upload, so a kit `HitFlash` finds
    /// `damage_flash` headless; then the prefabs and data entries in two
    /// phases, every ID checked ([`load_prefabs`](asset_loader::load_prefabs)).
    /// What needs the renderer is skipped: sprites, fonts, shaders and
    /// particle meshes; a prefab's sprite name is interned at its spawn. A
    /// manifest that fails to load is the error the loader gives.
    ///
    /// Call it after the plugins are built, as the window's startup does,
    /// so the prefabs check against every registered component and data
    /// kind.
    pub fn load_manifest(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let manifest = ResolvedManifest::load(path)?;
        let world = self.app.world_mut();
        asset_loader::load_animations(&manifest, world)?;
        asset_loader::load_ui_themes(&manifest, world)?;
        let mut materials = world
            .remove_resource::<MaterialRegistry>()
            .unwrap_or_default();
        let mut ids: Vec<&String> = manifest.materials.keys().collect();
        ids.sort();
        for id in ids {
            let material = &manifest.materials[id];
            materials.allocate(
                id,
                material.source_manifest.clone(),
                material.shader.clone(),
                material.uniform_defaults,
            );
        }
        world.insert_resource(materials);
        asset_loader::load_sounds(&manifest, world)?;
        asset_loader::load_tilemaps(&manifest, world)?;
        asset_loader::load_particles(&manifest, world)?;
        asset_loader::load_prefabs(&manifest, world)
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
    /// its later stages, as the window loop exits. The last frame after
    /// [`request_exit`](crate::request_exit) or [`request_close`](Self::request_close)
    /// runs whole and counts, its draw and audio kept (`D-180`). After
    /// either end, no frame runs.
    pub fn step(&mut self, frames: u32) -> u32 {
        for completed in 0..frames {
            if self.exit_requested {
                return completed;
            }
            let count_only = self.count_only;
            // Counting reuses the last frame's audio buffer, so the step
            // itself allocates nothing.
            let capture = if count_only {
                let mut buffer = std::mem::take(&mut self.audio);
                buffer.clear();
                buffer
            } else {
                Vec::new()
            };
            self.app.audio_capture = Some(capture);
            let mut draw = None;
            let mut sprite_counts = (0, 0);
            let end = self
                .app
                .run_frame(Instant::now(), FrameClock::Pinned(self.dt), |extract| {
                    if count_only {
                        sprite_counts = guard::sprite_counts(extract);
                    } else {
                        draw = Some(FrameDraw::copy_of(extract));
                    }
                });
            let audio = self.app.audio_capture.take().unwrap_or_default();
            if end == FrameEnd::ExitRequested {
                self.exit_requested = true;
                return completed;
            }
            self.draw = draw;
            self.audio = audio;
            self.sprite_counts = sprite_counts;
            self.frames += 1;
            if end == FrameEnd::Last {
                self.exit_requested = true;
                return completed + 1;
            }
        }
        frames
    }

    /// Asks the window to close, as its close button does (`D-180`): the
    /// next [`step`](Self::step) runs the run's last frame, with
    /// [`Exiting`] whose reason is [`ExitReason::CloseRequested`], or keeps
    /// the `Exiting` a [`request_exit`](crate::request_exit) already set.
    pub fn request_close(&mut self) {
        let world = self.app.world_mut();
        if !world.has_resource::<Exiting>() {
            world.insert_resource(Exiting {
                reason: ExitReason::CloseRequested,
            });
        }
    }

    /// Presses `action`'s first binding, as the key, button, wheel or
    /// gamepad event would between redraws: the next frame reads it as just
    /// pressed, and later frames as held until [`Harness::release_action`].
    /// A scroll binding lasts one frame, as a wheel notch does. A gamepad
    /// binding presses on [`GamepadId::new(0)`](GamepadId::new); a stick
    /// binding tilts its axis fully its way.
    ///
    /// # Panics
    ///
    /// When the action map has no binding for `action`.
    pub fn press_action(&mut self, action: &str) {
        let binding = self.first_binding(action);
        input_bridge::press_binding(self.input_mut(), binding)
            .unwrap_or_else(|err| panic!("Harness: {err}"));
    }

    /// Releases `action`'s first binding: the next frame reads it as just
    /// released. A scroll binding has nothing to release; a stick binding
    /// centres its axis.
    ///
    /// # Panics
    ///
    /// When the action map has no binding for `action`.
    pub fn release_action(&mut self, action: &str) {
        let binding = self.first_binding(action);
        input_bridge::release_binding(self.input_mut(), binding);
    }

    /// Runs `script` on the harness's frames with the window loop's runner
    /// (redo 7.2), from the next frame, and returns its report. Each step
    /// applies where a window run applies it: inputs, actions, spawns,
    /// scenes, the stage, sheets, overlays and `ui` before `PreUpdate`, a
    /// sheet's holds after `Update`, the camera override from the UI layout
    /// through render, captures, probes and asserts after render. With no
    /// renderer a capture is recorded as not written, so a script with one
    /// does not pass here. The run ends after the script's last frame, on
    /// `exit` or on the game's own exit; [`step`](Self::step) after it runs
    /// the game on, unscripted. A `resolution` sets [`WindowSize`] first.
    pub fn run_script(&mut self, script: &Script) -> Report {
        if let Some([width, height]) = script.resolution {
            self.world_mut()
                .insert_resource(WindowSize { width, height });
        }
        self.app.script = Some(ScriptRunner::new(script.clone(), "harness", true));
        let ending = loop {
            let ran = self.step(1);
            if let Some(ended) = self.app.script.as_ref().and_then(ScriptRunner::ended) {
                break ended;
            }
            if ran == 0 || self.exit_requested {
                break Ending::GameExit;
            }
        };
        let runner = self
            .app
            .script
            .take()
            .expect("the runner stays for the run");
        runner.finish(self.app.world_mut(), ending, None)
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

    /// Whether the run has ended: a frame stopped at an `engine_exit` press,
    /// or the last frame after [`request_exit`](crate::request_exit) or
    /// [`request_close`](Self::request_close) ran. No frame runs after it.
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
        input_bridge::first_binding(self.world(), action)
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
    /// The UI paint list the frame drew (M46): the app's retained list,
    /// repainted when the tree, the scale, `UiDebug` or the sprite epoch
    /// changed.
    pub ui: PaintList,
    /// The world labels the frame drew (M54): those pushed to
    /// [`WorldLabels`](tungsten_core::WorldLabels) since the last frame,
    /// projected through the camera, culled and capped.
    pub world_labels: Vec<WorldLabelRun>,
    /// The prewarm list the frame handed the renderer before its prepare
    /// (M56): the text runs of the roots `UiTree::prewarm` asked for, on the
    /// frame they were first laid out; `None` on every other frame.
    pub ui_prewarm: Option<PaintList>,
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
            ui: extract.ui.clone(),
            world_labels: extract.world_labels.clone(),
            ui_prewarm: extract.ui_prewarm.clone(),
        }
    }
}

#[cfg(test)]
#[path = "tests/testing.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/fixed_step.rs"]
mod fixed_step;
