//! M26 post-processing stack renderer.
//!
//! Holds one pipeline per stock effect, allocated once at `Renderer::new`.
//! `record` walks the `PostStack` and dispatches each pass against the
//! ping-pong ladder of `PostStackRenderer::plan_targets` (`D-058`).

use tungsten_core::assets::ShaderAssetId;
use tungsten_core::post::PostPass;
use tungsten_core::tween::UniformOverrideBlock;

use crate::passes::TargetId;
use crate::shader_hot_reload::ShaderModuleCache;
use crate::targets::{RenderTargetPool, TargetCache};

pub mod bloom;
pub mod fullscreen;
pub mod smaa;
pub mod smaa_luts;

use bloom::{BloomPipeline, BloomShaderIds};

/// The stock effects in `PostPass` order: the manifest ID of each one's shader
/// and its compiled-in source, which `assets/shaders/stock/<id>.wgsl` mirrors.
/// `Renderer::new` seeds the shader cache from this table, and a reload finds
/// an effect's pipeline by its ID (`D-091`).
pub(crate) const STOCK_SHADERS: [(&str, &str); 17] = [
    ("tonemap", include_str!("../shaders/stock/tonemap.wgsl")),
    ("vignette", include_str!("../shaders/stock/vignette.wgsl")),
    ("lut", include_str!("../shaders/stock/lut.wgsl")),
    (
        "chromatic_aberration",
        include_str!("../shaders/stock/chromatic_aberration.wgsl"),
    ),
    (
        "color_adjust",
        include_str!("../shaders/stock/color_adjust.wgsl"),
    ),
    ("tone_mono", include_str!("../shaders/stock/tone_mono.wgsl")),
    ("crt", include_str!("../shaders/stock/crt.wgsl")),
    (
        "film_grain",
        include_str!("../shaders/stock/film_grain.wgsl"),
    ),
    ("dither", include_str!("../shaders/stock/dither.wgsl")),
    (
        "pixel_outline",
        include_str!("../shaders/stock/pixel_outline.wgsl"),
    ),
    ("fade", include_str!("../shaders/stock/fade.wgsl")),
    (
        "wipe_radial",
        include_str!("../shaders/stock/wipe_radial.wgsl"),
    ),
    ("dissolve", include_str!("../shaders/stock/dissolve.wgsl")),
    ("glitch", include_str!("../shaders/stock/glitch.wgsl")),
    ("pixelate", include_str!("../shaders/stock/pixelate.wgsl")),
    ("fog", include_str!("../shaders/stock/fog.wgsl")),
    ("god_rays", include_str!("../shaders/stock/god_rays.wgsl")),
];

/// Row of `STOCK_SHADERS` that draws `pass`. Bloom has none: it owns four
/// stage shaders and records its own passes.
fn stock_index(pass: &PostPass) -> Option<usize> {
    Some(match pass {
        PostPass::Tonemap(_) => 0,
        PostPass::Vignette(_) => 1,
        PostPass::Lut(_) => 2,
        PostPass::ChromaticAberration(_) => 3,
        PostPass::ColorAdjust(_) => 4,
        PostPass::ToneMono(_) => 5,
        PostPass::Crt(_) => 6,
        PostPass::FilmGrain(_) => 7,
        PostPass::Dither(_) => 8,
        PostPass::PixelOutline(_) => 9,
        PostPass::Fade(_) => 10,
        PostPass::WipeRadial(_) => 11,
        PostPass::Dissolve(_) => 12,
        PostPass::Glitch(_) => 13,
        PostPass::Pixelate(_) => 14,
        PostPass::Fog(_) => 15,
        PostPass::GodRays(_) => 16,
        PostPass::Bloom(_) => return None,
    })
}

/// Shared resources every stock effect samples against: layouts, a linear
/// sampler for post passes, and a reusable source-bind-group factory.
pub(crate) struct StockResources {
    pub layouts: fullscreen::StockLayouts,
    pub sampler: wgpu::Sampler,
}

impl StockResources {
    pub fn new(device: &wgpu::Device) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post_sampler_linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            layouts: fullscreen::build_layouts(device),
            sampler,
        }
    }
}

/// One stock-effect pipeline. Its bind groups are not here. The source view
/// flips between `SceneColor`, `PostPing` and `PostPong` across the ping-pong
/// ladder, and every effect shares the three source groups that
/// `PostStackRenderer` caches. The params belong to the post-stack slot, not
/// to the effect (`ParamSlots`).
pub(crate) struct StockPipeline {
    pub pipeline: wgpu::RenderPipeline,
}

