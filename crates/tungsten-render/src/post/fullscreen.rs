//! Shared fullscreen-triangle support for the 17 stock post-processing effects.
//!
//! Every effect uses the same bind-group layout:
//!   - group 0 binding 0: source color texture
//!   - group 0 binding 1: linear sampler (post sampling is always linear)
//!   - group 1 binding 0: 256-byte effect UBO (matches `UniformOverrideBlock`
//!     / `MaterialUniforms` layout so slot-driven tweens can reach post params
//!     the same way they reach materials).
//!
//! The vertex shader is implicit: each stock WGSL file owns its own `vs_main`
//! that synthesises a fullscreen triangle from `@builtin(vertex_index)`. This
//! keeps one WGSL file = one module so `include_str!` + manifest hot-reload
//! stay uncomplicated.

/// Build the bind-group layouts shared by every stock post pipeline.
#[must_use]
pub fn build_layouts(device: &wgpu::Device) -> StockLayouts {
    let source_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post_source_bgl"),
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
    let params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("post_params_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    StockLayouts {
        source_bgl,
        params_bgl,
    }
}

pub struct StockLayouts {
    pub source_bgl: wgpu::BindGroupLayout,
    pub params_bgl: wgpu::BindGroupLayout,
}

/// Build one stock effect pipeline. Each effect module calls this exactly once
/// at renderer init. `wgsl_source` is `include_str!`'d; a matching file lives
/// under `assets/shaders/stock/` so the manifest can hot-reload the body.
#[must_use]
pub fn build_pipeline(
    device: &wgpu::Device,
    layouts: &StockLayouts,
    label: &str,
    wgsl_source: &str,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!("{label}_shader")),
        source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label}_layout")),
        bind_group_layouts: &[Some(&layouts.source_bgl), Some(&layouts.params_bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("{label}_pipeline")),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: None,
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
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// 256-byte effect UBO buffer allocator.
#[must_use]
pub fn build_params_ubo(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(&format!("{label}_params")),
        size: 256,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[must_use]
pub fn build_params_bind_group(
    device: &wgpu::Device,
    layouts: &StockLayouts,
    label: &str,
    ubo: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label}_params_bg")),
        layout: &layouts.params_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: ubo.as_entire_binding(),
        }],
    })
}

#[must_use]
pub fn build_source_bind_group(
    device: &wgpu::Device,
    layouts: &StockLayouts,
    label: &str,
    source_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label}_source_bg")),
        layout: &layouts.source_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
