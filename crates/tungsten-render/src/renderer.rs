use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::debug_line::{DebugLineInstance, DebugLinePipeline};
use crate::quad::{QuadInstance, QuadPipeline};
use crate::screenshot::{aligned_bytes_per_row, strip_row_padding};
use crate::sprite::{SpriteBatch, SpritePipeline};
use crate::text::{TextPipeline, TextSection};
use thiserror::Error;
use tungsten_core::assets::{FilterMode, TextureHandle};
use tungsten_core::config::{PresentModeConfig, RenderConfig};
use winit::window::Window;

/// GPU frame timing/adapter metadata.
#[derive(Debug, Clone, Default)]
pub struct GpuFrameTimings {
    /// Render-pass GPU duration; `None` without timestamp queries.
    pub frame_gpu_ms: Option<f32>,
    /// Adapter backend name.
    pub backend: Option<String>,
    /// Adapter name.
    pub adapter_name: Option<String>,
    /// Actual present mode.
    pub present_mode: Option<String>,
    /// Surface frame-latency hint.
    pub max_frame_latency: Option<u32>,
}

/// CPU render-frame timing.
#[derive(Debug, Clone, Default)]
pub struct CpuFrameTimings {
    /// Surface acquire time.
    pub acquire_ms: f32,
    /// Encode/command-recording time.
    pub encode_ms: f32,
    /// Submit/present/readback wait time.
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

