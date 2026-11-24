//! Broadphase pairs that persist across substeps under per-proxy travel
//! budgets (D-075, D-081), and the keyed impulse map that carries warm starts
//! across pair rebuilds and frames (D-076).

use super::PhysicsBuffers;
use super::gather::TileCache;
use crate::physics::PhysicsConfig;
use crate::physics::broadphase::{
    ProxyId, QueryMarks, Run, SpatialGrid, for_each_run_in_both, for_each_run_in_both_shared,
};
use crate::physics::collision::Aabb;
use glam::{Vec2, Vec4};

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

/// One participant's scratch for the step's forked stages (`D-298`), kept
/// across forks: its marks for the pair grid's two grids and the repair
/// grid, and what it wrote for the chunks it took.
#[derive(Debug, Default)]
pub(super) struct WorkerScratch {
    pub(super) bodies: QueryMarks,
    pub(super) tiles: QueryMarks,
    pub(super) repair: QueryMarks,
    /// A walk's candidates, for a stage that cuts them after the walk.
    pub(super) candidates: Vec<ProxyId>,
    /// The pairs of the build or repair chunks it took, or a removal's kept
    /// pairs, a chunk after another.
    pub(super) pairs: Vec<(u32, u32)>,
    /// Each chunk it took: the chunk, and its output's start and length; a
    /// removal's, its [`RemovalSpan`]'s place in `removal` and 0.
    pub(super) spans: Vec<(u32, u32, u32)>,
    /// Its share of an arrival round's search.
    pub(super) arrival: super::arrival::ArrivalWorker,
    /// Beside `pairs`, for a repair's chunks: each admitted pair's sent
    /// flag, carried impulse and, with friction, carried tangent; for a
    /// removal's, each kept pair's.
    pub(super) sent: Vec<bool>,
    pub(super) impulses: Vec<f32>,
    pub(super) tangents: Vec<f32>,
    /// A removal's chunks: where each one's outputs lie, and the removed
    /// pairs that sent and the removed impulses and tangents by key, each in
    /// pair order.
    pub(super) removal: Vec<RemovalSpan>,
    pub(super) found: Vec<(u32, u32)>,
    pub(super) removed_impulses: Vec<(u128, f32)>,
    pub(super) removed_tangents: Vec<(u128, f32)>,
}

/// Where a removal chunk's outputs lie in its participant's scratch, each
/// as a start and a count: its kept pairs, its removed pairs that sent, its
/// removed impulses and its removed tangents.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct RemovalSpan {
    kept: (u32, u32),
    found: (u32, u32),
    impulses: (u32, u32),
    tangents: (u32, u32),
}

/// A [`RemovalSpan`] part's range.
fn span_range((start, count): (u32, u32)) -> std::ops::Range<usize> {
    start as usize..(start + count) as usize
}

/// The entity proxies a participant takes at a time in a forked build, and
/// in one the lockstep tests force, so that a small world has chunks enough
/// for every participant.
const BUILD_CHUNK: usize = 256;
const BUILD_CHUNK_FORCED: usize = 16;
/// The fewest moving proxies whose build forks: under it the walks cost less
/// than waking the workers.
const BUILD_FORK_MIN: usize = 1024;
/// A repair's members a participant takes at a time when its walks fork, in
/// one the lockstep tests force, and the fewest members whose walks fork.
const REPAIR_CHUNK: usize = 32;
const REPAIR_CHUNK_FORCED: usize = 4;
const REPAIR_FORK_MIN: usize = 256;
/// A removal's pairs a participant takes at a time when its scan forks, in
/// one the lockstep tests force, and the fewest pairs whose scan forks.
const REMOVAL_CHUNK: usize = 4096;
const REMOVAL_CHUNK_FORCED: usize = 8;
const REMOVAL_FORK_MIN: usize = 16_384;

/// The pair list's columns a forked repair stage writes back: the pairs, their
/// sent flags, impulses and tangents.
type PairColumns<'a> = (
    &'a mut Vec<(u32, u32)>,
    &'a mut Vec<bool>,
    &'a mut Vec<f32>,
    &'a mut Vec<f32>,
);
/// A removal's carried impulses and tangents by key, in pair order.
type RemovedRecords<'a> = (&'a mut Vec<(u128, f32)>, &'a mut Vec<(u128, f32)>);

/// A participant's share of a forked removal: for each chunk of pairs it
/// takes, the kept pairs with their impulses, tangents and sent flags, and
/// the removed pairs that sent and the removed impulses and tangents by key,
/// each as [`repair_pairs`]' serial loop meets them.
pub(super) fn remove_work(
    data: &super::pool::ForkData,
    scratch: &mut WorkerScratch,
    chunk_size: usize,
    chunks: usize,
    friction_on: bool,
) {
    let (pairs, pair_impulses, pair_tangents, sent_pairs) = (
        &data.pairs[..],
        &data.pair_impulses[..],
        &data.pair_tangents[..],
        &data.sent_pairs[..],
    );
    let (pair_flags, proxies) = (&data.pair_flags[..], &data.proxies[..]);
    let WorkerScratch {
        pairs: kept_pairs,
        impulses: kept_impulses,
        tangents: kept_tangents,
        sent: kept_sent,
        spans,
        removal,
        found,
        removed_impulses,
        removed_tangents,
        ..
    } = scratch;
    kept_pairs.clear();
    kept_impulses.clear();
    kept_tangents.clear();
    kept_sent.clear();
    spans.clear();
    removal.clear();
    found.clear();
    removed_impulses.clear();
    removed_tangents.clear();
    while let Some(chunk) = data.take_chunk(chunks) {
        let from = chunk * chunk_size;
        let to = (from + chunk_size).min(pairs.len());
        let starts = [
            kept_pairs.len(),
            found.len(),
            removed_impulses.len(),
            removed_tangents.len(),
        ];
        for index in from..to {
            let (a, b) = pairs[index];
            let impulse = pair_impulses[index];
            let tangent = if friction_on {
                pair_tangents[index]
            } else {
                0.0
            };
            if (pair_flags[a as usize] | pair_flags[b as usize]) & PAIR_TRIPPED == 0 {
                kept_pairs.push((a, b));
                kept_impulses.push(impulse);
                if friction_on {
                    kept_tangents.push(tangent);
                }
                kept_sent.push(sent_pairs[index]);
            } else {
                if sent_pairs[index] {
                    found.push((a, b));
                }
                if impulse != 0.0 || tangent != 0.0 {
                    let key = pair_key(proxies[a as usize].key, proxies[b as usize].key);
                    if impulse != 0.0 {
                        removed_impulses.push((key, impulse));
                    }
                    if tangent != 0.0 {
                        removed_tangents.push((key, tangent));
                    }
                }
            }
        }
        let part = |start: usize, end: usize| (start as u32, (end - start) as u32);
        spans.push((chunk as u32, removal.len() as u32, 0));
        removal.push(RemovalSpan {
            kept: part(starts[0], kept_pairs.len()),
            found: part(starts[1], found.len()),
            impulses: part(starts[2], removed_impulses.len()),
            tangents: part(starts[3], removed_tangents.len()),
        });
    }
}

