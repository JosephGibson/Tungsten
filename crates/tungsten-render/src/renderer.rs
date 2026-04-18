use std::sync::Arc;
use std::time::Instant;

use crate::quad::{QuadInstance, QuadPipeline};
use crate::sprite::{SpriteBatch, SpritePipeline};
use crate::text::{TextPipeline, TextSection};
use thiserror::Error;
use tungsten_core::assets::TextureHandle;
use tungsten_core::config::{PresentModeConfig, RenderConfig};
use winit::window::Window;

/// GPU-side frame timing, in milliseconds.
/// All fields are `Option<f32>` because `TIMESTAMP_QUERY` may be unavailable
/// (software renderers, older Vulkan, WebGPU compatibility layer). Callers must
/// handle `None`.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameTimings {
    /// Render-pass GPU duration (begin to end). `None` when TIMESTAMP_QUERY
    /// is unavailable on the active backend.
    pub frame_gpu_ms: Option<f32>,
    /// Backend name from `wgpu::Adapter::get_info().backend`. Always `Some` after init.
    pub backend: Option<String>,
    /// Adapter name from `wgpu::Adapter::get_info().name`. Always `Some` after init.
    pub adapter_name: Option<String>,
    /// Actual surface present mode chosen at renderer init.
    pub present_mode: Option<String>,
    /// Requested frames-in-flight hint used for the surface configuration.
    pub max_frame_latency: Option<u32>,
}

/// CPU-side render-frame timing, in milliseconds.
#[derive(Debug, Clone, Default)]
pub struct CpuFrameTimings {
    /// CPU time spent acquiring the next surface texture.
    pub acquire_ms: f32,
    /// CPU time spent preparing render data, recording commands, and finishing the encoder.
    pub encode_ms: f32,
    /// CPU time spent submitting work, presenting, and any present/readback waits.
    pub submit_present_ms: f32,
}

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
}

fn present_mode_label(mode: wgpu::PresentMode) -> &'static str {
    match mode {
        wgpu::PresentMode::Fifo => "fifo",
        wgpu::PresentMode::FifoRelaxed => "fifo_relaxed",
        wgpu::PresentMode::Immediate => "immediate",
        wgpu::PresentMode::Mailbox => "mailbox",
        wgpu::PresentMode::AutoVsync => "auto_vsync",
        wgpu::PresentMode::AutoNoVsync => "auto_no_vsync",
    }
}

fn requested_present_mode_label(mode: PresentModeConfig) -> &'static str {
    mode.as_str()
}

fn choose_auto_vsync_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        wgpu::PresentMode::AutoVsync
    }
}

fn choose_auto_no_vsync_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else if supported.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::AutoNoVsync
    }
}

fn available_present_mode_labels(supported: &[wgpu::PresentMode]) -> Vec<String> {
    [
        wgpu::PresentMode::Fifo,
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Mailbox,
    ]
    .into_iter()
    .filter(|mode| supported.contains(mode))
    .map(present_mode_label)
    .map(str::to_string)
    .collect()
}

