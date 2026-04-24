//! Ordered pass list for the default M25 frame.

use super::desc::{PassDesc, TargetId};
use tungsten_core::config::DepthSortMode;

/// `Vec<PassDesc>` describing the frame in draw order.
#[derive(Debug, Clone)]
pub struct PassOrder(pub Vec<PassDesc>);

impl PassOrder {
    #[must_use]
    pub fn as_slice(&self) -> &[PassDesc] {
        &self.0
    }
}

/// Preallocated labels for spliced post passes. Static so each `PassDesc`
/// can keep its `&'static str` label without allocating per frame.
const POST_PASS_LABELS: [&str; 32] = [
    "tungsten_post_pass_0",
    "tungsten_post_pass_1",
    "tungsten_post_pass_2",
    "tungsten_post_pass_3",
    "tungsten_post_pass_4",
    "tungsten_post_pass_5",
    "tungsten_post_pass_6",
    "tungsten_post_pass_7",
    "tungsten_post_pass_8",
    "tungsten_post_pass_9",
    "tungsten_post_pass_10",
    "tungsten_post_pass_11",
    "tungsten_post_pass_12",
    "tungsten_post_pass_13",
    "tungsten_post_pass_14",
    "tungsten_post_pass_15",
    "tungsten_post_pass_16",
    "tungsten_post_pass_17",
    "tungsten_post_pass_18",
    "tungsten_post_pass_19",
    "tungsten_post_pass_20",
    "tungsten_post_pass_21",
    "tungsten_post_pass_22",
    "tungsten_post_pass_23",
    "tungsten_post_pass_24",
    "tungsten_post_pass_25",
    "tungsten_post_pass_26",
    "tungsten_post_pass_27",
    "tungsten_post_pass_28",
    "tungsten_post_pass_29",
    "tungsten_post_pass_30",
    "tungsten_post_pass_31",
];

fn post_target_for_index(i: usize) -> TargetId {
    if i % 2 == 0 {
        TargetId::PostPing
    } else {
        TargetId::PostPong
    }
}

/// Label for the `i`-th post pass. Beyond the preallocated table the label
/// defaults to "tungsten_post_pass_overflow"; only matters at 32+ stacks.
fn post_pass_label(i: usize) -> &'static str {
    POST_PASS_LABELS
        .get(i)
        .copied()
        .unwrap_or("tungsten_post_pass_overflow")
}

/// Default pass order with optional post-stack splice.
///
/// - `msaa > 1` routes the scene through `SceneColorMsaa` with resolve to `SceneColor`.
/// - `depth_sort == GpuDepth && depth_enabled` attaches `SceneDepth` and clears to 1.0.
/// - For `post_stack_len > 0`, `post_stack_len` passes are appended between
///   the scene pass and the text-overlay pass, ping-ponging `PostPing`/`PostPong`
///   (even index = Ping, odd = Pong). These passes never clear — they write
///   fullscreen fragments.
/// - A `tungsten_text_overlay_pass` runs after the post stack (or immediately
///   after the scene pass when the stack is empty). It loads the present-blit
///   source and composites screen-space text on top, so text is never sampled
///   by post shaders.
/// - The final `present` pass is always a fullscreen blit into the swapchain;
///   it never clears.
#[must_use]
pub fn default_pass_order(
    msaa: u32,
    depth_sort: DepthSortMode,
    depth_enabled: bool,
    post_stack_len: usize,
) -> PassOrder {
    let (color, resolve) = if msaa > 1 {
        (TargetId::SceneColorMsaa, Some(TargetId::SceneColor))
    } else {
        (TargetId::SceneColor, None)
    };

    let mut scene =
        PassDesc::new("tungsten_scene_pass", color).with_clear(wgpu::Color::TRANSPARENT);
    if let Some(r) = resolve {
        scene = scene.with_resolve(r);
    }
    if depth_sort == DepthSortMode::GpuDepth && depth_enabled {
        scene = scene.with_depth(TargetId::SceneDepth, 1.0);
    }

    let mut passes = Vec::with_capacity(3 + post_stack_len);
    passes.push(scene);

    for i in 0..post_stack_len {
        passes.push(PassDesc::new(post_pass_label(i), post_target_for_index(i)));
    }

    let overlay_target = text_overlay_target(post_stack_len);
    passes.push(PassDesc::new(
        "tungsten_text_overlay_pass",
        overlay_target,
    ));

    passes.push(PassDesc::new("tungsten_present_pass", TargetId::Swapchain));

    PassOrder(passes)
}

/// Target the text-overlay pass writes into: whichever texture the present
/// blit will sample from. Mirrors `PostStackRenderer::final_target` but
/// collapses the empty-stack case to `SceneColor`.
#[must_use]
pub fn text_overlay_target(post_stack_len: usize) -> TargetId {
    if post_stack_len == 0 {
        TargetId::SceneColor
    } else if (post_stack_len - 1).is_multiple_of(2) {
        TargetId::PostPing
    } else {
        TargetId::PostPong
    }
}

#[cfg(test)]
#[path = "../tests/passes_order.rs"]
mod tests;
