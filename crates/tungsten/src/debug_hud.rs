//! Runtime developer HUD; toggled by `engine_toggle_hud`. From M57 a
//! movable window on the core UI (`debug_views`): the fps row, the built-in
//! rows and the game's, refreshed on the frame interval, over a graph of
//! the last 240 frame intervals redrawn every frame.

use std::collections::VecDeque;

use tungsten_core::World;
use tungsten_core::camera::{CameraController, CameraMode, CameraState};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::text::TextAlign;
use tungsten_core::ui::{UiVertex, WidgetId};
use tungsten_render::GpuFrameTimings;

use crate::debug_views::{
    BACKGROUND, GRAPH_SAMPLES, Look, Shape, ViewFrame, ViewSpec, ViewWindow, graph_vertices,
    write_rows,
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
    /// The corner the window opens at when no place is saved for it.
    pub corner: HudCorner,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default (M57).
    pub font_id: String,
    /// Text size in UI units.
    pub font_size: f32,
    /// Line height in UI units.
    pub line_height: f32,
    pub color: [u8; 4],
    /// The window's fill (M57).
    pub background: [u8; 4],
    /// The window's border colour; the text outline's before M57.
    pub outline_color: [u8; 4],
    /// The window's border width in UI units; `0.0` draws none.
    pub outline_px: f32,
    /// Space between the window and its corner, and below it for the next
    /// window at that corner, in UI units.
    pub padding_px: f32,
    /// Row refresh interval on the frame interval; the averages and the
    /// graph still update every frame.
    pub refresh_interval_ms: f32,
    interval_ewma: f32,
    total_ewma: f32,
    ewma_alpha: f32,
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
const COLUMNS: &[TextAlign] = &[TextAlign::End, TextAlign::Start];

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
            corner: HudCorner::TopRight,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 26.0,
            line_height: 30.0,
            color: [240, 240, 240, 240],
            background: BACKGROUND,
            outline_color: [0, 0, 0, 220],
            outline_px: 1.0,
            padding_px: 10.0,
            refresh_interval_ms: 500.0,
            interval_ewma: 0.0,
            total_ewma: 0.0,
            ewma_alpha: 0.1,
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
    /// physical pixels: where a block below it at its corner starts. 0
    /// while hidden or before the window's first layout.
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

    fn fps_row(&self) -> HudRow {
        let fps = if self.interval_ewma > 0.0 {
            1000.0 / self.interval_ewma
        } else {
            0.0
        };
        HudRow {
            label: "fps",
            value: format!("{fps:>5.1}  {:>5.2}ms", self.total_ewma),
        }
    }

    /// The HUD's frame (M57): the averages, the graph and, on its interval,
    /// the rows, from the previous frame's `FrameTimings`; while it is
    /// shown. Its window opens, closes and moves through `frame`.
    pub(crate) fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        let spec = ViewSpec {
            name: "hud",
            title: "HUD",
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            look: Look {
                family: self.font_id.clone(),
                size: self.font_size,
                line_height: self.line_height,
                color: self.color,
                background: self.background,
                border: self.outline_color,
                border_width: self.outline_px,
            },
            shape: Shape {
                columns: COLUMNS,
                header: false,
                graph: true,
                scroll: false,
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
        if let Some(total) = total.filter(|ms| ms.is_finite()) {
            self.total_ewma = (1.0 - self.ewma_alpha) * self.total_ewma + self.ewma_alpha * total;
        }
        let interval = interval.filter(|ms| ms.is_finite() && *ms >= 0.0);
        if let Some(interval) = interval {
            self.interval_ewma =
                (1.0 - self.ewma_alpha) * self.interval_ewma + self.ewma_alpha * interval;
            if self.samples.len() == GRAPH_SAMPLES {
                self.samples.pop_front();
            }
            self.samples.push_back(interval);
        }
        if let Some(graph) = window.graph {
            self.graph_samples.clear();
            self.graph_samples.extend(self.samples.iter().copied());
            graph_vertices(&self.graph_samples, self.color, &mut self.vertices);
            frame.tree.set_mesh(graph, &self.vertices);
        }

        self.time_since_refresh_ms += interval.unwrap_or(0.0);
        if self.time_since_refresh_ms >= self.refresh_interval_ms {
            self.time_since_refresh_ms = 0.0;
            let mut rows = vec![self.fps_row()];
            for provider in self.built_in.iter().chain(&self.custom) {
                rows.extend(provider(world));
            }
            let cells: Vec<Vec<String>> = rows
                .into_iter()
                .map(|row| vec![row.label.to_string(), row.value])
                .collect();
            write_rows(frame.tree, window, &cells);
        }
        self.rendered_height = frame
            .tree
            .rect(window.root)
            .map_or(0.0, |rect| (rect.height() + self.padding_px) * frame.scale);
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
/// kept, culled and dropped, their glyphs and the cached layouts.
fn ui_provider(world: &World) -> Vec<HudRow> {
    let Some(rc) = world.get_resource::<RenderCounts>() else {
        return Vec::new();
    };
    vec![HudRow {
        label: "ui",
        value: format!(
            "{} cmds layout {}us {}r/{}n text {}b {}prep full {} atlas_full {} labels {}/{}c/{}d lg {} lay {}",
            rc.ui_commands,
            rc.ui_layout_us,
            rc.ui_roots_laid_out,
            rc.ui_nodes_laid_out,
            rc.text_batches,
            rc.text_batches_prepared,
            rc.text_full_frames,
            rc.text_atlas_full,
            rc.world_labels,
            rc.world_labels_culled,
            rc.world_labels_dropped,
            rc.text_label_glyphs,
            rc.text_layouts
        ),
    }]
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
