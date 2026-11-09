//! Room commands (redo 7.12, §2.5 of its plan): a batch is a JSON array of
//! commands, each an object with one verb, applied to a [`Room`] all or
//! nothing; [`Room::apply`] returns the inverse batch, which applied gives
//! the room back byte for byte in its canonical form. [`RoomJournal`] keeps
//! each room's applied batches with their inverses, at most
//! [`RoomJournal::LIMIT`], for undo and redo.
//!
//! | Verb | Shape | Inverse |
//! | --- | --- | --- |
//! | `place` | `{"objects" \| "shapes" \| "places": entry, "index"?}`, or `{"layers": layer, "index"?}` | `delete` |
//! | `move` | `{"id", "by": [dx, dy]}` or `{"id", "to": [x, y]}`; a shape carries the objects `on` it | the move back |
//! | `set` | `{"id" \| "layer"?, "path", "value"}`: no `id` for the room's `size`, `tileset`, `post`, `ambient` and `budget`; `set.<key>` one override; `null` removes an optional field | `set` the old value |
//! | `delete` | `{"id"}` or `{"layer"}`; objects `on` a deleted shape lose `on` | `place` at its index, `on` restored |
//! | `duplicate` | `{"id", "as", "by"?}` | `delete` |
//! | `paint` | `{"layer", "rect" \| "cells", "tile"}`: overrides; `"tile": null` removes them | the old overrides |
//! | `erase` | `{"layer", "rect" \| "cells"}`: empty overrides | the old overrides |
//! | `link` | `{"id", "to"}`, `null` unlinking | `link` the old target |
//! | `rename` | `{"id", "to"}`: the room's own references follow | `rename` back |
//!
//! Every command is checked against the room as the earlier ones leave it:
//! the room's own checks ([`Room::validate`]), the prefab of an object it
//! places or changes, and, once it touches the terrain, the compile against
//! the [`RoomContext`]'s tileset and physics layer names. Component values
//! in `set` are `tungsten level check`'s, not `apply`'s.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    Room, RoomError, RoomLayer, RoomObject, RoomPlace, RoomShape, RouteStop, Tileset, hint,
};
use crate::physics::PhysicsLayers;

/// One command of a batch: an object with one verb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum RoomCommand {
    /// Adds an entry or a layer.
    Place(Box<PlaceCommand>),
    /// Moves an entry by an offset or to a point.
    Move(MoveCommand),
    /// Sets a field of the room, an entry or a layer.
    Set(SetCommand),
    /// Removes an entry or a layer.
    Delete(DeleteCommand),
    /// Copies an entry under a new ID.
    Duplicate(DuplicateCommand),
    /// Writes a layer's cell overrides.
    Paint(PaintCommand),
    /// Writes empty cell overrides.
    Erase(EraseCommand),
    /// Links a point or an area to another room's place.
    Link(LinkCommand),
    /// Gives an entry a new ID.
    Rename(RenameCommand),
}

/// `place`: one of `objects`, `shapes`, `places` or `layers`, at `index`
/// in its list, its end when absent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PlaceCommand {
    /// An object to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<RoomObject>,
    /// A shape to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shapes: Option<RoomShape>,
    /// A place to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub places: Option<RoomPlace>,
    /// A layer to add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<RoomLayer>,
    /// Its place in its list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

/// `move`: an entry `by` an offset or `to` a point, in cells; a shape moves
/// by whole cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct MoveCommand {
    /// The entry.
    pub id: String,
    /// The offset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<[f32; 2]>,
    /// The point its position takes: an object's `at`, a point, an area's
    /// or a rect's top-left, a polygon's or a line's first point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<[f32; 2]>,
}

/// `set`: `value` at `path` in the room, entry `id` or layer `layer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct SetCommand {
    /// The entry; the room itself when neither it nor `layer` is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The layer, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// The field, dotted; `set.<key>` names one override of an object.
    pub path: String,
    /// The value; `null` removes an optional field.
    pub value: Value,
}

/// `delete`: an entry or a layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DeleteCommand {
    /// The entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The layer, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
}

/// `duplicate`: entry `id` copied as `as`, `by` an offset, right after it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DuplicateCommand {
    /// The entry to copy.
    pub id: String,
    /// The copy's ID.
    #[serde(rename = "as")]
    pub as_id: String,
    /// The copy's offset, in cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<[f32; 2]>,
}

