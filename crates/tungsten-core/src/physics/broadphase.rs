//! Uniform-grid broadphase: flat prefix-sum grid, built once per staging.
//!
//! `insert` stages `(id, cell range)` records; the first `query` after staging
//! builds the flat structure — count cell refs, exclusive prefix sum, scatter
//! ids into one flat entry array. Staged cells that fill a compact rectangle
//! get one slot per cell (D-080). Otherwise unbounded cell coordinates map
//! into a power-of-two slot table via a multiplicative hash; each entry
//! stores its exact cell, so hash aliasing merges buckets without false
//! candidates. Both layouts visit cells row by row and a cell's entries in
//! staging order, so a query returns the same ids in the same order.

use super::collision::Aabb;
use glam::IVec2;
#[cfg(test)]
use glam::Vec2;

/// Caller-owned proxy index.
pub type ProxyId = u32;

/// A direct table may spend this many slots per staged cell ref: about the
/// memory the hashed layout spends on its slots and exact cells.
const DIRECT_SLOTS_PER_REF: u64 = 4;

/// Staged insertion: covered cell range, resolved to slots at build time.
#[derive(Debug, Clone, Copy)]
struct Staged {
    id: ProxyId,
    min_cell: IVec2,
    max_cell: IVec2,
}

/// How a build maps a cell to a slot of `slot_starts`.
#[derive(Debug, Clone, Copy)]
enum Layout {
    /// One slot per cell of the staged bounds, row by row. No hashing or
    /// aliasing, and the cells of a row are adjacent slots.
    Direct {
        min: IVec2,
        max: IVec2,
        width: usize,
    },
    /// Power-of-two table behind `hash_cell`, for bounds too large or too
    /// sparse for a direct table.
    Hashed { slot_mask: u32 },
}

