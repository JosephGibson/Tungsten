//! Tiles edited in place through the [`TilemapRegistry`]: cells written with
//! [`TilemapRegistry::set_tile`] and broken by
//! [`TilemapRegistry::damage_tile`], each written cell journaled, so a
//! consumer holding a [`MapCursor`] reads what changed since
//! ([`TilemapRegistry::changes_since`]) instead of comparing the map whole.
//! `insert` starts a map's history again: a new revision and fingerprint, an
//! empty journal, no damage or edits.
//!
//! A map's fingerprint covers all of it but its objects: a mix of its sizes,
//! each layer's name, kind and physics and the tileset's sprites and
//! properties, fixed at `insert`, XORed with a mix of each non-empty cell's
//! layer, column, row, tile and auto bit, updated per write. Equal
//! fingerprints mean equal maps, so a cursor reads [`TileChanges::Unchanged`]
//! only for a map that is the one it saw.
//!
//! The auto-tiler: a tile with a `style` and no `mask` is that style's
//! brush, one with a `mask` a variant. A brush cell is an auto cell, which
//! holds a variant of its style for its open sides: a side is closed when
//! the neighbour holds any tile of the style or is off the map, and the mask
//! sums 1 up, 2 right, 4 down and 8 left for the open ones. One of the mask's
//! variants is drawn by weight from a mix of the cell's layer, column and
//! row, so a cell keeps its variant while its mask holds; a mask without a
//! variant draws the brush. Any other tile is an override, counted by its
//! neighbours' masks and never re-picked.

use std::collections::{BTreeMap, VecDeque};

use super::tilemap::{
    EMPTY_TILE, LayerKind, RegisteredMap, TileIndex, TilemapData, TilemapRegistry,
};

/// The journal entries a map keeps; a cursor older than the oldest reads
/// [`TileChanges::Replaced`].
pub(crate) const JOURNAL_BOUND: usize = 4_096;

/// One cell of a map: its layer's index, column and row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellChange {
    /// The layer's index in [`TilemapData::layers`].
    pub layer: usize,
    /// The cell's column, from the map's left.
    pub col: u32,
    /// The cell's row, from the map's top.
    pub row: u32,
}

/// Where a map's history stood when [`TilemapRegistry::cursor`] read it: its
/// revision and its fingerprint then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapCursor {
    revision: u64,
    fingerprint: u64,
}

/// What changed in a map since a [`MapCursor`]
/// ([`TilemapRegistry::changes_since`]).
#[derive(Debug, Clone)]
pub enum TileChanges<'a> {
    /// The map is the one the cursor saw.
    Unchanged,
    /// The cells written since the cursor, oldest first; a cell may repeat.
    Cells(ChangedCells<'a>),
    /// Read the map whole: it was inserted again since the cursor, its
    /// journal no longer reaches back to it, or the cursor is from another
    /// history (another registry's, or before a replace) of another map.
    Replaced,
}

/// The cells journaled after a cursor, oldest first.
#[derive(Debug, Clone)]
pub struct ChangedCells<'a> {
    entries: std::collections::vec_deque::Iter<'a, Written>,
}

impl Iterator for ChangedCells<'_> {
    type Item = CellChange;

    fn next(&mut self) -> Option<CellChange> {
        self.entries.next().map(|written| written.cell)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl ExactSizeIterator for ChangedCells<'_> {}

/// What [`TilemapRegistry::damage_tile`] did to a cell.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum TileDamage {
    /// Nothing: the registry lacks the map, the cell is empty or its tile
    /// has no `health`.
    Unbreakable,
    /// The cell holds.
    Damaged {
        /// What the cell has left.
        health: f32,
    },
    /// The cell broke.
    Broken {
        /// What the cell now holds: its old tile's `broken`, [`EMPTY_TILE`]
        /// when it had none.
        tile: TileIndex,
    },
}

/// A journal entry: the cell written and the map's fingerprint after it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Written {
    cell: CellChange,
    fingerprint: u64,
}

/// A map's history since its `insert`: its revision and fingerprint, the
/// journal of written cells, and the cells damaged and written.
#[derive(Debug, Clone, Default)]
pub(crate) struct MapHistory {
    revision: u64,
    fingerprint: u64,
    /// The revision and fingerprint before the oldest journal entry; the
    /// entries' revisions follow it one by one.
    base: (u64, u64),
    journal: VecDeque<Written>,
    /// Each damaged cell's health, by (layer, row, col).
    damage: BTreeMap<(usize, u32, u32), f32>,
    /// Each cell the game wrote, by (layer, row, col), with the tile it last
    /// wrote there.
    edits: BTreeMap<(usize, u32, u32), TileIndex>,
}