/// `paint`: `tile` over each cell of `rect` or `cells` in `layer`'s
/// overrides; `null` removes them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PaintCommand {
    /// The layer, by name.
    pub layer: String,
    /// `[x, y, w, h]` in cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<[u32; 4]>,
    /// The cells, `[col, row]` each.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<[u32; 2]>>,
    /// A tile by its sprite; `null` removes the overrides.
    pub tile: Option<String>,
}

/// `erase`: empty overrides over each cell of `rect` or `cells`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct EraseCommand {
    /// The layer, by name.
    pub layer: String,
    /// `[x, y, w, h]` in cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<[u32; 4]>,
    /// The cells, `[col, row]` each.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<[u32; 2]>>,
}

/// `link`: a point's or an area's `to`, `null` unlinking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct LinkCommand {
    /// The place.
    pub id: String,
    /// `<room>.<place>`, or `null`.
    pub to: Option<String>,
}

/// `rename`: entry `id` as `to`, the room's own references following.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RenameCommand {
    /// The entry.
    pub id: String,
    /// Its new ID.
    pub to: String,
}

impl RoomCommand {
    /// The command's verb, as a batch writes it.
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Place(_) => "place",
            Self::Move(_) => "move",
            Self::Set(_) => "set",
            Self::Delete(_) => "delete",
            Self::Duplicate(_) => "duplicate",
            Self::Paint(_) => "paint",
            Self::Erase(_) => "erase",
            Self::Link(_) => "link",
            Self::Rename(_) => "rename",
        }
    }

    /// Parses a batch: a JSON array of commands.
    ///
    /// # Errors
    ///
    /// A [`RoomCommandError`] naming the command and the field where
    /// reading stopped.
    pub fn parse_batch(json: &str) -> Result<Vec<Self>, RoomCommandError> {
        let value: Value = serde_json::from_str(json).map_err(|err| RoomCommandError {
            field: String::new(),
            message: format!("not JSON: {err}"),
        })?;
        Self::batch_from_value(&value)
    }

    /// [`parse_batch`](Self::parse_batch) from JSON already read.
    ///
    /// # Errors
    ///
    /// As [`parse_batch`](Self::parse_batch).
    pub fn batch_from_value(value: &Value) -> Result<Vec<Self>, RoomCommandError> {
        if !value.is_array() {
            return Err(RoomCommandError {
                field: String::new(),
                message: "a batch is a JSON array of commands".to_owned(),
            });
        }
        serde_path_to_error::deserialize(value).map_err(|err| {
            let path = err.path().to_string();
            RoomCommandError {
                field: format!("commands{}", if path == "." { "" } else { &path }),
                message: err.into_inner().to_string(),
            }
        })
    }
}

/// What a batch's checks read beyond the room: its ID (links to its own
/// places follow a rename), its tileset, the physics layer names and the
/// prefab IDs.
#[derive(Debug, Clone, Copy)]
pub struct RoomContext<'a> {
    /// The room's ID.
    pub room: &'a str,
    /// The tileset it names.
    pub tileset: &'a Tileset,
    /// The physics layer names its collision layers may name.
    pub layers: &'a PhysicsLayers,
    /// The prefab IDs, sorted.
    pub prefabs: &'a [&'a str],
}

/// A command that does not apply: the command's index, its verb and the
/// field, then what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RoomCommandError {
    /// The path, as `commands[3].move.id`.
    pub field: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for RoomCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.field.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
}

impl std::error::Error for RoomCommandError {}

/// What a paint or an erase writes over a cell's override.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Override {
    /// No override: the shapes show through.
    Remove,
    /// An empty cell.
    Empty,
    /// A tile, by its sprite.
    Tile(String),
}

/// Which list an entry is in, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Shape(usize),
    Object(usize),
    Place(usize),
}

impl Room {
    /// Applies `batch` against `context`: each command checked against the
    /// room as the earlier ones leave it, the room unchanged unless all
    /// pass. Returns the inverse batch, which applied gives the room back
    /// byte for byte in its canonical form.
    ///
    /// # Errors
    ///
    /// The first command that does not apply, as a [`RoomCommandError`]
    /// naming its index, verb and field, as
    /// ``commands[3].move.id: no entry `crawler_9`; did you mean `crawler_2`?``.
    pub fn apply(
        &mut self,
        batch: &[RoomCommand],
        context: &RoomContext<'_>,
    ) -> Result<Vec<RoomCommand>, RoomCommandError> {
        let mut room = self.clone();
        let mut inverses: Vec<Vec<RoomCommand>> = Vec::with_capacity(batch.len());
        for (index, command) in batch.iter().enumerate() {
            let field = format!("commands[{index}].{}", command.verb());
            let fail = |err: RoomError| RoomCommandError {
                field: if err.field.is_empty() {
                    field.clone()
                } else {
                    format!("{field}.{}", err.field)
                },
                message: err.message,
            };
            let (inverse, terrain) = room.apply_one(command, context).map_err(fail)?;
            room.check(context, terrain)
                .map_err(|err| RoomCommandError {
                    field: field.clone(),
                    message: err.to_string(),
                })?;
            inverses.push(inverse);
        }
        *self = room;
        Ok(inverses.into_iter().rev().flatten().collect())
    }

