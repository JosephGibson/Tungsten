//! Physics step: build broadphase -> fixed substeps (speculative contacts ->
//! soft solve -> integrate -> arrival pass) -> events.
//!
//! The substep count is fixed via `PhysicsConfig::substeps` (D-064); no
//! velocity-derived amplification exists. A world with a kinematic proxy,
//! moving or still, runs a step longer than 1/60 s in more substeps, up to
//! twice the count, so that no substep is longer than a 1/60 s step's up to
//! the default step bound: a crowd pressing on a kinematic body meets the
//! contact stiffness it has at 1/60 s. Tunneling safety comes from
//! speculative contacts: the narrow phase admits broadphase pairs with a
//! positive gap up to a per-pair margin (relative travel per substep plus
//! slack) as signed-distance contacts, and the solver's separated branch
//! removes exactly the excess approach velocity so fast bodies arrive at
//! touching instead of passing through — for every pair class (static/dynamic
//! AABBs and circles, dynamic-vs-dynamic). The slab sweep survives only as a
//! safety net for solver-injected velocity spikes vs statics (a pair whose
//! velocity changed after admission), conservatively promoting static circles
//! to their bounding squares. Once a frame's sweep queries outnumber the
//! statics, a statics-only grid answers them (D-080).
//!
//! Velocity the solver injects is checked before the bodies move (D-092): a
//! body whose velocity the solve changed by more than the admission slack is
//! clamped against every neighbour that velocity reaches, static or dynamic,
//! pushers first and walls last, so a body pushed at a wall arrives at it and
//! its pusher stops behind it, whatever order the contacts were solved in.
//!
//! Broadphase pairs persist across substeps under per-proxy travel budgets
//! (D-075). Each build stages fresh, symmetrically inflated AABBs for the
//! remaining frame. A proxy that exhausts its budget has its own pairs
//! rebuilt before the next narrow phase (D-081); a contact wake, or too many
//! exhausted budgets at once, rebuilds the whole list. Narrow phase, solve,
//! islands and events stay per substep.
//!
//! Body state is gathered into the dense proxy array **once per frame**
//! through a columnar 4-way ECS query (`query2_opt2`, no per-entity random
//! lookups) and written back once per frame after the last substep (D-066).
//! Substeps run entirely on the dense arrays — zero `World` reads or writes
//! inside the substep loop; collision events accumulate across substeps and
//! drain to the `EventQueue` once per frame in substep order. Proxy order is
//! the gather query's archetype/row order and is frame-stable, so grid ids,
//! sleep flags, and the writeback zip stay valid frame-wide.
//!
//! Contact resolution is a warm-started soft solver (D-063, Box2D-v3 shape).
//! Per substep: the narrow phase runs once into a contact buffer, accumulated
//! normal impulses (clamped >= 0) carry by pair index between substeps and by
//! pair key across a repair, with a keyed map at frame boundaries and pair
//! rebuilds (D-076). Biased iterations recover
//! penetration through a soft constraint (contact hertz / damping ratio,
//! linear slop, max push speed) instead of positional MTV projection,
//! position integration turns the bias velocity into depenetration, one
//! bias-free relax iteration then strips the injected bias energy, and
//! restitution replays the approach velocity stored at contact build above an
//! inelastic threshold. Collision events are emitted from the pre-solve
//! narrow-phase results — the same contact set the old solver saw on
//! iteration 0 — and only for contacts with `penetration > 0`: speculative
//! positive-gap contacts stay silent until actual touch (D-064).
//!
//! Island sleeping (D-065): a serial deterministic union-find over each
//! frame's touching contacts builds islands of awake dynamic bodies; an
//! island sleeps once every member stays below `sleep_threshold` for
//! `time_to_sleep`. Sleeping bodies stay in the broadphase but never initiate
//! pairs (sleeping-sleeping pairs are skipped entirely), skip gravity,
//! integration, and writeback, and enter awake-vs-sleeping contacts as
//! immovable (effective inverse mass 0). Wake paths: a contact from an awake
//! body (touching with relative motion, or a speculative gap approached
//! faster than the threshold — a bullet wakes its impact fringe before touch
//! resolves), an external `Position`/`Velocity` write (sleeping state is
//! frozen, so a bit-mismatch at gather is an external write), body despawn
//! (the map entry fails to carry over and its island wakes), and the `wake`
//! API. Because sleeping-sleeping pairs skip the narrow phase, sleeping
//! islands emit no `CollisionEvent`s; waking resumes emission.
//!
//! Kinematic bodies are proxies that move by their velocity each substep,
//! with no gravity, and that no contact moves: inverse mass 0, the static
//! softness, no sleep entry and no island. They initiate pairs as awake
//! bodies do, inflated by their travel; the narrow phase drops their pairs
//! with static, kinematic and tile proxies as it drops static pairs, and
//! their events name the dynamic body as `a`. A moving one wakes a sleeper it
//! touches or closes on at any speed, since a slower pair would be dropped
//! with both sides immovable, and a body touching it keeps its island awake
//! through the frame-end pass. The wake reaches through that island, as this
//! frame's unions have built it so far: a sleeper one of its bodies touches
//! or closes on wakes at any speed and joins it, so a pressed row wakes body
//! by body instead of holding the pressed bodies against the kinematic body.
//! The safety net and its statics-only grid leave them out; the arrival pass
//! holds them immovable where the substep began.
//!
//! Sensors are gathered beside the proxies, never as one, so no substep,
//! pair, solve or sweep sees them. The writeback moves each by its velocity;
//! then the step builds the spatial index and tests each sensor, in gather
//! order, against the bodies it overlaps at their final positions and the
//! solid cells of the tile cache's grids, in a layer its mask holds. A pair
//! the previous step lacked sends `SensorEntered`, after the frame's
//! collision events. A pair's key holds the sensor's key in its high half
//! and the other's identity in its low half (a tile's names its map, layer
//! and cell), and a step without sensors forgets every pair. A sensor whose
//! identity, box and mask are the last pass's keeps its tile pairs and tests
//! only the cells the step's tile refresh changed.
//!
//! The contact record (`ContactBegan`, `ContactEnded`) runs while either
//! queue exists. A pair touches in a substep when its contact penetrates or
//! the solve gave it a normal impulse, or an arrival or safety-net clamp
//! stopped one side against the other; a pair is its two sides, a tile by
//! its map, layer and cell (the cache's identity), never the solver's pair
//! key. Each contact carries the largest closing speed its pair had since
//! its last touch, sampled at its build and after the solve and the arrival
//! pass, by solver key between substeps and steps; a touch spends it. At
//! the step's end, before the sleep pass, a pair touched for the first time
//! begins and one not touched ends, unless both its sides were immovable
//! through the step (asleep, static, a tile or a still kinematic body) and
//! neither changed, or a changed kinematic side still overlaps it.
//!
//! Layers and masks: the gather keeps each proxy's layers and mask in an
//! array beside the proxies, as `pair_flags` is (D-080), a tile's those of
//! its collision layer, and notes whether any proxy or sensor is off the
//! defaults (layer 0, every layer met). Only
//! then do the pair build and repair, the safety net and the arrival pass
//! test that the two sides meet, so a world without layers runs the same
//! loops. A pair that does not meet is never admitted, so it makes no
//! contact, event, island, wake or clamp. A sleeper whose layers or mask
//! changed wakes its island, and a static or tile support whose layers or
//! mask changed wakes the sleepers near it.
//!
//! The whole step is serial and deterministic by design (D-067): a
//! color-parallel contact solver was built, measured and dropped — at 25k
//! awake bodies the contact-solve passes are ~2% of the frame (the cost is
//! pair query + narrow phase + contact build), so threading the solver cannot
//! move the number and the coloring/sort machinery alone cost the serial path
//! ~18%.

use super::PhysicsConfig;
use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::{
    Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_ONE_WAY, FACE_RIGHT, FACE_TOP,
};
use super::components::{
    BodyKind, Collider, Position, PrevPosition, RigidBody, Shape, Spin, Velocity,
};
use super::events::{CollisionEvent, ContactBegan, ContactEnded, SensorEntered};
use super::query::{IndexEntry, SpatialIndex, TileHit};
use crate::components::Transform;
use crate::ecs::{Entity, EventQueue, Without, World};
use crate::time::Time;
use glam::Vec2;

mod arrival;
mod gather;
mod outline;
mod pairs;
mod sleep;
mod solver;
mod sweep;

use arrival::{ARRIVAL_SLOPS, ArrivalScratch, arrival_pass};
pub(super) use gather::{GridCell, TileRun};
use gather::{TileCache, entity_key, gather_proxies};
use pairs::{ImpulseMap, PairBudget, PairGrid, pair_key, refresh_pairs, sync_impulses};
use sleep::{SleepTable, island_holds_any, sleep_frame_end, sleep_frame_start, uf_union};
use solver::{
    apply_impulse, apply_restitution, apply_tangent, narrow_phase, one_way_holds, soft_params,
    solve_contacts, solve_friction, solve_rolling,
};
use sweep::{SweepGrid, speculative_pass};

