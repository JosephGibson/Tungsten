//! Runtime developer HUD (M18).
//!
//! `DebugHud` is a world resource that aggregates telemetry rows and owns an
//! ordered list of built-in plus user-registered row providers.
//! `engine_toggle_hud` (default `F4`) toggles visibility through
//! [`hud_toggle_system`], registered by `App::new` as the first system each
//! frame. Default state is off; examples opt in by mutating the resource
//! during setup.
//!
//! Rows are rendered through the existing `glyphon` text pipeline as a single
//! [`TextSection`], composed every frame by [`compose_hud_text_sections`].
//! When disabled the compose helper returns an empty `Vec` without running any
//! provider.

use tungsten_core::camera::{CameraController, CameraMode, CameraState};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::World;
use tungsten_render::{GpuFrameTimings, TextSection};

use crate::telemetry::{DisplayTelemetry, FrameTimings, RenderCounts};

/// Fraction of `font_size` used as the glyph advance width when estimating
/// block width without running `glyphon` layout. JetBrains Mono (our
/// default "mono") measures ~0.60 em; 0.65 adds margin for the small
/// glyphon side-bearing padding so right-anchored blocks never spill past
/// the viewport edge.
pub(crate) const MONO_ADVANCE_RATIO: f32 = 0.65;

/// Screen-anchor corner for the HUD block. Right-side corners use a
/// monospace-width heuristic to estimate pixel width without running
/// `glyphon` layout; they are approximate by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A single diagnostic row: `"<label>  <value>"`. Providers return a `Vec`
/// because some built-ins emit multiple rows (e.g. top-N slowest systems).
#[derive(Debug, Clone)]
pub struct HudRow {
    pub label: &'static str,
    pub value: String,
}

/// Closure contract for a row provider. Providers read from `World` and
/// return zero or more [`HudRow`]s. An empty `Vec` means "skip"; providers
/// must never panic when a backing resource or entity is absent.
pub type HudRowProvider = Box<dyn Fn(&World) -> Vec<HudRow> + 'static>;

/// Optional hint populated by the M20 scene/state stack. Held separately
/// from the HUD itself so user code — a custom row provider, an external
/// diagnostic panel — can surface the active state id without pulling on
/// `StateStack` directly. The HUD's built-in row set is render-focused and
/// does not consume this resource.
#[derive(Debug, Clone, Default)]
pub struct HudActiveState(pub String);

/// World resource: runtime developer HUD model.
pub struct DebugHud {
    pub enabled: bool,
    pub corner: HudCorner,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    /// Outline color drawn behind the main text. Four cardinal offset copies
    /// are emitted at `+/- outline_px` to fake a stroke; `glyphon` has no
    /// native stroke API so this is the minimal path.
    pub outline_color: [u8; 4],
    /// Outline offset in pixels. Set to `0.0` to disable the outline and skip
    /// the extra text sections.
    pub outline_px: f32,
    pub padding_px: f32,
    /// Minimum wall-clock interval between text refreshes, in milliseconds.
    /// EWMA keeps updating every frame; only the displayed snapshot is
    /// throttled so fast-changing values stay readable. Defaults to 500 ms
    /// (2 Hz) — values dwell long enough for the human eye to actually
    /// read them. Set to `0.0` to refresh every frame.
    pub refresh_interval_ms: f32,
    fps_ewma: f32,
    frame_ms_ewma: f32,
    ewma_alpha: f32,
    time_since_refresh_ms: f32,
    cached_sections: Vec<TextSection>,
    built_in: Vec<HudRowProvider>,
    custom: Vec<HudRowProvider>,
}

impl DebugHud {
    pub fn new() -> Self {
        // Render-focused: the HUD is the at-a-glance render dashboard. Per-
        // system timings belong in the systems overlay (`F2`), per-entity
        // state belongs in the inspector (`F3`).
        let built_in: Vec<HudRowProvider> = vec![
            Box::new(gpu_provider),
            Box::new(display_provider),
            Box::new(camera_provider),
            Box::new(counts_provider),
            Box::new(render_cpu_provider),
        ];
        Self {
            enabled: false,
            corner: HudCorner::TopRight,
            font_id: "mono".to_string(),
            font_size: 26.0,
            line_height: 30.0,
            color: [240, 240, 240, 240],
            outline_color: [0, 0, 0, 220],
            outline_px: 1.0,
            padding_px: 10.0,
            refresh_interval_ms: 500.0,
            fps_ewma: 0.0,
            frame_ms_ewma: 0.0,
            ewma_alpha: 0.1,
            time_since_refresh_ms: f32::INFINITY,
            cached_sections: Vec::new(),
            built_in,
            custom: Vec::new(),
        }
    }