    /// The room's checks after a command: its own, then, when the command
    /// touched the terrain, its compile against `context`.
    fn check(&self, context: &RoomContext<'_>, terrain: bool) -> Result<(), RoomError> {
        self.validate()?;
        if terrain {
            self.compile(context.room, context.tileset, context.layers)?;
        }
        Ok(())
    }

    /// Applies one command: its inverse, and whether it touched the
    /// terrain. Errors name the command's own field.
    fn apply_one(
        &mut self,
        command: &RoomCommand,
        context: &RoomContext<'_>,
    ) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        match command {
            RoomCommand::Place(place) => self.place(place, context),
            RoomCommand::Move(movement) => self.move_entry(movement),
            RoomCommand::Set(set) => self.set_field(set, context),
            RoomCommand::Delete(delete) => self.delete(delete),
            RoomCommand::Duplicate(duplicate) => self.duplicate(duplicate, context),
            RoomCommand::Paint(paint) => {
                let cells = self.cells_of(&paint.layer, paint.rect, paint.cells.as_deref())?;
                let value = match &paint.tile {
                    Some(tile) => Override::Tile(tile.clone()),
                    None => Override::Remove,
                };
                Ok((self.overrides(&paint.layer, &cells, &value)?, true))
            }
            RoomCommand::Erase(erase) => {
                let cells = self.cells_of(&erase.layer, erase.rect, erase.cells.as_deref())?;
                Ok((
                    self.overrides(&erase.layer, &cells, &Override::Empty)?,
                    true,
                ))
            }
            RoomCommand::Link(link) => {
                let Some(Entry::Place(at)) = self.find(&link.id) else {
                    return Err(self.no_entry("id", &link.id, Some("place")));
                };
                let old = std::mem::replace(&mut self.places[at].to, link.to.clone());
                let inverse = RoomCommand::Link(LinkCommand {
                    id: link.id.clone(),
                    to: old,
                });
                Ok((vec![inverse], false))
            }
            RoomCommand::Rename(rename) => self.rename(rename, context),
        }
    }

    /// Entry `id`'s list and index.
    fn find(&self, id: &str) -> Option<Entry> {
        if let Some(at) = self.shapes.iter().position(|shape| shape.id == id) {
            return Some(Entry::Shape(at));
        }
        if let Some(at) = self.objects.iter().position(|object| object.id == id) {
            return Some(Entry::Object(at));
        }
        self.places
            .iter()
            .position(|place| place.id == id)
            .map(Entry::Place)
    }

    /// "no entry `id`", the nearest entry's ID suggested.
    fn no_entry(&self, field: &str, id: &str, kind: Option<&str>) -> RoomError {
        let ids = self
            .shapes
            .iter()
            .map(|shape| shape.id.as_str())
            .chain(self.objects.iter().map(|object| object.id.as_str()))
            .chain(self.places.iter().map(|place| place.id.as_str()));
        let what = kind.unwrap_or("entry");
        RoomError::new(field, format!("no {what} `{id}`{}", hint(id, ids)))
    }

    fn layer_index(&self, field: &str, name: &str) -> Result<usize, RoomError> {
        self.layers
            .iter()
            .position(|layer| layer.name == name)
            .ok_or_else(|| {
                RoomError::new(
                    field,
                    format!(
                        "no layer `{name}`{}",
                        hint(name, self.layers.iter().map(|layer| layer.name.as_str()))
                    ),
                )
            })
    }

    fn place(
        &mut self,
        place: &PlaceCommand,
        context: &RoomContext<'_>,
    ) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        let given = [
            place.objects.is_some(),
            place.shapes.is_some(),
            place.places.is_some(),
            place.layers.is_some(),
        ];
        if given.iter().filter(|set| **set).count() != 1 {
            return Err(RoomError::new(
                "",
                "a place gives one of `objects`, `shapes`, `places` or `layers`",
            ));
        }
        let at = |len: usize| -> Result<usize, RoomError> {
            match place.index {
                None => Ok(len),
                Some(index) if index <= len => Ok(index),
                Some(index) => Err(RoomError::new(
                    "index",
                    format!("index {index} is past the list's {len} entries"),
                )),
            }
        };
        if let Some(layer) = &place.layers {
            let index = at(self.layers.len())?;
            self.layers.insert(index, layer.clone());
            let inverse = RoomCommand::Delete(DeleteCommand {
                id: None,
                layer: Some(layer.name.clone()),
            });
            return Ok((vec![inverse], true));
        }
        let (id, terrain) = if let Some(object) = &place.objects {
            check_prefab(context, "objects.prefab", &object.prefab)?;
            let index = at(self.objects.len())?;
            self.objects.insert(index, object.clone());
            (object.id.clone(), false)
        } else if let Some(shape) = &place.shapes {
            let index = at(self.shapes.len())?;
            self.shapes.insert(index, shape.clone());
            (shape.id.clone(), true)
        } else {
            let entry = place.places.as_ref().expect("one of four is given");
            let index = at(self.places.len())?;
            self.places.insert(index, entry.clone());
            (entry.id.clone(), false)
        };
        let inverse = RoomCommand::Delete(DeleteCommand {
            id: Some(id),
            layer: None,
        });
        Ok((vec![inverse], terrain))
    }

    fn move_entry(
        &mut self,
        movement: &MoveCommand,
    ) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        if movement.by.is_some() == movement.to.is_some() {
            return Err(RoomError::new("", "a move gives one of `by` or `to`"));
        }
        let entry = self
            .find(&movement.id)
            .ok_or_else(|| self.no_entry("id", &movement.id, None))?;
        match entry {
            Entry::Object(at) => {
                let old = self.objects[at].at;
                self.objects[at].at = match (movement.by, movement.to) {
                    (Some([dx, dy]), _) => [old[0] + dx, old[1] + dy],
                    (_, Some(to)) => to,
                    _ => unreachable!("one of `by` or `to`"),
                };
                Ok((vec![move_to(&movement.id, old)], false))
            }
            Entry::Place(at) => {
                let place = &mut self.places[at];
                let old = place_anchor(place);
                let before = place.clone();
                match (movement.by, movement.to) {
                    (Some(by), _) => shift_place(place, by),
                    (_, Some(to)) => {
                        if old.is_none() {
                            return Err(RoomError::new("to", "a route moves by an offset"));
                        }
                        // Set, not shifted, so the point is exactly `to`.
                        if let Some(point) = &mut place.point {
                            *point = to;
                        }
                        if let Some([x, y, _, _]) = &mut place.rect {
                            [*x, *y] = to;
                        }
                    }
                    _ => unreachable!("one of `by` or `to`"),
                }
                let inverse = match old {
                    Some(old) => move_to(&movement.id, old),
                    None => RoomCommand::Set(SetCommand {
                        id: Some(movement.id.clone()),
                        layer: None,
                        path: "route".to_owned(),
                        value: serde_json::to_value(&before.route).unwrap_or_default(),
                    }),
                };
                Ok((vec![inverse], false))
            }
            Entry::Shape(at) => {
                let anchor = shape_anchor(&self.shapes[at]);
                let delta = match (movement.by, movement.to) {
                    (Some(by), _) => by,
                    (_, Some([x, y])) => [x - anchor[0] as f32, y - anchor[1] as f32],
                    _ => unreachable!("one of `by` or `to`"),
                };
                let whole = |v: f32| v.is_finite() && v.fract() == 0.0;
                if !(whole(delta[0]) && whole(delta[1])) {
                    return Err(RoomError::new(
                        if movement.by.is_some() { "by" } else { "to" },
                        "a shape moves by whole cells",
                    ));
                }
                let (dx, dy) = (delta[0] as i64, delta[1] as i64);
                shift_shape(&mut self.shapes[at], dx, dy).map_err(|message| {
                    RoomError::new(if movement.by.is_some() { "by" } else { "to" }, message)
                })?;
                // The objects on it ride along, and come back where they were.
                let mut inverse = vec![RoomCommand::Move(MoveCommand {
                    id: movement.id.clone(),
                    by: Some([-delta[0], -delta[1]]),
                    to: None,
                })];
                for object in &mut self.objects {
                    if object.on.as_deref() == Some(movement.id.as_str()) {
                        let old = object.at;
                        object.at = [old[0] + delta[0], old[1] + delta[1]];
                        inverse.push(move_to(&object.id, old));
                    }
                }
                Ok((inverse, true))
            }
        }
    }

    fn set_field(
        &mut self,
        set: &SetCommand,
        context: &RoomContext<'_>,
    ) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        if set.id.is_some() && set.layer.is_some() {
            return Err(RoomError::new(
                "",
                "a set names an `id` or a `layer`, not both",
            ));
        }
        let path = set.path.as_str();
        if let Some(name) = &set.layer {
            let at = self.layer_index("layer", name)?;
            let old = edit(&mut self.layers[at], path, &set.value, false)?;
            return Ok((vec![set_back(set, old)], true));
        }
        let Some(id) = &set.id else {
            let root = path.split('.').next().unwrap_or_default();
            if !["size", "tileset", "post", "ambient", "budget"].contains(&root) {
                return Err(RoomError::new(
                    "path",
                    format!(
                        "the room's own fields are `size`, `tileset`, `post`, `ambient` and `budget` (got `{path}`)"
                    ),
                ));
            }
            let old = edit(self, path, &set.value, false)?;
            let terrain = root == "size" || root == "tileset";
            return Ok((vec![set_back(set, old)], terrain));
        };
        let entry = self.find(id).ok_or_else(|| self.no_entry("id", id, None))?;
        if path == "id" {
            return Err(RoomError::new("path", "an ID changes by `rename`"));
        }
        let (old, terrain) = match entry {
            Entry::Object(at) => {
                let old = edit(&mut self.objects[at], path, &set.value, true)?;
                if path == "prefab" {
                    check_prefab(context, "value", &self.objects[at].prefab.clone())?;
                }
                (old, false)
            }
            Entry::Shape(at) => (edit(&mut self.shapes[at], path, &set.value, false)?, true),
            Entry::Place(at) => (edit(&mut self.places[at], path, &set.value, false)?, false),
        };
        Ok((vec![set_back(set, old)], terrain))
    }

    fn delete(&mut self, delete: &DeleteCommand) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        match (&delete.id, &delete.layer) {
            (Some(_), Some(_)) | (None, None) => {
                Err(RoomError::new("", "a delete names an `id` or a `layer`"))
            }
            (None, Some(name)) => {
                let at = self.layer_index("layer", name)?;
                let layer = self.layers.remove(at);
                let inverse = RoomCommand::Place(Box::new(PlaceCommand {
                    layers: Some(layer),
                    index: Some(at),
                    ..PlaceCommand::default()
                }));
                Ok((vec![inverse], true))
            }
            (Some(id), None) => {
                let entry = self.find(id).ok_or_else(|| self.no_entry("id", id, None))?;
                let mut inverse = Vec::new();
                let terrain = match entry {
                    Entry::Object(at) => {
                        let object = self.objects.remove(at);
                        inverse.push(RoomCommand::Place(Box::new(PlaceCommand {
                            objects: Some(object),
                            index: Some(at),
                            ..PlaceCommand::default()
                        })));
                        false
                    }
                    Entry::Place(at) => {
                        let place = self.places.remove(at);
                        inverse.push(RoomCommand::Place(Box::new(PlaceCommand {
                            places: Some(place),
                            index: Some(at),
                            ..PlaceCommand::default()
                        })));
                        false
                    }
                    Entry::Shape(at) => {
                        let shape = self.shapes.remove(at);
                        inverse.push(RoomCommand::Place(Box::new(PlaceCommand {
                            shapes: Some(shape),
                            index: Some(at),
                            ..PlaceCommand::default()
                        })));
                        for object in &mut self.objects {
                            if object.on.as_deref() == Some(id.as_str()) {
                                object.on = None;
                                inverse.push(RoomCommand::Set(SetCommand {
                                    id: Some(object.id.clone()),
                                    layer: None,
                                    path: "on".to_owned(),
                                    value: Value::String(id.clone()),
                                }));
                            }
                        }
                        true
                    }
                };
                Ok((inverse, terrain))
            }
        }
    }

    fn duplicate(
        &mut self,
        duplicate: &DuplicateCommand,
        context: &RoomContext<'_>,
    ) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        let entry = self
            .find(&duplicate.id)
            .ok_or_else(|| self.no_entry("id", &duplicate.id, None))?;
        let by = duplicate.by.unwrap_or([0.0, 0.0]);
        let terrain = match entry {
            Entry::Object(at) => {
                let mut copy = self.objects[at].clone();
                copy.id.clone_from(&duplicate.as_id);
                copy.at = [copy.at[0] + by[0], copy.at[1] + by[1]];
                check_prefab(context, "id", &copy.prefab)?;
                self.objects.insert(at + 1, copy);
                false
            }
            Entry::Place(at) => {
                let mut copy = self.places[at].clone();
                copy.id.clone_from(&duplicate.as_id);
                shift_place(&mut copy, by);
                self.places.insert(at + 1, copy);
                false
            }
            Entry::Shape(at) => {
                let whole = |v: f32| v.is_finite() && v.fract() == 0.0;
                if !(whole(by[0]) && whole(by[1])) {
                    return Err(RoomError::new("by", "a shape moves by whole cells"));
                }
                let mut copy = self.shapes[at].clone();
                copy.id.clone_from(&duplicate.as_id);
                shift_shape(&mut copy, by[0] as i64, by[1] as i64)
                    .map_err(|message| RoomError::new("by", message))?;
                self.shapes.insert(at + 1, copy);
                true
            }
        };
        let inverse = RoomCommand::Delete(DeleteCommand {
            id: Some(duplicate.as_id.clone()),
            layer: None,
        });
        Ok((vec![inverse], terrain))
    }

    /// The cells of `rect` or `cells`, each inside the room.
    fn cells_of(
        &self,
        layer: &str,
        rect: Option<[u32; 4]>,
        cells: Option<&[[u32; 2]]>,
    ) -> Result<Vec<(u32, u32)>, RoomError> {
        self.layer_index("layer", layer)?;
        let listed: Vec<(u32, u32)> = match (rect, cells) {
            (Some([x, y, w, h]), None) => (y..y.saturating_add(h))
                .flat_map(|row| (x..x.saturating_add(w)).map(move |col| (col, row)))
                .collect(),
            (None, Some(cells)) => cells.iter().map(|&[col, row]| (col, row)).collect(),
            _ => {
                return Err(RoomError::new("", "a paint gives one of `rect` or `cells`"));
            }
        };
        if let Some(&(col, row)) = listed
            .iter()
            .find(|&&(col, row)| col >= self.size[0] || row >= self.size[1])
        {
            return Err(RoomError::new(
                if rect.is_some() { "rect" } else { "cells" },
                format!("cell ({col}, {row}) is past the room's {}", self.extent()),
            ));
        }
        Ok(listed)
    }

    /// Writes `value` over each of `cells` in `layer`'s overrides; the
    /// inverse puts back each cell's old override.
    fn overrides(
        &mut self,
        layer: &str,
        cells: &[(u32, u32)],
        value: &Override,
    ) -> Result<Vec<RoomCommand>, RoomError> {
        let at = self.layer_index("layer", layer)?;
        // Each cell's old override, grouped into the commands that write it
        // back: removed, empty, or a tile.
        let mut removed = Vec::new();
        let mut empty = Vec::new();
        let mut tiles: BTreeMap<String, Vec<[u32; 2]>> = BTreeMap::new();
        let overrides = &mut self.layers[at].cells;
        for &(col, row) in cells {
            let key = format!("{col},{row}");
            let old = match value {
                Override::Remove => overrides.remove(&key),
                Override::Empty => overrides.insert(key, None),
                Override::Tile(tile) => overrides.insert(key, Some(tile.clone())),
            };
            match old {
                None => removed.push([col, row]),
                Some(None) => empty.push([col, row]),
                Some(Some(tile)) => tiles.entry(tile).or_default().push([col, row]),
            }
        }
        let mut inverse = Vec::new();
        if !removed.is_empty() && *value != Override::Remove {
            inverse.push(RoomCommand::Paint(PaintCommand {
                layer: layer.to_owned(),
                rect: None,
                cells: Some(removed),
                tile: None,
            }));
        }
        if !empty.is_empty() {
            inverse.push(RoomCommand::Erase(EraseCommand {
                layer: layer.to_owned(),
                rect: None,
                cells: Some(empty),
            }));
        }
        for (tile, cells) in tiles {
            inverse.push(RoomCommand::Paint(PaintCommand {
                layer: layer.to_owned(),
                rect: None,
                cells: Some(cells),
                tile: Some(tile),
            }));
        }
        Ok(inverse)
    }

    fn rename(
        &mut self,
        rename: &RenameCommand,
        context: &RoomContext<'_>,
    ) -> Result<(Vec<RoomCommand>, bool), RoomError> {
        let (old, new) = (rename.id.as_str(), rename.to.as_str());
        let entry = self
            .find(old)
            .ok_or_else(|| self.no_entry("id", old, None))?;
        if old != new && self.find(new).is_some() {
            return Err(RoomError::new(
                "to",
                format!("`{new}` is an entry's ID already"),
            ));
        }
        match entry {
            Entry::Shape(at) => {
                new.clone_into(&mut self.shapes[at].id);
                for object in &mut self.objects {
                    if object.on.as_deref() == Some(old) {
                        object.on = Some(new.to_owned());
                    }
                }
            }
            Entry::Object(at) => new.clone_into(&mut self.objects[at].id),
            Entry::Place(at) => {
                new.clone_into(&mut self.places[at].id);
                let (own_old, own_new) = (
                    format!("{}.{old}", context.room),
                    format!("{}.{new}", context.room),
                );
                for place in &mut self.places {
                    for stop in place.route.iter_mut().flatten() {
                        if matches!(stop, RouteStop::Place(id) if id == old) {
                            *stop = RouteStop::Place(new.to_owned());
                        }
                    }
                    if place.to.as_deref() == Some(own_old.as_str()) {
                        place.to = Some(own_new.clone());
                    }
                }
            }
        }
        let inverse = RoomCommand::Rename(RenameCommand {
            id: new.to_owned(),
            to: old.to_owned(),
        });
        Ok((vec![inverse], false))
    }
}

