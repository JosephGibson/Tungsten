use super::*;
use tungsten_core::post::{BloomParams, VignetteParams};

#[test]
fn counts_real_passes_and_rejects_query_overflow() {
    // Scene and text; a capture frame adds its present blit.
    assert_eq!(query_count(&PostStack::default(), 6, false, false), Some(4));
    assert_eq!(query_count(&PostStack::default(), 6, false, true), Some(6));
    let stack = PostStack(vec![
        PostPass::Bloom(BloomParams::default()),
        PostPass::Vignette(VignetteParams::default()),
        PostPass::Bloom(BloomParams::default()),
    ]);
    assert_eq!(query_count(&stack, 6, true, false), Some(60));
    assert_eq!(query_count(&stack, 6, true, true), Some(62));
    assert_eq!(query_count(&stack, 1, false, true), Some(16));
    assert_eq!(query_count(&stack, 0, false, true), Some(8));
    assert_eq!(query_count(&stack, 0, false, false), Some(6));
    assert_eq!(query_count(&stack, u32::MAX, false, false), None);
    let huge = PostStack(vec![PostPass::Vignette(VignetteParams::default()); 2047]);
    assert_eq!(query_count(&huge, 6, false, false), None);
}

#[test]
fn scene_compatibility_span_gaps_and_stale_data() {
    let labels = ["scene", "post0_bloom_threshold", "present"].map(str::to_string);
    let mut out = GpuFrameTimings::default();
    out.decode(7, &labels, &[10, 30, 40, 50, 80, 100], 1000.0);
    assert_eq!(out.frame_gpu_ms, Some(0.02));
    assert_eq!(out.render_gpu_ms, Some(0.09));
    assert_eq!(out.pass_gpu_ms.len(), 3);
    assert_eq!(out.samples.len(), 1);
    assert_eq!(out.samples[0].frame, 7);
    assert_eq!(out.samples[0].scene_ms, Some(0.02));
    out.decode(8, &labels, &[1, 2], 1.0);
    assert!(
        out.frame_gpu_ms.is_none() && out.render_gpu_ms.is_none() && out.pass_gpu_ms.is_empty()
    );
    assert_eq!(out.samples.len(), 1, "a mismatched readback adds no sample");
    out.decode(9, &["scene".into()], &[u64::MAX - 9, 10], 1000.0);
    assert_eq!(out.frame_gpu_ms, Some(0.02));
    out.clear_durations();
    assert!(out.pass_gpu_ms.is_empty() && out.frame_gpu_ms.is_none() && out.samples.is_empty());
}

#[test]
fn cpu_frame_timings_start_at_zero_with_the_pre_acquire_part() {
    let timings = CpuFrameTimings::default();
    assert_eq!(timings.acquire_ms, 0.0);
    assert_eq!(timings.encode_ms, 0.0);
    assert_eq!(timings.encode_pre_acquire_ms, 0.0);
    assert_eq!(timings.submit_present_ms, 0.0);
}

/// A slot standing in for the GPU's: whether its map has completed.
#[derive(Debug, Default)]
struct FakeSlot {
    mapped: bool,
}

fn layout(width: u32) -> TimingLayout {
    TimingLayout::new(&PostStack::default(), 6, false, (width, 720))
}

fn ring() -> Ring<FakeSlot> {
    let mut ring = Ring::default();
    assert!(ring.ensure(layout(1280), || Some(FakeSlot::default())));
    ring
}

/// Submits a frame on the ring's next free slot.
fn submit(ring: &mut Ring<FakeSlot>, frame: u64) {
    let (index, slot) = ring.take().expect("a free slot");
    ring.submit(index, slot, frame);
}

fn complete_all(ring: &mut Ring<FakeSlot>) -> Vec<u64> {
    let mut frames = Vec::new();
    ring.complete(|slot| slot.mapped, |frame, _| frames.push(frame));
    frames
}

fn map_all(ring: &mut Ring<FakeSlot>) {
    for slot in ring.slots.iter_mut().flatten() {
        slot.mapped = true;
    }
}

#[test]
fn timing_ring_full_skips_a_frame_and_counts_it() {
    let mut ring = ring();
    for frame in 1..=RING_DEPTH as u64 {
        submit(&mut ring, frame);
    }
    assert!(ring.take().is_none(), "every slot is in flight");
    assert!(ring.take().is_none());
    assert_eq!(ring.skipped(), 2);
    map_all(&mut ring);
    assert_eq!(
        complete_all(&mut ring),
        (1..=RING_DEPTH as u64).collect::<Vec<_>>()
    );
    assert!(ring.take().is_some(), "a read slot is free again");
}

#[test]
fn timing_ring_reports_each_completed_slot_once_oldest_first() {
    let mut ring = ring();
    submit(&mut ring, 10);
    submit(&mut ring, 11);
    assert!(complete_all(&mut ring).is_empty(), "nothing mapped yet");
    for slot in ring.slots.iter_mut().flatten() {
        slot.mapped = true;
    }
    assert_eq!(complete_all(&mut ring), vec![10, 11]);
    assert!(complete_all(&mut ring).is_empty(), "each slot reports once");

    // A released slot, whose frame wrote nothing, reports nothing.
    let (index, mut slot) = ring.take().expect("a free slot");
    slot.mapped = true;
    ring.release(index, slot);
    assert!(complete_all(&mut ring).is_empty());
}

#[test]
fn timing_ring_rebuilds_on_a_layout_change_only() {
    let mut ring = ring();
    submit(&mut ring, 1);
    assert!(!ring.ensure(layout(1280), || panic!("the same layout keeps its slots")));
    let mut built = 0;
    assert!(ring.ensure(layout(1920), || {
        built += 1;
        Some(FakeSlot::default())
    }));
    assert_eq!(built, RING_DEPTH);
    map_all(&mut ring);
    assert!(
        complete_all(&mut ring).is_empty(),
        "the old layout's frames are dropped"
    );
    let stack = PostStack(vec![PostPass::Bloom(BloomParams::default())]);
    assert!(
        ring.ensure(TimingLayout::new(&stack, 6, false, (1920, 720)), || Some(
            FakeSlot::default()
        ))
    );

    // A slot out for recording across a rebuild is dropped on return.
    let (index, slot) = ring.take().expect("a free slot");
    assert!(ring.ensure(layout(800), || Some(FakeSlot::default())));
    ring.submit(index, slot, 5);
    map_all(&mut ring);
    assert!(complete_all(&mut ring).is_empty());
    let mut untimed: Ring<FakeSlot> = Ring::default();
    assert!(untimed.ensure(layout(800), || None));
    assert!(untimed.take().is_none());
    assert_eq!(untimed.skipped(), 0, "an untimed layout skips nothing");
}
