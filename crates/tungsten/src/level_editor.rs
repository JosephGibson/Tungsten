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
//! The window is a panel ([`panel`]) that shows what is in force and holds
//! a button for every action: the last result or refusal and what the
//! primary button will do under the cursor; the live rooms, the current one
//! marked; the tools; undo, redo, delete, copy and the view's buttons; the
//! palette, the prefabs for Place and the room's tiles for Paint, each
//! with its sprite; the target layer; the pick; the room's findings; and
//! the keys. Whatever a button does, a key and the `editor` script action
//! do through the same [`Action`], so nothing is reachable by a key alone.
//!
//! In the world ([`overlay`]) it draws what a click will act on: each live
//! room's bounds and ID, the current room's cell grid (G), a marker at
//! every object's spawn point in view (an object a generator made in its
//! own hue), its places, a box about the object under the cursor and about
//! the pick with their IDs, and a ghost of what the tool will do at the
//! cursor's cell. Its view is its own: the arrows pan a cell, a middle
//! drag pans, the wheel zooms about the cursor, F fits the current room
//! beside the panel and Home goes back to the game's view, written to
//! `CameraState` after the frame's camera update so the extract draws it.
//!
//! `engine_editor_primary` (the left button) is the tool's: Select picks an
//! object, boxed as the inspector boxes an entity or within 6 px of its
//! marker, and a drag moves it by whole cells, one `move` at the release;
//! Place puts the palette's prefab at the cell's top-left; Paint and Erase
//! gather a stroke of cells, or with Shift held at the press the rectangle
//! from the first cell to the cursor's, one batch at the release, a `paint`
//! or an `erase` per target layer; Pick, and Alt with the button in any
//! tool, is the eyedropper: the prefab of the object under the cursor, or
//! the topmost tile at the cell and its layer, and the tool that uses it.
//! `engine_editor_secondary` (the right button) erases: a stroke of cells
//! under Paint and Erase, the object under the cursor under the others.
//! Escape drops a drag or a stroke under way, else the pick. Mark places a
//! place: a click a point, a drag an area. Select picks a place as it
//! picks an object, a point by its marker and an area by its outline, and
//! moves, copies and deletes it.
//!
//! What has no gesture goes through a one-line prompt ([`prompt`]): Rename
//! (R; F1 to F11 are the engine's) gives the pick a new ID, Set an object's override as
//! `path = value`, Link a point or an area as `room.place`. Its text shows
//! in the panel's hint; while it is open the keys type and nothing else
//! acts.
//!
//! The panel lists every registered room. One the game has not spawned
//! opens at a click: the editor spawns it right of the live rooms, under
//! the top state, and puts it away with `World::despawn_room` when it
//! closes, with the `PostStack` and `AmbientLight` the room's spawn may
//! have set. It opens no room whose area the World has loaded or a restore
//! kept, since that is a run's progress and a spawn would write over it,
//! and never puts away a room the game spawned. The run's last frame
//! starts by closing the editor ([`close_for_exit`]), so a save taken in
//! it holds none of the editor's rooms.
//!
//! Every edit goes through `World::apply_room_batch` (or its undo or redo),
//! checked, applied whole, patched live and journaled. Before it, a room
//! with a file must read from that file as its live document, else the edit
//! is refused; after it, the document is written to the file canonically
//! with `write_atomic`, and the reload that follows reads an equal document
//! and keeps the journal. A write that fails puts the room and the journal
//! back as they were before the action. A generated room, and a room whose
//! file another listed room names, is read only; a room with no file is
//! edited live. A refusal reads as a sentence, the edit in the editor's
//! words and the reason without the batch's path. It adds no system:
//! [`update`] runs first in the debug views' frame and returns at once
//! while it is closed and its key is up.

use std::path::{Path, PathBuf};

use glam::Vec2;
use serde_json::json;
use tungsten_core::assets::{
    PlaceShape, Room, RoomCommand, RoomJournal, RoomRegistry, TilemapInstance,
};
use tungsten_core::input::{ActionMap, InputState, KeyCode, Modifiers, MouseButton, RawInput};
use tungsten_core::save::{SaveId, SaveState};
use tungsten_core::ui::{UiLayer, UiTree, Visibility};
use tungsten_core::{AmbientLight, CameraState, Entity, PostStack, ScaleFactor, Time, World};

use crate::app::WindowSize;
use crate::debug_hud::HudCorner;
use crate::debug_views::{BORDER, PANEL, VALUE, ViewFrame, ViewWindow};
use crate::state::{SceneEntity, StateId, StateStack};

mod hooks;
mod overlay;
mod panel;
mod prompt;

pub(crate) use hooks::register as register_hooks;
pub(crate) use prompt::{Ask, Prompt};

/// The editor's toggle, F10 by default.
pub(crate) const TOGGLE: &str = "engine_toggle_level_editor";
/// The editor's primary button: the tool's.
pub(crate) const PRIMARY: &str = "engine_editor_primary";
/// The editor's secondary button: it erases.
pub(crate) const SECONDARY: &str = "engine_editor_secondary";

/// How far from its marker a press picks an object, in screen pixels.
const PICK_RADIUS: f32 = 6.0;
/// A wheel notch's zoom factor, and the zoom's bounds.
const ZOOM_NOTCH: f32 = 1.25;
const ZOOM_MIN: f32 = 0.25;
const ZOOM_MAX: f32 = 8.0;
/// The arrows' pan without a current room: 16 px.
const PAN_STEP: f32 = 16.0;
/// The room's margin in a fitted view, in screen pixels a side.
const FIT_MARGIN: f32 = 24.0;
/// The most characters a prompt holds.
const PROMPT_LIMIT: usize = 96;
/// The state an opened room's entities belong to while the stack is empty.
const EDITOR_STATE: StateId = "engine_level_editor";
/// How far from the cursor, in world pixels an axis, an object's spawn
/// point may lie and its box still be tested for a pick: what keeps a room
/// of thousands from a lookup an object a frame.
const PICK_REACH: f32 = 1024.0;

/// How the editor writes a room file: [`write_atomic`] unless a test
/// replaces it.
pub(crate) type RoomWriter = fn(&Path, &str) -> Result<(), String>;

/// A room file written so a reader sees the old file or the new one, never
/// a torn one; the watcher skips its temp file.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    // The cause alone: the error's own text names the file, which the
    // editor's sentence already does.
    tungsten_core::persist::write_atomic(path, text.as_bytes()).map_err(|err| {
        std::error::Error::source(&err).map_or_else(|| err.to_string(), ToString::to_string)
    })
}

/// A room file as a sentence names it: from the game's folder when it is
/// under it, as the manifest and the command line write it, else whole.
fn shown(path: &Path) -> String {
    let under = std::env::current_dir()
        .ok()
        .and_then(|dir| path.strip_prefix(dir).ok().map(Path::to_path_buf));
    under.as_deref().unwrap_or(path).display().to_string()
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

/// The primary button's tool, keys 1 to 6.
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
    /// Take the prefab or the tile under the cursor: the eyedropper.
    Pick,
    /// Place a place: a click a point, a drag an area.
    Mark,
}

impl Tool {
    /// Every tool, in its key's order.
    pub(crate) const ALL: [Self; 6] = [
        Self::Select,
        Self::Place,
        Self::Paint,
        Self::Erase,
        Self::Pick,
        Self::Mark,
    ];

    /// Its name in a script and a probe.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Place => "place",
            Self::Paint => "paint",
            Self::Erase => "erase",
            Self::Pick => "pick",
            Self::Mark => "mark",
        }
    }

    /// Its button's caption: its key, then its name.
    pub(crate) fn caption(self) -> &'static str {
        match self {
            Self::Select => "1 Select",
            Self::Place => "2 Place",
            Self::Paint => "3 Paint",
            Self::Erase => "4 Erase",
            Self::Pick => "5 Pick",
            Self::Mark => "6 Mark",
        }
    }

    /// The tool `name` names.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }
}

