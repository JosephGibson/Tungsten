//! A room compiled against its tileset and the physics layer names (redo
//! 7.12, §2.3 of its plan): pure and deterministic.
//!
//! 1. A map of the room's cell and size, the tileset's tiles in file order
//!    with their properties, and the layers with their physics and draws.
//! 2. Each shape in list order paints each layer it names, a later shape
//!    over an earlier one: a brush makes auto cells, any other tile an
//!    override, `null` empty. A `rect` fills its cells, a `line` the cells
//!    its segments pass. A `polygon` fills each cell wholly inside it; a
//!    cell a sloped edge crosses takes, for a style's brush, the style's
//!    slope tile for that edge's rise and corner (a 45° edge one `45` cell a
//!    row, a 2:1 edge a `2:1 low` and a `2:1 high` cell a row), and for any
//!    other tile the tile when the cell's centre is inside or on the edge.
//! 3. Each layer's `cells` last.
//! 4. One object layer, `objects`: each object a classed map object named
//!    `<room>.<id>` at `at × cell`, its `set` as its properties, so the map's
//!    object check and spawn serve rooms as they do Tiled maps.
//! 5. The places in pixels from the room's origin, each route's stops
//!    resolved to points, and the settings.

use std::collections::BTreeMap;

use glam::{Vec2, Vec3};

use super::{Room, RoomError, RoomLayerKind, RoomShape, RouteStop, Tileset, check_name, hint};
use crate::assets::tilemap::{
    EMPTY_TILE, LayerDraw, LayerKind, LayerPhysics, MapObject, ObjectLayer, Slope, SlopeCorner,
    SlopeRise, TileIndex, TilemapData,
};
use crate::physics::PhysicsLayers;
use crate::post::PostPass;

/// The object layer a compiled room's objects are in.
pub const ROOM_OBJECT_LAYER: &str = "objects";

/// A room compiled: its map, its places and its settings.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CompiledRoom {
    /// The map the `TilemapRegistry` holds under the room's ID, brushes
    /// not yet resolved (its `insert` auto-tiles them), its objects in the
    /// object layer [`ROOM_OBJECT_LAYER`].
    pub map: TilemapData,
    /// The places by ID, in pixels from the room's origin.
    pub places: BTreeMap<String, CompiledPlace>,
    /// The post stack the room's spawn sets, when it gives one.
    pub post: Option<Vec<PostPass>>,
    /// The ambient light the room's spawn sets, when it gives one.
    pub ambient: Option<Vec3>,
    /// The most objects with a light.
    pub lights: u32,
    /// The most objects.
    pub objects: u32,
}

impl CompiledRoom {
    /// The place `id`.
    #[must_use]
    pub fn place(&self, id: &str) -> Option<&CompiledPlace> {
        self.places.get(id)
    }
}

/// A compiled place: its shape in pixels, its link and its route's mover.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CompiledPlace {
    /// Where it is.
    pub shape: PlaceShape,
    /// A point's or an area's link, `<room>.<place>`.
    pub to: Option<String>,
    /// A route's mover, a prefab ID.
    pub by: Option<String>,
}

/// A compiled place's shape, in pixels.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PlaceShape {
    /// A point.
    Point(Vec2),
    /// An area: its top-left and size.
    Rect {
        /// Its top-left.
        min: Vec2,
        /// Its size.
        size: Vec2,
    },
    /// A route's stops, in order.
    Route(Vec<Vec2>),
}

impl PlaceShape {
    /// The point, or an area's top-left, or a route's first stop.
    #[must_use]
    pub fn anchor(&self) -> Vec2 {
        match self {
            Self::Point(point) => *point,
            Self::Rect { min, .. } => *min,
            Self::Route(stops) => stops.first().copied().unwrap_or(Vec2::ZERO),
        }
    }
}

