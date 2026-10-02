//! Physics step: build broadphase -> fixed substeps (speculative contacts ->
//! soft solve -> integrate -> arrival pass) -> events.
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
//! The whole step is serial and deterministic by design (D-067): a
//! color-parallel contact solver was built and measured for step 6 of
//! `docs/plans/physics-scale-and-ccd.md` and dropped — at 25k awake bodies
//! the contact-solve passes are ~2% of the frame (the cost is pair query +
//! narrow phase + contact build), so threading the solver cannot move the
//! number and the coloring/sort machinery alone cost the serial path ~18%.

use super::PhysicsConfig;
use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::{
    Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP,
    aabb_vs_aabb_speculative, aabb_vs_circle_speculative, circle_vs_circle_speculative,
    sweep_aabb_vs_aabb,
};
use super::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use super::events::CollisionEvent;
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
/// Synchronized from live contacts at frame end and before pair rebuilds;
/// never `std::HashMap` in the physics hot path (D-062 precedent).
#[derive(Debug, Default, Clone)]
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

/// Key of a proxy without sleep state, and the empty index slot; entity keys
/// always set bit 63 (`entity_key`), so 0 is never a real key.
const EMPTY_SLEEP: u64 = 0;

/// Per-body sleep state in arrays parallel to the proxy array (D-082).
///
/// While a frame's proxies are last frame's bodies in last frame's order,
/// every entry already sits at its proxy's index and nothing is rebuilt.
/// When the sequence differs (a spawn, a despawn, a body or collider change),
/// entries carry over by entity key and sleepers that did not carry over wake
/// their island. Never `std::HashMap` in the physics hot path (D-062
/// precedent).
#[derive(Debug, Default)]
struct SleepTable {
    /// Entity key of each dynamic entity proxy; `EMPTY_SLEEP` for the rest.
    keys: Vec<u64>,
    entries: Vec<SleepEntry>,
    /// Open-addressing slots from entity key to index in the arrays above;
    /// refilled only when the key sequence changes.
    slot_keys: Vec<u64>,
    slot_indices: Vec<u32>,
    mask: u64,
    /// Write side of a rebuild; swapped into `keys` and `entries`.
    next_keys: Vec<u64>,
    next_entries: Vec<SleepEntry>,
    /// Scratch: old entries a rebuild carried over.
    carried: Vec<bool>,
    #[cfg(test)]
    rebuilds: usize,
}

impl SleepTable {
    /// The key a proxy's entry lives under: only dynamic entity bodies sleep.
    fn key_of(proxy: &Proxy) -> u64 {
        if proxy.is_dynamic && proxy.entity.is_some() {
            proxy.key
        } else {
            EMPTY_SLEEP
        }
    }

    fn index_of(&self, key: u64) -> Option<usize> {
        if self.slot_keys.is_empty() {
            return None;
        }
        let mut i = (hash_key(key) & self.mask) as usize;
        loop {
            if self.slot_keys[i] == EMPTY_SLEEP {
                return None;
            }
            if self.slot_keys[i] == key {
                return Some(self.slot_indices[i] as usize);
            }
            i = (i + 1) & self.mask as usize;
        }
    }

    fn get(&self, key: u64) -> Option<&SleepEntry> {
        self.index_of(key).map(|i| &self.entries[i])
    }

    /// True when `proxies` are last frame's bodies in last frame's order, so
    /// the entries need no rebuild. Collects the islands of sleepers with an
    /// external write (a sleeper's `Position`/`Velocity` are frozen by the
    /// step, so any mismatch at gather time is one).
    fn same_bodies(&self, proxies: &[Proxy], wake_islands: &mut Vec<u64>) -> bool {
        if proxies.len() != self.keys.len() {
            return false;
        }
        for ((proxy, &key), entry) in proxies.iter().zip(&self.keys).zip(&self.entries) {
            if Self::key_of(proxy) != key {
                return false;
            }
            if entry.sleeping && (proxy.velocity != Vec2::ZERO || proxy.center != entry.center) {
                wake_islands.push(entry.island);
            }
        }
        true
    }

    /// Carry entries over by entity key into the order of `proxies`. Sleeping
    /// entries that do not carry over (despawn or component removal) wake
    /// their island: bodies above a removed support must fall.
    fn rebuild(&mut self, proxies: &[Proxy], wake_islands: &mut Vec<u64>) {
        self.next_keys.clear();
        self.next_entries.clear();
        self.carried.clear();
        self.carried.resize(self.entries.len(), false);
        let mut members = 0_usize;
        for proxy in proxies {
            let key = Self::key_of(proxy);
            let mut entry = SleepEntry::default();
            if key != EMPTY_SLEEP {
                members += 1;
                if let Some(old) = self.index_of(key) {
                    self.carried[old] = true;
                    entry = self.entries[old];
                    if entry.sleeping
                        && (proxy.velocity != Vec2::ZERO || proxy.center != entry.center)
                    {
                        wake_islands.push(entry.island);
                    }
                }
            }
            self.next_keys.push(key);
            self.next_entries.push(entry);
        }
        for (entry, &carried) in self.entries.iter().zip(&self.carried) {
            if entry.sleeping && !carried {
                wake_islands.push(entry.island);
            }
        }
        std::mem::swap(&mut self.keys, &mut self.next_keys);
        std::mem::swap(&mut self.entries, &mut self.next_entries);

        // Refill the index at <= 0.5 load factor.
        let capacity = (members * 2).next_power_of_two().max(64);
        self.mask = capacity as u64 - 1;
        self.slot_keys.clear();
        self.slot_keys.resize(capacity, EMPTY_SLEEP);
        self.slot_indices.clear();
        self.slot_indices.resize(capacity, 0);
        for (index, &key) in self.keys.iter().enumerate() {
            if key == EMPTY_SLEEP {
                continue;
            }
            let mut i = (hash_key(key) & self.mask) as usize;
            while self.slot_keys[i] != EMPTY_SLEEP {
                i = (i + 1) & self.mask as usize;
            }
            self.slot_keys[i] = key;
            self.slot_indices[i] = index as u32;
        }
        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
    }

