//! Spatial queries against the last physics step: ray and shape casts,
//! overlaps and nearest surfaces, over bodies, sensors and the solid tiles
//! of collision layers.
//!
//! Every query reads one index of the last step's final positions: the exact
//! boxes of its proxies and sensors in a grid, built by the step when it has
//! sensors and otherwise by the first query after it. A body moved since the
//! step is found where the step left it, one spawned since is not found and
//! one despawned since is skipped; before the first step nothing is found.
//! Ties go to the lower index (proxies in gather order, then sensors), so
//! every answer is deterministic. A filter sees the layers its mask holds
//! ([`SpatialFilter::with_mask`]), every layer by default, and can skip
//! layers ([`SpatialFilter::without_layers`]); a tile is in its collision
//! layer's physics layers ([`TilemapData::layer_physics`]), layer 0 by
//! default.
//!
//! [`TilemapData::layer_physics`]: crate::TilemapData::layer_physics
//!
//! Tiles keep `D-064`'s seams: a cast never hits a face the tile's neighbour
//! covers, and a corner beside such a face is square, so a cast along a tile
//! floor slides past its seams. Entity colliders keep every face. A one-way
//! tile ([`TileProps::one_way`]) is a surface: a cast meets it only moving
//! down from at or above its top face, through that face or a rounded top
//! corner, and [`overlap`], [`nearest`] and sensors leave it out.
//!
//! [`TileProps::one_way`]: crate::TileProps::one_way
//!
//! ```
//! use glam::Vec2;
//! use tungsten_core::physics::{self, Collider, Position, RigidBodyBundle, SpatialFilter};
//! use tungsten_core::{Time, World};
//!
//! let mut world = World::new();
//! let mut time = Time::new();
//! time.advance_frame(1.0 / 60.0);
//! world.insert_resource(time);
//! let wall = world.spawn_with(RigidBodyBundle::r#static(
//!     Position::new(100.0, 0.0),
//!     Collider::aabb(Vec2::new(10.0, 50.0)),
//! ));
//! physics::physics_step(&mut world);
//!
//! let filter = SpatialFilter::new();
//! let hit = physics::cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &filter).unwrap();
//! assert_eq!(hit.entity, Some(wall));
//! assert_eq!(hit.point, Vec2::new(90.0, 0.0));
//! assert_eq!(hit.normal, Vec2::NEG_X);
//! assert_eq!(hit.distance, 90.0);
//! ```

use std::cell::RefCell;

use super::PhysicsConfig;
use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::{
    Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_ONE_WAY, FACE_RIGHT, FACE_TOP,
    aabb_vs_aabb_speculative, aabb_vs_circle_speculative, circle_vs_circle_speculative,
};
use super::components::{RigidBody, Shape};
use super::step::slope::{self, SlopePolygon};
use super::step::{GridCell, PhysicsBuffers, TileRun};
use crate::ecs::{Entity, World};
use glam::Vec2;

/// Slack added to every query box, in px, so a shape that touches its edge
/// exactly is still a candidate.
const QUERY_PAD: f32 = 0.01;

/// Which bodies, sensors and tiles a query sees. The default sees bodies and
/// tiles in every layer, no sensors, and excludes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialFilter {
    exclude: Option<Entity>,
    bodies: bool,
    sensors: bool,
    tiles: bool,
    mask: u32,
    /// The layers whose bodies, sensors and tiles it skips.
    without: u32,
}

impl Default for SpatialFilter {
    fn default() -> Self {
        Self {
            exclude: None,
            bodies: true,
            sensors: false,
            tiles: true,
            mask: RigidBody::ALL_LAYERS,
            without: 0,
        }
    }
}

impl SpatialFilter {
    /// The default filter: bodies and tiles in every layer, no sensors,
    /// nothing excluded.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The filter, skipping `entity` as well, such as the body a cast moves.
    /// It replaces an earlier exclusion.
    #[must_use]
    pub fn excluding(mut self, entity: Entity) -> Self {
        self.exclude = Some(entity);
        self
    }

    /// The filter, seeing static, dynamic and kinematic bodies, and colliders
    /// without a body, only when `on`.
    #[must_use]
    pub fn with_bodies(mut self, on: bool) -> Self {
        self.bodies = on;
        self
    }

    /// The filter, seeing sensors only when `on`.
    #[must_use]
    pub fn with_sensors(mut self, on: bool) -> Self {
        self.sensors = on;
        self
    }

    /// The filter, seeing the solid tiles of collision layers only when `on`.
    #[must_use]
    pub fn with_tiles(mut self, on: bool) -> Self {
        self.tiles = on;
        self
    }

    /// The filter, seeing only the bodies, sensors and tiles in a layer
    /// `mask` holds, one bit per layer ([`RigidBody::layers`]); a tile is in
    /// its collision layer's layers. It replaces an earlier mask; the
    /// default holds every layer. Sensors still need
    /// [`with_sensors`](Self::with_sensors).
    #[must_use]
    pub fn with_mask(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }

    /// The filter, skipping the bodies, sensors and tiles in any layer
    /// `layers` holds, one bit per layer, beside
    /// [`with_mask`](Self::with_mask)'s rule: a tile in layers 0 and 1 is
    /// skipped without layer 1 and seen with a mask of layer 0. It replaces
    /// earlier skipped layers; the default skips none.
    #[must_use]
    pub fn without_layers(mut self, layers: u32) -> Self {
        self.without = layers;
        self
    }

    /// The entry passes the filter and its entity is still alive.
    fn admits(&self, entry: &IndexEntry, world: &World) -> bool {
        if entry.layers & self.mask == 0 || entry.layers & self.without != 0 {
            return false;
        }
        match entry.entity {
            None => self.tiles,
            Some(entity) => {
                (if entry.sensor {
                    self.sensors
                } else {
                    self.bodies
                }) && self.exclude != Some(entity)
                    && world.is_alive(entity)
            }
        }
    }
}