/// Per-substep collider proxy; tile proxies use `entity = None`.
#[derive(Debug, Clone, Copy)]
// Independent per-proxy flags, packed into the 80-byte proxy's padding.
#[allow(clippy::struct_excessive_bools)]
struct Proxy {
    entity: Option<Entity>,
    /// Frame-stable warm-start identity (entity id or tile-position hash).
    key: u64,
    center: Vec2,
    /// Pre-integration center within the current substep, for the
    /// speculative sweep.
    prev_center: Vec2,
    velocity: Vec2,
    offset: Vec2,
    shape: Shape,
    is_dynamic: bool,
    /// Entity carries a `Velocity` component; gates gravity + writeback.
    has_velocity: bool,
    inv_mass: f32,
    restitution: f32,
    /// Tile exposed-face mask; internal faces suppress seam contacts.
    face_mask: u8,
    /// Body is in a sleeping island (D-065); set at frame start from the
    /// persistent sleep map, may flip awake mid-substep when a contact wakes
    /// the body. Proxies persist across substeps (D-066), so this carries
    /// through the frame.
    sleeping: bool,
    /// Kinematic body: moves by its velocity, which nothing changes within
    /// the frame, and counts as immovable (`is_dynamic` is false).
    kinematic: bool,
    /// A dynamic body that touched a moving kinematic body this frame; the
    /// frame-end sleep pass keeps its island awake.
    kept_awake: bool,
}

impl Proxy {
    /// The proxy moves within a substep: an awake dynamic body or a
    /// kinematic one. Only these initiate pairs and spend travel budgets.
    fn moves(&self) -> bool {
        (self.is_dynamic && !self.sleeping) || self.kinematic
    }

    fn world_aabb(&self) -> Aabb {
        Aabb::new(self.center, self.half_extents())
    }

    fn half_extents(&self) -> Vec2 {
        match self.shape {
            Shape::Aabb { half_extents } => half_extents,
            Shape::Circle { radius } => Vec2::splat(radius),
        }
    }

    fn min_half_extent(&self) -> f32 {
        match self.shape {
            Shape::Aabb { half_extents } => half_extents.x.min(half_extents.y),
            Shape::Circle { radius } => radius,
        }
    }

    /// Union of pre/post AABBs for the post-integration speculative sweep.
    fn swept_aabb(&self) -> Aabb {
        let cur = self.world_aabb();
        if self.prev_center == self.center {
            return cur;
        }
        let prev = Aabb::new(self.prev_center, self.half_extents());
        cur.union(&prev)
    }
}

/// A body's collision layers and mask, as its `RigidBody` holds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layers {
    /// The layers it is in.
    layers: u32,
    /// The layers it meets.
    mask: u32,
}

impl Layers {
    /// Layer 0, meeting every layer: every constructor's body, a collider
    /// without a `RigidBody`, and a tile of a collision layer that names no
    /// physics layers.
    const DEFAULT: Self = Self {
        layers: RigidBody::DEFAULT_LAYERS,
        mask: RigidBody::ALL_LAYERS,
    };

    fn of(body: &RigidBody) -> Self {
        Self {
            layers: body.layers,
            mask: body.mask,
        }
    }

    /// The contact rule ([`RigidBody::meets`]).
    fn meets(self, other: Self) -> bool {
        self.layers & other.mask != 0 && other.layers & self.mask != 0
    }
}

impl Default for Layers {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Each proxy's layers, in proxy order, read instead of the 80-byte `Proxy`
/// as `pair_flags` is (D-080).
#[derive(Debug, Default)]
struct ProxyLayers {
    of: Vec<Layers>,
    /// Some proxy or sensor is off [`Layers::DEFAULT`]; while it is false,
    /// every pair meets and no pass tests it.
    layered: bool,
}

impl ProxyLayers {
    /// Proxies `a` and `b` may touch: always while nothing is layered.
    #[inline]
    fn meet(&self, a: usize, b: usize) -> bool {
        !self.layered || self.of[a].meets(self.of[b])
    }
}

/// A sensor for the frame: never a proxy, so no substep sees it. The
/// writeback moves it and the sensor pass tests it.
#[derive(Debug, Clone, Copy)]
struct SensorBody {
    entity: Entity,
    /// Its entity key: the high half of its overlap keys.
    key: u64,
    /// Collider centre; the writeback moves it to the step's end.
    center: Vec2,
    shape: Shape,
    velocity: Vec2,
    /// Its mask picks what it reports; its layers matter to queries alone.
    layers: Layers,
}

/// One narrow-phase contact prepared for the soft solver.
#[derive(Debug, Clone, Copy)]
struct ContactConstraint {
    a: u32,
    b: u32,
    normal: Vec2,
    /// Signed distance at contact build; negative = speculative gap (D-064).
    penetration: f32,
    /// Inverse of the summed inverse masses along the normal.
    normal_mass: f32,
    /// Relative normal velocity at contact build (negative = approaching);
    /// restitution replays this instead of the post-solve velocity.
    approach: f32,
    /// Accumulated normal impulse, warm-started and clamped >= 0.
    impulse: f32,
    restitution: f32,
    /// One side is immovable; solved with the stiffer static softness.
    vs_static: bool,
    /// Effective inverse masses captured at contact build; a sleeping side is
    /// frozen at 0 so the solver treats it as static for this substep (D-065).
    inv_a: f32,
    inv_b: f32,
    key: u128,
    /// Index in the current broadphase pair list (D-076).
    pair_index: usize,
}

/// Physics scratch buffers reused across substeps/frames.
#[derive(Debug, Default)]
// Test-only oracle flags are independent switches, not solver states.
#[cfg_attr(test, allow(clippy::struct_excessive_bools))]
pub struct PhysicsBuffers {
    proxies: Vec<Proxy>,
    /// The entity proxies at the front of `proxies`; the tile proxies, from
    /// `tiles`, follow them.
    bodies: usize,
    /// The collision layers' tile proxies, kept until a map changes.
    tiles: TileCache,
    /// Inflated AABBs and travel allowances for D-075 pair reuse, each set
    /// at the proxy's last pair build or repair.
    pair_aabbs: Vec<Aabb>,
    pair_budgets: Vec<PairBudget>,
    /// `PAIR_*` bits per proxy. Pair queries read this byte instead of the
    /// 80-byte `Proxy` (D-080).
    pair_flags: Vec<u8>,
    /// Each proxy's layers and mask, set by the gather.
    layers: ProxyLayers,
    /// Moving proxies whose travel budget ran out, in proxy order (D-081);
    /// refilled before every substep.
    tripped: Vec<u32>,
    /// Proxies repaired since the last pair build, in repair order.
    repaired: Vec<u32>,
    /// `repaired` staged with their current inflated AABBs.
    repair_grid: SpatialGrid,
    /// Nonzero carried impulses of the pairs a repair removes, by pair key.
    repair_impulses: ImpulseMap,
    /// Scratch for filling `repair_impulses`.
    removed_impulses: Vec<(u128, f32)>,
    /// Set by gather (new proxy set) or a contact wake; consumed before the
    /// next substep's narrow phase. Proxies never change mid-frame today.
    pairs_invalidated: bool,
    #[cfg(test)]
    pair_builds: usize,
    #[cfg(test)]
    pair_repairs: usize,
    #[cfg(test)]
    check_pair_contacts: bool,
    #[cfg(test)]
    checked_substeps: usize,
    /// Substeps run since the buffers were made.
    #[cfg(test)]
    substeps_run: usize,
    /// D-063 reference model, rebuilt every substep in regression tests only.
    #[cfg(test)]
    reference_impulses: Option<ImpulseMap>,
    pairs: Vec<(u32, u32)>,
    events: Vec<CollisionEvent>,
    /// Sensors gathered this frame, in gather order.
    sensors: Vec<SensorBody>,
    /// Each tile proxy's identity, in proxy order after the entity proxies;
    /// filled only in a frame with sensors.
    tile_ids: Vec<u64>,
    /// The last step's sensor pairs, sorted: the sensor's key in the high
    /// half, the other's identity in the low half.
    sensor_pairs: Vec<u128>,
    /// Scratch for this step's sensor pairs.
    sensor_scratch: Vec<u128>,
    /// Scratch: this step's sensor pairs with bodies, then all of them
    /// merged into `sensor_scratch`.
    sensor_body_pairs: Vec<u128>,
    /// Each sensor's tile pairs, by gather index, kept while the sensor and
    /// the tiles stay as they were; cleared by a step without sensors.
    sensor_tiles: Vec<SensorTiles>,
    /// Sensor passes run, which dates each `sensor_tiles` entry.
    sensor_passes: u64,
    /// The tile cells the last sensor pass tested.
    #[cfg(test)]
    sensor_cells: usize,
    sensor_events: Vec<SensorEntered>,
    candidates: Vec<ProxyId>,
    /// The pair grid: entity proxies staged per build, tiles per cache.
    grid: PairGrid,
    static_grid: SweepGrid,
    contacts: Vec<ContactConstraint>,
    /// Last synchronized contacts, keyed across frames and pair rebuilds.
    impulses: ImpulseMap,
    /// Direct warm starts while D-075 pair indices remain stable.
    pair_impulses: Vec<f32>,
    /// First substep after a build seeds contacts from the keyed map.
    seed_pair_impulses: bool,
    /// Contacts have changed since the last keyed-map synchronization.
    impulses_dirty: bool,
    /// Persistent per-body sleep state (D-065), parallel to `proxies` after
    /// `sleep_frame_start`.
    sleep: SleepTable,
    /// Union-find parent per proxy index; unions accumulate across substeps.
    island_parent: Vec<u32>,
    /// Proxies touching a moving kinematic body this frame
    /// (`Proxy::kept_awake`), in the order they first touched it.
    kept: Vec<u32>,
    /// Scratch: minimum member sleep timer per island root.
    island_min_timer: Vec<f32>,
    /// Scratch: adopted sleeping-island tag per island root (0 = fresh).
    island_tag: Vec<u64>,
    /// Scratch: island tags scheduled to wake this frame.
    wake_islands: Vec<u64>,
    arrival: ArrivalScratch,
    /// The step's state beyond the proxies' own.
    extra: Box<StepExtra>,
}

/// The step's state that no fixed field of [`PhysicsBuffers`] holds, boxed:
/// the step moves the buffers out of the World and back each step, so they
/// grow by this pointer alone. Its vectors are cleared and refilled, so a
/// steady world allocates nothing new a step.
#[derive(Debug, Default)]
struct StepExtra {
    /// Each entity proxy's gravity scale, in proxy order; empty while every
    /// one is 1.
    gravity_scales: Vec<f32>,
    /// Each proxy's friction, the entity proxies' and then the tiles', in
    /// proxy order; empty while every one is 0, which leaves friction out
    /// of the frame.
    frictions: Vec<f32>,
    /// The substep's friction constraints, one for each contact whose
    /// friction is above 0, in contact order.
    friction: Vec<FrictionConstraint>,
    /// Tangent impulses carried by pair index between substeps, as
    /// `pair_impulses` carries normal ones; kept only in a frame with
    /// friction.
    pair_tangents: Vec<f32>,
    /// Nonzero tangent impulses by pair key, across frames and pair builds.
    tangents: ImpulseMap,
    /// Nonzero carried tangents of the pairs a repair removes, by pair key.
    repair_tangents: ImpulseMap,
    /// Scratch for filling `repair_tangents`.
    removed_tangents: Vec<(u128, f32)>,
    /// Each proxy's slot in `spins`, in proxy order, [`NO_SPIN`] for one
    /// that does not spin; empty while nothing spins.
    spin_of: Vec<u32>,
    /// The spinning dynamic circles' turns, in proxy order.
    spins: Vec<SpinState>,
    /// Each proxy's rolling resistance, the tiles' included, in proxy
    /// order; empty while nothing spins or every one is 0.
    rollings: Vec<f32>,
    /// The substep's rolling resistance constraints, one for each contact
    /// with a spinning side whose rolling resistance is above 0.
    rolling: Vec<RollingConstraint>,
    /// The step keeps the contact record: a `ContactBegan` or `ContactEnded`
    /// queue exists.
    record_on: bool,
    /// The touching pairs and what tells their begins and ends.
    record: ContactRecord,
    /// The substep's contacts' carried closing speeds, parallel to the
    /// contacts; filled only while the record runs.
    contact_closing: Vec<f32>,
    /// The arrival pass's and the safety net's touches in the substep.
    touches: Vec<TouchObs>,
    /// The step's begins and ends, sent after its collision events.
    began: Vec<ContactBegan>,
    ended: Vec<ContactEnded>,
}

/// [`StepExtra::spin_of`]'s slot for a proxy that does not spin.
const NO_SPIN: u32 = u32::MAX;

/// A spinning dynamic circle's turn for the step ([`Spin`]).
#[derive(Debug, Clone, Copy)]
struct SpinState {
    /// Its proxy and entity.
    proxy: u32,
    entity: Entity,
    angle: f32,
    angular_velocity: f32,
    radius: f32,
}

/// One contact's rolling resistance (`D-063`'s solver, a row on the spin
/// alone): an angular impulse opposing the spinning side's spin relative to
/// the other side's, accumulated within `±bound · λn`.
#[derive(Debug, Clone, Copy)]
struct RollingConstraint {
    /// Its contact's index in the substep's contacts.
    contact: u32,
    /// The two sides' slots in the spins, [`NO_SPIN`] for one that does
    /// not spin.
    spin_a: u32,
    spin_b: u32,
    /// The larger rolling resistance of the two sides times the spinning
    /// radius.
    bound: f32,
    /// Inverse of the summed inverse moments of inertia.
    mass: f32,
    /// Accumulated angular impulse.
    impulse: f32,
}
/// One contact's Coulomb friction (`D-063`'s solver, a tangent row beside
/// the normal one): solved after the normal pass of each iteration, its
/// accumulated impulse clamped to `±mu · λn`. A spinning side's rim speed
/// enters its tangent velocity, and the impulse turns it.
#[derive(Debug, Clone, Copy)]
struct FrictionConstraint {
    /// Its contact's index in the substep's contacts.
    contact: u32,
    /// The two sides' slots in the spins, [`NO_SPIN`] for one that does
    /// not spin.
    spin_a: u32,
    spin_b: u32,
    /// The geometric mean of the two sides' friction, above 0.
    mu: f32,
    /// Inverse of the summed inverse masses along the tangent.
    tangent_mass: f32,
    /// Accumulated impulse along the tangent `(−n.y, n.x)`, warm-started.
    impulse: f32,
}

impl PhysicsBuffers {
    /// Wake `entity`'s sleep island (D-065). The single public sleep surface:
    /// call after writing `Velocity`/`Position` from game code, though the
    /// step also detects such writes on sleeping bodies by itself. Waking an
    /// awake body just resets its sleep timer; unknown entities are a no-op.
    pub fn wake(&mut self, entity: Entity) {
        let Some(index) = self.sleep.index_of(entity_key(entity)) else {
            return;
        };
        let entry = &mut self.sleep.entries[index];
        if entry.sleeping {
            let island = entry.island;
            self.sleep.wake_islands(&[island]);
        } else {
            entry.timer = 0.0;
        }
    }

