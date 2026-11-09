//! Rooms (redo 7.12, P7-17): a level in the engine's own format, one JSON
//! file a room, which compiles to 7.8's runtime terrain. A room holds its
//! terrain as shapes painted into its layers, objects by prefab, named
//! places (points, areas and routes) and links to other rooms' places, and
//! the post stack and ambient light it spawns with.
//!
//! ```json
//! {
//!   "version": 1,
//!   "cell": [16, 16],
//!   "size": [40, 24],
//!   "tileset": "cave",
//!   "layers": [
//!     { "name": "art", "z": 1 },
//!     { "name": "solid", "kind": "collision", "layers": ["terrain"], "mask": ["player"],
//!       "cells": { "7,19": "rock_crack", "8,19": null } }
//!   ],
//!   "shapes": [
//!     { "id": "floor", "rect": [0, 20, 40, 4], "tile": "rock", "layers": ["art", "solid"] },
//!     { "id": "ramp", "polygon": [[10, 20], [14, 16], [14, 20]], "tile": "rock", "layers": ["art", "solid"] }
//!   ],
//!   "objects": [{ "id": "crate_1", "prefab": "crate", "at": [12, 19], "on": "floor" }],
//!   "places": [
//!     { "id": "player_start", "point": [2, 19] },
//!     { "id": "exit", "point": [39, 19], "to": "hall.west_door" }
//!   ]
//! }
//! ```
//!
//! [`Room::from_json`] reads `version` first, so a version other than
//! [`ROOM_VERSION`] fails before any other field; an unknown field fails,
//! and every error names its field's path (`objects[3].at`).
//! [`Room::to_canonical`] writes the repository's form, which reads back as
//! the same room; [`json_schema`] describes the format. A room's tiles come
//! from a [`Tileset`] file every room on it shares, and
//! [`Room::compile`] turns the two into a [`CompiledRoom`]: a map for the
//! `TilemapRegistry`, its objects in one object layer, and its places in
//! pixels.
//!
//! IDs, layer names and room IDs are `[a-z][a-z0-9_]*`. Coordinates are in
//! cells from the room's top-left, y down. An object's name in the World is
//! `<room>.<id>`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value, json};

use crate::canonical::{self, Arrays};
use crate::post::PostPass;

mod command;
mod compile;
mod patch;
mod rules;
mod spawn;
pub mod tileset;

pub use command::{
    DeleteCommand, DuplicateCommand, EraseCommand, JournalEntry, LinkCommand, MoveCommand,
    PaintCommand, PlaceCommand, RenameCommand, RoomCommand, RoomCommandError, RoomContext,
    RoomJournal, SetCommand,
};
pub use compile::{CompiledPlace, CompiledRoom, PlaceShape, ROOM_OBJECT_LAYER};
pub use patch::{MapPatch, RoomPatch, RoomPatchError};
pub use rules::{
    CellKind, PrefabBody, RoomFinding, Support, Terrain, prefab_body, slope_normal, slope_top,
};
pub use spawn::{RoomRegistry, SpawnedRoom};
pub use tileset::{TILESET_VERSION, Tileset, TilesetTile};

/// The room format's version: a room file names it first.
pub const ROOM_VERSION: u32 = 1;

/// The most cells a room spans on an axis.
pub const MAX_ROOM_CELLS: u32 = 4_096;

/// A room file's model. Its lists keep their order; [`Room::from_json`]
/// checks it, and [`Room::validate`] checks one built in code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Room {
    /// The format's version, [`ROOM_VERSION`].
    pub version: u32,
    /// A cell's size in pixels, `[width, height]`, each at least 1.
    pub cell: [u32; 2],
    /// The room's size in cells, `[columns, rows]`, each 1 to
    /// [`MAX_ROOM_CELLS`].
    pub size: [u32; 2],
    /// The `tilesets` ID its tiles come from; needed once any layer exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tileset: Option<String>,
    /// Its layers, each one runtime layer, in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<RoomLayer>,
    /// Its terrain, painted in this order, a later shape over an earlier.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<RoomShape>,
    /// The prefabs it spawns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<RoomObject>,
    /// Its named points, areas and routes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub places: Vec<RoomPlace>,
    /// The post stack its spawn sets, as `PostStack`'s passes; left as it
    /// is when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<PostPass>>,
    /// The ambient light its spawn sets, linear RGB as `AmbientLight`; left
    /// as it is when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<[f32; 3]>,
    /// The caps its level rules hold it to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<RoomBudget>,
}