/// Where a query met a body or a tile.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SpatialHit {
    /// The body met; `None` for a tile.
    pub entity: Option<Entity>,
    /// Where the shapes first touch. For a cast that starts inside a shape,
    /// the cast shape's extreme point along `-normal`; for [`nearest`], the
    /// closest surface point.
    pub point: Vec2,
    /// The outward unit normal of the surface met; zero when [`nearest`]'s
    /// point is inside the shape.
    pub normal: Vec2,
    /// How far the ray or the cast shape's centre travelled; for
    /// [`nearest`], the distance to the surface.
    pub distance: f32,
    /// The layers of the body or tile met, one bit per layer: a body's
    /// [`RigidBody::layers`], a tile's those of its collision layer.
    pub layers: u32,
    /// The tile cell met; `None` for a body.
    pub tile: Option<TileHit>,
}

/// The tile cell a query met: its map's entity, its collision layer and
/// its column and row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct TileHit {
    /// The entity whose `TilemapInstance` places the map.
    pub map: Entity,
    /// The collision layer's index in the map's layers.
    pub layer: usize,
    /// The cell's column, from the map's left.
    pub col: u32,
    /// The cell's row, from the map's top.
    pub row: u32,
}

/// One shape in the index, at the last step's final position.
#[derive(Debug, Clone, Copy)]
pub(super) struct IndexEntry {
    /// The body; `None` for a tile.
    pub(super) entity: Option<Entity>,
    pub(super) center: Vec2,
    pub(super) shape: Shape,
    /// A tile's exposed faces; `FACE_ALL` for an entity collider.
    pub(super) face_mask: u8,
    /// A slope tile's kind, 1 to 12, its polygon meeting queries in place of
    /// its box; 0 for a full tile or a body.
    pub(super) slope: u8,
    pub(super) sensor: bool,
    /// The body's key or the tile's identity (the low half of a sensor
    /// pair's key); a sensor's key (the high half).
    pub(super) identity: u64,
    /// The layers it is in; a tile's are its collision layer's.
    pub(super) layers: u32,
    /// The layers it meets; a sensor reports what this holds.
    pub(super) mask: u32,
}

impl IndexEntry {
    fn aabb(&self) -> Aabb {
        Aabb::new(self.center, half_extents(self.shape))
    }

    /// A slope tile's polygon in its box.
    fn polygon(&self) -> Option<SlopePolygon> {
        (self.slope != 0).then(|| slope::polygon(self.slope, self.center, half_extents(self.shape)))
    }
}

/// The index every query reads, a `World` resource holding its state in a
/// `RefCell` so queries take `&World` (`D-086`'s pattern). `physics_step`
/// inserts it and marks it stale; the first query after a step rebuilds it
/// from the step's proxies.
#[derive(Debug, Default)]
pub(super) struct SpatialIndex(RefCell<IndexState>);

impl SpatialIndex {
    /// Called by every step: the next query sees its final positions.
    pub(super) fn mark_stale(world: &mut World) {
        if let Some(index) = world.get_resource_mut::<Self>() {
            index.0.get_mut().fresh = false;
        } else {
            world.insert_resource(Self::default());
        }
    }

    /// The step's sensor pass: builds the index of `buffers` and calls
    /// `visit` with each sensor, in gather order, and each body in a layer
    /// the sensor's mask holds that it overlaps with penetration above 0, in
    /// index order, then each such solid tile cell of the step's grids, in
    /// tile order, through `visit_tile` with the cell's map: every cell
    /// under the sensor, or, when `tiles_known(index, sensor)` says this
    /// sensor's tile pairs are the last step's, only the cells the step's
    /// refresh changed. Sensors do not detect sensors. Returns the cells
    /// tested; the step's queries reuse the index.
    pub(super) fn sensor_overlaps(
        world: &World,
        buffers: &PhysicsBuffers,
        swept: impl Fn(usize) -> bool,
        mut tiles_known: impl FnMut(usize, &IndexEntry) -> bool,
        mut visit: impl FnMut(&IndexEntry, &IndexEntry),
        mut visit_tile: impl FnMut(usize, &IndexEntry, &IndexEntry, TileHit),
    ) -> usize {
        with_index_of(world, buffers, |state| {
            let mut tested = 0;
            let sensors = state.tile_base..state.bodies.len();
            for (index, at) in sensors.enumerate() {
                // A swept sensor's pairs are what it met over its move.
                if swept(index) {
                    continue;
                }
                let sensor = state.bodies[at];
                let known = tiles_known(index, &sensor);
                state.gather_parts(&sensor.aabb(), false);
                state.candidates.sort_unstable();
                for &id in &state.candidates {
                    let other = state.entry(id);
                    if other.sensor
                        || sensor.mask & other.layers == 0
                        || !shapes_overlap(sensor.shape, sensor.center, other.shape, other.center)
                    {
                        continue;
                    }
                    visit(&sensor, other);
                }
                let (min, max) = padded(&sensor.aabb());
                let mut test = |cell: &GridCell| {
                    tested += 1;
                    let tile = cell_entry(cell);
                    if sensor.mask & tile.layers != 0
                        && entry_overlaps(sensor.shape, sensor.center, &tile)
                    {
                        visit_tile(index, &sensor, &tile, cell.cell);
                    }
                };
                if known {
                    buffers.grid_cells().changed_cells_in(min, max, &mut test);
                } else {
                    buffers.grid_cells().cells_in(min, max, &mut test);
                }
            }
            tested
        })
        .unwrap_or(0)
    }
}

#[derive(Debug, Default)]
struct IndexState {
    /// Built from the last step's proxies; every step clears it.
    fresh: bool,
    /// The entity proxies, then the sensors: every entry but the tiles. An
    /// entity proxy's id is its place; a sensor's comes after the tiles'.
    bodies: Vec<IndexEntry>,
    /// The entity proxy count: the first tile's id.
    tile_base: usize,
    /// The entity proxies' exact boxes, by id. Sensors are tested against a
    /// query's box directly: a few, often large.
    grid: SpatialGrid,
    /// The tiles' entries and grid, kept while the step's tiles are.
    tiles: TileIndex,
    /// The exact corners of the entries' union; queries are clamped to it.
    bounds: Option<(Vec2, Vec2)>,
    candidates: Vec<ProxyId>,
    #[cfg(test)]
    builds: usize,
}

/// The index's tile entries, the outline's runs, and the runs, in their
/// first cell's tile order, their exact boxes' grid by that order and their
/// union's corners, rebuilt when the step's tile cache or the cell size
/// changes.
#[derive(Debug, Default)]
struct TileIndex {
    /// The cache version and cell size bits it was built for.
    staged: Option<(u64, u32)>,
    entries: Vec<IndexEntry>,
    runs: Vec<TileRun>,
    grid: SpatialGrid,
    bounds: Option<(Vec2, Vec2)>,
}

