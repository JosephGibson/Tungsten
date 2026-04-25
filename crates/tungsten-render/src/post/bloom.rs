//! M28 bloom pipeline.
//!
//! Four sub-pipelines (threshold, downsample, upsample, composite) share the
//! `Rgba16Float` `BloomPyramid` allocated on `SceneTarget`. Bloom is a normal
//! reorderable `PostPass`, but unlike the 17 single-fullscreen-pass stock
//! effects it records `1 + 2*(N-1) + 1` sub-passes — each into a different
//! attachment — so it bypasses `PassRecorder::begin` and drives the encoder
//! directly. See `D-060`.

use tungsten_core::assets::ShaderAssetId;
use tungsten_core::post::BloomParams;
use tungsten_core::tween::UniformOverrideBlock;
use wgpu::util::DeviceExt;

use crate::passes::TargetId;
use crate::shader_hot_reload::ShaderModuleCache;
use crate::targets::{RenderTargetPool, BLOOM_PYRAMID_FORMAT};

/// Stage shader manifest names. Must match `assets/manifest.json` keys and the
/// pre-seeded ids in `Renderer::new`.
pub const BLOOM_THRESHOLD_SHADER_NAME: &str = "bloom_threshold";
pub const BLOOM_DOWNSAMPLE_SHADER_NAME: &str = "bloom_downsample";
pub const BLOOM_UPSAMPLE_SHADER_NAME: &str = "bloom_upsample";
pub const BLOOM_COMPOSITE_SHADER_NAME: &str = "bloom_composite";

/// Stage `pass_kind` values written into the UBO. Mirrored as comments in the
/// WGSL stages but unused there today; kept stable so future shader work can
/// branch on the kind without changing the host code.
const PASS_KIND_THRESHOLD: i32 = 0;
const PASS_KIND_DOWNSAMPLE: i32 = 1;
const PASS_KIND_UPSAMPLE: i32 = 2;
const PASS_KIND_COMPOSITE: i32 = 3;

/// Stable shader-id handles assigned by the renderer for the four bloom stages.
#[derive(Debug, Clone, Copy)]
pub struct BloomShaderIds {
    pub threshold: ShaderAssetId,
    pub downsample: ShaderAssetId,
    pub upsample: ShaderAssetId,
    pub composite: ShaderAssetId,
}

#[allow(clippy::struct_field_names)]
struct BloomLayouts {
    /// Group 0: source texture + sampler. Reused by all four pipelines for the
    /// per-stage source view (slot src for threshold/composite, prior mip for
    /// downsample/upsample).
    source_bgl: wgpu::BindGroupLayout,
    /// Group 1: 256-byte params UBO matching `UniformOverrideBlock` layout.
    params_bgl: wgpu::BindGroupLayout,
    /// Group 2: bloom pyramid mip 0 + sampler. Composite-only.
    composite_bgl: wgpu::BindGroupLayout,
}

fn build_layouts(device: &wgpu::Device) -> BloomLayouts {
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
    let source_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bloom_source_bgl"),
        entries: &[texture_entry(0), sampler_entry(1)],
    });
    let params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bloom_params_bgl"),
        entries: &[uniform_entry(0)],
    });
    let composite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bloom_composite_bgl"),
        entries: &[texture_entry(0), sampler_entry(1)],
    });
    BloomLayouts {
        source_bgl,
        params_bgl,
        composite_bgl,
    }
}

/// Pack `BloomParams` + per-subpass overrides into the shared 256-byte UBO.
#[must_use]
pub fn pack_params(
    params: &BloomParams,
    inv_src_size: (f32, f32),
    mip_count: u32,
    dst_level: u32,
    pass_kind: i32,
) -> UniformOverrideBlock {
    let mut block = UniformOverrideBlock::default();
    block.vec4[0] = [inv_src_size.0, inv_src_size.1, 0.0, 0.0];
    // composite_tint: white. Reserved for future tinted-bloom variants.
    block.vec4[1] = [1.0, 1.0, 1.0, 1.0];
    block.f32s[0] = params.threshold;
    block.f32s[1] = params.knee;
    block.f32s[2] = params.intensity;
    block.f32s[3] = params.radius;
    block.i32s[0] = mip_count as i32;
    block.i32s[1] = dst_level as i32;
    block.i32s[2] = pass_kind;
    block
}

/// Owns the four bloom pipelines + their shared bind-group layouts + UBO.
pub struct BloomPipeline {
    threshold: wgpu::RenderPipeline,
    downsample: wgpu::RenderPipeline,
    upsample: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    layouts: BloomLayouts,
    sampler: wgpu::Sampler,
    target_format: wgpu::TextureFormat,
    pub shader_ids: BloomShaderIds,
}

