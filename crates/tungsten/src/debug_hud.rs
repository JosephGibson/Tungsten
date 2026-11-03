//! Runtime developer HUD; toggled by `engine_toggle_hud`. From M57 a
//! movable window on the core UI (`debug_views`): the fps row, the built-in
//! rows and the game's, refreshed on the frame interval, under a graph of
//! the last 240 frame intervals redrawn every frame. From M58 the graph sits
//! in the fixed header at 240 units wide, with guide lines at 16.7 and
//! 33.3 ms and its least, mean, most and 1% low beside it, its bars and
//! times on the budget ramp; the averages start from the first interval,
//! and the values read `–` until it arrives. From debug-pass the built-in
//! rows read in words, fields apart by ` · `, each value in the view's
//! colour and its names and units in the palette's grey.

use std::collections::VecDeque;

use tungsten_core::World;
use tungsten_core::camera::{CameraController, CameraMode, CameraState};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::text::{StyledText, TextAlign, TextSpan};
use tungsten_core::ui::{Sizing, UiVertex, WidgetId};
use tungsten_render::GpuFrameTimings;

use crate::debug_views::{
    BORDER, Cell, Column, FRAME_BUDGET_MS, GRAPH_SAMPLES, Header, LABEL, MISSING, PANEL, Row,
    STRONG, Shape, Tone, VALUE, ViewFrame, ViewSpec, ViewWindow, Width, graph_stats,
    graph_vertices, look_of, ramp, toggle_key, write_table,
};
use crate::telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};

/// Screen-anchor corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Diagnostic row.
#[derive(Debug, Clone)]
pub struct HudRow {
    pub label: &'static str,
    pub value: String,
}

/// HUD row provider; missing resources should return no rows.
pub type HudRowProvider = Box<dyn Fn(&World) -> Vec<HudRow> + 'static>;

/// Active state hint for custom diagnostics.
#[derive(Debug, Clone, Default)]
pub struct HudActiveState(pub String);

/// Runtime developer HUD resource: a window on the UI's debug layer (M57).
pub struct DebugHud {
    pub enabled: bool,
    /// The corner the window docks at when no place is saved for it: the
    /// top left (M58).
    pub corner: HudCorner,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default (M57).
    pub font_id: String,
    /// Text size in UI units; 15 (M58), titles two more.
    pub font_size: f32,
    /// Line height in UI units; 20 (M58).
    pub line_height: f32,
    /// The values' and the title's colour; labels take the palette's grey
    /// (M58).
    pub color: [u8; 4],
    /// The window's fill (M57).
    pub background: [u8; 4],
    /// The window's border colour; the text outline's before M57; brighter
    /// while the pointer is over the window (M58).
    pub outline_color: [u8; 4],
    /// The window's border width in physical pixels (M58); `0.0` draws
    /// none.
    pub outline_px: f32,
    /// Space between the window and its corner, and below it for the next
    /// window at that corner, in UI units; 8 (M58).
    pub padding_px: f32,
    /// Row refresh interval on the frame interval; the averages and the
    /// graph still update every frame.
    pub refresh_interval_ms: f32,
    interval_ewma: f32,
    total_ewma: f32,
    ewma_alpha: f32,
    /// Whether an interval has arrived: the averages start from the first
    /// (M58).
    seeded: bool,
    time_since_refresh_ms: f32,
    /// The last [`GRAPH_SAMPLES`] frame intervals, oldest first.
    samples: VecDeque<f32>,
    /// The graph's vertices, reused.
    vertices: Vec<UiVertex>,
    /// The graph's samples, contiguous for the mesh, reused.
    graph_samples: Vec<f32>,
    rendered_height: f32,
    pub(crate) window: Option<ViewWindow>,
    built_in: Vec<fn(&World) -> Vec<Line>>,
    custom: Vec<HudRowProvider>,
}

/// The HUD's columns: labels right-aligned, then values.
const COLUMNS: &[Column] = &[
    Column::new(TextAlign::End, Width::Capped(40), Tone::Label),
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
];

impl DebugHud {
    #[must_use]
    pub fn new() -> Self {
        // HUD stays render-focused; systems/inspector own deeper views.
        let built_in: Vec<fn(&World) -> Vec<Line>> = vec![
            gpu_line,
            display_line,
            camera_line,
            counts_line,
            render_line,
            ui_lines,
        ];
        Self {
            enabled: false,
            corner: HudCorner::TopLeft,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 15.0,
            line_height: 20.0,
            color: VALUE,
            background: PANEL,
            outline_color: BORDER,
            outline_px: 1.0,
            padding_px: 8.0,
            refresh_interval_ms: 500.0,
            interval_ewma: 0.0,
            total_ewma: 0.0,
            ewma_alpha: 0.1,
            seeded: false,
            time_since_refresh_ms: f32::INFINITY,
            samples: VecDeque::with_capacity(GRAPH_SAMPLES),
            vertices: Vec::new(),
            graph_samples: Vec::new(),
            rendered_height: 0.0,
            window: None,
            built_in,
            custom: Vec::new(),
        }
    }

