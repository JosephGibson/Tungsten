//! Retained text nodes (`D-117`): text and a style, shaped once into a buffer
//! that layout measures at several widths and then commits. Declared inside
//! `engine`, so node methods reach the engine's fonts. A node with coloured
//! spans keeps a second, plain buffer while an outline or shadow copy draws
//! its plain view (P7-14).

use std::collections::HashMap;

use glam::Vec2;
use glyphon::cosmic_text::Ellipsize;
use glyphon::{Attrs, Buffer, Color, Family, FontSystem, Shaping, Wrap};
use tungsten_core::text::{
    FontEpoch, MeasureWidth, StyledText, TextMeasure, TextMetrics, TextNodeId, TextNodeStore,
    TextStyle, TextWrap,
};

use super::{
    SPARE_BUFFER_CAP, StoredFontAttrs, TextEngine, TextLayoutKey, cosmic_align, cosmic_ellipsize,
    cosmic_hinting, cosmic_wrap, layout_metrics, with_layout,
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
    /// Rises on a changing `set_text` and on every `commit_layout`, so a UI
    /// text batch's signature moves with what the buffer draws (M46).
    revision: u64,
    /// The plain view's buffer (P7-14): the text shaped without its spans'
    /// colours, laid out as `committed` is, while a frame's copies draw it.
    plain: Option<Buffer>,
    /// The layout `plain` holds.
    plain_laid: Option<LayoutState>,
    /// The plain-view pass that last drew `plain`.
    plain_pass: u64,
}

impl Node {
    fn new(text: StyledText, style: TextStyle) -> Self {
        // Never `Buffer::new`: it shapes an empty line first.
        let buffer = Buffer::new_empty(layout_metrics(style.size, style.line_height));
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
            revision: 0,
            plain: None,
            plain_laid: None,
            plain_pass: 0,
        }
    }

    /// Forgets the shaping and every measurement made from it, the plain
    /// view's buffer included.
    fn invalidate(&mut self) {
        self.shaped = None;
        self.laid_out = None;
        self.min = None;
        self.max = None;
        self.definite = None;
        self.committed = None;
        self.plain = None;
        self.plain_laid = None;
    }

    /// Whether a span carries a colour of its own, which a plain view drops.
    fn coloured(&self) -> bool {
        self.text.spans.iter().any(|span| span.color.is_some())
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
    /// Counts [`TextEngine::prepare_plain`] passes.
    plain_pass: u64,
    /// Whether a node may hold a plain buffer, so a pass sweeps the slots.
    plain_held: bool,
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

    /// The committed buffer of `id` and the node's revision, while the
    /// buffer still holds that layout under `epoch`; for a plain view's ID,
    /// the plain buffer laid out as the commit is, or the committed buffer
    /// itself for a node whose spans carry no colour.
    fn area(&self, id: TextNodeId, epoch: FontEpoch) -> Option<(&Buffer, u64)> {
        let current = self.slot(id)?.node.as_ref()?;
        let (state, _) = current.committed?;
        if !(current.shaped == Some(epoch) && current.laid_out == Some(state)) {
            return None;
        }
        if !id.is_plain() || !current.coloured() {
            return Some((&current.buffer, current.revision));
        }
        current
            .plain
            .as_ref()
            .filter(|_| current.plain_laid == Some(state))
            .map(|plain| (plain, current.revision))
    }
}

/// The committed node buffers, lent to a UI text batch's prepare beside the
/// font system (M46).
pub(crate) struct NodeAreas<'a> {
    nodes: &'a NodeStore,
    epoch: FontEpoch,
}

impl<'a> NodeAreas<'a> {
    pub(super) fn new(nodes: &'a NodeStore, epoch: FontEpoch) -> Self {
        Self { nodes, epoch }
    }

