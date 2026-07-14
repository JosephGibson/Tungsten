//! Physics step: build broadphase -> fixed substeps (speculative contacts ->
//! soft solve -> integrate) -> events.
//!
//! The substep count is fixed via `PhysicsConfig::substeps` (D-064); no
//! velocity-derived amplification exists. Tunneling safety comes from
//! speculative contacts: the narrow phase admits broadphase pairs with a
//! positive gap up to a per-pair margin (relative travel per substep plus
//! slack) as signed-distance contacts, and the solver's separated branch
//! removes exactly the excess approach velocity so fast bodies arrive at
//! touching instead of passing through — for every pair class (static/dynamic
//! AABBs and circles, dynamic-vs-dynamic). The slab sweep survives only as a
//! safety net for solver-injected velocity spikes vs statics (a pair whose
//! velocity changed after admission), conservatively promoting static circles
//! to their bounding squares.
//!
//! The broadphase grid is staged from per-substep-travel-inflated AABBs and
//! reused across substeps; it is restaged mid-frame only when accumulated max
//! travel exhausts the half-cell query margin (D-062). Settled scenes build
//! once per frame; extreme scenes degrade to the old per-substep rebuild.
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
//! normal impulses (clamped >= 0) warm-start from a pair-keyed impulse map
//! that survives across frames, biased velocity iterations recover
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
//! The whole step is serial and deterministic by design (D-067): a
//! color-parallel contact solver was built and measured for step 6 of
//! `docs/plans/physics-scale-and-ccd.md` and dropped — at 25k awake bodies
//! the contact-solve passes are ~2% of the frame (the cost is pair query +
//! narrow phase + contact build), so threading the solver cannot move the
//! number and the coloring/sort machinery alone cost the serial path ~18%.

use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::{
    aabb_vs_aabb_speculative, aabb_vs_circle_speculative, circle_vs_circle_speculative,
    sweep_aabb_vs_aabb, Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP,
};
use super::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use super::events::CollisionEvent;
use super::PhysicsConfig;
use crate::assets::{LayerKind, TilemapInstance, TilemapRegistry};
use crate::ecs::{Entity, EventQueue, World};
use crate::time::DeltaTime;
use glam::Vec2;

/// Per-substep collider proxy; tile proxies use `entity = None`.
#[derive(Debug, Clone, Copy)]
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
}

impl Proxy {
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

    /// Union of the current AABB and its predicted position after `sub_dt`;
    /// pair queries run before integration, so coverage must look ahead.
    fn travel_aabb(&self, sub_dt: f32) -> Aabb {
        let cur = self.world_aabb();
        if !self.is_dynamic || self.velocity == Vec2::ZERO {
            return cur;
        }
        let predicted = Aabb::new(self.center + self.velocity * sub_dt, self.half_extents());
        cur.union(&predicted)
    }
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
}

/// Soft-constraint coefficients derived from (hertz, damping ratio, sub_dt).
#[derive(Debug, Clone, Copy)]
struct Softness {
    bias_rate: f32,
    mass_scale: f32,
    impulse_scale: f32,
}

fn soft_params(hertz: f32, damping_ratio: f32, h: f32) -> Softness {
    // Quarter of the substep rate is the stability ceiling.
    let hertz = hertz.min(0.25 / h);
    if hertz <= 0.0 {
        return Softness {
            bias_rate: 0.0,
            mass_scale: 1.0,
            impulse_scale: 0.0,
        };
    }
    let omega = std::f32::consts::TAU * hertz;
    let a1 = 2.0 * damping_ratio + h * omega;
    let c = h * omega * a1;
    Softness {
        bias_rate: omega / a1,
        mass_scale: c / (1.0 + c),
        impulse_scale: 1.0 / (1.0 + c),
    }
}

/// Sentinel for empty slots; unreachable as a real key because a pair key
/// of `u128::MAX` would need both members to share the same proxy key.
const EMPTY_PAIR: u128 = u128::MAX;

/// Flat open-addressing map from contact pair key to accumulated impulse.
/// Rebuilt from live contacts each substep (stale pairs age out for free);
/// never `std::HashMap` in the physics hot path (D-062 precedent).
#[derive(Debug, Default)]
struct ImpulseMap {
    keys: Vec<u128>,
    values: Vec<f32>,
    mask: u64,
}

impl ImpulseMap {
    /// Clear and size for `expected` insertions at <= 0.5 load factor.
    fn reset(&mut self, expected: usize) {
        let capacity = (expected * 2).next_power_of_two().max(64);
        self.mask = capacity as u64 - 1;
        self.keys.clear();
        self.keys.resize(capacity, EMPTY_PAIR);
        self.values.clear();
        self.values.resize(capacity, 0.0);
    }

    fn insert(&mut self, key: u128, value: f32) {
        debug_assert!(key != EMPTY_PAIR);
        let mut i = (hash_pair(key) & self.mask) as usize;
        loop {
            if self.keys[i] == EMPTY_PAIR || self.keys[i] == key {
                self.keys[i] = key;
                self.values[i] = value;
                return;
            }
            i = (i + 1) & self.mask as usize;
        }
    }

