//! Retained text nodes (`D-117`): text and a style, shaped once into a buffer
//! that layout measures at several widths and then commits. Declared inside
//! `engine`, so node methods reach the engine's fonts.

use glam::Vec2;
use glyphon::cosmic_text::Ellipsize;
use glyphon::{Attrs, Buffer, Color, Family, Metrics, Shaping, Wrap};
use tungsten_core::text::{
    FontEpoch, MeasureWidth, StyledText, TextMeasure, TextMetrics, TextNodeId, TextNodeStore,
    TextStyle, TextWrap,
};

use super::{
    SPARE_BUFFER_CAP, StoredFontAttrs, TextEngine, TextLayoutKey, cosmic_align, cosmic_ellipsize,
    cosmic_hinting, cosmic_wrap, with_layout,
};

/// The settings a node's buffer is laid out with.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LayoutState {
    width: Option<f32>,
    height: Option<f32>,
    wrap: Wrap,
    ellipsize: Ellipsize,
}

struct Node {
    text: StyledText,
    style: TextStyle,
    buffer: Buffer,
    /// The epoch the buffer was shaped under; `None` until it is shaped.
    shaped: Option<FontEpoch>,
    /// What the buffer is laid out with now.
    laid_out: Option<LayoutState>,
    min: Option<TextMetrics>,
    max: Option<TextMetrics>,
    /// The last definite width measured, by its bits, and its metrics.
    definite: Option<(u32, TextMetrics)>,
    /// The committed layout and its metrics.
    committed: Option<(LayoutState, TextMetrics)>,
}

impl Node {
    fn new(text: StyledText, style: TextStyle) -> Self {
        // Never `Buffer::new`: it shapes an empty line first.
        let buffer = Buffer::new_empty(Metrics::new(style.size, style.line_height));
        Self {
            text,
            style,
            buffer,
            shaped: None,
            laid_out: None,
            min: None,
            max: None,
            definite: None,
            committed: None,
        }
    }

    /// Forgets the shaping and every measurement made from it.
    fn invalidate(&mut self) {
        self.shaped = None;
        self.laid_out = None;
        self.min = None;
        self.max = None;
        self.definite = None;
        self.committed = None;
    }

    fn wrap(&self) -> Wrap {
        cosmic_wrap(self.style.layout.wrap)
    }
}

struct NodeSlot {
    generation: u32,
    live: bool,
    /// `None` until the node's first `set_text`.
    node: Option<Node>,
}

/// Nodes in a generational slot list with a free list.
#[derive(Default)]
pub(super) struct NodeStore {
    slots: Vec<NodeSlot>,
    free: Vec<u32>,
    /// Buffers shaped, for tests.
    #[cfg(test)]
    shapes: u64,
    /// Buffers laid out, for tests.
    #[cfg(test)]
    layouts: u64,
}

impl NodeStore {
    fn slot(&self, id: TextNodeId) -> Option<&NodeSlot> {
        self.slots
            .get(id.index() as usize)
            .filter(|slot| slot.live && slot.generation == id.generation())
    }

    fn slot_mut(&mut self, id: TextNodeId) -> Option<&mut NodeSlot> {
        self.slots
            .get_mut(id.index() as usize)
            .filter(|slot| slot.live && slot.generation == id.generation())
    }

    /// The live node with text behind `id`.
    fn node_mut(&mut self, id: TextNodeId) -> Option<&mut Node> {
        self.slot_mut(id)?.node.as_mut()
    }
}

impl TextEngine {
    /// A new node with no text; it measures zero until [`Self::set_text`].
    pub fn create_node(&mut self) -> TextNodeId {
        let nodes = &mut self.nodes;
        if let Some(index) = nodes.free.pop() {
            let slot = &mut nodes.slots[index as usize];
            slot.live = true;
            return TextNodeId::new(index, slot.generation);
        }
        let index = u32::try_from(nodes.slots.len()).expect("text node count fits in u32");
        nodes.slots.push(NodeSlot {
            generation: 0,
            live: true,
            node: None,
        });
        TextNodeId::new(index, 0)
    }

    /// Removes `node`. Its ID never names a node again, and its buffer joins
    /// the sections' spare list (`D-085`).
    pub fn remove_node(&mut self, node: TextNodeId) {
        let Some(slot) = self.nodes.slot_mut(node) else {
            self.stale_node(node);
            return;
        };
        slot.live = false;
        slot.generation = slot.generation.wrapping_add(1);
        let removed = slot.node.take();
        self.nodes.free.push(node.index());
        if let Some(removed) = removed
            && self.spare.len() < SPARE_BUFFER_CAP
        {
            self.spare.push((TextLayoutKey::default(), removed.buffer));
        }
    }

