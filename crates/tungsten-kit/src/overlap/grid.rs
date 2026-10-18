//! The kit's hit grid: every live hurtbox's swept bounds in 64-px cells,
//! rebuilt first in the overlap phase each step, the swept test the hit
//! items run against it, and the strike they share.

use glam::{DVec2, Vec2};
use tungsten_core::physics::{Position, PrevPosition, Shape};
use tungsten_core::{Entity, EventQueue, Transform, World};

use crate::damage::health::Health;
use crate::damage::pipeline::DamageRequest;
use crate::foundation::source::Source;
use crate::foundation::tags::Tags;
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::Despawning;
use crate::overlap::hit::{Hurtbox, Struck};
use crate::transform::hierarchy::Parent;

/// The grid's cell size, in world pixels (Q12).
const CELL: f32 = 64.0;
/// The most cells an entry fills; one past it goes on the oversized list.
const MAX_ENTRY_CELLS: i64 = 64;
/// Slack added to a query's box, so a shape whose bounds touch an entry's
/// exactly is still a candidate.
const QUERY_PAD: f32 = 0.01;

/// A shape's motion over a step: its centre at the step's start and at its
/// end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sweep {
    pub(crate) shape: Shape,
    pub(crate) start: Vec2,
    pub(crate) end: Vec2,
}

impl Sweep {
    /// The shape's centre at `t` of the step.
    pub(crate) fn at(&self, t: f32) -> Vec2 {
        self.start + (self.end - self.start) * t
    }

    /// The box the shape covers over the step: its box at both ends.
    pub(crate) fn bounds(&self) -> (Vec2, Vec2) {
        let half = half_extents(self.shape);
        (
            self.start.min(self.end) - half,
            self.start.max(self.end) + half,
        )
    }
}

/// A shape's half extents.
pub(crate) fn half_extents(shape: Shape) -> Vec2 {
    match shape {
        Shape::Circle { radius } => Vec2::splat(radius),
        Shape::Aabb { half_extents } => half_extents,
    }
}

/// Where `entity` stands over the step: a root with a `Position` from its
/// `PrevPosition` to its `Position`, the step's motion (or at its
/// `Position` without a history); otherwise, a child included, at its
/// `Transform.position`. `None` for an entity with neither.
pub(crate) fn sweep_points(world: &World, entity: Entity) -> Option<(Vec2, Vec2)> {
    if !world.has::<Parent>(entity)
        && let Some(position) = world.get::<Position>(entity)
    {
        let start = world
            .get::<PrevPosition>(entity)
            .map_or(position.0, |prev| prev.0);
        return Some((start, position.0));
    }
    world
        .get::<Transform>(entity)
        .map(|transform| (transform.position, transform.position))
}

/// `shape` at `offset` from `entity`'s points over the step.
pub(crate) fn sweep_of(world: &World, entity: Entity, shape: Shape, offset: Vec2) -> Option<Sweep> {
    let (start, end) = sweep_points(world, entity)?;
    let sweep = Sweep {
        shape,
        start: start + offset,
        end: end + offset,
    };
    (sweep.start.is_finite() && sweep.end.is_finite()).then_some(sweep)
}

/// Whether `entity` can be met: alive, not being despawned by the kit and
/// not dead by its [`Health`].
pub(crate) fn takes_part(world: &World, entity: Entity) -> bool {
    world.is_alive(entity)
        && !world.has::<Despawning>(entity)
        && !world.get::<Health>(entity).is_some_and(Health::is_dead)
}

/// The entity a spawn came from, derived at the flush as the spawn's own
/// source: a bolt's shooter, which its hits spare, as they spare the spawn
/// chain's origin, its [`Source`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Emitter(pub(crate) Entity);

/// The spawn's source itself; the `Option` is the derivation's signature.
#[allow(clippy::unnecessary_wraps)]
fn derive_emitter(_: &World, source: Entity) -> Option<Emitter> {
    Some(Emitter(source))
}

/// Registers the derivation of [`Emitter`]; every plugin whose items hit
/// calls it in `build`.
pub(crate) fn ensure_emitter(world: &mut World) {
    world.derive_from_source::<Emitter>(derive_emitter);
}

