//! M28 bloom pipeline.
//!
//! Four sub-pipelines (threshold, downsample, upsample, composite) share an
//! `Rgba16Float` pyramid the frame's graph declares and the pool lends.
//! Bloom is a normal reorderable `PostPass`, but unlike the 17
//! single-fullscreen-pass stock effects it draws `1 + 2*(N-1) + 1`
//! sub-passes, each a pass of the graph into a level of the pyramid or the
//! slot's target. See `D-060`.

use tungsten_core::assets::ShaderAssetId;
use tungsten_core::post::{BloomParams, VignetteParams};
use tungsten_core::tween::UniformOverrideBlock;

use super::whole_target;
use crate::counters::{self, Counts};
use crate::graph::pool::PoolTexture;
use crate::shader_hot_reload::{ShaderModuleCache, build_validated};
use crate::targets::{BLOOM_PYRAMID_FORMAT, TargetCache, bloom_mip_extent_for};

/// Stage shader manifest names. Must match `assets/manifest.json` keys and the
/// pre-seeded ids in `Renderer::new`.
pub const BLOOM_THRESHOLD_SHADER_NAME: &str = "bloom_threshold";
pub const BLOOM_DOWNSAMPLE_SHADER_NAME: &str = "bloom_downsample";
pub const BLOOM_UPSAMPLE_SHADER_NAME: &str = "bloom_upsample";
pub const BLOOM_COMPOSITE_SHADER_NAME: &str = "bloom_composite";

/// The four stage shaders with their compiled-in text, which the renderer
/// seeds its cache with and holds a loaded file against (`D-306`).
pub(crate) const BLOOM_SHADERS: [(&str, &str); 4] = [
    (
        BLOOM_THRESHOLD_SHADER_NAME,
        include_str!("../shaders/stock/bloom_threshold.wgsl"),
    ),
    (
        BLOOM_DOWNSAMPLE_SHADER_NAME,
        include_str!("../shaders/stock/bloom_downsample.wgsl"),
    ),
    (
        BLOOM_UPSAMPLE_SHADER_NAME,
        include_str!("../shaders/stock/bloom_upsample.wgsl"),
    ),
    (
        BLOOM_COMPOSITE_SHADER_NAME,
        include_str!("../shaders/stock/bloom_composite.wgsl"),
    ),
];

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

/// What a bloom slot's GPU objects were built for: they hold views of the
/// slot's source and of the pyramid's levels, pooled textures named by
/// their identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BloomSlotKey {
    pub source: u64,
    pub pyramid: u64,
    pub mip_count: u32,
}

/// One sub-pass of a bloom slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BloomStageKind {
    /// The bright pass into the pyramid's first level.
    Threshold,
    /// The downsample into `level` from the level above it.
    Down(u32),
    /// The additive upsample into `level` from the level below it.
    Up(u32),
    /// The composite into the slot's target; `vignette` when it also draws
    /// the stack's trailing vignette (`D-302`).
    Composite { vignette: bool },
}

/// Where `stage` sits in a slot's recording order: threshold, the
/// downsamples from level 1 up, the upsamples from the second-smallest
/// level down, composite.
pub(crate) fn stage_index(stage: BloomStageKind, mip_count: u32) -> usize {
    let levels = mip_count.max(1) as usize;
    match stage {
        BloomStageKind::Threshold => 0,
        BloomStageKind::Down(level) => level as usize,
        BloomStageKind::Up(level) => levels + (levels - 2 - level as usize),
        BloomStageKind::Composite { .. } => 2 * levels - 1,
    }
}

/// One sub-pass of a bloom slot: its params UBO and the two bind groups it
/// draws with.
struct BloomStage {
    ubo: wgpu::Buffer,
    /// Bytes last written to `ubo`.
    written: [u8; 256],
    params_bg: wgpu::BindGroup,
    source_bg: wgpu::BindGroup,
}