    /// Toggle visibility.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// The HUD window's laid-out height plus [`Self::padding_px`], in
    /// physical pixels at the debug layer's scale (M58): where a block below
    /// it at its corner starts. 0 while hidden or before the window's first
    /// layout.
    #[must_use]
    pub fn rendered_height_px(&self) -> f32 {
        if self.enabled {
            self.rendered_height
        } else {
            0.0
        }
    }

    /// The window's root while the HUD is shown (M57).
    #[must_use]
    pub fn window(&self) -> Option<WidgetId> {
        self.window.as_ref().map(|window| window.root)
    }

    /// Register custom rows after built-ins.
    pub fn add_row<F>(&mut self, provider: F)
    where
        F: Fn(&World) -> Vec<HudRow> + 'static,
    {
        self.custom.push(Box::new(provider));
    }

    /// The fps row's cells (M58): `1000 / average interval`, then the
    /// average CPU frame time at the strong weight in the budget ramp's
    /// colour, ` 59.9 · frame  2.80 ms`; `–` before the first interval.
    fn fps_cells(&self) -> Vec<Cell> {
        let label = Cell::new("fps");
        if !self.seeded {
            return vec![label, Cell::new(MISSING)];
        }
        let fps = if self.interval_ewma > 0.0 {
            1000.0 / self.interval_ewma
        } else {
            0.0
        };
        let value = Spans::default()
            .value(format!("{fps:>5.1}"))
            .name(" \u{b7} frame ")
            .time(self.total_ewma);
        vec![label, Cell::styled(value.text())]
    }

    /// The HUD's frame (M57): the averages, the graph and, on its interval,
    /// the rows, from the previous frame's `FrameTimings`; while it is
    /// shown. Its window opens, closes and moves through `frame`; closed, it
    /// drops its intervals and averages (debug-pass).
    pub(crate) fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        if frame.closes(self.window.as_ref()) {
            self.enabled = false;
        }
        let spec = ViewSpec {
            name: "hud",
            title: "HUD",
            key: toggle_key(world, "engine_toggle_hud"),
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            strip: false,
            look: look_of!(self),
            shape: Shape {
                columns: COLUMNS,
                header: Header::Graph,
                buttons: 0,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            // A closed HUD keeps no frame from before it closed, so a reopened
            // one starts as a new one does (debug-pass); the buffers keep
            // their capacity.
            self.time_since_refresh_ms = f32::INFINITY;
            self.rendered_height = 0.0;
            self.samples.clear();
            self.graph_samples.clear();
            self.vertices.clear();
            self.interval_ewma = 0.0;
            self.total_ewma = 0.0;
            self.seeded = false;
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let (interval, total) = world
            .get_resource::<FrameTimings>()
            .map_or((None, None), |ft| (ft.interval_ms, Some(ft.total_ms)));
        let total = total.filter(|ms| ms.is_finite());
        let interval = interval.filter(|ms| ms.is_finite() && *ms >= 0.0);
        let alpha = self.ewma_alpha;
        if self.seeded {
            if let Some(total) = total {
                self.total_ewma = (1.0 - alpha) * self.total_ewma + alpha * total;
            }
            if let Some(interval) = interval {
                self.interval_ewma = (1.0 - alpha) * self.interval_ewma + alpha * interval;
            }
        } else if let Some(interval) = interval {
            // The averages start from the first interval (M58), and the rows
            // show it at once.
            self.seeded = true;
            self.interval_ewma = interval;
            self.total_ewma = total.unwrap_or(0.0);
            self.time_since_refresh_ms = f32::INFINITY;
        }
        if let Some(interval) = interval {
            if self.samples.len() == GRAPH_SAMPLES {
                self.samples.pop_front();
            }
            self.samples.push_back(interval);
        }
        if let Some(graph) = window.graph {
            self.graph_samples.clear();
            self.graph_samples.extend(self.samples.iter().copied());
            let height = frame
                .tree
                .layout_style(graph)
                .and_then(|style| match style.height {
                    Sizing::Px(height) => Some(height),
                    _ => None,
                })
                .unwrap_or(0.0);
            graph_vertices(&self.graph_samples, height, &mut self.vertices);
            frame.tree.set_mesh(graph, &self.vertices);
        }

        self.time_since_refresh_ms += interval.unwrap_or(0.0);
        if self.time_since_refresh_ms >= self.refresh_interval_ms {
            self.time_since_refresh_ms = 0.0;
            window.write_stats(frame.tree, graph_stats(&self.graph_samples));
            let mut rows = vec![Row::data(self.fps_cells())];
            for provider in &self.built_in {
                for line in provider(world) {
                    // The frame's CPU times are 0 until it has run once.
                    let value = if line.label == RENDER && !self.seeded {
                        Cell::new(MISSING)
                    } else {
                        Cell::styled(line.value.text())
                    };
                    rows.push(Row::data(vec![Cell::new(line.label), value]));
                }
            }
            for provider in &self.custom {
                for row in provider(world) {
                    rows.push(Row::data(vec![Cell::new(row.label), Cell::new(row.value)]));
                }
            }
            write_table(frame.tree, window, &rows);
        }
        let unit = frame.scale * frame.tree.debug_scale();
        self.rendered_height = frame
            .tree
            .rect(window.root)
            .map_or(0.0, |rect| (rect.height() + self.padding_px) * unit);
    }
}

impl Default for DebugHud {
    fn default() -> Self {
        Self::new()
    }
}

/// The render row's label, whose CPU times read `–` until the frame has
/// run once.
const RENDER: &str = "render";

/// A built-in row (debug-pass): its label and its value's spans.
struct Line {
    label: &'static str,
    value: Spans,
}

/// A built-in row's value as spans (debug-pass): values, numbers and
/// states, in the view's colour; their names, units and separators in the
/// palette's grey; a time on the budget ramp at the strong weight.
#[derive(Default)]
struct Spans(Vec<TextSpan>);

impl Spans {
    /// `text` in the view's colour.
    fn value(mut self, text: impl Into<String>) -> Self {
        self.0.push(TextSpan::new(text));
        self
    }

