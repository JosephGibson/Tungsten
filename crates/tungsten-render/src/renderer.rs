use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::counters::{self, Counts, FrameCounters};
use crate::debug_line::{DebugLineInstance, DebugLinePipeline};
use crate::graph::frame::{
    EngineStage, PassKind, PresentPath, Replaced, ReplacedStages, Shape, Stage, declare,
};
use crate::graph::pool::{Pool, PoolTexture};
use crate::graph::{At, Compiled, CompiledPass, Load, Target};
use crate::lighting::{LightUbo, LightingResources};
use crate::lit_sprite::{
    EMISSIVE_MASK_SHADER_NAME, LIT_SPRITE_SHADER_NAME, LIT_SPRITE_SHADER_SOURCE, LitSpritePipeline,
    RIM_LIGHT_SHADER_NAME,
};
use crate::material::{
    MaterialPipeline, PendingMaterial, PendingMaterials, build_material_pipeline,
};
use crate::mesh_particle::{MeshParticleBatch, MeshParticlePipeline};
use crate::post::bloom::{
    BLOOM_COMPOSITE_SHADER_NAME, BLOOM_DOWNSAMPLE_SHADER_NAME, BLOOM_SHADERS,
    BLOOM_THRESHOLD_SHADER_NAME, BLOOM_UPSAMPLE_SHADER_NAME, BloomShaderIds, BloomStageKind,
};
use crate::post::compose::{self, Between, Finish, StageInfo};
use crate::post::smaa::{
    SMAA_BLEND_WEIGHTS_SHADER_NAME, SMAA_EDGE_SHADER_NAME, SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
    SMAA_SHADERS, SmaaPipeline, SmaaShaderIds,
};
use crate::post::{
    PostStackRenderer, STOCK_MATERIAL_SHADERS, STOCK_SHADERS, drawn_stack, whole_target,
};
use crate::quad::{QuadInstance, QuadPipeline};
use crate::screenshot::{
    PendingCapture, ScreenshotError, aligned_bytes_per_row, capture_for_frame, strip_row_padding,
};
use crate::shader_hot_reload::{
    ShaderError, ShaderModuleCache, build_validated, naga_capabilities,
};
use crate::sprite::{SpriteBatch, SpritePipeline};
use crate::surface::{
    device_format_flags, device_format_usages, hdr_scene_format, msaa_sample_count,
    present_mode_label, resolve_max_frame_latency, resolve_present_mode, surface_extent,
};
use crate::targets::{DEPTH_FORMAT, TargetCache, bloom_mip_count_for_halvings};
use crate::text::{
    FontSource, TextEngine, TextNodes, TextPipeline, TextPrewarm, TextSection, TextStats,
    WorldLabelRun,
};
pub use crate::timing::{CpuFrameTimings, GpuFrameSample, GpuFrameTimings};
use crate::timing::{Ring, TimingLayout, TimingResources};
use crate::ui::{DrawItem, UiPipeline, attachment_scissor};
use thiserror::Error;
use tungsten_core::assets::{
    FilterMode, MaterialAssetId, MaterialUniformDefaults, ParticleMeshAssetId, PostEffectId,
    ResolvedFontFamily, ShaderAssetId, TextureHandle,
};
use tungsten_core::config::{
    BloomBase, DepthSortMode, PostAaMode, PresentModeConfig, RenderConfig, is_supported_msaa,
};
use tungsten_core::post::{PostPass, PostStack};
use tungsten_core::ui::PaintList;
use winit::window::Window;

use crate::surface_acquire::{
    AcquireAction, AcquireStatus, MAX_ACQUIRE_ATTEMPTS, TargetsAt, acquire_action,
    reconfigure_for_suboptimal, record_again_after_acquire,
};

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("no suitable GPU adapter found: {0}")]
    NoAdapter(#[from] wgpu::RequestAdapterError),
    #[error("failed to request device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error(
        "unsupported present mode '{requested}'; available configurable modes: {}",
        available.join(", ")
    )]
    UnsupportedPresentMode {
        requested: String,
        available: Vec<String>,
    },
    #[error("invalid max frame latency '{0}'; expected >= 1")]
    InvalidFrameLatency(u32),
    #[error("surface error: {0}")]
    Surface(String),
    #[error("shader error: {0}")]
    Shader(#[from] ShaderError),
    /// A game's post effect that cannot draw (`D-308`): its shader has no
    /// module, or its module does not build as a post pass.
    #[error("post effect '{name}': {reason}")]
    PostEffect { name: String, reason: String },
}

/// What one `render_frame*` call did with its frame (`D-142`). A failed frame
/// is the call's `Err(RenderError)`: a surface lost again right after its
/// recreation, which device loss produces, or a failed recreation.
#[must_use]
#[non_exhaustive]
#[derive(Debug)]
pub enum RenderOutcome {
    /// Drawn, submitted and presented. `capture` is the result of the capture
    /// armed for this frame ([`Renderer::capture_frame`]): the path written,
    /// or the readback, encode or IO error; `None` when none was armed.
    Presented {
        capture: Option<Result<PathBuf, ScreenshotError>>,
    },
    /// No surface texture (a timeout, an occluded window, or a surface still
    /// outdated after one reconfigure): nothing was presented, and a capture
    /// armed for this frame was dropped unwritten. Work recorded before the
    /// acquire may have run.
    Skipped,
}

/// Stable well-known id for the engine-internal sprite shader. Matches the
/// manifest entry under `shaders.sprite`.
pub const SPRITE_SHADER_NAME: &str = "sprite";

/// Submit a frame's first command buffer, its passes before the swapchain
/// stage, before the surface acquire, so the GPU starts on them while the CPU
/// waits for the swapchain (`D-145`). Off, the first buffer is submitted with
/// the second.
const SUBMIT_BEFORE_ACQUIRE: bool = true;

/// Cache id of the first stock post shader. Ids 0-10 belong to the sprite,
/// SMAA, bloom and lit-sprite shaders that `Renderer::new` seeds by number;
/// the rows of `STOCK_SHADERS` follow, then those of
/// `STOCK_MATERIAL_SHADERS`, then the shaders a manifest adds.
const STOCK_SHADER_ID_BASE: u32 = 11;

/// Cache id of the first stock material shader (redo 7.9).
const STOCK_MATERIAL_SHADER_ID_BASE: u32 = STOCK_SHADER_ID_BASE + STOCK_SHADERS.len() as u32;

/// WGPU renderer state.
pub struct Renderer {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    /// Kept to recreate the surface after `CurrentSurfaceTexture::Lost` and to
    /// read its size when the surface is outdated or suboptimal.
    window: Arc<Window>,
    /// Size already reconfigured for a suboptimal frame since the last clean
    /// acquire; see `surface_acquire::reconfigure_for_suboptimal`.
    suboptimal_reconfigured: Option<(u32, u32)>,
    pub clear_color: wgpu::Color,
    quad_pipeline: QuadPipeline,
    sprite_pipeline: SpritePipeline,
    debug_line_pipeline: DebugLinePipeline,
    mesh_particle_pipeline: MeshParticlePipeline,
    text_pipeline: TextPipeline,
    /// M46 UI primitives, drawn in the overlay pass before the text.
    ui_pipeline: UiPipeline,
    present_blit: PresentBlitPipeline,
    post_stack: PostStackRenderer,
    /// The pooled textures the frame's passes draw into and sample.
    targets: Pool<PoolTexture>,
    /// The compiled frame of the shape last planned, with the pooled
    /// textures bound to its slots. It stands while the shape holds.
    frame: Option<FramePlan>,
    /// The shape of the frame being planned. Kept for its allocation.
    shape_scratch: Shape,
    /// How many times a resize changed the surface's size: with the size,
    /// what an acquire can move under a frame's first part (`D-145`).
    target_resizes: u64,
    /// When the renderer was made: the clock the pool's keep rule reads.
    started: Instant,
    /// The engine's full-screen shaders a manifest's file or a reload
    /// replaced. Their passes clear their targets (`D-306`).
    replaced: Replaced,
    shader_cache: ShaderModuleCache,
    sample_count: u32,
    depth_enabled: bool,
    depth_sort: DepthSortMode,
    post_aa: PostAaMode,
    /// SMAA presentation pipeline. Allocated on demand when `post_aa != Off`.
    smaa: Option<SmaaPipeline>,
    smaa_shader_ids: SmaaShaderIds,
    bloom_max_mips: u32,
    bloom_base: BloomBase,
    /// `render.post_fuse`: whether the stages the composer's rule allows
    /// draw in one pass (`D-307`).
    post_fuse: bool,
    /// The scene's float format under `render.hdr` (`D-309`): the scene's
    /// pipelines and every post pass but the frame's last draw in it. `None`
    /// draws them in the surface's format.
    float: Option<wgpu::TextureFormat>,
    /// The stages of the stack being planned, as the composer's rule reads
    /// them. Kept for its allocation.
    stage_infos: Vec<StageInfo>,
    /// The runs of the stack being planned under a float scene. Kept for
    /// its allocation.
    run_scratch: Vec<std::ops::Range<usize>>,
    /// The stack a frame draws when a game's effect has no pipeline: the
    /// frame's own without that effect's passes. Kept for its allocation.
    stack_scratch: PostStack,
    /// M29 lit sprite pipeline; rebuilt on `lit_sprite` shader hot-reload.
    lit_sprite_pipeline: LitSpritePipeline,
    /// M29 per-frame lighting UBO + bind group bound at group 2 of the lit
    /// pipeline. Resize does not invalidate this; the buffer + bind group
    /// stay live across surface reconfigures.
    lighting: LightingResources,
    /// M26 material pipelines keyed by id.
    materials: HashMap<MaterialAssetId, MaterialPipeline>,
    /// Materials whose upload failed with no pipeline to keep, built when
    /// their shader next uploads or reloads.
    pending_materials: PendingMaterials,
    /// M26 known manifest-tracked shader ids (name → id). The built-in
    /// sprite shader is always pre-seeded; material/stock loads register the
    /// rest. `upload_shader` / `reload_shader` consult this map to pick the
    /// correct id and rebuild the right set of pipelines.
    shader_ids: HashMap<String, ShaderAssetId>,
    /// Next id for a non-sprite shader.
    next_shader_id: u32,
    /// Timestamp query support.
    pub timestamp_support: bool,
    /// Last GPU frame timings.
    pub gpu_timings: GpuFrameTimings,
    /// Last CPU render timings.
    pub cpu_timings: CpuFrameTimings,
    /// The timestamp readbacks of timed frames in flight (redo 7.3).
    timing_ring: Ring<TimingResources>,
    /// The app's number for the frame being rendered, which a timed frame's
    /// samples name.
    frame_number: u64,
    /// The renderer's own draws: the present blits.
    counts: Box<Counts>,
    /// The last rendered frame's draws and uploads.
    frame_counters: FrameCounters,
    /// One-shot PNG capture armed for the next frame.
    pub(crate) pending_capture: Option<PendingCapture>,
    /// View-projection bytes last written to the quad and sprite camera
    /// buffers.
    camera_written: Option<[u8; 64]>,
    /// Payload last written to the lighting UBO.
    lights_written: Option<LightUbo>,
    /// The 1 × 1 white texel's handle, once made (redo 7.9).
    white_texture: Option<TextureHandle>,
}

