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

/// Caller-owned proxy index, dense: a grid keeps two tables of four bytes
/// per id up to the largest id it has seen, never shrunk, so a caller with
/// sparse keys maps them to dense indices first.
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

    /// Tests: the (proxy, cell) refs staged since the last clear.
    #[cfg(test)]
    pub(crate) fn cell_refs(&self) -> usize {
        self.cell_refs
    }

    /// True when at most `few` records are staged and none covers a cell of
    /// `query`: a query would visit nothing, so a caller can skip the walk.
    pub(crate) fn misses_few(&self, query: &Aabb, few: usize) -> bool {
        if self.staged.len() > few {
            return false;
        }
        let (min_cell, max_cell) = self.cell_range(query);
        !self.staged.iter().any(|staged| {
            staged.min_cell.x <= max_cell.x
                && staged.max_cell.x >= min_cell.x
                && staged.min_cell.y <= max_cell.y
                && staged.max_cell.y >= min_cell.y
        })
    }

    /// Stage `id`, a dense index ([`ProxyId`]), for every overlapped cell;
    /// built lazily on the next query.
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
        if self.single_cell_ids {
            self.for_each_in(query, exclude, |id| out.push(id));
        } else {
            self.query_repeats(query, exclude, out);
        }
    }

    /// [`Self::query`] of a grid whose cells repeat ids, by runs, each
    /// copied before its ids are pushed ([`Run::each_copied`]): a push is
    /// cheap beside a mispredicted repeat, and the dense query bench's
    /// boxes, 1.9 cells an entry, read a fifth faster for it. Out of line,
    /// so a grid of points keeps the walk by entries as its whole query.
    #[inline(never)]
    fn query_repeats(&mut self, query: &Aabb, exclude: Option<ProxyId>, out: &mut Vec<ProxyId>) {
        self.for_each_run_in(query, 0, |mut run| {
            run.each_copied(exclude, |id| out.push(id));
        });
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

    /// [`Self::for_each_in`]'s walk a [`Run`] at a time, for a visitor too
    /// large to inline at each entry: each id offset by `base`, a direct
    /// layout's row of cells or one cell's entries in a hashed slot.
    /// [`Run::each`] over every run visits what `for_each_in` does, in its
    /// order. The caller's loop over a run is its own code: its test of an
    /// id is compiled into that loop whether or not the call that hands the
    /// run over is inlined.
    #[inline(always)]
    pub(crate) fn for_each_run_in(
        &mut self,
        query: &Aabb,
        base: ProxyId,
        mut visit: impl FnMut(Run<'_>),
    ) {
        if self.dirty {
            self.build();
        }
        if self.entry_ids.is_empty() {
            return;
        }
        let generation = if self.single_cell_ids {
            0
        } else {
            self.begin_query()
        };
        let (min_cell, max_cell) = self.cell_range(query);
        let scattered = self.scattered();
        let mut firsts = [0; RUN_CHUNK];
        let slot_starts = &self.slot_starts;
        let entry_ids = &self.entry_ids;
        let entry_cells = &self.entry_cells;
        let query_marks = &mut self.query_marks;
        let layout = self.layout;
        let (lo, hi) = match layout {
            Layout::Direct { min, max, .. } => (min_cell.max(min), max_cell.min(max)),
            Layout::Hashed { .. } => (min_cell, max_cell),
        };
        if lo.x > hi.x || lo.y > hi.y {
            return;
        }
        // One loop for both layouts, so `visit` is called from one place
        // and compiles into it: a direct row's cells are adjacent slots,
        // one run; a hashed row is walked a cell at a time, a stretch of
        // the cell's entries at a time.
        let slot_of = |cell: IVec2, slot_mask: u32| {
            let slot = (hash_cell(cell) & slot_mask) as usize;
            (slot_starts[slot] as usize, slot_starts[slot + 1] as usize)
        };
        for y in lo.y..=hi.y {
            let mut x = lo.x;
            let (mut at, mut end) = match layout {
                Layout::Direct { min, width, .. } => {
                    let row = cells_between(min.y, y) * width;
                    (
                        slot_starts[row + cells_between(min.x, lo.x)] as usize,
                        slot_starts[row + cells_between(min.x, hi.x) + 1] as usize,
                    )
                }
                Layout::Hashed { slot_mask } => slot_of(IVec2::new(x, y), slot_mask),
            };
            loop {
                let (from, stop) = match layout {
                    Layout::Direct { .. } => (at, end),
                    Layout::Hashed { slot_mask } => loop {
                        let found = stretch(entry_cells, IVec2::new(x, y), at, end);
                        if found.0 != found.1 || x == hi.x {
                            break found;
                        }
                        x += 1;
                        (at, end) = slot_of(IVec2::new(x, y), slot_mask);
                    },
                };
                if from == stop {
                    break;
                }
                visit(Run {
                    ids: &entry_ids[from..stop],
                    base,
                    marks: &mut *query_marks,
                    generation,
                    scattered,
                    firsts: &mut firsts,
                });
                if matches!(layout, Layout::Direct { .. }) {
                    break;
                }
                at = stop;
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
        // A right/bottom edge exactly on a boundary does not claim the next
        // cell, at any distance from the origin: the last cell is
        // `ceil(max) - 1`, never before the first.
        let min_cell = IVec2::new(floor_to_i32(min.x), floor_to_i32(min.y));
        let max_cell = IVec2::new(
            ceil_to_i32(max.x).saturating_sub(1),
            ceil_to_i32(max.y).saturating_sub(1),
        );
        let max_cell = IVec2::new(max_cell.x.max(min_cell.x), max_cell.y.max(min_cell.y));
        (min_cell, max_cell)
    }
}

/// One stretch of a query's walk: entries in staging order, a direct
/// layout's row of cells or the entries of one cell in a hashed slot, with
/// what [`Self::each`] needs to visit them as the query does.
pub(crate) struct Run<'a> {
    ids: &'a [ProxyId],
    /// Added to each id: the second grid's offset in a walk of two.
    base: ProxyId,
    marks: &'a mut Vec<u32>,
    /// The query's dedupe generation; 0 while no id can repeat.
    generation: u32,
    /// The grid's entries cover [`SCATTERED_QUARTER_CELLS`] quarter cells
    /// or more on average, so a query meets most ids more than once.
    scattered: bool,
    /// Scratch of the walk for a copied run's first sightings.
    firsts: &'a mut [ProxyId; RUN_CHUNK],
}

/// Entries a [`Run`] takes at a time.
const RUN_CHUNK: usize = 64;

/// Quarter cells an entry covers on average from which a grid counts as
/// scattered ([`Run::each`]), 2.5 cells. Measured on the pair passes'
/// rows: example 01's pile covers 1.7 to 1.9, up to 2.8 in the frame after
/// a blast, and reads faster tested entry by entry; the pachinko board's
/// balls in flight, their boxes grown by their travel, cover 2.8 and read
/// faster copied.
const SCATTERED_QUARTER_CELLS: usize = 10;

impl Run<'_> {
    /// Visit the run's ids: skip `exclude` and, when cells can repeat an
    /// id, the ids the query already visited. Always inlined, with one call
    /// of `visit`, so the caller's code for an id compiles into this loop.
    ///
    /// Which ids a query has met is data a branch predictor learns where
    /// few repeat, or repeat in a pattern as a pile's do, and cannot where
    /// most do. A scattered grid's run is therefore copied as
    /// [`Self::each_copied`] does; any other is tested id by id. Both visit
    /// the same ids in the same order.
    #[inline(always)]
    pub(crate) fn each(&mut self, exclude: Option<ProxyId>, visit: impl FnMut(ProxyId)) {
        self.walk(self.scattered, exclude, visit);
    }

    /// [`Self::each`] with a run whose ids can repeat always copied first,
    /// [`RUN_CHUNK`] entries at a time: each id is written and then kept or
    /// overwritten by the next, with no branch on whether the query has
    /// met it, and `visit` runs over the ones kept.
    #[inline(always)]
    pub(crate) fn each_copied(&mut self, exclude: Option<ProxyId>, visit: impl FnMut(ProxyId)) {
        self.walk(true, exclude, visit);
    }

    #[inline(always)]
    fn walk(&mut self, copy: bool, exclude: Option<ProxyId>, mut visit: impl FnMut(ProxyId)) {
        let dedupe = self.generation != 0;
        let copied = dedupe && copy;
        for chunk in self.ids.chunks(RUN_CHUNK) {
            let ids = if copied {
                let mut kept = 0;
                for &id in chunk {
                    let mut first = Some(id + self.base) != exclude;
                    if first {
                        let mark = mark_slot(self.marks, id);
                        first = *mark != self.generation;
                        *mark = self.generation;
                    }
                    self.firsts[kept] = id;
                    kept += usize::from(first);
                }
                &self.firsts[..kept]
            } else {
                chunk
            };
            for &id in ids {
                if Some(id + self.base) == exclude {
                    continue;
                }
                if dedupe && !copied {
                    let mark = mark_slot(self.marks, id);
                    if *mark == self.generation {
                        continue;
                    }
                    *mark = self.generation;
                }
                visit(id + self.base);
            }
        }
    }
}

/// The next stretch of `cell`'s entries in `from..end` of its hashed slot,
/// which mixes the cells that alias to it: the next entries whose exact
/// cell is `cell`, empty when none is left.
#[inline(always)]
fn stretch(entry_cells: &[IVec2], cell: IVec2, from: usize, end: usize) -> (usize, usize) {
    let mut at = from;
    while at < end && entry_cells[at] != cell {
        at += 1;
    }
    let mut stop = at;
    while stop < end && entry_cells[stop] == cell {
        stop += 1;
    }
    (at, stop)
}

/// Visit each unique proxy overlapping `query` in `first` and in `second`,
/// whose ids are offset by `base`, in the order one grid that staged
/// `first`'s entries and then `second`'s returns them: both visit cells row
/// by row and a cell's entries in staging order, so each cell's `first`
/// entries come before its `second` ones. Each grid dedupes its own ids,
/// which the two never share. The two must share a cell size.
#[inline(always)]
pub(crate) fn for_each_in_both(
    first: &mut SpatialGrid,
    second: &mut SpatialGrid,
    base: ProxyId,
    query: &Aabb,
    exclude: Option<ProxyId>,
    mut visit: impl FnMut(ProxyId),
) {
    debug_assert!(
        (first.cell_size - second.cell_size).abs() <= f32::EPSILON,
        "both grids need one cell size"
    );
    if second.dirty {
        second.build();
    }
    if second.entry_ids.is_empty() {
        first.for_each_in(query, exclude, visit);
        return;
    }
    if first.dirty {
        first.build();
    }
    if first.entry_ids.is_empty() {
        let exclude = exclude.and_then(|id| id.checked_sub(base));
        second.for_each_in(query, exclude, |id| visit(id + base));
        return;
    }
    let first_generation = if first.single_cell_ids {
        None
    } else {
        Some(first.begin_query())
    };
    let second_generation = if second.single_cell_ids {
        None
    } else {
        Some(second.begin_query())
    };
    let (min_cell, max_cell) = first.cell_range(query);
    // Cells outside both grids' staged bounds hold nothing.
    let lo = min_cell.max(first.bounds_min.min(second.bounds_min));
    let hi = max_cell.min(first.bounds_max.max(second.bounds_max));
    for y in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            let cell = IVec2::new(x, y);
            first.visit_cell(cell, 0, exclude, first_generation, &mut visit);
            second.visit_cell(cell, base, exclude, second_generation, &mut visit);
        }
    }
}