    /// `text` in the palette's grey: a name, a unit or a separator.
    fn name(mut self, text: impl Into<String>) -> Self {
        self.0.push(TextSpan::new(text).with_color(LABEL));
        self
    }

    /// `n` with its thousands apart, then `one` or `many` as `n` is 1 or
    /// not: `1 root`, `9 nodes`.
    fn count(self, n: impl Into<u64>, one: &str, many: &str) -> Self {
        let n = n.into();
        let noun = if n == 1 { one } else { many };
        self.value(grouped(n)).name(format!(" {noun}"))
    }

    /// `ms` five wide to two places, on the frame's budget ramp at the
    /// strong weight, then its unit.
    fn time(mut self, ms: f32) -> Self {
        self.0.push(
            TextSpan::new(format!("{ms:>5.2}"))
                .with_weight(STRONG)
                .with_color(ramp(ms, FRAME_BUDGET_MS)),
        );
        self.name(" ms")
    }

    /// The row labelled `label`.
    fn line(self, label: &'static str) -> Line {
        Line { label, value: self }
    }

    fn text(self) -> StyledText {
        StyledText { spans: self.0 }
    }
}

/// `n` with its thousands apart: `12,345`.
fn grouped(n: impl Into<u64>) -> String {
    let digits = n.into().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `gpu`: the backend, then the GPU's frame time on the budget ramp, `–`
/// while none is timed.
fn gpu_line(world: &World) -> Vec<Line> {
    let Some(gpu) = world.get_resource::<GpuFrameTimings>() else {
        return Vec::new();
    };
    let value = Spans::default()
        .value(gpu.backend.as_deref().unwrap_or("?"))
        .name(" \u{b7} ");
    let value = match gpu.frame_gpu_ms {
        Some(ms) => value.time(ms),
        None => value.value(MISSING),
    };
    vec![value.line("gpu")]
}

/// `display`: the size, the mode and vsync, `1280×720 · windowed · vsync off`.
fn display_line(world: &World) -> Vec<Line> {
    let Some(display) = world.get_resource::<DisplayTelemetry>() else {
        return Vec::new();
    };
    let (width, height) = display.resolution;
    vec![
        Spans::default()
            .value(format!("{width}\u{d7}{height}"))
            .name(" \u{b7} ")
            .value(display.display_mode.as_str())
            .name(" \u{b7} vsync ")
            .value(if display.vsync { "on" } else { "off" })
            .line("display"),
    ]
}

/// `camera`: its position, zoom and mode, `(0, 1635) · zoom 0.62 · follow`.
fn camera_line(world: &World) -> Vec<Line> {
    let (Some(state), Some(controller)) = (
        world.get_resource::<CameraState>(),
        world.get_resource::<CameraController>(),
    ) else {
        return Vec::new();
    };
    let mode = match controller.mode {
        CameraMode::Free => "free",
        CameraMode::Follow(_) => "follow",
        CameraMode::Scripted => "scripted",
    };
    vec![
        Spans::default()
            .value(format!(
                "({:.0}, {:.0})",
                state.position.x, state.position.y
            ))
            .name(" \u{b7} zoom ")
            .value(format!("{:.2}", state.zoom))
            .name(" \u{b7} ")
            .value(mode)
            .line("camera"),
    ]
}

/// `draw`: the entities and the sprites drawn, `1,429 entities · 457 sprites`.
fn counts_line(world: &World) -> Vec<Line> {
    let Some(counts) = world.get_resource::<RenderCounts>() else {
        return Vec::new();
    };
    vec![
        Spans::default()
            .count(counts.entities, "entity", "entities")
            .name(" \u{b7} ")
            .count(counts.sprite_instances, "sprite", "sprites")
            .line("draw"),
    ]
}

/// `render`: the render stage's CPU times, acquire, encode and submit with
/// present, in ms.
fn render_line(world: &World) -> Vec<Line> {
    let Some(ft) = world.get_resource::<FrameTimings>() else {
        return Vec::new();
    };
    vec![
        Spans::default()
            .name("acq ")
            .value(format!("{:.2}", ft.render_acquire_ms))
            .name(" \u{b7} enc ")
            .value(format!("{:.2}", ft.render_encode_ms))
            .name(" \u{b7} sub ")
            .value(format!("{:.2}", ft.render_submit_present_ms))
            .name(" ms")
            .line(RENDER),
    ]
}

/// The text row's value (debug-pass): batches prepared, cached layouts and
/// full frames, which trim the atlas, then the times the atlas was full
/// once it has been.
fn text_spans(rc: &RenderCounts) -> Spans {
    let spans = Spans::default()
        .value(grouped(rc.text_batches_prepared))
        .name(" prepared \u{b7} ")
        .count(rc.text_layouts, "layout", "layouts")
        .name(" \u{b7} ")
        .count(rc.text_full_frames, "trim", "trims");
    if rc.text_atlas_full == 0 {
        return spans;
    }
    spans
        .name(" \u{b7} ")
        .value(grouped(rc.text_atlas_full))
        .name(" atlas full")
}

/// M46: the UI paint list's size, the layout stage's cost and the text
/// batch telemetry (`D-126`'s observable half, Q3); M54: the world labels
/// kept, culled and dropped, their glyphs and the cached layouts. From
/// debug-pass four rows, each a value the HUD's width holds at the views'
/// floor: `ui` (paint commands, text batches), `layout` (µs, roots and
/// nodes laid out), `text` ([`text_spans`]) and `labels` (world labels
/// kept, culled and dropped, glyphs).
fn ui_lines(world: &World) -> Vec<Line> {
    let Some(rc) = world.get_resource::<RenderCounts>() else {
        return Vec::new();
    };
    vec![
        Spans::default()
            .count(rc.ui_commands, "command", "commands")
            .name(" \u{b7} ")
            .count(rc.text_batches, "text batch", "text batches")
            .line("ui"),
        Spans::default()
            .value(grouped(rc.ui_layout_us))
            .name(" \u{b5}s \u{b7} ")
            .count(rc.ui_roots_laid_out, "root", "roots")
            .name(" \u{b7} ")
            .count(rc.ui_nodes_laid_out, "node", "nodes")
            .line("layout"),
        text_spans(rc).line("text"),
        Spans::default()
            .value(grouped(rc.world_labels))
            .name(" \u{b7} ")
            .value(grouped(rc.world_labels_culled))
            .name(" culled \u{b7} ")
            .value(grouped(rc.world_labels_dropped))
            .name(" dropped \u{b7} ")
            .count(rc.text_label_glyphs, "glyph", "glyphs")
            .line("labels"),
    ]
}

/// Toggle HUD on `engine_toggle_hud` edge.
pub fn hud_toggle_system(world: &mut World) {
    let pressed = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        actions.just_pressed(input, "engine_toggle_hud")
    };
    if pressed && let Some(hud) = world.get_resource_mut::<DebugHud>() {
        hud.toggle();
    }
}

#[cfg(test)]
#[path = "tests/debug_hud.rs"]
mod tests;
