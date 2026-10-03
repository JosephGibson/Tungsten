use super::*;
use tungsten_core::config::PostAaMode;

// Blit order: the tail every frame had before `D-087`, which a capture frame
// keeps.

#[test]
fn default_order_msaa1_cpu_stable_is_scene_overlay_then_present() {
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        0,
        PostAaMode::Off,
        PresentPath::Blit,
    );
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
    let order = default_pass_order(
        4,
        DepthSortMode::CpuStable,
        true,
        0,
        PostAaMode::Off,
        PresentPath::Blit,
    );
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColorMsaa);
    assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    assert!(scene.depth.is_none());
}

#[test]
fn default_order_msaa4_gpu_depth_attaches_depth_and_resolve() {
    let order = default_pass_order(
        4,
        DepthSortMode::GpuDepth,
        true,
        0,
        PostAaMode::Off,
        PresentPath::Blit,
    );
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColorMsaa);
    assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    assert_eq!(scene.depth, Some(TargetId::SceneDepth));
    assert_eq!(scene.depth_clear, Some(1.0));
}

#[test]
fn default_order_msaa1_gpu_depth_attaches_depth_no_resolve() {
    let order = default_pass_order(
        1,
        DepthSortMode::GpuDepth,
        true,
        0,
        PostAaMode::Off,
        PresentPath::Blit,
    );
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
    let order = default_pass_order(
        1,
        DepthSortMode::GpuDepth,
        false,
        0,
        PostAaMode::Off,
        PresentPath::Blit,
    );
    let scene = &order.as_slice()[0];
    assert!(scene.depth.is_none());
    assert!(scene.depth_clear.is_none());
}

#[test]
fn text_overlay_follows_scene_when_post_stack_empty() {
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        0,
        PostAaMode::Off,
        PresentPath::Blit,
    );
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
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        1,
        PostAaMode::Off,
        PresentPath::Blit,
    );
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
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        2,
        PostAaMode::Off,
        PresentPath::Blit,
    );
    let passes = order.as_slice();
    assert_eq!(passes.len(), 5);
    assert_eq!(passes[1].color, TargetId::PostPing);
    assert_eq!(passes[2].color, TargetId::PostPong);
    assert_eq!(passes[3].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[3].color, TargetId::PostPong);
}

