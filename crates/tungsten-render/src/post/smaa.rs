//! M27 SMAA 1x presentation pipeline.
//!
//! Three sub-pipelines (edge detect, blend weights, neighborhood blend) plus
//! the area / search lookup textures and one 256-byte preset UBO. The renderer
//! frame loop opens each `PassDesc` via `PassRecorder::begin`, then delegates
//! to the matching `record_*_pass` method here so SMAA never calls
//! `begin_render_pass` itself.
//!
//! Preset switches update the UBO only; they do not rebuild pipelines or LUTs.
//! Stage shader hot reload routes through `Renderer::upload_shader` /
//! `reload_shader`, which call `rebuild_stage_with_module` here.
//!
//! The bind-group layouts mirror the WGSL stage modules:
//!
//! ```text
//! edge:               group(0): src_tex + src_sampler;
//!                     group(1) binding 0: preset UBO
//! blend weights:      group(0): edges_tex + edges_sampler;
//!                     group(1): area_tex, search_tex, lut_sampler, preset UBO
//! neighborhood blend: group(0): src_tex + src_sampler;
//!                     group(1): blend_tex, blend_sampler, preset UBO
//! ```

use bytemuck::{Pod, Zeroable};
use tungsten_core::assets::ShaderAssetId;
use tungsten_core::config::PostAaMode;
use wgpu::util::DeviceExt;

use crate::passes::TargetId;
use crate::post::smaa_luts;
use crate::targets::{RenderTargetPool, SMAA_BLEND_FORMAT, SMAA_EDGES_FORMAT};

/// Stage shader manifest names. Must match `assets/manifest.json` keys and the
/// pre-seeded ids in `Renderer::new`.
pub const SMAA_EDGE_SHADER_NAME: &str = "smaa_edge";
pub const SMAA_BLEND_WEIGHTS_SHADER_NAME: &str = "smaa_blend_weights";
pub const SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME: &str = "smaa_neighborhood_blend";

/// Shader ids assigned by `Renderer` for the three SMAA stages. Used as a
/// stable handle for `rebuild_stage_with_module` dispatch.
#[derive(Debug, Clone, Copy)]
pub struct SmaaShaderIds {
    pub edge: ShaderAssetId,
    pub blend_weights: ShaderAssetId,
    pub neighborhood_blend: ShaderAssetId,
}

/// Source-side SMAA preset knobs. Mirrors the canonical SMAA preset block
/// (Low / Medium / High / Ultra). Ultra's diag/corner values are encoded but
/// the current orthogonal-only WGSL ports treat the diag/corner gates as
/// passthrough; future work can wire them in without changing this struct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmaaPreset {
    pub threshold: f32,
    pub max_search_steps: u32,
    pub max_search_steps_diag: u32,
    pub corner_rounding: u32,
}

impl SmaaPreset {
    #[must_use]
    pub fn from_mode(mode: PostAaMode) -> Option<Self> {
        match mode {
            PostAaMode::SmaaLow => Some(Self {
                threshold: 0.15,
                max_search_steps: 4,
                max_search_steps_diag: 0,
                corner_rounding: u32::MAX,
            }),
            PostAaMode::SmaaMedium => Some(Self {
                threshold: 0.10,
                max_search_steps: 8,
                max_search_steps_diag: 0,
                corner_rounding: u32::MAX,
            }),
            PostAaMode::SmaaHigh => Some(Self {
                threshold: 0.10,
                max_search_steps: 16,
                max_search_steps_diag: 8,
                corner_rounding: 25,
            }),
            PostAaMode::SmaaUltra => Some(Self {
                threshold: 0.05,
                max_search_steps: 32,
                max_search_steps_diag: 16,
                corner_rounding: 25,
            }),
            _ => None,
        }
    }
}

/// 256-byte UBO matching `UniformOverrideBlock` total size. The first 32 bytes
/// hold preset knobs + `rt_metrics`; the rest is padding so the layout matches
/// the engine-wide post UBO contract.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[allow(clippy::pub_underscore_fields)]
pub struct SmaaPresetUbo {
    pub threshold: f32,
    pub max_search_steps: f32,
    pub max_search_steps_diag: f32,
    pub corner_rounding: f32,
    pub rt_metrics: [f32; 4],
    pub _pad: [f32; 56],
}

impl SmaaPresetUbo {
    #[must_use]
    pub fn from_preset(preset: SmaaPreset, size: (u32, u32)) -> Self {
        let w = size.0.max(1) as f32;
        let h = size.1.max(1) as f32;
        Self {
            threshold: preset.threshold,
            max_search_steps: preset.max_search_steps as f32,
            max_search_steps_diag: preset.max_search_steps_diag as f32,
            corner_rounding: preset.corner_rounding as f32,
            rt_metrics: [1.0 / w, 1.0 / h, w, h],
            _pad: [0.0; 56],
        }
    }
}

const _: () = assert!(std::mem::size_of::<SmaaPresetUbo>() == 256);

