//! M26 post-processing stack renderer.
//!
//! Holds one pipeline per stock effect, allocated once at `Renderer::new`.
//! The frame's graph declares a pass a stack slot, each reading the version
//! of the frame's colour before it (`D-058`); `record_pass` draws one. A
//! stack's last two stages draw in one pass where the composer's rule joins
//! them (`compose`, `D-307`); `record_fused` draws that one.

use std::collections::HashMap;

use tungsten_core::assets::{PostEffectId, ShaderAssetId};
use tungsten_core::post::PostPass;
use tungsten_core::tween::UniformOverrideBlock;

use crate::shader_hot_reload::ShaderModuleCache;

pub mod bloom;
pub(crate) mod compose;
pub mod fullscreen;
pub mod smaa;
pub mod smaa_luts;

use crate::counters::{self, Counts};
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

/// The class of each stock effect's `@fx` section, by its row of
/// [`STOCK_SHADERS`]: what the composer's rule reads for a stage on the
/// engine's own text, without parsing it a frame. A test holds each file's
/// marker to this table.
pub(crate) const STOCK_CLASSES: [compose::Class; 17] = {
    use compose::Class::{Head, Mix, Pixel, Steps};
    [
        Pixel, // tonemap
        Mix,   // vignette
        Pixel, // lut
        Head,  // chromatic_aberration
        Pixel, // color_adjust
        Pixel, // tone_mono
        Head,  // crt
        Pixel, // film_grain
        Steps, // dither
        Head,  // pixel_outline
        Mix,   // fade
        Mix,   // wipe_radial
        Mix,   // dissolve
        Head,  // glitch
        Head,  // pixelate
        Mix,   // fog
        Head,  // god_rays
    ]
};

/// The stock materials' shaders (redo 7.9), compiled in as the stock post
/// shaders are and mirrored in `assets/shaders/stock/`: `Renderer::new`
/// seeds them after `STOCK_SHADERS`, and the umbrella binds `glow` to the
/// reserved material `engine_glow` and `damage_flash` to the kit's hit
/// flash (redo 7.11), unless a manifest declares that material.
pub(crate) const STOCK_MATERIAL_SHADERS: [(&str, &str); 2] = [
    ("glow", include_str!("../shaders/stock/glow.wgsl")),
    (
        "damage_flash",
        include_str!("../shaders/stock/damage_flash.wgsl"),
    ),
];

/// Row of `STOCK_SHADERS` that draws `pass`. Bloom has none: it owns four
/// stage shaders and records its own passes. A game's effect has none: its
/// pipeline is its own (`D-308`).
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
        PostPass::Bloom(_) | PostPass::Custom(_) => return None,
    })
}

/// The shader that draws stock pass `pass`, by its `STOCK_SHADERS` name;
/// `None` for bloom, whose four stages are its own, and for a game's effect.
pub(crate) fn stock_shader_name(pass: &PostPass) -> Option<&'static str> {
    stock_index(pass).map(|index| STOCK_SHADERS[index].0)
}

/// The colour target of a pass the frame's graph may take for a whole
/// write: no blend and every channel written, so the pass's one triangle
/// replaces every pixel its target held (`D-306`).
pub(crate) fn whole_target(format: wgpu::TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    }
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

/// One stock-effect pipeline. Its bind groups are not here. The source is
/// whichever pooled texture holds the version of the frame's colour before
/// the pass, and every effect shares the source groups that
/// `PostStackRenderer` caches by texture. The params belong to the
/// post-stack slot, not to the effect (`ParamSlots`).
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

/// Bind groups that sample a pooled texture, by the texture's identity. A
/// frame's post passes read two or three textures, so a short list serves.
pub(crate) struct SourceGroups<T> {
    groups: Vec<(u64, T)>,
}

impl<T> Default for SourceGroups<T> {
    fn default() -> Self {
        Self { groups: Vec::new() }
    }
}

impl<T> SourceGroups<T> {
    /// The group over the texture `id`, built by `build` on its first use.
    pub(crate) fn get_or_build(&mut self, id: u64, build: impl FnOnce() -> T) -> &T {
        let held = self.groups.iter().position(|(held, _)| *held == id);
        let index = held.unwrap_or_else(|| {
            self.groups.push((id, build()));
            self.groups.len() - 1
        });
        &self.groups[index].1
    }

