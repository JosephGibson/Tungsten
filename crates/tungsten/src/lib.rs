//! Umbrella crate for the Tungsten 2D engine. Ties together
//! [`tungsten_core`] and [`tungsten_render`] with a winit-driven
//! application loop, asset loading helpers, and input bridging.

pub mod app;
pub mod asset_loader;
pub mod audio;
mod input_bridge;

pub use app::{App, WindowSize};
pub use tungsten_core as core;
pub use tungsten_render as render;
