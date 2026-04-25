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

/// M28 bloom pyramid format. `Rgba16Float` avoids 8-bit sRGB quantization
/// during downsample/upsample even though the source scene stays LDR.
pub const BLOOM_PYRAMID_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// Allocated bloom mip-chain: one 2D texture with N mip levels plus per-level
/// views. Mip 0 is half resolution, each successive mip halves again.
#[derive(Debug)]
pub struct BloomPyramid {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    mip_views: Vec<wgpu::TextureView>,
    mip_extents: Vec<(u32, u32)>,
}

impl BloomPyramid {
    #[must_use]
    pub fn mip_count(&self) -> u32 {
        self.mip_extents.len() as u32
    }

    #[must_use]
    pub fn mip_view(&self, level: u32) -> Option<&wgpu::TextureView> {
        self.mip_views.get(level as usize)
    }

    #[must_use]
    pub fn mip_extent(&self, level: u32) -> Option<(u32, u32)> {
        self.mip_extents.get(level as usize).copied()
    }
}

/// Compute the mip count for a viewport-sized bloom pyramid. Mip 0 sits at
/// half resolution, so the natural ceiling is `floor(log2(min(w, h))) - 1`.
/// Returns at least 1 so the pyramid texture always has a mip 0.
#[must_use]
pub fn bloom_mip_count_for_size(width: u32, height: u32, max_mips: u32) -> u32 {
    let smallest = width.min(height).max(1);
    let log2 = u32::BITS - smallest.leading_zeros() - 1;
    let size_limit = log2.saturating_sub(1).max(1);
    max_mips.max(1).min(size_limit)
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
    /// M28 bloom pyramid. Always allocated; bloom_max_mips is config-validated
    /// to `1..=8` so an Option is unnecessary. Slots that do not include
    /// `PostPass::Bloom` simply never sample or write into it.
    pub bloom_pyramid: BloomPyramid,
    pub size: (u32, u32),
    pub sample_count: u32,
    pub format: TextureFormat,
    pub depth_enabled: bool,
    pub post_aa: PostAaMode,
    pub bloom_max_mips: u32,
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
        bloom_max_mips: u32,
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

        let bloom_pyramid = create_bloom_pyramid(device, w, h, bloom_max_mips);

        Self {
            color,
            depth,
            color_msaa,
            post_ping,
            post_pong,
            smaa,
            bloom_pyramid,
            size: (w, h),
            sample_count,
            format,
            depth_enabled,
            post_aa,
            bloom_max_mips,
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

    #[must_use]
    pub fn bloom_mip_view(&self, level: u32) -> Option<&wgpu::TextureView> {
        self.bloom_pyramid.mip_view(level)
    }

    #[must_use]
    pub fn bloom_mip_count(&self) -> u32 {
        self.bloom_pyramid.mip_count()
    }

    #[must_use]
    pub fn bloom_mip_extent(&self, level: u32) -> Option<(u32, u32)> {
        self.bloom_pyramid.mip_extent(level)
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
        bloom_max_mips: u32,
    ) -> Self {
        Self {
            scene: SceneTarget::new(
                device,
                size,
                format,
                sample_count,
                depth_enabled,
                post_aa,
                bloom_max_mips,
            ),
        }
    }

    /// Re-allocate when size or config changes. No-op when already matching.
    #[allow(clippy::too_many_arguments)]
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
        sample_count: u32,
        depth_enabled: bool,
        post_aa: PostAaMode,
        bloom_max_mips: u32,
    ) {
        let new_size = (size.0.max(1), size.1.max(1));
        let shape_changed = self.scene.size != new_size
            || self.scene.format != format
            || self.scene.sample_count != sample_count
            || self.scene.depth_enabled != depth_enabled
            || self.scene.post_aa != post_aa
            || self.scene.bloom_max_mips != bloom_max_mips;
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
            bloom_max_mips,
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

fn create_bloom_pyramid(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    max_mips: u32,
) -> BloomPyramid {
    // Mip 0 sits at half resolution (the COD/Frostbite "downsample by 2 then
    // start the chain" convention). Pyramid extents shrink by 2 per level.
    let half_w = (width / 2).max(1);
    let half_h = (height / 2).max(1);
    let mip_count = bloom_mip_count_for_size(width, height, max_mips);

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tungsten_bloom_pyramid"),
        size: Extent3d {
            width: half_w,
            height: half_h,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BLOOM_PYRAMID_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let mut mip_views = Vec::with_capacity(mip_count as usize);
    let mut mip_extents = Vec::with_capacity(mip_count as usize);
    for level in 0..mip_count {
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("tungsten_bloom_mip_view"),
            base_mip_level: level,
            mip_level_count: Some(1),
            ..Default::default()
        });
        mip_views.push(view);
        let w = (half_w >> level).max(1);
        let h = (half_h >> level).max(1);
        mip_extents.push((w, h));
    }

    BloomPyramid {
        texture,
        mip_views,
        mip_extents,
    }
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

    #[test]
    fn bloom_mip_count_clamps_to_viewport_size() {
        // 1080p tall enough for the requested 6 mips at half-res start.
        assert_eq!(bloom_mip_count_for_size(1920, 1080, 6), 6);
        // 64x64 viewport: floor(log2(64)) = 6, minus 1 = 5 mip ceiling.
        assert_eq!(bloom_mip_count_for_size(64, 64, 6), 5);
        // Tiny viewport: floor must not underflow.
        assert_eq!(bloom_mip_count_for_size(2, 2, 6), 1);
        assert_eq!(bloom_mip_count_for_size(1, 1, 6), 1);
        // max_mips = 0 still yields at least 1.
        assert_eq!(bloom_mip_count_for_size(1024, 1024, 0), 1);
        // Larger pyramids respect the viewport ceiling.
        assert_eq!(bloom_mip_count_for_size(1024, 1024, 8), 8);
    }
}
