//! Runtime developer HUD; toggled by `engine_toggle_hud`. From M57 a
//! movable window on the core UI (`debug_views`): the fps row, the built-in
//! rows and the game's, refreshed on the frame interval, under a graph of
//! the last 240 frame intervals redrawn every frame. From M58 the graph sits
//! in the fixed header at 240 units wide, with guide lines at 16.7 and
//! 33.3 ms and its least, mean, most and 1% low beside it, its bars and
//! times on the budget ramp; the averages start from the first interval,
//! and the values read `–` until it arrives. From debug-pass the built-in
//! rows set their fields in columns on the engine's monospace face, each
//! value in the view's colour and its name or unit in the palette's grey.

use std::collections::VecDeque;

use tungsten_core::World;
use tungsten_core::camera::{CameraController, CameraRig, CameraState};
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
    /// The GPU's render time's average, from the first timed frame read
    /// back (debug-pass): the app times the GPU while the HUD shows.
    gpu_ewma: Option<f32>,
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
            gpu_ewma: None,
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

    /// The fps row (M58): `1000 / average interval`, then the average CPU
    /// frame time and, from debug-pass, the average GPU render time, each
    /// in ms at the strong weight in the budget ramp's colour before its
    /// name, `59.6  4.75 cpu  1.23 gpu`; the GPU's `–` until a timed frame
    /// is read back, and the row's before the first interval.
    fn fps_line(&self) -> Line {
        if !self.seeded {
            return Line::new("fps", vec![field(MISSING)]);
        }
        let fps = if self.interval_ewma > 0.0 {
            1000.0 / self.interval_ewma
        } else {
            0.0
        };
        let gpu = match self.gpu_ewma {
            Some(ms) => Spans::default().time(ms, "gpu"),
            None => field(MISSING).name(" gpu"),
        };
        let fields = vec![
            field(format!("{fps:.1}")),
            Spans::default().time(self.total_ewma, "cpu"),
            gpu,
        ];
        Line::new("fps", fields)
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
            self.gpu_ewma = None;
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
        // Each timed frame read back this frame, from first scene timestamp
        // to the end of its last pass (debug-pass).
        if let Some(gpu) = world.get_resource::<GpuFrameTimings>() {
            for ms in gpu.samples.iter().map(|sample| sample.render_ms) {
                if ms.is_finite() && ms >= 0.0 {
                    self.gpu_ewma = Some(
                        self.gpu_ewma
                            .map_or(ms, |average| (1.0 - alpha) * average + alpha * ms),
                    );
                }
            }
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
            let mut lines = vec![self.fps_line()];
            for provider in &self.built_in {
                for line in provider(world) {
                    // The frame's CPU times are 0 until it has run once.
                    lines.push(if line.label == RENDER && !self.seeded {
                        Line::new(RENDER, vec![field(MISSING)])
                    } else {
                        line
                    });
                }
            }
            let mut rows: Vec<Row> = aligned(lines)
                .into_iter()
                .map(|(label, value)| Row::data(vec![Cell::new(label), Cell::styled(value)]))
                .collect();
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

/// Spaces between a row's columns (debug-pass).
const FIELD_GAP: usize = 2;

/// A built-in row (debug-pass): its label and its fields, each a value with
/// its name or unit, which [`aligned`] sets in columns across the rows.
struct Line {
    label: &'static str,
    fields: Vec<Spans>,
}

impl Line {
    fn new(label: &'static str, fields: Vec<Spans>) -> Self {
        Self { label, fields }
    }
}

/// One field of a built-in row as spans (debug-pass): its value in the
/// view's colour and its name or unit in the palette's grey; a time on the
/// budget ramp at the strong weight.
#[derive(Default)]
struct Spans(Vec<TextSpan>);

impl Spans {
    /// `text` in the view's colour.
    fn value(mut self, text: impl Into<String>) -> Self {
        self.0.push(TextSpan::new(text));
        self
    }

    /// `text` in the palette's grey: a name or a unit.
    fn name(mut self, text: impl Into<String>) -> Self {
        self.0.push(TextSpan::new(text).with_color(LABEL));
        self
    }

    /// `n` with its thousands apart, then `one` or `many` as `n` is 1 or
    /// not: `1 root`, `9 nodes`.
    fn count(self, n: impl Into<u64>, one: &str, many: &str) -> Self {
        let n = n.into();
        let unit = if n == 1 { one } else { many };
        self.value(grouped(n)).name(format!(" {unit}"))
    }

    /// `ms` to two places on the frame's budget ramp at the strong weight,
    /// then `name`.
    fn time(mut self, ms: f32, name: &str) -> Self {
        self.0.push(
            TextSpan::new(format!("{ms:.2}"))
                .with_weight(STRONG)
                .with_color(ramp(ms, FRAME_BUDGET_MS)),
        );
        self.name(format!(" {name}"))
    }

    /// Its width in glyphs, each as wide on the monospace face.
    fn width(&self) -> usize {
        self.0.iter().map(|span| span.text.chars().count()).sum()
    }
}

/// A field of `text` alone, in the view's colour.
fn field(text: impl Into<String>) -> Spans {
    Spans::default().value(text)
}

/// The built-in rows' values with their fields in columns (debug-pass):
/// every field but a row's last padded to the widest of its column among
/// the rows with a field after it, then [`FIELD_GAP`] spaces, so each
/// column starts at one place on the engine's monospace face.
fn aligned(lines: Vec<Line>) -> Vec<(&'static str, StyledText)> {
    let mut widths: Vec<usize> = Vec::new();
    for line in &lines {
        let inner = line.fields.len().saturating_sub(1);
        for (k, field) in line.fields[..inner].iter().enumerate() {
            if k == widths.len() {
                widths.push(0);
            }
            widths[k] = widths[k].max(field.width());
        }
    }
    lines
        .into_iter()
        .map(|line| {
            let last = line.fields.len().saturating_sub(1);
            let mut spans = Vec::new();
            for (k, field) in line.fields.into_iter().enumerate() {
                let pad = (k < last).then(|| widths[k] - field.width() + FIELD_GAP);
                spans.extend(field.0);
                if let Some(pad) = pad {
                    spans.push(TextSpan::new(" ".repeat(pad)));
                }
            }
            (line.label, StyledText { spans })
        })
        .collect()
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

/// `gpu`: the backend, then the newest timed frame's scene pass in ms, `–`
/// while none is read back; the fps row carries the whole render.
fn gpu_line(world: &World) -> Vec<Line> {
    let Some(gpu) = world.get_resource::<GpuFrameTimings>() else {
        return Vec::new();
    };
    let scene = match gpu.frame_gpu_ms {
        Some(ms) => field(format!("{ms:.2}")),
        None => field(MISSING),
    };
    let backend = field(gpu.backend.as_deref().unwrap_or("?"));
    vec![Line::new("gpu", vec![backend, scene.name(" scene")])]
}

/// `display`: the size, the mode and vsync.
fn display_line(world: &World) -> Vec<Line> {
    let Some(display) = world.get_resource::<DisplayTelemetry>() else {
        return Vec::new();
    };
    let (width, height) = display.resolution;
    let vsync = if display.vsync { "on" } else { "off" };
    vec![Line::new(
        "display",
        vec![
            field(format!("{width}\u{d7}{height}")),
            field(display.display_mode.as_str()),
            Spans::default().name("vsync ").value(vsync),
        ],
    )]
}

/// `camera`: its position, its zoom as a factor and the winning rig's
/// kind.
fn camera_line(world: &World) -> Vec<Line> {
    let (Some(state), Some(controller)) = (
        world.get_resource::<CameraState>(),
        world.get_resource::<CameraController>(),
    ) else {
        return Vec::new();
    };
    let winner = controller
        .winner()
        .and_then(|entity| world.get::<CameraRig>(entity))
        .unwrap_or(&controller.rig);
    let mode = winner.kind.name();
    vec![Line::new(
        "camera",
        vec![
            field(format!("{:.0}, {:.0}", state.position.x, state.position.y)),
            Spans::default()
                .name("\u{d7}")
                .value(format!("{:.2}", state.zoom)),
            field(mode),
        ],
    )]
}

/// `draw`: the entities and the sprites drawn.
fn counts_line(world: &World) -> Vec<Line> {
    let Some(counts) = world.get_resource::<RenderCounts>() else {
        return Vec::new();
    };
    vec![Line::new(
        "draw",
        vec![
            Spans::default().count(counts.entities, "ent", "ents"),
            Spans::default().count(counts.sprite_instances, "sprite", "sprites"),
        ],
    )]
}

/// `render`: the render stage's CPU times in ms, each number before its
/// name: acquire, encode and submit with present.
fn render_line(world: &World) -> Vec<Line> {
    let Some(ft) = world.get_resource::<FrameTimings>() else {
        return Vec::new();
    };
    let time = |ms: f32, name: &str| {
        Spans::default()
            .value(format!("{ms:.2}"))
            .name(format!(" {name}"))
    };
    vec![Line::new(
        RENDER,
        vec![
            time(ft.render_acquire_ms, "acq"),
            time(ft.render_encode_ms, "enc"),
            time(ft.render_submit_present_ms, "sub"),
        ],
    )]
}

/// M46: the UI paint list's size, the layout stage's cost and the text
/// batch telemetry (`D-126`'s observable half, Q3); M54: the world labels
/// kept, culled and dropped, and the cached layouts. From debug-pass four
/// rows of at most three fields, each held to the HUD's width at the
/// views' floor: `ui` (paint commands, text batches), `layout` (µs, roots
/// and nodes laid out), `text` (batches prepared, cached layouts, full
/// frames, which trim the atlas, and the times the atlas was full once it
/// has been) and `labels` (world labels drawn, culled and dropped).
fn ui_lines(world: &World) -> Vec<Line> {
    let Some(rc) = world.get_resource::<RenderCounts>() else {
        return Vec::new();
    };
    let mut text = vec![
        Spans::default()
            .value(grouped(rc.text_batches_prepared))
            .name(" prep"),
        Spans::default().count(rc.text_layouts, "layout", "layouts"),
        Spans::default().count(rc.text_full_frames, "trim", "trims"),
    ];
    if rc.text_atlas_full > 0 {
        text.push(
            Spans::default()
                .value(grouped(rc.text_atlas_full))
                .name(" atlas full"),
        );
    }
    let labels = vec![
        Spans::default()
            .value(grouped(rc.world_labels))
            .name(" drawn"),
        Spans::default()
            .value(grouped(rc.world_labels_culled))
            .name(" culled"),
        Spans::default()
            .value(grouped(rc.world_labels_dropped))
            .name(" dropped"),
    ];
    vec![
        Line::new(
            "ui",
            vec![
                Spans::default().count(rc.ui_commands, "cmd", "cmds"),
                Spans::default().count(rc.text_batches, "batch", "batches"),
            ],
        ),
        Line::new(
            "layout",
            vec![
                Spans::default()
                    .value(grouped(rc.ui_layout_us))
                    .name(" \u{b5}s"),
                Spans::default().count(rc.ui_roots_laid_out, "root", "roots"),
                Spans::default().count(rc.ui_nodes_laid_out, "node", "nodes"),
            ],
        ),
        Line::new("text", text),
        Line::new("labels", labels),
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