/// [`repair_pairs`]' removal scan on the step's participants: each chunk's
/// kept pairs, impulses, tangents and sent flags laid back in chunk order,
/// which is the serial loop's order, and its removed records merged in that
/// order, the `found` inserts serial among them. Returns the kept count. Out
/// of line, so [`repair_pairs`] holds one call for the fork and its serial
/// loop's code stays as it was.
#[inline(never)]
fn removal_forked(
    fork: &mut super::pool::Fork,
    lists: PairColumns<'_>,
    pair_flags: &mut Vec<u8>,
    proxies: &mut Vec<super::Proxy>,
    found: &mut PairSet,
    removed: RemovedRecords<'_>,
    friction_on: bool,
) -> usize {
    let (pairs, sent_pairs, pair_impulses, pair_tangents) = lists;
    let (removed_impulses, removed_tangents) = removed;
    let chunk = fork.chunk(REMOVAL_CHUNK, REMOVAL_CHUNK_FORCED);
    let chunks = pairs.len().div_ceil(chunk);
    let workers = fork.workers();
    let data = workers.fork(super::pool::ForkData {
        stage: super::pool::Stage::Remove {
            chunk,
            chunks,
            friction_on,
        },
        pairs: std::mem::take(pairs),
        pair_impulses: std::mem::take(pair_impulses),
        pair_tangents: std::mem::take(pair_tangents),
        sent_pairs: std::mem::take(sent_pairs),
        pair_flags: std::mem::take(pair_flags),
        proxies: std::mem::take(proxies),
        ..Default::default()
    });
    *pairs = data.pairs;
    *pair_impulses = data.pair_impulses;
    *pair_tangents = data.pair_tangents;
    *sent_pairs = data.sent_pairs;
    *pair_flags = data.pair_flags;
    *proxies = data.proxies;
    pairs.clear();
    pair_impulses.clear();
    sent_pairs.clear();
    if friction_on {
        pair_tangents.clear();
    }
    workers.index_chunks(chunks, |scratch| &scratch.spans);
    for &(participant, ordinal, _) in &workers.chunk_at {
        let from = &workers.scratches[participant as usize];
        let span = from.removal[ordinal as usize];
        let kept = span_range(span.kept);
        pairs.extend_from_slice(&from.pairs[kept.clone()]);
        pair_impulses.extend_from_slice(&from.impulses[kept.clone()]);
        if friction_on {
            pair_tangents.extend_from_slice(&from.tangents[kept.clone()]);
        }
        sent_pairs.extend_from_slice(&from.sent[kept]);
        for &(a, b) in &from.found[span_range(span.found)] {
            found.insert(a, b);
        }
        removed_impulses.extend_from_slice(&from.removed_impulses[span_range(span.impulses)]);
        removed_tangents.extend_from_slice(&from.removed_tangents[span_range(span.tangents)]);
    }
    #[cfg(test)]
    {
        workers.removal_forks += 1;
        workers.tangent_removals += usize::from(!removed_tangents.is_empty());
    }
    pairs.len()
}

