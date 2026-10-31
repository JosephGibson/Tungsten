//! Hand-rolled 2D physics: AABB/circle, uniform grid, warm-started soft
//! contact solver, fixed substeps with speculative-contact CCD, island
//! sleeping. Bodies are static, dynamic or kinematic, or sensors
//! ([`BodyKind`]): a kinematic body moves by its velocity alone and pushes
//! dynamic bodies; a sensor moves the same way, collides with nothing and
//! reports what it begins to overlap ([`SensorEntered`]). Contacts send
//! [`CollisionEvent`]s with their closing speed and tile cell, and pairs
//! their begins and ends ([`ContactBegan`], [`ContactEnded`]). Ray and shape
//! casts, overlaps and nearest-surface queries read the last step's bodies,
//! sensors and tiles ([`cast_ray`], [`cast_shape`], [`overlap`],
//! [`nearest`]). A game names its physics layers and materials in its
//! manifests ([`PhysicsLayers`], [`PhysicsMaterials`]).

pub mod broadphase;
pub mod collision;
pub mod components;
pub mod events;
mod plugin;
mod query;
pub mod step;

pub use broadphase::{ProxyId, SpatialGrid};
pub use collision::{
    Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP, aabb_vs_aabb,
    aabb_vs_aabb_masked, aabb_vs_aabb_speculative, aabb_vs_circle, aabb_vs_circle_masked,
    aabb_vs_circle_speculative, circle_vs_circle, circle_vs_circle_speculative,
};
pub use components::{
    BodyKind, Collider, Position, PrevPosition, RigidBody, RigidBodyBundle, Shape, Spin, Velocity,
};
pub use events::{CollisionEvent, ContactBegan, ContactEnded, SensorEntered};
pub use plugin::{
    PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PHYSICS_SYNC, PhysicsPlugin, physics_prev_snapshot,
    physics_sync,
};
pub use query::{SpatialFilter, SpatialHit, TileHit, cast_ray, cast_shape, nearest, overlap};
pub use step::{PhysicsBuffers, TerrainFace, physics_step, teleport, wake};

use std::collections::BTreeMap;

use glam::Vec2;

/// The physics layers a game names in its manifests' `physics_layers`
/// (`D-178`): each name's layer, 0 to 31. The asset loader inserts it and a
/// manifest reload replaces it; `rigid_body` data names layers through it,
/// and game code reads the bits it gives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhysicsLayers {
    by_name: BTreeMap<String, u32>,
}

impl PhysicsLayers {
    /// Each name's layer, as `layers` gives them; a name given twice keeps
    /// its last layer.
    ///
    /// # Panics
    ///
    /// When a layer is 32 or more.
    #[must_use]
    pub fn new(layers: impl IntoIterator<Item = (String, u32)>) -> Self {
        let by_name: BTreeMap<String, u32> = layers.into_iter().collect();
        for (name, &layer) in &by_name {
            assert!(
                layer < u32::BITS,
                "physics layer `{name}` must be 0 to 31 (got {layer})"
            );
        }
        Self { by_name }
    }

    /// The layer `name` names, 0 to 31; `None` for a name it lacks.
    #[must_use]
    pub fn layer(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// The bit of the layer `name` names, `1 << layer`, as
    /// [`RigidBody::layers`] and [`RigidBody::mask`] hold it; `None` for a
    /// name it lacks.
    #[must_use]
    pub fn bit(&self, name: &str) -> Option<u32> {
        self.layer(name).map(|layer| 1 << layer)
    }

    /// The bits of every layer `names` names; the first name it lacks is
    /// the error.
    pub fn bits<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Result<u32, UnknownLayer> {
        names.into_iter().try_fold(0, |bits, name| {
            self.bit(name)
                .map(|bit| bits | bit)
                .ok_or_else(|| UnknownLayer(name.to_owned()))
        })
    }

    /// The names and their layers, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, u32)> {
        self.by_name
            .iter()
            .map(|(name, &layer)| (name.as_str(), layer))
    }
}

/// A physics layer name that [`PhysicsLayers`] lacks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown physics layer `{0}`")]
pub struct UnknownLayer(pub String);

/// A physics material's contact coefficients, as a manifest's
/// `physics_materials` entry gives them: what a body or a tile of it
/// contacts with.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicsMaterial {
    /// Coulomb friction, finite and at least 0; a contact's is the
    /// geometric mean of its two sides'.
    pub friction: f32,
    /// Restitution, 0 to 1; a contact's is the larger of its two sides'.
    pub restitution: f32,
    /// Rolling resistance, finite and at least 0; a contact's is the larger
    /// of its two sides'.
    pub rolling: f32,
}

/// The physics materials a game names in its manifests'
/// `physics_materials`, by name. The asset loader inserts it and a manifest
/// reload replaces its materials, moving its [`version`](Self::version) on,
/// which the tile cache reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PhysicsMaterials {
    by_name: BTreeMap<String, PhysicsMaterial>,
    version: u64,
}

impl PhysicsMaterials {
    /// The materials `materials` names, at version 0.
    #[must_use]
    pub fn new(materials: impl IntoIterator<Item = (String, PhysicsMaterial)>) -> Self {
        Self {
            by_name: materials.into_iter().collect(),
            version: 0,
        }
    }

    /// The material `name` names; `None` for a name it lacks.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<PhysicsMaterial> {
        self.by_name.get(name).copied()
    }

    /// Replaces every material with `materials`' and moves the version on.
    pub fn replace(&mut self, materials: impl IntoIterator<Item = (String, PhysicsMaterial)>) {
        self.by_name = materials.into_iter().collect();
        self.version += 1;
    }

    /// Bumped by each [`replace`](Self::replace); 0 when made.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The names and their materials, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, PhysicsMaterial)> {
        self.by_name
            .iter()
            .map(|(name, &material)| (name.as_str(), material))
    }
}

/// Per-world physics tunables resource.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsConfig {
    /// Uniform grid cell size in world pixels.
    pub broadphase_cell_size: f32,
    /// Fixed solver substeps per frame (D-064). Speculative contacts are the
    /// primary CCD, so no per-speed substep amplification exists; more
    /// substeps buy solver stiffness (the contact-hertz cap scales with the
    /// substep rate), not tunneling safety.
    ///
    /// A step whose world holds a kinematic body with a collider, moving or
    /// still, runs `clamp(⌈substeps · dt · 60 − 0.001⌉, substeps,
    /// 2 · substeps)` substeps, `dt` being the step after `max_step_dt`:
    /// `substeps` at 1/60 s and shorter, and with the default 4, six at
    /// 1/45 s and eight at 1/30 s. No substep is then longer than a 1/60 s
    /// step's up to the default bound, so contacts against the kinematic
    /// body keep their 1/60 s stiffness. Every other world runs `substeps`.
    pub substeps: u32,
    /// Longest simulated time, in seconds, one `physics_step` call advances
    /// (D-094); a longer frame dt is clamped to it and the rest of the frame
    /// is not simulated. Protects the contact-hertz cap: at the default
    /// 1/30 s over 4 substeps the substep never exceeds 1/120 s, so
    /// `contact_hertz` keeps its 30 Hz however slow the frame; a world with a
    /// kinematic body runs that step in up to twice `substeps` (see
    /// `substeps`), which a larger bound or none cannot raise further.
    /// `<= 0` means unbounded.
    pub max_step_dt: f32,
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
    /// the island sleeps (D-065). At 0 or less an island sleeps at the end of
    /// the first frame in which every member is below it.
    pub time_to_sleep: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            broadphase_cell_size: 32.0,
            substeps: 4,
            max_step_dt: 1.0 / 30.0,
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
