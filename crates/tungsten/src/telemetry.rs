//! CPU frame-stage timing telemetry.
//!
//! `FrameTimings` is a World resource populated each frame by `App`.
//! It is consumed by the runtime HUD (M18) and offline tooling.
//! All timings are wall-clock milliseconds from `std::time::Instant`.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let ft = FrameTimings::new();
        assert_eq!(ft.update_ms, 0.0);
        assert_eq!(ft.render_ms, 0.0);
        assert_eq!(ft.render_acquire_ms, 0.0);
        assert_eq!(ft.render_encode_ms, 0.0);
        assert_eq!(ft.render_submit_present_ms, 0.0);
        assert_eq!(ft.flush_ms, 0.0);
        assert!(ft.system_timings.is_empty());
    }

    #[test]
    fn slowest_system_empty() {
        assert!(FrameTimings::new().slowest_system().is_none());
    }

    #[test]
    fn slowest_system_finds_max() {
        let mut ft = FrameTimings::new();
        ft.system_timings = vec![
            ("a".to_string(), 1.0),
            ("b".to_string(), 5.0),
            ("c".to_string(), 2.0),
        ];
        let (name, ms) = ft.slowest_system().unwrap();
        assert_eq!(name, "b");
        assert!((ms - 5.0).abs() < f32::EPSILON);
    }
}