        let quad_pipeline = QuadPipeline::new(&device, format);
        let sprite_pipeline = SpritePipeline::new(&device, format);
        let debug_line_pipeline =
            DebugLinePipeline::new(&device, format, quad_pipeline.camera_bind_group_layout());
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
            debug_line_pipeline,
            text_pipeline,
            timestamp_support,
            gpu_timings,
            cpu_timings: CpuFrameTimings::default(),
            pending_capture: None,
        })
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
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

    /// Drop texture pool entry and bind group.
    pub fn drop_texture(&mut self, handle: TextureHandle) {
        self.sprite_pipeline.drop_texture(handle);
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

    /// Reconfigure surface after resize.
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

    /// Clear-only frame.
    pub fn render_frame(&mut self) -> Result<(), RenderError> {
        self.render_frame_with_quads(&[])
    }

    /// D-018 direct-data quad frame with default pixel ortho.
    pub fn render_frame_with_quads(&mut self, quads: &[QuadInstance]) -> Result<(), RenderError> {
        let w = self.surface_config.width as f32;
        let h = self.surface_config.height as f32;
        let default_view_proj = glam::Mat4::orthographic_rh(0.0, w, h, 0.0, -1.0, 1.0);
        self.render_frame_full(&default_view_proj, quads, &[], &[], &[], &[])
    }

    /// Render quads, sprites, debug primitives, and screen-space text.
    pub fn render_frame_full(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
    ) -> Result<(), RenderError> {
        self.cpu_timings = CpuFrameTimings::default();
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

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        let capture_path = self.pending_capture.take();
        let capture_target = capture_path
            .as_ref()
            .map(|_| create_capture_target(&self.device, self.surface_config.format, w, h));

        encoder.push_debug_group("tungsten_frame");
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tungsten_main_pass"),
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

            self.record_main_draws(
                &mut render_pass,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
            );
        }

        if let Some(target) = capture_target.as_ref() {
            encoder.push_debug_group("tungsten_capture_pass");
            {
                let mut capture_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tungsten_capture_main_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target.view,
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

                self.record_main_draws(
                    &mut capture_pass,
                    quads,
                    sprite_batches,
                    debug_quads,
                    debug_lines,
                );
            }

            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target.texture,
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
            encoder.pop_debug_group();
        }

        encoder.pop_debug_group();

        let finished = encoder.finish();
        self.cpu_timings.encode_ms = encode_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let submit_present_start = Instant::now();
        self.queue.submit(std::iter::once(finished));
        output.present();

        if let (Some(path), Some(target)) = (capture_path, capture_target) {
            if let Err(e) = finalize_capture(&self.device, &target, w, h, &path) {
                log::warn!("screenshot capture failed: {e}");
            }
        }

        self.text_pipeline.post_frame();
        self.cpu_timings.submit_present_ms =
            submit_present_start.elapsed().as_secs_f64() as f32 * 1000.0;

        Ok(())
    }

    fn record_main_draws<'a>(
        &'a mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
    ) {
        render_pass.push_debug_group("quads");
        self.quad_pipeline.draw(&self.device, render_pass, quads);
        render_pass.pop_debug_group();

        render_pass.push_debug_group("sprites");
        self.sprite_pipeline
            .draw(&self.device, &self.queue, render_pass, sprite_batches);
        render_pass.pop_debug_group();

        render_pass.push_debug_group("debug_quads");
        self.quad_pipeline
            .draw(&self.device, render_pass, debug_quads);
        render_pass.pop_debug_group();

        render_pass.push_debug_group("debug_lines");
        self.debug_line_pipeline.draw(
            &self.device,
            render_pass,
            self.quad_pipeline.camera_bind_group(),
            debug_lines,
        );
        render_pass.pop_debug_group();

        render_pass.push_debug_group("text");
        self.text_pipeline.render(render_pass);
        render_pass.pop_debug_group();
    }

    /// Timed full frame; stalls on `device.poll(wait_indefinitely())`.
    pub fn render_frame_full_timed(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
    ) -> Result<(), RenderError> {
        self.cpu_timings = CpuFrameTimings::default();
        self.gpu_timings.frame_gpu_ms = None;
        if !self.timestamp_support {
            return self.render_frame_full(
                view_proj,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
                text_sections,
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

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder_timed"),
            });

        let capture_path = self.pending_capture.take();
        let capture_target = capture_path
            .as_ref()
            .map(|_| create_capture_target(&self.device, self.surface_config.format, w, h));

        encoder.push_debug_group("tungsten_frame");
        {
            let ts_writes = wgpu::RenderPassTimestampWrites {
                query_set: &query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tungsten_main_pass_timed"),
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

            self.record_main_draws(
                &mut render_pass,
                quads,
                sprite_batches,
                debug_quads,
                debug_lines,
            );
        }

        if let Some(target) = capture_target.as_ref() {
            encoder.push_debug_group("tungsten_capture_pass");
            {
                let mut capture_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tungsten_capture_main_pass_timed"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &target.view,
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

                self.record_main_draws(
                    &mut capture_pass,
                    quads,
                    sprite_batches,
                    debug_quads,
                    debug_lines,
                );
            }

            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target.texture,
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
            encoder.pop_debug_group();
        }

        encoder.pop_debug_group();

        encoder.resolve_query_set(&query_set, 0..2, &resolve_buf, 0);
        encoder.copy_buffer_to_buffer(&resolve_buf, 0, &readback_buf, 0, 16);

        let finished = encoder.finish();
        self.cpu_timings.encode_ms = encode_start.elapsed().as_secs_f64() as f32 * 1000.0;

        let submit_present_start = Instant::now();
        self.queue.submit(std::iter::once(finished));
        output.present();
        self.text_pipeline.post_frame();

        if let (Some(path), Some(target)) = (capture_path, capture_target) {
            if let Err(e) = finalize_capture(&self.device, &target, w, h, &path) {
                log::warn!("screenshot capture failed: {e}");
            }
        }

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
        self.cpu_timings.submit_present_ms =
            submit_present_start.elapsed().as_secs_f64() as f32 * 1000.0;

        Ok(())
    }
}

pub(crate) struct CaptureTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) readback: wgpu::Buffer,
    pub(crate) padded_bytes_per_row: u32,
}

pub(crate) fn create_capture_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> CaptureTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tungsten_screenshot_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let padded_bytes_per_row = aligned_bytes_per_row(width);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("tungsten_screenshot_readback"),
        size: u64::from(padded_bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    CaptureTarget {
        texture,
        view,
        readback,
        padded_bytes_per_row,
    }
}

pub(crate) fn finalize_capture(
    device: &wgpu::Device,
    target: &CaptureTarget,
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
        target.texture.format(),
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

#[cfg(test)]
#[path = "tests/renderer.rs"]
mod tests;
