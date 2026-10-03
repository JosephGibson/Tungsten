//! D-042 render components; D-033 physics `Position` stays separate.

use std::sync::Arc;

use glam::{Vec2, Vec3};

use crate::assets::{AssetId, MaterialAssetId, ParticleConfig, ParticleMeshAssetId};
use crate::ecs::{Entity, World};
use crate::physics::Position;
use crate::rng::Pcg32;
use crate::tween::Easing;

/// Visual transform; rotation is radians CCW around quad center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Transform {
    /// Unit-scale transform at `position`.
    #[must_use]
    pub fn from_position(position: Vec2) -> Self {
        Self {
            position,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

/// Sprite render data resolved by asset ID at extract time.
///
/// M26: `material_id` selects a user-authored WGSL material pipeline on the
/// sprite draw path; `None` keeps the built-in sprite pipeline and the M25
/// default output bytes.
#[derive(Debug, Clone)]
pub struct Sprite {
    pub asset_id: String,
    pub color: [u8; 4],
    pub z_order: i32,
    pub material_id: Option<MaterialAssetId>,
}

impl Sprite {
    /// No tint, z-order 0, built-in sprite pipeline.
    pub fn new(asset_id: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
            color: [255; 4],
            z_order: 0,
            material_id: None,
        }
    }

    #[must_use]
    pub fn with_material(mut self, material: MaterialAssetId) -> Self {
        self.material_id = Some(material);
        self
    }
}

/// Explicit render gate required by default sprite extract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visibility {
    pub visible: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Self { visible: true }
    }
}

/// 2D forward light source (M29). Closed-enum kind per `D-054`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
}

/// Light shape (M29). `Point` carries world-space radius driving attenuation;
/// `Directional` carries a 2D angle (radians from +x axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightKind {
    Point { radius: f32 },
    Directional { angle: f32 },
}

impl Light {
    /// Point light at unit intensity with the given world-space radius.
    #[must_use]
    pub fn point(color: Vec3, radius: f32) -> Self {
        Self {
            kind: LightKind::Point { radius },
            color,
            intensity: 1.0,
        }
    }

    /// Directional light from `angle` radians (CCW from +x) at unit intensity.
    #[must_use]
    pub fn directional(color: Vec3, angle: f32) -> Self {
        Self {
            kind: LightKind::Directional { angle },
            color,
            intensity: 1.0,
        }
    }
}

/// Debug/find label.
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Declarative particle emitter.
#[derive(Debug, Clone, Copy)]
pub struct ParticleEmitter {
    pub config: AssetId<ParticleConfig>,
    pub seed_override: Option<u64>,
}

impl ParticleEmitter {
    #[must_use]
    pub fn new(config: AssetId<ParticleConfig>) -> Self {
        Self {
            config,
            seed_override: None,
        }
    }

    #[must_use]
    pub fn with_seed(config: AssetId<ParticleConfig>, seed: u64) -> Self {
        Self {
            config,
            seed_override: Some(seed),
        }
    }
}

/// Particle emitter runtime state; first tick captures config `Arc`.
#[derive(Debug, Clone)]
pub struct ParticleEmitterState {
    pub config_snapshot: Option<Arc<ParticleConfig>>,
    pub rng: Pcg32,
    pub elapsed: f32,
    pub continuous_accum: f32,
    pub pulse_timer: f32,
    pub pulses_fired: u32,
    pub active_count: u32,
    pub drained: bool,
    pub first_tick_done: bool,
    pub drain_reported: bool,
}

impl Default for ParticleEmitterState {
    fn default() -> Self {
        Self {
            config_snapshot: None,
            rng: Pcg32::seeded(0),
            elapsed: 0.0,
            continuous_accum: 0.0,
            pulse_timer: 0.0,
            pulses_fired: 0,
            active_count: 0,
            drained: false,
            first_tick_done: false,
            drain_reported: false,
        }
    }
}