    /// True when `entity` is currently in a sleeping island.
    #[must_use]
    pub fn is_sleeping(&self, entity: Entity) -> bool {
        self.sleep
            .get(entity_key(entity))
            .is_some_and(|entry| entry.sleeping)
    }

    /// Whether the last step turned a spinner: the snapshot and the sync
    /// read [`Spin`]s only then.
    pub(crate) fn has_spins(&self) -> bool {
        !self.extra.spins.is_empty()
    }

    /// Number of sleeping bodies (diagnostic).
    #[must_use]
    pub fn sleeping_count(&self) -> usize {
        self.sleep.sleeping_count()
    }

    /// Proxies gathered by the last step: entities plus tile colliders
    /// (diagnostic).
    #[must_use]
    pub fn proxy_count(&self) -> usize {
        self.proxies.len()
    }

    /// The exposed faces of the tile outline's runs as world segments, at
    /// the last step's map positions, run by run in tile order, each run's
    /// top, bottom, left and right (diagnostic): what the physics overlay
    /// draws over a tile map.
    pub fn terrain_faces(&self) -> impl Iterator<Item = TerrainFace> + '_ {
        self.tiles.runs().iter().flat_map(|run| {
            let (min, max) = (run.center - run.half, run.center + run.half);
            let one_way = run.face_mask & FACE_ONE_WAY != 0;
            [
                (FACE_TOP, min, Vec2::new(max.x, min.y), Vec2::NEG_Y),
                (FACE_BOTTOM, Vec2::new(min.x, max.y), max, Vec2::Y),
                (FACE_LEFT, min, Vec2::new(min.x, max.y), Vec2::NEG_X),
                (FACE_RIGHT, Vec2::new(max.x, min.y), max, Vec2::X),
            ]
            .into_iter()
            .filter(move |&(face, ..)| run.face_mask & face != 0)
            .map(move |(_, start, end, normal)| TerrainFace {
                start,
                end,
                normal,
                one_way,
            })
        })
    }

    /// Chunks of tile outline traced since the buffers were made, a 16 × 16
    /// block of a collision layer's cells each (diagnostic): a map's every
    /// chunk when it appears, moves or is replaced, and for a written cell
    /// the chunks holding it and its four neighbours.
    #[must_use]
    pub fn terrain_traces(&self) -> u64 {
        self.tiles.traced()
    }

    /// Dynamic entity bodies gathered by the last step, the population
    /// `sleeping_count` is drawn from (diagnostic).
    #[must_use]
    pub fn dynamic_count(&self) -> usize {
        self.proxies
            .iter()
            .filter(|proxy| proxy.is_dynamic && proxy.entity.is_some())
            .count()
    }

    /// Broadphase pairs of the last step's final substep (diagnostic).
    #[must_use]
    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }

    /// Contact constraints of the last step's final substep (diagnostic).
    #[must_use]
    pub fn contact_count(&self) -> usize {
        self.contacts.len()
    }

    /// The last step's entity proxies at their final positions, in proxy
    /// order, then its sensors: the spatial index's entries but the tiles,
    /// which [`Self::index_tiles`] gives. A body's identity is its key.
    /// Returns the entity proxy count, the first tile's index.
    pub(super) fn index_bodies(&self, out: &mut Vec<IndexEntry>) -> usize {
        let bodies = &self.proxies[..self.bodies];
        out.extend(
            bodies
                .iter()
                .zip(&self.layers.of)
                .map(|(proxy, layers)| IndexEntry {
                    entity: proxy.entity,
                    center: proxy.center,
                    shape: proxy.shape,
                    face_mask: proxy.face_mask,
                    sensor: false,
                    identity: proxy.key,
                    layers: layers.layers,
                    mask: layers.mask,
                }),
        );
        out.extend(self.sensors.iter().map(|sensor| IndexEntry {
            entity: Some(sensor.entity),
            center: sensor.center,
            shape: sensor.shape,
            face_mask: FACE_ALL,
            sensor: true,
            identity: sensor.key,
            layers: sensor.layers.layers,
            mask: sensor.layers.mask,
        }));
        self.bodies
    }

    /// The tile outline's runs as index entries, in their first cell's tile
    /// order, each in its collision layer's layers, and the runs into
    /// `runs`; with the cache version they are of.
    pub(super) fn index_tiles(&self, out: &mut Vec<IndexEntry>, runs: &mut Vec<TileRun>) -> u64 {
        let tile_runs = self.tiles.runs();
        runs.extend_from_slice(tile_runs);
        out.extend(tile_runs.iter().map(|run| IndexEntry {
            entity: None,
            center: run.center,
            shape: Shape::Aabb {
                half_extents: run.half,
            },
            face_mask: run.face_mask,
            sensor: false,
            identity: run.identity,
            layers: run.layers,
            mask: run.mask,
        }));
        self.tiles.version()
    }

    /// The version of the tile cache the last step used.
    pub(super) fn tiles_version(&self) -> u64 {
        self.tiles.version()
    }

    /// The tile cache the last step used, whose grids region queries and
    /// sensors read.
    pub(super) fn grid_cells(&self) -> &TileCache {
        &self.tiles
    }
}