impl MapHistory {
    /// A history for `data` just inserted: a revision past every one
    /// `clock` has given, the map's fingerprint, nothing journaled.
    pub(crate) fn start(data: &TilemapData, clock: &mut u64) -> Self {
        *clock += 1;
        let fingerprint = fingerprint(data);
        Self {
            revision: *clock,
            fingerprint,
            base: (*clock, fingerprint),
            ..Self::default()
        }
    }

    /// Journals `cell`'s change from `old` to `new`, each a tile and its
    /// auto bit, when they differ: the fingerprint, the revision and `clock`
    /// move on, and an entry past the bound drops the oldest.
    fn journal(
        &mut self,
        clock: &mut u64,
        cell: CellChange,
        old: (TileIndex, bool),
        new: (TileIndex, bool),
    ) {
        if old == new {
            return;
        }
        self.fingerprint ^= cell_mix(cell, old) ^ cell_mix(cell, new);
        self.revision += 1;
        *clock = (*clock).max(self.revision);
        self.journal.push_back(Written {
            cell,
            fingerprint: self.fingerprint,
        });
        if self.journal.len() > JOURNAL_BOUND
            && let Some(dropped) = self.journal.pop_front()
        {
            self.base = (self.base.0 + 1, dropped.fingerprint);
        }
    }
}

/// A tileset's auto-tile styles: each tile's style, each style's brush, and
/// its variants by open-side mask with their weights, in tileset order.
struct Styles {
    /// By tile: its style's index.
    of: Vec<Option<usize>>,
    /// By style: its brush.
    brushes: Vec<Option<TileIndex>>,
    /// By style × 16 + mask: its variants and their weights.
    variants: Vec<Vec<(TileIndex, u32)>>,
}

impl Styles {
    fn of(data: &TilemapData) -> Self {
        let mut names: Vec<&str> = Vec::new();
        let mut styles = Self {
            of: vec![None; data.tileset.len()],
            brushes: Vec::new(),
            variants: Vec::new(),
        };
        for (index, slot) in styles.of.iter_mut().enumerate() {
            let tile = index as TileIndex;
            let props = data.tile_props(tile);
            let Some(name) = props.style.as_deref() else {
                continue;
            };
            let style = names
                .iter()
                .position(|&known| known == name)
                .unwrap_or_else(|| {
                    names.push(name);
                    styles.brushes.push(None);
                    styles.variants.resize(names.len() * 16, Vec::new());
                    names.len() - 1
                });
            *slot = Some(style);
            match props.mask {
                None => {
                    styles.brushes[style].get_or_insert(tile);
                }
                Some(mask) => {
                    styles.variants[style * 16 + usize::from(mask)].push((tile, props.weight));
                }
            }
        }
        styles
    }

    fn is_empty(&self) -> bool {
        self.brushes.is_empty()
    }

    /// The style of `tile`; `None` for an empty or unstyled one.
    fn style(&self, tile: TileIndex) -> Option<usize> {
        usize::try_from(tile)
            .ok()
            .and_then(|tile| self.of.get(tile).copied().flatten())
    }

    fn is_brush(&self, tile: TileIndex) -> bool {
        self.style(tile)
            .is_some_and(|style| self.brushes[style] == Some(tile))
    }
}

impl TilemapData {
    /// Resolves every brush: each cell holding one becomes an auto cell, and
    /// every auto cell takes its style's variant for its open sides, or the
    /// brush when its mask has none. The loader and
    /// [`TilemapRegistry::insert`] call it, so a registered map holds no
    /// unresolved brush.
    pub fn auto_tile(&mut self) {
        let styles = Styles::of(self);
        if styles.is_empty() {
            return;
        }
        let width = (self.width as usize).max(1);
        for layer in 0..self.layers.len() {
            let cells = self.layers[layer].tiles.len();
            for index in 0..cells {
                if styles.is_brush(self.layers[layer].tiles[index]) {
                    self.terrain.set_auto(layer, index, cells, true);
                }
            }
            for index in 0..cells {
                let (col, row) = ((index % width) as u32, (index / width) as u32);
                if self.terrain.is_auto(layer, index)
                    && let Some(tile) = self.pick(&styles, layer, col, row)
                {
                    self.layers[layer].tiles[index] = tile;
                }
            }
        }
    }

    /// Whether the cell at `(col, row)` of `layer` is an auto cell, which
    /// the auto-tiler re-picks when its open sides change; false outside the
    /// map.
    #[must_use]
    pub fn is_auto(&self, layer: usize, col: u32, row: u32) -> bool {
        let index = (row as usize) * (self.width as usize) + (col as usize);
        col < self.width && row < self.height && self.terrain.is_auto(layer, index)
    }