    /// Drops the groups over the textures `freed`, and with them their
    /// views.
    pub(crate) fn forget(&mut self, freed: &[u64]) {
        self.groups.retain(|(id, _)| !freed.contains(id));
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.groups.len()
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
    /// Source bind groups over the pooled textures the stack's passes
    /// sample, shared by every stock effect.
    source_bind_groups: SourceGroups<wgpu::BindGroup>,
    /// Params of the stock passes, per post-stack slot.
    params: ParamSlots<StockParams>,
    /// The [`InertParam`]s already warned about, one bit each.
    inert_warned: u8,
    /// The format every post pipeline draws into.
    format: wgpu::TextureFormat,
    /// The pipelines of fused passes, by what each was generated from
    /// (`D-307`).
    fused: compose::Fused<Signature, wgpu::RenderPipeline>,
    /// The blocks of each fused pass, by the slot of its first stage.
    fused_params: Vec<Option<FusedParams>>,
    /// The fused pass being recorded: its stages' blocks in a row. Kept for
    /// its allocation.
    fused_payload: Vec<u8>,
    /// The game's own effects, by ID (`D-308`).
    custom: HashMap<PostEffectId, CustomEffect>,
    /// The effect IDs no load knows that a frame has already named.
    unknown_warned: Vec<PostEffectId>,
    counts: Box<Counts>,
}

/// What draws one stage of a fused pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    /// A stock effect, by its row of [`STOCK_SHADERS`].
    Stock(u8),
    /// Bloom's composite, which only ever leads.
    Composite,
    /// A game's own effect (`D-308`).
    Custom(PostEffectId),
}

impl Effect {
    fn of(pass: &PostPass) -> Self {
        match (stock_index(pass), pass) {
            (Some(row), _) => Self::Stock(row as u8),
            (None, PostPass::Custom(custom)) => Self::Custom(custom.effect),
            (None, _) => Self::Composite,
        }
    }

    /// The name the effect's stage goes by in a pass's label: a stock
    /// effect's, the composite's, or `custom` for any game's effect.
    fn label(self) -> &'static str {
        match self {
            Self::Stock(row) => STOCK_SHADERS[usize::from(row)].0,
            Self::Composite => bloom::BLOOM_COMPOSITE_SECTION,
            Self::Custom(_) => "custom",
        }
    }
}

/// The stages a fused pass draws, in order, and what joins them: what its
/// module and pipeline are generated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Signature {
    effects: [Effect; compose::MAX_STAGES],
    len: u8,
    between: compose::Between,
}

impl Signature {
    /// The signature of the pass that draws `stages`; `None` for more
    /// stages than a pass holds or a bloom that does not lead.
    pub(crate) fn of(stages: &[PostPass], between: compose::Between) -> Option<Self> {
        if stages.is_empty() || stages.len() > compose::MAX_STAGES {
            return None;
        }
        let mut effects = [Effect::Composite; compose::MAX_STAGES];
        for (index, stage) in stages.iter().enumerate() {
            let effect = Effect::of(stage);
            if effect == Effect::Composite && index != 0 {
                return None;
            }
            effects[index] = effect;
        }
        Some(Self {
            effects,
            len: stages.len() as u8,
            between,
        })
    }

    fn effects(&self) -> &[Effect] {
        &self.effects[..usize::from(self.len)]
    }

    /// Whether bloom's composite leads the pass, which then binds the
    /// pyramid beside the source.
    fn led_by_composite(&self) -> bool {
        self.effects[0] == Effect::Composite
    }

    /// The names of the pass's stages, as its label holds them.
    fn names(&self) -> Vec<&'static str> {
        self.effects().iter().map(|effect| effect.label()).collect()
    }

    /// Whether the pass draws the engine effect whose shader is `name`.
    pub(crate) fn holds(&self, name: &str) -> bool {
        self.effects().iter().any(|effect| match effect {
            Effect::Stock(row) => STOCK_SHADERS[usize::from(*row)].0 == name,
            Effect::Composite => name == bloom::BLOOM_COMPOSITE_SHADER_NAME,
            Effect::Custom(_) => false,
        })
    }

    /// Whether the pass draws the game's effect `effect`.
    fn holds_custom(&self, effect: PostEffectId) -> bool {
        self.effects().contains(&Effect::Custom(effect))
    }
}

