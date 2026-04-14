//! Collision events. Each physics step clears the event list and then
//! appends one `CollisionEvent` per resolved contact. Game systems read
//! this resource after the physics step to react (ground detection,
//! trigger volumes, damage, etc).

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

/// Resource: per-frame collision events. Cleared at the start of every
/// physics step and refilled during narrow-phase resolution.
#[derive(Debug, Clone, Default)]
pub struct CollisionEvents {
    pub events: Vec<CollisionEvent>,
}

impl CollisionEvents {
    pub fn new() -> Self {
        Self::default()
    }

    /// True if any event names `entity` as the `a` side (the dynamic
    /// body that was pushed out). Game code uses this for ground
    /// detection and trigger polling.
    pub fn involves(&self, entity: Entity) -> bool {
        self.events
            .iter()
            .any(|e| e.a == entity || e.b == Some(entity))
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn push(&mut self, event: CollisionEvent) {
        self.events.push(event);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