/// A room layer: one runtime layer, drawn (`render`, the default) or
/// collided with (`collision`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RoomLayer {
    /// Its name, unique in the room.
    pub name: String,
    /// Drawn or collided with.
    #[serde(default, skip_serializing_if = "RoomLayerKind::is_render")]
    pub kind: RoomLayerKind,
    /// A render layer's place among the sprites, as `LayerDraw::z`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<i32>,
    /// A render layer drawn on the stock pipeline, as `LayerDraw::unlit`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unlit: bool,
    /// A render layer's tint, sRGB bytes, as `LayerDraw::tint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<[u8; 4]>,
    /// A collision layer's physics layers, by `physics_layers` name; layer
    /// 0 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<Vec<String>>,
    /// The physics layers a collision layer meets, by name; every layer
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Vec<String>>,
    /// Per-cell overrides keyed `"<col>,<row>"`: a tile by its sprite, or
    /// `null` for empty, painted after the shapes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cells: BTreeMap<String, Option<String>>,
}

/// Whether a [`RoomLayer`] is drawn or collided with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RoomLayerKind {
    /// Drawn.
    #[default]
    Render,
    /// Collided with.
    Collision,
}

impl RoomLayerKind {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference.
    fn is_render(&self) -> bool {
        *self == Self::Render
    }
}

/// A shape of terrain: a `rect`, a `polygon` or a `line` of cells, painted
/// with one tile into the layers it names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RoomShape {
    /// Its ID, unique across the room's shapes, objects and places.
    pub id: String,
    /// `[x, y, w, h]` in cells, each side at least 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<[u32; 4]>,
    /// At least three vertices at cell corners; every edge horizontal,
    /// vertical, 45° or 2:1, the polygon simple, and no cell crossed by two
    /// sloped edges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<[u32; 2]>>,
    /// At least two cells, both ends included, joined by horizontal or
    /// vertical segments one cell thick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<Vec<[u32; 2]>>,
    /// A tileset tile by its sprite, a style's brush included; `null`
    /// erases. Given always.
    #[serde(deserialize_with = "nullable")]
    pub tile: Option<String>,
    /// The layers it paints, at least one.
    pub layers: Vec<String>,
}

/// A prefab the room spawns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RoomObject {
    /// Its ID, unique across the room's shapes, objects and places; its
    /// World name is `<room>.<id>`.
    pub id: String,
    /// The prefab it spawns.
    pub prefab: String,
    /// Its spawn position in cells, fractions allowed: `at × cell` is its
    /// `Transform.position`, where its sprite's anchor draws.
    pub at: [f32; 2],
    /// Overrides as dotted paths into the prefab's components, as a Tiled
    /// object's properties apply.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, Value>,
    /// The shape it is anchored to, which a `move` of that shape carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
}

/// A named place: a `point`, a `rect` or a `route`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RoomPlace {
    /// Its ID, unique across the room's shapes, objects and places.
    pub id: String,
    /// A point in cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<[f32; 2]>,
    /// An area, `[x, y, w, h]` in cells, each side above 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f32; 4]>,
    /// At least two stops, each a point or a `point` place's ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Vec<RouteStop>>,
    /// A point's or an area's link, `<room>.<place>`: a door or a socket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// A route's mover: the prefab its reach is checked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

/// A route's stop: a point place's ID, or a point in cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum RouteStop {
    /// A `point` place's ID.
    Place(String),
    /// A point in cells.
    Point([f32; 2]),
}

/// The caps a room's level rules hold it to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RoomBudget {
    /// The most objects with a light; 16 when absent (`D-061`'s cap).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lights: Option<u32>,
    /// The most objects; 1,024 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<u32>,
}

impl RoomBudget {
    /// The light cap when the room sets none.
    pub const DEFAULT_LIGHTS: u32 = 16;
    /// The object cap when the room sets none.
    pub const DEFAULT_OBJECTS: u32 = 1_024;

    /// The light cap: its own, else [`Self::DEFAULT_LIGHTS`].
    #[must_use]
    pub fn lights(&self) -> u32 {
        self.lights.unwrap_or(Self::DEFAULT_LIGHTS)
    }

