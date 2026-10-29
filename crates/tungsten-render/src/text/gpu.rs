//! The GPU half of text (`D-026`, `D-117`): glyphon's atlas, viewport and
//! swash cache, one renderer for the screen-space sections, one for the
//! world labels and one per UI text batch on the shared atlas, under T1b's
//! trim rule (`D-126`, `D-147`, `D-161`): `trim` runs only before a full
//! frame, which re-prepares every batch, the labels and the sections; a full
//! frame is forced after [`FULL_FRAME_INTERVAL`] changed frames, on
//! `AtlasFull`, on a font reload and on every changed frame while there are
//! no batches, the labels counting as one, so frames without UI or labels
//! prepare and trim as before.
//! Between full frames only batches whose signature changed re-prepare, and
//! a batch whose prepare failed is not drawn. The rule, the signatures and
//! the batch bookkeeping are GPU-free types with their own tests.

use std::collections::HashMap;
use std::ops::Range;

use glyphon::{
    Cache, Color, PrepareError, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use tungsten_core::assets::ResolvedFontFamily;
use tungsten_core::text::{FontEpoch, TextNodeId};
use tungsten_core::ui::{ClipRect, PaintCommand, PaintList, Rect};
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

use super::engine::NodeAreas;
use super::{FontSource, TextEngine, TextPrewarm, TextSection, WorldLabelRun};
use crate::counters::Counts;

/// K (`D-126`): the changed frame at which a full frame is forced while
/// batches are retained. Sixteen trades prepare cost against atlas bytes on
/// the prototype's sweep; a game that needs another value is M4's evidence.
pub(crate) const FULL_FRAME_INTERVAL: u32 = 16;

const MULTISAMPLE: MultisampleState = MultisampleState {
    count: 1,
    mask: !0,
    alpha_to_coverage_enabled: false,
};

/// Text telemetry for the frame (M46 Q3): what is observable without an
/// atlas size, which glyphon 0.12 keeps private.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextStats {
    /// UI text batches in the frame's paint list.
    pub batches: u32,
    /// Batches prepared this frame.
    pub prepared: u32,
    /// Whether this frame was full: the atlas trimmed, every batch and the
    /// sections prepared.
    pub full_frame: bool,
    /// Whether a full frame was forced by `AtlasFull` or a font reload
    /// rather than reached by the interval or the epoch.
    pub forced: bool,
    /// `AtlasFull` results since the pipeline was made.
    pub atlas_full_total: u64,
    /// Full frames since the pipeline was made.
    pub full_frames_total: u64,
    /// World labels drawn this frame (M54).
    pub labels: u32,
    /// Glyphs in the label areas this frame's prepare took; 0 when the
    /// labels did not prepare.
    pub label_glyphs: u32,
    /// Layouts in the engine's cache, sections and labels together.
    pub layouts: u32,
}

/// What a frame's prepare does under the trim rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameKind {
    /// Nothing changed: no prepare, no trim, no counter moved.
    Nothing,
    /// Only the changed batches and, when changed, the sections prepare.
    Partial,
    /// The atlas trims, then every batch and the sections prepare.
    Full {
        /// Forced by `AtlasFull` or a reload, not reached by the interval or
        /// a font epoch move.
        forced: bool,
    },
}

/// The trim rule (T1b, `D-126`) as a GPU-free state machine.
#[derive(Debug, Default)]
pub(crate) struct TrimPolicy {
    force_full: bool,
    /// Partial frames since the last full one.
    changed_since_full: u32,
    /// The font epoch of the last decision; `None` before the first.
    epoch: Option<FontEpoch>,
    full_frames_total: u64,
    atlas_full_total: u64,
}

impl TrimPolicy {
    /// The next frame that prepares anything is full: a font reload.
    pub(crate) fn force_full(&mut self) {
        self.force_full = true;
    }

    /// A prepare returned `AtlasFull`: counted, and the next frame is full.
    pub(crate) fn atlas_full(&mut self) {
        self.atlas_full_total += 1;
        self.force_full = true;
    }