    fn get(&self, key: u128) -> f32 {
        if self.keys.is_empty() {
            return 0.0;
        }
        let mut i = (hash_pair(key) & self.mask) as usize;
        loop {
            if self.keys[i] == key {
                return self.values[i];
            }
            if self.keys[i] == EMPTY_PAIR {
                return 0.0;
            }
            i = (i + 1) & self.mask as usize;
        }
    }
}

fn hash_pair(key: u128) -> u64 {
    let h = (key as u64) ^ ((key >> 64) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let h = (h ^ (h >> 32)).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

fn hash_key(key: u64) -> u64 {
    let h = key.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

/// Per-body sleep bookkeeping (D-065); persists across frames in a side
/// table, never a component.
#[derive(Debug, Clone, Copy, Default)]
struct SleepEntry {
    /// Consecutive seconds below the sleep velocity threshold.
    timer: f32,
    sleeping: bool,
    /// Island tag at sleep time (a member entity key); islands wake as a
    /// unit on despawn or external write.
    island: u64,
    /// Collider center frozen at sleep, stored with the exact fp ops the
    /// next gather performs; a mismatch means an external `Position` write.
    center: Vec2,
}

/// Sentinel for empty sleep slots; entity keys always set bit 63
/// (`entity_key`), so 0 is never a real key.
const EMPTY_SLEEP: u64 = 0;

/// Flat open-addressing map from entity key to sleep state; rebuilt from live
/// proxies each frame (despawned entries age out and wake their island).
/// Never `std::HashMap` in the physics hot path (D-062 precedent).
#[derive(Debug, Default)]
struct SleepMap {
    keys: Vec<u64>,
    values: Vec<SleepEntry>,
    mask: u64,
}

impl SleepMap {
    /// Clear and size for `expected` insertions at <= 0.5 load factor.
    fn reset(&mut self, expected: usize) {
        let capacity = (expected * 2).next_power_of_two().max(64);
        self.mask = capacity as u64 - 1;
        self.keys.clear();
        self.keys.resize(capacity, EMPTY_SLEEP);
        self.values.clear();
        self.values.resize(capacity, SleepEntry::default());
    }

    fn slot(&self, key: u64) -> Option<usize> {
        if self.keys.is_empty() {
            return None;
        }
        let mut i = (hash_key(key) & self.mask) as usize;
        loop {
            if self.keys[i] == key {
                return Some(i);
            }
            if self.keys[i] == EMPTY_SLEEP {
                return None;
            }
            i = (i + 1) & self.mask as usize;
        }
    }

    fn get(&self, key: u64) -> Option<&SleepEntry> {
        self.slot(key).map(|i| &self.values[i])
    }

    fn get_mut(&mut self, key: u64) -> Option<&mut SleepEntry> {
        self.slot(key).map(|i| &mut self.values[i])
    }

    /// Insert during the frame-start rebuild only; capacity is pre-sized by
    /// `reset`, so probing always finds a slot.
    fn insert(&mut self, key: u64, value: SleepEntry) {
        debug_assert!(key != EMPTY_SLEEP);
        let mut i = (hash_key(key) & self.mask) as usize;
        loop {
            if self.keys[i] == EMPTY_SLEEP || self.keys[i] == key {
                self.keys[i] = key;
                self.values[i] = value;
                return;
            }
            i = (i + 1) & self.mask as usize;
        }
    }

    fn iter(&self) -> impl Iterator<Item = (u64, &SleepEntry)> {
        self.keys
            .iter()
            .zip(self.values.iter())
            .filter(|(&k, _)| k != EMPTY_SLEEP)
            .map(|(&k, v)| (k, v))
    }

    /// Wake every sleeping member of the given island tags.
    fn wake_islands(&mut self, islands: &[u64]) {
        for (key, value) in self.keys.iter().zip(self.values.iter_mut()) {
            if *key != EMPTY_SLEEP && value.sleeping && islands.contains(&value.island) {
                value.sleeping = false;
                value.timer = 0.0;
            }
        }
    }

    /// Rewrite island tag `from` to `to` (sleeping-island merge).
    fn retag(&mut self, from: u64, to: u64) {
        for (key, value) in self.keys.iter().zip(self.values.iter_mut()) {
            if *key != EMPTY_SLEEP && value.sleeping && value.island == from {
                value.island = to;
            }
        }
    }

    fn sleeping_count(&self) -> usize {
        self.iter().filter(|(_, v)| v.sleeping).count()
    }
}

/// Union-find root with path halving; serial and deterministic (D-065).
fn uf_find(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        let grandparent = parent[parent[i as usize] as usize];
        parent[i as usize] = grandparent;
        i = grandparent;
    }
    i
}

/// Min-index root keeps island roots independent of union order.
fn uf_union(parent: &mut [u32], a: u32, b: u32) {
    let root_a = uf_find(parent, a);
    let root_b = uf_find(parent, b);
    if root_a != root_b {
        let (lo, hi) = if root_a < root_b {
            (root_a, root_b)
        } else {
            (root_b, root_a)
        };
        parent[hi as usize] = lo;
    }
}

/// Warm-start identity for entity proxies; tag bit 63 separates the entity
/// keyspace from tile hashes.
fn entity_key(entity: Entity) -> u64 {
    (1 << 63) | (u64::from(entity.generation & 0x7FFF_FFFF) << 32) | u64::from(entity.index)
}

/// Warm-start identity for tile proxies, hashed from the tile center; stable
/// while the tilemap does not move (a moved map only loses warm starts).
fn tile_key(center: Vec2) -> u64 {
    let h = u64::from(center.x.to_bits()).wrapping_mul(0x8DA6_B343_D816_3841)
        ^ u64::from(center.y.to_bits()).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let h = h ^ (h >> 31);
    h & !(1 << 63)
}

/// Order-independent pair key; the impulse is a scalar along the contact
/// normal, so orientation does not matter.
fn pair_key(a: u64, b: u64) -> u128 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    (u128::from(hi) << 64) | u128::from(lo)
}

/// Physics scratch buffers reused across substeps/frames.
#[derive(Debug, Default)]
pub struct PhysicsBuffers {
    proxies: Vec<Proxy>,
    pairs: Vec<(u32, u32)>,
    events: Vec<CollisionEvent>,
    candidates: Vec<ProxyId>,
    grid: SpatialGrid,
    contacts: Vec<ContactConstraint>,
    /// Read side of the warm-start map (previous substep or frame).
    impulses: ImpulseMap,
    /// Write side; swapped with `impulses` after each substep.
    impulses_next: ImpulseMap,
    loose_bodies: Vec<Entity>,
    /// Persistent per-body sleep state (D-065), keyed by entity key.
    sleep: SleepMap,
    /// Write side of the frame-start sleep-map rebuild; swapped into `sleep`.
    sleep_next: SleepMap,
    /// Union-find parent per proxy index; unions accumulate across substeps.
    island_parent: Vec<u32>,
    /// Scratch: minimum member sleep timer per island root.
    island_min_timer: Vec<f32>,
    /// Scratch: adopted sleeping-island tag per island root (0 = fresh).
    island_tag: Vec<u64>,
    /// Scratch: island tags scheduled to wake this frame.
    wake_islands: Vec<u64>,
}

impl PhysicsBuffers {
    /// Wake `entity`'s sleep island (D-065). The single public sleep surface:
    /// call after writing `Velocity`/`Position` from game code, though the
    /// step also detects such writes on sleeping bodies by itself. Waking an
    /// awake body just resets its sleep timer; unknown entities are a no-op.
    pub fn wake(&mut self, entity: Entity) {
        let key = entity_key(entity);
        let Some(entry) = self.sleep.get_mut(key) else {
            return;
        };
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

    /// Number of sleeping bodies (diagnostic).
    #[must_use]
    pub fn sleeping_count(&self) -> usize {
        self.sleep.sleeping_count()
    }
}

/// Wake `entity`'s sleep island (D-065); no-op when nothing has ever slept.
pub fn wake(world: &mut World, entity: Entity) {
    if let Some(buffers) = world.get_resource_mut::<PhysicsBuffers>() {
        buffers.wake(entity);
    }
}

/// Run one physics tick with fixed substeps (D-064).
pub fn physics_step(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, super::super::time::DeltaTime::seconds);
    if dt <= 0.0 {
        return;
    }

    let config = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default();

    let substeps = config.substeps.max(1);
    let sub_dt = dt / substeps as f32;

    // Remove/reinsert buffers to avoid resource borrow + entity borrow overlap.
    let mut buffers = world
        .remove_resource::<PhysicsBuffers>()
        .unwrap_or_default();

    integrate_loose_bodies(world, dt, config.gravity, &mut buffers.loose_bodies);

    build_broadphase(world, sub_dt, &config, &mut buffers);

    let sleep_enabled = config.sleep_threshold > 0.0;
    if sleep_enabled {
        // Gather leaves `sleeping = false` on every proxy; this flags the
        // proxies of persistent sleepers for the frame.
        sleep_frame_start(&mut buffers);
    }
    // Island union-find over this frame's touching contacts (D-065).
    buffers.island_parent.clear();
    buffers
        .island_parent
        .extend(0..buffers.proxies.len() as u32);
    buffers.events.clear();

    // Accumulated max per-substep travel since the last grid staging; once it
    // exceeds the half-cell query margin, staged cells may be stale.
    let mut drift = 0.0f32;
    for _ in 0..substeps {
        substep(&config, sub_dt, &mut buffers, &mut drift, sleep_enabled);
    }

    write_back(world, &buffers.proxies);

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

    world.insert_resource(buffers);
}

/// Write end-of-frame state back to the `World`, once per frame (D-066).
/// Sleeping bodies skip writeback so their components stay bit-frozen — the
/// invariant external-write detection relies on (D-065). The mutable query
/// iterates the exact archetype/row order `gather_proxies` used, so entity
/// proxies zip positionally with the query rows.
fn write_back(world: &mut World, proxies: &[Proxy]) {
    let mut rows = proxies.iter();
    for (entity, _collider, position, _body, velocity) in
        world.query2_opt2_mut::<Collider, Position, RigidBody, Velocity>()
    {
        let proxy = rows
            .next()
            .expect("gather and writeback queries must agree on entity count");
        debug_assert_eq!(proxy.entity, Some(entity), "proxy order diverged");
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        position.0 = proxy.center - proxy.offset;
        if let Some(vel) = velocity {
            vel.0 = proxy.velocity;
        }
    }
}

/// Frame-start sleep bookkeeping (D-065): rebuild the persistent sleep map
/// from live proxies, waking islands whose members despawned (or lost their
/// dynamic body/collider) and islands with an external write — a sleeping
/// body's `Position`/`Velocity` are frozen by the step, so any mismatch at
/// gather time is an external write.
fn sleep_frame_start(buffers: &mut PhysicsBuffers) {
    let PhysicsBuffers {
        proxies,
        sleep,
        sleep_next,
        wake_islands,
        ..
    } = buffers;

    wake_islands.clear();
    sleep_next.reset(proxies.len());
    for proxy in proxies.iter() {
        if proxy.entity.is_none() || !proxy.is_dynamic {
            continue;
        }
        let entry = match sleep.get(proxy.key) {
            Some(&entry) => {
                if entry.sleeping && (proxy.velocity != Vec2::ZERO || proxy.center != entry.center)
                {
                    wake_islands.push(entry.island);
                }
                entry
            }
            None => SleepEntry::default(),
        };
        sleep_next.insert(proxy.key, entry);
    }
    // Sleeping entries that did not carry over (despawn or component
    // removal) wake their island: bodies above a removed support must fall.
    for (key, entry) in sleep.iter() {
        if entry.sleeping && sleep_next.get(key).is_none() {
            wake_islands.push(entry.island);
        }
    }
    std::mem::swap(sleep, sleep_next);
    if !wake_islands.is_empty() {
        sleep.wake_islands(wake_islands);
    }

    // Proxies persist across the frame's substeps (D-066), so the flag set
    // here carries through until a contact wake flips it.
    for proxy in proxies.iter_mut() {
        proxy.sleeping = proxy.is_dynamic
            && proxy.entity.is_some()
            && sleep.get(proxy.key).is_some_and(|entry| entry.sleeping);
    }
}

/// Frame-end sleep pass (D-065): update per-body low-velocity timers from
/// end-of-frame velocities, then sleep every island whose slowest member has
/// been below the threshold for `time_to_sleep`. Members freeze with zeroed
/// velocity. An island falling asleep while resting on an already sleeping
/// island adopts that island's tag, so a later island wake (despawn deep in a
/// pile) also lifts bodies that settled on top after it slept; bridging two
/// sleeping islands merges their tags.
fn sleep_frame_end(
    world: &mut World,
    config: &PhysicsConfig,
    dt: f32,
    buffers: &mut PhysicsBuffers,
) {
    let PhysicsBuffers {
        proxies,
        contacts,
        sleep,
        island_parent,
        island_min_timer,
        island_tag,
        ..
    } = buffers;

    let threshold_sq = config.sleep_threshold * config.sleep_threshold;
    island_min_timer.clear();
    island_min_timer.resize(proxies.len(), f32::INFINITY);
    island_tag.clear();
    island_tag.resize(proxies.len(), 0);

    for (index, proxy) in proxies.iter().enumerate() {
        if !proxy.is_dynamic || proxy.sleeping || proxy.entity.is_none() {
            continue;
        }
        let Some(entry) = sleep.get_mut(proxy.key) else {
            continue;
        };
        entry.timer = if proxy.velocity.length_squared() <= threshold_sq {
            entry.timer + dt
        } else {
            0.0
        };
        let root = uf_find(island_parent, index as u32) as usize;
        island_min_timer[root] = island_min_timer[root].min(entry.timer);
    }

    // Tag adoption from the final substep's resting contacts vs sleepers.
    for contact in contacts.iter() {
        if contact.penetration <= 0.0 {
            continue;
        }
        let b_idx = contact.b as usize;
        if !proxies[b_idx].sleeping || proxies[b_idx].entity.is_none() {
            continue;
        }
        let root = uf_find(island_parent, contact.a) as usize;
        if island_min_timer[root] < config.time_to_sleep {
            continue;
        }
        let Some(tag) = sleep.get(proxies[b_idx].key).map(|entry| entry.island) else {
            continue;
        };
        let current = island_tag[root];
        if current == 0 {
            island_tag[root] = tag;
        } else if current != tag {
            sleep.retag(tag, current);
            for slot in island_tag.iter_mut() {
                if *slot == tag {
                    *slot = current;
                }
            }
        }
    }

    for index in 0..proxies.len() {
        let proxy = proxies[index];
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        let Some(entity) = proxy.entity else { continue };
        let root = uf_find(island_parent, index as u32) as usize;
        if island_min_timer[root] < config.time_to_sleep {
            continue;
        }
        let tag = if island_tag[root] != 0 {
            island_tag[root]
        } else {
            proxies[root].key
        };
        if let Some(entry) = sleep.get_mut(proxy.key) {
            entry.sleeping = true;
            entry.island = tag;
            // Same fp ops the next gather performs on the written-back
            // Position, so an untouched body compares bit-equal.
            entry.center = (proxy.center - proxy.offset) + proxy.offset;
        }
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 = Vec2::ZERO;
        }
    }
}