/// Whether `hitter` spares `target`: its own entity, the entity it was
/// spawned from, or its spawn chain's origin.
pub(crate) fn spares(world: &World, hitter: Entity, target: Entity) -> bool {
    target == hitter
        || world
            .get::<Emitter>(hitter)
            .is_some_and(|emitter| emitter.0 == target)
        || world
            .get::<Source>(hitter)
            .is_some_and(|source| source.entity() == target)
}

/// One live hurtbox in the grid.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridEntry {
    pub(crate) entity: Entity,
    pub(crate) sweep: Sweep,
}

/// The step's hurtboxes, a World resource that [`HIT_GRID`] rebuilds:
/// each entry's swept bounds in 64-px cells as (cell, entry) pairs, sorted;
/// the entries whose bounds fill more than 64 cells on an oversized list
/// that every query tests; and the bounds of them all.
#[derive(Debug, Default)]
pub(crate) struct HitGrid {
    /// By entity index.
    entries: Vec<GridEntry>,
    cells: Vec<(u64, u32)>,
    oversized: Vec<u32>,
    bounds: Option<(Vec2, Vec2)>,
}

/// The cell holding `point`, its coordinates clamped to `i32`.
fn cell_of(point: Vec2) -> (i32, i32) {
    let cell = |v: f32| (v / CELL).floor().clamp(i32::MIN as f32, i32::MAX as f32) as i32;
    (cell(point.x), cell(point.y))
}

/// A cell's sort key.
fn key(x: i32, y: i32) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

/// The cells from `min` to `max`, inclusive, and how many they are.
fn cell_span(min: Vec2, max: Vec2) -> ((i32, i32), (i32, i32), i64) {
    let (lo, hi) = (cell_of(min), cell_of(max));
    let count = (i64::from(hi.0) - i64::from(lo.0) + 1) * (i64::from(hi.1) - i64::from(lo.1) + 1);
    (lo, hi, count)
}

impl HitGrid {
    /// The entries, by entity index.
    pub(crate) fn entries(&self) -> &[GridEntry] {
        &self.entries
    }

    /// Gathers every hurtbox that can be met, with its sweep, and sorts its
    /// cells.
    pub(crate) fn rebuild(&mut self, world: &World) {
        self.entries.clear();
        self.cells.clear();
        self.oversized.clear();
        self.bounds = None;
        for (entity, hurtbox) in world.query::<(Entity, &Hurtbox)>() {
            if !takes_part(world, entity) {
                continue;
            }
            if let Some(sweep) = sweep_of(world, entity, hurtbox.shape().shape(), hurtbox.offset())
            {
                self.entries.push(GridEntry { entity, sweep });
            }
        }
        self.entries.sort_by_key(|entry| entry.entity.id());
        for (index, entry) in self.entries.iter().enumerate() {
            let (min, max) = entry.sweep.bounds();
            self.bounds = Some(
                self.bounds
                    .map_or((min, max), |(lo, hi)| (lo.min(min), hi.max(max))),
            );
            let (lo, hi, count) = cell_span(min, max);
            if count > MAX_ENTRY_CELLS {
                self.oversized.push(index as u32);
                continue;
            }
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    self.cells.push((key(x, y), index as u32));
                }
            }
        }
        self.cells.sort_unstable();
    }

    /// The entries whose bounds may meet the box `min..max`, in entry order,
    /// each once: from the cells the box covers and the oversized list, or
    /// every entry when the box, clamped to the occupied bounds, covers
    /// more cells than four per entry, and at least 64.
    pub(crate) fn candidates(&self, min: Vec2, max: Vec2, out: &mut Vec<u32>) {
        out.clear();
        let Some((lo, hi)) = self.bounds else {
            return;
        };
        let pad = Vec2::splat(QUERY_PAD);
        let (min, max) = ((min - pad).max(lo), (max + pad).min(hi));
        if !(min.x <= max.x && min.y <= max.y) {
            return;
        }
        let (from, to, count) = cell_span(min, max);
        if count > (4 * self.entries.len() as i64).max(64) {
            out.extend(0..self.entries.len() as u32);
            return;
        }
        for x in from.0..=to.0 {
            for y in from.1..=to.1 {
                let cell = key(x, y);
                let start = self.cells.partition_point(|&(k, _)| k < cell);
                out.extend(
                    self.cells[start..]
                        .iter()
                        .take_while(|&&(k, _)| k == cell)
                        .map(|&(_, index)| index),
                );
            }
        }
        out.extend_from_slice(&self.oversized);
        out.sort_unstable();
        out.dedup();
    }
}