#[allow(clippy::struct_field_names)]
struct SmaaLayouts {
    edge_source_bgl: wgpu::BindGroupLayout,
    edge_params_bgl: wgpu::BindGroupLayout,
    blend_input_bgl: wgpu::BindGroupLayout,
    blend_lut_bgl: wgpu::BindGroupLayout,
    nbh_input_bgl: wgpu::BindGroupLayout,
    nbh_params_bgl: wgpu::BindGroupLayout,
}

fn build_layouts(device: &wgpu::Device) -> SmaaLayouts {
    let texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let sampler_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    };
    let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };

    let edge_source_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa_edge_source_bgl"),
        entries: &[texture_entry(0), sampler_entry(1)],
    });
    let edge_params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa_edge_params_bgl"),
        entries: &[uniform_entry(0)],
    });
    let blend_input_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa_blend_input_bgl"),
        entries: &[texture_entry(0), sampler_entry(1)],
    });
    let blend_lut_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa_blend_lut_bgl"),
        entries: &[
            texture_entry(0),
            texture_entry(1),
            sampler_entry(2),
            uniform_entry(3),
        ],
    });
    let nbh_input_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa_nbh_input_bgl"),
        entries: &[texture_entry(0), sampler_entry(1)],
    });
    let nbh_params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("smaa_nbh_params_bgl"),
        entries: &[texture_entry(0), sampler_entry(1), uniform_entry(2)],
    });

    SmaaLayouts {
        edge_source_bgl,
        edge_params_bgl,
        blend_input_bgl,
        blend_lut_bgl,
        nbh_input_bgl,
        nbh_params_bgl,
    }
}

/// Owns the three SMAA pipelines + LUT textures + preset UBO.
pub struct SmaaPipeline {
    edge_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    nbh_pipeline: wgpu::RenderPipeline,
    layouts: SmaaLayouts,
    preset_ubo: wgpu::Buffer,
    edge_params_bg: wgpu::BindGroup,
    blend_lut_bg: wgpu::BindGroup,
    #[allow(dead_code)]
    area_view: wgpu::TextureView,
    #[allow(dead_code)]
    search_view: wgpu::TextureView,
    #[allow(dead_code)]
    area_tex: wgpu::Texture,
    #[allow(dead_code)]
    search_tex: wgpu::Texture,
    linear_sampler: wgpu::Sampler,
    target_format: wgpu::TextureFormat,
    pub shader_ids: SmaaShaderIds,
}