    /// Decides the frame: `Nothing` when no batch and no section changed
    /// and nothing forces a frame; otherwise `Full` when forced, when the
    /// font `epoch` moved, when there are no `batches`, or when this is the
    /// [`FULL_FRAME_INTERVAL`]th changed frame since the last full one; else
    /// `Partial`. A full frame resets the count; a partial one advances it.
    pub(crate) fn decide(&mut self, epoch: FontEpoch, batches: usize, changed: bool) -> FrameKind {
        let epoch_moved = self.epoch.is_some_and(|last| last != epoch);
        self.epoch = Some(epoch);
        if !changed && !self.force_full && !epoch_moved {
            return FrameKind::Nothing;
        }
        let forced = std::mem::take(&mut self.force_full);
        if forced
            || epoch_moved
            || batches == 0
            || self.changed_since_full + 1 >= FULL_FRAME_INTERVAL
        {
            self.changed_since_full = 0;
            self.full_frames_total += 1;
            FrameKind::Full { forced }
        } else {
            self.changed_since_full += 1;
            FrameKind::Partial
        }
    }

    pub(crate) fn full_frames_total(&self) -> u64 {
        self.full_frames_total
    }

    pub(crate) fn atlas_full_total(&self) -> u64 {
        self.atlas_full_total
    }
}

/// The batches the trim rule counts: the UI's, and the world labels as one
/// when there are any (`D-161`), so a frame of labels without UI defers its
/// trim as a frame of UI does.
pub(crate) fn trim_batches(ui_batches: usize, labels: &[WorldLabelRun]) -> usize {
    ui_batches + usize::from(!labels.is_empty())
}

/// One text command's draw inputs: everything glyphon bakes into the batch's
/// vertices, so an equal signature draws the same pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandSignature {
    node: TextNodeId,
    /// The node's revision; `None` for a node the engine does not hold.
    revision: Option<u64>,
    position: [u32; 2],
    size: [u32; 2],
    color: [u8; 4],
    /// The command's own text scale (M56).
    scale_bits: u32,
    /// The clip's rectangle, never its index (M46 critique 1.5).
    clip: ClipRect,
}

/// A batch's draw inputs: its commands', the paint scale and the viewport
/// glyphon clamps bounds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchSignature {
    scale_bits: u32,
    viewport: (u32, u32),
    commands: Vec<CommandSignature>,
}

/// The signature of the batch at `range` of `list`, drawn at `width` ×
/// `height`.
pub(crate) fn batch_signature(
    list: &PaintList,
    range: Range<usize>,
    engine: &TextEngine,
    width: u32,
    height: u32,
) -> BatchSignature {
    let commands = list.commands[range]
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Text {
                node,
                rect,
                color,
                scale,
                ..
            } => Some(CommandSignature {
                node: *node,
                revision: engine.node_revision(*node),
                position: rect.min.to_array().map(f32::to_bits),
                size: rect.size().to_array().map(f32::to_bits),
                color: *color,
                scale_bits: scale.to_bits(),
                clip: list.clip_of(command),
            }),
            _ => None,
        })
        .collect();
    BatchSignature {
        scale_bits: list.scale.to_bits(),
        viewport: (width, height),
        commands,
    }
}

/// The bounds a text command draws inside: its box intersected with its clip,
/// under the rounding rule's floor and ceil, so an unwrapped line never
/// leaves the box the partition rule reasons about (M46 critique 2.3).
pub(crate) fn text_bounds(rect: Rect, clip: ClipRect) -> TextBounds {
    let left = (rect.min.x.floor() as i32).max(clip.x);
    let top = (rect.min.y.floor() as i32).max(clip.y);
    let right = (rect.max.x.ceil() as i32).min(clip.x + clip.width);
    let bottom = (rect.max.y.ceil() as i32).min(clip.y + clip.height);
    TextBounds {
        left,
        top,
        right: right.max(left),
        bottom: bottom.max(top),
    }
}

#[derive(Debug, Default)]
struct BatchEntry {
    /// The signature the batch's renderer was prepared with; `None` before
    /// its first prepare, after a failed one and after a frame without it.
    signature: Option<BatchSignature>,
}

