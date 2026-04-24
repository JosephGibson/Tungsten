//! Hand-rolled 2D physics: AABB/circle, uniform grid, MTV resolution, substeps.

pub mod broadphase;
pub mod collision;
pub mod components;
pub mod events;
pub mod step;

pub use broadphase::{ProxyId, SpatialGrid};
pub use collision::{
    aabb_vs_aabb, aabb_vs_aabb_masked, aabb_vs_circle, aabb_vs_circle_masked, circle_vs_circle,
    Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP,
};
pub use components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
pub use events::CollisionEvent;
pub use step::{physics_step, PhysicsBuffers};

use glam::Vec2;

/// Per-world physics tunables resource.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsConfig {
    /// Uniform grid cell size in world pixels.
    pub broadphase_cell_size: f32,
    /// Max integration substeps per frame.
    pub max_substeps: u32,
    /// Gauss-Seidel passes per substep.
    pub solver_iterations: u32,
    /// World-space acceleration per dynamic body.
    pub gravity: Vec2,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            broadphase_cell_size: 32.0,
            max_substeps: 8,
            solver_iterations: 4,
            gravity: Vec2::ZERO,
        }
    }
}
