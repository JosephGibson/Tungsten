//! wgpu-based rendering backend for the Tungsten 2D engine.
//! Provides a colored-quad pipeline, a textured-sprite pipeline with
//! per-sprite filter modes, a texture pool keyed by opaque handles,
//! and a text pipeline backed by glyphon/cosmic-text.

pub mod quad;
pub mod renderer;
pub mod sprite;
pub mod text;

pub use quad::QuadInstance;
pub use renderer::Renderer;
pub use sprite::{SpriteBatch, SpriteInstance, SpritePipeline};
pub use text::{TextPipeline, TextSection};
