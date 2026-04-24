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

/// M25 default pass order. Clear color is baked into the `scene` pass and
/// overwritten at record time with the current `Renderer::clear_color`.
///
/// - `msaa > 1` routes the scene through `SceneColorMsaa` with resolve to `SceneColor`.
/// - `depth_sort == GpuDepth && depth_enabled` attaches `SceneDepth` and clears to 1.0.
/// - The final `present` pass is always a fullscreen blit from `SceneColor`
///   into the swapchain; it never clears.
#[must_use]
pub fn default_pass_order(msaa: u32, depth_sort: DepthSortMode, depth_enabled: bool) -> PassOrder {
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

    let present = PassDesc::new("tungsten_present_pass", TargetId::Swapchain);

    PassOrder(vec![scene, present])
}

#[cfg(test)]
#[path = "../tests/passes_order.rs"]
mod tests;