impl Room {
    /// Compiles the room as `id` against `tileset` and `layers`, the
    /// physics layer names its collision layers name.
    ///
    /// # Errors
    ///
    /// A [`RoomError`] naming the field: an ID that is not
    /// `[a-z][a-z0-9_]*`, a room or tileset that fails its checks, a tile
    /// the tileset lacks, an unknown physics layer, a shape past the room,
    /// a polygon edge at another angle than 0°, 90°, 45° or 2:1, a polygon
    /// that crosses itself, a cell two sloped edges cross, or a style that
    /// lacks the slope tile a cell needs, each naming the shape and the
    /// cell.
    pub fn compile(
        &self,
        id: &str,
        tileset: &Tileset,
        layers: &PhysicsLayers,
    ) -> Result<CompiledRoom, RoomError> {
        check_name(id).map_err(|message| RoomError::new("", format!("room ID {message}")))?;
        self.validate()?;
        let props = tileset
            .props()
            .map_err(|err| RoomError::new(format!("tileset.{}", err.field), err.message))?;
        let [cell_w, cell_h] = self.cell;
        let [cols, rows] = self.size;
        let mut map = TilemapData::new(cell_w, cell_h, cols, rows);
        for tile in &tileset.tiles {
            map.add_tile(tile.sprite.clone());
        }
        for (index, props) in props.into_iter().enumerate() {
            if props != crate::assets::tilemap::TileProps::default() {
                map.set_tile_props(index as TileIndex, props);
            }
        }
        let tiles = Tiles::of(tileset, &map);
        for (at, layer) in self.layers.iter().enumerate() {
            let field = format!("layers[{at}]");
            match layer.kind {
                RoomLayerKind::Render => {
                    let index = map.add_layer(layer.name.clone(), LayerKind::Render);
                    let draw = LayerDraw {
                        z: layer.z,
                        unlit: layer.unlit,
                        tint: layer.tint.unwrap_or([255; 4]),
                    };
                    if draw != LayerDraw::DEFAULT {
                        map.set_layer_draw(index, draw);
                    }
                }
                RoomLayerKind::Collision => {
                    let index = map.add_layer(layer.name.clone(), LayerKind::Collision);
                    let bits = |names: &Option<Vec<String>>, list: &str, default: u32| {
                        names.as_ref().map_or(Ok(default), |names| {
                            layers
                                .bits(names.iter().map(String::as_str))
                                .map_err(|unknown| {
                                    RoomError::new(
                                        format!("{field}.{list}"),
                                        format!("unknown physics layer `{}`", unknown.0),
                                    )
                                })
                        })
                    };
                    let physics = LayerPhysics {
                        layers: bits(&layer.layers, "layers", LayerPhysics::DEFAULT.layers)?,
                        mask: bits(&layer.mask, "mask", LayerPhysics::DEFAULT.mask)?,
                    };
                    if physics != LayerPhysics::DEFAULT {
                        map.set_layer_physics(index, physics);
                    }
                }
            }
        }
        let layer_index = |name: &str| self.layers.iter().position(|layer| layer.name == name);
        for (at, shape) in self.shapes.iter().enumerate() {
            let field = format!("shapes[{at}]");
            let tile = match &shape.tile {
                None => EMPTY_TILE,
                Some(sprite) => tiles.index(sprite).ok_or_else(|| {
                    RoomError::new(
                        format!("{field}.tile"),
                        format!(
                            "no tile `{sprite}` in the tileset{}",
                            hint(sprite, tileset.tiles.iter().map(|t| t.sprite.as_str()))
                        ),
                    )
                })?,
            };
            let cells = self.shape_cells(shape, &field, tile, &tiles)?;
            for name in &shape.layers {
                let layer = layer_index(name).expect("a validated shape names its layers");
                for &(col, row, tile) in &cells {
                    map.set_tile(layer, col, row, tile);
                }
            }
        }
        for (at, layer) in self.layers.iter().enumerate() {
            for (key, sprite) in &layer.cells {
                let (col, row) = super::parse_cell_key(key).expect("a validated cell key");
                let tile = match sprite {
                    None => EMPTY_TILE,
                    Some(sprite) => tiles.index(sprite).ok_or_else(|| {
                        RoomError::new(
                            format!("layers[{at}].cells.{key}"),
                            format!(
                                "no tile `{sprite}` in the tileset{}",
                                hint(sprite, tileset.tiles.iter().map(|t| t.sprite.as_str()))
                            ),
                        )
                    })?,
                };
                map.set_tile(at, col, row, tile);
            }
        }
        let cell = Vec2::new(cell_w as f32, cell_h as f32);
        let objects = self
            .objects
            .iter()
            .enumerate()
            .map(|(at, object)| MapObject {
                id: (at + 1) as u32,
                name: format!("{id}.{}", object.id),
                class: object.prefab.clone(),
                position: Vec2::from(object.at) * cell,
                size: Vec2::ZERO,
                properties: object.set.clone(),
            })
            .collect();
        map.object_layers.push(ObjectLayer {
            name: ROOM_OBJECT_LAYER.to_owned(),
            objects,
        });
        let points: BTreeMap<&str, Vec2> = self
            .places
            .iter()
            .filter_map(|place| {
                place
                    .point
                    .map(|point| (place.id.as_str(), Vec2::from(point) * cell))
            })
            .collect();
        let places = self
            .places
            .iter()
            .map(|place| {
                let shape = if let Some(point) = place.point {
                    PlaceShape::Point(Vec2::from(point) * cell)
                } else if let Some([x, y, w, h]) = place.rect {
                    PlaceShape::Rect {
                        min: Vec2::new(x, y) * cell,
                        size: Vec2::new(w, h) * cell,
                    }
                } else {
                    let stops = place.route.as_deref().unwrap_or_default();
                    PlaceShape::Route(
                        stops
                            .iter()
                            .map(|stop| match stop {
                                RouteStop::Point(point) => Vec2::from(*point) * cell,
                                RouteStop::Place(id) => points[id.as_str()],
                            })
                            .collect(),
                    )
                };
                let compiled = CompiledPlace {
                    shape,
                    to: place.to.clone(),
                    by: place.by.clone(),
                };
                (place.id.clone(), compiled)
            })
            .collect();
        let budget = self.budget.unwrap_or_default();
        Ok(CompiledRoom {
            map,
            places,
            post: self.post.clone(),
            ambient: self.ambient.map(Vec3::from),
            lights: budget.lights(),
            objects: budget.objects(),
        })
    }

