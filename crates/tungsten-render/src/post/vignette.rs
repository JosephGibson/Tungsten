use super::{StockPipeline, StockResources};

pub(crate) fn build(
    device: &wgpu::Device,
    resources: &StockResources,
    format: wgpu::TextureFormat,
) -> StockPipeline {
    StockPipeline::new(
        device,
        resources,
        "vignette",
        include_str!("../shaders/stock/vignette.wgsl"),
        format,
    )
}
