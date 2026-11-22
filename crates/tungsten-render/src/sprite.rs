use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use tungsten_core::assets::{FilterMode, MaterialAssetId, TextureHandle};
use tungsten_core::tween::UniformOverrideBlock;

use crate::counters::{self, Counts};
use crate::lit_sprite::LitSpritePipeline;
use crate::material::{MaterialPipeline, MaterialSlot};
use crate::post::ParamSlots;
use crate::shader_hot_reload::build_validated;

/// GPU sprite instance; POD layout.
///
/// `z_norm` is a pass-through in [0, 1], the clip range: no stock pipeline's
/// depth reads it. Under `DepthSortMode::GpuDepth` a batch's depth is its
/// place in the frame's list (redo 7.15), set by the viewport's depth range,
/// which maps every `z_norm` in the clip range onto it; under `CpuStable` no
/// depth is tested. A material's own vertex stage may read it.
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

    /// A flat-colour quad on the renderer's white texel (redo 7.9), the
    /// reserved sprite `engine_white`: `size` from its top-left `position`,
    /// turned about its centre by `rotation`.
    #[must_use]
    pub fn flat(
        position: [f32; 2],
        size: [f32; 2],
        rotation: f32,
        color: [u8; 4],
        z_norm: f32,
    ) -> Self {
        Self {
            z_norm,
            ..Self::whole(position, size, rotation, color)
        }
    }

    /// A line `width` thick from `from` to `to`, as a [`flat`](Self::flat)
    /// quad turned along it, centred on its midpoint and a `width` longer, so
    /// joined segments overlap at their ends.
    #[must_use]
    pub fn segment(from: [f32; 2], to: [f32; 2], width: f32, color: [u8; 4], z_norm: f32) -> Self {
        let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
        let size = [dx.hypot(dy) + width, width];
        let centre = [f32::midpoint(from[0], to[0]), f32::midpoint(from[1], to[1])];
        Self::flat(
            [centre[0] - size[0] * 0.5, centre[1] - size[1] * 0.5],
            size,
            dy.atan2(dx),
            color,
            z_norm,
        )
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
struct GpuTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    filter: FilterMode,
}

/// M29 lit-texture pool entry: parallel albedo / normal / emissive views all
/// keyed by the same atlas page handle, sharing one filter and one sampler.
struct GpuLitTextures {
    albedo: wgpu::Texture,
    normal: wgpu::Texture,
    emissive: wgpu::Texture,
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
    /// out of scope in M29, `D-061`).
    pub lit: bool,
    /// Every fragment the batch draws is clear or opaque (redo 7.15): each
    /// instance samples only texels of alpha 0 or 255 through a nearest
    /// sampler ([`SpriteAlpha::solid`](tungsten_core::SpriteAlpha::solid))
    /// and carries a colour alpha of 255. Under `DepthSortMode::GpuDepth`
    /// such a batch draws ahead of the frame's other batches and writes
    /// depth, so what an opaque texel of it covers is never shaded, and its
    /// clear texels are dropped; the frame's pixels are the painter's. A
    /// batch marked solid that draws a fragment between the two loses that
    /// fragment. Ignored under `CpuStable`, and on a batch whose material
    /// draws it.
    pub solid: bool,
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
            solid: false,
        }
    }
}

/// One batch's turn in the sprite draw: its index in the frame's list and
/// whether it draws as a solid batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Turn {
    pub(crate) batch: u32,
    pub(crate) solid: bool,
}

/// The order the sprite draw takes `batches` in (redo 7.15). Without the
/// depth test (`depth` false) the list's own, the painter's. With it, the
/// solid batches first, the last of them first, so a fragment an opaque
/// texel covers fails the test before it is shaded; then every other batch
/// in the list's order, tested against them and writing no depth. A batch
/// whose material draws it promises nothing of its fragments, so it is never
/// solid; a lit batch draws the lit shader whatever material it names.
pub(crate) fn draw_turns(batches: &[SpriteBatch], depth: bool, out: &mut Vec<Turn>) {
    let solid =
        |batch: &SpriteBatch| depth && batch.solid && (batch.lit || batch.material_id.is_none());
    out.clear();
    let turn = |(index, batch): (usize, &SpriteBatch)| Turn {
        batch: index as u32,
        solid: solid(batch),
    };
    out.extend(
        batches
            .iter()
            .enumerate()
            .rev()
            .map(turn)
            .filter(|t| t.solid),
    );
    out.extend(batches.iter().enumerate().map(turn).filter(|t| !t.solid));
}

