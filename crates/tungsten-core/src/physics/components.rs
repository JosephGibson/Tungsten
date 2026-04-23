//! Physics components: AABB/circle, static/dynamic, no rotation.

use glam::Vec2;

/// World-space position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position(pub Vec2);

impl Position {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

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

impl Shape {
    /// Smallest bounding half-extent for substep tunneling guard.
    #[must_use]
    pub fn min_half_extent(&self) -> f32 {
        match *self {
            Shape::Aabb { half_extents } => half_extents.x.min(half_extents.y).max(0.0),
            Shape::Circle { radius } => radius.max(0.0),
        }
    }
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
