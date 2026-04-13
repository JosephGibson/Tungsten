use std::sync::Arc;

use crate::quad::{QuadInstance, QuadPipeline};
use crate::sprite::{SpriteBatch, SpritePipeline};
use crate::text::{TextPipeline, TextSection};
use thiserror::Error;
use tungsten_core::assets::TextureHandle;
use tungsten_core::config::RenderConfig;
use winit::window::Window;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("no suitable GPU adapter found: {0}")]
    NoAdapter(#[from] wgpu::RequestAdapterError),
    #[error("failed to request device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("surface error: {0}")]
    Surface(String),
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

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("tungsten_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }))?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = if vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

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
        let output = match self.acquire_texture()? {
            Some(tex) => tex,
            None => return Ok(()),
        };

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
                .draw(&self.device, &mut render_pass, sprite_batches);
            self.text_pipeline.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.text_pipeline.post_frame();

        Ok(())
    }
}