    /// The tile the auto cell at `(col, row)` of `layer` resolves to: its
    /// style's variant for its open sides, drawn by weight from its place,
    /// or the brush; `None` when its tile has no style.
    fn pick(&self, styles: &Styles, layer: usize, col: u32, row: u32) -> Option<TileIndex> {
        let width = self.width as usize;
        let tiles = &self.layers[layer].tiles;
        let tile = tiles[(row as usize) * width + (col as usize)];
        let style = styles.style(tile)?;
        let closed = |col: i64, row: i64| {
            if col < 0 || row < 0 || col >= i64::from(self.width) || row >= i64::from(self.height) {
                return true;
            }
            styles.style(tiles[(row as usize) * width + (col as usize)]) == Some(style)
        };
        let (c, r) = (i64::from(col), i64::from(row));
        let mut mask = 0;
        for (bit, (dc, dr)) in [(1, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))] {
            if !closed(c + dc, r + dr) {
                mask |= bit;
            }
        }
        let variants = &styles.variants[style * 16 + mask];
        let total: u64 = variants.iter().map(|&(_, weight)| u64::from(weight)).sum();
        if total == 0 {
            return Some(styles.brushes[style].unwrap_or(tile));
        }
        let mut roll = place_mix(layer, col, row) % total;
        for &(variant, weight) in variants {
            if roll < u64::from(weight) {
                return Some(variant);
            }
            roll -= u64::from(weight);
        }
        unreachable!("the roll is below the weights' total")
    }
}

/// Re-picks the auto cells beside `(col, row)` of `layer` whose open sides
/// a write there changed, journaling each that changed.
fn retile_neighbours(
    map: &mut RegisteredMap,
    clock: &mut u64,
    styles: &Styles,
    layer: usize,
    col: u32,
    row: u32,
) {
    let RegisteredMap { data, history } = map;
    let (c, r) = (i64::from(col), i64::from(row));
    for (dc, dr) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let (Ok(col), Ok(row)) = (u32::try_from(c + dc), u32::try_from(r + dr)) else {
            continue;
        };
        if !data.is_auto(layer, col, row) {
            continue;
        }
        let index = (row as usize) * (data.width as usize) + (col as usize);
        let old = data.layers[layer].tiles[index];
        let Some(new) = data.pick(styles, layer, col, row) else {
            continue;
        };
        data.layers[layer].tiles[index] = new;
        history.journal(
            clock,
            CellChange { layer, col, row },
            (old, true),
            (new, true),
        );
    }
}

impl TilemapRegistry {
    /// Writes `tile` at `(col, row)` of map `id`'s `layer`, as
    /// [`TilemapData::set_tile`] does, and runs the auto-tiler: a brush makes
    /// the cell an auto cell and resolves it, any other tile makes it an
    /// override, and the auto cells beside it whose open sides changed are
    /// re-picked. Each cell whose tile changed is journaled; the written
    /// cell's damage is cleared and it joins
    /// [`edited_cells`](Self::edited_cells) with `tile` as written. False,
    /// with nothing written, for a map the registry lacks.
    ///
    /// # Panics
    ///
    /// As [`TilemapData::set_tile`]: when `layer`, `col` or `row` is out of
    /// range, or `tile` is past the tileset.
    #[track_caller]
    pub fn set_tile(
        &mut self,
        id: &str,
        layer: usize,
        col: u32,
        row: u32,
        tile: TileIndex,
    ) -> bool {
        let Some(map) = self.maps.get_mut(id) else {
            return false;
        };
        let was_auto = map.data.is_auto(layer, col, row);
        let old = map.data.tile_at(layer, col, row);
        map.data.set_tile(layer, col, row, tile);
        let (Some(old), Some(written)) = (old, map.data.tile_at(layer, col, row)) else {
            unreachable!("TilemapData::set_tile checked the cell");
        };
        let styles = Styles::of(&map.data);
        let mut new = (written, false);
        if styles.is_brush(written) {
            let index = (row as usize) * (map.data.width as usize) + (col as usize);
            let cells = map.data.layers[layer].tiles.len();
            map.data.terrain.set_auto(layer, index, cells, true);
            let resolved = map.data.pick(&styles, layer, col, row).unwrap_or(written);
            map.data.layers[layer].tiles[index] = resolved;
            new = (resolved, true);
        }
        let key = (layer, row, col);
        map.history.damage.remove(&key);
        map.history.edits.insert(key, written);
        let cell = CellChange { layer, col, row };
        map.history
            .journal(&mut self.clock, cell, (old, was_auto), new);
        if !styles.is_empty() {
            retile_neighbours(map, &mut self.clock, &styles, layer, col, row);
        }
        true
    }

