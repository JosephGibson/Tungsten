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
