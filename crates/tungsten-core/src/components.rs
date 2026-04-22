//! Engine-level game components introduced in M15 (see D-042).
//!
//! Four small, self-contained types that other engine subsystems and the
//! default sprite-extract path build on:
//!
//! - [`Transform`] — world-space position, rotation (radians, CCW positive),
//!   and per-axis scale. Rotation is applied around the quad centre by the
//!   sprite shader.
//! - [`Sprite`] — asset lookup + tint + z-order. The tint multiplies the
//!   sampled texel at draw time (`[255; 4]` = no tint). `z_order` is a stable
//!   ascending sort key used by the default extract.
//! - [`Visibility`] — explicit render gate. Required by the default sprite
//!   extract path: entities with `Transform + Sprite` but no `Visibility` are
//!   never emitted. No implicit fallback (D-042).
//! - [`Tag`] — a debug-friendly name for find-by-name lookups.
//!
//! Physics `Position` (see [`crate::physics::Position`]) stays separate per
//! `D-033`. Use [`sync_position_to_transform`] to opt in to copying
//! `Position.0` into `Transform.position` after the physics step; there is no
//! reverse sync.
//!
//! `Transform` is `Copy`; `Sprite` and `Tag` hold `String`s and are not.

use std::sync::Arc;

use glam::Vec2;

use crate::assets::{AssetId, ParticleConfig};
use crate::ecs::{Entity, World};
use crate::physics::Position;
use crate::rng::Pcg32;

/// World-space transform for a visual entity. Rotation is in radians, CCW
/// positive, applied around the quad centre at draw time; scale is per-axis
/// and multiplies the sprite's intrinsic pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Transform {
    /// Identity rotation, unit scale, with the given position.
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

/// Render description for a sprite entity. `asset_id` is resolved against
/// [`crate::AssetRegistry`] at extract time. `color` is an RGBA tint applied
/// at draw time (`[255; 4]` = no tint). `z_order` is a stable ascending sort
/// key for the default extract path; larger values render on top.
#[derive(Debug, Clone)]
pub struct Sprite {
    pub asset_id: String,
    pub color: [u8; 4],
    pub z_order: i32,
}

impl Sprite {
    /// Defaults to no tint (`[255; 4]`) and `z_order = 0`.
    pub fn new(asset_id: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
            color: [255; 4],
            z_order: 0,
        }
    }
}

/// Explicit render gate. Required by the default sprite extract: entities
/// with `Transform + Sprite` but no `Visibility` are never emitted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visibility {
    pub visible: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Self { visible: true }
    }
}

/// Debug / find-by-name label for an entity. Not consulted by any render
/// path; examples and tools use it to locate specific entities.
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Declarative particle emitter. The config id resolves against
/// [`ParticleConfigRegistry`](crate::assets::ParticleConfigRegistry); the
/// optional seed override pins the per-emitter RNG for reproducible replays.
///
/// An emitter is always paired with a [`ParticleEmitterState`] — one is the
/// immutable declaration, the other holds the evolving runtime state.
#[derive(Debug, Clone, Copy)]
pub struct ParticleEmitter {
    pub config: AssetId<ParticleConfig>,
    pub seed_override: Option<u64>,
}

impl ParticleEmitter {
    pub fn new(config: AssetId<ParticleConfig>) -> Self {
        Self {
            config,
            seed_override: None,
        }
    }

    pub fn with_seed(config: AssetId<ParticleConfig>, seed: u64) -> Self {
        Self {
            config,
            seed_override: Some(seed),
        }
    }
}

/// Runtime state for a [`ParticleEmitter`]. Holds the `Arc` snapshot taken at
/// first tick so hot-reloads can swap the registry entry without perturbing
/// in-flight emitters (plan: "in-flight snapshot semantics").
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

/// A live particle. Stored per-entity alongside `Transform + Sprite +
/// Visibility` so the default M15 extract path picks it up. All sampled
/// values are captured at spawn — the `Arc<ParticleConfig>` lives on the
/// particle so hot-reload cannot retroactively rewrite its motion or colour.
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

/// Opt-in system: copies `Position.0` into `Transform.position` for every
/// entity that carries both components. Does not touch rotation or scale.
///
/// Physics `Position` remains the source of truth per `D-033`; this is a
/// one-way sync meant to run after `physics_step` and before any sprite
/// extract stage that needs authoritative post-physics visuals. No reverse
/// sync.
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
