//! Runtime developer HUD; toggled by `engine_toggle_hud`. From M57 a
//! movable window on the core UI (`debug_views`): the fps row, the built-in
//! rows and the game's, refreshed on the frame interval, under a graph of
//! the last 240 frame intervals redrawn every frame. From M58 the graph sits
//! in the fixed header at 240 units wide, with guide lines at 16.7 and
//! 33.3 ms and its least, mean, most and 1% low beside it, its bars and
//! times on the budget ramp; the averages start from the first interval,
//! and the values read `–` until it arrives.

use std::collections::VecDeque;

use tungsten_core::World;
use tungsten_core::camera::{CameraController, CameraMode, CameraState};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::text::{StyledText, TextAlign, TextSpan};
use tungsten_core::ui::{Sizing, UiVertex, WidgetId};
use tungsten_render::GpuFrameTimings;

use crate::debug_views::{
    BORDER, Cell, Column, FRAME_BUDGET_MS, GRAPH_SAMPLES, Header, MISSING, PANEL, Row, STRONG,
    Shape, Tone, VALUE, ViewFrame, ViewSpec, ViewWindow, Width, graph_stats, graph_vertices,
    look_of, ramp, toggle_key, write_table,
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
    built_in: Vec<HudRowProvider>,
    custom: Vec<HudRowProvider>,
}

/// The HUD's columns: labels right-aligned, then values.
const COLUMNS: &[Column] = &[
    Column::new(TextAlign::End, Width::Capped(40), Tone::Label),
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
];

impl DebugHud {
    pub fn new() -> Self {
        // HUD stays render-focused; systems/inspector own deeper views.
        let built_in: Vec<HudRowProvider> = vec![
            Box::new(gpu_provider),
            Box::new(display_provider),
            Box::new(camera_provider),
            Box::new(counts_provider),
            Box::new(render_cpu_provider),
            Box::new(ui_provider),
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
    /// colour; `–` before the first interval.
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
        let spans = StyledText {
            spans: vec![
                TextSpan::new(format!("{fps:>5.1}  ")),
                TextSpan::new(format!("{:>5.2}ms", self.total_ewma))
                    .with_weight(STRONG)
                    .with_color(ramp(self.total_ewma, FRAME_BUDGET_MS)),
            ],
        };
        vec![label, Cell::styled(spans)]
    }

    /// The HUD's frame (M57): the averages, the graph and, on its interval,
    /// the rows, from the previous frame's `FrameTimings`; while it is
    /// shown. Its window opens, closes and moves through `frame`.
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
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.time_since_refresh_ms = f32::INFINITY;
            self.rendered_height = 0.0;
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
            for provider in self.built_in.iter().chain(&self.custom) {
                for row in provider(world) {
                    // The frame's CPU times are 0 until it has run once.
                    let value = if row.label == "cpu" && !self.seeded {
                        MISSING.to_string()
                    } else {
                        row.value
                    };
                    rows.push(Row::data(vec![Cell::new(row.label), Cell::new(value)]));
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

fn camera_provider(world: &World) -> Vec<HudRow> {
    let Some(state) = world.get_resource::<CameraState>() else {
        return Vec::new();
    };
    let Some(controller) = world.get_resource::<CameraController>() else {
        return Vec::new();
    };
    let mode = match controller.mode {
        CameraMode::Free => "free",
        CameraMode::Follow(_) => "foll",
        CameraMode::Scripted => "scr",
    };
    vec![HudRow {
        label: "cam",
        value: format!(
            "({:.0},{:.0}) z{:.2} {}",
            state.position.x, state.position.y, state.zoom, mode
        ),
    }]
}

fn display_provider(world: &World) -> Vec<HudRow> {
    let Some(dt) = world.get_resource::<DisplayTelemetry>() else {
        return Vec::new();
    };
    let vsync = if dt.vsync { "on" } else { "off" };
    vec![HudRow {
        label: "view",
        value: format!(
            "{}x{} {} vs:{}",
            dt.resolution.0,
            dt.resolution.1,
            dt.display_mode.as_str(),
            vsync
        ),
    }]
}

fn gpu_provider(world: &World) -> Vec<HudRow> {
    let Some(gpu) = world.get_resource::<GpuFrameTimings>() else {
        return Vec::new();
    };
    let gpu_ms = match gpu.frame_gpu_ms {
        Some(ms) => format!("{ms:.2}ms"),
        None => "n/a".to_string(),
    };
    let backend = gpu.backend.as_deref().unwrap_or("?");
    vec![HudRow {
        label: "gpu",
        value: format!("{gpu_ms} {backend}"),
    }]
}

fn counts_provider(world: &World) -> Vec<HudRow> {
    let rc = match world.get_resource::<RenderCounts>() {
        Some(rc) => *rc,
        None => return Vec::new(),
    };
    vec![HudRow {
        label: "draw",
        value: format!("{} ents  {} spr", rc.entities, rc.sprite_instances),
    }]
}

fn render_cpu_provider(world: &World) -> Vec<HudRow> {
    let Some(ft) = world.get_resource::<FrameTimings>() else {
        return Vec::new();
    };
    vec![HudRow {
        label: "cpu",
        value: format!(
            "acq {:.2} enc {:.2} sub {:.2}ms",
            ft.render_acquire_ms, ft.render_encode_ms, ft.render_submit_present_ms
        ),
    }]
}

/// M46: the UI paint list's size, the layout stage's cost and the text
/// batch telemetry (`D-126`'s observable half, Q3); M54: the world labels
/// kept, culled and dropped, their glyphs and the cached layouts. From M58
/// three rows: `ui` (commands, layout µs, roots and nodes laid out), `text`
/// (batches, prepared, full frames, atlas full, layouts) and `labels`
/// (world labels kept, culled and dropped, glyphs).
fn ui_provider(world: &World) -> Vec<HudRow> {
    let Some(rc) = world.get_resource::<RenderCounts>() else {
        return Vec::new();
    };
    vec![
        HudRow {
            label: "ui",
            value: format!(
                "{} cmds layout {}us {}r/{}n",
                rc.ui_commands, rc.ui_layout_us, rc.ui_roots_laid_out, rc.ui_nodes_laid_out
            ),
        },
        HudRow {
            label: "text",
            value: format!(
                "{}b {}prep full {} atlas_full {} lay {}",
                rc.text_batches,
                rc.text_batches_prepared,
                rc.text_full_frames,
                rc.text_atlas_full,
                rc.text_layouts
            ),
        },
        HudRow {
            label: "labels",
            value: format!(
                "{}/{}c/{}d lg {}",
                rc.world_labels,
                rc.world_labels_culled,
                rc.world_labels_dropped,
                rc.text_label_glyphs
            ),
        },
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
