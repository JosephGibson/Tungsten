//! The level editor (redo 7.12): a debug view that edits the live rooms
//! through the room commands and writes each edit to the room's file.
//!
//! `engine_toggle_level_editor` (F10) opens it in a build that watches its
//! assets (`assets.hot_reload`, on by default in debug builds), since the
//! files it writes are then reloaded. Open, it stops the game clock
//! ([`Time::set_editing`], a stop of its own beside the game's pause and
//! the state stack's hold) and keeps a window on `UiLayer::Debug` whose
//! root is modal, so the game's `RoutedInput` gets no press; the editor
//! reads the raw `InputState` behind it, as a shell screen reads its
//! hotkeys. A press inside any shown debug window is never the world's.
//!
//! It draws each live room's bounds, a marker at every object's spawn point
//! (an object a generator made in its own hue) and its places through
//! `DebugDraw`, and a view of its own: the arrows pan a cell, a middle drag
//! pans, the wheel zooms about the cursor and Home goes back to the game's
//! view, written to `CameraState` after the frame's camera update so the
//! extract draws it. `engine_editor_primary` (the left button) picks an
//! object, boxed as the inspector boxes an entity or within 6 px of its
//! marker, and a drag moves it by whole cells, one `move` at the release;
//! Delete or Backspace deletes the pick, Ctrl+D copies it a cell right,
//! Ctrl+Z and Ctrl+Y (or Ctrl+Shift+Z) undo and redo the current room's
//! batches in the World's `RoomJournal`. The keys 1 to 4 pick the primary
//! button's tool: select, place (the palette's prefab at the cell's
//! top-left), paint (a stroke of the palette's tile, one batch at the
//! release, a `paint` per target layer) and erase (the same with `erase`);
//! `[` and `]` step the palette through the prefabs or, painting, the
//! room's tileset, and L the target layer through each layer, then all.
//! `engine_editor_secondary` (the right button) is the eyedropper: the
//! prefab of the object under the cursor, or the topmost tile at the cell
//! (the last of the room's layers holding one) and its layer. After each
//! edit the window shows the room's findings, core's level rules and,
//! with the kit, its routes', as `tungsten check` runs them.
//!
//! Every edit goes through `World::apply_room_batch` (or its undo or redo),
//! checked, applied whole, patched live and journaled. Before it, a room
//! with a file must read from that file as its live document, else the edit
//! is refused; after it, the document is written to the file canonically
//! with `write_atomic`, and the reload that follows reads an equal document
//! and keeps the journal. A write that fails puts the room and the journal
//! back as they were before the action. A generated room, and a room whose
//! file another listed room names, is read only; a room with no file is
//! edited live. It adds no system: [`update`] runs first in the debug
//! views' frame and returns at once while it is closed and its key is up.

use std::path::{Path, PathBuf};

use glam::Vec2;
use serde_json::json;
use tungsten_core::assets::{Room, RoomCommand, RoomJournal, RoomRegistry, TilemapInstance};
use tungsten_core::input::{ActionMap, InputState, KeyCode, Modifiers, MouseButton, RawInput};
use tungsten_core::save::{SaveId, SaveState};
use tungsten_core::text::TextAlign;
use tungsten_core::ui::{UiLayer, UiTree, Visibility};
use tungsten_core::{CameraState, DebugDraw, Entity, ScaleFactor, Time, World};

use crate::debug_hud::HudCorner;
use crate::debug_views::{
    BORDER, Cell, Column, Header, PANEL, Row, Shape, Tone, VALUE, ViewFrame, ViewSpec, ViewWindow,
    Width, look_of, toggle_key, write_table,
};

/// The editor's toggle, F10 by default.
pub(crate) const TOGGLE: &str = "engine_toggle_level_editor";
/// The editor's primary button: the tool's.
pub(crate) const PRIMARY: &str = "engine_editor_primary";
/// The editor's secondary button: the eyedropper.
pub(crate) const SECONDARY: &str = "engine_editor_secondary";

/// The window's columns: the field, then its value.
const COLUMNS: &[Column] = &[
    Column::new(TextAlign::Start, Width::Auto, Tone::Label),
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
];

/// How far from its marker a press picks an object, in screen pixels.
const PICK_RADIUS: f32 = 6.0;
/// A wheel notch's zoom factor, and the zoom's bounds.
const ZOOM_NOTCH: f32 = 1.25;
const ZOOM_MIN: f32 = 0.25;
const ZOOM_MAX: f32 = 8.0;
/// The arrows' pan without a current room: 16 px.
const PAN_STEP: f32 = 16.0;

/// The overlay's colours, linear RGBA.
const BOUNDS: [f32; 4] = [0.55, 0.55, 0.62, 0.9];
const AUTHORED: [f32; 4] = [0.3, 0.8, 0.95, 1.0];
const GENERATED: [f32; 4] = [0.72, 0.6, 1.0, 1.0];
const PLACE: [f32; 4] = [0.35, 0.85, 0.45, 0.9];
const SELECTED: [f32; 4] = [0.95, 0.85, 0.3, 1.0];
const PREVIEW: [f32; 4] = [0.96, 0.6, 0.25, 1.0];

/// How the editor writes a room file: [`write_atomic`] unless a test
/// replaces it.
pub(crate) type RoomWriter = fn(&Path, &str) -> Result<(), String>;

/// A room file written so a reader sees the old file or the new one, never
/// a torn one; the watcher skips its temp file.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    tungsten_core::persist::write_atomic(path, text.as_bytes()).map_err(|err| err.to_string())
}

/// The editor's view: its top-left and zoom, unrotated.
#[derive(Debug, Clone, Copy, PartialEq)]
struct View {
    position: Vec2,
    zoom: f32,
}

/// A drag under way: the object, where the press was in world pixels, its
/// room's cell, and the whole cells it has moved so far.
#[derive(Debug, Clone, PartialEq)]
struct Drag {
    room: String,
    id: String,
    start: Vec2,
    cell: Vec2,
    by: [f32; 2],
}

/// A middle drag under way: where it started on screen and the view's
/// top-left then.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pan {
    screen: Vec2,
    position: Vec2,
}