    /// The cells `shape` paints and the tile each takes, `tile` being the
    /// shape's own.
    fn shape_cells(
        &self,
        shape: &RoomShape,
        field: &str,
        tile: TileIndex,
        tiles: &Tiles,
    ) -> Result<Vec<(u32, u32, TileIndex)>, RoomError> {
        let [cols, rows] = self.size;
        let past = |col: u32, row: u32, field: String| {
            RoomError::new(
                field,
                format!("cell ({col}, {row}) is past the room's {}", self.extent()),
            )
        };
        if let Some([x, y, w, h]) = shape.rect {
            let (end_col, end_row) = (u64::from(x) + u64::from(w), u64::from(y) + u64::from(h));
            if end_col > u64::from(cols) || end_row > u64::from(rows) {
                let col = if end_col > u64::from(cols) { cols } else { x };
                let row = if end_row > u64::from(rows) { rows } else { y };
                return Err(past(col, row, format!("{field}.rect")));
            }
            let mut cells = Vec::with_capacity((w * h) as usize);
            for row in y..y + h {
                for col in x..x + w {
                    cells.push((col, row, tile));
                }
            }
            return Ok(cells);
        }
        if let Some(points) = &shape.line {
            let mut cells = Vec::new();
            for (at, &[col, row]) in points.iter().enumerate() {
                if col >= cols || row >= rows {
                    return Err(past(col, row, format!("{field}.line[{at}]")));
                }
            }
            for (at, pair) in points.windows(2).enumerate() {
                let ([x0, y0], [x1, y1]) = (pair[0], pair[1]);
                if x0 != x1 && y0 != y1 {
                    return Err(RoomError::new(
                        format!("{field}.line[{}]", at + 1),
                        format!(
                            "from [{x0}, {y0}] to [{x1}, {y1}] is neither horizontal nor vertical"
                        ),
                    ));
                }
                for row in y0.min(y1)..=y0.max(y1) {
                    for col in x0.min(x1)..=x0.max(x1) {
                        if !cells.iter().any(|&(c, r, _)| (c, r) == (col, row)) {
                            cells.push((col, row, tile));
                        }
                    }
                }
            }
            return Ok(cells);
        }
        let points = shape.polygon.as_deref().unwrap_or_default();
        polygon_cells(self, points, field, tile, tiles)
    }
}