/// Dynamic bodies without a collider never enter the solver; integrate them
/// once per frame (old behavior integrated them per substep — for pure
/// ballistic motion the trajectories differ only at O(g·dt²)).
fn integrate_loose_bodies(world: &mut World, dt: f32, gravity: Vec2, scratch: &mut Vec<Entity>) {
    scratch.clear();
    for (entity, _velocity, _position, body) in world.query3::<Velocity, Position, RigidBody>() {
        if body.kind == BodyKind::Dynamic && world.get::<Collider>(entity).is_none() {
            scratch.push(entity);
        }
    }
    for &entity in scratch.iter() {
        let Some(vel) = world.get_mut::<Velocity>(entity) else {
            continue;
        };
        vel.0 += gravity * dt;
        let new_vel = vel.0;
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 += new_vel * dt;
        }
    }
}

/// Build the frame's broadphase from pre-integration proxies.
fn build_broadphase(
    world: &World,
    sub_dt: f32,
    config: &PhysicsConfig,
    buffers: &mut PhysicsBuffers,
) {
    let PhysicsBuffers { proxies, grid, .. } = buffers;

    // Preserve grid allocations unless cell size changes.
    if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
        grid.set_cell_size(config.broadphase_cell_size);
    }

    gather_proxies(world, proxies);
    stage_grid(proxies, grid, sub_dt);
}

