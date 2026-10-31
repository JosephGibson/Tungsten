//! Physics events: contacts, their begins and ends, and sensor overlaps.

use crate::ecs::Entity;
use crate::physics::TileHit;
use glam::Vec2;

/// A contact the step resolved, sent once it penetrates. `a` is the body the
/// contact pushed and `normal` the direction it was pushed; against a
/// kinematic body, `a` is always the dynamic one. `b` is the other body, or
/// `None` for a tile, whose cell `tile` names. Build one with
/// [`CollisionEvent::new`].
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
    /// The closing speed along the normal when the contact was built, in
    /// px/s, 0 for one separating; a clamp's, the speed it stopped. After a
    /// speculative arrival it is small: the solve already removed the
    /// approach. [`ContactBegan::speed`] keeps an impact's.
    pub speed: f32,
    /// The tile `b` stands for: its map, layer and cell.
    pub tile: Option<TileHit>,
}

impl CollisionEvent {
    /// An event with these fields, a speed of 0 and no tile, as the step
    /// sends one.
    #[must_use]
    pub fn new(a: Entity, b: Option<Entity>, normal: Vec2, penetration: f32) -> Self {
        Self {
            a,
            b,
            normal,
            penetration,
            speed: 0.0,
            tile: None,
        }
    }

    /// The event with its closing speed `speed`.
    #[must_use]
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// The event with its tile side's cell `tile`.
    #[must_use]
    pub fn with_tile(mut self, tile: TileHit) -> Self {
        self.tile = Some(tile);
        self
    }
}

/// Two bodies, or a body and a tile, began to touch: sent at the end of the
/// step in which the pair touched and the step before did not, after the
/// collision events. A pair touches in a substep when its contact
/// penetrates or the solve gave it a normal impulse, or a clamp of the
/// arrival pass or the safety net stopped one against the other. `a`, `b`
/// and `normal` are its first touching contact's, as [`CollisionEvent`]
/// names them; `speed` is the largest closing speed along the normal the
/// pair had since it came within reach, so an impact reads its speed
/// however the gap closed. A pair is its two sides, a tile by its map,
/// layer and cell.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ContactBegan {
    /// The body the first contact pushed.
    pub a: Entity,
    /// The other body; `None` for a tile.
    pub b: Option<Entity>,
    /// The direction `a` was pushed, a unit vector.
    pub normal: Vec2,
    /// The impact's closing speed along the normal, in px/s.
    pub speed: f32,
    /// The tile `b` stands for.
    pub tile: Option<TileHit>,
}

impl ContactBegan {
    /// A begin with these fields and no tile, as the step sends one.
    #[must_use]
    pub fn new(a: Entity, b: Option<Entity>, normal: Vec2, speed: f32) -> Self {
        Self {
            a,
            b,
            normal,
            speed,
            tile: None,
        }
    }

    /// The begin with its tile side's cell `tile`.
    #[must_use]
    pub fn with_tile(mut self, tile: TileHit) -> Self {
        self.tile = Some(tile);
        self
    }
}

/// A pair that touched in the step before stopped touching: sent at the end
/// of the step, after the begins, naming the pair's sides as its
/// [`ContactBegan`] did. A pair whose sides are both still (asleep, static,
/// a tile, or a kinematic body at rest) keeps touching through the steps
/// that do not solve it, unless one side moved, changed its shape or layers
/// or went: a despawned body ends its pairs, naming the dead entity, and an
/// emptied cell ends its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ContactEnded {
    /// The pair's `a`.
    pub a: Entity,
    /// Its `b`; `None` for a tile.
    pub b: Option<Entity>,
    /// The tile `b` stands for.
    pub tile: Option<TileHit>,
}

impl ContactEnded {
    /// An end with these sides and no tile, as the step sends one.
    #[must_use]
    pub fn new(a: Entity, b: Option<Entity>) -> Self {
        Self { a, b, tile: None }
    }

    /// The end with its tile side's cell `tile`.
    #[must_use]
    pub fn with_tile(mut self, tile: TileHit) -> Self {
        self.tile = Some(tile);
        self
    }
}

/// A sensor began to overlap a body (`other`) or a solid tile (`other:
/// None`, `tile` its cell) in a layer its mask holds
/// ([`RigidBody`](crate::physics::RigidBody)), sent at the end of the step in
/// which the overlap began: once per pair while it lasts, and again after
/// the two separate and overlap anew. A body already overlapping when the
/// sensor's mask comes to hold its layer is reported at that step. A swept
/// sensor ([`RigidBody::swept`](crate::physics::RigidBody::swept)) reports
/// what it met over the step. Build one with [`SensorEntered::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SensorEntered {
    /// The sensor.
    pub sensor: Entity,
    /// The body it began to overlap; `None` for a tile.
    pub other: Option<Entity>,
    /// The tile's cell.
    pub tile: Option<TileHit>,
    /// The other's centre at the step's end, or, for a swept sensor, where
    /// they first touched; a tile's, its cell's centre.
    pub point: Vec2,
}

impl SensorEntered {
    /// An event with these fields, no tile and its point at the origin, as
    /// the step sends one.
    #[must_use]
    pub fn new(sensor: Entity, other: Option<Entity>) -> Self {
        Self {
            sensor,
            other,
            tile: None,
            point: Vec2::ZERO,
        }
    }

    /// The event with its tile's cell `tile`.
    #[must_use]
    pub fn with_tile(mut self, tile: TileHit) -> Self {
        self.tile = Some(tile);
        self
    }

    /// The event with its point `point`.
    #[must_use]
    pub fn with_point(mut self, point: Vec2) -> Self {
        self.point = point;
        self
    }
}

/// A sensor stopped overlapping a body or a tile it reported entering: sent
/// at the end of the step whose sensor pass lacks the pair, after the
/// entries, whether the two separated, the sensor's mask dropped the layer,
/// the cell was written away or either was despawned (the entities may be
/// dead). A swept sensor's pairs are what it met over the step, so one it
/// passed exits at the next step unless met again. Build one with
/// [`SensorExited::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SensorExited {
    /// The sensor.
    pub sensor: Entity,
    /// The body; `None` for a tile.
    pub other: Option<Entity>,
    /// The tile's cell.
    pub tile: Option<TileHit>,
}

impl SensorExited {
    /// An event with these fields and no tile, as the step sends one.
    #[must_use]
    pub fn new(sensor: Entity, other: Option<Entity>) -> Self {
        Self {
            sensor,
            other,
            tile: None,
        }
    }

    /// The event with its tile's cell `tile`.
    #[must_use]
    pub fn with_tile(mut self, tile: TileHit) -> Self {
        self.tile = Some(tile);
        self
    }
}
