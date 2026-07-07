use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::debug_line::{DebugLineInstance, DebugLinePipeline};
use crate::lighting::{LightUbo, LightingResources};
use crate::lit_sprite::{
    LitSpritePipeline, EMISSIVE_MASK_SHADER_NAME, LIT_SPRITE_SHADER_NAME, LIT_SPRITE_SHADER_SOURCE,
    RIM_LIGHT_SHADER_NAME,
};
use crate::material::{build_material_pipeline, MaterialPipeline};
use crate::passes::{default_pass_order, text_overlay_target, PassRecorder, TargetId};
use crate::post::bloom::{
    BloomShaderIds, BLOOM_COMPOSITE_SHADER_NAME, BLOOM_DOWNSAMPLE_SHADER_NAME,
    BLOOM_THRESHOLD_SHADER_NAME, BLOOM_UPSAMPLE_SHADER_NAME,
};
use crate::post::smaa::{
    SmaaPipeline, SmaaShaderIds, SMAA_BLEND_WEIGHTS_SHADER_NAME, SMAA_EDGE_SHADER_NAME,
    SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
};
use crate::post::PostStackRenderer;
use crate::quad::{QuadInstance, QuadPipeline};
use crate::screenshot::{aligned_bytes_per_row, strip_row_padding};
use crate::shader_hot_reload::{ShaderError, ShaderModuleCache};
use crate::sprite::{SpriteBatch, SpritePipeline};
use crate::surface::{present_mode_label, resolve_max_frame_latency, resolve_present_mode};
use crate::targets::RenderTargetPool;
use crate::text::{TextPipeline, TextSection};
pub use crate::timing::{CpuFrameTimings, GpuFrameTimings};
use thiserror::Error;
use tungsten_core::assets::{
    FilterMode, MaterialAssetId, MaterialUniformDefaults, ShaderAssetId, TextureHandle,
};
use tungsten_core::config::{
    is_supported_msaa, DepthSortMode, PostAaMode, PresentModeConfig, RenderConfig,
};
use tungsten_core::post::{PostPass, PostStack};
use winit::window::Window;

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
}

/// Stable well-known id for the engine-internal sprite shader. Matches the
/// manifest entry under `shaders.sprite`.
pub const SPRITE_SHADER_NAME: &str = "sprite";

/// WGPU renderer state.
pub struct Renderer {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub clear_color: wgpu::Color,
    quad_pipeline: QuadPipeline,
    sprite_pipeline: SpritePipeline,
    debug_line_pipeline: DebugLinePipeline,
    text_pipeline: TextPipeline,
    present_blit: PresentBlitPipeline,
    post_stack: PostStackRenderer,
    target_pool: RenderTargetPool,
    shader_cache: ShaderModuleCache,
    sample_count: u32,
    depth_enabled: bool,
    depth_sort: DepthSortMode,
    post_aa: PostAaMode,
    /// SMAA presentation pipeline. Allocated on demand when `post_aa != Off`.
    smaa: Option<SmaaPipeline>,
    smaa_shader_ids: SmaaShaderIds,
    #[allow(dead_code)] // mirror of post_stack.bloom.shader_ids; future debug HUD will read it.
    bloom_shader_ids: BloomShaderIds,
    bloom_max_mips: u32,
    #[allow(dead_code)] // kept for symmetry with shader_ids map; future work wires to debug HUD.
    sprite_shader_id: ShaderAssetId,
    /// M29 lit sprite pipeline; rebuilt on `lit_sprite` shader hot-reload.
    lit_sprite_pipeline: LitSpritePipeline,
    /// M29 per-frame lighting UBO + bind group bound at group 2 of the lit
    /// pipeline. Resize does not invalidate this; the buffer + bind group
    /// stay live across surface reconfigures.
    lighting: LightingResources,
    #[allow(dead_code)]
    lit_sprite_shader_id: ShaderAssetId,
    /// M26 material pipelines keyed by id.
    materials: HashMap<MaterialAssetId, MaterialPipeline>,
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
    /// One-shot offscreen PNG capture target.
    pub(crate) pending_capture: Option<PathBuf>,
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
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;

