//! Collision event payloads emitted by the physics step.

use crate::ecs::Entity;
use glam::Vec2;

/// A single resolved contact from the narrow-phase.
///
/// `normal` points **from `a` into the free space of `b`** — i.e. the
/// direction `a` was pushed to resolve the penetration. Ground detection
/// on a downward-gravity world is thus `normal.y < 0.0` from the falling
/// body's perspective (the ground pushes it up).
///
/// `b_entity == None` when the other side is a static tilemap tile:
/// tile colliders are transient and don't own an Entity id.
#[derive(Debug, Clone, Copy)]
pub struct CollisionEvent {
    pub a: Entity,
    pub b: Option<Entity>,
    pub normal: Vec2,
    pub penetration: f32,
}
