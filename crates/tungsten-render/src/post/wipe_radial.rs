use super::{StockPipeline, StockResources};

pub(crate) fn build(
    device: &wgpu::Device,
    resources: &StockResources,
    format: wgpu::TextureFormat,
) -> StockPipeline {
    StockPipeline::new(
        device,
        resources,
        "wipe_radial",
        include_str!("../shaders/stock/wipe_radial.wgsl"),
        format,
    )
}