        // Optional feature only; device creation must not depend on timestamps.
        let adapter_features = adapter.features();
        let desired_features = adapter_features & wgpu::Features::TIMESTAMP_QUERY;

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
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency,
        };
        surface.configure(&device, &surface_config);

        let gpu_timings = GpuFrameTimings {
            frame_gpu_ms: None,
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

        let sample_count = if is_supported_msaa(config.msaa) {
            config.msaa
        } else {
            log::warn!(
                "unsupported msaa sample_count {} in RenderConfig; falling back to 1",
                config.msaa
            );
            1
        };
        let depth_enabled = config.depth_enabled;
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
        // sprite gets the writing `LessEqual` variant.
        let depth_attached = matches!(depth_sort, DepthSortMode::GpuDepth);

        let quad_pipeline = QuadPipeline::new(&device, format, sample_count, depth_attached);
        let sprite_pipeline = SpritePipeline::new(&device, format, sample_count, depth_attached);
        let debug_line_pipeline = DebugLinePipeline::new(
            &device,
            format,
            quad_pipeline.camera_bind_group_layout(),
            sample_count,
            depth_attached,
        );
        // Text renders in its own overlay pass after the post stack, which
        // always targets a single-sample color texture with no depth. Baking
        // those attachment bits here keeps text pixels out of the post stack
        // regardless of scene MSAA / depth config.
        let text_pipeline = TextPipeline::new(&device, &queue, format, 1, false);

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
        let target_pool = RenderTargetPool::new(
            &device,
            (surface_config.width, surface_config.height),
            format,
            sample_count,
            depth_enabled,
            post_aa,
            bloom_max_mips,
        );
        let present_blit = PresentBlitPipeline::new(&device, format);

        // Seed the shader cache with the compile-time sprite WGSL so the
        // first `asset_loader::load_shaders` call is a no-op when the
        // on-disk bytes match. Same for the three SMAA stage shaders.
        let mut shader_cache = ShaderModuleCache::new();
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
        let edge_src = include_str!("shaders/stock/smaa_edge.wgsl").to_string();
        let blend_src = include_str!("shaders/stock/smaa_blend_weights.wgsl").to_string();
        let nbh_src = include_str!("shaders/stock/smaa_neighborhood_blend.wgsl").to_string();
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

        let bloom_shader_ids = BloomShaderIds {
            threshold: ShaderAssetId(4),
            downsample: ShaderAssetId(5),
            upsample: ShaderAssetId(6),
            composite: ShaderAssetId(7),
        };
        let bloom_threshold_src = include_str!("shaders/stock/bloom_threshold.wgsl").to_string();
        let bloom_downsample_src = include_str!("shaders/stock/bloom_downsample.wgsl").to_string();
        let bloom_upsample_src = include_str!("shaders/stock/bloom_upsample.wgsl").to_string();
        let bloom_composite_src = include_str!("shaders/stock/bloom_composite.wgsl").to_string();
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

        let post_stack = PostStackRenderer::new(&device, format, &shader_cache, bloom_shader_ids);

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
            format,
            sample_count,
            depth_attached,
        );

        let smaa = if post_aa.is_smaa() {
            Some(build_smaa_pipeline(
                &device,
                &queue,
                format,
                &shader_cache,
                smaa_shader_ids,
                post_aa,
                (surface_config.width, surface_config.height),
            ))
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

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            surface_config,
            clear_color,
            quad_pipeline,
            sprite_pipeline,
            debug_line_pipeline,
            text_pipeline,
            present_blit,
            post_stack,
            target_pool,
            shader_cache,
            sample_count,
            depth_enabled,
            depth_sort,
            post_aa,
            smaa,
            smaa_shader_ids,
            bloom_shader_ids,
            bloom_max_mips,
            sprite_shader_id,
            lit_sprite_pipeline,
            lighting,
            lit_sprite_shader_id,
            materials: HashMap::new(),
            shader_ids,
            next_shader_id: 11,
            timestamp_support,
            gpu_timings,
            cpu_timings: CpuFrameTimings::default(),
            pending_capture: None,
        })
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
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

    /// M29 upload one frame of the lighting UBO. The bind group built at
    /// startup stays valid; resize does not invalidate it.
    pub fn update_lights(&self, ubo: &LightUbo) {
        self.lighting.write(&self.queue, ubo);
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

    /// Hot-reload font bytes.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        self.text_pipeline.reload_font(id, data);
    }

    /// Register a manifest-tracked shader. `"sprite"` rebuilds the built-in
    /// sprite pipeline. Other ids are cached for later use by material or
    /// post pipelines. Byte-equal short-circuits skip validation.
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
        if name == SPRITE_SHADER_NAME {
            self.sprite_pipeline.rebuild_with_shader(
                &self.device,
                &module,
                self.surface_config.format,
                self.sample_count,
                matches!(self.depth_sort, DepthSortMode::GpuDepth),
            );
        }
        if matches!(
            name,
            SMAA_EDGE_SHADER_NAME
                | SMAA_BLEND_WEIGHTS_SHADER_NAME
                | SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME
        ) {
            if let Some(smaa) = self.smaa.as_mut() {
                smaa.rebuild_stage_with_module(&self.device, id, &module);
            }
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
                self.surface_config.format,
                id,
                &module,
            );
        }
        if name == LIT_SPRITE_SHADER_NAME {
            self.lit_sprite_pipeline.rebuild_with_shader(
                &self.device,
                &module,
                self.surface_config.format,
                self.sample_count,
                matches!(self.depth_sort, DepthSortMode::GpuDepth),
            );
        }
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
        if name == SPRITE_SHADER_NAME {
            self.sprite_pipeline.rebuild_with_shader(
                &self.device,
                &module,
                self.surface_config.format,
                self.sample_count,
                matches!(self.depth_sort, DepthSortMode::GpuDepth),
            );
        }
        if matches!(
            name,
            SMAA_EDGE_SHADER_NAME
                | SMAA_BLEND_WEIGHTS_SHADER_NAME
                | SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME
        ) {
            if let Some(smaa) = self.smaa.as_mut() {
                smaa.rebuild_stage_with_module(&self.device, id, &module);
            }
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
                self.surface_config.format,
                id,
                &module,
            );
        }
        if name == LIT_SPRITE_SHADER_NAME {
            self.lit_sprite_pipeline.rebuild_with_shader(
                &self.device,
                &module,
                self.surface_config.format,
                self.sample_count,
                matches!(self.depth_sort, DepthSortMode::GpuDepth),
            );
        }
        self.shader_cache.commit(id, wgsl, module);
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
        log::info!("shader '{name}' reloaded");
        Ok(())
    }

    fn resolve_or_allocate_shader_id(&mut self, name: &str) -> ShaderAssetId {
        if let Some(&id) = self.shader_ids.get(name) {
            return id;
        }
        let id = ShaderAssetId(self.next_shader_id);
        self.next_shader_id += 1;
        self.shader_ids.insert(name.to_string(), id);
        id
    }

    /// Upload a new user-authored material (M26). Validation-failed builds
    /// keep the last-known-good entry; first-time failure leaves nothing.
    pub fn upload_material(
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

        let (pipeline, material_bgl, ubo, bind_group) = build_material_pipeline(
            &self.device,
            module,
            self.sprite_pipeline.camera_bind_group_layout(),
            self.sprite_pipeline.texture_bind_group_layout(),
            self.surface_config.format,
            self.sample_count,
            matches!(self.depth_sort, DepthSortMode::GpuDepth),
            name,
        );
        // Seed defaults so first-frame draws don't read uninitialised memory.
        self.queue
            .write_buffer(&ubo, 0, &defaults.to_override_block().to_bytes());

        self.materials.insert(
            id,
            MaterialPipeline {
                pipeline,
                ubo,
                bind_group,
                material_bind_group_layout: material_bgl,
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

    fn rebuild_materials_for_shader(&mut self, shader_name: &str) {
        let ids_to_rebuild: Vec<(MaterialAssetId, String, MaterialUniformDefaults)> = self
            .materials
            .iter()
            .filter(|(_, mp)| mp.shader_id_name == shader_name)
            .map(|(id, mp)| (*id, mp.name.clone(), mp.defaults))
            .collect();
        for (id, name, defaults) in ids_to_rebuild {
            if let Err(e) = self.upload_material(id, &name, shader_name, defaults) {
                log::error!("material '{name}' rebuild after shader reload failed: {e}");
            }
        }
    }

    /// Reconfigure surface after resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.target_pool.resize(
                &self.device,
                (width, height),
                self.surface_config.format,
                self.sample_count,
                self.depth_enabled,
                self.post_aa,
                self.bloom_max_mips,
            );
            if let Some(smaa) = self.smaa.as_ref() {
                smaa.update_preset(&self.queue, self.post_aa, (width, height));
            }
        }
    }

    /// Current post-AA mode.
    #[must_use]
    pub fn post_aa(&self) -> PostAaMode {
        self.post_aa
    }

    /// Switch presentation AA at a frame boundary. Allocates / drops SMAA
    /// intermediates as needed without relaunch. Must not be called from
    /// within `render_frame_internal`.
    pub fn set_post_aa(&mut self, mode: PostAaMode) {
        if mode == self.post_aa {
            return;
        }
        self.post_aa = mode;
        let size = (self.surface_config.width, self.surface_config.height);
        self.target_pool.resize(
            &self.device,
            size,
            self.surface_config.format,
            self.sample_count,
            self.depth_enabled,
            self.post_aa,
            self.bloom_max_mips,
        );
        match (self.post_aa.is_smaa(), self.smaa.is_some()) {
            (true, false) => {
                self.smaa = Some(build_smaa_pipeline(
                    &self.device,
                    &self.queue,
                    self.surface_config.format,
                    &self.shader_cache,
                    self.smaa_shader_ids,
                    self.post_aa,
                    size,
                ));
            }
            (true, true) => {
                if let Some(smaa) = self.smaa.as_ref() {
                    smaa.update_preset(&self.queue, self.post_aa, size);
                }
            }
            (false, _) => {
                self.smaa = None;
            }
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

    fn acquire_texture(&self) -> Result<Option<wgpu::SurfaceTexture>, RenderError> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => Ok(Some(tex)),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.surface_config);
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                Err(RenderError::Surface("validation error".into()))
            }
        }
    }

    /// Clear-only frame.
    pub fn render_frame(&mut self) -> Result<(), RenderError> {
        self.render_frame_with_quads(&[])
    }

    /// D-018 direct-data quad frame with default pixel ortho.
    pub fn render_frame_with_quads(&mut self, quads: &[QuadInstance]) -> Result<(), RenderError> {
        let w = self.surface_config.width as f32;
        let h = self.surface_config.height as f32;
        let default_view_proj = glam::Mat4::orthographic_rh(0.0, w, h, 0.0, -1.0, 1.0);
        let empty_stack = PostStack::default();
        self.render_frame_full(&default_view_proj, quads, &[], &[], &[], &[], &empty_stack)
    }

    /// Render quads, sprites, debug primitives, and screen-space text.
    #[allow(clippy::too_many_arguments)]
    pub fn render_frame_full(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
        post_stack: &PostStack,
    ) -> Result<(), RenderError> {
        self.render_frame_internal(
            view_proj,
            quads,
            sprite_batches,
            debug_quads,
            debug_lines,
            text_sections,
            post_stack,
            None,
        )
    }

    /// Timed full frame; stalls on `device.poll(wait_indefinitely())`.
    #[allow(clippy::too_many_arguments)]
    pub fn render_frame_full_timed(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
        post_stack: &PostStack,
    ) -> Result<(), RenderError> {
        if !self.timestamp_support {
            self.gpu_timings.frame_gpu_ms = None;
            return self.render_frame_full(
                view_proj,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
                text_sections,
                post_stack,
            );
        }

        let query_set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("frame_ts_qs"),
            count: 2,
            ty: wgpu::QueryType::Timestamp,
        });
        let resolve_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ts_resolve"),
            size: 16,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ts_readback"),
            size: 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let timing = TimingResources {
            query_set,
            resolve_buf,
            readback_buf,
        };

        self.render_frame_internal(
            view_proj,
            quads,
            sprite_batches,
            debug_quads,
            debug_lines,
            text_sections,
            post_stack,
            Some(timing),
        )
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
        post_stack: &PostStack,
        timing: Option<TimingResources>,
    ) -> Result<(), RenderError> {
        self.cpu_timings = CpuFrameTimings::default();
        if timing.is_some() {
            self.gpu_timings.frame_gpu_ms = None;
        }

        let acquire_start = Instant::now();
        let Some(output) = self.acquire_texture()? else {
            return Ok(());
        };
        self.cpu_timings.acquire_ms = acquire_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let encode_start = Instant::now();
        let w = self.surface_config.width;
        let h = self.surface_config.height;
        self.quad_pipeline.update_camera(&self.queue, view_proj);
        self.sprite_pipeline.update_camera(&self.queue, view_proj);
        self.text_pipeline
            .prepare(&self.device, &self.queue, text_sections, w, h);

        let swap_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        let post_stack_len = post_stack.len();
        let smaa_active = self.post_aa.is_smaa() && self.smaa.is_some();
        // After M26/text-overlay split: the present-blit and screenshot
        // source both sample the text-overlay target, which is whichever
        // of SceneColor/PostPing/PostPong/PresentSource holds the composited
        // frame.
        let final_source_target = text_overlay_target(post_stack_len, self.post_aa);
        let final_source_view = match final_source_target {
            TargetId::PostPing => self.target_pool.scene.post_ping_view(),
            TargetId::PostPong => self.target_pool.scene.post_pong_view(),
            TargetId::PresentSource => self
                .target_pool
                .scene
                .present_source_view()
                .expect("PresentSource view must exist while post_aa != Off"),
            _ => self.target_pool.scene.color_view(),
        };
        // Rebuild the present-blit bind group every frame against the live
        // source view; cheap, avoids stale-view hazards after resize.
        let blit_bind_group = self
            .present_blit
            .make_bind_group(&self.device, final_source_view);

        let order = default_pass_order(
            self.sample_count,
            self.depth_sort,
            self.depth_enabled,
            post_stack_len,
            self.post_aa,
        );
        let post_plan = PostStackRenderer::plan_targets(post_stack_len);
        // Text overlay sits between the last post pass / SMAA tail and the
        // present pass. `post_stack_len + 1` accounts for the leading scene
        // pass; SMAA inserts three more before the overlay.
        let smaa_edge_idx = post_stack_len + 1;
        let smaa_blend_idx = smaa_edge_idx + 1;
        let smaa_nbh_idx = smaa_blend_idx + 1;
        let text_overlay_idx = if smaa_active {
            smaa_nbh_idx + 1
        } else {
            post_stack_len + 1
        };
        // SMAA edge / neighborhood read whatever target the post stack ended
        // on (or `SceneColor` when the stack is empty). When the swapchain
        // format has a non-sRGB twin, sample through it so edge detection
        // sees gamma-encoded values; otherwise fall back to the primary view.
        let smaa_source_target = if post_stack_len == 0 {
            TargetId::SceneColor
        } else if (post_stack_len - 1).is_multiple_of(2) {
            TargetId::PostPing
        } else {
            TargetId::PostPong
        };
        let smaa_source_view: &wgpu::TextureView = if smaa_active {
            match smaa_source_target {
                TargetId::SceneColor => self
                    .target_pool
                    .scene
                    .scene_color_smaa_read_view()
                    .unwrap_or_else(|| self.target_pool.scene.color_view()),
                TargetId::PostPing => self
                    .target_pool
                    .scene
                    .post_ping_smaa_read_view()
                    .unwrap_or_else(|| self.target_pool.scene.post_ping_view()),
                TargetId::PostPong => self
                    .target_pool
                    .scene
                    .post_pong_smaa_read_view()
                    .unwrap_or_else(|| self.target_pool.scene.post_pong_view()),
                _ => self.target_pool.scene.color_view(),
            }
        } else {
            self.target_pool.scene.color_view()
        };

        encoder.push_debug_group("tungsten_frame");
        for (idx, pass_desc) in order.as_slice().iter().enumerate() {
            let is_scene = idx == 0;
            let is_present = pass_desc.color == TargetId::Swapchain;
            let is_text_overlay = !is_scene && !is_present && idx == text_overlay_idx;
            let is_smaa_edge = smaa_active && idx == smaa_edge_idx;
            let is_smaa_blend = smaa_active && idx == smaa_blend_idx;
            let is_smaa_nbh = smaa_active && idx == smaa_nbh_idx;
            let post_index = if !is_scene
                && !is_present
                && !is_text_overlay
                && !is_smaa_edge
                && !is_smaa_blend
                && !is_smaa_nbh
            {
                Some(idx - 1)
            } else {
                None
            };

            // Bloom is the only post variant that does not record a single
            // fullscreen draw into the slot's auto-opened render pass; it
            // opens its own per-subpass passes through the encoder. Detect it
            // before `PassRecorder::begin` and skip the auto-open path.
            if let Some(pi) = post_index {
                if let (Some(PostPass::Bloom(params)), Some(&(src, dst))) =
                    (post_stack.0.get(pi), post_plan.get(pi))
                {
                    self.post_stack.record_bloom_slot(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        &self.target_pool,
                        params,
                        src,
                        dst,
                    );
                    continue;
                }
            }

            let clear_override = if is_scene {
                Some(self.clear_color)
            } else {
                None
            };
            let mut pass = if is_scene && timing.is_some() {
                // Attach timestamp queries to the main scene pass.
                begin_scene_pass_with_timestamps(
                    &mut encoder,
                    pass_desc,
                    &self.target_pool,
                    &swap_view,
                    self.clear_color,
                    timing.as_ref().map(|t| &t.query_set),
                )
            } else {
                PassRecorder::begin(
                    &mut encoder,
                    pass_desc,
                    &self.target_pool,
                    &swap_view,
                    clear_override,
                )
            };

            if is_scene {
                record_main_draws(
                    &mut pass,
                    &self.device,
                    &self.queue,
                    &self.quad_pipeline,
                    &mut self.sprite_pipeline,
                    &self.debug_line_pipeline,
                    quads,
                    sprite_batches,
                    debug_quads,
                    debug_lines,
                    &self.materials,
                    &self.lit_sprite_pipeline,
                    &self.lighting,
                );
            } else if is_smaa_edge {
                if let Some(smaa) = self.smaa.as_ref() {
                    smaa.record_edge_pass(&self.device, &mut pass, smaa_source_view);
                }
            } else if is_smaa_blend {
                if let Some(smaa) = self.smaa.as_ref() {
                    smaa.record_blend_weights_pass(&self.device, &mut pass, &self.target_pool);
                }
            } else if is_smaa_nbh {
                if let Some(smaa) = self.smaa.as_ref() {
                    smaa.record_neighborhood_pass(
                        &self.device,
                        &mut pass,
                        &self.target_pool,
                        smaa_source_view,
                    );
                }
            } else if is_text_overlay {
                pass.push_debug_group("text");
                self.text_pipeline.render(&mut pass);
                pass.pop_debug_group();
            } else if is_present {
                pass.set_pipeline(&self.present_blit.pipeline);
                pass.set_bind_group(0, &blit_bind_group, &[]);
                pass.draw(0..3, 0..1);
            } else if let Some(pi) = post_index {
                if let (Some(post_pass), Some(&(src, _dst))) =
                    (post_stack.0.get(pi), post_plan.get(pi))
                {
                    self.post_stack.record_pass(
                        &self.device,
                        &self.queue,
                        &mut pass,
                        &self.target_pool,
                        post_pass,
                        src,
                    );
                }
            }
        }

        // Screenshot path: readback from the final composed frame source,
        // which is the text-overlay target (post-target when the stack is
        // non-empty, else SceneColor).
        let capture_path = self.pending_capture.take();
        let capture_target = capture_path
            .as_ref()
            .map(|_| create_capture_target(&self.device, self.surface_config.format, w, h));
        if let Some(target) = capture_target.as_ref() {
            let capture_src_texture = match final_source_target {
                TargetId::PostPing => self.target_pool.scene.post_ping_texture(),
                TargetId::PostPong => self.target_pool.scene.post_pong_texture(),
                TargetId::PresentSource => self
                    .target_pool
                    .scene
                    .present_source_texture()
                    .expect("PresentSource texture must exist while post_aa != Off"),
                _ => self.target_pool.scene.color_texture(),
            };
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: capture_src_texture,
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

        if let Some(t) = timing.as_ref() {
            encoder.resolve_query_set(&t.query_set, 0..2, &t.resolve_buf, 0);
            encoder.copy_buffer_to_buffer(&t.resolve_buf, 0, &t.readback_buf, 0, 16);
        }

        encoder.pop_debug_group();

        let finished = encoder.finish();
        self.cpu_timings.encode_ms = encode_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let submit_present_start = Instant::now();
        self.queue.submit(std::iter::once(finished));
        output.present();

        if let (Some(path), Some(target)) = (capture_path, capture_target) {
            if let Err(e) = finalize_capture(
                &self.device,
                &target,
                self.surface_config.format,
                w,
                h,
                &path,
            ) {
                log::warn!("screenshot capture failed: {e}");
            }
        }

        self.text_pipeline.post_frame();

        if let Some(t) = timing {
            self.read_gpu_timestamp(&t.readback_buf);
        }

        self.cpu_timings.submit_present_ms =
            submit_present_start.elapsed().as_secs_f64() as f32 * 1000.0;

        Ok(())
    }

    fn read_gpu_timestamp(&mut self, readback_buf: &wgpu::Buffer) {
        let slice = readback_buf.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        if receiver.recv().ok().and_then(Result::ok).is_some() {
            let data = slice.get_mapped_range();
            let ts0 = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0u8; 8]));
            let ts1 = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0u8; 8]));
            drop(data);
            readback_buf.unmap();

            let period = self.queue.get_timestamp_period();
            let delta_ns = ts1.wrapping_sub(ts0) as f64 * f64::from(period);
            self.gpu_timings.frame_gpu_ms = Some((delta_ns / 1_000_000.0) as f32);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_main_draws<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    quad_pipeline: &'a QuadPipeline,
    sprite_pipeline: &'a mut SpritePipeline,
    debug_line_pipeline: &'a DebugLinePipeline,
    quads: &[QuadInstance],
    sprite_batches: &[SpriteBatch],
    debug_quads: &[QuadInstance],
    debug_lines: &[DebugLineInstance],
    materials: &'a HashMap<MaterialAssetId, MaterialPipeline>,
    lit_sprite_pipeline: &'a LitSpritePipeline,
    lighting: &'a LightingResources,
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
        Some(&lit_sprite_pipeline.pipeline),
        Some(&lighting.bind_group),
    );
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

