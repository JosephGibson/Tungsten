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
    /// The candidates under their save-order keys and places while a build
    /// sorts them, kept so a warm build allocates nothing.
    keyed: Vec<((bool, u64), usize, Candidate)>,
    /// `(cell key, candidate)`, sorted.
    cells: Vec<(u64, u32)>,
    /// Each candidate's cell key, by candidate: while a build finds them
    /// all as the last one left them, the sorted cells and their directory
    /// stand, and only the packed entries are read again.
    keys: Vec<u64>,
    /// `cells`' candidates as a query's scan reads them, in `cells`' order.
    packed: Vec<Packed>,
    /// Where each cell's entries start, while the cells' box is small.
    directory: Directory,
    /// The farthest a candidate's surface lies from its centre: a query
    /// widens its cells by it.
    reach: f32,
    built: bool,
    /// The build holds the `Health` holders alone, each `tagged` as it
    /// carries `Tags`: what a blast that sends no stimulus reads
    /// ([`blast`](crate::blast)).
    pub(crate) health_only: bool,
    /// Tests: the builds that sorted the cells, and a switch that makes
    /// every build sort them, as each did.
    #[cfg(test)]
    pub(crate) sorted: u32,
    #[cfg(test)]
    pub(crate) sort_every_build: bool,
}

/// A candidate under its cell: its centre, its place among the candidates
/// and whether it has a shape, side by side for a scan of a cell's entries.
#[derive(Debug, Clone, Copy)]
struct Packed {
    centre: Vec2,
    index: u32,
    shaped: bool,
}

/// A directory may hold this many cells a candidate; past it a query
/// searches the sorted cells, as every query did.
const DIRECTORY_CELLS: u64 = 4;

/// Each cell's first entry in the sorted cells, over the box the occupied
/// cells span: a query reads a column's rows from it in place of two
/// searches of the cells.
#[derive(Debug, Default)]
struct Directory {
    /// The box's first column and row, as [`coordinate`] gives them.
    column: u64,
    row: u64,
    /// Its columns, and its rows a column; 0 rows while there is no table.
    columns: u64,
    rows: u64,
    /// By column, then row: the entries before the cell's own, with one
    /// more count that ends the last cell.
    starts: Vec<u32>,
}

impl Directory {
    /// Rebuilds the table over `cells`, sorted, or leaves none when their
    /// box holds more than [`DIRECTORY_CELLS`] cells an entry.
    fn build(&mut self, cells: &[(u64, u32)]) {
        self.rows = 0;
        self.starts.clear();
        let (Some(&(first, _)), Some(&(last, _))) = (cells.first(), cells.last()) else {
            return;
        };
        let row_of = |key: u64| key & u64::from(u32::MAX);
        let (mut low, mut high) = (u64::MAX, 0);
        for &(key, _) in cells {
            low = low.min(row_of(key));
            high = high.max(row_of(key));
        }
        let (columns, rows) = ((last >> 32) - (first >> 32) + 1, high - low + 1);
        let slots = columns.saturating_mul(rows);
        if slots > DIRECTORY_CELLS.saturating_mul(cells.len() as u64) {
            return;
        }
        (self.column, self.row, self.columns, self.rows) = (first >> 32, low, columns, rows);
        // The cells are sorted by column, then row, so their slots only
        // rise: each slot up to an entry's own starts at that entry.
        self.starts.resize(slots as usize + 1, cells.len() as u32);
        let mut next = 0;
        for (at, &(key, _)) in cells.iter().enumerate() {
            let slot = (((key >> 32) - self.column) * rows + (row_of(key) - self.row)) as usize;
            while next <= slot {
                self.starts[next] = at as u32;
                next += 1;
            }
        }
    }

    /// The entries of `column`'s cells from row `from` to row `to`, as a
    /// range of the sorted cells: the one the two searches give.
    fn rows_of(&self, cells: &[(u64, u32)], column: u64, from: u64, to: u64) -> (usize, usize) {
        // A coordinate at the clamp's top end is past a key's 32 bits, and
        // the keys it makes wrap: those rows keep the searches' answer.
        let wraps = column > u64::from(u32::MAX) || to > u64::from(u32::MAX);
        if self.rows == 0 || wraps {
            return (
                cells.partition_point(|&(at, _)| at < key(column, from)),
                cells.partition_point(|&(at, _)| at <= key(column, to)),
            );
        }
        let (from, to) = (from.max(self.row), to.min(self.row + self.rows - 1));
        if column < self.column || column >= self.column + self.columns || from > to {
            return (0, 0);
        }
        let first = ((column - self.column) * self.rows + (from - self.row)) as usize;
        let last = ((column - self.column) * self.rows + (to - self.row)) as usize;
        (self.starts[first] as usize, self.starts[last + 1] as usize)
    }
}

/// The largest `f32` whose square root is at most `radius`, a finite radius
/// that is not negative: since the root never falls as its square rises,
/// `d.sqrt() <= radius` exactly when `d <= square_bound(radius)`, for every
/// `d` a squared length can be.
fn square_bound(radius: f32) -> f32 {
    let mut bound = (radius * radius).min(f32::MAX);
    // The product is rounded, and so is a root: step to the last square
    // whose root still fits.
    while bound.sqrt() > radius {
        bound = bound.next_down();
    }
    while bound < f32::MAX && bound.next_up().sqrt() <= radius {
        bound = bound.next_up();
    }
    bound
}