/// Something the editor is asked to do. A key, a button of its panel and
/// the `editor` script action each make one, and [`LevelEditor::act`] is
/// the one place they are carried out, so every action a key has a button
/// has too.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// Takes up a tool.
    Tool(Tool),
    /// Undoes the current room's newest batch.
    Undo,
    /// Redoes its newest undone batch.
    Redo,
    /// Deletes the pick.
    Delete,
    /// Copies the pick a cell right.
    Copy,
    /// Fits the current room into the view beside the panel.
    Fit,
    /// Goes back to the game's view.
    GameView,
    /// Shows or hides the current room's cell grid.
    Grid,
    /// Opens a prompt for the pick: Rename, Set or Link.
    Prompt(Ask),
    /// Makes a live room the current one and fits it; opens a registered
    /// room that is not live.
    Room(String),
    /// Takes a prefab into the palette and takes up Place.
    Prefab(String),
    /// Takes a tile into the palette and takes up Paint.
    Tile(String),
    /// Sets the target layer; every layer for `None`.
    Layer(Option<String>),
    /// Steps the palette on or back.
    PaletteStep(i64),
    /// Steps the target layer on through the layers, then all.
    LayerStep,
    /// Picks an entry of the current room by its ID; nothing for `None`.
    Select(Option<String>),
}

/// A paint or erase stroke under way: its room and the cells it crossed,
/// in order, each once; or, for a rectangle, its first cell and the
/// cursor's.
#[derive(Debug, Clone, PartialEq)]
struct Stroke {
    room: String,
    cells: Vec<[u32; 2]>,
    first: [i64; 2],
    last: [i64; 2],
    /// Shift was held at the press: the rectangle from `first` to `last`.
    rect: bool,
    /// It erases: the Erase tool's, or the secondary button's.
    erase: bool,
    /// The secondary button holds it.
    secondary: bool,
}

impl Stroke {
    /// A rectangle stroke's cells as a batch writes them: its top-left
    /// cell, then its width and height.
    fn bounds(&self) -> [u32; 4] {
        let min = [
            self.first[0].min(self.last[0]),
            self.first[1].min(self.last[1]),
        ];
        let max = [
            self.first[0].max(self.last[0]),
            self.first[1].max(self.last[1]),
        ];
        [
            min[0].max(0) as u32,
            min[1].max(0) as u32,
            (max[0] - min[0] + 1) as u32,
            (max[1] - min[1] + 1) as u32,
        ]
    }

    /// What it covers, in a sentence: "3 cells", "a 4 × 2 rectangle".
    fn extent(&self) -> String {
        if self.rect {
            let [_, _, width, height] = self.bounds();
            format!("a {width} \u{d7} {height} rectangle")
        } else {
            let cells = self.cells.len();
            format!("{cells} cell{}", plural(cells))
        }
    }
}

/// What an edit asks the World for.
#[derive(Debug, Clone)]
enum Edit {
    Apply(Vec<RoomCommand>),
    Undo,
    Redo,
}

