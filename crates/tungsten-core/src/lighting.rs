//! M29 lighting resources and constants.
//!
//! `LIGHT_CAP` mirrors `tungsten_render::LIT_LIGHT_CAP` and is the hard upper
//! bound on lights packed into a single `LightUbo`.

use glam::Vec3;

/// Maximum number of lights packed into the per-frame `LightUbo`. Raising this
/// requires a UBO resize and a matching shader literal change.
pub const LIGHT_CAP: usize = 16;

/// Scene-wide ambient light contribution. Default `Vec3::ONE` keeps the lit
/// path output equal to albedo when no lights are present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight(pub Vec3);

impl Default for AmbientLight {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

#[cfg(test)]
#[path = "tests/lighting.rs"]
mod tests;
