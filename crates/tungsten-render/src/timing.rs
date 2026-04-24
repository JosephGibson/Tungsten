//! Per-frame CPU and GPU timing snapshots.

/// GPU frame timing/adapter metadata.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameTimings {
    /// Render-pass GPU duration; `None` without timestamp queries.
    pub frame_gpu_ms: Option<f32>,
    /// Adapter backend name.
    pub backend: Option<String>,
    /// Adapter name.
    pub adapter_name: Option<String>,
    /// Actual present mode.
    pub present_mode: Option<String>,
    /// Surface frame-latency hint.
    pub max_frame_latency: Option<u32>,
}

/// CPU render-frame timing.
#[derive(Debug, Clone, Default)]
pub struct CpuFrameTimings {
    /// Surface acquire time.
    pub acquire_ms: f32,
    /// Encode/command-recording time.
    pub encode_ms: f32,
    /// Submit/present/readback wait time.
    pub submit_present_ms: f32,
}