/// A live room as the editor sees it: its ID, its instance's top-left,
/// its cell and its size in pixels.
#[derive(Debug, Clone, PartialEq)]
struct Live {
    id: String,
    origin: Vec2,
    cell: Vec2,
    size: Vec2,
}

impl Live {
    fn holds(&self, at: Vec2) -> bool {
        let max = self.origin + self.size;
        at.x >= self.origin.x && at.y >= self.origin.y && at.x < max.x && at.y < max.y
    }
}

/// The primary button's tool, keys 1 to 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    /// Pick, drag to move.
    Select,
    /// Place the palette's prefab.
    Place,
    /// Paint the palette's tile.
    Paint,
    /// Erase cells to empty.
    Erase,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Place => "place",
            Self::Paint => "paint",
            Self::Erase => "erase",
        }
    }
}

/// A paint or erase stroke under way: its room and the cells it crossed,
/// in order, each once.
#[derive(Debug, Clone, PartialEq)]
struct Stroke {
    room: String,
    cells: Vec<[u32; 2]>,
    last: [i64; 2],
}

/// What an edit asks the World for.
#[derive(Debug, Clone)]
enum Edit {
    Apply(Vec<RoomCommand>),
    Undo,
    Redo,
}

/// The level editor's state: a crate-private resource `App::new` inserts.
pub(crate) struct LevelEditor {
    /// Whether this build can open it: `assets.hot_reload`'s.
    pub(crate) available: bool,
    pub(crate) open: bool,
    /// The corner the window docks at: the top right.
    pub(crate) corner: HudCorner,
    pub(crate) padding_px: f32,
    pub(crate) font_id: String,
    pub(crate) font_size: f32,
    pub(crate) line_height: f32,
    pub(crate) color: [u8; 4],
    pub(crate) background: [u8; 4],
    pub(crate) outline_color: [u8; 4],
    pub(crate) outline_px: f32,
    /// How a room file is written; tests replace it.
    pub(crate) writer: RoomWriter,
    /// The room last picked or edited.
    pub(crate) current: Option<String>,
    /// The picked object's ID, in the current room.
    pub(crate) selection: Option<String>,
    /// The last edit or error, the window's header.
    pub(crate) status: String,
    pub(crate) tool: Tool,
    /// The palette's prefab and tile, by ID and sprite; the first of each
    /// while unset.
    pub(crate) prefab: Option<String>,
    pub(crate) tile: Option<String>,
    /// The target layer, by name; every layer while unset.
    pub(crate) layer: Option<String>,
    /// The current room's findings after the last edit.
    pub(crate) findings: Vec<String>,
    stroke: Option<Stroke>,
    view: Option<View>,
    drag: Option<Drag>,
    pan: Option<Pan>,
    /// The game's camera, and the editor's last write over it.
    game_camera: Option<CameraState>,
    written: Option<CameraState>,
    /// The cursor's cell and its room, at the last frame.
    cursor: Option<(String, [i64; 2])>,
    pub(crate) window: Option<ViewWindow>,
    shown: Option<(String, Vec<Row>)>,
}

impl LevelEditor {
    /// A closed editor, `available` in a build that watches its assets.
    pub(crate) fn new(available: bool) -> Self {
        Self {
            available,
            open: false,
            corner: HudCorner::TopRight,
            padding_px: 8.0,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 15.0,
            line_height: 20.0,
            color: VALUE,
            background: PANEL,
            outline_color: BORDER,
            outline_px: 1.0,
            writer: write_atomic,
            current: None,
            selection: None,
            status: String::new(),
            tool: Tool::Select,
            prefab: None,
            tile: None,
            layer: None,
            findings: Vec::new(),
            stroke: None,
            view: None,
            drag: None,
            pan: None,
            game_camera: None,
            written: None,
            cursor: None,
            window: None,
            shown: None,
        }
    }

    fn open(&mut self, world: &mut World) {
        self.open = true;
        if let Some(time) = world.get_resource_mut::<Time>() {
            time.set_editing(true);
        }
        log::info!("Level editor open: the game clock stops until it closes");
    }

    fn close(&mut self, world: &mut World) {
        self.open = false;
        self.drag = None;
        self.stroke = None;
        self.pan = None;
        if let Some(time) = world.get_resource_mut::<Time>() {
            time.set_editing(false);
        }
        // The game's view back, should no camera update write over ours.
        if let (Some(game), Some(written)) = (self.game_camera.take(), self.written.take())
            && let Some(camera) = world.get_resource_mut::<CameraState>()
            && *camera == written
        {
            *camera = game;
        }
        self.view = None;
    }

    /// The camera the editor draws through this frame: its view, else the
    /// game's.
    fn drawn(&self, game: CameraState) -> CameraState {
        match self.view {
            Some(view) => CameraState {
                position: view.position,
                zoom: view.zoom,
                ..CameraState::new()
            },
            None => game,
        }
    }

    /// The view, made the editor's own from the game's when it has none.
    fn view_mut(&mut self, game: CameraState) -> &mut View {
        self.view.get_or_insert(View {
            position: game.position,
            zoom: game.zoom,
        })
    }

    /// The editor's frame while open, and the toggle's.
    fn frame(&mut self, world: &mut World, toggled: bool) {
        if toggled {
            if self.open {
                self.close(world);
                return;
            }
            self.open(world);
        }
        let Some(frame) = Input::read(world) else {
            return;
        };
        let game = world
            .get_resource::<CameraState>()
            .copied()
            .unwrap_or_default();
        if self.written != Some(game) {
            self.game_camera = Some(game);
        }
        let rooms = live_rooms(world, self.current.as_deref());
        let camera = self.drawn(game);
        let at = frame.cursor.map(|cursor| camera.screen_to_world(cursor));
        let blocked = frame
            .cursor
            .is_some_and(|cursor| over_debug_window(world, cursor));
        self.cursor = at.and_then(|at| {
            let room = rooms.iter().find(|room| room.holds(at))?;
            let cell = ((at - room.origin) / room.cell).floor();
            Some((room.id.clone(), [cell.x as i64, cell.y as i64]))
        });
        self.keys(world, &frame, &rooms, game);
        self.wheel_and_pan(&frame, blocked, game);
        if let Some(at) = at {
            match self.tool {
                Tool::Select => self.select(world, &frame, &rooms, camera, at, blocked),
                Tool::Place => self.place(world, &frame, &rooms, at, blocked),
                Tool::Paint | Tool::Erase => self.stroke(world, &frame, &rooms, at, blocked),
            }
            if frame.secondary.pressed && !blocked {
                self.eyedropper(world, &rooms, camera, at);
            }
        }
        self.draw(world, &rooms, self.drawn(game));
        if self.view.is_some() {
            let mine = self.drawn(game);
            if let Some(camera) = world.get_resource_mut::<CameraState>() {
                *camera = mine;
            }
            self.written = Some(mine);
        } else {
            self.written = None;
        }
    }

