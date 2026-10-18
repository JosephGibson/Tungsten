//! What the game runs: its plugin (the setup its first frame needs, its
//! systems by stage and its text). The player's movement, health, hit flash,
//! knockback and camera, the hazard's patrol, touch and slow, and the gems'
//! drift and count are the kit's: components named on their prefabs, run by
//! `KitPlugins::set()`, with no game Rust for them.

use tungsten::core::{ActionMap, CameraController, InputState, Settings, World};
use tungsten::kit::KitPlugins;
use tungsten::render::TextSection;
use tungsten::{
    App, DebugHud, Extracts, HudCorner, Plugin, Schedule, Stage, StateId, StateStack, system,
};

use crate::components::Player;
use crate::states::{GAMEPLAY, PAUSE, TITLE, TitleState};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";
/// The manifest's font for the game's text.
pub const FONT: &str = "sans";
/// The camera's dead zone, in world pixels: the player roams the middle
/// of a 1280 × 720 view before the camera follows, so its movement shows
/// against the view. A level with a world to move against can shrink it.
pub const CAMERA_DEAD_ZONE: [f32; 2] = [640.0, 360.0];

/// The game as a plugin: the component its prefab files name, `setup` at
/// startup, `toggle_mute` in `Update`, its text after what the engine draws,
/// the camera's dead zone, and the engine's HUD on in debug builds, at the
/// top right, clear of the title. It names no engine system; the engine's
/// own come from `DefaultPlugins`, which `App::new` installs, and the kit's
/// from `KitPlugins::set()`, which [`register`] adds before this plugin.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Player>("player");
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
    }
}

/// Registers the game with `app`: the kit's plugins, then the game's. The
/// kit reads the player prefab's `action_input` into the move actions,
/// moves its `top_down_mover` on the fixed step, keeps its `health`,
/// flashes its `hit_flash` and runs its `knockback` on a hit, and points the
/// camera at its `camera_target`; it swings the hazard's `oscillator` and
/// hurts the player's `hurtbox` through the hazard's `contact_damage`, once
/// per interval while they touch, pushing it away, and the hazard's `hit`
/// cue applies the `slowed` effect to the player, whose `move_speed` stat
/// its modifier halves for the effect's seconds; it draws each gem's
/// `pickup` to the player whose `pickup_radius` stat reaches it and counts
/// it on the game's `Counters` through the gem's `picked_up` cue.
pub fn register(app: &mut App) {
    app.add_plugins(KitPlugins::set());
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
