//! M25 offscreen scene targets: color + optional depth + optional MSAA.
//!
//! `SceneColor` always matches the swapchain sRGB format so the default path
//! can blit back onto the swapchain byte-identically to the 0.21 baseline.
//! `SceneDepth` uses `Depth32Float` (portable) and is allocated only when
//! `RenderConfig::depth_enabled` is true. `SceneColorMsaa` is allocated only
//! when `sample_count > 1` and resolves into `SceneColor`.
//!
//! M27 adds an optional SMAA presentation tail. When `post_aa != Off`, the
//! pool also allocates `SmaaEdges` (`Rg8Unorm`), `SmaaBlend` (`Rgba8Unorm`),
//! and a `PresentSource` texture sized to the surface. SMAA reads the scene
//! and post ping/pong textures through non-sRGB twin views so edge detection
//! sees gamma-encoded values.

use tungsten_core::config::PostAaMode;
use wgpu::{Extent3d, TextureFormat, TextureUsages};

/// Named render-target slots referenced by `PassDesc`.
///
/// M26 added `PostPing` / `PostPong`. M27 adds `SmaaEdges`, `SmaaBlend`, and
/// `PresentSource` for the optional SMAA presentation tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetId {
    SceneColor,
    SceneDepth,
    SceneColorMsaa,
    PostPing,
    PostPong,
    SmaaEdges,
    SmaaBlend,
    PresentSource,
    Swapchain,
}

/// Allocated SMAA-only working set + non-sRGB read views.
#[derive(Debug)]
pub struct SmaaTargets {
    pub edges: (wgpu::Texture, wgpu::TextureView),
    pub blend: (wgpu::Texture, wgpu::TextureView),
    pub present: (wgpu::Texture, wgpu::TextureView),
    /// Non-sRGB twin view re-binding `SceneColor`. SMAA edge detection samples
    /// gamma-encoded values, so this view is `Rgba8Unorm`/`Bgra8Unorm` over
    /// the same memory as the sRGB attachment view.
    pub scene_color_linear_view: wgpu::TextureView,
    pub post_ping_linear_view: wgpu::TextureView,
    pub post_pong_linear_view: wgpu::TextureView,
}

/// Allocated textures for the scene target pool.
#[derive(Debug)]
#[must_use]
pub struct SceneTarget {
    pub color: (wgpu::Texture, wgpu::TextureView),
    pub depth: Option<(wgpu::Texture, wgpu::TextureView)>,
    pub color_msaa: Option<(wgpu::Texture, wgpu::TextureView)>,
    /// M26 post-stack ping target. Always allocated; same format as `color`.
    pub post_ping: (wgpu::Texture, wgpu::TextureView),
    /// M26 post-stack pong target. Always allocated; same format as `color`.
    pub post_pong: (wgpu::Texture, wgpu::TextureView),
    /// M27 SMAA working set + non-sRGB read views. `None` when `post_aa == Off`.
    pub smaa: Option<SmaaTargets>,
    pub size: (u32, u32),
    pub sample_count: u32,
    pub format: TextureFormat,
    pub depth_enabled: bool,
    pub post_aa: PostAaMode,
}

/// Depth format for `SceneDepth`; portable across wgpu backends.
pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// SMAA edges target format. 2-channel unorm matches the upstream reference.
pub const SMAA_EDGES_FORMAT: TextureFormat = TextureFormat::Rg8Unorm;
/// SMAA blend-weights target format. 4-channel unorm for the per-pixel weights.
pub const SMAA_BLEND_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

/// Map an sRGB swapchain format to its non-sRGB twin so SMAA can sample
/// gamma-encoded values via `view_formats`. Returns `None` for already-linear
/// formats; in that case the SMAA read views collapse to the primary view.
#[must_use]
pub fn non_srgb_twin(format: TextureFormat) -> Option<TextureFormat> {
    match format {
        TextureFormat::Rgba8UnormSrgb => Some(TextureFormat::Rgba8Unorm),
        TextureFormat::Bgra8UnormSrgb => Some(TextureFormat::Bgra8Unorm),
        _ => None,
    }
}

