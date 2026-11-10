//! `tungsten level map` (editor-pass): a room as text, a character a cell,
//! so its terrain and where its objects and places sit can be read without
//! a render.
//!
//! The room is compiled and auto-tiled as the game loads it. A cell shows
//! what collides there first: `#` a solid tile, `=` a one-way tile, `/` or
//! `\` a slope by the way its surface runs; then `+` for a tile that is
//! only drawn and `.` for an empty cell. An object's letter and a place's
//! digit go over the terrain at the cell their position falls in, an
//! area's at its top-left cell and over each empty cell it covers, so it
//! reads as an area, and a route's at its first point. The legend under
//! the map gives the ID and geometry of each entry the window holds; the
//! entries past the symbols, which all show `*`, are counted in one line.
//! A room larger than [`MAX_COLS`] by [`MAX_ROWS`] prints through `--rect`.

use std::fmt::Write as _;

use anyhow::bail;
use tungsten_core::assets::{Room, RoomLayerKind, RoomPlace, RouteStop};

use super::Game;

/// The widest and the tallest window `map` prints.
pub(crate) const MAX_COLS: u32 = 200;
pub(crate) const MAX_ROWS: u32 = 100;

/// The symbols objects take in order, then `*` for the rest.
const OBJECT_SYMBOLS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The symbols places take in order, then `*` for the rest.
const PLACE_SYMBOLS: &[u8] = b"123456789";