    /// The keys, raw while open: Home, the arrows, Delete and Backspace,
    /// Ctrl+D, Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z, 1 to 4, `[`, `]` and L.
    fn keys(&mut self, world: &mut World, frame: &Input, rooms: &[Live], game: CameraState) {
        for &(code, modifiers, repeat) in &frame.keys {
            let control = modifiers.control();
            let step = self
                .current
                .as_deref()
                .and_then(|current| rooms.iter().find(|room| room.id == current))
                .map_or(Vec2::splat(PAN_STEP), |room| room.cell);
            let pan = match code {
                KeyCode::ArrowLeft => Some(Vec2::new(-step.x, 0.0)),
                KeyCode::ArrowRight => Some(Vec2::new(step.x, 0.0)),
                KeyCode::ArrowUp => Some(Vec2::new(0.0, -step.y)),
                KeyCode::ArrowDown => Some(Vec2::new(0.0, step.y)),
                _ => None,
            };
            if let Some(pan) = pan {
                self.view_mut(game).position += pan;
                continue;
            }
            if repeat {
                continue;
            }
            match code {
                KeyCode::Home => self.view = None,
                KeyCode::Delete | KeyCode::Backspace if !control => self.delete(world),
                KeyCode::KeyD if control => self.duplicate(world),
                KeyCode::KeyZ if control && modifiers.shift() => self.history(world, Edit::Redo),
                KeyCode::KeyZ if control => self.history(world, Edit::Undo),
                KeyCode::KeyY if control => self.history(world, Edit::Redo),
                KeyCode::Digit1 if !control => self.tool = Tool::Select,
                KeyCode::Digit2 if !control => self.tool = Tool::Place,
                KeyCode::Digit3 if !control => self.tool = Tool::Paint,
                KeyCode::Digit4 if !control => self.tool = Tool::Erase,
                KeyCode::BracketLeft if !control => self.palette(world, rooms, -1),
                KeyCode::BracketRight if !control => self.palette(world, rooms, 1),
                KeyCode::KeyL if !control => self.next_layer(world),
                _ => {}
            }
        }
    }

    /// The wheel zooms about the cursor and a middle drag pans, outside the
    /// debug windows.
    fn wheel_and_pan(&mut self, frame: &Input, blocked: bool, game: CameraState) {
        let Some(cursor) = frame.cursor else {
            self.pan = None;
            return;
        };
        if frame.wheel != 0.0 && !blocked {
            let view = self.view_mut(game);
            let anchor = view.position + cursor / view.zoom;
            let zoom = (view.zoom * ZOOM_NOTCH.powf(frame.wheel)).clamp(ZOOM_MIN, ZOOM_MAX);
            view.zoom = zoom;
            view.position = anchor - cursor / zoom;
        }
        if frame.middle.pressed && !blocked {
            let position = self.view_mut(game).position;
            self.pan = Some(Pan {
                screen: cursor,
                position,
            });
        }
        match self.pan {
            Some(pan) if frame.middle.held => {
                let view = self.view_mut(game);
                view.position = pan.position - (cursor - pan.screen) / view.zoom;
            }
            _ => self.pan = None,
        }
    }

    /// Select's primary button: a press picks, or on nothing clears the
    /// pick; a drag moves the pick by whole cells, one `move` at the
    /// release.
    fn select(
        &mut self,
        world: &mut World,
        frame: &Input,
        rooms: &[Live],
        camera: CameraState,
        at: Vec2,
        blocked: bool,
    ) {
        if frame.primary.pressed && !blocked {
            if let Some((room, id)) = pick(world, rooms, camera, at) {
                let cell = rooms
                    .iter()
                    .find(|live| live.id == room)
                    .map_or(Vec2::ONE, |live| live.cell);
                self.current = Some(room.clone());
                self.selection = Some(id.clone());
                self.drag = Some(Drag {
                    room,
                    id,
                    start: at,
                    cell,
                    by: [0.0, 0.0],
                });
            } else {
                self.selection = None;
                if let Some(room) = rooms.iter().find(|room| room.holds(at)) {
                    self.current = Some(room.id.clone());
                }
            }
        }
        if let Some(drag) = &mut self.drag {
            let moved = ((at - drag.start) / drag.cell).round();
            drag.by = [moved.x, moved.y];
        }
        if frame.primary.released
            && let Some(drag) = self.drag.take()
            && drag.by != [0.0, 0.0]
        {
            let batch = parse(&format!(
                r#"[{{ "move": {{ "id": "{}", "by": [{}, {}] }} }}]"#,
                drag.id, drag.by[0], drag.by[1]
            ));
            let what = format!("moved {} by {}, {}", drag.id, drag.by[0], drag.by[1]);
            self.edit(world, &drag.room, Edit::Apply(batch), &what);
        }
        if !frame.primary.held && !frame.primary.released {
            self.drag = None;
        }
    }

