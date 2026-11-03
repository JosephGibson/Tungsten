//! M29 lighting resources and constants, and the light at a point (redo
//! 7.10b).
//!
//! `LIGHT_CAP` mirrors `tungsten_render::LIT_LIGHT_CAP` and is the hard upper
//! bound on lights packed into a single `LightUbo`.
//!
//! [`light_at`] reads, on the CPU, what the lit shader
//! (`assets/shaders/lit_sprite.wgsl`) gives a flat white sprite at a point:
//! no albedo, no ambient, no 16-light cap and no view, so a fixed-step read
//! never follows the camera. A test reads the shader's light lines and fails
//! when they change.

use glam::{Vec2, Vec3};

use crate::components::{Light, LightKind, Transform};
use crate::ecs::World;

/// Maximum number of lights packed into the per-frame `LightUbo`. Raising this
/// requires a UBO resize and a matching shader literal change.
pub const LIGHT_CAP: usize = 16;

/// The least radius a point light is taken at, as the lit shader clamps it.
pub const MIN_LIGHT_RADIUS: f32 = 1e-4;

/// Scene-wide ambient light contribution. Default `Vec3::ONE` keeps the lit
/// path output equal to albedo when no lights are present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight(pub Vec3);

impl Default for AmbientLight {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

/// What a flat white lit sprite at `point` gets from one `light` at
/// `position`, as the lit shader computes it.
///
/// A point light gives `color · intensity · a² · h / √(d² + h²)`: `d` the
/// distance from `position`, `r` its radius taken as at least
/// [`MIN_LIGHT_RADIUS`], `a = clamp(1 − d / r, 0, 1)` and `h = r / 2`, the
/// height the shader floats it at. A directional light gives
/// `color · intensity / √2`, a flat normal's share of its `(cos, sin, 1)`
/// direction. A contribution that is not finite, or a directional light
/// whose angle is not, counts as none.
///
/// ```
/// use glam::{Vec2, Vec3};
/// use tungsten_core::{Light, lighting::light_from};
///
/// let lamp = Light::point(Vec3::ONE, 100.0);
/// assert_eq!(light_from(&lamp, Vec2::ZERO, Vec2::ZERO), Vec3::ONE);
/// assert_eq!(light_from(&lamp, Vec2::ZERO, Vec2::new(100.0, 0.0)), Vec3::ZERO);
/// ```
#[must_use]
pub fn light_from(light: &Light, position: Vec2, point: Vec2) -> Vec3 {
    let rgb = light.color * light.intensity;
    let contribution = match light.kind {
        LightKind::Point { radius } => {
            let radius = radius.max(MIN_LIGHT_RADIUS);
            let height = radius * 0.5;
            let distance = (point - position).length();
            let attenuation = (1.0 - distance / radius).clamp(0.0, 1.0);
            let n_dot_l = height / (distance * distance + height * height).sqrt();
            rgb * (attenuation * attenuation * n_dot_l)
        }
        LightKind::Directional { angle } => {
            if !angle.is_finite() {
                return Vec3::ZERO;
            }
            rgb * std::f32::consts::FRAC_1_SQRT_2
        }
    };
    if contribution.is_finite() {
        contribution
    } else {
        Vec3::ZERO
    }
}

/// The light at `point`: the sum, over every entity with a `Transform` and a
/// `Light` (as the light extract takes them), of [`light_from`] at the
/// `Transform`'s position. No ambient (a game adds [`AmbientLight`] itself),
/// no cap and no view.
///
/// ```
/// use glam::{Vec2, Vec3};
/// use tungsten_core::{Light, Transform, World, lighting::light_at};
///
/// let mut world = World::new();
/// world.spawn_with((Transform::from_position(Vec2::ZERO), Light::point(Vec3::ONE, 100.0)));
/// assert_eq!(light_at(&world, Vec2::ZERO), Vec3::ONE);
/// ```
#[must_use]
pub fn light_at(world: &World, point: Vec2) -> Vec3 {
    world
        .query::<(&Transform, &Light)>()
        .map(|(transform, light)| light_from(light, transform.position, point))
        .fold(Vec3::ZERO, |sum, rgb| sum + rgb)
}

/// The Rec. 709 luma of a light's colour: `0.2126 r + 0.7152 g + 0.0722 b`.
#[must_use]
pub fn luma(rgb: Vec3) -> f32 {
    rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// The luma of [`light_at`]: one amount for the light at `point`, which the
/// kit's chemistry reads as its light stimulus.
#[must_use]
pub fn light_level(world: &World, point: Vec2) -> f32 {
    luma(light_at(world, point))
}

#[cfg(test)]
#[path = "tests/lighting.rs"]
mod tests;
