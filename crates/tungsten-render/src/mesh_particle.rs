//! Instanced mesh particle pipeline (M31, `D-093`); shares the quad camera
//! bind group and draws one indexed, instanced call per mesh batch.

use bytemuck::{Pod, Zeroable};
use tungsten_core::ParticleMeshAssetId;
use wgpu::util::DeviceExt;

/// One mesh particle: the mesh origin in world pixels, a per-axis scale, a
/// rotation in radians about the origin, and a color.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct MeshParticleInstance {
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub color: [u8; 4],
}

impl MeshParticleInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32,
        4 => Unorm8x4,
    ];

    #[must_use]
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// The instances of one mesh for one frame.
#[derive(Debug, Clone)]
pub struct MeshParticleBatch {
    pub mesh: ParticleMeshAssetId,
    pub instances: Vec<MeshParticleInstance>,
}

const MESH_VERTEX_ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

fn mesh_vertex_desc() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_VERTEX_ATTRIBS,
    }
}

/// One indexed draw: a mesh and its instance range in the instance buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DrawRange {
    mesh: ParticleMeshAssetId,
    first: u32,
    count: u32,
}

/// Plans a frame without touching the GPU: one draw per non-empty batch whose
/// mesh is uploaded, with those batches' instances appended to `staging` in
/// draw order. Batches of a mesh that is not uploaded are left out, and the
/// mesh is listed once in `missing`.
fn plan_draws(
    batches: &[MeshParticleBatch],
    is_uploaded: impl Fn(ParticleMeshAssetId) -> bool,
    staging: &mut Vec<MeshParticleInstance>,
    draws: &mut Vec<DrawRange>,
    missing: &mut Vec<ParticleMeshAssetId>,
) {
    staging.clear();
    draws.clear();
    missing.clear();
    for batch in batches {
        if batch.instances.is_empty() {
            continue;
        }
        if !is_uploaded(batch.mesh) {
            if !missing.contains(&batch.mesh) {
                missing.push(batch.mesh);
            }
            continue;
        }
        draws.push(DrawRange {
            mesh: batch.mesh,
            first: staging.len() as u32,
            count: batch.instances.len() as u32,
        });
        staging.extend_from_slice(&batch.instances);
    }
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

pub struct MeshParticlePipeline {
    pipeline: wgpu::RenderPipeline,
    /// Indexed by `ParticleMeshAssetId::index`; `None` until uploaded.
    meshes: Vec<Option<GpuMesh>>,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    /// This frame's instances in draw order; kept to reuse its allocation.
    staging: Vec<MeshParticleInstance>,
    draws: Vec<DrawRange>,
    missing: Vec<ParticleMeshAssetId>,
    /// Meshes already reported as drawn without an upload.
    warned: Vec<ParticleMeshAssetId>,
}

impl MeshParticlePipeline {
    const INITIAL_INSTANCE_CAPACITY: usize = 256;

    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        sample_count: u32,
        depth_attached: bool,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh_particle_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mesh_particle.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh_particle_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh_particle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(mesh_vertex_desc()), Some(MeshParticleInstance::desc())],
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

        let instance_capacity = Self::INITIAL_INSTANCE_CAPACITY;
        let instance_buffer = Self::create_instance_buffer(device, instance_capacity);

        Self {
            pipeline,
            meshes: Vec::new(),
            instance_buffer,
            instance_capacity,
            staging: Vec::new(),
            draws: Vec::new(),
            missing: Vec::new(),
            warned: Vec::new(),
        }
    }

    fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh_particle_instance_buffer"),
            size: (capacity * std::mem::size_of::<MeshParticleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn ensure_instance_capacity(&mut self, device: &wgpu::Device, required_instances: usize) {
        if required_instances <= self.instance_capacity {
            return;
        }

        self.instance_capacity = required_instances.next_power_of_two().max(1);
        self.instance_buffer = Self::create_instance_buffer(device, self.instance_capacity);
    }

    /// Uploads or replaces the vertex and index buffers of `id`.
    pub fn upload_mesh(
        &mut self,
        device: &wgpu::Device,
        id: ParticleMeshAssetId,
        vertices: &[[f32; 2]],
        indices: &[u16],
    ) {
        if vertices.is_empty() || indices.is_empty() {
            log::warn!(
                "particle mesh {} has no vertices or indices; upload skipped",
                id.index()
            );
            return;
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_particle_vertex_buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_particle_index_buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let slot = id.index() as usize;
        if slot >= self.meshes.len() {
            self.meshes.resize_with(slot + 1, || None);
        }
        self.meshes[slot] = Some(GpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        });
    }

    /// Writes this frame's instances into the kept instance buffer and records
    /// one draw range per batch. The ranges hold until the next call.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batches: &[MeshParticleBatch],
    ) {
        let meshes = &self.meshes;
        plan_draws(
            batches,
            |id| matches!(meshes.get(id.index() as usize), Some(Some(_))),
            &mut self.staging,
            &mut self.draws,
            &mut self.missing,
        );

        for id in &self.missing {
            if !self.warned.contains(id) {
                self.warned.push(*id);
                log::warn!(
                    "mesh particles reference particle mesh {} before its upload; batch skipped",
                    id.index()
                );
            }
        }

        if self.staging.is_empty() {
            return;
        }
        self.ensure_instance_capacity(device, self.staging.len());
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.staging),
        );
    }

    /// Draws the ranges recorded by the last `prepare`.
    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        if self.draws.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        for draw in &self.draws {
            let Some(Some(mesh)) = self.meshes.get(draw.mesh.index() as usize) else {
                continue;
            };
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..mesh.index_count, 0, draw.first..draw.first + draw.count);
        }
    }
}

#[cfg(test)]
#[path = "tests/mesh_particle.rs"]
mod tests;