    /// Sets `node`'s text and style. Equal input does no work; anything else
    /// reshapes at the next measure or commit.
    pub fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle) {
        let Some(slot) = self.nodes.slot_mut(node) else {
            self.stale_node(node);
            return;
        };
        match &mut slot.node {
            Some(current) if current.text == *text && current.style == *style => {}
            Some(current) => {
                current.text.clone_from(text);
                current.style.clone_from(style);
                current.invalidate();
            }
            None => slot.node = Some(Node::new(text.clone(), style.clone())),
        }
    }

    /// Lays `node` out in its final box: `size.x` wide, and `size.y` high for
    /// an ellipsis without a line limit. The buffer keeps this layout, after
    /// any probes, until the next measure.
    pub fn commit_layout(&mut self, node: TextNodeId, size: Vec2) -> TextMetrics {
        if !self.shape_node(node) {
            return TextMetrics::default();
        }
        let Some(current) = self.nodes.node_mut(node) else {
            return TextMetrics::default();
        };
        let state = LayoutState {
            width: Some(size.x),
            height: Some(size.y),
            wrap: current.wrap(),
            ellipsize: cosmic_ellipsize(current.style.layout.overflow, Some(size.y)),
        };
        let metrics = self.lay_out(node, state);
        if let Some(current) = self.nodes.node_mut(node) {
            debug_assert_eq!(
                current.buffer.size(),
                (Some(size.x.max(0.0)), Some(size.y.max(0.0))),
                "the buffer holds the committed size",
            );
            current.committed = Some((state, metrics));
        }
        metrics
    }

    /// The metrics of `node`'s last commit, while its buffer still holds that
    /// layout under the current font epoch.
    #[must_use]
    pub fn committed(&self, node: TextNodeId) -> Option<TextMetrics> {
        let current = self.nodes.slot(node)?.node.as_ref()?;
        let (state, metrics) = current.committed?;
        (current.shaped == Some(self.epoch) && current.laid_out == Some(state)).then_some(metrics)
    }

    /// Logs a stale or removed node ID, once per ID.
    fn stale_node(&mut self, node: TextNodeId) {
        let key = format!("node {}:{}", node.index(), node.generation());
        if self.logged.insert(key) {
            log::warn!(
                "Text node {}:{} is stale or removed",
                node.index(),
                node.generation()
            );
        }
    }

    /// Shapes `node` if its text, style or epoch moved. False for a stale ID
    /// (logged) or a node without text.
    fn shape_node(&mut self, node: TextNodeId) -> bool {
        let epoch = self.epoch;
        let (text, style) = match self.nodes.slot(node) {
            None => {
                self.stale_node(node);
                return false;
            }
            Some(NodeSlot { node: None, .. }) => return false,
            Some(NodeSlot {
                node: Some(current),
                ..
            }) => {
                if current.shaped == Some(epoch) {
                    return true;
                }
                (current.text.clone(), current.style.clone())
            }
        };

        // Each span's face, resolved through the style's family (`D-116`).
        let base = self.resolve_face(&style.family, style.weight, style.italic);
        let span_faces: Vec<Option<String>> = text
            .spans
            .iter()
            .map(|span| {
                if span.weight.is_none() && span.italic.is_none() {
                    return base.clone();
                }
                self.resolve_face(
                    &style.family,
                    span.weight.unwrap_or(style.weight),
                    span.italic.unwrap_or(style.italic),
                )
            })
            .collect();

        let font_attrs = &self.font_attrs;
        let face_attrs = |face: Option<&String>| {
            face.and_then(|face| font_attrs.get(face)).map_or_else(
                || Attrs::new().family(Family::SansSerif),
                StoredFontAttrs::attrs,
            )
        };
        let color = |[r, g, b, a]: [u8; 4]| Color::rgba(r, g, b, a);
        let default_attrs = with_layout(
            face_attrs(base.as_ref()).color(color(style.color)),
            &style.layout,
        );
        let mut spans: Vec<(&str, Attrs)> = text
            .spans
            .iter()
            .zip(&span_faces)
            .map(|(span, face)| {
                let attrs =
                    face_attrs(face.as_ref()).color(color(span.color.unwrap_or(style.color)));
                (span.text.as_str(), with_layout(attrs, &style.layout))
            })
            .collect();
        if spans.is_empty() {
            spans.push(("", default_attrs.clone()));
        }

        let Some(current) = self.nodes.node_mut(node) else {
            return false;
        };
        let metrics = Metrics::new(style.size, style.line_height);
        if metrics.font_size == 0.0 || metrics.line_height == 0.0 {
            // `set_metrics` rejects zero; `new_empty` takes it.
            current.buffer = Buffer::new_empty(metrics);
        } else {
            current.buffer.set_metrics(metrics);
        }
        current
            .buffer
            .set_hinting(cosmic_hinting(style.layout.hinting));
        current.buffer.set_rich_text(
            spans,
            &default_attrs,
            Shaping::Advanced,
            cosmic_align(style.layout.align),
        );
        current.invalidate();
        current.shaped = Some(epoch);
        #[cfg(test)]
        {
            self.nodes.shapes += 1;
        }
        true
    }

    /// Lays `node`'s buffer out with `state`, unless it already is, and
    /// returns the metrics of its lines.
    fn lay_out(&mut self, node: TextNodeId, state: LayoutState) -> TextMetrics {
        let Some(current) = self.nodes.node_mut(node) else {
            return TextMetrics::default();
        };
        let relaid = current.laid_out != Some(state);
        if relaid {
            current.buffer.set_wrap(state.wrap);
            current.buffer.set_ellipsize(state.ellipsize);
            current.buffer.set_size(state.width, state.height);
            current
                .buffer
                .shape_until_scroll(&mut self.font_system, false);
            current.laid_out = Some(state);
        }
        let metrics = line_metrics(&current.buffer, current.style.line_height);
        #[cfg(test)]
        if relaid {
            self.nodes.layouts += 1;
        }
        metrics
    }

    /// The unwrapped width, in every wrap mode; overflow is ignored.
    fn max_content(&mut self, node: TextNodeId) -> TextMetrics {
        let Some(current) = self.nodes.node_mut(node) else {
            return TextMetrics::default();
        };
        if let Some(max) = current.max {
            return max;
        }
        let state = LayoutState {
            width: None,
            height: None,
            wrap: current.wrap(),
            ellipsize: Ellipsize::None,
        };
        let max = self.lay_out(node, state);
        if let Some(current) = self.nodes.node_mut(node) {
            current.max = Some(max);
        }
        max
    }

    /// The widest word (the widest glyph under [`TextWrap::Glyph`], the
    /// unwrapped width under [`TextWrap::None`]), laid out at that width;
    /// overflow is ignored.
    fn min_content(&mut self, node: TextNodeId) -> TextMetrics {
        let Some(current) = self.nodes.node_mut(node) else {
            return TextMetrics::default();
        };
        if let Some(min) = current.min {
            return min;
        }
        let wrap = current.wrap();
        let probe_wrap = match current.style.layout.wrap {
            TextWrap::None => None,
            TextWrap::Glyph => Some(Wrap::Glyph),
            // Under `WordOrGlyph`, width 0 would split every word into glyphs.
            _ => Some(Wrap::Word),
        };
        let min = match probe_wrap {
            None => self.max_content(node),
            Some(probe_wrap) => {
                let probe = self.lay_out(
                    node,
                    LayoutState {
                        width: Some(0.0),
                        height: None,
                        wrap: probe_wrap,
                        ellipsize: Ellipsize::None,
                    },
                );
                // The width is the probe's: under `Glyph`, cosmic-text counts
                // a trailing blank in a line's width. The lines are those at
                // that width.
                let lines = self.lay_out(
                    node,
                    LayoutState {
                        width: Some(probe.size.x),
                        height: None,
                        wrap,
                        ellipsize: Ellipsize::None,
                    },
                );
                TextMetrics::new(
                    Vec2::new(probe.size.x, lines.size.y),
                    lines.first_baseline,
                    lines.line_count,
                )
            }
        };
        if let Some(current) = self.nodes.node_mut(node) {
            current.min = Some(min);
        }
        min
    }

    /// The layout at `width` with the node's overflow; `measure` has no
    /// height, so an ellipsis without a line limit keeps every line.
    fn definite(&mut self, node: TextNodeId, width: f32) -> TextMetrics {
        let Some(current) = self.nodes.node_mut(node) else {
            return TextMetrics::default();
        };
        if let Some((bits, metrics)) = current.definite
            && bits == width.to_bits()
        {
            return metrics;
        }
        let state = LayoutState {
            width: Some(width),
            height: None,
            wrap: current.wrap(),
            ellipsize: cosmic_ellipsize(current.style.layout.overflow, None),
        };
        let metrics = self.lay_out(node, state);
        if let Some(current) = self.nodes.node_mut(node) {
            current.definite = Some((width.to_bits(), metrics));
        }
        metrics
    }
}

