//! Text: the device-free [`TextEngine`] that lays text out and the GPU
//! [`TextPipeline`] that draws it (`D-026`, `D-085`, `D-117`). Layout measures
//! retained nodes through [`TextNodes`].

mod engine;
mod fonts;
mod gpu;

pub use engine::{TextEngine, TextNodes};
pub use fonts::FontSource;
pub use gpu::{TextPipeline, TextStats};
use tungsten_core::text::TextLayout;

/// High-level text draw command. Build one with `..Default::default()` for
/// the fields you leave as they are.
#[derive(Debug, Clone, Default)]
pub struct TextSection {
    pub content: String,
    pub font_id: String,
    pub font_size: f32,
    /// Line height in pixels. One that is not finite and above zero, such as
    /// the default 0, lays out at 1.25 times `font_size`.
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

/// One world label's draw for a frame, which the umbrella's extract projects
/// from a core `WorldLabel`. It lays out on one line at `font_size`, then
/// draws at `scale` with its `pivot` at `position`. Build one from
/// `default()` and field assignments.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldLabelRun {
    /// The label's text.
    pub content: String,
    /// A face or family ID.
    pub font_id: String,
    /// Font size in UI units, before `scale`.
    pub font_size: f32,
    /// Line height in UI units; one that is not finite and above zero lays
    /// out at 1.25 times `font_size`, as for [`TextSection`].
    pub line_height: f32,
    /// The physical pixel the pivot lands on.
    pub position: [f32; 2],
    /// The point of the label's box at `position`, from (0, 0) at its
    /// top-left to (1, 1) at its bottom-right.
    pub pivot: [f32; 2],
    /// Physical pixels per UI unit: the label's scale times the display's.
    pub scale: f32,
    /// RGBA colour.
    pub color: [u8; 4],
}