/// Flat prefix-sum grid; negative/unbounded coordinates allowed.
#[derive(Debug, Clone)]
pub struct SpatialGrid {
    cell_size: f32,
    staged: Vec<Staged>,
    /// Total (proxy, cell) refs staged; sizes the slot table and entry arrays.
    cell_refs: usize,
    /// Cell bounds of the staged records; inverted while nothing is staged.
    bounds_min: IVec2,
    bounds_max: IVec2,
    dirty: bool,
    layout: Layout,
    /// `slot_starts[s]..slot_starts[s + 1]` bounds slot `s` in the entry arrays.
    slot_starts: Vec<u32>,
    /// Scatter cursors; reset from `slot_starts` each build.
    cursors: Vec<u32>,
    entry_ids: Vec<ProxyId>,
    /// Exact cell per entry; hashed layout only.
    entry_cells: Vec<IVec2>,
    query_marks: Vec<u32>,
    query_generation: u32,
    /// True while every staged entry covers one cell under a distinct id; a
    /// query then meets each id at most once and skips the dedupe marks.
    single_cell_ids: bool,
    /// Per-id generation marks for the duplicate-id check at insert.
    insert_marks: Vec<u32>,
    insert_generation: u32,
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
            bounds_min: IVec2::MAX,
            bounds_max: IVec2::MIN,
            dirty: true,
            layout: Layout::Hashed { slot_mask: 0 },
            slot_starts: Vec::new(),
            cursors: Vec::new(),
            entry_ids: Vec::new(),
            entry_cells: Vec::new(),
            query_marks: Vec::new(),
            query_generation: 1,
            single_cell_ids: true,
            insert_marks: Vec::new(),
            insert_generation: 1,
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
        self.bounds_min = IVec2::MAX;
        self.bounds_max = IVec2::MIN;
        self.entry_ids.clear();
        self.entry_cells.clear();
        self.dirty = true;
        self.single_cell_ids = true;
        if self.insert_generation == u32::MAX {
            self.insert_marks.fill(0);
            self.insert_generation = 1;
        } else {
            self.insert_generation += 1;
        }
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
        self.bounds_min = self.bounds_min.min(min_cell);
        self.bounds_max = self.bounds_max.max(max_cell);
        if self.single_cell_ids {
            let mark = mark_slot(&mut self.insert_marks, id);
            self.single_cell_ids = span == 1 && *mark != self.insert_generation;
            *mark = self.insert_generation;
        }
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
        self.for_each_in(query, exclude, |id| out.push(id));
    }

    /// Visit each unique proxy overlapping `query` once, in the order `query`
    /// returns them, without the intermediate `Vec` write and re-read.
    pub fn for_each_in(
        &mut self,
        query: &Aabb,
        exclude: Option<ProxyId>,
        mut visit: impl FnMut(ProxyId),
    ) {
        if self.dirty {
            self.build();
        }
        if self.entry_ids.is_empty() {
            return;
        }
        let dedupe = !self.single_cell_ids;
        let generation = if dedupe { self.begin_query() } else { 0 };
        let (min_cell, max_cell) = self.cell_range(query);
        let slot_starts = &self.slot_starts;
        let entry_ids = &self.entry_ids;
        let query_marks = &mut self.query_marks;
        match self.layout {
            Layout::Direct { min, max, width } => {
                let lo = min_cell.max(min);
                let hi = max_cell.min(max);
                if lo.x > hi.x || lo.y > hi.y {
                    return;
                }
                let first = cells_between(min.x, lo.x);
                let last = cells_between(min.x, hi.x);
                for y in lo.y..=hi.y {
                    // Adjacent slots, so the row's entries are one run.
                    let row = cells_between(min.y, y) * width;
                    let start = slot_starts[row + first] as usize;
                    let end = slot_starts[row + last + 1] as usize;
                    for &id in &entry_ids[start..end] {
                        visit_entry(id, exclude, dedupe, generation, query_marks, &mut visit);
                    }
                }
            }
            Layout::Hashed { slot_mask } => {
                let entry_cells = &self.entry_cells;
                for y in min_cell.y..=max_cell.y {
                    for x in min_cell.x..=max_cell.x {
                        let cell = IVec2::new(x, y);
                        let slot = (hash_cell(cell) & slot_mask) as usize;
                        let start = slot_starts[slot] as usize;
                        let end = slot_starts[slot + 1] as usize;
                        for i in start..end {
                            // Exact-cell compare filters hash-aliased bucket entries.
                            if entry_cells[i] == cell {
                                let id = entry_ids[i];
                                visit_entry(
                                    id,
                                    exclude,
                                    dedupe,
                                    generation,
                                    query_marks,
                                    &mut visit,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Pick the layout for the staged bounds, then fill its slots.
    fn build(&mut self) {
        self.dirty = false;
        self.layout = self.choose_layout();
        match self.layout {
            Layout::Direct { min, max, width } => {
                let slots = width * (cells_between(min.y, max.y) + 1);
                self.fill(slots, false, |x, y| {
                    cells_between(min.y, y) * width + cells_between(min.x, x)
                });
            }
            Layout::Hashed { slot_mask } => {
                self.fill(slot_mask as usize + 1, true, |x, y| {
                    (hash_cell(IVec2::new(x, y)) & slot_mask) as usize
                });
            }
        }
    }

    /// Direct while one slot per cell of the staged bounds stays within
    /// `DIRECT_SLOTS_PER_REF`; hashed for larger or sparser worlds.
    fn choose_layout(&self) -> Layout {
        if self.cell_refs > 0 {
            let (min, max) = (self.bounds_min, self.bounds_max);
            let width = cells_between(min.x, max.x) as u64 + 1;
            let height = cells_between(min.y, max.y) as u64 + 1;
            let budget = DIRECT_SLOTS_PER_REF.saturating_mul(self.cell_refs as u64);
            if width
                .checked_mul(height)
                .is_some_and(|cells| cells <= budget)
            {
                return Layout::Direct {
                    min,
                    max,
                    width: width as usize,
                };
            }
        }
        let slots = self.cell_refs.next_power_of_two().max(64);
        Layout::Hashed {
            slot_mask: slots as u32 - 1,
        }
    }

    /// Count refs per slot, exclusive prefix sum, scatter ids (and exact
    /// cells for the hashed layout) in staging order.
    fn fill(&mut self, slots: usize, store_cells: bool, slot_of: impl Fn(i32, i32) -> usize) {
        self.slot_starts.clear();
        self.slot_starts.resize(slots + 1, 0);
        for staged in &self.staged {
            for y in staged.min_cell.y..=staged.max_cell.y {
                for x in staged.min_cell.x..=staged.max_cell.x {
                    self.slot_starts[slot_of(x, y) + 1] += 1;
                }
            }
        }
        for i in 1..=slots {
            self.slot_starts[i] += self.slot_starts[i - 1];
        }
        self.cursors.clear();
        self.cursors.extend_from_slice(&self.slot_starts[..slots]);
        self.entry_ids.resize(self.cell_refs, 0);
        if store_cells {
            self.entry_cells.resize(self.cell_refs, IVec2::ZERO);
        }
        for staged in &self.staged {
            for y in staged.min_cell.y..=staged.max_cell.y {
                for x in staged.min_cell.x..=staged.max_cell.x {
                    let slot = slot_of(x, y);
                    let at = self.cursors[slot] as usize;
                    self.cursors[slot] += 1;
                    self.entry_ids[at] = staged.id;
                    if store_cells {
                        self.entry_cells[at] = IVec2::new(x, y);
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
        let min_cell = IVec2::new(floor_to_i32(min.x), floor_to_i32(min.y));
        let max_cell = IVec2::new(
            floor_to_i32(max.x - f32::EPSILON),
            floor_to_i32(max.y - f32::EPSILON),
        );
        let max_cell = IVec2::new(max_cell.x.max(min_cell.x), max_cell.y.max(min_cell.y));
        (min_cell, max_cell)
    }
}

/// `x.floor() as i32` for every `f32`, without the `floorf` call a generic
/// x86-64 build makes (D-080): truncate toward zero, saturating as `as` does
/// (NaN gives 0), then step down for a negative non-integer.
#[inline]
fn floor_to_i32(x: f32) -> i32 {
    let truncated = x as i32;
    truncated.saturating_sub(i32::from((truncated as f32) > x))
}

/// One entry of a query: skip the excluded id and, when cells can repeat an
/// id, the ids this query already visited. Always inlined: both layouts' loops
/// call it per entry, and as a shared closure it was compiled out of line.
#[inline(always)]
fn visit_entry(
    id: ProxyId,
    exclude: Option<ProxyId>,
    dedupe: bool,
    generation: u32,
    query_marks: &mut Vec<u32>,
    visit: &mut impl FnMut(ProxyId),
) {
    if Some(id) == exclude {
        return;
    }
    if dedupe {
        let mark = mark_slot(query_marks, id);
        if *mark == generation {
            return;
        }
        *mark = generation;
    }
    visit(id);
}

/// Cells from `from` up to `to` (`to >= from`), exact over the whole `i32`
/// range.
#[inline]
fn cells_between(from: i32, to: i32) -> usize {
    to.wrapping_sub(from) as u32 as usize
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