impl TextMeasure for TextEngine {
    /// Min- and max-content ignore overflow and are cached per text, style
    /// and epoch; a definite width applies the node's overflow. A stale or
    /// removed ID, or a node without text, measures zero.
    fn measure(
        &mut self,
        node: TextNodeId,
        known_width: Option<f32>,
        available: MeasureWidth,
    ) -> TextMetrics {
        if !self.shape_node(node) {
            return TextMetrics::default();
        }
        match known_width.map_or(available, MeasureWidth::Definite) {
            MeasureWidth::MinContent => self.min_content(node),
            MeasureWidth::Definite(width) => self.definite(node, width),
            _ => self.max_content(node),
        }
    }
}

/// Size, first baseline and line count of a laid-out buffer. Text with no
/// lines measures one line height.
fn line_metrics(buffer: &Buffer, line_height: f32) -> TextMetrics {
    let mut width = 0.0_f32;
    let mut bottom = 0.0_f32;
    let mut first_baseline = None;
    let mut lines = 0_u32;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        bottom = run.line_top + run.line_height;
        first_baseline.get_or_insert(run.line_y);
        lines += 1;
    }
    if lines == 0 {
        return TextMetrics::new(Vec2::new(0.0, line_height), 0.0, 1);
    }
    TextMetrics::new(
        Vec2::new(width, bottom),
        first_baseline.unwrap_or(0.0),
        lines,
    )
}