/// GPU objects of one bloom slot: `2 * mip_count` stages in recording order
/// (threshold, downsamples, upsamples, composite) and the composite's
/// pyramid bind group.
struct BloomSlot {
    stages: Vec<BloomStage>,
    composite_bg: wgpu::BindGroup,
    mip_count: u32,
}

/// The cache of post-stack slot `slot`, growing the list to reach it: two
/// bloom passes in one stack keep separate objects.
pub(crate) fn slot_cache<V>(
    slots: &mut Vec<TargetCache<BloomSlotKey, V>>,
    slot: usize,
) -> &mut TargetCache<BloomSlotKey, V> {
    if slots.len() <= slot {
        slots.resize_with(slot + 1, TargetCache::default);
    }
    &mut slots[slot]
}

/// Owns the four bloom pipelines, their shared bind-group layouts and, per
/// post-stack slot, the UBOs and bind groups of that slot's sub-passes.
pub struct BloomPipeline {
    threshold: wgpu::RenderPipeline,
    downsample: wgpu::RenderPipeline,
    upsample: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    /// The composite's `fs_vignette` entry: the stack's trailing vignette
    /// drawn in the composite's pass (`D-302`). Built from the compiled-in
    /// module alone; a replaced composite ends the fold.
    composite_vignette: wgpu::RenderPipeline,
    /// A reload or the manifest replaced the composite's module, which may
    /// hold no `fs_vignette`.
    pub(crate) composite_replaced: bool,
    layouts: BloomLayouts,
    sampler: wgpu::Sampler,
    target_format: wgpu::TextureFormat,
    pub shader_ids: BloomShaderIds,
    slots: Vec<TargetCache<BloomSlotKey, BloomSlot>>,
    /// The slot being recorded: UBO bytes of its sub-passes. Kept for its
    /// allocation.
    payloads: Vec<[u8; 256]>,
    pub(crate) counts: Box<Counts>,
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
        let composite =
            build_composite_pipeline(device, &layouts, composite_module, format, "fs_main");
        let composite_vignette =
            build_composite_pipeline(device, &layouts, composite_module, format, "fs_vignette");

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
            composite_vignette,
            composite_replaced: false,
            layouts,
            sampler,
            target_format: format,
            shader_ids: ids,
            slots: Vec::new(),
            payloads: Vec::new(),
            counts: Counts::boxed(),
        }
    }

    /// Drops the objects of every slot that holds a view of one of the
    /// pooled textures `freed`.
    pub(crate) fn forget_targets(&mut self, freed: &[u64]) {
        for slot in &mut self.slots {
            slot.clear_if(|key| freed.contains(&key.source) || freed.contains(&key.pyramid));
        }
    }

    /// Hot-reload entry: rebuild only the affected stage's pipeline against a
    /// freshly validated module. A pipeline that fails validation is not
    /// swapped in (`Err`). Caller commits the module to the cache after this
    /// returns `Ok`.
    pub fn rebuild_stage_with_module(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader_id: ShaderAssetId,
        module: &wgpu::ShaderModule,
    ) -> Result<(), String> {
        let layouts = &self.layouts;
        let pyramid = |label, blend| {
            build_validated(device, || {
                build_pyramid_pipeline(device, layouts, module, label, blend)
            })
        };
        if shader_id == self.shader_ids.threshold {
            self.threshold = pyramid("bloom_threshold", BlendKind::Replace)?;
        } else if shader_id == self.shader_ids.downsample {
            self.downsample = pyramid("bloom_downsample", BlendKind::Replace)?;
        } else if shader_id == self.shader_ids.upsample {
            self.upsample = pyramid("bloom_upsample", BlendKind::Additive)?;
        } else if shader_id == self.shader_ids.composite {
            self.composite = build_validated(device, || {
                build_composite_pipeline(device, layouts, module, format, "fs_main")
            })?;
            self.composite_replaced = true;
        }
        Ok(())
    }

    #[must_use]
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
    }

    /// Readies post-stack slot `slot` for a frame, before its threshold
    /// pass records: stages the sub-passes' blocks, builds the slot's UBOs
    /// and bind groups once (again when its source, the pyramid or the mip
    /// count change) and writes each block whose bytes changed. `source` is
    /// the pooled texture the slot samples and `pyramid` the pooled pyramid,
    /// each with its identity; `scene_size` is the frame's.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slot: usize,
        params: &BloomParams,
        scene_size: (u32, u32),
        halvings: u32,
        vignette: Option<&VignetteParams>,
        source: (u64, &wgpu::TextureView),
        pyramid: (u64, &PoolTexture),
        mip_count: u32,
    ) {
        if mip_count == 0 {
            return;
        }
        let payloads = &mut self.payloads;
        stage_payloads(
            payloads,
            params,
            scene_size,
            mip_count,
            halvings,
            vignette,
            |level| bloom_mip_extent_for(scene_size.0, scene_size.1, halvings, level),
        );
        let key = BloomSlotKey {
            source: source.0,
            pyramid: pyramid.0,
            mip_count,
        };
        let layouts = &self.layouts;
        let sampler = &self.sampler;
        let counts = &*self.counts;
        let objects = slot_cache(&mut self.slots, slot).get_or_build(key, || {
            build_slot(
                counts, device, layouts, sampler, source.1, pyramid.1, mip_count, payloads,
            )
        });
        // A stage's UBO is written only when its bytes change: new params or
        // new sizes.
        for (stage, payload) in objects.stages.iter_mut().zip(payloads.iter()) {
            if stage.written != *payload {
                counters::write_buffer(counts, queue, &stage.ubo, 0, payload);
                stage.written = *payload;
            }
        }
    }

    /// Draws sub-pass `stage` of the prepared slot `slot` into `pass`, which
    /// the frame's graph opened on the stage's target. Each sub-pass's
    /// debug group opens inside its pass: a group pushed or popped on the
    /// encoder between two passes makes wgpu open a command buffer that
    /// holds only the label.
    ///
    /// The threshold, the downsamples and an offscreen composite load their
    /// targets where they once cleared them: each draws one triangle over
    /// the whole target with blending off and every channel written, so no
    /// texel of what was there survives. A stage that came to draw less
    /// than its whole target would need its clear back. The upsamples add
    /// into what the downsamples left, so they load. Returns whether it
    /// drew: `false` for a slot that was not prepared.
    pub(crate) fn draw_stage(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        slot: usize,
        stage: BloomStageKind,
    ) -> bool {
        let Some(objects) = self.slots.get(slot).and_then(TargetCache::get) else {
            return false;
        };
        let Some(blocks) = objects.stages.get(stage_index(stage, objects.mip_count)) else {
            return false;
        };
        let (group, pipeline) = match stage {
            BloomStageKind::Threshold => ("bloom_threshold", &self.threshold),
            BloomStageKind::Down(_) => ("bloom_downsample", &self.downsample),
            BloomStageKind::Up(_) => ("bloom_upsample", &self.upsample),
            BloomStageKind::Composite { vignette: true } => {
                ("bloom_composite", &self.composite_vignette)
            }
            BloomStageKind::Composite { vignette: false } => ("bloom_composite", &self.composite),
        };
        pass.push_debug_group(group);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &blocks.source_bg, &[]);
        pass.set_bind_group(1, &blocks.params_bg, &[]);
        if matches!(stage, BloomStageKind::Composite { .. }) {
            pass.set_bind_group(2, &objects.composite_bg, &[]);
        }
        counters::draw(&self.counts, pass, 0..3, 0..1);
        pass.pop_debug_group();
        true
    }
}

