//! Translate `PassDesc` into a live `wgpu::RenderPass`.

use super::desc::{PassDesc, TargetId};
use crate::targets::RenderTargetPool;

pub struct PassRecorder;

impl PassRecorder {
    /// Begin a render pass described by `desc`. `clear_override` lets the
    /// renderer substitute its current clear color for the default baked
    /// into `PassDesc`. Returns the live `RenderPass` for draw recording.
    ///
    /// # Panics
    ///
    /// Panics when a referenced `TargetId` is not allocated in `pool` / the
    /// provided swapchain view. Those conditions are config bugs the caller
    /// controls, so we assert loudly rather than silently falling back.
    pub fn begin<'a>(
        encoder: &'a mut wgpu::CommandEncoder,
        desc: &PassDesc,
        pool: &'a RenderTargetPool,
        swap_view: &'a wgpu::TextureView,
        clear_override: Option<wgpu::Color>,
    ) -> wgpu::RenderPass<'a> {
        let color_view = resolve_view(desc.color, pool, swap_view);
        let resolve_view_opt = desc
            .color_resolve
            .map(|target| resolve_view(target, pool, swap_view));

        let load = match (desc.clear, clear_override) {
            (Some(baseline), override_color) => {
                wgpu::LoadOp::Clear(override_color.unwrap_or(baseline))
            }
            (None, _) => wgpu::LoadOp::Load,
        };

        let depth_attachment = desc.depth.map(|target| {
            let view = resolve_view(target, pool, swap_view);
            let load = desc
                .depth_clear
                .map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear);
            wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }
        });

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(desc.label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: resolve_view_opt,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_attachment,
            ..Default::default()
        })
    }
}

fn resolve_view<'a>(
    id: TargetId,
    pool: &'a RenderTargetPool,
    swap_view: &'a wgpu::TextureView,
) -> &'a wgpu::TextureView {
    match id {
        TargetId::SceneColor => pool.scene.color_view(),
        TargetId::SceneColorMsaa => pool
            .scene
            .color_msaa_view()
            .expect("SceneColorMsaa requested but sample_count == 1"),
        TargetId::SceneDepth => pool
            .scene
            .depth_view()
            .expect("SceneDepth requested but depth_enabled = false"),
        TargetId::PostPing => pool.scene.post_ping_view(),
        TargetId::PostPong => pool.scene.post_pong_view(),
        TargetId::SmaaEdges => pool
            .scene
            .smaa_edges_view()
            .expect("SmaaEdges requested but post_aa == Off"),
        TargetId::SmaaBlend => pool
            .scene
            .smaa_blend_view()
            .expect("SmaaBlend requested but post_aa == Off"),
        TargetId::PresentSource => pool
            .scene
            .present_source_view()
            .expect("PresentSource requested but post_aa == Off"),
        TargetId::Swapchain => swap_view,
    }
}
