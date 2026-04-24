//! D-042 render components; D-033 physics `Position` stays separate.

use std::sync::Arc;

use glam::Vec2;

use crate::assets::{AssetId, MaterialAssetId, ParticleConfig};
use crate::ecs::{Entity, World};
use crate::physics::Position;
use crate::rng::Pcg32;

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

/// D-033 one-way sync: physics `Position` -> visual `Transform.position`.
pub fn sync_position_to_transform(world: &mut World) {
    let entities = world.query2_entities::<Position, Transform>();
    for entity in entities {
        let position = match world.get::<Position>(entity) {
            Some(p) => p.0,
            None => continue,
        };
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.position = position;
        }
    }
}

#[cfg(test)]
#[path = "tests/components.rs"]
mod tests;
