//! `tungsten level` (redo 7.12): a game's rooms made, edited, read, checked
//! and rendered from the command line.
//!
//! `new` ([`new`](self::new)) writes a room to start from and lists it in
//! the manifest; `map` ([`map`](self::map)) prints a room as text.
//! `new`, `apply`, `query`, `map` and `describe` read the game's files with this command
//! line's own engine, as `schema` does, so they need no build: the manifest
//! `tungsten.json`'s `assets.manifest` names (`assets/manifest.json` when it
//! names none), its tilesets, physics layer names and prefab IDs. `apply`
//! writes the room canonically and appends `{batch, inverse, before,
//! after}` (the room file's FNV-1a hashes) as a line to
//! `target/tungsten/journal/<room>.jsonl`, cargo's ignored folder; `--undo`
//! applies the newest line's inverse and moves the line to
//! `<room>.redo.jsonl` with `at`, the hash of the file as it wrote it,
//! refusing a file that changed outside the journal; `--redo` applies the
//! redo journal's newest batch again, refusing a file that changed since
//! its undo, and an `apply` empties the redo journal. Each of the three
//! writes its files as one set ([`write_set`]): the room, the journal, the
//! redo journal and `--inverse`'s file, in that order, each through
//! `write_atomic`, the files written before a write that fails put back.
//! `check` runs the game's check and prints
//! the room findings alone; `render` writes and runs a script that stages
//! the room and captures it. `generate` runs the game's build with
//! `TUNGSTEN_GENERATE` naming a report, which the engine fills with a
//! generation's rooms (`World::author_rooms`), then decides every room's
//! file before it writes any, each with `write_atomic`, and puts back the
//! files written before a write that fails. It never edits the manifest
//! and appends to no journal: Git undoes a generation.

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

mod map;
mod new;

pub(crate) use new::{DEFAULT_SIZE, NewArgs};

/// `level`'s verb and its arguments.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LevelArgs {
    /// `apply <room> <batch> [--inverse <file>]`, `apply <room> --undo` or
    /// `apply <room> --redo`, each with `--dry-run` or `--check`.
    Apply {
        room: String,
        edit: ApplyEdit,
        inverse: Option<PathBuf>,
        dry_run: bool,
        check: bool,
    },
    /// `new <room> [--size] [--cell] [--tileset] [--tile]... [--dry-run]`.
    New(NewArgs),
    /// `map <room> [--rect <col>,<row>,<w>,<h>]`.
    Map {
        room: String,
        rect: Option<[u32; 4]>,
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
    /// `generate <generator> [--seed <seed>] [--prefix <prefix>]
    /// [--into <folder>] [--regenerate]`.
    Generate {
        generator: String,
        seed: u64,
        prefix: Option<String>,
        into: Option<PathBuf>,
        regenerate: bool,
    },
}

/// What `apply` does to its room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApplyEdit {
    /// Applies the batch in this file, `-` for stdin.
    Batch(PathBuf),
    /// Undoes the journal's newest batch.
    Undo,
    /// Applies the newest undone batch again.
    Redo,
}

