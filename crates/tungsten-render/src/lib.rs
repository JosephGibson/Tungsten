//! wgpu-based rendering backend for the Tungsten 2D engine.
//! Provides a colored-quad pipeline, a textured-sprite pipeline with
//! per-sprite filter modes, and a texture pool keyed by opaque handles.

pub mod quad;
pub mod renderer;
pub mod sprite;

pub use quad::QuadInstance;
pub use renderer::Renderer;
pub use sprite::{SpriteBatch, SpriteInstance, SpritePipeline};
