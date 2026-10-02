/// Manifest ID of the effect's shader.
pub(crate) const NAME: &str = "dither";
/// Its compiled-in source; `assets/shaders/stock/dither.wgsl` mirrors it.
pub(crate) const WGSL: &str = include_str!("../shaders/stock/dither.wgsl");