/// What each batch index holds across frames, GPU-free.
#[derive(Debug, Default)]
pub(crate) struct BatchBook {
    entries: Vec<BatchEntry>,
}

impl BatchBook {
    /// Which batches of `frame` must prepare: a new index, a changed
    /// signature, a failed or absent last prepare. Indices beyond the frame's
    /// count lose their signature, so a batch that disappears and returns
    /// prepares again rather than drawing vertices over recycled atlas slots
    /// (M46 critique 2.2).
    pub(crate) fn changed(&mut self, frame: &[BatchSignature]) -> Vec<bool> {
        if self.entries.len() < frame.len() {
            self.entries.resize_with(frame.len(), BatchEntry::default);
        }
        for entry in &mut self.entries[frame.len()..] {
            entry.signature = None;
        }
        frame
            .iter()
            .zip(&self.entries)
            .map(|(signature, entry)| entry.signature.as_ref() != Some(signature))
            .collect()
    }

    /// Records a successful prepare.
    pub(crate) fn mark_prepared(&mut self, batch: usize, signature: BatchSignature) {
        if let Some(entry) = self.entries.get_mut(batch) {
            entry.signature = Some(signature);
        }
    }

    /// Records a failed prepare: the batch is not drawn and prepares again.
    pub(crate) fn mark_failed(&mut self, batch: usize) {
        if let Some(entry) = self.entries.get_mut(batch) {
            entry.signature = None;
        }
    }

    /// Whether `batch` holds vertices its last prepare completed.
    pub(crate) fn drawable(&self, batch: usize) -> bool {
        self.entries
            .get(batch)
            .is_some_and(|entry| entry.signature.is_some())
    }
}

/// The text areas of the batch at `range`: one per text command whose node
/// is committed under the current font epoch, at the command's physical box,
/// scaled by the paint scale (`D-127`) times the command's (M56) and bounded
/// by its box ∩ clip.
fn batch_areas<'a>(
    list: &'a PaintList,
    range: Range<usize>,
    nodes: &NodeAreas<'a>,
    out: &mut Vec<TextArea<'a>>,
) {
    for command in &list.commands[range] {
        let PaintCommand::Text {
            node,
            rect,
            color,
            scale,
            ..
        } = command
        else {
            continue;
        };
        let Some((buffer, _)) = nodes.area(*node) else {
            continue;
        };
        let [r, g, b, a] = *color;
        out.push(TextArea {
            buffer,
            left: rect.min.x,
            top: rect.min.y,
            scale: list.scale * scale,
            bounds: text_bounds(*rect, list.clip_of(command)),
            default_color: Color::rgba(r, g, b, a),
            custom_glyphs: &[],
        });
    }
}

/// The prewarm's text areas (M56): every text command of `list`, as a
/// batch's, at its box and scale.
fn prewarm_areas<'a>(list: &'a PaintList, nodes: &NodeAreas<'a>, out: &mut Vec<TextArea<'a>>) {
    batch_areas(list, 0..list.commands.len(), nodes, out);
}

