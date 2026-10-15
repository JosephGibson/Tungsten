//! What the game runs: its plugin (the setup its first frame needs, its
//! systems by stage and its text).

use tungsten::core::{ActionMap, InputState, Settings, Time, Transform, With, World};
use tungsten::render::TextSection;
use tungsten::{App, DebugHud, Extracts, Plugin, Schedule, Stage, StateId, StateStack, system};

use crate::components::Player;
use crate::states::{GAMEPLAY, PAUSE, TITLE, TitleState};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";
/// The manifest's font for the game's text.
pub const FONT: &str = "sans";
/// How far the player moves per second, in pixels.
pub const PLAYER_SPEED: f32 = 240.0;

/// The game as a plugin: the components its prefab files name, `setup` at
/// startup, `player_movement` and `toggle_mute` in `Update`, its text after
/// what the engine draws, and the engine's HUD on in debug builds. It names no engine
/// system; the engine's own come from `DefaultPlugins`, which `App::new`
/// installs.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Player>("player");
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = cfg!(debug_assertions);
        }
        world.resource_mut::<Extracts>().add_text(text);
        schedule.add(Stage::Startup, system("setup", setup));
        schedule.add(Stage::Update, system("player_movement", player_movement));
        schedule.add(Stage::Update, system("toggle_mute", toggle_mute));
    }
}

/// Registers the game with `app`: its plugin.
pub fn register(app: &mut App) {
    app.add_plugin(GamePlugin);
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

/// Moves the player by the move actions while gameplay is on top: a key at
/// full speed, a stick in proportion to its tilt (`ActionMap::value`).
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
        actions.value(input, positive) - actions.value(input, negative)
    };
    let (dx, dy) = (
        axis("move_left", "move_right"),
        axis("move_up", "move_down"),
    );
    let step = PLAYER_SPEED * world.resource::<Time>().delta();
    for transform in world.query_mut_filtered::<&mut Transform, With<Player>>() {
        transform.position.x += dx * step;
        transform.position.y += dy * step;
    }
}

/// Mutes or unmutes on `toggle_mute` and keeps the choice in the player's
/// settings, in the game's user folder, so it survives a restart: the master
/// volume set to 0, or unset to return to full.
pub fn toggle_mute(world: &mut World) {
    let pressed = match (
        world.get_resource::<InputState>(),
        world.get_resource::<ActionMap>(),
    ) {
        (Some(input), Some(actions)) => actions.just_pressed(input, "toggle_mute"),
        _ => false,
    };
    let Some(settings) = world.get_resource_mut::<Settings>().filter(|_| pressed) else {
        return;
    };
    let muted = settings.master_volume().is_some_and(|volume| volume <= 0.0);
    settings.set_master_volume(if muted { None } else { Some(0.0) });
    tungsten::apply_settings(world);
    if let Err(err) = world.resource::<Settings>().save() {
        log::warn!("Settings not saved: {err}");
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
        Some(GAMEPLAY) => &[("Arrows or WASD move · P pauses · M mutes", 24.0)],
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