/// `move {id, to: at}`.
fn move_to(id: &str, at: [f32; 2]) -> RoomCommand {
    RoomCommand::Move(MoveCommand {
        id: id.to_owned(),
        by: None,
        to: Some(at),
    })
}

/// The `set` that puts `old` back at `set`'s path.
fn set_back(set: &SetCommand, old: Value) -> RoomCommand {
    RoomCommand::Set(SetCommand {
        id: set.id.clone(),
        layer: set.layer.clone(),
        path: set.path.clone(),
        value: old,
    })
}

/// An object's prefab, which the context's must hold.
fn check_prefab(context: &RoomContext<'_>, field: &str, prefab: &str) -> Result<(), RoomError> {
    if context.prefabs.binary_search(&prefab).is_ok() {
        return Ok(());
    }
    Err(RoomError::new(
        field,
        format!(
            "unknown prefab `{prefab}`{}",
            hint(prefab, context.prefabs.iter().copied())
        ),
    ))
}

/// A place's position: a point, or an area's top-left; `None` for a route.
fn place_anchor(place: &RoomPlace) -> Option<[f32; 2]> {
    place
        .point
        .or_else(|| place.rect.map(|[x, y, _, _]| [x, y]))
}

/// A place moved by `[dx, dy]`: a point, an area, or each of a route's
/// points.
fn shift_place(place: &mut RoomPlace, [dx, dy]: [f32; 2]) {
    if let Some([x, y]) = &mut place.point {
        *x += dx;
        *y += dy;
    }
    if let Some([x, y, _, _]) = &mut place.rect {
        *x += dx;
        *y += dy;
    }
    for stop in place.route.iter_mut().flatten() {
        if let RouteStop::Point([x, y]) = stop {
            *x += dx;
            *y += dy;
        }
    }
}

