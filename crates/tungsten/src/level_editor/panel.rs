//! The level editor's window: a panel that shows what is in force and holds
//! a button for every action.
//!
//! The window is a debug view like the others (`ViewFrame::sync`: a title
//! bar that drags it, collapse and close, a saved place), with no table:
//! its body holds one column, [`WIDTH`] units wide, built once a window and
//! written only where what it shows changed ([`Shown`]), so an idle editor
//! writes no node.
//!
//! Top to bottom: the last result or refusal, wrapped; what the primary
//! button will do under the cursor; the live rooms, the current one marked;
//! the tools, the active one marked; undo and redo with their counts,
//! delete, copy, fit and the game's view; the palette, the prefabs for
//! Select and Place and the current room's tiles for Paint and Erase, each
//! with its sprite; the target layer; the pick; the room's findings; the
//! mouse and the keys. A click on a button asks the editor for the same
//! [`Action`] its key does.

use glam::Vec2;
use tungsten_core::assets::{PrefabRegistry, RoomJournal, RoomRegistry, SpriteAssetId};
use tungsten_core::text::{EllipsisAt, TextAlign, TextOverflow, TextWrap};
use tungsten_core::ui::{
    Align, Direction, Edges, LayoutStyle, Sizing, StyleOverrides, UiTree, Visibility, WidgetId,
};
use tungsten_core::{AssetRegistry, World};

use super::{Action, LevelEditor, TOGGLE, Tool, live_rooms, prefab_ids, read_only, tile_ids};
use crate::debug_views::{
    CELL_PAD, ERROR_TEXT, Header, INSET, Ink, LABEL, Look, RADIUS, Shape, TRANSPARENT, ViewFrame,
    ViewSpec, YELLOW, look_of, toggle_key,
};

/// The panel's width, in the debug layer's units.
pub(super) const WIDTH: f32 = 300.0;
/// What the panel's text spans: its width less its insets.
const INNER: f32 = WIDTH - 2.0 * INSET;
/// The gap between two buttons of a row.
const GAP: f32 = 4.0;
/// The buttons a row of the tools and of the actions holds.
const PER_ROW: usize = 3;
/// A palette row's sprite box, a square.
const THUMB: f32 = 16.0;
/// The palette's rows in view before it scrolls.
const PALETTE_ROWS: f32 = 7.0;
/// A button's fill, and an active button's or a marked row's.
const BUTTON: [u8; 4] = [255, 255, 255, 12];
const ACTIVE: [u8; 4] = [255, 255, 255, 44];

/// The lines the status and the hint wrap over before they end in `…`: a
/// refusal naming a file's path, and the hint of a game with no live room,
/// read whole.
const SENTENCE_LINES: u32 = 6;

/// What the panel shows while the editor has said nothing yet.
const WELCOME: &str = "Every edit is checked, written to the room's file and can be undone. F10 or the × closes the editor.";

/// The mouse and the keys, the panel's last lines.
const KEYS: [&str; 11] = [
    "left button: the tool",
    "Shift+drag: a rectangle of cells",
    "right button: erase cells, delete an object",
    "Alt+left: take a prefab or a tile",
    "middle drag, arrows: pan   wheel: zoom",
    "F: fit   Home: game view   G: grid",
    "1 2 3 4 5: the tools",
    "[ ]: palette   L: layer",
    "Del: delete   Ctrl+D: copy",
    "Ctrl+Z: undo   Ctrl+Y: redo",
    "Esc: drop a drag or the pick   F10: close",
];

/// The action buttons under the tools, in order, with their captions; undo
/// and redo show their counts after them, and Grid is marked while on.
const ACTIONS: [(Action, &str); 7] = [
    (Action::Undo, "Undo"),
    (Action::Redo, "Redo"),
    (Action::Delete, "Delete"),
    (Action::Copy, "Copy"),
    (Action::Fit, "Fit"),
    (Action::GameView, "Game view"),
    (Action::Grid, "Grid"),
];

/// A list's row: its button, its text and the action a click asks for.
struct Entry {
    button: WidgetId,
    label: WidgetId,
    action: Action,
}