impl StockPipeline {
    pub fn new(
        device: &wgpu::Device,
        resources: &StockResources,
        label: &str,
        module: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            pipeline: fullscreen::build_pipeline_with_module(
                device,
                &resources.layouts,
                label,
                module,
                format,
            ),
        }
    }

    /// Hot-reload entry: swap in a pipeline built on a freshly validated
    /// module. The old pipeline stays until the new one exists, and stays for
    /// good when the new one fails validation (`Err`).
    fn rebuild_with_module(
        &mut self,
        device: &wgpu::Device,
        resources: &StockResources,
        label: &str,
        module: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        *self = crate::shader_hot_reload::build_validated(device, || {
            Self::new(device, resources, label, module, format)
        })?;
        Ok(())
    }
}

/// Params UBO and bind group of one post-stack slot.
pub(crate) struct StockParams {
    ubo: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// One slot's GPU objects and the bytes its buffer holds.
struct ParamSlot<T> {
    objects: T,
    written: Option<[u8; 256]>,
}

/// Params of the stock passes, one entry per post-stack slot (`D-090`).
///
/// Two passes of one effect sit in different slots, so each draws with a
/// buffer of its own: a buffer per effect took both writes before the frame
/// was submitted and gave both draws the second. Every stock effect shares
/// the params layout, so an entry serves whichever effect its slot holds.
/// Entries are built on first use and kept between frames; they hold no view
/// of the scene targets. `T` is the GPU objects, generic so the rules are
/// testable without a device. Material batches use the same slots, one set
/// per material (`D-101`).
pub(crate) struct ParamSlots<T> {
    slots: Vec<Option<ParamSlot<T>>>,
}

impl<T> Default for ParamSlots<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> ParamSlots<T> {
    /// The objects of `slot`, built by `build` on its first use, and whether
    /// `payload` differs from the bytes its buffer holds. The slot records
    /// `payload` as held, so the caller must write it when told to.
    pub(crate) fn stage(
        &mut self,
        slot: usize,
        payload: &[u8; 256],
        build: impl FnOnce() -> T,
    ) -> (&T, bool) {
        if self.slots.len() <= slot {
            self.slots.resize_with(slot + 1, || None);
        }
        let entry = self.slots[slot].get_or_insert_with(|| ParamSlot {
            objects: build(),
            written: None,
        });
        let stale = entry.written.as_ref() != Some(payload);
        if stale {
            entry.written = Some(*payload);
        }
        (&entry.objects, stale)
    }
}

/// Cache slot of a post-stack source target.
fn source_slot(src: TargetId) -> usize {
    match src {
        TargetId::SceneColor => 0,
        TargetId::PostPing => 1,
        TargetId::PostPong => 2,
        _ => unreachable!("invalid post-source target {src:?}"),
    }
}

/// Owns one live pipeline per stock effect variant.
pub struct PostStackRenderer {
    pub(crate) resources: StockResources,
    /// One pipeline per row of `STOCK_SHADERS`.
    stock: [StockPipeline; 17],
    /// M28 bloom pipeline. Records its own multi-subpass slot at the encoder
    /// level instead of the per-slot single-render-pass path; see `D-060`.
    pub(crate) bloom: BloomPipeline,
    /// Source bind groups for `SceneColor`, `PostPing` and `PostPong`, shared
    /// by every stock effect and keyed on the pool generation.
    source_bind_groups: [TargetCache<u64, wgpu::BindGroup>; 3],
    /// Params of the stock passes, per post-stack slot.
    params: ParamSlots<StockParams>,
}

impl PostStackRenderer {
    /// Build every pipeline from the shared `ShaderModuleCache`, which the
    /// renderer pre-seeds with the compile-time `include_str!` WGSL.
    /// `stock_shader_ids` are the cache IDs of the `STOCK_SHADERS` rows.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader_cache: &ShaderModuleCache,
        bloom_shader_ids: BloomShaderIds,
        stock_shader_ids: &[ShaderAssetId; 17],
    ) -> Self {
        let resources = StockResources::new(device);
        let stock = std::array::from_fn(|index| {
            let module = shader_cache
                .get(stock_shader_ids[index])
                .expect("stock post shader module must be pre-seeded");
            StockPipeline::new(device, &resources, STOCK_SHADERS[index].0, module, format)
        });
        Self {
            stock,
            bloom: BloomPipeline::new(device, format, shader_cache, bloom_shader_ids),
            resources,
            source_bind_groups: Default::default(),
            params: ParamSlots::default(),
        }
    }