/// A shape's position: a rect's top-left, a polygon's or a line's first
/// point.
fn shape_anchor(shape: &RoomShape) -> [u32; 2] {
    if let Some([x, y, _, _]) = shape.rect {
        return [x, y];
    }
    shape
        .polygon
        .as_ref()
        .or(shape.line.as_ref())
        .and_then(|points| points.first().copied())
        .unwrap_or_default()
}

/// A shape moved by whole cells; an error when a point would leave the
/// room's top or left.
fn shift_shape(shape: &mut RoomShape, dx: i64, dy: i64) -> Result<(), String> {
    let shift = |value: &mut u32, by: i64| -> Result<(), String> {
        let moved = i64::from(*value) + by;
        *value = u32::try_from(moved)
            .map_err(|_| format!("the shape would leave the room ({moved} is past its edge)"))?;
        Ok(())
    };
    if let Some([x, y, _, _]) = &mut shape.rect {
        shift(x, dx)?;
        shift(y, dy)?;
    }
    for [x, y] in shape
        .polygon
        .iter_mut()
        .chain(shape.line.iter_mut())
        .flatten()
    {
        shift(x, dx)?;
        shift(y, dy)?;
    }
    Ok(())
}

/// Sets `value` at the dotted `path` of `model` through its JSON, `null`
/// removing the field; returns the old value (`null` when absent). With
/// `overrides`, `set.<key>` names one key of `set` whole.
fn edit<T: Serialize + serde::de::DeserializeOwned>(
    model: &mut T,
    path: &str,
    value: &Value,
    overrides: bool,
) -> Result<Value, RoomError> {
    let mut json = serde_json::to_value(&*model)
        .map_err(|e| RoomError::new("path", format!("the model does not write: {e}")))?;
    let segments: Vec<&str> = match path.strip_prefix("set.") {
        Some(key) if overrides && !key.is_empty() => vec!["set", key],
        _ => path.split('.').collect(),
    };
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(RoomError::new(
            "path",
            format!("`{path}` has an empty segment"),
        ));
    }
    let (last, parents) = segments.split_last().expect("split yields one");
    let mut target = &mut json;
    for segment in parents {
        let Value::Object(fields) = target else {
            return Err(RoomError::new(
                "path",
                format!("`{path}` steps into a value that is not an object"),
            ));
        };
        target = fields
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let Value::Object(fields) = target else {
        return Err(RoomError::new(
            "path",
            format!("`{path}` steps into a value that is not an object"),
        ));
    };
    let old = if value.is_null() {
        fields.remove(*last)
    } else {
        fields.insert((*last).to_owned(), value.clone())
    }
    .unwrap_or(Value::Null);
    *model = super::from_value(&json).map_err(|err| {
        RoomError::new(
            "value",
            if err.field.is_empty() {
                err.message
            } else {
                format!("{}: {}", err.field, err.message)
            },
        )
    })?;
    Ok(old)
}