/// The depth of batch `index` of `count` under the depth test: lower for a
/// later batch, so under `LessEqual` a later batch's fragment passes over an
/// earlier one's and an earlier one's fails where a later solid batch wrote.
/// Every value lies inside (0, 1); the scene's depth clears to 1.
pub(crate) fn batch_depth(index: usize, count: usize) -> f32 {
    1.0 - (index + 1) as f32 / (count + 1) as f32
}

/// Instances a growing instance buffer is sized for: `required` rounded up
/// to a power of two, at most what one buffer of `stride`-byte instances may
/// hold under `max_buffer_size` (wgpu's default limit is 256 MiB), and at
/// least 1. A frame with more draws the first that many.
pub(crate) fn instance_capacity(required: usize, stride: usize, max_buffer_size: u64) -> usize {
    let limit = usize::try_from(max_buffer_size / stride as u64).unwrap_or(usize::MAX);
    required
        .max(1)
        .checked_next_power_of_two()
        .unwrap_or(usize::MAX)
        .min(limit)
        .max(1)
}

/// Texture handles (`D-048`, redo 7.9): a counter rising from 0 for the
/// textures a run keeps, and a reserved range falling from `u32::MAX`, which
/// the counter never reaches, for textures dropped before the counter mints
/// the rest, such as the boot group's pages. Dropping those leaves every
/// counter handle as a run without them mints it.
#[derive(Debug, Clone)]
pub struct TextureHandles {
    next: u32,
    next_reserved: u32,
}

impl TextureHandles {
    /// The reserved range's lowest handle; the counter stops below it.
    pub const RESERVED_FLOOR: u32 = u32::MAX - 1023;

    /// A counter at 0 and the reserved range at `u32::MAX`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: 0,
            next_reserved: u32::MAX,
        }
    }

    /// The counter's next handle.
    ///
    /// # Panics
    ///
    /// When the counter reaches [`Self::RESERVED_FLOOR`].
    pub fn allocate(&mut self) -> TextureHandle {
        assert!(
            self.next < Self::RESERVED_FLOOR,
            "the texture handle counter reached the reserved range"
        );
        let handle = TextureHandle(self.next);
        self.next += 1;
        handle
    }

    /// The reserved range's next handle, falling from `u32::MAX`.
    ///
    /// # Panics
    ///
    /// When the 1,024 reserved handles are spent.
    pub fn allocate_reserved(&mut self) -> TextureHandle {
        assert!(
            self.next_reserved >= Self::RESERVED_FLOOR,
            "the reserved texture handles are spent"
        );
        let handle = TextureHandle(self.next_reserved);
        self.next_reserved = self.next_reserved.wrapping_sub(1);
        handle
    }
}

impl Default for TextureHandles {
    fn default() -> Self {
        Self::new()
    }
}

