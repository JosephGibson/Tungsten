use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use tungsten_core::assets::{FilterMode, TextureHandle};
use wgpu::util::DeviceExt;

/// Per-instance sprite data sent to the GPU.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpriteInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
}

impl SpriteInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        2 => Float32x2,
        3 => Float32x2,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
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

/// A GPU texture entry in the texture pool.
#[allow(dead_code)]
struct GpuTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group_nearest: wgpu::BindGroup,
    bind_group_linear: wgpu::BindGroup,
}

/// A batch of sprites sharing the same texture handle.
pub struct SpriteBatch {
    pub texture: TextureHandle,
    pub filter: FilterMode,
    pub instances: Vec<SpriteInstance>,
}

/// Manages the textured-sprite pipeline, samplers, and texture pool.
pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    textures: HashMap<TextureHandle, GpuTexture>,
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
            camera_buffer,
            camera_bind_group,
            texture_bind_group_layout,
            sampler_nearest,
            sampler_linear,
            textures: HashMap::new(),
        }
    }

    /// Upload a decoded RGBA image to the GPU and store it in the texture pool.
    ///
    /// # Panics
    /// Panics if `width` or `height` is zero, or if `rgba_data` length does
    /// not match `width * height * 4`.
    pub fn upload_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: TextureHandle,
        rgba_data: &[u8],
        width: u32,
        height: u32,
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

        let bind_group_nearest = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("sprite_bg_nearest_{}", handle.0)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_nearest),
                },
            ],
        });

        let bind_group_linear = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("sprite_bg_linear_{}", handle.0)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                },
            ],
        });

        self.textures.insert(
            handle,
            GpuTexture {
                texture,
                view,
                bind_group_nearest,
                bind_group_linear,
            },
        );
    }

    /// Upload the camera view-projection matrix. Caller owns matrix
    /// construction (typically `Camera2D::view_projection(w, h)`) so
    /// tilemaps, sprites, and quads all share a single source of truth.
    pub fn update_camera(&self, queue: &wgpu::Queue, view_proj: &glam::Mat4) {
        let matrix_ref: &[f32; 16] = view_proj.as_ref();
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(matrix_ref));
    }

    /// Draw sprite batches. Each batch is a set of instances sharing a texture.
    pub fn draw(
        &self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'_>,
        batches: &[SpriteBatch],
    ) {
        if batches.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        for batch in batches {
            if batch.instances.is_empty() {
                continue;
            }

            let gpu_tex = match self.textures.get(&batch.texture) {
                Some(t) => t,
                None => {
                    log::warn!("Missing GPU texture for handle {:?}", batch.texture);
                    continue;
                }
            };

            let bind_group = match batch.filter {
                FilterMode::Nearest => &gpu_tex.bind_group_nearest,
                FilterMode::Linear => &gpu_tex.bind_group_linear,
            };

            let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite_instance_buffer"),
                contents: bytemuck::cast_slice(&batch.instances),
                usage: wgpu::BufferUsages::VERTEX,
            });

            render_pass.set_bind_group(1, bind_group, &[]);
            render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
            render_pass.draw(0..6, 0..batch.instances.len() as u32);
        }
    }
}