/// [`HIT_GRID`]'s work: the grid rebuilt from the World's hurtboxes.
pub(crate) fn hit_grid(world: &mut World) {
    let mut grid = world.remove_resource::<HitGrid>().unwrap_or_default();
    grid.rebuild(world);
    world.insert_resource(grid);
}

/// The open interval of `t` over which `start + t·motion` lies strictly
/// inside a convex shape about the origin, with the shape's outward normal
/// where the line enters it; `None` when it never does.
type Span = Option<(f64, f64, DVec2)>;

/// [`Span`] of the disk of `radius`.
fn disk(start: DVec2, motion: DVec2, radius: f64) -> Span {
    let speed = motion.dot(motion);
    let excess = start.dot(start) - radius * radius;
    if speed == 0.0 {
        return (excess < 0.0).then_some((f64::NEG_INFINITY, f64::INFINITY, DVec2::ZERO));
    }
    let along = start.dot(motion);
    let discriminant = along * along - speed * excess;
    if discriminant <= 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let enter = (-along - root) / speed;
    let normal = (start + motion * enter).normalize_or_zero();
    Some((enter, (-along + root) / speed, normal))
}

/// [`Span`] of the box of half extents `half`: the slabs' overlap, entering
/// through the face of the axis it enters last.
fn slab(start: DVec2, motion: DVec2, half: DVec2) -> Span {
    let (mut enter, mut leave) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut normal = DVec2::ZERO;
    for (axis, p, d, h) in [
        (DVec2::X, start.x, motion.x, half.x),
        (DVec2::Y, start.y, motion.y, half.y),
    ] {
        if d == 0.0 {
            if p.abs() >= h {
                return None;
            }
            continue;
        }
        let (a, b) = ((-h - p) / d, (h - p) / d);
        if a.min(b) > enter {
            enter = a.min(b);
            normal = axis * -d.signum();
        }
        leave = leave.min(a.max(b));
    }
    (enter < leave).then_some((enter, leave, normal))
}

/// [`Span`] of the box of half extents `half` grown by `radius`, its
/// corners round: two boxes and four disks, whose union is convex, so the
/// span runs from the earliest entry, whose normal it keeps, to the latest
/// exit.
fn rounded(start: DVec2, motion: DVec2, half: DVec2, radius: f64) -> Span {
    let corners = [
        DVec2::new(half.x, half.y),
        DVec2::new(-half.x, half.y),
        DVec2::new(half.x, -half.y),
        DVec2::new(-half.x, -half.y),
    ];
    [
        slab(start, motion, half + DVec2::new(radius, 0.0)),
        slab(start, motion, half + DVec2::new(0.0, radius)),
    ]
    .into_iter()
    .chain(
        corners
            .into_iter()
            .map(|corner| disk(start - corner, motion, radius)),
    )
    .flatten()
    .reduce(|a, b| {
        let first = if b.0 < a.0 { b } else { a };
        (first.0, a.1.max(b.1), first.2)
    })
}

/// The span of `a` relative to `b`, their centres apart by `start` and
/// moving apart by `motion`: a ray against their Minkowski sum.
fn span(a: Shape, b: Shape, start: DVec2, motion: DVec2) -> Span {
    match (a, b) {
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            disk(start, motion, f64::from(ra) + f64::from(rb))
        }
        (Shape::Aabb { half_extents: ha }, Shape::Aabb { half_extents: hb }) => {
            slab(start, motion, (ha + hb).as_dvec2())
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents })
        | (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            rounded(start, motion, half_extents.as_dvec2(), f64::from(radius))
        }
    }
}

/// The relative start and motion of `a` against `b`.
fn relative(a: &Sweep, b: &Sweep) -> (DVec2, DVec2) {
    (
        (a.start - b.start).as_dvec2(),
        ((a.end - a.start) - (b.end - b.start)).as_dvec2(),
    )
}

/// The first `t` in `[0, 1]` of the step at which `a` overlaps `b` with
/// penetration above 0, each moving from its start to its end: the ray of
/// their relative motion against their Minkowski sum, 0 when they start
/// overlapping; `None` when they never overlap in the step, a graze
/// included.
pub(crate) fn first_contact(a: &Sweep, b: &Sweep) -> Option<f32> {
    let (start, motion) = relative(a, b);
    let (enter, leave, _) = span(a.shape, b.shape, start, motion)?;
    if enter >= 1.0 || leave <= 0.0 {
        return None;
    }
    Some(enter.max(0.0) as f32)
}