impl Renderer {
    /// Initialize wgpu renderer for `window`.
    #[allow(clippy::needless_pass_by_value)] // Arc<Window> is cheap; public ctor shape matters.
    pub fn new(
        window: Arc<Window>,
        config: &RenderConfig,
        vsync: bool,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all().with_env(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| RenderError::Surface(e.to_string()))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            // Limit bucketing only matters when untrusted content can query the adapter.
            apply_limit_buckets: false,
        }))?;

        // Optional features only; device creation must not depend on them.
        // Without the adapter's own format features, wgpu holds every format
        // to WebGPU's guarantee, where 4 is the only multisample count.
        let adapter_features = adapter.features();
        let desired_features = adapter_features
            & (wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("tungsten_device"),
                required_features: desired_features,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }))?;

        let timestamp_support = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let adapter_info = adapter.get_info();

        let size = window.inner_size();
        let window_size = (size.width.max(1), size.height.max(1));
        let (width, height) = surface_extent(window_size, device.limits().max_texture_dimension_2d);
        if (width, height) != window_size {
            log::warn!(
                "window {}x{} is larger than the device's textures; rendering at {width}x{height}",
                window_size.0,
                window_size.1
            );
        }
        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode =
            resolve_present_mode(&surface_caps.present_modes, config.present_mode, vsync)?;
        let desired_maximum_frame_latency =
            resolve_max_frame_latency(config.max_frame_latency, present_mode)?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency,
            // HDR output is deferred; Auto keeps the SDR behavior wgpu 29 had.
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &surface_config);

        let gpu_timings = GpuFrameTimings {
            frame_gpu_ms: None,
            render_gpu_ms: None,
            pass_gpu_ms: Vec::new(),
            samples: Vec::new(),
            skipped_frames: 0,
            backend: Some(format!("{:?}", adapter_info.backend)),
            adapter_name: Some(adapter_info.name.clone()),
            present_mode: Some(present_mode_label(present_mode).to_string()),
            max_frame_latency: Some(desired_maximum_frame_latency),
        };

        let c = config.clear_color;
        let clear_color = wgpu::Color {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        };

        let requested_msaa = if is_supported_msaa(config.msaa) {
            config.msaa
        } else {
            log::warn!(
                "unsupported msaa sample_count {} in RenderConfig; falling back to 1",
                config.msaa
            );
            1
        };
        let depth_enabled = config.depth_enabled;
        // The multisampled targets are the scene color, in the surface
        // format, and the depth target when depth is enabled.
        let downlevel = adapter.get_downlevel_capabilities().flags;
        let flags_of = |format: wgpu::TextureFormat| {
            let adapter_flags = adapter.get_texture_format_features(format).flags;
            device_format_flags(device.features(), downlevel, adapter_flags, format)
        };
        // The scene's format under `render.hdr`: a float one, `rg11b10`
        // only where the device draws, blends, filters and resolves it.
        let rg11b10 = wgpu::TextureFormat::Rg11b10Ufloat;
        let rg11b10_usages = device_format_usages(
            device.features(),
            downlevel,
            adapter.get_texture_format_features(rg11b10).allowed_usages,
            rg11b10,
        );
        let float = hdr_scene_format(
            config.hdr,
            rg11b10_usages,
            flags_of(rg11b10),
            requested_msaa,
        );
        if let Some((chosen, true)) = float {
            log::warn!(
                "render.hdr = {}: this adapter does not draw {rg11b10:?} with blending, filtering and msaa {requested_msaa}; using {chosen:?}",
                config.hdr.as_str()
            );
        }
        let float = float.map(|(chosen, _)| chosen);
        // What the scene's pipelines draw in; the overlay, SMAA and the
        // frame's last pass draw in the surface's `format`.
        let scene_format = float.unwrap_or(format);
        let sample_count = msaa_sample_count(
            requested_msaa,
            flags_of(scene_format),
            depth_enabled.then(|| flags_of(DEPTH_FORMAT)),
        );
        if sample_count != requested_msaa {
            let depth = if depth_enabled {
                format!(" and {DEPTH_FORMAT:?}")
            } else {
                String::new()
            };
            log::warn!(
                "msaa {requested_msaa} is not supported for {scene_format:?}{depth} on this adapter; using {sample_count}"
            );
        }
        // `GpuDepth` without a depth target is a nonsense combo; normalize
        // before any pipeline/pass decision reads it so the two knobs can
        // never disagree at runtime.
        let depth_sort = if matches!(config.depth_sort, DepthSortMode::GpuDepth) && !depth_enabled {
            log::warn!(
                "render.depth_sort = gpu_depth requires depth_enabled = true; falling back to cpu_stable"
            );
            DepthSortMode::CpuStable
        } else {
            config.depth_sort
        };
        // Every pipeline that renders into the scene pass must agree with
        // the pass attachments. Under GpuDepth the pass has a depth buffer,
        // so quad/debug_line/text get a read-only `Always` depth state and
        // the sprite pipelines the `LessEqual` test, written by a solid
        // batch's variant alone (redo 7.15).
        let depth_attached = matches!(depth_sort, DepthSortMode::GpuDepth);

        let quad_pipeline = QuadPipeline::new(&device, scene_format, sample_count, depth_attached);
        let sprite_pipeline =
            SpritePipeline::new(&device, scene_format, sample_count, depth_attached);
        let debug_line_pipeline = DebugLinePipeline::new(
            &device,
            scene_format,
            quad_pipeline.camera_bind_group_layout(),
            sample_count,
            depth_attached,
        );
        let mesh_particle_pipeline = MeshParticlePipeline::new(
            &device,
            scene_format,
            quad_pipeline.camera_bind_group_layout(),
            sample_count,
            depth_attached,
        );
        let font_source = if config.system_fonts {
            FontSource::PackagedThenSystem
        } else {
            FontSource::Packaged
        };
        let text_pipeline = TextPipeline::with_font_source(&device, &queue, format, font_source);
        let ui_pipeline = UiPipeline::new(
            &device,
            &queue,
            format,
            sprite_pipeline.texture_bind_group_layout(),
        );

        let post_aa = config.post_aa;
        let bloom_max_mips =
            if tungsten_core::config::is_supported_bloom_max_mips(config.bloom_max_mips) {
                config.bloom_max_mips
            } else {
                log::warn!(
                    "unsupported bloom_max_mips {} in RenderConfig; falling back to 6",
                    config.bloom_max_mips
                );
                6
            };
        let bloom_base = config.bloom_base;
        // The frame's targets are pooled: the first frame allocates the ones
        // its shape uses, and no other.
        let shape_scratch = Shape {
            output: (surface_config.width, surface_config.height),
            format,
            samples: sample_count,
            depth: false,
            smaa: false,
            present: PresentPath::Direct,
            stages: Vec::new(),
            bloom_mips: 0,
            bloom_halvings: 0,
            replaced: ReplacedStages::NONE,
            float,
            tail_alone: false,
        };
        let present_blit = PresentBlitPipeline::new(&device, format);

        // Seed the shader cache with the compile-time sprite WGSL so the
        // first `asset_loader::load_shaders` call is a no-op when the
        // on-disk bytes match. Same for the three SMAA stage shaders.
        let mut shader_cache = ShaderModuleCache::new();
        shader_cache.set_capabilities(naga_capabilities(
            device.features(),
            adapter.get_downlevel_capabilities().flags,
        ));
        let sprite_shader_id = ShaderAssetId(0);
        let sprite_src = include_str!("../../../assets/shaders/sprite.wgsl").to_string();
        shader_cache
            .upload(&device, sprite_shader_id, SPRITE_SHADER_NAME, sprite_src)
            .expect("compile-time sprite shader must validate");

        let smaa_shader_ids = SmaaShaderIds {
            edge: ShaderAssetId(1),
            blend_weights: ShaderAssetId(2),
            neighborhood_blend: ShaderAssetId(3),
        };
        let edge_src = SMAA_SHADERS[0].1.to_string();
        let blend_src = SMAA_SHADERS[1].1.to_string();
        let nbh_src = SMAA_SHADERS[2].1.to_string();
        shader_cache
            .upload(
                &device,
                smaa_shader_ids.edge,
                SMAA_EDGE_SHADER_NAME,
                edge_src,
            )
            .expect("compile-time smaa_edge shader must validate");
        shader_cache
            .upload(
                &device,
                smaa_shader_ids.blend_weights,
                SMAA_BLEND_WEIGHTS_SHADER_NAME,
                blend_src,
            )
            .expect("compile-time smaa_blend_weights shader must validate");
        shader_cache
            .upload(
                &device,
                smaa_shader_ids.neighborhood_blend,
                SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
                nbh_src,
            )
            .expect("compile-time smaa_neighborhood_blend shader must validate");

        // Bloom's ids 4..=7 follow the sprite's 0 and SMAA's 1..=3: the cache
        // keys modules by id and `reload_shader` rebuilds the stage whose
        // stored id matches, so a shared id would swap the wrong stage.
        let bloom_shader_ids = BloomShaderIds {
            threshold: ShaderAssetId(4),
            downsample: ShaderAssetId(5),
            upsample: ShaderAssetId(6),
            composite: ShaderAssetId(7),
        };
        let bloom_threshold_src = BLOOM_SHADERS[0].1.to_string();
        let bloom_downsample_src = BLOOM_SHADERS[1].1.to_string();
        let bloom_upsample_src = BLOOM_SHADERS[2].1.to_string();
        let bloom_composite_src = BLOOM_SHADERS[3].1.to_string();
        shader_cache
            .upload(
                &device,
                bloom_shader_ids.threshold,
                BLOOM_THRESHOLD_SHADER_NAME,
                bloom_threshold_src,
            )
            .expect("compile-time bloom_threshold shader must validate");
        shader_cache
            .upload(
                &device,
                bloom_shader_ids.downsample,
                BLOOM_DOWNSAMPLE_SHADER_NAME,
                bloom_downsample_src,
            )
            .expect("compile-time bloom_downsample shader must validate");
        shader_cache
            .upload(
                &device,
                bloom_shader_ids.upsample,
                BLOOM_UPSAMPLE_SHADER_NAME,
                bloom_upsample_src,
            )
            .expect("compile-time bloom_upsample shader must validate");
        shader_cache
            .upload(
                &device,
                bloom_shader_ids.composite,
                BLOOM_COMPOSITE_SHADER_NAME,
                bloom_composite_src,
            )
            .expect("compile-time bloom_composite shader must validate");

        // The 17 stock post shaders (`D-091`): seeded like the ones above, so
        // their pipelines build from the cache, `load_shaders` is a no-op
        // while the on-disk bytes match, and a reload can rebuild them.
        let stock_shader_ids: [ShaderAssetId; 17] =
            std::array::from_fn(|index| ShaderAssetId(STOCK_SHADER_ID_BASE + index as u32));
        for (&(name, source), &id) in STOCK_SHADERS.iter().zip(&stock_shader_ids) {
            shader_cache
                .upload(&device, id, name, source.to_string())
                .expect("compile-time stock post shader must validate");
        }

        // The stock materials' shaders (redo 7.9), after the post rows.
        for (index, &(name, source)) in STOCK_MATERIAL_SHADERS.iter().enumerate() {
            let id = ShaderAssetId(STOCK_MATERIAL_SHADER_ID_BASE + index as u32);
            shader_cache
                .upload(&device, id, name, source.to_string())
                .expect("compile-time stock material shader must validate");
        }

        let mut post_stack = PostStackRenderer::new(
            &device,
            scene_format,
            format,
            &shader_cache,
            bloom_shader_ids,
            &stock_shader_ids,
        );
        // Under a float scene every frame ends in the tail, in its last
        // post pass or in a pass of its own: that pass is the engine's own
        // module, built here as the stock pipelines are.
        if float.is_some() {
            let tail = post_stack.tail();
            assert!(
                post_stack.fused_ready(&device, &[], Between::Nothing, tail),
                "the frame's tail must build"
            );
        }

        // M29: pre-seed lit_sprite (8) + emissive_mask (9) + rim_light (10).
        let lit_sprite_shader_id = ShaderAssetId(8);
        let emissive_mask_shader_id = ShaderAssetId(9);
        let rim_light_shader_id = ShaderAssetId(10);
        shader_cache
            .upload(
                &device,
                lit_sprite_shader_id,
                LIT_SPRITE_SHADER_NAME,
                LIT_SPRITE_SHADER_SOURCE.to_string(),
            )
            .expect("compile-time lit_sprite shader must validate");
        shader_cache
            .upload(
                &device,
                emissive_mask_shader_id,
                EMISSIVE_MASK_SHADER_NAME,
                include_str!("shaders/stock/emissive_mask.wgsl").to_string(),
            )
            .expect("compile-time emissive_mask shader must validate");
        shader_cache
            .upload(
                &device,
                rim_light_shader_id,
                RIM_LIGHT_SHADER_NAME,
                include_str!("shaders/stock/rim_light.wgsl").to_string(),
            )
            .expect("compile-time rim_light shader must validate");

        let lighting = LightingResources::new(&device);
        let lit_module = shader_cache
            .get(lit_sprite_shader_id)
            .expect("lit_sprite module pre-seeded above");
        let lit_sprite_pipeline = LitSpritePipeline::new(
            &device,
            lit_module,
            sprite_pipeline.camera_bind_group_layout(),
            sprite_pipeline.lit_texture_bind_group_layout(),
            &lighting.bind_group_layout,
            scene_format,
            sample_count,
            depth_attached,
        );

        let smaa =
            if post_aa.is_smaa() {
                let built = build_smaa_pipeline(
                    &device,
                    &queue,
                    format,
                    &shader_cache,
                    smaa_shader_ids,
                    post_aa,
                    (surface_config.width, surface_config.height),
                );
                Some(built.unwrap_or_else(|report| {
                    panic!("compile-time SMAA shaders must build: {report}")
                }))
            } else {
                None
            };

        let mut shader_ids = HashMap::new();
        shader_ids.insert(SPRITE_SHADER_NAME.to_string(), sprite_shader_id);
        shader_ids.insert(SMAA_EDGE_SHADER_NAME.to_string(), smaa_shader_ids.edge);
        shader_ids.insert(
            SMAA_BLEND_WEIGHTS_SHADER_NAME.to_string(),
            smaa_shader_ids.blend_weights,
        );
        shader_ids.insert(
            SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME.to_string(),
            smaa_shader_ids.neighborhood_blend,
        );
        shader_ids.insert(
            BLOOM_THRESHOLD_SHADER_NAME.to_string(),
            bloom_shader_ids.threshold,
        );
        shader_ids.insert(
            BLOOM_DOWNSAMPLE_SHADER_NAME.to_string(),
            bloom_shader_ids.downsample,
        );
        shader_ids.insert(
            BLOOM_UPSAMPLE_SHADER_NAME.to_string(),
            bloom_shader_ids.upsample,
        );
        shader_ids.insert(
            BLOOM_COMPOSITE_SHADER_NAME.to_string(),
            bloom_shader_ids.composite,
        );
        shader_ids.insert(LIT_SPRITE_SHADER_NAME.to_string(), lit_sprite_shader_id);
        shader_ids.insert(
            EMISSIVE_MASK_SHADER_NAME.to_string(),
            emissive_mask_shader_id,
        );
        shader_ids.insert(RIM_LIGHT_SHADER_NAME.to_string(), rim_light_shader_id);
        for (&(name, _), &id) in STOCK_SHADERS.iter().zip(&stock_shader_ids) {
            shader_ids.insert(name.to_string(), id);
        }
        for (index, &(name, _)) in STOCK_MATERIAL_SHADERS.iter().enumerate() {
            let id = ShaderAssetId(STOCK_MATERIAL_SHADER_ID_BASE + index as u32);
            shader_ids.insert(name.to_string(), id);
        }

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            surface_config,
            window,
            suboptimal_reconfigured: None,
            clear_color,
            quad_pipeline,
            sprite_pipeline,
            debug_line_pipeline,
            mesh_particle_pipeline,
            text_pipeline,
            ui_pipeline,
            present_blit,
            post_stack,
            targets: Pool::default(),
            frame: None,
            shape_scratch,
            target_resizes: 0,
            started: Instant::now(),
            replaced: Replaced::default(),
            shader_cache,
            sample_count,
            depth_enabled,
            depth_sort,
            post_aa,
            smaa,
            smaa_shader_ids,
            bloom_max_mips,
            bloom_base,
            post_fuse: config.post_fuse,
            float,
            stage_infos: Vec::new(),
            run_scratch: Vec::new(),
            stack_scratch: PostStack::default(),
            lit_sprite_pipeline,
            lighting,
            materials: HashMap::new(),
            pending_materials: PendingMaterials::default(),
            shader_ids,
            next_shader_id: STOCK_MATERIAL_SHADER_ID_BASE + STOCK_MATERIAL_SHADERS.len() as u32,
            timestamp_support,
            gpu_timings,
            cpu_timings: CpuFrameTimings::default(),
            timing_ring: Ring::default(),
            frame_number: 0,
            counts: Counts::boxed(),
            frame_counters: FrameCounters::default(),
            pending_capture: None,
            camera_written: None,
            lights_written: None,
            white_texture: None,
        })
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// The format the scene's pipelines and the stack's passes draw in: the
    /// surface's, or the float one `render.hdr` chose (`D-309`).
    fn scene_format(&self) -> wgpu::TextureFormat {
        self.float.unwrap_or(self.surface_config.format)
    }

    /// Current MSAA sample count.
    #[must_use]
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Current depth-sort mode.
    #[must_use]
    pub fn depth_sort(&self) -> DepthSortMode {
        self.depth_sort
    }

    /// Upload RGBA texture; sampler filter is baked into bind group.
    pub fn upload_texture(
        &mut self,
        handle: TextureHandle,
        rgba_data: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    ) {
        self.sprite_pipeline.upload_texture(
            &self.device,
            &self.queue,
            handle,
            rgba_data,
            width,
            height,
            filter,
        );
    }

    /// Mint renderer-owned texture handle.
    pub fn allocate_texture_handle(&mut self) -> TextureHandle {
        self.sprite_pipeline.allocate_texture_handle()
    }

    /// The 1 × 1 white texel (redo 7.9): its handle minted and its pixel
    /// uploaded on first use, then kept. The umbrella registers it as the
    /// reserved sprite `engine_white`, which flat quads, segments and the
    /// stock glow draw on.
    pub fn white_texture(&mut self) -> TextureHandle {
        if let Some(handle) = self.white_texture {
            return handle;
        }
        let handle = self.allocate_texture_handle();
        self.upload_texture(handle, &[255; 4], 1, 1, FilterMode::Nearest);
        self.white_texture = Some(handle);
        handle
    }

    /// Drop texture pool entry and bind group. Also drops the matching M29
    /// lit-bundle entry if present, keeping the two pools in lockstep.
    pub fn drop_texture(&mut self, handle: TextureHandle) {
        self.sprite_pipeline.drop_texture(handle);
        self.sprite_pipeline.drop_lit_texture(handle);
    }

    /// M29 upload albedo + normal + emissive RGBA pages keyed by the same
    /// page handle as the regular albedo upload.
    #[allow(clippy::too_many_arguments)]
    pub fn upload_lit_texture(
        &mut self,
        handle: TextureHandle,
        albedo: &[u8],
        normal: &[u8],
        emissive: &[u8],
        width: u32,
        height: u32,
        filter: FilterMode,
    ) {
        self.sprite_pipeline.upload_lit_texture(
            &self.device,
            &self.queue,
            handle,
            albedo,
            normal,
            emissive,
            width,
            height,
            filter,
        );
    }

    /// M29 copy a packed cell into the lit texture bundle.
    #[allow(clippy::too_many_arguments)]
    pub fn write_subtexture_lit(
        &self,
        handle: TextureHandle,
        albedo: &[u8],
        normal: &[u8],
        emissive: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        self.sprite_pipeline.write_subtexture_lit(
            &self.queue,
            handle,
            albedo,
            normal,
            emissive,
            x,
            y,
            width,
            height,
        );
    }

    /// M29 upload one frame of the lighting UBO, unless it already holds
    /// these bytes. The bind group built at startup stays valid; resize does
    /// not invalidate it.
    pub fn update_lights(&mut self, ubo: &LightUbo) {
        let unchanged = self
            .lights_written
            .as_ref()
            .is_some_and(|written| bytemuck::bytes_of(written) == bytemuck::bytes_of(ubo));
        if !unchanged {
            self.lighting.write(&self.queue, ubo);
            self.lights_written = Some(*ubo);
        }
    }

    /// M31 upload or replace the geometry of one particle mesh (`D-093`).
    pub fn upload_particle_mesh(
        &mut self,
        id: ParticleMeshAssetId,
        vertices: &[[f32; 2]],
        indices: &[u16],
    ) {
        self.mesh_particle_pipeline
            .upload_mesh(&self.device, id, vertices, indices);
    }

    /// M31 instances for the next frame; they hold until the next call, so the
    /// app calls it every frame.
    pub fn update_mesh_particles(&mut self, batches: &[MeshParticleBatch]) {
        self.mesh_particle_pipeline
            .prepare(&self.device, &self.queue, batches);
    }

    /// M54 world labels for the next frame (`D-161`): a copy is kept until
    /// the next call, so the app calls it every frame; an empty list draws
    /// none.
    pub fn update_world_labels(&mut self, labels: &[WorldLabelRun]) {
        self.text_pipeline.set_world_labels(labels);
    }

    /// Portable atlas page dimension cap.
    pub fn max_2d_texture_dimension(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d.min(8192)
    }

    /// Copy RGBA sub-rect into existing texture.
    pub fn write_subtexture(
        &self,
        handle: TextureHandle,
        rgba: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        self.sprite_pipeline
            .write_subtexture(&self.queue, handle, rgba, x, y, width, height);
    }

    /// Load font bytes under manifest ID.
    pub fn load_font(&mut self, id: &str, data: Vec<u8>) {
        self.text_pipeline.load_font(id, data);
    }

    /// Hot-reload font bytes; false when they hold no face, which keeps the
    /// font's old faces.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) -> bool {
        self.text_pipeline.reload_font(id, data)
    }

    /// The text engine, read-only: its font epoch and committed node layouts
    /// (`D-117`).
    #[must_use]
    pub fn text_engine(&self) -> &TextEngine {
        self.text_pipeline.engine()
    }

    /// The last frame's text telemetry: UI batches, batches prepared, full
    /// and forced frames, `AtlasFull` events (M46), and the world labels
    /// drawn, their glyphs and the layouts cached (M54).
    #[must_use]
    pub fn text_stats(&self) -> TextStats {
        self.text_pipeline.stats()
    }

    /// Rasterizes the UI text runs of `list`, a prewarm list from
    /// `UiTree::paint_prewarm`, into the glyph atlas without drawing them,
    /// at the surface's size (M56). Call it before the frame's render, so a
    /// menu's first open prepares glyphs the atlas already holds; it logs
    /// `UI prewarm: <runs> text runs, <glyphs> glyphs in <ms> ms`.
    pub fn prewarm_ui_text(&mut self, list: &PaintList) -> TextPrewarm {
        let (width, height) = (self.surface_config.width, self.surface_config.height);
        self.text_pipeline
            .prewarm(&self.device, &self.queue, list, width, height)
    }

    /// The text node methods, for layout to measure and commit with
    /// (`D-117`).
    pub fn text_nodes(&mut self) -> TextNodes<'_> {
        TextNodes::new(self.text_pipeline.engine_mut())
    }

    /// Set the manifest's font families and fallback chain (`D-115`). Equal
    /// input changes nothing.
    pub fn set_font_families(
        &mut self,
        families: &HashMap<String, ResolvedFontFamily>,
        fallback: &[String],
    ) {
        self.text_pipeline.set_font_families(families, fallback);
    }

    /// Register a manifest-tracked shader. A shader the engine compiles into
    /// a pipeline of its own (sprite, SMAA, bloom, lit sprite, the stock post
    /// effects) rebuilds that pipeline; other ids are cached for the
    /// materials bound to them. Byte-equal short-circuits skip validation.
    pub fn upload_shader(
        &mut self,
        name: &str,
        wgsl: String,
    ) -> Result<ShaderAssetId, RenderError> {
        let id = self.resolve_or_allocate_shader_id(name);

        if self.shader_cache.bytes_equal(id, &wgsl) {
            return Ok(id);
        }

        let module = self.shader_cache.validate(&self.device, name, &wgsl)?;
        self.apply_shader_module(name, id, wgsl, module)?;
        Ok(id)
    }

    /// Hot-reload a manifest-tracked shader. Failures log and keep the live
    /// `ShaderModule` + pipelines untouched (last-known-good).
    pub fn reload_shader(&mut self, name: &str, wgsl: String) -> Result<(), RenderError> {
        let Some(id) = self.shader_ids.get(name).copied() else {
            log::error!("shader reload: unknown id '{name}'");
            return Ok(());
        };
        if self.shader_cache.bytes_equal(id, &wgsl) {
            return Ok(());
        }
        let module = match self.shader_cache.validate(&self.device, name, &wgsl) {
            Ok(m) => m,
            Err(e) => {
                log::error!("shader '{name}' validation failed: {e}");
                return Ok(());
            }
        };
        if let Err(e) = self.apply_shader_module(name, id, wgsl, module) {
            log::error!("{e}; keeping the live pipeline");
            return Ok(());
        }
        log::info!("shader '{name}' reloaded");
        Ok(())
    }

    /// Switch shader `name` to a freshly validated `module`: rebuild the
    /// pipeline the engine compiles it into, commit the module to the cache,
    /// then rebuild the materials bound to it. The load path and the reload
    /// path share it, so a pipeline connected here reloads on both. A rebuilt
    /// pipeline that fails wgpu validation is not swapped in and the module is
    /// not committed: `Err`, and the live pipeline and module stay (`D-057`).
    fn apply_shader_module(
        &mut self,
        name: &str,
        id: ShaderAssetId,
        wgsl: String,
        module: wgpu::ShaderModule,
    ) -> Result<(), ShaderError> {
        self.rebuild_shader_pipelines(name, id, &module)
            .map_err(|report| ShaderError::Pipeline {
                name: name.to_string(),
                report,
            })?;
        // A full-screen shader on a text that is not the engine's own may
        // leave pixels unwritten, so its pass clears: the frame's shape is
        // read from this record (`D-306`).
        self.replaced.commit(name, &wgsl);
        // A fused pass that draws this effect is built again at its next
        // use, from the compiled-in sections: a replaced file's stage draws
        // alone meanwhile.
        self.post_stack.forget_fused(name);
        self.shader_cache.commit(id, wgsl, module);
        // Any material bound to this shader needs a rebuild against the new module.
        if name != SPRITE_SHADER_NAME
            && !matches!(
                name,
                SMAA_EDGE_SHADER_NAME
                    | SMAA_BLEND_WEIGHTS_SHADER_NAME
                    | SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME
                    | BLOOM_THRESHOLD_SHADER_NAME
                    | BLOOM_DOWNSAMPLE_SHADER_NAME
                    | BLOOM_UPSAMPLE_SHADER_NAME
                    | BLOOM_COMPOSITE_SHADER_NAME
                    | LIT_SPRITE_SHADER_NAME
                    | EMISSIVE_MASK_SHADER_NAME
                    | RIM_LIGHT_SHADER_NAME
            )
        {
            self.rebuild_materials_for_shader(name);
        }
        self.rebuild_post_effects_for_shader(name);
        Ok(())
    }

    /// Rebuild the engine pipeline that compiles shader `name`, if any (a
    /// name feeds at most one); `Err` when the candidate fails validation,
    /// with the live pipeline kept.
    fn rebuild_shader_pipelines(
        &mut self,
        name: &str,
        id: ShaderAssetId,
        module: &wgpu::ShaderModule,
    ) -> Result<(), String> {
        let depth_write = matches!(self.depth_sort, DepthSortMode::GpuDepth);
        if name == SPRITE_SHADER_NAME {
            self.sprite_pipeline.rebuild_with_shader(
                &self.device,
                module,
                self.scene_format(),
                self.sample_count,
                depth_write,
            )?;
        }
        if matches!(
            name,
            SMAA_EDGE_SHADER_NAME
                | SMAA_BLEND_WEIGHTS_SHADER_NAME
                | SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME
        ) && let Some(smaa) = self.smaa.as_mut()
        {
            smaa.rebuild_stage_with_module(&self.device, id, module)?;
        }
        if matches!(
            name,
            BLOOM_THRESHOLD_SHADER_NAME
                | BLOOM_DOWNSAMPLE_SHADER_NAME
                | BLOOM_UPSAMPLE_SHADER_NAME
                | BLOOM_COMPOSITE_SHADER_NAME
        ) {
            self.post_stack.bloom.rebuild_stage_with_module(
                &self.device,
                self.scene_format(),
                id,
                module,
            )?;
        }
        if name == LIT_SPRITE_SHADER_NAME {
            self.lit_sprite_pipeline.rebuild_with_shader(
                &self.device,
                module,
                self.scene_format(),
                self.sample_count,
                depth_write,
            )?;
        }
        // A stock post effect's pipeline; a no-op for any other name.
        self.post_stack
            .rebuild_stock_with_module(&self.device, self.scene_format(), name, module)
    }

    /// The ID of shader `name`, minted on first use. The renderer is the one
    /// allocator of shader IDs, as of texture handles (`D-048`); the umbrella
    /// records each in core's `ShaderRegistry`, so both sides key a shader by
    /// the same ID.
    pub fn resolve_or_allocate_shader_id(&mut self, name: &str) -> ShaderAssetId {
        if let Some(&id) = self.shader_ids.get(name) {
            return id;
        }
        let id = ShaderAssetId(self.next_shader_id);
        self.next_shader_id += 1;
        self.shader_ids.insert(name.to_string(), id);
        id
    }

    /// Upload a new user-authored material (M26). Validation-failed builds
    /// keep the last-known-good entry; a first-time failure leaves none, and
    /// the material builds when its shader next uploads or reloads.
    pub fn upload_material(
        &mut self,
        id: MaterialAssetId,
        name: &str,
        shader_name: &str,
        defaults: MaterialUniformDefaults,
    ) -> Result<(), RenderError> {
        let result = self.build_material(id, name, shader_name, defaults);
        if result.is_ok() {
            self.pending_materials.built(id);
        } else if !self.materials.contains_key(&id) {
            self.pending_materials.wait(
                id,
                PendingMaterial {
                    name: name.to_string(),
                    shader_name: shader_name.to_string(),
                    defaults,
                },
            );
        }
        result
    }

    /// Builds the game's post effect `id`, manifest entry `name`, on the
    /// module of shader `shader_name`: a whole module on the stock post
    /// prefix, drawn as `PostPass::Custom` (`D-308`). A shader that is not
    /// loaded, or a module that does not build on the post layouts, is an
    /// error; the effect is then built when its shader next loads, and a
    /// frame leaves its pass out until it has a pipeline.
    pub fn upload_post_effect(
        &mut self,
        id: PostEffectId,
        name: &str,
        shader_name: &str,
    ) -> Result<(), RenderError> {
        self.post_stack.register_custom(id, name, shader_name);
        let loaded = self
            .shader_ids
            .get(shader_name)
            .copied()
            .and_then(|shader| {
                Some((
                    self.shader_cache.text(shader)?,
                    self.shader_cache.get(shader)?,
                ))
            });
        let Some((wgsl, module)) = loaded else {
            return Err(RenderError::PostEffect {
                name: name.to_string(),
                reason: format!(
                    "its shader '{shader_name}' has no module yet; it is built when the shader loads"
                ),
            });
        };
        self.post_stack
            .upload_custom(&self.device, id, name, shader_name, wgsl, module)
            .map_err(|reason| RenderError::PostEffect {
                name: name.to_string(),
                reason: format!(
                    "its shader '{shader_name}' does not draw as a post pass: {reason}"
                ),
            })
    }

    /// Builds again every game post effect on shader `shader_name`, whose
    /// module just changed.
    fn rebuild_post_effects_for_shader(&mut self, shader_name: &str) {
        for (id, name) in self.post_stack.custom_on_shader(shader_name) {
            if let Err(e) = self.upload_post_effect(id, &name, shader_name) {
                log::error!("post effect '{name}' rebuild after shader reload failed: {e}");
            }
        }
    }

    fn build_material(
        &mut self,
        id: MaterialAssetId,
        name: &str,
        shader_name: &str,
        defaults: MaterialUniformDefaults,
    ) -> Result<(), RenderError> {
        let Some(shader_id) = self.shader_ids.get(shader_name).copied() else {
            return Err(RenderError::Shader(ShaderError::Validation {
                name: name.to_string(),
                report: format!("material references unknown shader id '{shader_name}'"),
            }));
        };
        let Some(module) = self.shader_cache.get(shader_id) else {
            return Err(RenderError::Shader(ShaderError::Validation {
                name: name.to_string(),
                report: format!("shader '{shader_name}' has no live module"),
            }));
        };

        // A build that fails validation keeps the last good material, or
        // none on first load (B2, `D-057`).
        let (pipeline, bind_group_layout) = build_validated(&self.device, || {
            build_material_pipeline(
                &self.device,
                module,
                self.sprite_pipeline.camera_bind_group_layout(),
                self.sprite_pipeline.texture_bind_group_layout(),
                self.scene_format(),
                self.sample_count,
                matches!(self.depth_sort, DepthSortMode::GpuDepth),
                name,
            )
        })
        .map_err(|report| {
            RenderError::Shader(ShaderError::Pipeline {
                name: shader_name.to_string(),
                report,
            })
        })?;
        // UBO slots bind the uniform layout of the pipeline they were built
        // for; the next draw builds and writes fresh ones before use.
        self.sprite_pipeline.reset_material_slots(id);

        self.materials.insert(
            id,
            MaterialPipeline {
                pipeline,
                bind_group_layout,
                defaults,
                name: name.to_string(),
                shader_id_name: shader_name.to_string(),
                material_id: id,
            },
        );
        log::info!("material '{name}' uploaded (shader={shader_name})");
        Ok(())
    }

    /// Hot-reload a material. Body-only edits: rebuilds the pipeline against
    /// the current shader module and the stored shader name. Validation
    /// failure logs and keeps the prior pipeline.
    pub fn reload_material(
        &mut self,
        id: MaterialAssetId,
        defaults: MaterialUniformDefaults,
    ) -> Result<(), RenderError> {
        let (shader_name, name) = {
            let Some(mp) = self.materials.get(&id) else {
                log::error!("material reload: unknown id {id:?}");
                return Ok(());
            };
            (mp.shader_id_name.clone(), mp.name.clone())
        };
        self.upload_material(id, &name, &shader_name, defaults)
    }

    /// Builds every material bound to `shader_name` again, those whose first
    /// upload failed included.
    fn rebuild_materials_for_shader(&mut self, shader_name: &str) {
        let mut ids_to_rebuild: Vec<(MaterialAssetId, String, MaterialUniformDefaults)> = self
            .materials
            .iter()
            .filter(|(_, mp)| mp.shader_id_name == shader_name)
            .map(|(id, mp)| (*id, mp.name.clone(), mp.defaults))
            .collect();
        ids_to_rebuild.extend(
            self.pending_materials
                .on_shader(shader_name)
                .into_iter()
                .map(|(id, pending)| (id, pending.name, pending.defaults)),
        );
        for (id, name, defaults) in ids_to_rebuild {
            if let Err(e) = self.upload_material(id, &name, shader_name, defaults) {
                log::error!("material '{name}' rebuild after shader reload failed: {e}");
            }
        }
    }

    /// Reconfigure surface after resize. A side larger than the device's
    /// textures is clamped to them, with a warning. The frame's targets
    /// follow at the next frame's plan ([`Self::plan_frame`]).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            let (width, height) = self.clamp_to_device(width, height);
            if (width, height) != (self.surface_config.width, self.surface_config.height) {
                self.target_resizes += 1;
            }
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            if let Some(smaa) = self.smaa.as_ref() {
                smaa.update_preset(&self.queue, self.post_aa, (width, height));
            }
        }
    }

    /// The window's size as a surface size: each side clamped to the
    /// device's textures; zero while minimized.
    fn window_extent(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        surface_extent(
            (size.width, size.height),
            self.device.limits().max_texture_dimension_2d,
        )
    }

    /// `width` × `height` clamped to the device's textures, warning when
    /// that cuts it.
    fn clamp_to_device(&self, width: u32, height: u32) -> (u32, u32) {
        let clamped = surface_extent(
            (width, height),
            self.device.limits().max_texture_dimension_2d,
        );
        if clamped != (width, height) {
            log::warn!(
                "window {width}x{height} is larger than the device's textures; rendering at {}x{}",
                clamped.0,
                clamped.1
            );
        }
        clamped
    }

    /// How the next frame reaches the surface (`D-087`). A capture reads the
    /// composed frame, so its frame draws offscreen and a blit shows it; a
    /// direct capture draws the direct path into a stand-in for the surface,
    /// which is read back and then blitted. The armed capture picks the
    /// path; the acquire takes it.
    fn present_path(&self) -> PresentPath {
        match &self.pending_capture {
            None => PresentPath::Direct,
            Some(capture) if capture.direct => PresentPath::DirectCapture,
            Some(_) => PresentPath::Capture,
        }
    }

    /// Plans the frame the next record draws. Its shape is everything the
    /// frame's declaration depends on: the surface's size and format, the
    /// config, how the frame reaches the surface, and the stack it draws
    /// with the stages the composer fuses (`D-307`). While the shape is the
    /// last frame's, the compiled frame and its textures stand, and every
    /// bind group over them with them. A new shape is declared and
    /// compiled, and its slots take pooled textures.
    fn plan_frame(&mut self, present: PresentPath, post_stack: &PostStack) {
        let output = (self.surface_config.width, self.surface_config.height);
        let halvings = self.bloom_base.halvings(output.1);
        let shape = &mut self.shape_scratch;
        shape.output = output;
        shape.format = self.surface_config.format;
        shape.samples = self.sample_count;
        shape.depth = self.depth_enabled && self.depth_sort == DepthSortMode::GpuDepth;
        shape.smaa = self.post_aa.is_smaa() && self.smaa.is_some();
        shape.present = present;
        let replaced = &self.replaced;
        shape.set_stages(post_stack, |pass| replaced.own(pass));
        shape.replaced = replaced.stages();
        // The composer's rule: the stages that draw in one pass. A run
        // joins only once its pass is built, so a failed build leaves its
        // stages drawn alone.
        shape.float = self.float;
        shape.tail_alone = self.float.is_some();
        let fuses = if self.float.is_some() {
            self.post_fuse && !post_stack.is_empty()
        } else {
            self.post_fuse && post_stack.len() >= 2 && !shape.smaa
        };
        if fuses {
            let composite = !shape.replaced.holds(EngineStage::BloomComposite);
            self.stage_infos.clear();
            self.stage_infos
                .extend(post_stack.0.iter().zip(&shape.stages).map(|(pass, stage)| {
                    let own = match stage {
                        Stage::Stock { own, .. } => *own,
                        Stage::Bloom => composite,
                    };
                    self.post_stack.stage_info(pass, own)
                }));
        }
        if fuses && self.float.is_some() {
            // Under a float scene (`D-309`) the float rule's runs fuse, and
            // the stack's last pass ends in the frame's tail where every
            // stage of it has a section; else the tail is a pass of its own.
            let tail = self.post_stack.tail();
            compose::runs_into(&self.stage_infos, true, &mut self.run_scratch);
            for (index, run) in self.run_scratch.iter().enumerate() {
                let stages = &post_stack.0[run.clone()];
                if compose::tail_rides(&self.stage_infos, &self.run_scratch, index)
                    && self
                        .post_stack
                        .fused_ready(&self.device, stages, Between::Nothing, tail)
                {
                    shape.tail_alone = false;
                    shape.join(run.clone());
                } else if run.len() > 1
                    && self.post_stack.fused_ready(
                        &self.device,
                        stages,
                        Between::Nothing,
                        Finish::Plain,
                    )
                {
                    shape.join(run.clone());
                }
            }
        } else if fuses
            && let Some(pair) = compose::fused_pair(&self.stage_infos, self.post_fuse, shape.smaa)
            && self.post_stack.fused_ready(
                &self.device,
                &post_stack.0[pair.clone()],
                Between::Clamp,
                Finish::Plain,
            )
        {
            // The two stages that draw in one pass (`compose::fused_pair`).
            shape.join(pair);
        }
        shape.bloom_halvings = halvings;
        shape.bloom_mips =
            bloom_mip_count_for_halvings(output.0, output.1, self.bloom_max_mips, halvings);

        if let Some(frame) = &self.frame
            && frame.shape == *shape
        {
            self.targets.touch(&frame.bound);
            return;
        }
        let compiled = declare(shape)
            .compile()
            .expect("the engine's frame draws over no target before a pass writes it");
        let device = &self.device;
        let bound = self
            .targets
            .bind(&compiled.slots, |spec, _| PoolTexture::new(device, spec));
        // A resize or a presentation-AA switch leaves no use for the
        // textures of the frames before it: they go now, not after the
        // pool's keep frames.
        let stale = self.frame.as_ref().is_some_and(|frame| {
            frame.shape.output != shape.output || frame.shape.smaa != shape.smaa
        });
        let freed = if stale {
            self.targets.keep_only(&bound)
        } else {
            Vec::new()
        };
        self.frame = Some(FramePlan {
            shape: shape.clone(),
            compiled,
            bound,
        });
        self.forget_targets(&freed);
    }

    /// Drops every cached bind group that holds a view of one of the pooled
    /// textures `freed`, so their memory goes with them.
    fn forget_targets(&mut self, freed: &[u64]) {
        if freed.is_empty() {
            return;
        }
        self.post_stack.forget_targets(freed);
        self.present_blit.forget_targets(freed);
        if let Some(smaa) = self.smaa.as_mut() {
            smaa.forget_targets(freed);
        }
    }

    /// Current post-AA mode.
    #[must_use]
    pub fn post_aa(&self) -> PostAaMode {
        self.post_aa
    }

    /// Switch presentation AA at a frame boundary, without relaunch: the
    /// next frame's plan takes SMAA's targets or frees them. Must not be called from
    /// within `render_frame_internal`. Turning SMAA on builds its pipelines
    /// first: a stage shader reloaded while SMAA was off was committed
    /// without one, so a build that fails validation logs and leaves the
    /// mode off ([`Self::post_aa`] reads what applied).
    pub fn set_post_aa(&mut self, mode: PostAaMode) {
        if mode == self.post_aa {
            return;
        }
        let size = (self.surface_config.width, self.surface_config.height);
        if mode.is_smaa() && self.smaa.is_none() {
            match build_smaa_pipeline(
                &self.device,
                &self.queue,
                self.surface_config.format,
                &self.shader_cache,
                self.smaa_shader_ids,
                mode,
                size,
            ) {
                Ok(smaa) => self.smaa = Some(smaa),
                Err(report) => {
                    log::error!(
                        "SMAA pipeline failed validation; post_aa stays {:?}: {report}",
                        self.post_aa
                    );
                    return;
                }
            }
        }
        self.post_aa = mode;
        if let Some(smaa) = self.smaa.as_ref()
            && self.post_aa.is_smaa()
        {
            smaa.update_preset(&self.queue, self.post_aa, size);
        } else {
            self.smaa = None;
        }
    }

    pub fn reconfigure_surface_pacing(
        &mut self,
        present_mode: Option<PresentModeConfig>,
        vsync: bool,
        max_frame_latency: Option<u32>,
    ) -> Result<(), RenderError> {
        let surface_caps = self.surface.get_capabilities(&self.adapter);
        let resolved_present_mode =
            resolve_present_mode(&surface_caps.present_modes, present_mode, vsync)?;
        let resolved_max_frame_latency =
            resolve_max_frame_latency(max_frame_latency, resolved_present_mode)?;

        self.surface_config.present_mode = resolved_present_mode;
        self.surface_config.desired_maximum_frame_latency = resolved_max_frame_latency;
        self.surface.configure(&self.device, &self.surface_config);

        self.gpu_timings.present_mode = Some(present_mode_label(resolved_present_mode).to_string());
        self.gpu_timings.max_frame_latency = Some(resolved_max_frame_latency);
        Ok(())
    }

    /// Acquire the frame's surface texture following `surface_acquire`'s
    /// policy. `Ok(None)` skips the frame; the `bool` asks for a reconfigure
    /// after the texture is presented (suboptimal surface).
    fn acquire_texture(&mut self) -> Result<Option<(wgpu::SurfaceTexture, bool)>, RenderError> {
        for attempt in 0..MAX_ACQUIRE_ATTEMPTS {
            let result = self.surface.get_current_texture();
            let status = AcquireStatus::of(&result);
            match acquire_action(status, attempt) {
                AcquireAction::Render { reconfigure_after } => {
                    if status == AcquireStatus::Success {
                        self.suboptimal_reconfigured = None;
                    }
                    return Ok(match result {
                        wgpu::CurrentSurfaceTexture::Success(tex)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => {
                            Some((tex, reconfigure_after))
                        }
                        _ => None,
                    });
                }
                AcquireAction::Skip => {
                    log::debug!("frame skipped: surface acquire returned {status:?}");
                    return Ok(None);
                }
                // Outdated and Lost carry no texture, so nothing is held while
                // the surface is reconfigured or replaced.
                AcquireAction::ReconfigureAndRetry => self.sync_surface_to_window(),
                AcquireAction::RecreateAndRetry => self.recreate_surface()?,
                AcquireAction::Fail => {
                    return Err(RenderError::Surface(format!(
                        "surface acquire failed ({status:?}); device recovery is not supported"
                    )));
                }
            }
        }
        Ok(None)
    }

    /// Replace a lost surface with a new one for the same window.
    fn recreate_surface(&mut self) -> Result<(), RenderError> {
        log::warn!("Surface lost; recreating it");
        // Assigning drops the old surface and its swapchain before the new
        // surface is configured, so the window never has two swapchains.
        self.surface = self
            .instance
            .create_surface(self.window.clone())
            .map_err(|e| RenderError::Surface(format!("surface recreation failed: {e}")))?;
        self.surface.configure(&self.device, &self.surface_config);
        Ok(())
    }

    /// Reconfigure the surface, first picking up a window size change whose
    /// resize event hasn't arrived yet (a zero size means minimized: keep the
    /// current size).
    fn sync_surface_to_window(&mut self) {
        let size = self.window_extent();
        let current = (self.surface_config.width, self.surface_config.height);
        if size.0 > 0 && size.1 > 0 && size != current {
            self.resize(size.0, size.1);
        } else {
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    /// Refresh the surface after presenting a suboptimal frame, at most once
    /// per unchanged size.
    fn reconfigure_after_suboptimal(&mut self) {
        let size = self.window_extent();
        let configured = (self.surface_config.width, self.surface_config.height);
        if reconfigure_for_suboptimal(size, configured, self.suboptimal_reconfigured) {
            self.sync_surface_to_window();
            self.suboptimal_reconfigured =
                Some((self.surface_config.width, self.surface_config.height));
        }
    }

    /// Clear-only frame.
    pub fn render_frame(&mut self) -> Result<RenderOutcome, RenderError> {
        self.render_frame_with_quads(&[])
    }

    /// D-018 direct-data quad frame with default pixel ortho.
    pub fn render_frame_with_quads(
        &mut self,
        quads: &[QuadInstance],
    ) -> Result<RenderOutcome, RenderError> {
        let w = self.surface_config.width as f32;
        let h = self.surface_config.height as f32;
        let default_view_proj =
            glam::camera::rh::proj::directx::orthographic(0.0, w, h, 0.0, -1.0, 1.0);
        let empty_stack = PostStack::default();
        let empty_ui = PaintList::default();
        self.render_frame_full(
            &default_view_proj,
            quads,
            &[],
            &[],
            &[],
            &[],
            &empty_ui,
            &empty_stack,
        )
    }

    /// Render quads, sprites, debug primitives, the UI paint list and
    /// screen-space text. The UI draws in the overlay pass in paint order,
    /// before the text sections (M46); an empty list records no UI draw.
    ///
    /// # Errors
    ///
    /// The failed frame: a surface lost again right after its recreation, or
    /// a failed recreation (`D-142`). A skipped frame is
    /// [`RenderOutcome::Skipped`], not an error.
    #[allow(clippy::too_many_arguments)]
    pub fn render_frame_full(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
        ui: &PaintList,
        post_stack: &PostStack,
    ) -> Result<RenderOutcome, RenderError> {
        self.gpu_timings.clear_durations();
        self.with_drawn_stack(post_stack, |renderer, post_stack| {
            renderer.render_frame_internal(
                view_proj,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
                text_sections,
                ui,
                post_stack,
                None,
            )
        })
    }

    /// Runs `frame` with the stack this frame draws: `post_stack`, less the
    /// passes of a game's effect that has no pipeline (`D-308`). Its module
    /// did not build, so the frame is declared, timed and drawn without it,
    /// and the effect is named in the log once.
    fn with_drawn_stack<R>(
        &mut self,
        post_stack: &PostStack,
        frame: impl FnOnce(&mut Self, &PostStack) -> R,
    ) -> R {
        if post_stack.0.iter().all(|pass| self.post_stack.draws(pass)) {
            return frame(self, post_stack);
        }
        self.post_stack.warn_left_out(&post_stack.0);
        let mut drawn = std::mem::take(&mut self.stack_scratch);
        drawn_stack(
            &post_stack.0,
            |pass| self.post_stack.draws(pass),
            &mut drawn.0,
        );
        let outcome = frame(self, &drawn);
        self.stack_scratch = drawn;
        outcome
    }

    /// The app's number for the frame it renders next, which a timed
    /// frame's [`GpuFrameSample`] names when it is read back.
    pub fn set_frame_number(&mut self, frame: u64) {
        self.frame_number = frame;
    }

    /// The last rendered frame's draw calls and upload bytes, the engine's
    /// own, and its glyphon `render` calls: what was recorded between the end
    /// of the frame before and the end of this one.
    #[must_use]
    pub fn frame_counters(&self) -> FrameCounters {
        self.frame_counters
    }

    /// Adds every pipeline's counts to a frame's total and starts them over.
    fn take_counts(&self) -> FrameCounters {
        let mut total = FrameCounters::default();
        self.counts.take_into(&mut total);
        self.quad_pipeline.counts.take_into(&mut total);
        self.sprite_pipeline.counts.take_into(&mut total);
        self.debug_line_pipeline.counts.take_into(&mut total);
        self.mesh_particle_pipeline.counts.take_into(&mut total);
        self.ui_pipeline.counts.take_into(&mut total);
        self.lighting.counts.take_into(&mut total);
        self.post_stack.take_counts(&mut total);
        if let Some(smaa) = &self.smaa {
            smaa.take_counts(&mut total);
        }
        self.text_pipeline.take_counts(&mut total);
        total
    }

    /// Timed full frame. It never waits on the GPU: the timestamps of a
    /// frame resolve into a slot of a ring of four
    /// readbacks, mapped after its submit and read by a later frame's
    /// non-blocking poll, into [`GpuFrameTimings::samples`] with the frame
    /// they measured. A frame that finds every slot in flight runs untimed
    /// and counts in [`GpuFrameTimings::skipped_frames`].
    ///
    /// # Errors
    ///
    /// As [`Renderer::render_frame_full`].
    #[allow(clippy::too_many_arguments)]
    pub fn render_frame_full_timed(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
        ui: &PaintList,
        post_stack: &PostStack,
    ) -> Result<RenderOutcome, RenderError> {
        self.with_drawn_stack(post_stack, |renderer, post_stack| {
            renderer.render_frame_timed(
                view_proj,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
                text_sections,
                ui,
                post_stack,
            )
        })
    }

    /// [`Self::render_frame_full_timed`] over the stack the frame draws.
    #[allow(clippy::too_many_arguments)]
    fn render_frame_timed(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
        ui: &PaintList,
        post_stack: &PostStack,
    ) -> Result<RenderOutcome, RenderError> {
        self.gpu_timings.samples.clear();
        if !self.timestamp_support {
            self.gpu_timings.clear_durations();
            return self.render_frame_internal(
                view_proj,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
                text_sections,
                ui,
                post_stack,
                None,
            );
        }
        // Reads the slots whose maps an earlier frame left completing.
        let _ = self.device.poll(wgpu::PollType::Poll);
        let period = self.queue.get_timestamp_period();
        let out = &mut self.gpu_timings;
        self.timing_ring
            .complete(TimingResources::mapped, |frame, slot| {
                slot.read(frame, period, out);
            });
        // The ring is sized for the frame's passes and one more, a capture's
        // blit, so a capture frame rebuilds nothing.
        let present = self.present_path();
        self.plan_frame(present, post_stack);
        let passes = self
            .frame
            .as_ref()
            .map_or(0, |frame| frame.compiled.passes.len());
        let layout = TimingLayout::new(
            passes + usize::from(present == PresentPath::Direct),
            (self.surface_config.width, self.surface_config.height),
        );
        let device = &self.device;
        let count = layout.query_count();
        let rebuilt = self.timing_ring.ensure(layout, || {
            count.map(|count| TimingResources::new(device, count))
        });
        if rebuilt && count.is_none() {
            log::warn!(
                "GPU pass timing exceeds the timestamp query limit; rendering without timings"
            );
        }
        let mut taken = self.timing_ring.take();
        if let Some((_, slot)) = taken.as_mut() {
            slot.begin();
        }
        let result = self.render_frame_internal(
            view_proj,
            quads,
            sprite_batches,
            debug_quads,
            debug_lines,
            text_sections,
            ui,
            post_stack,
            taken.as_mut().map(|(_, slot)| slot),
        );
        if let Some((index, slot)) = taken {
            if matches!(result, Ok(RenderOutcome::Presented { .. })) && slot.resolved() {
                slot.map();
                self.timing_ring.submit(index, slot, self.frame_number);
            } else {
                self.timing_ring.release(index, slot);
            }
        }
        self.gpu_timings.skipped_frames = self.timing_ring.skipped();
        result
    }

    #[allow(clippy::too_many_arguments)] // stable frame-extract surface mirrors render_frame_full.
    fn render_frame_internal(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
        ui: &PaintList,
        post_stack: &PostStack,
        mut timing: Option<&mut TimingResources>,
    ) -> Result<RenderOutcome, RenderError> {
        self.cpu_timings = CpuFrameTimings::default();

        // The armed capture picks the frame's path here; the acquire takes
        // it.
        let present_path = self.present_path();
        self.plan_frame(present_path, post_stack);
        let draws = FrameDraws {
            quads,
            sprite_batches,
            debug_quads,
            debug_lines,
            ui,
            post_stack,
        };

        // Late acquire (`D-145`): the passes that draw into no imported
        // texture record, and submit, before the acquire waits for the
        // swapchain.
        let encode_start = Instant::now();
        let mut first =
            self.record_before_acquire(view_proj, text_sections, draws, timing.as_deref_mut());
        let mut encode_ms = encode_start.elapsed().as_secs_f64() as f32 * 1000.0;
        let mut submit_ms = 0.0;
        if SUBMIT_BEFORE_ACQUIRE && let Some(buffer) = first.take() {
            let submit_start = Instant::now();
            self.queue.submit(std::iter::once(buffer));
            submit_ms = submit_start.elapsed().as_secs_f64() as f32 * 1000.0;
        }
        self.cpu_timings.encode_pre_acquire_ms = encode_ms;
        self.cpu_timings.encode_ms = encode_ms;
        self.cpu_timings.submit_present_ms = submit_ms;

        let targets_before = self.targets_at();
        let acquire_start = Instant::now();
        let acquired = self.acquire_texture();
        // The capture armed for this frame is its alone: taken whatever the
        // acquire gave, it survives only into a frame that presents.
        let capture = capture_for_frame(&mut self.pending_capture, &acquired);
        let (output, reconfigure_after) = match acquired {
            Ok(Some(acquired)) => acquired,
            // A skipped frame keeps its prepare: the trim rule trims before
            // the next full frame (`D-147`).
            Ok(None) => return Ok(RenderOutcome::Skipped),
            Err(err) => return Err(err),
        };
        self.cpu_timings.acquire_ms = acquire_start.elapsed().as_secs_f64() as f32 * 1000.0;

        // An acquire that resized the surface leaves the first part drawn at
        // the old size: plan the frame at the new one and record that part
        // again, untimed, since the ring's slots are sized for the old
        // layout; the next frame rebuilds them.
        if record_again_after_acquire(targets_before, self.targets_at()) {
            log::debug!(
                "late acquire: the acquire resized the surface to {}x{}; recording the passes before the swapchain stage again",
                self.surface_config.width,
                self.surface_config.height
            );
            let record_start = Instant::now();
            if let Some(slot) = timing.take() {
                slot.begin();
            }
            self.plan_frame(present_path, post_stack);
            first =
                self.record_before_acquire(view_proj, text_sections, draws, timing.as_deref_mut());
            encode_ms += record_start.elapsed().as_secs_f64() as f32 * 1000.0;
        }

        let record_start = Instant::now();
        let w = self.surface_config.width;
        let h = self.surface_config.height;
        let swap_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        // A direct capture's frame ends on a stand-in for the surface, which
        // the capture reads and the frame's last pass blits.
        let standin = (present_path == PresentPath::DirectCapture)
            .then(|| create_swapchain_standin(&self.device, self.surface_config.format, w, h));
        let frame_view = standin.as_ref().map_or(&swap_view, |(_, view)| view);
        // The frame's imported textures in their order of import: its
        // output, then the surface beside a stand-in.
        let imports = [frame_view, &swap_view];

        encoder.push_debug_group("tungsten_frame");
        let (split, pass_count) = self.frame.as_ref().map_or((0, 0), |frame| {
            (frame.compiled.split, frame.compiled.passes.len())
        });
        self.record_passes(
            &mut encoder,
            split..pass_count,
            draws,
            &imports,
            timing.as_deref_mut(),
        );

        // Screenshot path: readback of what the frame's declaration marks as
        // copied from: the version the blit shows, or the stand-in of a
        // direct capture.
        let capture_target = capture
            .as_ref()
            .map(|_| create_capture_target(&self.device, w, h));
        if let Some(target) = capture_target.as_ref() {
            let frame = self.frame.as_ref();
            let source = match frame.and_then(|frame| frame.compiled.copied) {
                Some(Target::Slot(slot)) => {
                    frame.map(|frame| &self.targets.get(frame.bound[usize::from(slot)]).texture)
                }
                Some(Target::Imported(_)) => standin.as_ref().map(|(texture, _)| texture),
                None => None,
            };
            let source = source.expect("a capture frame's declaration names what it copies");
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: source,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &target.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(target.padded_bytes_per_row),
                        rows_per_image: Some(h),
                    },
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }

        if let Some(t) = timing {
            t.resolve(&mut encoder);
        }

        encoder.pop_debug_group();

        let finished = encoder.finish();
        encode_ms += record_start.elapsed().as_secs_f64() as f32 * 1000.0;
        self.cpu_timings.encode_ms = encode_ms;

        let submit_present_start = Instant::now();
        // The first part, when it was not submitted before the acquire or
        // was recorded again after it, goes first.
        self.queue
            .submit(first.into_iter().chain(std::iter::once(finished)));
        self.queue.present(output);

        let capture = capture.zip(capture_target).map(|(capture, target)| {
            finalize_capture(
                &self.device,
                &target,
                self.surface_config.format,
                w,
                h,
                &capture.path,
            )
            .map(|()| capture.path)
        });

        self.cpu_timings.submit_present_ms =
            submit_ms + submit_present_start.elapsed().as_secs_f64() as f32 * 1000.0;

        // The presented texture is released, so the surface can be reconfigured.
        if reconfigure_after {
            self.reconfigure_after_suboptimal();
        }
        self.frame_counters = self.take_counts();

        // A pooled texture no frame has used for the keep frames and the
        // keep time goes, and every bind group over it.
        let freed = self.targets.end_frame(self.started.elapsed());
        self.forget_targets(&freed);

        Ok(RenderOutcome::Presented { capture })
    }

    /// The surface's size and how many resizes have changed it, which an
    /// acquire can move.
    fn targets_at(&self) -> TargetsAt {
        TargetsAt {
            resizes: self.target_resizes,
            size: (self.surface_config.width, self.surface_config.height),
        }
    }

    /// The frame's work before the surface acquire (`D-145`): uploads the
    /// camera, prepares the text and records the planned frame's passes
    /// before its split, the first that draws into an imported texture, into
    /// a command buffer of their own; `None` when the scene pass is that
    /// pass. A frame whose acquire resized the surface plans again and calls
    /// it again at the new size.
    fn record_before_acquire(
        &mut self,
        view_proj: &glam::Mat4,
        text_sections: &[TextSection],
        draws: FrameDraws<'_>,
        timing: Option<&mut TimingResources>,
    ) -> Option<wgpu::CommandBuffer> {
        // Both camera buffers hold the last matrix written; a camera that
        // stands still uploads nothing.
        let camera: [u8; 64] = bytemuck::cast(view_proj.to_cols_array());
        if self.camera_written != Some(camera) {
            self.quad_pipeline.update_camera(&self.queue, view_proj);
            self.sprite_pipeline.update_camera(&self.queue, view_proj);
            self.camera_written = Some(camera);
        }
        self.text_pipeline.prepare(
            &self.device,
            &self.queue,
            text_sections,
            draws.ui,
            self.surface_config.width,
            self.surface_config.height,
        );
        self.ui_pipeline.upload(
            &self.device,
            &self.queue,
            draws.ui,
            self.surface_config.width,
            self.surface_config.height,
        );
        let split = self.frame.as_ref().map_or(0, |frame| frame.compiled.split);
        if split == 0 {
            return None;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder_before_acquire"),
            });
        encoder.push_debug_group("tungsten_frame_before_acquire");
        self.record_passes(&mut encoder, 0..split, draws, &[], timing);
        encoder.pop_debug_group();
        Some(encoder.finish())
    }

    /// Records the planned frame's passes at the indices `passes` into
    /// `encoder`, each by its kind. `imports` are the views of the frame's
    /// imported textures in their order of import: its output (the
    /// surface's texture, or a direct capture's stand-in), then the surface
    /// beside a stand-in. The passes before the split draw into none, and
    /// record without them (`D-145`).
    fn record_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        passes: std::ops::Range<usize>,
        draws: FrameDraws<'_>,
        imports: &[&wgpu::TextureView],
        mut timing: Option<&mut TimingResources>,
    ) {
        let FrameDraws {
            quads,
            sprite_batches,
            debug_quads,
            debug_lines,
            post_stack,
            ..
        } = draws;
        let Some(frame) = self.frame.as_ref() else {
            return;
        };
        let views = FrameViews {
            targets: &self.targets,
            bound: &frame.bound,
            imports,
        };
        let attachment = (self.surface_config.width, self.surface_config.height);

        for pass in &frame.compiled.passes[passes] {
            // A bloom slot's blocks and bind groups are readied before its
            // first pass opens; its later passes draw with them, the
            // composite on the far side of the acquire where it writes the
            // output.
            if let PassKind::BloomThreshold { slot } = pass.kind
                && let Some(PostPass::Bloom(params)) = post_stack.0.get(slot)
                && let Some(source) = views.sampled(pass.reads.first())
                && let Some(pyramid) = views.pooled(pass.color.at)
            {
                self.post_stack.bloom.prepare_slot(
                    &self.device,
                    &self.queue,
                    slot,
                    params,
                    attachment,
                    frame.shape.bloom_halvings,
                    source,
                    pyramid,
                    frame.shape.bloom_mips.max(1),
                );
            }

            let timestamp_writes = timing.as_deref_mut().map(|t| t.next(pass.timing.clone()));
            let mut render_pass = begin_pass(
                encoder,
                pass,
                views.view(pass.color.at),
                pass.resolve.map(|at| views.view(at)),
                pass.depth
                    .and_then(|depth| {
                        views.pooled(At {
                            target: Target::Slot(depth.slot),
                            mip: 0,
                        })
                    })
                    .map(|(_, texture)| texture.view(0)),
                self.clear_color,
                timestamp_writes,
            );

            // What a generated pass does between its stages, and last where
            // it writes the frame's 8-bit output from a float scene.
            let between = if self.float.is_some() {
                Between::Nothing
            } else {
                Between::Clamp
            };
            let tail_finish = self.post_stack.tail();
            let drew = match pass.kind {
                PassKind::Scene => {
                    record_main_draws(
                        &mut render_pass,
                        &self.device,
                        &self.queue,
                        &self.quad_pipeline,
                        &mut self.sprite_pipeline,
                        &self.debug_line_pipeline,
                        &self.mesh_particle_pipeline,
                        quads,
                        sprite_batches,
                        debug_quads,
                        debug_lines,
                        &self.materials,
                        &self.lit_sprite_pipeline,
                        &self.lighting,
                        attachment,
                    );
                    true
                }
                // A stock effect alone draws with its own pipeline; with
                // the stages joined to it, with the pass the composer
                // generated for them (`D-307`).
                PassKind::Post { slot, stages, tail } => {
                    if let Some(run) = post_stack.0.get(slot..slot + stages)
                        && let Some(source) = views.sampled(pass.reads.first())
                    {
                        for stage in run {
                            self.post_stack.warn_inert(stage, self.clear_color.a as f32);
                        }
                        match run {
                            [post_pass] if !tail => self.post_stack.record_pass(
                                &self.device,
                                &self.queue,
                                &mut render_pass,
                                source,
                                post_pass,
                                slot,
                            ),
                            _ => self.post_stack.record_fused(
                                &self.device,
                                &self.queue,
                                &mut render_pass,
                                Some(source),
                                run,
                                slot,
                                between,
                                if tail { tail_finish } else { Finish::Plain },
                            ),
                        }
                    } else {
                        false
                    }
                }
                PassKind::BloomThreshold { slot } => self.post_stack.bloom.draw_stage(
                    &mut render_pass,
                    slot,
                    BloomStageKind::Threshold,
                ),
                PassKind::BloomDown { slot, level } => self.post_stack.bloom.draw_stage(
                    &mut render_pass,
                    slot,
                    BloomStageKind::Down(level),
                ),
                PassKind::BloomUp { slot, level } => self.post_stack.bloom.draw_stage(
                    &mut render_pass,
                    slot,
                    BloomStageKind::Up(level),
                ),
                PassKind::BloomComposite {
                    slot,
                    stages: 1,
                    tail: false,
                } => self.post_stack.bloom.draw_stage(
                    &mut render_pass,
                    slot,
                    BloomStageKind::Composite,
                ),
                // The composite with the stages joined to it, or with the
                // frame's tail, in the pass the composer generated: it binds
                // the slot's own groups.
                PassKind::BloomComposite { slot, stages, tail } => {
                    if let Some(run) = post_stack.0.get(slot..slot + stages) {
                        for stage in &run[1..] {
                            self.post_stack.warn_inert(stage, self.clear_color.a as f32);
                        }
                        self.post_stack.record_fused(
                            &self.device,
                            &self.queue,
                            &mut render_pass,
                            None,
                            run,
                            slot,
                            between,
                            if tail { tail_finish } else { Finish::Plain },
                        )
                    } else {
                        false
                    }
                }
                // The frame's tail alone (`D-309`): the float colour before
                // it, clamped and dithered into the 8-bit output.
                PassKind::Tail { slot } => match views.sampled(pass.reads.first()) {
                    Some(source) => self.post_stack.record_fused(
                        &self.device,
                        &self.queue,
                        &mut render_pass,
                        Some(source),
                        &[],
                        slot,
                        Between::Nothing,
                        tail_finish,
                    ),
                    None => false,
                },
                // The edge test reads the stack's last version through its
                // non-sRGB twin where it has one, so it sees encoded values.
                PassKind::SmaaEdges => {
                    if let Some(smaa) = self.smaa.as_mut()
                        && let Some((id, texture)) =
                            pass.reads.first().and_then(|read| views.pooled(*read))
                    {
                        smaa.record_edge_pass(
                            &self.device,
                            &mut render_pass,
                            (id, texture.edge_view()),
                        );
                        true
                    } else {
                        false
                    }
                }
                PassKind::SmaaWeights => {
                    if let Some(smaa) = self.smaa.as_mut()
                        && let Some(edges) = views.sampled(pass.reads.first())
                    {
                        smaa.record_blend_weights_pass(&self.device, &mut render_pass, edges);
                        true
                    } else {
                        false
                    }
                }
                // The neighborhood pass's target encodes on store, so it
                // reads the stack's last version through its own sRGB view:
                // the twin would encode the frame twice and wash it out.
                PassKind::SmaaNeighborhood => {
                    if let Some(smaa) = self.smaa.as_mut()
                        && let Some(source) = views.sampled(pass.reads.first())
                        && let Some(weights) = views.sampled(pass.reads.get(1))
                    {
                        smaa.record_neighborhood_pass(
                            &self.device,
                            &mut render_pass,
                            source,
                            weights,
                        );
                        true
                    } else {
                        false
                    }
                }
                PassKind::Overlay => {
                    // M54: the world labels first, under the paint list's game
                    // layers (`D-161`); a frame without labels records nothing
                    // here. M46: the UI's items in paint order, each setting its
                    // scissor, then the text sections under the whole attachment.
                    // An empty paint list records nothing here.
                    if self.text_pipeline.has_labels() {
                        render_pass.push_debug_group("world labels");
                        self.text_pipeline.render_labels(&mut render_pass);
                        render_pass.pop_debug_group();
                    }
                    let items = self.ui_pipeline.items();
                    if !items.is_empty() {
                        render_pass.push_debug_group("ui");
                        for item in items {
                            match item {
                                DrawItem::Primitives { .. } => self.ui_pipeline.draw_primitives(
                                    &mut render_pass,
                                    item,
                                    &self.sprite_pipeline,
                                    attachment,
                                ),
                                DrawItem::Mesh { .. } => {
                                    self.ui_pipeline
                                        .draw_mesh(&mut render_pass, item, attachment);
                                }
                                DrawItem::Text { batch, scissor } => {
                                    // glyphon clips through `TextBounds` and
                                    // leaves the scissor as it finds it, so the
                                    // batch draws under the viewport's.
                                    if let Some((x, y, width, height)) =
                                        attachment_scissor(*scissor, attachment)
                                    {
                                        render_pass.set_scissor_rect(x, y, width, height);
                                        self.text_pipeline.render_batch(&mut render_pass, *batch);
                                    }
                                }
                            }
                        }
                        render_pass.set_scissor_rect(0, 0, attachment.0, attachment.1);
                        render_pass.pop_debug_group();
                    }
                    render_pass.push_debug_group("text");
                    self.text_pipeline.render(&mut render_pass);
                    render_pass.pop_debug_group();
                    true
                }
                // A capture frame's blit samples the composed frame: a
                // pooled texture, its bind group kept while the texture
                // lives, or a direct capture's stand-in, built for the
                // frame.
                PassKind::Blit => {
                    let bind_group = pass.reads.first().map(|read| match views.pooled(*read) {
                        Some((id, texture)) => {
                            self.present_blit
                                .bind_group(&self.device, id, texture.view(read.mip))
                        }
                        None => self
                            .present_blit
                            .bind_group_for_view(&self.device, views.view(*read)),
                    });
                    if let Some(bind_group) = bind_group {
                        render_pass.set_pipeline(&self.present_blit.pipeline);
                        render_pass.set_bind_group(0, &bind_group, &[]);
                        counters::draw(&self.counts, &mut render_pass, 0..3, 0..1);
                        true
                    } else {
                        false
                    }
                }
            };
            // What the surface held is undefined under a pass that loads
            // nothing, so that pass must have drawn. The plan and the
            // records read one stack and one set of textures, so no guard
            // above fails; this holds a later change to that.
            assert!(
                drew || pass.color.load != Load::Nothing,
                "the pass `{}` loads nothing and drew nothing",
                pass.label
            );
        }
    }
}