impl SmaaPipeline {
    /// Build all three pipelines + LUTs. Stage shader modules come from the
    /// shared `ShaderModuleCache`; the renderer pre-seeds them with the
    /// compile-time `include_str!` WGSL.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        edge_module: &wgpu::ShaderModule,
        blend_module: &wgpu::ShaderModule,
        nbh_module: &wgpu::ShaderModule,
        shader_ids: SmaaShaderIds,
    ) -> Self {
        let layouts = build_layouts(device);
        let preset_ubo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("smaa_preset_ubo"),
            contents: bytemuck::bytes_of(&SmaaPresetUbo::from_preset(
                SmaaPreset {
                    threshold: 0.10,
                    max_search_steps: 16,
                    max_search_steps_diag: 0,
                    corner_rounding: u32::MAX,
                },
                (1, 1),
            )),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let (area_tex, area_view) = smaa_luts::upload_area(device, queue);
        let (search_tex, search_view) = smaa_luts::upload_search(device, queue);
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("smaa_linear_clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let edge_pipeline = build_edge_pipeline(device, &layouts, edge_module);
        let blend_pipeline = build_blend_pipeline(device, &layouts, blend_module);
        let nbh_pipeline = build_nbh_pipeline(device, &layouts, nbh_module, format);

        let edge_params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa_edge_params_bg"),
            layout: &layouts.edge_params_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: preset_ubo.as_entire_binding(),
            }],
        });
        let blend_lut_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa_blend_lut_bg"),
            layout: &layouts.blend_lut_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&area_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&search_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: preset_ubo.as_entire_binding(),
                },
            ],
        });
        Self {
            edge_pipeline,
            blend_pipeline,
            nbh_pipeline,
            layouts,
            preset_ubo,
            edge_params_bg,
            blend_lut_bg,
            area_view,
            search_view,
            area_tex,
            search_tex,
            linear_sampler,
            target_format: format,
            shader_ids,
        }
    }

    /// Repack `SmaaPresetUbo` from the active preset + viewport. Called at
    /// allocation, on resize, and on `set_post_aa`. Cheap; no pipeline work.
    pub fn update_preset(&self, queue: &wgpu::Queue, mode: PostAaMode, size: (u32, u32)) {
        if let Some(preset) = SmaaPreset::from_mode(mode) {
            let ubo = SmaaPresetUbo::from_preset(preset, size);
            queue.write_buffer(&self.preset_ubo, 0, bytemuck::bytes_of(&ubo));
        }
    }

    /// Record the edge-detection pass into an open render pass.
    pub fn record_edge_pass(
        &self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'_>,
        source_view: &wgpu::TextureView,
    ) {
        let source_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa_edge_source_bg"),
            layout: &self.layouts.edge_source_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        });
        render_pass.set_pipeline(&self.edge_pipeline);
        render_pass.set_bind_group(0, &source_bg, &[]);
        render_pass.set_bind_group(1, &self.edge_params_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Record the blend-weights pass.
    pub fn record_blend_weights_pass(
        &self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'_>,
        pool: &RenderTargetPool,
    ) {
        let edges_view = pool
            .scene
            .smaa_edges_view()
            .expect("blend_weights requires SmaaEdges target");
        let input_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa_blend_input_bg"),
            layout: &self.layouts.blend_input_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(edges_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        });
        render_pass.set_pipeline(&self.blend_pipeline);
        render_pass.set_bind_group(0, &input_bg, &[]);
        render_pass.set_bind_group(1, &self.blend_lut_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Record the neighborhood-blend pass.
    pub fn record_neighborhood_pass(
        &self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'_>,
        pool: &RenderTargetPool,
        source_view: &wgpu::TextureView,
    ) {
        let blend_view = pool
            .scene
            .smaa_blend_view()
            .expect("neighborhood requires SmaaBlend target");
        let input_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa_nbh_input_bg"),
            layout: &self.layouts.nbh_input_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        });
        let params_bg = build_nbh_params_bg(
            device,
            &self.layouts.nbh_params_bgl,
            blend_view,
            &self.linear_sampler,
            &self.preset_ubo,
        );
        render_pass.set_pipeline(&self.nbh_pipeline);
        render_pass.set_bind_group(0, &input_bg, &[]);
        render_pass.set_bind_group(1, &params_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Hot-reload entry: rebuild only the affected stage's pipeline against a
    /// freshly validated module. Caller (Renderer) commits the module to the
    /// `ShaderModuleCache` after this returns.
    pub fn rebuild_stage_with_module(
        &mut self,
        device: &wgpu::Device,
        shader_id: ShaderAssetId,
        module: &wgpu::ShaderModule,
    ) {
        if shader_id == self.shader_ids.edge {
            self.edge_pipeline = build_edge_pipeline(device, &self.layouts, module);
        } else if shader_id == self.shader_ids.blend_weights {
            self.blend_pipeline = build_blend_pipeline(device, &self.layouts, module);
        } else if shader_id == self.shader_ids.neighborhood_blend {
            self.nbh_pipeline =
                build_nbh_pipeline(device, &self.layouts, module, self.target_format);
        }
    }

    #[must_use]
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
    }
}

fn build_nbh_params_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    blend_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    ubo: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("smaa_nbh_params_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(blend_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ubo.as_entire_binding(),
            },
        ],
    })
}

fn build_edge_pipeline(
    device: &wgpu::Device,
    layouts: &SmaaLayouts,
    module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("smaa_edge_layout"),
        bind_group_layouts: &[
            Some(&layouts.edge_source_bgl),
            Some(&layouts.edge_params_bgl),
        ],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smaa_edge_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: SMAA_EDGES_FORMAT,
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

fn build_blend_pipeline(
    device: &wgpu::Device,
    layouts: &SmaaLayouts,
    module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("smaa_blend_layout"),
        bind_group_layouts: &[Some(&layouts.blend_input_bgl), Some(&layouts.blend_lut_bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smaa_blend_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: SMAA_BLEND_FORMAT,
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

fn build_nbh_pipeline(
    device: &wgpu::Device,
    layouts: &SmaaLayouts,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("smaa_nbh_layout"),
        bind_group_layouts: &[Some(&layouts.nbh_input_bgl), Some(&layouts.nbh_params_bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("smaa_nbh_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
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

/// Free fn `PassDesc` builders consumed by the pass-order splice. Edge + blend
/// clear to transparent so unwritten regions stay zero. Neighborhood loads
/// (covers the full screen via fullscreen triangle).
#[must_use]
pub fn edge_pass_desc() -> crate::passes::PassDesc {
    crate::passes::PassDesc::new("tungsten_smaa_edge_pass", TargetId::SmaaEdges)
        .with_clear(wgpu::Color::TRANSPARENT)
}

#[must_use]
pub fn blend_weights_pass_desc() -> crate::passes::PassDesc {
    crate::passes::PassDesc::new("tungsten_smaa_blend_weights_pass", TargetId::SmaaBlend)
        .with_clear(wgpu::Color::TRANSPARENT)
}

#[must_use]
pub fn neighborhood_pass_desc() -> crate::passes::PassDesc {
    crate::passes::PassDesc::new("tungsten_smaa_neighborhood_pass", TargetId::PresentSource)
        .with_clear(wgpu::Color::TRANSPARENT)
}

#[cfg(test)]
#[path = "../tests/smaa.rs"]
mod tests;