    /// Wake every sleeping member of the given island tags.
    fn wake_islands(&mut self, islands: &[u64]) {
        for entry in &mut self.entries {
            if entry.sleeping && islands.contains(&entry.island) {
                entry.sleeping = false;
                entry.timer = 0.0;
            }
        }
    }

    /// Rewrite island tag `from` to `to` (sleeping-island merge).
    fn retag(&mut self, from: u64, to: u64) {
        for entry in &mut self.entries {
            if entry.sleeping && entry.island == from {
                entry.island = to;
            }
        }
    }

    fn sleeping_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.sleeping).count()
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

/// Conservative displacement allowance since the proxy's most recent pair
/// build or repair.
#[derive(Debug, Clone, Copy)]
struct PairBudget {
    radius: f32,
    travel: f32,
}

/// Margin on every awake body's travel budget, in units of `linear_slop`
/// (D-081). It absorbs small velocity changes, and the rounding that would
/// otherwise decide the check for a body moving exactly as predicted.
const PAIR_MARGIN_SLOPS: f32 = 2.0;

/// A repair is cheaper than a rebuild only for a minority: more than one
/// tripped proxy in this many awake bodies rebuilds instead (D-081).
const REBUILD_ONE_IN: usize = 4;

/// `PhysicsBuffers::pair_flags` bit: awake dynamic body at the last build.
const PAIR_AWAKE: u8 = 1;
/// `pair_flags` bit: member of the repair in progress.
const PAIR_TRIPPED: u8 = 1 << 1;
/// `pair_flags` bit: repaired since the last build, so the pair grid still
/// holds the cells of its old AABB.
const PAIR_REPAIRED: u8 = 1 << 2;

/// Travel gravity can add beyond `|v|·time_left` before the frame's last
/// budget check: `|g|·h²·(n−1)(n+2)/2` with `n = time_left / h` substeps
/// left (D-081). The integrator adds `g·h` to the velocity before it moves a
/// body, so the last check sees `n − 1` substeps of travel plus one more of
/// predicted travel at the speed gravity has built up by then.
fn gravity_allowance(config: &PhysicsConfig, sub_dt: f32, time_left: f32) -> f32 {
    0.5 * config.gravity.length() * (time_left - sub_dt).max(0.0) * (time_left + 2.0 * sub_dt)
}

/// Pair inflation of an awake dynamic body moving at `speed` (D-081).
fn awake_pair_radius(
    config: &PhysicsConfig,
    speed: f32,
    time_left: f32,
    gravity_allowance: f32,
) -> f32 {
    speed * time_left + gravity_allowance + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop
}

/// Travel change per substep, in units of `linear_slop`, above which a body's
/// solved velocity counts as one the narrow phase did not admit (D-092). Two
/// bodies under it change a pair's closing travel by at most `4·linear_slop`,
/// the admission slack of D-064.
const ARRIVAL_SLOPS: f32 = 2.0;

/// Rounds of the arrival pass per substep, and levels of push depth.
const ARRIVAL_ROUNDS: usize = 4;

/// Closing travel per substep, in px, within which an arrival pair counts as
/// tight: its mover is pressed on its target.
const ARRIVAL_TIGHT: f32 = 1.0e-3;

/// `ArrivalScratch::state` bit: the proxy is on the worklist this substep.
const ARRIVAL_LISTED: u8 = 1;
/// `state` bit: the proxy is pressed on another one this substep.
const ARRIVAL_PRESSED: u8 = 1 << 1;
/// `state` bit: the pass changed the proxy's velocity, so its move is redone.
const ARRIVAL_MOVED: u8 = 1 << 2;

/// A listed body and one neighbour its new velocity reaches (D-092).
#[derive(Debug, Clone, Copy)]
struct ArrivalPair {
    x: u32,
    y: u32,
    /// Points from `y` to `x`.
    normal: Vec2,
    /// Closing speed that brings the pair to touching within the substep; 0
    /// once it overlaps by more than the slop.
    allowance: f32,
    /// Push depth of the pair's mover.
    depth: u8,
    /// Inverse mass of the pair's target; 0 for a static or sleeping one.
    target_inv: f32,
}

impl ArrivalPair {
    /// Closing speed beyond the allowance; negative when the pair closes too
    /// fast.
    fn closing(&self, proxies: &[Proxy]) -> f32 {
        (proxies[self.x as usize].velocity - proxies[self.y as usize].velocity).dot(self.normal)
            + self.allowance
    }

    /// The pair's mover, the member whose own velocity closes it faster, its
    /// target, and the direction the mover travels to close it.
    fn mover(&self, proxies: &[Proxy]) -> (usize, usize, Vec2) {
        let toward_y = -proxies[self.x as usize].velocity.dot(self.normal);
        let toward_x = proxies[self.y as usize].velocity.dot(self.normal);
        if toward_x > toward_y {
            (self.y as usize, self.x as usize, self.normal)
        } else {
            (self.x as usize, self.y as usize, -self.normal)
        }
    }
}

