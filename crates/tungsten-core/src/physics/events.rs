//! Physics collision events.

use crate::ecs::Entity;
use glam::Vec2;

/// Resolved contact; `normal` is direction `a` was pushed. Tile `b` is `None`.
#[derive(Debug, Clone, Copy)]
pub struct CollisionEvent {
    pub a: Entity,
    pub b: Option<Entity>,
    pub normal: Vec2,
    pub penetration: f32,
}
