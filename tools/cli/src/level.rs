//! `tungsten level` (redo 7.12): a game's rooms edited, read, checked and
//! rendered from the command line.
//!
//! `apply`, `query` and `describe` read the game's files with this command
//! line's own engine, as `schema` does, so they need no build: the manifest
//! `tungsten.json`'s `assets.manifest` names (`assets/manifest.json` when it
//! names none), its tilesets, physics layer names and prefab IDs. `apply`
//! writes the room canonically and appends `{batch, inverse, before,
//! after}` (the room file's FNV-1a hashes) as a line to
//! `target/tungsten/journal/<room>.jsonl`, cargo's ignored folder; `--undo`
//! applies the newest line's inverse and drops it, refusing a file that
//! changed outside the journal. `check` runs the game's check and prints
//! the room findings alone; `render` writes and runs a script that stages
//! the room and captures it.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use serde_json::{Value, json};
use tungsten_core::Config;
use tungsten_core::assets::{
    ResolvedManifest, Room, RoomCommand, RoomContext, RoomPlace, RouteStop, Tileset,
};
use tungsten_core::physics::PhysicsLayers;

use crate::check;
use crate::render::{self, RenderArgs};

/// `level`'s verb and its arguments.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LevelArgs {
    /// `apply <room> <batch> [--inverse <file>]` or `apply <room> --undo`.
    Apply {
        room: String,
        batch: Option<PathBuf>,
        inverse: Option<PathBuf>,
        undo: bool,
    },
    /// `query <room> [<id>] [--at <col>,<row>] [--json]`.
    Query {
        room: String,
        id: Option<String>,
        at: Option<[u32; 2]>,
        json: bool,
    },
    /// `describe [<room>] [--json]`.
    Describe { room: Option<String>, json: bool },
    /// `check [<room>...] [--seeds <count> | --seed <seed>]`.
    Check {
        rooms: Vec<String>,
        seeds: Option<check::Seeds>,
    },
    /// `render <room> -o <png> [--overlay] [--size] [--frame] [--release]`.
    Render { args: RenderArgs },
}

/// Runs `args` on the game in `dir`; `false` when it found errors.
pub(crate) fn run(dir: &Path, args: &LevelArgs) -> anyhow::Result<bool> {
    match args {
        LevelArgs::Apply {
            room,
            batch,
            inverse,
            undo,
        } => {
            let game = Game::open(dir)?;
            let message = if *undo {
                undo_last(&game, room)?
            } else {
                let batch = batch.as_deref().context("`apply` needs a batch")?;
                let text = read_batch(dir, batch)?;
                apply(&game, room, &text, inverse.as_deref())?
            };
            println!("{message}");
            Ok(true)
        }
        LevelArgs::Query { room, id, at, json } => {
            print!(
                "{}",
                query(&Game::open(dir)?, room, id.as_deref(), *at, *json)?
            );
            Ok(true)
        }
        LevelArgs::Describe { room, json } => {
            print!("{}", describe(&Game::open(dir)?, room.as_deref(), *json)?);
            Ok(true)
        }
        LevelArgs::Check { rooms, seeds } => check_rooms(dir, rooms, *seeds),
        LevelArgs::Render { args } => {
            check::require_game(dir)?;
            render::run(dir, args)
        }
    }
}

/// A game's folder and its manifest, read with this command line's engine.
pub(crate) struct Game {
    dir: PathBuf,
    manifest: ResolvedManifest,
}

/// A room the arguments name: its ID and its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) id: String,
    pub(crate) path: PathBuf,
}

impl Game {
    /// The game in `dir`: `tungsten.json`'s manifest, or
    /// `assets/manifest.json`.
    pub(crate) fn open(dir: &Path) -> anyhow::Result<Self> {
        check::require_game_folder(dir)?;
        let config = Config::load(dir.join("tungsten.json")).context("tungsten.json")?;
        let manifest = config
            .assets
            .manifest
            .map_or_else(|| dir.join("assets/manifest.json"), |path| dir.join(path));
        if !manifest.is_file() {
            bail!(
                "no manifest at '{}': tungsten.json's assets.manifest names the game's",
                manifest.display()
            );
        }
        let manifest = ResolvedManifest::load(&manifest).context("the manifest")?;
        Ok(Self {
            dir: dir.to_path_buf(),
            manifest,
        })
    }