/// The tileset as a compile reads it: each sprite's tile and each style's
/// slope tiles.
struct Tiles {
    by_sprite: BTreeMap<String, TileIndex>,
    /// By style and slope code: the style's slope tile.
    slopes: BTreeMap<(String, u8), TileIndex>,
    /// By tile: its style when it is that style's brush.
    brushes: BTreeMap<TileIndex, String>,
}

impl Tiles {
    fn of(tileset: &Tileset, map: &TilemapData) -> Self {
        let mut tiles = Self {
            by_sprite: BTreeMap::new(),
            slopes: BTreeMap::new(),
            brushes: BTreeMap::new(),
        };
        for (index, tile) in tileset.tiles.iter().enumerate() {
            let index = index as TileIndex;
            tiles.by_sprite.insert(tile.sprite.clone(), index);
            let props = map.tile_props(index);
            if let Some(style) = &props.style {
                if props.is_brush() {
                    tiles.brushes.insert(index, style.clone());
                } else if let (None, Some(slope)) = (props.mask, props.slope) {
                    tiles
                        .slopes
                        .entry((style.clone(), slope.code()))
                        .or_insert(index);
                }
            }
        }
        tiles
    }

    fn index(&self, sprite: &str) -> Option<TileIndex> {
        self.by_sprite.get(sprite).copied()
    }
}

/// A polygon's vertex or a test point, in twelfths of a cell.
type Point12 = (i64, i64);

/// What a sloped edge does in one cell it crosses: its edge's index and its
/// two ends in the cell, in half cells from the cell's top-left (each 0, 1
/// or 2).
#[derive(Clone, Copy)]
struct Crossing {
    edge: usize,
    ends: [(u8, u8); 2],
}