impl SceneTarget {
    pub fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
        sample_count: u32,
        depth_enabled: bool,
        post_aa: PostAaMode,
    ) -> Self {
        let (w, h) = (size.0.max(1), size.1.max(1));
        let smaa_active = post_aa.is_smaa();
        let twin = if smaa_active {
            non_srgb_twin(format)
        } else {
            None
        };
        // Resolve target + screenshot source: needs COPY_SRC + TEXTURE_BINDING
        // for the present-blit sampling path. sample_count is always 1 here —
        // MSAA lives in a sibling `SceneColorMsaa` that resolves into this one.
        let color = create_resolved_color(device, "tungsten_scene_color", w, h, format, twin);
        // MSAA color: RENDER_ATTACHMENT-only. Multisampled textures cannot
        // carry COPY_SRC and the present blit reads the resolved texture,
        // never this one.
        let color_msaa = if sample_count > 1 {
            Some(create_msaa_color(
                device,
                "tungsten_scene_color_msaa",
                w,
                h,
                format,
                sample_count,
            ))
        } else {
            None
        };
        let depth = if depth_enabled {
            Some(create_depth(
                device,
                "tungsten_scene_depth",
                w,
                h,
                sample_count,
            ))
        } else {
            None
        };

        let post_ping = create_post_target(device, "tungsten_post_ping", w, h, format, twin);
        let post_pong = create_post_target(device, "tungsten_post_pong", w, h, format, twin);

        let smaa = if smaa_active {
            let edges = create_smaa_edges(device, w, h);
            let blend = create_smaa_blend(device, w, h);
            let present = create_present_source(device, w, h, format);
            // When the swapchain format is already linear there is no twin to
            // re-view through — SMAA samples the primary view directly. Build
            // descriptors against `twin` only when present.
            let scene_color_linear_view = make_linear_view(&color.0, twin, "scene_color_linear");
            let post_ping_linear_view = make_linear_view(&post_ping.0, twin, "post_ping_linear");
            let post_pong_linear_view = make_linear_view(&post_pong.0, twin, "post_pong_linear");
            Some(SmaaTargets {
                edges,
                blend,
                present,
                scene_color_linear_view,
                post_ping_linear_view,
                post_pong_linear_view,
            })
        } else {
            None
        };

        Self {
            color,
            depth,
            color_msaa,
            post_ping,
            post_pong,
            smaa,
            size: (w, h),
            sample_count,
            format,
            depth_enabled,
            post_aa,
        }
    }

    #[must_use]
    pub fn post_ping_view(&self) -> &wgpu::TextureView {
        &self.post_ping.1
    }

    #[must_use]
    pub fn post_pong_view(&self) -> &wgpu::TextureView {
        &self.post_pong.1
    }

    #[must_use]
    pub fn post_ping_texture(&self) -> &wgpu::Texture {
        &self.post_ping.0
    }

    #[must_use]
    pub fn post_pong_texture(&self) -> &wgpu::Texture {
        &self.post_pong.0
    }

    #[must_use]
    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.color.1
    }

    #[must_use]
    pub fn color_texture(&self) -> &wgpu::Texture {
        &self.color.0
    }

    #[must_use]
    pub fn color_msaa_view(&self) -> Option<&wgpu::TextureView> {
        self.color_msaa.as_ref().map(|(_, v)| v)
    }

    #[must_use]
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.depth.as_ref().map(|(_, v)| v)
    }

    #[must_use]
    pub fn smaa_edges_view(&self) -> Option<&wgpu::TextureView> {
        self.smaa.as_ref().map(|s| &s.edges.1)
    }

    #[must_use]
    pub fn smaa_blend_view(&self) -> Option<&wgpu::TextureView> {
        self.smaa.as_ref().map(|s| &s.blend.1)
    }

    #[must_use]
    pub fn present_source_view(&self) -> Option<&wgpu::TextureView> {
        self.smaa.as_ref().map(|s| &s.present.1)
    }

    #[must_use]
    pub fn present_source_texture(&self) -> Option<&wgpu::Texture> {
        self.smaa.as_ref().map(|s| &s.present.0)
    }

    /// SMAA read view for `SceneColor`: non-sRGB twin when one exists, else
    /// the primary view. `None` when SMAA is not active.
    #[must_use]
    pub fn scene_color_smaa_read_view(&self) -> Option<&wgpu::TextureView> {
        self.smaa.as_ref().map(|s| &s.scene_color_linear_view)
    }

    #[must_use]
    pub fn post_ping_smaa_read_view(&self) -> Option<&wgpu::TextureView> {
        self.smaa.as_ref().map(|s| &s.post_ping_linear_view)
    }

    #[must_use]
    pub fn post_pong_smaa_read_view(&self) -> Option<&wgpu::TextureView> {
        self.smaa.as_ref().map(|s| &s.post_pong_linear_view)
    }
}