    /// The room `arg` names: a `rooms` ID, or a path to a room file, whose
    /// ID is the entry's that lists it, else the file's stem.
    pub(crate) fn target(&self, arg: &str) -> anyhow::Result<Target> {
        if let Some(entry) = self.manifest.rooms.get(arg) {
            return Ok(Target {
                id: arg.to_owned(),
                path: entry.path.clone(),
            });
        }
        let path = self.dir.join(arg);
        let is_path = arg.contains('/')
            || arg.contains('\\')
            || Path::new(arg)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if !is_path || !path.is_file() {
            let mut ids: Vec<&String> = self.manifest.rooms.keys().collect();
            ids.sort();
            let ids: Vec<&str> = ids.into_iter().map(String::as_str).collect();
            bail!(
                "no room `{arg}`: name a rooms ID ({}) or a room file",
                if ids.is_empty() {
                    "the manifest lists none".to_owned()
                } else {
                    ids.join(", ")
                }
            );
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        let listed = self.manifest.rooms.iter().find(|(_, entry)| {
            entry
                .path
                .canonicalize()
                .unwrap_or_else(|_| entry.path.clone())
                == canonical
        });
        let id = match listed {
            Some((id, _)) => id.clone(),
            None => path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        Ok(Target {
            id,
            path: canonical,
        })
    }

    /// The room file at `target`, read and checked.
    fn room(&self, target: &Target) -> anyhow::Result<(String, Room)> {
        let text = std::fs::read_to_string(&target.path)
            .with_context(|| format!("cannot read '{}'", self.relative(&target.path)))?;
        let room = Room::from_json(&text)
            .map_err(|e| anyhow::anyhow!("{}: {e}", self.relative(&target.path)))?;
        Ok((text, room))
    }

    /// The tileset `room` names, read; an empty one for a room without.
    fn tileset(&self, room: &Room) -> anyhow::Result<Tileset> {
        let Some(id) = &room.tileset else {
            return Ok(Tileset::from_json(r#"{ "version": 1 }"#).expect("an empty tileset"));
        };
        let entry = self
            .manifest
            .tilesets
            .get(id)
            .with_context(|| format!("the manifest lists no tileset `{id}`"))?;
        let text = std::fs::read_to_string(&entry.path)
            .with_context(|| format!("cannot read '{}'", self.relative(&entry.path)))?;
        Tileset::from_json(&text)
            .map_err(|e| anyhow::anyhow!("{}: {e}", self.relative(&entry.path)))
    }

    fn layers(&self) -> PhysicsLayers {
        PhysicsLayers::new(
            self.manifest
                .physics_layers
                .iter()
                .map(|(name, &layer)| (name.clone(), layer)),
        )
    }

    fn prefabs(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.manifest.prefabs.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }

    /// `path` relative to the game's folder, `/`-separated.
    fn relative(&self, path: &Path) -> String {
        let dir = self.dir.canonicalize().unwrap_or_else(|_| self.dir.clone());
        let path = path
            .strip_prefix(&dir)
            .or_else(|_| path.strip_prefix(&self.dir))
            .unwrap_or(path);
        let parts: Vec<String> = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        parts.join("/")
    }

    /// The journal file of room `id`.
    pub(crate) fn journal(&self, id: &str) -> PathBuf {
        self.dir
            .join("target/tungsten/journal")
            .join(format!("{id}.jsonl"))
    }
}

/// A batch's text: the file `batch` names, relative to `dir`, or stdin for
/// `-`.
fn read_batch(dir: &Path, batch: &Path) -> anyhow::Result<String> {
    if batch == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("cannot read the batch from stdin")?;
        return Ok(text);
    }
    std::fs::read_to_string(dir.join(batch))
        .with_context(|| format!("cannot read the batch '{}'", batch.display()))
}

/// A file's FNV-1a 64-bit hash, as 16 hex digits.
pub(crate) fn hash(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Applies the batch `text` to room `arg`: the room written canonically,
/// the batch journaled with its inverse, which `inverse` also gets when it
/// names a file.
pub(crate) fn apply(
    game: &Game,
    arg: &str,
    text: &str,
    inverse_file: Option<&Path>,
) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let (before, mut room) = game.room(&target)?;
    let batch = RoomCommand::parse_batch(text).map_err(|e| anyhow::anyhow!("{e}"))?;
    let tileset = game.tileset(&room)?;
    let layers = game.layers();
    let prefabs = game.prefabs();
    let context = RoomContext {
        room: &target.id,
        tileset: &tileset,
        layers: &layers,
        prefabs: &prefabs,
    };
    let inverse = room
        .apply(&batch, &context)
        .map_err(|e| anyhow::anyhow!("{}: {e}", game.relative(&target.path)))?;
    let after = room.to_canonical();
    std::fs::write(&target.path, &after)
        .with_context(|| format!("cannot write '{}'", game.relative(&target.path)))?;
    let line = json!({
        "batch": batch,
        "inverse": inverse,
        "before": hash(before.as_bytes()),
        "after": hash(after.as_bytes()),
    });
    let journal = game.journal(&target.id);
    if let Some(parent) = journal.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot make '{}'", parent.display()))?;
    }
    let mut lines = std::fs::read_to_string(&journal).unwrap_or_default();
    lines.push_str(&line.to_string());
    lines.push('\n');
    std::fs::write(&journal, lines)
        .with_context(|| format!("cannot write the journal '{}'", journal.display()))?;
    if let Some(file) = inverse_file {
        let text = serde_json::to_string_pretty(&inverse)? + "\n";
        std::fs::write(game.dir.join(file), text)
            .with_context(|| format!("cannot write '{}'", file.display()))?;
    }
    Ok(format!(
        "applied {} to {}; `tungsten level apply {} --undo` undoes it",
        count(batch.len(), "command"),
        game.relative(&target.path),
        target.id
    ))
}

/// Undoes room `arg`'s newest journaled batch, refusing a file that changed
/// since it was written.
pub(crate) fn undo_last(game: &Game, arg: &str) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let journal = game.journal(&target.id);
    let text = std::fs::read_to_string(&journal).unwrap_or_default();
    let mut lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    let Some(last) = lines.pop() else {
        bail!("room `{}` has nothing to undo", target.id);
    };
    let entry: Value = serde_json::from_str(last).context("the journal's newest line")?;
    let (current, mut room) = game.room(&target)?;
    if Some(hash(current.as_bytes()).as_str()) != entry["after"].as_str() {
        bail!(
            "{} changed outside the journal since its last apply; nothing undone",
            game.relative(&target.path)
        );
    }
    let inverse = RoomCommand::batch_from_value(&entry["inverse"])
        .map_err(|e| anyhow::anyhow!("the journal's inverse: {e}"))?;
    let tileset = game.tileset(&room)?;
    let layers = game.layers();
    let prefabs = game.prefabs();
    let context = RoomContext {
        room: &target.id,
        tileset: &tileset,
        layers: &layers,
        prefabs: &prefabs,
    };
    room.apply(&inverse, &context)
        .map_err(|e| anyhow::anyhow!("{}: {e}", game.relative(&target.path)))?;
    std::fs::write(&target.path, room.to_canonical())
        .with_context(|| format!("cannot write '{}'", game.relative(&target.path)))?;
    let rest: String = lines.iter().fold(String::new(), |mut rest, line| {
        rest.push_str(line);
        rest.push('\n');
        rest
    });
    std::fs::write(&journal, rest)
        .with_context(|| format!("cannot write the journal '{}'", journal.display()))?;
    Ok(format!(
        "undid the last batch on {}",
        game.relative(&target.path)
    ))
}

/// An entry of room `arg` by ID, or what lies at a cell.
pub(crate) fn query(
    game: &Game,
    arg: &str,
    id: Option<&str>,
    at: Option<[u32; 2]>,
    json: bool,
) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let (_, room) = game.room(&target)?;
    match (id, at) {
        (Some(id), None) => {
            let (kind, index, entry) = entry(&room, id)
                .with_context(|| format!("room `{}` has no entry `{id}`", target.id))?;
            if json {
                return Ok(format!(
                    "{}\n",
                    serde_json::to_string_pretty(
                        &json!({ "kind": kind, "index": index, "entry": entry })
                    )?
                ));
            }
            Ok(format!(
                "{kind} {id} ({kind}s[{index}]):\n{}\n",
                serde_json::to_string_pretty(&entry)?
            ))
        }
        (None, Some([col, row])) => {
            let cell = at_cell(game, &target, &room, col, row)?;
            if json {
                return Ok(format!("{}\n", serde_json::to_string_pretty(&cell)?));
            }
            Ok(cell_text(&target.id, &cell))
        }
        _ => bail!("`query` names an entry's <id> or a cell with --at, one of them"),
    }
}