fn resolve_present_mode(
    supported: &[wgpu::PresentMode],
    requested: Option<PresentModeConfig>,
    vsync: bool,
) -> Result<wgpu::PresentMode, RenderError> {
    let requested = requested.unwrap_or(PresentModeConfig::Auto);
    match requested {
        PresentModeConfig::Auto => {
            if vsync {
                Ok(choose_auto_vsync_present_mode(supported))
            } else {
                Ok(choose_auto_no_vsync_present_mode(supported))
            }
        }
        PresentModeConfig::AutoVsync => Ok(choose_auto_vsync_present_mode(supported)),
        PresentModeConfig::AutoNoVsync => Ok(choose_auto_no_vsync_present_mode(supported)),
        PresentModeConfig::Immediate => {
            if supported.contains(&wgpu::PresentMode::Immediate) {
                Ok(wgpu::PresentMode::Immediate)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested_present_mode_label(requested).to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
        PresentModeConfig::Mailbox => {
            if supported.contains(&wgpu::PresentMode::Mailbox) {
                Ok(wgpu::PresentMode::Mailbox)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested_present_mode_label(requested).to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
        PresentModeConfig::Fifo => {
            if supported.contains(&wgpu::PresentMode::Fifo) {
                Ok(wgpu::PresentMode::Fifo)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested_present_mode_label(requested).to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
    }
}

fn default_max_frame_latency(present_mode: wgpu::PresentMode) -> u32 {
    if matches!(
        present_mode,
        wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox | wgpu::PresentMode::AutoNoVsync
    ) {
        1
    } else {
        2
    }
}

fn resolve_max_frame_latency(
    requested: Option<u32>,
    present_mode: wgpu::PresentMode,
) -> Result<u32, RenderError> {
    match requested {
        Some(0) => Err(RenderError::InvalidFrameLatency(0)),
        Some(value) => Ok(value),
        None => Ok(default_max_frame_latency(present_mode)),
    }
}

/// Core renderer state wrapping wgpu resources.
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
    text_pipeline: TextPipeline,
    /// Whether TIMESTAMP_QUERY is available. Determined at init time; never changes.
    pub timestamp_support: bool,
    /// Most recently computed GPU frame timings.
    pub gpu_timings: GpuFrameTimings,
    /// Most recently computed CPU render timings.
    pub cpu_timings: CpuFrameTimings,
}

impl Renderer {
    /// Initialize wgpu and create a renderer attached to the given window.
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

        // Request TIMESTAMP_QUERY only when the adapter supports it; never fail
        // device creation over a missing optional feature.
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

        let quad_pipeline = QuadPipeline::new(&device, format);
        let sprite_pipeline = SpritePipeline::new(&device, format);
        let text_pipeline = TextPipeline::new(&device, &queue, format);

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
            text_pipeline,
            timestamp_support,
            gpu_timings,
            cpu_timings: CpuFrameTimings::default(),
        })
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Upload a decoded RGBA texture to the GPU.
    pub fn upload_texture(
        &mut self,
        handle: TextureHandle,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) {
        self.sprite_pipeline.upload_texture(
            &self.device,
            &self.queue,
            handle,
            rgba_data,
            width,
            height,
        );
    }

    /// Load a font from raw TTF/OTF bytes and register it under a manifest ID.
    pub fn load_font(&mut self, id: &str, data: Vec<u8>) {
        self.text_pipeline.load_font(id, data);
    }

    /// Hot-reload a font: replace the old face data with new bytes in-place.
    pub fn reload_font(&mut self, id: &str, data: Vec<u8>) {
        self.text_pipeline.reload_font(id, data);
    }

    /// Reconfigure the surface after a window resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
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

    /// Render a frame: clear to the configured color (no geometry).
    pub fn render_frame(&mut self) -> Result<(), RenderError> {
        self.render_frame_with_quads(&[])
    }

    /// Render a frame with colored quads. Direct-data API per D-018.
    /// Uses the pre-M10 default pixel ortho (no camera scrolling).
    pub fn render_frame_with_quads(&mut self, quads: &[QuadInstance]) -> Result<(), RenderError> {
        let w = self.surface_config.width as f32;
        let h = self.surface_config.height as f32;
        let default_view_proj = glam::Mat4::orthographic_rh(0.0, w, h, 0.0, -1.0, 1.0);
        self.render_frame_full(&default_view_proj, quads, &[], &[])
    }

    /// Render a full frame with colored quads, textured sprites, and text.
    ///
    /// `view_proj` controls where world-space sprites and quads appear
    /// on screen. Text is always drawn in screen space regardless of
    /// the camera — glyphon manages its own viewport.
    pub fn render_frame_full(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        text_sections: &[TextSection],
    ) -> Result<(), RenderError> {
        self.cpu_timings = CpuFrameTimings::default();
        let acquire_start = Instant::now();
        let output = match self.acquire_texture()? {
            Some(tex) => tex,
            None => return Ok(()),
        };
        self.cpu_timings.acquire_ms = acquire_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let encode_start = Instant::now();
        let w = self.surface_config.width;
        let h = self.surface_config.height;
        self.quad_pipeline.update_camera(&self.queue, view_proj);
        self.sprite_pipeline.update_camera(&self.queue, view_proj);

        self.text_pipeline
            .prepare(&self.device, &self.queue, text_sections, w, h);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            self.quad_pipeline
                .draw(&self.device, &mut render_pass, quads);
            self.sprite_pipeline
                .draw(&self.device, &self.queue, &mut render_pass, sprite_batches);
            self.text_pipeline.render(&mut render_pass);
        }

        let finished = encoder.finish();
        self.cpu_timings.encode_ms = encode_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let submit_present_start = Instant::now();
        self.queue.submit(std::iter::once(finished));
        output.present();

        self.text_pipeline.post_frame();
        self.cpu_timings.submit_present_ms =
            submit_present_start.elapsed().as_secs_f64() as f32 * 1000.0;

        Ok(())
    }

    /// Render a full frame and record GPU timing in `self.gpu_timings.frame_gpu_ms`.
    ///
    /// When `TIMESTAMP_QUERY` is available, injects timestamps at render-pass begin/end
    /// via `RenderPassDescriptor.timestamp_writes` and reads them back after submit.
    /// When unavailable, falls through to `render_frame_full` and `frame_gpu_ms` stays `None`.
    ///
    /// CAUTION: Calls `device.poll(wait_indefinitely())` per frame to read back timestamps.
    /// This stalls the CPU until GPU work is done and inflates frame timings.
    /// Only call when `TUNGSTEN_GPU_TIMING=1`. Never call in production.
    pub fn render_frame_full_timed(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        text_sections: &[TextSection],
    ) -> Result<(), RenderError> {
        self.cpu_timings = CpuFrameTimings::default();
        self.gpu_timings.frame_gpu_ms = None;
        if !self.timestamp_support {
            return self.render_frame_full(view_proj, quads, sprite_batches, text_sections);
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

        let acquire_start = Instant::now();
        let output = match self.acquire_texture()? {
            Some(tex) => tex,
            None => return Ok(()),
        };
        self.cpu_timings.acquire_ms = acquire_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let encode_start = Instant::now();
        let w = self.surface_config.width;
        let h = self.surface_config.height;
        self.quad_pipeline.update_camera(&self.queue, view_proj);
        self.sprite_pipeline.update_camera(&self.queue, view_proj);
        self.text_pipeline
            .prepare(&self.device, &self.queue, text_sections, w, h);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder_timed"),
            });

        {
            let ts_writes = wgpu::RenderPassTimestampWrites {
                query_set: &query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_pass_timed"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: Some(ts_writes),
                ..Default::default()
            });

            self.quad_pipeline
                .draw(&self.device, &mut render_pass, quads);
            self.sprite_pipeline
                .draw(&self.device, &self.queue, &mut render_pass, sprite_batches);
            self.text_pipeline.render(&mut render_pass);
        }

        encoder.resolve_query_set(&query_set, 0..2, &resolve_buf, 0);
        encoder.copy_buffer_to_buffer(&resolve_buf, 0, &readback_buf, 0, 16);

        let finished = encoder.finish();
        self.cpu_timings.encode_ms = encode_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let submit_present_start = Instant::now();
        self.queue.submit(std::iter::once(finished));
        output.present();
        self.text_pipeline.post_frame();

        let slice = readback_buf.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        if receiver.recv().ok().and_then(|r| r.ok()).is_some() {
            let data = slice.get_mapped_range();
            let ts0 = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0u8; 8]));
            let ts1 = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0u8; 8]));
            drop(data);
            readback_buf.unmap();

            let period = self.queue.get_timestamp_period();
            let delta_ns = ts1.wrapping_sub(ts0) as f64 * period as f64;
            self.gpu_timings.frame_gpu_ms = Some((delta_ns / 1_000_000.0) as f32);
        }
        self.cpu_timings.submit_present_ms =
            submit_present_start.elapsed().as_secs_f64() as f32 * 1000.0;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_present_mode_preserves_vsync_selection() {
        let supported = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];

        assert_eq!(
            resolve_present_mode(&supported, None, true).unwrap(),
            wgpu::PresentMode::Fifo
        );
        assert_eq!(
            resolve_present_mode(&supported, Some(PresentModeConfig::Auto), false).unwrap(),
            wgpu::PresentMode::Immediate
        );
    }

    #[test]
    fn auto_present_mode_uses_documented_fallbacks() {
        assert_eq!(
            resolve_present_mode(&[wgpu::PresentMode::Mailbox], None, false).unwrap(),
            wgpu::PresentMode::Mailbox
        );
        assert_eq!(
            resolve_present_mode(&[wgpu::PresentMode::FifoRelaxed], None, false).unwrap(),
            wgpu::PresentMode::AutoNoVsync
        );
        assert_eq!(
            resolve_present_mode(&[wgpu::PresentMode::FifoRelaxed], None, true).unwrap(),
            wgpu::PresentMode::AutoVsync
        );
    }

    #[test]
    fn explicit_present_mode_override_beats_vsync() {
        let supported = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];
        let chosen =
            resolve_present_mode(&supported, Some(PresentModeConfig::Immediate), true).unwrap();
        assert_eq!(chosen, wgpu::PresentMode::Immediate);
    }

    #[test]
    fn unsupported_explicit_present_mode_returns_error() {
        let err = resolve_present_mode(
            &[wgpu::PresentMode::Fifo],
            Some(PresentModeConfig::Mailbox),
            false,
        )
        .unwrap_err();

        match err {
            RenderError::UnsupportedPresentMode {
                requested,
                available,
            } => {
                assert_eq!(requested, "mailbox");
                assert_eq!(available, vec!["fifo".to_string()]);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn zero_frame_latency_is_rejected() {
        let err = resolve_max_frame_latency(Some(0), wgpu::PresentMode::Fifo).unwrap_err();
        match err {
            RenderError::InvalidFrameLatency(0) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn default_frame_latency_preserves_existing_policy() {
        assert_eq!(
            resolve_max_frame_latency(None, wgpu::PresentMode::Immediate).unwrap(),
            1
        );
        assert_eq!(
            resolve_max_frame_latency(None, wgpu::PresentMode::Mailbox).unwrap(),
            1
        );
        assert_eq!(
            resolve_max_frame_latency(None, wgpu::PresentMode::AutoNoVsync).unwrap(),
            1
        );
        assert_eq!(
            resolve_max_frame_latency(None, wgpu::PresentMode::Fifo).unwrap(),
            2
        );
    }

    #[test]
    fn present_mode_labels_are_stable_lowercase_strings() {
        assert_eq!(present_mode_label(wgpu::PresentMode::Fifo), "fifo");
        assert_eq!(
            present_mode_label(wgpu::PresentMode::Immediate),
            "immediate"
        );
        assert_eq!(present_mode_label(wgpu::PresentMode::Mailbox), "mailbox");
        assert_eq!(
            present_mode_label(wgpu::PresentMode::AutoVsync),
            "auto_vsync"
        );
        assert_eq!(
            present_mode_label(wgpu::PresentMode::AutoNoVsync),
            "auto_no_vsync"
        );
    }
}