/// The section of an effect: an engine effect's from its compiled-in text,
/// the only text its stage of a fused pass draws with, since a replaced
/// file's stage draws alone; a game's as its shader's load read it.
fn effect_section(
    effect: Effect,
    custom: &HashMap<PostEffectId, CustomEffect>,
) -> Result<compose::Section, String> {
    let (name, class, source) = match effect {
        Effect::Stock(row) => {
            let row = usize::from(row);
            (
                STOCK_SHADERS[row].0,
                STOCK_CLASSES[row],
                STOCK_SHADERS[row].1,
            )
        }
        Effect::Composite => (
            bloom::BLOOM_COMPOSITE_SECTION,
            compose::Class::Head,
            bloom::BLOOM_SHADERS[3].1,
        ),
        Effect::Custom(id) => {
            return custom
                .get(&id)
                .and_then(|effect| effect.section.clone())
                .ok_or_else(|| "a game's effect without a section".to_string());
        }
    };
    let (_, text) = compose::section_text(source)?;
    Ok(compose::Section {
        name: name.to_string(),
        class,
        text: text.to_string(),
    })
}

/// A game's own post effect (`D-308`): a manifest `post_effects` entry.
pub(crate) struct CustomEffect {
    /// Its manifest ID, which its section's names start with.
    name: String,
    /// The `shaders` ID of its module.
    shader_name: String,
    /// Its pipeline on the stock effects' layouts; `None` until its module
    /// builds on them, and its stage is left out of the frame meanwhile.
    pipeline: Option<wgpu::RenderPipeline>,
    /// Its `@fx` section, when its module marks a valid one: without one
    /// its stage always draws alone.
    section: Option<compose::Section>,
    /// Whether a frame has logged that it leaves the effect out.
    warned: bool,
}

/// The section of a game's effect `name` in its module's text `wgsl`: `None`
/// for a module that marks none, and, with the reason logged, for one whose
/// section is refused or whose name is an engine effect's, since a fused
/// module holds each name's section once.
pub(crate) fn custom_section(name: &str, wgsl: &str) -> Option<compose::Section> {
    if !wgsl.contains("// @fx begin") {
        return None;
    }
    // A fused pass draws the section on the engine's prefix: a module on
    // another one (its block's lanes in another order, say) would draw one
    // thing alone and another fused.
    if !wgsl.contains(compose::prefix()) {
        log::warn!(
            "post effect '{name}': its module does not open with the stock post prefix byte for byte, so its `@fx` section is not read and it draws in a pass of its own"
        );
        return None;
    }
    let taken = name == bloom::BLOOM_COMPOSITE_SECTION
        || STOCK_SHADERS.iter().any(|(stock, _)| *stock == name);
    if taken {
        log::warn!(
            "post effect '{name}': an engine effect has that name, so its `@fx` section is not read and it draws in a pass of its own"
        );
        return None;
    }
    match compose::read_section(name, wgsl) {
        Ok(section) => Some(section),
        Err(reason) => {
            log::warn!(
                "post effect '{name}': its `@fx` section is refused and it draws in a pass of its own: {reason}"
            );
            None
        }
    }
}

/// Why the module `wgsl` cannot draw as a post effect though its pipeline
/// builds: the uniform block it reads at group 1, binding 0 is larger than
/// the 256 bytes a pass binds there. The layout sets no least size, so wgpu
/// would refuse the draw, not the pipeline, and a refused draw is a panic.
pub(crate) fn block_fault(wgsl: &str) -> Option<String> {
    use wgpu::naga;
    let module = naga::front::wgsl::parse_str(wgsl).ok()?;
    let block = module.global_variables.iter().find_map(|(_, variable)| {
        let binding = variable.binding.as_ref()?;
        (binding.group == 1 && binding.binding == 0).then_some(variable.ty)
    })?;
    let size = module.types[block].inner.size(module.to_ctx());
    (size > 256)
        .then(|| format!("its block at group 1, binding 0 is {size} bytes, and a pass binds 256"))
}

/// `stack` without the passes `draws` refuses, into `drawn`: the stack a
/// frame draws when a game's effect has no pipeline.
pub(crate) fn drawn_stack(
    stack: &[PostPass],
    draws: impl Fn(&PostPass) -> bool,
    drawn: &mut Vec<PostPass>,
) {
    drawn.clear();
    drawn.extend(stack.iter().filter(|pass| draws(pass)).copied());
}

