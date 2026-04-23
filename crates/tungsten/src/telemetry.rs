//! CPU frame-stage telemetry; timings are wall-clock milliseconds.

use tungsten_core::{DisplayMode, DisplayState, ScaleMode};

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
    /// Encode/command recording time.
    pub render_encode_ms: f32,
    /// Submit/present/readback wait time.
    pub render_submit_present_ms: f32,
    /// Audio command forwarding time.
    pub audio_ms: f32,
    /// Hot-reload processing time.
    pub hot_reload_ms: f32,
    /// `CommandBuffer` flush time.
    pub flush_ms: f32,
    /// Total frame wall time.
    pub total_ms: f32,
    /// Per-system `(name, duration_ms)` in registration order.
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
    pub scale_mode: ScaleMode,
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
            scale_mode: state.scale_mode,
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

/// Per-frame render counts from extract.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderCounts {
    pub entities: u32,
    pub sprite_instances: u32,
}

#[cfg(test)]
#[path = "tests/telemetry.rs"]
mod tests;