/// `bounds` grown to hold `aabb`, its corners compared exactly.
fn grow(bounds: Option<(Vec2, Vec2)>, aabb: &Aabb) -> (Vec2, Vec2) {
    let (min, max) = (aabb.min(), aabb.max());
    bounds.map_or((min, max), |(lo, hi)| (lo.min(min), hi.max(max)))
}

impl IndexState {
    /// The run `id` names; `None` for a body or a sensor.
    fn tile_run(&self, id: ProxyId) -> Option<&TileRun> {
        let at = (id as usize).checked_sub(self.tile_base)?;
        self.tiles.runs.get(at)
    }

    /// The entry `id` names.
    fn entry(&self, id: ProxyId) -> &IndexEntry {
        let id = id as usize;
        let tiles = self.tiles.entries.len();
        if id < self.tile_base {
            &self.bodies[id]
        } else if id < self.tile_base + tiles {
            &self.tiles.entries[id - self.tile_base]
        } else {
            &self.bodies[id - tiles]
        }
    }

    /// Rebuild from the last step's proxies unless already built; the tile
    /// part only when the step's tiles or the cell size changed.
    fn refresh(&mut self, buffers: &PhysicsBuffers, cell_size: f32) {
        if self.fresh {
            return;
        }
        self.bodies.clear();
        self.tile_base = buffers.index_bodies(&mut self.bodies);
        let key = (buffers.tiles_version(), cell_size.to_bits());
        if self.tiles.staged != Some(key) {
            let tiles = &mut self.tiles;
            tiles.entries.clear();
            tiles.runs.clear();
            buffers.index_tiles(&mut tiles.entries, &mut tiles.runs);
            tiles.grid.set_cell_size(cell_size);
            tiles.bounds = None;
            for (ordinal, entry) in tiles.entries.iter().enumerate() {
                let aabb = entry.aabb();
                tiles.grid.insert(ordinal as ProxyId, &aabb);
                tiles.bounds = Some(grow(tiles.bounds, &aabb));
            }
            tiles.staged = Some(key);
        }
        if (self.grid.cell_size() - cell_size).abs() > f32::EPSILON {
            self.grid.set_cell_size(cell_size);
        }
        self.grid.clear();
        let mut bounds = self.tiles.bounds;
        for (id, entry) in self.bodies.iter().enumerate() {
            let aabb = entry.aabb();
            if id < self.tile_base {
                self.grid.insert(id as ProxyId, &aabb);
            }
            bounds = Some(grow(bounds, &aabb));
        }
        self.bounds = bounds;
        self.fresh = true;
        #[cfg(test)]
        {
            self.builds += 1;
        }
    }

    /// Fill `candidates` with the entries whose boxes may meet `query`: from
    /// the grids and the sensors, or every entry when the box, clamped to
    /// the entries' bounds, spans more cells than there are entries to
    /// test.
    fn gather(&mut self, query: &Aabb) {
        self.gather_parts(query, true);
    }

    /// [`Self::gather`], leaving the tiles out unless `tiles`. Each part,
    /// the entity proxies, the tiles and the sensors, gives a superset of
    /// its entries whose boxes meet the query box: every one of its ids
    /// when the box spans more cells than four times its entries, else its
    /// grid's, and the sensors by their boxes. The exact tests after it
    /// decide every answer, whatever the order (ties go by id).
    fn gather_parts(&mut self, query: &Aabb, tiles: bool) {
        self.candidates.clear();
        let Some((low, high)) = self.bounds else {
            return;
        };
        let pad = Vec2::splat(QUERY_PAD);
        let min = (query.min() - pad).max(low);
        let max = (query.max() + pad).min(high);
        if min.x > max.x || min.y > max.y {
            return;
        }
        let span = (max - min) / self.grid.cell_size() + Vec2::ONE;
        let cells = span.x * span.y;
        let wide = |entries: usize| cells > (4 * entries).max(64) as f32;
        let tile_count = self.tiles.entries.len();
        let base = self.tile_base;
        let clamped = Aabb::from_min_max(min, max);
        let Self {
            grid,
            tiles: tile_index,
            bodies,
            candidates,
            ..
        } = self;
        if wide(base) {
            candidates.extend(0..base as ProxyId);
        } else {
            grid.for_each_in(&clamped, None, |id| candidates.push(id));
        }
        if tiles {
            if wide(tile_count) {
                candidates.extend(base as ProxyId..(base + tile_count) as ProxyId);
            } else {
                tile_index
                    .grid
                    .for_each_in(&clamped, None, |id| candidates.push(id + base as ProxyId));
            }
        }
        // The sensors, against the clamped box, edges included.
        for (at, sensor) in bodies.iter().enumerate().skip(base) {
            let (lo, hi) = (sensor.aabb().min(), sensor.aabb().max());
            if lo.cmple(clamped.max()).all() && clamped.min().cmple(hi).all() {
                candidates.push((at + tile_count) as ProxyId);
            }
        }
    }
}

/// Runs `f` on the index of `world`'s last step, rebuilt first when a step
/// has run since, and the step's buffers; `None` before the first step.
fn with_index<R>(
    world: &World,
    f: impl FnOnce(&mut IndexState, &PhysicsBuffers) -> R,
) -> Option<R> {
    let buffers = world.get_resource::<PhysicsBuffers>()?;
    with_index_of(world, buffers, |state| f(state, buffers))
}

/// `aabb`'s corners, each moved out by [`QUERY_PAD`].
fn padded(aabb: &Aabb) -> (Vec2, Vec2) {
    let pad = Vec2::splat(QUERY_PAD);
    (aabb.min() - pad, aabb.max() + pad)
}

/// A solid cell of the step's grids as an index entry: its box, a slope
/// cell's kind, its layers and identity, every face exposed, which region
/// queries do not read.
fn cell_entry(cell: &GridCell) -> IndexEntry {
    IndexEntry {
        entity: None,
        center: cell.center,
        shape: Shape::Aabb {
            half_extents: cell.half,
        },
        face_mask: FACE_ALL,
        slope: cell.slope,
        sensor: false,
        identity: cell.identity,
        layers: cell.layers,
        mask: cell.mask,
    }
}

