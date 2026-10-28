//! CPU frame-stage telemetry; timings are wall-clock milliseconds.

use tungsten_core::{DisplayMode, DisplayState};

/// Per-frame stage timings.
#[derive(Debug, Clone, Default)]
pub struct FrameTimings {
    /// Total registered-system time.
    pub update_ms: f32,
    /// Extract closure time.
    pub extract_ms: f32,
    /// Render stage time.
    pub render_ms: f32,
    /// Surface acquire time.
    pub render_acquire_ms: f32,
    /// Encode/command recording time: the whole frame's, before and after
    /// the surface acquire.
    pub render_encode_ms: f32,
    /// The part of `render_encode_ms` recorded before the surface acquire
    /// (`D-145`).
    pub render_encode_pre_acquire_ms: f32,
    /// Submit/present/readback wait time, the submit before the acquire
    /// included.
    pub render_submit_present_ms: f32,
    /// Audio command forwarding time.
    pub audio_ms: f32,
    /// Hot-reload processing time.
    pub hot_reload_ms: f32,
    /// `CommandBuffer` flush time.
    pub flush_ms: f32,
    /// Total frame wall time.
    pub total_ms: f32,
    /// Time from the previous frame's start to this frame's start: that
    /// frame's total plus the wait before this one. `None` on the first frame.
    pub interval_ms: Option<f32>,
    /// How many fixed steps the frame ran: zero or more, at most
    /// `Time::max_steps_per_frame()`; 1 in a world without `Time`.
    pub fixed_steps: u32,
    /// Per-system `(name, duration_ms)` in stage order, each name once: a
    /// `fixed_update` system's runs summed over the frame's steps, 0 ms in a
    /// frame with no step.
    pub system_timings: Vec<(String, f32)>,
}

impl FrameTimings {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Slowest system this frame.
    #[must_use]
    pub fn slowest_system(&self) -> Option<(&str, f32)> {
        self.system_timings
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, ms)| (name.as_str(), *ms))
    }
}

/// Runtime display/window telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayTelemetry {
    pub resolution: (u32, u32),
    pub display_mode: DisplayMode,
    pub vsync: bool,
    pub actual_present_mode: Option<String>,
    pub max_frame_latency: Option<u32>,
    pub frame_rate_cap: Option<u32>,
}

impl DisplayTelemetry {
    #[must_use]
    pub fn from_state(state: &DisplayState, actual_present_mode: Option<String>) -> Self {
        Self {
            resolution: (state.resolution.width, state.resolution.height),
            display_mode: state.display_mode,
            vsync: state.vsync,
            actual_present_mode,
            max_frame_latency: state.max_frame_latency,
            frame_rate_cap: state.frame_rate_cap,
        }
    }

    pub fn apply_state(&mut self, state: &DisplayState, actual_present_mode: Option<String>) {
        *self = Self::from_state(state, actual_present_mode);
    }
}

impl Default for DisplayTelemetry {
    fn default() -> Self {
        Self::from_state(&DisplayState::default(), None)
    }
}

/// Per-frame render counts from extract, and the UI's counts (M46): the
/// paint list's, the layout stage's and the world labels' from this frame's
/// extract, the text batch counts from the previous frame's render.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderCounts {
    pub entities: u32,
    pub sprite_instances: u32,
    /// Commands in the UI paint list.
    pub ui_commands: u32,
    /// Microseconds the UI layout stage took.
    pub ui_layout_us: u32,
    /// Roots the UI layout stage laid out.
    pub ui_roots_laid_out: u32,
    /// Nodes the UI layout stage laid out.
    pub ui_nodes_laid_out: u32,
    /// UI text batches in the last rendered frame.
    pub text_batches: u32,
    /// Text batches prepared in the last rendered frame.
    pub text_batches_prepared: u32,
    /// Full text frames (atlas trimmed, everything prepared) so far.
    pub text_full_frames: u64,
    /// `AtlasFull` results so far.
    pub text_atlas_full: u64,
    /// World labels this frame's extract kept for drawing (M54).
    pub world_labels: u32,
    /// World labels this frame's extract culled outside the view.
    pub world_labels_culled: u32,
    /// World labels this frame's extract dropped past the cap.
    pub world_labels_dropped: u32,
    /// Glyphs the world labels' prepare took in the last rendered frame; 0
    /// when they did not prepare.
    pub text_label_glyphs: u32,
    /// Layouts in the text cache after the last rendered frame.
    pub text_layouts: u32,
}

#[cfg(test)]
#[path = "tests/telemetry.rs"]
mod tests;