/// The cells `points` fills, each with its tile.
fn polygon_cells(
    room: &Room,
    points: &[[u32; 2]],
    field: &str,
    tile: TileIndex,
    tiles: &Tiles,
) -> Result<Vec<(u32, u32, TileIndex)>, RoomError> {
    let [cols, rows] = room.size;
    let polygon_field = format!("{field}.polygon");
    for (at, &[x, y]) in points.iter().enumerate() {
        if x > cols || y > rows {
            return Err(RoomError::new(
                format!("{polygon_field}[{at}]"),
                format!(
                    "corner [{x}, {y}] is past the room's {} (corners run 0 to {cols} and 0 to {rows})",
                    room.extent()
                ),
            ));
        }
    }
    let count = points.len();
    let edge = |at: usize| -> ((i64, i64), (i64, i64)) {
        let [x0, y0] = points[at];
        let [x1, y1] = points[(at + 1) % count];
        (
            (i64::from(x0), i64::from(y0)),
            (i64::from(x1), i64::from(y1)),
        )
    };
    for at in 0..count {
        let ((x0, y0), (x1, y1)) = edge(at);
        let (dx, dy) = ((x1 - x0).abs(), (y1 - y0).abs());
        let fine = (dx == 0) != (dy == 0) || (dx > 0 && (dx == dy || dx == 2 * dy));
        if !fine {
            return Err(RoomError::new(
                format!("{polygon_field}[{at}]"),
                format!(
                    "the edge from [{x0}, {y0}] to [{x1}, {y1}] is not horizontal, vertical, 45\u{b0} or 2:1"
                ),
            ));
        }
    }
    // Simple: no two edges meet but neighbours at their shared corner, and
    // no neighbour folds back over the other.
    for a in 0..count {
        for b in a + 1..count {
            let neighbours = b == a + 1 || (a == 0 && b == count - 1);
            let (p, q) = edge(a);
            let (r, s) = edge(b);
            let crosses = if neighbours {
                folds_back(edge(a), edge(b))
            } else {
                segments_meet(p, q, r, s)
            };
            if crosses {
                return Err(RoomError::new(
                    polygon_field,
                    format!("edges {a} and {b} cross; a polygon is simple"),
                ));
            }
        }
    }
    let mut crossings: BTreeMap<(u32, u32), Crossing> = BTreeMap::new();
    for at in 0..count {
        let ((x0, y0), (x1, y1)) = edge(at);
        let (dx, dy) = (x1 - x0, y1 - y0);
        if dx == 0 || dy == 0 {
            continue;
        }
        // In half cells: each piece of the edge inside one cell.
        let (sx, sy) = (dx.signum(), dy.signum());
        let pieces: Vec<((i64, i64), (i64, i64))> = if dx.abs() == dy.abs() {
            (0..dx.abs())
                .map(|i| {
                    let start = (2 * (x0 + i * sx), 2 * (y0 + i * sy));
                    (start, (start.0 + 2 * sx, start.1 + 2 * sy))
                })
                .collect()
        } else {
            (0..dy.abs())
                .flat_map(|i| {
                    let start = (2 * (x0 + 2 * i * sx), 2 * (y0 + i * sy));
                    let mid = (start.0 + 2 * sx, start.1 + sy);
                    let end = (mid.0 + 2 * sx, mid.1 + sy);
                    [(start, mid), (mid, end)]
                })
                .collect()
        };
        for (start, end) in pieces {
            let col = start.0.min(end.0).div_euclid(2);
            let row = start.1.min(end.1).div_euclid(2);
            let local = |(x, y): (i64, i64)| ((x - 2 * col) as u8, (y - 2 * row) as u8);
            let cell = (col as u32, row as u32);
            if let Some(first) = crossings.get(&cell) {
                return Err(RoomError::new(
                    polygon_field,
                    format!(
                        "cell ({}, {}) is crossed by edges {} and {at}; a slope cell holds one face",
                        cell.0, cell.1, first.edge
                    ),
                ));
            }
            crossings.insert(
                cell,
                Crossing {
                    edge: at,
                    ends: [local(start), local(end)],
                },
            );
        }
    }
    let vertices: Vec<Point12> = points
        .iter()
        .map(|&[x, y]| (12 * i64::from(x), 12 * i64::from(y)))
        .collect();
    let inside = |point: Point12| contains(&vertices, point);
    let style = tiles.brushes.get(&tile);
    let (min_x, max_x) = min_max(points.iter().map(|p| p[0]));
    let (min_y, max_y) = min_max(points.iter().map(|p| p[1]));
    let mut cells = Vec::new();
    for row in min_y..max_y {
        for col in min_x..max_x {
            let origin = (12 * i64::from(col), 12 * i64::from(row));
            let centre = (origin.0 + 6, origin.1 + 6);
            let Some(crossing) = crossings.get(&(col, row)) else {
                if inside(centre) {
                    cells.push((col, row, tile));
                }
                continue;
            };
            let [a, b] = crossing.ends;
            let (low, high) = if a.0 <= b.0 { (a, b) } else { (b, a) };
            // A point just below the face's middle, in twelfths.
            let mid = (3 * i64::from(low.0 + high.0), 3 * i64::from(low.1 + high.1));
            let below = inside((origin.0 + mid.0, origin.1 + mid.1 + 2));
            let face = (low, high);
            let Some(slope) = slope_of(face, below) else {
                continue;
            };
            if let Some(style) = style {
                let Some(&slope_tile) = tiles.slopes.get(&(style.clone(), slope.code())) else {
                    return Err(RoomError::new(
                        field,
                        format!(
                            "cell ({col}, {row}) needs style `{style}`'s `{}` tile, which the tileset lacks",
                            slope_text(slope)
                        ),
                    ));
                };
                cells.push((col, row, slope_tile));
            } else {
                // A 45° face runs through the centre: on the edge.
                let on_edge = slope.rise == SlopeRise::Full;
                if on_edge || inside(centre) {
                    cells.push((col, row, tile));
                }
            }
        }
    }
    Ok(cells)
}

/// The slope of a cell whose face runs from `low` to `high` (by x, in half
/// cells), solid `below` it or above it.
fn slope_of(face: ((u8, u8), (u8, u8)), below: bool) -> Option<Slope> {
    use SlopeCorner::{BottomLeft, BottomRight, TopLeft, TopRight};
    use SlopeRise::{Full, High, Low};
    let (rise, corner) = match (face, below) {
        (((0, 2), (2, 0)), true) => (Full, BottomRight),
        (((0, 2), (2, 0)), false) => (Full, TopLeft),
        (((0, 0), (2, 2)), true) => (Full, BottomLeft),
        (((0, 0), (2, 2)), false) => (Full, TopRight),
        (((0, 2), (2, 1)), true) => (Low, BottomRight),
        (((0, 2), (2, 1)), false) => (High, TopLeft),
        (((0, 1), (2, 0)), true) => (High, BottomRight),
        (((0, 1), (2, 0)), false) => (Low, TopLeft),
        (((0, 0), (2, 1)), true) => (High, BottomLeft),
        (((0, 0), (2, 1)), false) => (Low, TopRight),
        (((0, 1), (2, 2)), true) => (Low, BottomLeft),
        (((0, 1), (2, 2)), false) => (High, TopRight),
        _ => return None,
    };
    Some(Slope { rise, corner })
}

