//! Physics events: contacts and sensor overlaps.

use crate::ecs::Entity;
use glam::Vec2;

/// A contact the step resolved, sent once it penetrates. `a` is the body the
/// contact pushed and `normal` the direction it was pushed; against a
/// kinematic body, `a` is always the dynamic one. `b` is the other body, or
/// `None` for a tile. Build one with [`CollisionEvent::new`].
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct CollisionEvent {
    /// The body the contact pushed.
    pub a: Entity,
    /// The other body; `None` for a tile.
    pub b: Option<Entity>,
    /// The direction `a` was pushed, a unit vector.
    pub normal: Vec2,
    /// How deep the two overlapped; 0 for the safety net's clamp.
    pub penetration: f32,
}

impl CollisionEvent {
    /// An event with these fields, as the step sends one.
    #[must_use]
    pub fn new(a: Entity, b: Option<Entity>, normal: Vec2, penetration: f32) -> Self {
        Self {
            a,
            b,
            normal,
            penetration,
        }
    }
}

/// A sensor began to overlap a body (`other`) or a solid tile (`other:
/// None`) in a layer its mask holds ([`RigidBody`](crate::physics::RigidBody)),
/// sent at the end of the step in which the overlap began: once per pair
/// while it lasts, and again after the two separate and overlap anew. A body
/// already overlapping when the sensor's mask comes to hold its layer is
/// reported at that step. Build one with [`SensorEntered::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SensorEntered {
    /// The sensor.
    pub sensor: Entity,
    /// The body it began to overlap; `None` for a tile.
    pub other: Option<Entity>,
}

impl SensorEntered {
    /// An event with these fields, as the step sends one.
    #[must_use]
    pub fn new(sensor: Entity, other: Option<Entity>) -> Self {
        Self { sensor, other }
    }
}