/// The direction that pushes `a` out of `b` when their centres are apart
/// by `start` and they overlap: along the axis of least depth for boxes,
/// from the nearest point of a box to a circle, between two circles' centres.
fn push_out(a: Shape, b: Shape, start: DVec2) -> DVec2 {
    let least = |half: DVec2| {
        let depth = half - start.abs();
        if depth.x < depth.y {
            DVec2::new(start.x.signum(), 0.0)
        } else {
            DVec2::new(0.0, start.y.signum())
        }
    };
    match (a, b) {
        (Shape::Circle { .. }, Shape::Circle { .. }) => start.normalize_or(DVec2::Y),
        (Shape::Aabb { half_extents: ha }, Shape::Aabb { half_extents: hb }) => {
            least((ha + hb).as_dvec2())
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents })
        | (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            let half = half_extents.as_dvec2();
            if start.x.abs() < half.x && start.y.abs() < half.y {
                least(half + DVec2::splat(f64::from(radius)))
            } else {
                (start - start.clamp(-half, half)).normalize_or(DVec2::Y)
            }
        }
    }
}

/// Where `a`, moving over the step, first meets `wall`, with the wall's
/// outward normal there: its first contact and the face it enters through;
/// or, when it starts overlapping the wall, at 0 with the push-out normal
/// if it moves into the wall and nowhere if it moves out, so a projectile
/// can always leave a wall it touches.
pub(crate) fn wall_contact(a: &Sweep, wall: &Sweep) -> Option<(f32, Vec2)> {
    let (start, motion) = relative(a, wall);
    let (enter, leave, normal) = span(a.shape, wall.shape, start, motion)?;
    if enter >= 1.0 || leave <= 0.0 {
        return None;
    }
    if enter >= 0.0 {
        return Some((enter as f32, normal.as_vec2()));
    }
    let out = push_out(a.shape, wall.shape, start);
    (motion.dot(out) < 0.0).then(|| (0.0, out.as_vec2()))
}

/// `velocity` reflected about the unit `normal`.
pub(crate) fn reflect(velocity: Vec2, normal: Vec2) -> Vec2 {
    velocity - normal * (2.0 * velocity.dot(normal))
}

/// One hit an item landed: the hitting and the hit entity, both shapes'
/// centres where they met, the relative motion, and what the hit carries.
pub(crate) struct Blow<'a> {
    pub(crate) hitter: Entity,
    pub(crate) target: Entity,
    pub(crate) hitter_at: Vec2,
    pub(crate) target_at: Vec2,
    pub(crate) relative: Vec2,
    pub(crate) damage: f32,
    pub(crate) tags: &'a Tags,
    pub(crate) knockback: f32,
}

/// Lands `blow`: sends [`Struck`], traces `hit` on the hitter and, when
/// its damage is above 0, queues a [`DamageRequest`] from the hitter's
/// [`Source`] or the hitter itself, at the target's centre, pushing the
/// target away from the hitter, else along the relative motion.
pub(crate) fn strike(world: &mut World, blow: &Blow<'_>) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<Struck>>() {
        queue.send(Struck::new(
            blow.hitter,
            blow.target,
            blow.hitter_at,
            blow.damage,
        ));
    }
    trace(world, blow.hitter, || TraceKind::Event {
        name: "hit",
        other: Some(blow.target),
        amount: Some(blow.damage),
    });
    if blow.damage <= 0.0 {
        return;
    }
    let source = world
        .get::<Source>(blow.hitter)
        .map_or(blow.hitter, |source| source.entity());
    let mut request = DamageRequest::new(blow.target, blow.damage)
        .with_source(source)
        .with_tags(blow.tags.clone())
        .with_position(blow.target_at);
    if blow.knockback > 0.0 {
        let away = (blow.target_at - blow.hitter_at).normalize_or_zero();
        let direction = if away == Vec2::ZERO {
            blow.relative.normalize_or_zero()
        } else {
            away
        };
        if direction != Vec2::ZERO {
            request = request.with_knockback(direction * blow.knockback);
        }
    }
    request.queue(world);
}

#[cfg(test)]
#[path = "../tests/grid.rs"]
mod tests;
