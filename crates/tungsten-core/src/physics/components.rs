//! Physics components. Game-jam grade: AABBs and circles, dynamic vs
//! static bodies, no rotation, no angular velocity. Tunables live on
//! `PhysicsConfig` in [`super::PhysicsConfig`].

use glam::Vec2;

/// World-space position of an entity. For AABB colliders this is the
/// top-left corner after applying `Collider::offset`; for circles it's
/// the center. Introduced as a library-level component in M11 so the
/// physics step has a canonical place to read and write motion state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position(pub Vec2);

impl Position {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

/// World-space velocity in pixels/second.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity(pub Vec2);

impl Velocity {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

/// Collider shape. All shapes are axis-aligned; there is no rotation
/// in M11 so a full SAT is unnecessary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Axis-aligned box. `half_extents` is half the width/height.
    Aabb { half_extents: Vec2 },
    /// Circle. `radius` is the full radius.
    Circle { radius: f32 },
}

impl Shape {
    /// Half of the smallest side of the bounding box of this shape,
    /// used by the physics step to decide substep count for tunneling
    /// guard. Returns 0 if the shape is degenerate.
    pub fn min_half_extent(&self) -> f32 {
        match *self {
            Shape::Aabb { half_extents } => half_extents.x.min(half_extents.y).max(0.0),
            Shape::Circle { radius } => radius.max(0.0),
        }
    }
}

/// Attached-to-entity collider. `offset` is added to `Position` at
/// test time so a visual sprite and its collider can sit at different
/// local offsets without a second `Position`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Collider {
    pub shape: Shape,
    pub offset: Vec2,
}

impl Collider {
    pub fn aabb(half_extents: Vec2) -> Self {
        Self {
            shape: Shape::Aabb { half_extents },
            offset: Vec2::ZERO,
        }
    }

    pub fn circle(radius: f32) -> Self {
        Self {
            shape: Shape::Circle { radius },
            offset: Vec2::ZERO,
        }
    }

    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }
}

/// Whether a body is immovable or integrated each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Static,
    Dynamic,
}

/// Physical body state. `inv_mass == 0.0` is immovable; `BodyKind::Static`
/// implies `inv_mass = 0.0`. `restitution` is clamped to `[0, 1]` at use
/// sites; `0.0` = stop at contact, `1.0` = perfect elastic bounce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidBody {
    pub kind: BodyKind,
    pub inv_mass: f32,
    pub restitution: f32,
}

impl RigidBody {
    /// Dynamic body with unit mass and no bounce.
    pub fn dynamic() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            inv_mass: 1.0,
            restitution: 0.0,
        }
    }

    /// Static body: infinite mass, never moves. Still participates in
    /// collision tests, but is never pushed.
    pub fn r#static() -> Self {
        Self {
            kind: BodyKind::Static,
            inv_mass: 0.0,
            restitution: 0.0,
        }
    }

    pub fn with_mass(mut self, mass: f32) -> Self {
        self.inv_mass = if mass > 0.0 { 1.0 / mass } else { 0.0 };
        self
    }

    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution.clamp(0.0, 1.0);
        self
    }
}