impl BloomPipeline {
    /// Build all four pipelines. Stage modules come from the shared
    /// `ShaderModuleCache`; the renderer pre-seeds them with the compile-time
    /// `include_str!` WGSL.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        cache: &ShaderModuleCache,
        ids: BloomShaderIds,
    ) -> Self {
        let layouts = build_layouts(device);
        let threshold_module = cache
            .get(ids.threshold)
            .expect("bloom_threshold module must be pre-seeded");
        let downsample_module = cache
            .get(ids.downsample)
            .expect("bloom_downsample module must be pre-seeded");
        let upsample_module = cache
            .get(ids.upsample)
            .expect("bloom_upsample module must be pre-seeded");
        let composite_module = cache
            .get(ids.composite)
            .expect("bloom_composite module must be pre-seeded");

        let threshold = build_pyramid_pipeline(
            device,
            &layouts,
            threshold_module,
            "bloom_threshold",
            BlendKind::Replace,
        );
        let downsample = build_pyramid_pipeline(
            device,
            &layouts,
            downsample_module,
            "bloom_downsample",
            BlendKind::Replace,
        );
        let upsample = build_pyramid_pipeline(
            device,
            &layouts,
            upsample_module,
            "bloom_upsample",
            BlendKind::Additive,
        );
        let composite = build_composite_pipeline(device, &layouts, composite_module, format);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom_sampler_linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Self {
            threshold,
            downsample,
            upsample,
            composite,
            layouts,
            sampler,
            target_format: format,
            shader_ids: ids,
        }
    }

    /// Hot-reload entry: rebuild only the affected stage's pipeline against a
    /// freshly validated module. Caller commits the module to the cache after
    /// this returns.
    pub fn rebuild_stage_with_module(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader_id: ShaderAssetId,
        module: &wgpu::ShaderModule,
    ) {
        if shader_id == self.shader_ids.threshold {
            self.threshold = build_pyramid_pipeline(
                device,
                &self.layouts,
                module,
                "bloom_threshold",
                BlendKind::Replace,
            );
        } else if shader_id == self.shader_ids.downsample {
            self.downsample = build_pyramid_pipeline(
                device,
                &self.layouts,
                module,
                "bloom_downsample",
                BlendKind::Replace,
            );
        } else if shader_id == self.shader_ids.upsample {
            self.upsample = build_pyramid_pipeline(
                device,
                &self.layouts,
                module,
                "bloom_upsample",
                BlendKind::Additive,
            );
        } else if shader_id == self.shader_ids.composite {
            self.composite = build_composite_pipeline(device, &self.layouts, module, format);
        }
    }

    #[must_use]
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
    }

    /// Record one bloom slot: threshold → N-1 downsamples → N-1 additive
    /// upsamples → composite. Each sub-pass opens its own `RenderPass` because
    /// the attachments differ per stage (a different mip view, then dst).
    #[allow(clippy::too_many_arguments)]
    pub fn record_pass(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        pool: &RenderTargetPool,
        params: &BloomParams,
        src: TargetId,
        dst: TargetId,
    ) {
        let mip_count = pool.scene.bloom_mip_count();
        if mip_count == 0 {
            return;
        }

        let src_view = resolve_post_view(pool, src);
        let dst_view = resolve_post_view(pool, dst);
        let scene_size = pool.scene.size;
        let inv_scene = inv_size(scene_size);

        encoder.push_debug_group("bloom_slot");

        // Stage 1 — threshold: sample slot src, write mip 0.
        {
            let payload = pack_params(params, inv_scene, mip_count, 0, PASS_KIND_THRESHOLD);
            let ubo = create_params_ubo(device, "bloom_threshold_ubo", &payload);
            let params_bg = build_params_bg(device, &self.layouts, &ubo, "bloom_threshold");
            let source_bg = build_source_bg(
                device,
                &self.layouts,
                src_view,
                &self.sampler,
                "bloom_threshold",
            );
            let mip0_view = pool
                .scene
                .bloom_mip_view(0)
                .expect("bloom mip 0 must exist");
            encoder.push_debug_group("bloom_threshold");
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tungsten_bloom_threshold"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: mip0_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.threshold);
            pass.set_bind_group(0, &source_bg, &[]);
            pass.set_bind_group(1, &params_bg, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            encoder.pop_debug_group();
        }

        // Stage 2 — downsample chain: each level reads (level-1) and writes level.
        for level in 1..mip_count {
            let prev_extent = pool
                .scene
                .bloom_mip_extent(level - 1)
                .expect("prev mip must exist");
            let payload = pack_params(
                params,
                inv_size(prev_extent),
                mip_count,
                level,
                PASS_KIND_DOWNSAMPLE,
            );
            let ubo = create_params_ubo(device, "bloom_downsample_ubo", &payload);
            let params_bg = build_params_bg(device, &self.layouts, &ubo, "bloom_downsample");
            let prev_view = pool
                .scene
                .bloom_mip_view(level - 1)
                .expect("prev mip view must exist");
            let level_view = pool
                .scene
                .bloom_mip_view(level)
                .expect("level mip view must exist");
            let source_bg = build_source_bg(
                device,
                &self.layouts,
                prev_view,
                &self.sampler,
                "bloom_downsample",
            );
            encoder.push_debug_group("bloom_downsample");
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tungsten_bloom_downsample"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: level_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.downsample);
            pass.set_bind_group(0, &source_bg, &[]);
            pass.set_bind_group(1, &params_bg, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            encoder.pop_debug_group();
        }

        // Stage 3 — upsample chain: each iteration reads (level+1) and adds
        // into level. Pipeline blend state contributes the One+One.
        if mip_count >= 2 {
            for level in (0..mip_count - 1).rev() {
                let next_extent = pool
                    .scene
                    .bloom_mip_extent(level + 1)
                    .expect("next mip must exist");
                let payload = pack_params(
                    params,
                    inv_size(next_extent),
                    mip_count,
                    level,
                    PASS_KIND_UPSAMPLE,
                );
                let ubo = create_params_ubo(device, "bloom_upsample_ubo", &payload);
                let params_bg = build_params_bg(device, &self.layouts, &ubo, "bloom_upsample");
                let next_view = pool
                    .scene
                    .bloom_mip_view(level + 1)
                    .expect("next mip view must exist");
                let level_view = pool
                    .scene
                    .bloom_mip_view(level)
                    .expect("level mip view must exist");
                let source_bg = build_source_bg(
                    device,
                    &self.layouts,
                    next_view,
                    &self.sampler,
                    "bloom_upsample",
                );
                encoder.push_debug_group("bloom_upsample");
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tungsten_bloom_upsample"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: level_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // Load: pipeline blend additively combines fragment
                            // with the prior downsample contents. Clearing here
                            // would discard the downsampled energy.
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    ..Default::default()
                });
                pass.set_pipeline(&self.upsample);
                pass.set_bind_group(0, &source_bg, &[]);
                pass.set_bind_group(1, &params_bg, &[]);
                pass.draw(0..3, 0..1);
                drop(pass);
                encoder.pop_debug_group();
            }
        }

        // Stage 4 — composite: src + bloom * intensity, mixed by radius, into dst.
        {
            let payload = pack_params(params, inv_scene, mip_count, 0, PASS_KIND_COMPOSITE);
            let ubo = create_params_ubo(device, "bloom_composite_ubo", &payload);
            let params_bg = build_params_bg(device, &self.layouts, &ubo, "bloom_composite");
            let source_bg = build_source_bg(
                device,
                &self.layouts,
                src_view,
                &self.sampler,
                "bloom_composite",
            );
            let mip0_view = pool
                .scene
                .bloom_mip_view(0)
                .expect("bloom mip 0 must exist");
            let composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bloom_composite_bg"),
                layout: &self.layouts.composite_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(mip0_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            encoder.push_debug_group("bloom_composite");
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tungsten_bloom_composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.composite);
            pass.set_bind_group(0, &source_bg, &[]);
            pass.set_bind_group(1, &params_bg, &[]);
            pass.set_bind_group(2, &composite_bg, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            encoder.pop_debug_group();
        }

        encoder.pop_debug_group();
    }
}