/// A participant's share of a forked repair's walks: for each member of the
/// chunks it takes, the pairs [`repair_pairs`]' serial loop admits, in its
/// order, with their sent flags, impulses and tangents.
pub(super) fn repair_work(
    data: &super::pool::ForkData,
    scratch: &mut WorkerScratch,
    chunk_size: usize,
    chunks: usize,
    friction_on: bool,
    carried: bool,
    carried_tangents: bool,
) {
    let (grid, repair_grid, proxies) = (&data.grid, &data.repair_grid, &data.proxies[..]);
    let (pair_flags, pair_bounds, layers, oversize) = (
        &data.pair_flags[..],
        &data.pair_bounds[..],
        &data.layers,
        &data.oversize[..],
    );
    let (found, repair_impulses, repair_tangents) =
        (&data.found, &data.repair_impulses, &data.repair_tangents);
    let WorkerScratch {
        bodies: body_marks,
        tiles: tile_marks,
        repair: repair_marks,
        pairs,
        spans,
        sent,
        impulses,
        tangents,
        ..
    } = scratch;
    pairs.clear();
    sent.clear();
    impulses.clear();
    tangents.clear();
    spans.clear();
    while let Some(chunk) = data.take_chunk(chunks) {
        let start = pairs.len();
        let from = chunk * chunk_size;
        let to = (from + chunk_size).min(data.list.len());
        for &member in &data.list[from..to] {
            let aabb = &data.pair_aabbs[member as usize];
            let query = query_bounds(aabb);
            let member_key = proxies[member as usize].key;
            let mut admit = |a: u32, b: u32, other: u32| {
                pairs.push((a, b));
                sent.push(found.contains(a, b));
                let key = pair_key(member_key, proxies[other as usize].key);
                impulses.push(if carried {
                    repair_impulses.get(key)
                } else {
                    0.0
                });
                if friction_on {
                    tangents.push(if carried_tangents {
                        repair_tangents.get(key)
                    } else {
                        0.0
                    });
                }
            };
            grid.for_each_run_in_marks(
                body_marks,
                tile_marks,
                aabb,
                #[inline(always)]
                |mut run| {
                    run.each(Some(member), |other| {
                        let flags = pair_flags[other as usize];
                        if flags & PAIR_REPAIRED != 0
                            || !bounds_overlap(query, pair_bounds[other as usize])
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
                },
            );
            repair_grid.for_each_run_in_shared(
                repair_marks,
                aabb,
                0,
                #[inline(always)]
                |mut run| {
                    run.each(Some(member), |other| {
                        let member_too = pair_flags[other as usize] & PAIR_TRIPPED != 0;
                        if (member_too && other < member)
                            || !bounds_overlap(query, pair_bounds[other as usize])
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
                },
            );
            for &other in oversize {
                if bounds_overlap(query, pair_bounds[other as usize])
                    && layers.meet(member as usize, other as usize)
                {
                    if other < member {
                        admit(other, member, other);
                    } else {
                        admit(member, other, other);
                    }
                }
            }
        }
        spans.push((chunk as u32, start as u32, (pairs.len() - start) as u32));
    }
}

/// [`repair_pairs`]' walks on the step's participants: the members in
/// chunks, each chunk's pairs, sent flags, impulses and tangents appended in
/// chunk order, which is the serial loop's order. Out of line, so
/// [`repair_pairs`] holds one call for the fork and its serial loop's code
/// stays as it was.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn repair_walks_forked(
    fork: &mut super::pool::Fork,
    buffers: (&mut PairGrid, &mut crate::physics::broadphase::SpatialGrid),
    pair_flags: &mut Vec<u8>,
    pair_bounds: &mut Vec<Vec4>,
    pair_aabbs: &mut Vec<Aabb>,
    layers: &mut super::ProxyLayers,
    oversize: &mut Vec<u32>,
    proxies: &mut Vec<super::Proxy>,
    tripped: &mut Vec<u32>,
    found: &mut PairSet,
    carried_maps: (&mut ImpulseMap, &mut ImpulseMap),
    outputs: PairColumns<'_>,
    switches: (bool, bool, bool),
) {
    let (grid, repair_grid) = buffers;
    let (repair_impulses, repair_tangents) = carried_maps;
    let (pairs, sent_pairs, pair_impulses, pair_tangents) = outputs;
    let (friction_on, carried, carried_tangents) = switches;
    let chunk = fork.chunk(REPAIR_CHUNK, REPAIR_CHUNK_FORCED);
    let chunks = tripped.len().div_ceil(chunk);
    grid.prepare();
    repair_grid.prepare();
    let workers = fork.workers();
    let data = workers.fork(super::pool::ForkData {
        stage: super::pool::Stage::Repair {
            chunk,
            chunks,
            friction_on,
            carried,
            carried_tangents,
        },
        grid: std::mem::take(grid),
        repair_grid: std::mem::take(repair_grid),
        pair_flags: std::mem::take(pair_flags),
        pair_bounds: std::mem::take(pair_bounds),
        pair_aabbs: std::mem::take(pair_aabbs),
        layers: std::mem::take(layers),
        oversize: std::mem::take(oversize),
        proxies: std::mem::take(proxies),
        list: std::mem::take(tripped),
        found: std::mem::take(found),
        repair_impulses: std::mem::take(repair_impulses),
        repair_tangents: std::mem::take(repair_tangents),
        ..Default::default()
    });
    *grid = data.grid;
    *repair_grid = data.repair_grid;
    *pair_flags = data.pair_flags;
    *pair_bounds = data.pair_bounds;
    *pair_aabbs = data.pair_aabbs;
    *layers = data.layers;
    *oversize = data.oversize;
    *proxies = data.proxies;
    *tripped = data.list;
    *found = data.found;
    *repair_impulses = data.repair_impulses;
    *repair_tangents = data.repair_tangents;
    workers.index_chunks(chunks, |scratch| &scratch.spans);
    #[cfg(test)]
    let admitted = pairs.len();
    for &(participant, start, count) in &workers.chunk_at {
        let from = &workers.scratches[participant as usize];
        let (start, end) = (start as usize, (start + count) as usize);
        pairs.extend_from_slice(&from.pairs[start..end]);
        sent_pairs.extend_from_slice(&from.sent[start..end]);
        pair_impulses.extend_from_slice(&from.impulses[start..end]);
        if friction_on {
            pair_tangents.extend_from_slice(&from.tangents[start..end]);
        }
    }
    #[cfg(test)]
    {
        workers.repair_forks += 1;
        workers.tangent_repairs += usize::from(carried_tangents);
        workers.oversize_repairs +=
            usize::from(pairs[admitted..].iter().any(|&(a, b)| {
                (pair_flags[a as usize] | pair_flags[b as usize]) & PAIR_OVERSIZE != 0
            }));
    }
}

/// A participant's share of a forked build's walks: the next chunks free,
/// each chunk's pairs as [`build_pairs`]' serial loop pushes them.
pub(super) fn build_work(
    data: &super::pool::ForkData,
    scratch: &mut WorkerScratch,
    bodies: usize,
    chunk_size: usize,
    chunks: usize,
) {
    let (grid, pair_flags, pair_bounds, layers) = (
        &data.grid,
        &data.pair_flags[..],
        &data.pair_bounds[..],
        &data.layers,
    );
    let pair_aabbs = &data.pair_aabbs[..bodies];
    let WorkerScratch {
        bodies: body_marks,
        tiles: tile_marks,
        pairs: out,
        spans,
        ..
    } = scratch;
    out.clear();
    spans.clear();
    while let Some(chunk) = data.take_chunk(chunks) {
        let start = out.len();
        let from = chunk * chunk_size;
        let to = (from + chunk_size).min(bodies);
        for a_idx in from..to {
            if pair_flags[a_idx] & PAIR_AWAKE == 0 {
                continue;
            }
            let aabb = &pair_aabbs[a_idx];
            let query = query_bounds(aabb);
            if pair_flags[a_idx] & PAIR_OVERSIZE != 0 {
                oversize_pairs(a_idx, query, pair_bounds, pair_flags, layers, out);
                continue;
            }
            grid.for_each_run_in_marks(
                body_marks,
                tile_marks,
                aabb,
                #[inline(always)]
                |mut run| {
                    run.each(Some(a_idx as ProxyId), |b_id| {
                        let b_idx = b_id as usize;
                        if pair_flags[b_idx] & PAIR_AWAKE != 0 && b_idx <= a_idx {
                            return;
                        }
                        if bounds_overlap(query, pair_bounds[b_idx]) && layers.meet(a_idx, b_idx) {
                            out.push((a_idx as u32, b_id));
                        }
                    });
                },
            );
        }
        spans.push((chunk as u32, start as u32, (out.len() - start) as u32));
    }
}

/// [`build_pairs`]' walks on the step's participants: the entity proxies in
/// chunks, each chunk's pairs appended to `pairs` in chunk order, which is
/// the serial loop's order. The calling thread runs `before`, the carry's
/// remap when it is due, while the workers walk, then walks chunks of its
/// own (OVERLAP-1); `before` says whether it remapped. Out of line with the
/// pool's start inside it, so [`build_pairs`] holds one call for the fork
/// and its serial loop's code stays as it was.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn build_walks_forked(
    fork: &mut super::pool::Fork,
    bodies: usize,
    grid: &mut PairGrid,
    pair_flags: &mut Vec<u8>,
    pair_bounds: &mut Vec<Vec4>,
    pair_aabbs: &mut Vec<Aabb>,
    layers: &mut super::ProxyLayers,
    pairs: &mut Vec<(u32, u32)>,
    before: impl FnOnce() -> bool,
) {
    let chunk = fork.chunk(BUILD_CHUNK, BUILD_CHUNK_FORCED);
    let workers = fork.workers();
    grid.prepare();
    let chunks = bodies.div_ceil(chunk);
    let (data, remapped) = workers.fork_with(
        super::pool::ForkData {
            stage: super::pool::Stage::Build {
                bodies,
                chunk,
                chunks,
            },
            grid: std::mem::take(grid),
            pair_flags: std::mem::take(pair_flags),
            pair_bounds: std::mem::take(pair_bounds),
            pair_aabbs: std::mem::take(pair_aabbs),
            layers: std::mem::take(layers),
            ..Default::default()
        },
        |data, scratch, _| {
            let remapped = before();
            build_work(data, scratch, bodies, chunk, chunks);
            remapped
        },
    );
    #[cfg(test)]
    {
        workers.remaps_overlapped += usize::from(remapped);
    }
    #[cfg(not(test))]
    let _ = remapped;
    *grid = data.grid;
    *pair_flags = data.pair_flags;
    *pair_bounds = data.pair_bounds;
    *pair_aabbs = data.pair_aabbs;
    *layers = data.layers;
    workers.index_chunks(chunks, |scratch| &scratch.spans);
    for &(participant, start, count) in &workers.chunk_at {
        let from = &workers.scratches[participant as usize].pairs;
        pairs.extend_from_slice(&from[start as usize..(start + count) as usize]);
    }
}

impl PairGrid {
    pub(super) fn cell_size(&self) -> f32 {
        self.bodies.cell_size()
    }

    /// Build the staged entries of the grids a walk reads, as the first
    /// [`Self::for_each_run_in`] would, so
    /// [`Self::for_each_run_in_marks`] finds them built.
    pub(super) fn prepare(&mut self) {
        self.bodies.prepare();
        if self.split {
            self.tiles.prepare();
        }
    }

    /// [`Self::for_each_run_in`] over prepared grids through `&self`, with a
    /// participant's marks: the same runs in the same order.
    #[inline(always)]
    pub(super) fn for_each_run_in_marks(
        &self,
        body_marks: &mut QueryMarks,
        tile_marks: &mut QueryMarks,
        query: &Aabb,
        visit: impl FnMut(Run<'_>),
    ) {
        if !self.split {
            self.bodies
                .for_each_run_in_shared(body_marks, query, 0, visit);
            return;
        }
        for_each_run_in_both_shared(
            &self.bodies,
            &self.tiles,
            body_marks,
            tile_marks,
            self.tile_base,
            query,
            visit,
        );
    }

    /// Walk the proxies overlapping `query` a [`Run`] at a time, as one
    /// grid would: the entity grid's walk when it holds the tiles too, else
    /// both grids' walk. The pair passes loop over each run themselves
    /// ([`Run::each`]), so their tests of a candidate compile into that
    /// loop wherever the walk hands a run over.
    #[inline(always)]
    pub(super) fn for_each_run_in(&mut self, query: &Aabb, visit: impl FnMut(Run<'_>)) {
        if !self.split {
            self.bodies.for_each_run_in(query, 0, visit);
            return;
        }
        for_each_run_in_both(
            &mut self.bodies,
            &mut self.tiles,
            self.tile_base,
            query,
            visit,
        );
    }

    /// Collect the proxies overlapping `query`, in
    /// [`Self::for_each_run_in`]'s order.
    pub(super) fn query(&mut self, query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>) {
        out.clear();
        self.for_each_run_in(query, |mut run| {
            run.each(exclude, |id| out.push(id));
        });
    }

    /// Stage `tiles`' proxies at `cell_size`, each box grown by `radius`,
    /// and their [`box_bounds`] into `bounds`, unless the grid holds them so
    /// already.
    pub(super) fn stage_tiles(
        &mut self,
        tiles: &TileCache,
        cell_size: f32,
        radius: f32,
        bounds: &mut Vec<Vec4>,
    ) {
        let key = (tiles.version(), cell_size.to_bits(), radius.to_bits());
        if self.staged == Some(key) {
            return;
        }
        self.tiles.set_cell_size(cell_size);
        self.tile_aabbs.clear();
        bounds.clear();
        for (ordinal, proxy) in tiles.proxies.iter().enumerate() {
            let mut aabb = proxy.world_aabb();
            aabb.half_extents += Vec2::splat(radius);
            self.tile_aabbs.push(aabb);
            bounds.push(box_bounds(&aabb));
            self.tiles.insert(ordinal as ProxyId, &aabb);
        }
        self.staged = Some(key);
    }
}

/// A pair box's bounds as the pair tests read them, computed once where the
/// box is set: its min in `x, y` and its negated max in `z, w`, the sums
/// [`Aabb::overlaps`] takes from the box.
pub(super) fn box_bounds(aabb: &Aabb) -> Vec4 {
    let (min, max) = (aabb.min(), aabb.max());
    Vec4::new(min.x, min.y, -max.x, -max.y)
}

/// A querier's side of [`box_bounds`]: its max and its negated min. Each
/// lane above a candidate's bounds is one of [`Aabb::overlaps`]' four strict
/// tests, since negation is exact, so one compare answers as it does.
pub(super) fn query_bounds(aabb: &Aabb) -> Vec4 {
    let (min, max) = (aabb.min(), aabb.max());
    Vec4::new(max.x, max.y, -min.x, -min.y)
}

/// [`Aabb::overlaps`] of a querier's box and a candidate's.
#[inline(always)]
pub(super) fn bounds_overlap(query: Vec4, bounds: Vec4) -> bool {
    query.cmpgt(bounds).all()
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

/// A set of proxy pairs, either way round: open addressing, grown as it
/// fills, kept across steps so a steady world allocates nothing for it.
#[derive(Debug, Default)]
pub(super) struct PairSet {
    /// A pair's lower index in the high half, its higher in the low one;
    /// [`Self::EMPTY`] for a free slot, which no pair makes.
    slots: Vec<u64>,
    len: usize,
}

impl PairSet {
    const EMPTY: u64 = u64::MAX;

    fn key(a: u32, b: u32) -> u64 {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        (u64::from(lo) << 32) | u64::from(hi)
    }

    fn slot(&self, key: u64) -> usize {
        let hash = key.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        ((hash ^ (hash >> 32)) & (self.slots.len() as u64 - 1)) as usize
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Empty the set and give it room for `pairs` of them, so that a step
    /// which records no more than that grows nothing.
    pub(super) fn reset(&mut self, pairs: usize) {
        let capacity = (pairs * 2).next_power_of_two().max(64);
        if self.slots.len() < capacity {
            self.slots.clear();
            self.slots.resize(capacity, Self::EMPTY);
        } else if self.len > 0 {
            self.slots.fill(Self::EMPTY);
        }
        self.len = 0;
    }

    pub(super) fn contains(&self, a: u32, b: u32) -> bool {
        if self.len == 0 {
            return false;
        }
        let key = Self::key(a, b);
        let mut at = self.slot(key);
        loop {
            if self.slots[at] == key {
                return true;
            }
            if self.slots[at] == Self::EMPTY {
                return false;
            }
            at = (at + 1) & (self.slots.len() - 1);
        }
    }

    /// Add the pair; false when the set held it.
    pub(super) fn insert(&mut self, a: u32, b: u32) -> bool {
        if (self.len + 1) * 2 > self.slots.len() {
            self.grow();
        }
        let key = Self::key(a, b);
        let mut at = self.slot(key);
        loop {
            if self.slots[at] == key {
                return false;
            }
            if self.slots[at] == Self::EMPTY {
                self.slots[at] = key;
                self.len += 1;
                return true;
            }
            at = (at + 1) & (self.slots.len() - 1);
        }
    }

    /// Double the slots, 64 at least, and place the pairs again.
    fn grow(&mut self) {
        let capacity = (self.slots.len() * 2).max(64);
        let old = std::mem::replace(&mut self.slots, vec![Self::EMPTY; capacity]);
        for key in old {
            if key != Self::EMPTY {
                let mut at = self.slot(key);
                while self.slots[at] != Self::EMPTY {
                    at = (at + 1) & (capacity - 1);
                }
                self.slots[at] = key;
            }
        }
    }
}

/// The step's record of the pairs that sent their `CollisionEvent` (D-270):
/// a pair sends one a step, the first time the pair loop finds its contact
/// penetrating or the safety net clamps it. The record lasts the step,
/// through its pair builds and repairs, whether or not the contact stays,
/// where the carried impulses forget a contact absent for a substep.
#[derive(Debug, Default)]
pub(super) struct SentRecord {
    /// One flag a pair of the pair list, beside it: the pair sent.
    pub(super) pairs: Vec<bool>,
    /// The pairs that sent and that a later producer has to find by their
    /// proxies: every pair with a side the safety net clamps against, a
    /// static body or a tile, which it may meet outside the pair list, and
    /// every pair that left the list at a repair or a rebuild, which may
    /// come back.
    pub(super) found: PairSet,
}

impl SentRecord {
    /// A new step over `proxies` proxies: no pair sent. The pairs to find
    /// by their proxies are a few a body at a boundary, so room for one a
    /// proxy holds a step's without growing.
    pub(super) fn begin_step(&mut self, proxies: usize) {
        self.pairs.fill(false);
        self.found.reset(proxies);
    }

    /// The pair at `index` of the list, `(a, b)`, finds its contact
    /// penetrating: whether it sends its event now. `net_side` says the
    /// safety net clamps against `b`, so it may have sent the pair's.
    #[inline]
    pub(super) fn send(&mut self, index: usize, a: u32, b: u32, net_side: bool) -> bool {
        if std::mem::replace(&mut self.pairs[index], true) {
            return false;
        }
        !net_side || self.found.insert(a, b)
    }

    /// Before a build clears the list: the pairs that sent are found by
    /// their proxies from here on.
    fn leave(&mut self, pairs: &[(u32, u32)]) {
        for (&(a, b), &sent) in pairs.iter().zip(&self.pairs) {
            if sent {
                self.found.insert(a, b);
            }
        }
    }

    /// After a build: each pair's flag, from the pairs that sent.
    fn enter(&mut self, pairs: &[(u32, u32)]) {
        self.pairs.clear();
        if self.found.is_empty() {
            self.pairs.resize(pairs.len(), false);
        } else {
            self.pairs
                .extend(pairs.iter().map(|&(a, b)| self.found.contains(a, b)));
        }
    }
}

/// One synchronized contact in an [`ImpulseCarry`] run.
#[derive(Debug, Clone, Copy, Default)]
struct Carried {
    /// The contact's higher proxy index.
    other: u32,
    /// Its normal impulse, 0 unless positive, and its tangent impulse, as
    /// the keyed maps read an absent pair.
    normal: f32,
    tangent: f32,
}

/// Scratch for [`ImpulseCarry::remap`] (`D-299`), and whether the tile
/// proxies' keys are distinct, found once a tile version.
#[derive(Debug, Default)]
pub(super) struct CarryRemap {
    /// By entity index: the proxy that entity's body is now, plus one; 0
    /// where no gathered body has the index.
    new_of: Vec<u32>,
    /// The carry's next run bounds and entries, swapped with its own.
    starts: Vec<u32>,
    entries: Vec<Carried>,
    /// The tile proxies' keys, sorted to find a repeated one.
    tile_keys: Vec<u64>,
    /// The tile version `distinct` was found for.
    checked: Option<u64>,
    distinct: bool,
}

/// The last synchronized contacts' impulses by proxy index (D-076's carry,
/// D-269): each contact with a positive normal impulse or a tangent impulse,
/// in a run under its lower proxy index, which a seed scans for its higher
/// one. A proxy keeps its index while the gathered bodies and the tiles stay
/// as they were, which the frame's first pair build checks key by key. When
/// the bodies changed and the tiles did not, the entries' indices are
/// rewritten to the new proxies' and seeds go on reading by index (`D-299`);
/// when the tiles changed, or two of them share a key, the entries move to
/// the keyed maps under the keys they were taken with, and that build seeds
/// from those.
#[derive(Debug, Default)]
pub(super) struct ImpulseCarry {
    /// `starts[lo]..starts[lo + 1]` bounds proxy `lo`'s run in `entries`.
    starts: Vec<u32>,
    entries: Vec<Carried>,
    /// Each proxy's key as of the frame's first pair build, which the
    /// entries' indices mean; with the entity proxies' count and the tile
    /// cache's version then.
    keys: Vec<u64>,
    bodies: usize,
    tile_version: u64,
    /// The proxies changed and no synchronization followed yet: the entries
    /// are in the keyed maps, and seeds read those.
    keyed: bool,
    /// Tests: the pair builds that found the proxies as they were, those
    /// that moved the entries to the keyed maps, and those that rewrote
    /// their indices.
    #[cfg(test)]
    pub(super) kept: usize,
    #[cfg(test)]
    pub(super) rekeyed: usize,
    #[cfg(test)]
    pub(super) remapped: usize,
    /// Tests: `remap` sends its first surviving entry to the wrong proxy,
    /// which the reference model must catch.
    #[cfg(test)]
    pub(super) remap_wrong: bool,
}

impl ImpulseCarry {
    /// Take `contacts`' impulses, and `friction`'s for the contacts it
    /// rides, as the carry of `proxies` proxies: a counting sort by lower
    /// index that keeps contact order within a run.
    fn take(
        &mut self,
        proxies: usize,
        contacts: &[super::ContactConstraint],
        friction: &[super::FrictionConstraint],
    ) {
        self.keyed = false;
        self.starts.clear();
        self.starts.resize(proxies + 1, 0);
        // Friction rows are in contact order: one cursor follows them.
        let tangent_of = |row: &mut usize, index: usize| match friction.get(*row) {
            Some(friction) if friction.contact as usize == index => {
                *row += 1;
                friction.impulse
            }
            _ => 0.0,
        };
        let mut row = 0;
        let mut carried = 0;
        for (index, contact) in contacts.iter().enumerate() {
            let tangent = tangent_of(&mut row, index);
            if contact.impulse > 0.0 || tangent != 0.0 {
                self.starts[contact.a.min(contact.b) as usize] += 1;
                carried += 1;
            }
        }
        // Counts become each run's end, which the fill walks back to its
        // start.
        let mut end = 0;
        for count in &mut self.starts {
            end += *count;
            *count = end;
        }
        // No clear first: the fill below writes every one of the `carried`
        // slots, so the entries left from the last take need no reset.
        self.entries.resize(carried, Carried::default());
        let mut row = friction.len();
        for (index, contact) in contacts.iter().enumerate().rev() {
            let tangent = match row.checked_sub(1).map(|last| &friction[last]) {
                Some(friction) if friction.contact as usize == index => {
                    row -= 1;
                    friction.impulse
                }
                _ => 0.0,
            };
            if contact.impulse > 0.0 || tangent != 0.0 {
                let start = &mut self.starts[contact.a.min(contact.b) as usize];
                *start -= 1;
                self.entries[*start as usize] = Carried {
                    other: contact.a.max(contact.b),
                    normal: if contact.impulse > 0.0 {
                        contact.impulse
                    } else {
                        0.0
                    },
                    tangent: if tangent == 0.0 { 0.0 } else { tangent },
                };
            }
        }
    }

    /// The carried normal and tangent impulses of the pair of proxies `a`
    /// and `b`, zeros when it carried none.
    #[inline]
    pub(super) fn get(&self, a: u32, b: u32) -> (f32, f32) {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let Some(&[start, end]) = self.starts.get(lo as usize..lo as usize + 2).map(|run| {
            let run: &[u32; 2] = run.try_into().expect("two bounds");
            run
        }) else {
            return (0.0, 0.0);
        };
        for entry in &self.entries[start as usize..end as usize] {
            if entry.other == hi {
                return (entry.normal, entry.tangent);
            }
        }
        (0.0, 0.0)
    }

    /// Whether seeds read the keyed maps: the proxies changed since the
    /// entries were taken.
    pub(super) fn keyed(&self) -> bool {
        self.keyed
    }

    /// Tests: no contact carried an impulse.
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether `proxies` and `tiles` are what the entries' indices mean,
    /// but for the entity proxies' keys, which the pair build compares in
    /// its own pass over them.
    fn same_counts(&self, proxies: usize, bodies: usize, tile_version: u64) -> bool {
        self.keys.len() == proxies && self.bodies == bodies && self.tile_version == tile_version
    }

    /// The entity proxies changed and the tiles did not: rewrite the
    /// entries' indices from the proxies they were taken from to `proxies`,
    /// dropping each entry a side of which left, and take `proxies`' keys,
    /// so that seeds go on reading by index (`D-299`). An entity proxy is
    /// found by its key through a table by entity index; a tile proxy keeps
    /// its place after the entity proxies. A seed then reads what a seed by
    /// key reads: entity keys are unique among the gathered bodies and never
    /// a tile's, and a pair whose side left reads 0 either way.
    ///
    /// Returns false, having changed nothing, where a seed by key could read
    /// otherwise, which [`Self::rekey`] then serves: the tiles changed, or
    /// two tile proxies share a key, which the keyed maps fold into one
    /// entry. Out of line: it runs at most once a frame.
    #[inline(never)]
    fn remap(
        &mut self,
        proxies: &[super::Proxy],
        bodies: usize,
        tile_version: u64,
        scratch: &mut CarryRemap,
    ) -> bool {
        let old_bodies = self.bodies;
        let tiles = proxies.len() - bodies;
        if self.tile_version != tile_version || self.keys.len() != old_bodies + tiles {
            return false;
        }
        if scratch.checked != Some(tile_version) {
            scratch.tile_keys.clear();
            scratch
                .tile_keys
                .extend(proxies[bodies..].iter().map(|proxy| proxy.key));
            scratch.tile_keys.sort_unstable();
            scratch.distinct = scratch.tile_keys.windows(2).all(|pair| pair[0] != pair[1]);
            scratch.checked = Some(tile_version);
        }
        if !scratch.distinct {
            return false;
        }
        // An entity key's low half is the entity's index (`gather::entity_key`).
        scratch.new_of.clear();
        for (index, proxy) in proxies[..bodies].iter().enumerate() {
            let entity = proxy.key as u32 as usize;
            if scratch.new_of.len() <= entity {
                scratch.new_of.resize(entity + 1, 0);
            }
            scratch.new_of[entity] = index as u32 + 1;
        }
        let (keys, new_of) = (&self.keys, &scratch.new_of);
        let moved = |old: usize| -> Option<u32> {
            if old >= old_bodies {
                return Some((bodies + (old - old_bodies)) as u32);
            }
            let key = keys[old];
            let new = new_of.get(key as u32 as usize)?.checked_sub(1)?;
            (proxies[new as usize].key == key).then_some(new)
        };
        // A counting sort by the new lower index, as `take`'s: the counts
        // become each run's end, which the fill walks back to its start.
        scratch.starts.clear();
        scratch.starts.resize(proxies.len() + 1, 0);
        let mut kept = 0;
        for (lo, bounds) in self.starts.windows(2).enumerate() {
            if bounds[0] == bounds[1] {
                continue;
            }
            let Some(new_lo) = moved(lo) else {
                continue;
            };
            for entry in &self.entries[bounds[0] as usize..bounds[1] as usize] {
                if let Some(new_hi) = moved(entry.other as usize) {
                    scratch.starts[new_lo.min(new_hi) as usize] += 1;
                    kept += 1;
                }
            }
        }
        let mut end = 0;
        for count in &mut scratch.starts {
            end += *count;
            *count = end;
        }
        scratch.entries.clear();
        scratch.entries.resize(kept, Carried::default());
        #[cfg(test)]
        let mut wrong = self.remap_wrong;
        for (lo, bounds) in self.starts.windows(2).enumerate().rev() {
            if bounds[0] == bounds[1] {
                continue;
            }
            let Some(new_lo) = moved(lo) else {
                continue;
            };
            for entry in self.entries[bounds[0] as usize..bounds[1] as usize]
                .iter()
                .rev()
            {
                let Some(new_hi) = moved(entry.other as usize) else {
                    continue;
                };
                let start = &mut scratch.starts[new_lo.min(new_hi) as usize];
                *start -= 1;
                #[allow(unused_mut)]
                let mut other = new_lo.max(new_hi);
                #[cfg(test)]
                if std::mem::take(&mut wrong) {
                    other = other.wrapping_add(1);
                }
                scratch.entries[*start as usize] = Carried {
                    other,
                    normal: entry.normal,
                    tangent: entry.tangent,
                };
            }
        }
        std::mem::swap(&mut self.starts, &mut scratch.starts);
        std::mem::swap(&mut self.entries, &mut scratch.entries);
        self.keys.clear();
        self.keys.extend(proxies.iter().map(|proxy| proxy.key));
        self.bodies = bodies;
        self.keyed = false;
        #[cfg(test)]
        {
            self.remapped += 1;
        }
        true
    }

    /// The tiles changed, or two share a key: move the entries to
    /// `impulses` and `tangents` under the keys they were taken with, as a
    /// synchronization by key leaves them, and take `proxies`' keys for the
    /// next ones.
    fn rekey(
        &mut self,
        proxies: &[super::Proxy],
        bodies: usize,
        tile_version: u64,
        impulses: &mut ImpulseMap,
        tangents: &mut ImpulseMap,
    ) {
        impulses.reset(self.entries.len());
        let any_tangent = self.entries.iter().any(|entry| entry.tangent != 0.0);
        if any_tangent {
            tangents.reset(self.entries.len());
        } else {
            tangents.clear();
        }
        for (lo, bounds) in self.starts.windows(2).enumerate() {
            for entry in &self.entries[bounds[0] as usize..bounds[1] as usize] {
                let key = pair_key(self.keys[lo], self.keys[entry.other as usize]);
                if entry.normal > 0.0 {
                    impulses.insert(key, entry.normal);
                }
                if entry.tangent != 0.0 {
                    tangents.insert(key, entry.tangent);
                }
            }
        }
        self.keys.clear();
        self.keys.extend(proxies.iter().map(|proxy| proxy.key));
        self.bodies = bodies;
        self.tile_version = tile_version;
        self.keyed = true;
        #[cfg(test)]
        {
            self.rekeyed += 1;
        }
    }

    /// Tests: the entries' normal and tangent impulses by pair key, as the
    /// keyed maps held them.
    #[cfg(test)]
    pub(super) fn by_key(&self) -> (ImpulseMap, ImpulseMap) {
        let (mut normals, mut tangents) = (ImpulseMap::default(), ImpulseMap::default());
        normals.reset(self.entries.len());
        tangents.reset(self.entries.len());
        for (lo, bounds) in self.starts.windows(2).enumerate() {
            for entry in &self.entries[bounds[0] as usize..bounds[1] as usize] {
                let key = pair_key(self.keys[lo], self.keys[entry.other as usize]);
                if entry.normal > 0.0 {
                    normals.insert(key, entry.normal);
                }
                if entry.tangent != 0.0 {
                    tangents.insert(key, entry.tangent);
                }
            }
        }
        (normals, tangents)
    }

    /// Tests: the entries that carry a tangent impulse.
    #[cfg(test)]
    pub(super) fn tangents(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.tangent != 0.0)
            .count()
    }
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
/// tripped or woken proxy in this many awake bodies rebuilds instead (D-081,
/// D-234).
const REBUILD_ONE_IN: usize = 4;

/// `PhysicsBuffers::pair_flags` bit: a moving proxy (an awake dynamic or a
/// kinematic body) at its last build or repair; a moving proxy without it
/// is one a contact woke since (D-234).
pub(super) const PAIR_AWAKE: u8 = 1;
/// `pair_flags` bit: member of the repair in progress.
const PAIR_TRIPPED: u8 = 1 << 1;
/// `pair_flags` bit: repaired since the last build, so the pair grid still
/// holds the cells of its old AABB.
pub(super) const PAIR_REPAIRED: u8 = 1 << 2;
/// `pair_flags` bit: a moving proxy the last build left out of the pair
/// grid, its pair inflation over [`OVERSIZE_CELLS`] cells (D-081).
const PAIR_OVERSIZE: u8 = 1 << 3;

/// A moving proxy whose pair inflation exceeds this many pair-grid cells is
/// oversize (D-081): the build leaves it out of the grid and finds its pairs
/// in one pass over every proxy's bounds, a repair's members test the
/// oversize list beside their grid walks, and one that trips, or a member
/// that would become one, rebuilds. No speed stages more than this many
/// cells around a box.
pub(super) const OVERSIZE_CELLS: f32 = 64.0;

/// Travel gravity can add beyond `|v|·time_left` before the frame's last
/// budget check: `|g|·h²·(n−1)(n+2)/2` with `n = time_left / h` substeps
/// left (D-081). The integrator adds `g·h` to the velocity before it moves a
/// body, so the last check sees `n − 1` substeps of travel plus one more of
/// predicted travel at the speed gravity has built up by then.
pub(super) fn gravity_allowance(config: &PhysicsConfig, sub_dt: f32, time_left: f32) -> f32 {
    0.5 * config.gravity.length() * (time_left - sub_dt).max(0.0) * (time_left + 2.0 * sub_dt)
}

/// The share of the world's gravity allowance the budget of the entity
/// proxy at `index` takes (D-081): its own gravity scale's size, at least 1,
/// so a body falling faster than the world's gravity keeps its pairs and no
/// other body's budget changes; at scales up to 1 it is the world's, bit for
/// bit.
fn gravity_lift(gravity_scales: &[f32], index: usize) -> f32 {
    gravity_scales
        .get(index)
        .map_or(1.0, |scale| scale.abs().max(1.0))
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

/// Keep the pair list complete for the coming narrow phase (D-081, D-234):
/// build it when it is invalid, reuse it while every budget holds and no
/// contact woke a sleeper, and otherwise repair the tripped and woken
/// proxies, or rebuild when too many need it.
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
    if buffers.tripped.len() * REBUILD_ONE_IN > awake
        || trips_oversize(config, sub_dt, time_left, buffers)
    {
        build_pairs(config, sub_dt, time_left, buffers);
    } else {
        repair_pairs(config, sub_dt, time_left, buffers);
    }
}

/// Whether a tripped proxy is oversize or its fresh inflation would make it
/// one, which a repair does not handle.
fn trips_oversize(
    config: &PhysicsConfig,
    sub_dt: f32,
    time_left: f32,
    buffers: &PhysicsBuffers,
) -> bool {
    let cap = OVERSIZE_CELLS * buffers.grid.cell_size();
    let gravity_allowance = gravity_allowance(config, sub_dt, time_left);
    buffers.tripped.iter().any(|&member| {
        if buffers.pair_flags[member as usize] & PAIR_OVERSIZE != 0 {
            return true;
        }
        let proxy = &buffers.proxies[member as usize];
        let gravity = if proxy.kinematic {
            0.0
        } else {
            gravity_allowance * gravity_lift(&buffers.extra.gravity_scales, member as usize)
        };
        awake_pair_radius(config, proxy.velocity.length(), time_left, gravity) > cap
    })
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
    // Rebuilds can reorder/reverse pairs or introduce new ones. Transfer only
    // the immediately preceding substep's live impulses, by proxy index
    // (D-269).
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
        impulses,
        extra,
        ..
    } = buffers;
    let super::StepExtra {
        gravity_scales,
        pair_bounds,
        tile_bounds,
        oversize,
        carry,
        carry_remap,
        tangents,
        sent,
        fork,
        ..
    } = &mut **extra;
    let bodies = *bodies;
    // The frame's first build checks that the carry's indices still name
    // the proxies they were taken from: the counts and the tiles here, each
    // entity proxy's key in the pass below.
    sent.leave(pairs);
    let check_carry = *pairs_invalidated || carry.keys.len() != proxies.len();
    let mut same = carry.same_counts(proxies.len(), bodies, tiles.version());
    let carried_keys = if check_carry && same {
        &carry.keys[..bodies]
    } else {
        &[]
    };
    if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
        grid.bodies.set_cell_size(config.broadphase_cell_size);
    }
    oversize.clear();
    let oversize_radius = OVERSIZE_CELLS * grid.cell_size();
    grid.bodies.clear();
    pair_aabbs.clear();
    pair_bounds.clear();
    pair_budgets.clear();
    pair_flags.clear();
    repaired.clear();
    pairs.clear();
    static_grid.statics = 0;
    let static_radius = 2.0 * config.linear_slop;
    let gravity_allowance = gravity_allowance(config, sub_dt, time_left);
    let mut moving = 0;
    for (id, proxy) in proxies[..bodies].iter().enumerate() {
        if let Some(&key) = carried_keys.get(id) {
            same &= key == proxy.key;
        }
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
                    gravity_allowance * gravity_lift(gravity_scales, id)
                },
            )
        } else {
            static_radius
        };
        let mut aabb = proxy.world_aabb();
        aabb.half_extents += Vec2::splat(radius);
        pair_aabbs.push(aabb);
        pair_bounds.push(box_bounds(&aabb));
        pair_budgets.push(PairBudget {
            radius,
            travel: 0.0,
        });
        if awake && radius > oversize_radius {
            pair_flags.push(PAIR_AWAKE | PAIR_OVERSIZE);
            oversize.push(id as u32);
        } else {
            pair_flags.push(if awake { PAIR_AWAKE } else { 0 });
            grid.bodies.insert(id as ProxyId, &aabb);
        }
    }
    // A build that forks remaps the carry on its own thread while the
    // workers walk (OVERLAP-1): the walks read none of what it writes.
    let forks = fork.splits(moving, BUILD_FORK_MIN);
    let remap_due = check_carry && !same;
    if check_carry && same {
        #[cfg(test)]
        {
            carry.kept += 1;
        }
    } else if remap_due && !forks && !carry.remap(proxies, bodies, tiles.version(), carry_remap) {
        carry.rekey(proxies, bodies, tiles.version(), impulses, tangents);
    }
    // The tiles: static, never awake, grown by the static radius; staged
    // once for the cache and kept across builds, and into the entity grid
    // too, after the entity proxies, when the moving proxies' queries
    // outnumber them.
    grid.stage_tiles(tiles, grid.cell_size(), static_radius, tile_bounds);
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
    pair_bounds.extend_from_slice(tile_bounds);
    pair_budgets.resize(
        proxies.len(),
        PairBudget {
            radius: static_radius,
            travel: 0.0,
        },
    );
    pair_flags.resize(proxies.len(), 0);
    if forks {
        let tile_version = tiles.version();
        build_walks_forked(
            fork,
            bodies,
            grid,
            pair_flags,
            pair_bounds,
            pair_aabbs,
            layers,
            pairs,
            || {
                if remap_due && !carry.remap(proxies, bodies, tile_version, carry_remap) {
                    carry.rekey(proxies, bodies, tile_version, impulses, tangents);
                }
                remap_due
            },
        );
    } else {
        for (a_idx, aabb) in pair_aabbs[..bodies].iter().enumerate() {
            if pair_flags[a_idx] & PAIR_AWAKE == 0 {
                continue;
            }
            let query = query_bounds(aabb);
            if pair_flags[a_idx] & PAIR_OVERSIZE != 0 {
                oversize_pairs(a_idx, query, pair_bounds, pair_flags, layers, pairs);
                continue;
            }
            grid.for_each_run_in(
                aabb,
                #[inline(always)]
                |mut run| {
                    run.each(Some(a_idx as ProxyId), |b_id| {
                        let b_idx = b_id as usize;
                        // An awake candidate at or below this index runs its own
                        // query.
                        if pair_flags[b_idx] & PAIR_AWAKE != 0 && b_idx <= a_idx {
                            return;
                        }
                        if bounds_overlap(query, pair_bounds[b_idx]) && layers.meet(a_idx, b_idx) {
                            pairs.push((a_idx as u32, b_id));
                        }
                    });
                },
            );
        }
    }
    *pairs_invalidated = false;
    sent.enter(pairs);
    // No clear first: the first substep after a build replaces every slot
    // with 0 and seeds from the carry, never from what the slot held.
    buffers.pair_impulses.resize(pairs.len(), 0.0);
    buffers.seed_pair_impulses = true;
    // Tangents ride beside the normal impulses only in a frame with friction.
    let extra = &mut *buffers.extra;
    if extra.frictions.is_empty() {
        extra.pair_tangents.clear();
    } else {
        extra.pair_tangents.resize(pairs.len(), 0.0);
    }
    // The list is new: no pair of it has joined its two bodies' islands.
    extra.joined_stale = true;
    #[cfg(test)]
    {
        buffers.pair_builds += 1;
    }
}

/// A build's pairs of the oversize proxy `a_idx` (D-081). No cell holds it
/// and no other querier meets it: one pass over every proxy in proxy order,
/// each pair stored as a build stores it, a pair of two oversize proxies by
/// the lower one. Out of line and cold, so the build's loop over the grid's
/// queriers carries none of it.
#[cold]
#[inline(never)]
fn oversize_pairs(
    a_idx: usize,
    query: Vec4,
    pair_bounds: &[Vec4],
    pair_flags: &[u8],
    layers: &super::ProxyLayers,
    pairs: &mut Vec<(u32, u32)>,
) {
    for (b_idx, &bounds) in pair_bounds.iter().enumerate() {
        let b_flags = pair_flags[b_idx];
        let lower_awake = b_flags & PAIR_AWAKE != 0 && b_idx < a_idx;
        if b_idx == a_idx
            || (lower_awake && b_flags & PAIR_OVERSIZE != 0)
            || !bounds_overlap(query, bounds)
            || !layers.meet(a_idx, b_idx)
        {
            continue;
        }
        pairs.push(if lower_awake {
            (b_idx as u32, a_idx as u32)
        } else {
            (a_idx as u32, b_idx as u32)
        });
    }
}

/// Collect, in proxy order, the moving proxies whose budget does not cover
/// the coming substep and those a contact woke since their last build or
/// repair (D-234), and return the moving count. Solver impulses and gravity
/// can change velocity after a build, so predicted travel alone is
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
        pair_flags,
        tripped,
        ..
    } = buffers;
    tripped.clear();
    let slack = 2.0 * config.linear_slop;
    let mut awake = 0;
    // Tile proxies never move, so the entity proxies are every candidate.
    for (index, ((proxy, budget), &flags)) in proxies[..*bodies]
        .iter()
        .zip(pair_budgets.iter())
        .zip(pair_flags.iter())
        .enumerate()
    {
        if !proxy.moves() {
            continue;
        }
        awake += 1;
        if flags & PAIR_AWAKE == 0
            || budget.travel + proxy.velocity.length() * sub_dt + slack > budget.radius
        {
            tripped.push(index as u32);
        }
    }
    awake
}