/// Glyphs in the laid-out lines of `areas`' buffers.
fn area_glyphs(areas: &[TextArea<'_>]) -> u32 {
    areas
        .iter()
        .map(|area| {
            area.buffer
                .layout_runs()
                .map(|run| run.glyphs.len() as u32)
                .sum::<u32>()
        })
        .sum()
}

/// Glyphon text pipeline state: the [`TextEngine`] that lays text out, and
/// the atlas, renderers, viewport and swash cache that draw it.
pub struct TextPipeline {
    engine: TextEngine,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    /// The screen-space sections' renderer.
    text_renderer: TextRenderer,
    /// Whether the sections' last prepare completed, so they draw.
    sections_drawable: bool,
    /// The world labels' renderer (`D-161`).
    labels_renderer: TextRenderer,
    /// Whether the labels' last prepare completed, so they draw.
    labels_drawable: bool,
    /// The labels of the next prepare, from [`Self::set_world_labels`].
    world_labels: Vec<WorldLabelRun>,
    /// One renderer per UI batch index, grown and never shrunk.
    batches: Vec<TextRenderer>,
    /// The prewarm's renderer, made at the first prewarm and never drawn.
    prewarm_renderer: Option<TextRenderer>,
    /// Batches in the last prepared paint list.
    batch_count: usize,
    book: BatchBook,
    policy: TrimPolicy,
    stats: TextStats,
    /// Glyphon `render` calls since the renderer last took them.
    counts: Box<Counts>,
}

impl TextPipeline {
    /// Text renders in its own overlay pass after the post stack, which always
    /// targets a single-sample color texture with no depth, so the pipeline
    /// bakes those attachment bits whatever the scene's MSAA and depth config.
    ///
    /// Text draws with packaged fonts only ([`FontSource::Packaged`]).
    #[must_use]
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        Self::with_font_source(device, queue, format, FontSource::Packaged)
    }

    /// [`Self::new`] with the faces `source` allows (`D-116`).
    #[must_use]
    pub fn with_font_source(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        source: FontSource,
    ) -> Self {
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            device,
            MULTISAMPLE,
            crate::quad::passthrough_depth_stencil(false),
        );
        let labels_renderer = TextRenderer::new(
            &mut atlas,
            device,
            MULTISAMPLE,
            crate::quad::passthrough_depth_stencil(false),
        );

        Self {
            engine: TextEngine::new(source),
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            sections_drawable: true,
            labels_renderer,
            labels_drawable: false,
            world_labels: Vec::new(),
            batches: Vec::new(),
            prewarm_renderer: None,
            batch_count: 0,
            book: BatchBook::default(),
            policy: TrimPolicy::default(),
            stats: TextStats::default(),
            counts: Counts::boxed(),
        }
    }

    /// Load font bytes under manifest ID.
    pub fn load_font(&mut self, id: &str, data: Vec<u8>) {
        self.engine.load_font(id, data);
    }

    /// Hot-reload font bytes and evict dependent caches: the next frame that
    /// prepares is full, so the atlas trims before every batch re-prepares.
    /// False when the data holds no face: the font keeps its old faces.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) -> bool {
        let reloaded = self.engine.reload_font(id, data);
        if reloaded {
            self.policy.force_full();
        }
        reloaded
    }

    pub(crate) fn engine(&self) -> &TextEngine {
        &self.engine
    }

    /// Crate-private: the atlas and swash cache key glyphs by the engine's
    /// fontdb IDs, so the engine must not be swapped under them.
    pub(crate) fn engine_mut(&mut self) -> &mut TextEngine {
        &mut self.engine
    }

    /// Set the font families and the fallback chain (`D-115`).
    pub(crate) fn set_font_families(
        &mut self,
        families: &HashMap<String, ResolvedFontFamily>,
        fallback: &[String],
    ) {
        self.engine.set_font_families(families, fallback);
    }

    /// Sets the world labels the next [`Self::prepare`] lays out, reusing
    /// the strings of the last call's.
    pub(crate) fn set_world_labels(&mut self, labels: &[WorldLabelRun]) {
        copy_runs(&mut self.world_labels, labels);
    }

    /// The frame's text telemetry, from the last `prepare`.
    #[must_use]
    pub fn stats(&self) -> TextStats {
        self.stats
    }

    /// Prepares the sections, the world labels of the last
    /// `set_world_labels` and the UI text batches of `ui` for an attachment
    /// of `width` × `height` under the trim rule: a full frame trims first
    /// and prepares everything; a partial one only the batches whose
    /// signature changed and, when changed, the labels and the sections; a
    /// frame with nothing changed prepares nothing. A batch, the labels or
    /// the sections whose prepare returns `AtlasFull` are not drawn until a
    /// prepare completes, and the next frame is full. An empty label list
    /// prepares and draws nothing.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        sections: &[TextSection],
        ui: &PaintList,
        width: u32,
        height: u32,
    ) {
        self.viewport.update(queue, Resolution { width, height });

        let ranges = ui.text_batches();
        let signatures: Vec<BatchSignature> = ranges
            .iter()
            .map(|range| batch_signature(ui, range.clone(), &self.engine, width, height))
            .collect();
        let changed = self.book.changed(&signatures);
        let update = self
            .engine
            .update_frame(sections, &self.world_labels, width, height);
        let any_changed =
            update.sections || update.labels || changed.iter().any(|changed| *changed);
        let kind = self.policy.decide(
            self.engine.font_epoch(),
            trim_batches(ranges.len(), &self.world_labels),
            any_changed,
        );
        self.batch_count = ranges.len();
        if self.world_labels.is_empty() {
            self.labels_drawable = false;
        }
        self.stats = TextStats {
            batches: ranges.len() as u32,
            prepared: 0,
            full_frame: false,
            forced: false,
            atlas_full_total: self.policy.atlas_full_total(),
            full_frames_total: self.policy.full_frames_total(),
            labels: 0,
            label_glyphs: 0,
            layouts: self.engine.layout_count() as u32,
        };
        let full = match kind {
            FrameKind::Nothing => {
                self.stats.labels = self.drawn_labels();
                return;
            }
            FrameKind::Partial => false,
            FrameKind::Full { forced } => {
                // Every retained batch re-prepares below, so nothing a
                // retained batch samples can be evicted by this trim.
                self.atlas.trim();
                self.stats.full_frame = true;
                self.stats.forced = forced;
                true
            }
        };

        while self.batches.len() < ranges.len() {
            self.batches.push(TextRenderer::new(
                &mut self.atlas,
                device,
                MULTISAMPLE,
                crate::quad::passthrough_depth_stencil(false),
            ));
        }
        let (font_system, nodes) = self.engine.batch_parts();
        let mut areas = Vec::new();
        for (batch, range) in ranges.iter().enumerate() {
            if !full && !changed[batch] {
                continue;
            }
            areas.clear();
            batch_areas(ui, range.clone(), &nodes, &mut areas);
            match self.batches[batch].prepare(
                device,
                queue,
                font_system,
                &mut self.atlas,
                &self.viewport,
                areas.drain(..),
                &mut self.swash_cache,
            ) {
                Ok(()) => {
                    self.book.mark_prepared(batch, signatures[batch].clone());
                    self.stats.prepared += 1;
                }
                Err(PrepareError::AtlasFull) => {
                    self.book.mark_failed(batch);
                    self.policy.atlas_full();
                    log::warn!(
                        "Text batch {batch} prepare: the glyph atlas is full; the batch is not drawn this frame"
                    );
                }
            }
        }

        if !self.world_labels.is_empty() && (full || update.labels) {
            let (font_system, areas) = self.engine.label_prepare_parts(width, height);
            match self.labels_renderer.prepare(
                device,
                queue,
                font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash_cache,
            ) {
                Ok(()) => {
                    self.engine.mark_labels_prepared(width, height);
                    self.labels_drawable = true;
                    self.stats.label_glyphs = self.engine.label_glyphs();
                }
                Err(PrepareError::AtlasFull) => {
                    self.labels_drawable = false;
                    self.policy.atlas_full();
                    log::warn!(
                        "World label prepare: the glyph atlas is full; the labels are not drawn this frame"
                    );
                }
            }
        }

        if full || update.sections {
            let (font_system, areas) = self.engine.prepare_parts();
            match self.text_renderer.prepare(
                device,
                queue,
                font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash_cache,
            ) {
                Ok(()) => {
                    self.engine.mark_prepared(width, height);
                    self.sections_drawable = true;
                }
                Err(PrepareError::AtlasFull) => {
                    self.sections_drawable = false;
                    self.policy.atlas_full();
                    log::error!(
                        "Text prepare: the glyph atlas is full; the sections are not drawn this frame"
                    );
                }
            }
        }
        self.stats.atlas_full_total = self.policy.atlas_full_total();
        self.stats.full_frames_total = self.policy.full_frames_total();
        self.stats.labels = self.drawn_labels();
    }

    /// Rasterizes the text runs of `list` into the shared atlas without
    /// drawing them (M56): updates the viewport to `width` × `height`,
    /// prepares the runs on a renderer of their own and logs `UI prewarm:
    /// <runs> text runs, <glyphs> glyphs in <ms> ms`. The same positions
    /// and scale give the same glyph keys, so a later frame's prepare finds
    /// the glyphs; they stay until the atlas needs their room after a full
    /// frame's trim (`D-126`). An `AtlasFull` result counts toward the trim
    /// rule as a batch's does.
    pub fn prewarm(
        &mut self,
        device: &Device,
        queue: &Queue,
        list: &PaintList,
        width: u32,
        height: u32,
    ) -> TextPrewarm {
        let start = std::time::Instant::now();
        self.viewport.update(queue, Resolution { width, height });
        let renderer = self.prewarm_renderer.get_or_insert_with(|| {
            TextRenderer::new(
                &mut self.atlas,
                device,
                MULTISAMPLE,
                crate::quad::passthrough_depth_stencil(false),
            )
        });
        let (font_system, nodes) = self.engine.batch_parts();
        let mut areas = Vec::new();
        prewarm_areas(list, &nodes, &mut areas);
        let runs = areas.len() as u32;
        let glyphs = area_glyphs(&areas);
        if let Err(PrepareError::AtlasFull) = renderer.prepare(
            device,
            queue,
            font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        ) {
            self.policy.atlas_full();
            log::warn!("UI prewarm: the glyph atlas is full; the next frame trims");
        }
        let micros = start.elapsed().as_micros() as u64;
        log::info!(
            "UI prewarm: {runs} text runs, {glyphs} glyphs in {:.3} ms",
            micros as f64 / 1000.0
        );
        TextPrewarm {
            runs,
            glyphs,
            micros,
        }
    }

    /// The labels the overlay draws: the last prepared list when its
    /// prepare completed, else none.
    fn drawn_labels(&self) -> u32 {
        if self.labels_drawable {
            self.world_labels.len() as u32
        } else {
            0
        }
    }

    /// Draws the prepared world labels, when there are some and their last
    /// prepare completed.
    pub(crate) fn render_labels<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.labels_drawable || self.world_labels.is_empty() {
            return;
        }
        self.counts.text_render();
        if let Err(e) = self
            .labels_renderer
            .render(&self.atlas, &self.viewport, pass)
        {
            log::error!("World label render error: {e:?}");
        }
    }

    /// Whether the overlay has world labels to draw this frame.
    pub(crate) fn has_labels(&self) -> bool {
        self.labels_drawable && !self.world_labels.is_empty()
    }

    /// Adds the overlay's glyphon `render` calls to `total`.
    pub(crate) fn take_counts(&self, total: &mut crate::FrameCounters) {
        self.counts.take_into(total);
    }

    /// Draws the prepared sections.
    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.sections_drawable {
            return;
        }
        self.counts.text_render();
        if let Err(e) = self.text_renderer.render(&self.atlas, &self.viewport, pass) {
            log::error!("Text render error: {e:?}");
        }
    }

    /// Draws UI text batch `batch` of the last prepared paint list, when its
    /// last prepare completed.
    pub(crate) fn render_batch<'pass>(&'pass self, pass: &mut RenderPass<'pass>, batch: usize) {
        if batch >= self.batch_count || !self.book.drawable(batch) {
            return;
        }
        let Some(renderer) = self.batches.get(batch) else {
            return;
        };
        self.counts.text_render();
        if let Err(e) = renderer.render(&self.atlas, &self.viewport, pass) {
            log::error!("Text batch {batch} render error: {e:?}");
        }
    }
}

/// Copies `src` into `dst`, reusing `dst`'s strings for the runs both hold.
fn copy_runs(dst: &mut Vec<WorldLabelRun>, src: &[WorldLabelRun]) {
    dst.truncate(src.len());
    for (to, from) in dst.iter_mut().zip(src) {
        to.content.clone_from(&from.content);
        to.font_id.clone_from(&from.font_id);
        to.font_size = from.font_size;
        to.line_height = from.line_height;
        to.position = from.position;
        to.pivot = from.pivot;
        to.scale = from.scale;
        to.color = from.color;
    }
    let kept = dst.len();
    dst.extend_from_slice(&src[kept..]);
}

#[cfg(test)]
#[path = "../tests/text_batches.rs"]
mod tests;