/// Fills `payloads` with the UBO bytes of a slot's sub-passes in recording
/// order: threshold, the downsamples from level 1 up, the upsamples from the
/// second-smallest level down, composite. `2 * mip_count` entries. The
/// threshold's block carries `halvings` in `levels.w`: at 2 its mip 0 is a
/// quarter of the scene, and it reads a texel's whole 4x4 footprint
/// (`D-301`).
fn stage_payloads(
    payloads: &mut Vec<[u8; 256]>,
    params: &BloomParams,
    scene_size: (u32, u32),
    mip_count: u32,
    halvings: u32,
    vignette: Option<&VignetteParams>,
    mip_extent: impl Fn(u32) -> (u32, u32),
) {
    let inv_scene = inv_size(scene_size);
    payloads.clear();
    let mut threshold = pack_params(params, inv_scene, mip_count, 0, PASS_KIND_THRESHOLD);
    threshold.i32s[3] = halvings as i32;
    payloads.push(threshold.to_bytes());
    for level in 1..mip_count {
        let inv_prev = inv_size(mip_extent(level - 1));
        payloads
            .push(pack_params(params, inv_prev, mip_count, level, PASS_KIND_DOWNSAMPLE).to_bytes());
    }
    for level in (0..mip_count - 1).rev() {
        let inv_next = inv_size(mip_extent(level + 1));
        payloads
            .push(pack_params(params, inv_next, mip_count, level, PASS_KIND_UPSAMPLE).to_bytes());
    }
    let mut composite = pack_params(params, inv_scene, mip_count, 0, PASS_KIND_COMPOSITE);
    if let Some(vignette) = vignette {
        composite.vec4[2] = vignette.color;
        composite.vec4[3] = [vignette.inner, vignette.outer, vignette.strength, 0.0];
    }
    payloads.push(composite.to_bytes());
}

