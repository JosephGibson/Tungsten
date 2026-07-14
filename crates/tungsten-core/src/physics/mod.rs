//! Hand-rolled 2D physics: AABB/circle, uniform grid, warm-started soft
//! contact solver, fixed substeps with speculative-contact CCD, island
//! sleeping.

pub mod broadphase;
pub mod collision;
pub mod components;
pub mod events;
pub mod step;

pub use broadphase::{ProxyId, SpatialGrid};
pub use collision::{
    aabb_vs_aabb, aabb_vs_aabb_masked, aabb_vs_aabb_speculative, aabb_vs_circle,
    aabb_vs_circle_masked, aabb_vs_circle_speculative, circle_vs_circle,
    circle_vs_circle_speculative, Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT,
    FACE_TOP,
};
pub use components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
pub use events::CollisionEvent;
pub use step::{physics_step, wake, PhysicsBuffers};

use glam::Vec2;

/// Per-world physics tunables resource.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsConfig {
    /// Uniform grid cell size in world pixels.
    pub broadphase_cell_size: f32,
    /// Fixed solver substeps per frame (D-064). Speculative contacts are the
    /// primary CCD, so no per-speed substep amplification exists; more
    /// substeps buy solver stiffness (the contact-hertz cap scales with the
    /// substep rate), not tunneling safety.
    pub substeps: u32,
    /// Biased velocity iterations per substep; one bias-free relax iteration
    /// always follows (D-063). Substeps-over-iterations (D-064): the default
    /// is 1 iteration x 4 substeps, the Box2D-v3 "TGS Soft" shape.
    pub solver_iterations: u32,
    /// World-space acceleration per dynamic body.
    pub gravity: Vec2,
    /// Soft-constraint contact stiffness in Hz, capped at a quarter of the
    /// substep rate for stability; contacts against statics run at twice this.
    pub contact_hertz: f32,
    /// Soft-constraint damping ratio (non-dimensional).
    pub contact_damping_ratio: f32,
    /// Allowed rest penetration in pixels; keeps contacts persistent for warm
    /// starting without visible sink.
    pub linear_slop: f32,
    /// Cap on penetration-recovery speed in px/s; bounds the energy a deep
    /// contact can inject.
    pub max_push_speed: f32,
    /// Contacts approaching slower than this (px/s) rebound inelastically.
    pub restitution_threshold: f32,
    /// Island sleeping (D-065): bodies slower than this (px/s) accumulate
    /// sleep time, and an island sleeps once every member stays below it for
    /// `time_to_sleep`. Doubles as the wake tolerance for contacts against
    /// sleeping bodies. `<= 0` disables sleeping entirely.
    pub sleep_threshold: f32,
    /// Seconds every island member must stay below `sleep_threshold` before
    /// the island sleeps (D-065).
    pub time_to_sleep: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            broadphase_cell_size: 32.0,
            substeps: 4,
            solver_iterations: 1,
            gravity: Vec2::ZERO,
            contact_hertz: 30.0,
            contact_damping_ratio: 10.0,
            linear_slop: 0.25,
            max_push_speed: 120.0,
            restitution_threshold: 30.0,
            sleep_threshold: 20.0,
            time_to_sleep: 0.5,
        }
    }
}
