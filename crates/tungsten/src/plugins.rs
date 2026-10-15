//! The engine's plugins and `DefaultPlugins` (W15a's design, spiked
//! 2026-10-05). Each registers its systems under the public names below,
//! which core defines in `tungsten_core::schedule::names` so a crate on core
//! alone can name them, in the stage the frame map gives it, and the events
//! it sends. `App::new`
//! installs [`DefaultPlugins::set`]; a game that replaces an engine feature
//! builds its app with [`App::with_plugins`](crate::App::with_plugins) and
//! the set without that plugin. The resources every engine feature reads
//! are still inserted by `App`, whichever plugins run.
//!
//! Order across plugins is the set's order; constraints across plugins use
//! `after_if_present`, so leaving a plugin out never fails startup.
//! Constraints within a plugin are required.

use tungsten_core::ui::{ui_animate_system, ui_route_system};
use tungsten_core::{
    Plugin, PluginSet, Schedule, ShakeEvent, SquashEvent, Stage, TweenComplete, UiEvent, World,
    system,
};

use crate::camera::camera_update_system;
use crate::debug_hud::hud_toggle_system;
use crate::debug_views::{log_console_toggle_system, ui_inspector_toggle_system};
use crate::display::engine_display_input_system;
use crate::game_feel::{
    shake_tick_system, squash_stretch_tick_system, squash_stretch_trigger_system,
};
use crate::inspector::{inspector_pick_system, inspector_toggle_system};
use crate::particles::{
    ParticleBurstEmitted, ParticleSystemDrained, particle_count_refresh_system,
    particle_emit_system, particle_tick_system,
};
use crate::physics_debug::physics_debug_toggle_system;
use crate::state::{state_dispatcher_system, ui_state_roots_system};
use crate::systems_overlay::systems_overlay_toggle_system;
use crate::tweens::tween_tick_system;
use crate::ui_debug::ui_debug_toggle_system;

pub use tungsten_core::physics::{
    PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PHYSICS_SYNC, PhysicsPlugin,
};
pub use tungsten_core::schedule::names::{
    CAMERA_UPDATE, DISPLAY_INPUT, HUD_TOGGLE, INSPECTOR_PICK, INSPECTOR_TOGGLE, LOG_CONSOLE_TOGGLE,
    PARTICLE_COUNT_REFRESH, PARTICLE_EMIT, PARTICLE_TICK, PHYSICS_DEBUG_TOGGLE, SHAKE_TICK,
    SQUASH_STRETCH_TICK, SQUASH_STRETCH_TRIGGER, STATE_DISPATCHER, SYSTEMS_OVERLAY_TOGGLE,
    TWEEN_TICK, UI_ANIMATE, UI_DEBUG_TOGGLE, UI_INSPECTOR_TOGGLE, UI_ROUTE, UI_STATE_ROOTS,
};

/// The overlay toggles, the inspector's pick, the UI debug toggles and the
/// UI inspector's and log console's toggles (M57), in `PreUpdate`. The
/// pick runs after the UI's routing when it is installed (M57), so it reads
/// the input the UI did not take.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(
            Stage::PreUpdate,
            system(PHYSICS_DEBUG_TOGGLE, physics_debug_toggle_system),
        );
        schedule.add(
            Stage::PreUpdate,
            system(SYSTEMS_OVERLAY_TOGGLE, systems_overlay_toggle_system),
        );
        schedule.add(
            Stage::PreUpdate,
            system(INSPECTOR_TOGGLE, inspector_toggle_system),
        );
        schedule.add(
            Stage::PreUpdate,
            system(INSPECTOR_PICK, inspector_pick_system)
                .after(INSPECTOR_TOGGLE)
                .after_if_present(UI_ROUTE),
        );
        schedule.add(Stage::PreUpdate, system(HUD_TOGGLE, hud_toggle_system));
        schedule.add(
            Stage::PreUpdate,
            system(UI_DEBUG_TOGGLE, ui_debug_toggle_system),
        );
        schedule.add(
            Stage::PreUpdate,
            system(UI_INSPECTOR_TOGGLE, ui_inspector_toggle_system),
        );
        schedule.add(
            Stage::PreUpdate,
            system(LOG_CONSOLE_TOGGLE, log_console_toggle_system),
        );
    }
}

/// The display hotkeys, in `PreUpdate`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(
            Stage::PreUpdate,
            system(DISPLAY_INPUT, engine_display_input_system),
        );
    }
}