/// The class of the section of the stock effect that draws `pass`; `None`
/// for bloom, whose composite leads a pass.
pub(crate) fn stock_class(pass: &PostPass) -> Option<compose::Class> {
    stock_index(pass).map(|row| STOCK_CLASSES[row])
}

/// The stage that draws `pass` as the composer's rule reads it. `own` is
/// whether its shader, bloom's composite for a bloom, is the engine's
/// compiled-in text: a replaced file's stage has no class and draws alone.
pub(crate) fn stage_info(pass: &PostPass, own: bool) -> compose::StageInfo {
    let class = stock_class(pass).unwrap_or(compose::Class::Head);
    compose::StageInfo {
        class: (own && !matches!(pass, PostPass::Custom(_))).then_some(class),
        bounded: compose::bounded(pass),
    }
}

/// A fused pass's blocks: one buffer of 256 bytes a stage and the bind
/// group over it.
struct FusedParams {
    ubo: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Whether the group is on bloom's layout, for a pass its composite
    /// leads.
    composite: bool,
    /// The bytes last written to `ubo`.
    written: Vec<u8>,
}

/// A stock effect's setting that draws nothing, or not as named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InertParam {
    /// `DitherMode::Bayer8` and `BlueNoise` draw as `Bayer4`.
    DitherMode,
    /// `CrtParams::bleed` is not read by the shader.
    CrtBleed,
    /// `PixelOutline` outlines where the scene's alpha crosses its threshold;
    /// a clear alpha at or above it leaves no such edge.
    OutlineClearAlpha,
}

impl InertParam {
    fn message(self) -> &'static str {
        match self {
            Self::DitherMode => "dither: `bayer8` and `blue_noise` draw as `bayer4`",
            Self::CrtBleed => "crt: `bleed` does nothing yet",
            Self::OutlineClearAlpha => {
                "pixel_outline: the clear colour's alpha is at or above `alpha_threshold`, so nothing is outlined; clear with a lower alpha"
            }
        }
    }
}

/// The setting of `pass` that draws nothing or not as named, if any, with the
/// scene cleared at `clear_alpha`.
pub(crate) fn inert_param(pass: &PostPass, clear_alpha: f32) -> Option<InertParam> {
    use tungsten_core::post::DitherMode;
    match pass {
        PostPass::Dither(p) if p.mode != DitherMode::Bayer4 => Some(InertParam::DitherMode),
        PostPass::Crt(p) if p.bleed != 0.0 => Some(InertParam::CrtBleed),
        PostPass::PixelOutline(p) if clear_alpha >= p.alpha_threshold => {
            Some(InertParam::OutlineClearAlpha)
        }
        _ => None,
    }
}