impl SpatialGrid {
    /// Visit `cell`'s entries in staging order, each id offset by `base`:
    /// skip the excluded id and, with a generation, the ids this query
    /// already visited, as [`visit_entry`] does.
    #[inline(always)]
    fn visit_cell(
        &mut self,
        cell: IVec2,
        base: ProxyId,
        exclude: Option<ProxyId>,
        generation: Option<u32>,
        visit: &mut impl FnMut(ProxyId),
    ) {
        let (start, end) = match self.layout {
            Layout::Direct { min, max, width } => {
                if cell.x < min.x || cell.y < min.y || cell.x > max.x || cell.y > max.y {
                    return;
                }
                let slot = cells_between(min.y, cell.y) * width + cells_between(min.x, cell.x);
                (
                    self.slot_starts[slot] as usize,
                    self.slot_starts[slot + 1] as usize,
                )
            }
            Layout::Hashed { slot_mask } => {
                let slot = (hash_cell(cell) & slot_mask) as usize;
                (
                    self.slot_starts[slot] as usize,
                    self.slot_starts[slot + 1] as usize,
                )
            }
        };
        let hashed = matches!(self.layout, Layout::Hashed { .. });
        for at in start..end {
            // Exact-cell compare filters hash-aliased bucket entries.
            if hashed && self.entry_cells[at] != cell {
                continue;
            }
            let id = self.entry_ids[at];
            if Some(id + base) == exclude {
                continue;
            }
            if let Some(generation) = generation {
                let mark = mark_slot(&mut self.query_marks, id);
                if *mark == generation {
                    continue;
                }
                *mark = generation;
            }
            visit(id + base);
        }
    }
}

