use super::{StockPipeline, StockResources};

pub(crate) fn build(
    device: &wgpu::Device,
    resources: &StockResources,
    format: wgpu::TextureFormat,
) -> StockPipeline {
    StockPipeline::new(
        device,
        resources,
        "pixelate",
        include_str!("../shaders/stock/pixelate.wgsl"),
        format,
    )
}
