/// Manifest ID of the effect's shader.
pub(crate) const NAME: &str = "lut";
/// Its compiled-in source; `assets/shaders/stock/lut.wgsl` mirrors it.
pub(crate) const WGSL: &str = include_str!("../shaders/stock/lut.wgsl");
