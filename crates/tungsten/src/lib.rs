//! Tungsten umbrella crate: app loop, asset loading, input bridge.

pub mod app;
pub mod asset_loader;
pub mod audio;
pub mod camera;
pub mod debug_hud;
mod display;
pub mod hot_reload;
mod input_bridge;
pub mod inspector;
pub mod particles;
pub mod physics_debug;
pub mod sprite_extract;
pub mod state;
pub mod systems_overlay;
pub mod telemetry;
mod tilemap_extract;

pub use app::{App, WindowSize};
pub use camera::camera_update_system;
pub use debug_hud::{hud_toggle_system, DebugHud, HudActiveState, HudCorner, HudRow};
pub use display::request_display_settings;
pub use hot_reload::HotReloadWatcher;
pub use inspector::InspectorState;
pub use particles::{
    particle_count_refresh_system, particle_emit_system, particle_tick_system,
    ParticleBurstEmitted, ParticleSystemDrained,
};
pub use physics_debug::PhysicsDebugOverlay;
pub use sprite_extract::extract_sprites_default;
pub use state::{
    despawn_scene_entities, state_dispatcher_system, GameState, SceneEntity, StateContext, StateId,
    StateStack,
};
pub use systems_overlay::SystemTimingOverlay;
pub use telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};
pub use tilemap_extract::extract_tilemaps;
pub use tungsten_core as core;
pub use tungsten_core::physics;
pub use tungsten_core::{ActionMap, ActionMapError, Binding, DebugDraw, DebugShape, Inspectable};
pub use tungsten_render as render;