struct TimingResources {
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
}

fn begin_scene_pass_with_timestamps<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    desc: &crate::passes::PassDesc,
    pool: &'a RenderTargetPool,
    swap_view: &'a wgpu::TextureView,
    clear: wgpu::Color,
    query_set: Option<&'a wgpu::QuerySet>,
) -> wgpu::RenderPass<'a> {
    let color_view = match desc.color {
        TargetId::SceneColor => pool.scene.color_view(),
        TargetId::SceneColorMsaa => pool
            .scene
            .color_msaa_view()
            .expect("SceneColorMsaa requested but sample_count == 1"),
        TargetId::PostPing => pool.scene.post_ping_view(),
        TargetId::PostPong => pool.scene.post_pong_view(),
        TargetId::SmaaEdges => pool
            .scene
            .smaa_edges_view()
            .expect("SmaaEdges requested but post_aa == Off"),
        TargetId::SmaaBlend => pool
            .scene
            .smaa_blend_view()
            .expect("SmaaBlend requested but post_aa == Off"),
        TargetId::PresentSource => pool
            .scene
            .present_source_view()
            .expect("PresentSource requested but post_aa == Off"),
        TargetId::Swapchain => swap_view,
        TargetId::SceneDepth => unreachable!("depth is not a valid color target"),
    };
    let resolve_view_opt = desc.color_resolve.map(|target| match target {
        TargetId::SceneColor => pool.scene.color_view(),
        TargetId::PostPing => pool.scene.post_ping_view(),
        TargetId::PostPong => pool.scene.post_pong_view(),
        TargetId::Swapchain => swap_view,
        _ => unreachable!("invalid resolve target"),
    });
    let depth_attachment = desc.depth.map(|target| {
        let view = match target {
            TargetId::SceneDepth => pool
                .scene
                .depth_view()
                .expect("SceneDepth requested but depth_enabled = false"),
            _ => unreachable!("invalid depth target"),
        };
        let load = desc
            .depth_clear
            .map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear);
        wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }
    });
    let ts_writes = query_set.map(|qs| wgpu::RenderPassTimestampWrites {
        query_set: qs,
        beginning_of_pass_write_index: Some(0),
        end_of_pass_write_index: Some(1),
    });

    let load = if desc.clear.is_some() {
        wgpu::LoadOp::Clear(clear)
    } else {
        wgpu::LoadOp::Load
    };

    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(desc.label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            depth_slice: None,
            resolve_target: resolve_view_opt,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: ts_writes,
        ..Default::default()
    })
}

