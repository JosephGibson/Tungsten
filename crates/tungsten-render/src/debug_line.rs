//! `DebugLinePipeline` (M21): draws oriented-quad lines for arbitrary-angle
//! debug primitives — single lines and circle polylines emitted by
//! `DebugDraw`. Axis-aligned AABB outlines are drawn via the existing
//! `QuadPipeline`, not here.
//!
//! Camera uniform is borrowed from `QuadPipeline::camera_bind_group_layout()`
//! (and the bind group is passed into `draw`) so only one `view_proj` buffer
//! lives on the GPU across quad / sprite / debug-line paths.
//!
//! Thickness is specified in world-space units; expansion is computed in the
//! vertex shader from the line's tangent. This avoids any viewport uniform
//! or push constants — debug rendering at 1x camera zoom treats one world
//! unit as one screen pixel (see the ortho projection in
//! `Renderer::render_frame_with_quads`).

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Per-instance data for one oriented-quad line segment.
///
/// `_pad` keeps the `color` field 8-byte aligned after the `thickness`
/// scalar. Layout is vertex-buffer-only; no uniform/storage buffer path
/// depends on WGSL struct alignment rules.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
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
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
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
mod tests {
    use super::*;

    #[test]
    fn debug_line_instance_layout_is_stable() {
        assert_eq!(std::mem::size_of::<DebugLineInstance>(), 40);
        assert_eq!(std::mem::align_of::<DebugLineInstance>(), 4);
    }

    #[test]
    fn debug_line_instance_is_pod() {
        let inst = DebugLineInstance {
            a: [0.0, 0.0],
            b: [10.0, 0.0],
            thickness: 1.5,
            _pad: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&inst);
        assert_eq!(bytes.len(), std::mem::size_of::<DebugLineInstance>());
    }
}
