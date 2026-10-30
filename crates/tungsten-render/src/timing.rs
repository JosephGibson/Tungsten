//! Per-frame CPU and GPU timing snapshots, and the ring of timestamp
//! readbacks that reports a frame's GPU passes a few frames later instead of
//! waiting for them (redo 7.3).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use tungsten_core::post::{PostPass, PostStack};

/// Slots in the timestamp ring. A frame's timings are read at the poll of
/// the fourth frame after it when the CPU runs three frames ahead of the GPU,
/// as it does in immediate mode on the reference adapter, so three slots
/// left every fourth frame untimed; a frame with every slot in flight runs
/// untimed.
pub(crate) const RING_DEPTH: usize = 4;

/// One timed frame's GPU passes, read back after the frame that measured
/// them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuFrameSample {
    /// The number the app gave the frame it measured
    /// ([`Renderer::set_frame_number`](crate::Renderer::set_frame_number)).
    pub frame: u64,
    /// The scene pass's duration.
    pub scene_ms: Option<f32>,
    /// First scene timestamp through the end of the frame's last pass.
    pub render_ms: f32,
    /// Render passes in execution order; post slot and bloom mip labels are unique.
    pub passes: Vec<(String, f32)>,
}

/// GPU frame timing/adapter metadata.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameTimings {
    /// The newest read-back scene pass's duration (the historical `gpu=`
    /// metric), not the whole frame. `None` when timing is disabled,
    /// unsupported or nothing has been read back yet.
    pub frame_gpu_ms: Option<f32>,
    /// The newest read-back frame's first scene timestamp through the end of
    /// its last pass, including gaps: the text overlay, or the present blit
    /// on a capture frame (`D-087`). Excludes queued uploads, query
    /// resolve/readback and presentation waits.
    pub render_gpu_ms: Option<f32>,
    /// The newest read-back frame's render passes in execution order.
    pub pass_gpu_ms: Vec<(String, f32)>,
    /// The timed frames read back during this frame, oldest first; each
    /// names the frame it measured, up to four frames earlier.
    pub samples: Vec<GpuFrameSample>,
    /// Timed frames that ran untimed because every ring slot was in flight.
    pub skipped_frames: u64,
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
        self.samples.clear();
    }

    /// Decodes frame `frame`'s timestamps into a sample and the newest
    /// fields; mismatched stamps clear the newest fields and add no sample.
    fn decode(&mut self, frame: u64, labels: &[String], stamps: &[u64], period_ns: f32) {
        self.frame_gpu_ms = None;
        self.render_gpu_ms = None;
        self.pass_gpu_ms.clear();
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
        let render_ms = ms(stamps[0], stamps[stamps.len() - 1]);
        self.render_gpu_ms = Some(render_ms);
        self.samples.push(GpuFrameSample {
            frame,
            scene_ms: self.frame_gpu_ms,
            render_ms,
            passes: self.pass_gpu_ms.clone(),
        });
    }
}

/// Two queries per real render pass, including every bloom stage. `blit` adds
/// the present blit, which only a capture frame records (`D-087`).
fn query_count(stack: &PostStack, bloom_mips: u32, smaa: bool, blit: bool) -> Option<u32> {
    // Scene and text, the SMAA tail, the present blit.
    let mut passes: u32 = 2 + if smaa { 3 } else { 0 } + u32::from(blit);
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

/// What a ring's query sets are sized for: the frame's pass layout (the post
/// stack, bloom's mips, SMAA, room for a capture's present blit) and the
/// target size. A change rebuilds the ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimingLayout {
    /// Each post pass, whether it is a bloom.
    post: Vec<bool>,
    bloom_mips: u32,
    smaa: bool,
    size: (u32, u32),
}

impl TimingLayout {
    pub(crate) fn new(stack: &PostStack, bloom_mips: u32, smaa: bool, size: (u32, u32)) -> Self {
        Self {
            post: stack
                .0
                .iter()
                .map(|pass| matches!(pass, PostPass::Bloom(_)))
                .collect(),
            bloom_mips,
            smaa,
            size,
        }
    }