#[test]
fn post_stack_seventeen_ends_on_ping_pattern() {
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        17,
        PostAaMode::Off,
        PresentPath::Blit,
    );
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
    // `D-059` invariant: with `post_aa = Off`, the blit pass list must match
    // the M26 baseline byte-for-byte across the msaa x depth_sort x
    // stack-length matrix.
    for msaa in [1u32, 4] {
        for depth_sort in [DepthSortMode::CpuStable, DepthSortMode::GpuDepth] {
            for stack in [0usize, 1, 3] {
                let order = default_pass_order(
                    msaa,
                    depth_sort,
                    true,
                    stack,
                    PostAaMode::Off,
                    PresentPath::Blit,
                );
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
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        2,
        PostAaMode::SmaaHigh,
        PresentPath::Blit,
    );
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

#[test]
fn bloom_only_stack_emits_one_post_pass_writing_to_ping() {
    // M28 invariant: although bloom is recorded as multiple sub-passes through
    // the encoder, the outer pass list still allocates exactly one slot for
    // the bloom variant, writing to PostPing. This keeps text_overlay_idx and
    // present-source indexing stable across `PostPass::Bloom` insertion.
    let order = default_pass_order(
        1,
        DepthSortMode::CpuStable,
        true,
        1,
        PostAaMode::Off,
        PresentPath::Blit,
    );
    let passes = order.as_slice();
    assert_eq!(passes.len(), 4);
    assert_eq!(passes[0].label, "tungsten_scene_pass");
    assert_eq!(passes[1].label, "tungsten_post_pass_0");
    assert_eq!(passes[1].color, TargetId::PostPing);
    assert!(passes[1].clear.is_none());
    assert_eq!(passes[2].label, "tungsten_text_overlay_pass");
    assert_eq!(passes[2].color, TargetId::PostPing);
    assert_eq!(passes[3].label, "tungsten_present_pass");
}

// Direct order (`D-087`): the last full-screen stage writes the swapchain.

fn direct(msaa: u32, post_stack_len: usize, post_aa: PostAaMode) -> PassOrder {
    default_pass_order(
        msaa,
        DepthSortMode::CpuStable,
        true,
        post_stack_len,
        post_aa,
        PresentPath::Direct,
    )
}

fn labels(order: &PassOrder) -> Vec<&'static str> {
    order.as_slice().iter().map(|pass| pass.label).collect()
}

#[test]
fn direct_smaa_neighborhood_writes_the_swapchain_and_text_loads_it() {
    for stack in [0usize, 2] {
        let order = direct(1, stack, PostAaMode::SmaaHigh);
        let passes = order.as_slice();
        // scene + post + 3 smaa + overlay, no present pass.
        assert_eq!(passes.len(), 1 + stack + 4);
        // Everything SMAA samples stays offscreen.
        assert_eq!(passes[0].color, TargetId::SceneColor);
        for (i, pass) in passes[1..=stack].iter().enumerate() {
            assert_eq!(pass.color, post_target_for_index(i));
            assert!(pass.clear.is_none());
        }
        assert_eq!(passes[stack + 1].color, TargetId::SmaaEdges);
        assert_eq!(passes[stack + 2].color, TargetId::SmaaBlend);
        let neighborhood = &passes[stack + 3];
        assert_eq!(neighborhood.label, "tungsten_smaa_neighborhood_pass");
        assert_eq!(neighborhood.color, TargetId::Swapchain);
        assert!(neighborhood.clear.is_some());
        let overlay = &passes[stack + 4];
        assert_eq!(overlay.label, "tungsten_text_overlay_pass");
        assert_eq!(overlay.color, TargetId::Swapchain);
        assert!(overlay.clear.is_none());
    }
}

#[test]
fn direct_last_post_pass_writes_the_swapchain() {
    // A stock effect records into this pass; bloom records its composite
    // into the same target (the renderer hands it the slot's `color`).
    for stack in [1usize, 2, 3, 17] {
        let order = direct(1, stack, PostAaMode::Off);
        let passes = order.as_slice();
        assert_eq!(passes.len(), 1 + stack + 1);
        assert_eq!(passes[0].color, TargetId::SceneColor);
        for (i, pass) in passes[1..stack].iter().enumerate() {
            assert_eq!(pass.color, post_target_for_index(i), "pass {i}");
            assert!(pass.clear.is_none());
        }
        let last = &passes[stack];
        assert_eq!(last.label, post_pass_label(stack - 1));
        assert_eq!(last.color, TargetId::Swapchain);
        // A post pass loads its offscreen target; the swapchain is fresh.
        assert!(last.clear.is_some());
        let overlay = &passes[stack + 1];
        assert_eq!(overlay.label, "tungsten_text_overlay_pass");
        assert_eq!(overlay.color, TargetId::Swapchain);
        assert!(overlay.clear.is_none());
    }
}

#[test]
fn direct_scene_writes_the_swapchain_without_post_or_smaa() {
    let order = direct(1, 0, PostAaMode::Off);
    let passes = order.as_slice();
    assert_eq!(
        labels(&order),
        ["tungsten_scene_pass", "tungsten_text_overlay_pass"]
    );
    assert_eq!(passes[0].color, TargetId::Swapchain);
    assert!(passes[0].color_resolve.is_none());
    assert!(passes[0].clear.is_some());
    assert_eq!(passes[1].color, TargetId::Swapchain);
    assert!(passes[1].clear.is_none());

    // The depth attachment does not depend on the color target.
    let order = default_pass_order(
        1,
        DepthSortMode::GpuDepth,
        true,
        0,
        PostAaMode::Off,
        PresentPath::Direct,
    );
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::Swapchain);
    assert_eq!(scene.depth, Some(TargetId::SceneDepth));
    assert_eq!(scene.depth_clear, Some(1.0));
}

