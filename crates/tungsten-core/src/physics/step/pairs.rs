//! Broadphase pairs that persist across substeps under per-proxy travel
//! budgets (D-075, D-081), and the keyed impulse map that carries warm starts
//! across pair rebuilds and frames (D-076).

use super::PhysicsBuffers;
use super::gather::TileCache;
use crate::physics::PhysicsConfig;
use crate::physics::broadphase::{ProxyId, SpatialGrid, for_each_in_both};
use crate::physics::collision::Aabb;
use glam::Vec2;

/// The pair grid: the entity proxies, staged at every pair build, and the
/// tile proxies, staged once for each tile cache version, cell size and
/// inflation, their ids offset by the entity count. While the tiles
/// outnumber the build's moving proxies a query walks both in the order one
/// grid staging every proxy in proxy order returns; otherwise the build
/// stages the tiles into the entity grid, that one grid. Either way no
/// candidate order changes (D-080's rule), so the choice is cost alone:
/// each moving proxy's query pays the walk, each build the staging.
#[derive(Debug, Default)]
pub(super) struct PairGrid {
    pub(super) bodies: SpatialGrid,
    pub(super) tiles: SpatialGrid,
    /// The first tile proxy's index: the frame's entity proxy count.
    pub(super) tile_base: ProxyId,
    /// The last build left the tiles in `tiles` alone, not in `bodies`.
    split: bool,
    /// Tests: the layout every build takes, whatever the cost.
    #[cfg(test)]
    pub(super) force_split: Option<bool>,
    /// What `tiles` holds: the cache version and the cell size and
    /// inflation bits it was staged with.
    staged: Option<(u64, u32, u32)>,
    /// The tiles' inflated boxes, in tile order, for the pair arrays.
    pub(super) tile_aabbs: Vec<Aabb>,
}

impl PairGrid {
    pub(super) fn cell_size(&self) -> f32 {
        self.bodies.cell_size()
    }

    /// Visit each proxy overlapping `query`, as one grid would: the entity
    /// grid's walk when it holds the tiles too, else both grids' walk.
    #[inline(always)]
    pub(super) fn for_each_in(
        &mut self,
        query: &Aabb,
        exclude: Option<ProxyId>,
        visit: impl FnMut(ProxyId),
    ) {
        if !self.split {
            self.bodies.for_each_in(query, exclude, visit);
            return;
        }
        for_each_in_both(
            &mut self.bodies,
            &mut self.tiles,
            self.tile_base,
            query,
            exclude,
            visit,
        );
    }

    /// Collect the proxies overlapping `query`, in [`Self::for_each_in`]'s
    /// order.
    pub(super) fn query(&mut self, query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>) {
        out.clear();
        self.for_each_in(query, exclude, |id| out.push(id));
    }

    /// Stage `tiles`' proxies at `cell_size`, each box grown by `radius`,
    /// unless the grid holds them so already.
    pub(super) fn stage_tiles(&mut self, tiles: &TileCache, cell_size: f32, radius: f32) {
        let key = (tiles.version(), cell_size.to_bits(), radius.to_bits());
        if self.staged == Some(key) {
            return;
        }
        self.tiles.set_cell_size(cell_size);
        self.tile_aabbs.clear();
        for (ordinal, proxy) in tiles.proxies.iter().enumerate() {
            let mut aabb = proxy.world_aabb();
            aabb.half_extents += Vec2::splat(radius);
            self.tile_aabbs.push(aabb);
            self.tiles.insert(ordinal as ProxyId, &aabb);
        }
        self.staged = Some(key);
    }
}

/// Sentinel for empty slots; unreachable as a real key because a pair key
/// of `u128::MAX` would need both members to share the same proxy key.
pub(super) const EMPTY_PAIR: u128 = u128::MAX;

/// Flat open-addressing map from contact pair key to accumulated impulse.
/// Synchronized from live contacts at frame end and before pair rebuilds;
/// never `std::HashMap` in the physics hot path (D-062 precedent).
#[derive(Debug, Default, Clone)]
pub(super) struct ImpulseMap {
    pub(super) keys: Vec<u128>,
    values: Vec<f32>,
    mask: u64,
}

