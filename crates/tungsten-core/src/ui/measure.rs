//! A [`TextNodeStore`] with no fonts, for headless layout tests (`D-125`).
//! Every glyph advances the same fraction of the font size, so a test can
//! predict every width by counting characters.

use glam::Vec2;

use crate::text::{
    FontEpoch, MeasureWidth, StyledText, TextMeasure, TextMetrics, TextNodeId, TextNodeStore,
    TextStyle, TextWrap,
};

/// Where the first baseline sits in a line, as a fraction of the line height.
const BASELINE_RATIO: f32 = 0.8;

/// Counts the double keeps. Each counts every call, stale IDs included, so a
/// test can check that a layout pass measured, set and committed what it
/// should and nothing more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeasureCounts {
    /// [`TextMeasure::measure`] calls.
    pub measure_calls: u32,
    /// [`TextNodeStore::set_text`] calls, equal input included.
    pub set_text_calls: u32,
    /// [`TextNodeStore::commit_layout`] calls.
    pub commits: u32,
}

struct NodeData {
    text: StyledText,
    style: TextStyle,
    /// The last commit and the epoch it was made under.
    committed: Option<(FontEpoch, TextMetrics)>,
}

struct Slot {
    generation: u32,
    live: bool,
    node: Option<NodeData>,
}

/// A text store with no fonts: a glyph advances `size × advance_em` plus the
/// style's letter spacing, newlines end lines, lines wrap greedily per
/// [`TextWrap`] (a word wider than the box overflows under `Word` and breaks
/// inside under `WordOrGlyph`), height is `lines × line_height`, the first
/// baseline sits at 0.8 of the line height, empty text is one line, and
/// nothing is rounded. Overflow and ellipses are ignored. Min-content is the
/// widest word, the widest glyph under `Glyph`, or the unwrapped width under
/// `None`, as the engine computes it. A stale ID or a node without text
/// measures [`TextMetrics::default()`]. Public, since games' headless tests
/// lay out with it too.
#[derive(Debug)]
pub struct FixedAdvanceMeasure {
    advance_em: f32,
    epoch: FontEpoch,
    slots: Vec<Slot>,
    free: Vec<u32>,
    /// Calls so far.
    pub counts: MeasureCounts,
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("generation", &self.generation)
            .field("live", &self.live)
            .field("has_text", &self.node.is_some())
            .finish()
    }
}

impl Default for FixedAdvanceMeasure {
    /// Advance 0.5 em: 8 px at size 16.
    fn default() -> Self {
        Self::new(0.5)
    }
}

impl FixedAdvanceMeasure {
    /// A store whose glyphs advance `advance_em` of the font size.
    #[must_use]
    pub fn new(advance_em: f32) -> Self {
        Self {
            advance_em,
            epoch: FontEpoch::default(),
            slots: Vec::new(),
            free: Vec::new(),
            counts: MeasureCounts::default(),
        }
    }

    /// The glyph advance as a fraction of the font size.
    #[must_use]
    pub fn advance_em(&self) -> f32 {
        self.advance_em
    }

    /// Raises the font epoch, as a font load or reload would: every commit
    /// made before it goes stale.
    pub fn bump_font_epoch(&mut self) {
        self.epoch = self.epoch.next();
    }

    /// Nodes created and not yet removed.
    #[must_use]
    pub fn live_nodes(&self) -> usize {
        self.slots.iter().filter(|slot| slot.live).count()
    }

    fn slot(&self, id: TextNodeId) -> Option<&Slot> {
        self.slots
            .get(id.index() as usize)
            .filter(|slot| slot.live && slot.generation == id.generation())
    }

    fn slot_mut(&mut self, id: TextNodeId) -> Option<&mut Slot> {
        self.slots
            .get_mut(id.index() as usize)
            .filter(|slot| slot.live && slot.generation == id.generation())
    }

    fn node(&self, id: TextNodeId) -> Option<&NodeData> {
        self.slot(id)?.node.as_ref()
    }

    /// The metrics of `data` laid out at `width`, or unwrapped when `None`.
    fn metrics_of(&self, data: &NodeData, width: Option<f32>) -> TextMetrics {
        let advance = glyph_advance(&data.style, self.advance_em);
        let text = flatten(&data.text);
        let lines = match width {
            Some(width) => line_lengths(&text, data.style.layout.wrap, advance, width),
            None => text.split('\n').map(|line| line.chars().count()).collect(),
        };
        let widest = lines.iter().copied().max().unwrap_or(0);
        let line_height = data.style.line_height;
        TextMetrics::new(
            Vec2::new(widest as f32 * advance, lines.len() as f32 * line_height),
            BASELINE_RATIO * line_height,
            lines.len() as u32,
        )
    }

    /// The narrowest width `data` wraps to without overflowing.
    fn min_content_width(&self, data: &NodeData) -> f32 {
        let advance = glyph_advance(&data.style, self.advance_em);
        let text = flatten(&data.text);
        let glyphs = match data.style.layout.wrap {
            TextWrap::None => text
                .split('\n')
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0),
            TextWrap::Glyph => usize::from(text.chars().any(|c| c != '\n')),
            TextWrap::Word | TextWrap::WordOrGlyph => text
                .split(['\n', ' '])
                .map(|word| word.chars().count())
                .max()
                .unwrap_or(0),
        };
        glyphs as f32 * advance
    }
}

