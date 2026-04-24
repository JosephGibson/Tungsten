//! Debug line pipeline; shares quad camera bind group and expands in shader.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Oriented-quad line instance.
///
/// `_pad` keeps the struct 16-byte aligned for GPU vertex layout — not dead code.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[allow(clippy::pub_underscore_fields)]
pub struct DebugLineInstance {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub thickness: f32,
    pub _pad: f32,
    pub color: [f32; 4],
}

impl DebugLineInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32,
        4 => Float32,
        5 => Float32x4,
    ];

    #[must_use]
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DebugLineInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Vertex {
    corner: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const UNIT_QUAD_VERTICES: &[Vertex] = &[
    Vertex { corner: [0.0, 0.0] },
    Vertex { corner: [1.0, 0.0] },
    Vertex { corner: [1.0, 1.0] },
    Vertex { corner: [0.0, 0.0] },
    Vertex { corner: [1.0, 1.0] },
    Vertex { corner: [0.0, 1.0] },
];

pub struct DebugLinePipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
}

impl DebugLinePipeline {
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        sample_count: u32,
        depth_attached: bool,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("debug_line_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("debug_line.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("debug_line_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("debug_line_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc(), DebugLineInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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
            depth_stencil: crate::quad::passthrough_depth_stencil(depth_attached),
            multisample: wgpu::MultisampleState {
                count: sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debug_line_unit_quad"),
            contents: bytemuck::cast_slice(UNIT_QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            pipeline,
            vertex_buffer,
        }
    }

    pub fn draw(
        &self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        instances: &[DebugLineInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debug_line_instance_buffer"),
            contents: bytemuck::cast_slice(instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.draw(0..6, 0..instances.len() as u32);
    }
}

#[cfg(test)]
#[path = "tests/debug_line.rs"]
mod tests;