/// Live particle; spawn-time config snapshot preserves hot-reload isolation.
#[derive(Debug, Clone)]
pub struct Particle {
    pub config: Arc<ParticleConfig>,
    pub emitter: Option<Entity>,
    pub age: f32,
    pub lifetime: f32,
    pub velocity: Vec2,
    pub angular_velocity: f32,
    pub start_scale: f32,
    pub base_rgba: [f32; 4],
}

/// Mesh-drawn particle (M31, `D-093`); takes the place of `Sprite` on a particle entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshParticle {
    pub mesh: ParticleMeshAssetId,
    pub color: [u8; 4],
}

/// Parallax scroll factor per axis (M30, `D-073`). `1.0` = world-locked,
/// `0.0` = screen-locked. Applied at extract time; never mutates `Transform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParallaxLayer {
    pub scroll_factor: Vec2,
}

impl ParallaxLayer {
    #[must_use]
    pub fn new(scroll_factor: Vec2) -> Self {
        Self { scroll_factor }
    }

    /// Uniform factor on both axes.
    #[must_use]
    pub fn uniform(factor: f32) -> Self {
        Self {
            scroll_factor: Vec2::splat(factor),
        }
    }
}

/// Parallax position remap (M30, `D-073`): a CPU-side offset applied at extract
/// time, so one view-projection still draws every layer correctly and
/// `crates/tungsten-render/` needs no per-layer camera.
///
/// `scroll_factor == 1.0` returns `authored` unchanged (world-locked);
/// `0.0` pins the layer to the camera (screen-locked).
#[inline]
#[must_use]
pub fn parallax_world_position(authored: Vec2, scroll_factor: Vec2, camera_position: Vec2) -> Vec2 {
    authored + camera_position * (Vec2::ONE - scroll_factor)
}

/// Which gameplay trigger arms this entity's squash (M30). Closed enum, `D-054` style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquashTrigger {
    OnLand,
    OnHit,
    OnPickup,
    Manual,
}

/// Squash/stretch config (M30). `amount` is the peak scale multiplier
/// (e.g. `(1.3, 0.7)` = widen and flatten).
///
/// `D-073`: deliberately not a `Tween` — `TweenRepeat` has no out-and-back
/// one-shot, and `D-055`'s single slot per entity is already spent on the M26
/// damage flash for the entity that must both squash and flash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteSquashStretch {
    pub on: SquashTrigger,
    pub amount: Vec2,
    pub duration: f32,
    pub easing: Easing,
}

impl SpriteSquashStretch {
    /// Symmetric envelope sample: `sin(easing(t) * PI)` scales `amount`'s
    /// deviation from `1.0`, so the peak is `amount` and both endpoints return
    /// `base_scale` exactly — no separate cleanup write is needed.
    ///
    /// A non-positive `duration` is inert rather than a divide by zero.
    #[must_use]
    pub fn scale_at(&self, base_scale: Vec2, elapsed: f32) -> Vec2 {
        if self.duration <= 0.0 || elapsed <= 0.0 || elapsed >= self.duration {
            return base_scale;
        }
        let env = (self.easing.apply(elapsed / self.duration) * std::f32::consts::PI).sin();
        base_scale * (Vec2::ONE + (self.amount - Vec2::ONE) * env)
    }
}

/// Squash/stretch runtime envelope state (M30), inserted by
/// `squash_stretch_trigger_system`. `base_scale` is captured at trigger time so
/// the envelope restores the authored scale and a re-trigger mid-flight cannot
/// compound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquashStretchState {
    pub elapsed: f32,
    pub base_scale: Vec2,
}

/// D-033 one-way sync: physics `Position` -> visual `Transform.position`.
pub fn sync_position_to_transform(world: &mut World) {
    for (_entity, transform, position) in world.query2_mut::<Transform, Position>() {
        transform.position = position.0;
    }
}

#[cfg(test)]
#[path = "tests/components.rs"]
mod tests;