    /// See [`TextEngine::node_area`].
    pub(crate) fn area(&self, id: TextNodeId) -> Option<(&'a Buffer, u64)> {
        self.nodes.area(id, self.epoch)
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
                current.revision += 1;
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
            current.revision += 1;
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

    /// The node's revision: it rises on a changing [`Self::set_text`] and on
    /// every [`Self::commit_layout`], never on equal text; `None` for a stale
    /// ID or a node without text. A UI text batch's signature carries it
    /// (M46).
    #[must_use]
    pub fn node_revision(&self, node: TextNodeId) -> Option<u64> {
        Some(self.nodes.slot(node)?.node.as_ref()?.revision)
    }

    /// The committed buffer of `node` and its revision, while the buffer
    /// still holds that layout under the current font epoch; what a UI text
    /// batch draws through [`NodeAreas::area`].
    #[cfg(test)]
    pub(crate) fn node_area(&self, node: TextNodeId) -> Option<(&Buffer, u64)> {
        self.nodes.area(node, self.epoch)
    }

    /// Readies the plain views `ids` name for a frame's UI text batches
    /// (P7-14): a committed node with a coloured span shapes its text
    /// without the spans' colours into a second buffer and lays it out as
    /// its commit; a node no plain ID names drops its plain buffer. IDs that
    /// name no plain view are skipped.
    pub(crate) fn prepare_plain(&mut self, ids: impl IntoIterator<Item = TextNodeId>) {
        self.nodes.plain_pass += 1;
        let pass = self.nodes.plain_pass;
        for id in ids {
            if id.is_plain() {
                self.ready_plain(id.base(), pass);
            }
        }
        if self.nodes.plain_held {
            let mut held = false;
            for node in self
                .nodes
                .slots
                .iter_mut()
                .filter_map(|slot| slot.node.as_mut())
            {
                if node.plain.is_some() && node.plain_pass != pass {
                    node.plain = None;
                    node.plain_laid = None;
                }
                held |= node.plain.is_some();
            }
            self.nodes.plain_held = held;
        }
    }

    /// [`Self::prepare_plain`] for one node, in pass `pass`.
    fn ready_plain(&mut self, id: TextNodeId, pass: u64) {
        let epoch = self.epoch;
        let Some(current) = self.nodes.slot(id).and_then(|slot| slot.node.as_ref()) else {
            return;
        };
        let Some((state, _)) = current.committed else {
            return;
        };
        if current.shaped != Some(epoch) || current.laid_out != Some(state) || !current.coloured() {
            return;
        }
        let shape = current.plain.is_none();
        let (text, style) = (current.text.clone(), current.style.clone());
        let faces = shape.then(|| self.span_faces(&text, &style));
        let Some(current) = self.nodes.node_mut(id) else {
            return;
        };
        current.plain_pass = pass;
        if let Some((base, span_faces)) = faces {
            let mut buffer = Buffer::new_empty(layout_metrics(style.size, style.line_height));
            set_spans(
                &mut buffer,
                &self.font_attrs,
                &text,
                &style,
                base.as_ref(),
                &span_faces,
                false,
            );
            current.plain = Some(buffer);
            current.plain_laid = None;
        }
        if current.plain_laid != Some(state)
            && let Some(plain) = &mut current.plain
        {
            plain.set_wrap(state.wrap);
            plain.set_ellipsize(state.ellipsize);
            plain.set_size(state.width, state.height);
            plain.shape_until_scroll(&mut self.font_system, false);
            current.plain_laid = Some(state);
        }
        self.nodes.plain_held = true;
    }

    /// The face of `style` and of each of `text`'s spans, resolved through
    /// the style's family (`D-116`).
    fn span_faces(
        &mut self,
        text: &StyledText,
        style: &TextStyle,
    ) -> (Option<String>, Vec<Option<String>>) {
        let base = self.resolve_face(&style.family, style.weight, style.italic);
        let span_faces = text
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
        (base, span_faces)
    }

    /// The font system and the committed node buffers, for a UI text
    /// batch's prepare.
    pub(crate) fn batch_parts(&mut self) -> (&mut FontSystem, NodeAreas<'_>) {
        (
            &mut self.font_system,
            NodeAreas::new(&self.nodes, self.epoch),
        )
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
        let (base, span_faces) = self.span_faces(&text, &style);
        let Some(current) = self.nodes.node_mut(node) else {
            return false;
        };
        set_spans(
            &mut current.buffer,
            &self.font_attrs,
            &text,
            &style,
            base.as_ref(),
            &span_faces,
            true,
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
        let metrics = line_metrics(&current.buffer);
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

/// Sets `buffer`'s metrics, hinting and rich text from `text` in `style`,
/// each span in its face from `span_faces`, its own colour baked in only
/// when `colours`. The style's colour is never baked (M56): glyphs without a
/// span colour take the text area's default colour, which the paint command
/// sets, so a colour change or a fade re-prepares the batch and never
/// reshapes.
fn set_spans(
    buffer: &mut Buffer,
    font_attrs: &HashMap<String, StoredFontAttrs>,
    text: &StyledText,
    style: &TextStyle,
    base: Option<&String>,
    span_faces: &[Option<String>],
    colours: bool,
) {
    let face_attrs = |face: Option<&String>| {
        face.and_then(|face| font_attrs.get(face)).map_or_else(
            || Attrs::new().family(Family::SansSerif),
            StoredFontAttrs::attrs,
        )
    };
    let color = |[r, g, b, a]: [u8; 4]| Color::rgba(r, g, b, a);
    let default_attrs = with_layout(face_attrs(base), &style.layout);
    let mut spans: Vec<(&str, Attrs)> = text
        .spans
        .iter()
        .zip(span_faces)
        .map(|(span, face)| {
            let attrs = face_attrs(face.as_ref());
            let attrs = match span.color.filter(|_| colours) {
                Some(own) => attrs.color(color(own)),
                None => attrs,
            };
            (span.text.as_str(), with_layout(attrs, &style.layout))
        })
        .collect();
    if spans.is_empty() {
        spans.push(("", default_attrs.clone()));
    }
    let metrics = layout_metrics(style.size, style.line_height);
    if metrics.font_size == 0.0 {
        // `set_metrics` rejects a zero font size; `new_empty` takes one.
        *buffer = Buffer::new_empty(metrics);
    } else {
        buffer.set_metrics(metrics);
    }
    buffer.set_hinting(cosmic_hinting(style.layout.hinting));
    buffer.set_rich_text(
        spans,
        &default_attrs,
        Shaping::Advanced,
        cosmic_align(style.layout.align),
    );
}

/// Size, first baseline and line count of a laid-out buffer. Text with no
/// lines measures one line height, the one the buffer lays out with.
fn line_metrics(buffer: &Buffer) -> TextMetrics {
    let line_height = buffer.metrics().line_height;
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
