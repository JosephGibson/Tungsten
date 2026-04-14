//! Uniform spatial grid used by the physics step to cull collision
//! pairs. Every collider is inserted into each cell its world-space
//! AABB overlaps; candidate pairs come from querying cells that a
//! test AABB overlaps.
//!
//! Cell size is a knob on `PhysicsConfig::broadphase_cell_size`. A
//! reasonable default is one to two tile widths — small enough that
//! each cell rarely holds more than a handful of proxies, large
//! enough that most proxies land in a single cell. The grid is
//! rebuilt from scratch each physics step; there is no incremental
//! update.

use super::collision::Aabb;
use glam::IVec2;
#[cfg(test)]
use glam::Vec2;
use std::collections::HashMap;

/// Opaque id used by `SpatialGrid` to refer back to an inserted proxy.
/// The physics step stores one proxy per collider-bearing entity and
/// one per tilemap-derived static tile; the returned id is the index
/// into whatever parallel `Vec` the caller uses to resolve it.
pub type ProxyId = u32;

/// Uniform grid broad-phase. Cells are integer-indexed from the floor
/// of `position / cell_size`; negative coordinates and unbounded worlds
/// both work because `HashMap<IVec2, _>` allocates on demand.
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<IVec2, Vec<ProxyId>>,
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        debug_assert!(cell_size > 0.0, "cell_size must be positive");
        Self {
            cell_size: cell_size.max(1.0),
            cells: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Insert `id` into every cell that `aabb` overlaps. Called once
    /// per proxy per physics step.
    pub fn insert(&mut self, id: ProxyId, aabb: &Aabb) {
        let (min_cell, max_cell) = self.cell_range(aabb);
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                self.cells.entry(IVec2::new(x, y)).or_default().push(id);
            }
        }
    }

    /// Collect every unique proxy whose cells overlap `query`. Includes
    /// `exclude` only if it was never inserted; the caller is expected
    /// to filter self-pairs after the fact.
    pub fn query(&self, query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>) {
        out.clear();
        let (min_cell, max_cell) = self.cell_range(query);
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                if let Some(bucket) = self.cells.get(&IVec2::new(x, y)) {
                    for &id in bucket {
                        if Some(id) == exclude {
                            continue;
                        }
                        if !out.contains(&id) {
                            out.push(id);
                        }
                    }
                }
            }
        }
    }

    fn cell_range(&self, aabb: &Aabb) -> (IVec2, IVec2) {
        let inv = 1.0 / self.cell_size;
        let min = aabb.min() * inv;
        let max = aabb.max() * inv;
        // `ceil - 1` on max so a shape whose right edge sits exactly on
        // a cell boundary doesn't spuriously claim the next cell over.
        let min_cell = IVec2::new(min.x.floor() as i32, min.y.floor() as i32);
        let max_cell = IVec2::new(
            (max.x - f32::EPSILON).floor() as i32,
            (max.y - f32::EPSILON).floor() as i32,
        );
        // Guard against the case where max < min due to an empty AABB.
        let max_cell = IVec2::new(max_cell.x.max(min_cell.x), max_cell.y.max(min_cell.y));
        (min_cell, max_cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aabb(cx: f32, cy: f32, hx: f32, hy: f32) -> Aabb {
        Aabb::new(Vec2::new(cx, cy), Vec2::new(hx, hy))
    }

    #[test]
    fn single_cell_insert_and_query() {
        let mut grid = SpatialGrid::new(32.0);
        let a = aabb(16.0, 16.0, 4.0, 4.0);
        grid.insert(0, &a);
        let mut out = Vec::new();
        grid.query(&a, None, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn shape_spanning_cells_is_returned_once() {
        let mut grid = SpatialGrid::new(32.0);
        // AABB spans 4 cells (straddles origin).
        let a = aabb(0.0, 0.0, 40.0, 40.0);
        grid.insert(7, &a);
        let mut out = Vec::new();
        grid.query(&a, None, &mut out);
        assert_eq!(out, vec![7]);
    }

    #[test]
    fn separated_shapes_dont_collide() {
        let mut grid = SpatialGrid::new(32.0);
        grid.insert(0, &aabb(16.0, 16.0, 4.0, 4.0));
        grid.insert(1, &aabb(200.0, 200.0, 4.0, 4.0));
        let mut out = Vec::new();
        grid.query(&aabb(16.0, 16.0, 4.0, 4.0), Some(0), &mut out);
        assert!(out.is_empty(), "got: {out:?}");
    }

    #[test]
    fn neighbours_are_candidates() {
        let mut grid = SpatialGrid::new(32.0);
        grid.insert(0, &aabb(0.0, 0.0, 4.0, 4.0));
        grid.insert(1, &aabb(6.0, 0.0, 4.0, 4.0));
        let mut out = Vec::new();
        grid.query(&aabb(0.0, 0.0, 4.0, 4.0), Some(0), &mut out);
        assert_eq!(out, vec![1]);
    }

    #[test]
    fn clear_empties_grid() {
        let mut grid = SpatialGrid::new(32.0);
        grid.insert(0, &aabb(0.0, 0.0, 4.0, 4.0));
        grid.clear();
        let mut out = Vec::new();
        grid.query(&aabb(0.0, 0.0, 4.0, 4.0), None, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn negative_coords_work() {
        let mut grid = SpatialGrid::new(32.0);
        grid.insert(0, &aabb(-100.0, -100.0, 4.0, 4.0));
        let mut out = Vec::new();
        grid.query(&aabb(-100.0, -100.0, 4.0, 4.0), None, &mut out);
        assert_eq!(out, vec![0]);
    }
}