/// A cell of `run` as an index entry: its box, layers and identity, with
/// the run's face mask, which each of its cells has.
fn run_cell(cell: &GridCell, run: &TileRun) -> IndexEntry {
    IndexEntry {
        face_mask: run.face_mask,
        ..cell_entry(cell)
    }
}

/// The order ties go by: the bodies by index id, then the tiles by map,
/// layer, row and column, then the sensors.
type Rank = (u8, u64, u64);

/// The rank of the body or sensor `id` names.
fn rank(state: &IndexState, id: ProxyId) -> Rank {
    if (id as usize) < state.tile_base {
        (0, u64::from(id), 0)
    } else {
        (2, u64::from(id), 0)
    }
}

/// The rank of a tile cell: its map's and layer's places in the cache, its
/// row and its column.
fn tile_rank(cell: &GridCell) -> Rank {
    (
        1,
        (u64::from(cell.map_slot) << 32) | u64::from(cell.layer_slot),
        (u64::from(cell.cell.row) << 32) | u64::from(cell.cell.col),
    )
}

/// Whether a cast from `origin` along `dir`, its shape reaching `below`
/// under its centre, may meet `run`: any run but a one-way one, which it
/// meets only moving down from at or above its top face.
fn meets_run(run: &TileRun, origin: Vec2, dir: Vec2, below: f32) -> bool {
    run.face_mask & FACE_ONE_WAY == 0
        || (dir.y > 0.0 && origin.y + below <= run.center.y - run.half.y)
}

/// Whether a hit on a cell of `run` counts: any but one on a one-way cell
/// whose normal does not point up, its top face or a rounded top corner.
fn counts_on(run: &TileRun, hit: &SpatialHit) -> bool {
    run.face_mask & FACE_ONE_WAY == 0 || hit.normal.y < 0.0
}

/// Whether a path from `origin` along `dir` meets the box at `center`
/// grown by `half`, and a little more for rounding, from any side or from
/// inside: a run whose cells are worth testing.
fn path_meets(origin: Vec2, dir: Vec2, center: Vec2, half: Vec2) -> bool {
    let slack = 1.0e-3 + center.abs().max_element() * 1.0e-6;
    enter_box(origin - center, dir, half + Vec2::splat(slack), true).is_some()
}

/// Runs `f` on `world`'s index of `buffers`, rebuilt first when a step has
/// run since; `None` before the first step. A borrow already taken, which
/// nothing can cause since no query calls back, gets a temporary index.
fn with_index_of<R>(
    world: &World,
    buffers: &PhysicsBuffers,
    f: impl FnOnce(&mut IndexState) -> R,
) -> Option<R> {
    let index = world.get_resource::<SpatialIndex>()?;
    let cell_size = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default()
        .broadphase_cell_size;
    if let Ok(mut state) = index.0.try_borrow_mut() {
        state.refresh(buffers, cell_size);
        return Some(f(&mut state));
    }
    let mut state = IndexState::default();
    state.refresh(buffers, cell_size);
    Some(f(&mut state))
}

/// The first body or tile a ray from `origin` along `direction` meets within
/// `max_distance`, at the last step's positions. `origin` must be finite,
/// `direction` finite and nonzero (it is normalized) and `max_distance`
/// finite and not negative, or there is no hit. Shapes are closed: a ray
/// that grazes a face hits it, and a ray ignores a shape that contains its
/// origin, even on its edge.
#[must_use]
pub fn cast_ray(
    world: &World,
    origin: Vec2,
    direction: Vec2,
    max_distance: f32,
    filter: &SpatialFilter,
) -> Option<SpatialHit> {
    cast_ray_where(world, origin, direction, max_distance, filter, |_| true)
}

/// [`cast_ray`], passing the hits `keep` refuses: the first hit along the
/// path that `keep` accepts, with `cast_ray`'s order and ties, in one walk
/// of the index. `keep` sees each hit nearer than the nearest it has
/// accepted so far, in no set order, and may read the World, for a body's
/// kind, mass or tags.
#[must_use]
pub fn cast_ray_where(
    world: &World,
    origin: Vec2,
    direction: Vec2,
    max_distance: f32,
    filter: &SpatialFilter,
    mut keep: impl FnMut(&SpatialHit) -> bool,
) -> Option<SpatialHit> {
    let dir = cast_direction(origin, direction, max_distance)?;
    let end = origin + dir * max_distance;
    with_index(world, |state, buffers| {
        let query = Aabb::from_min_max(origin.min(end), origin.max(end));
        state.gather(&query);
        let (min, max) = padded(&query);
        let hit_of = |entry: &IndexEntry, tile| {
            ray_against(origin, dir, entry).map(|(distance, normal)| SpatialHit {
                entity: entry.entity,
                point: origin + dir * distance,
                normal,
                distance,
                layers: entry.layers,
                tile,
            })
        };
        let mut best = None;
        for &id in &state.candidates {
            let entry = state.entry(id);
            if !filter.admits(entry, world) {
                continue;
            }
            if let Some(run) = state.tile_run(id) {
                // A run: the hits of its cells, each its own box.
                if meets_run(run, origin, dir, 0.0) && path_meets(origin, dir, run.center, run.half)
                {
                    buffers.grid_cells().run_cells_in(run, min, max, |cell| {
                        if let Some(hit) = hit_of(&run_cell(cell, run), Some(cell.cell))
                            && counts_on(run, &hit)
                        {
                            keep_closer_where(
                                &mut best,
                                hit,
                                tile_rank(cell),
                                max_distance,
                                &mut keep,
                            );
                        }
                    });
                }
                continue;
            }
            if let Some(hit) = hit_of(entry, None) {
                keep_closer_where(&mut best, hit, rank(state, id), max_distance, &mut keep);
            }
        }
        best.map(|(hit, _)| hit)
    })
    .flatten()
}

/// The first body or tile `shape`, its centre moving from `origin` along
/// `direction`, meets within `max_distance`, at the last step's positions.
/// The inputs follow [`cast_ray`]'s rules, and the shape's extents must be
/// finite and not negative. Shapes are open: a shape touching a face may
/// slide along it. A shape that starts overlapping a body or tile hits it
/// at distance 0 when `direction` points into it (against the push-out
/// normal of the narrow phase) and otherwise passes it, so a mover can
/// always leave an overlap and never sink deeper. A shape with no extent
/// casts as a ray.
#[must_use]
pub fn cast_shape(
    world: &World,
    shape: Shape,
    origin: Vec2,
    direction: Vec2,
    max_distance: f32,
    filter: &SpatialFilter,
) -> Option<SpatialHit> {
    cast_shape_where(
        world,
        shape,
        origin,
        direction,
        max_distance,
        filter,
        |_| true,
    )
}

