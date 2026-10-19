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
//! ([`SpatialFilter::with_mask`]), every layer by default; tiles are in
//! layer 0.
//!
//! Tiles keep `D-064`'s seams: a cast never hits a face the tile's neighbour
//! covers, and a corner beside such a face is square, so a cast along a tile
//! floor slides past its seams. Entity colliders keep every face.
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
    Aabb, Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP,
    aabb_vs_aabb_speculative, aabb_vs_circle_speculative, circle_vs_circle_speculative,
};
use super::components::{RigidBody, Shape};
use super::step::PhysicsBuffers;
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
}

impl Default for SpatialFilter {
    fn default() -> Self {
        Self {
            exclude: None,
            bodies: true,
            sensors: false,
            tiles: true,
            mask: RigidBody::ALL_LAYERS,
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
    /// `mask` holds, one bit per layer ([`RigidBody::layers`]); tiles are in
    /// layer 0. It replaces an earlier mask; the default holds every layer.
    /// Sensors still need [`with_sensors`](Self::with_sensors).
    #[must_use]
    pub fn with_mask(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }

    /// The entry passes the filter and its entity is still alive.
    fn admits(&self, entry: &IndexEntry, world: &World) -> bool {
        if entry.layers & self.mask == 0 {
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
    pub(super) sensor: bool,
    /// The body's key or the tile's identity (the low half of a sensor
    /// pair's key); a sensor's key (the high half).
    pub(super) identity: u64,
    /// The layers it is in; a tile's is layer 0.
    pub(super) layers: u32,
    /// The layers it meets; a sensor reports what this holds.
    pub(super) mask: u32,
}

impl IndexEntry {
    fn aabb(&self) -> Aabb {
        Aabb::new(self.center, half_extents(self.shape))
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
    /// `visit` with each sensor, in gather order, and each body or tile in a
    /// layer the sensor's mask holds that it overlaps with penetration above
    /// 0, in index order. Sensors do not detect sensors. The step's queries
    /// reuse the index.
    pub(super) fn sensor_overlaps(
        world: &World,
        buffers: &PhysicsBuffers,
        mut visit: impl FnMut(&IndexEntry, &IndexEntry),
    ) {
        with_index_of(world, buffers, |state| {
            let first = state.entries.partition_point(|entry| !entry.sensor);
            for index in first..state.entries.len() {
                let sensor = state.entries[index];
                state.gather(&sensor.aabb());
                state.candidates.sort_unstable();
                for &id in &state.candidates {
                    let other = &state.entries[id as usize];
                    if !other.sensor
                        && sensor.mask & other.layers != 0
                        && shapes_overlap(sensor.shape, sensor.center, other.shape, other.center)
                    {
                        visit(&sensor, other);
                    }
                }
            }
        });
    }
}

#[derive(Debug, Default)]
struct IndexState {
    /// Built from the last step's proxies; every step clears it.
    fresh: bool,
    entries: Vec<IndexEntry>,
    /// The entries' exact boxes, by entry index.
    grid: SpatialGrid,
    /// Union of the entries' boxes; queries are clamped to it.
    bounds: Option<Aabb>,
    candidates: Vec<ProxyId>,
    #[cfg(test)]
    builds: usize,
}

impl IndexState {
    /// Rebuild from the last step's proxies unless already built.
    fn refresh(&mut self, buffers: &PhysicsBuffers, cell_size: f32) {
        if self.fresh {
            return;
        }
        self.entries.clear();
        buffers.index_entries(&mut self.entries);
        if (self.grid.cell_size() - cell_size).abs() > f32::EPSILON {
            self.grid.set_cell_size(cell_size);
        }
        self.grid.clear();
        self.bounds = None;
        for (id, entry) in self.entries.iter().enumerate() {
            let aabb = entry.aabb();
            self.grid.insert(id as ProxyId, &aabb);
            self.bounds = Some(self.bounds.map_or(aabb, |bounds| bounds.union(&aabb)));
        }
        self.fresh = true;
        #[cfg(test)]
        {
            self.builds += 1;
        }
    }

    /// Fill `candidates` with the entries whose boxes may meet `query`: from
    /// the grid, or every entry when the box, clamped to the entries'
    /// bounds, spans more cells than there are entries to test.
    fn gather(&mut self, query: &Aabb) {
        self.candidates.clear();
        let Some(bounds) = self.bounds else {
            return;
        };
        let pad = Vec2::splat(QUERY_PAD);
        let min = (query.min() - pad).max(bounds.min());
        let max = (query.max() + pad).min(bounds.max());
        if min.x > max.x || min.y > max.y {
            return;
        }
        let span = (max - min) / self.grid.cell_size() + Vec2::ONE;
        if span.x * span.y > (4 * self.entries.len()).max(64) as f32 {
            self.candidates.extend(0..self.entries.len() as ProxyId);
        } else {
            let clamped = Aabb::from_min_max(min, max);
            self.grid.query(&clamped, None, &mut self.candidates);
        }
    }
}

/// Runs `f` on the index of `world`'s last step, rebuilt first when a step
/// has run since; `None` before the first step.
fn with_index<R>(world: &World, f: impl FnOnce(&mut IndexState) -> R) -> Option<R> {
    with_index_of(world, world.get_resource::<PhysicsBuffers>()?, f)
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
    let dir = cast_direction(origin, direction, max_distance)?;
    let end = origin + dir * max_distance;
    with_index(world, |state| {
        state.gather(&Aabb::from_min_max(origin.min(end), origin.max(end)));
        let mut best = None;
        for &id in &state.candidates {
            let entry = &state.entries[id as usize];
            if !filter.admits(entry, world) {
                continue;
            }
            let Some((distance, normal)) = ray_against(origin, dir, entry) else {
                continue;
            };
            let hit = SpatialHit {
                entity: entry.entity,
                point: origin + dir * distance,
                normal,
                distance,
            };
            keep_closer(&mut best, hit, id, max_distance);
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
    let half = valid_half_extents(shape)?;
    if half == Vec2::ZERO {
        return cast_ray(world, origin, direction, max_distance, filter);
    }
    let dir = cast_direction(origin, direction, max_distance)?;
    let end = origin + dir * max_distance;
    with_index(world, |state| {
        state.gather(&Aabb::from_min_max(
            origin.min(end) - half,
            origin.max(end) + half,
        ));
        let mut best = None;
        for &id in &state.candidates {
            let entry = &state.entries[id as usize];
            if !filter.admits(entry, world) {
                continue;
            }
            if let Some(hit) = cast_against(shape, origin, dir, entry) {
                keep_closer(&mut best, hit, id, max_distance);
            }
        }
        best.map(|(hit, _)| hit)
    })
    .flatten()
}

/// Every body and tile whose shape overlaps `shape` at `center` with
/// penetration above 0, at the last step's positions, in index order; a
/// tile is listed as `None`. `out` is cleared first and stays empty for a
/// non-finite `center` or extents that are not finite and positive or 0.
/// Tile faces play no part here: a shape inside a tile overlaps it.
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
    with_index(world, |state| {
        state.gather(&Aabb::new(center, half));
        state.candidates.sort_unstable();
        for &id in &state.candidates {
            let entry = &state.entries[id as usize];
            if filter.admits(entry, world)
                && shapes_overlap(shape, center, entry.shape, entry.center)
            {
                out.push(entry.entity);
            }
        }
    });
}

/// The body or tile whose surface is closest to `point` within
/// `max_distance`, at the last step's positions. The hit's `point` is the
/// closest surface point; a `point` inside a shape, or on its edge, gives
/// distance 0, that point and a zero normal. `point` must be finite and
/// `max_distance` finite and not negative, or there is no hit.
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
    with_index(world, |state| {
        state.gather(&Aabb::new(point, Vec2::splat(max_distance)));
        let mut best = None;
        for &id in &state.candidates {
            let entry = &state.entries[id as usize];
            if filter.admits(entry, world) {
                keep_closer(&mut best, surface_hit(point, entry), id, max_distance);
            }
        }
        best.map(|(hit, _)| hit)
    })
    .flatten()
}

/// Keep `hit` when it lies within `max_distance` and beats `best`: a lower
/// distance, or the same at a lower index.
fn keep_closer(
    best: &mut Option<(SpatialHit, ProxyId)>,
    hit: SpatialHit,
    id: ProxyId,
    max_distance: f32,
) {
    if hit.distance > max_distance {
        return;
    }
    let closer = best.as_ref().is_none_or(|(kept, kept_id)| {
        hit.distance < kept.distance || (hit.distance == kept.distance && id < *kept_id)
    });
    if closer {
        *best = Some((hit, id));
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
    let (surface, normal, distance) = match entry.shape {
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
    };
    SpatialHit {
        entity: entry.entity,
        point: surface,
        normal,
        distance,
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

/// The `t` at which `origin + t·dir` enters the circle, closed for a ray
/// and open for a shape cast; `None` when it misses, moves away, or starts
/// inside (on the edge too, when closed).
fn enter_circle(origin: Vec2, dir: Vec2, center: Vec2, radius: f32, closed: bool) -> Option<f32> {
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

#[cfg(test)]
#[path = "../tests/physics/query.rs"]
mod tests;