/// Textured sprite pipeline, samplers, and texture pool.
pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    /// Under `DepthSortMode::GpuDepth`, the pipeline a solid batch draws
    /// with: `fs_solid`, writing depth (redo 7.15).
    pipeline_solid: Option<wgpu::RenderPipeline>,
    /// The frame's turns and each batch's first instance, kept between
    /// frames.
    turns: Vec<Turn>,
    starts: Vec<u32>,
    pipeline_layout: wgpu::PipelineLayout,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
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
    /// Its counter mints every handle `allocate_texture_handle` gives.
    handles: TextureHandles,
    /// Each material's UBO slots, one per batch of the material in a frame,
    /// with the bytes each holds (`D-101`).
    material_slots: HashMap<MaterialAssetId, ParamSlots<MaterialSlot>>,
    /// Batches of each material drawn so far this frame.
    material_batches: HashMap<MaterialAssetId, usize>,
    /// Redo 7.9: the lit batches' group-3 slots, one per lit batch with a
    /// uniform block in a frame, as material batches keep theirs (`D-101`),
    /// and the zeroed slot every lit batch without one binds.
    lit_flash_slots: ParamSlots<MaterialSlot>,
    lit_no_flash: Option<MaterialSlot>,
    /// Whether a frame has outgrown the largest instance buffer, logged once.
    warned_instance_cap: bool,
    pub(crate) counts: Box<Counts>,
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
            depth_write.then_some(false),
        );
        let pipeline_solid = depth_write.then(|| {
            build_sprite_pipeline(
                device,
                &shader,
                &pipeline_layout,
                surface_format,
                sample_count,
                Some(true),
            )
        });

        let counts = Counts::boxed();
        let vertex_buffer = counters::create_buffer_init(
            &counts,
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("sprite_vertex_buffer"),
                contents: bytemuck::cast_slice(SPRITE_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
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
            pipeline_solid,
            turns: Vec::new(),
            starts: Vec::new(),
            pipeline_layout,
            vertex_buffer,
            instance_buffer,
            instance_capacity,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
            texture_bind_group_layout,
            lit_texture_bind_group_layout,
            sampler_nearest,
            sampler_linear,
            textures: HashMap::new(),
            lit_textures: HashMap::new(),
            handles: TextureHandles::new(),
            material_slots: HashMap::new(),
            material_batches: HashMap::new(),
            lit_flash_slots: ParamSlots::default(),
            lit_no_flash: None,
            warned_instance_cap: false,
            counts,
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

    /// The bind group of an uploaded page, for the UI pipeline's image
    /// batches (M46); `None` for a handle nothing uploaded.
    pub(crate) fn texture_bind_group(&self, handle: TextureHandle) -> Option<&wgpu::BindGroup> {
        self.textures
            .get(&handle)
            .map(|texture| &texture.bind_group)
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

    /// Hot-reload entry point: swap the live pipelines to ones built against
    /// `new_module`. Leaves texture bind groups, samplers, and buffers intact.
    /// `depth_write = true` produces the GpuDepth pair, the tested pipeline
    /// (`LessEqual`, no write) and the solid one that writes; `false` the
    /// CpuStable one (no depth_stencil state). A pipeline that fails
    /// validation is not swapped in: `Err` carries the report and the live
    /// pipelines stay.
    pub fn rebuild_with_shader(
        &mut self,
        device: &wgpu::Device,
        new_module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) -> Result<(), String> {
        let build = |write: Option<bool>| {
            build_validated(device, || {
                build_sprite_pipeline(
                    device,
                    new_module,
                    &self.pipeline_layout,
                    surface_format,
                    sample_count,
                    write,
                )
            })
        };
        let pipeline = build(depth_write.then_some(false))?;
        let pipeline_solid = if depth_write {
            Some(build(Some(true))?)
        } else {
            None
        };
        self.pipeline = pipeline;
        self.pipeline_solid = pipeline_solid;
        Ok(())
    }

    /// Mint renderer-owned texture handle.
    pub fn allocate_texture_handle(&mut self) -> TextureHandle {
        self.handles.allocate()
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

        counters::write_texture(
            &self.counts,
            queue,
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
        counters::write_texture(
            &self.counts,
            queue,
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
            counters::write_texture(
                &self.counts,
                queue,
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
            counters::write_texture(
                &self.counts,
                queue,
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
        counters::write_buffer(
            &self.counts,
            queue,
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(matrix_ref),
        );
    }

    /// Drops material `id`'s UBO slots: a new or rebuilt material pipeline has
    /// a new uniform layout, and `draw` builds and writes fresh slots on use.
    pub(crate) fn reset_material_slots(&mut self, id: MaterialAssetId) {
        self.material_slots.remove(&id);
    }

    fn ensure_instance_capacity(&mut self, device: &wgpu::Device, required_instances: usize) {
        let capacity = instance_capacity(
            required_instances,
            std::mem::size_of::<SpriteInstance>(),
            device.limits().max_buffer_size,
        );
        if capacity <= self.instance_capacity {
            return;
        }

        self.instance_capacity = capacity;
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
    /// wins over `material_id` when both are present. Redo 7.9: a lit batch
    /// binds its uniform block at group 3, the k-th lit batch with one slot
    /// k, and a lit batch without one a zeroed block, so it draws as before.
    ///
    /// Redo 7.15: under `DepthSortMode::GpuDepth` the batches draw in
    /// `draw_turns`' order, each at `batch_depth`, which the viewport's
    /// depth range sets over the pass's `target` size; the range is the
    /// whole one again when the draw returns.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        batches: &[SpriteBatch],
        material_pipelines: &HashMap<MaterialAssetId, MaterialPipeline>,
        lit_sprite_pipeline: Option<&LitSpritePipeline>,
        lighting_bind_group: Option<&wgpu::BindGroup>,
        target: (u32, u32),
    ) {
        let total_instances: usize = batches.iter().map(|batch| batch.instances.len()).sum();
        if total_instances == 0 {
            return;
        }

        self.ensure_instance_capacity(device, total_instances);
        // Past the largest buffer wgpu allows, the frame draws what fits.
        let drawn = total_instances.min(self.instance_capacity);
        if drawn < total_instances && !self.warned_instance_cap {
            self.warned_instance_cap = true;
            log::warn!(
                "{total_instances} sprite instances outgrow one instance buffer; drawing the first {drawn}"
            );
        }
        self.material_batches.clear();
        let mut lit_flash_batches = 0;
        let instance_stride = std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress;
        // Each batch lands in the staging view as one slice copy, in batch
        // order, so draw ranges below index it the same way. No intermediate
        // flattened Vec: element-wise flattening was 21% of samples at 100k.
        let Some(upload_size) = wgpu::BufferSize::new(drawn as u64 * instance_stride) else {
            return;
        };
        let Some(mut upload) =
            counters::write_buffer_with(&self.counts, queue, &self.instance_buffer, 0, upload_size)
        else {
            log::error!("sprite instance upload of {drawn} instances was rejected");
            return;
        };
        let mut offset = 0usize;
        let mut left = drawn;
        for batch in batches {
            let take = batch.instances.len().min(left);
            let bytes: &[u8] = bytemuck::cast_slice(&batch.instances[..take]);
            upload
                .slice(offset..offset + bytes.len())
                .copy_from_slice(bytes);
            offset += bytes.len();
            left -= take;
        }
        drop(upload);

        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        // Pipeline-key tracking covers built-in / material / lit, in that
        // order of precedence, each with its solid variant; adjacent batches
        // of one key don't rebind. Material precedence remains; lit wins
        // over material.
        #[derive(PartialEq, Clone, Copy)]
        enum PipelineKey {
            BuiltIn { solid: bool },
            Material(MaterialAssetId),
            Lit { solid: bool },
        }
        let mut last_pipeline_key: Option<PipelineKey> = None;

        // Each batch's first instance in the upload, and the frame's turns.
        let depth = self.pipeline_solid.is_some();
        let mut starts = std::mem::take(&mut self.starts);
        let mut turns = std::mem::take(&mut self.turns);
        starts.clear();
        let mut first = 0usize;
        for batch in batches {
            starts.push(first.min(drawn) as u32);
            first += batch.instances.len();
        }
        draw_turns(batches, depth, &mut turns);
        let (width, height) = (target.0 as f32, target.1 as f32);

        for turn in &turns {
            let index = turn.batch as usize;
            let batch = &batches[index];
            let base_instance = starts[index] as usize;
            let count = batch.instances.len().min(drawn - base_instance);
            if count == 0 {
                continue;
            }
            let lit = batch.lit;

            let start = (base_instance as wgpu::BufferAddress) * instance_stride;
            let end = start + (count as wgpu::BufferAddress) * instance_stride;

            // Decide pipeline up-front.
            let lit_resources = if lit {
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

            if lit && lit_resources.is_none() {
                log::warn!(
                    "lit sprite batch missing lit-pool entry or lit pipeline for handle {:?}; skipping",
                    batch.texture
                );
                continue;
            }

            let material_pipeline = if lit {
                None
            } else {
                batch.material_id.and_then(|id| material_pipelines.get(&id))
            };

            let pipeline_key = if lit {
                PipelineKey::Lit { solid: turn.solid }
            } else if let Some(mp) = material_pipeline {
                PipelineKey::Material(mp.material_id)
            } else {
                PipelineKey::BuiltIn { solid: turn.solid }
            };

            if last_pipeline_key != Some(pipeline_key) {
                // A solid turn comes only under the depth test, which
                // built the solid pipelines.
                match pipeline_key {
                    PipelineKey::BuiltIn { solid: false } => {
                        render_pass.set_pipeline(&self.pipeline);
                    }
                    PipelineKey::BuiltIn { solid: true } => {
                        if let Some(pipeline) = &self.pipeline_solid {
                            render_pass.set_pipeline(pipeline);
                        }
                    }
                    PipelineKey::Material(_) => {
                        render_pass.set_pipeline(&material_pipeline.unwrap().pipeline);
                    }
                    PipelineKey::Lit { solid: false } => {
                        render_pass.set_pipeline(&lit_resources.unwrap().1.pipeline);
                    }
                    PipelineKey::Lit { solid: true } => {
                        if let Some(pipeline) = &lit_resources.unwrap().1.pipeline_solid {
                            render_pass.set_pipeline(pipeline);
                        }
                    }
                }
                last_pipeline_key = Some(pipeline_key);
            }

            if let Some((lit, pipeline, lighting_bg)) = lit_resources {
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
                let layout = &pipeline.flash_bind_group_layout;
                let flash = if let Some(block) = &batch.uniform_overrides {
                    let payload = block.to_bytes();
                    let (slot, stale) =
                        self.lit_flash_slots.stage(lit_flash_batches, &payload, || {
                            MaterialSlot::new(device, layout, "lit_flash")
                        });
                    lit_flash_batches += 1;
                    if stale {
                        counters::write_buffer(&self.counts, queue, &slot.ubo, 0, &payload);
                    }
                    slot
                } else {
                    self.lit_no_flash.get_or_insert_with(|| {
                        // A new buffer reads as zeros: no flash.
                        MaterialSlot::new(device, layout, "lit_no_flash")
                    })
                };
                render_pass.set_bind_group(3, &flash.bind_group, &[]);
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
                        .unwrap_or_else(|| mp.defaults.to_override_block())
                        .to_bytes();
                    // The k-th batch of a material this frame draws with
                    // slot k: every write lands before the frame runs, so
                    // batches sharing one buffer all drew the last payload
                    // (`D-101`). A slot is written only when its bytes change.
                    let count = self.material_batches.entry(mp.material_id).or_insert(0);
                    let slot_index = *count;
                    *count += 1;
                    let (slot, stale) = self
                        .material_slots
                        .entry(mp.material_id)
                        .or_default()
                        .stage(slot_index, &payload, || {
                            MaterialSlot::new(device, &mp.bind_group_layout, &mp.name)
                        });
                    if stale {
                        counters::write_buffer(&self.counts, queue, &slot.ubo, 0, &payload);
                    }
                    render_pass.set_bind_group(2, &slot.bind_group, &[]);
                }
            }

            if depth {
                let z = batch_depth(index, batches.len());
                render_pass.set_viewport(0.0, 0.0, width, height, z, z);
            }
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(start..end));
            counters::draw(&self.counts, render_pass, 0..6, 0..count as u32);
        }
        if depth {
            render_pass.set_viewport(0.0, 0.0, width, height, 0.0, 1.0);
        }
        self.starts = starts;
        self.turns = turns;
    }

    /// Vertex layout used by both the built-in sprite pipeline and any
    /// `MaterialPipeline` rendering sprites.
    #[must_use]
    pub fn vertex_layouts() -> [Option<wgpu::VertexBufferLayout<'static>>; 2] {
        [Some(SpriteVertex::desc()), Some(SpriteInstance::desc())]
    }
}

fn build_sprite_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    sample_count: u32,
    depth: Option<bool>,
) -> wgpu::RenderPipeline {
    // `depth`: no depth state, or the test with (`Some(true)`, a solid
    // batch's `fs_solid`) or without a write.
    let solid = depth == Some(true);
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(if solid {
            "sprite_solid_pipeline"
        } else {
            "sprite_pipeline"
        }),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(SpriteVertex::desc()), Some(SpriteInstance::desc())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(if solid { "fs_solid" } else { "fs_main" }),
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
        depth_stencil: depth.map(|write| wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(write),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
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