/// One exposed face of the tile outline, a world segment
/// ([`PhysicsBuffers::terrain_faces`]).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TerrainFace {
    /// The face's top or left end.
    pub start: Vec2,
    /// Its bottom or right end.
    pub end: Vec2,
    /// Its outward unit normal.
    pub normal: Vec2,
    /// A one-way tile's face: solid from above alone.
    pub one_way: bool,
}

/// Wake `entity`'s sleep island (D-065); no-op when nothing has ever slept.
pub fn wake(world: &mut World, entity: Entity) {
    if let Some(buffers) = world.get_resource_mut::<PhysicsBuffers>() {
        buffers.wake(entity);
    }
}

/// Moves `entity` to `at` at once: its `Position`, `PrevPosition` and
/// `Transform.position`, each it has, set to `at`, its `Velocity` zeroed
/// when it has one, and its island woken, so nothing draws it sliding from
/// the old point. A kinematic body or a sensor jumps as a `Position` write
/// moves it: no contact sees the jump.
pub fn teleport(world: &mut World, entity: Entity, at: Vec2) {
    if let Some(position) = world.get_mut::<Position>(entity) {
        position.0 = at;
    }
    if let Some(prev) = world.get_mut::<PrevPosition>(entity) {
        prev.0 = at;
    }
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = at;
    }
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 = Vec2::ZERO;
    }
    wake(world, entity);
}

/// Run one physics tick with fixed substeps (D-064), advancing at most
/// `PhysicsConfig::max_step_dt` of the frame's game dt, [`Time::delta`]
/// (D-094); a paused clock leaves every body as it is.
pub fn physics_step(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }

    let config = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default();

    // Step bound (D-094): a longer substep would lower the contact-hertz cap
    // in `soft_params` and soften every contact, so the rest of a slow
    // frame's time is dropped instead of simulated.
    let dt = if config.max_step_dt > 0.0 {
        dt.min(config.max_step_dt)
    } else {
        dt
    };

    // Remove/reinsert buffers to avoid resource borrow + entity borrow overlap.
    let mut buffers = world
        .remove_resource::<PhysicsBuffers>()
        .unwrap_or_default();

    integrate_loose_bodies(world, dt, config.gravity);
    // The contact record runs while a queue reads it.
    let extra = &mut *buffers.extra;
    extra.record_on = world.has_resource::<EventQueue<ContactBegan>>()
        || world.has_resource::<EventQueue<ContactEnded>>();
    if extra.record_on {
        extra.record.begin_step();
    } else {
        extra.record.clear();
    }

    let kinematics = gather_proxies(
        world,
        &mut buffers.proxies,
        &mut buffers.layers,
        &mut buffers.sensors,
        &mut buffers.tile_ids,
        &mut buffers.tiles,
        &mut buffers.extra,
    );
    buffers.bodies = buffers.proxies.len() - buffers.tiles.len();
    let substeps = substep_count(&config, dt, kinematics > 0);
    let sub_dt = dt / substeps as f32;
    buffers.pairs_invalidated = true;
    buffers.static_grid.begin_frame();

    let sleep_enabled = config.sleep_threshold > 0.0;
    if sleep_enabled {
        // Gather leaves `sleeping = false` on every proxy; this flags the
        // proxies of persistent sleepers for the frame.
        // A sleeper within the speculative slack of a support that changed
        // wakes; one resting on it sits closer.
        sleep_frame_start(&mut buffers, 4.0 * config.linear_slop);
    }
    // Island union-find over this frame's touching contacts (D-065).
    buffers.island_parent.clear();
    buffers
        .island_parent
        .extend(0..buffers.proxies.len() as u32);
    buffers.kept.clear();
    buffers.events.clear();

    for step in 0..substeps {
        let time_left = (dt - step as f32 * sub_dt).max(sub_dt);
        substep(&config, sub_dt, time_left, &mut buffers, sleep_enabled);
        #[cfg(test)]
        {
            buffers.substeps_run += 1;
        }
    }

    sync_impulses(&mut buffers);
    if buffers.extra.record_on {
        let PhysicsBuffers {
            proxies,
            bodies,
            tiles,
            layers,
            extra,
            ..
        } = &mut buffers;
        let StepExtra {
            record,
            began,
            ended,
            ..
        } = &mut **extra;
        record.finish(proxies, *bodies, tiles, layers, began, ended);
    }
    write_back(
        world,
        &buffers.proxies,
        &mut buffers.sensors,
        &buffers.extra,
        dt,
    );
    // Queries see this step's final positions from here on.
    SpatialIndex::mark_stale(world);
    if buffers.sensors.is_empty() {
        buffers.sensor_pairs.clear();
        buffers.sensor_tiles.clear();
    } else {
        sensor_pass(world, &mut buffers);
    }

    if sleep_enabled {
        sleep_frame_end(world, &config, dt, &mut buffers);
    }

    // Events accumulated across the frame's substeps drain once, in substep
    // order; the drain preserves the event buffer allocation.
    if !buffers.events.is_empty() {
        if let Some(sink) = world.get_resource_mut::<EventQueue<CollisionEvent>>() {
            for event in buffers.events.drain(..) {
                sink.send(event);
            }
        } else {
            buffers.events.clear();
        }
    }
    // Begins and ends follow the collision events.
    let extra = &mut *buffers.extra;
    if !extra.began.is_empty() {
        if let Some(sink) = world.get_resource_mut::<EventQueue<ContactBegan>>() {
            for event in extra.began.drain(..) {
                sink.send(event);
            }
        } else {
            extra.began.clear();
        }
    }
    if !extra.ended.is_empty() {
        if let Some(sink) = world.get_resource_mut::<EventQueue<ContactEnded>>() {
            for event in extra.ended.drain(..) {
                sink.send(event);
            }
        } else {
            extra.ended.clear();
        }
    }
    if !buffers.sensor_events.is_empty() {
        if let Some(sink) = world.get_resource_mut::<EventQueue<SensorEntered>>() {
            for event in buffers.sensor_events.drain(..) {
                sink.send(event);
            }
        } else {
            buffers.sensor_events.clear();
        }
    }

    world.insert_resource(buffers);
}

/// The config the pair budgets read (D-081): `config`, its gravity times
/// the largest gravity scale's size when one is above 1, so a body falling
/// faster than the world's gravity keeps its pairs; at scales up to 1 the
/// budgets are `config`'s, bit for bit.
fn pair_config(config: &PhysicsConfig, gravity_scales: &[f32]) -> PhysicsConfig {
    let lift = gravity_scales
        .iter()
        .fold(1.0_f32, |most, &scale| most.max(scale.abs()));
    PhysicsConfig {
        gravity: config.gravity * lift,
        ..*config
    }
}

/// The step's substep count (D-064): `PhysicsConfig::substeps`, or, in a
/// world with a kinematic proxy, enough that no substep is longer than a
/// 1/60 s step's, from `substeps` up to twice it. Contacts against a
/// kinematic body take the immovable softness, which `soft_params` caps at a
/// quarter of the substep rate, and a crowd pressing on the body needs what
/// a 1/60 s step gives it. A still kinematic body counts, so one that starts
/// or stops moving leaves the count as it was.
fn substep_count(config: &PhysicsConfig, dt: f32, kinematic: bool) -> u32 {
    let substeps = config.substeps.max(1);
    if !kinematic {
        return substeps;
    }
    // The 0.001 keeps rounding at 1/60 and 1/30 s from adding a substep.
    let wanted = (substeps as f32 * dt * 60.0 - 0.001).ceil();
    (wanted as u32).clamp(substeps, substeps.saturating_mul(2))
}

/// Write end-of-frame state back to the `World`, once per frame (D-066).
/// Sleeping bodies skip writeback so their components stay bit-frozen — the
/// invariant external-write detection relies on (D-065). A moving kinematic
/// body writes its `Position` only: the step never changes its velocity. A
/// sensor row pulls no proxy: it moves by `velocity · dt` and records its
/// new centre. An awake spinner writes its `Spin`'s angle and angular
/// velocity. The mutable query iterates the exact archetype/row order
/// `gather_proxies` used, so entity proxies zip positionally with the query
/// rows.
fn write_back(
    world: &mut World,
    proxies: &[Proxy],
    sensors: &mut [SensorBody],
    extra: &StepExtra,
    dt: f32,
) {
    let mut rows = proxies.iter();
    let mut sensor_rows = sensors.iter_mut();
    for (entity, collider, position, body, velocity) in world.query_mut::<(
        Entity,
        &Collider,
        &mut Position,
        Option<&RigidBody>,
        Option<&mut Velocity>,
    )>() {
        if body.is_some_and(|body| body.kind == BodyKind::Sensor) {
            let sensor = sensor_rows
                .next()
                .expect("gather and writeback queries must agree on sensors");
            debug_assert_eq!(sensor.entity, entity, "sensor order diverged");
            if sensor.velocity != Vec2::ZERO {
                position.0 += sensor.velocity * dt;
            }
            sensor.center = position.0 + collider.offset;
            continue;
        }
        let proxy = rows
            .next()
            .expect("gather and writeback queries must agree on entity count");
        debug_assert_eq!(proxy.entity, Some(entity), "proxy order diverged");
        if !proxy.is_dynamic || proxy.sleeping {
            if proxy.kinematic && proxy.velocity != Vec2::ZERO {
                position.0 = proxy.center - proxy.offset;
            }
            continue;
        }
        position.0 = proxy.center - proxy.offset;
        if let Some(vel) = velocity {
            vel.0 = proxy.velocity;
        }
    }
    for turn in &extra.spins {
        if proxies[turn.proxy as usize].sleeping {
            continue;
        }
        if let Some(spin) = world.get_mut::<Spin>(turn.entity) {
            spin.angle = turn.angle;
            spin.angular_velocity = turn.angular_velocity;
        }
    }
}