/// [`for_each_in_both`]'s walk a [`Run`] at a time, as
/// [`SpatialGrid::for_each_run_in`] is `for_each_in`'s: a cell's `first`
/// entries, then its `second` ones.
#[inline(always)]
pub(crate) fn for_each_run_in_both(
    first: &mut SpatialGrid,
    second: &mut SpatialGrid,
    base: ProxyId,
    query: &Aabb,
    mut visit: impl FnMut(Run<'_>),
) {
    debug_assert!(
        (first.cell_size - second.cell_size).abs() <= f32::EPSILON,
        "both grids need one cell size"
    );
    if second.dirty {
        second.build();
    }
    if second.entry_ids.is_empty() {
        first.for_each_run_in(query, 0, visit);
        return;
    }
    if first.dirty {
        first.build();
    }
    if first.entry_ids.is_empty() {
        second.for_each_run_in(query, base, visit);
        return;
    }
    let first_generation = if first.single_cell_ids {
        0
    } else {
        first.begin_query()
    };
    let second_generation = if second.single_cell_ids {
        0
    } else {
        second.begin_query()
    };
    let (min_cell, max_cell) = first.cell_range(query);
    let scattered = (first.scattered(), second.scattered());
    let mut firsts = [0; RUN_CHUNK];
    // Cells outside both grids' staged bounds hold nothing.
    let lo = min_cell.max(first.bounds_min.min(second.bounds_min));
    let hi = max_cell.min(first.bounds_max.max(second.bounds_max));
    for y in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            let cell = IVec2::new(x, y);
            // One call of `visit` for both grids' runs.
            for second_turn in [false, true] {
                let (grid, base, generation, scattered) = if second_turn {
                    (&mut *second, base, second_generation, scattered.1)
                } else {
                    (&mut *first, 0, first_generation, scattered.0)
                };
                let Some((mut at, end, hashed)) = grid.cell_slot(cell) else {
                    continue;
                };
                loop {
                    let (from, stop) = if hashed {
                        stretch(&grid.entry_cells, cell, at, end)
                    } else {
                        (at, end)
                    };
                    if from == stop {
                        break;
                    }
                    visit(Run {
                        ids: &grid.entry_ids[from..stop],
                        base,
                        marks: &mut grid.query_marks,
                        generation,
                        scattered,
                        firsts: &mut firsts,
                    });
                    if !hashed {
                        break;
                    }
                    at = stop;
                }
            }
        }
    }
}