/// A compiled frame and the pooled textures bound to its slots. It stands
/// while the frame's shape holds, so the bind groups over those textures
/// hold from frame to frame.
struct FramePlan {
    shape: Shape,
    compiled: Compiled<PassKind>,
    /// The identity of each slot's pooled texture.
    bound: Vec<u64>,
}

/// The textures a planned frame's passes name: the pooled ones by the
/// identities bound to the frame's slots, the imported ones by their views.
struct FrameViews<'f> {
    targets: &'f Pool<PoolTexture>,
    bound: &'f [u64],
    imports: &'f [&'f wgpu::TextureView],
}

impl<'f> FrameViews<'f> {
    /// The pooled texture `at` names, with its identity; `None` for an
    /// imported one.
    fn pooled(&self, at: At) -> Option<(u64, &'f PoolTexture)> {
        match at.target {
            Target::Slot(slot) => {
                let id = self.bound[usize::from(slot)];
                Some((id, self.targets.get(id)))
            }
            Target::Imported(_) => None,
        }
    }

    /// The pooled level a pass samples, with its texture's identity.
    fn sampled(&self, read: Option<&At>) -> Option<(u64, &'f wgpu::TextureView)> {
        let read = *read?;
        self.pooled(read)
            .map(|(id, texture)| (id, texture.view(read.mip)))
    }

