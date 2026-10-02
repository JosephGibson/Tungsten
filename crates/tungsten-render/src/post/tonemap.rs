/// Manifest ID of the effect's shader.
pub(crate) const NAME: &str = "tonemap";
/// Its compiled-in source; `assets/shaders/stock/tonemap.wgsl` mirrors it.
pub(crate) const WGSL: &str = include_str!("../shaders/stock/tonemap.wgsl");