/// Scratch of the arrival pass (D-092), kept between substeps and frames.
/// The per-proxy vectors are parallel to the proxies.
#[derive(Debug, Default)]
struct ArrivalScratch {
    /// Velocity each awake dynamic proxy had when the narrow phase ran.
    admitted: Vec<Vec2>,
    /// Proxies whose velocity the solve changed, in proxy order, then the ones
    /// a clamp pushed, in the order they joined.
    list: Vec<u32>,
    /// A listed proxy's position in `list`.
    slot: Vec<u32>,
    pairs: Vec<ArrivalPair>,
    /// `ARRIVAL_*` bits; all zero outside the pass.
    state: Vec<u8>,
    /// Proxies with a nonzero `state`, so the reset touches only them.
    touched: Vec<u32>,
    /// Push depth, valid for the members of `pairs`.
    depth: Vec<u8>,
    /// For a pressed proxy: the direction it is pressed in, the proxy it is
    /// pressed on, and the inverse mass of the chain it heads (0 when the
    /// chain ends at a static or sleeping body).
    pressed_dir: Vec<Vec2>,
    pressed_on: Vec<u32>,
    chain_inv: Vec<f32>,
    /// Bodies listed and clamps applied since the buffers were created.
    #[cfg(test)]
    listed: usize,
    #[cfg(test)]
    clamps: usize,
}

impl ArrivalScratch {
    /// Size the per-proxy vectors for `len` proxies.
    fn fit(&mut self, len: usize) {
        if self.admitted.len() != len {
            self.admitted.resize(len, Vec2::ZERO);
            self.slot.resize(len, 0);
            self.state.resize(len, 0);
            self.depth.resize(len, 0);
            self.pressed_dir.resize(len, Vec2::ZERO);
            self.pressed_on.resize(len, 0);
            self.chain_inv.resize(len, 0.0);
        }
    }

    /// Set `bit` on `index`, remembering the proxy for the reset.
    fn mark(state: &mut [u8], touched: &mut Vec<u32>, index: usize, bit: u8) {
        if state[index] == 0 {
            touched.push(index as u32);
        }
        state[index] |= bit;
    }
}

/// Statics-only grid for the safety-net sweep (D-080).
///
/// It returns the static candidates a query of the pair grid returns, in the
/// same order, so which grid answers a sweep query is a matter of cost alone.
/// Staging one static costs about as much as one query of the pair grid, so
/// the sweep rents the pair grid until the frame's sweep queries reach the
/// static count and then stages this grid for the rest of the frame. A tile
/// map with a few fast bodies never stages it; thousands of fast bodies
/// between a few walls stage it after a handful of queries.
#[derive(Debug, Default)]
struct SweepGrid {
    grid: SpatialGrid,
    staged: bool,
    /// Sweep queries the pair grid answered this frame.
    pair_queries: usize,
    /// Static proxies, counted by every pair build.
    statics: usize,
}

impl SweepGrid {
    /// Gather changes the proxy set, so staged ids are stale.
    fn begin_frame(&mut self) {
        self.staged = false;
        self.pair_queries = 0;
    }

    /// True when this grid answers the next sweep query, staging it first if
    /// the pair grid has answered as many queries as there are statics.
    fn answers_query(&mut self, config: &PhysicsConfig, proxies: &[Proxy]) -> bool {
        if !self.staged {
            if self.pair_queries < self.statics {
                self.pair_queries += 1;
                return false;
            }
            self.stage(config, proxies);
        }
        true
    }

    /// Stage every static proxy with the `2·linear_slop` inflation a pair
    /// build gives it, in proxy order. Statics never move within a frame, so
    /// one staging serves every substep.
    fn stage(&mut self, config: &PhysicsConfig, proxies: &[Proxy]) {
        let grid = &mut self.grid;
        if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
            grid.set_cell_size(config.broadphase_cell_size);
        }
        grid.clear();
        let inflation = Vec2::splat(2.0 * config.linear_slop);
        for (id, proxy) in proxies.iter().enumerate() {
            if proxy.is_dynamic {
                continue;
            }
            let mut aabb = proxy.world_aabb();
            aabb.half_extents += inflation;
            grid.insert(id as ProxyId, &aabb);
        }
        self.staged = true;
    }
}

/// Physics scratch buffers reused across substeps/frames.
#[derive(Debug, Default)]
// Test-only oracle flags are independent switches, not solver states.
#[cfg_attr(test, allow(clippy::struct_excessive_bools))]
pub struct PhysicsBuffers {
    proxies: Vec<Proxy>,
    /// Inflated AABBs and travel allowances for D-075 pair reuse, each set
    /// at the proxy's last pair build or repair.
    pair_aabbs: Vec<Aabb>,
    pair_budgets: Vec<PairBudget>,
    /// `PAIR_*` bits per proxy. Pair queries read this byte instead of the
    /// 80-byte `Proxy` (D-080).
    pair_flags: Vec<u8>,
    /// Awake dynamic proxies whose travel budget ran out, in proxy order
    /// (D-081); refilled before every substep.
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
    /// D-063 reference model, rebuilt every substep in regression tests only.
    #[cfg(test)]
    reference_impulses: Option<ImpulseMap>,
    pairs: Vec<(u32, u32)>,
    events: Vec<CollisionEvent>,
    candidates: Vec<ProxyId>,
    grid: SpatialGrid,
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
    /// Scratch: minimum member sleep timer per island root.
    island_min_timer: Vec<f32>,
    /// Scratch: adopted sleeping-island tag per island root (0 = fresh).
    island_tag: Vec<u64>,
    /// Scratch: island tags scheduled to wake this frame.
    wake_islands: Vec<u64>,
    arrival: ArrivalScratch,
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
}

/// Wake `entity`'s sleep island (D-065); no-op when nothing has ever slept.
pub fn wake(world: &mut World, entity: Entity) {
    if let Some(buffers) = world.get_resource_mut::<PhysicsBuffers>() {
        buffers.wake(entity);
    }
}

