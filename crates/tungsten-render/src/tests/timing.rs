use super::*;

#[test]
fn a_layout_holds_two_queries_a_pass_and_rejects_query_overflow() {
    let count = |passes| TimingLayout::new(passes, (1280, 720)).query_count();
    // Scene and text, with room for a capture's present blit.
    assert_eq!(count(3), Some(6));
    // Two blooms of six levels around a vignette, SMAA's tail and the blit.
    assert_eq!(count(31), Some(62));
    assert_eq!(count(2048), Some(wgpu::QUERY_SET_MAX_QUERIES));
    assert_eq!(count(2049), None);
    assert_eq!(count(usize::MAX), None);
}

#[test]
fn a_layout_moves_with_its_pass_count_or_its_size() {
    // Bloom then vignette draws fifteen passes, and fourteen where bloom's
    // composite draws the vignette (`D-307`; the frame's own test is
    // `graph_frame::a_run_of_joined_stages_is_one_pass_under_its_stages_names`):
    // with the blit's room, a stack that starts or stops fusing rebuilds the
    // ring.
    let size = (3840, 2160);
    assert_eq!(TimingLayout::new(16, size), TimingLayout::new(16, size));
    assert_ne!(TimingLayout::new(16, size), TimingLayout::new(15, size));
    assert_ne!(
        TimingLayout::new(16, size),
        TimingLayout::new(16, (1920, 1080))
    );
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
    TimingLayout::new(3, (width, 720))
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
    // A stack that adds passes at the same size.
    assert!(ring.ensure(TimingLayout::new(15, (1920, 720)), || Some(
        FakeSlot::default()
    )));

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
