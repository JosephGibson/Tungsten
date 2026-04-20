//! wgpu-based rendering backend for the Tungsten 2D engine.
//! Provides a colored-quad pipeline, a textured-sprite pipeline with
//! per-sprite filter modes, a texture pool keyed by opaque handles,
//! and a text pipeline backed by glyphon/cosmic-text.

pub mod debug_line;
pub mod image_diff;
pub mod quad;
pub mod renderer;
pub mod screenshot;
pub mod sprite;
pub mod text;

pub use debug_line::{DebugLineInstance, DebugLinePipeline};
pub use image_diff::{compare_png, DiffReport, ImageDiffError};
pub use quad::QuadInstance;
pub use renderer::{CpuFrameTimings, GpuFrameTimings, Renderer};
pub use screenshot::ScreenshotError;
pub use sprite::{SpriteBatch, SpriteInstance, SpritePipeline};
pub use text::{TextPipeline, TextSection};
