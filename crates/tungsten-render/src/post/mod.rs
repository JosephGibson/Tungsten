//! M26 post-processing stack renderer.
//!
//! Holds one pipeline per stock effect, allocated once at `Renderer::new`.
//! `record` walks the `PostStack` and dispatches each pass against the
//! ping-pong ladder described in the M26 plan's "Scene → Post → Present
//! Target Flow" table.

use tungsten_core::post::PostPass;
use tungsten_core::tween::UniformOverrideBlock;

use crate::passes::TargetId;
use crate::targets::RenderTargetPool;

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
pub mod tone_mono;
pub mod tonemap;
pub mod vignette;
pub mod wipe_radial;

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

/// One stock-effect pipeline + its params UBO + its params bind group.
/// Source bind group is rebuilt each frame because the source view flips
/// between `SceneColor`, `PostPing`, `PostPong` across the ping-pong ladder.
pub(crate) struct StockPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub params_ubo: wgpu::Buffer,
    pub params_bg: wgpu::BindGroup,
}

impl StockPipeline {
    pub fn new(
        device: &wgpu::Device,
        resources: &StockResources,
        label: &str,
        wgsl: &str,
        format: wgpu::TextureFormat,
    ) -> Self {
        let pipeline = fullscreen::build_pipeline(device, &resources.layouts, label, wgsl, format);
        let params_ubo = fullscreen::build_params_ubo(device, label);
        let params_bg =
            fullscreen::build_params_bind_group(device, &resources.layouts, label, &params_ubo);
        Self {
            pipeline,
            params_ubo,
            params_bg,
        }
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
}

impl PostStackRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
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
            resources,
        }
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
            let dst = if i % 2 == 0 {
                TargetId::PostPing
            } else {
                TargetId::PostPong
            };
            let src = if i == 0 {
                TargetId::SceneColor
            } else if (i - 1) % 2 == 0 {
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
        Some(if (len - 1) % 2 == 0 {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        })
    }

    /// Record one post-stack pass into an already-open `render_pass`. The
    /// caller has selected the correct dst view via `PassRecorder::begin`
    /// with the matching `PassDesc`; we just upload params, bind source,
    /// set pipeline, and draw.
    pub fn record_pass(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        pool: &RenderTargetPool,
        pass: &PostPass,
        src: TargetId,
    ) {
        let pipeline = self.pipeline_for(pass);
        let payload = Self::pack(pass);
        queue.write_buffer(&pipeline.params_ubo, 0, &payload.to_bytes());
        let src_view = match src {
            TargetId::SceneColor => pool.scene.color_view(),
            TargetId::PostPing => pool.scene.post_ping_view(),
            TargetId::PostPong => pool.scene.post_pong_view(),
            _ => unreachable!("invalid post-source target {src:?}"),
        };
        let source_bg = fullscreen::build_source_bind_group(
            device,
            &self.resources.layouts,
            pass.kind_name(),
            src_view,
            &self.resources.sampler,
        );
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &source_bg, &[]);
        render_pass.set_bind_group(1, &pipeline.params_bg, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
#[path = "../tests/post.rs"]
mod tests;