    /// Flip `enabled`.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Vertical pixel extent of the HUD as composed on the last frame,
    /// including top padding. Returns `0.0` when disabled or before the
    /// first compose call. Used by the systems-timing overlay to stack
    /// itself directly below the HUD without guessing dimensions.
    pub fn rendered_height_px(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        let Some(main) = self.cached_sections.last() else {
            return 0.0;
        };
        let lines = main.content.lines().count().max(1) as f32;
        self.padding_px + lines * self.line_height
    }

    /// Register an additional row provider. Custom rows render after all
    /// built-ins, in registration order.
    pub fn add_row<F>(&mut self, provider: F)
    where
        F: Fn(&World) -> Vec<HudRow> + 'static,
    {
        self.custom.push(Box::new(provider));
    }

    /// Compute the FPS / frame-ms row from the smoothed state. Kept as a
    /// method rather than a free-function provider so the smoothed values
    /// can be read without stashing them in a shadow resource.
    fn fps_row(&self) -> Vec<HudRow> {
        vec![HudRow {
            label: "fps",
            value: format!("{:>5.1}  {:>5.2}ms", self.fps_ewma, self.frame_ms_ewma),
        }]
    }
}

impl Default for DebugHud {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in providers
// ---------------------------------------------------------------------------

fn camera_provider(world: &World) -> Vec<HudRow> {
    let state = match world.get_resource::<CameraState>() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let controller = match world.get_resource::<CameraController>() {
        Some(c) => c,
        None => return Vec::new(),
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
    let dt = match world.get_resource::<DisplayTelemetry>() {
        Some(d) => d,
        None => return Vec::new(),
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
    let gpu = match world.get_resource::<GpuFrameTimings>() {
        Some(g) => g,
        None => return Vec::new(),
    };
    // Two compact fields: gpu timing + backend name. Present mode is
    // already on the `view` row, so don't duplicate it here.
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
    let ft = match world.get_resource::<FrameTimings>() {
        Some(ft) => ft,
        None => return Vec::new(),
    };
    vec![HudRow {
        label: "cpu",
        value: format!(
            "acq {:.2} enc {:.2} sub {:.2}ms",
            ft.render_acquire_ms, ft.render_encode_ms, ft.render_submit_present_ms
        ),
    }]
}

// ---------------------------------------------------------------------------
// Compose / toggle
// ---------------------------------------------------------------------------

/// Called by `App` each frame after the extract stage. Updates the HUD's
/// EWMA smoothing and produces a single `TextSection` holding all active
/// rows. Returns an empty `Vec` when the HUD is disabled, without running
/// any provider.
///
/// The canonical call pattern in `App` is: `remove_resource::<DebugHud>()`,
/// call this helper with `&mut hud` and `&world`, then `insert_resource(hud)`
/// afterwards. That dance is needed because the providers take `&World` and
/// the HUD itself lives as a `World` resource.
pub(crate) fn compose_hud_text_sections(
    hud: &mut DebugHud,
    world: &World,
    viewport: (u32, u32),
    frame_ms: f32,
) -> Vec<TextSection> {
    if !hud.enabled {
        hud.cached_sections.clear();
        hud.time_since_refresh_ms = f32::INFINITY;
        return Vec::new();
    }

    // EWMA smoothing of frame time / fps. Runs every frame so the snapshot
    // captured at the next refresh tick reflects all frames in between.
    hud.frame_ms_ewma = (1.0 - hud.ewma_alpha) * hud.frame_ms_ewma + hud.ewma_alpha * frame_ms;
    hud.fps_ewma = if hud.frame_ms_ewma > 0.0 {
        1000.0 / hud.frame_ms_ewma
    } else {
        0.0
    };

    // Throttle refresh so fast-changing values stay readable. The cached
    // sections are re-emitted between ticks; `time_since_refresh_ms` is
    // seeded to +infinity so the very first call after enable always
    // rebuilds.
    hud.time_since_refresh_ms += frame_ms;
    if hud.time_since_refresh_ms < hud.refresh_interval_ms && !hud.cached_sections.is_empty() {
        return hud.cached_sections.clone();
    }
    hud.time_since_refresh_ms = 0.0;

    let mut rows = hud.fps_row();
    for provider in &hud.built_in {
        rows.extend(provider(world));
    }
    for provider in &hud.custom {
        rows.extend(provider(world));
    }

    if rows.is_empty() {
        hud.cached_sections.clear();
        return Vec::new();
    }

    // Right-align the label column and separate from the value with a
    // single space. Values are already whitespace-compact so double-
    // spacing here would just pad the row for no signal.
    let label_w = rows
        .iter()
        .map(|r| r.label.chars().count())
        .max()
        .unwrap_or(0)
        .max(3);
    let rendered: Vec<String> = rows
        .iter()
        .map(|r| format!("{:>w$} {}", r.label, r.value, w = label_w))
        .collect();
    let content = rendered.join("\n");

    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    let line_count = rendered.len() as f32;
    let pad = hud.padding_px;

    let (x, y) = match hud.corner {
        HudCorner::TopLeft => (pad, pad),
        HudCorner::BottomLeft => (pad, (vh - line_count * hud.line_height - pad).max(0.0)),
        HudCorner::TopRight => {
            let char_w = hud.font_size * MONO_ADVANCE_RATIO;
            let max_chars = rendered
                .iter()
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0) as f32;
            let text_w = char_w * max_chars;
            ((vw - text_w - pad).max(0.0), pad)
        }
        HudCorner::BottomRight => {
            let char_w = hud.font_size * MONO_ADVANCE_RATIO;
            let max_chars = rendered
                .iter()
                .map(|s| s.chars().count())
                .max()
                .unwrap_or(0) as f32;
            let text_w = char_w * max_chars;
            (
                (vw - text_w - pad).max(0.0),
                (vh - line_count * hud.line_height - pad).max(0.0),
            )
        }
    };

    let main = TextSection {
        content,
        font_id: hud.font_id.clone(),
        font_size: hud.font_size,
        line_height: hud.line_height,
        color: hud.color,
        position: [x, y],
        bounds: None,
    };

    let sections = if hud.outline_px > 0.0 {
        let ox = hud.outline_px;
        let mut out = Vec::with_capacity(5);
        for (dx, dy) in [(-ox, 0.0), (ox, 0.0), (0.0, -ox), (0.0, ox)] {
            out.push(TextSection {
                content: main.content.clone(),
                font_id: main.font_id.clone(),
                font_size: main.font_size,
                line_height: main.line_height,
                color: hud.outline_color,
                position: [x + dx, y + dy],
                bounds: None,
            });
        }
        out.push(main);
        out
    } else {
        vec![main]
    };
    hud.cached_sections = sections.clone();
    sections
}

/// Compute the top-left origin for a multi-line text block anchored to one of
/// the four screen corners. Shared by the systems-timing overlay and the
/// entity inspector so their layout math stays in one place. Width is
/// estimated with the same `font_size * MONO_ADVANCE_RATIO` monospace
/// heuristic used by HUD's right-side corners.
pub(crate) fn anchor_text_block(
    corner: HudCorner,
    lines: &[String],
    font_size: f32,
    line_height: f32,
    padding: f32,
    viewport: (u32, u32),
) -> (f32, f32) {
    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    let line_count = lines.len().max(1) as f32;
    let max_chars = lines.iter().map(|s| s.chars().count()).max().unwrap_or(0) as f32;
    let text_w = font_size * MONO_ADVANCE_RATIO * max_chars;
    let text_h = line_count * line_height;
    match corner {
        HudCorner::TopLeft => (padding, padding),
        HudCorner::TopRight => ((vw - text_w - padding).max(0.0), padding),
        HudCorner::BottomLeft => (padding, (vh - text_h - padding).max(0.0)),
        HudCorner::BottomRight => (
            (vw - text_w - padding).max(0.0),
            (vh - text_h - padding).max(0.0),
        ),
    }
}

/// Engine-registered system: toggles `DebugHud.enabled` on the
/// `engine_toggle_hud` action edge. Runs as the first system each frame so
/// input is observed before any user system consumes `just_pressed`.
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
    if pressed {
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.toggle();
        }
    }
}

#[cfg(test)]
#[path = "tests/debug_hud.rs"]
mod tests;