    /// The object cap: its own, else [`Self::DEFAULT_OBJECTS`].
    #[must_use]
    pub fn objects(&self) -> u32 {
        self.objects.unwrap_or(Self::DEFAULT_OBJECTS)
    }
}

/// A room or tileset that does not read, check or compile: the field's
/// path and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RoomError {
    /// The field's path, as `objects[3].at`; empty for the whole file.
    pub field: String,
    /// What is wrong.
    pub message: String,
}

impl RoomError {
    pub(crate) fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for RoomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.field.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
}

impl std::error::Error for RoomError {}

impl Room {
    /// An empty room of `size` cells of `cell` pixels: no tileset, layers
    /// or entries.
    #[must_use]
    pub fn new(cell: [u32; 2], size: [u32; 2]) -> Self {
        Self {
            version: ROOM_VERSION,
            cell,
            size,
            tileset: None,
            layers: Vec::new(),
            shapes: Vec::new(),
            objects: Vec::new(),
            places: Vec::new(),
            post: None,
            ambient: None,
            budget: None,
        }
    }

    /// Parses room JSON: `version` first, so a version other than
    /// [`ROOM_VERSION`] fails before any other field; then the model, an
    /// error naming its field's path; then [`Room::validate`].
    ///
    /// # Errors
    ///
    /// A [`RoomError`] naming the field.
    pub fn from_json(json: &str) -> Result<Self, RoomError> {
        let value = versioned(json, "room", ROOM_VERSION)?;
        let room: Self = from_value(&value)?;
        room.validate()?;
        Ok(room)
    }

