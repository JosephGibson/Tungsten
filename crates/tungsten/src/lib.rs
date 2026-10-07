//! Tungsten umbrella crate: app loop, asset loading, input bridge.

pub mod app;
pub mod asset_loader;
pub mod audio;
pub mod camera;
mod crash;
pub mod debug_hud;
mod display;
mod engine_font;
pub mod game_feel;
pub mod hot_reload;
mod input_bridge;
pub mod inspector;
pub mod light_extract;
mod logging;
pub mod particles;
pub mod physics_debug;
pub mod plugins;
pub mod post_aa;
pub mod sprite_extract;
pub mod state;
pub mod systems_overlay;
pub mod telemetry;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod tilemap_extract;
pub mod transition;
pub mod tweens;
mod user_dir;

pub use app::{App, WindowSize};
pub use camera::camera_update_system;
pub use debug_hud::{DebugHud, HudActiveState, HudCorner, HudRow, hud_toggle_system};
pub use display::request_display_settings;
pub use engine_font::ENGINE_FONT_ID;
pub use game_feel::{shake_tick_system, squash_stretch_tick_system, squash_stretch_trigger_system};
pub use hot_reload::HotReloadWatcher;
pub use inspector::InspectorState;
pub use light_extract::extract_lights;
pub use particles::{
    ParticleBurstEmitted, ParticleSystemDrained, extract_mesh_particles,
    particle_count_refresh_system, particle_emit_system, particle_tick_system,
    spawn_mesh_particle_via,
};
pub use physics_debug::PhysicsDebugOverlay;
pub use plugins::{
    CameraPlugin, DebugPlugin, DefaultPlugins, DisplayPlugin, GameFeelPlugin, ParticlesPlugin,
    PhysicsPlugin, StatePlugin, TweensPlugin,
};
pub use post_aa::{PostAaState, request_post_aa};
pub use sprite_extract::extract_sprites_default;
pub use state::{
    GameState, SceneEntity, StateContext, StateId, StateStack, despawn_scene_entities,
    state_dispatcher_system,
};
pub use systems_overlay::SystemTimingOverlay;
pub use telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};
pub use tilemap_extract::extract_tilemaps;
pub use transition::{Transition, TransitionEffect, TransitionPhase, TransitionState};
pub use tungsten_core as core;
pub use tungsten_core::physics;
pub use tungsten_core::{
    ActionMap, ActionMapError, Binding, Bundle, DebugDraw, DebugShape, Inspectable, OptionalColumn,
    Plugin, PluginSet, Schedule, ScheduleError, Stage, SystemDesc, With, Without, system,
};
pub use tungsten_render as render;
pub use tweens::tween_tick_system;