impl ImpulseMap {
    /// Clear and size for `expected` insertions at <= 0.5 load factor.
    pub(super) fn reset(&mut self, expected: usize) {
        let capacity = (expected * 2).next_power_of_two().max(64);
        self.mask = capacity as u64 - 1;
        self.keys.clear();
        self.keys.resize(capacity, EMPTY_PAIR);
        self.values.clear();
        self.values.resize(capacity, 0.0);
    }

    pub(super) fn insert(&mut self, key: u128, value: f32) {
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

    /// Empty the map, keeping its buffers: every key reads 0.
    pub(super) fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
        self.mask = 0;
    }

    pub(super) fn get(&self, key: u128) -> f32 {
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

/// Order-independent pair key; the impulse is a scalar along the contact
/// normal, so orientation does not matter.
pub(super) fn pair_key(a: u64, b: u64) -> u128 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    (u128::from(hi) << 64) | u128::from(lo)
}

/// Conservative displacement allowance since the proxy's most recent pair
/// build or repair.
#[derive(Debug, Clone, Copy)]
pub(super) struct PairBudget {
    pub(super) radius: f32,
    pub(super) travel: f32,
}

/// Margin on every awake body's travel budget, in units of `linear_slop`
/// (D-081). It absorbs small velocity changes, and the rounding that would
/// otherwise decide the check for a body moving exactly as predicted.
pub(super) const PAIR_MARGIN_SLOPS: f32 = 2.0;

/// A repair is cheaper than a rebuild only for a minority: more than one
/// tripped proxy in this many awake bodies rebuilds instead (D-081).
const REBUILD_ONE_IN: usize = 4;

/// `PhysicsBuffers::pair_flags` bit: a moving proxy (an awake dynamic or a
/// kinematic body) at the last build.
pub(super) const PAIR_AWAKE: u8 = 1;
/// `pair_flags` bit: member of the repair in progress.
const PAIR_TRIPPED: u8 = 1 << 1;
/// `pair_flags` bit: repaired since the last build, so the pair grid still
/// holds the cells of its old AABB.
pub(super) const PAIR_REPAIRED: u8 = 1 << 2;

/// Travel gravity can add beyond `|v|·time_left` before the frame's last
/// budget check: `|g|·h²·(n−1)(n+2)/2` with `n = time_left / h` substeps
/// left (D-081). The integrator adds `g·h` to the velocity before it moves a
/// body, so the last check sees `n − 1` substeps of travel plus one more of
/// predicted travel at the speed gravity has built up by then.
pub(super) fn gravity_allowance(config: &PhysicsConfig, sub_dt: f32, time_left: f32) -> f32 {
    0.5 * config.gravity.length() * (time_left - sub_dt).max(0.0) * (time_left + 2.0 * sub_dt)
}

/// Pair inflation of a moving body at `speed` (D-081); a kinematic body
/// passes a `gravity_allowance` of 0.
fn awake_pair_radius(
    config: &PhysicsConfig,
    speed: f32,
    time_left: f32,
    gravity_allowance: f32,
) -> f32 {
    speed * time_left + gravity_allowance + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop
}

/// Keep the pair list complete for the coming narrow phase (D-081): build it
/// when it is invalid, reuse it while every budget holds, and otherwise
/// repair the tripped proxies, or rebuild when too many tripped.
pub(super) fn refresh_pairs(
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
/// grid-staleness margin. A kinematic proxy initiates as an awake one does;
/// it moves, so it is not counted among the statics. Two proxies whose
/// layers do not meet are never paired.
pub(super) fn build_pairs(
    config: &PhysicsConfig,
    sub_dt: f32,
    time_left: f32,
    buffers: &mut PhysicsBuffers,
) {
    // Rebuilds can reorder/reverse pairs or introduce new ones after a wake.
    // Transfer only the immediately preceding substep's live impulses by key.
    sync_impulses(buffers);
    let PhysicsBuffers {
        proxies,
        bodies,
        tiles,
        grid,
        static_grid,
        pair_aabbs,
        pair_budgets,
        pair_flags,
        layers,
        repaired,
        pairs,
        pairs_invalidated,
        ..
    } = buffers;
    let bodies = *bodies;
    if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
        grid.bodies.set_cell_size(config.broadphase_cell_size);
    }
    grid.bodies.clear();
    pair_aabbs.clear();
    pair_budgets.clear();
    pair_flags.clear();
    repaired.clear();
    pairs.clear();
    static_grid.statics = 0;
    let static_radius = 2.0 * config.linear_slop;
    let gravity_allowance = gravity_allowance(config, sub_dt, time_left);
    let mut moving = 0;
    for (id, proxy) in proxies[..bodies].iter().enumerate() {
        static_grid.statics += usize::from(!proxy.is_dynamic && !proxy.kinematic);
        let awake = proxy.moves();
        moving += usize::from(awake);
        let radius = if awake {
            awake_pair_radius(
                config,
                proxy.velocity.length(),
                time_left,
                if proxy.kinematic {
                    0.0
                } else {
                    gravity_allowance
                },
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
        grid.bodies.insert(id as ProxyId, &aabb);
    }
    // The tiles: static, never awake, grown by the static radius; staged
    // once for the cache and kept across builds, and into the entity grid
    // too, after the entity proxies, when the moving proxies' queries
    // outnumber them.
    grid.stage_tiles(tiles, grid.cell_size(), static_radius);
    grid.tile_base = bodies as ProxyId;
    grid.split = tiles.len() > moving;
    #[cfg(test)]
    if let Some(split) = grid.force_split {
        grid.split = split;
    }
    if !grid.split {
        let PairGrid {
            bodies: entity_grid,
            tile_aabbs,
            ..
        } = grid;
        for (ordinal, aabb) in tile_aabbs.iter().enumerate() {
            entity_grid.insert((bodies + ordinal) as ProxyId, aabb);
        }
    }
    static_grid.statics += tiles.len();
    pair_aabbs.extend_from_slice(&grid.tile_aabbs);
    pair_budgets.resize(
        proxies.len(),
        PairBudget {
            radius: static_radius,
            travel: 0.0,
        },
    );
    pair_flags.resize(proxies.len(), 0);
    for (a_idx, aabb) in pair_aabbs[..bodies].iter().enumerate() {
        if pair_flags[a_idx] & PAIR_AWAKE == 0 {
            continue;
        }
        grid.for_each_in(aabb, Some(a_idx as ProxyId), |b_id| {
            let b_idx = b_id as usize;
            // An awake candidate at or below this index runs its own query.
            if pair_flags[b_idx] & PAIR_AWAKE != 0 && b_idx <= a_idx {
                return;
            }
            if aabb.overlaps(&pair_aabbs[b_idx]) && layers.meet(a_idx, b_idx) {
                pairs.push((a_idx as u32, b_id));
            }
        });
    }
    *pairs_invalidated = false;
    buffers.pair_impulses.clear();
    buffers.pair_impulses.resize(pairs.len(), 0.0);
    buffers.seed_pair_impulses = true;
    // Tangents ride beside the normal impulses only in a frame with friction.
    let extra = &mut *buffers.extra;
    extra.pair_tangents.clear();
    if !extra.frictions.is_empty() {
        extra.pair_tangents.resize(pairs.len(), 0.0);
    }
    #[cfg(test)]
    {
        buffers.pair_builds += 1;
    }
}

/// Collect, in proxy order, the moving proxies whose budget does not cover
/// the coming substep, and return the moving count. Solver impulses and
/// gravity can change velocity after a build, so predicted travel alone is
/// insufficient: accumulated actual travel must fit as well. A kinematic
/// proxy's velocity cannot change within a frame, so it never trips.
pub(super) fn collect_tripped(
    config: &PhysicsConfig,
    sub_dt: f32,
    buffers: &mut PhysicsBuffers,
) -> usize {
    let PhysicsBuffers {
        proxies,
        bodies,
        pair_budgets,
        tripped,
        ..
    } = buffers;
    tripped.clear();
    let slack = 2.0 * config.linear_slop;
    let mut awake = 0;
    // Tile proxies never move, so the entity proxies are every candidate.
    for (index, (proxy, budget)) in proxies[..*bodies]
        .iter()
        .zip(pair_budgets.iter())
        .enumerate()
    {
        if !proxy.moves() {
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
/// restaged. As in a build, proxies whose layers do not meet are not paired.
fn repair_pairs(config: &PhysicsConfig, sub_dt: f32, time_left: f32, buffers: &mut PhysicsBuffers) {
    let PhysicsBuffers {
        proxies,
        grid,
        repair_grid,
        pair_aabbs,
        pair_budgets,
        pair_flags,
        layers,
        tripped,
        repaired,
        repair_impulses,
        removed_impulses,
        pairs,
        pair_impulses,
        extra,
        ..
    } = buffers;
    let super::StepExtra {
        frictions,
        pair_tangents,
        repair_tangents,
        removed_tangents,
        ..
    } = &mut **extra;
    let friction_on = !frictions.is_empty();
    for &member in tripped.iter() {
        pair_flags[member as usize] |= PAIR_TRIPPED;
    }

    // Remove the members' pairs, keeping the order of the rest, and remember
    // each removed pair's carried impulse, and tangent in a frame with
    // friction, under its key.
    removed_impulses.clear();
    removed_tangents.clear();
    let mut kept = 0;
    for index in 0..pairs.len() {
        let (a, b) = pairs[index];
        let impulse = pair_impulses[index];
        let tangent = if friction_on {
            pair_tangents[index]
        } else {
            0.0
        };
        if (pair_flags[a as usize] | pair_flags[b as usize]) & PAIR_TRIPPED == 0 {
            pairs[kept] = (a, b);
            pair_impulses[kept] = impulse;
            if friction_on {
                pair_tangents[kept] = tangent;
            }
            kept += 1;
        } else if impulse != 0.0 || tangent != 0.0 {
            let key = pair_key(proxies[a as usize].key, proxies[b as usize].key);
            if impulse != 0.0 {
                removed_impulses.push((key, impulse));
            }
            if tangent != 0.0 {
                removed_tangents.push((key, tangent));
            }
        }
    }
    pairs.truncate(kept);
    pair_impulses.truncate(kept);
    repair_impulses.reset(removed_impulses.len());
    for &(key, impulse) in removed_impulses.iter() {
        repair_impulses.insert(key, impulse);
    }
    if friction_on {
        pair_tangents.truncate(kept);
    }
    if removed_tangents.is_empty() {
        repair_tangents.clear();
    } else {
        repair_tangents.reset(removed_tangents.len());
        for &(key, tangent) in removed_tangents.iter() {
            repair_tangents.insert(key, tangent);
        }
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
            if proxy.kinematic {
                0.0
            } else {
                gravity_allowance
            },
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
    let carried_tangents = !removed_tangents.is_empty();
    for &member in tripped.iter() {
        let aabb = &pair_aabbs[member as usize];
        let member_key = proxies[member as usize].key;
        let mut admit = |a: u32, b: u32, other: u32| {
            pairs.push((a, b));
            let key = pair_key(member_key, proxies[other as usize].key);
            pair_impulses.push(if carried {
                repair_impulses.get(key)
            } else {
                0.0
            });
            if friction_on {
                pair_tangents.push(if carried_tangents {
                    repair_tangents.get(key)
                } else {
                    0.0
                });
            }
        };
        // Only the members query, so this is not the build's initiator rule:
        // an untripped awake body of lower index must still be paired here.
        grid.for_each_in(aabb, Some(member), |other| {
            let flags = pair_flags[other as usize];
            if flags & PAIR_REPAIRED != 0
                || !aabb.overlaps(&pair_aabbs[other as usize])
                || !layers.meet(member as usize, other as usize)
            {
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
            if (member_too && other < member)
                || !aabb.overlaps(&pair_aabbs[other as usize])
                || !layers.meet(member as usize, other as usize)
            {
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

/// Preserve D-063's immediately-previous-contact semantics, including empty
/// final substeps. The common case pays one map rebuild per frame (D-076).
pub(super) fn sync_impulses(buffers: &mut PhysicsBuffers) {
    if !buffers.impulses_dirty {
        return;
    }
    buffers.impulses.reset(buffers.contacts.len());
    for contact in &buffers.contacts {
        if contact.impulse > 0.0 {
            buffers.impulses.insert(contact.key, contact.impulse);
        }
    }
    // The tangents of the same contacts, in a frame with friction.
    let extra = &mut *buffers.extra;
    if extra.friction.is_empty() {
        extra.tangents.clear();
    } else {
        extra.tangents.reset(extra.friction.len());
        for friction in &extra.friction {
            if friction.impulse != 0.0 {
                let key = buffers.contacts[friction.contact as usize].key;
                extra.tangents.insert(key, friction.impulse);
            }
        }
    }
    buffers.impulses_dirty = false;
}
