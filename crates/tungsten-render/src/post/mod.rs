//! M26 post-processing stack renderer.
//!
//! Holds one pipeline per stock effect, allocated once at `Renderer::new`.
//! `record` walks the `PostStack` and dispatches each pass against the
//! ping-pong ladder described in the M26 plan's "Scene → Post → Present
//! Target Flow" table.

use tungsten_core::post::PostPass;
use tungsten_core::tween::UniformOverrideBlock;

use crate::passes::TargetId;
use crate::shader_hot_reload::ShaderModuleCache;
use crate::targets::{RenderTargetPool, TargetCache};

pub mod bloom;
pub mod chromatic_aberration;
pub mod color_adjust;
pub mod crt;
pub mod dissolve;
pub mod dither;
pub mod fade;
pub mod film_grain;
pub mod fog;
pub mod fullscreen;
pub mod glitch;
pub mod god_rays;
pub mod lut;
pub mod pixel_outline;
pub mod pixelate;
pub mod smaa;
pub mod smaa_luts;
pub mod tone_mono;
pub mod tonemap;
pub mod vignette;
pub mod wipe_radial;

use bloom::{BloomPipeline, BloomShaderIds};

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
        wgsl: &str,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            pipeline: fullscreen::build_pipeline(device, &resources.layouts, label, wgsl, format),
        }
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
/// testable without a device.
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
    pub(crate) tonemap: StockPipeline,
    pub(crate) vignette: StockPipeline,
    pub(crate) lut: StockPipeline,
    pub(crate) chromatic_aberration: StockPipeline,
    pub(crate) color_adjust: StockPipeline,
    pub(crate) tone_mono: StockPipeline,
    pub(crate) crt: StockPipeline,
    pub(crate) film_grain: StockPipeline,
    pub(crate) dither: StockPipeline,
    pub(crate) pixel_outline: StockPipeline,
    pub(crate) fade: StockPipeline,
    pub(crate) wipe_radial: StockPipeline,
    pub(crate) dissolve: StockPipeline,
    pub(crate) glitch: StockPipeline,
    pub(crate) pixelate: StockPipeline,
    pub(crate) fog: StockPipeline,
    pub(crate) god_rays: StockPipeline,
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
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        shader_cache: &ShaderModuleCache,
        bloom_shader_ids: BloomShaderIds,
    ) -> Self {
        let resources = StockResources::new(device);
        Self {
            tonemap: tonemap::build(device, &resources, format),
            vignette: vignette::build(device, &resources, format),
            lut: lut::build(device, &resources, format),
            chromatic_aberration: chromatic_aberration::build(device, &resources, format),
            color_adjust: color_adjust::build(device, &resources, format),
            tone_mono: tone_mono::build(device, &resources, format),
            crt: crt::build(device, &resources, format),
            film_grain: film_grain::build(device, &resources, format),
            dither: dither::build(device, &resources, format),
            pixel_outline: pixel_outline::build(device, &resources, format),
            fade: fade::build(device, &resources, format),
            wipe_radial: wipe_radial::build(device, &resources, format),
            dissolve: dissolve::build(device, &resources, format),
            glitch: glitch::build(device, &resources, format),
            pixelate: pixelate::build(device, &resources, format),
            fog: fog::build(device, &resources, format),
            god_rays: god_rays::build(device, &resources, format),
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
        match pass {
            PostPass::Tonemap(_) => &self.tonemap,
            PostPass::Vignette(_) => &self.vignette,
            PostPass::Lut(_) => &self.lut,
            PostPass::ChromaticAberration(_) => &self.chromatic_aberration,
            PostPass::ColorAdjust(_) => &self.color_adjust,
            PostPass::ToneMono(_) => &self.tone_mono,
            PostPass::Crt(_) => &self.crt,
            PostPass::FilmGrain(_) => &self.film_grain,
            PostPass::Dither(_) => &self.dither,
            PostPass::PixelOutline(_) => &self.pixel_outline,
            PostPass::Fade(_) => &self.fade,
            PostPass::WipeRadial(_) => &self.wipe_radial,
            PostPass::Dissolve(_) => &self.dissolve,
            PostPass::Glitch(_) => &self.glitch,
            PostPass::Pixelate(_) => &self.pixelate,
            PostPass::Fog(_) => &self.fog,
            PostPass::GodRays(_) => &self.god_rays,
            PostPass::Bloom(_) => {
                unreachable!("PostPass::Bloom is recorded by record_bloom_slot, not record_pass")
            }
        }
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
            // `BloomPipeline::record_pass` via `bloom::pack_params`.
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

    /// Returns the final post-target id (i.e. the one the present blit
    /// samples from) when the stack is non-empty. `None` if empty.
    #[must_use]
    pub fn final_target(len: usize) -> Option<TargetId> {
        if len == 0 {
            return None;
        }
        Some(if (len - 1).is_multiple_of(2) {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        })
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
    /// index in the post stack: each slot keeps GPU objects of its own.
    #[allow(clippy::too_many_arguments)]
    pub fn record_bloom_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        pool: &RenderTargetPool,
        params: &tungsten_core::post::BloomParams,
        src: TargetId,
        dst: TargetId,
        slot: usize,
    ) {
        self.bloom
            .record_pass(device, queue, encoder, pool, params, src, dst, slot);
    }
    /// As [`record_bloom_slot`](Self::record_bloom_slot), with pass timing and
    /// the frame's swapchain view, which the composite writes when `dst` is
    /// [`TargetId::Swapchain`].
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
