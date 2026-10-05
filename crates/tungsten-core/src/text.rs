//! Neutral text types shared by the text engine, layout and the paint list
//! (`D-117`). Core defines them and the [`TextMeasure`] trait; the renderer's
//! text engine implements it and mints [`TextNodeId`]s (`D-007`, `D-048`).

use glam::Vec2;

/// Horizontal alignment of each line inside its box, in logical order.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    /// The line's start edge: left for left-to-right text.
    #[default]
    Start,
    /// Centred between both edges.
    Center,
    /// The line's end edge: right for left-to-right text.
    End,
    /// Stretched to both edges, except a paragraph's last line.
    Justified,
}

/// Where lines break when text is wider than its box.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TextWrap {
    /// Never break; a line runs past its box.
    None,
    /// Break between any two glyphs.
    Glyph,
    /// Break between words only; a word wider than the box overflows it.
    Word,
    /// Break between words, and inside a word too wide for a line of its own.
    #[default]
    WordOrGlyph,
}

/// Which part of a line an ellipsis replaces.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EllipsisAt {
    /// The start of the line.
    Start,
    /// The middle of the line.
    Middle,
    /// The end of the line.
    End,
}

/// What happens to text that does not fit its box.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TextOverflow {
    /// Lines past the box are clipped when drawn.
    #[default]
    Clip,
    /// The last line that fits ends in `…`.
    Ellipsis {
        /// Which part of that line the ellipsis replaces.
        at: EllipsisAt,
        /// Lines kept before the ellipsis. The limit counts lines per
        /// paragraph: each newline starts a new count. `None` keeps the lines
        /// that fit the box height: a section's bounds, or a node's committed
        /// size; a measurement, which has no height, keeps every line.
        lines: Option<u32>,
    },
}

/// Whether glyph positions snap to whole pixels during layout.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TextHinting {
    /// Subpixel positions.
    #[default]
    Disabled,
    /// Horizontal positions snap to whole pixels.
    Enabled,
}

/// One OpenType feature setting, such as `tnum` = 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFeature {
    /// The four-byte feature tag, such as `*b"tnum"`.
    pub tag: [u8; 4],
    /// The feature's value: 0 turns it off, 1 on; some features take more.
    pub value: u16,
}

/// OpenType feature settings applied to shaping, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FontFeatures(Vec<FontFeature>);

impl FontFeatures {
    /// No feature settings: each font's defaults apply.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Tabular figures (`tnum` = 1): every digit takes the same advance, so
    /// counters don't shift as they change.
    #[must_use]
    pub fn tabular_figures() -> Self {
        Self::new().with(*b"tnum", 1)
    }

    /// Sets `tag` to `value`, replacing an earlier setting of the same tag.
    #[must_use]
    pub fn with(mut self, tag: [u8; 4], value: u16) -> Self {
        self.set(tag, value);
        self
    }

    /// Sets `tag` to `value`, replacing an earlier setting of the same tag.
    pub fn set(&mut self, tag: [u8; 4], value: u16) {
        match self.0.iter_mut().find(|feature| feature.tag == tag) {
            Some(feature) => feature.value = value,
            None => self.0.push(FontFeature { tag, value }),
        }
    }

    /// The settings, in the order they were first set.
    #[must_use]
    pub fn as_slice(&self) -> &[FontFeature] {
        &self.0
    }

    /// True when nothing is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// How a block of text lays out inside its box. The default reproduces the
/// layout every section had before `TextLayout` existed.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextLayout {
    /// Line alignment.
    pub align: TextAlign,
    /// Line breaking.
    pub wrap: TextWrap,
    /// Text that does not fit.
    pub overflow: TextOverflow,
    /// Pixel snapping of glyph positions.
    pub hinting: TextHinting,
    /// Extra space after each glyph, in em (multiples of the font size).
    pub letter_spacing: f32,
    /// OpenType feature settings.
    pub features: FontFeatures,
}

impl TextLayout {
    /// This layout with `align`.
    #[must_use]
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// This layout with `wrap`.
    #[must_use]
    pub fn with_wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// This layout with `overflow`.
    #[must_use]
    pub fn with_overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// This layout with `hinting`.
    #[must_use]
    pub fn with_hinting(mut self, hinting: TextHinting) -> Self {
        self.hinting = hinting;
        self
    }

    /// This layout with `letter_spacing`, in em.
    #[must_use]
    pub fn with_letter_spacing(mut self, letter_spacing: f32) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }

    /// This layout with `features`.
    #[must_use]
    pub fn with_features(mut self, features: FontFeatures) -> Self {
        self.features = features;
        self
    }
}

/// The base style of a block of text; spans may override parts of it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// A `font_families` ID from the manifest (`D-115`).
    pub family: String,
    /// Font weight, 100–900; 400 is regular and 700 bold.
    pub weight: u16,
    /// Whether to use the family's italic face.
    pub italic: bool,
    /// Font size in pixels.
    pub size: f32,
    /// Line height in pixels.
    pub line_height: f32,
    /// RGBA colour.
    pub color: [u8; 4],
    /// Alignment, wrapping, overflow, hinting, spacing and features.
    pub layout: TextLayout,
}