    /// Damages the cell at `(col, row)` of map `id`'s `layer` by `amount`:
    /// a cell of a tile with `health` keeps what is left, sparsely, and
    /// breaks at 0, set to its tile's `broken` (or cleared) through
    /// [`set_tile`](Self::set_tile). A non-finite amount or one of 0 or less
    /// changes nothing.
    ///
    /// # Panics
    ///
    /// When `layer`, `col` or `row` is out of range for a map the registry
    /// holds.
    #[track_caller]
    pub fn damage_tile(
        &mut self,
        id: &str,
        layer: usize,
        col: u32,
        row: u32,
        amount: f32,
    ) -> TileDamage {
        let Some(map) = self.maps.get(id) else {
            return TileDamage::Unbreakable;
        };
        let tile = checked_cell("damage_tile", &map.data, layer, col, row);
        let props = map.data.tile_props(tile);
        let (Some(full), true) = (props.health, tile >= 0) else {
            return TileDamage::Unbreakable;
        };
        let broken = props.broken;
        let key = (layer, row, col);
        let health = map.history.damage.get(&key).copied().unwrap_or(full);
        if !(amount.is_finite() && amount > 0.0) {
            return TileDamage::Damaged { health };
        }
        let left = health - amount;
        if left > 0.0 {
            if let Some(map) = self.maps.get_mut(id) {
                map.history.damage.insert(key, left);
            }
            return TileDamage::Damaged { health: left };
        }
        self.set_tile(id, layer, col, row, broken);
        TileDamage::Broken { tile: broken }
    }

    /// The health of the cell at `(col, row)` of map `id`'s `layer`: its
    /// tile's `health` until damaged, what is left after. `None` for a map
    /// the registry lacks, a cell outside it, an empty cell or a tile
    /// without `health`.
    #[must_use]
    pub fn health(&self, id: &str, layer: usize, col: u32, row: u32) -> Option<f32> {
        let map = self.maps.get(id)?;
        let tile = map.data.tile_at(layer, col, row)?;
        let full = map.data.tile_props(tile).health.filter(|_| tile >= 0)?;
        Some(
            map.history
                .damage
                .get(&(layer, row, col))
                .copied()
                .unwrap_or(full),
        )
    }

    /// Where map `id`'s history stands, for a later
    /// [`changes_since`](Self::changes_since); `None` for a map the registry
    /// lacks.
    #[must_use]
    pub fn cursor(&self, id: &str) -> Option<MapCursor> {
        let history = &self.maps.get(id)?.history;
        Some(MapCursor {
            revision: history.revision,
            fingerprint: history.fingerprint,
        })
    }

    /// What changed in map `id` since `cursor`: nothing, the cells written
    /// since, or a replaced map to read whole (past the journal's 4,096
    /// entries a map, too). `None` for a map the registry lacks.
    #[must_use]
    pub fn changes_since(&self, id: &str, cursor: &MapCursor) -> Option<TileChanges<'_>> {
        let history = &self.maps.get(id)?.history;
        if cursor.fingerprint == history.fingerprint {
            return Some(TileChanges::Unchanged);
        }
        let (base, base_fingerprint) = history.base;
        if cursor.revision < base || cursor.revision > history.revision {
            return Some(TileChanges::Replaced);
        }
        let seen = (cursor.revision - base) as usize;
        let then = match seen {
            0 => base_fingerprint,
            _ => history.journal[seen - 1].fingerprint,
        };
        if then != cursor.fingerprint {
            return Some(TileChanges::Replaced);
        }
        Some(TileChanges::Cells(ChangedCells {
            entries: history.journal.range(seen..),
        }))
    }

    /// Each cell of map `id` the game wrote since its `insert`, by
    /// [`set_tile`](Self::set_tile) or a break, with the tile it last wrote
    /// there, in (layer, row, col) order; none for a map the registry lacks.
    /// Writing them again with `set_tile` on the map as inserted, and
    /// damaging [`damaged_cells`](Self::damaged_cells) by what they lost,
    /// rebuilds its terrain.
    pub fn edited_cells(&self, id: &str) -> impl Iterator<Item = (CellChange, TileIndex)> + '_ {
        self.maps.get(id).into_iter().flat_map(|map| {
            map.history
                .edits
                .iter()
                .map(|(&(layer, row, col), &tile)| (CellChange { layer, col, row }, tile))
        })
    }

    /// Each damaged cell of map `id` with the health it has left, in
    /// (layer, row, col) order; none for a map the registry lacks.
    pub fn damaged_cells(&self, id: &str) -> impl Iterator<Item = (CellChange, f32)> + '_ {
        self.maps.get(id).into_iter().flat_map(|map| {
            map.history
                .damage
                .iter()
                .map(|(&(layer, row, col), &health)| (CellChange { layer, col, row }, health))
        })
    }
}

