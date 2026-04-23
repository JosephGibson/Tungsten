//! Uniform-grid broadphase; rebuilt from scratch each physics step.

use super::collision::Aabb;
use glam::IVec2;
#[cfg(test)]
use glam::Vec2;
use std::collections::HashMap;

/// Caller-owned proxy index.
pub type ProxyId = u32;

/// HashMap-backed uniform grid; negative/unbounded coordinates allowed.
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<IVec2, Vec<ProxyId>>,
    query_marks: Vec<u32>,
    query_generation: u32,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self::new(32.0)
    }
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        debug_assert!(cell_size > 0.0, "cell_size must be positive");
        Self {
            cell_size: cell_size.max(1.0),
            cells: HashMap::new(),
            query_marks: Vec::new(),
            query_generation: 1,
        }
    }

    /// Change cell size and discard buckets.
    pub fn set_cell_size(&mut self, cell_size: f32) {
        debug_assert!(cell_size > 0.0, "cell_size must be positive");
        self.cell_size = cell_size.max(1.0);
        self.cells.clear();
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Insert `id` into every overlapped cell.
    pub fn insert(&mut self, id: ProxyId, aabb: &Aabb) {
        let (min_cell, max_cell) = self.cell_range(aabb);
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                self.cells.entry(IVec2::new(x, y)).or_default().push(id);
            }
        }
    }

    /// Collect unique proxies overlapping `query`; generation marks dedupe cells.
    pub fn query(&mut self, query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>) {
        out.clear();
        let generation = self.begin_query();
        let (min_cell, max_cell) = self.cell_range(query);
        let cells = &self.cells;
        let query_marks = &mut self.query_marks;
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                if let Some(bucket) = cells.get(&IVec2::new(x, y)) {
                    for &id in bucket {
                        if Some(id) == exclude {
                            continue;
                        }
                        let mark = mark_slot(query_marks, id);
                        if *mark == generation {
                            continue;
                        }
                        *mark = generation;
                        out.push(id);
                    }
                }
            }
        }
    }

    fn begin_query(&mut self) -> u32 {
        if self.query_generation == u32::MAX {
            self.query_marks.fill(0);
            self.query_generation = 1;
        }

        let generation = self.query_generation;
        self.query_generation += 1;
        generation
    }

    fn cell_range(&self, aabb: &Aabb) -> (IVec2, IVec2) {
        let inv = 1.0 / self.cell_size;
        let min = aabb.min() * inv;
        let max = aabb.max() * inv;
        // Right/bottom edge exactly on boundary does not claim next cell.
        let min_cell = IVec2::new(min.x.floor() as i32, min.y.floor() as i32);
        let max_cell = IVec2::new(
            (max.x - f32::EPSILON).floor() as i32,
            (max.y - f32::EPSILON).floor() as i32,
        );
        let max_cell = IVec2::new(max_cell.x.max(min_cell.x), max_cell.y.max(min_cell.y));
        (min_cell, max_cell)
    }
}

fn mark_slot(query_marks: &mut Vec<u32>, id: ProxyId) -> &mut u32 {
    let index = id as usize;
    if index >= query_marks.len() {
        query_marks.resize(index + 1, 0);
    }
    &mut query_marks[index]
}

#[cfg(test)]
#[path = "../tests/physics/broadphase.rs"]
mod tests;
