//! What the game runs: its plugin (the setup its first frame needs, its
//! systems by stage and its text). The player's movement, health, hit flash,
//! knockback and camera and the hazard's patrol are the kit's: components
//! named on their prefabs, run by `KitPlugins::set()`, with no game Rust for
//! them. The hazard's touch is the game's `hazard_touch`, until the kit's
//! contact damage takes it into data.

use tungsten::core::{
    ActionMap, CameraController, Entity, InputState, Position, Settings, With, World,
};
use tungsten::kit::{Hit, KitPlugins, Phase, damage, ensure_phases, knock_back};
use tungsten::render::TextSection;
use tungsten::{
    App, DebugHud, Extracts, HudCorner, Plugin, Schedule, Stage, StateId, StateStack, system,
};

use crate::components::{Hazard, Player};
use crate::states::{GAMEPLAY, PAUSE, TITLE, TitleState};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";
/// The manifest's font for the game's text.
pub const FONT: &str = "sans";
/// The camera's dead zone, in world pixels: the player roams the middle
/// of a 1280 × 720 view before the camera follows, so its movement shows
/// against the view. A level with a world to move against can shrink it.
pub const CAMERA_DEAD_ZONE: [f32; 2] = [640.0, 360.0];

/// The game as a plugin: the components its prefab files name, `setup` at
/// startup, `toggle_mute` in `Update`, [`hazard_touch`] in the kit's overlap
/// phase, its text after what the engine draws, the camera's dead zone, and
/// the engine's HUD on in debug builds, at the top right, clear of the
/// title. It names no engine system; the engine's own come from
/// `DefaultPlugins`, which `App::new` installs, and the kit's from
/// `KitPlugins::set()`, which [`register`] adds before this plugin.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Player>("player");
        world.register_component::<Hazard>("hazard");
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = cfg!(debug_assertions);
            // The HUD docks at the top left by default, over the title.
            hud.corner = HudCorner::TopRight;
        }
        world.resource_mut::<Extracts>().add_text(text);
        if let Some(camera) = world.get_resource_mut::<CameraController>() {
            camera.dead_zone_size = CAMERA_DEAD_ZONE.into();
        }
        schedule.add(Stage::Startup, system("setup", setup));
        schedule.add(Stage::Update, system("toggle_mute", toggle_mute));
        // After the step's motion, on the fixed step, beside the kit's own
        // overlap checks.
        ensure_phases(schedule);
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system("hazard_touch", hazard_touch)),
        );
    }
}

/// Registers the game with `app`: the kit's plugins, then the game's. The
/// kit reads the player prefab's `action_input` into the move actions,
/// moves its `top_down_mover` on the fixed step, keeps its `health`,
/// flashes its `hit_flash` and runs its `knockback` on a hit, and points the
/// camera at its `camera_target`; it swings the hazard's `oscillator`.
pub fn register(app: &mut App) {
    app.add_plugins(KitPlugins::set());
    app.add_plugin(GamePlugin);
}

/// A hazard within its radius of the player hurts it, and a hit that lands
/// pushes the player away with `tungsten::kit::knock_back`, so the
/// player's invulnerability window paces the hits: `damage` ignores a touch
/// inside it. It runs in the kit's overlap phase, after the step's motion.
pub fn hazard_touch(world: &mut World) {
    let Some((player, at)) = world
        .query_filtered::<(Entity, &Position), With<Player>>()
        .next()
        .map(|(entity, position)| (entity, position.0))
    else {
        return;
    };
    let touching: Vec<_> = world
        .query::<(&Position, &Hazard)>()
        .filter(|(position, hazard)| position.0.distance(at) <= hazard.radius)
        .map(|(position, hazard)| (position.0, *hazard))
        .collect();
    for (from, hazard) in touching {
        if damage(world, player, hazard.damage) == Hit::Hurt {
            // Away from the hazard; straight up when the two points meet.
            let away = (at - from).normalize_or(Position::new(0.0, -1.0).0);
            knock_back(world, player, away * hazard.push);
        }
    }
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
