use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use tungsten_core::assets::{FilterMode, MaterialAssetId, TextureHandle};
use tungsten_core::tween::UniformOverrideBlock;
use wgpu::util::DeviceExt;

use crate::material::MaterialPipeline;

/// GPU sprite instance; POD layout.
///
/// `z_norm` is the [0, 1] NDC-space depth derived from stable CPU painter
/// order. It is ignored by the pipeline unless `DepthSortMode::GpuDepth` is
/// active, in which case it drives the depth buffer; under `CpuStable` it
/// stays a benign per-instance pass-through.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[allow(clippy::pub_underscore_fields)]
pub struct SpriteInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [u8; 4],
    pub uv_min: [f32; 2],
    pub uv_size: [f32; 2],
    pub z_norm: f32,
    pub _pad: f32,
}

impl SpriteInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32,
        5 => Unorm8x4,
        6 => Float32x2,
        7 => Float32x2,
        8 => Float32,
    ];

    #[must_use]
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    /// Full-texture instance for non-atlas paths.
    #[must_use]
    pub fn whole(position: [f32; 2], size: [f32; 2], rotation: f32, color: [u8; 4]) -> Self {
        Self {
            position,
            size,
            rotation,
            color,
            uv_min: [0.0, 0.0],
            uv_size: [1.0, 1.0],
            z_norm: 0.0,
            _pad: 0.0,
        }
    }
}

impl Default for SpriteInstance {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            rotation: 0.0,
            color: [255; 4],
            uv_min: [0.0, 0.0],
            uv_size: [1.0, 1.0],
            z_norm: 0.0,
            _pad: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SpriteVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl SpriteVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const SPRITE_VERTICES: &[SpriteVertex] = &[
    SpriteVertex {
        position: [0.0, 0.0],
        uv: [0.0, 0.0],
    },
    SpriteVertex {
        position: [1.0, 0.0],
        uv: [1.0, 0.0],
    },
    SpriteVertex {
        position: [1.0, 1.0],
        uv: [1.0, 1.0],
    },
    SpriteVertex {
        position: [0.0, 0.0],
        uv: [0.0, 0.0],
    },
    SpriteVertex {
        position: [1.0, 1.0],
        uv: [1.0, 1.0],
    },
    SpriteVertex {
        position: [0.0, 1.0],
        uv: [0.0, 1.0],
    },
];

/// Texture pool entry; sampler filter baked into bind group.
#[allow(dead_code)]
struct GpuTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    filter: FilterMode,
}

/// M29 lit-texture pool entry: parallel albedo / normal / emissive views all
/// keyed by the same atlas page handle, sharing one filter and one sampler.
#[allow(dead_code)]
struct GpuLitTextures {
    albedo: wgpu::Texture,
    normal: wgpu::Texture,
    emissive: wgpu::Texture,
    albedo_view: wgpu::TextureView,
    normal_view: wgpu::TextureView,
    emissive_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    filter: FilterMode,
}

/// Sprite batch sharing one texture handle.
///
/// M26: adding `material_id` + `uniform_overrides` stays additive — leaving
/// both at their defaults preserves the M25 byte-identical sprite draw path.
/// The extract layer must split batches on effective material state so the
/// renderer can upload per-batch UBOs without re-reading world data.
pub struct SpriteBatch {
    pub texture: TextureHandle,
    pub filter: FilterMode,
    pub instances: Vec<SpriteInstance>,
    /// When set, the batch renders through the matching `MaterialPipeline`
    /// on `Renderer::materials`. `None` keeps the built-in sprite pipeline.
    pub material_id: Option<MaterialAssetId>,
    /// Per-entity animation override payload. `None` means the batch uses
    /// the material's authored defaults. Ignored when `material_id` is `None`.
    pub uniform_overrides: Option<UniformOverrideBlock>,
    /// M29 lit-pipeline opt-in. When `true`, the renderer binds the lit
    /// texture bundle (group 1) and `LightingResources` (group 2) for this
    /// batch and runs the `LitSpritePipeline` instead of the built-in /
    /// material pipelines. `lit` wins over `material_id` (lit + material is
    /// out-of-scope in M29; see plan non-goals).
    pub lit: bool,
}

