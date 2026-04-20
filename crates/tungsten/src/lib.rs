//! Umbrella crate for the Tungsten 2D engine. Ties together
//! [`tungsten_core`] and [`tungsten_render`] with a winit-driven
//! application loop, asset loading helpers, and input bridging.

pub mod app;
pub mod asset_loader;
pub mod audio;
pub mod camera;
pub mod debug_hud;
mod display;
pub mod hot_reload;
mod input_bridge;
pub mod sprite_extract;
pub mod state;
pub mod telemetry;
mod tilemap_extract;

pub use app::{App, WindowSize};
pub use camera::camera_update_system;
pub use debug_hud::{hud_toggle_system, DebugHud, HudActiveState, HudCorner, HudRow};
pub use display::request_display_settings;
pub use hot_reload::HotReloadWatcher;
pub use sprite_extract::extract_sprites_default;
pub use state::{
    despawn_scene_entities, state_dispatcher_system, GameState, SceneEntity, StateContext, StateId,
    StateStack,
};
pub use telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};
pub use tilemap_extract::extract_tilemaps;
pub use tungsten_core as core;
pub use tungsten_core::physics;
pub use tungsten_core::{ActionMap, ActionMapError, Binding};
pub use tungsten_render as render;