/// The level editor's state: a crate-private resource `App::new` inserts.
#[allow(clippy::struct_excessive_bools)] // Independent switches: the build's, the window's, the status', the grid's, the eyedropper's.
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
    /// The room last picked or edited; the live room nearest the view
    /// while none is.
    pub(crate) current: Option<String>,
    /// The picked object's ID, in the current room.
    pub(crate) selection: Option<String>,
    /// The last edit or refusal, a sentence: the panel's first lines.
    pub(crate) status: String,
    /// Whether `status` is a refusal.
    pub(crate) refused: bool,
    /// What the primary button will do under the cursor, and where the
    /// cursor is: the panel's second lines.
    pub(crate) hint: String,
    pub(crate) tool: Tool,
    /// The palette's prefab and tile, by ID and sprite; the first of each
    /// while unset.
    pub(crate) prefab: Option<String>,
    pub(crate) tile: Option<String>,
    /// The target layer, by name; every layer while unset.
    pub(crate) layer: Option<String>,
    /// The current room's findings after the last edit.
    pub(crate) findings: Vec<String>,
    /// The rooms the editor opened itself, which closing puts away.
    pub(crate) opened: Vec<String>,
    /// The World's `PostStack` and `AmbientLight` before the first room it
    /// opened, their absence included: what closing puts back.
    look: Option<(Option<PostStack>, Option<AmbientLight>)>,
    /// Whether the current room's cell grid is drawn.
    pub(crate) grid: bool,
    /// Whether the primary button is the eyedropper this frame: the Pick
    /// tool, or Alt held.
    picking: bool,
    /// The prompt under way: its text is the keys' until it ends.
    pub(crate) prompt: Option<Prompt>,
    /// A mark under way: the rectangle from its first cell to the
    /// cursor's, a point place while it is one cell.
    mark: Option<Stroke>,
    stroke: Option<Stroke>,
    view: Option<View>,
    drag: Option<Drag>,
    pan: Option<Pan>,
    /// The game's camera, and the editor's last write over it.
    game_camera: Option<CameraState>,
    written: Option<CameraState>,
    /// The cursor's cell and its room, at the last frame.
    cursor: Option<(String, [i64; 2])>,
    /// The object under the cursor at the last frame, as `(room, id)`.
    hover: Option<(String, String)>,
    /// Whether the cursor is over a debug window: `cursor` and `hover` are
    /// then where it last was in the world.
    over_window: bool,
    /// The game camera's rotation when the editor opened.
    rotation: Option<f32>,
    /// A fit made before the panel's box was known: made again when it is.
    pub(crate) refit: bool,
    pub(crate) window: Option<ViewWindow>,
    /// The window's contents, built once a window.
    panel: Option<panel::Panel>,
    /// The window's box on screen at its last update, in physical pixels:
    /// what a fitted view keeps clear of.
    panel_rect: Option<(Vec2, Vec2)>,
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
            refused: false,
            hint: String::new(),
            tool: Tool::Select,
            prefab: None,
            tile: None,
            layer: None,
            findings: Vec::new(),
            opened: Vec::new(),
            look: None,
            grid: true,
            picking: false,
            prompt: None,
            mark: None,
            stroke: None,
            view: None,
            drag: None,
            pan: None,
            game_camera: None,
            written: None,
            cursor: None,
            hover: None,
            over_window: false,
            rotation: None,
            refit: false,
            window: None,
            panel: None,
            panel_rect: None,
        }
    }

    pub(crate) fn open(&mut self, world: &mut World) {
        if self.open || !self.available {
            return;
        }
        self.open = true;
        self.rotation = world
            .get_resource::<CameraState>()
            .map(|camera| camera.rotation);
        if let Some(time) = world.get_resource_mut::<Time>() {
            time.set_editing(true);
        }
        log::info!("Level editor open: the game clock stops until it closes");
    }

    pub(crate) fn close(&mut self, world: &mut World) {
        if !self.open {
            return;
        }
        self.open = false;
        self.drag = None;
        self.stroke = None;
        self.mark = None;
        self.prompt = None;
        self.pan = None;
        self.hover = None;
        if let Some(time) = world.get_resource_mut::<Time>() {
            time.set_editing(false);
        }
        // The rooms it opened go, and what the first of them found in the
        // World's post stack and ambient light comes back.
        for id in std::mem::take(&mut self.opened) {
            if let Err(err) = world.despawn_room(&id) {
                log::warn!("Level editor: room `{id}` was not put away: {err}");
            }
        }
        if let Some((post, ambient)) = self.look.take() {
            match post {
                Some(post) => world.insert_resource(post),
                None => drop(world.remove_resource::<PostStack>()),
            }
            match ambient {
                Some(ambient) => world.insert_resource(ambient),
                None => drop(world.remove_resource::<AmbientLight>()),
            }
        }
        // The game's view back, should no camera update write over ours.
        if let (Some(game), Some(written)) = (self.game_camera.take(), self.written.take())
            && let Some(camera) = world.get_resource_mut::<CameraState>()
            && *camera == written
        {
            *camera = game;
        }
        self.restore_rotation(world, true);
        self.view = None;
        self.refit = false;
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

    /// The game's camera as the editor last saw it, else the World's.
    fn game_view(&self, world: &World) -> CameraState {
        self.game_camera.unwrap_or_else(|| {
            world
                .get_resource::<CameraState>()
                .copied()
                .unwrap_or_default()
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
        let camera = self.drawn(game);
        self.keep_current(world, camera);
        let rooms = live_rooms(world, self.current.as_deref());
        let at = frame.cursor.map(|cursor| camera.screen_to_world(cursor));
        let blocked = frame
            .cursor
            .is_some_and(|cursor| over_debug_window(world, cursor));
        // Over a debug window the cursor's cell and the object under it
        // stay what they last were in the world, so the hint does not
        // change, and the panel's buttons do not move, as the cursor comes
        // onto the panel for one of them. The overlay draws neither then.
        self.over_window = blocked;
        if !blocked {
            self.cursor = at.and_then(|at| {
                let room = rooms.iter().find(|room| room.holds(at))?;
                let cell = ((at - room.origin) / room.cell).floor();
                Some((room.id.clone(), [cell.x as i64, cell.y as i64]))
            });
            self.hover = at.and_then(|at| pick(world, &rooms, camera, at));
        }
        self.keys(world, &frame, &rooms, game);
        self.wheel_and_pan(&frame, blocked, game);
        // Alt is read at a press: a gesture under way stays its tool's and
        // ends as it would have.
        let gesture = self.drag.is_some() || self.stroke.is_some() || self.mark.is_some();
        self.picking = (self.tool == Tool::Pick || frame.alt) && !gesture;
        // An open prompt has the keys; the world waits for it to end.
        if let Some(at) = at.filter(|_| self.prompt.is_none()) {
            // Read before the eyedropper takes up a tool, so the press that
            // took a tile starts no stroke.
            let picking = self.picking;
            let strokes = matches!(self.tool, Tool::Paint | Tool::Erase);
            if picking {
                if frame.primary.pressed && !blocked {
                    self.eyedropper(world, &rooms, camera, at);
                }
            } else {
                match self.tool {
                    Tool::Select => self.select(world, &frame, &rooms, camera, at, blocked),
                    Tool::Place => self.place(world, &frame, &rooms, at, blocked),
                    Tool::Mark => self.mark(world, &frame, &rooms, at, blocked),
                    Tool::Paint | Tool::Erase | Tool::Pick => {}
                }
            }
            if strokes {
                self.stroke(world, &frame, &rooms, at, blocked, !picking);
            } else if frame.secondary.pressed
                && !blocked
                && let Some((room, id)) = pick(world, &rooms, camera, at)
            {
                self.set_current(&room);
                self.delete_object(world, &room, &id);
            }
        }
        self.hint = match &self.prompt {
            Some(prompt) => prompt.line(),
            None => self.hint_of(world, &rooms),
        };
        overlay::draw(self, world, &rooms, self.drawn(game));
        if self.view.is_some() {
            let mine = self.drawn(game);
            if let Some(camera) = world.get_resource_mut::<CameraState>() {
                *camera = mine;
            }
            self.written = Some(mine);
        } else if self.written.take().is_some() {
            // Back to the game's view: its rotation with it.
            self.restore_rotation(world, false);
        }
    }

    /// Gives the game's camera the rotation it had when the editor opened.
    /// The editor's own view is unrotated, and the camera update keeps
    /// whatever rotation `CameraState` holds, so a view written over it
    /// would leave the game's camera level for good.
    fn restore_rotation(&mut self, world: &mut World, closing: bool) {
        let rotation = if closing {
            self.rotation.take()
        } else {
            self.rotation
        };
        if let (Some(rotation), Some(camera)) = (rotation, world.get_resource_mut::<CameraState>())
        {
            camera.rotation = rotation;
        }
    }

    /// Keeps `current` a live room: the one picked while it lives, else
    /// the live room that holds the view's centre, else the nearest to it,
    /// so a room is current from the frame the editor opens.
    fn keep_current(&mut self, world: &World, camera: CameraState) {
        let rooms = live_rooms(world, None);
        let alive = |id: &str| rooms.iter().any(|room| room.id == id);
        if self.current.as_deref().is_some_and(alive) {
            return;
        }
        let window = world
            .get_resource::<WindowSize>()
            .map_or(Vec2::new(1280.0, 720.0), |size| {
                Vec2::new(size.width as f32, size.height as f32)
            });
        let centre = camera.screen_to_world(window * 0.5);
        let nearest = rooms
            .iter()
            .map(|room| {
                let middle = room.origin + room.size * 0.5;
                let away = if room.holds(centre) {
                    -1.0
                } else {
                    middle.distance_squared(centre)
                };
                (away, &room.id)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        self.current = nearest.map(|(_, id)| id.clone());
        self.selection = None;
        self.findings.clear();
    }

    /// The keys, raw while open: the arrows pan, and each other one is an
    /// [`Action`] a button does too.
    fn keys(&mut self, world: &mut World, frame: &Input, rooms: &[Live], game: CameraState) {
        for &(code, modifiers, repeat) in &frame.keys {
            let control = modifiers.control();
            if self.prompt.is_some() {
                // Every press is the prompt's, a held key's repeats too.
                if !control {
                    self.type_key(world, code, modifiers.shift());
                }
                continue;
            }
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
            if code == KeyCode::Escape {
                self.cancel();
                continue;
            }
            if let Some(action) = key_action(code, control, modifiers.shift()) {
                self.act(world, action);
            }
        }
    }

    /// Escape: drops a drag or a stroke under way, nothing edited; else the
    /// pick. A gesture has no button, so this is the one key without one.
    fn cancel(&mut self) {
        if self.drag.take().is_some() {
            self.say("Dropped the move: nothing changed.".to_owned());
        } else if self.stroke.take().is_some() {
            self.say("Dropped the stroke: nothing changed.".to_owned());
        } else if self.mark.take().is_some() {
            self.say("Dropped the mark: nothing changed.".to_owned());
        } else {
            self.selection = None;
        }
    }

    /// A key press while a prompt is open: a character, Backspace, Enter
    /// or Escape.
    fn type_key(&mut self, world: &mut World, code: KeyCode, shift: bool) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        match prompt::typed(code, shift) {
            prompt::Typed::Char(c) => {
                if prompt.text.chars().count() < PROMPT_LIMIT {
                    prompt.text.push(c);
                }
            }
            prompt::Typed::Backspace => {
                prompt.text.pop();
            }
            prompt::Typed::Commit => self.commit_prompt(world),
            prompt::Typed::Cancel => {
                self.prompt = None;
                self.say("Left as it was.".to_owned());
            }
            prompt::Typed::Nothing => {}
        }
    }

    /// Opens `ask` for the pick, its text what the pick holds now: its ID
    /// for Rename, its link for Link, nothing for Set. Refused for a pick
    /// the prompt does not fit.
    fn ask(&mut self, world: &World, ask: Ask) {
        // One open before this one ends here, whether or not this opens.
        self.prompt = None;
        let picked = (self.current.clone())
            .zip(self.selection.clone())
            .zip(self.current_room(world));
        let Some(((room, id), document)) = picked else {
            self.refuse(
                match ask {
                    Ask::Rename => {
                        "Nothing is picked to rename: click an object or a place with Select."
                    }
                    Ask::Set => {
                        "Nothing is picked to set an override on: click an object with Select."
                    }
                    Ask::Link => "Nothing is picked to link: click a point or an area with Select.",
                }
                .to_owned(),
            );
            return;
        };
        let place = document.places.iter().find(|place| place.id == id);
        let text = match (ask, place) {
            (Ask::Rename, _) => id.clone(),
            (Ask::Set, None) => String::new(),
            (Ask::Set, Some(_)) => {
                self.refuse(format!(
                    "`{id}` is a place: Set writes an object's override."
                ));
                return;
            }
            (Ask::Link, Some(place)) if place.route.is_none() => {
                place.to.clone().unwrap_or_default()
            }
            (Ask::Link, Some(_)) => {
                self.refuse(format!(
                    "`{id}` is a route: Link joins a point or an area to another room's place."
                ));
                return;
            }
            (Ask::Link, None) => {
                self.refuse(format!(
                    "`{id}` is an object: Link joins a point or an area to another room's place."
                ));
                return;
            }
        };
        self.drag = None;
        self.prompt = Some(Prompt {
            ask,
            room,
            id,
            text,
        });
    }

    /// Enter: the prompt's text as one command. It lands and the prompt
    /// ends, or it is refused and the prompt stays open with its text, the
    /// status saying why.
    pub(crate) fn commit_prompt(&mut self, world: &mut World) {
        let Some(Prompt {
            ask,
            room,
            id,
            text,
        }) = self.prompt.clone()
        else {
            return;
        };
        let text = text.trim();
        let (command, what, done) = match ask {
            Ask::Rename if text == id => {
                self.prompt = None;
                self.say(format!("`{id}` keeps its name."));
                return;
            }
            Ask::Rename => (
                json!({ "rename": { "id": id, "to": text } }),
                format!("rename `{id}` to `{text}`"),
                format!("Renamed `{id}` to `{text}`."),
            ),
            Ask::Set => {
                let Some((path, value)) = text.split_once('=') else {
                    self.refuse(
                        "Can't set: write it as path = value, as `health = 5`. An empty value takes the override off."
                            .to_owned(),
                    );
                    return;
                };
                let (path, value) = (path.trim(), value.trim());
                // JSON where it reads as JSON, so `5`, `true` and `[1, 2]`
                // are what they look like; anything else is text.
                let value = if value.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_str(value)
                        .unwrap_or_else(|_| serde_json::Value::String(value.to_owned()))
                };
                let done = if value.is_null() {
                    format!("Took `{path}` off `{id}`.")
                } else {
                    format!("Set `{path}` on `{id}` to {value}.")
                };
                (
                    json!({ "set": { "id": id, "path": format!("set.{path}"), "value": value } }),
                    format!("set `{path}` on `{id}`"),
                    done,
                )
            }
            Ask::Link if text.is_empty() => (
                json!({ "link": { "id": id, "to": null } }),
                format!("unlink `{id}`"),
                format!("Unlinked `{id}`."),
            ),
            Ask::Link => (
                json!({ "link": { "id": id, "to": text } }),
                format!("link `{id}` to `{text}`"),
                format!("Linked `{id}` to `{text}`."),
            ),
        };
        let batch = match RoomCommand::parse_batch(&json!([command]).to_string()) {
            Ok(batch) => batch,
            Err(err) => {
                self.refuse(format!("Can't {what}: {}.", err.reason));
                return;
            }
        };
        if self.edit(world, &room, Edit::Apply(batch), &what, &done) {
            self.prompt = None;
            if ask == Ask::Rename {
                self.selection = Some(text.to_owned());
            }
        }
    }

    /// Carries out `action`: the one place a key, a panel button and the
    /// `editor` script action meet.
    pub(crate) fn act(&mut self, world: &mut World, action: Action) {
        // A button pressed while a prompt is open ends it, text dropped.
        if !matches!(action, Action::Prompt(_)) {
            self.prompt = None;
        }
        match action {
            Action::Tool(tool) => self.take_up(tool),
            Action::Prompt(ask) => self.ask(world, ask),
            Action::Undo => self.history(world, Edit::Undo),
            Action::Redo => self.history(world, Edit::Redo),
            Action::Delete => self.delete(world),
            Action::Copy => self.duplicate(world),
            Action::Fit => self.fit(world),
            Action::GameView => self.view = None,
            Action::Grid => self.grid = !self.grid,
            Action::Room(id) => {
                let registered = world
                    .get_resource::<RoomRegistry>()
                    .is_some_and(|rooms| rooms.room(&id).is_some());
                if live_rooms(world, None).iter().any(|room| room.id == id) {
                    self.current = Some(id);
                    self.selection = None;
                    self.findings.clear();
                    self.fit(world);
                } else if registered {
                    self.open_room(world, id);
                } else {
                    let ids: Vec<&str> = world
                        .get_resource::<RoomRegistry>()
                        .map(|rooms| rooms.ids().collect())
                        .unwrap_or_default();
                    self.refuse(if ids.is_empty() {
                        format!("No room `{id}`: the game lists none.")
                    } else {
                        format!("No room `{id}`: the game lists {}.", ids.join(", "))
                    });
                }
            }
            Action::Prefab(id) => {
                if prefab_ids(world).contains(&id) {
                    self.prefab = Some(id);
                    self.take_up(Tool::Place);
                } else {
                    self.refuse(format!("No prefab `{id}`."));
                }
            }
            Action::Tile(sprite) => {
                let room = self.current.clone().unwrap_or_default();
                if tile_ids(world, &room).contains(&sprite) {
                    self.tile = Some(sprite);
                    self.take_up(Tool::Paint);
                } else {
                    self.refuse(format!("Room `{room}`'s tileset has no tile `{sprite}`."));
                }
            }
            Action::Layer(layer) => {
                let known = layer.as_deref().is_none_or(|name| {
                    self.current_room(world)
                        .is_some_and(|room| room.layers.iter().any(|layer| layer.name == name))
                });
                if known {
                    self.layer = layer;
                } else {
                    self.refuse(format!(
                        "The room has no layer `{}`.",
                        layer.unwrap_or_default()
                    ));
                }
            }
            Action::PaletteStep(step) => self.palette(world, step),
            Action::LayerStep => self.next_layer(world),
            Action::Select(id) => {
                let known = id.as_deref().is_none_or(|id| {
                    self.current_room(world).is_some_and(|room| {
                        room.objects.iter().any(|object| object.id == id)
                            || room.places.iter().any(|place| place.id == id)
                    })
                });
                if known {
                    self.selection = id;
                } else {
                    self.refuse(format!(
                        "The room has no object or place `{}`.",
                        id.unwrap_or_default()
                    ));
                }
            }
        }
    }

    /// Opens registered room `id`, which is not live: spawned two cells
    /// right of the live rooms under the top state, made current and
    /// fitted. Refused, nothing spawned, for a room whose area holds a
    /// run's progress ([`why_closed`]).
    fn open_room(&mut self, world: &mut World, id: String) {
        if let Some(why) = why_closed(world, &id) {
            self.refuse(format!("Can't open `{id}`: {why}"));
            return;
        }
        let cell = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(&id))
            .map_or(Vec2::splat(PAN_STEP), |room| {
                Vec2::new(room.cell[0] as f32, room.cell[1] as f32)
            });
        let live = live_rooms(world, None);
        let origin = if live.is_empty() {
            Vec2::ZERO
        } else {
            let right = (live.iter())
                .map(|room| room.origin.x + room.size.x)
                .fold(f32::MIN, f32::max);
            let top = (live.iter())
                .map(|room| room.origin.y)
                .fold(f32::MAX, f32::min);
            Vec2::new(right + 2.0 * cell.x, top)
        };
        let look = (
            world.get_resource::<PostStack>().cloned(),
            world.get_resource::<AmbientLight>().copied(),
        );
        let owner = world
            .get_resource::<StateStack>()
            .and_then(StateStack::active_id)
            .unwrap_or(EDITOR_STATE);
        match world.spawn_room(&id, origin, &(SceneEntity { state_id: owner },)) {
            Ok(_) => {
                self.look.get_or_insert(look);
                self.opened.push(id.clone());
                self.say(format!(
                    "Opened `{id}` beside the live rooms. Closing the editor puts it away."
                ));
                self.current = Some(id);
                self.selection = None;
                self.findings.clear();
                self.fit(world);
            }
            Err(err) => self.refuse(format!("Can't open `{id}`: {err}.")),
        }
    }

    /// Takes up `tool`, dropping a gesture the last one had under way: a
    /// tool's key, a palette row and the eyedropper all come here, so no
    /// drag, stroke or mark outlives its tool.
    fn take_up(&mut self, tool: Tool) {
        self.tool = tool;
        self.drag = None;
        self.stroke = None;
        self.mark = None;
    }

    /// The current room's document.
    fn current_room<'a>(&self, world: &'a World) -> Option<&'a Room> {
        world
            .get_resource::<RoomRegistry>()?
            .room(self.current.as_deref()?)
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

    /// Fits the current room into the part of the window the panel leaves:
    /// the wider side of it, at the largest zoom that shows the whole room,
    /// a whole number of screen pixels a world pixel once that is 1 or
    /// more, the room centred there.
    pub(crate) fn fit(&mut self, world: &World) {
        let Some(room) = live_rooms(world, self.current.as_deref())
            .into_iter()
            .find(|room| Some(room.id.as_str()) == self.current.as_deref())
        else {
            self.refuse("No room to fit: none is live.".to_owned());
            return;
        };
        let window = world
            .get_resource::<WindowSize>()
            .map_or(Vec2::new(1280.0, 720.0), |size| {
                Vec2::new(size.width as f32, size.height as f32)
            });
        // The free strip left or right of the panel, whichever is wider.
        let (left, right) = match self.panel_rect {
            Some((min, max)) if min.x > window.x - max.x => (0.0, min.x.clamp(0.0, window.x)),
            Some((_, max)) => (max.x.clamp(0.0, window.x), window.x),
            None => (0.0, window.x),
        };
        let free = Vec2::new(right - left, window.y) - Vec2::splat(2.0 * FIT_MARGIN);
        if free.x <= 0.0 || free.y <= 0.0 || room.size.x <= 0.0 || room.size.y <= 0.0 {
            return;
        }
        let mut zoom = (free.x / room.size.x).min(free.y / room.size.y);
        if zoom >= 1.0 {
            zoom = zoom.floor();
        }
        let zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        let centre = Vec2::new(f32::midpoint(left, right), window.y * 0.5);
        self.view = Some(View {
            position: room.origin + room.size * 0.5 - centre / zoom,
            zoom,
        });
        // Asked before the panel was laid out, as a script's first step
        // asks: fitted again beside it once its box is known.
        self.refit = self.panel_rect.is_none();
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
                self.set_current(&room);
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
                    self.set_current(&room.id);
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
            let id = &drag.id;
            let by = format!("{}, {}", drag.by[0], drag.by[1]);
            self.edit(
                world,
                &drag.room,
                Edit::Apply(batch),
                &format!("move `{id}`"),
                &format!("Moved `{id}` by {by}."),
            );
        }
        if !frame.primary.held && !frame.primary.released {
            self.drag = None;
        }
    }

    /// Makes `room` the current one, its findings forgotten when it
    /// changes.
    fn set_current(&mut self, room: &str) {
        if self.current.as_deref() != Some(room) {
            self.current = Some(room.to_owned());
            self.findings.clear();
            // The pick is an ID of the room left: the next room may hold
            // the same ID, which Delete, Copy and the prompts would act on.
            self.selection = None;
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
            self.refuse("Can't place here: no live room is under the cursor.".to_owned());
            return;
        };
        let Some(prefab) = self.prefab_or_first(world) else {
            self.refuse("Can't place: the game lists no prefab.".to_owned());
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
        self.set_current(&room);
        if self.edit(
            world,
            &room,
            Edit::Apply(batch),
            &format!("place `{prefab}` at {col}, {row}"),
            &format!("Placed `{id}` at {col}, {row}."),
        ) {
            self.selection = Some(id);
        }
    }

    /// Paint's and Erase's buttons: a press starts a stroke in the room
    /// under the cursor, the primary button's the tool's (while `primary`,
    /// which the eyedropper turns off) and the secondary's an erasing one.
    /// The cells it crosses gather while its button is held, or with Shift
    /// held at the press it is the rectangle from its first cell to the
    /// cursor's, kept inside the room; the release applies it as one batch,
    /// a `paint` or an `erase` per target layer.
    fn stroke(
        &mut self,
        world: &mut World,
        frame: &Input,
        rooms: &[Live],
        at: Vec2,
        blocked: bool,
        primary: bool,
    ) {
        let pressed = if primary && frame.primary.pressed {
            Some(false)
        } else if frame.secondary.pressed {
            Some(true)
        } else {
            None
        };
        if self.stroke.is_none()
            && !blocked
            && let Some(secondary) = pressed
            && let Some((room, cell)) = cell_under(rooms, at)
        {
            self.set_current(&room);
            let first = [i64::from(cell[0]), i64::from(cell[1])];
            self.stroke = Some(Stroke {
                room,
                cells: vec![cell],
                first,
                last: first,
                rect: frame.shift,
                erase: secondary || self.tool == Tool::Erase,
                secondary,
            });
        }
        let Some(stroke) = &mut self.stroke else {
            return;
        };
        if let Some(live) = rooms.iter().find(|live| live.id == stroke.room) {
            let cell = ((at - live.origin) / live.cell).floor();
            let to = [cell.x as i64, cell.y as i64];
            let size = [
                (live.size.x / live.cell.x) as i64,
                (live.size.y / live.cell.y) as i64,
            ];
            if stroke.rect {
                stroke.last = [
                    to[0].clamp(0, (size[0] - 1).max(0)),
                    to[1].clamp(0, (size[1] - 1).max(0)),
                ];
            } else {
                for [col, row] in line(stroke.last, to) {
                    let inside = (0..size[0]).contains(&col) && (0..size[1]).contains(&row);
                    let cell = [col as u32, row as u32];
                    if inside && !stroke.cells.contains(&cell) {
                        stroke.cells.push(cell);
                    }
                }
                stroke.last = to;
            }
        }
        let button = if stroke.secondary {
            frame.secondary
        } else {
            frame.primary
        };
        if button.released {
            if let Some(stroke) = self.stroke.take() {
                self.apply_stroke(world, &stroke);
            }
        } else if !button.held {
            self.stroke = None;
        }
    }

    /// Mark's primary button: a press starts a mark at the cell under the
    /// cursor and the release places it, a point place where it never left
    /// its cell, else the area from the first cell to the last, kept inside
    /// the room.
    fn mark(&mut self, world: &mut World, frame: &Input, rooms: &[Live], at: Vec2, blocked: bool) {
        if frame.primary.pressed
            && !blocked
            && let Some((room, cell)) = cell_under(rooms, at)
        {
            self.set_current(&room);
            let first = [i64::from(cell[0]), i64::from(cell[1])];
            self.mark = Some(Stroke {
                room,
                cells: Vec::new(),
                first,
                last: first,
                rect: true,
                erase: false,
                secondary: false,
            });
        }
        let Some(mark) = &mut self.mark else {
            return;
        };
        if let Some(live) = rooms.iter().find(|live| live.id == mark.room) {
            let cell = ((at - live.origin) / live.cell).floor();
            let size = [
                (live.size.x / live.cell.x) as i64,
                (live.size.y / live.cell.y) as i64,
            ];
            mark.last = [
                (cell.x as i64).clamp(0, (size[0] - 1).max(0)),
                (cell.y as i64).clamp(0, (size[1] - 1).max(0)),
            ];
        }
        if frame.primary.released {
            if let Some(mark) = self.mark.take() {
                self.apply_mark(world, &mark);
            }
        } else if !frame.primary.held {
            self.mark = None;
        }
    }

    /// Places `mark` as `point_<n>` or `area_<n>`, the lowest free `n`,
    /// and picks it, so Rename and Link act on it next.
    fn apply_mark(&mut self, world: &mut World, mark: &Stroke) {
        let Some(document) = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(&mark.room))
        else {
            return;
        };
        let [col, row, width, height] = mark.bounds();
        let (id, entry, what, done) = if width == 1 && height == 1 {
            let id = free_id(document, "point");
            (
                id.clone(),
                json!({ "id": id, "point": [col, row] }),
                format!("mark a point at {col}, {row}"),
                format!("Marked point `{id}` at {col}, {row}. Rename names it."),
            )
        } else {
            let id = free_id(document, "area");
            (
                id.clone(),
                json!({ "id": id, "rect": [col, row, width, height] }),
                format!("mark a {width} \u{d7} {height} area"),
                format!(
                    "Marked area `{id}`, {width} \u{d7} {height} at {col}, {row}. Rename names it."
                ),
            )
        };
        let batch = parse(&json!([{ "place": { "places": entry } }]).to_string());
        if self.edit(world, &mark.room, Edit::Apply(batch), &what, &done) {
            self.selection = Some(id);
        }
    }

    /// The layers a stroke writes: the target layer while the room has it,
    /// else every layer.
    fn target_layers(&self, document: &Room) -> Vec<String> {
        match self.layer.as_deref() {
            Some(layer) if document.layers.iter().any(|known| known.name == layer) => {
                vec![layer.to_owned()]
            }
            _ => document
                .layers
                .iter()
                .map(|layer| layer.name.clone())
                .collect(),
        }
    }

    fn apply_stroke(&mut self, world: &mut World, stroke: &Stroke) {
        let Some(document) = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(&stroke.room))
        else {
            return;
        };
        let layers = self.target_layers(document);
        let erase = stroke.erase;
        if layers.is_empty() {
            self.refuse(format!(
                "Can't {}: room `{}` has no layer.",
                if erase { "erase" } else { "paint" },
                stroke.room
            ));
            return;
        }
        let tile = if erase {
            None
        } else {
            let Some(tile) = self.tile_or_first(world, &stroke.room) else {
                self.refuse(format!(
                    "Can't paint: room `{}`'s tileset has no tile.",
                    stroke.room
                ));
                return;
            };
            Some(tile)
        };
        // A rectangle is the batch's `rect`, so its undo step and its file
        // diff are the rectangle, not its cells.
        let (shape, area) = if stroke.rect {
            ("rect", json!(stroke.bounds()))
        } else {
            ("cells", json!(stroke.cells))
        };
        let commands: Vec<serde_json::Value> = layers
            .iter()
            .map(|layer| match &tile {
                None => json!({ "erase": { "layer": layer, shape: area } }),
                Some(tile) => json!({ "paint": { "layer": layer, shape: area, "tile": tile } }),
            })
            .collect();
        let cells = stroke.extent();
        let into = layers.join(" and ");
        let (what, done) = match &tile {
            None => (
                format!("erase {cells}"),
                format!("Erased {cells} of {into}."),
            ),
            Some(tile) => (
                format!("paint {cells}"),
                format!("Painted {cells} of {into} with `{tile}`."),
            ),
        };
        let batch = parse(&serde_json::Value::Array(commands).to_string());
        self.edit(world, &stroke.room, Edit::Apply(batch), &what, &done);
    }

    /// The eyedropper: the prefab of the object under the cursor and Place,
    /// else the topmost tile the room's author put at the cell, its layer
    /// and Paint.
    fn eyedropper(&mut self, world: &World, rooms: &[Live], camera: CameraState, at: Vec2) {
        let registry = world.get_resource::<RoomRegistry>();
        if let Some((room, id)) = pick(world, rooms, camera, at)
            && let Some(object) = registry
                .and_then(|registry| registry.room(&room))
                .and_then(|document| document.objects.iter().find(|object| object.id == id))
        {
            self.say(format!(
                "Took prefab `{}` from `{id}`: Place puts it down.",
                object.prefab
            ));
            self.prefab = Some(object.prefab.clone());
            self.take_up(Tool::Place);
            return;
        }
        let Some((room, [col, row])) = cell_under(rooms, at) else {
            self.say("Nothing to take here: the cursor is over no room.".to_owned());
            return;
        };
        // The room's map as compiled, not the live one: a cell reads the
        // tile its author wrote, where the live map holds the variant the
        // auto-tiler picked for its neighbours and whatever play left.
        let Some((document, map)) = registry
            .and_then(|registry| Some((registry.room(&room)?, &registry.compiled(&room)?.map)))
        else {
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
                self.say(format!(
                    "Took tile `{sprite}` and layer `{layer}`: Paint writes there."
                ));
                self.tile = Some(sprite);
                self.layer = Some(layer);
                self.take_up(Tool::Paint);
            }
            None => self.say(format!("Cell {col}, {row} holds no tile to take.")),
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
        let tiles = tile_ids(world, room);
        self.tile
            .clone()
            .filter(|tile| tiles.contains(tile))
            .or_else(|| tiles.into_iter().next())
    }

    /// `[` and `]`: the palette one step back or on, through what the panel
    /// shows: the tiles of the current room's tileset under Paint and
    /// Erase, else the prefabs.
    fn palette(&mut self, world: &World, step: i64) {
        let painting = matches!(self.tool, Tool::Paint | Tool::Erase);
        let (items, chosen) = if painting {
            let room = self.current.clone().unwrap_or_default();
            (tile_ids(world, &room), &mut self.tile)
        } else {
            (prefab_ids(world), &mut self.prefab)
        };
        if items.is_empty() {
            return;
        }
        // With nothing chosen the first shows, and the step is from it.
        let at = chosen
            .as_deref()
            .and_then(|item| items.iter().position(|known| known == item))
            .unwrap_or(0) as i64;
        let count = items.len() as i64;
        *chosen = Some(items[(at + step).rem_euclid(count) as usize].clone());
    }

    /// L: the target layer on through the current room's layers, then all.
    fn next_layer(&mut self, world: &World) {
        let names: Vec<String> = self
            .current_room(world)
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
            self.refuse("Nothing is picked to delete: click an object with Select.".to_owned());
            return;
        };
        self.delete_object(world, &room, &id);
    }

    /// Deletes `room`'s object `id`: the pick's Delete, and the secondary
    /// button's on the object under the cursor.
    fn delete_object(&mut self, world: &mut World, room: &str, id: &str) {
        let batch = parse(&json!([{ "delete": { "id": id } }]).to_string());
        if self.edit(
            world,
            room,
            Edit::Apply(batch),
            &format!("delete `{id}`"),
            &format!("Deleted `{id}`."),
        ) && self.selection.as_deref() == Some(id)
        {
            self.selection = None;
            self.drag = None;
        }
    }

    /// Copies the pick a cell right as `<stem>_<n>`, the lowest free `n`,
    /// and picks the copy.
    fn duplicate(&mut self, world: &mut World) {
        let (Some(room), Some(id)) = (self.current.clone(), self.selection.clone()) else {
            self.refuse("Nothing is picked to copy: click an object with Select.".to_owned());
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
            &format!("copy `{id}`"),
            &format!("Copied `{id}` as `{copy}`, a cell right."),
        ) {
            self.selection = Some(copy);
        }
    }

    /// Undoes or redoes the current room's newest batch.
    fn history(&mut self, world: &mut World, edit: Edit) {
        let undo = matches!(edit, Edit::Undo);
        let Some(room) = self.current.clone() else {
            self.refuse(format!(
                "Nothing to {}: no room is live.",
                if undo { "undo" } else { "redo" }
            ));
            return;
        };
        let (what, done) = if undo {
            ("undo", "Undid the last edit.")
        } else {
            ("redo", "Redid the edit.")
        };
        self.edit(world, &room, edit, what, done);
    }

    /// Shows `text` as the last result.
    fn say(&mut self, text: String) {
        self.status = text;
        self.refused = false;
    }

    /// Shows `text` as a refusal, and logs it: the panel keeps three lines
    /// of it and the log console the whole.
    fn refuse(&mut self, text: String) {
        log::warn!("Level editor: {text}");
        self.status = text;
        self.refused = true;
    }

    /// Runs `edit` on `room` and writes the room's file: refused, nothing
    /// changed, for a read-only room or a file that does not hold the live
    /// document; put back whole when the write fails. `what` names the edit
    /// in a refusal ("Can't `what`: …") and `done` is the sentence shown
    /// when it lands. Returns whether it landed.
    fn edit(&mut self, world: &mut World, room: &str, edit: Edit, what: &str, done: &str) -> bool {
        match self.try_edit(world, room, edit) {
            Ok(Landed::Written) => {
                self.say(done.to_owned());
                self.findings = findings(world, room);
                true
            }
            Ok(Landed::Live) => {
                self.say(format!(
                    "{done} The room has no file, so the edit stays in the running game."
                ));
                self.findings = findings(world, room);
                true
            }
            Ok(Landed::Nothing) => {
                self.say(format!("Nothing to {what} in `{room}`."));
                false
            }
            Err(why) => {
                self.refuse(format!("Can't {what}: {why}"));
                false
            }
        }
    }

    fn try_edit(&mut self, world: &mut World, room: &str, edit: Edit) -> Result<Landed, String> {
        let file = writable(world, room)?;
        let before = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(room))
            .cloned()
            .ok_or_else(|| format!("no room `{room}`."))?;
        let journal = world.get_resource::<RoomJournal>().cloned();
        let result = match edit {
            Edit::Apply(batch) => world.apply_room_batch(room, &batch).map(Some),
            Edit::Undo => world.undo_room_batch(room),
            Edit::Redo => world.redo_room_batch(room),
        };
        match result {
            // The reason alone: the batch is the editor's own, so its path
            // tells an author nothing.
            Err(err) => return Err(format!("{}.", err.reason())),
            Ok(None) => return Ok(Landed::Nothing),
            Ok(Some(_)) => {}
        }
        let Some(path) = file else {
            return Ok(Landed::Live);
        };
        let text = world
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.room(room))
            .map(Room::to_canonical)
            .unwrap_or_default();
        let Err(err) = (self.writer)(&path, &text) else {
            return Ok(Landed::Written);
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
        let file = shown(&path);
        Err(match restored {
            Ok(_) => format!("'{file}' cannot be written ({err}); nothing edited."),
            Err(again) => format!(
                "'{file}' cannot be written ({err}), and the room did not go back ({again}): the live room and the file differ until the file reloads."
            ),
        })
    }

    /// What the primary button will do under the cursor, then where the
    /// cursor is: the panel's hint.
    fn hint_of(&self, world: &World, rooms: &[Live]) -> String {
        if rooms.is_empty() {
            return no_live_room(world);
        }
        let read_only = self
            .current
            .as_deref()
            .and_then(|room| read_only(world, room));
        if let Some(why) = read_only {
            return why;
        }
        let does = match self.tool {
            _ if self.picking => {
                "Click an object: take its prefab. Click a cell: take its tile and layer."
                    .to_owned()
            }
            Tool::Select => match (&self.drag, &self.hover) {
                (Some(drag), _) if drag.by != [0.0, 0.0] => format!(
                    "Release: move `{}` by {}, {}. Esc drops it.",
                    drag.id, drag.by[0], drag.by[1]
                ),
                (_, Some((_, id))) => {
                    format!("Click: pick `{id}`. Drag: move it by cells. Right click: delete it.")
                }
                _ => "Click an object or a place to pick it, then drag it.".to_owned(),
            },
            Tool::Place => match (self.prefab_or_first(world), &self.cursor) {
                (Some(prefab), Some((_, [col, row]))) => {
                    format!("Click: place `{prefab}` at {col}, {row}.")
                }
                (Some(prefab), None) => format!("Click in a room: place `{prefab}`."),
                (None, _) => "The game lists no prefab to place.".to_owned(),
            },
            Tool::Mark => match (self.mark.as_ref().map(Stroke::bounds), &self.cursor) {
                (Some([col, row, 1, 1]), _) => {
                    format!("Release: mark a point at {col}, {row}. Esc drops it.")
                }
                (Some([_, _, width, height]), _) => {
                    format!("Release: mark a {width} \u{d7} {height} area. Esc drops it.")
                }
                (None, Some((_, [col, row]))) => {
                    format!("Click: mark a point at {col}, {row}. Drag: mark an area.")
                }
                (None, None) => "Click in a room: mark a point. Drag: mark an area.".to_owned(),
            },
            Tool::Paint | Tool::Erase | Tool::Pick => {
                let into = (self.current_room(world))
                    .map_or_else(String::new, |room| self.target_layers(room).join(" and "));
                let room = self.current.clone().unwrap_or_default();
                // A stroke under way says what its release does; the
                // secondary button's erases under Paint too.
                let erase =
                    (self.stroke.as_ref()).map_or(self.tool == Tool::Erase, |stroke| stroke.erase);
                let extent = self.stroke.as_ref().map(Stroke::extent);
                match (erase, extent, self.tile_or_first(world, &room)) {
                    (true, Some(extent), _) => {
                        format!("Release: erase {extent} of {into}. Esc drops it.")
                    }
                    (true, None, _) => {
                        format!("Drag: erase cells of {into}. Shift+drag: a rectangle.")
                    }
                    (false, Some(extent), Some(tile)) => {
                        format!("Release: paint {extent} of {into} with `{tile}`. Esc drops it.")
                    }
                    (false, None, Some(tile)) => format!(
                        "Drag: paint `{tile}` into {into}. Shift+drag: a rectangle. Right drag: erase."
                    ),
                    (false, _, None) => format!("Room `{room}`'s tileset has no tile to paint."),
                }
            }
        };
        match &self.cursor {
            Some((room, [col, row])) => format!("{does} At {col}, {row} in `{room}`."),
            None => does,
        }
    }

    /// The window's frame (redo 7.12): built, kept modal and filled while
    /// open, removed when closed; its close button closes the editor.
    pub(crate) fn update_window(&mut self, world: &mut World, frame: &mut ViewFrame<'_>) {
        panel::update(self, world, frame);
    }
}