/// Dynamic bodies without a collider never enter the solver; integrate them
/// once per frame (old behavior integrated them per substep — for pure
/// ballistic motion the trajectories differ only at O(g·dt²)). A
/// collider-less kinematic body or sensor moves by its velocity, without
/// gravity. A dynamic one falls by its gravity scale.
/// Columnar pass over collider-less archetypes; no per-entity lookups.
fn integrate_loose_bodies(world: &mut World, dt: f32, gravity: Vec2) {
    for (_entity, velocity, position, body) in world.query_mut_filtered::<(
        Entity,
        &mut Velocity,
        &mut Position,
        &mut RigidBody,
    ), Without<Collider>>()
    {
        match body.kind {
            BodyKind::Dynamic => {
                velocity.0 += gravity * dt * body.gravity_scale;
                position.0 += velocity.0 * dt;
            }
            BodyKind::Kinematic | BodyKind::Sensor => position.0 += velocity.0 * dt,
            BodyKind::Static => {}
        }
    }
}

/// One solver substep: contacts once, warm-started soft solve, integrate,
/// arrival pass, relax, restitution, speculative safety net. Runs entirely on the
/// dense proxy arrays — no `World` access (D-066). The pair budgets read
/// [`pair_config`]'s gravity.
fn substep(
    config: &PhysicsConfig,
    sub_dt: f32,
    time_left: f32,
    buffers: &mut PhysicsBuffers,
    sleep_enabled: bool,
) {
    let pair_config = pair_config(config, &buffers.extra.gravity_scales);
    refresh_pairs(&pair_config, sub_dt, time_left, buffers);
    #[cfg(test)]
    if buffers.check_pair_contacts {
        tests::assert_pair_contacts(config, sub_dt, buffers);
        buffers.checked_substeps += 1;
    }
    let PhysicsBuffers {
        proxies,
        bodies,
        tiles,
        pair_budgets,
        pair_flags,
        layers,
        repaired,
        repair_grid,
        pairs_invalidated,
        pairs,
        events,
        candidates,
        grid,
        static_grid,
        contacts,
        impulses,
        pair_impulses,
        seed_pair_impulses,
        impulses_dirty,
        #[cfg(test)]
        reference_impulses,
        sleep,
        island_parent,
        kept,
        arrival,
        extra,
        ..
    } = buffers;
    candidates.clear();
    contacts.clear();
    extra.friction.clear();
    extra.rolling.clear();
    extra.contact_closing.clear();
    extra.touches.clear();
    let bodies = *bodies;
    let friction_on = !extra.frictions.is_empty();
    let spins_on = !extra.spin_of.is_empty();
    let record_on = extra.record_on;
    let seed_impulses = std::mem::take(seed_pair_impulses);

    // Narrow phase once per substep; solver iterations reuse these contacts.
    // Speculative admission (D-064): pairs separated by less than one substep
    // of relative travel (plus slack for gravity/solver-injected velocity)
    // enter the buffer with negative penetration (= gap); the solver's
    // separated branch then clamps approach so they arrive exactly touching.
    // Events emit from this pre-solve pass — the exact contact set the old
    // solver saw on iteration 0 — but only once actually penetrating;
    // positive-gap contacts stay silent until touch.
    let speculative_slack = 4.0 * config.linear_slop;
    let wake_threshold = config.sleep_threshold.max(0.0);
    for (pair_index, &(a_u, b_u)) in pairs.iter().enumerate() {
        // Forget a disappearing contact immediately, even if it returns
        // later without a broadphase rebuild.
        let cached_impulse = std::mem::replace(&mut pair_impulses[pair_index], 0.0);
        let cached_tangent = if friction_on {
            std::mem::replace(&mut extra.pair_tangents[pair_index], 0.0)
        } else {
            0.0
        };
        let a_idx = a_u as usize;
        let b_idx = b_u as usize;
        let margin = (proxies[a_idx].velocity - proxies[b_idx].velocity).length() * sub_dt
            + speculative_slack;
        let Some(contact) = narrow_phase(&proxies[a_idx], &proxies[b_idx], margin) else {
            continue;
        };
        // A one-way tile's new contact needs the body's bottom within the
        // slack of its top face; one the pair carried last substep keeps the
        // narrow phase's quarter cell, so a pressed body does not sink
        // through.
        if proxies[b_idx].face_mask & FACE_ONE_WAY != 0 {
            let held = if seed_impulses {
                impulses.get(pair_key(proxies[a_idx].key, proxies[b_idx].key))
            } else {
                cached_impulse
            };
            let (a, b) = (&proxies[a_idx], &proxies[b_idx]);
            if held <= 0.0
                && !one_way_holds(a, a.center, b, b.center, contact.normal, speculative_slack)
            {
                continue;
            }
        }
        // Wake test (D-065): a touching contact with real relative motion, or
        // a speculative gap approached faster than the threshold, wakes the
        // sleeping side before the solver sees the contact — a bullet wakes
        // its impact fringe while still a gap away. Resting-speed contacts
        // leave the sleeper frozen (immovable below), so wake waves damp out
        // where motion dies instead of sweeping the whole pile.
        if proxies[b_idx].sleeping {
            debug_assert!(!proxies[a_idx].sleeping, "sleeping bodies never initiate");
            let relative = proxies[a_idx].velocity - proxies[b_idx].velocity;
            let wakes = if proxies[a_idx].kinematic {
                // A moving kinematic body wakes a sleeper it touches or
                // closes on at any speed: under the threshold both sides
                // would stay immovable, the pair would be dropped below and
                // the kinematic body would pass through. A still one wakes
                // nothing, as a static body does.
                proxies[a_idx].velocity != Vec2::ZERO
                    && (contact.penetration > 0.0 || relative.dot(contact.normal) < 0.0)
            } else if !kept.is_empty() && island_holds_any(island_parent, a_u, kept) {
                // The same wake reaches through the island a moving
                // kinematic body presses: a sleeper ahead of it would hold
                // the pressed bodies at inverse mass 0 against the
                // kinematic body, and they would sink into it.
                contact.penetration > 0.0 || relative.dot(contact.normal) < 0.0
            } else if contact.penetration > 0.0 {
                relative.length_squared() > wake_threshold * wake_threshold
            } else {
                relative.dot(contact.normal) < -wake_threshold
            };
            if wakes {
                proxies[b_idx].sleeping = false;
                *pairs_invalidated = true;
                let entry = &mut sleep.entries[b_idx];
                entry.sleeping = false;
                entry.timer = 0.0;
            }
        }
        // A sleeping side is frozen at inverse mass 0 for this substep.
        let inv_a = proxies[a_idx].inv_mass;
        let inv_b = if proxies[b_idx].sleeping {
            0.0
        } else {
            proxies[b_idx].inv_mass
        };
        let inv_mass_sum = inv_a + inv_b;
        if inv_mass_sum <= 0.0 {
            continue;
        }
        let key = pair_key(proxies[a_idx].key, proxies[b_idx].key);
        let approach = (proxies[a_idx].velocity - proxies[b_idx].velocity).dot(contact.normal);
        let impulse = if seed_impulses {
            impulses.get(key)
        } else {
            cached_impulse
        };
        #[cfg(test)]
        if let Some(reference) = reference_impulses.as_ref() {
            assert_eq!(
                impulse.to_bits(),
                reference.get(key).to_bits(),
                "warm-start diverged"
            );
        }
        contacts.push(ContactConstraint {
            a: a_u,
            b: b_u,
            normal: contact.normal,
            penetration: contact.penetration,
            normal_mass: 1.0 / inv_mass_sum,
            approach,
            impulse,
            restitution: proxies[a_idx].restitution.max(proxies[b_idx].restitution),
            vs_static: inv_a == 0.0 || inv_b == 0.0,
            inv_a,
            inv_b,
            key,
            pair_index,
        });
        // The record carries the largest closing speed of a pair in reach
        // since its last touch.
        if record_on {
            let carried = extra.record.closing.get(key);
            extra.contact_closing.push(carried.max(-approach));
        }
        // Friction rides beside the contact when both sides have some, and
        // rolling resistance when a side spins. A spinning side's moment of
        // inertia is `m r² / 2`, so its rim adds `r² / I = 2 / m` to the
        // tangent's inverse mass.
        let (spin_a, spin_b) = if spins_on {
            (extra.spin_of[a_idx], extra.spin_of[b_idx])
        } else {
            (NO_SPIN, NO_SPIN)
        };
        if friction_on {
            let mu = (extra.frictions[a_idx] * extra.frictions[b_idx]).sqrt();
            if mu > 0.0 {
                let mut inv_tangent = inv_mass_sum;
                if spin_a != NO_SPIN {
                    inv_tangent += 2.0 * inv_a;
                }
                if spin_b != NO_SPIN {
                    inv_tangent += 2.0 * inv_b;
                }
                extra.friction.push(FrictionConstraint {
                    contact: (contacts.len() - 1) as u32,
                    spin_a,
                    spin_b,
                    mu,
                    tangent_mass: 1.0 / inv_tangent,
                    impulse: if seed_impulses {
                        extra.tangents.get(key)
                    } else {
                        cached_tangent
                    },
                });
            }
        }
        if (spin_a != NO_SPIN || spin_b != NO_SPIN) && !extra.rollings.is_empty() {
            let k = extra.rollings[a_idx].max(extra.rollings[b_idx]);
            let radius_of = |slot: u32, inv: f32| {
                if slot == NO_SPIN || inv == 0.0 {
                    (0.0, 0.0)
                } else {
                    let radius = extra.spins[slot as usize].radius;
                    (radius, 2.0 * inv / (radius * radius))
                }
            };
            let ((ra, inv_ia), (rb, inv_ib)) = (radius_of(spin_a, inv_a), radius_of(spin_b, inv_b));
            if k > 0.0 && inv_ia + inv_ib > 0.0 {
                extra.rolling.push(RollingConstraint {
                    contact: (contacts.len() - 1) as u32,
                    spin_a,
                    spin_b,
                    bound: k * ra.max(rb),
                    mass: 1.0 / (inv_ia + inv_ib),
                    impulse: 0.0,
                });
            }
        }
        if contact.penetration > 0.0 {
            if proxies[a_idx].kinematic || proxies[b_idx].kinematic {
                kinematic_touch(proxies, events, kept, a_idx, b_idx, &contact, approach);
                continue;
            }
            // Touching contacts between awake dynamic bodies join one island;
            // islands sleep and re-sleep as a unit (D-065). Without a
            // kinematic side, `a` is an awake dynamic body.
            if sleep_enabled
                && proxies[b_idx].is_dynamic
                && !proxies[b_idx].sleeping
                && proxies[b_idx].entity.is_some()
            {
                uf_union(island_parent, a_u, b_u);
            }
            if let Some(a_entity) = proxies[a_idx].entity {
                events.push(CollisionEvent {
                    a: a_entity,
                    b: proxies[b_idx].entity,
                    normal: contact.normal,
                    penetration: contact.penetration,
                    speed: (-approach).max(0.0),
                    tile: tile_hit(tiles, bodies, b_idx, &proxies[b_idx]),
                });
            }
        }
    }

    // Integrate velocities (gravity), then warm start from stored impulses.
    // Each awake body's velocity is kept as the narrow phase saw it: the
    // arrival pass measures what the solve adds against it (D-092).
    // Tile proxies follow the entity proxies and never move: the passes
    // that skip them walk the entity proxies alone.
    // A body falls by its gravity scale; while every scale is 1 the loop
    // reads none.
    arrival.fit(proxies.len());
    let gravity = config.gravity * sub_dt;
    if extra.gravity_scales.is_empty() {
        for (proxy, admitted) in proxies[..bodies]
            .iter_mut()
            .zip(arrival.admitted.iter_mut())
        {
            if proxy.is_dynamic && !proxy.sleeping {
                *admitted = proxy.velocity;
                if proxy.has_velocity {
                    proxy.velocity += gravity;
                }
            }
        }
    } else {
        for ((proxy, admitted), &scale) in proxies[..bodies]
            .iter_mut()
            .zip(arrival.admitted.iter_mut())
            .zip(&extra.gravity_scales)
        {
            if proxy.is_dynamic && !proxy.sleeping {
                *admitted = proxy.velocity;
                if proxy.has_velocity {
                    proxy.velocity += gravity * scale;
                }
            }
        }
    }
    for contact in contacts.iter() {
        if contact.impulse > 0.0 {
            apply_impulse(proxies, contact, contact.normal * contact.impulse);
        }
    }
    for friction in &extra.friction {
        if friction.impulse != 0.0 {
            let contact = &contacts[friction.contact as usize];
            apply_tangent(
                proxies,
                &mut extra.spins,
                contact,
                friction,
                friction.impulse,
            );
        }
    }

    let inv_h = 1.0 / sub_dt;
    let soft = soft_params(config.contact_hertz, config.contact_damping_ratio, sub_dt);
    // Contacts against immovables get the stiffer static softness.
    let soft_static = soft_params(
        2.0 * config.contact_hertz,
        config.contact_damping_ratio,
        sub_dt,
    );

    for _ in 0..config.solver_iterations.max(1) {
        solve_contacts(proxies, contacts, config, soft, soft_static, inv_h, true);
        solve_friction(proxies, &mut extra.spins, contacts, &mut extra.friction);
        solve_rolling(&mut extra.spins, contacts, &mut extra.rolling);
    }

    // Position integration converts the bias velocity into depenetration. A
    // body whose velocity the solve changed by more than the arrival
    // threshold is listed on the way (D-092).
    let arrival_threshold = ARRIVAL_SLOPS * config.linear_slop * inv_h;
    let arrival_threshold_sq = arrival_threshold * arrival_threshold;
    arrival.list.clear();
    for (index, (proxy, admitted)) in proxies[..bodies]
        .iter_mut()
        .zip(arrival.admitted.iter())
        .enumerate()
    {
        if !proxy.is_dynamic || proxy.sleeping {
            // A kinematic body moves by its velocity; no solve changed it,
            // so it is never listed.
            if proxy.kinematic {
                proxy.prev_center = proxy.center;
                proxy.center += proxy.velocity * sub_dt;
            }
            continue;
        }
        if (proxy.velocity - *admitted).length_squared() > arrival_threshold_sq {
            arrival.list.push(index as u32);
        }
        proxy.prev_center = proxy.center;
        let travel = proxy.velocity * sub_dt;
        proxy.center += travel;
    }
    // A spinner turns as it moves; one asleep stays as it was.
    for spin in &mut extra.spins {
        if !proxies[spin.proxy as usize].sleeping {
            spin.angle += spin.angular_velocity * sub_dt;
        }
    }

    // A velocity the solve handed a body is checked against what it reaches
    // from where the substep began, and the move is corrected.
    if !arrival.list.is_empty() {
        arrival_pass(
            config,
            sub_dt,
            proxies,
            grid,
            repair_grid,
            pair_flags,
            layers,
            repaired,
            candidates,
            arrival,
            record_on.then_some(&mut extra.touches),
        );
    }
    // The record samples each contact's closing speed after the solve and
    // the arrival pass, which catches a body knocked into a contact built
    // while it rested; the arrival pass's clamps are touches.
    if record_on {
        for (contact, closing) in contacts.iter().zip(extra.contact_closing.iter_mut()) {
            let approach = (proxies[contact.a as usize].velocity
                - proxies[contact.b as usize].velocity)
                .dot(contact.normal);
            *closing = closing.max(-approach);
        }
        for &touch in &extra.touches {
            extra.record.observe(proxies, bodies, tiles, touch);
        }
        extra.touches.clear();
    }

    // One bias-free relax iteration strips the injected bias energy.
    solve_contacts(proxies, contacts, config, soft, soft_static, inv_h, false);
    solve_friction(proxies, &mut extra.spins, contacts, &mut extra.friction);
    solve_rolling(&mut extra.spins, contacts, &mut extra.rolling);

    apply_restitution(proxies, contacts, config.restitution_threshold);

    // A contact that penetrates or that the solve gave an impulse touches;
    // a touch spends its pair's carried speed.
    if record_on {
        for (contact, closing) in contacts.iter().zip(extra.contact_closing.iter_mut()) {
            if contact.penetration > 0.0 || contact.impulse > 0.0 {
                let (a, b, normal) = if proxies[contact.a as usize].kinematic {
                    (contact.b, contact.a, -contact.normal)
                } else {
                    (contact.a, contact.b, contact.normal)
                };
                let touch = TouchObs {
                    a,
                    b,
                    normal,
                    speed: *closing,
                };
                extra.record.observe(proxies, bodies, tiles, touch);
                *closing = 0.0;
            }
        }
        extra.record.carry(contacts, &extra.contact_closing);
    }

    // Direct indexed carry between substeps; synchronize the keyed map only
    // at frame end or when a pair rebuild invalidates these indices.
    for contact in contacts.iter() {
        pair_impulses[contact.pair_index] = contact.impulse;
    }
    for friction in &extra.friction {
        extra.pair_tangents[contacts[friction.contact as usize].pair_index] = friction.impulse;
    }
    *impulses_dirty = true;
    #[cfg(test)]
    if let Some(reference) = reference_impulses.as_mut() {
        reference.reset(contacts.len());
        for contact in contacts.iter().filter(|c| c.impulse > 0.0) {
            reference.insert(contact.key, contact.impulse);
        }
    }

    // Safety net: clamp extreme dynamics to first static sweep hit (covers
    // solver-injected velocity a speculative pair admission never saw).
    // Events keep accumulating across substeps; the frame drains them once.
    speculative_pass(
        config,
        proxies,
        bodies,
        layers,
        grid,
        static_grid,
        candidates,
        events,
        tiles,
        record_on.then_some(&mut extra.touches),
    );
    if record_on {
        for &touch in &extra.touches {
            extra.record.observe(proxies, bodies, tiles, touch);
        }
    }

    // Sum displacement between narrow phases, including the sweep's clamp.
    // This bounds distance from the build center even after direction changes.
    for (proxy, budget) in proxies[..bodies].iter().zip(pair_budgets) {
        if proxy.moves() {
            budget.travel += (proxy.center - proxy.prev_center).length();
        }
    }
}