/// The window's contents: the widgets it writes and what they last showed.
pub(super) struct Panel {
    /// The window's root it was built under.
    root: WidgetId,
    status: WidgetId,
    hint: WidgetId,
    rooms: WidgetId,
    room_rows: Vec<Entry>,
    tools: Vec<(WidgetId, Tool)>,
    actions: Vec<(WidgetId, Action, &'static str)>,
    palette_title: WidgetId,
    palette: WidgetId,
    palette_rows: Vec<Entry>,
    layers: WidgetId,
    layer_rows: Vec<Entry>,
    selection: WidgetId,
    findings_title: WidgetId,
    findings: WidgetId,
    shown: Option<Shown>,
    /// How many times a part of it was written: an idle panel's stands.
    pub(super) writes: u64,
}

/// A room's row: its ID, what its edits write to, and whether it is the
/// current one.
#[derive(Debug, Clone, PartialEq)]
struct RoomRow {
    id: String,
    note: String,
    current: bool,
}

/// The palette's rows: its title, each item's ID and sprite, the chosen
/// one, and whether its items are tiles.
#[derive(Debug, Clone, Default, PartialEq)]
struct Palette {
    title: String,
    items: Vec<(String, Option<String>)>,
    chosen: Option<String>,
    tiles: bool,
}

/// Everything the panel shows, read from the editor and the World each
/// frame and compared with the last written.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    status: String,
    refused: bool,
    hint: String,
    rooms: Vec<RoomRow>,
    tool: Tool,
    /// The current room's batches to undo and to redo.
    history: (usize, usize),
    picked: bool,
    grid: bool,
    palette: Palette,
    /// Each layer's name and kind.
    layers: Vec<(String, String)>,
    layer: Option<String>,
    selection: Vec<String>,
    findings: Vec<String>,
}