    /// The view of the level `at` names.
    ///
    /// # Panics
    ///
    /// On an imported texture before the surface acquire: the frame's split
    /// keeps every pass that names one after it.
    fn view(&self, at: At) -> &'f wgpu::TextureView {
        match at.target {
            Target::Slot(slot) => self.targets.get(self.bound[usize::from(slot)]).view(at.mip),
            Target::Imported(index) => self
                .imports
                .get(usize::from(index))
                .copied()
                .expect("a pass on an imported texture records after the surface acquire"),
        }
    }
}

/// The load of a pass that writes every pixel of the surface before any
/// other pass has: wgpu's `DontCare`, which neither clears the texture nor
/// reads it. Vulkan and Metal take it as their own load op; DX12 and GL
/// load a back buffer wgpu counts as written, so every backend draws the
/// same picture. A texture the renderer made itself never takes it: the
/// graph clears a direct capture's stand-in, as D3D12 asks of a new render
/// target.
fn load_nothing() -> wgpu::LoadOp<wgpu::Color> {
    // SAFETY: wgpu asks that every pixel of the target be written before any
    // is read, blended with or stored. The frame's graph compiles this load
    // only for the surface's first write by a `Write::Whole` pass: one
    // triangle over the whole target with no blend, every channel written,
    // no culling, no depth test and no scissor, by a shader of the engine's
    // own text. The shader coverage test holds those files to no `discard`
    // and each builder to that pipeline state, and `record_passes` asserts
    // that a pass which loads nothing drew. No draw of such a pass samples
    // its own target.
    wgpu::LoadOp::DontCare(unsafe { wgpu::LoadOpDontCare::enabled() })
}