/// One sensor's tile pairs, sorted, each with its cell's map, and what
/// they were found for.
#[derive(Debug, Default)]
struct SensorTiles {
    key: Option<SensorKey>,
    /// The sensor pass that last found or kept them.
    used: u64,
    pairs: Vec<(u128, Entity)>,
}

/// What a sensor's tile overlaps depend on beside the tiles: the sensor,
/// its box and its mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SensorKey {
    identity: u64,
    center: [u32; 2],
    shape: [u32; 3],
    mask: u32,
}

impl SensorKey {
    fn of(sensor: &IndexEntry) -> Self {
        let shape = match sensor.shape {
            Shape::Aabb { half_extents } => [0, half_extents.x.to_bits(), half_extents.y.to_bits()],
            Shape::Circle { radius } => [1, radius.to_bits(), 0],
        };
        Self {
            identity: sensor.identity,
            center: [sensor.center.x.to_bits(), sensor.center.y.to_bits()],
            shape,
            mask: sensor.mask,
        }
    }
}

/// Sensor pass: each sensor, in gather order, against the bodies it
/// overlaps at the step's final positions, in index order (the spatial
/// index, built here), then the solid cells of the tile cache's grids, in
/// tile order. A pair the previous step lacked sends `SensorEntered`; the
/// step's pairs, sorted, replace the previous ones. A sensor whose key
/// matches the one the last pass kept keeps its tile pairs, drops those of
/// the cells the step's refresh changed and tests those cells alone; the
/// events and the pairs are those a test of every cell gives.
fn sensor_pass(world: &World, buffers: &mut PhysicsBuffers) {
    let mut pairs = std::mem::take(&mut buffers.sensor_scratch);
    let mut body_pairs = std::mem::take(&mut buffers.sensor_body_pairs);
    let mut entered = std::mem::take(&mut buffers.sensor_events);
    let mut kept = std::mem::take(&mut buffers.sensor_tiles);
    buffers.sensor_passes += 1;
    let pass = buffers.sensor_passes;
    let (replaced, written) = (buffers.tiles.replaced(), buffers.tiles.written_ids());
    kept.resize_with(buffers.sensors.len(), SensorTiles::default);
    let mut fresh = vec![false; buffers.sensors.len()];
    body_pairs.clear();
    let previous = &buffers.sensor_pairs;
    let pair_of = |sensor: &IndexEntry, other: &IndexEntry| {
        (u128::from(sensor.identity) << 64) | u128::from(other.identity)
    };
    {
        let kept = &mut kept;
        let fresh = &mut fresh;
        let entered = &mut entered;
        let body_pairs = &mut body_pairs;
        // Both closures below take the event list and the kept pairs, so
        // they go through cells shared by the three callbacks.
        let entered = std::cell::RefCell::new(entered);
        let kept = std::cell::RefCell::new(kept);
        let tested = SpatialIndex::sensor_overlaps(
            world,
            buffers,
            |index, sensor| {
                let key = SensorKey::of(sensor);
                let mut kept = kept.borrow_mut();
                let entry = &mut kept[index];
                if entry.key == Some(key) && entry.used + 1 == pass {
                    entry.used = pass;
                    if !(replaced.is_empty() && written.is_empty()) {
                        entry.pairs.retain(|&(pair, map)| {
                            !replaced.contains(&map)
                                && written.binary_search(&(pair as u64)).is_err()
                        });
                        fresh[index] = true;
                    }
                    true
                } else {
                    entry.key = Some(key);
                    entry.used = pass;
                    entry.pairs.clear();
                    fresh[index] = true;
                    false
                }
            },
            |sensor, other| {
                let pair = pair_of(sensor, other);
                body_pairs.push(pair);
                if previous.binary_search(&pair).is_err()
                    && let Some(entity) = sensor.entity
                {
                    entered
                        .borrow_mut()
                        .push(SensorEntered::new(entity, other.entity));
                }
            },
            |index, sensor, tile, map| {
                let pair = pair_of(sensor, tile);
                kept.borrow_mut()[index].pairs.push((pair, map));
                if previous.binary_search(&pair).is_err()
                    && let Some(entity) = sensor.entity
                {
                    entered
                        .borrow_mut()
                        .push(SensorEntered::new(entity, tile.entity));
                }
            },
        );
        #[cfg(test)]
        {
            buffers.sensor_cells = tested;
        }
        #[cfg(not(test))]
        let _ = tested;
    }
    for (entry, &fresh) in kept.iter_mut().zip(&fresh) {
        if fresh {
            entry.pairs.sort_unstable_by_key(|&(pair, _)| pair);
        }
    }
    // Every pair, sorted: the body pairs merged with each sensor's sorted
    // tile pairs, the sensors in key order, since a pair's high half is its
    // sensor's identity.
    body_pairs.sort_unstable();
    let mut order: Vec<usize> = (0..kept.len()).collect();
    order.sort_unstable_by_key(|&index| kept[index].key.map_or(0, |key| key.identity));
    pairs.clear();
    let mut bodies = body_pairs.iter().copied().peekable();
    for &index in &order {
        for &(tile, _) in &kept[index].pairs {
            while let Some(body) = bodies.next_if(|&body| body < tile) {
                pairs.push(body);
            }
            pairs.push(tile);
        }
    }
    pairs.extend(bodies);
    buffers.sensor_scratch = std::mem::replace(&mut buffers.sensor_pairs, pairs);
    buffers.sensor_body_pairs = body_pairs;
    buffers.sensor_events = entered;
    buffers.sensor_tiles = kept;
}

