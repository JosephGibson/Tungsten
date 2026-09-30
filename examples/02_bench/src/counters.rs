//! Benchmark telemetry on log target `bench`, the stream the engine's perf
//! lines use: `bench-config:` once (JSON) and `bench:` once per frame.
//!
//! Frame N + 1's first system logs frame N's counters, so each `bench:` line
//! follows frame N's `frame:`, `systems:`, `gpu_passes:` and `physics:` group
//! like every other companion line. The last frame has no `bench:` line.

use serde_json::Value as Json;
use tungsten::core::World;

/// One benchmark's per-frame workload counters.
pub(crate) trait FrameCounters: 'static {
    /// Append ` name=value` pairs for the frame that just ended. `world` still
    /// holds that frame's final state, so gauges such as `visible` read it here.
    fn write(&self, world: &World, line: &mut String);
    /// Clear per-frame counts before the next frame's systems run.
    fn reset(&mut self);
}

/// Resource holding the running frame's counters.
pub(crate) struct BenchCounters<T> {
    pub(crate) counts: T,
    frame_ended: bool,
}

impl<T> BenchCounters<T> {
    pub(crate) fn new(counts: T) -> Self {
        Self {
            counts,
            frame_ended: false,
        }
    }
}

pub(crate) fn log_config(config: &Json) {
    log::info!(target: "bench", "bench-config: {config}");
}

/// Register first: logs the previous frame's `bench:` line, then resets.
pub(crate) fn bench_counters_system<T: FrameCounters>(world: &mut World) {
    let Some(state) = world.get_resource::<BenchCounters<T>>() else {
        return;
    };
    if state.frame_ended && log::log_enabled!(target: "bench", log::Level::Debug) {
        let mut line = String::from("bench:");
        state.counts.write(world, &mut line);
        log::debug!(target: "bench", "{line}");
    }
    if let Some(state) = world.get_resource_mut::<BenchCounters<T>>() {
        state.counts.reset();
        state.frame_ended = true;
    }
}