/// Room `arg` as text: the window `rect` (`[col, row, width, height]`) or
/// the whole room.
///
/// # Errors
///
/// A room that does not read or compile; a `rect` that starts past the
/// room; a window, or with no `rect` a room, past the bounds.
pub(crate) fn map(game: &Game, arg: &str, rect: Option<[u32; 4]>) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let (_, room) = game.room(&target)?;
    let [cols, rows] = room.size;
    let [x, y, w, h] = match rect {
        Some([x, y, w, h]) => {
            if x >= cols || y >= rows || w == 0 || h == 0 {
                bail!(
                    "`--rect {x},{y},{w},{h}` holds no cell of room `{}`'s {cols} \u{d7} {rows} cells",
                    target.id
                );
            }
            [x, y, w.min(cols - x), h.min(rows - y)]
        }
        None => [0, 0, cols, rows],
    };
    if w > MAX_COLS || h > MAX_ROWS {
        bail!(
            "room `{}` is {cols} \u{d7} {rows} cells and `map` prints at most {MAX_COLS} \u{d7} {MAX_ROWS}: give `--rect <col>,<row>,<width>,<height>`, as `--rect {x},{y},{},{}`",
            target.id,
            w.min(MAX_COLS),
            h.min(MAX_ROWS)
        );
    }
    let tileset = game.tileset(&room)?;
    let layers = game.layers();
    let mut compiled = room
        .compile(&target.id, &tileset, &layers)
        .map_err(|e| anyhow::anyhow!("{}: {e}", game.relative(&target.path)))?
        .map;
    compiled.auto_tile();

    // A cell's terrain: what collides first, then what is only drawn.
    let terrain = |col: u32, row: u32| -> u8 {
        let mut drawn = false;
        for (index, layer) in room.layers.iter().enumerate() {
            let Some(tile) = compiled.tile_at(index, col, row).filter(|tile| *tile >= 0) else {
                continue;
            };
            if layer.kind != RoomLayerKind::Collision {
                drawn = true;
                continue;
            }
            let sprite = usize::try_from(tile)
                .ok()
                .and_then(|tile| compiled.tileset.get(tile));
            let props = sprite
                .and_then(|sprite| tileset.tiles.iter().find(|known| known.sprite == *sprite));
            return match props {
                Some(props) if props.slope.is_some() => {
                    // The corner its solid part holds: a floor rising to the
                    // right or a ceiling falling to it reads `/`.
                    let slope = props.slope.as_deref().unwrap_or_default();
                    if slope.ends_with("br") || slope.ends_with("tl") {
                        b'/'
                    } else {
                        b'\\'
                    }
                }
                Some(props) if props.one_way => b'=',
                _ => b'#',
            };
        }
        if drawn { b'+' } else { b'.' }
    };
    let mut grid: Vec<Vec<u8>> = (y..y + h)
        .map(|row| (x..x + w).map(|col| terrain(col, row)).collect())
        .collect();
    // The window's cell `at` falls in, when it holds it.
    let cell = |at: [f32; 2]| -> Option<(usize, usize)> {
        let (col, row) = (at[0].floor(), at[1].floor());
        (col >= x as f32 && row >= y as f32 && col < (x + w) as f32 && row < (y + h) as f32)
            .then(|| (row as usize - y as usize, col as usize - x as usize))
    };
    // Symbols go to the entries the window shows, in the room's order, so
    // a part of a large room has letters of its own.
    let symbol = |symbols: &[u8], shown: usize| symbols.get(shown).copied().unwrap_or(b'*');
    // The legend names what the window shows; the entries past the
    // symbols all read `*` and are counted.
    let legend = |list: &mut Vec<String>, more: &mut usize, symbol: u8, line: String| {
        if symbol == b'*' {
            *more += 1;
        } else {
            list.push(format!("{} {line}", symbol as char));
        }
    };
    let (mut places, mut more_places) = (Vec::new(), 0);
    for place in &room.places {
        let symbol = symbol(PLACE_SYMBOLS, places.len() + more_places);
        let (at, shape) = place_shape(&room, place);
        let mut shown = false;
        // An area over each empty cell it covers, so its extent reads.
        if let Some([px, py, pw, ph]) = place.rect {
            for (row, cells) in grid.iter_mut().enumerate() {
                for (col, glyph) in cells.iter_mut().enumerate() {
                    let (cx, cy) = ((x as usize + col) as f32, (y as usize + row) as f32);
                    let inside = cx + 1.0 > px && cx < px + pw && cy + 1.0 > py && cy < py + ph;
                    shown |= inside;
                    if inside && *glyph == b'.' {
                        *glyph = symbol;
                    }
                }
            }
        }
        if let Some((row, col)) = at.and_then(cell) {
            grid[row][col] = symbol;
            shown = true;
        }
        if shown {
            let link = (place.to.as_ref()).map_or_else(String::new, |to| format!(" to {to}"));
            let line = format!("{} {shape}{link}", place.id);
            legend(&mut places, &mut more_places, symbol, line);
        }
    }
    let (mut objects, mut more_objects) = (Vec::new(), 0);
    for object in &room.objects {
        let symbol = symbol(OBJECT_SYMBOLS, objects.len() + more_objects);
        let Some((row, col)) = cell(object.at) else {
            continue;
        };
        grid[row][col] = symbol;
        let line = format!(
            "{} ({}) at {}, {}",
            object.id, object.prefab, object.at[0], object.at[1]
        );
        legend(&mut objects, &mut more_objects, symbol, line);
    }

    let mut out = String::new();
    let window = if rect.is_some() {
        format!(", cells {x},{y} to {},{}", x + w - 1, y + h - 1)
    } else {
        String::new()
    };
    let _ = writeln!(
        out,
        "{}: {cols} \u{d7} {rows} cells of {} \u{d7} {} px{window}",
        target.id, room.cell[0], room.cell[1]
    );
    let _ = writeln!(
        out,
        "# solid  = one-way  / \\ slope  + drawn only  . empty  a-z objects  1-9 places, an area over its empty cells"
    );
    // The column ruler: the tens over the ones.
    let gutter = (y + h - 1).to_string().len();
    let digit = |col: u32, tens: bool| -> char {
        let value = if tens { col / 10 % 10 } else { col % 10 };
        if tens && !col.is_multiple_of(10) {
            ' '
        } else {
            char::from_digit(value, 10).unwrap_or(' ')
        }
    };
    for tens in [true, false] {
        let ruler: String = (x..x + w).map(|col| digit(col, tens)).collect();
        let _ = writeln!(out, "{:gutter$} {}", "", ruler.trim_end());
    }
    for (row, cells) in (y..y + h).zip(&grid) {
        let _ = writeln!(out, "{row:>gutter$} {}", String::from_utf8_lossy(cells));
    }
    for (what, list, more) in [
        ("objects", objects, more_objects),
        ("places", places, more_places),
    ] {
        if list.is_empty() && more == 0 {
            continue;
        }
        let _ = writeln!(out, "{what}:");
        for line in list {
            let _ = writeln!(out, "  {line}");
        }
        if more > 0 {
            let _ = writeln!(
                out,
                "  * {more} more: `tungsten level query {}` lists them, and `--rect` maps a part",
                target.id
            );
        }
    }
    Ok(out)
}

/// A place's cell on the map and its geometry in words: a point, an
/// area by its top-left, a route by its first point.
fn place_shape(room: &Room, place: &RoomPlace) -> (Option<[f32; 2]>, String) {
    if let Some(point) = place.point {
        return (Some(point), format!("point at {}, {}", point[0], point[1]));
    }
    if let Some([x, y, w, h]) = place.rect {
        return (Some([x, y]), format!("area at {x}, {y}, {w} by {h}"));
    }
    let stops = place.route.as_deref().unwrap_or_default();
    // A stop that names a point place sits where that place does.
    let point_of = |stop: &RouteStop| match stop {
        RouteStop::Point(point) => Some(*point),
        RouteStop::Place(id) => room
            .places
            .iter()
            .find(|known| known.id == *id)
            .and_then(|known| known.point),
        _ => None,
    };
    let by = (place.by.as_ref()).map_or_else(String::new, |by| format!(" by {by}"));
    (
        stops.first().and_then(point_of),
        format!("route of {} stops{by}", stops.len()),
    )
}