/// Fullscreen-triangle blit pipeline copying `SceneColor` → swapchain.
struct PresentBlitPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
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
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
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
        });
        Self {
            pipeline,
            bind_group_layout: bgl,
        }
    }

    fn make_bind_group(
        &self,
        device: &wgpu::Device,
        source_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present_blit_bg"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source_view),
            }],
        })
    }
}

pub(crate) struct CaptureTarget {
    pub(crate) readback: wgpu::Buffer,
    pub(crate) padded_bytes_per_row: u32,
}

pub(crate) fn create_capture_target(
    device: &wgpu::Device,
    _format: wgpu::TextureFormat,
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
) -> Result<(), crate::screenshot::ScreenshotError> {
    use crate::screenshot::ScreenshotError;

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

    let mapped = slice.get_mapped_range();
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
        for chunk in rgba.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| ScreenshotError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        }
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
) -> SmaaPipeline {
    let edge_module = cache
        .get(ids.edge)
        .expect("smaa_edge module must be in cache");
    let blend_module = cache
        .get(ids.blend_weights)
        .expect("smaa_blend_weights module must be in cache");
    let nbh_module = cache
        .get(ids.neighborhood_blend)
        .expect("smaa_neighborhood_blend module must be in cache");
    let pipeline = SmaaPipeline::new(
        device,
        queue,
        format,
        edge_module,
        blend_module,
        nbh_module,
        ids,
    );
    pipeline.update_preset(queue, mode, size);
    pipeline
}

#[cfg(test)]
#[path = "tests/renderer.rs"]
mod tests;
