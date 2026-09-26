//! WGPU rendering backend: quads, sprites, debug lines, screenshots, text.

pub mod debug_line;
pub mod image_diff;
pub mod lighting;
pub mod lit_sprite;
pub mod material;
pub mod passes;
pub mod post;
pub mod quad;
pub mod renderer;
pub mod screenshot;
pub mod shader_hot_reload;
pub mod sprite;
pub mod surface;
mod surface_acquire;
pub mod targets;
pub mod text;
pub mod timing;

pub use debug_line::{DebugLineInstance, DebugLinePipeline};
pub use image_diff::{DiffReport, ImageDiffError, compare_png};
pub use lighting::{
    GpuLight, LIT_LIGHT_CAP, LightUbo, LightingResources, cull_to_cap, distance_to_aabb_sq,
    pack_lights, pack_one_light,
};
pub use lit_sprite::{
    EMISSIVE_MASK_SHADER_NAME, LIT_SPRITE_SHADER_NAME, LIT_SPRITE_SHADER_SOURCE, LitSpritePipeline,
    RIM_LIGHT_SHADER_NAME,
};
pub use material::{MaterialBuildError, MaterialPipeline};
pub use passes::{PassDesc, PassOrder, PassRecorder, TargetId, default_pass_order};
pub use post::PostStackRenderer;
pub use post::smaa::{
    SMAA_BLEND_WEIGHTS_SHADER_NAME, SMAA_EDGE_SHADER_NAME, SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
    SmaaPipeline, SmaaPreset, SmaaPresetUbo, SmaaShaderIds,
};
pub use quad::QuadInstance;
pub use renderer::{CpuFrameTimings, GpuFrameTimings, Renderer};
pub use screenshot::ScreenshotError;
pub use shader_hot_reload::{ShaderError, ShaderModuleCache, validate_wgsl_source};
pub use sprite::{SpriteBatch, SpriteInstance, SpritePipeline};
pub use targets::{RenderTargetPool, SceneTarget};
pub use text::{TextPipeline, TextSection};
