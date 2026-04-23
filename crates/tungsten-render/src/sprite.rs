use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use tungsten_core::assets::{FilterMode, TextureHandle};
use wgpu::util::DeviceExt;

/// GPU sprite instance; 40-byte POD layout.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpriteInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [u8; 4],
    pub uv_min: [f32; 2],
    pub uv_size: [f32; 2],
}

impl SpriteInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32,
        5 => Unorm8x4,
        6 => Float32x2,
        7 => Float32x2,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    /// Full-texture instance for non-atlas paths.
    pub fn whole(position: [f32; 2], size: [f32; 2], rotation: f32, color: [u8; 4]) -> Self {
        Self {
            position,
            size,
            rotation,
            color,
            uv_min: [0.0, 0.0],
            uv_size: [1.0, 1.0],
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

/// Sprite batch sharing one texture handle.
pub struct SpriteBatch {
    pub texture: TextureHandle,
    pub filter: FilterMode,
    pub instances: Vec<SpriteInstance>,
}

/// Textured sprite pipeline, samplers, and texture pool.
pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_upload: Vec<SpriteInstance>,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    textures: HashMap<TextureHandle, GpuTexture>,
    next_handle: u32,
}

impl SpritePipeline {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite_pipeline_layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[SpriteVertex::desc(), SpriteInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

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
            vertex_buffer,
            instance_buffer,
            instance_capacity,
            instance_upload: Vec::new(),
            camera_buffer,
            camera_bind_group,
            texture_bind_group_layout,
            sampler_nearest,
            sampler_linear,
            textures: HashMap::new(),
            next_handle: 0,
        }
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

    /// Draw sprite batches.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        batches: &[SpriteBatch],
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

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        let instance_stride = std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress;
        let mut base_instance = 0usize;
        for batch in batches {
            if batch.instances.is_empty() {
                continue;
            }

            let start = (base_instance as wgpu::BufferAddress) * instance_stride;
            let end = start + (batch.instances.len() as wgpu::BufferAddress) * instance_stride;
            // Advance before possible skip; upload slices stay aligned.
            base_instance += batch.instances.len();

            let gpu_tex = match self.textures.get(&batch.texture) {
                Some(t) => t,
                None => {
                    log::warn!("Missing GPU texture for handle {:?}", batch.texture);
                    continue;
                }
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
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(start..end));
            render_pass.draw(0..6, 0..batch.instances.len() as u32);
        }
    }
}

#[cfg(test)]
#[path = "tests/sprite.rs"]
mod tests;