/// Run one physics tick with fixed substeps (D-064), advancing at most
/// `PhysicsConfig::max_step_dt` of the frame's dt (D-094).
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

    // Step bound (D-094): a longer substep would lower the contact-hertz cap
    // in `soft_params` and soften every contact, so the rest of a slow
    // frame's time is dropped instead of simulated.
    let dt = if config.max_step_dt > 0.0 {
        dt.min(config.max_step_dt)
    } else {
        dt
    };

    let substeps = config.substeps.max(1);
    let sub_dt = dt / substeps as f32;

    // Remove/reinsert buffers to avoid resource borrow + entity borrow overlap.
    let mut buffers = world
        .remove_resource::<PhysicsBuffers>()
        .unwrap_or_default();

    integrate_loose_bodies(world, dt, config.gravity);

    gather_proxies(world, &mut buffers.proxies);
    buffers.pairs_invalidated = true;
    buffers.static_grid.begin_frame();

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

    for step in 0..substeps {
        let time_left = (dt - step as f32 * sub_dt).max(sub_dt);
        substep(&config, sub_dt, time_left, &mut buffers, sleep_enabled);
    }

    sync_impulses(&mut buffers);
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

/// Frame-start sleep bookkeeping (D-065): line the persistent sleep state up
/// with this frame's proxies (D-082), waking islands whose members despawned
/// (or lost their dynamic body/collider) and islands with an external write.
fn sleep_frame_start(buffers: &mut PhysicsBuffers) {
    let PhysicsBuffers {
        proxies,
        sleep,
        wake_islands,
        ..
    } = buffers;

    wake_islands.clear();
    if !sleep.same_bodies(proxies, wake_islands) {
        wake_islands.clear();
        sleep.rebuild(proxies, wake_islands);
    }
    if !wake_islands.is_empty() {
        sleep.wake_islands(wake_islands);
    }

    // Proxies persist across the frame's substeps (D-066), so the flag set
    // here carries through until a contact wake flips it. Statics and tiles
    // hold a default entry, which never sleeps.
    for (proxy, entry) in proxies.iter_mut().zip(&sleep.entries) {
        proxy.sleeping = entry.sleeping;
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

    debug_assert_eq!(
        sleep.entries.len(),
        proxies.len(),
        "sleep table not lined up"
    );
    let threshold_sq = config.sleep_threshold * config.sleep_threshold;
    island_min_timer.clear();
    island_min_timer.resize(proxies.len(), f32::INFINITY);
    island_tag.clear();
    island_tag.resize(proxies.len(), 0);

    for (index, proxy) in proxies.iter().enumerate() {
        if !proxy.is_dynamic || proxy.sleeping || proxy.entity.is_none() {
            continue;
        }
        let entry = &mut sleep.entries[index];
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
        let tag = sleep.entries[b_idx].island;
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
        let entry = &mut sleep.entries[index];
        entry.sleeping = true;
        entry.island = tag;
        // Same fp ops the next gather performs on the written-back
        // Position, so an untouched body compares bit-equal.
        entry.center = (proxy.center - proxy.offset) + proxy.offset;
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 = Vec2::ZERO;
        }
    }
}

/// Dynamic bodies without a collider never enter the solver; integrate them
/// once per frame (old behavior integrated them per substep — for pure
/// ballistic motion the trajectories differ only at O(g·dt²)).
/// Columnar pass over collider-less archetypes; no per-entity lookups.
fn integrate_loose_bodies(world: &mut World, dt: f32, gravity: Vec2) {
    for (_entity, velocity, position, body) in
        world.query3_mut_without::<Velocity, Position, RigidBody, Collider>()
    {
        if body.kind != BodyKind::Dynamic {
            continue;
        }
        velocity.0 += gravity * dt;
        position.0 += velocity.0 * dt;
    }
}

/// Keep the pair list complete for the coming narrow phase (D-081): build it
/// when it is invalid, reuse it while every budget holds, and otherwise
/// repair the tripped proxies, or rebuild when too many tripped.
fn refresh_pairs(
    config: &PhysicsConfig,
    sub_dt: f32,
    time_left: f32,
    buffers: &mut PhysicsBuffers,
) {
    if buffers.pairs_invalidated || buffers.proxies.len() != buffers.pair_budgets.len() {
        build_pairs(config, sub_dt, time_left, buffers);
        return;
    }
    let awake = collect_tripped(config, sub_dt, buffers);
    if buffers.tripped.is_empty() {
        return;
    }
    if buffers.tripped.len() * REBUILD_ONE_IN > awake {
        build_pairs(config, sub_dt, time_left, buffers);
    } else {
        repair_pairs(config, sub_dt, time_left, buffers);
    }
}

/// Build pairs from fresh per-proxy travel-inflated AABBs (D-075, radius per
/// D-081). The inflation includes the speculative slack on both sides of a
/// pair, so neither the query nor the overlap prefilter needs a
/// grid-staleness margin.
fn build_pairs(config: &PhysicsConfig, sub_dt: f32, time_left: f32, buffers: &mut PhysicsBuffers) {
    // Rebuilds can reorder/reverse pairs or introduce new ones after a wake.
    // Transfer only the immediately preceding substep's live impulses by key.
    sync_impulses(buffers);
    let PhysicsBuffers {
        proxies,
        grid,
        static_grid,
        pair_aabbs,
        pair_budgets,
        pair_flags,
        repaired,
        pairs,
        pairs_invalidated,
        ..
    } = buffers;
    if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
        grid.set_cell_size(config.broadphase_cell_size);
    }
    grid.clear();
    pair_aabbs.clear();
    pair_budgets.clear();
    pair_flags.clear();
    repaired.clear();
    pairs.clear();
    static_grid.statics = 0;
    let static_radius = 2.0 * config.linear_slop;
    let gravity_allowance = gravity_allowance(config, sub_dt, time_left);
    for (id, proxy) in proxies.iter().enumerate() {
        static_grid.statics += usize::from(!proxy.is_dynamic);
        let awake = proxy.is_dynamic && !proxy.sleeping;
        let radius = if awake {
            awake_pair_radius(
                config,
                proxy.velocity.length(),
                time_left,
                gravity_allowance,
            )
        } else {
            static_radius
        };
        let mut aabb = proxy.world_aabb();
        aabb.half_extents += Vec2::splat(radius);
        pair_aabbs.push(aabb);
        pair_budgets.push(PairBudget {
            radius,
            travel: 0.0,
        });
        pair_flags.push(if awake { PAIR_AWAKE } else { 0 });
        grid.insert(id as ProxyId, &aabb);
    }
    for (a_idx, aabb) in pair_aabbs.iter().enumerate() {
        if pair_flags[a_idx] & PAIR_AWAKE == 0 {
            continue;
        }
        grid.for_each_in(aabb, Some(a_idx as ProxyId), |b_id| {
            let b_idx = b_id as usize;
            // An awake candidate at or below this index runs its own query.
            if pair_flags[b_idx] & PAIR_AWAKE != 0 && b_idx <= a_idx {
                return;
            }
            if aabb.overlaps(&pair_aabbs[b_idx]) {
                pairs.push((a_idx as u32, b_id));
            }
        });
    }
    *pairs_invalidated = false;
    buffers.pair_impulses.clear();
    buffers.pair_impulses.resize(pairs.len(), 0.0);
    buffers.seed_pair_impulses = true;
    #[cfg(test)]
    {
        buffers.pair_builds += 1;
    }
}