/// Restage the grid from current proxies. Dynamic AABBs inflate symmetrically
/// by one substep of travel; the query-side half-cell margin absorbs further
/// drift until the caller restages.
fn stage_grid(proxies: &[Proxy], grid: &mut SpatialGrid, sub_dt: f32) {
    grid.clear();
    for (id, proxy) in proxies.iter().enumerate() {
        let mut aabb = proxy.world_aabb();
        if proxy.is_dynamic {
            aabb.half_extents += Vec2::splat(proxy.velocity.length() * sub_dt);
        }
        grid.insert(id as ProxyId, &aabb);
    }
}

/// Gather entity + tilemap proxies in deterministic `World` order, once per
/// frame (D-066); the gather runs before any integration, so
/// `prev_center == center` at gather time.
fn gather_proxies(world: &World, proxies: &mut Vec<Proxy>) {
    proxies.clear();

    // Columnar 4-way gather: RigidBody/Velocity columns resolve once per
    // archetype — no per-entity random lookups (D-066).
    for (entity, collider, position, body, velocity) in
        world.query2_opt2::<Collider, Position, RigidBody, Velocity>()
    {
        let has_velocity = velocity.is_some();
        let velocity = velocity.map_or(Vec2::ZERO, |v| v.0);
        let body = body.copied();
        let (is_dynamic, inv_mass, restitution) = match body {
            Some(b) => (
                b.kind == BodyKind::Dynamic,
                if b.kind == BodyKind::Dynamic {
                    b.inv_mass
                } else {
                    0.0
                },
                b.restitution,
            ),
            None => (false, 0.0, 0.0),
        };
        let center = position.0 + collider.offset;
        proxies.push(Proxy {
            entity: Some(entity),
            key: entity_key(entity),
            center,
            prev_center: center,
            velocity,
            offset: collider.offset,
            shape: collider.shape,
            is_dynamic,
            has_velocity,
            inv_mass,
            restitution,
            face_mask: FACE_ALL,
            sleeping: false,
        });
    }

    gather_tilemap_proxies(world, proxies);
}