impl PostStackRenderer {
    /// Adds the stack's draw and upload counts, bloom's included, to `total`.
    pub(crate) fn take_counts(&self, total: &mut crate::FrameCounters) {
        self.counts.take_into(total);
        self.bloom.counts.take_into(total);
    }

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
            source_bind_groups: SourceGroups::default(),
            params: ParamSlots::default(),
            inert_warned: 0,
            format,
            fused: compose::Fused::default(),
            fused_params: Vec::new(),
            fused_payload: Vec::new(),
            custom: HashMap::new(),
            unknown_warned: Vec::new(),
            counts: Counts::boxed(),
        }
    }

    /// Whether the pass that draws `stages` in one draw is built, building
    /// it at its signature's first use (`D-307`). A build that fails is
    /// logged and held as failed, so the stages draw alone and nothing is
    /// built again until a member's file reloads.
    pub(crate) fn fused_ready(
        &mut self,
        device: &wgpu::Device,
        stages: &[PostPass],
        between: compose::Between,
    ) -> bool {
        let Some(signature) = Signature::of(stages, between) else {
            return false;
        };
        let (resources, bloom, format) = (&self.resources, &self.bloom, self.format);
        let custom = &self.custom;
        self.fused
            .get_or_build(signature, || {
                build_fused(device, resources, bloom, custom, format, signature)
            })
            .is_some()
    }

    /// What the composer's rule reads of `pass`: an engine effect's class
    /// while its text is the engine's `own`, a game's effect's by the
    /// section its shader's load read.
    pub(crate) fn stage_info(&self, pass: &PostPass, own: bool) -> compose::StageInfo {
        match pass {
            PostPass::Custom(custom) => compose::StageInfo {
                class: self
                    .custom
                    .get(&custom.effect)
                    .and_then(|effect| effect.section.as_ref())
                    .map(|section| section.class),
                bounded: true,
            },
            _ => stage_info(pass, own),
        }
    }

    /// Makes game effect `id` known, with nothing to draw it yet: its
    /// shader has no module, and builds the effect when one loads.
    pub(crate) fn register_custom(&mut self, id: PostEffectId, name: &str, shader_name: &str) {
        let effect = self.custom.entry(id).or_insert_with(|| CustomEffect {
            name: String::new(),
            shader_name: String::new(),
            pipeline: None,
            section: None,
            warned: false,
        });
        effect.name = name.to_string();
        effect.shader_name = shader_name.to_string();
    }

    /// Builds game effect `id`'s pipeline from its shader's `module` on the
    /// stock effects' layouts, inside a validation scope, and reads its
    /// section from the module's text `wgsl` (`D-308`). A module that does
    /// not build is `Err` with wgpu's report, and the effect keeps the
    /// pipeline it had, or none. Every fused pass that holds the effect is
    /// generated again at its next use.
    pub(crate) fn upload_custom(
        &mut self,
        device: &wgpu::Device,
        id: PostEffectId,
        name: &str,
        shader_name: &str,
        wgsl: &str,
        module: &wgpu::ShaderModule,
    ) -> Result<(), String> {
        self.register_custom(id, name, shader_name);
        self.fused.forget(|signature| signature.holds_custom(id));
        if let Some(fault) = block_fault(wgsl) {
            return Err(fault);
        }
        let label = format!("post_custom_{name}");
        let (layouts, format) = (&self.resources.layouts, self.format);
        let pipeline = crate::shader_hot_reload::build_validated(device, || {
            fullscreen::build_pipeline_with_module(device, layouts, &label, module, format)
        })?;
        let section = custom_section(name, wgsl);
        if let Some(effect) = self.custom.get_mut(&id) {
            effect.section = section;
            effect.pipeline = Some(pipeline);
            effect.warned = false;
        }
        Ok(())
    }

    /// The game effects whose module is shader `shader_name`, each with its
    /// manifest name: those a reload of that shader builds again.
    pub(crate) fn custom_on_shader(&self, shader_name: &str) -> Vec<(PostEffectId, String)> {
        let mut effects: Vec<(PostEffectId, String)> = self
            .custom
            .iter()
            .filter(|(_, effect)| effect.shader_name == shader_name)
            .map(|(id, effect)| (*id, effect.name.clone()))
            .collect();
        effects.sort_by_key(|(id, _)| id.0);
        effects
    }

    /// Whether `pass` has a pipeline to draw it: every engine pass, and a
    /// game's effect whose module built.
    pub(crate) fn draws(&self, pass: &PostPass) -> bool {
        match pass {
            PostPass::Custom(custom) => self
                .custom
                .get(&custom.effect)
                .is_some_and(|effect| effect.pipeline.is_some()),
            _ => true,
        }
    }

    /// Logs, once an effect, each game effect in `stack` that a frame leaves
    /// out for want of a pipeline.
    pub(crate) fn warn_left_out(&mut self, stack: &[PostPass]) {
        for pass in stack {
            let PostPass::Custom(custom) = pass else {
                continue;
            };
            match self.custom.get_mut(&custom.effect) {
                Some(effect) if effect.pipeline.is_none() && !effect.warned => {
                    effect.warned = true;
                    log::error!(
                        "post effect '{}' has no pipeline (its shader '{}' did not build as a post effect): the frame leaves its pass out",
                        effect.name,
                        effect.shader_name
                    );
                }
                Some(_) => {}
                None => {
                    if !self.unknown_warned.contains(&custom.effect) {
                        self.unknown_warned.push(custom.effect);
                        log::error!(
                            "post effect {:?} is no effect a manifest loaded: the frame leaves its pass out",
                            custom.effect
                        );
                    }
                }
            }
        }
    }

    /// Drops every fused pass that draws the engine effect whose shader is
    /// `name`, whose file reloaded: the pass is built again at its next use,
    /// which gives one whose build failed another try. An engine effect's
    /// section is read from the compiled-in text either way: while its file
    /// is a replacement its stage draws alone, and no pass holds it. A
    /// game's effect drops its own passes as its shader loads
    /// ([`Self::upload_custom`]), where its section is read again.
    pub(crate) fn forget_fused(&mut self, name: &str) {
        self.fused.forget(|signature| signature.holds(name));
    }

    /// Draws the fused pass of `stages`, the first in post-stack slot
    /// `slot`, into `render_pass`: one draw with a block a stage. A pass a
    /// stock effect leads samples `source`, a pooled texture with its
    /// identity; a pass bloom's composite leads binds the groups the slot's
    /// prepare made, and `source` is not read. Returns whether it drew:
    /// `false` when the pass is not built or its slot was not prepared,
    /// which the frame's plan rules out.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_fused(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        source: Option<(u64, &wgpu::TextureView)>,
        stages: &[PostPass],
        slot: usize,
        between: compose::Between,
    ) -> bool {
        let Some(signature) = Signature::of(stages, between) else {
            return false;
        };
        let composite = signature.led_by_composite();
        // The stages' blocks in a row: the composite's as its slot wrote
        // it, each stock stage's as it packs alone.
        let payload = &mut self.fused_payload;
        payload.clear();
        for (index, stage) in stages.iter().enumerate() {
            if index == 0 && composite {
                let Some((_, _, block)) = self.bloom.composite_parts(slot) else {
                    return false;
                };
                payload.extend_from_slice(block);
            } else {
                payload.extend_from_slice(&Self::pack(stage).to_bytes());
            }
        }

        if self.fused_params.len() <= slot {
            self.fused_params.resize_with(slot + 1, || None);
        }
        let held = &mut self.fused_params[slot];
        if held
            .as_ref()
            .is_none_or(|params| params.composite != composite)
        {
            let label = format!("post_fused_slot{slot}");
            let ubo = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&label),
                size: (compose::MAX_STAGES * 256) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let layout = if composite {
                self.bloom.composite_layouts()[1]
            } else {
                &self.resources.layouts.params_bgl
            };
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&label),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubo.as_entire_binding(),
                }],
            });
            *held = Some(FusedParams {
                ubo,
                bind_group,
                composite,
                written: Vec::new(),
            });
        }
        let Some(params) = held.as_mut() else {
            return false;
        };
        // The write is skipped when the buffer already holds the bytes.
        if params.written != *payload {
            counters::write_buffer(&self.counts, queue, &params.ubo, 0, payload);
            params.written.clone_from(payload);
        }
        let params_bg = params.bind_group.clone();

        let Some(pipeline) = self.fused.get(&signature) else {
            return false;
        };
        render_pass.set_pipeline(pipeline);
        if composite {
            let Some((source_bg, pyramid_bg, _)) = self.bloom.composite_parts(slot) else {
                return false;
            };
            render_pass.set_bind_group(0, source_bg, &[]);
            render_pass.set_bind_group(1, &params_bg, &[]);
            render_pass.set_bind_group(2, pyramid_bg, &[]);
        } else {
            let Some(source) = source else {
                return false;
            };
            let resources = &self.resources;
            let source_bg = self.source_bind_groups.get_or_build(source.0, || {
                fullscreen::build_source_bind_group(
                    device,
                    &resources.layouts,
                    "post",
                    source.1,
                    &resources.sampler,
                )
            });
            render_pass.set_bind_group(0, source_bg, &[]);
            render_pass.set_bind_group(1, &params_bg, &[]);
        }
        counters::draw(&self.counts, render_pass, 0..3, 0..1);
        true
    }

    /// Warns once per kind when `pass` uses a setting that draws nothing
    /// or not as named ([`inert_param`]).
    pub(crate) fn warn_inert(&mut self, pass: &PostPass, clear_alpha: f32) {
        if let Some(param) = inert_param(pass, clear_alpha) {
            let bit = 1 << param as u8;
            if self.inert_warned & bit == 0 {
                self.inert_warned |= bit;
                log::warn!("{}", param.message());
            }
        }
    }

    /// Drops every cached object that holds a view of one of the pooled
    /// textures `freed`, so the pool's frees take effect at once.
    pub(crate) fn forget_targets(&mut self, freed: &[u64]) {
        self.source_bind_groups.forget(freed);
        self.bloom.forget_targets(freed);
    }

    /// The pipeline that draws `pass` alone: a stock effect's, or a game
    /// effect's own; `None` for bloom, drawn by its own passes, and for a
    /// game's effect that has none.
    fn pipeline_for(&self, pass: &PostPass) -> Option<&wgpu::RenderPipeline> {
        match pass {
            PostPass::Custom(custom) => self
                .custom
                .get(&custom.effect)
                .and_then(|effect| effect.pipeline.as_ref()),
            _ => stock_index(pass).map(|index| &self.stock[index].pipeline),
        }
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
            // `BloomPipeline::prepare_slot` via `bloom::pack_params`.
            PostPass::Bloom(_) => {}
            // A game's effect: its lanes are the block's.
            PostPass::Custom(custom) => {
                block.vec4 = custom.vec4;
                block.f32s = custom.f32s;
                block.i32s = custom.i32s;
            }
        }
        block
    }

    /// Draws one post-stack pass into an already-open `render_pass`, which
    /// the frame's graph opened on the pass's target: uploads the params if
    /// they changed, binds the cached source group, sets the pipeline and
    /// draws. `source` is the pooled texture the pass samples, with its
    /// identity; `slot` is the pass's index in the post stack: each slot
    /// keeps a params buffer of its own. Returns whether it drew: `false`
    /// for a pass with no pipeline, which the frame's stack leaves out.
    pub(crate) fn record_pass(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        source: (u64, &wgpu::TextureView),
        pass: &PostPass,
        slot: usize,
    ) -> bool {
        let payload = Self::pack(pass).to_bytes();
        let resources = &self.resources;
        let source_bg = self
            .source_bind_groups
            .get_or_build(source.0, || {
                fullscreen::build_source_bind_group(
                    device,
                    &resources.layouts,
                    "post",
                    source.1,
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
            counters::write_buffer(&self.counts, queue, &params.ubo, 0, &payload);
        }
        let params_bg = params.bind_group.clone();
        let Some(pipeline) = self.pipeline_for(pass) else {
            return false;
        };
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &source_bg, &[]);
        render_pass.set_bind_group(1, &params_bg, &[]);
        counters::draw(&self.counts, render_pass, 0..3, 0..1);
        true
    }
}