/// One applied batch and the batch that undoes it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct JournalEntry {
    /// The batch as applied.
    pub batch: Vec<RoomCommand>,
    /// The batch that undoes it.
    pub inverse: Vec<RoomCommand>,
}

/// A room's history: the batches applied, oldest first, and those undone,
/// the next to redo last.
#[derive(Debug, Clone, Default)]
struct History {
    done: VecDeque<JournalEntry>,
    undone: Vec<JournalEntry>,
}

/// Each room's applied batches with their inverses, at most
/// [`RoomJournal::LIMIT`] a room, for undo and redo: a resource the level
/// step and an editor keep. A reload that changes a room's document clears
/// the room's history, whose inverses were written against the document
/// before it.
#[derive(Debug, Clone, Default)]
pub struct RoomJournal {
    rooms: BTreeMap<String, History>,
}

impl RoomJournal {
    /// The batches a room keeps; an older one drops.
    pub const LIMIT: usize = 64;

    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `batch`, applied to `room`, with its `inverse`; the redo
    /// list empties, and past [`LIMIT`](Self::LIMIT) the oldest drops.
    pub fn record(&mut self, room: &str, batch: Vec<RoomCommand>, inverse: Vec<RoomCommand>) {
        let history = self.rooms.entry(room.to_owned()).or_default();
        history.undone.clear();
        history.done.push_back(JournalEntry { batch, inverse });
        while history.done.len() > Self::LIMIT {
            history.done.pop_front();
        }
    }