    /// Queries a frame of this layout writes at most, a capture's present
    /// blit included; `None` past the query limit.
    pub(crate) fn query_count(&self, stack: &PostStack) -> Option<u32> {
        query_count(stack, self.bloom_mips, self.smaa, true)
    }
}

/// Where a ring slot is in its cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    Free,
    /// Taken by the frame being recorded.
    Recording,
    /// Submitted for frame `frame`; its readback maps.
    InFlight {
        frame: u64,
    },
}

/// The timestamp ring's slot logic over slots of any kind, so it runs
/// without a GPU: a frame takes a free slot or runs untimed, a submitted
/// slot is read once its map completes, oldest frame first, and is free
/// again after.
pub(crate) struct Ring<S> {
    layout: Option<TimingLayout>,
    states: Vec<SlotState>,
    slots: Vec<Option<S>>,
    skipped: u64,
}

impl<S> Default for Ring<S> {
    fn default() -> Self {
        Self {
            layout: None,
            states: Vec::new(),
            slots: Vec::new(),
            skipped: 0,
        }
    }
}

impl<S> Ring<S> {
    /// Rebuilds the ring with `build`'s slots when `layout` differs from its
    /// own, dropping the slots in flight; returns whether it rebuilt. A
    /// `build` that gives `None` leaves the layout untimed.
    pub(crate) fn ensure(
        &mut self,
        layout: TimingLayout,
        mut build: impl FnMut() -> Option<S>,
    ) -> bool {
        if self.layout.as_ref() == Some(&layout) {
            return false;
        }
        self.layout = Some(layout);
        self.slots = (0..RING_DEPTH).map_while(|_| build().map(Some)).collect();
        self.states = vec![SlotState::Free; self.slots.len()];
        true
    }

    /// A free slot for the frame being recorded, moved out of the ring, or
    /// `None` when every slot is in flight, which counts a skipped frame.
    pub(crate) fn take(&mut self) -> Option<(usize, S)> {
        if self.slots.is_empty() {
            return None;
        }
        let Some(index) = self
            .states
            .iter()
            .position(|state| *state == SlotState::Free)
        else {
            self.skipped += 1;
            return None;
        };
        self.states[index] = SlotState::Recording;
        self.slots[index].take().map(|slot| (index, slot))
    }

    /// Returns a slot whose frame wrote nothing to read: free again.
    pub(crate) fn release(&mut self, index: usize, slot: S) {
        self.put(index, slot, SlotState::Free);
    }

    /// Returns a slot submitted for `frame`, its readback mapping.
    pub(crate) fn submit(&mut self, index: usize, slot: S, frame: u64) {
        self.put(index, slot, SlotState::InFlight { frame });
    }

    fn put(&mut self, index: usize, slot: S, state: SlotState) {
        // A rebuild while the slot was out drops it.
        if self.states.get(index) == Some(&SlotState::Recording) {
            self.states[index] = state;
            self.slots[index] = Some(slot);
        }
    }

    /// Hands each in-flight slot that `ready` says has mapped to `read`, with
    /// the frame it measured, oldest frame first; each is then free again,
    /// so it reports once.
    pub(crate) fn complete(
        &mut self,
        ready: impl Fn(&S) -> bool,
        mut read: impl FnMut(u64, &mut S),
    ) {
        let mut done: Vec<(u64, usize)> = self
            .states
            .iter()
            .enumerate()
            .filter_map(|(index, state)| match *state {
                SlotState::InFlight { frame } => self.slots[index]
                    .as_ref()
                    .is_some_and(&ready)
                    .then_some((frame, index)),
                _ => None,
            })
            .collect();
        done.sort_unstable();
        for (frame, index) in done {
            if let Some(slot) = self.slots[index].as_mut() {
                read(frame, slot);
            }
            self.states[index] = SlotState::Free;
        }
    }

