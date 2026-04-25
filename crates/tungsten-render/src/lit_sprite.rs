//! M29 lit sprite pipeline. Reuses the sprite vertex/instance layout but
//! adds a third bind group (the lighting UBO) and binds a lit texture bundle
//! at group 1. Compile-time `include_str!` matches `assets/shaders/lit_sprite.wgsl`
//! so manifest hot-reload picks up body edits via `Renderer::reload_shader`.

use crate::sprite::SpritePipeline;

/// Manifest id used by the lit sprite shader. Mirrors `assets/manifest.json`.
pub const LIT_SPRITE_SHADER_NAME: &str = "lit_sprite";
/// Manifest id used by the optional emissive-mask helper shader.
pub const EMISSIVE_MASK_SHADER_NAME: &str = "emissive_mask";
/// Manifest id used by the optional rim-light helper shader.
pub const RIM_LIGHT_SHADER_NAME: &str = "rim_light";

/// Compile-time source for the lit sprite shader. The runtime path also
/// reads it through `assets/manifest.json` so a body edit hot-reloads via
/// `LitSpritePipeline::rebuild_with_shader`.
pub const LIT_SPRITE_SHADER_SOURCE: &str = include_str!("../../../assets/shaders/lit_sprite.wgsl");

/// M29 lit sprite pipeline. Owns its `wgpu::PipelineLayout` so a hot-reload
/// rebuild keeps bind-group indices stable even if `LitSpritePipeline::pipeline`
/// is replaced.
pub struct LitSpritePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub pipeline_layout: wgpu::PipelineLayout,
}

impl LitSpritePipeline {
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        camera_bgl: &wgpu::BindGroupLayout,
        lit_texture_bgl: &wgpu::BindGroupLayout,
        lighting_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) -> Self {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lit_sprite_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bgl), Some(lit_texture_bgl), Some(lighting_bgl)],
            immediate_size: 0,
        });
        let pipeline = build_lit_sprite_pipeline(
            device,
            module,
            &pipeline_layout,
            surface_format,
            sample_count,
            depth_write,
        );
        Self {
            pipeline,
            pipeline_layout,
        }
    }

    /// Rebuild only the pipeline against `module`, preserving the layout.
    pub fn rebuild_with_shader(
        &mut self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) {
        self.pipeline = build_lit_sprite_pipeline(
            device,
            module,
            &self.pipeline_layout,
            surface_format,
            sample_count,
            depth_write,
        );
    }
}

fn build_lit_sprite_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    sample_count: u32,
    depth_write: bool,
) -> wgpu::RenderPipeline {
    let vertex_layouts = SpritePipeline::vertex_layouts();
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("lit_sprite_pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &vertex_layouts,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        depth_stencil: if depth_write {
            Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
        } else {
            None
        },
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
#[path = "tests/lit_sprite.rs"]
mod tests;