fn resolve_post_view(pool: &RenderTargetPool, target: TargetId) -> &wgpu::TextureView {
    match target {
        TargetId::SceneColor => pool.scene.color_view(),
        TargetId::PostPing => pool.scene.post_ping_view(),
        TargetId::PostPong => pool.scene.post_pong_view(),
        _ => unreachable!("invalid bloom slot target {target:?}"),
    }
}

fn inv_size(size: (u32, u32)) -> (f32, f32) {
    let w = size.0.max(1) as f32;
    let h = size.1.max(1) as f32;
    (1.0 / w, 1.0 / h)
}

fn create_params_ubo(
    device: &wgpu::Device,
    label: &'static str,
    block: &UniformOverrideBlock,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: &block.to_bytes(),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

fn build_params_bg(
    device: &wgpu::Device,
    layouts: &BloomLayouts,
    ubo: &wgpu::Buffer,
    label: &str,
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

fn build_source_bg(
    device: &wgpu::Device,
    layouts: &BloomLayouts,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label}_source_bg")),
        layout: &layouts.source_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[derive(Clone, Copy)]
enum BlendKind {
    Replace,
    Additive,
}

fn build_pyramid_pipeline(
    device: &wgpu::Device,
    layouts: &BloomLayouts,
    module: &wgpu::ShaderModule,
    label: &str,
    blend: BlendKind,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label}_layout")),
        bind_group_layouts: &[Some(&layouts.source_bgl), Some(&layouts.params_bgl)],
        immediate_size: 0,
    });
    let blend_state = match blend {
        BlendKind::Replace => None,
        BlendKind::Additive => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
        }),
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("{label}_pipeline")),
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
                format: BLOOM_PYRAMID_FORMAT,
                blend: blend_state,
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

#[cfg(test)]
#[path = "../tests/bloom.rs"]
mod tests;

fn build_composite_pipeline(
    device: &wgpu::Device,
    layouts: &BloomLayouts,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bloom_composite_layout"),
        bind_group_layouts: &[
            Some(&layouts.source_bgl),
            Some(&layouts.params_bgl),
            Some(&layouts.composite_bgl),
        ],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bloom_composite_pipeline"),
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