/// Re-pair the tripped and woken proxies in place of a rebuild (D-081,
/// D-234). Every other proxy keeps its pairs, inflated AABB and budget, and
/// the pair grid is not restaged. A woken member is awake from here on, so
/// it pairs with sleepers and statics as its querier. As in a build, proxies
/// whose layers do not meet are not paired.
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
        gravity_scales,
        oversize,
        frictions,
        pair_tangents,
        repair_tangents,
        removed_tangents,
        pair_bounds,
        sent,
        fork,
        ..
    } = &mut **extra;
    let friction_on = !frictions.is_empty();
    for &member in tripped.iter() {
        pair_flags[member as usize] |= PAIR_TRIPPED | PAIR_AWAKE;
    }

    // Remove the members' pairs, keeping the order of the rest, and remember
    // each removed pair's carried impulse, and tangent in a frame with
    // friction, under its key.
    removed_impulses.clear();
    removed_tangents.clear();
    let kept = if fork.splits(pairs.len(), REMOVAL_FORK_MIN) {
        removal_forked(
            fork,
            (pairs, &mut sent.pairs, pair_impulses, pair_tangents),
            pair_flags,
            proxies,
            &mut sent.found,
            (removed_impulses, removed_tangents),
            friction_on,
        )
    } else {
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
                sent.pairs[kept] = sent.pairs[index];
                kept += 1;
            } else {
                // A removed pair that sent its event is found by its proxies
                // when a member's query brings it back (D-270).
                if sent.pairs[index] {
                    sent.found.insert(a, b);
                }
                if impulse != 0.0 || tangent != 0.0 {
                    let key = pair_key(proxies[a as usize].key, proxies[b as usize].key);
                    if impulse != 0.0 {
                        removed_impulses.push((key, impulse));
                    }
                    if tangent != 0.0 {
                        removed_tangents.push((key, tangent));
                    }
                }
            }
        }
        kept
    };
    pairs.truncate(kept);
    pair_impulses.truncate(kept);
    sent.pairs.truncate(kept);
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
                gravity_allowance * gravity_lift(gravity_scales, index)
            },
        );
        let mut aabb = proxy.world_aabb();
        aabb.half_extents += Vec2::splat(radius);
        pair_aabbs[index] = aabb;
        pair_bounds[index] = box_bounds(&aabb);
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
    if fork.splits(tripped.len(), REPAIR_FORK_MIN) {
        repair_walks_forked(
            fork,
            (grid, repair_grid),
            pair_flags,
            pair_bounds,
            pair_aabbs,
            layers,
            oversize,
            proxies,
            tripped,
            &mut sent.found,
            (repair_impulses, repair_tangents),
            (pairs, &mut sent.pairs, pair_impulses, pair_tangents),
            (friction_on, carried, carried_tangents),
        );
    } else {
        for &member in tripped.iter() {
            let aabb = &pair_aabbs[member as usize];
            let query = query_bounds(aabb);
            let member_key = proxies[member as usize].key;
            let mut admit = |a: u32, b: u32, other: u32| {
                pairs.push((a, b));
                sent.pairs.push(sent.found.contains(a, b));
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
            grid.for_each_run_in(
                aabb,
                #[inline(always)]
                |mut run| {
                    run.each(Some(member), |other| {
                        let flags = pair_flags[other as usize];
                        if flags & PAIR_REPAIRED != 0
                            || !bounds_overlap(query, pair_bounds[other as usize])
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
                },
            );
            repair_grid.for_each_run_in(
                aabb,
                0,
                #[inline(always)]
                |mut run| {
                    run.each(Some(member), |other| {
                        // A pair between two members is added by the lower one.
                        let member_too = pair_flags[other as usize] & PAIR_TRIPPED != 0;
                        if (member_too && other < member)
                            || !bounds_overlap(query, pair_bounds[other as usize])
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
                },
            );
            // The oversize proxies are in no grid and never members here.
            for &other in oversize.iter() {
                if bounds_overlap(query, pair_bounds[other as usize])
                    && layers.meet(member as usize, other as usize)
                {
                    if other < member {
                        admit(other, member, other);
                    } else {
                        admit(member, other, other);
                    }
                }
            }
        }
    }
    for &member in tripped.iter() {
        pair_flags[member as usize] &= !PAIR_TRIPPED;
    }
    // The repair moved pairs about in the list: the marks of the joined
    // ones no longer sit at their pairs.
    buffers.extra.joined_stale = true;
    #[cfg(test)]
    {
        buffers.pair_repairs += 1;
    }
}

/// Preserve D-063's immediately-previous-contact semantics, including empty
/// final substeps: the last substep's contacts become the carry, by proxy
/// index (D-076, D-269). The common case pays one counting sort a frame.
pub(super) fn sync_impulses(buffers: &mut PhysicsBuffers) {
    if !buffers.impulses_dirty {
        return;
    }
    let extra = &mut *buffers.extra;
    extra
        .carry
        .take(buffers.proxies.len(), &buffers.contacts, &extra.friction);
    buffers.impulses_dirty = false;
}