    /// The room in its canonical form: the model as JSON with defaults
    /// left out, keys sorted, lists in order, two-space indents, an array of
    /// numbers on one line, whole numbers without a fraction and a final
    /// newline. It reads back as the same room, and is its own canonical
    /// form.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        canonical::to_canonical(self, Arrays::InlineNumbers)
    }

    /// The checks [`Room::from_json`] runs past the model's own, for a room
    /// built in code: the version, the sizes, the tileset once any layer
    /// exists, names and IDs (`[a-z][a-z0-9_]*`, IDs unique across shapes,
    /// objects and places), each layer's fields for its kind and its cells'
    /// keys, each shape's one geometry and its layers, each object's
    /// position and anchor, each place's one form, its stops and its link.
    /// Geometry against the grid and tiles against the tileset are
    /// [`Room::compile`]'s.
    ///
    /// # Errors
    ///
    /// The first [`RoomError`], naming its field.
    pub fn validate(&self) -> Result<(), RoomError> {
        if self.version != ROOM_VERSION {
            return Err(RoomError::new(
                "version",
                format!(
                    "version {} is not supported; this engine reads version {ROOM_VERSION}",
                    self.version
                ),
            ));
        }
        for (axis, &pixels) in self.cell.iter().enumerate() {
            if pixels == 0 {
                return Err(RoomError::new(
                    format!("cell[{axis}]"),
                    "a cell is at least 1 pixel",
                ));
            }
        }
        for (axis, &cells) in self.size.iter().enumerate() {
            if !(1..=MAX_ROOM_CELLS).contains(&cells) {
                return Err(RoomError::new(
                    format!("size[{axis}]"),
                    format!("a room is 1 to {MAX_ROOM_CELLS} cells on an axis (got {cells})"),
                ));
            }
        }
        if let Some(tileset) = &self.tileset
            && tileset.is_empty()
        {
            return Err(RoomError::new("tileset", "a tileset ID is not empty"));
        }
        if self.tileset.is_none() && !self.layers.is_empty() {
            return Err(RoomError::new(
                "tileset",
                "missing; a room with layers names its tileset",
            ));
        }
        self.validate_layers()?;
        let entries = (self.shapes.iter().enumerate())
            .map(|(at, shape)| (shape.id.as_str(), format!("shapes[{at}]")))
            .chain(
                (self.objects.iter().enumerate())
                    .map(|(at, object)| (object.id.as_str(), format!("objects[{at}]"))),
            )
            .chain(
                (self.places.iter().enumerate())
                    .map(|(at, place)| (place.id.as_str(), format!("places[{at}]"))),
            );
        let mut ids: BTreeMap<&str, String> = BTreeMap::new();
        for (id, field) in entries {
            check_name(id).map_err(|message| RoomError::new(format!("{field}.id"), message))?;
            if let Some(first) = ids.get(id) {
                return Err(RoomError::new(
                    format!("{field}.id"),
                    format!("`{id}` is also {first}'s ID"),
                ));
            }
            ids.insert(id, field);
        }
        for (at, shape) in self.shapes.iter().enumerate() {
            self.validate_shape(shape, &format!("shapes[{at}]"))?;
        }
        for (at, object) in self.objects.iter().enumerate() {
            self.validate_object(object, &format!("objects[{at}]"))?;
        }
        for (at, place) in self.places.iter().enumerate() {
            self.validate_place(place, &format!("places[{at}]"))?;
        }
        if let Some(ambient) = self.ambient
            && !ambient
                .iter()
                .all(|channel| channel.is_finite() && *channel >= 0.0)
        {
            return Err(RoomError::new(
                "ambient",
                "each channel is finite and at least 0",
            ));
        }
        Ok(())
    }

    fn validate_layers(&self) -> Result<(), RoomError> {
        let mut names = BTreeSet::new();
        for (at, layer) in self.layers.iter().enumerate() {
            let field = format!("layers[{at}]");
            check_name(&layer.name)
                .map_err(|message| RoomError::new(format!("{field}.name"), message))?;
            if !names.insert(layer.name.as_str()) {
                return Err(RoomError::new(
                    format!("{field}.name"),
                    format!("layer `{}` is named twice", layer.name),
                ));
            }
            let misplaced: Vec<(&str, bool)> = match layer.kind {
                RoomLayerKind::Render => vec![
                    ("layers", layer.layers.is_some()),
                    ("mask", layer.mask.is_some()),
                ],
                RoomLayerKind::Collision => vec![
                    ("z", layer.z.is_some()),
                    ("unlit", layer.unlit),
                    ("tint", layer.tint.is_some()),
                ],
            };
            if let Some((name, _)) = misplaced.iter().find(|(_, set)| *set) {
                let kind = match layer.kind {
                    RoomLayerKind::Render => "a render layer",
                    RoomLayerKind::Collision => "a collision layer",
                };
                return Err(RoomError::new(
                    format!("{field}.{name}"),
                    format!("{kind} takes no `{name}`"),
                ));
            }
            for (list, names) in [("layers", &layer.layers), ("mask", &layer.mask)] {
                if let Some(empty) = names.iter().flatten().position(String::is_empty) {
                    return Err(RoomError::new(
                        format!("{field}.{list}[{empty}]"),
                        "a physics layer's name is not empty",
                    ));
                }
            }
            for (key, tile) in &layer.cells {
                let cell_field = format!("{field}.cells.{key}");
                let (col, row) = parse_cell_key(key)
                    .ok_or_else(|| RoomError::new(&cell_field, "a cell is `\"<col>,<row>\"`"))?;
                if col >= self.size[0] || row >= self.size[1] {
                    return Err(RoomError::new(
                        &cell_field,
                        format!("cell ({col}, {row}) is past the room's {}", self.extent()),
                    ));
                }
                if tile.as_deref() == Some("") {
                    return Err(RoomError::new(
                        &cell_field,
                        "a tile is its sprite, not empty",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_shape(&self, shape: &RoomShape, field: &str) -> Result<(), RoomError> {
        let forms = [
            shape.rect.is_some(),
            shape.polygon.is_some(),
            shape.line.is_some(),
        ];
        if forms.iter().filter(|set| **set).count() != 1 {
            return Err(RoomError::new(
                field,
                "a shape is one of `rect`, `polygon` or `line`",
            ));
        }
        if let Some([_, _, w, h]) = shape.rect
            && (w == 0 || h == 0)
        {
            return Err(RoomError::new(
                format!("{field}.rect"),
                "a rect's width and height are at least 1",
            ));
        }
        if let Some(points) = &shape.polygon
            && points.len() < 3
        {
            return Err(RoomError::new(
                format!("{field}.polygon"),
                "a polygon has at least three vertices",
            ));
        }
        if let Some(points) = &shape.line
            && points.len() < 2
        {
            return Err(RoomError::new(
                format!("{field}.line"),
                "a line has at least two cells",
            ));
        }
        if shape.tile.as_deref() == Some("") {
            return Err(RoomError::new(
                format!("{field}.tile"),
                "a tile is its sprite, or `null` to erase",
            ));
        }
        if shape.layers.is_empty() {
            return Err(RoomError::new(
                format!("{field}.layers"),
                "a shape paints at least one layer",
            ));
        }
        for (at, name) in shape.layers.iter().enumerate() {
            if !self.layers.iter().any(|layer| layer.name == *name) {
                return Err(RoomError::new(
                    format!("{field}.layers[{at}]"),
                    format!(
                        "no layer `{name}`{}",
                        hint(name, self.layers.iter().map(|layer| layer.name.as_str()))
                    ),
                ));
            }
            if shape.layers[..at].contains(name) {
                return Err(RoomError::new(
                    format!("{field}.layers[{at}]"),
                    format!("layer `{name}` is named twice"),
                ));
            }
        }
        Ok(())
    }

    fn validate_object(&self, object: &RoomObject, field: &str) -> Result<(), RoomError> {
        if object.prefab.is_empty() {
            return Err(RoomError::new(
                format!("{field}.prefab"),
                "a prefab ID is not empty",
            ));
        }
        self.check_point(object.at, &format!("{field}.at"))?;
        if let Some(shape) = &object.on
            && !self.shapes.iter().any(|known| known.id == *shape)
        {
            return Err(RoomError::new(
                format!("{field}.on"),
                format!(
                    "no shape `{shape}`{}",
                    hint(shape, self.shapes.iter().map(|known| known.id.as_str()))
                ),
            ));
        }
        for path in object.set.keys() {
            if path.is_empty() || path.split('.').any(str::is_empty) {
                return Err(RoomError::new(
                    format!("{field}.set"),
                    format!("`{path}` is not a dotted path into a component"),
                ));
            }
        }
        Ok(())
    }

    fn validate_place(&self, place: &RoomPlace, field: &str) -> Result<(), RoomError> {
        let forms = [
            place.point.is_some(),
            place.rect.is_some(),
            place.route.is_some(),
        ];
        if forms.iter().filter(|set| **set).count() != 1 {
            return Err(RoomError::new(
                field,
                "a place is one of `point`, `rect` or `route`",
            ));
        }
        if let Some(point) = place.point {
            self.check_point(point, &format!("{field}.point"))?;
        }
        if let Some([x, y, w, h]) = place.rect {
            if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
                return Err(RoomError::new(
                    format!("{field}.rect"),
                    "an area's width and height are above 0",
                ));
            }
            self.check_point([x, y], &format!("{field}.rect"))?;
            self.check_point([x + w, y + h], &format!("{field}.rect"))?;
        }
        if let Some(route) = &place.route {
            if route.len() < 2 {
                return Err(RoomError::new(
                    format!("{field}.route"),
                    "a route has at least two stops",
                ));
            }
            for (at, stop) in route.iter().enumerate() {
                let stop_field = format!("{field}.route[{at}]");
                match stop {
                    RouteStop::Point(point) => self.check_point(*point, &stop_field)?,
                    RouteStop::Place(id) => {
                        let found = self.places.iter().find(|known| known.id == *id);
                        match found {
                            Some(known) if known.point.is_some() => {}
                            Some(_) => {
                                return Err(RoomError::new(
                                    stop_field,
                                    format!("place `{id}` is not a point"),
                                ));
                            }
                            None => {
                                return Err(RoomError::new(
                                    stop_field,
                                    format!(
                                        "no place `{id}`{}",
                                        hint(id, self.places.iter().map(|p| p.id.as_str()))
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }
        if let Some(to) = &place.to {
            if place.route.is_some() {
                return Err(RoomError::new(
                    format!("{field}.to"),
                    "a route takes no `to`; a point or an area links",
                ));
            }
            let valid = to
                .split_once('.')
                .is_some_and(|(room, id)| check_name(room).is_ok() && check_name(id).is_ok());
            if !valid {
                return Err(RoomError::new(
                    format!("{field}.to"),
                    format!("a link is `<room>.<place>` (got `{to}`)"),
                ));
            }
        }
        if let Some(by) = &place.by {
            if place.route.is_none() {
                return Err(RoomError::new(
                    format!("{field}.by"),
                    "only a route takes `by`",
                ));
            }
            if by.is_empty() {
                return Err(RoomError::new(
                    format!("{field}.by"),
                    "a prefab ID is not empty",
                ));
            }
        }
        Ok(())
    }

    /// A point in cells: finite and inside the room, its edges included.
    fn check_point(&self, [x, y]: [f32; 2], field: &str) -> Result<(), RoomError> {
        let inside = x.is_finite()
            && y.is_finite()
            && (0.0..=self.size[0] as f32).contains(&x)
            && (0.0..=self.size[1] as f32).contains(&y);
        if inside {
            Ok(())
        } else {
            Err(RoomError::new(
                field,
                format!("[{x}, {y}] is past the room's {}", self.extent()),
            ))
        }
    }

    /// The room's size as an error names it: `40 × 24 cells`.
    pub(crate) fn extent(&self) -> String {
        format!("{} \u{d7} {} cells", self.size[0], self.size[1])
    }
}

/// `"<col>,<row>"` as the two numbers.
pub(crate) fn parse_cell_key(key: &str) -> Option<(u32, u32)> {
    let (col, row) = key.split_once(',')?;
    let number = |text: &str| -> Option<u32> {
        (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    Some((number(col)?, number(row)?))
}

/// Whether `name` is an ID, a layer name or a room ID: `[a-z][a-z0-9_]*`.
pub(crate) fn check_name(name: &str) -> Result<(), String> {
    let mut bytes = name.bytes();
    let valid = bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(format!("`{name}` is not `[a-z][a-z0-9_]*`"))
    }
}

/// "; did you mean `<name>`?" for the candidate nearest `target`, or
/// nothing.
pub(crate) fn hint<'a>(target: &str, candidates: impl Iterator<Item = &'a str>) -> String {
    let mut sorted: Vec<&str> = candidates.collect();
    sorted.sort_unstable();
    crate::assets::data::did_you_mean(crate::assets::data::nearest(target, sorted.into_iter()))
}

/// `json` as an object whose `version` is `version`, checked before any
/// other field.
pub(crate) fn versioned(json: &str, what: &str, version: u32) -> Result<Value, RoomError> {
    let value: Value =
        serde_json::from_str(json).map_err(|err| RoomError::new("", format!("not JSON: {err}")))?;
    if !value.is_object() {
        return Err(RoomError::new(
            "",
            format!("a {what} file is a JSON object"),
        ));
    }
    match value.get("version") {
        None => Err(RoomError::new(
            "version",
            format!("missing; this engine reads version {version}"),
        )),
        Some(Value::Number(n)) if n.as_u64() == Some(u64::from(version)) => Ok(value),
        Some(other) => Err(RoomError::new(
            "version",
            format!("version {other} is not supported; this engine reads version {version}"),
        )),
    }
}

/// `value` as a `T`, an error naming the field where it stopped.
pub(crate) fn from_value<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, RoomError> {
    serde_path_to_error::deserialize(value).map_err(|err| {
        let field = err.path().to_string();
        let field = if field == "." { String::new() } else { field };
        RoomError::new(field, err.into_inner().to_string())
    })
}

/// An `Option` field the file must give, `null` allowed.
fn nullable<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference.
fn is_false(value: &bool) -> bool {
    !*value
}

/// The JSON Schema (draft 2020-12) of a room file: every object closed, a
/// shape and a place each one of their forms, IDs and names as their
/// pattern, a route's stop a point or an ID.
#[must_use]
pub fn json_schema() -> Value {
    let reference = |name: &str| json!({ "$ref": format!("#/$defs/{name}") });
    let id = json!({ "type": "string", "pattern": "^[a-z][a-z0-9_]*$" });
    let text = json!({ "type": "string", "minLength": 1 });
    let number = json!({ "type": "number" });
    let cell = json!({ "type": "integer", "minimum": 0 });
    let pair =
        |item: Value| json!({ "type": "array", "items": item, "minItems": 2, "maxItems": 2 });
    let quad =
        |item: Value| json!({ "type": "array", "items": item, "minItems": 4, "maxItems": 4 });
    let names = json!({ "type": "array", "items": text.clone() });
    let one_of = |forms: &[&str]| -> Value {
        json!(
            forms
                .iter()
                .map(|form| json!({ "required": [form] }))
                .collect::<Vec<_>>()
        )
    };
    let mut defs = Map::new();
    defs.insert(
        "layer".into(),
        closed(
            &[
                ("name", id.clone()),
                ("kind", json!({ "enum": ["render", "collision"] })),
                ("z", json!({ "type": "integer" })),
                ("unlit", json!({ "type": "boolean" })),
                (
                    "tint",
                    quad(json!({ "type": "integer", "minimum": 0, "maximum": 255 })),
                ),
                ("layers", names.clone()),
                ("mask", names.clone()),
                (
                    "cells",
                    json!({
                        "type": "object",
                        "propertyNames": { "pattern": "^[0-9]+,[0-9]+$" },
                        "additionalProperties": { "type": ["string", "null"], "minLength": 1 },
                    }),
                ),
            ],
            &["name"],
        ),
    );
    let mut shape = closed(
        &[
            ("id", id.clone()),
            ("rect", quad(cell.clone())),
            (
                "polygon",
                json!({ "type": "array", "items": pair(cell.clone()), "minItems": 3 }),
            ),
            (
                "line",
                json!({ "type": "array", "items": pair(cell.clone()), "minItems": 2 }),
            ),
            (
                "tile",
                json!({ "type": ["string", "null"], "minLength": 1 }),
            ),
            (
                "layers",
                json!({ "type": "array", "items": id.clone(), "minItems": 1 }),
            ),
        ],
        &["id", "tile", "layers"],
    );
    shape["oneOf"] = one_of(&["rect", "polygon", "line"]);
    defs.insert("shape".into(), shape);
    defs.insert(
        "object".into(),
        closed(
            &[
                ("id", id.clone()),
                ("prefab", text.clone()),
                ("at", pair(number.clone())),
                ("set", json!({ "type": "object" })),
                ("on", id.clone()),
            ],
            &["id", "prefab", "at"],
        ),
    );
    let mut place = closed(
        &[
            ("id", id.clone()),
            ("point", pair(number.clone())),
            ("rect", quad(number.clone())),
            (
                "route",
                json!({
                    "type": "array",
                    "minItems": 2,
                    "items": { "oneOf": [id.clone(), pair(number.clone())] },
                }),
            ),
            (
                "to",
                json!({ "type": "string", "pattern": "^[a-z][a-z0-9_]*\\.[a-z][a-z0-9_]*$" }),
            ),
            ("by", text.clone()),
        ],
        &["id"],
    );
    place["oneOf"] = one_of(&["point", "rect", "route"]);
    defs.insert("place".into(), place);
    defs.insert(
        "budget".into(),
        closed(
            &[
                ("lights", json!({ "type": "integer", "minimum": 0 })),
                ("objects", json!({ "type": "integer", "minimum": 0 })),
            ],
            &[],
        ),
    );
    let list = |name: &str| json!({ "type": "array", "items": reference(name) });
    let mut schema = closed(
        &[
            ("version", json!({ "const": ROOM_VERSION })),
            ("cell", pair(json!({ "type": "integer", "minimum": 1 }))),
            (
                "size",
                pair(json!({ "type": "integer", "minimum": 1, "maximum": MAX_ROOM_CELLS })),
            ),
            ("tileset", text),
            ("layers", list("layer")),
            ("shapes", list("shape")),
            ("objects", list("object")),
            ("places", list("place")),
            (
                "post",
                json!({
                    "type": "array",
                    "items": { "type": "object", "required": ["kind"] },
                }),
            ),
            (
                "ambient",
                json!({
                    "type": "array",
                    "items": { "type": "number", "minimum": 0 },
                    "minItems": 3,
                    "maxItems": 3,
                }),
            ),
            ("budget", reference("budget")),
        ],
        &["version", "cell", "size"],
    );
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["title"] = json!("Tungsten room");
    schema["description"] = json!("A room file, listed in a manifest's rooms.");
    schema["$defs"] = Value::Object(defs);
    schema
}

/// A closed object of `properties`, `required` among them.
pub(crate) fn closed(properties: &[(&str, Value)], required: &[&str]) -> Value {
    let properties: Map<String, Value> = properties
        .iter()
        .map(|(name, schema)| ((*name).to_owned(), schema.clone()))
        .collect();
    let mut object = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
    });
    if !required.is_empty() {
        object["required"] = json!(required);
    }
    object
}

#[cfg(test)]
#[path = "../../tests/assets/room.rs"]
mod tests;