/// Opens the render pass of a compiled pass on its attachments' views. The
/// scene's clear is the renderer's `scene_clear`, which a game may change
/// between two frames of one shape; a store the frame does not need is a
/// discard.
fn begin_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    pass: &CompiledPass<PassKind>,
    color: &wgpu::TextureView,
    resolve: Option<&wgpu::TextureView>,
    depth: Option<&wgpu::TextureView>,
    scene_clear: wgpu::Color,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    let load = match pass.color.load {
        Load::Clear(_) if pass.kind == PassKind::Scene => wgpu::LoadOp::Clear(scene_clear),
        Load::Clear(color) => wgpu::LoadOp::Clear(color),
        Load::Keep => wgpu::LoadOp::Load,
        Load::Nothing => load_nothing(),
    };
    let store = |keep: bool| {
        if keep {
            wgpu::StoreOp::Store
        } else {
            wgpu::StoreOp::Discard
        }
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(pass.label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
            depth_slice: None,
            resolve_target: resolve,
            ops: wgpu::Operations {
                load,
                store: store(pass.color.store),
            },
        })],
        depth_stencil_attachment: pass.depth.zip(depth).map(|(attach, view)| {
            wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(attach.clear),
                    store: store(attach.store),
                }),
                stencil_ops: None,
            }
        }),
        timestamp_writes,
        ..Default::default()
    })
}