/// The tile at `(col, row)` of `layer`, or a panic naming `method` when the
/// cell is outside the map.
#[track_caller]
fn checked_cell(method: &str, data: &TilemapData, layer: usize, col: u32, row: u32) -> TileIndex {
    assert!(
        layer < data.layers.len(),
        "TilemapRegistry::{method}: layer {layer} is past the map's {} layers",
        data.layers.len()
    );
    assert!(
        col < data.width && row < data.height,
        "TilemapRegistry::{method}: cell ({col}, {row}) is outside the {}x{} map",
        data.width,
        data.height
    );
    data.tile_at(layer, col, row).unwrap_or(EMPTY_TILE)
}

/// SplitMix64's finalizer: a bijective 64-bit mix.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A running mix of values, in order.
struct Fold(u64);

impl Fold {
    fn add(&mut self, value: u64) {
        self.0 = mix(self.0.rotate_left(23) ^ value).wrapping_add(0x9E37_79B9_7F4A_7C15);
    }

    fn add_str(&mut self, text: &str) {
        self.add(text.len() as u64);
        for chunk in text.as_bytes().chunks(8) {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn add_option_str(&mut self, text: Option<&str>) {
        match text {
            Some(text) => {
                self.add(1);
                self.add_str(text);
            }
            None => self.add(0),
        }
    }
}

/// A mix of a cell's place: its layer, column and row.
fn place_mix(layer: usize, col: u32, row: u32) -> u64 {
    mix(
        mix(mix(layer as u64 ^ 0xA076_1D64_78BD_642F) ^ u64::from(col))
            ^ u64::from(row).wrapping_mul(0xE703_7ED1_A0B4_28DB),
    )
}

/// A non-empty cell's share of the fingerprint, by its tile and auto bit; 0
/// for an empty one.
fn cell_mix(cell: CellChange, (tile, auto): (TileIndex, bool)) -> u64 {
    if tile < 0 {
        return 0;
    }
    let place = place_mix(cell.layer, cell.col, cell.row);
    mix(place ^ (tile as u64).wrapping_mul(0x8EBC_6AF0_9C88_C6E3) ^ u64::from(auto) << 63)
}

/// The fingerprint of `data` as a history starts it: the mix of its sizes,
/// layers and tileset XORed with each non-empty cell's.
fn fingerprint(data: &TilemapData) -> u64 {
    let mut fold = Fold(0x5851_F42D_4C95_7F2D);
    for size in [data.tile_width, data.tile_height, data.width, data.height] {
        fold.add(u64::from(size));
    }
    fold.add(data.layers.len() as u64);
    for (index, layer) in data.layers.iter().enumerate() {
        fold.add_str(&layer.name);
        fold.add(u64::from(layer.kind == LayerKind::Collision));
        let physics = data.layer_physics(index);
        fold.add(u64::from(physics.layers) << 32 | u64::from(physics.mask));
    }
    fold.add(data.tileset.len() as u64);
    for (index, sprite) in data.tileset.iter().enumerate() {
        fold.add_str(sprite);
        let props = data.tile_props(index as TileIndex);
        fold.add_option_str(props.material.as_deref());
        fold.add(
            props
                .health
                .map_or(u64::MAX, |health| u64::from(health.to_bits())),
        );
        fold.add(props.broken as u64);
        fold.add(u64::from(props.one_way));
        fold.add_option_str(props.style.as_deref());
        fold.add(props.mask.map_or(u64::MAX, u64::from));
        fold.add(u64::from(props.weight));
    }
    let width = (data.width as usize).max(1);
    let mut cells = 0;
    for (layer, tiles) in data.layers.iter().enumerate() {
        for (index, &tile) in tiles.tiles.iter().enumerate() {
            let cell = CellChange {
                layer,
                col: (index % width) as u32,
                row: (index / width) as u32,
            };
            cells ^= cell_mix(cell, (tile, data.terrain.is_auto(layer, index)));
        }
    }
    fold.0 ^ cells
}

#[cfg(test)]
#[path = "../tests/assets/terrain.rs"]
mod tests;
