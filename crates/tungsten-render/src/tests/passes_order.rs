use super::*;
use tungsten_core::config::PostAaMode;

#[test]
fn default_order_msaa1_cpu_stable_is_scene_overlay_then_present() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true, 0, PostAaMode::Off);
    let passes = order.as_slice();
    assert_eq!(passes.len(), 3);

    let scene = &passes[0];
    assert_eq!(scene.label, "tungsten_scene_pass");
    assert_eq!(scene.color, TargetId::SceneColor);
    assert!(scene.color_resolve.is_none());
    assert!(scene.depth.is_none());
    assert!(scene.clear.is_some());
    assert!(scene.depth_clear.is_none());

    let overlay = &passes[1];
    assert_eq!(overlay.label, "tungsten_text_overlay_pass");
    assert_eq!(overlay.color, TargetId::SceneColor);
    assert!(overlay.clear.is_none());
    assert!(overlay.depth.is_none());

    let present = &passes[2];
    assert_eq!(present.label, "tungsten_present_pass");
    assert_eq!(present.color, TargetId::Swapchain);
    assert!(present.color_resolve.is_none());
    assert!(present.depth.is_none());
    assert!(present.clear.is_none());
}

#[test]
fn default_order_msaa4_cpu_stable_resolves_to_scene_color() {
    let order = default_pass_order(4, DepthSortMode::CpuStable, true, 0, PostAaMode::Off);
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColorMsaa);
    assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    assert!(scene.depth.is_none());
}

#[test]
fn default_order_msaa4_gpu_depth_attaches_depth_and_resolve() {
    let order = default_pass_order(4, DepthSortMode::GpuDepth, true, 0, PostAaMode::Off);
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColorMsaa);
    assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    assert_eq!(scene.depth, Some(TargetId::SceneDepth));
    assert_eq!(scene.depth_clear, Some(1.0));
}

#[test]
fn default_order_msaa1_gpu_depth_attaches_depth_no_resolve() {
    let order = default_pass_order(1, DepthSortMode::GpuDepth, true, 0, PostAaMode::Off);
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColor);
    assert!(scene.color_resolve.is_none());
    assert_eq!(scene.depth, Some(TargetId::SceneDepth));
}

#[test]
fn gpu_depth_with_depth_disabled_drops_the_depth_attachment() {
    // `depth_sort = GpuDepth` + `depth_enabled = false` would otherwise
    // reference a `SceneDepth` view that `SceneTarget::new` never allocated
    // under the same flag, and the recorder would panic.
    let order = default_pass_order(1, DepthSortMode::GpuDepth, false, 0, PostAaMode::Off);
    let scene = &order.as_slice()[0];
    assert!(scene.depth.is_none());
    assert!(scene.depth_clear.is_none());
}

#[test]
fn text_overlay_follows_scene_when_post_stack_empty() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true, 0, PostAaMode::Off);
    let passes = order.as_slice();
    assert_eq!(passes.len(), 3);
    assert_eq!(passes[0].label, "tungsten_scene_pass");
    assert_eq!(passes[1].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[1].color, TargetId::SceneColor);
    assert_eq!(passes[2].label, "tungsten_present_pass");
    assert_eq!(passes[2].color, TargetId::Swapchain);
}

#[test]
fn post_stack_one_splices_post_then_text_overlay_on_ping() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true, 1, PostAaMode::Off);
    let passes = order.as_slice();
    assert_eq!(passes.len(), 4);
    assert_eq!(passes[0].color, TargetId::SceneColor);
    assert_eq!(passes[1].label, "tungsten_post_pass_0");
    assert_eq!(passes[1].color, TargetId::PostPing);
    assert!(passes[1].clear.is_none());
    assert_eq!(passes[2].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[2].color, TargetId::PostPing);
    assert!(passes[2].clear.is_none());
    assert_eq!(passes[3].label, "tungsten_present_pass");
    assert_eq!(passes[3].color, TargetId::Swapchain);
}

#[test]
fn post_stack_two_alternates_and_overlay_lands_on_pong() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true, 2, PostAaMode::Off);
    let passes = order.as_slice();
    assert_eq!(passes.len(), 5);
    assert_eq!(passes[1].color, TargetId::PostPing);
    assert_eq!(passes[2].color, TargetId::PostPong);
    assert_eq!(passes[3].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[3].color, TargetId::PostPong);
}

#[test]
fn post_stack_seventeen_ends_on_ping_pattern() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true, 17, PostAaMode::Off);
    let passes = order.as_slice();
    // scene + 17 post + overlay + present
    assert_eq!(passes.len(), 20);
    for i in 0..17 {
        let expected = if i % 2 == 0 {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        };
        assert_eq!(passes[1 + i].color, expected, "pass {i}");
    }
    assert_eq!(passes[18].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[18].color, TargetId::PostPing);
    assert_eq!(passes[19].color, TargetId::Swapchain);
}

#[test]
fn post_aa_off_matches_m26_baseline_across_matrix() {
    // Step-9 invariant: with `post_aa = Off`, the pass list must match the
    // M26 baseline byte-for-byte across the msaa x depth_sort x stack-length
    // matrix. The new signature is purely additive.
    for msaa in [1u32, 4] {
        for depth_sort in [DepthSortMode::CpuStable, DepthSortMode::GpuDepth] {
            for stack in [0usize, 1, 3] {
                let order = default_pass_order(msaa, depth_sort, true, stack, PostAaMode::Off);
                let baseline_len = 3 + stack;
                assert_eq!(
                    order.as_slice().len(),
                    baseline_len,
                    "msaa={msaa} sort={depth_sort:?} stack={stack}"
                );
                let last = order.as_slice().last().unwrap();
                assert_eq!(last.color, TargetId::Swapchain);
            }
        }
    }
}

#[test]
fn post_aa_smaa_inserts_three_passes_and_present_source_overlay() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true, 2, PostAaMode::SmaaHigh);
    let passes = order.as_slice();
    // scene + 2 post + 3 smaa + overlay + present
    assert_eq!(passes.len(), 8);
    assert_eq!(passes[3].label, "tungsten_smaa_edge_pass");
    assert_eq!(passes[3].color, TargetId::SmaaEdges);
    assert!(passes[3].clear.is_some());
    assert_eq!(passes[4].label, "tungsten_smaa_blend_weights_pass");
    assert_eq!(passes[4].color, TargetId::SmaaBlend);
    assert!(passes[4].clear.is_some());
    assert_eq!(passes[5].label, "tungsten_smaa_neighborhood_pass");
    assert_eq!(passes[5].color, TargetId::PresentSource);
    assert_eq!(passes[6].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[6].color, TargetId::PresentSource);
    assert_eq!(passes[7].color, TargetId::Swapchain);
}

#[test]
fn text_overlay_target_with_smaa_is_present_source() {
    assert_eq!(
        text_overlay_target(0, PostAaMode::SmaaLow),
        TargetId::PresentSource
    );
    assert_eq!(
        text_overlay_target(3, PostAaMode::SmaaUltra),
        TargetId::PresentSource
    );
}

#[test]
fn text_overlay_target_off_matches_m26_baseline() {
    assert_eq!(
        text_overlay_target(0, PostAaMode::Off),
        TargetId::SceneColor
    );
    assert_eq!(text_overlay_target(1, PostAaMode::Off), TargetId::PostPing);
    assert_eq!(text_overlay_target(2, PostAaMode::Off), TargetId::PostPong);
}
