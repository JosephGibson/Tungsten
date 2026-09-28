//! Per-frame CPU and GPU timing snapshots.

use tungsten_core::post::{PostPass, PostStack};

/// GPU frame timing/adapter metadata.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameTimings {
    /// Scene-pass GPU duration (the historical `gpu=` metric), not the whole frame.
    /// `None` when timing is disabled, unsupported, skipped or readback fails.
    pub frame_gpu_ms: Option<f32>,
    /// First scene timestamp through the end of the present blit, including gaps.
    /// Excludes queued uploads, query resolve/readback and presentation waits.
    pub render_gpu_ms: Option<f32>,
    /// Render passes in execution order; post slot and bloom mip labels are unique.
    pub pass_gpu_ms: Vec<(String, f32)>,
    /// Adapter backend name.
    pub backend: Option<String>,
    /// Adapter name.
    pub adapter_name: Option<String>,
    /// Actual present mode.
    pub present_mode: Option<String>,
    /// Surface frame-latency hint.
    pub max_frame_latency: Option<u32>,
}

impl GpuFrameTimings {
    pub(crate) fn clear_durations(&mut self) {
        self.frame_gpu_ms = None;
        self.render_gpu_ms = None;
        self.pass_gpu_ms.clear();
    }

    fn decode(&mut self, labels: &[String], stamps: &[u64], period_ns: f32) {
        self.clear_durations();
        if labels.is_empty() || stamps.len() != labels.len() * 2 {
            return;
        }
        let ms =
            |a: u64, b: u64| (b.wrapping_sub(a) as f64 * f64::from(period_ns) / 1_000_000.0) as f32;
        self.pass_gpu_ms.extend(
            labels
                .iter()
                .zip(stamps.as_chunks::<2>().0.iter())
                .map(|(label, pair)| (label.clone(), ms(pair[0], pair[1]))),
        );
        self.frame_gpu_ms = self
            .pass_gpu_ms
            .iter()
            .find(|(name, _)| name == "scene")
            .map(|(_, ms)| *ms);
        self.render_gpu_ms = Some(ms(stamps[0], stamps[stamps.len() - 1]));
    }
}

/// Two queries per real render pass, including every bloom stage.
fn query_count(stack: &PostStack, bloom_mips: u32, smaa: bool) -> Option<u32> {
    let mut passes: u32 = if smaa { 6 } else { 3 }; // scene, text, present, SMAA tail
    for pass in &stack.0 {
        passes = passes.checked_add(if matches!(pass, PostPass::Bloom(_)) {
            bloom_mips.checked_mul(2)?
        } else {
            1
        })?;
    }
    let count = passes.checked_mul(2)?;
    (count <= wgpu::QUERY_SET_MAX_QUERIES).then_some(count)
}

pub(crate) struct TimingResources {
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
    labels: Vec<String>,
    capacity: u32,
}

impl TimingResources {
    pub(crate) fn new(
        device: &wgpu::Device,
        stack: &PostStack,
        bloom_mips: u32,
        smaa: bool,
    ) -> Option<Self> {
        let Some(count) = query_count(stack, bloom_mips, smaa) else {
            log::warn!(
                "GPU pass timing exceeds the timestamp query limit; rendering without timings"
            );
            return None;
        };
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("frame_ts_qs"),
            count,
            ty: wgpu::QueryType::Timestamp,
        });
        let size = u64::from(count) * 8;
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ts_resolve"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ts_readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            query_set,
            resolve_buf,
            readback_buf,
            labels: Vec::with_capacity(count as usize / 2),
            capacity: count,
        })
    }

    pub(crate) fn next(&mut self, label: String) -> wgpu::RenderPassTimestampWrites<'_> {
        let begin = self.labels.len() as u32 * 2;
        assert!(
            begin + 1 < self.capacity,
            "timestamp pass count must match the recording plan"
        );
        self.labels.push(label);
        wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(begin + 1),
        }
    }

    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let count = self.labels.len() as u32 * 2;
        encoder.resolve_query_set(&self.query_set, 0..count, &self.resolve_buf, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buf,
            0,
            &self.readback_buf,
            0,
            u64::from(count) * 8,
        );
    }

    pub(crate) fn read(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        out: &mut GpuFrameTimings,
    ) {
        let slice = self.readback_buf.slice(..self.labels.len() as u64 * 16);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        if receiver.recv().ok().and_then(Result::ok).is_some() {
            let stamps = slice.get_mapped_range().map(|data| {
                data.as_chunks::<8>()
                    .0
                    .iter()
                    .map(|bytes| u64::from_le_bytes(*bytes))
                    .collect::<Vec<_>>()
            });
            self.readback_buf.unmap();
            match stamps {
                Ok(stamps) => out.decode(&self.labels, &stamps, queue.get_timestamp_period()),
                Err(e) => log::warn!("GPU timing readback failed: {e:?}"),
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten_core::post::{BloomParams, VignetteParams};

    #[test]
    fn counts_real_passes_and_rejects_query_overflow() {
        assert_eq!(query_count(&PostStack::default(), 6, false), Some(6));
        let stack = PostStack(vec![
            PostPass::Bloom(BloomParams::default()),
            PostPass::Vignette(VignetteParams::default()),
            PostPass::Bloom(BloomParams::default()),
        ]);
        assert_eq!(query_count(&stack, 6, true), Some(62));
        assert_eq!(query_count(&stack, 1, false), Some(16));
        assert_eq!(query_count(&stack, 0, false), Some(8));
        assert_eq!(query_count(&stack, u32::MAX, false), None);
        let huge = PostStack(vec![PostPass::Vignette(VignetteParams::default()); 2046]);
        assert_eq!(query_count(&huge, 6, false), None);
    }

    #[test]
    fn scene_compatibility_span_gaps_and_stale_data() {
        let labels = ["scene", "post0_bloom_threshold", "present"].map(str::to_string);
        let mut out = GpuFrameTimings::default();
        out.decode(&labels, &[10, 30, 40, 50, 80, 100], 1000.0);
        assert_eq!(out.frame_gpu_ms, Some(0.02));
        assert_eq!(out.render_gpu_ms, Some(0.09));
        assert_eq!(out.pass_gpu_ms.len(), 3);
        out.decode(&labels, &[1, 2], 1.0);
        assert!(
            out.frame_gpu_ms.is_none() && out.render_gpu_ms.is_none() && out.pass_gpu_ms.is_empty()
        );
        out.decode(&["scene".into()], &[u64::MAX - 9, 10], 1000.0);
        assert_eq!(out.frame_gpu_ms, Some(0.02));
        out.clear_durations();
        assert!(out.pass_gpu_ms.is_empty() && out.frame_gpu_ms.is_none());
    }
}
