//! M25 offscreen scene targets: color + optional depth + optional MSAA.
//!
//! `SceneColor` always matches the swapchain sRGB format so the default path
//! can blit back onto the swapchain byte-identically to the 0.21 baseline.
//! `SceneDepth` uses `Depth32Float` (portable) and is allocated only when
//! `RenderConfig::depth_enabled` is true. `SceneColorMsaa` is allocated only
//! when `sample_count > 1` and resolves into `SceneColor`.

use wgpu::{Extent3d, TextureFormat, TextureUsages};

/// Named render-target slots referenced by `PassDesc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetId {
    SceneColor,
    SceneDepth,
    SceneColorMsaa,
    Swapchain,
}

/// Allocated textures for the scene target pool.
#[derive(Debug)]
#[must_use]
pub struct SceneTarget {
    pub color: (wgpu::Texture, wgpu::TextureView),
    pub depth: Option<(wgpu::Texture, wgpu::TextureView)>,
    pub color_msaa: Option<(wgpu::Texture, wgpu::TextureView)>,
    pub size: (u32, u32),
    pub sample_count: u32,
    pub format: TextureFormat,
    pub depth_enabled: bool,
}

/// Depth format for `SceneDepth`; portable across wgpu backends.
pub const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

impl SceneTarget {
    pub fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        format: TextureFormat,
        sample_count: u32,
        depth_enabled: bool,
    ) -> Self {
        let (w, h) = (size.0.max(1), size.1.max(1));
        // Resolve target + screenshot source: needs COPY_SRC + TEXTURE_BINDING
        // for the present-blit sampling path. sample_count is always 1 here —
        // MSAA lives in a sibling `SceneColorMsaa` that resolves into this one.
        let color = create_resolved_color(device, "tungsten_scene_color", w, h, format);
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

        Self {
            color,
            depth,
            color_msaa,
            size: (w, h),
            sample_count,
            format,
            depth_enabled,
        }
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
    ) -> Self {
        Self {
            scene: SceneTarget::new(device, size, format, sample_count, depth_enabled),
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
    ) {
        let new_size = (size.0.max(1), size.1.max(1));
        let shape_changed = self.scene.size != new_size
            || self.scene.format != format
            || self.scene.sample_count != sample_count
            || self.scene.depth_enabled != depth_enabled;
        if !shape_changed {
            return;
        }
        self.scene = SceneTarget::new(device, new_size, format, sample_count, depth_enabled);
    }
}

fn create_resolved_color(
    device: &wgpu::Device,
    label: &'static str,
    width: u32,
    height: u32,
    format: TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
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
        view_formats: &[],
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
