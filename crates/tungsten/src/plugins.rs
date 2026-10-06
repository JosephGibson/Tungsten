//! The engine's plugins and `DefaultPlugins` (W15a's design, spiked
//! 2026-10-05). Each registers its systems under the public names below,
//! in the stage the frame map gives it, and the events it sends. `App::new`
//! installs [`DefaultPlugins::set`]; a game that replaces an engine feature
//! builds its app with [`App::with_plugins`](crate::App::with_plugins) and
//! the set without that plugin. The resources every engine feature reads
//! are still inserted by `App`, whichever plugins run.
//!
//! Order across plugins is the set's order; constraints across plugins use
//! `after_if_present`, so leaving a plugin out never fails startup.
//! Constraints within a plugin are required.

use tungsten_core::{
    Plugin, PluginSet, Schedule, ShakeEvent, SquashEvent, Stage, TweenComplete, World, system,
};

use crate::camera::camera_update_system;
use crate::debug_hud::hud_toggle_system;
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
use crate::state::state_dispatcher_system;
use crate::systems_overlay::systems_overlay_toggle_system;
use crate::tweens::tween_tick_system;

pub use tungsten_core::physics::{PHYSICS_STEP, PHYSICS_SYNC, PhysicsPlugin};

/// `PreUpdate`: the physics overlay toggle.
pub const PHYSICS_DEBUG_TOGGLE: &str = "physics_debug_toggle";
/// `PreUpdate`: the systems overlay toggle.
pub const SYSTEMS_OVERLAY_TOGGLE: &str = "systems_overlay_toggle";
/// `PreUpdate`: the inspector toggle.
pub const INSPECTOR_TOGGLE: &str = "inspector_toggle";
/// `PreUpdate`: the inspector's pick, after its toggle.
pub const INSPECTOR_PICK: &str = "inspector_pick";
/// `PreUpdate`: the HUD toggle.
pub const HUD_TOGGLE: &str = "hud_toggle";
/// `PreUpdate`: vsync and fullscreen hotkeys.
pub const DISPLAY_INPUT: &str = "display_input";
/// `PreUpdate`: the state stack's dispatcher, after the display hotkeys.
pub const STATE_DISPATCHER: &str = "state_dispatcher";
/// `PostUpdate`: particle emitter counts, before the emit.
pub const PARTICLE_COUNT_REFRESH: &str = "particle_count_refresh";
/// `PostUpdate`: particle emission, before the tick.
pub const PARTICLE_EMIT: &str = "particle_emit";
/// `PostUpdate`: particle ageing and despawn.
pub const PARTICLE_TICK: &str = "particle_tick";
/// `PostUpdate`: the tween tick, after the particle tick so tween writes win.
pub const TWEEN_TICK: &str = "tween_tick";
/// `PostUpdate`: arms squash envelopes from this frame's `SquashEvent`s.
pub const SQUASH_STRETCH_TRIGGER: &str = "squash_stretch_trigger";
/// `PostUpdate`: advances squash envelopes, after the trigger.
pub const SQUASH_STRETCH_TICK: &str = "squash_stretch_tick";
/// `PostUpdate`: camera trauma from this frame's `ShakeEvent`s.
pub const SHAKE_TICK: &str = "shake_tick";
/// `PostUpdate`: the camera, after the shake tick.
pub const CAMERA_UPDATE: &str = "camera_update";

/// The overlay toggles and the inspector's pick, in `PreUpdate`.
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
            system(INSPECTOR_PICK, inspector_pick_system).after(INSPECTOR_TOGGLE),
        );
        schedule.add(Stage::PreUpdate, system(HUD_TOGGLE, hud_toggle_system));
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

/// The state stack's dispatcher, in `PreUpdate` after the display hotkeys.
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

/// Count refresh, emit and tick in `PostUpdate`, and the burst and drained
/// events.
#[derive(Debug, Default, Clone, Copy)]
pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<ParticleBurstEmitted>();
        world.register_event::<ParticleSystemDrained>();
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
/// tween tick, and the `ShakeEvent` and `SquashEvent` queues (`D-073`).
#[derive(Debug, Default, Clone, Copy)]
pub struct GameFeelPlugin;

impl Plugin for GameFeelPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<ShakeEvent>();
        world.register_event::<SquashEvent>();
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
    /// Debug, display, state, physics, particles, tweens, game feel, camera.
    #[must_use]
    pub fn set() -> PluginSet {
        PluginSet::new()
            .with(DebugPlugin)
            .with(DisplayPlugin)
            .with(StatePlugin)
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
