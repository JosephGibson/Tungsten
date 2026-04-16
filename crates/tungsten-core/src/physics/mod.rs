//! Hand-rolled 2D physics. Game-jam grade: AABB + circle collisions,
//! static + dynamic bodies, uniform-grid broad-phase, minimum-translation
//! resolution, per-step substeps to prevent tunneling. No joints, no
//! continuous collision detection, no rotation.
//!
//! Registration:
//!
//! ```ignore
//! use tungsten_core::physics::physics_step;
//! app.add_system(physics_step);
//! ```
//!
//! Components (`Position`, `Velocity`, `Collider`, `RigidBody`) live in
//! `components.rs`; the step function, broadphase grid, and narrow-phase
//! shape tests are in sibling modules. Collision contacts are delivered via
//! `EventQueue<CollisionEvent>` each step for gameplay systems to read.

pub mod broadphase;
pub mod collision;
pub mod components;
pub mod events;
pub mod step;

pub use broadphase::{ProxyId, SpatialGrid};
pub use collision::{aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, Aabb, Contact};
pub use components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
pub use events::CollisionEvent;
pub use step::physics_step;

use glam::Vec2;

/// Per-world physics tunables, stored as a resource and auto-inserted
/// by `App::new` with defaults. Games override any field before calling
/// `app.run()`.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsConfig {
    /// Uniform grid cell size in world-space pixels. One to two tile
    /// widths is typical. Larger cells trade per-cell bucket size for
    /// fewer cells per AABB; smaller cells trade the other way.
    pub broadphase_cell_size: f32,
    /// Upper bound on substeps per frame. Guards against pathological
    /// dt or velocity spikes monopolizing the frame budget.
    pub max_substeps: u32,
    /// World-space acceleration applied to every dynamic body each
    /// substep. Defaults to zero so top-down games cost nothing;
    /// a platformer sets `Vec2::new(0.0, 900.0)` or similar.
    pub gravity: Vec2,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            broadphase_cell_size: 32.0,
            max_substeps: 8,
            gravity: Vec2::ZERO,
        }
    }
}
