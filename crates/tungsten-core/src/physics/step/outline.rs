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
//! A slope cell's mask holds each side whose leg the neighbour across does
//! not cover whole, a full leg by a full side or a leg, a half leg by an
//! equal half leg, and each side without a leg whose vertex farthest across
//! it no neighbour, a diagonal one too, is solid just past on either hand:
//! a box against that vertex would stand in the neighbour, as one sliding
//! down a ramp of slope cells would meet each lower cell's top corner. Its
//! sloped face is always exposed, so it always has a run, of its own. A
//! full cell's face is covered by a full neighbour, or by a slope
//! neighbour's full leg, so a map without slopes masks as before.
//!
//! Masks and runs read the chunk's cells and a one-cell apron around it, so
//! a written cell changes the masks and runs of its chunk, of the chunks
//! holding its four neighbours and of those holding a diagonal neighbour
//! that is a slope.

use super::slope;
use crate::assets::TileIndex;
use crate::physics::collision::{FACE_BOTTOM, FACE_LEFT, FACE_ONE_WAY, FACE_RIGHT, FACE_TOP};

/// A chunk's width and height in cells.
pub(super) const CHUNK: u32 = 16;

/// What a tile breaks runs on: its material, interned, its one-way flag
/// and its slope's code, 0 for a full cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct TileClass {
    pub(super) material: u32,
    pub(super) one_way: bool,
    pub(super) slope: u8,
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

    /// The span, in the cell's local units, that `(col, row)` covers of its
    /// side `side`: a full tile the whole side, a slope tile its leg there,
    /// an empty or one-way cell nothing.
    fn cover(&self, col: i64, row: i64, side: u8) -> Option<(f32, f32)> {
        let class = self.class(self.tile(col, row)?);
        if class.one_way {
            return None;
        }
        if class.slope == 0 {
            return Some((0.0, 1.0));
        }
        slope::legs(class.slope)
            .into_iter()
            .find(|&(face, ..)| face == side)
            .map(|(_, low, high)| (low, high))
    }

    /// Whether the neighbour of `(col, row)` across its side `side`, of the
    /// opposite side `across`, covers the span `need`.
    fn covered(&self, col: i64, row: i64, side: u8, across: u8, need: (f32, f32)) -> bool {
        let (dc, dr) = match side {
            FACE_TOP => (0, -1),
            FACE_BOTTOM => (0, 1),
            FACE_LEFT => (-1, 0),
            _ => (1, 0),
        };
        self.cover(col + dc, row + dr, across)
            .is_some_and(|(low, high)| low <= need.0 && need.1 <= high)
    }

    /// Whether slope `code`'s cell at `(col, row)` has its vertex across
    /// `side`, a side without a leg, shadowed: a neighbour, a diagonal one
    /// too, solid just past it on either hand, so a box against it there
    /// would stand in that neighbour. Each probe leaves the vertex between
    /// the bounds a cell's polygon can have, the axes and the 45° and 2:1
    /// faces' angles.
    fn shadowed(&self, col: i64, row: i64, code: u8, side: u8) -> bool {
        const DEGREES: [f32; 4] = [13.0, 36.0, 54.0, 77.0];
        let (u, v) = slope::apex(code, side);
        let out = match side {
            FACE_TOP => (0.0, -1.0),
            FACE_BOTTOM => (0.0, 1.0),
            FACE_LEFT => (-1.0, 0.0),
            _ => (1.0, 0.0),
        };
        for hand in [-1.0, 1.0] {
            for degrees in DEGREES {
                let (sin, cos) = f32::to_radians(degrees).sin_cos();
                let pu = u + 1e-3 * (out.0 * cos + out.1 * hand * sin);
                let pv = v + 1e-3 * (out.1 * cos + out.0 * hand * sin);
                let (dc, dr) = (pu.floor(), pv.floor());
                // Its own cell is empty past its polygon's vertex.
                if (dc, dr) == (0.0, 0.0) {
                    continue;
                }
                let Some(tile) = self.tile(col + dc as i64, row + dr as i64) else {
                    continue;
                };
                let class = self.class(tile);
                if !class.one_way
                    && (class.slope == 0 || slope::holds(class.slope, pu - dc, pv - dr))
                {
                    return true;
                }
            }
        }
        false
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
        let sides = [
            (FACE_TOP, FACE_BOTTOM),
            (FACE_BOTTOM, FACE_TOP),
            (FACE_LEFT, FACE_RIGHT),
            (FACE_RIGHT, FACE_LEFT),
        ];
        let slope = self.class(tile).slope;
        let mut mask = 0;
        for (side, across) in sides {
            let leg = if slope == 0 {
                Some((0.0, 1.0))
            } else {
                slope::legs(slope)
                    .into_iter()
                    .find(|&(face, ..)| face == side)
                    .map(|(_, low, high)| (low, high))
            };
            let closed = match leg {
                Some(need) => self.covered(c, r, side, across, need),
                None => self.shadowed(c, r, slope, side),
            };
            if !closed {
                mask |= side;
            }
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
            let class = cells.class_at(col, row);
            // A slope cell's face is always exposed: a run of its own.
            if taken[local(col, row)]
                || (mask & (sides | ends) == 0 && class.slope == 0)
                || cells.tile(i64::from(col), i64::from(row)).is_none()
            {
                continue;
            }
            taken[local(col, row)] = true;
            let joins = |taken: &[bool], col: u32, row: u32| {
                !taken[local(col, row)]
                    && masks[at(col, row)] == mask
                    && cells.class_at(col, row) == class
            };
            let (mut span_cols, mut span_rows) = (1, 1);
            if class.slope != 0 {
                // Alone.
            } else if mask & sides == 0 {
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