/// The frame's draw data, as [`Renderer::render_frame_full`] takes it.
#[derive(Clone, Copy)]
struct FrameDraws<'f> {
    quads: &'f [QuadInstance],
    sprite_batches: &'f [SpriteBatch],
    debug_quads: &'f [QuadInstance],
    debug_lines: &'f [DebugLineInstance],
    /// The UI paint list (M46).
    ui: &'f PaintList,
    post_stack: &'f PostStack,
}

#[allow(clippy::too_many_arguments)]
fn record_main_draws<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    quad_pipeline: &'a QuadPipeline,
    sprite_pipeline: &'a mut SpritePipeline,
    debug_line_pipeline: &'a DebugLinePipeline,
    mesh_particle_pipeline: &'a MeshParticlePipeline,
    quads: &[QuadInstance],
    sprite_batches: &[SpriteBatch],
    debug_quads: &[QuadInstance],
    debug_lines: &[DebugLineInstance],
    materials: &'a HashMap<MaterialAssetId, MaterialPipeline>,
    lit_sprite_pipeline: &'a LitSpritePipeline,
    lighting: &'a LightingResources,
    target: (u32, u32),
) {
    render_pass.push_debug_group("quads");
    quad_pipeline.draw(device, render_pass, quads);
    render_pass.pop_debug_group();

    render_pass.push_debug_group("sprites");
    sprite_pipeline.draw(
        device,
        queue,
        render_pass,
        sprite_batches,
        materials,
        Some(lit_sprite_pipeline),
        Some(&lighting.bind_group),
        target,
    );
    render_pass.pop_debug_group();

    render_pass.push_debug_group("mesh_particles");
    mesh_particle_pipeline.draw(render_pass, quad_pipeline.camera_bind_group());
    render_pass.pop_debug_group();

    render_pass.push_debug_group("debug_quads");
    quad_pipeline.draw(device, render_pass, debug_quads);
    render_pass.pop_debug_group();

    render_pass.push_debug_group("debug_lines");
    debug_line_pipeline.draw(
        device,
        render_pass,
        quad_pipeline.camera_bind_group(),
        debug_lines,
    );
    render_pass.pop_debug_group();
}

