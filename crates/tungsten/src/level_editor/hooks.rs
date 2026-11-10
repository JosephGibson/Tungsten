//! The level editor's script hooks: the `editor` probe reads its state and
//! the `editor` action asks it for what its keys and its panel's buttons
//! do, through the same [`Action`], so a script drives the editor without a
//! raw key and a test or a capture reaches every control.
//!
//! ```json
//! { "frame": 5, "action": "editor", "args": { "open": true, "tool": "paint", "tile": "rock" } }
//! { "frame": 9, "action": "editor", "args": { "do": "undo" } }
//! { "frame": 10, "assert": { "probe": "editor", "path": "/status", "op": "==", "value": "Undid the last edit." } }
//! ```
//!
//! The action's fields apply in this order, each optional: `open`, `room`,
//! `tool`, `prefab`, `tile`, `layer` (a name, or `null` for every layer),
//! `select` (an object's ID, or `null`), `do`. A field the editor refuses
//! fails the step with the editor's own sentence.

use serde::Deserialize;
use serde_json::{Value, json};
use tungsten_core::World;
use tungsten_core::assets::RoomJournal;
use tungsten_core::script::ScriptHooks;

use super::{Action, LevelEditor, Tool};

/// What `do` may name, with the action each is.
const DOES: [(&str, Action); 9] = [
    ("undo", Action::Undo),
    ("redo", Action::Redo),
    ("delete", Action::Delete),
    ("copy", Action::Copy),
    ("fit", Action::Fit),
    ("game_view", Action::GameView),
    ("palette_next", Action::PaletteStep(1)),
    ("palette_previous", Action::PaletteStep(-1)),
    ("layer_next", Action::LayerStep),
];

/// Adds the `editor` probe and action to `hooks`.
pub(crate) fn register(hooks: &mut ScriptHooks) {
    hooks.add_probe(
        "editor",
        "The level editor's state: `available`, `open`, `tool`, `room`, `rooms` (the live ones), `layer`, `prefab`, `tile`, `selection`, `status`, `refused`, `hint`, `undo`, `redo`, `findings`, `view`.",
        probe,
    );
    hooks.add_action(
        "editor",
        "Drives the level editor as its keys and buttons do: any of `{\"open\": bool, \"room\": id, \"tool\": \"select\" | \"place\" | \"paint\" | \"erase\", \"prefab\": id, \"tile\": sprite, \"layer\": name | null, \"select\": id | null, \"do\": \"undo\" | \"redo\" | \"delete\" | \"copy\" | \"fit\" | \"game_view\" | \"palette_next\" | \"palette_previous\" | \"layer_next\"}`, applied in that order.",
        action,
    );
}

fn probe(world: &World, _: &Value) -> Result<Value, String> {
    let editor = world
        .get_resource::<LevelEditor>()
        .ok_or("the World has no level editor")?;
    let rooms: Vec<String> = super::live_rooms(world, None)
        .into_iter()
        .map(|room| room.id)
        .collect();
    let (undo, redo) = (editor.current.as_deref())
        .zip(world.get_resource::<RoomJournal>())
        .map_or((0, 0), |(room, journal)| {
            (journal.undo_len(room), journal.redo_len(room))
        });
    Ok(json!({
        "available": editor.available,
        "open": editor.open,
        "tool": editor.tool.name(),
        "room": editor.current,
        "rooms": rooms,
        "layer": editor.layer,
        "prefab": editor.prefab_or_first(world),
        "tile": editor
            .current
            .as_deref()
            .and_then(|room| editor.tile_or_first(world, room)),
        "selection": editor.selection,
        "status": editor.status,
        "refused": editor.refused,
        "hint": editor.hint,
        "undo": undo,
        "redo": redo,
        "findings": editor.findings,
        "view": editor.view.map(|view| json!({
            "position": [view.position.x, view.position.y],
            "zoom": view.zoom,
        })),
    }))
}

/// The action's arguments. `layer` and `select` tell an absent key from
/// `null`, which asks for every layer and for no pick.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(clippy::option_option, reason = "an absent key differs from `null`")]
struct Args {
    open: Option<bool>,
    room: Option<String>,
    tool: Option<String>,
    prefab: Option<String>,
    tile: Option<String>,
    #[serde(default, deserialize_with = "present")]
    layer: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    select: Option<Option<String>>,
    #[serde(rename = "do")]
    does: Option<String>,
}

/// A present key, `null` included.
#[expect(clippy::option_option, reason = "an absent key differs from `null`")]
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

fn action(world: &mut World, args: &Value) -> Result<(), String> {
    let args = Args::deserialize(args).map_err(|err| format!("args: {err}"))?;
    let mut actions = Vec::new();
    if let Some(room) = args.room {
        actions.push(Action::Room(room));
    }
    if let Some(name) = args.tool {
        let tool = Tool::named(&name).ok_or_else(|| {
            let names: Vec<&str> = Tool::ALL.iter().map(|tool| tool.name()).collect();
            format!("no tool `{name}`: {}", names.join(", "))
        })?;
        actions.push(Action::Tool(tool));
    }
    if let Some(prefab) = args.prefab {
        actions.push(Action::Prefab(prefab));
    }
    if let Some(tile) = args.tile {
        actions.push(Action::Tile(tile));
    }
    // A palette pick takes up its tool; a tool named beside it stands.
    if let Some(tool) = actions.iter().find_map(|action| match action {
        Action::Tool(tool) => Some(*tool),
        _ => None,
    }) {
        actions.push(Action::Tool(tool));
    }
    if let Some(layer) = args.layer {
        actions.push(Action::Layer(layer));
    }
    if let Some(id) = args.select {
        actions.push(Action::Select(id));
    }
    if let Some(name) = args.does {
        let does = (DOES.iter())
            .find(|(known, _)| *known == name)
            .map(|(_, action)| action.clone())
            .ok_or_else(|| {
                let names: Vec<&str> = DOES.iter().map(|(name, _)| *name).collect();
                format!("`do` names no action `{name}`: {}", names.join(", "))
            })?;
        actions.push(does);
    }
    let mut editor = world
        .remove_resource::<LevelEditor>()
        .ok_or("the World has no level editor")?;
    let result = run(&mut editor, world, args.open, actions);
    world.insert_resource(editor);
    result
}

/// Opens or closes the editor, then carries out `actions` in order,
/// stopping at the first the editor refuses, whose sentence is the error.
fn run(
    editor: &mut LevelEditor,
    world: &mut World,
    open: Option<bool>,
    actions: Vec<Action>,
) -> Result<(), String> {
    match open {
        Some(true) if !editor.available => {
            return Err(
                "the level editor opens only where assets are watched (`assets.hot_reload`)"
                    .to_owned(),
            );
        }
        Some(true) => editor.open(world),
        Some(false) => editor.close(world),
        None => {}
    }
    if !actions.is_empty() && !editor.open {
        return Err("the level editor is closed: `{\"open\": true}` opens it".to_owned());
    }
    // A room is current before the first frame, so a script's first step
    // acts on one.
    let camera = editor.drawn(editor.game_view(world));
    editor.keep_current(world, camera);
    for action in actions {
        editor.refused = false;
        editor.act(world, action);
        if editor.refused {
            return Err(editor.status.clone());
        }
    }
    Ok(())
}
