//! Ordered pass list for the default M25 frame.

use super::desc::{PassDesc, TargetId};
use tungsten_core::config::{DepthSortMode, PostAaMode};

/// `Vec<PassDesc>` describing the frame in draw order.
#[derive(Debug, Clone)]
pub struct PassOrder(pub Vec<PassDesc>);

impl PassOrder {
    #[must_use]
    pub fn as_slice(&self) -> &[PassDesc] {
        &self.0
    }
}

/// How a frame reaches the swapchain (`D-087`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentPath {
    /// The last full-screen stage renders into the swapchain and the text
    /// overlay draws there. No present blit.
    Direct,
    /// Every stage renders offscreen and a blit copies the result into the
    /// swapchain. Capture frames take this path: the screenshot reads the
    /// blit's source.
    Blit,
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
    if i.is_multiple_of(2) {
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
/// - When `post_aa != Off` (M27), three SMAA passes (edge, blend weights,
///   neighborhood blend) splice in between the post stack and the text overlay
///   pass.
/// - A `tungsten_text_overlay_pass` runs after the post stack / SMAA tail and
///   composites screen-space text on top, so text is never sampled by post
///   shaders.
///
/// `present` picks the tail (`D-087`):
///
/// - [`PresentPath::Direct`]: the last full-screen stage writes the swapchain.
///   That is SMAA's neighborhood pass; without SMAA the last post pass (bloom
///   writes its composite there); with neither the scene pass, or its resolve
///   when `msaa > 1`. The text overlay then loads the swapchain, and there is
///   no present pass. The stage that first writes the swapchain clears it, so
///   wgpu adds no clear pass of its own for the fresh surface texture.
/// - [`PresentPath::Blit`]: every stage renders offscreen (the neighborhood
///   pass into `PresentSource`), the overlay writes [`text_overlay_target`]
///   and a final `tungsten_present_pass` blits that target into the
///   swapchain without clearing.
#[must_use]
pub fn default_pass_order(
    msaa: u32,
    depth_sort: DepthSortMode,
    depth_enabled: bool,
    post_stack_len: usize,
    post_aa: PostAaMode,
    present: PresentPath,
) -> PassOrder {
    let smaa_active = post_aa.is_smaa();
    let direct = present == PresentPath::Direct;
    let scene_is_last = direct && post_stack_len == 0 && !smaa_active;

    let (color, resolve) = match (msaa > 1, scene_is_last) {
        (true, true) => (TargetId::SceneColorMsaa, Some(TargetId::Swapchain)),
        (true, false) => (TargetId::SceneColorMsaa, Some(TargetId::SceneColor)),
        (false, true) => (TargetId::Swapchain, None),
        (false, false) => (TargetId::SceneColor, None),
    };

    let mut scene =
        PassDesc::new("tungsten_scene_pass", color).with_clear(wgpu::Color::TRANSPARENT);
    if let Some(r) = resolve {
        scene = scene.with_resolve(r);
    }
    if depth_sort == DepthSortMode::GpuDepth && depth_enabled {
        scene = scene.with_depth(TargetId::SceneDepth, 1.0);
    }

    let extra = if smaa_active { 3 } else { 0 };
    let mut passes = Vec::with_capacity(3 + post_stack_len + extra);
    passes.push(scene);

    for i in 0..post_stack_len {
        if direct && !smaa_active && i + 1 == post_stack_len {
            passes.push(
                PassDesc::new(post_pass_label(i), TargetId::Swapchain)
                    .with_clear(wgpu::Color::TRANSPARENT),
            );
        } else {
            passes.push(PassDesc::new(post_pass_label(i), post_target_for_index(i)));
        }
    }

    if smaa_active {
        passes.push(
            PassDesc::new("tungsten_smaa_edge_pass", TargetId::SmaaEdges)
                .with_clear(wgpu::Color::TRANSPARENT),
        );
        passes.push(
            PassDesc::new("tungsten_smaa_blend_weights_pass", TargetId::SmaaBlend)
                .with_clear(wgpu::Color::TRANSPARENT),
        );
        let neighborhood_target = if direct {
            TargetId::Swapchain
        } else {
            TargetId::PresentSource
        };
        passes.push(
            PassDesc::new("tungsten_smaa_neighborhood_pass", neighborhood_target)
                .with_clear(wgpu::Color::TRANSPARENT),
        );
    }

    let overlay_target = if direct {
        TargetId::Swapchain
    } else {
        text_overlay_target(post_stack_len, post_aa)
    };
    passes.push(PassDesc::new("tungsten_text_overlay_pass", overlay_target));

    if !direct {
        passes.push(PassDesc::new("tungsten_present_pass", TargetId::Swapchain));
    }

    PassOrder(passes)
}

/// Target the text-overlay pass writes into on a [`PresentPath::Blit`] frame:
/// whichever texture the present blit samples and the screenshot reads. When
/// `post_aa != Off` the SMAA tail has already composited into `PresentSource`,
/// so the overlay lands there. Otherwise it follows the M26 post-stack
/// ping-pong rule. On a direct frame the overlay writes the swapchain.
#[must_use]
pub fn text_overlay_target(post_stack_len: usize, post_aa: PostAaMode) -> TargetId {
    if post_aa.is_smaa() {
        return TargetId::PresentSource;
    }
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