/// [`cast_shape`], passing the hits `keep` refuses, as [`cast_ray_where`]
/// passes them.
#[must_use]
pub fn cast_shape_where(
    world: &World,
    shape: Shape,
    origin: Vec2,
    direction: Vec2,
    max_distance: f32,
    filter: &SpatialFilter,
    mut keep: impl FnMut(&SpatialHit) -> bool,
) -> Option<SpatialHit> {
    let half = valid_half_extents(shape)?;
    if half == Vec2::ZERO {
        return cast_ray_where(world, origin, direction, max_distance, filter, keep);
    }
    let dir = cast_direction(origin, direction, max_distance)?;
    let end = origin + dir * max_distance;
    with_index(world, |state, buffers| {
        let query = Aabb::from_min_max(origin.min(end) - half, origin.max(end) + half);
        state.gather(&query);
        let (min, max) = padded(&query);
        let mut best = None;
        for &id in &state.candidates {
            let entry = state.entry(id);
            if !filter.admits(entry, world) {
                continue;
            }
            if let Some(run) = state.tile_run(id) {
                // A run: the hits of its cells, each its own box.
                if meets_run(run, origin, dir, half.y)
                    && path_meets(origin, dir, run.center, run.half + half)
                {
                    buffers.grid_cells().run_cells_in(run, min, max, |cell| {
                        if let Some(hit) = cast_against(shape, origin, dir, &run_cell(cell, run))
                            && counts_on(run, &hit)
                        {
                            let hit = SpatialHit {
                                tile: Some(cell.cell),
                                ..hit
                            };
                            keep_closer_where(
                                &mut best,
                                hit,
                                tile_rank(cell),
                                max_distance,
                                &mut keep,
                            );
                        }
                    });
                }
                continue;
            }
            if let Some(hit) = cast_against(shape, origin, dir, entry) {
                keep_closer_where(&mut best, hit, rank(state, id), max_distance, &mut keep);
            }
        }
        best.map(|(hit, _)| hit)
    })
    .flatten()
}

/// Every body and tile whose shape overlaps `shape` at `center` with
/// penetration above 0, at the last step's positions, in index order: the
/// bodies, then the tiles by map, layer, row and column, each a cell of the
/// step's grids listed as `None`, then the sensors. `out` is cleared first
/// and stays empty for a non-finite `center` or extents that are not finite
/// and positive or 0. Tile faces play no part here: a shape inside a tile
/// overlaps it.
pub fn overlap(
    world: &World,
    shape: Shape,
    center: Vec2,
    filter: &SpatialFilter,
    out: &mut Vec<Option<Entity>>,
) {
    out.clear();
    let Some(half) = valid_half_extents(shape) else {
        return;
    };
    if !center.is_finite() {
        return;
    }
    with_index(world, |state, buffers| {
        let query = Aabb::new(center, half);
        state.gather_parts(&query, false);
        state.candidates.sort_unstable();
        let split = state
            .candidates
            .partition_point(|&id| (id as usize) < state.tile_base);
        let mut list = |entry: &IndexEntry| {
            if filter.admits(entry, world) && entry_overlaps(shape, center, entry) {
                out.push(entry.entity);
            }
        };
        for &id in &state.candidates[..split] {
            list(state.entry(id));
        }
        if filter.tiles {
            let (min, max) = padded(&query);
            buffers
                .grid_cells()
                .cells_in(min, max, |cell| list(&cell_entry(cell)));
        }
        for &id in &state.candidates[split..] {
            list(state.entry(id));
        }
    });
}

/// The body or tile whose surface is closest to `point` within
/// `max_distance`, at the last step's positions, a tile being a cell of the
/// step's grids. The hit's `point` is the closest surface point; a `point`
/// inside a shape, or on its edge, gives distance 0, that point and a zero
/// normal. Ties go to the body, then the tile in tile order, then the
/// sensor. `point` must be finite and `max_distance` finite and not
/// negative, or there is no hit.
#[must_use]
pub fn nearest(
    world: &World,
    point: Vec2,
    max_distance: f32,
    filter: &SpatialFilter,
) -> Option<SpatialHit> {
    if !point.is_finite() || !max_distance.is_finite() || max_distance < 0.0 {
        return None;
    }
    with_index(world, |state, buffers| {
        let query = Aabb::new(point, Vec2::splat(max_distance));
        state.gather_parts(&query, false);
        let mut best = None;
        for &id in &state.candidates {
            let entry = state.entry(id);
            if filter.admits(entry, world) {
                keep_closer(
                    &mut best,
                    surface_hit(point, entry),
                    rank(state, id),
                    max_distance,
                );
            }
        }
        if filter.tiles {
            let (min, max) = padded(&query);
            buffers.grid_cells().cells_in(min, max, |cell| {
                let entry = cell_entry(cell);
                if filter.admits(&entry, world) {
                    let hit = SpatialHit {
                        tile: Some(cell.cell),
                        ..surface_hit(point, &entry)
                    };
                    keep_closer(&mut best, hit, tile_rank(cell), max_distance);
                }
            });
        }
        best.map(|(hit, _)| hit)
    })
    .flatten()
}

/// Keep `hit` when it lies within `max_distance` and beats `best`: a lower
/// distance, or the same at a lower rank (its index order).
fn keep_closer(
    best: &mut Option<(SpatialHit, Rank)>,
    hit: SpatialHit,
    rank: Rank,
    max_distance: f32,
) {
    keep_closer_where(best, hit, rank, max_distance, &mut |_| true);
}

/// [`keep_closer`] for a hit `keep` accepts; `keep` sees only a hit that
/// would be kept.
fn keep_closer_where(
    best: &mut Option<(SpatialHit, Rank)>,
    hit: SpatialHit,
    rank: Rank,
    max_distance: f32,
    keep: &mut impl FnMut(&SpatialHit) -> bool,
) {
    if hit.distance > max_distance {
        return;
    }
    let closer = best.as_ref().is_none_or(|(kept, kept_rank)| {
        hit.distance < kept.distance || (hit.distance == kept.distance && rank < *kept_rank)
    });
    if closer && keep(&hit) {
        *best = Some((hit, rank));
    }
}