/// Collect, in proxy order, the awake dynamic proxies whose budget does not
/// cover the coming substep, and return the awake dynamic count. Solver
/// impulses and gravity can change velocity after a build, so predicted
/// travel alone is insufficient: accumulated actual travel must fit as well.
fn collect_tripped(config: &PhysicsConfig, sub_dt: f32, buffers: &mut PhysicsBuffers) -> usize {
    let PhysicsBuffers {
        proxies,
        pair_budgets,
        tripped,
        ..
    } = buffers;
    tripped.clear();
    let slack = 2.0 * config.linear_slop;
    let mut awake = 0;
    for (index, (proxy, budget)) in proxies.iter().zip(pair_budgets.iter()).enumerate() {
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        awake += 1;
        if budget.travel + proxy.velocity.length() * sub_dt + slack > budget.radius {
            tripped.push(index as u32);
        }
    }
    awake
}

/// Re-pair the tripped proxies in place of a rebuild (D-081). Every other
/// proxy keeps its pairs, inflated AABB and budget, and the pair grid is not
/// restaged.
fn repair_pairs(config: &PhysicsConfig, sub_dt: f32, time_left: f32, buffers: &mut PhysicsBuffers) {
    let PhysicsBuffers {
        proxies,
        grid,
        repair_grid,
        pair_aabbs,
        pair_budgets,
        pair_flags,
        tripped,
        repaired,
        repair_impulses,
        removed_impulses,
        pairs,
        pair_impulses,
        ..
    } = buffers;
    for &member in tripped.iter() {
        pair_flags[member as usize] |= PAIR_TRIPPED;
    }

    // Remove the members' pairs, keeping the order of the rest, and remember
    // each removed pair's carried impulse under its key.
    removed_impulses.clear();
    let mut kept = 0;
    for index in 0..pairs.len() {
        let (a, b) = pairs[index];
        let impulse = pair_impulses[index];
        if (pair_flags[a as usize] | pair_flags[b as usize]) & PAIR_TRIPPED == 0 {
            pairs[kept] = (a, b);
            pair_impulses[kept] = impulse;
            kept += 1;
        } else if impulse != 0.0 {
            let key = pair_key(proxies[a as usize].key, proxies[b as usize].key);
            removed_impulses.push((key, impulse));
        }
    }
    pairs.truncate(kept);
    pair_impulses.truncate(kept);
    repair_impulses.reset(removed_impulses.len());
    for &(key, impulse) in removed_impulses.iter() {
        repair_impulses.insert(key, impulse);
    }

    // A fresh allowance from each member's current state.
    let gravity_allowance = gravity_allowance(config, sub_dt, time_left);
    for &member in tripped.iter() {
        let index = member as usize;
        let proxy = &proxies[index];
        let radius = awake_pair_radius(
            config,
            proxy.velocity.length(),
            time_left,
            gravity_allowance,
        );
        let mut aabb = proxy.world_aabb();
        aabb.half_extents += Vec2::splat(radius);
        pair_aabbs[index] = aabb;
        pair_budgets[index] = PairBudget {
            radius,
            travel: 0.0,
        };
        if pair_flags[index] & PAIR_REPAIRED == 0 {
            pair_flags[index] |= PAIR_REPAIRED;
            repaired.push(member);
        }
    }

    // The pair grid still holds every repaired proxy under the cells of its
    // old AABB, so a second grid finds those.
    if (repair_grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
        repair_grid.set_cell_size(config.broadphase_cell_size);
    }
    repair_grid.clear();
    for &id in repaired.iter() {
        repair_grid.insert(id, &pair_aabbs[id as usize]);
    }

    let carried = !removed_impulses.is_empty();
    for &member in tripped.iter() {
        let aabb = &pair_aabbs[member as usize];
        let member_key = proxies[member as usize].key;
        let mut admit = |a: u32, b: u32, other: u32| {
            pairs.push((a, b));
            pair_impulses.push(if carried {
                repair_impulses.get(pair_key(member_key, proxies[other as usize].key))
            } else {
                0.0
            });
        };
        // Only the members query, so this is not the build's initiator rule:
        // an untripped awake body of lower index must still be paired here.
        grid.for_each_in(aabb, Some(member), |other| {
            let flags = pair_flags[other as usize];
            if flags & PAIR_REPAIRED != 0 || !aabb.overlaps(&pair_aabbs[other as usize]) {
                return;
            }
            if flags & PAIR_AWAKE != 0 && other < member {
                admit(other, member, other);
            } else {
                admit(member, other, other);
            }
        });
        repair_grid.for_each_in(aabb, Some(member), |other| {
            // A pair between two members is added by the lower one.
            let member_too = pair_flags[other as usize] & PAIR_TRIPPED != 0;
            if (member_too && other < member) || !aabb.overlaps(&pair_aabbs[other as usize]) {
                return;
            }
            if other < member {
                admit(other, member, other);
            } else {
                admit(member, other, other);
            }
        });
    }
    for &member in tripped.iter() {
        pair_flags[member as usize] &= !PAIR_TRIPPED;
    }
    #[cfg(test)]
    {
        buffers.pair_repairs += 1;
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
/// arrival pass, relax, restitution, speculative safety net. Runs entirely on the
/// dense proxy arrays — no `World` access (D-066).
fn substep(
    config: &PhysicsConfig,
    sub_dt: f32,
    time_left: f32,
    buffers: &mut PhysicsBuffers,
    sleep_enabled: bool,
) {
    refresh_pairs(config, sub_dt, time_left, buffers);
    #[cfg(test)]
    if buffers.check_pair_contacts {
        tests::assert_pair_contacts(config, sub_dt, buffers);
        buffers.checked_substeps += 1;
    }
    let PhysicsBuffers {
        proxies,
        pair_budgets,
        pair_flags,
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
        arrival,
        ..
    } = buffers;
    candidates.clear();
    contacts.clear();
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
        if contact.penetration > 0.0
            && let Some(a_entity) = proxies[a_idx].entity
        {
            events.push(CollisionEvent {
                a: a_entity,
                b: proxies[b_idx].entity,
                normal: contact.normal,
                penetration: contact.penetration,
            });
        }
    }

    // Integrate velocities (gravity), then warm start from stored impulses.
    // Each awake body's velocity is kept as the narrow phase saw it: the
    // arrival pass measures what the solve adds against it (D-092).
    arrival.fit(proxies.len());
    for (proxy, admitted) in proxies.iter_mut().zip(arrival.admitted.iter_mut()) {
        if proxy.is_dynamic && !proxy.sleeping {
            *admitted = proxy.velocity;
            if proxy.has_velocity {
                proxy.velocity += config.gravity * sub_dt;
            }
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

    // Position integration converts the bias velocity into depenetration. A
    // body whose velocity the solve changed by more than the arrival
    // threshold is listed on the way (D-092).
    let arrival_threshold = ARRIVAL_SLOPS * config.linear_slop * inv_h;
    let arrival_threshold_sq = arrival_threshold * arrival_threshold;
    arrival.list.clear();
    for (index, (proxy, admitted)) in proxies.iter_mut().zip(arrival.admitted.iter()).enumerate() {
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        if (proxy.velocity - *admitted).length_squared() > arrival_threshold_sq {
            arrival.list.push(index as u32);
        }
        proxy.prev_center = proxy.center;
        let travel = proxy.velocity * sub_dt;
        proxy.center += travel;
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
            repaired,
            candidates,
            arrival,
        );
    }

    // One bias-free relax iteration strips the injected bias energy.
    solve_contacts(proxies, contacts, config, soft, soft_static, inv_h, false);

    apply_restitution(proxies, contacts, config.restitution_threshold);

    // Direct indexed carry between substeps; synchronize the keyed map only
    // at frame end or when a pair rebuild invalidates these indices.
    for contact in contacts.iter() {
        pair_impulses[contact.pair_index] = contact.impulse;
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
    speculative_pass(config, proxies, grid, static_grid, candidates, events);

    // Sum displacement between narrow phases, including the sweep's clamp.
    // This bounds distance from the build center even after direction changes.
    for (proxy, budget) in proxies.iter().zip(pair_budgets) {
        if proxy.is_dynamic && !proxy.sleeping {
            budget.travel += (proxy.center - proxy.prev_center).length();
        }
    }
}

/// Preserve D-063's immediately-previous-contact semantics, including empty
/// final substeps. The common case pays one map rebuild per frame (D-076).
fn sync_impulses(buffers: &mut PhysicsBuffers) {
    if !buffers.impulses_dirty {
        return;
    }
    buffers.impulses.reset(buffers.contacts.len());
    for contact in &buffers.contacts {
        if contact.impulse > 0.0 {
            buffers.impulses.insert(contact.key, contact.impulse);
        }
    }
    buffers.impulses_dirty = false;
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

/// Inverse mass in the arrival pass: a static or sleeping body does not give
/// way.
fn arrival_inv(proxy: &Proxy) -> f32 {
    if proxy.is_dynamic && !proxy.sleeping {
        proxy.inv_mass
    } else {
        0.0
    }
}

/// Where a proxy was when the substep began. The pass runs after position
/// integration, which moved the awake dynamic ones.
fn arrival_center(proxy: &Proxy) -> Vec2 {
    if proxy.is_dynamic && !proxy.sleeping {
        proxy.prev_center
    } else {
        proxy.center
    }
}

/// Arrival pass (D-092): a body whose velocity the solve just changed is
/// clamped against whatever that velocity reaches in this substep, and its
/// move is redone with the clamped velocity.
///
/// The narrow phase admits contacts for the velocities it sees. A body the
/// solver then accelerates, such as a resting ball hit by a fast one, may
/// have no pair with the wall behind it, or a contact with it that was solved
/// before the push arrived: with one velocity iteration the contact solved
/// last has the last word. Position integration would then carry the body
/// through the wall, at once or a few pixels per substep.
///
/// 1. Position integration lists each awake body whose velocity differs
///    from the admitted one by more than `ARRIVAL_SLOPS · linear_slop` of
///    travel. The pass runs only when it listed one.
/// 2. Each listed body is paired with every neighbour within reach of its
///    new velocity (static, sleeping or dynamic; from the pair grid and the
///    repair grid) that the narrow phase accepts at the D-064 margin, with
///    both bodies where the substep began.
/// 3. The pairs are ordered pushers first, walls last: by the push depth of
///    each pair's mover, then lighter targets first.
/// 4. Forward sweep: a pair closing faster than its gap allows takes the
///    impulse that removes the excess, split by inverse mass. A neighbour
///    this pushes past the threshold joins the list for the next round.
/// 5. Backward sweep: a mover left pressed on its target moves with it, so a
///    pusher still closing on a pressed body is clamped against the pressed
///    chain as one body. A chain that ends at a static or sleeping body
///    takes no impulse, and the pusher stops behind it.
///
/// The pair list, the contacts, their impulses and the events are left
/// alone; the regular repair (D-081) picks a new pair up before the next
/// narrow phase. A clamp is inelastic, and a sleeper is immovable here: the
/// wake test fires at the next narrow phase. Every loop runs in proxy, grid
/// or sorted order, so the pass is deterministic.
#[allow(clippy::too_many_arguments)]
fn arrival_pass(
    config: &PhysicsConfig,
    sub_dt: f32,
    proxies: &mut [Proxy],
    grid: &mut SpatialGrid,
    repair_grid: &mut SpatialGrid,
    pair_flags: &[u8],
    repaired: &[u32],
    candidates: &mut Vec<ProxyId>,
    arrival: &mut ArrivalScratch,
) {
    let ArrivalScratch {
        admitted,
        list,
        slot,
        pairs,
        state,
        touched,
        depth,
        pressed_dir,
        pressed_on,
        chain_inv,
        #[cfg(test)]
        listed,
        #[cfg(test)]
        clamps,
    } = arrival;
    let inv_h = 1.0 / sub_dt;
    let slack = 4.0 * config.linear_slop;
    let threshold = ARRIVAL_SLOPS * config.linear_slop * inv_h;
    let threshold_sq = threshold * threshold;
    let tight = ARRIVAL_TIGHT * inv_h;

    for (at, &index) in list.iter().enumerate() {
        slot[index as usize] = at as u32;
        ArrivalScratch::mark(state, touched, index as usize, ARRIVAL_LISTED);
    }

    let mut round_start = 0;
    for _ in 0..ARRIVAL_ROUNDS {
        let round_end = list.len();
        if round_start == round_end {
            break;
        }
        #[cfg(test)]
        {
            *listed += round_end - round_start;
        }

        // The neighbours each body of this round can reach.
        pairs.clear();
        for (at, &body) in list[..round_end].iter().enumerate().skip(round_start) {
            let body = body as usize;
            let velocity = proxies[body].velocity;
            let center = proxies[body].prev_center;
            let half = proxies[body].half_extents();
            let mut aabb = Aabb::new(center, half);
            aabb.half_extents += Vec2::splat(velocity.length() * sub_dt + slack);
            // As in a pair repair: the pair grid holds a repaired proxy under
            // the cells of its old AABB, so those come from the repair grid.
            candidates.clear();
            grid.for_each_in(&aabb, Some(body as ProxyId), |id| {
                if pair_flags[id as usize] & PAIR_REPAIRED == 0 {
                    candidates.push(id);
                }
            });
            if !repaired.is_empty() {
                repair_grid.for_each_in(&aabb, Some(body as ProxyId), |id| candidates.push(id));
            }
            for &other_id in candidates.iter() {
                let other = other_id as usize;
                // Two bodies of one round share a pair: the one listed first
                // has it.
                if state[other] & ARRIVAL_LISTED != 0
                    && (round_start..at).contains(&(slot[other] as usize))
                {
                    continue;
                }
                let target = &proxies[other];
                let target_center = arrival_center(target);
                let relative = velocity - target.velocity;
                // A cell holds far more neighbours than the margin admits:
                // bounding boxes farther apart than it are dropped before
                // the square root and the narrow phase.
                let apart = ((target_center - center).abs() - half - target.half_extents())
                    .max_element()
                    - slack;
                if apart > 0.0 && apart * apart > relative.length_squared() * sub_dt * sub_dt {
                    continue;
                }
                let margin = relative.length() * sub_dt + slack;
                let Some(contact) =
                    narrow_phase_at(&proxies[body], center, target, target_center, margin)
                else {
                    continue;
                };
                let separation = config.linear_slop - contact.penetration;
                pairs.push(ArrivalPair {
                    x: body as u32,
                    y: other_id,
                    normal: contact.normal,
                    allowance: if separation > 0.0 {
                        separation * inv_h
                    } else {
                        0.0
                    },
                    depth: 0,
                    target_inv: 0.0,
                });
            }
        }
        round_start = round_end;
        if pairs.is_empty() {
            continue;
        }

        // Pushers first, walls last. A target sits one level deeper than its
        // mover; the bound on the levels also ends a cycle.
        for pair in pairs.iter() {
            depth[pair.x as usize] = 0;
            depth[pair.y as usize] = 0;
        }
        for _ in 0..ARRIVAL_ROUNDS {
            let mut changed = false;
            for pair in pairs.iter() {
                let (mover, target, _) = pair.mover(proxies);
                let next = depth[mover] + 1;
                if depth[target] < next && next as usize <= ARRIVAL_ROUNDS {
                    depth[target] = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for pair in pairs.iter_mut() {
            let (mover, target, _) = pair.mover(proxies);
            pair.depth = depth[mover];
            pair.target_inv = arrival_inv(&proxies[target]);
        }
        // Stable: pairs of one depth and target mass keep query order.
        pairs.sort_by(|p, q| {
            p.depth
                .cmp(&q.depth)
                .then(q.target_inv.total_cmp(&p.target_inv))
        });

        // Forward: each pair arrives, mass-weighted like the solver.
        for pair in pairs.iter() {
            let (x, y) = (pair.x as usize, pair.y as usize);
            let (inv_x, inv_y) = (arrival_inv(&proxies[x]), arrival_inv(&proxies[y]));
            let closing = pair.closing(proxies);
            if closing >= 0.0 || inv_x + inv_y <= 0.0 {
                continue;
            }
            let lambda = -closing / (inv_x + inv_y);
            proxies[x].velocity += pair.normal * (lambda * inv_x);
            proxies[y].velocity -= pair.normal * (lambda * inv_y);
            #[cfg(test)]
            {
                *clamps += 1;
            }
            for body in [x, y] {
                if arrival_inv(&proxies[body]) == 0.0 {
                    continue;
                }
                ArrivalScratch::mark(state, touched, body, ARRIVAL_MOVED);
                // The clamp can hand a neighbour a velocity nobody admitted.
                if state[body] & ARRIVAL_LISTED == 0
                    && (proxies[body].velocity - admitted[body]).length_squared() > threshold_sq
                {
                    slot[body] = list.len() as u32;
                    ArrivalScratch::mark(state, touched, body, ARRIVAL_LISTED);
                    list.push(body as u32);
                }
            }
        }

        // Backward, walls first: a pair that is tight, or still closing too
        // fast, presses its mover on its target.
        for pair in pairs.iter().rev() {
            let closing = pair.closing(proxies);
            if closing > tight {
                continue;
            }
            let (mover, target, dir) = pair.mover(proxies);
            let inv_mover = arrival_inv(&proxies[mover]);
            if inv_mover == 0.0 {
                continue;
            }
            // What the mover is clamped against: the target alone, or the
            // chain the target heads when it is pressed the same way.
            let inv_target = arrival_inv(&proxies[target]);
            let pressed_along = |index: usize, state: &[u8], pressed_dir: &[Vec2]| {
                state[index] & ARRIVAL_PRESSED != 0 && pressed_dir[index].dot(dir) > 0.5
            };
            let inv_chain = if inv_target == 0.0 {
                0.0
            } else if pressed_along(target, state, pressed_dir) {
                chain_inv[target]
            } else {
                inv_target
            };
            if closing < 0.0 {
                let lambda = -closing / (inv_mover + inv_chain);
                proxies[mover].velocity -= dir * (lambda * inv_mover);
                ArrivalScratch::mark(state, touched, mover, ARRIVAL_MOVED);
                if inv_chain > 0.0 {
                    // Every member takes the same change, so the chain stays
                    // pressed together.
                    let gain = dir * (lambda * inv_chain);
                    let mut member = target;
                    for _ in 0..=touched.len() {
                        proxies[member].velocity += gain;
                        ArrivalScratch::mark(state, touched, member, ARRIVAL_MOVED);
                        let next = pressed_on[member] as usize;
                        if !pressed_along(member, state, pressed_dir)
                            || arrival_inv(&proxies[next]) == 0.0
                        {
                            break;
                        }
                        member = next;
                    }
                }
                #[cfg(test)]
                {
                    *clamps += 1;
                }
            }
            ArrivalScratch::mark(state, touched, mover, ARRIVAL_PRESSED);
            pressed_dir[mover] = dir;
            pressed_on[mover] = target as u32;
            chain_inv[mover] = if inv_chain == 0.0 {
                0.0
            } else {
                1.0 / (1.0 / inv_mover + 1.0 / inv_chain)
            };
        }
    }

    // Redo the move of every body the pass slowed or pushed.
    for &index in touched.iter() {
        let index = index as usize;
        if state[index] & ARRIVAL_MOVED != 0 {
            let proxy = &mut proxies[index];
            proxy.center = proxy.prev_center + proxy.velocity * sub_dt;
        }
        state[index] = 0;
    }
    touched.clear();
}

/// Safety-net slab sweep (D-064): clamp fast dynamics to the first static
/// sweep hit. Speculative contacts are the primary CCD; this pass only
/// catches trajectories the pair admission never saw — velocity injected by
/// the solver after contacts were built (impulse chains, deep-recovery bias).
/// Static circles promote conservatively to their bounding squares.
///
/// Only statics can be hit. The pair grid answers the query until the
/// statics-only grid is worth staging (D-080); both return the same statics
/// in the same order, so the first hit is the same.
fn speculative_pass(
    config: &PhysicsConfig,
    proxies: &mut [Proxy],
    grid: &mut SpatialGrid,
    static_grid: &mut SweepGrid,
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
        if static_grid.answers_query(config, proxies) {
            static_grid.grid.query(&swept, None, candidates);
        } else {
            grid.query(&swept, Some(a_idx as ProxyId), candidates);
        }

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
/// Always inlined: the substep's pair loop is the hottest loop of the step,
/// and a second caller must not turn it into a call.
#[inline(always)]
fn narrow_phase(a: &Proxy, b: &Proxy, margin: f32) -> Option<Contact> {
    narrow_phase_at(a, a.center, b, b.center, margin)
}

/// `narrow_phase` with the two bodies placed at `a_center` and `b_center`.
#[inline(always)]
fn narrow_phase_at(
    a: &Proxy,
    a_center: Vec2,
    b: &Proxy,
    b_center: Vec2,
    margin: f32,
) -> Option<Contact> {
    match (a.shape, b.shape) {
        (
            Shape::Aabb {
                half_extents: ha, ..
            },
            Shape::Aabb {
                half_extents: hb, ..
            },
        ) => aabb_vs_aabb_speculative(
            &Aabb::new(a_center, ha),
            &Aabb::new(b_center, hb),
            b.face_mask,
            margin,
        ),
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            circle_vs_circle_speculative(a_center, ra, b_center, rb, margin)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => aabb_vs_circle_speculative(
            &Aabb::new(a_center, half_extents),
            b_center,
            radius,
            a.face_mask,
            margin,
        ),
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            // Helper returns AABB escape normal; circle needs the opposite.
            aabb_vs_circle_speculative(
                &Aabb::new(b_center, half_extents),
                a_center,
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