/// A touching contact with a kinematic side; the other is an awake dynamic
/// body, since a pair of immovables never reaches here. Its event names the
/// dynamic body as `a`, pushed along `normal`, and a moving kinematic body
/// keeps that body's island awake for the frame and lists it in `kept`,
/// through which the island wakes the sleepers it reaches. Kinematic bodies
/// join no island.
#[cold]
#[inline(never)]
fn kinematic_touch(
    proxies: &mut [Proxy],
    events: &mut Vec<CollisionEvent>,
    kept: &mut Vec<u32>,
    a_idx: usize,
    b_idx: usize,
    contact: &Contact,
    approach: f32,
) {
    let (pushed, kinematic, normal) = if proxies[a_idx].kinematic {
        (b_idx, a_idx, -contact.normal)
    } else {
        (a_idx, b_idx, contact.normal)
    };
    if proxies[kinematic].velocity != Vec2::ZERO && !proxies[pushed].kept_awake {
        proxies[pushed].kept_awake = true;
        kept.push(pushed as u32);
    }
    if let Some(entity) = proxies[pushed].entity {
        events.push(CollisionEvent {
            a: entity,
            b: proxies[kinematic].entity,
            normal,
            penetration: contact.penetration,
            speed: (-approach).max(0.0),
            tile: None,
        });
    }
}

/// The cell a tile proxy at `index` stands for; `None` for a body's.
fn tile_hit(tiles: &TileCache, bodies: usize, index: usize, proxy: &Proxy) -> Option<TileHit> {
    if proxy.entity.is_some() {
        return None;
    }
    tiles.cell_hit(index - bodies, proxy.center)
}

/// One touch a substep saw, by proxy: `a` the body it pushed along
/// `normal`, as `CollisionEvent` names it, and the pair's closing speed.
#[derive(Debug, Clone, Copy)]
struct TouchObs {
    a: u32,
    b: u32,
    normal: Vec2,
    speed: f32,
}

/// Flat open-addressing map from a 128-bit key to a `u32`.
#[derive(Debug, Default)]
struct KeyIndex {
    keys: Vec<u128>,
    values: Vec<u32>,
    mask: u64,
}

impl KeyIndex {
    /// Clear and size for `expected` insertions at <= 0.5 load factor.
    fn reset(&mut self, expected: usize) {
        let capacity = (expected * 2).next_power_of_two().max(64);
        self.mask = capacity as u64 - 1;
        self.keys.clear();
        self.keys.resize(capacity, u128::MAX);
        self.values.clear();
        self.values.resize(capacity, 0);
    }

    /// Empty the map, keeping its buffers.
    fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
        self.mask = 0;
    }

    fn slot(&self, key: u128) -> usize {
        let h = (key as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ ((key >> 64) as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        ((h ^ (h >> 29)) & self.mask) as usize
    }

    fn insert(&mut self, key: u128, value: u32) {
        debug_assert!(key != u128::MAX && !self.keys.is_empty());
        let mut i = self.slot(key);
        loop {
            if self.keys[i] == u128::MAX || self.keys[i] == key {
                self.keys[i] = key;
                self.values[i] = value;
                return;
            }
            i = (i + 1) & self.mask as usize;
        }
    }

    fn get(&self, key: u128) -> Option<u32> {
        if self.keys.is_empty() {
            return None;
        }
        let mut i = self.slot(key);
        loop {
            if self.keys[i] == key {
                return Some(self.values[i]);
            }
            if self.keys[i] == u128::MAX {
                return None;
            }
            i = (i + 1) & self.mask as usize;
        }
    }
}

/// A pair's identity in the contact record: its two sides, each an entity
/// key or a tile's identity (its map, layer and cell), smaller first.
fn side_pair(a: u64, b: u64) -> u128 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    (u128::from(hi) << 64) | u128::from(lo)
}