/// `direction` normalized, when `origin`, `direction` and `max_distance`
/// make a usable cast.
fn cast_direction(origin: Vec2, direction: Vec2, max_distance: f32) -> Option<Vec2> {
    if !origin.is_finite() || !max_distance.is_finite() || max_distance < 0.0 {
        return None;
    }
    direction.try_normalize()
}

fn half_extents(shape: Shape) -> Vec2 {
    match shape {
        Shape::Aabb { half_extents } => half_extents,
        Shape::Circle { radius } => Vec2::splat(radius),
    }
}

/// The shape's bounding half extents, when they are finite and not negative.
fn valid_half_extents(shape: Shape) -> Option<Vec2> {
    let half = half_extents(shape);
    (half.is_finite() && half.x >= 0.0 && half.y >= 0.0).then_some(half)
}

/// The ray against one entry: its entry distance and the surface normal
/// there.
fn ray_against(origin: Vec2, dir: Vec2, entry: &IndexEntry) -> Option<(f32, Vec2)> {
    if let Some(poly) = entry.polygon() {
        return slope::ray(origin, dir, &poly, entry.face_mask);
    }
    match entry.shape {
        Shape::Aabb { half_extents } => {
            let local = origin - entry.center;
            let (t, entered) = enter_box(local, dir, half_extents, true)?;
            // A ray starting on or in the closed box is inside it.
            if t <= 0.0 {
                return None;
            }
            Some((t, face_normal(dir, entered, entry.face_mask)?))
        }
        Shape::Circle { radius } => {
            let t = enter_circle(origin, dir, entry.center, radius, true)?;
            let normal = (origin + dir * t - entry.center).normalize_or_zero();
            Some((t, normal))
        }
    }
}

/// `shape` cast from `origin` along `dir` against one entry, at any
/// distance.
fn cast_against(shape: Shape, origin: Vec2, dir: Vec2, entry: &IndexEntry) -> Option<SpatialHit> {
    let center = entry.center;
    if let Some(contact) = start_overlap(shape, origin, entry) {
        // Moving into the shape stops at once; any other direction passes.
        if dir.dot(contact.normal) >= 0.0 {
            return None;
        }
        let reach = match shape {
            Shape::Aabb { half_extents } => contact.normal.abs().dot(half_extents),
            Shape::Circle { radius } => radius,
        };
        return Some(SpatialHit {
            entity: entry.entity,
            point: origin - contact.normal * reach,
            normal: contact.normal,
            distance: 0.0,
            layers: entry.layers,
            tile: None,
        });
    }
    if let Some(poly) = entry.polygon() {
        let (distance, normal) = slope::cast(shape, origin, dir, &poly, entry.face_mask)?;
        let at = origin + dir * distance;
        let point = match shape {
            Shape::Aabb { half_extents } => slope_contact_point(at, half_extents, &poly, normal),
            Shape::Circle { radius } => at - normal * radius,
        };
        return Some(SpatialHit {
            entity: entry.entity,
            point,
            normal,
            distance,
            layers: entry.layers,
            tile: None,
        });
    }
    let (distance, normal, point) = match (shape, entry.shape) {
        (
            Shape::Aabb { half_extents },
            Shape::Aabb {
                half_extents: other,
            },
        ) => {
            let (t, normal) = enter_rounded_box(
                origin - center,
                dir,
                other + half_extents,
                0.0,
                entry.face_mask,
            )?;
            let at = origin + dir * t;
            let point = box_contact_point(at, half_extents, center, other, normal);
            (t, normal, point)
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            let (t, normal) =
                enter_rounded_box(origin - center, dir, half_extents, radius, entry.face_mask)?;
            (t, normal, origin + dir * t - normal * radius)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            let (t, normal) =
                enter_rounded_box(origin - center, dir, half_extents, radius, FACE_ALL)?;
            (t, normal, center + normal * radius)
        }
        (Shape::Circle { radius }, Shape::Circle { radius: other }) => {
            let t = enter_circle(origin, dir, center, other + radius, false)?;
            let normal = (origin + dir * t - center).normalize_or_zero();
            (t, normal, center + normal * other)
        }
    };
    Some(SpatialHit {
        entity: entry.entity,
        point,
        normal,
        distance,
        layers: entry.layers,
        tile: None,
    })
}

/// The narrow phase's contact for `shape` at `origin` against the entry
/// when they overlap with penetration above 0; its normal pushes the shape
/// out. Tiles use their face masks, as the step's contacts do.
fn start_overlap(shape: Shape, origin: Vec2, entry: &IndexEntry) -> Option<Contact> {
    if !Aabb::new(origin, half_extents(shape)).overlaps(&entry.aabb()) {
        return None;
    }
    let (center, mask) = (entry.center, entry.face_mask);
    if let Some(poly) = entry.polygon() {
        return match shape {
            Shape::Aabb { half_extents } => {
                slope::box_contact(origin, half_extents, &poly, mask, 0.0)
            }
            Shape::Circle { radius } => slope::circle_contact(origin, radius, &poly, mask, 0.0),
        };
    }
    match (shape, entry.shape) {
        (
            Shape::Aabb { half_extents },
            Shape::Aabb {
                half_extents: other,
            },
        ) => aabb_vs_aabb_speculative(
            &Aabb::new(origin, half_extents),
            &Aabb::new(center, other),
            mask,
            0.0,
        ),
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => aabb_vs_circle_speculative(
            &Aabb::new(origin, half_extents),
            center,
            radius,
            FACE_ALL,
            0.0,
        ),
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            // The helper returns the box's escape normal; the circle leaves
            // the other way.
            aabb_vs_circle_speculative(&Aabb::new(center, half_extents), origin, radius, mask, 0.0)
                .map(|contact| Contact {
                    normal: -contact.normal,
                    penetration: contact.penetration,
                })
        }
        (Shape::Circle { radius }, Shape::Circle { radius: other }) => {
            circle_vs_circle_speculative(origin, radius, center, other, 0.0)
        }
    }
}