/// Generates and builds the pass of `signature`: its module from the
/// stages' sections, its pipeline on the stock layouts, or on bloom's when
/// the composite leads. `None`, with the report logged, when a section does
/// not read or wgpu refuses the module or the pipeline.
fn build_fused(
    device: &wgpu::Device,
    resources: &StockResources,
    bloom: &BloomPipeline,
    custom: &HashMap<PostEffectId, CustomEffect>,
    format: wgpu::TextureFormat,
    signature: Signature,
) -> Option<wgpu::RenderPipeline> {
    let started = std::time::Instant::now();
    let names = signature.names().join(" + ");
    let sections: Result<Vec<compose::Section>, String> = signature
        .effects()
        .iter()
        .map(|effect| effect_section(*effect, custom))
        .collect();
    let sections = match sections {
        Ok(sections) => sections,
        Err(error) => {
            log::error!(
                "post: the fused pass {names} has no module, its stages draw alone: {error}"
            );
            return None;
        }
    };
    let stages: Vec<&compose::Section> = sections.iter().collect();
    // A game's effect by its manifest name, where the label says `custom`.
    let names = stages
        .iter()
        .map(|section| section.name.as_str())
        .collect::<Vec<_>>()
        .join(" + ");
    let wgsl = compose::generate(&stages, signature.between);
    let label = format!("post_fused_{}", signature.names().join("_"));
    let composite_layouts = bloom.composite_layouts();
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> = if signature.led_by_composite() {
        composite_layouts
            .iter()
            .map(|layout| Some(*layout))
            .collect()
    } else {
        vec![
            Some(&resources.layouts.source_bgl),
            Some(&resources.layouts.params_bgl),
        ]
    };
    let built = crate::shader_hot_reload::build_validated(device, || {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&label),
            source: wgpu::ShaderSource::Wgsl(wgsl.as_str().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&label),
            bind_group_layouts: &layouts,
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
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
    });
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    match built {
        Ok(pipeline) => {
            log::info!("post: the fused pass {names} built in {elapsed:.1} ms");
            Some(pipeline)
        }
        Err(report) => {
            log::error!(
                "post: the fused pass {names} failed to build, its stages draw alone: {report}"
            );
            None
        }
    }
}

#[cfg(test)]
#[path = "../tests/post.rs"]
mod tests;
