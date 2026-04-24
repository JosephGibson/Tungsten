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
///   the scene pass and the present pass, ping-ponging `PostPing`/`PostPong`
///   (even index = Ping, odd = Pong). These passes never clear — they write
///   fullscreen fragments.
/// - The final `present` pass is always a fullscreen blit into the swapchain;
///   it never clears.
///
/// When `post_stack_len == 0` the output is byte-identical to the pre-M26
/// pass order — the M25 baseline gate depends on this.
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

    let mut passes = Vec::with_capacity(2 + post_stack_len);
    passes.push(scene);

    for i in 0..post_stack_len {
        passes.push(PassDesc::new(post_pass_label(i), post_target_for_index(i)));
    }

    passes.push(PassDesc::new("tungsten_present_pass", TargetId::Swapchain));

    PassOrder(passes)
}

#[cfg(test)]
#[path = "../tests/passes_order.rs"]
mod tests;
