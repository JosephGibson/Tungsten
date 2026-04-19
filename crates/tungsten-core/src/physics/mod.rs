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
pub use step::{physics_step, PhysicsBuffers};

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
    /// Upper bound on substeps per frame. Guards against tunneling from
    /// pathological dt or velocity spikes. Orthogonal to `solver_iterations`:
    /// substeps are about integration granularity, iterations about contact
    /// convergence.
    pub max_substeps: u32,
    /// Number of Gauss–Seidel constraint passes per substep. 1 is enough
    /// for isolated contacts; stacks of dynamic bodies need 3–8 so pressure
    /// from upper bodies doesn't squeeze the bottom through a static tile
    /// before its pair is revisited. Velocity impulses are self-limiting
    /// (separation gates further impulse on the same pair), so additional
    /// iterations mainly tighten position correction — they don't over-damp
    /// bounces.
    pub solver_iterations: u32,
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
            solver_iterations: 4,
            gravity: Vec2::ZERO,
        }
    }
}