/// The first `t` in `[0, 1]` at which `shape`, moving from `start` by
/// `motion`, overlaps `other`, moving from `other_start` by `other_motion`,
/// with penetration above 0: 0 when they start overlapping, `None` when
/// they never do, a graze included. Their relative motion cast against
/// their Minkowski sum, as the query's shape casts meet a body.
pub(super) fn first_overlap(
    shape: Shape,
    start: Vec2,
    motion: Vec2,
    other: Shape,
    other_start: Vec2,
    other_motion: Vec2,
) -> Option<f32> {
    if shapes_overlap(shape, start, other, other_start) {
        return Some(0.0);
    }
    let (local, dir) = (start - other_start, motion - other_motion);
    // A disc's entry takes a unit direction; its `t` is then in px.
    let length = dir.length();
    let unit = dir.try_normalize();
    let t = match (shape, other) {
        (
            Shape::Aabb { half_extents },
            Shape::Aabb {
                half_extents: other,
            },
        ) => enter_box(local, dir, half_extents + other, false)?.0,
        (Shape::Circle { radius }, Shape::Circle { radius: other }) => {
            enter_circle(local, unit?, Vec2::ZERO, radius + other, false)? / length
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents })
        | (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            enter_rounded_box(local, unit?, half_extents, radius, FACE_ALL)?.0 / length
        }
    };
    (t <= 1.0).then_some(t.max(0.0))
}

/// [`first_overlap`] of `shape` moving from `start` by `motion` against a
/// still slope tile's polygon, every side exposed.
pub(super) fn first_overlap_slope(
    shape: Shape,
    start: Vec2,
    motion: Vec2,
    poly: &SlopePolygon,
) -> Option<f32> {
    if slope::overlaps(shape, start, poly) {
        return Some(0.0);
    }
    let (t, _) = slope::cast(shape, start, motion.try_normalize()?, poly, FACE_ALL)?;
    let t = t / motion.length();
    (t <= 1.0).then_some(t.max(0.0))
}

/// `shape` at `center` overlaps the entry with penetration above 0: its
/// shape, or a slope tile's polygon.
fn entry_overlaps(shape: Shape, center: Vec2, entry: &IndexEntry) -> bool {
    match entry.polygon() {
        Some(poly) => slope::overlaps(shape, center, &poly),
        None => shapes_overlap(shape, center, entry.shape, entry.center),
    }
}

/// `shape` at `center` and `other` at `other_center` overlap with
/// penetration above 0; touching shapes do not.
pub(super) fn shapes_overlap(shape: Shape, center: Vec2, other: Shape, other_center: Vec2) -> bool {
    match (shape, other) {
        (
            Shape::Aabb { half_extents },
            Shape::Aabb {
                half_extents: other,
            },
        ) => Aabb::new(center, half_extents).overlaps(&Aabb::new(other_center, other)),
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            circle_overlaps_box(center, radius, other_center, half_extents)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            circle_overlaps_box(other_center, radius, center, half_extents)
        }
        (Shape::Circle { radius }, Shape::Circle { radius: other }) => {
            let reach = radius + other;
            (other_center - center).length_squared() < reach * reach
        }
    }
}

/// A circle overlaps a box when its centre is strictly inside it or its
/// surface reaches past the box's edge.
fn circle_overlaps_box(circle: Vec2, radius: f32, center: Vec2, half: Vec2) -> bool {
    let outside = (circle - center).abs() - half;
    (outside.x < 0.0 && outside.y < 0.0)
        || outside.max(Vec2::ZERO).length_squared() < radius * radius
}

/// The closest surface point of the entry to `point`, as a hit.
fn surface_hit(point: Vec2, entry: &IndexEntry) -> SpatialHit {
    let (surface, normal, distance) = if let Some(poly) = entry.polygon() {
        let closest = poly.closest(point);
        let offset = point - closest;
        let distance = offset.length();
        if poly.contains(point) || distance == 0.0 {
            (point, Vec2::ZERO, 0.0)
        } else {
            (closest, offset / distance, distance)
        }
    } else {
        match entry.shape {
            Shape::Aabb { half_extents } => {
                let closest = point
                    .max(entry.center - half_extents)
                    .min(entry.center + half_extents);
                let offset = point - closest;
                let distance = offset.length();
                if distance > 0.0 {
                    (closest, offset / distance, distance)
                } else {
                    (point, Vec2::ZERO, 0.0)
                }
            }
            Shape::Circle { radius } => {
                let offset = point - entry.center;
                let length = offset.length();
                if length > radius {
                    let normal = offset / length;
                    (entry.center + normal * radius, normal, length - radius)
                } else {
                    (point, Vec2::ZERO, 0.0)
                }
            }
        }
    };
    SpatialHit {
        entity: entry.entity,
        point: surface,
        normal,
        distance,
        layers: entry.layers,
        tile: None,
    }
}

/// Which faces of a box a path enters through first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entered {
    X,
    Y,
    /// Both at once: the path enters at a corner.
    Corner,
}

/// Entry and exit `t` of `o + t·d` through the slab `[-half, half]`, closed
/// for a ray and open for a shape cast. A component under `f32::EPSILON`
/// travels parallel to the slab.
fn slab(o: f32, d: f32, half: f32, closed: bool) -> Option<(f32, f32)> {
    if d.abs() < f32::EPSILON {
        let inside = if closed {
            o >= -half && o <= half
        } else {
            o > -half && o < half
        };
        return inside.then_some((f32::NEG_INFINITY, f32::INFINITY));
    }
    let inv = 1.0 / d;
    let t1 = (-half - o) * inv;
    let t2 = (half - o) * inv;
    Some((t1.min(t2), t1.max(t2)))
}

/// The `t` at which `local + t·dir` enters the box `±half` around the
/// origin, and through which faces; `None` when the path misses it or the
/// box lies behind. The `t` is negative when the path starts inside.
/// A closed box admits a path that only touches it, an open one does not.
fn enter_box(local: Vec2, dir: Vec2, half: Vec2, closed: bool) -> Option<(f32, Entered)> {
    let (tx0, tx1) = slab(local.x, dir.x, half.x, closed)?;
    let (ty0, ty1) = slab(local.y, dir.y, half.y, closed)?;
    let t_near = tx0.max(ty0);
    let t_far = tx1.min(ty1);
    let crosses = if closed {
        t_near <= t_far
    } else {
        t_near < t_far
    };
    if !crosses || t_far < 0.0 || (!closed && t_far == 0.0) {
        return None;
    }
    let entered = if tx0 > ty0 {
        Entered::X
    } else if ty0 > tx0 {
        Entered::Y
    } else {
        Entered::Corner
    };
    Some((t_near, entered))
}