/// Entry `id`'s kind, index and JSON.
fn entry(room: &Room, id: &str) -> Option<(&'static str, usize, Value)> {
    let json = |value: Result<Value, serde_json::Error>| value.unwrap_or_default();
    if let Some(at) = room.shapes.iter().position(|shape| shape.id == id) {
        return Some(("shape", at, json(serde_json::to_value(&room.shapes[at]))));
    }
    if let Some(at) = room.objects.iter().position(|object| object.id == id) {
        return Some(("object", at, json(serde_json::to_value(&room.objects[at]))));
    }
    room.places
        .iter()
        .position(|place| place.id == id)
        .map(|at| ("place", at, json(serde_json::to_value(&room.places[at]))))
}

/// What lies at cell `(col, row)`: each layer's tile and whether it is an
/// auto cell, the shapes over it, the objects whose `at` falls in it and
/// the places holding it.
fn at_cell(game: &Game, target: &Target, room: &Room, col: u32, row: u32) -> anyhow::Result<Value> {
    if col >= room.size[0] || row >= room.size[1] {
        bail!(
            "cell ({col}, {row}) is past room `{}`'s {} \u{d7} {} cells",
            target.id,
            room.size[0],
            room.size[1]
        );
    }
    let tileset = game.tileset(room)?;
    let layers = game.layers();
    let compile = |room: &Room| {
        room.compile(&target.id, &tileset, &layers)
            .map_err(|e| anyhow::anyhow!("{}: {e}", game.relative(&target.path)))
    };
    let mut map = compile(room)?.map;
    map.auto_tile();
    let layer_cells: Vec<Value> = map
        .layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let tile = map.tile_at(index, col, row).unwrap_or(-1);
            let sprite = usize::try_from(tile)
                .ok()
                .and_then(|tile| map.tileset.get(tile).cloned());
            json!({
                "name": layer.name,
                "tile": sprite,
                "auto": map.is_auto(index, col, row),
            })
        })
        .collect();
    // A shape is over the cell when it alone paints it: an eraser is probed
    // with a plain tile in its place.
    let plain = tileset
        .tiles
        .iter()
        .find(|tile| tile.style.is_none())
        .map(|tile| tile.sprite.clone());
    let mut shapes = Vec::new();
    for shape in &room.shapes {
        let mut alone = room.clone();
        let mut probe = shape.clone();
        if probe.tile.is_none() {
            probe.tile.clone_from(&plain);
        }
        alone.shapes = vec![probe];
        // Objects and places draw nothing, and name shapes the probe lacks.
        alone.objects.clear();
        alone.places.clear();
        for layer in &mut alone.layers {
            layer.cells.clear();
        }
        let Ok(compiled) = compile(&alone) else {
            continue;
        };
        let over = (0..compiled.map.layers.len()).any(|layer| {
            compiled
                .map
                .tile_at(layer, col, row)
                .is_some_and(|tile| tile >= 0)
        });
        if over {
            shapes.push(shape.id.clone());
        }
    }
    let inside =
        |[x, y]: [f32; 2]| x.floor() as i64 == i64::from(col) && y.floor() as i64 == i64::from(row);
    let objects: Vec<&str> = room
        .objects
        .iter()
        .filter(|object| inside(object.at))
        .map(|object| object.id.as_str())
        .collect();
    let places: Vec<&str> = room
        .places
        .iter()
        .filter(|place| holds(place, col, row, &inside))
        .map(|place| place.id.as_str())
        .collect();
    Ok(json!({
        "room": target.id,
        "cell": [col, row],
        "layers": layer_cells,
        "shapes": shapes,
        "objects": objects,
        "places": places,
    }))
}

