//! Telemetry + shared helpers used by both the baseline and ECS-high-load
//! scenes. Kept out of each scene module so neither can drift away from the
//! common stdout format the perf harness consumes.

use tungsten::core::World;
use tungsten::render::GpuFrameTimings;
use tungsten::FrameTimings;

pub(crate) const LOG_INTERVAL: u32 = 60;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct TelemetryState {
    pub(crate) frame_count: u32,
    pub(crate) metadata_logged: bool,
}

pub(crate) fn log_telemetry(world: &mut World) {
    let (frame_count, metadata_logged) = match world.get_resource::<TelemetryState>() {
        Some(state) => (state.frame_count, state.metadata_logged),
        None => return,
    };
    let sample_frame = frame_count.saturating_sub(1);

    if !metadata_logged {
        let metadata_line = world.get_resource::<GpuFrameTimings>().map(|gpu| {
            format!(
                "[gpu] backend={} adapter={} present_mode={} max_frame_latency={}",
                gpu.backend.as_deref().unwrap_or("unknown"),
                gpu.adapter_name.as_deref().unwrap_or("unknown"),
                gpu.present_mode.as_deref().unwrap_or("unknown"),
                gpu.max_frame_latency.unwrap_or(0)
            )
        });
        if let Some(line) = metadata_line {
            println!("{line}");
            if let Some(state) = world.get_resource_mut::<TelemetryState>() {
                state.metadata_logged = true;
            }
        }
    }

    if sample_frame > 0 && sample_frame % LOG_INTERVAL == 0 {
        if let Some(ft) = world.get_resource::<FrameTimings>() {
            let gpu_ms = world
                .get_resource::<GpuFrameTimings>()
                .and_then(|gpu| gpu.frame_gpu_ms)
                .map(|ms| format!("{ms:.2}ms"))
                .unwrap_or_else(|| "n/a".to_string());
            println!(
                "[frame {:>5}] total={:.2}ms update={:.2}ms extract={:.2}ms render={:.2}ms acquire={:.2}ms encode={:.2}ms submit={:.2}ms gpu={}",
                sample_frame,
                ft.total_ms,
                ft.update_ms,
                ft.extract_ms,
                ft.render_ms,
                ft.render_acquire_ms,
                ft.render_encode_ms,
                ft.render_submit_present_ms,
                gpu_ms
            );
        }
    }
}

pub(crate) fn telemetry_frame(world: &World) -> u32 {
    world
        .get_resource::<TelemetryState>()
        .map(|state| state.frame_count)
        .unwrap_or(0)
}

pub(crate) fn rgb_wheel_color(time: f32, phase: f32) -> [u8; 4] {
    let r = ((time * 0.9 + phase).sin() * 0.5 + 0.5) * 255.0;
    let g = ((time * 1.1 + phase + 2.1).sin() * 0.5 + 0.5) * 255.0;
    let b = ((time * 1.3 + phase + 4.2).sin() * 0.5 + 0.5) * 255.0;
    [r as u8, g as u8, b as u8, 255]
}