/// A kinematic proxy's centre, box and layers, hashed, so the record sees
/// one written between steps; 0 for every other kind, which a change wakes
/// or solves.
fn kinematic_state(proxy: &Proxy, layers: Layers) -> u64 {
    if !proxy.kinematic {
        return 0;
    }
    let half = proxy.half_extents();
    [
        proxy.center.x.to_bits(),
        proxy.center.y.to_bits(),
        half.x.to_bits(),
        half.y.to_bits(),
        layers.layers,
        layers.mask,
    ]
    .into_iter()
    .fold(0x243F_6A88_85A3_08D3_u64, |h, word| {
        (h ^ u64::from(word)).wrapping_mul(0x0000_0100_0000_01B3)
    }) | 1
}

/// Whether a proxy cannot move in the step: static, a tile, asleep through
/// it, or a kinematic body at rest.
fn immovable(proxy: &Proxy) -> bool {
    (!proxy.is_dynamic && !proxy.kinematic)
        || proxy.sleeping
        || (proxy.kinematic && proxy.velocity == Vec2::ZERO)
}

/// One touching pair of the contact record.
#[derive(Debug, Clone, Copy)]
struct RecordEntry {
    id: u128,
    /// Its sides and normal as its first touching contact named them, and
    /// its tile side's cell.
    a: Entity,
    b: Option<Entity>,
    tile: Option<TileHit>,
    normal: Vec2,
    /// The begin's speed: the largest closing speed of its touches in the
    /// step it began.
    speed: f32,
    /// The record step that last found it touching, and the one it began.
    touched: u64,
    began: u64,
    /// Its sides' keys, a tile's identity for `b` when `b_tile`.
    a_key: u64,
    b_key: u64,
    b_tile: bool,
    /// Where its sides were: a body's proxy index, a tile's ordinal.
    a_at: u32,
    b_at: u32,
    /// Its kinematic sides' [`kinematic_state`].
    a_state: u64,
    b_state: u64,
}

/// The contact record (`ContactBegan`, `ContactEnded`): the pairs touching
/// at the last step's end, in the order they began. A step's touches mark
/// their pairs, a new one beginning; at its end a pair it did not touch ends,
/// unless its sides are both immovable through the step and still where
/// they were, or still overlapping.
#[derive(Debug, Default)]
struct ContactRecord {
    entries: Vec<RecordEntry>,
    /// Each entry's place by its identity.
    index: KeyIndex,
    /// Steps the record has run.
    stamp: u64,
    /// The closing speeds carried from the last substep, by solver pair key:
    /// each pair in reach the largest it had since its last touch, the
    /// nonzero ones.
    closing: ImpulseMap,
    next_closing: ImpulseMap,
    /// Entity proxy indices by key and tile ordinals by identity, built the
    /// first time a step's stored place misses.
    by_key: KeyIndex,
    by_key_built: bool,
    by_identity: KeyIndex,
    by_identity_built: bool,
}

impl ContactRecord {
    /// Forgets every pair and carried speed.
    fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
        self.closing.clear();
        self.next_closing.clear();
    }

    /// A new step: its touches take a new stamp, and the places are looked up
    /// afresh.
    fn begin_step(&mut self) {
        self.stamp += 1;
        self.by_key_built = false;
        self.by_identity_built = false;
    }

    /// Notes `touch`: its pair touches this step, beginning when the record
    /// does not hold it.
    fn observe(&mut self, proxies: &[Proxy], bodies: usize, tiles: &TileCache, touch: TouchObs) {
        let (pa, pb) = (&proxies[touch.a as usize], &proxies[touch.b as usize]);
        let Some(a) = pa.entity else {
            return;
        };
        let (b_key, b_at, b_tile) = if pb.entity.is_some() {
            (pb.key, touch.b, false)
        } else {
            let ordinal = touch.b as usize - bodies;
            (tiles.ids[ordinal], ordinal as u32, true)
        };
        let id = side_pair(pa.key, b_key);
        let stamp = self.stamp;
        if let Some(at) = self.index.get(id) {
            let entry = &mut self.entries[at as usize];
            entry.touched = stamp;
            if entry.began == stamp {
                entry.speed = entry.speed.max(touch.speed);
            }
            if entry.a_key == pa.key {
                (entry.a_at, entry.b_at) = (touch.a, b_at);
            } else {
                (entry.a_at, entry.b_at) = (b_at, touch.a);
            }
            return;
        }
        if self.index.keys.is_empty() || self.entries.len() * 2 >= self.index.keys.len() {
            self.reindex(self.entries.len() + 1);
        }
        self.index.insert(id, self.entries.len() as u32);
        self.entries.push(RecordEntry {
            id,
            a,
            b: pb.entity,
            tile: if b_tile {
                tiles.cell_hit(b_at as usize, pb.center)
            } else {
                None
            },
            normal: touch.normal,
            speed: touch.speed,
            touched: stamp,
            began: stamp,
            a_key: pa.key,
            b_key,
            b_tile,
            a_at: touch.a,
            b_at,
            a_state: 0,
            b_state: 0,
        });
    }

    /// The map of the entries' places, rebuilt for `expected` of them.
    fn reindex(&mut self, expected: usize) {
        self.index.reset(expected);
        for (at, entry) in self.entries.iter().enumerate() {
            self.index.insert(entry.id, at as u32);
        }
    }

    /// The closing speeds the next substep carries: each contact's still in
    /// reach, the nonzero ones.
    fn carry(&mut self, contacts: &[ContactConstraint], closing: &[f32]) {
        let carried = closing.iter().filter(|&&speed| speed > 0.0).count();
        if carried == 0 {
            self.next_closing.clear();
        } else {
            self.next_closing.reset(carried);
            for (contact, &speed) in contacts.iter().zip(closing) {
                if speed > 0.0 {
                    self.next_closing.insert(contact.key, speed);
                }
            }
        }
        std::mem::swap(&mut self.closing, &mut self.next_closing);
    }

    /// The proxy index of the body keyed `key`, last at `at`.
    fn body(&mut self, proxies: &[Proxy], bodies: usize, key: u64, at: u32) -> Option<usize> {
        let at = at as usize;
        if at < bodies && proxies[at].key == key {
            return Some(at);
        }
        if !self.by_key_built {
            self.by_key.reset(bodies);
            for (index, proxy) in proxies[..bodies].iter().enumerate() {
                self.by_key.insert(u128::from(proxy.key), index as u32);
            }
            self.by_key_built = true;
        }
        self.by_key.get(u128::from(key)).map(|index| index as usize)
    }

    /// The proxy index of the tile of identity `identity`, last ordinal `at`.
    fn tile(&mut self, tiles: &TileCache, bodies: usize, identity: u64, at: u32) -> Option<usize> {
        let at = at as usize;
        if tiles.ids.get(at) == Some(&identity) {
            return Some(bodies + at);
        }
        if !self.by_identity_built {
            self.by_identity.reset(tiles.ids.len());
            for (ordinal, &id) in tiles.ids.iter().enumerate() {
                self.by_identity.insert(u128::from(id), ordinal as u32);
            }
            self.by_identity_built = true;
        }
        self.by_identity
            .get(u128::from(identity))
            .map(|ordinal| bodies + ordinal as usize)
    }

    /// The step's end: a pair it touched and the last did not begins; one it
    /// did not touch ends, unless its sides are both immovable through the
    /// step and neither changed, or they still overlap. Begins come in
    /// first-touch order, ends in the record's.
    fn finish(
        &mut self,
        proxies: &[Proxy],
        bodies: usize,
        tiles: &TileCache,
        layers: &ProxyLayers,
        began: &mut Vec<ContactBegan>,
        ended: &mut Vec<ContactEnded>,
    ) {
        let stamp = self.stamp;
        let mut entries = std::mem::take(&mut self.entries);
        let before = entries.len();
        entries.retain_mut(|entry| {
            let ends = |entry: &RecordEntry, ended: &mut Vec<ContactEnded>| {
                ended.push(ContactEnded {
                    a: entry.a,
                    b: entry.b,
                    tile: entry.tile,
                });
            };
            let a = self.body(proxies, bodies, entry.a_key, entry.a_at);
            let b = if entry.b_tile {
                self.tile(tiles, bodies, entry.b_key, entry.b_at)
            } else {
                self.body(proxies, bodies, entry.b_key, entry.b_at)
            };
            let (Some(a), Some(b)) = (a, b) else {
                // A side went: a despawned body or an emptied cell.
                if entry.touched != stamp {
                    ends(entry, ended);
                    return false;
                }
                return true;
            };
            let state = |index: usize| kinematic_state(&proxies[index], layers.of[index]);
            if entry.touched != stamp {
                // A pair the step solved touches only as solved; one it did
                // not keeps touching while its sides stay as they were, or
                // still overlap.
                if !(immovable(&proxies[a]) && immovable(&proxies[b])) {
                    ends(entry, ended);
                    return false;
                }
                if (state(a), state(b)) != (entry.a_state, entry.b_state) {
                    let overlaps = layers.meet(a, b)
                        && narrow_phase(&proxies[a], &proxies[b], 0.0)
                            .is_some_and(|contact| contact.penetration > 0.0);
                    if !overlaps {
                        ends(entry, ended);
                        return false;
                    }
                }
                entry.touched = stamp;
            } else if entry.began == stamp {
                began.push(ContactBegan {
                    a: entry.a,
                    b: entry.b,
                    normal: entry.normal,
                    speed: entry.speed,
                    tile: entry.tile,
                });
            }
            entry.a_at = a as u32;
            entry.b_at = if entry.b_tile {
                (b - bodies) as u32
            } else {
                b as u32
            };
            (entry.a_state, entry.b_state) = (state(a), state(b));
            true
        });
        self.entries = entries;
        if self.entries.len() != before {
            self.reindex(self.entries.len());
        }
    }
}

#[cfg(test)]
#[path = "../../tests/physics/step.rs"]
mod tests;