/// Whether `place` holds cell `(col, row)`: a point in it, an area over
/// it, a route with a point stop in it.
fn holds(place: &RoomPlace, col: u32, row: u32, inside: &dyn Fn([f32; 2]) -> bool) -> bool {
    if let Some(point) = place.point {
        return inside(point);
    }
    if let Some([x, y, w, h]) = place.rect {
        let (cx, cy) = (col as f32 + 0.5, row as f32 + 0.5);
        return cx >= x && cx <= x + w && cy >= y && cy <= y + h;
    }
    place.route.iter().flatten().any(|stop| match stop {
        RouteStop::Point(point) => inside(*point),
        _ => false,
    })
}

/// [`at_cell`]'s value as text.
fn cell_text(room: &str, cell: &Value) -> String {
    let mut out = format!("{room} ({}, {}):\n", cell["cell"][0], cell["cell"][1]);
    for layer in cell["layers"].as_array().into_iter().flatten() {
        let tile = layer["tile"].as_str().unwrap_or("empty");
        let auto = if layer["auto"] == true { " (auto)" } else { "" };
        let _ = writeln!(out, "  {}: {tile}{auto}", text(&layer["name"]));
    }
    for (kind, key) in [
        ("shapes", "shapes"),
        ("objects", "objects"),
        ("places", "places"),
    ] {
        let ids: Vec<&str> = cell[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let listed = if ids.is_empty() {
            "none".to_owned()
        } else {
            ids.join(", ")
        };
        let _ = writeln!(out, "  {kind}: {listed}");
    }
    out
}

/// Each room's layers and named things by kind, links with their targets:
/// every listed room, or the one `arg` names.
pub(crate) fn describe(game: &Game, arg: Option<&str>, json: bool) -> anyhow::Result<String> {
    let targets = if let Some(arg) = arg {
        vec![game.target(arg)?]
    } else {
        let mut ids: Vec<&String> = game.manifest.rooms.keys().collect();
        ids.sort();
        ids.into_iter()
            .map(|id| game.target(id))
            .collect::<anyhow::Result<_>>()?
    };
    let mut rooms = Vec::new();
    for target in &targets {
        let (_, room) = game.room(target)?;
        let place_kind = |place: &RoomPlace| {
            if place.point.is_some() {
                "point"
            } else if place.rect.is_some() {
                "rect"
            } else {
                "route"
            }
        };
        rooms.push(json!({
            "id": target.id,
            "file": game.relative(&target.path),
            "cell": room.cell,
            "size": room.size,
            "tileset": room.tileset,
            "layers": room.layers.iter().map(|layer| json!({
                "name": layer.name,
                "kind": serde_json::to_value(layer.kind).unwrap_or_default(),
            })).collect::<Vec<_>>(),
            "shapes": room.shapes.iter().map(|shape| shape.id.as_str()).collect::<Vec<_>>(),
            "objects": room.objects.iter().map(|object| json!({
                "id": object.id, "prefab": object.prefab,
            })).collect::<Vec<_>>(),
            "places": room.places.iter().map(|place| {
                let mut entry = json!({ "id": place.id, "kind": place_kind(place) });
                if let Some(by) = &place.by {
                    entry["by"] = json!(by);
                }
                entry
            }).collect::<Vec<_>>(),
            "links": room.places.iter().filter_map(|place| {
                place.to.as_ref().map(|to| json!({ "id": place.id, "to": to }))
            }).collect::<Vec<_>>(),
        }));
    }
    if json {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({ "rooms": rooms }))?
        ));
    }
    let mut out = String::new();
    for room in &rooms {
        let _ = writeln!(
            out,
            "{} ({}): {} \u{d7} {} cells of {} \u{d7} {} px{}",
            text(&room["id"]),
            text(&room["file"]),
            room["size"][0],
            room["size"][1],
            room["cell"][0],
            room["cell"][1],
            room["tileset"]
                .as_str()
                .map_or_else(String::new, |tileset| format!(", tileset {tileset}"))
        );
        let list = |out: &mut String, kind: &str, items: Vec<String>| {
            if !items.is_empty() {
                let _ = writeln!(out, "  {kind} ({}): {}", items.len(), items.join(", "));
            }
        };
        let array = |key: &str| room[key].as_array().cloned().unwrap_or_default();
        list(
            &mut out,
            "layers",
            array("layers")
                .iter()
                .map(|layer| format!("{} ({})", text(&layer["name"]), text(&layer["kind"])))
                .collect(),
        );
        list(
            &mut out,
            "shapes",
            array("shapes")
                .iter()
                .map(|id| text(id).to_owned())
                .collect(),
        );
        list(
            &mut out,
            "objects",
            array("objects")
                .iter()
                .map(|object| format!("{} ({})", text(&object["id"]), text(&object["prefab"])))
                .collect(),
        );
        list(
            &mut out,
            "places",
            array("places")
                .iter()
                .map(|place| match place["by"].as_str() {
                    Some(by) => {
                        format!("{} ({} by {by})", text(&place["id"]), text(&place["kind"]))
                    }
                    None => format!("{} ({})", text(&place["id"]), text(&place["kind"])),
                })
                .collect(),
        );
        list(
            &mut out,
            "links",
            array("links")
                .iter()
                .map(|link| format!("{} \u{2192} {}", text(&link["id"]), text(&link["to"])))
                .collect(),
        );
    }
    Ok(out)
}