/// The advance of one glyph under `style`.
fn glyph_advance(style: &TextStyle, advance_em: f32) -> f32 {
    style.size * (advance_em + style.layout.letter_spacing)
}

/// The spans' text, concatenated.
fn flatten(text: &StyledText) -> String {
    text.spans.iter().map(|span| span.text.as_str()).collect()
}

/// Whether `glyphs` glyphs fit in `width`.
fn fits(glyphs: usize, advance: f32, width: f32) -> bool {
    glyphs as f32 * advance <= width
}

/// Glyphs per line when `text` wraps at `width` under `wrap`. Every paragraph
/// yields at least one line, so empty text is one line of no glyphs.
fn line_lengths(text: &str, wrap: TextWrap, advance: f32, width: f32) -> Vec<usize> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let glyphs = paragraph.chars().count();
        match wrap {
            TextWrap::None => lines.push(glyphs),
            TextWrap::Glyph => glyph_chunks(glyphs, advance, width, &mut lines),
            TextWrap::Word | TextWrap::WordOrGlyph => {
                let break_inside = wrap == TextWrap::WordOrGlyph;
                let mut line = 0_usize;
                let mut has_word = false;
                for word in paragraph.split(' ') {
                    let word_glyphs = word.chars().count();
                    let candidate = if has_word {
                        line + 1 + word_glyphs
                    } else {
                        word_glyphs
                    };
                    if fits(candidate, advance, width) {
                        line = candidate;
                        has_word = true;
                        continue;
                    }
                    if has_word {
                        lines.push(line);
                    }
                    if break_inside && !fits(word_glyphs, advance, width) {
                        glyph_chunks(word_glyphs, advance, width, &mut lines);
                        line = lines.pop().unwrap_or(0);
                    } else {
                        // Fits on a line of its own, or overflows under `Word`.
                        line = word_glyphs;
                    }
                    has_word = true;
                }
                lines.push(line);
            }
        }
    }
    lines
}

/// Splits `glyphs` glyphs into lines of as many as fit, at least one each.
fn glyph_chunks(glyphs: usize, advance: f32, width: f32, lines: &mut Vec<usize>) {
    if glyphs == 0 {
        lines.push(0);
        return;
    }
    let mut per_line = 0_usize;
    while fits(per_line + 1, advance, width) {
        per_line += 1;
    }
    let per_line = per_line.max(1);
    let mut left = glyphs;
    while left > 0 {
        let take = left.min(per_line);
        lines.push(take);
        left -= take;
    }
}

impl TextMeasure for FixedAdvanceMeasure {
    fn measure(
        &mut self,
        node: TextNodeId,
        known_width: Option<f32>,
        available: MeasureWidth,
    ) -> TextMetrics {
        self.counts.measure_calls += 1;
        let Some(data) = self.node(node) else {
            return TextMetrics::default();
        };
        match known_width.map_or(available, MeasureWidth::Definite) {
            MeasureWidth::MinContent => {
                let width = self.min_content_width(data);
                self.metrics_of(data, Some(width))
            }
            MeasureWidth::Definite(width) => self.metrics_of(data, Some(width)),
            MeasureWidth::MaxContent => self.metrics_of(data, None),
        }
    }
}

impl TextNodeStore for FixedAdvanceMeasure {
    fn font_epoch(&self) -> FontEpoch {
        self.epoch
    }

    fn create_node(&mut self) -> TextNodeId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.live = true;
            return TextNodeId::new(index, slot.generation);
        }
        let index = u32::try_from(self.slots.len()).expect("text node count fits in u32");
        self.slots.push(Slot {
            generation: 0,
            live: true,
            node: None,
        });
        TextNodeId::new(index, 0)
    }

    fn remove_node(&mut self, node: TextNodeId) {
        let Some(slot) = self.slot_mut(node) else {
            return;
        };
        slot.live = false;
        slot.generation = slot.generation.wrapping_add(1);
        slot.node = None;
        self.free.push(node.index());
    }

    fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle) {
        self.counts.set_text_calls += 1;
        let Some(slot) = self.slot_mut(node) else {
            return;
        };
        match &mut slot.node {
            Some(current) if current.text == *text && current.style == *style => {}
            Some(current) => {
                current.text.clone_from(text);
                current.style.clone_from(style);
                current.committed = None;
            }
            None => {
                slot.node = Some(NodeData {
                    text: text.clone(),
                    style: style.clone(),
                    committed: None,
                });
            }
        }
    }

    fn commit_layout(&mut self, node: TextNodeId, size: Vec2) -> TextMetrics {
        self.counts.commits += 1;
        let Some(data) = self.node(node) else {
            return TextMetrics::default();
        };
        let metrics = self.metrics_of(data, Some(size.x));
        let epoch = self.epoch;
        if let Some(data) = self.slot_mut(node).and_then(|slot| slot.node.as_mut()) {
            data.committed = Some((epoch, metrics));
        }
        metrics
    }

    fn committed(&self, node: TextNodeId) -> Option<TextMetrics> {
        let (epoch, metrics) = self.node(node)?.committed?;
        (epoch == self.epoch).then_some(metrics)
    }
}

#[cfg(test)]
#[path = "../tests/ui/measure.rs"]
mod tests;