/// A cell coordinate, clamped to `i32` and offset so keys sort by column,
/// then row: the floor of `value / CELL`, 0 for a NaN, taken by a truncation
/// and a step down for a negative fraction, with no call to `floorf`. The
/// clamp's bounds are whole, so clamping first gives the floor's clamp.
fn coordinate(value: f32) -> u64 {
    let scaled = value / CELL;
    let cell = if scaled.is_nan() {
        0
    } else {
        // At most 2^31 either way, so the cast is exact, and its `f32` is
        // the float itself or the whole number it was cut to.
        let scaled = scaled.clamp(i32::MIN as f32, i32::MAX as f32);
        let whole = scaled as i64;
        whole - i64::from(whole as f32 > scaled)
    };
    (cell + (1_i64 << 31)) as u64
}

/// Tests: [`coordinate`] as it stood, by `f32::floor`.
#[cfg(test)]
pub(crate) fn coordinate_by_floor(value: f32) -> u64 {
    let cell = (value / CELL).floor();
    let cell = if cell.is_nan() {
        0
    } else {
        cell.clamp(i32::MIN as f32, i32::MAX as f32) as i64
    };
    (cell + (1_i64 << 31)) as u64
}

/// Tests: [`coordinate`].
#[cfg(test)]
pub(crate) fn coordinate_of(value: f32) -> u64 {
    coordinate(value)
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
    /// `Health` and a point, or with `Health` and a point while it holds
    /// the health holders alone.
    pub(crate) fn build(&mut self, world: &World) {
        if self.built {
            return;
        }
        self.built = true;
        self.candidates.clear();
        self.reach = 0.0;
        if !self.health_only {
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
        }
        for (entity, _) in world.query::<(Entity, &Health)>() {
            let tagged = world.has::<Tags>(entity);
            if tagged && !self.health_only {
                continue;
            }
            if let Some((centre, shape)) = locate(world, entity) {
                self.candidates.push(Candidate {
                    entity,
                    centre,
                    shape,
                    tagged,
                });
            }
        }
        // In save order, each key read once: a saved entity's is a lookup,
        // and the sort compares often. The place breaks a tie, so the
        // unstable sort gives the stable sort's order with no buffer.
        let order = order_of(world);
        self.keyed.clear();
        self.keyed.extend(
            self.candidates
                .drain(..)
                .enumerate()
                .map(|(at, candidate)| (order.key(candidate.entity), at, candidate)),
        );
        self.keyed.sort_unstable_by_key(|&(key, at, _)| (key, at));
        self.candidates
            .extend(self.keyed.drain(..).map(|(_, _, candidate)| candidate));
        // The candidates' keys, and whether each is the last build's: the
        // cells are `(key, candidate)` sorted, so they stand while every
        // candidate keeps its key.
        let mut same = self.keys.len() == self.candidates.len();
        self.keys.resize(self.candidates.len(), 0);
        for (candidate, kept) in self.candidates.iter().zip(&mut self.keys) {
            if let Some(shape) = candidate.shape {
                self.reach = self.reach.max(shape.half_extents().length());
            }
            let key = key(
                coordinate(candidate.centre.x),
                coordinate(candidate.centre.y),
            );
            same &= *kept == key;
            *kept = key;
        }
        #[cfg(test)]
        {
            same &= !self.sort_every_build;
            self.sorted += u32::from(!same);
        }
        if !same {
            self.cells.clear();
            self.cells
                .extend(self.keys.iter().zip(0..).map(|(&key, index)| (key, index)));
            self.cells.sort_unstable();
            self.directory.build(&self.cells);
        }
        self.packed.clear();
        self.packed.extend(self.cells.iter().map(|&(_, index)| {
            let candidate = &self.candidates[index as usize];
            Packed {
                centre: candidate.centre,
                index,
                shaped: candidate.shape.is_some(),
            }
        }));
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
        // A candidate with no shape is `radius` away or nearer when its
        // squared distance is at most this: the same answer as its
        // distance's, without the root.
        let bound = square_bound(radius);
        for column in x0..=x1 {
            let (from, to) = self.directory.rows_of(&self.cells, column, y0, y1);
            for entry in &self.packed[from..to] {
                let near = if entry.shaped {
                    self.candidates[entry.index as usize].distance(point) <= radius
                } else {
                    (point - entry.centre).length_squared() <= bound
                };
                if near {
                    out.push(entry.index);
                }
            }
        }
        out.sort_unstable();
    }

    /// Tests: [`Self::within`] as it stood: two searches of the sorted
    /// cells a column and each candidate's own distance.
    #[cfg(test)]
    pub(crate) fn within_by_search(&self, point: Vec2, radius: f32, out: &mut Vec<u32>) {
        out.clear();
        if !(point.is_finite() && radius.is_finite() && radius >= 0.0) {
            return;
        }
        let widened = radius + self.reach;
        let (x0, x1) = (coordinate(point.x - widened), coordinate(point.x + widened));
        let (y0, y1) = (coordinate(point.y - widened), coordinate(point.y + widened));
        let span = (x1 - x0 + 1).saturating_mul(y1 - y0 + 1);
        if span > self.candidates.len() as u64 {
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

    /// Tests: whether the last build made a directory, and the candidates
    /// it holds.
    #[cfg(test)]
    pub(crate) fn directory_and_len(&self) -> (bool, usize) {
        (self.directory.rows != 0, self.candidates.len())
    }

    /// Tests: [`square_bound`].
    #[cfg(test)]
    pub(crate) fn square_bound(radius: f32) -> f32 {
        square_bound(radius)
    }
}