impl SpriteBatch {
    /// Built-in pipeline batch; material slot defaults to `None`.
    #[must_use]
    pub fn new(texture: TextureHandle, filter: FilterMode) -> Self {
        Self {
            texture,
            filter,
            instances: Vec::new(),
            material_id: None,
            uniform_overrides: None,
            lit: false,
        }
    }
}

/// Textured sprite pipeline, samplers, and texture pool.
pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    pipeline_layout: wgpu::PipelineLayout,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_upload: Vec<SpriteInstance>,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    /// M29 lit bind-group layout used by `LitSpritePipeline` and the lit
    /// texture pool. Three texture views + one sampler.
    lit_texture_bind_group_layout: wgpu::BindGroupLayout,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    textures: HashMap<TextureHandle, GpuTexture>,
    /// M29 parallel pool: keyed by the albedo page handle, holds three views
    /// + one bind group. Lit batches bind from this pool at group 1.
    lit_textures: HashMap<TextureHandle, GpuLitTextures>,
    next_handle: u32,
}

impl SpritePipeline {
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../assets/shaders/sprite.wgsl").into(),
            ),
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_camera_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sprite_camera_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_camera_bg"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sprite_texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let lit_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sprite_lit_texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite_pipeline_layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = build_sprite_pipeline(
            device,
            &shader,
            &pipeline_layout,
            surface_format,
            sample_count,
            depth_write,
        );

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sprite_vertex_buffer"),
            contents: bytemuck::cast_slice(SPRITE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instance_capacity = 1;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_instance_buffer"),
            size: (instance_capacity * std::mem::size_of::<SpriteInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler_nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sampler_linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            pipeline_layout,
            vertex_buffer,
            instance_buffer,
            instance_capacity,
            instance_upload: Vec::new(),
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            texture_bind_group_layout,
            lit_texture_bind_group_layout,
            sampler_nearest,
            sampler_linear,
            textures: HashMap::new(),
            lit_textures: HashMap::new(),
            next_handle: 0,
        }
    }

    /// Camera-uniform bind-group layout used by the built-in sprite pipeline.
    /// Material pipelines reuse it so vertex/instance buffers stay interchangeable.
    #[must_use]
    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    /// Texture bind-group layout (group 1). Material pipelines reuse it.
    #[must_use]
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// M29 lit-texture bind-group layout (group 1) used by `LitSpritePipeline`.
    /// Three texture views (albedo, normal, emissive) + one filtering sampler.
    #[must_use]
    pub fn lit_texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.lit_texture_bind_group_layout
    }

    /// Camera bind group (group 0) the sprite draw binds every frame.
    #[must_use]
    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    /// Hot-reload entry point: swap the live pipeline to one built against
    /// `new_module`. Leaves texture bind groups, samplers, and buffers intact.
    /// `depth_write = true` produces the GpuDepth variant (LessEqual + write);
    /// `false` produces the CpuStable variant (no depth_stencil state).
    pub fn rebuild_with_shader(
        &mut self,
        device: &wgpu::Device,
        new_module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) {
        self.pipeline = build_sprite_pipeline(
            device,
            new_module,
            &self.pipeline_layout,
            surface_format,
            sample_count,
            depth_write,
        );
    }

    /// Mint renderer-owned texture handle.
    pub fn allocate_texture_handle(&mut self) -> TextureHandle {
        let handle = TextureHandle(self.next_handle);
        self.next_handle += 1;
        handle
    }

    /// Remove texture and bind group from pool.
    pub fn drop_texture(&mut self, handle: TextureHandle) {
        self.textures.remove(&handle);
    }

    /// Upload RGBA texture; sampler filter baked into bind group.
    ///
    /// # Panics
    /// Panics on zero dimensions or RGBA length mismatch.
    #[allow(clippy::too_many_arguments)] // stable M22 surface; see D-048
    pub fn upload_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: TextureHandle,
        rgba_data: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    ) {
        assert!(
            width > 0 && height > 0,
            "texture dimensions must be non-zero"
        );
        let expected = (width as usize) * (height as usize) * 4;
        assert_eq!(
            rgba_data.len(),
            expected,
            "rgba_data length ({}) does not match {}x{}x4 ({})",
            rgba_data.len(),
            width,
            height,
            expected,
        );

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("texture_{}", handle.0)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = match filter {
            FilterMode::Nearest => &self.sampler_nearest,
            FilterMode::Linear => &self.sampler_linear,
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("sprite_bg_{}", handle.0)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        self.textures.insert(
            handle,
            GpuTexture {
                texture,
                view,
                bind_group,
                filter,
            },
        );
    }

    /// Copy RGBA rectangle into existing atlas page.
    ///
    /// # Panics
    /// Panics on missing handle, escaped rect, or RGBA length mismatch.
    #[allow(clippy::too_many_arguments)] // stable M22 surface; see D-048
    pub fn write_subtexture(
        &self,
        queue: &wgpu::Queue,
        handle: TextureHandle,
        rgba: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        let gpu_tex = self
            .textures
            .get(&handle)
            .expect("write_subtexture: handle not in pool");
        assert_eq!(
            rgba.len(),
            (width as usize) * (height as usize) * 4,
            "write_subtexture: rgba length mismatch"
        );
        let tex_size = gpu_tex.texture.size();
        assert!(
            x + width <= tex_size.width && y + height <= tex_size.height,
            "write_subtexture: rect ({x},{y},{width},{height}) escapes texture {}x{}",
            tex_size.width,
            tex_size.height
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu_tex.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// M29 upload albedo + normal + emissive RGBA pages under one
    /// `TextureHandle`. The lit texture pool reuses the same handle as the
    /// regular `upload_texture` call so atlas-shared sprites keep one packed
    /// rect and the renderer can pick the lit pipeline by batch flag.
    ///
    /// Albedo is `Rgba8UnormSrgb`, normal/emissive are `Rgba8Unorm` (linear)
    /// so normal vectors and mask intensities are not gamma-decoded.
    ///
    /// # Panics
    /// Panics on zero dimensions or RGBA length mismatch.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_lit_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: TextureHandle,
        albedo_rgba: &[u8],
        normal_rgba: &[u8],
        emissive_rgba: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    ) {
        assert!(
            width > 0 && height > 0,
            "lit texture dimensions must be non-zero"
        );
        let expected = (width as usize) * (height as usize) * 4;
        assert_eq!(albedo_rgba.len(), expected, "albedo length mismatch");
        assert_eq!(normal_rgba.len(), expected, "normal length mismatch");
        assert_eq!(emissive_rgba.len(), expected, "emissive length mismatch");

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let albedo = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("lit_albedo_{}", handle.0)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let normal = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("lit_normal_{}", handle.0)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let emissive = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("lit_emissive_{}", handle.0)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (tex, src) in [
            (&albedo, albedo_rgba),
            (&normal, normal_rgba),
            (&emissive, emissive_rgba),
        ] {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                src,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                size,
            );
        }
        let albedo_view = albedo.create_view(&wgpu::TextureViewDescriptor::default());
        let normal_view = normal.create_view(&wgpu::TextureViewDescriptor::default());
        let emissive_view = emissive.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = match filter {
            FilterMode::Nearest => &self.sampler_nearest,
            FilterMode::Linear => &self.sampler_linear,
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("lit_bg_{}", handle.0)),
            layout: &self.lit_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&emissive_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        self.lit_textures.insert(
            handle,
            GpuLitTextures {
                albedo,
                normal,
                emissive,
                albedo_view,
                normal_view,
                emissive_view,
                bind_group,
                filter,
            },
        );
    }

    /// M29 copy a packed cell into the lit texture bundle (albedo + normal +
    /// emissive). Mirrors `write_subtexture` for hot-reload + in-place
    /// updates of a single sprite cell on a shared atlas page.
    ///
    /// # Panics
    /// Panics on missing handle, escaped rect, or length mismatch.
    #[allow(clippy::too_many_arguments)]
    pub fn write_subtexture_lit(
        &self,
        queue: &wgpu::Queue,
        handle: TextureHandle,
        albedo_rgba: &[u8],
        normal_rgba: &[u8],
        emissive_rgba: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        let lit = self
            .lit_textures
            .get(&handle)
            .expect("write_subtexture_lit: handle not in lit pool");
        let expected = (width as usize) * (height as usize) * 4;
        assert_eq!(albedo_rgba.len(), expected, "albedo length mismatch");
        assert_eq!(normal_rgba.len(), expected, "normal length mismatch");
        assert_eq!(emissive_rgba.len(), expected, "emissive length mismatch");
        let tex_size = lit.albedo.size();
        assert!(
            x + width <= tex_size.width && y + height <= tex_size.height,
            "write_subtexture_lit: rect ({x},{y},{width},{height}) escapes texture {}x{}",
            tex_size.width,
            tex_size.height
        );
        for (tex, src) in [
            (&lit.albedo, albedo_rgba),
            (&lit.normal, normal_rgba),
            (&lit.emissive, emissive_rgba),
        ] {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                src,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Drop the matching lit-texture bundle for `handle`. No-op when the pool
    /// has no entry; called by `Renderer::drop_texture` to keep the pools in
    /// lockstep.
    pub fn drop_lit_texture(&mut self, handle: TextureHandle) {
        self.lit_textures.remove(&handle);
    }

    /// Upload caller-owned camera view-projection matrix.
    pub fn update_camera(&self, queue: &wgpu::Queue, view_proj: &glam::Mat4) {
        let matrix_ref: &[f32; 16] = view_proj.as_ref();
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(matrix_ref));
    }

    fn ensure_instance_capacity(&mut self, device: &wgpu::Device, required_instances: usize) {
        if required_instances <= self.instance_capacity {
            return;
        }

        self.instance_capacity = required_instances.next_power_of_two().max(1);
        self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_instance_buffer"),
            size: (self.instance_capacity * std::mem::size_of::<SpriteInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    /// Draw sprite batches. `material_pipelines` is the renderer-owned
    /// registry of user-authored material pipelines; a batch with
    /// `material_id = Some(_)` re-binds the matching pipeline and its
    /// material UBO (group 2), otherwise the built-in sprite pipeline runs.
    ///
    /// M29: when `batch.lit` is set, the renderer pulls the lit texture
    /// bundle from `self.lit_textures` (group 1, three views + sampler) and
    /// uses `lit_sprite_pipeline` + `lighting_bind_group` (group 2). `lit`
    /// wins over `material_id` when both are present.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        batches: &[SpriteBatch],
        material_pipelines: &HashMap<MaterialAssetId, MaterialPipeline>,
        lit_sprite_pipeline: Option<&wgpu::RenderPipeline>,
        lighting_bind_group: Option<&wgpu::BindGroup>,
    ) {
        let total_instances: usize = batches.iter().map(|batch| batch.instances.len()).sum();
        if total_instances == 0 {
            return;
        }

        self.ensure_instance_capacity(device, total_instances);
        self.instance_upload.clear();
        self.instance_upload.extend(
            batches
                .iter()
                .flat_map(|batch| batch.instances.iter().copied()),
        );
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instance_upload),
        );

        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        // Pipeline-key tracking covers built-in / material / lit, in that
        // order of precedence. `None` = built-in sprite pipeline; lit batches
        // collapse onto a synthetic sentinel id so adjacent lit batches don't
        // rebind. Material precedence remains; lit wins over material.
        #[derive(PartialEq, Clone, Copy)]
        enum PipelineKey {
            BuiltIn,
            Material(MaterialAssetId),
            Lit,
        }
        let instance_stride = std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress;
        let mut base_instance = 0usize;
        let mut last_pipeline_key: Option<PipelineKey> = None;

        for batch in batches {
            if batch.instances.is_empty() {
                continue;
            }

            let start = (base_instance as wgpu::BufferAddress) * instance_stride;
            let end = start + (batch.instances.len() as wgpu::BufferAddress) * instance_stride;
            base_instance += batch.instances.len();

            // Decide pipeline up-front.
            let lit_resources = if batch.lit {
                match (
                    self.lit_textures.get(&batch.texture),
                    lit_sprite_pipeline,
                    lighting_bind_group,
                ) {
                    (Some(lit), Some(pipeline), Some(bg)) => Some((lit, pipeline, bg)),
                    _ => None,
                }
            } else {
                None
            };

            if batch.lit && lit_resources.is_none() {
                log::warn!(
                    "lit sprite batch missing lit-pool entry or lit pipeline for handle {:?}; skipping",
                    batch.texture
                );
                continue;
            }

            let material_pipeline = if !batch.lit {
                batch.material_id.and_then(|id| material_pipelines.get(&id))
            } else {
                None
            };

            let pipeline_key = if batch.lit {
                PipelineKey::Lit
            } else if let Some(_mp) = material_pipeline {
                PipelineKey::Material(batch.material_id.unwrap())
            } else {
                PipelineKey::BuiltIn
            };

            if last_pipeline_key != Some(pipeline_key) {
                match pipeline_key {
                    PipelineKey::BuiltIn => render_pass.set_pipeline(&self.pipeline),
                    PipelineKey::Material(_) => {
                        render_pass.set_pipeline(&material_pipeline.unwrap().pipeline);
                    }
                    PipelineKey::Lit => {
                        render_pass.set_pipeline(lit_resources.unwrap().1);
                    }
                }
                last_pipeline_key = Some(pipeline_key);
            }

            if let Some((lit, _pipeline, lighting_bg)) = lit_resources {
                if batch.filter != lit.filter {
                    log::warn!(
                        "lit sprite batch filter {:?} != lit pool filter {:?} for handle {:?}",
                        batch.filter,
                        lit.filter,
                        batch.texture
                    );
                    continue;
                }
                render_pass.set_bind_group(1, &lit.bind_group, &[]);
                render_pass.set_bind_group(2, lighting_bg, &[]);
            } else {
                let Some(gpu_tex) = self.textures.get(&batch.texture) else {
                    log::warn!("Missing GPU texture for handle {:?}", batch.texture);
                    continue;
                };
                if batch.filter != gpu_tex.filter {
                    log::warn!(
                        "sprite batch filter {:?} != pool filter {:?} for handle {:?}",
                        batch.filter,
                        gpu_tex.filter,
                        batch.texture
                    );
                    continue;
                }
                render_pass.set_bind_group(1, &gpu_tex.bind_group, &[]);
                if let Some(mp) = material_pipeline {
                    let payload = batch
                        .uniform_overrides
                        .unwrap_or_else(|| mp.defaults.to_override_block());
                    queue.write_buffer(&mp.ubo, 0, &payload.to_bytes());
                    render_pass.set_bind_group(2, &mp.bind_group, &[]);
                }
            }

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(start..end));
            render_pass.draw(0..6, 0..batch.instances.len() as u32);
        }
    }

    /// Vertex layout used by both the built-in sprite pipeline and any
    /// `MaterialPipeline` rendering sprites.
    #[must_use]
    pub fn vertex_layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
        [SpriteVertex::desc(), SpriteInstance::desc()]
    }
}

fn build_sprite_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    sample_count: u32,
    depth_write: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sprite_pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[SpriteVertex::desc(), SpriteInstance::desc()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: if depth_write {
            Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
        } else {
            None
        },
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
#[path = "tests/sprite.rs"]
mod tests;
