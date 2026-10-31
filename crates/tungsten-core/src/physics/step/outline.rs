//! The tile outline: each collision layer's cells traced per 16 × 16 chunk
//! into their exposed-face masks, which the step's tile proxies keep, and
//! the runs the spatial index casts against.
//!
//! A cell's face is exposed when its neighbour is off the map, empty or
//! one-way. A cell whose exposed faces are all top or bottom joins its row
//! neighbours of the same mask into one run, and one whose exposed faces are
//! all left or right joins its column neighbours of the same mask; each run
//! breaks at the chunk's border and where its cells' material or one-way
//! flag changes. A cell exposed on both axes keeps a run of its own, and an
//! interior cell has none. A run is the union of its cells and keeps their
//! mask, so every masked face is backed by a solid cell, and a cast tested
//! against a run's cells one by one gets the hits they give.
//!
//! A one-way cell is solid through its top face alone: its neighbours count
//! it as empty, and its own mask holds its top face when the cell above is
//! empty or off the map, plus each side where the next cell is not one-way,
//! with [`FACE_ONE_WAY`]; one whose top is covered has mask 0 and no run.
//!
//! Masks and runs read the chunk's cells and a one-cell apron around it, so
//! a written cell changes the masks and runs of its chunk and of the chunks
//! holding its four neighbours alone.

use crate::assets::TileIndex;
use crate::physics::collision::{FACE_BOTTOM, FACE_LEFT, FACE_ONE_WAY, FACE_RIGHT, FACE_TOP};

/// A chunk's width and height in cells.
pub(super) const CHUNK: u32 = 16;

/// What a tile breaks runs on: its material, interned, and its one-way flag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct TileClass {
    pub(super) material: u32,
    pub(super) one_way: bool,
}

/// One collision layer's cells as the tracer reads them.
#[derive(Debug, Clone, Copy)]
pub(super) struct LayerCells<'a> {
    pub(super) width: u32,
    pub(super) height: u32,
    /// Row-major; a negative tile is empty.
    pub(super) tiles: &'a [TileIndex],
    /// By tileset entry; a tile past it is a plain one.
    pub(super) classes: &'a [TileClass],
}

/// One traced run: its first cell, its size in cells and its cells' mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Run {
    pub(super) col: u32,
    pub(super) row: u32,
    pub(super) cols: u32,
    pub(super) rows: u32,
    /// The exposed faces, with [`FACE_ONE_WAY`] for a one-way run.
    pub(super) mask: u8,
}

impl LayerCells<'_> {
    /// The tile at `(col, row)`; `None` off the map or for an empty cell.
    fn tile(&self, col: i64, row: i64) -> Option<TileIndex> {
        if col < 0 || row < 0 || col >= i64::from(self.width) || row >= i64::from(self.height) {
            return None;
        }
        let tile = self.tiles[(row as usize) * (self.width as usize) + (col as usize)];
        (tile >= 0).then_some(tile)
    }

    fn class(&self, tile: TileIndex) -> TileClass {
        self.classes.get(tile as usize).copied().unwrap_or_default()
    }

    /// Whether `(col, row)` holds a one-way tile.
    fn one_way(&self, col: i64, row: i64) -> bool {
        self.tile(col, row)
            .is_some_and(|tile| self.class(tile).one_way)
    }

    /// Whether `(col, row)` closes a solid neighbour's face: a solid tile
    /// that is not one-way.
    fn closes(&self, col: i64, row: i64) -> bool {
        self.tile(col, row)
            .is_some_and(|tile| !self.class(tile).one_way)
    }

    /// The exposed-face mask of the cell at `(col, row)`: 0 for an empty
    /// cell, an interior one, or a one-way one whose top is covered.
    pub(super) fn mask(&self, col: u32, row: u32) -> u8 {
        let (c, r) = (i64::from(col), i64::from(row));
        let Some(tile) = self.tile(c, r) else {
            return 0;
        };
        if self.class(tile).one_way {
            if self.tile(c, r - 1).is_some() {
                return 0;
            }
            let mut mask = FACE_TOP | FACE_ONE_WAY;
            if !self.one_way(c - 1, r) {
                mask |= FACE_LEFT;
            }
            if !self.one_way(c + 1, r) {
                mask |= FACE_RIGHT;
            }
            return mask;
        }
        let mut mask = 0;
        if !self.closes(c, r - 1) {
            mask |= FACE_TOP;
        }
        if !self.closes(c, r + 1) {
            mask |= FACE_BOTTOM;
        }
        if !self.closes(c - 1, r) {
            mask |= FACE_LEFT;
        }
        if !self.closes(c + 1, r) {
            mask |= FACE_RIGHT;
        }
        mask
    }

    /// The class of the solid cell at `(col, row)`.
    fn class_at(&self, col: u32, row: u32) -> TileClass {
        self.tile(i64::from(col), i64::from(row))
            .map_or(TileClass::default(), |tile| self.class(tile))
    }

    /// The chunks across and down.
    #[cfg(test)]
    pub(super) fn chunks(&self) -> (u32, u32) {
        (self.width.div_ceil(CHUNK), self.height.div_ceil(CHUNK))
    }
}

/// Traces chunk `(cx, cy)` of `cells`: its cells' masks into `masks`, the
/// layer's, row-major, and its runs into `out`, cleared first, in their
/// first cell's row-major order.
pub(super) fn trace_chunk(
    cells: &LayerCells<'_>,
    cx: u32,
    cy: u32,
    masks: &mut [u8],
    out: &mut Vec<Run>,
) {
    out.clear();
    let cols = cx * CHUNK..((cx + 1) * CHUNK).min(cells.width);
    let rows = cy * CHUNK..((cy + 1) * CHUNK).min(cells.height);
    let width = cells.width as usize;
    let at = |col: u32, row: u32| (row as usize) * width + col as usize;
    for row in rows.clone() {
        for col in cols.clone() {
            masks[at(col, row)] = cells.mask(col, row);
        }
    }
    let local = |col: u32, row: u32| ((row - rows.start) * CHUNK + (col - cols.start)) as usize;
    let mut taken = [false; (CHUNK * CHUNK) as usize];
    let sides = FACE_LEFT | FACE_RIGHT;
    let ends = FACE_TOP | FACE_BOTTOM;
    for row in rows.clone() {
        for col in cols.clone() {
            let mask = masks[at(col, row)];
            if taken[local(col, row)] || mask & (sides | ends) == 0 {
                continue;
            }
            taken[local(col, row)] = true;
            let class = cells.class_at(col, row);
            let joins = |taken: &[bool], col: u32, row: u32| {
                !taken[local(col, row)]
                    && masks[at(col, row)] == mask
                    && cells.class_at(col, row) == class
            };
            let (mut span_cols, mut span_rows) = (1, 1);
            if mask & sides == 0 {
                // Exposed top or bottom only: a run along the row.
                while col + span_cols < cols.end && joins(&taken, col + span_cols, row) {
                    taken[local(col + span_cols, row)] = true;
                    span_cols += 1;
                }
            } else if mask & ends == 0 {
                // Exposed left or right only: a run down the column.
                while row + span_rows < rows.end && joins(&taken, col, row + span_rows) {
                    taken[local(col, row + span_rows)] = true;
                    span_rows += 1;
                }
            }
            out.push(Run {
                col,
                row,
                cols: span_cols,
                rows: span_rows,
                mask,
            });
        }
    }
}

#[cfg(test)]
#[path = "../../tests/physics/outline.rs"]
mod tests;
