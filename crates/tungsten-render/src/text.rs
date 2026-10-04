//! Text: the device-free [`TextEngine`] that lays text out and the GPU
//! [`TextPipeline`] that draws it (`D-026`, `D-085`, `D-117`). Layout measures
//! retained nodes through [`TextNodes`].

mod engine;
mod fonts;
mod gpu;

pub use engine::{TextEngine, TextNodes};
pub use fonts::FontSource;
pub use gpu::TextPipeline;
use tungsten_core::text::TextLayout;

/// High-level text draw command. Build one with `..Default::default()` for
/// the fields you leave as they are.
#[derive(Debug, Clone, Default)]
pub struct TextSection {
    pub content: String,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    /// RGBA color.
    pub color: [u8; 4],
    /// Screen-space top-left position.
    pub position: [f32; 2],
    /// Optional wrap/clip bounds.
    pub bounds: Option<[f32; 2]>,
    /// Alignment, wrapping, overflow, hinting, letter spacing and features;
    /// the default lays out as sections always did.
    pub layout: TextLayout,
}
