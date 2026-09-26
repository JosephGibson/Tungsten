//! Uniform-grid broadphase: flat prefix-sum spatial hash, built once per frame.
//!
//! `insert` stages `(id, cell range)` records; the first `query` after staging
//! builds the flat structure — count cell refs, exclusive prefix sum, scatter
//! ids into one flat entry array. Unbounded cell coordinates map into a
//! power-of-two slot table via a multiplicative hash; each entry stores its
//! exact cell, so hash aliasing merges buckets without false candidates.

use super::collision::Aabb;
use glam::IVec2;
#[cfg(test)]
use glam::Vec2;

/// Caller-owned proxy index.
pub type ProxyId = u32;

/// Staged insertion: covered cell range, resolved to slots at build time.
#[derive(Debug, Clone, Copy)]
struct Staged {
    id: ProxyId,
    min_cell: IVec2,
    max_cell: IVec2,
}

/// Flat prefix-sum spatial hash grid; negative/unbounded coordinates allowed.
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    cell_size: f32,
    staged: Vec<Staged>,
    /// Total (proxy, cell) refs staged; sizes the slot table and entry arrays.
    cell_refs: usize,
    dirty: bool,
    slot_mask: u32,
    /// `slot_starts[s]..slot_starts[s + 1]` bounds slot `s` in the entry arrays.
    slot_starts: Vec<u32>,
    /// Scatter cursors; reset from `slot_starts` each build.
    cursors: Vec<u32>,
    entry_ids: Vec<ProxyId>,
    entry_cells: Vec<IVec2>,
    query_marks: Vec<u32>,
    query_generation: u32,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self::new(32.0)
    }
}

impl SpatialGrid {
    #[must_use]
    pub fn new(cell_size: f32) -> Self {
        debug_assert!(cell_size > 0.0, "cell_size must be positive");
        Self {
            cell_size: cell_size.max(1.0),
            staged: Vec::new(),
            cell_refs: 0,
            dirty: true,
            slot_mask: 0,
            slot_starts: Vec::new(),
            cursors: Vec::new(),
            entry_ids: Vec::new(),
            entry_cells: Vec::new(),
            query_marks: Vec::new(),
            query_generation: 1,
        }
    }

    /// Change cell size and discard staged and built contents.
    pub fn set_cell_size(&mut self, cell_size: f32) {
        debug_assert!(cell_size > 0.0, "cell_size must be positive");
        self.cell_size = cell_size.max(1.0);
        self.clear();
    }

    pub fn clear(&mut self) {
        self.staged.clear();
        self.cell_refs = 0;
        self.entry_ids.clear();
        self.entry_cells.clear();
        self.dirty = true;
    }

    #[must_use]
    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Stage `id` for every overlapped cell; built lazily on the next query.
    pub fn insert(&mut self, id: ProxyId, aabb: &Aabb) {
        let (min_cell, max_cell) = self.cell_range(aabb);
        let span =
            ((max_cell.x - min_cell.x + 1) as usize) * ((max_cell.y - min_cell.y + 1) as usize);
        self.cell_refs += span;
        self.staged.push(Staged {
            id,
            min_cell,
            max_cell,
        });
        self.dirty = true;
    }

    /// Collect unique proxies overlapping `query`; generation marks dedupe cells.
    pub fn query(&mut self, query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>) {
        out.clear();
        if self.dirty {
            self.build();
        }
        if self.entry_ids.is_empty() {
            return;
        }
        let generation = self.begin_query();
        let (min_cell, max_cell) = self.cell_range(query);
        let slot_mask = self.slot_mask;
        let slot_starts = &self.slot_starts;
        let entry_ids = &self.entry_ids;
        let entry_cells = &self.entry_cells;
        let query_marks = &mut self.query_marks;
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                let cell = IVec2::new(x, y);
                let slot = (hash_cell(cell) & slot_mask) as usize;
                let start = slot_starts[slot] as usize;
                let end = slot_starts[slot + 1] as usize;
                for i in start..end {
                    // Exact-cell compare filters hash-aliased bucket entries.
                    if entry_cells[i] != cell {
                        continue;
                    }
                    let id = entry_ids[i];
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

    /// Count refs per slot, exclusive prefix sum, scatter ids + cells.
    fn build(&mut self) {
        self.dirty = false;
        let slots = self.cell_refs.next_power_of_two().max(64);
        let slot_mask = slots as u32 - 1;
        self.slot_mask = slot_mask;
        self.slot_starts.clear();
        self.slot_starts.resize(slots + 1, 0);
        for staged in &self.staged {
            for y in staged.min_cell.y..=staged.max_cell.y {
                for x in staged.min_cell.x..=staged.max_cell.x {
                    let slot = (hash_cell(IVec2::new(x, y)) & slot_mask) as usize;
                    self.slot_starts[slot + 1] += 1;
                }
            }
        }
        for i in 1..=slots {
            self.slot_starts[i] += self.slot_starts[i - 1];
        }
        self.cursors.clear();
        self.cursors.extend_from_slice(&self.slot_starts[..slots]);
        self.entry_ids.resize(self.cell_refs, 0);
        self.entry_cells.resize(self.cell_refs, IVec2::ZERO);
        for staged in &self.staged {
            for y in staged.min_cell.y..=staged.max_cell.y {
                for x in staged.min_cell.x..=staged.max_cell.x {
                    let cell = IVec2::new(x, y);
                    let slot = (hash_cell(cell) & slot_mask) as usize;
                    let at = self.cursors[slot] as usize;
                    self.cursors[slot] += 1;
                    self.entry_ids[at] = staged.id;
                    self.entry_cells[at] = cell;
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

/// Multiplicative spatial hash (Ericson RTCD constants) with an xor fold so
/// the low bits surviving the mask carry high-bit entropy.
fn hash_cell(cell: IVec2) -> u32 {
    let h = (cell.x as u32).wrapping_mul(0x8DA6_B343) ^ (cell.y as u32).wrapping_mul(0xD816_3841);
    h ^ (h >> 16)
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
