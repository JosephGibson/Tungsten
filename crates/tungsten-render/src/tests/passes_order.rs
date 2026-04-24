use super::*;

#[test]
fn default_order_msaa1_cpu_stable_is_scene_then_present() {
    let order = default_pass_order(1, DepthSortMode::CpuStable, true);
    let passes = order.as_slice();
    assert_eq!(passes.len(), 2);

    let scene = &passes[0];
    assert_eq!(scene.label, "tungsten_scene_pass");
    assert_eq!(scene.color, TargetId::SceneColor);
    assert!(scene.color_resolve.is_none());
    assert!(scene.depth.is_none());
    assert!(scene.clear.is_some());
    assert!(scene.depth_clear.is_none());

    let present = &passes[1];
    assert_eq!(present.label, "tungsten_present_pass");
    assert_eq!(present.color, TargetId::Swapchain);
    assert!(present.color_resolve.is_none());
    assert!(present.depth.is_none());
    assert!(present.clear.is_none());
}

#[test]
fn default_order_msaa4_cpu_stable_resolves_to_scene_color() {
    let order = default_pass_order(4, DepthSortMode::CpuStable, true);
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColorMsaa);
    assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    assert!(scene.depth.is_none());
}

#[test]
fn default_order_msaa4_gpu_depth_attaches_depth_and_resolve() {
    let order = default_pass_order(4, DepthSortMode::GpuDepth, true);
    let scene = &order.as_slice()[0];
    assert_eq!(scene.color, TargetId::SceneColorMsaa);
    assert_eq!(scene.color_resolve, Some(TargetId::SceneColor));
    assert_eq!(scene.depth, Some(TargetId::SceneDepth));
    assert_eq!(scene.depth_clear, Some(1.0));
}

#[test]
fn default_order_msaa1_gpu_depth_attaches_depth_no_resolve() {
    let order = default_pass_order(1, DepthSortMode::GpuDepth, true);
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
    let order = default_pass_order(1, DepthSortMode::GpuDepth, false);
    let scene = &order.as_slice()[0];
    assert!(scene.depth.is_none());
    assert!(scene.depth_clear.is_none());
}
