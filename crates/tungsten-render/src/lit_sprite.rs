//! M29 lit sprite pipeline. Reuses the sprite vertex/instance layout but
//! adds a third bind group (the lighting UBO) and binds a lit texture bundle
//! at group 1. Compile-time `include_str!` matches `assets/shaders/lit_sprite.wgsl`
//! so manifest hot-reload picks up body edits via `Renderer::reload_shader`.
//! Redo 7.9 adds group 3, each batch's 256-byte uniform block, whose flash
//! lanes (`vec4[0]`, `f32s[0]`) the shader mixes in as the stock flash does.

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
    /// Under `DepthSortMode::GpuDepth`, the pipeline a solid batch draws
    /// with: `fs_solid`, writing depth (redo 7.15).
    pub(crate) pipeline_solid: Option<wgpu::RenderPipeline>,
    /// Whether the renderer asked for the pipelines of a frame that walks
    /// the lights' tiles (`render.light_cap` past the block's 16, `D-310`),
    /// so a shader reload builds them again.
    wants_tiled: bool,
    /// Those pipelines, the tested one and under the depth sort the solid
    /// one: the shader's `tiled_lights` on and its vertex mark off. `None`
    /// where not asked for, and for a lit file that declares no
    /// `tiled_lights`, a game's written before it, whose frames of more
    /// than 16 lights draw the block's 16 with the pipelines above.
    tiled: Option<(wgpu::RenderPipeline, Option<wgpu::RenderPipeline>)>,
    /// Whether the frame being recorded walks tiles: which pair it draws
    /// with ([`Self::set_frame_tiled`]).
    frame_tiled: bool,
    pub pipeline_layout: wgpu::PipelineLayout,
    /// Group 3: a batch's `UniformOverrideBlock`, 256 bytes (`D-101`).
    pub flash_bind_group_layout: wgpu::BindGroupLayout,
}