/// Builds a slot's UBOs, seeded with `payloads`, and its bind groups, in
/// the order of [`stage_payloads`].
#[allow(clippy::too_many_arguments)]
fn build_slot(
    counts: &Counts,
    device: &wgpu::Device,
    layouts: &BloomLayouts,
    sampler: &wgpu::Sampler,
    src_view: &wgpu::TextureView,
    pyramid: &PoolTexture,
    mip_count: u32,
    payloads: &[[u8; 256]],
) -> BloomSlot {
    let mip_view = |level: u32| pyramid.view(level);
    // Each sub-pass's label and the view it samples, in recording order.
    let mut sources: Vec<(&'static str, &wgpu::TextureView)> = Vec::with_capacity(payloads.len());
    sources.push(("bloom_threshold", src_view));
    for level in 1..mip_count {
        sources.push(("bloom_downsample", mip_view(level - 1)));
    }
    for level in (0..mip_count - 1).rev() {
        sources.push(("bloom_upsample", mip_view(level + 1)));
    }
    sources.push(("bloom_composite", src_view));

    let stages = sources
        .into_iter()
        .zip(payloads)
        .map(|((label, view), payload)| {
            let ubo = create_params_ubo(counts, device, label, payload);
            BloomStage {
                params_bg: build_params_bg(device, layouts, &ubo, label),
                source_bg: build_source_bg(device, layouts, view, sampler, label),
                ubo,
                written: *payload,
            }
        })
        .collect();
    let composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bloom_composite_bg"),
        layout: &layouts.composite_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(mip_view(0)),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    BloomSlot {
        stages,
        composite_bg,
        mip_count,
    }
}

fn inv_size(size: (u32, u32)) -> (f32, f32) {
    let w = size.0.max(1) as f32;
    let h = size.1.max(1) as f32;
    (1.0 / w, 1.0 / h)
}

fn create_params_ubo(
    counts: &Counts,
    device: &wgpu::Device,
    label: &str,
    payload: &[u8; 256],
) -> wgpu::Buffer {
    counters::create_buffer_init(
        counts,
        device,
        &wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label}_ubo")),
            contents: payload,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        },
    )
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
    // The threshold and the downsamples replace their level whole; an
    // upsample adds into what its level holds.
    let target = match blend {
        BlendKind::Replace => whole_target(BLOOM_PYRAMID_FORMAT),
        BlendKind::Additive => wgpu::ColorTargetState {
            format: BLOOM_PYRAMID_FORMAT,
            blend: Some(wgpu::BlendState {
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
            write_mask: wgpu::ColorWrites::ALL,
        },
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
            targets: &[Some(target)],
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
    fragment_entry: &str,
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
            entry_point: Some(fragment_entry),
            targets: &[Some(whole_target(format))],
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