/// Fullscreen-triangle blit pipeline copying a composed frame into the
/// swapchain. Only capture frames use it (`D-087`).
struct PresentBlitPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// The source bind group, keyed on the identity of the pooled texture it
    /// samples.
    bind_group: TargetCache<u64, wgpu::BindGroup>,
}

impl PresentBlitPipeline {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present_blit_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/present_blit.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present_blit_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present_blit_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present_blit_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(whole_target(target_format))],
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
        });
        Self {
            pipeline,
            bind_group_layout: bgl,
            bind_group: TargetCache::default(),
        }
    }

    /// The bind group sampling `source_view`, the view of the pooled texture
    /// with identity `source`.
    fn bind_group(
        &mut self,
        device: &wgpu::Device,
        source: u64,
        source_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let layout = &self.bind_group_layout;
        self.bind_group
            .get_or_build(source, || {
                Self::build_bind_group(device, layout, source_view)
            })
            .clone()
    }

    /// A bind group sampling `source_view`, built for one frame: the view is
    /// no pooled texture's, so nothing keys a cache.
    fn bind_group_for_view(
        &self,
        device: &wgpu::Device,
        source_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        Self::build_bind_group(device, &self.bind_group_layout, source_view)
    }

    fn build_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        source_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present_blit_bg"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source_view),
            }],
        })
    }

    /// Drops the bind group when it holds a view of one of the pooled
    /// textures `freed`.
    fn forget_targets(&mut self, freed: &[u64]) {
        self.bind_group.clear_if(|source| freed.contains(source));
    }
}

