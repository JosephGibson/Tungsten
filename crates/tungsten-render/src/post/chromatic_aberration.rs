use super::{StockPipeline, StockResources};

pub(crate) fn build(
    device: &wgpu::Device,
    resources: &StockResources,
    format: wgpu::TextureFormat,
) -> StockPipeline {
    StockPipeline::new(
        device,
        resources,
        "chromatic_aberration",
        include_str!("../shaders/stock/chromatic_aberration.wgsl"),
        format,
    )
}