/// A slope as a tileset writes it: `"45 br"`.
fn slope_text(slope: Slope) -> String {
    let rise = match slope.rise {
        SlopeRise::Full => "45",
        SlopeRise::Low => "2:1 low",
        SlopeRise::High => "2:1 high",
    };
    let corner = match slope.corner {
        SlopeCorner::BottomRight => "br",
        SlopeCorner::BottomLeft => "bl",
        SlopeCorner::TopRight => "tr",
        SlopeCorner::TopLeft => "tl",
    };
    format!("{rise} {corner}")
}

fn min_max(values: impl Iterator<Item = u32>) -> (u32, u32) {
    values.fold((u32::MAX, 0), |(low, high), value| {
        (low.min(value), high.max(value))
    })
}

/// Whether `point` is inside the polygon of `vertices`, by the even-odd
/// rule; a point on an edge is never asked.
fn contains(vertices: &[Point12], (x, y): Point12) -> bool {
    let mut inside = false;
    let count = vertices.len();
    for at in 0..count {
        let (x0, y0) = vertices[at];
        let (x1, y1) = vertices[(at + 1) % count];
        if (y0 > y) != (y1 > y) {
            // The edge's x at `y`, compared without dividing.
            let lhs = (x - x0) * (y1 - y0);
            let rhs = (x1 - x0) * (y - y0);
            let left_of = if y1 > y0 { lhs < rhs } else { lhs > rhs };
            if left_of {
                inside = !inside;
            }
        }
    }
    inside
}

fn orientation(p: (i64, i64), q: (i64, i64), r: (i64, i64)) -> i64 {
    ((q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)).signum()
}

fn on_segment(p: (i64, i64), q: (i64, i64), r: (i64, i64)) -> bool {
    r.0 >= p.0.min(q.0) && r.0 <= p.0.max(q.0) && r.1 >= p.1.min(q.1) && r.1 <= p.1.max(q.1)
}

/// Whether segments `p`–`q` and `r`–`s` share any point.
fn segments_meet(p: (i64, i64), q: (i64, i64), r: (i64, i64), s: (i64, i64)) -> bool {
    let (o1, o2) = (orientation(p, q, r), orientation(p, q, s));
    let (o3, o4) = (orientation(r, s, p), orientation(r, s, q));
    if o1 != o2 && o3 != o4 && o1 * o2 <= 0 && o3 * o4 <= 0 {
        return true;
    }
    (o1 == 0 && on_segment(p, q, r))
        || (o2 == 0 && on_segment(p, q, s))
        || (o3 == 0 && on_segment(r, s, p))
        || (o4 == 0 && on_segment(r, s, q))
}

/// Whether two neighbouring edges overlap past their shared corner: the
/// second turns straight back along the first.
fn folds_back((a0, a1): ((i64, i64), (i64, i64)), (b0, b1): ((i64, i64), (i64, i64))) -> bool {
    // The shared corner and the two far ends.
    let (corner, far_a, far_b) = if a1 == b0 {
        (a1, a0, b1)
    } else if a0 == b1 {
        (a0, a1, b0)
    } else if a0 == b0 {
        (a0, a1, b1)
    } else {
        (a1, a0, b0)
    };
    let u = (far_a.0 - corner.0, far_a.1 - corner.1);
    let v = (far_b.0 - corner.0, far_b.1 - corner.1);
    let cross = u.0 * v.1 - u.1 * v.0;
    let dot = u.0 * v.0 + u.1 * v.1;
    cross == 0 && dot > 0
}

#[cfg(test)]
#[path = "../../tests/assets/room_compile.rs"]
mod tests;