impl TextStyle {
    /// Regular, upright, white text in `family` with the default layout.
    #[must_use]
    pub fn new(family: impl Into<String>, size: f32, line_height: f32) -> Self {
        Self {
            family: family.into(),
            weight: 400,
            italic: false,
            size,
            line_height,
            color: [255, 255, 255, 255],
            layout: TextLayout::default(),
        }
    }

    /// This style with `weight`.
    #[must_use]
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    /// This style, italic or upright.
    #[must_use]
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// This style with `color`.
    #[must_use]
    pub fn with_color(mut self, color: [u8; 4]) -> Self {
        self.color = color;
        self
    }

    /// This style with `layout`.
    #[must_use]
    pub fn with_layout(mut self, layout: TextLayout) -> Self {
        self.layout = layout;
        self
    }
}

/// A run of text; each `None` takes the [`TextStyle`]'s value.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextSpan {
    /// The run's text.
    pub text: String,
    /// Weight override.
    pub weight: Option<u16>,
    /// Italic override.
    pub italic: Option<bool>,
    /// RGBA colour override.
    pub color: Option<[u8; 4]>,
}

impl TextSpan {
    /// A span of `text` with no overrides.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// This span with a weight override.
    #[must_use]
    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = Some(weight);
        self
    }

    /// This span with an italic override.
    #[must_use]
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = Some(italic);
        self
    }

    /// This span with a colour override.
    #[must_use]
    pub fn with_color(mut self, color: [u8; 4]) -> Self {
        self.color = Some(color);
        self
    }
}

/// Text made of spans, in reading order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyledText {
    /// The spans, concatenated when laid out.
    pub spans: Vec<TextSpan>,
}

impl From<&str> for StyledText {
    /// One span with no overrides.
    fn from(text: &str) -> Self {
        Self {
            spans: vec![TextSpan::new(text)],
        }
    }
}

impl From<String> for StyledText {
    /// One span with no overrides.
    fn from(text: String) -> Self {
        Self {
            spans: vec![TextSpan::new(text)],
        }
    }
}

/// The width a measurement may use, as layout's available space.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasureWidth {
    /// The narrowest width the text can take without overflowing: its widest
    /// word, or glyph under [`TextWrap::Glyph`].
    MinContent,
    /// The width of the text with no wrapping.
    MaxContent,
    /// A fixed width in pixels.
    Definite(f32),
}

/// The laid-out size of a text node.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextMetrics {
    /// Width and height in pixels.
    pub size: Vec2,
    /// Distance from the top to the first line's baseline, in pixels.
    pub first_baseline: f32,
    /// Lines laid out.
    pub line_count: u32,
}

impl TextMetrics {
    /// Metrics with these values.
    #[must_use]
    pub const fn new(size: Vec2, first_baseline: f32, line_count: u32) -> Self {
        Self {
            size,
            first_baseline,
            line_count,
        }
    }
}

/// A retained text node. The text engine mints IDs; a removed node's ID never
/// names the node that later takes its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextNodeId {
    index: u32,
    generation: u32,
}

impl TextNodeId {
    /// The ID of slot `index` at `generation`. For the engine that mints them.
    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// The slot index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The slot's generation when the ID was minted.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// A count of font changes: it rises whenever a font load, a reload or a
/// change of families or the fallback chain could change shaping. A layout
/// made under an older epoch is stale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontEpoch(u64);

impl FontEpoch {
    /// The epoch `value`. For the engine that counts them.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The epoch after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Measures retained text nodes for layout. The renderer's text engine
/// implements it with the fonts, shaping and wrapping that draw the text.
pub trait TextMeasure {
    /// The size of `node` laid out in `available` width. `known_width` is a
    /// width layout has already fixed; it wins over `available`. A stale or
    /// removed node measures [`TextMetrics::default()`].
    fn measure(
        &mut self,
        node: TextNodeId,
        known_width: Option<f32>,
        available: MeasureWidth,
    ) -> TextMetrics;
}

/// Retained text nodes for layout (`D-125`): creation, text and style, the
/// committed box and the font epoch, beside [`TextMeasure`]'s measurement.
/// The renderer's text engine and the facade it lends implement it; core's
/// UI tree drives its labels through it and never calls render (`D-007`).
pub trait TextNodeStore: TextMeasure {
    /// The current font epoch. A commit made under an older one is stale,
    /// and [`Self::committed`] no longer returns it.
    fn font_epoch(&self) -> FontEpoch;

    /// A new node with no text; it measures [`TextMetrics::default()`] until
    /// [`Self::set_text`].
    fn create_node(&mut self) -> TextNodeId;

    /// Removes `node`. Its ID never names a node again; a stale ID does
    /// nothing.
    fn remove_node(&mut self, node: TextNodeId);

    /// Sets `node`'s text and base style. Equal input does no work; any
    /// other text or style drops the node's layout and its commit.
    fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle);

    /// Lays `node` out in its final box, `size.x` wide and `size.y` high,
    /// and keeps that layout for drawing.
    fn commit_layout(&mut self, node: TextNodeId, size: Vec2) -> TextMetrics;

    /// The metrics of `node`'s last commit while it still holds, under the
    /// current font epoch and with no later text or style change.
    fn committed(&self, node: TextNodeId) -> Option<TextMetrics>;
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