impl Shown {
    fn of(editor: &LevelEditor, world: &World) -> Self {
        let registry = world.get_resource::<RoomRegistry>();
        let current = editor.current.as_deref();
        let rooms = live_rooms(world, None)
            .into_iter()
            .map(|room| {
                let note = if read_only(world, &room.id).is_some() {
                    "read only".to_owned()
                } else {
                    registry
                        .and_then(|registry| registry.path(&room.id))
                        .and_then(|path| path.file_name())
                        .map_or_else(
                            || "no file: edits stay live".to_owned(),
                            |name| name.to_string_lossy().into_owned(),
                        )
                };
                RoomRow {
                    current: current == Some(room.id.as_str()),
                    id: room.id,
                    note,
                }
            })
            .collect();
        let document = current.and_then(|room| registry?.room(room));
        let history = current
            .zip(world.get_resource::<RoomJournal>())
            .map_or((0, 0), |(room, journal)| {
                (journal.undo_len(room), journal.redo_len(room))
            });
        let tiles = matches!(editor.tool, Tool::Paint | Tool::Erase);
        let palette = if tiles {
            let room = current.unwrap_or_default();
            Palette {
                title: match document.and_then(|document| document.tileset.as_deref()) {
                    Some(tileset) => format!("Tiles of `{tileset}`"),
                    None => "Tiles".to_owned(),
                },
                items: tile_ids(world, room)
                    .into_iter()
                    .map(|tile| (tile.clone(), Some(tile)))
                    .collect(),
                chosen: editor.tile_or_first(world, room),
                tiles,
            }
        } else {
            let prefabs = world.get_resource::<PrefabRegistry>();
            Palette {
                title: "Prefabs".to_owned(),
                items: prefab_ids(world)
                    .into_iter()
                    .map(|id| {
                        let sprite = prefabs
                            .and_then(|prefabs| prefabs.get(&id))
                            .and_then(|prefab| prefab.sprite.as_ref())
                            .map(|sprite| sprite.asset_id.clone());
                        (id, sprite)
                    })
                    .collect(),
                chosen: editor.prefab_or_first(world),
                tiles,
            }
        };
        let layers = document.map_or_else(Vec::new, |document| {
            document
                .layers
                .iter()
                .map(|layer| {
                    let kind = serde_json::to_value(layer.kind)
                        .ok()
                        .and_then(|kind| kind.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    (layer.name.clone(), kind)
                })
                .collect()
        });
        let object = document
            .zip(editor.selection.as_deref())
            .and_then(|(document, id)| document.objects.iter().find(|object| object.id == id));
        let selection = match object {
            Some(object) => {
                let mut lines = vec![
                    format!("`{}`, a `{}`", object.id, object.prefab),
                    format!("at {}, {}", object.at[0], object.at[1]),
                ];
                if let Some(shape) = &object.on {
                    lines.push(format!("on shape `{shape}`"));
                }
                if let Some(generator) = &object.generator {
                    lines.push(format!("made by generator `{generator}`"));
                }
                lines.extend(
                    (object.set.iter()).map(|(path, value)| format!("set {path} = {value}")),
                );
                lines
            }
            None => vec!["nothing: click an object with Select".to_owned()],
        };
        Self {
            status: if editor.status.is_empty() {
                WELCOME.to_owned()
            } else {
                editor.status.clone()
            },
            refused: editor.refused,
            hint: editor.hint.clone(),
            rooms,
            tool: editor.tool,
            history,
            picked: object.is_some(),
            grid: editor.grid,
            layer: editor
                .layer
                .clone()
                .filter(|layer| layers.iter().any(|(name, _)| name == layer)),
            palette,
            layers,
            selection,
            findings: editor.findings.clone(),
        }
    }
}

/// The window's frame: built, kept modal and filled while the editor is
/// open, removed when it closes; a click on a button is carried out before
/// the panel is written, so it shows the click's result the same frame.
pub(super) fn update(editor: &mut LevelEditor, world: &mut World, frame: &mut ViewFrame<'_>) {
    if frame.closes(editor.window.as_ref()) {
        editor.close(world);
    }
    let spec = ViewSpec {
        name: "level_editor",
        title: "Level editor",
        key: toggle_key(world, TOGGLE),
        enabled: editor.open,
        corner: editor.corner,
        padding: editor.padding_px,
        strip: false,
        tall: true,
        look: look_of!(editor),
        shape: Shape {
            columns: &[],
            header: Header::None,
            buttons: 0,
        },
    };
    if !frame.sync(&mut editor.window, &spec) {
        editor.panel = None;
        editor.panel_rect = None;
        return;
    }
    let Some(window) = editor.window.as_ref() else {
        return;
    };
    let (root, body) = (window.root, window.body);
    frame.tree.set_modal(root, true);
    editor.panel_rect = frame
        .tree
        .visual_rect(root)
        .map(|rect| (rect.min * frame.scale, rect.max * frame.scale));
    let mut panel = match editor.panel.take() {
        Some(panel) if panel.root == root && frame.tree.contains(panel.status) => panel,
        _ => Panel::build(frame.tree, &spec.look, root, body),
    };
    for action in panel.clicked(frame) {
        editor.act(world, action);
    }
    panel.write(frame.tree, &spec.look, world, Shown::of(editor, world));
    editor.panel = Some(panel);
}

impl Panel {
    /// A new panel under the window's `body`: every part that keeps its
    /// shape built and styled, the lists empty until the first write.
    fn build(tree: &mut UiTree, look: &Look, root: WidgetId, body: WidgetId) -> Self {
        let column = tree.panel(
            body,
            LayoutStyle {
                width: Sizing::Px(WIDTH),
                padding: Edges {
                    start: INSET,
                    end: INSET,
                    top: 0.0,
                    bottom: INSET,
                },
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(column, look.plain(Ink::body(look.color)));
        tree.set_key(column, "panel");
        let status = wrapped(tree, look, column, SENTENCE_LINES);
        tree.set_key(status, "status");
        let hint = wrapped(tree, look, column, SENTENCE_LINES);
        tree.set_key(hint, "hint");
        tree.set_overrides(hint, wrap(look.plain(Ink::body(LABEL)), SENTENCE_LINES));

        heading(tree, look, column, "Rooms");
        let rooms = list(tree, look, column, "rooms");

        heading(tree, look, column, "Tools");
        let mut tools = Vec::new();
        for row in Tool::ALL.chunks(PER_ROW) {
            let line = button_row(tree, look, column);
            for &tool in row {
                let button = button(tree, line);
                tree.set_key(button, &format!("tool_{}", tool.name()));
                tools.push((button, tool));
            }
        }
        let mut actions = Vec::new();
        for row in ACTIONS.chunks(PER_ROW) {
            let line = button_row(tree, look, column);
            for (action, caption) in row {
                let button = button(tree, line);
                tree.set_key(button, &caption.to_lowercase().replace(' ', "_"));
                actions.push((button, action.clone(), *caption));
            }
        }

        let palette_title = heading(tree, look, column, "Palette");
        let palette = tree.scroll_panel(
            column,
            LayoutStyle {
                max_height: Some(PALETTE_ROWS * look.line_height),
                ..LayoutStyle::default()
            },
        );
        tree.set_overrides(palette, look.plain(Ink::body(look.color)));
        tree.set_key(palette, "palette");

        heading(tree, look, column, "Layer");
        let layers = list(tree, look, column, "layers");

        heading(tree, look, column, "Pick");
        let selection = list(tree, look, column, "pick");

        let findings_title = heading(tree, look, column, "Findings");
        let findings = list(tree, look, column, "findings");

        heading(tree, look, column, "Mouse and keys");
        for line in KEYS {
            let label = wrapped(tree, look, column, 2);
            tree.set_plain_text(label, line);
            tree.set_overrides(label, wrap(look.plain(Ink::body(LABEL)), 2));
        }
        Self {
            root,
            status,
            hint,
            rooms,
            room_rows: Vec::new(),
            tools,
            actions,
            palette_title,
            palette,
            palette_rows: Vec::new(),
            layers,
            layer_rows: Vec::new(),
            selection,
            findings_title,
            findings,
            shown: None,
            writes: 0,
        }
    }

    /// The actions this frame's clicks ask for, in the panel's order.
    fn clicked(&self, frame: &ViewFrame<'_>) -> Vec<Action> {
        let tools = (self.tools.iter())
            .filter(|(button, _)| frame.activated(*button))
            .map(|(_, tool)| Action::Tool(*tool));
        let actions = (self.actions.iter())
            .filter(|(button, ..)| frame.activated(*button))
            .map(|(_, action, _)| action.clone());
        let rows = (self.room_rows.iter())
            .chain(&self.palette_rows)
            .chain(&self.layer_rows)
            .filter(|entry| frame.activated(entry.button))
            .map(|entry| entry.action.clone());
        tools.chain(actions).chain(rows).collect()
    }

    /// Writes the parts of `shown` that differ from the last written.
    fn write(&mut self, tree: &mut UiTree, look: &Look, world: &World, shown: Shown) {
        let last = self.shown.take();
        let last = last.as_ref();
        // Whether a part differs from the last written, or nothing was.
        macro_rules! changed {
            ($part:ident) => {
                last.is_none_or(|last| last.$part != shown.$part)
            };
        }
        if changed!(status) || changed!(refused) {
            self.writes += 1;
            tree.set_plain_text(self.status, &shown.status);
            let color = if shown.refused {
                ERROR_TEXT
            } else {
                look.color
            };
            tree.set_overrides(
                self.status,
                wrap(look.plain(Ink::body(color).strong()), SENTENCE_LINES),
            );
        }
        if changed!(hint) {
            self.writes += 1;
            tree.set_plain_text(self.hint, &shown.hint);
        }
        if changed!(rooms) {
            self.writes += 1;
            let same_rows = last.is_some_and(|last| {
                last.rooms.len() == shown.rooms.len()
                    && (last.rooms.iter().zip(&shown.rooms))
                        .all(|(a, b)| a.id == b.id && a.note == b.note)
            });
            if !same_rows {
                clear(tree, self.rooms);
                self.room_rows = (shown.rooms.iter())
                    .map(|room| {
                        let key = format!("room_{}", room.id);
                        let entry = entry(tree, look, self.rooms, &Thumb::None, &key);
                        tree.set_plain_text(entry.label, &format!("{}  {}", room.id, room.note));
                        Entry {
                            action: Action::Room(room.id.clone()),
                            ..entry
                        }
                    })
                    .collect();
                if shown.rooms.is_empty() {
                    let none = tree.label(self.rooms, "none is live");
                    tree.set_overrides(none, look.plain(Ink::body(LABEL)));
                }
            }
            for (entry, room) in self.room_rows.iter().zip(&shown.rooms) {
                mark(tree, look, entry, room.current);
            }
        }
        if changed!(tool) {
            self.writes += 1;
            for (button, tool) in &self.tools {
                paint(
                    tree,
                    look,
                    *button,
                    tool.caption(),
                    *tool == shown.tool,
                    false,
                );
            }
        }
        if changed!(history) || changed!(picked) || changed!(grid) {
            self.writes += 1;
            for (button, action, caption) in &self.actions {
                let (text, active, dim) = match action {
                    Action::Undo => (
                        format!("{caption} {}", shown.history.0),
                        false,
                        shown.history.0 == 0,
                    ),
                    Action::Redo => (
                        format!("{caption} {}", shown.history.1),
                        false,
                        shown.history.1 == 0,
                    ),
                    Action::Delete | Action::Copy => ((*caption).to_owned(), false, !shown.picked),
                    Action::Grid => ((*caption).to_owned(), shown.grid, false),
                    _ => ((*caption).to_owned(), false, false),
                };
                paint(tree, look, *button, &text, active, dim);
            }
        }
        if changed!(palette) {
            self.writes += 1;
            let same_rows = last.is_some_and(|last| {
                last.palette.items == shown.palette.items
                    && last.palette.tiles == shown.palette.tiles
            });
            if !same_rows {
                tree.set_plain_text(self.palette_title, &shown.palette.title);
                clear(tree, self.palette);
                let sprites = world.get_resource::<AssetRegistry>();
                self.palette_rows = (shown.palette.items.iter())
                    .map(|(id, sprite)| {
                        let thumb = (sprite.as_deref().zip(sprites))
                            .and_then(|(name, sprites)| {
                                let id = sprites.sprite_id(name)?;
                                let asset = sprites.sprite(id)?;
                                let size = Vec2::new(asset.width as f32, asset.height as f32);
                                Some(Thumb::Sprite(id, size))
                            })
                            .unwrap_or(Thumb::Empty);
                        let key = format!("item_{id}");
                        let entry = entry(tree, look, self.palette, &thumb, &key);
                        tree.set_plain_text(entry.label, id);
                        Entry {
                            action: if shown.palette.tiles {
                                Action::Tile(id.clone())
                            } else {
                                Action::Prefab(id.clone())
                            },
                            ..entry
                        }
                    })
                    .collect();
                if shown.palette.items.is_empty() {
                    let none = tree.label(
                        self.palette,
                        if shown.palette.tiles {
                            "the tileset has no tile"
                        } else {
                            "the game lists no prefab"
                        },
                    );
                    tree.set_overrides(none, look.plain(Ink::body(LABEL)));
                }
            }
            for (entry, (id, _)) in self.palette_rows.iter().zip(&shown.palette.items) {
                mark(
                    tree,
                    look,
                    entry,
                    shown.palette.chosen.as_deref() == Some(id),
                );
            }
        }
        if changed!(layers) || changed!(layer) {
            self.writes += 1;
            if changed!(layers) {
                clear(tree, self.layers);
                let all = entry(tree, look, self.layers, &Thumb::None, "layer_all");
                tree.set_plain_text(all.label, "all  every layer");
                self.layer_rows = vec![Entry {
                    action: Action::Layer(None),
                    ..all
                }];
                for (name, kind) in &shown.layers {
                    let key = format!("layer_{name}");
                    let entry = entry(tree, look, self.layers, &Thumb::None, &key);
                    tree.set_plain_text(entry.label, &format!("{name}  {kind}"));
                    self.layer_rows.push(Entry {
                        action: Action::Layer(Some(name.clone())),
                        ..entry
                    });
                }
            }
            for entry in &self.layer_rows {
                let chosen = matches!(&entry.action, Action::Layer(layer) if *layer == shown.layer);
                mark(tree, look, entry, chosen);
            }
        }
        if changed!(selection) {
            self.writes += 1;
            lines(tree, look, self.selection, &shown.selection, look.color, 2);
        }
        if changed!(findings) {
            self.writes += 1;
            tree.set_plain_text(
                self.findings_title,
                &match shown.findings.len() {
                    0 => "Findings: none".to_owned(),
                    n => format!("Findings: {n}"),
                },
            );
            lines(tree, look, self.findings, &shown.findings, YELLOW, 3);
        }
        self.shown = Some(shown);
    }
}

/// `style` with its text wrapped by word over at most `lines` lines, the
/// last ending in `…`.
fn wrap(mut style: StyleOverrides, lines: u32) -> StyleOverrides {
    style.text.wrap = Some(TextWrap::WordOrGlyph);
    style.text.overflow = Some(TextOverflow::Ellipsis {
        at: EllipsisAt::End,
        lines: Some(lines),
    });
    style
}

/// An empty label under `parent` as wide as the panel's text, wrapping
/// over at most `lines` lines.
fn wrapped(tree: &mut UiTree, look: &Look, parent: WidgetId, lines: u32) -> WidgetId {
    let label = tree.label(parent, "");
    tree.set_layout_style(
        label,
        LayoutStyle {
            width: Sizing::Px(INNER),
            ..LayoutStyle::default()
        },
    );
    tree.set_overrides(label, wrap(look.plain(Ink::body(look.color)), lines));
    label
}

/// A section's heading: a strong grey line with room above it.
fn heading(tree: &mut UiTree, look: &Look, parent: WidgetId, text: &str) -> WidgetId {
    let label = tree.label(parent, text);
    tree.set_layout_style(
        label,
        LayoutStyle {
            padding: Edges {
                top: INSET,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        },
    );
    tree.set_overrides(label, look.plain(Ink::body(LABEL).strong()));
    label
}

/// An empty column under `parent`, keyed `key`: a list's rows go in it.
fn list(tree: &mut UiTree, look: &Look, parent: WidgetId, key: &str) -> WidgetId {
    let panel = tree.panel(parent, LayoutStyle::column());
    tree.set_overrides(panel, look.plain(Ink::body(look.color)));
    tree.set_key(panel, key);
    panel
}

/// A row of buttons under `parent`.
fn button_row(tree: &mut UiTree, look: &Look, parent: WidgetId) -> WidgetId {
    let row = tree.panel(
        parent,
        LayoutStyle {
            direction: Direction::Row,
            gap: GAP,
            padding: Edges {
                top: GAP,
                ..Edges::default()
            },
            ..LayoutStyle::default()
        },
    );
    tree.set_overrides(row, look.plain(Ink::body(look.color)));
    row
}

/// A button in a row of [`PER_ROW`], each a third of the panel's text
/// wide; [`paint`] gives it its caption and look.
fn button(tree: &mut UiTree, row: WidgetId) -> WidgetId {
    let button = tree.button(row, "");
    tree.set_focusable(button, false);
    tree.set_layout_style(
        button,
        LayoutStyle {
            width: Sizing::Px((INNER - GAP * (PER_ROW as f32 - 1.0)) / PER_ROW as f32),
            ..LayoutStyle::default()
        },
    );
    button
}

/// A button's caption and look: its fill brighter and its text strong
/// while `active`, its text grey while `dim`.
fn paint(tree: &mut UiTree, look: &Look, button: WidgetId, text: &str, active: bool, dim: bool) {
    tree.set_text(button, text);
    let ink = match (active, dim) {
        (true, _) => Ink::body(look.color).strong(),
        (false, true) => Ink::body(LABEL),
        (false, false) => Ink::body(look.color),
    }
    .aligned(TextAlign::Center);
    let fill = if active { ACTIVE } else { BUTTON };
    tree.set_overrides(button, look.node(fill, TRANSPARENT, 0.0, RADIUS, ink));
    if let Some(&label) = tree.children(button).first() {
        tree.set_overrides(label, look.plain(ink));
    }
}

/// What a list's row shows before its text.
enum Thumb {
    /// Nothing: the row is its text.
    None,
    /// The sprite box, empty: the item's sprite is not in the registry.
    Empty,
    /// The sprite box with the sprite of this size in pixels.
    Sprite(SpriteAssetId, Vec2),
}

/// A list's row under `parent`, keyed `key`: a button as wide as the
/// list holding what `thumb` asks for and its text. Its action is the
/// caller's to set.
fn entry(tree: &mut UiTree, look: &Look, parent: WidgetId, thumb: &Thumb, key: &str) -> Entry {
    let button = tree.button(parent, "");
    tree.set_focusable(button, false);
    tree.set_key(button, key);
    tree.set_layout_style(
        button,
        LayoutStyle {
            direction: Direction::Row,
            gap: CELL_PAD,
            align_items: Align::Center,
            padding: Edges::symmetric(CELL_PAD, 0.0),
            ..LayoutStyle::default()
        },
    );
    // The button's own label stays empty: the row's text is its own node,
    // after the sprite.
    if let Some(&own) = tree.children(button).first() {
        tree.set_visibility(own, Visibility::Collapsed);
    }
    if !matches!(thumb, Thumb::None) {
        let frame = tree.panel(
            button,
            LayoutStyle {
                align_items: Align::Center,
                ..LayoutStyle::default().with_size(Sizing::Px(THUMB), Sizing::Px(THUMB))
            },
        );
        tree.set_overrides(frame, look.plain(Ink::body(look.color)));
        tree.set_hit_testable(frame, false);
        if let &Thumb::Sprite(id, size) = thumb {
            // The sprite whole inside the box, its shape kept: one larger
            // than the box shrinks to it, a smaller one keeps its pixels.
            // The shrunk size is the image's natural one, since a snapped
            // root draws an image at a whole multiple of that.
            let fit = (THUMB / size.x.max(size.y).max(1.0)).min(1.0);
            let shown = size * fit;
            let image = tree.image(
                frame,
                id,
                shown,
                LayoutStyle::default().with_size(Sizing::Px(shown.x), Sizing::Px(shown.y)),
            );
            tree.set_hit_testable(image, false);
        }
    }
    let label = tree.label(button, "");
    tree.set_hit_testable(label, false);
    let entry = Entry {
        button,
        label,
        action: Action::GameView,
    };
    mark(tree, look, &entry, false);
    entry
}

/// A row's look: filled and strong while `marked`.
fn mark(tree: &mut UiTree, look: &Look, entry: &Entry, marked: bool) {
    let (fill, ink) = if marked {
        (ACTIVE, Ink::body(look.color).strong())
    } else {
        (TRANSPARENT, Ink::body(look.color))
    };
    tree.set_overrides(entry.button, look.node(fill, TRANSPARENT, 0.0, RADIUS, ink));
    tree.set_overrides(entry.label, look.plain(ink));
}

/// Removes every child of `parent`.
fn clear(tree: &mut UiTree, parent: WidgetId) {
    for child in tree.children(parent).to_vec() {
        tree.remove(child);
    }
}

/// `parent`'s children made `texts`, a label each in `color`, wrapped over
/// at most `wrap_lines` lines.
fn lines(
    tree: &mut UiTree,
    look: &Look,
    parent: WidgetId,
    texts: &[String],
    color: [u8; 4],
    wrap_lines: u32,
) {
    clear(tree, parent);
    for text in texts {
        let label = wrapped(tree, look, parent, wrap_lines);
        tree.set_plain_text(label, text);
        tree.set_overrides(label, wrap(look.plain(Ink::body(color)), wrap_lines));
    }
}