/// The node methods of the renderer's text engine, lent to layout as
/// `&mut dyn TextMeasure` (`D-117`). Nothing else of the engine is reachable
/// through it: the GPU half keys glyphs by the engine's font IDs, so the
/// engine itself is never lent mutably.
pub struct TextNodes<'a> {
    engine: &'a mut TextEngine,
}

impl<'a> TextNodes<'a> {
    pub(crate) fn new(engine: &'a mut TextEngine) -> Self {
        Self { engine }
    }

    /// See [`TextEngine::font_epoch`].
    #[must_use]
    pub fn font_epoch(&self) -> FontEpoch {
        self.engine.font_epoch()
    }

    /// See [`TextEngine::create_node`].
    pub fn create_node(&mut self) -> TextNodeId {
        self.engine.create_node()
    }

    /// See [`TextEngine::remove_node`].
    pub fn remove_node(&mut self, node: TextNodeId) {
        self.engine.remove_node(node);
    }

    /// See [`TextEngine::set_text`].
    pub fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle) {
        self.engine.set_text(node, text, style);
    }

    /// See [`TextEngine::commit_layout`].
    pub fn commit_layout(&mut self, node: TextNodeId, size: Vec2) -> TextMetrics {
        self.engine.commit_layout(node, size)
    }

    /// See [`TextEngine::committed`].
    #[must_use]
    pub fn committed(&self, node: TextNodeId) -> Option<TextMetrics> {
        self.engine.committed(node)
    }
}

impl TextMeasure for TextNodes<'_> {
    fn measure(
        &mut self,
        node: TextNodeId,
        known_width: Option<f32>,
        available: MeasureWidth,
    ) -> TextMetrics {
        self.engine.measure(node, known_width, available)
    }
}

/// Core's node seam (`D-125`), forwarding to the inherent methods so the UI
/// tree drives labels through `&mut dyn TextNodeStore`.
impl TextNodeStore for TextEngine {
    fn font_epoch(&self) -> FontEpoch {
        TextEngine::font_epoch(self)
    }

    fn create_node(&mut self) -> TextNodeId {
        TextEngine::create_node(self)
    }

    fn remove_node(&mut self, node: TextNodeId) {
        TextEngine::remove_node(self, node);
    }

    fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle) {
        TextEngine::set_text(self, node, text, style);
    }

    fn commit_layout(&mut self, node: TextNodeId, size: Vec2) -> TextMetrics {
        TextEngine::commit_layout(self, node, size)
    }

    fn committed(&self, node: TextNodeId) -> Option<TextMetrics> {
        TextEngine::committed(self, node)
    }
}

impl TextNodeStore for TextNodes<'_> {
    fn font_epoch(&self) -> FontEpoch {
        self.engine.font_epoch()
    }

    fn create_node(&mut self) -> TextNodeId {
        self.engine.create_node()
    }

    fn remove_node(&mut self, node: TextNodeId) {
        self.engine.remove_node(node);
    }

    fn set_text(&mut self, node: TextNodeId, text: &StyledText, style: &TextStyle) {
        self.engine.set_text(node, text, style);
    }

    fn commit_layout(&mut self, node: TextNodeId, size: Vec2) -> TextMetrics {
        self.engine.commit_layout(node, size)
    }

    fn committed(&self, node: TextNodeId) -> Option<TextMetrics> {
        self.engine.committed(node)
    }
}

#[cfg(test)]
#[path = "../tests/text_nodes.rs"]
mod tests;