impl LitSpritePipeline {
    #[must_use]
    #[allow(clippy::too_many_arguments)] // mirrors SpritePipeline's stable constructor surface; see D-061
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
        let flash_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("lit_sprite_flash_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lit_sprite_pipeline_layout"),
            bind_group_layouts: &[
                Some(camera_bgl),
                Some(lit_texture_bgl),
                Some(lighting_bgl),
                Some(&flash_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let pipeline = build_lit_sprite_pipeline(
            device,
            module,
            &pipeline_layout,
            surface_format,
            sample_count,
            depth_write.then_some(false),
            false,
        );
        let pipeline_solid = depth_write.then(|| {
            build_lit_sprite_pipeline(
                device,
                module,
                &pipeline_layout,
                surface_format,
                sample_count,
                Some(true),
                false,
            )
        });
        Self {
            pipeline,
            pipeline_solid,
            wants_tiled: false,
            tiled: None,
            frame_tiled: false,
            pipeline_layout,
            flash_bind_group_layout,
        }
    }

    /// Builds the pipelines of a frame that walks the lights' tiles
    /// (`D-310`) against `module`, and keeps them built over a shader
    /// reload. A module that does not build with `tiled_lights` set is
    /// logged and has none: a frame of more than 16 lights then draws the
    /// block's 16.
    pub fn build_tiled(
        &mut self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) {
        self.wants_tiled = true;
        self.tiled = self.tiled_pair(device, module, surface_format, sample_count, depth_write);
    }

    fn tiled_pair(
        &self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) -> Option<(wgpu::RenderPipeline, Option<wgpu::RenderPipeline>)> {
        let build = |write: Option<bool>| {
            crate::shader_hot_reload::build_validated(device, || {
                build_lit_sprite_pipeline(
                    device,
                    module,
                    &self.pipeline_layout,
                    surface_format,
                    sample_count,
                    write,
                    true,
                )
            })
        };
        let pair = build(depth_write.then_some(false)).and_then(|tested| {
            let solid = if depth_write {
                Some(build(Some(true))?)
            } else {
                None
            };
            Ok((tested, solid))
        });
        match pair {
            Ok(pair) => Some(pair),
            Err(report) => {
                log::warn!(
                    "lit shader: no pipeline that walks the lights' tiles, so a frame of more than 16 lights draws its first 16: {report}"
                );
                None
            }
        }
    }

    /// Whether a frame can walk the lights' tiles: the tiled pipelines are
    /// built.
    #[must_use]
    pub fn has_tiled(&self) -> bool {
        self.tiled.is_some()
    }

    /// Sets which pipelines the frame being recorded draws its lit batches
    /// with: the tiled ones for a frame that walks tiles, where they are
    /// built.
    pub fn set_frame_tiled(&mut self, tiled: bool) {
        self.frame_tiled = tiled && self.tiled.is_some();
    }

    /// The pipeline the frame's lit batches draw with: the tested one, or
    /// the `solid` one of the depth sort, which only that sort builds.
    pub(crate) fn frame_pipeline(&self, solid: bool) -> Option<&wgpu::RenderPipeline> {
        match &self.tiled {
            Some((tested, solid_one)) if self.frame_tiled => {
                if solid {
                    solid_one.as_ref()
                } else {
                    Some(tested)
                }
            }
            _ => {
                if solid {
                    self.pipeline_solid.as_ref()
                } else {
                    Some(&self.pipeline)
                }
            }
        }
    }

    /// Rebuild only the pipelines against `module`, preserving the layout:
    /// under `depth_write` the tested one and the solid one. A pipeline that
    /// fails validation is not swapped in (`Err`), and neither is its pair.
    pub fn rebuild_with_shader(
        &mut self,
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        surface_format: wgpu::TextureFormat,
        sample_count: u32,
        depth_write: bool,
    ) -> Result<(), String> {
        let build = |write: Option<bool>| {
            crate::shader_hot_reload::build_validated(device, || {
                build_lit_sprite_pipeline(
                    device,
                    module,
                    &self.pipeline_layout,
                    surface_format,
                    sample_count,
                    write,
                    false,
                )
            })
        };
        let pipeline = build(depth_write.then_some(false))?;
        let pipeline_solid = if depth_write {
            Some(build(Some(true))?)
        } else {
            None
        };
        self.pipeline = pipeline;
        self.pipeline_solid = pipeline_solid;
        if self.wants_tiled {
            self.tiled = self.tiled_pair(device, module, surface_format, sample_count, depth_write);
        }
        Ok(())
    }
}

fn build_lit_sprite_pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    sample_count: u32,
    depth: Option<bool>,
    tiled: bool,
) -> wgpu::RenderPipeline {
    // `depth` as the sprite pipeline's: none, tested, or tested and written
    // by a solid batch's `fs_solid`.
    let solid = depth == Some(true);
    let vertex_layouts = SpritePipeline::vertex_layouts();
    // A quad's vertices mark the lights that cannot reach it, which holds
    // while a fragment's position lies inside its quad: without MSAA (redo
    // 7.15; the shader's `skip_lights`). A frame that walks tiles reads no
    // mark, and `tiled_lights` is set for its pipelines alone, so a lit
    // file that declares none still builds the others (`D-310`).
    let mark = if sample_count == 1 && !tiled {
        1.0
    } else {
        0.0
    };
    let plain = [("skip_lights", mark)];
    let walking = [("skip_lights", mark), ("tiled_lights", 1.0)];
    let constants: &[(&str, f64)] = if tiled { &walking } else { &plain };
    let compilation_options = wgpu::PipelineCompilationOptions {
        constants,
        ..Default::default()
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match (solid, tiled) {
            (true, false) => "lit_sprite_solid_pipeline",
            (false, false) => "lit_sprite_pipeline",
            (true, true) => "lit_sprite_solid_tiled_pipeline",
            (false, true) => "lit_sprite_tiled_pipeline",
        }),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            buffers: &vertex_layouts,
            compilation_options: compilation_options.clone(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(if solid { "fs_solid" } else { "fs_main" }),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth.map(|write| wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(write),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
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
