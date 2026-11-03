//! Chemistry's receivers in a radius: a grid of 64 px cells over every
//! entity with `Tags` or `Health` and a point, built once a run that has
//! radius work, its candidates tested exactly.

use glam::Vec2;
use tungsten_core::physics::{Collider, Position, Shape};
use tungsten_core::{Entity, Transform, World};

use crate::damage::health::Health;
use crate::foundation::saved::order_of;
use crate::foundation::tags::Tags;

/// A cell's side, in px.
const CELL: f32 = 64.0;

/// Where `entity` is for chemistry: a body's `Position` plus its collider's
/// offset, with the collider's shape, else its `Transform`'s translation
/// with none.
pub(crate) fn locate(world: &World, entity: Entity) -> Option<(Vec2, Option<Shape>)> {
    if let Some(position) = world.get::<Position>(entity) {
        return Some(match world.get::<Collider>(entity) {
            Some(collider) => (position.0 + collider.offset, Some(collider.shape)),
            None => (position.0, None),
        });
    }
    world
        .get::<Transform>(entity)
        .map(|transform| (transform.position, None))
}

/// One entity the grid holds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Candidate {
    pub(crate) entity: Entity,
    /// Its point: a body's shape's centre, else its translation.
    pub(crate) centre: Vec2,
    pub(crate) shape: Option<Shape>,
    /// It carries `Tags`, so a stimulus reaches it.
    pub(crate) tagged: bool,
}

impl Candidate {
    /// How far `point` lies from it: from a body's surface, 0 inside it,
    /// else from its point.
    pub(crate) fn distance(&self, point: Vec2) -> f32 {
        match self.shape {
            Some(shape) => shape.distance(point - self.centre),
            None => (point - self.centre).length(),
        }
    }
}

/// The grid: the candidates in [`order_of`]'s order, each under its
/// centre's cell, kept between runs so a warm build allocates nothing.
#[derive(Default)]
pub(crate) struct Grid {
    candidates: Vec<Candidate>,
    /// `(cell key, candidate)`, sorted.
    cells: Vec<(u64, u32)>,
    /// The farthest a candidate's surface lies from its centre: a query
    /// widens its cells by it.
    reach: f32,
    built: bool,
}

/// A cell coordinate, clamped to `i32` and offset so keys sort by column,
/// then row.
fn coordinate(value: f32) -> u64 {
    let cell = (value / CELL).floor();
    let cell = if cell.is_nan() {
        0
    } else {
        cell.clamp(i32::MIN as f32, i32::MAX as f32) as i64
    };
    (cell + (1_i64 << 31)) as u64
}

fn key(column: u64, row: u64) -> u64 {
    (column << 32) | row
}

impl Grid {
    /// Forgets the last run's build, so the next query builds anew.
    pub(crate) fn clear(&mut self) {
        self.built = false;
    }

    /// The candidate at `index`.
    pub(crate) fn get(&self, index: u32) -> &Candidate {
        &self.candidates[index as usize]
    }

    /// Builds the grid when this run has not: every entity with `Tags` or
    /// `Health` and a point.
    pub(crate) fn build(&mut self, world: &World) {
        if self.built {
            return;
        }
        self.built = true;
        self.candidates.clear();
        self.cells.clear();
        self.reach = 0.0;
        for (entity, _) in world.query::<(Entity, &Tags)>() {
            if let Some((centre, shape)) = locate(world, entity) {
                self.candidates.push(Candidate {
                    entity,
                    centre,
                    shape,
                    tagged: true,
                });
            }
        }
        for (entity, _) in world.query::<(Entity, &Health)>() {
            if world.has::<Tags>(entity) {
                continue;
            }
            if let Some((centre, shape)) = locate(world, entity) {
                self.candidates.push(Candidate {
                    entity,
                    centre,
                    shape,
                    tagged: false,
                });
            }
        }
        let order = order_of(world);
        self.candidates
            .sort_by_key(|candidate| order.key(candidate.entity));
        for (index, candidate) in self.candidates.iter().enumerate() {
            if let Some(shape) = candidate.shape {
                self.reach = self.reach.max(shape.half_extents().length());
            }
            self.cells.push((
                key(
                    coordinate(candidate.centre.x),
                    coordinate(candidate.centre.y),
                ),
                index as u32,
            ));
        }
        self.cells.sort_unstable();
    }

    /// Into `out`, in the candidates' order, every candidate within
    /// `radius` of `point`: its [`distance`](Candidate::distance) at most
    /// `radius`.
    pub(crate) fn within(&self, point: Vec2, radius: f32, out: &mut Vec<u32>) {
        out.clear();
        if !(point.is_finite() && radius.is_finite() && radius >= 0.0) {
            return;
        }
        let widened = radius + self.reach;
        let (x0, x1) = (coordinate(point.x - widened), coordinate(point.x + widened));
        let (y0, y1) = (coordinate(point.y - widened), coordinate(point.y + widened));
        let span = (x1 - x0 + 1).saturating_mul(y1 - y0 + 1);
        if span > self.candidates.len() as u64 {
            // Wider than the grid is full: every candidate, tested.
            for (index, candidate) in self.candidates.iter().enumerate() {
                if candidate.distance(point) <= radius {
                    out.push(index as u32);
                }
            }
            return;
        }
        for column in x0..=x1 {
            let from = self.cells.partition_point(|&(at, _)| at < key(column, y0));
            let to = self.cells.partition_point(|&(at, _)| at <= key(column, y1));
            for &(_, index) in &self.cells[from..to] {
                if self.candidates[index as usize].distance(point) <= radius {
                    out.push(index);
                }
            }
        }
        out.sort_unstable();
    }
}