/// One solver substep: contacts once, warm-started soft solve, integrate,
/// relax, restitution, speculative safety net. Runs entirely on the dense
/// proxy arrays — no `World` access (D-066).
fn substep(
    config: &PhysicsConfig,
    sub_dt: f32,
    buffers: &mut PhysicsBuffers,
    drift: &mut f32,
    sleep_enabled: bool,
) {
    let PhysicsBuffers {
        proxies,
        pairs,
        events,
        candidates,
        grid,
        contacts,
        impulses,
        impulses_next,
        sleep,
        island_parent,
        ..
    } = buffers;
    pairs.clear();
    candidates.clear();
    contacts.clear();

    // Staleness budget: previous substeps moved bodies; once accumulated
    // movement can exceed the query margin, restage the grid from current
    // positions so staged cells stay conservative.
    let margin = config.broadphase_cell_size * 0.5;
    if *drift > margin {
        stage_grid(proxies, grid, sub_dt);
        *drift = 0.0;
    }

    // Pair query: current + predicted AABB + half-cell margin covers this
    // substep's travel and residual staging drift. Sleeping bodies never
    // initiate — sleeping-sleeping pairs skip the narrow phase entirely
    // (D-065); they remain staged in the grid as wake targets.
    let query_margin = Vec2::splat(config.broadphase_cell_size * 0.5);
    for a_idx in 0..proxies.len() {
        if !proxies[a_idx].is_dynamic || proxies[a_idx].sleeping {
            continue;
        }
        let mut a_aabb = proxies[a_idx].travel_aabb(sub_dt);
        a_aabb.half_extents += query_margin;
        grid.query(&a_aabb, Some(a_idx as ProxyId), candidates);
        for &b_id in candidates.iter() {
            let b_idx = b_id as usize;
            // Awake dynamic pairs once; static and sleeping targets are
            // already unique per initiating query.
            if proxies[b_idx].is_dynamic && !proxies[b_idx].sleeping && b_idx <= a_idx {
                continue;
            }
            // Grid cells are coarse; only actually-overlapping inflated AABBs pair.
            if !a_aabb.overlaps(&proxies[b_idx].travel_aabb(sub_dt)) {
                continue;
            }
            pairs.push((a_idx as u32, b_idx as u32));
        }
    }

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
    for &(a_u, b_u) in pairs.iter() {
        let a_idx = a_u as usize;
        let b_idx = b_u as usize;
        let margin = (proxies[a_idx].velocity - proxies[b_idx].velocity).length() * sub_dt
            + speculative_slack;
        let Some(contact) = narrow_phase(&proxies[a_idx], &proxies[b_idx], margin) else {
            continue;
        };
        // Wake test (D-065): a touching contact with real relative motion, or
        // a speculative gap approached faster than the threshold, wakes the
        // sleeping side before the solver sees the contact — a bullet wakes
        // its impact fringe while still a gap away. Resting-speed contacts
        // leave the sleeper frozen (immovable below), so wake waves damp out
        // where motion dies instead of sweeping the whole pile.
        if proxies[b_idx].sleeping {
            debug_assert!(!proxies[a_idx].sleeping, "sleeping bodies never initiate");
            let relative = proxies[a_idx].velocity - proxies[b_idx].velocity;
            let wakes = if contact.penetration > 0.0 {
                relative.length_squared() > wake_threshold * wake_threshold
            } else {
                relative.dot(contact.normal) < -wake_threshold
            };
            if wakes {
                proxies[b_idx].sleeping = false;
                if let Some(entry) = sleep.get_mut(proxies[b_idx].key) {
                    entry.sleeping = false;
                    entry.timer = 0.0;
                }
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
        contacts.push(ContactConstraint {
            a: a_u,
            b: b_u,
            normal: contact.normal,
            penetration: contact.penetration,
            normal_mass: 1.0 / inv_mass_sum,
            approach,
            impulse: impulses.get(key),
            restitution: proxies[a_idx].restitution.max(proxies[b_idx].restitution),
            vs_static: inv_a == 0.0 || inv_b == 0.0,
            inv_a,
            inv_b,
            key,
        });
        // Touching contacts between awake dynamic bodies join one island;
        // islands sleep and re-sleep as a unit (D-065).
        if sleep_enabled
            && contact.penetration > 0.0
            && proxies[b_idx].is_dynamic
            && !proxies[b_idx].sleeping
            && proxies[b_idx].entity.is_some()
        {
            uf_union(island_parent, a_u, b_u);
        }
        if contact.penetration > 0.0 {
            if let Some(a_entity) = proxies[a_idx].entity {
                events.push(CollisionEvent {
                    a: a_entity,
                    b: proxies[b_idx].entity,
                    normal: contact.normal,
                    penetration: contact.penetration,
                });
            }
        }
    }

    // Integrate velocities (gravity), then warm start from stored impulses.
    for proxy in proxies.iter_mut() {
        if proxy.is_dynamic && proxy.has_velocity && !proxy.sleeping {
            proxy.velocity += config.gravity * sub_dt;
        }
    }
    for contact in contacts.iter() {
        if contact.impulse > 0.0 {
            apply_impulse(proxies, contact, contact.normal * contact.impulse);
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
    }

    // Position integration converts the bias velocity into depenetration.
    let mut max_travel = 0.0f32;
    for proxy in proxies.iter_mut() {
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        proxy.prev_center = proxy.center;
        let travel = proxy.velocity * sub_dt;
        proxy.center += travel;
        max_travel = max_travel.max(travel.length());
    }
    *drift += max_travel;

    // One bias-free relax iteration strips the injected bias energy.
    solve_contacts(proxies, contacts, config, soft, soft_static, inv_h, false);

    apply_restitution(proxies, contacts, config.restitution_threshold);

    // Persist accumulated impulses for the next substep/frame; rebuilding
    // from live contacts ages out stale pairs for free.
    impulses_next.reset(contacts.len());
    for contact in contacts.iter() {
        if contact.impulse > 0.0 {
            impulses_next.insert(contact.key, contact.impulse);
        }
    }
    std::mem::swap(impulses, impulses_next);

    // Safety net: clamp extreme dynamics to first static sweep hit (covers
    // solver-injected velocity a speculative pair admission never saw).
    // Events keep accumulating across substeps; the frame drains them once.
    speculative_pass(proxies, grid, candidates, events);
}

/// Apply a contact impulse through the inverse masses captured at contact
/// build; statics and sides asleep at build time carry 0 and never move.
fn apply_impulse(proxies: &mut [Proxy], contact: &ContactConstraint, impulse: Vec2) {
    proxies[contact.a as usize].velocity += impulse * contact.inv_a;
    proxies[contact.b as usize].velocity -= impulse * contact.inv_b;
}

/// One Gauss-Seidel pass over the contact buffer. With `use_bias`, deep
/// contacts push out at the soft-constraint rate (capped by max push speed);
/// contacts inside the slop band limit approach instead. The bias-free relax
/// pass reuses the same accumulated impulses.
fn solve_contacts(
    proxies: &mut [Proxy],
    contacts: &mut [ContactConstraint],
    config: &PhysicsConfig,
    soft: Softness,
    soft_static: Softness,
    inv_h: f32,
    use_bias: bool,
) {
    for contact in contacts.iter_mut() {
        let a = contact.a as usize;
        let b = contact.b as usize;
        let vn = (proxies[a].velocity - proxies[b].velocity).dot(contact.normal);

        // Separation relative to the slop band; negative = must push out.
        let s = config.linear_slop - contact.penetration;
        let (bias, mass_scale, impulse_scale) = if s > 0.0 {
            // Within slop: allow approach to close the band, never push.
            (s * inv_h, 1.0, 0.0)
        } else if use_bias {
            let softness = if contact.vs_static { soft_static } else { soft };
            (
                (softness.bias_rate * s).max(-config.max_push_speed),
                softness.mass_scale,
                softness.impulse_scale,
            )
        } else {
            (0.0, 1.0, 0.0)
        };

        let raw = -contact.normal_mass * mass_scale * (vn + bias) - impulse_scale * contact.impulse;
        let new_impulse = (contact.impulse + raw).max(0.0);
        let delta = new_impulse - contact.impulse;
        contact.impulse = new_impulse;
        if delta != 0.0 {
            apply_impulse(proxies, contact, contact.normal * delta);
        }
    }
}

/// Restitution from the approach velocity stored at contact build; contacts
/// slower than the threshold stay inelastic so piles can settle.
fn apply_restitution(proxies: &mut [Proxy], contacts: &mut [ContactConstraint], threshold: f32) {
    for contact in contacts.iter_mut() {
        if contact.restitution == 0.0 || contact.approach > -threshold || contact.impulse <= 0.0 {
            continue;
        }
        let a = contact.a as usize;
        let b = contact.b as usize;
        let vn = (proxies[a].velocity - proxies[b].velocity).dot(contact.normal);
        let raw = -contact.normal_mass * (vn + contact.restitution * contact.approach);
        let new_impulse = (contact.impulse + raw).max(0.0);
        let delta = new_impulse - contact.impulse;
        contact.impulse = new_impulse;
        if delta != 0.0 {
            apply_impulse(proxies, contact, contact.normal * delta);
        }
    }
}

/// Safety-net slab sweep (D-064): clamp fast dynamics to the first static
/// sweep hit. Speculative contacts are the primary CCD; this pass only
/// catches trajectories the pair admission never saw — velocity injected by
/// the solver after contacts were built (impulse chains, deep-recovery bias).
/// Static circles promote conservatively to their bounding squares.
fn speculative_pass(
    proxies: &mut [Proxy],
    grid: &mut SpatialGrid,
    candidates: &mut Vec<ProxyId>,
    events: &mut Vec<CollisionEvent>,
) {
    const BACKOFF: f32 = 1.0e-3;

    for a_idx in 0..proxies.len() {
        let proxy = &proxies[a_idx];
        if !proxy.is_dynamic {
            continue;
        }
        let min_extent = proxy.min_half_extent();
        if min_extent <= 0.0 {
            continue;
        }
        let travel_sq = (proxy.center - proxy.prev_center).length_squared();
        // Same threshold as substep picker, squared.
        if travel_sq <= min_extent * min_extent {
            continue;
        }

        let swept = proxy.swept_aabb();
        grid.query(&swept, Some(a_idx as ProxyId), candidates);

        let a_prev = proxy.prev_center;
        let a_cur = proxy.center;
        let a_half = proxy.half_extents();

        let mut best: Option<(f32, Vec2, usize)> = None;
        for &b_id in candidates.iter() {
            let b_idx = b_id as usize;
            let target = &proxies[b_idx];
            if target.is_dynamic {
                continue;
            }
            let b_half = match target.shape {
                Shape::Aabb { half_extents } => half_extents,
                // Conservative circle->AABB promotion: the bounding square
                // can clamp a diagonal pass slightly early; acceptable for a
                // last-resort net.
                Shape::Circle { radius } => Vec2::splat(radius),
            };
            if let Some((t, n)) = sweep_aabb_vs_aabb(a_prev, a_cur, a_half, target.center, b_half) {
                // Skip hits on internal tile faces.
                let face_bit = if n.x < 0.0 {
                    FACE_LEFT
                } else if n.x > 0.0 {
                    FACE_RIGHT
                } else if n.y < 0.0 {
                    FACE_TOP
                } else {
                    FACE_BOTTOM
                };
                if target.face_mask & face_bit == 0 {
                    continue;
                }
                if best.is_none_or(|(best_t, _, _)| t < best_t) {
                    best = Some((t, n, b_idx));
                }
            }
        }

        let Some((t_hit, normal, b_idx)) = best else {
            continue;
        };
        let t_safe = (t_hit - BACKOFF).max(0.0);
        let clamped = a_prev + (a_cur - a_prev) * t_safe;
        proxies[a_idx].center = clamped;

        // Reflect closing velocity along contact normal.
        let v = proxies[a_idx].velocity;
        let v_along_normal = v.dot(normal);
        if v_along_normal < 0.0 {
            let restitution = proxies[a_idx].restitution;
            let impulse = -(1.0 + restitution) * v_along_normal;
            proxies[a_idx].velocity = v + normal * impulse;
        }

        if let Some(a_entity) = proxies[a_idx].entity {
            events.push(CollisionEvent {
                a: a_entity,
                b: proxies[b_idx].entity,
                normal,
                penetration: 0.0,
            });
        }
    }
}

/// Signed-distance narrow phase (D-064): separated pairs with a gap under
/// `margin` return contacts with negative penetration for speculative CCD.
fn narrow_phase(a: &Proxy, b: &Proxy, margin: f32) -> Option<Contact> {
    match (a.shape, b.shape) {
        (
            Shape::Aabb {
                half_extents: ha, ..
            },
            Shape::Aabb {
                half_extents: hb, ..
            },
        ) => aabb_vs_aabb_speculative(
            &Aabb::new(a.center, ha),
            &Aabb::new(b.center, hb),
            b.face_mask,
            margin,
        ),
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            circle_vs_circle_speculative(a.center, ra, b.center, rb, margin)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => aabb_vs_circle_speculative(
            &Aabb::new(a.center, half_extents),
            b.center,
            radius,
            a.face_mask,
            margin,
        ),
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            // Helper returns AABB escape normal; circle needs the opposite.
            aabb_vs_circle_speculative(
                &Aabb::new(b.center, half_extents),
                a.center,
                radius,
                b.face_mask,
                margin,
            )
            .map(|c| Contact {
                normal: -c.normal,
                penetration: c.penetration,
            })
        }
    }
}

/// Emit transient static tile proxies for collision layers.
fn gather_tilemap_proxies(world: &World, proxies: &mut Vec<Proxy>) {
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return;
    };

    for (_entity, instance) in world.query::<TilemapInstance>() {
        let Some(data) = registry.get(&instance.id) else {
            continue;
        };
        let tw = data.tile_width as f32;
        let th = data.tile_height as f32;
        let half = Vec2::new(tw * 0.5, th * 0.5);

        for layer in &data.layers {
            if layer.kind != LayerKind::Collision {
                continue;
            }
            let w = data.width as i32;
            let h = data.height as i32;
            let is_solid = |c: i32, r: i32| -> bool {
                if c < 0 || r < 0 || c >= w || r >= h {
                    return false;
                }
                let idx = (r as usize) * (data.width as usize) + (c as usize);
                layer.tiles[idx] >= 0
            };
            for row in 0..data.height {
                for col in 0..data.width {
                    let idx = (row as usize) * (data.width as usize) + (col as usize);
                    let tile = layer.tiles[idx];
                    if tile < 0 {
                        continue;
                    }
                    let c = col as i32;
                    let r = row as i32;
                    // Solid neighbors mask internal faces; map edges remain exposed.
                    let mut face_mask: u8 = 0;
                    if !is_solid(c, r - 1) {
                        face_mask |= FACE_TOP;
                    }
                    if !is_solid(c, r + 1) {
                        face_mask |= FACE_BOTTOM;
                    }
                    if !is_solid(c - 1, r) {
                        face_mask |= FACE_LEFT;
                    }
                    if !is_solid(c + 1, r) {
                        face_mask |= FACE_RIGHT;
                    }
                    let center = Vec2::new(
                        instance.origin.x + (col as f32) * tw + half.x,
                        instance.origin.y + (row as f32) * th + half.y,
                    );
                    proxies.push(Proxy {
                        entity: None,
                        key: tile_key(center),
                        center,
                        prev_center: center,
                        velocity: Vec2::ZERO,
                        offset: Vec2::ZERO,
                        shape: Shape::Aabb { half_extents: half },
                        is_dynamic: false,
                        has_velocity: false,
                        inv_mass: 0.0,
                        restitution: 0.0,
                        face_mask,
                        sleeping: false,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/physics/step.rs"]
mod tests;