/// How an edit landed.
enum Landed {
    /// Applied and written to the room's file.
    Written,
    /// Applied to a room with no file.
    Live,
    /// Nothing to undo or redo.
    Nothing,
}

/// The action a key asks for, the arrows and Escape aside: 1 to 6 the
/// tools, R the rename prompt, `[` and `]` the palette, L the layer, F fit, G the grid, Home the
/// game's view, Delete or Backspace, Ctrl+D, Ctrl+Z, and Ctrl+Y or
/// Ctrl+Shift+Z.
fn key_action(code: KeyCode, control: bool, shift: bool) -> Option<Action> {
    Some(match code {
        KeyCode::KeyD if control => Action::Copy,
        KeyCode::KeyZ if control && shift => Action::Redo,
        KeyCode::KeyZ if control => Action::Undo,
        KeyCode::KeyY if control => Action::Redo,
        _ if control => return None,
        KeyCode::Home => Action::GameView,
        KeyCode::Delete | KeyCode::Backspace => Action::Delete,
        KeyCode::Digit1 => Action::Tool(Tool::Select),
        KeyCode::Digit2 => Action::Tool(Tool::Place),
        KeyCode::Digit3 => Action::Tool(Tool::Paint),
        KeyCode::Digit4 => Action::Tool(Tool::Erase),
        KeyCode::Digit5 => Action::Tool(Tool::Pick),
        KeyCode::Digit6 => Action::Tool(Tool::Mark),
        KeyCode::KeyR => Action::Prompt(Ask::Rename),
        KeyCode::BracketLeft => Action::PaletteStep(-1),
        KeyCode::BracketRight => Action::PaletteStep(1),
        KeyCode::KeyL => Action::LayerStep,
        KeyCode::KeyF => Action::Fit,
        KeyCode::KeyG => Action::Grid,
        _ => return None,
    })
}