#[test]
fn direct_msaa_scene_resolves_into_the_swapchain() {
    let order = direct(4, 0, PostAaMode::Off);
    let passes = order.as_slice();
    assert_eq!(passes.len(), 2);
    assert_eq!(passes[0].color, TargetId::SceneColorMsaa);
    assert_eq!(passes[0].color_resolve, Some(TargetId::Swapchain));
    assert!(passes[0].clear.is_some());
    assert_eq!(passes[1].color, TargetId::Swapchain);

    // With a later full-screen stage the scene resolves offscreen as before.
    for (stack, post_aa) in [(1, PostAaMode::Off), (0, PostAaMode::SmaaHigh)] {
        let order = direct(4, stack, post_aa);
        let scene = &order.as_slice()[0];
        assert_eq!(scene.color, TargetId::SceneColorMsaa);
        assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    }
}

#[test]
fn capture_frame_keeps_the_blit_order() {
    // The screenshot reads the blit's source, so a capture frame leaves
    // every stage offscreen and ends with the present pass.
    for msaa in [1u32, 4] {
        for stack in [0usize, 1, 2] {
            for post_aa in [PostAaMode::Off, PostAaMode::SmaaHigh] {
                let order = default_pass_order(
                    msaa,
                    DepthSortMode::CpuStable,
                    true,
                    stack,
                    post_aa,
                    PresentPath::Blit,
                );
                let passes = order.as_slice();
                let (present, staged) = passes.split_last().unwrap();
                assert_eq!(present.label, "tungsten_present_pass");
                assert_eq!(present.color, TargetId::Swapchain);
                assert!(present.clear.is_none());
                let context = format!("msaa={msaa} stack={stack} aa={post_aa:?}");
                for pass in staged {
                    assert_ne!(pass.color, TargetId::Swapchain, "{context}");
                    assert_ne!(pass.color_resolve, Some(TargetId::Swapchain), "{context}");
                }
                let overlay = staged.last().unwrap();
                assert_eq!(overlay.label, "tungsten_text_overlay_pass");
                assert_eq!(
                    overlay.color,
                    text_overlay_target(stack, post_aa),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn direct_order_is_the_blit_order_without_its_present_pass() {
    // Same stages in the same order on both paths. On the direct path one
    // stage before the overlay writes the swapchain, it clears, and the
    // overlay loads what it wrote.
    for msaa in [1u32, 4] {
        for depth_sort in [DepthSortMode::CpuStable, DepthSortMode::GpuDepth] {
            for stack in [0usize, 1, 2, 3] {
                for post_aa in [PostAaMode::Off, PostAaMode::SmaaLow, PostAaMode::SmaaUltra] {
                    let context =
                        format!("msaa={msaa} sort={depth_sort:?} stack={stack} aa={post_aa:?}");
                    let order = |present| {
                        default_pass_order(msaa, depth_sort, true, stack, post_aa, present)
                    };
                    let (blit, direct) = (order(PresentPath::Blit), order(PresentPath::Direct));
                    let blit_labels = labels(&blit);
                    assert_eq!(
                        labels(&direct),
                        blit_labels[..blit_labels.len() - 1],
                        "{context}"
                    );

                    let (overlay, staged) = direct.as_slice().split_last().unwrap();
                    assert_eq!(overlay.color, TargetId::Swapchain, "{context}");
                    assert!(overlay.clear.is_none(), "{context}");
                    let writers: Vec<_> = staged
                        .iter()
                        .filter(|pass| {
                            pass.color == TargetId::Swapchain
                                || pass.color_resolve == Some(TargetId::Swapchain)
                        })
                        .collect();
                    assert_eq!(writers.len(), 1, "{context}");
                    assert!(writers[0].clear.is_some(), "{context}");
                    assert_eq!(
                        writers[0].label,
                        staged.last().unwrap().label,
                        "the last stage before the overlay writes the swapchain: {context}"
                    );
                    // Depth and the scene's sample count follow the config
                    // on both paths.
                    assert_eq!(staged[0].depth, blit.as_slice()[0].depth, "{context}");
                    assert_eq!(
                        staged[0].color == TargetId::SceneColorMsaa,
                        msaa > 1,
                        "{context}"
                    );
                }
            }
        }
    }
}