/// The outward normal of the face a path along `dir` enters, by the box's
/// face mask: an internal face is never entered, and a corner beside one is
/// entered through its exposed face.
fn face_normal(dir: Vec2, entered: Entered, mask: u8) -> Option<Vec2> {
    let (x_face, x_normal) = if dir.x > 0.0 {
        (FACE_LEFT, Vec2::NEG_X)
    } else {
        (FACE_RIGHT, Vec2::X)
    };
    let (y_face, y_normal) = if dir.y > 0.0 {
        (FACE_TOP, Vec2::NEG_Y)
    } else {
        (FACE_BOTTOM, Vec2::Y)
    };
    let x_open = mask & x_face != 0;
    let y_open = mask & y_face != 0;
    match entered {
        Entered::X => x_open.then_some(x_normal),
        Entered::Y => y_open.then_some(y_normal),
        Entered::Corner => match (x_open, y_open) {
            (true, true) => Some((x_normal + y_normal) * std::f32::consts::FRAC_1_SQRT_2),
            (true, false) => Some(x_normal),
            (false, true) => Some(y_normal),
            (false, false) => None,
        },
    }
}

/// The first `t >= 0` at which a shape cast's centre, `local + t·dir` from
/// the box's centre, enters the box `±core` rounded by `radius`, and the
/// outward normal there; `None` when it misses or starts inside. A corner
/// is rounded only when `mask` exposes both of its faces; beside an internal
/// face it is square, and an internal face is never entered (`D-064`).
fn enter_rounded_box(
    local: Vec2,
    dir: Vec2,
    core: Vec2,
    radius: f32,
    mask: u8,
) -> Option<(f32, Vec2)> {
    let (t, entered) = enter_box(local, dir, core + Vec2::splat(radius), false)?;
    // Where the path meets the outer box, or starts inside it.
    let at = local + dir * t.max(0.0);
    let past = at.abs() - core;
    if radius > 0.0 && past.x > 0.0 && past.y > 0.0 {
        // A corner square: a rounded corner is its quarter disc, which the
        // path must reach before anything past it.
        let x_face = if at.x > 0.0 { FACE_RIGHT } else { FACE_LEFT };
        let y_face = if at.y > 0.0 { FACE_BOTTOM } else { FACE_TOP };
        if mask & x_face != 0 && mask & y_face != 0 {
            let corner = Vec2::new(core.x.copysign(at.x), core.y.copysign(at.y));
            let t = enter_circle(local, dir, corner, radius, false)?;
            let normal = (local + dir * t - corner).normalize_or_zero();
            return Some((t, normal));
        }
    }
    // Starting inside, outside every corner disc: the narrow phase found
    // no overlap, so only internal faces lie around the start.
    if t < 0.0 {
        return None;
    }
    Some((t, face_normal(dir, entered, mask)?))
}

/// The `t` at which `origin + t·dir`, `dir` a unit vector, enters the
/// circle, closed for a ray and open for a shape cast; `None` when it
/// misses, moves away, or starts inside (on the edge too, when closed).
pub(super) fn enter_circle(
    origin: Vec2,
    dir: Vec2,
    center: Vec2,
    radius: f32,
    closed: bool,
) -> Option<f32> {
    let m = origin - center;
    let c = m.length_squared() - radius * radius;
    let inside = if closed { c <= 0.0 } else { c < 0.0 };
    if inside {
        return None;
    }
    let b = m.dot(dir);
    if b >= 0.0 {
        return None;
    }
    let disc = b * b - c;
    let meets = if closed { disc >= 0.0 } else { disc > 0.0 };
    meets.then(|| (-b - disc.sqrt()).max(0.0))
}

/// Where a box at `at` with `half` first touches the box at `center` with
/// `other` across the face with normal `normal`: the middle of the faces'
/// shared span, or the corner a diagonal normal names.
fn box_contact_point(at: Vec2, half: Vec2, center: Vec2, other: Vec2, normal: Vec2) -> Vec2 {
    let shared = |a: f32, a_half: f32, b: f32, b_half: f32| {
        let low = (a - a_half).max(b - b_half);
        let high = (a + a_half).min(b + b_half);
        f32::midpoint(low, high)
    };
    if normal.y == 0.0 {
        Vec2::new(
            center.x + normal.x * other.x,
            shared(at.y, half.y, center.y, other.y),
        )
    } else if normal.x == 0.0 {
        Vec2::new(
            shared(at.x, half.x, center.x, other.x),
            center.y + normal.y * other.y,
        )
    } else {
        center + Vec2::new(other.x.copysign(normal.x), other.y.copysign(normal.y))
    }
}

/// Where a box at `at` with `half` first touches a slope polygon across the
/// side of outward `normal`: the polygon's vertex farthest along it, else
/// the box's corner farthest against it on the sloped face, else the middle
/// of the span a leg and the box's face share.
fn slope_contact_point(at: Vec2, half: Vec2, poly: &SlopePolygon, normal: Vec2) -> Vec2 {
    let points = &poly.points[..poly.len];
    let far = points
        .iter()
        .map(|p| p.dot(normal))
        .fold(f32::NEG_INFINITY, f32::max);
    let mut support = points.iter().filter(|p| far - p.dot(normal) < 1.0e-3);
    let (Some(&first), second) = (support.next(), support.next()) else {
        return at;
    };
    let Some(&second) = second else {
        return first;
    };
    if normal.x != 0.0 && normal.y != 0.0 {
        return at - Vec2::new(half.x.copysign(normal.x), half.y.copysign(normal.y));
    }
    let shared = |a: f32, a_half: f32, low: f32, high: f32| {
        f32::midpoint((a - a_half).max(low), (a + a_half).min(high))
    };
    if normal.y == 0.0 {
        let (low, high) = (first.y.min(second.y), first.y.max(second.y));
        Vec2::new(first.x, shared(at.y, half.y, low, high))
    } else {
        let (low, high) = (first.x.min(second.x), first.x.max(second.x));
        Vec2::new(shared(at.x, half.x, low, high), first.y)
    }
}

#[cfg(test)]
#[path = "../tests/physics/query.rs"]
mod tests;