    /// The batches `room` can undo.
    #[must_use]
    pub fn undo_len(&self, room: &str) -> usize {
        self.rooms.get(room).map_or(0, |history| history.done.len())
    }

    /// The batches `room` can redo.
    #[must_use]
    pub fn redo_len(&self, room: &str) -> usize {
        self.rooms
            .get(room)
            .map_or(0, |history| history.undone.len())
    }

    /// Forgets `room`'s history.
    pub fn clear(&mut self, room: &str) {
        self.rooms.remove(room);
    }

    /// Undoes `room`'s newest batch: its inverse applied to `document` as
    /// any batch is. `Ok(false)` when there is nothing to undo.
    ///
    /// # Errors
    ///
    /// The inverse's error, when it no longer applies; the document and the
    /// journal stay as they were.
    pub fn undo(
        &mut self,
        room: &str,
        document: &mut Room,
        context: &RoomContext<'_>,
    ) -> Result<bool, RoomCommandError> {
        let Some(history) = self.rooms.get_mut(room) else {
            return Ok(false);
        };
        let Some(entry) = history.done.back() else {
            return Ok(false);
        };
        document.apply(&entry.inverse, context)?;
        let entry = history.done.pop_back().expect("an entry to undo");
        history.undone.push(entry);
        Ok(true)
    }

    /// Redoes `room`'s newest undone batch, applied to `document` again.
    /// `Ok(false)` when there is nothing to redo.
    ///
    /// # Errors
    ///
    /// The batch's error, when it no longer applies; the document and the
    /// journal stay as they were.
    pub fn redo(
        &mut self,
        room: &str,
        document: &mut Room,
        context: &RoomContext<'_>,
    ) -> Result<bool, RoomCommandError> {
        let Some(history) = self.rooms.get_mut(room) else {
            return Ok(false);
        };
        let Some(entry) = history.undone.last() else {
            return Ok(false);
        };
        let inverse = document.apply(&entry.batch, context)?;
        let mut entry = history.undone.pop().expect("an entry to redo");
        entry.inverse = inverse;
        history.done.push_back(entry);
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../tests/assets/room_command.rs"]
mod tests;
