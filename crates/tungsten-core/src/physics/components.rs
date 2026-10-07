//! Physics components: AABB/circle, static/dynamic, no rotation.

use glam::Vec2;

use crate::ecs::bundle::{Bundle, BundleSink};

/// World-space position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position(pub Vec2);

impl Position {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

/// A body's `Position` before the last fixed step: the history render
/// interpolation draws from. [`physics_prev_snapshot`](crate::physics::physics_prev_snapshot)
/// copies `Position` here before each step, and
/// [`physics_sync`](crate::physics::physics_sync) draws the body at
/// `prev + (cur - prev) * alpha`; a body without it is drawn at its
/// `Position`.
///
/// Spawn it with the body, as [`RigidBodyBundle`] does: inserting it later
/// moves the entity to another archetype, which reorders the solve
/// (`D-066`). A teleport writes `Position` and `PrevPosition` together, so
/// the body is not drawn sliding from the old point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrevPosition(pub Vec2);

/// World-space velocity in pixels/second.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity(pub Vec2);

impl Velocity {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

/// Axis-aligned collider shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Axis-aligned box.
    Aabb { half_extents: Vec2 },
    /// Circle radius.
    Circle { radius: f32 },
}

/// Collider shape plus local offset from `Position`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Collider {
    pub shape: Shape,
    pub offset: Vec2,
}

impl Collider {
    #[must_use]
    pub fn aabb(half_extents: Vec2) -> Self {
        Self {
            shape: Shape::Aabb { half_extents },
            offset: Vec2::ZERO,
        }
    }

    #[must_use]
    pub fn circle(radius: f32) -> Self {
        Self {
            shape: Shape::Circle { radius },
            offset: Vec2::ZERO,
        }
    }

    #[must_use]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }
}

/// Static or integrated body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Static,
    Dynamic,
}

/// Body mass/restitution state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBody {
    pub kind: BodyKind,
    pub inv_mass: f32,
    pub restitution: f32,
}

impl RigidBody {
    /// Unit-mass dynamic body.
    #[must_use]
    pub fn dynamic() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            inv_mass: 1.0,
            restitution: 0.0,
        }
    }

    /// Immovable static body.
    #[must_use]
    pub fn r#static() -> Self {
        Self {
            kind: BodyKind::Static,
            inv_mass: 0.0,
            restitution: 0.0,
        }
    }

    #[must_use]
    pub fn with_mass(mut self, mass: f32) -> Self {
        self.inv_mass = if mass > 0.0 { 1.0 / mass } else { 0.0 };
        self
    }

    #[must_use]
    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution.clamp(0.0, 1.0);
        self
    }
}

/// One body's physics components, inserted in one archetype move:
/// `world.spawn_with(RigidBodyBundle::dynamic(position, collider))`, or
/// `.with((Player, transform))` for the entity's other components.
///
/// A dynamic body gets `Position`, [`PrevPosition`] at the same point,
/// `Velocity`, `Collider` and `RigidBody`; a static one the same without
/// `Velocity`. So every bundle body is drawn interpolated; a game that wants
/// a body without history spawns its components as a tuple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBodyBundle {
    /// Where the body is.
    pub position: Position,
    /// The body's velocity; `None` leaves the component off.
    pub velocity: Option<Velocity>,
    /// The body's shape.
    pub collider: Collider,
    /// Static or dynamic, with its mass and restitution.
    pub body: RigidBody,
}

impl RigidBodyBundle {
    /// A unit-mass dynamic body at rest.
    #[must_use]
    pub fn dynamic(position: Position, collider: Collider) -> Self {
        Self {
            position,
            velocity: Some(Velocity(Vec2::ZERO)),
            collider,
            body: RigidBody::dynamic(),
        }
    }

    /// An immovable static body, with no `Velocity`.
    #[must_use]
    pub fn r#static(position: Position, collider: Collider) -> Self {
        Self {
            position,
            velocity: None,
            collider,
            body: RigidBody::r#static(),
        }
    }

    /// The bundle with `velocity`; on a static body this adds the component.
    #[must_use]
    pub fn with_velocity(mut self, velocity: Velocity) -> Self {
        self.velocity = Some(velocity);
        self
    }

    /// The bundle with `body` in place of its constructor's.
    #[must_use]
    pub fn with_body(mut self, body: RigidBody) -> Self {
        self.body = body;
        self
    }
}

impl Bundle for RigidBodyBundle {
    /// `Position`, `PrevPosition` at the same point, `Velocity` when there
    /// is one, `Collider`, `RigidBody`.
    #[inline]
    fn put<S: BundleSink>(self, sink: &mut S) {
        sink.put(self.position);
        sink.put(PrevPosition(self.position.0));
        if let Some(velocity) = self.velocity {
            sink.put(velocity);
        }
        sink.put(self.collider);
        sink.put(self.body);
    }
}