    /// Drops every cached object that holds a view of the scene targets, so
    /// reallocated targets are freed at once and not at their next use.
    pub(crate) fn release_target_views(&mut self) {
        for cache in &mut self.source_bind_groups {
            cache.clear();
        }
        self.bloom.release_target_views();
    }

    fn pipeline_for(&self, pass: &PostPass) -> &StockPipeline {
        let Some(index) = stock_index(pass) else {
            unreachable!("PostPass::Bloom is recorded by record_bloom_slot_timed, not record_pass")
        };
        &self.stock[index]
    }

    /// Hot-reload entry: when `name` is a stock effect's shader, rebuild that
    /// effect's pipeline against a freshly validated module. Params buffers
    /// and bind groups stay; a pipeline that fails validation is not swapped
    /// in (`Err`). The caller commits the module to the cache after this
    /// returns `Ok`.
    pub(crate) fn rebuild_stock_with_module(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        name: &str,
        module: &wgpu::ShaderModule,
    ) -> Result<(), String> {
        if let Some(index) = STOCK_SHADERS.iter().position(|(stock, _)| *stock == name) {
            self.stock[index].rebuild_with_module(device, &self.resources, name, module, format)?;
        }
        Ok(())
    }

    /// Pack a `PostPass` into the shared 256-byte UBO layout. Slot
    /// assignments match each effect's WGSL comment header.
    fn pack(pass: &PostPass) -> UniformOverrideBlock {
        let mut block = UniformOverrideBlock::default();
        match pass {
            PostPass::Tonemap(p) => {
                block.f32s[0] = match p.mode {
                    tungsten_core::post::TonemapMode::Reinhard => 0.0,
                    tungsten_core::post::TonemapMode::AcesApprox => 1.0,
                    tungsten_core::post::TonemapMode::AcesFitted => 2.0,
                };
                block.f32s[1] = p.exposure;
                block.f32s[2] = p.white_point;
            }
            PostPass::Vignette(p) => {
                block.vec4[0] = p.color;
                block.f32s[0] = p.inner;
                block.f32s[1] = p.outer;
                block.f32s[2] = p.strength;
            }
            PostPass::Lut(p) => {
                block.f32s[0] = p.mix;
                block.i32s[0] = p.lut_sprite_id as i32;
            }
            PostPass::ChromaticAberration(strength) => {
                block.f32s[0] = *strength;
            }
            PostPass::ColorAdjust(p) => {
                block.f32s[0] = p.hue;
                block.f32s[1] = p.saturation;
                block.f32s[2] = p.contrast;
            }
            PostPass::ToneMono(p) => {
                block.vec4[0] = p.tint_a;
                block.vec4[1] = p.tint_b;
                block.f32s[0] = match p.mode {
                    tungsten_core::post::ToneMonoMode::Sepia => 0.0,
                    tungsten_core::post::ToneMonoMode::Mono => 1.0,
                    tungsten_core::post::ToneMonoMode::Duotone => 2.0,
                };
                block.f32s[1] = p.amount;
            }
            PostPass::Crt(p) => {
                block.f32s[0] = p.scanline_strength;
                block.f32s[1] = p.curvature;
                block.f32s[2] = p.mask as f32;
                block.f32s[3] = p.bleed;
            }
            PostPass::FilmGrain(p) => {
                block.f32s[0] = p.strength;
                block.f32s[1] = p.time_seed;
            }
            PostPass::Dither(p) => {
                block.f32s[0] = match p.mode {
                    tungsten_core::post::DitherMode::Bayer4 => 0.0,
                    tungsten_core::post::DitherMode::Bayer8 => 1.0,
                    tungsten_core::post::DitherMode::BlueNoise => 2.0,
                };
                block.f32s[1] = p.levels as f32;
                block.f32s[2] = p.strength;
            }
            PostPass::PixelOutline(p) => {
                block.vec4[0] = p.color;
                block.f32s[0] = p.thickness_px;
                block.f32s[1] = p.alpha_threshold;
            }
            PostPass::Fade(p) => {
                block.vec4[0] = p.color;
                block.f32s[0] = p.progress;
            }
            PostPass::WipeRadial(p) => {
                block.f32s[0] = p.progress;
                block.f32s[1] = p.softness;
                block.f32s[2] = p.center[0];
                block.f32s[3] = p.center[1];
            }
            PostPass::Dissolve(p) => {
                block.vec4[0] = p.edge_color;
                block.f32s[0] = p.progress;
                block.f32s[1] = p.noise_scale;
            }
            PostPass::Glitch(p) => {
                block.f32s[0] = p.block_strength;
                block.f32s[1] = p.shift_px;
                block.f32s[2] = p.time_seed;
            }
            PostPass::Pixelate(block_px) => {
                block.f32s[0] = *block_px;
            }
            PostPass::Fog(p) => {
                block.vec4[0] = p.color;
                block.f32s[0] = p.density;
                block.f32s[1] = p.height_falloff;
            }
            PostPass::GodRays(p) => {
                block.vec4[0] = [p.center[0], p.center[1], 0.0, 0.0];
                block.f32s[0] = p.density;
                block.f32s[1] = p.decay;
                block.f32s[2] = p.weight;
                block.f32s[3] = p.samples as f32;
            }
            // Bloom is multi-subpass; UBO packing happens per sub-pass inside
            // `BloomPipeline::record_pass_timed` via `bloom::pack_params`.
            PostPass::Bloom(_) => {}
        }
        block
    }

