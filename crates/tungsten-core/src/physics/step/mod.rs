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
//! color-parallel contact solver was built, measured and dropped — at 25k
//! awake bodies the contact-solve passes are ~2% of the frame (the cost is
//! pair query + narrow phase + contact build), so threading the solver cannot
//! move the number and the coloring/sort machinery alone cost the serial path
//! ~18%.

use super::PhysicsConfig;
use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::Aabb;
use super::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use super::events::CollisionEvent;
use crate::ecs::{Entity, EventQueue, World};
use crate::time::Time;
use glam::Vec2;

mod arrival;
mod gather;
mod pairs;
mod sleep;
mod solver;
mod sweep;

use arrival::{ARRIVAL_SLOPS, ArrivalScratch, arrival_pass};
use gather::{entity_key, gather_proxies};
use pairs::{ImpulseMap, PairBudget, pair_key, refresh_pairs, sync_impulses};
use sleep::{SleepTable, sleep_frame_end, sleep_frame_start, uf_union};
use solver::{apply_impulse, apply_restitution, narrow_phase, soft_params, solve_contacts};
use sweep::{SweepGrid, speculative_pass};

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

#[cfg(test)]
#[path = "../../tests/physics/step.rs"]
mod tests;