/// The state stack's dispatcher, in `PreUpdate` after the display hotkeys
/// (and the UI's systems, which order themselves before it).
#[derive(Debug, Default, Clone, Copy)]
pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(
            Stage::PreUpdate,
            system(STATE_DISPATCHER, state_dispatcher_system).after_if_present(DISPLAY_INPUT),
        );
    }
}

/// The UI in `PreUpdate`, before the state dispatcher (M56): the state
/// stack's roots synced for a transition queued since the dispatcher, the
/// input routing, then the animation tracks; and the `UiEvent` queue
/// (`D-149`). A state's `update` reads this frame's routing. Without it
/// the UI takes no input and `RoutedInput` stays empty.
#[derive(Debug, Default, Clone, Copy)]
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<UiEvent>();
        schedule.add(
            Stage::PreUpdate,
            system(UI_STATE_ROOTS, ui_state_roots_system).before(UI_ROUTE),
        );
        schedule.add(
            Stage::PreUpdate,
            system(UI_ROUTE, ui_route_system).before_if_present(STATE_DISPATCHER),
        );
        // Both constraints: the scheduler runs the earliest-registered ready
        // system, and the state plugin registers first.
        schedule.add(
            Stage::PreUpdate,
            system(UI_ANIMATE, ui_animate_system)
                .after(UI_ROUTE)
                .before_if_present(STATE_DISPATCHER),
        );
    }
}

/// Count refresh, emit and tick in `PostUpdate`, the burst and drained
/// events, and the `particle_emitter` scene and prefab component, checked
/// against the World's particle configs.
#[derive(Debug, Default, Clone, Copy)]
pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<ParticleBurstEmitted>();
        world.register_event::<ParticleSystemDrained>();
        crate::asset_loader::register_particle_emitter(world);
        schedule.add(
            Stage::PostUpdate,
            system(PARTICLE_COUNT_REFRESH, particle_count_refresh_system)
                .after_if_present(PHYSICS_SYNC),
        );
        schedule.add(
            Stage::PostUpdate,
            system(PARTICLE_EMIT, particle_emit_system).after(PARTICLE_COUNT_REFRESH),
        );
        schedule.add(
            Stage::PostUpdate,
            system(PARTICLE_TICK, particle_tick_system).after(PARTICLE_EMIT),
        );
    }
}

/// The tween tick in `PostUpdate`, after the particle tick, and `TweenComplete`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TweensPlugin;

impl Plugin for TweensPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<TweenComplete>();
        schedule.add(
            Stage::PostUpdate,
            system(TWEEN_TICK, tween_tick_system)
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(PARTICLE_TICK),
        );
    }
}

/// Squash trigger and tick and the shake tick in `PostUpdate`, after the
/// tween tick, the `ShakeEvent` and `SquashEvent` queues (`D-073`) and the
/// `squash_stretch` scene component.
#[derive(Debug, Default, Clone, Copy)]
pub struct GameFeelPlugin;

impl Plugin for GameFeelPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<ShakeEvent>();
        world.register_event::<SquashEvent>();
        crate::asset_loader::register_squash_stretch(world);
        schedule.add(
            Stage::PostUpdate,
            system(SQUASH_STRETCH_TRIGGER, squash_stretch_trigger_system)
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(TWEEN_TICK),
        );
        schedule.add(
            Stage::PostUpdate,
            system(SQUASH_STRETCH_TICK, squash_stretch_tick_system).after(SQUASH_STRETCH_TRIGGER),
        );
        schedule.add(
            Stage::PostUpdate,
            system(SHAKE_TICK, shake_tick_system)
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(TWEEN_TICK),
        );
    }
}

/// The camera update in `PostUpdate`, after the shake tick and the sync.
#[derive(Debug, Default, Clone, Copy)]
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(
            Stage::PostUpdate,
            system(CAMERA_UPDATE, camera_update_system)
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(TWEEN_TICK)
                .after_if_present(SHAKE_TICK),
        );
    }
}

/// The engine's default plugins, in the order they register.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPlugins;

impl DefaultPlugins {
    /// Debug, display, state, UI, physics, particles, tweens, game feel,
    /// camera.
    #[must_use]
    pub fn set() -> PluginSet {
        PluginSet::new()
            .with(DebugPlugin)
            .with(DisplayPlugin)
            .with(StatePlugin)
            .with(UiPlugin)
            .with(PhysicsPlugin)
            .with(ParticlesPlugin)
            .with(TweensPlugin)
            .with(GameFeelPlugin)
            .with(CameraPlugin)
    }
}

#[cfg(test)]
#[path = "tests/plugins.rs"]
mod tests;