    /// Plan the (src, dst) ladder for a stack of `len` passes. First pass
    /// samples `SceneColor`, subsequent passes alternate between ping/pong.
    /// Each entry's `src` is the previous `dst`. Even dst = PostPing.
    #[must_use]
    pub fn plan_targets(len: usize) -> Vec<(TargetId, TargetId)> {
        let mut out = Vec::with_capacity(len);
        for i in 0..len {
            let dst = if i.is_multiple_of(2) {
                TargetId::PostPing
            } else {
                TargetId::PostPong
            };
            let src = if i == 0 {
                TargetId::SceneColor
            } else if (i - 1).is_multiple_of(2) {
                TargetId::PostPing
            } else {
                TargetId::PostPong
            };
            out.push((src, dst));
        }
        out
    }

    /// Record one post-stack pass into an already-open `render_pass`. The
    /// caller has selected the correct dst view via `PassRecorder::begin`
    /// with the matching `PassDesc`; we upload params if they changed, bind
    /// the cached source group, set pipeline, and draw. `slot` is the pass's
    /// index in the post stack: each slot keeps a params buffer of its own.
    #[allow(clippy::too_many_arguments)]
    pub fn record_pass(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        pool: &RenderTargetPool,
        pass: &PostPass,
        src: TargetId,
        slot: usize,
    ) {
        let payload = Self::pack(pass).to_bytes();
        let src_view = match src {
            TargetId::SceneColor => pool.scene.color_view(),
            TargetId::PostPing => pool.scene.post_ping_view(),
            TargetId::PostPong => pool.scene.post_pong_view(),
            _ => unreachable!("invalid post-source target {src:?}"),
        };
        let resources = &self.resources;
        let source_bg = self.source_bind_groups[source_slot(src)]
            .get_or_build(pool.generation(), || {
                fullscreen::build_source_bind_group(
                    device,
                    &resources.layouts,
                    "post",
                    src_view,
                    &resources.sampler,
                )
            })
            .clone();
        // The write is skipped when the slot's buffer already holds the bytes.
        let (params, stale) = self.params.stage(slot, &payload, || {
            let label = format!("post_slot{slot}");
            let ubo = fullscreen::build_params_ubo(device, &label);
            let bind_group =
                fullscreen::build_params_bind_group(device, &resources.layouts, &label, &ubo);
            StockParams { ubo, bind_group }
        });
        if stale {
            queue.write_buffer(&params.ubo, 0, &payload);
        }
        let params_bg = params.bind_group.clone();
        render_pass.set_pipeline(&self.pipeline_for(pass).pipeline);
        render_pass.set_bind_group(0, &source_bg, &[]);
        render_pass.set_bind_group(1, &params_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Record a `PostPass::Bloom` slot at encoder level. Unlike `record_pass`,
    /// this opens its own per-subpass `RenderPass`es (threshold, downsample
    /// chain, additive upsample chain, composite); the renderer's outer slot
    /// `PassDesc` is treated as a debug-only label. `slot` is the pass's
    /// index in the post stack: each slot keeps GPU objects of its own. The
    /// composite writes the frame's swapchain view when `dst` is
    /// [`TargetId::Swapchain`]; `timing` records the sub-passes.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_bloom_slot_timed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        pool: &RenderTargetPool,
        swap_view: &wgpu::TextureView,
        params: &tungsten_core::post::BloomParams,
        src: TargetId,
        dst: TargetId,
        timing: Option<&mut crate::timing::TimingResources>,
        slot: usize,
    ) {
        self.bloom.record_pass_timed(
            device,
            queue,
            encoder,
            pool,
            Some(swap_view),
            params,
            src,
            dst,
            timing,
            slot,
        );
    }
}

#[cfg(test)]
#[path = "../tests/post.rs"]
mod tests;