/// Runs the game's check and prints the findings in room and tileset
/// files, of `rooms` alone when it names any; `false` on any.
fn check_rooms(dir: &Path, rooms: &[String], seeds: Option<check::Seeds>) -> anyhow::Result<bool> {
    check::require_game(dir)?;
    let game = Game::open(dir)?;
    let mut files: BTreeSet<String> = BTreeSet::new();
    if rooms.is_empty() {
        for entry in game
            .manifest
            .rooms
            .values()
            .chain(game.manifest.tilesets.values())
        {
            files.insert(game.relative(&entry.path));
        }
    } else {
        for arg in rooms {
            files.insert(game.relative(&game.target(arg)?.path));
        }
    }
    let config = check::config_stage(dir);
    if !config.is_empty() {
        for line in check::format_findings(&config) {
            println!("{line}");
        }
        println!("{}", check::config_summary(config.len()));
        return Ok(false);
    }
    let report =
        std::env::temp_dir().join(format!("tungsten-level-check-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&report);
    let status = check::content_stage(dir, &report, seeds, &mut Command::new("cargo"))?;
    let report = check::read_report(&report, status)?;
    let (lines, found) = check_lines(&report, &files, seeds);
    for line in &lines {
        println!("{line}");
    }
    println!(
        "tungsten level check: {}",
        if found == 0 {
            "no room findings".to_owned()
        } else {
            count(found, "room finding")
        }
    );
    Ok(found == 0)
}

/// What `level check` prints before its summary, and how many findings:
/// the report's notes ([`check::note_lines`]), then the findings in
/// `files` and every room generator's.
pub(crate) fn check_lines(
    report: &check::Report,
    files: &BTreeSet<String>,
    seeds: Option<check::Seeds>,
) -> (Vec<String>, usize) {
    let mut lines = check::note_lines(report, seeds);
    let found = room_findings(&report.findings, files);
    let count = found.len();
    lines.extend(found);
    (lines, count)
}

/// The findings in `files` and every room generator's (filed under
/// `generator <id>`), as `<file>: <message>`.
pub(crate) fn room_findings(findings: &[check::Finding], files: &BTreeSet<String>) -> Vec<String> {
    let kept: Vec<check::Finding> = findings
        .iter()
        .filter(|finding| files.contains(&finding.file) || finding.file.starts_with("generator "))
        .cloned()
        .collect();
    check::format_findings(&kept)
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}

#[cfg(test)]
#[path = "tests/level.rs"]
mod tests;