/// Owns the scene-target allocation; reallocates on resize or config change.
#[derive(Debug)]
#[must_use]
pub struct RenderTargetPool {
    pub scene: SceneTarget,
}

impl RenderTargetPool {
    pub fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
        sample_count: u32,
        depth_enabled: bool,
        post_aa: PostAaMode,
    ) -> Self {
        Self {
            scene: SceneTarget::new(device, size, format, sample_count, depth_enabled, post_aa),
        }
    }

    /// Re-allocate when size or config changes. No-op when already matching.
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
        sample_count: u32,
        depth_enabled: bool,
        post_aa: PostAaMode,
    ) {
        let new_size = (size.0.max(1), size.1.max(1));
        let shape_changed = self.scene.size != new_size
            || self.scene.format != format
            || self.scene.sample_count != sample_count
            || self.scene.depth_enabled != depth_enabled
            || self.scene.post_aa != post_aa;
        if !shape_changed {
            return;
        }
        self.scene = SceneTarget::new(
            device,
            new_size,
            format,
            sample_count,
            depth_enabled,
            post_aa,
        );
    }
}

fn create_resolved_color(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    format: TextureFormat,
    twin: Option<TextureFormat>,
) -> (wgpu::Texture, wgpu::TextureView) {
    let twin_slice: &[TextureFormat] = match &twin {
        Some(t) => std::slice::from_ref(t),
        None => &[],
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC,
        view_formats: twin_slice,
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_post_target(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    format: TextureFormat,
    twin: Option<TextureFormat>,
) -> (wgpu::Texture, wgpu::TextureView) {
    // Post targets need RENDER_ATTACHMENT (to be written by a pass) +
    // TEXTURE_BINDING (to be sampled by the next pass) + COPY_SRC so the
    // screenshot path can read back from the last-written post target.
    let twin_slice: &[TextureFormat] = match &twin {
        Some(t) => std::slice::from_ref(t),
        None => &[],
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC,
        view_formats: twin_slice,
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_msaa_color(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    format: TextureFormat,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    // Multisampled textures reject COPY_SRC in wgpu; this one is solely a
    // render target that resolves into the sibling sample_count=1 texture.
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_depth(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_smaa_edges(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tungsten_smaa_edges"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SMAA_EDGES_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_smaa_blend(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tungsten_smaa_blend"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SMAA_BLEND_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_present_source(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    // Same usage shape as `create_resolved_color`: RENDER_ATTACHMENT for the
    // SMAA neighborhood pass, TEXTURE_BINDING for present-blit + text overlay
    // sampling, COPY_SRC so the screenshot path can read it back.
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tungsten_present_source"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn make_linear_view(
    tex: &wgpu::Texture,
    twin: Option<TextureFormat>,
    label: &'static str,
) -> wgpu::TextureView {
    match twin {
        Some(linear_format) => tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            format: Some(linear_format),
            ..Default::default()
        }),
        None => tex.create_view(&wgpu::TextureViewDescriptor::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_srgb_twin_maps_rgba8_unorm_srgb() {
        assert_eq!(
            non_srgb_twin(TextureFormat::Rgba8UnormSrgb),
            Some(TextureFormat::Rgba8Unorm)
        );
    }

    #[test]
    fn non_srgb_twin_maps_bgra8_unorm_srgb() {
        assert_eq!(
            non_srgb_twin(TextureFormat::Bgra8UnormSrgb),
            Some(TextureFormat::Bgra8Unorm)
        );
    }

    #[test]
    fn non_srgb_twin_returns_none_for_linear_input() {
        assert_eq!(non_srgb_twin(TextureFormat::Rgba8Unorm), None);
        assert_eq!(non_srgb_twin(TextureFormat::Bgra8Unorm), None);
    }
}