/// Runs `args` on the game in `dir`; `false` when it found errors.
pub(crate) fn run(dir: &Path, args: &LevelArgs) -> anyhow::Result<bool> {
    match args {
        LevelArgs::Apply {
            room,
            edit,
            inverse,
            dry_run,
            check,
        } => {
            let game = Game::open(dir)?;
            let message = match edit {
                ApplyEdit::Batch(batch) => {
                    let text = read_batch(dir, batch)?;
                    let options = ApplyOptions {
                        inverse: inverse.as_deref(),
                        dry_run: *dry_run,
                    };
                    apply(&game, room, &text, &options, &mut write_file)?
                }
                ApplyEdit::Undo => undo_last(&game, room, *dry_run, &mut write_file)?,
                ApplyEdit::Redo => redo_last(&game, room, *dry_run, &mut write_file)?,
            };
            println!("{message}");
            if *check && !*dry_run {
                // The room is written; the check's findings say what a
                // further batch, or an undo, should fix.
                return check_rooms(dir, std::slice::from_ref(room), None);
            }
            Ok(true)
        }
        LevelArgs::New(args) => {
            println!("{}", new::new(&Game::open(dir)?, args, &mut write_file)?);
            Ok(true)
        }
        LevelArgs::Map { room, rect } => {
            print!("{}", map::map(&Game::open(dir)?, room, *rect)?);
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
        LevelArgs::Generate {
            generator,
            seed,
            prefix,
            into,
            regenerate,
        } => generate(
            dir,
            generator,
            *seed,
            prefix.as_deref().unwrap_or(generator),
            into.as_deref(),
            *regenerate,
        ),
    }
}

/// A game's folder and its manifest, read with this command line's engine.
pub(crate) struct Game {
    dir: PathBuf,
    manifest: ResolvedManifest,
    /// The manifest's file, whose folder a new room's path is relative to.
    manifest_file: PathBuf,
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
        let manifest_file = manifest;
        let manifest = ResolvedManifest::load(&manifest_file).context("the manifest")?;
        Ok(Self {
            dir: dir.to_path_buf(),
            manifest,
            manifest_file,
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

    /// The redo journal of room `id`: the batches its undos took back, the
    /// next to redo last.
    pub(crate) fn redo_journal(&self, id: &str) -> PathBuf {
        self.dir
            .join("target/tungsten/journal")
            .join(format!("{id}.redo.jsonl"))
    }

    /// What a batch on `room`, the file at `target`, is checked against.
    fn context_of(&self, room: &Room) -> anyhow::Result<(Tileset, PhysicsLayers)> {
        Ok((self.tileset(room)?, self.layers()))
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

/// One file of a command's write set: what a message calls it, where it
/// is, and the text it holds after the command, `None` for a file removed.
struct Write {
    what: &'static str,
    path: PathBuf,
    text: Option<String>,
}

/// How a write set changes one file: the text it takes, or `None` to
/// remove it. [`write_file`] unless a test fails one.
pub(crate) type Writer<'a> = dyn FnMut(&Path, Option<&str>) -> anyhow::Result<()> + 'a;

/// Writes `text` to `path` so a reader sees the old file or the new one
/// (`write_atomic`, which makes the folder), or for `None` removes the
/// file, an absent one included.
pub(crate) fn write_file(path: &Path, text: Option<&str>) -> anyhow::Result<()> {
    match text {
        Some(text) => {
            tungsten_core::persist::write_atomic(path, text.as_bytes()).map_err(anyhow::Error::from)
        }
        None => match std::fs::remove_file(path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        },
    }
}

/// Writes a command's files in order through `write`, all or none: no two
/// of them may be one file (`--inverse` naming the room or a journal),
/// which is refused before anything is written, and a write that fails
/// puts the files written before it back as they were, an absent one
/// removed, and fails the command.
///
/// # Errors
///
/// Two entries naming one file; a write that failed, the error naming any
/// file that could not be put back and so holds the command's result.
fn write_set(game: &Game, set: &[Write], write: &mut Writer<'_>) -> anyhow::Result<()> {
    let keys: Vec<PathBuf> = set.iter().map(|file| file_key(&file.path)).collect();
    for (at, key) in keys.iter().enumerate() {
        if let Some(first) = keys[..at].iter().position(|other| other == key) {
            bail!(
                "{} '{}' is also {}; nothing written",
                set[at].what,
                game.relative(&set[at].path),
                set[first].what
            );
        }
    }
    let old: Vec<Option<String>> = set
        .iter()
        .map(|file| std::fs::read_to_string(&file.path).ok())
        .collect();
    for (at, file) in set.iter().enumerate() {
        let Err(err) = write(&file.path, file.text.as_deref()) else {
            continue;
        };
        let mut stuck = Vec::new();
        for (written, text) in set[..at].iter().zip(&old).rev() {
            if write_file(&written.path, text.as_deref()).is_err() {
                stuck.push(format!(
                    "{} '{}'",
                    written.what,
                    game.relative(&written.path)
                ));
            }
        }
        let back = if stuck.is_empty() {
            "nothing changed".to_owned()
        } else {
            format!(
                "and could not put back {}, which holds the command's result",
                stuck.join(" and ")
            )
        };
        bail!(
            "cannot write {} '{}': {err:#}; {back}",
            file.what,
            game.relative(&file.path)
        );
    }
    Ok(())
}

/// A journal's lines, oldest first; none for a file that is not there.
fn journal_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `lines` as a journal's text; `None`, the file removed, for none.
fn journal_text(lines: &[String]) -> Option<String> {
    (!lines.is_empty()).then(|| {
        lines
            .iter()
            .fold(String::new(), |text, line| text + line + "\n")
    })
}

/// A batch as a report lists it: a line a command, indented.
fn summary(batch: &[RoomCommand]) -> String {
    batch.iter().fold(String::new(), |mut lines, command| {
        let _ = write!(lines, "\n  {}", command.summary());
        lines
    })
}

/// `apply`'s options beside its batch.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ApplyOptions<'a> {
    /// A file that also gets the batch that undoes this one.
    pub(crate) inverse: Option<&'a Path>,
    /// Check and report, writing nothing.
    pub(crate) dry_run: bool,
}

/// Applies the batch `text` to room `arg`: the room written canonically,
/// the batch journaled with its inverse, the redo journal emptied, and the
/// inverse written to `options.inverse` when it names a file, as one
/// [`write_set`]. Returns the report: a line a command, then the commands
/// that undo and check it.
pub(crate) fn apply(
    game: &Game,
    arg: &str,
    text: &str,
    options: &ApplyOptions<'_>,
    write: &mut Writer<'_>,
) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let (before, mut room) = game.room(&target)?;
    let batch = RoomCommand::parse_batch(text).map_err(|e| anyhow::anyhow!("{e}"))?;
    let (tileset, layers) = game.context_of(&room)?;
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
    let file = game.relative(&target.path);
    let commands = count(batch.len(), "command");
    if options.dry_run {
        return Ok(format!(
            "would apply {commands} to {file}:{}\ndry run: nothing written",
            summary(&batch)
        ));
    }
    let mut lines = journal_lines(&game.journal(&target.id));
    lines.push(
        json!({
            "batch": batch,
            "inverse": inverse,
            "before": hash(before.as_bytes()),
            "after": hash(after.as_bytes()),
        })
        .to_string(),
    );
    let mut set = vec![
        Write {
            what: "the room's file",
            path: target.path.clone(),
            text: Some(after),
        },
        Write {
            what: "the journal",
            path: game.journal(&target.id),
            text: journal_text(&lines),
        },
        Write {
            what: "the redo journal",
            path: game.redo_journal(&target.id),
            text: None,
        },
    ];
    if let Some(file) = options.inverse {
        set.push(Write {
            what: "`--inverse`'s file",
            path: game.dir.join(file),
            text: Some(serde_json::to_string_pretty(&inverse)? + "\n"),
        });
    }
    write_set(game, &set, write)?;
    // What `apply` cannot check is the game's: an object's prefab fields.
    let unchecked = batch.iter().any(|command| match command {
        RoomCommand::Place(place) => place.objects.is_some(),
        RoomCommand::Set(set) => set.id.is_some(),
        _ => false,
    });
    let check = if unchecked {
        format!(
            "\ncheck: tungsten level check {} (the level rules and the game's components)",
            target.id
        )
    } else {
        String::new()
    };
    Ok(format!(
        "applied {commands} to {file}:{}\nundo: tungsten level apply {} --undo{check}",
        summary(&batch),
        target.id
    ))
}

/// Undoes room `arg`'s newest journaled batch, refusing a file that changed
/// since it was written: the room written, the journal's line moved to the
/// redo journal with `at`, the hash of the file as written, as one
/// [`write_set`].
pub(crate) fn undo_last(
    game: &Game,
    arg: &str,
    dry_run: bool,
    write: &mut Writer<'_>,
) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let mut lines = journal_lines(&game.journal(&target.id));
    let Some(last) = lines.pop() else {
        bail!("room `{}` has nothing to undo", target.id);
    };
    let entry: Value = serde_json::from_str(&last).context("the journal's newest line")?;
    let (current, mut room) = game.room(&target)?;
    let file = game.relative(&target.path);
    if Some(hash(current.as_bytes()).as_str()) != entry["after"].as_str() {
        bail!("{file} changed outside the journal since its last apply; nothing undone");
    }
    let batch = RoomCommand::batch_from_value(&entry["batch"])
        .map_err(|e| anyhow::anyhow!("the journal's batch: {e}"))?;
    let inverse = RoomCommand::batch_from_value(&entry["inverse"])
        .map_err(|e| anyhow::anyhow!("the journal's inverse: {e}"))?;
    let (tileset, layers) = game.context_of(&room)?;
    let prefabs = game.prefabs();
    let context = RoomContext {
        room: &target.id,
        tileset: &tileset,
        layers: &layers,
        prefabs: &prefabs,
    };
    room.apply(&inverse, &context)
        .map_err(|e| anyhow::anyhow!("{file}: {e}"))?;
    let commands = count(batch.len(), "command");
    if dry_run {
        return Ok(format!(
            "would undo {commands} on {file}:{}\ndry run: nothing written",
            summary(&batch)
        ));
    }
    let text = room.to_canonical();
    let mut undone = journal_lines(&game.redo_journal(&target.id));
    undone.push(json!({ "batch": batch, "at": hash(text.as_bytes()) }).to_string());
    let set = [
        Write {
            what: "the room's file",
            path: target.path.clone(),
            text: Some(text),
        },
        Write {
            what: "the journal",
            path: game.journal(&target.id),
            text: journal_text(&lines),
        },
        Write {
            what: "the redo journal",
            path: game.redo_journal(&target.id),
            text: journal_text(&undone),
        },
    ];
    write_set(game, &set, write)?;
    Ok(format!(
        "undid {commands} on {file}:{}\nredo: tungsten level apply {} --redo",
        summary(&batch),
        target.id
    ))
}

/// Applies room `arg`'s newest undone batch again, refusing a file that
/// changed since the undo wrote it: the room written, the batch journaled
/// with its new inverse and its line dropped from the redo journal, as one
/// [`write_set`].
pub(crate) fn redo_last(
    game: &Game,
    arg: &str,
    dry_run: bool,
    write: &mut Writer<'_>,
) -> anyhow::Result<String> {
    let target = game.target(arg)?;
    let mut undone = journal_lines(&game.redo_journal(&target.id));
    let Some(last) = undone.pop() else {
        bail!("room `{}` has nothing to redo", target.id);
    };
    let entry: Value = serde_json::from_str(&last).context("the redo journal's newest line")?;
    let (current, mut room) = game.room(&target)?;
    let file = game.relative(&target.path);
    if Some(hash(current.as_bytes()).as_str()) != entry["at"].as_str() {
        bail!("{file} changed since its undo; nothing redone");
    }
    let batch = RoomCommand::batch_from_value(&entry["batch"])
        .map_err(|e| anyhow::anyhow!("the redo journal's batch: {e}"))?;
    let (tileset, layers) = game.context_of(&room)?;
    let prefabs = game.prefabs();
    let context = RoomContext {
        room: &target.id,
        tileset: &tileset,
        layers: &layers,
        prefabs: &prefabs,
    };
    let inverse = room
        .apply(&batch, &context)
        .map_err(|e| anyhow::anyhow!("{file}: {e}"))?;
    let commands = count(batch.len(), "command");
    if dry_run {
        return Ok(format!(
            "would redo {commands} on {file}:{}\ndry run: nothing written",
            summary(&batch)
        ));
    }
    let after = room.to_canonical();
    let mut lines = journal_lines(&game.journal(&target.id));
    lines.push(
        json!({
            "batch": batch,
            "inverse": inverse,
            "before": hash(current.as_bytes()),
            "after": hash(after.as_bytes()),
        })
        .to_string(),
    );
    let set = [
        Write {
            what: "the room's file",
            path: target.path.clone(),
            text: Some(after),
        },
        Write {
            what: "the journal",
            path: game.journal(&target.id),
            text: journal_text(&lines),
        },
        Write {
            what: "the redo journal",
            path: game.redo_journal(&target.id),
            text: journal_text(&undone),
        },
    ];
    write_set(game, &set, write)?;
    Ok(format!(
        "redid {commands} on {file}:{}\nundo: tungsten level apply {} --undo",
        summary(&batch),
        target.id
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
        (None, None) => listing(game, &target, &room, json),
        (Some(_), Some(_)) => {
            bail!("`query` takes an entry's <id> or a cell with --at, not both")
        }
    }
}

/// Every layer, shape, object and place of `room` with its geometry: as
/// text a line each, as JSON the entries whole.
fn listing(game: &Game, target: &Target, room: &Room, json: bool) -> anyhow::Result<String> {
    let kind = |layer: &tungsten_core::assets::RoomLayer| {
        serde_json::to_value(layer.kind).unwrap_or_default()
    };
    if json {
        let value = json!({
            "room": target.id,
            "file": game.relative(&target.path),
            "cell": room.cell,
            "size": room.size,
            "tileset": room.tileset,
            "layers": room.layers.iter().map(|layer| json!({
                "name": layer.name,
                "kind": kind(layer),
                "cells": layer.cells.len(),
            })).collect::<Vec<_>>(),
            "shapes": room.shapes,
            "objects": room.objects,
            "places": room.places,
        });
        return Ok(format!("{}\n", serde_json::to_string_pretty(&value)?));
    }
    let points = |points: &[[u32; 2]]| {
        let listed: Vec<String> = points.iter().map(|[x, y]| format!("{x},{y}")).collect();
        listed.join(" ")
    };
    // Each section's rows: an ID, then what follows it.
    let mut sections: Vec<(&str, Vec<(String, String)>)> = Vec::new();
    sections.push((
        "layers",
        (room.layers.iter())
            .map(|layer| {
                let overrides = match layer.cells.len() {
                    0 => String::new(),
                    n => format!(", {}", count(n, "cell override")),
                };
                (
                    layer.name.clone(),
                    format!("{}{overrides}", text(&kind(layer))),
                )
            })
            .collect(),
    ));
    sections.push((
        "shapes",
        (room.shapes.iter())
            .map(|shape| {
                let geometry = if let Some([x, y, w, h]) = shape.rect {
                    format!("rect {x}, {y}, {w} by {h}")
                } else if let Some(polygon) = &shape.polygon {
                    format!("polygon {}", points(polygon))
                } else {
                    format!("line {}", points(shape.line.as_deref().unwrap_or_default()))
                };
                let tile = (shape.tile.as_ref())
                    .map_or_else(|| "erases".to_owned(), |tile| format!("tile {tile}"));
                (
                    shape.id.clone(),
                    format!("{geometry}  {tile}  in {}", shape.layers.join(", ")),
                )
            })
            .collect(),
    ));
    sections.push((
        "objects",
        (room.objects.iter())
            .map(|object| {
                let mut rest = format!("{}  at {}, {}", object.prefab, object.at[0], object.at[1]);
                if let Some(shape) = &object.on {
                    let _ = write!(rest, "  on {shape}");
                }
                if !object.set.is_empty() {
                    let set: Vec<String> = (object.set.iter())
                        .map(|(path, value)| format!("{path}={value}"))
                        .collect();
                    let _ = write!(rest, "  set {}", set.join(" "));
                }
                if let Some(generator) = &object.generator {
                    let _ = write!(rest, "  generator {generator}");
                }
                (object.id.clone(), rest)
            })
            .collect(),
    ));
    sections.push((
        "places",
        (room.places.iter())
            .map(|place| {
                let mut rest = if let Some([x, y]) = place.point {
                    format!("point at {x}, {y}")
                } else if let Some([x, y, w, h]) = place.rect {
                    format!("area at {x}, {y}, {w} by {h}")
                } else {
                    let stops: Vec<String> = (place.route.iter().flatten())
                        .map(|stop| match stop {
                            RouteStop::Point([x, y]) => format!("{x},{y}"),
                            RouteStop::Place(id) => id.clone(),
                            _ => "?".to_owned(),
                        })
                        .collect();
                    format!("route {}", stops.join(" "))
                };
                if let Some(to) = &place.to {
                    let _ = write!(rest, "  to {to}");
                }
                if let Some(by) = &place.by {
                    let _ = write!(rest, "  by {by}");
                }
                (place.id.clone(), rest)
            })
            .collect(),
    ));
    let mut out = format!(
        "{} ({}): {} \u{d7} {} cells of {} \u{d7} {} px{}\n",
        target.id,
        game.relative(&target.path),
        room.size[0],
        room.size[1],
        room.cell[0],
        room.cell[1],
        (room.tileset.as_ref()).map_or_else(String::new, |tileset| format!(", tileset {tileset}"))
    );
    for (name, rows) in sections {
        if rows.is_empty() {
            continue;
        }
        let width = rows.iter().map(|(id, _)| id.len()).max().unwrap_or(0);
        let _ = writeln!(out, "{name}:");
        for (id, rest) in rows {
            let _ = writeln!(out, "  {id:width$}  {rest}");
        }
    }
    Ok(out)
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
    if rooms.is_empty() {
        out.push_str(
            "the manifest lists no room; `tungsten level new <room>` makes one and lists it\n",
        );
    }
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

/// Runs `tungsten level generate` in `dir`: the config stage, the game's
/// build writing generation `generator` at `seed` under `prefix` to a
/// report, then [`finish_generation`] writing it; `false` on a report
/// error.
fn generate(
    dir: &Path,
    generator: &str,
    seed: u64,
    prefix: &str,
    into: Option<&Path>,
    regenerate: bool,
) -> anyhow::Result<bool> {
    check::require_game(dir)?;
    let game = Game::open(dir)?;
    let config = check::config_stage(dir);
    if !config.is_empty() {
        for line in check::format_findings(&config) {
            println!("{line}");
        }
        println!("{}", check::config_summary(config.len()));
        return Ok(false);
    }
    let report = std::env::temp_dir().join(format!(
        "tungsten-level-generate-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&report);
    let mut command = Command::new("cargo");
    command.env(
        check::GENERATE_RUN_ENV,
        format!("{generator}:{seed}:{prefix}"),
    );
    let status = check::run_game(dir, false, &[(check::GENERATE_ENV, &report)], &mut command)?;
    let Ok(text) = std::fs::read_to_string(&report) else {
        bail!(
            "the generation did not run: the game stopped ({status}) before App::run generated; its output above says why"
        );
    };
    let _ = std::fs::remove_file(&report);
    let report = parse_generation(&text)?;
    let (lines, written) =
        finish_generation(&game, &report, into, regenerate, &mut write_room_file)?;
    for line in &lines {
        println!("{line}");
    }
    Ok(written)
}

/// The version of the report a generate run writes.
const GENERATE_REPORT_VERSION: u64 = 1;

/// A generate run's report: its errors, else the generation's generator,
/// seed and templates and its rooms.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GenerateReport {
    pub(crate) errors: Vec<check::Finding>,
    pub(crate) generator: String,
    pub(crate) seed: u64,
    pub(crate) templates: Vec<String>,
    pub(crate) rooms: Vec<GeneratedRoom>,
}

/// One room of a [`GenerateReport`]: its ID, its canonical text, whether
/// it regenerates a listed room, and its counts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GeneratedRoom {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) generated: u64,
    pub(crate) kept: usize,
    pub(crate) dropped: usize,
    pub(crate) reshaped: bool,
}

/// A generate run's report from its JSON.
pub(crate) fn parse_generation(json: &str) -> anyhow::Result<GenerateReport> {
    let value: Value = serde_json::from_str(json).context("the generation's report")?;
    if value["version"].as_u64() != Some(GENERATE_REPORT_VERSION) {
        bail!(
            "the generation's report is version {}; this command line reads version {GENERATE_REPORT_VERSION}",
            value["version"]
        );
    }
    let list = |value: &Value| value.as_array().cloned().unwrap_or_default();
    let errors = list(&value["errors"])
        .iter()
        .map(|error| check::Finding {
            file: text(&error["file"]).to_owned(),
            message: text(&error["message"]).to_owned(),
        })
        .collect();
    let generation = &value["generation"];
    let mut templates: Vec<String> = generation["templates"]
        .as_object()
        .map(|templates| templates.keys().cloned().collect())
        .unwrap_or_default();
    templates.sort();
    let rooms = list(&value["rooms"])
        .iter()
        .map(|room| GeneratedRoom {
            id: text(&room["id"]).to_owned(),
            text: text(&room["text"]).to_owned(),
            generated: room["generated"].as_u64().unwrap_or(0),
            kept: room["kept"].as_array().map_or(0, Vec::len),
            dropped: room["dropped"].as_array().map_or(0, Vec::len),
            reshaped: room["reshaped"].as_bool().unwrap_or(false),
        })
        .collect();
    Ok(GenerateReport {
        errors,
        generator: text(&generation["generator"]).to_owned(),
        seed: generation["seed"].as_u64().unwrap_or(0),
        templates,
        rooms,
    })
}

/// A room file written by `level generate`: `write_atomic`, so a reader
/// sees the old file or the new one; the folder is made.
fn write_room_file(path: &Path, text: &str) -> anyhow::Result<()> {
    tungsten_core::persist::write_atomic(path, text.as_bytes()).map_err(anyhow::Error::from)
}

/// What `level generate` prints for `report`, and whether it wrote its
/// rooms: a report's errors and `false`; else every room's file decided
/// first, then each written through `write`, a line per room and the
/// manifest entry each new room needs.
///
/// # Errors
///
/// A refusal, nothing written: a listed room without `regenerate`, a new
/// room whose file exists or lies outside the game's folder, a file two of
/// the rooms would write, or a file a listed room it does not write or a
/// template it read names (rooms may share a file). A write that fails,
/// after the files written before it are put back.
pub(crate) fn finish_generation(
    game: &Game,
    report: &GenerateReport,
    into: Option<&Path>,
    regenerate: bool,
    write: &mut dyn FnMut(&Path, &str) -> anyhow::Result<()>,
) -> anyhow::Result<(Vec<String>, bool)> {
    if !report.errors.is_empty() {
        let mut lines = check::format_findings(&report.errors);
        lines.push(format!(
            "tungsten level generate: {}; nothing written",
            count(report.errors.len(), "error")
        ));
        return Ok((lines, false));
    }
    let manifest_dir = game
        .manifest_file
        .parent()
        .unwrap_or(&game.dir)
        .to_path_buf();
    let folder = into.map_or_else(|| manifest_dir.join("rooms"), |into| game.dir.join(into));
    // Every room's file, decided before any is written.
    let mut targets: Vec<(&GeneratedRoom, PathBuf, Option<String>)> = Vec::new();
    for room in &report.rooms {
        if let Some(entry) = game.manifest.rooms.get(&room.id) {
            let file = game.relative(&entry.path);
            if !regenerate {
                bail!(
                    "room `{}` is listed ({file}); `--regenerate` rewrites it",
                    room.id
                );
            }
            let old = std::fs::read_to_string(&entry.path)
                .with_context(|| format!("cannot read '{file}'"))?;
            targets.push((room, entry.path.clone(), Some(old)));
            continue;
        }
        let path = folder.join(format!("{}.json", room.id));
        let file = game.relative(&path);
        if !normal(&path).starts_with(normal(&game.dir)) {
            bail!(
                "room `{}`'s file '{file}' is outside the game's folder",
                room.id
            );
        }
        if path.exists() {
            bail!(
                "room `{}`'s file '{file}' exists; a new room is written over no file",
                room.id
            );
        }
        targets.push((room, path, None));
    }
    let keys: Vec<PathBuf> = targets.iter().map(|(_, path, _)| file_key(path)).collect();
    for (at, key) in keys.iter().enumerate() {
        if let Some(first) = keys[..at].iter().position(|other| other == key) {
            bail!(
                "rooms `{}` and `{}` would both write '{}'",
                targets[first].0.id,
                targets[at].0.id,
                game.relative(&targets[at].1)
            );
        }
    }
    let written: BTreeSet<&str> = report.rooms.iter().map(|room| room.id.as_str()).collect();
    let mut listed: Vec<(&String, &PathBuf)> = (game.manifest.rooms.iter())
        .map(|(id, entry)| (id, &entry.path))
        .collect();
    listed.sort();
    // A template's file first, for its own message; then any listed room
    // the generation does not write.
    let named = listed
        .iter()
        .filter(|(id, _)| report.templates.contains(id))
        .map(|(id, path)| (format!("template `{id}`"), *path))
        .chain(
            (listed.iter())
                .filter(|(id, _)| !written.contains(id.as_str()))
                .map(|(id, path)| (format!("room `{id}`"), *path)),
        );
    for (owner, path) in named {
        let key = file_key(path);
        if let Some(at) = keys.iter().position(|target| *target == key) {
            bail!(
                "room `{}`'s file '{}' is also {owner}'s, which the generation does not write; nothing written",
                targets[at].0.id,
                game.relative(path)
            );
        }
    }
    for (at, (room, path, _)) in targets.iter().enumerate() {
        if let Err(err) = write(path, &room.text) {
            let mut failed = Vec::new();
            for (_, path, old) in targets[..at].iter().rev() {
                let restored = match old {
                    Some(old) => write_room_file(path, old),
                    None => std::fs::remove_file(path).map_err(anyhow::Error::from),
                };
                if restored.is_err() {
                    failed.push(game.relative(path));
                }
            }
            let back = if failed.is_empty() {
                format!("{} written before it put back", count(at, "file"))
            } else {
                format!("and could not put back {}", failed.join(", "))
            };
            bail!("cannot write '{}': {err:#}; {back}", game.relative(path));
        }
    }
    let mut lines = Vec::new();
    let mut entries = Vec::new();
    for (room, path, old) in &targets {
        let file = game.relative(path);
        if old.is_some() {
            lines.push(format!(
                "regenerated {file}: {} generated, {} kept, {} dropped{}",
                room.generated,
                room.kept,
                room.dropped,
                if room.reshaped { ", reshaped" } else { "" }
            ));
        } else {
            lines.push(format!("wrote {file}: {} generated", room.generated));
            entries.push(format!(
                "  \"{}\": {{ \"path\": \"{}\" }}",
                room.id,
                relative_to(&manifest_dir, path)
            ));
        }
    }
    if !entries.is_empty() {
        lines.push("add to the manifest's rooms:".to_owned());
        lines.extend(entries);
    }
    lines.push(format!(
        "tungsten level generate: generator `{}` at seed {}, {} written",
        report.generator,
        report.seed,
        count(targets.len(), "room")
    ));
    Ok((lines, true))
}

/// `path` made absolute and its `.` and `..` resolved by name.
fn normal(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// What names `path`'s file: the path canonical when the file exists,
/// else its folder's canonical path when that exists, else [`normal`].
fn file_key(path: &Path) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    match (path.parent().map(Path::canonicalize), path.file_name()) {
        (Some(Ok(folder)), Some(name)) => folder.join(name),
        _ => normal(path),
    }
}

/// `path` relative to the folder `from`, `/`-separated, with `..` where it
/// leaves it.
fn relative_to(from: &Path, path: &Path) -> String {
    let (from, path) = (normal(from), normal(path));
    let common = from
        .components()
        .zip(path.components())
        .take_while(|(a, b)| a == b)
        .count();
    let ups = from.components().count() - common;
    let parts: Vec<String> = std::iter::repeat_n("..".to_owned(), ups)
        .chain(
            path.components()
                .skip(common)
                .map(|part| part.as_os_str().to_string_lossy().into_owned()),
        )
        .collect();
    parts.join("/")
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
