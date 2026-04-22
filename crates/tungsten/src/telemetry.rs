//! CPU frame-stage timing telemetry.
//!
//! `FrameTimings` is a World resource populated each frame by `App`.
//! It is consumed by the runtime HUD (M18) and offline tooling.
//! All timings are wall-clock milliseconds from `std::time::Instant`.

use tungsten_core::{DisplayMode, DisplayState, ScaleMode};

/// Per-stage CPU timing for a single frame, in milliseconds.
/// Populated by `App` at the end of each `RedrawRequested` pass and
/// inserted as a resource so any system or HUD can read it.
#[derive(Debug, Clone, Default)]
pub struct FrameTimings {
    /// Total wall time for all registered systems (sum of system_timings durations).
    pub update_ms: f32,
    /// Time spent in all extract closures (quads + sprites + text).
    pub extract_ms: f32,
    /// Total time spent in the render stage, including encode plus submit/present waits.
    pub render_ms: f32,
    /// CPU time spent acquiring the next surface texture.
    pub render_acquire_ms: f32,
    /// CPU time spent preparing render data, recording commands, and finishing the encoder.
    pub render_encode_ms: f32,
    /// CPU time spent submitting work, presenting, and waiting on present/readback.
    pub render_submit_present_ms: f32,
    /// Time spent draining AudioCommands and forwarding to the audio thread.
    pub audio_ms: f32,
    /// Time spent in process_hot_reload.
    pub hot_reload_ms: f32,
    /// Time spent draining and applying the `CommandBuffer` resource each frame.
    /// Includes all deferred spawn/despawn/insert/remove mutations from this frame's systems.
    pub flush_ms: f32,
    /// Total wall time for the frame (RedrawRequested entry to end of render).
    pub total_ms: f32,
    /// Per-system breakdown: (name, duration_ms) in registration order.
    /// Systems registered with `App::add_system` use auto-generated name "system_N".
    /// Systems registered with `App::add_system_named` use the provided name.
    pub system_timings: Vec<(String, f32)>,
}

impl FrameTimings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the name and duration of the slowest system this frame, or None.
    pub fn slowest_system(&self) -> Option<(&str, f32)> {
        self.system_timings
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, ms)| (name.as_str(), *ms))
    }
}

/// Runtime display/window telemetry published by the umbrella crate.
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

/// Per-frame counts of what the render path saw this frame. Populated by
/// `App` after the extract stage and read by the runtime HUD (M18).
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderCounts {
    pub entities: u32,
    pub sprite_instances: u32,
}

#[cfg(test)]
#[path = "tests/telemetry.rs"]
mod tests;