impl SpatialGrid {
    /// Whether the staged entries cover [`SCATTERED_QUARTER_CELLS`]
    /// quarter cells or more on average.
    #[inline(always)]
    fn scattered(&self) -> bool {
        4 * self.cell_refs >= SCATTERED_QUARTER_CELLS * self.staged.len()
    }

    /// The entries of `cell`'s slot and whether the slot is a hashed one,
    /// which other cells share; none for a cell outside a direct layout.
    #[inline(always)]
    fn cell_slot(&self, cell: IVec2) -> Option<(usize, usize, bool)> {
        let (slot, hashed) = match self.layout {
            Layout::Direct { min, max, width } => {
                if cell.x < min.x || cell.y < min.y || cell.x > max.x || cell.y > max.y {
                    return None;
                }
                (
                    cells_between(min.y, cell.y) * width + cells_between(min.x, cell.x),
                    false,
                )
            }
            Layout::Hashed { slot_mask } => ((hash_cell(cell) & slot_mask) as usize, true),
        };
        Some((
            self.slot_starts[slot] as usize,
            self.slot_starts[slot + 1] as usize,
            hashed,
        ))
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

/// `x.ceil() as i32` for every `f32`, as [`floor_to_i32`] floors: truncate
/// toward zero, saturating as `as` does (NaN gives 0), then step up for a
/// positive non-integer.
#[inline]
fn ceil_to_i32(x: f32) -> i32 {
    let truncated = x as i32;
    truncated.saturating_add(i32::from((truncated as f32) < x))
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
