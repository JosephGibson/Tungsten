//! Umbrella crate for the Tungsten 2D engine. Ties together
//! [`tungsten_core`] and [`tungsten_render`] with a winit-driven
//! application loop, asset loading helpers, and input bridging.

pub mod app;
pub mod asset_loader;
pub mod audio;
pub mod hot_reload;
mod input_bridge;
pub mod telemetry;
mod tilemap_extract;

pub use app::{App, WindowSize};
pub use hot_reload::HotReloadWatcher;
pub use telemetry::FrameTimings;
pub use tilemap_extract::extract_tilemaps;
pub use tungsten_core as core;
pub use tungsten_core::physics;
pub use tungsten_render as render;