/// The texture a direct capture renders into in place of the swapchain: the
/// surface's format and size, readable by the screenshot copy and by the
/// present blit.
fn create_swapchain_standin(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tungsten_capture_swapchain_standin"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(crate) struct CaptureTarget {
    pub(crate) readback: wgpu::Buffer,
    pub(crate) padded_bytes_per_row: u32,
}

pub(crate) fn create_capture_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> CaptureTarget {
    let padded_bytes_per_row = aligned_bytes_per_row(width);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tungsten_screenshot_readback"),
        size: u64::from(padded_bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    CaptureTarget {
        readback,
        padded_bytes_per_row,
    }
}

pub(crate) fn finalize_capture(
    device: &wgpu::Device,
    target: &CaptureTarget,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    path: &std::path::Path,
) -> Result<(), ScreenshotError> {
    let slice = target.readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    match receiver.recv() {
        Ok(Ok(())) => {}
        Ok(Err(_)) | Err(_) => return Err(ScreenshotError::MapFailed),
    }

    let mapped = slice
        .get_mapped_range()
        .map_err(|_| ScreenshotError::MapFailed)?;
    let rgba_surface = strip_row_padding(&mapped, width, height, target.padded_bytes_per_row);
    drop(mapped);
    target.readback.unmap();

    if rgba_surface.len() != (width * height * 4) as usize {
        return Err(ScreenshotError::SizeMismatch(
            rgba_surface.len(),
            (width * height * 4) as usize,
        ));
    }

    let is_bgra = matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    let mut rgba = rgba_surface;
    if is_bgra {
        for chunk in rgba.as_chunks_mut::<4>().0 {
            chunk.swap(0, 2);
        }
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|source| ScreenshotError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)?;
    Ok(())
}

fn build_smaa_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    cache: &ShaderModuleCache,
    ids: SmaaShaderIds,
    mode: PostAaMode,
    size: (u32, u32),
) -> Result<SmaaPipeline, String> {
    let edge_module = cache
        .get(ids.edge)
        .expect("smaa_edge module must be in cache");
    let blend_module = cache
        .get(ids.blend_weights)
        .expect("smaa_blend_weights module must be in cache");
    let nbh_module = cache
        .get(ids.neighborhood_blend)
        .expect("smaa_neighborhood_blend module must be in cache");
    // Inside an error scope: a stage module may have been committed while
    // no SMAA pipeline existed to check it against (B2, `D-057`).
    let pipeline = build_validated(device, || {
        SmaaPipeline::new(
            device,
            queue,
            format,
            edge_module,
            blend_module,
            nbh_module,
            ids,
        )
    })?;
    pipeline.update_preset(queue, mode, size);
    Ok(pipeline)
}

#[cfg(test)]
#[path = "tests/renderer.rs"]
mod tests;