/// What the hint says while no room is live: whether the game lists any,
/// and how one comes to be live.
fn no_live_room(world: &World) -> String {
    let ids: Vec<&str> = world
        .get_resource::<RoomRegistry>()
        .map(|registry| registry.ids().collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return "No room to edit: the game lists none. `tungsten level new <room>` makes one, and the game spawns it with `world.spawn_room`.".to_owned();
    }
    format!(
        "No room is live: the game has spawned none of its {}. Click one under Rooms to open it here.",
        if ids.len() == 1 {
            "room".to_owned()
        } else {
            format!("{} rooms", ids.len())
        },
    )
}

/// Why the editor will not open registered room `room`, which is not live:
/// its area holds a run's progress, which a spawn here would write over.
/// `None` for a room it may open.
fn why_closed(world: &World, room: &str) -> Option<String> {
    let state = world.get_resource::<SaveState>()?;
    if state.is_kept(room) {
        return Some(
            "a loaded save holds progress there, which a spawn here would write over. It opens once the game spawns it."
                .to_owned(),
        );
    }
    state.is_loaded(room).then(|| {
        "the game has had it live in this run, and a spawn here would write over what changed. It opens once the game spawns it."
            .to_owned()
    })
}

/// Closes the editor at the start of the run's last frame, before its
/// first stage: the rooms it opened are put away, so a save a game takes
/// in that frame, as early as `PreUpdate`, holds none of them.
pub(crate) fn close_for_exit(world: &mut World) {
    if !world
        .get_resource::<LevelEditor>()
        .is_some_and(|editor| editor.open)
    {
        return;
    }
    if let Some(mut editor) = world.remove_resource::<LevelEditor>() {
        editor.close(world);
        world.insert_resource(editor);
    }
}