    /// Place's primary button: a press places the palette's prefab at the
    /// cell under the cursor, `at` its top-left, as `<prefab>_<n>`, and
    /// picks it.
    fn place(&mut self, world: &mut World, frame: &Input, rooms: &[Live], at: Vec2, blocked: bool) {
        if !frame.primary.pressed || blocked {
            return;
        }
        let Some((room, [col, row])) = cell_under(rooms, at) else {
            "no live room under the cursor".clone_into(&mut self.status);
            return;
        };
        let Some(prefab) = self.prefab_or_first(world) else {
            "no prefab to place".clone_into(&mut self.status);
            return;
        };
        let Some(document) = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(&room))
        else {
            return;
        };
        let id = free_id(document, &id_of(&prefab));
        let batch = parse(
            &json!([{ "place": { "objects": { "id": id, "prefab": prefab, "at": [col, row] } } }])
                .to_string(),
        );
        self.current = Some(room.clone());
        if self.edit(
            world,
            &room,
            Edit::Apply(batch),
            &format!("placed {id} at {col}, {row}"),
        ) {
            self.selection = Some(id);
        }
    }

    /// Paint's and erase's primary button: a press starts a stroke in the
    /// room under the cursor, the cells it crosses gather while it is held,
    /// and the release applies them as one batch, a `paint` or an `erase`
    /// per target layer.
    fn stroke(
        &mut self,
        world: &mut World,
        frame: &Input,
        rooms: &[Live],
        at: Vec2,
        blocked: bool,
    ) {
        if frame.primary.pressed
            && !blocked
            && let Some((room, cell)) = cell_under(rooms, at)
        {
            self.current = Some(room.clone());
            self.stroke = Some(Stroke {
                room,
                cells: vec![cell],
                last: [i64::from(cell[0]), i64::from(cell[1])],
            });
        }
        if let Some(stroke) = &mut self.stroke
            && let Some(live) = rooms.iter().find(|live| live.id == stroke.room)
        {
            let cell = ((at - live.origin) / live.cell).floor();
            let to = [cell.x as i64, cell.y as i64];
            let size = [
                (live.size.x / live.cell.x) as i64,
                (live.size.y / live.cell.y) as i64,
            ];
            for [col, row] in line(stroke.last, to) {
                let inside = (0..size[0]).contains(&col) && (0..size[1]).contains(&row);
                let cell = [col as u32, row as u32];
                if inside && !stroke.cells.contains(&cell) {
                    stroke.cells.push(cell);
                }
            }
            stroke.last = to;
        }
        if frame.primary.released
            && let Some(stroke) = self.stroke.take()
        {
            self.apply_stroke(world, &stroke);
        }
        if !frame.primary.held && !frame.primary.released {
            self.stroke = None;
        }
    }

    fn apply_stroke(&mut self, world: &mut World, stroke: &Stroke) {
        let Some(document) = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(&stroke.room))
        else {
            return;
        };
        let layers: Vec<String> = match self.layer.as_deref() {
            Some(layer) if document.layers.iter().any(|known| known.name == layer) => {
                vec![layer.to_owned()]
            }
            _ => document
                .layers
                .iter()
                .map(|layer| layer.name.clone())
                .collect(),
        };
        let erase = self.tool == Tool::Erase;
        let tile = if erase {
            None
        } else {
            let Some(tile) = self.tile_or_first(world, &stroke.room) else {
                "no tile to paint: the room names no tileset".clone_into(&mut self.status);
                return;
            };
            Some(tile)
        };
        let commands: Vec<serde_json::Value> = layers
            .iter()
            .map(|layer| match &tile {
                None => json!({ "erase": { "layer": layer, "cells": stroke.cells } }),
                Some(tile) => {
                    json!({ "paint": { "layer": layer, "cells": stroke.cells, "tile": tile } })
                }
            })
            .collect();
        let cells = stroke.cells.len();
        let what = match &tile {
            None => format!("erased {cells} cell{}", plural(cells)),
            Some(tile) => format!("painted {cells} cell{} {tile}", plural(cells)),
        };
        let batch = parse(&serde_json::Value::Array(commands).to_string());
        self.edit(world, &stroke.room, Edit::Apply(batch), &what);
    }

    /// The eyedropper: the prefab of the object under the cursor, else the
    /// topmost tile at the cell and its layer.
    fn eyedropper(&mut self, world: &World, rooms: &[Live], camera: CameraState, at: Vec2) {
        let registry = world.get_resource::<RoomRegistry>();
        if let Some((room, id)) = pick(world, rooms, camera, at)
            && let Some(object) = registry
                .and_then(|registry| registry.room(&room))
                .and_then(|document| document.objects.iter().find(|object| object.id == id))
        {
            self.status = format!("took {} from {id}", object.prefab);
            self.prefab = Some(object.prefab.clone());
            return;
        }
        let Some((room, [col, row])) = cell_under(rooms, at) else {
            return;
        };
        let maps = world.get_resource::<tungsten_core::assets::TilemapRegistry>();
        let (Some(document), Some(map)) = (
            registry.and_then(|registry| registry.room(&room)),
            maps.and_then(|maps| maps.get(&room)),
        ) else {
            return;
        };
        let topmost = (0..document.layers.len()).rev().find_map(|layer| {
            let index = map.tile_at(layer, col, row)?;
            let sprite = usize::try_from(index)
                .ok()
                .and_then(|i| map.tileset.get(i))?;
            Some((sprite.clone(), document.layers[layer].name.clone()))
        });
        match topmost {
            Some((sprite, layer)) => {
                self.status = format!("took {sprite} from layer {layer}");
                self.tile = Some(sprite);
                self.layer = Some(layer);
            }
            None => self.status = format!("cell {col}, {row} holds no tile"),
        }
    }

    /// The palette's prefab, the first prefab while unset.
    fn prefab_or_first(&self, world: &World) -> Option<String> {
        self.prefab
            .clone()
            .or_else(|| prefab_ids(world).into_iter().next())
    }

    /// The palette's tile, the room's tileset's first while unset.
    fn tile_or_first(&self, world: &World, room: &str) -> Option<String> {
        self.tile
            .clone()
            .or_else(|| tile_ids(world, room).into_iter().next())
    }

    /// `[` and `]`: the palette one step back or on, through the tiles of
    /// the current room's tileset (the first live room's while none is
    /// current) while painting, else the prefabs.
    fn palette(&mut self, world: &World, rooms: &[Live], step: i64) {
        let painting = self.tool == Tool::Paint;
        let (items, chosen) = if painting {
            let room = (self.current.as_deref())
                .or_else(|| rooms.first().map(|room| room.id.as_str()))
                .unwrap_or_default();
            (tile_ids(world, room), &mut self.tile)
        } else {
            (prefab_ids(world), &mut self.prefab)
        };
        if items.is_empty() {
            return;
        }
        let at = chosen
            .as_deref()
            .and_then(|item| items.iter().position(|known| known == item))
            .unwrap_or(0) as i64;
        let count = items.len() as i64;
        let next = items[(at + step).rem_euclid(count) as usize].clone();
        *chosen = Some(next);
    }

    /// L: the target layer on through the current room's layers, then all.
    fn next_layer(&mut self, world: &World) {
        let names: Vec<String> = self
            .current
            .as_deref()
            .and_then(|room| world.get_resource::<RoomRegistry>()?.room(room))
            .map(|document| {
                document
                    .layers
                    .iter()
                    .map(|layer| layer.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        self.layer = match self.layer.as_deref() {
            None => names.first().cloned(),
            Some(layer) => names
                .iter()
                .position(|name| name == layer)
                .and_then(|at| names.get(at + 1))
                .cloned(),
        };
    }

    /// Deletes the pick.
    fn delete(&mut self, world: &mut World) {
        let (Some(room), Some(id)) = (self.current.clone(), self.selection.clone()) else {
            "nothing picked to delete".clone_into(&mut self.status);
            return;
        };
        let batch = parse(&format!(r#"[{{ "delete": {{ "id": "{id}" }} }}]"#));
        if self.edit(world, &room, Edit::Apply(batch), &format!("deleted {id}")) {
            self.selection = None;
        }
    }

    /// Copies the pick a cell right as `<stem>_<n>`, the lowest free `n`,
    /// and picks the copy.
    fn duplicate(&mut self, world: &mut World) {
        let (Some(room), Some(id)) = (self.current.clone(), self.selection.clone()) else {
            "nothing picked to copy".clone_into(&mut self.status);
            return;
        };
        let Some(document) = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(&room))
        else {
            return;
        };
        let copy = free_id(document, stem(&id));
        let batch = parse(&format!(
            r#"[{{ "duplicate": {{ "id": "{id}", "as": "{copy}", "by": [1, 0] }} }}]"#
        ));
        if self.edit(
            world,
            &room,
            Edit::Apply(batch),
            &format!("copied {id} as {copy}"),
        ) {
            self.selection = Some(copy);
        }
    }

    /// Undoes or redoes the current room's newest batch.
    fn history(&mut self, world: &mut World, edit: Edit) {
        let Some(room) = self.current.clone() else {
            "no room picked: click in one first".clone_into(&mut self.status);
            return;
        };
        let what = if matches!(edit, Edit::Undo) {
            "undid a batch"
        } else {
            "redid a batch"
        };
        self.edit(world, &room, edit, what);
    }

    /// Runs `edit` on `room` and writes the room's file: refused, nothing
    /// changed, for a read-only room or a file that does not hold the live
    /// document; put back whole when the write fails. Returns whether it
    /// landed; the window's last line says what happened.
    fn edit(&mut self, world: &mut World, room: &str, edit: Edit, what: &str) -> bool {
        match self.try_edit(world, room, edit, what) {
            Ok(done) => {
                self.status = done;
                self.findings = findings(world, room);
                true
            }
            Err(why) => {
                log::warn!("Level editor: {why}");
                self.status = why;
                false
            }
        }
    }

    fn try_edit(
        &mut self,
        world: &mut World,
        room: &str,
        edit: Edit,
        what: &str,
    ) -> Result<String, String> {
        let file = writable(world, room)?;
        let before = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(room))
            .cloned()
            .ok_or_else(|| format!("no room `{room}`"))?;
        let journal = world.get_resource::<RoomJournal>().cloned();
        let result = match edit {
            Edit::Apply(batch) => world.apply_room_batch(room, &batch).map(Some),
            Edit::Undo => world.undo_room_batch(room),
            Edit::Redo => world.redo_room_batch(room),
        };
        match result {
            Err(err) => return Err(err.to_string()),
            Ok(None) => {
                return Ok(if what.starts_with("undid") {
                    format!("{room}: nothing to undo")
                } else {
                    format!("{room}: nothing to redo")
                });
            }
            Ok(Some(_)) => {}
        }
        let Some(path) = file else {
            return Ok(format!("{what} (no file: kept live)"));
        };
        let text = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(room))
            .map(Room::to_canonical)
            .unwrap_or_default();
        let Err(err) = (self.writer)(&path, &text) else {
            return Ok(what.to_owned());
        };
        // Everything back as it was before the action: the room, then the
        // journal, which the patch cleared and the edit had moved.
        let restored = world.patch_room(room, before);
        match journal {
            Some(journal) => world.insert_resource(journal),
            None => {
                world.remove_resource::<RoomJournal>();
            }
        }
        let file = path.display();
        Err(match restored {
            Ok(_) => format!("cannot write '{file}': {err}; nothing edited"),
            Err(again) => format!(
                "cannot write '{file}': {err}; and the room did not go back ({again}): the live room and the file differ until the file reloads"
            ),
        })
    }

    /// The overlay: each live room's bounds, its objects' markers and its
    /// places; the pick's box and a drag's preview.
    fn draw(&self, world: &mut World, rooms: &[Live], camera: CameraState) {
        let mut draw = Vec::new();
        let picked = |room: &str, id: &str| {
            self.current.as_deref() == Some(room) && self.selection.as_deref() == Some(id)
        };
        let radius = 3.0 / camera.zoom.max(f32::EPSILON);
        if let Some(registry) = world.get_resource::<RoomRegistry>() {
            for live in rooms {
                let Some(document) = registry.room(&live.id) else {
                    continue;
                };
                draw.push(Mark::Aabb(live.origin, live.origin + live.size, BOUNDS));
                for object in &document.objects {
                    let marker = live.origin + Vec2::from(object.at) * live.cell;
                    let hue = if object.generator.is_some() {
                        GENERATED
                    } else {
                        AUTHORED
                    };
                    draw.push(Mark::Circle(marker, radius, hue));
                    if picked(&live.id, &object.id) {
                        let (min, size) = object_box(world, &live.id, &object.id, marker, camera);
                        draw.push(Mark::Aabb(min, min + size, SELECTED));
                        if let Some(drag) = &self.drag {
                            let by = Vec2::from(drag.by) * live.cell;
                            draw.push(Mark::Aabb(min + by, min + size + by, PREVIEW));
                        }
                    }
                }
                if let Some(stroke) = self.stroke.as_ref().filter(|stroke| stroke.room == live.id) {
                    for &[col, row] in &stroke.cells {
                        let min = live.origin + Vec2::new(col as f32, row as f32) * live.cell;
                        draw.push(Mark::Aabb(min, min + live.cell, PREVIEW));
                    }
                }
                if let Some(compiled) = registry.compiled(&live.id) {
                    for place in compiled.places.values() {
                        place_marks(&mut draw, live.origin, place, radius);
                    }
                }
            }
        }
        if let Some(debug) = world.get_resource_mut::<DebugDraw>() {
            for mark in draw {
                match mark {
                    Mark::Aabb(min, max, color) => debug.draw_aabb(min, max, color, 1.0),
                    Mark::Circle(centre, radius, color) => {
                        debug.draw_circle(centre, radius, color, 1.0);
                    }
                    Mark::Line(a, b, color) => debug.draw_line(a, b, color, 1.0),
                }
            }
        }
    }

    /// The window's frame (redo 7.12): built, kept modal and filled while
    /// open, removed when closed; its close button closes the editor.
    pub(crate) fn update_window(&mut self, world: &mut World, frame: &mut ViewFrame<'_>) {
        if frame.closes(self.window.as_ref()) {
            self.close(world);
        }
        let spec = ViewSpec {
            name: "level_editor",
            title: "Level editor",
            key: toggle_key(world, TOGGLE),
            enabled: self.open,
            corner: self.corner,
            padding: self.padding_px,
            strip: false,
            look: look_of!(self),
            shape: Shape {
                columns: COLUMNS,
                header: Header::Line,
                buttons: 0,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.shown = None;
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        frame.tree.set_modal(window.root, true);
        let content = self.rows(world);
        if self.shown.as_ref() == Some(&content) {
            return;
        }
        if let Some(line) = window.header {
            frame.tree.set_plain_text(line, &content.0);
        }
        write_table(frame.tree, window, &content.1);
        self.shown = Some(content);
    }

    /// The header line, the last edit or error, and the rows: the room, the
    /// tool, the cursor's cell, the pick, the history, then the keys.
    fn rows(&self, world: &World) -> (String, Vec<Row>) {
        let header = if self.status.is_empty() {
            "drag an object to move it".to_owned()
        } else {
            self.status.clone()
        };
        let registry = world.get_resource::<RoomRegistry>();
        let row = |label: &str, value: String| Row::data(vec![Cell::new(label), Cell::new(value)]);
        let mut rows = Vec::new();
        let room = self.current.as_deref();
        rows.push(row("room", room.map_or("–".to_owned(), str::to_owned)));
        let file = match (room, registry) {
            (Some(room), Some(registry)) => {
                match (registry.generation(room), registry.path(room)) {
                    (Some(generation), _) => format!("generated: {}", generation.generator),
                    (None, Some(path)) => path
                        .file_name()
                        .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
                    (None, None) => "no file".to_owned(),
                }
            }
            _ => "–".to_owned(),
        };
        rows.push(row("file", file));
        let key = match self.tool {
            Tool::Select => 1,
            Tool::Place => 2,
            Tool::Paint => 3,
            Tool::Erase => 4,
        };
        rows.push(row("tool", format!("{key} {}", self.tool.name())));
        let palette = if self.tool == Tool::Paint {
            room.and_then(|room| self.tile_or_first(world, room))
        } else {
            self.prefab_or_first(world)
        };
        rows.push(row("palette", palette.unwrap_or_else(|| "–".to_owned())));
        rows.push(row(
            "layer",
            self.layer.clone().unwrap_or_else(|| "all".to_owned()),
        ));
        let cell = self
            .cursor
            .as_ref()
            .map_or("–".to_owned(), |(room, [col, row])| {
                format!("{col}, {row}  {room}")
            });
        rows.push(row("cell", cell));
        let object = room
            .zip(self.selection.as_deref())
            .and_then(|(room, id)| registry?.room(room)?.objects.iter().find(|o| o.id == id));
        match object {
            Some(object) => {
                rows.push(row("pick", format!("{}  {}", object.id, object.prefab)));
                rows.push(row("at", format!("{}, {}", object.at[0], object.at[1])));
                rows.push(row(
                    "mark",
                    object.generator.clone().unwrap_or_else(|| "–".to_owned()),
                ));
            }
            None => rows.push(row("pick", "–".to_owned())),
        }
        let (undo, redo) = room
            .zip(world.get_resource::<RoomJournal>())
            .map_or((0, 0), |(room, journal)| {
                (journal.undo_len(room), journal.redo_len(room))
            });
        rows.push(row("history", format!("{undo} undo  {redo} redo")));
        rows.push(row("findings", self.findings.len().to_string()));
        for finding in self.findings.iter().take(2) {
            rows.push(Row::data(vec![
                Cell::new(""),
                Cell::new(finding.clone()).colored(crate::debug_views::YELLOW),
            ]));
        }
        rows.push(Row {
            cells: vec![Cell::new("keys")],
            header: true,
            indent: false,
        });
        for (key, does) in [
            ("1–4", "select place paint erase"),
            ("[ ]", "palette"),
            ("L", "layer, then all"),
            ("right", "eyedropper"),
            ("drag", "move by cells"),
            ("Del", "delete"),
            ("Ctrl+D", "copy a cell right"),
            ("Ctrl+Z", "undo"),
            ("Ctrl+Y", "redo"),
            ("arrows", "pan a cell"),
            ("middle", "drag to pan"),
            ("wheel", "zoom"),
            ("Home", "the game's view"),
        ] {
            rows.push(row(key, does.to_owned()));
        }
        (header, rows)
    }
}

/// One overlay mark, gathered before the `DebugDraw` is borrowed.
enum Mark {
    Aabb(Vec2, Vec2, [f32; 4]),
    Circle(Vec2, f32, [f32; 4]),
    Line(Vec2, Vec2, [f32; 4]),
}

/// A place's marks: a point's circle, an area's box, a route's lines.
fn place_marks(
    draw: &mut Vec<Mark>,
    origin: Vec2,
    place: &tungsten_core::assets::CompiledPlace,
    radius: f32,
) {
    use tungsten_core::assets::PlaceShape;
    match &place.shape {
        PlaceShape::Point(point) => draw.push(Mark::Circle(origin + *point, radius * 2.0, PLACE)),
        PlaceShape::Rect { min, size } => {
            draw.push(Mark::Aabb(origin + *min, origin + *min + *size, PLACE));
        }
        PlaceShape::Route(points) => {
            for pair in points.windows(2) {
                draw.push(Mark::Line(origin + pair[0], origin + pair[1], PLACE));
            }
        }
        _ => {}
    }
}

/// The frame's input, read from the raw `InputState`.
struct Input {
    cursor: Option<Vec2>,
    keys: Vec<(KeyCode, Modifiers, bool)>,
    wheel: f32,
    primary: Button,
    secondary: Button,
    middle: Button,
}

/// A button's frame: pressed, held and released.
#[derive(Debug, Clone, Copy)]
struct Button {
    pressed: bool,
    held: bool,
    released: bool,
}

impl Input {
    fn read(world: &World) -> Option<Self> {
        let input = world.get_resource::<InputState>()?;
        let actions = world.get_resource::<ActionMap>()?;
        let keys = input
            .raw_events()
            .iter()
            .filter_map(|event| match event.input {
                RawInput::Key {
                    code,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } => Some((code, modifiers, repeat)),
                _ => None,
            })
            .collect();
        let (_, lines) = input.scroll_line_delta();
        let (_, pixels) = input.scroll_pixel_delta();
        Some(Self {
            cursor: input.cursor_position().map(|(x, y)| Vec2::new(x, y)),
            keys,
            wheel: lines + pixels / 120.0,
            primary: Button {
                pressed: actions.just_pressed(input, PRIMARY),
                held: actions.is_pressed(input, PRIMARY),
                released: actions.just_released(input, PRIMARY),
            },
            secondary: Button {
                pressed: actions.just_pressed(input, SECONDARY),
                held: actions.is_pressed(input, SECONDARY),
                released: actions.just_released(input, SECONDARY),
            },
            middle: Button {
                pressed: input.mouse_just_pressed(MouseButton::Middle),
                held: input.is_mouse_pressed(MouseButton::Middle),
                released: input.mouse_just_released(MouseButton::Middle),
            },
        })
    }
}

/// The editor's frame (redo 7.12), first in the debug views' frame: at
/// once back while it is closed and its key is up, so a closed editor
/// reads two resources and one action and allocates nothing.
pub(crate) fn update(world: &mut World) {
    let Some(editor) = world.get_resource::<LevelEditor>() else {
        return;
    };
    if !editor.available {
        return;
    }
    let open = editor.open;
    let toggled = world
        .get_resource::<InputState>()
        .zip(world.get_resource::<ActionMap>())
        .is_some_and(|(input, actions)| actions.just_pressed(input, TOGGLE));
    if !open && !toggled {
        return;
    }
    let Some(mut editor) = world.remove_resource::<LevelEditor>() else {
        return;
    };
    editor.frame(world, toggled);
    world.insert_resource(editor);
}

/// Whether the editor shows or keeps a window: the views' frame then runs.
pub(crate) fn active(world: &World) -> bool {
    world
        .get_resource::<LevelEditor>()
        .is_some_and(|editor| editor.open || editor.window.is_some())
}

/// The live rooms: the registry's rooms whose instance lives, `current`
/// first, then by ID.
fn live_rooms(world: &World, current: Option<&str>) -> Vec<Live> {
    let Some(registry) = world.get_resource::<RoomRegistry>() else {
        return Vec::new();
    };
    let mut rooms: Vec<Live> = registry
        .ids()
        .filter_map(|id| {
            let instance = registry.live(id).filter(|&entity| world.is_alive(entity))?;
            let origin = world.get::<TilemapInstance>(instance)?.origin;
            let document = registry.room(id)?;
            let cell = Vec2::new(document.cell[0] as f32, document.cell[1] as f32);
            let size = Vec2::new(document.size[0] as f32, document.size[1] as f32) * cell;
            Some(Live {
                id: id.to_owned(),
                origin,
                cell,
                size,
            })
        })
        .collect();
    if let Some(at) = current.and_then(|current| rooms.iter().position(|room| room.id == current)) {
        let room = rooms.remove(at);
        rooms.insert(0, room);
    }
    rooms
}

/// The live entity room `room`'s object `id` spawned as, `<room>.<id>`.
fn object_entity(world: &World, room: &str, id: &str) -> Option<Entity> {
    world
        .get_resource::<SaveState>()?
        .named_entity(SaveId::authored(&format!("{room}.{id}")))
        .filter(|&entity| world.is_alive(entity))
}

/// The box an object is picked by: its entity's drawn box, as the
/// inspector boxes it, else the square of the pick radius about its
/// marker; `(min, size)` in world pixels.
fn object_box(
    world: &World,
    room: &str,
    id: &str,
    marker: Vec2,
    camera: CameraState,
) -> (Vec2, Vec2) {
    let radius = PICK_RADIUS / camera.zoom.max(f32::EPSILON);
    object_entity(world, room, id)
        .and_then(|entity| crate::inspector::drawn_box(world, entity, camera.position))
        .unwrap_or((marker - Vec2::splat(radius), Vec2::splat(radius * 2.0)))
}

/// The object under `at`: inside its box or within the pick radius of its
/// marker, the smallest box winning, as `(room, id)`.
fn pick(world: &World, rooms: &[Live], camera: CameraState, at: Vec2) -> Option<(String, String)> {
    let registry = world.get_resource::<RoomRegistry>()?;
    let radius = PICK_RADIUS / camera.zoom.max(f32::EPSILON);
    let mut best: Option<(f32, &str, &str)> = None;
    for live in rooms {
        let Some(document) = registry.room(&live.id) else {
            continue;
        };
        for object in &document.objects {
            let marker = live.origin + Vec2::from(object.at) * live.cell;
            let boxed = object_entity(world, &live.id, &object.id)
                .and_then(|entity| crate::inspector::drawn_box(world, entity, camera.position))
                .filter(|(min, size)| {
                    let max = *min + *size;
                    at.x >= min.x && at.y >= min.y && at.x <= max.x && at.y <= max.y
                })
                .map(|(_, size)| size.x * size.y);
            let near = (at.distance(marker) <= radius).then(|| (radius * 2.0).powi(2));
            let Some(area) = boxed.or(near) else {
                continue;
            };
            if best.is_none_or(|(smallest, ..)| area < smallest) {
                best = Some((area, &live.id, &object.id));
            }
        }
    }
    best.map(|(_, room, id)| (room.to_owned(), id.to_owned()))
}

/// Whether the cursor, in physical pixels, lies inside a shown debug
/// window: any visible, laid-out root on a debug layer, the editor's, one
/// opened over it, or one opened before it that its modal root blocks.
fn over_debug_window(world: &World, cursor: Vec2) -> bool {
    let Some(tree) = world.get_resource::<UiTree>() else {
        return false;
    };
    let scale = world
        .get_resource::<ScaleFactor>()
        .map_or(1.0, ScaleFactor::get)
        .max(f32::EPSILON);
    let point = cursor / scale;
    tree.roots().iter().any(|&root| {
        matches!(tree.layer(root), Some(UiLayer::Debug | UiLayer::DebugPopup))
            && tree.visibility(root) == Some(Visibility::Visible)
            && tree
                .visual_rect(root)
                .is_some_and(|rect| rect.contains(point))
    })
}

/// The file an edit of `room` writes, `None` for a room with no file; an
/// error for a read-only room or a file that does not hold its live
/// document.
fn writable(world: &World, room: &str) -> Result<Option<PathBuf>, String> {
    let registry = world
        .get_resource::<RoomRegistry>()
        .ok_or_else(|| format!("no room `{room}`"))?;
    let document = registry
        .room(room)
        .ok_or_else(|| format!("no room `{room}`"))?;
    if let Some(generation) = registry.generation(room) {
        return Err(format!(
            "`{room}` is a generated room ({generation}): read only; `tungsten level generate` writes rooms to edit"
        ));
    }
    let Some(path) = registry.path(room) else {
        return Ok(None);
    };
    if let Some(other) = registry
        .ids()
        .find(|other| *other != room && registry.path(other) == Some(path))
    {
        return Err(format!(
            "`{room}` shares its file with room `{other}`: read only, since writing one rewrites the other"
        ));
    }
    let file = path.display();
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("cannot read '{file}': {err}"))?;
    match Room::from_json(&text) {
        Ok(read) if read == *document => Ok(Some(path.to_path_buf())),
        Ok(_) => Err(format!(
            "'{file}' differs from the live `{room}` (a reload pending or failed, or an edit outside): nothing edited"
        )),
        Err(err) => Err(format!(
            "'{file}' does not read ({err}): nothing edited, so no work is written over"
        )),
    }
}

/// A batch the editor writes itself.
fn parse(json: &str) -> Vec<RoomCommand> {
    RoomCommand::parse_batch(json).expect("the editor writes batches that read")
}

/// The room under `at` and its cell there, the current room first.
fn cell_under(rooms: &[Live], at: Vec2) -> Option<(String, [u32; 2])> {
    let room = rooms.iter().find(|room| room.holds(at))?;
    let cell = ((at - room.origin) / room.cell).floor();
    Some((room.id.clone(), [cell.x as u32, cell.y as u32]))
}

/// The cells from `from` to `to`, both included, a step at a time.
fn line(from: [i64; 2], to: [i64; 2]) -> Vec<[i64; 2]> {
    let steps = (to[0] - from[0]).abs().max((to[1] - from[1]).abs());
    (0..=steps)
        .map(|i| {
            let t = if steps == 0 {
                0.0
            } else {
                i as f64 / steps as f64
            };
            let lerp = |a: i64, b: i64| a + ((b - a) as f64 * t).round() as i64;
            [lerp(from[0], to[0]), lerp(from[1], to[1])]
        })
        .collect()
}

/// The registered prefab IDs, sorted.
fn prefab_ids(world: &World) -> Vec<String> {
    let mut ids: Vec<String> = world
        .get_resource::<tungsten_core::assets::PrefabRegistry>()
        .map(|prefabs| prefabs.ids().map(str::to_owned).collect())
        .unwrap_or_default();
    ids.sort();
    ids
}

/// The tiles of room `room`'s tileset, by sprite, in its order.
fn tile_ids(world: &World, room: &str) -> Vec<String> {
    let Some(registry) = world.get_resource::<RoomRegistry>() else {
        return Vec::new();
    };
    registry
        .room(room)
        .and_then(|document| document.tileset.as_deref())
        .and_then(|tileset| registry.tileset(tileset))
        .map(|tileset| {
            tileset
                .tiles
                .iter()
                .map(|tile| tile.sprite.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Room `room`'s findings: core's level rules (the umbrella's `light`
/// lighting an object) and, with the kit, each route's reach, as
/// `tungsten check` runs them.
fn findings(world: &World, room: &str) -> Vec<String> {
    #[allow(unused_mut)] // Without the kit, core's alone.
    let mut found: Vec<String> = world
        .room_findings(room, &["light"])
        .iter()
        .map(ToString::to_string)
        .collect();
    #[cfg(feature = "kit")]
    found.extend(
        tungsten_kit::route_findings(world, room)
            .iter()
            .map(ToString::to_string),
    );
    found
}

/// A prefab ID made an ID, `[a-z][a-z0-9_]*`: lower case, any other
/// character `_`, `o_` before a first character that is no letter.
fn id_of(prefab: &str) -> String {
    let mut id: String = prefab
        .chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !id.starts_with(|c: char| c.is_ascii_lowercase()) {
        id.insert_str(0, "o_");
    }
    id
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `id` without a trailing `_<n>`: `crate` for `crate_1`.
fn stem(id: &str) -> &str {
    match id.rsplit_once('_') {
        Some((stem, n))
            if !stem.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) =>
        {
            stem
        }
        _ => id,
    }
}

/// `<stem>_<n>` for the lowest `n` from 1 no entry of `room` holds: one
/// past the entries' count at most.
fn free_id(room: &Room, stem: &str) -> String {
    let taken = |id: &str| {
        room.shapes.iter().any(|shape| shape.id == id)
            || room.objects.iter().any(|object| object.id == id)
            || room.places.iter().any(|place| place.id == id)
    };
    let entries = room.shapes.len() + room.objects.len() + room.places.len();
    (1..=entries + 1)
        .map(|n| format!("{stem}_{n}"))
        .find(|id| !taken(id))
        .expect("one of entries + 1 names is free")
}

#[cfg(test)]
#[path = "tests/level_editor.rs"]
mod tests;
