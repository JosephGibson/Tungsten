//! What the game runs: its plugin (the setup its first frame needs and its
//! systems, by stage) and its text.

use tungsten::core::{ActionMap, InputState, Time, Transform, World};
use tungsten::render::TextSection;
use tungsten::{App, DebugHud, Plugin, Schedule, Stage, StateId, StateStack, system};

use crate::components::Player;
use crate::states::{GAMEPLAY, PAUSE, TITLE, TitleState};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";
/// The manifest's font for the game's text.
pub const FONT: &str = "sans";
/// How far the player moves per second, in pixels.
pub const PLAYER_SPEED: f32 = 240.0;

/// The game as a plugin: `setup` at startup, `player_movement` in `Update`,
/// and the engine's HUD on in debug builds. It names no engine system; the
/// engine's own come from `DefaultPlugins`, which `App::new` installs.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = cfg!(debug_assertions);
        }
        schedule.add(Stage::Startup, system("setup", setup));
        schedule.add(Stage::Update, system("player_movement", player_movement));
    }
}

/// Registers the game with `app`: its plugin and its text.
pub fn register(app: &mut App) {
    app.add_plugin(GamePlugin);
    app.set_extract_text(text);
}

/// The startup system: the title state first. The headless harness runs
/// the `Startup` stage on its first step, so tests need not call this.
pub fn setup(world: &mut World) {
    if let Some(stack) = world.get_resource_mut::<StateStack>() {
        stack.request_push(TitleState);
    }
}

/// The state on top of the stack.
#[must_use]
pub fn active_state(world: &World) -> Option<StateId> {
    world
        .get_resource::<StateStack>()
        .and_then(StateStack::active_id)
}

/// Moves the player by the move actions while gameplay is on top.
pub fn player_movement(world: &mut World) {
    if active_state(world) != Some(GAMEPLAY) {
        return;
    }
    let (Some(input), Some(actions)) = (
        world.get_resource::<InputState>(),
        world.get_resource::<ActionMap>(),
    ) else {
        return;
    };
    let axis = |negative: &str, positive: &str| {
        f32::from(u8::from(actions.is_pressed(input, positive)))
            - f32::from(u8::from(actions.is_pressed(input, negative)))
    };
    let (dx, dy) = (
        axis("move_left", "move_right"),
        axis("move_up", "move_down"),
    );
    let step = PLAYER_SPEED * world.get_resource::<Time>().map_or(0.0, Time::delta);
    for (_, _, transform) in world.query2_mut::<Player, Transform>() {
        transform.position.x += dx * step;
        transform.position.y += dy * step;
    }
}

/// The text for the state on top: the title, the controls or the pause notice.
#[must_use]
pub fn text(world: &World) -> Vec<TextSection> {
    let lines: &[(&str, f32)] = match active_state(world) {
        Some(TITLE) => &[
            ("Tungsten template", 56.0),
            ("Enter starts · Esc quits", 24.0),
        ],
        Some(GAMEPLAY) => &[("Arrows or WASD move · P pauses", 24.0)],
        Some(PAUSE) => &[("Paused", 56.0), ("Backspace or P resumes", 24.0)],
        _ => &[],
    };
    let mut y = 240.0;
    lines
        .iter()
        .map(|&(content, size)| {
            let section = TextSection {
                content: content.to_string(),
                font_id: FONT.to_string(),
                font_size: size,
                line_height: size * 1.25,
                color: [235, 240, 255, 255],
                position: [96.0, y],
                ..TextSection::default()
            };
            y += size * 1.5;
            section
        })
        .collect()
}