/// Why room `room` is read only, without reading its file: a generated
/// room, or one whose file another listed room names. `None` for a room
/// the editor may write, which an edit still checks against its file.
fn read_only(world: &World, room: &str) -> Option<String> {
    let registry = world.get_resource::<RoomRegistry>()?;
    if let Some(generation) = registry.generation(room) {
        return Some(format!(
            "`{room}` is read only: generator `{}` made it for this run. `tungsten level generate` writes rooms to edit.",
            generation.generator
        ));
    }
    let path = registry.path(room)?;
    let other = registry
        .ids()
        .find(|other| *other != room && registry.path(other) == Some(path))?;
    Some(format!(
        "`{room}` is read only: it shares its file with room `{other}`, and writing one would rewrite the other."
    ))
}

/// The frame's input, read from the raw `InputState`.
struct Input {
    cursor: Option<Vec2>,
    keys: Vec<(KeyCode, Modifiers, bool)>,
    wheel: f32,
    /// Shift and Alt, as held now.
    shift: bool,
    alt: bool,
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
            shift: input.modifiers().shift(),
            alt: input.modifiers().alt(),
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

/// The object or place under `at`, as `(room, id)`: an object inside its
/// box or within the pick radius of its marker, the smallest box winning;
/// else a point place within twice that of its marker, or an area within
/// it of its outline. An area is not picked by its inside, where a press
/// is the room's: it can cover a whole arena. A route is not picked.
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
            if (at - marker).abs().max_element() > PICK_REACH {
                continue;
            }
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
        // A place counts as larger than any marker, so an object at the
        // same spot wins.
        let places = registry.compiled(&live.id).map(|room| &room.places);
        for (id, place) in places.into_iter().flatten() {
            let hit = match &place.shape {
                PlaceShape::Point(point) => at.distance(live.origin + *point) <= radius * 2.0,
                PlaceShape::Rect { min, size } => {
                    let min = live.origin + *min;
                    let max = min + *size;
                    let within = |grow: f32| {
                        at.x >= min.x - grow
                            && at.y >= min.y - grow
                            && at.x <= max.x + grow
                            && at.y <= max.y + grow
                    };
                    within(radius) && !within(-radius)
                }
                _ => false,
            };
            let area = (radius * 4.0).powi(2);
            if hit && best.is_none_or(|(smallest, ..)| area < smallest) {
                best = Some((area, &live.id, id));
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
/// error, a sentence's end, for a read-only room or a file that does not
/// hold its live document.
fn writable(world: &World, room: &str) -> Result<Option<PathBuf>, String> {
    let registry = world
        .get_resource::<RoomRegistry>()
        .ok_or_else(|| format!("no room `{room}`."))?;
    let document = registry
        .room(room)
        .ok_or_else(|| format!("no room `{room}`."))?;
    if let Some(generation) = registry.generation(room) {
        return Err(format!(
            "`{room}` is a generated room ({generation}): read only. `tungsten level generate` writes rooms to edit."
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
            "`{room}` shares its file with room `{other}`: read only, since writing one rewrites the other."
        ));
    }
    let file = shown(path);
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("'{file}' cannot be read ({err})."))?;
    match Room::from_json(&text) {
        Ok(read) if read == *document => Ok(Some(path.to_path_buf())),
        Ok(_) => Err(format!(
            "'{file}' differs from the live `{room}` (a reload pending or failed, or an edit outside): nothing edited."
        )),
        Err(err) => Err(format!(
            "'{file}' does not read ({err}): nothing edited, so no work is written over."
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

/// The size of prefab `prefab`'s sprite in pixels: the box Place's ghost
/// draws at the cell. `None` for a prefab with no sprite, or one whose
/// sprite is not registered.
fn prefab_size(world: &World, prefab: &str) -> Option<Vec2> {
    let name = &world
        .get_resource::<tungsten_core::assets::PrefabRegistry>()?
        .get(prefab)?
        .sprite
        .as_ref()?
        .asset_id;
    let sprites = world.get_resource::<tungsten_core::assets::AssetRegistry>()?;
    let sprite = sprites.sprite(sprites.sprite_id(name)?)?;
    Some(Vec2::new(sprite.width as f32, sprite.height as f32))
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