    /// Frames that ran untimed with every slot in flight.
    pub(crate) fn skipped(&self) -> u64 {
        self.skipped
    }
}

/// A readback's state, as its map callback leaves it.
const MAP_PENDING: u8 = 0;
const MAP_DONE: u8 = 1;
const MAP_FAILED: u8 = 2;

/// One ring slot: a query set, its resolve and readback buffers and the
/// labels of the passes a frame wrote into it.
pub(crate) struct TimingResources {
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
    labels: Vec<String>,
    capacity: u32,
    resolved: bool,
    map: Arc<AtomicU8>,
}

impl TimingResources {
    pub(crate) fn new(device: &wgpu::Device, count: u32) -> Self {
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
        Self {
            query_set,
            resolve_buf,
            readback_buf,
            labels: Vec::with_capacity(count as usize / 2),
            capacity: count,
            resolved: false,
            map: Arc::new(AtomicU8::new(MAP_PENDING)),
        }
    }

    /// Starts a frame on the slot: no labels, nothing resolved.
    pub(crate) fn begin(&mut self) {
        self.labels.clear();
        self.resolved = false;
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

    /// Resolves every query taken so far, in the frame's last command
    /// buffer: those written by the buffer submitted before the acquire are
    /// resolved there too (`D-145`).
    pub(crate) fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let count = self.labels.len() as u32 * 2;
        encoder.resolve_query_set(&self.query_set, 0..count, &self.resolve_buf, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buf,
            0,
            &self.readback_buf,
            0,
            u64::from(count) * 8,
        );
        self.resolved = count > 0;
    }

    /// Whether the frame resolved queries for a readback to read.
    pub(crate) fn resolved(&self) -> bool {
        self.resolved
    }

    /// Maps the resolved stamps after the frame's submit, without waiting:
    /// a later frame's poll completes it.
    pub(crate) fn map(&self) {
        self.map.store(MAP_PENDING, Ordering::Release);
        let state = Arc::clone(&self.map);
        self.readback_buf
            .slice(..self.labels.len() as u64 * 16)
            .map_async(wgpu::MapMode::Read, move |result| {
                let done = if result.is_ok() { MAP_DONE } else { MAP_FAILED };
                state.store(done, Ordering::Release);
            });
    }

    /// Whether the map has completed, either way.
    pub(crate) fn mapped(&self) -> bool {
        self.map.load(Ordering::Acquire) != MAP_PENDING
    }

    /// Decodes the mapped stamps of frame `frame` into `out` and unmaps.
    pub(crate) fn read(&mut self, frame: u64, period_ns: f32, out: &mut GpuFrameTimings) {
        if self.map.load(Ordering::Acquire) != MAP_DONE {
            log::warn!("GPU timing readback of frame {frame} failed to map");
            return;
        }
        let slice = self.readback_buf.slice(..self.labels.len() as u64 * 16);
        let stamps = slice.get_mapped_range().map(|data| {
            data.as_chunks::<8>()
                .0
                .iter()
                .map(|bytes| u64::from_le_bytes(*bytes))
                .collect::<Vec<_>>()
        });
        self.readback_buf.unmap();
        match stamps {
            Ok(stamps) => out.decode(frame, &self.labels, &stamps, period_ns),
            Err(e) => log::warn!("GPU timing readback failed: {e:?}"),
        }
    }
}

/// CPU render-frame timing.
#[derive(Debug, Clone, Default)]
pub struct CpuFrameTimings {
    /// Surface acquire time.
    pub acquire_ms: f32,
    /// Encode/command-recording time: the whole frame's, before and after
    /// the acquire.
    pub encode_ms: f32,
    /// The part of `encode_ms` recorded before the acquire: the camera
    /// upload, the text preparation and the passes before the one that
    /// writes the swapchain (`D-145`).
    pub encode_pre_acquire_ms: f32,
    /// Submit/present/readback wait time, the submit before the acquire
    /// included.
    pub submit_present_ms: f32,
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
