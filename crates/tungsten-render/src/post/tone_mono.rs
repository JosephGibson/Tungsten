use super::{StockPipeline, StockResources};

pub(crate) fn build(
    device: &wgpu::Device,
    resources: &StockResources,
    format: wgpu::TextureFormat,
) -> StockPipeline {
    StockPipeline::new(
        device,
        resources,
        "tone_mono",
        include_str!("../shaders/stock/tone_mono.wgsl"),
        format,
    )
}
