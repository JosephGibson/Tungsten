//! M26 material pipeline: per-material WGSL, rendered on the sprite
//! vertex/instance path.
//!
//! Material pipelines reuse the sprite pipeline layout for groups 0 (camera)
//! and 1 (texture + sampler). A third group binds a 256-byte UBO matching
//! `UniformOverrideBlock`'s byte layout, one per batch of the material in a
//! frame (`MaterialSlot`, `D-101`). Vertex + instance layouts
//! come from `SpritePipeline::vertex_layouts`, so a `SpriteBatch` can be
//! drawn through either pipeline without repacking its instance buffer.

use tungsten_core::assets::{MaterialAssetId, MaterialUniformDefaults};

/// Per-material GPU resources: the pipeline and the layout of its uniform
/// group, which every `MaterialSlot` of the material binds.
pub struct MaterialPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub defaults: MaterialUniformDefaults,
    pub name: String,
    pub shader_id_name: String,
    /// Material id this pipeline was allocated for; lets the renderer key
    /// rebuild-on-hot-reload without a parallel index.
    pub material_id: MaterialAssetId,
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn build_material_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    camera_bind_group_layout: &wgpu::BindGroupLayout,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
    sample_count: u32,
    depth_write: bool,
    name: &str,
) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
    let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("material_{name}_bgl")),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("material_{name}_layout")),
        bind_group_layouts: &[
            Some(camera_bind_group_layout),
            Some(texture_bind_group_layout),
            Some(&material_bgl),
        ],
        immediate_size: 0,
    });

    let vertex_layouts = crate::sprite::SpritePipeline::vertex_layouts();
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("material_{name}_pipeline")),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &vertex_layouts,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
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
    });
    (pipeline, material_bgl)
}

/// One batch's 256-byte uniform buffer and the group-2 bind group over it.
/// The k-th batch of a material in a frame draws with slot k, so batches with
/// different overrides keep their own bytes (`D-101`).
pub(crate) struct MaterialSlot {
    pub ubo: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

impl MaterialSlot {
    pub(crate) fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, name: &str) -> Self {
        let ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("material_{name}_ubo")),
            size: 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("material_{name}_bg")),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ubo.as_entire_binding(),
            }],
        });
        Self { ubo, bind_group }
    }
}

#[cfg(test)]
#[path = "tests/material.rs"]
mod tests;
