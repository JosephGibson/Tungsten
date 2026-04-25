//! WGPU rendering backend: quads, sprites, debug lines, screenshots, text.

pub mod debug_line;
pub mod image_diff;
pub mod material;
pub mod passes;
pub mod post;
pub mod quad;
pub mod renderer;
pub mod screenshot;
pub mod shader_hot_reload;
pub mod sprite;
pub mod surface;
pub mod targets;
pub mod text;
pub mod timing;

pub use debug_line::{DebugLineInstance, DebugLinePipeline};
pub use image_diff::{compare_png, DiffReport, ImageDiffError};
pub use material::{MaterialBuildError, MaterialPipeline};
pub use passes::{default_pass_order, PassDesc, PassOrder, PassRecorder, TargetId};
pub use post::smaa::{
    SmaaPipeline, SmaaPreset, SmaaPresetUbo, SmaaShaderIds, SMAA_BLEND_WEIGHTS_SHADER_NAME,
    SMAA_EDGE_SHADER_NAME, SMAA_NEIGHBORHOOD_BLEND_SHADER_NAME,
};
pub use post::PostStackRenderer;
pub use quad::QuadInstance;
pub use renderer::{CpuFrameTimings, GpuFrameTimings, Renderer};
pub use screenshot::ScreenshotError;
pub use shader_hot_reload::{validate_wgsl_source, ShaderError, ShaderModuleCache};
pub use sprite::{SpriteBatch, SpriteInstance, SpritePipeline};
pub use targets::{RenderTargetPool, SceneTarget};
pub use text::{TextPipeline, TextSection};
