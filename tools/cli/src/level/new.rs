//! `tungsten level new` (editor-pass): a room file to start from, a tileset
//! file when the game has none to build on, and both listed in the
//! manifest.
//!
//! The room is `<manifest folder>/rooms/<room>.json`, in its canonical
//! form: its size and cell, the tileset it names, a render layer `art` and
//! a collision layer `solid`, so terrain painted into both draws and
//! collides. A new tileset is `<manifest folder>/tilesets/<id>.json` with
//! the tiles `--tile` names. The manifest takes the entries by a textual
//! insert ([`insert_entry`]) that keeps the file's formatting, since the
//! engine has no writer for a manifest; the result is read back and must be
//! the old manifest plus the entries, else nothing is written and the
//! entries are printed for a hand edit. The files are one write set, as an
//! apply's are.

use anyhow::{Context, bail};
use serde_json::{Value, json};
use tungsten_core::assets::{Room, Tileset};

use super::{Game, Write, Writer, write_set};

/// `new`'s arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewArgs {
    /// The room's ID.
    pub(crate) room: String,
    /// Its size in cells, `[columns, rows]`.
    pub(crate) size: [u32; 2],
    /// A cell's size in pixels; the tileset's other rooms', else 16, when
    /// absent.
    pub(crate) cell: Option<[u32; 2]>,
    /// The tileset it builds on; the game's only one, or a new `tiles`,
    /// when absent.
    pub(crate) tileset: Option<String>,
    /// A new tileset's tiles, by sprite.
    pub(crate) tiles: Vec<String>,
    /// Report what would be written, writing nothing.
    pub(crate) dry_run: bool,
}

/// The size a room has when `--size` gives none: 40 by 24 cells.
pub(crate) const DEFAULT_SIZE: [u32; 2] = [40, 24];
/// The cell a room has when neither `--cell` nor another room on its
/// tileset gives one.
const DEFAULT_CELL: [u32; 2] = [16, 16];
/// The tileset a game with none gets.
const DEFAULT_TILESET: &str = "tiles";

/// Makes room `args.room` in `game`: its file, a new tileset's file and
/// their manifest entries, as one write set through `write`. Returns the
/// report.
///
/// # Errors
///
/// An ID that is no ID or that a room or tilemap holds; a tileset that
/// cannot be chosen; `--tile` with a listed tileset or naming no sprite; a
/// file that exists; a manifest the entries cannot be inserted into; a
/// write that fails. Nothing is written on any of them.
pub(crate) fn new(game: &Game, args: &NewArgs, write: &mut Writer<'_>) -> anyhow::Result<String> {
    let id = args.room.as_str();
    if !valid_id(id) {
        bail!("`{id}` is no room ID: lowercase letters, digits and `_`, a letter first");
    }
    if let Some(entry) = game.manifest.rooms.get(id) {
        bail!(
            "room `{id}` is listed already ({}); `tungsten level apply {id} <batch>` edits it",
            game.relative(&entry.path)
        );
    }
    if game.manifest.tilemaps.contains_key(id) {
        bail!("`{id}` is a tilemap's ID; a room's ID is its map's, so pick another");
    }
    let mut listed: Vec<&str> = game.manifest.tilesets.keys().map(String::as_str).collect();
    listed.sort_unstable();
    let tileset = match (&args.tileset, listed.as_slice()) {
        (Some(tileset), _) => tileset.clone(),
        (None, [only]) => (*only).to_owned(),
        (None, []) => DEFAULT_TILESET.to_owned(),
        (None, several) => bail!(
            "the game lists {} tilesets ({}): name the room's with `--tileset <id>`",
            several.len(),
            several.join(", ")
        ),
    };
    if !valid_id(&tileset) {
        bail!("`{tileset}` is no tileset ID: lowercase letters, digits and `_`, a letter first");
    }
    let new_tileset = !game.manifest.tilesets.contains_key(&tileset);
    if !new_tileset && !args.tiles.is_empty() {
        bail!(
            "`--tile` names a new tileset's tiles, and tileset `{tileset}` is listed; add tiles to its file, or name a new one with `--tileset <id>`"
        );
    }
    for sprite in &args.tiles {
        if !game.manifest.sprites.contains_key(sprite) {
            bail!(
                "`--tile {sprite}`: the manifest lists no sprite `{sprite}`{}",
                some_of("its sprites", game.manifest.sprites.keys())
            );
        }
        if args.tiles.iter().filter(|other| *other == sprite).count() > 1 {
            bail!("`--tile {sprite}` is given twice; a tileset names a sprite once");
        }
    }
    let cell = args.cell.unwrap_or_else(|| {
        let mut rooms: Vec<&String> = game.manifest.rooms.keys().collect();
        rooms.sort();
        rooms
            .into_iter()
            .filter_map(|other| {
                let text = std::fs::read_to_string(&game.manifest.rooms[other].path).ok()?;
                Room::from_json(&text).ok()
            })
            .find(|room| room.tileset.as_deref() == Some(tileset.as_str()))
            .map_or(DEFAULT_CELL, |room| room.cell)
    });

    let folder = game
        .manifest_file
        .parent()
        .unwrap_or(&game.dir)
        .to_path_buf();
    let room_path = folder.join("rooms").join(format!("{id}.json"));
    let tileset_path = folder.join("tilesets").join(format!("{tileset}.json"));
    for path in std::iter::once(&room_path).chain(new_tileset.then_some(&tileset_path)) {
        if path.exists() {
            bail!(
                "'{}' exists; a new room is written over no file",
                game.relative(path)
            );
        }
    }
    let room = Room::from_json(
        &json!({
            "version": 1,
            "cell": cell,
            "size": args.size,
            "tileset": tileset,
            "layers": [{ "name": "art" }, { "name": "solid", "kind": "collision" }],
        })
        .to_string(),
    )
    .map_err(|err| anyhow::anyhow!("the room: {err}"))?;
    let tiles: Vec<Value> = (args.tiles.iter())
        .map(|sprite| json!({ "sprite": sprite }))
        .collect();
    let tileset_text = Tileset::from_json(&json!({ "version": 1, "tiles": tiles }).to_string())
        .map_err(|err| anyhow::anyhow!("the tileset: {err}"))?
        .to_canonical();

    // The manifest with the entries, read back against the old one.
    let manifest_file = game.relative(&game.manifest_file);
    let old = std::fs::read_to_string(&game.manifest_file)
        .with_context(|| format!("cannot read '{manifest_file}'"))?;
    let mut entries = vec![("rooms", id, format!("rooms/{id}.json"))];
    if new_tileset {
        entries.push((
            "tilesets",
            tileset.as_str(),
            format!("tilesets/{tileset}.json"),
        ));
    }
    let by_hand = || {
        let lines: Vec<String> = entries
            .iter()
            .map(|(section, entry, path)| {
                format!("  in `{section}`: \"{entry}\": {{ \"path\": \"{path}\" }}")
            })
            .collect();
        format!(
            "cannot add the entries to '{manifest_file}' and keep its formatting; nothing written. Add them by hand, then run this again:\n{}",
            lines.join("\n")
        )
    };
    let mut text = old.clone();
    let mut expected: Value =
        serde_json::from_str(&old).with_context(|| format!("'{manifest_file}' is not JSON"))?;
    for (section, entry, path) in &entries {
        text = insert_entry(&text, section, entry, path).with_context(by_hand)?;
        if !expected[*section].is_object() {
            expected[*section] = json!({});
        }
        expected[*section][*entry] = json!({ "path": path });
    }
    if serde_json::from_str::<Value>(&text).ok().as_ref() != Some(&expected) {
        bail!(by_hand());
    }

    let size = format!(
        "{} \u{d7} {} cells of {} \u{d7} {} px",
        args.size[0], args.size[1], cell[0], cell[1]
    );
    let tiles = match args.tiles.len() {
        0 => "no tiles yet".to_owned(),
        1 => format!("1 tile ({})", args.tiles[0]),
        n => format!("{n} tiles ({})", args.tiles.join(", ")),
    };
    let room_file = game.relative(&room_path);
    let tileset_file = game.relative(&tileset_path);
    let mut lines = Vec::new();
    let (wrote, listed_verb) = if args.dry_run {
        ("would write", "would list")
    } else {
        ("wrote", "listed")
    };
    lines.push(format!(
        "{wrote} {room_file}: {size} on tileset `{tileset}`, layers `art` (render) and `solid` (collision)"
    ));
    if new_tileset {
        lines.push(format!("{wrote} {tileset_file}: {tiles}"));
        lines.push(format!(
            "{listed_verb} room `{id}` and tileset `{tileset}` in {manifest_file}"
        ));
    } else {
        lines.push(format!("{listed_verb} room `{id}` in {manifest_file}"));
    }
    if args.dry_run {
        lines.push("dry run: nothing written".to_owned());
        return Ok(lines.join("\n"));
    }
    let mut set = vec![Write {
        what: "the room's file",
        path: room_path,
        text: Some(room.to_canonical()),
    }];
    if new_tileset {
        set.push(Write {
            what: "the tileset's file",
            path: tileset_path,
            text: Some(tileset_text),
        });
    }
    set.push(Write {
        what: "the manifest",
        path: game.manifest_file.clone(),
        text: Some(text),
    });
    write_set(game, &set, write)?;
    if new_tileset && args.tiles.is_empty() {
        lines.push(format!(
            "next: add tiles to {tileset_file}, each `{{ \"sprite\": \"<a sprites ID>\" }}`; a room paints with them"
        ));
    }
    lines.push(format!(
        "next: tungsten level apply {id} <batch> builds it (`tungsten level apply --help` lists the commands)"
    ));
    lines.push(format!(
        "      tungsten level map {id} and tungsten level render {id} -o {id}.png show it"
    ));
    lines.push(format!(
        "      in the game: world.spawn_room(\"{id}\", origin, &with)"
    ));
    Ok(lines.join("\n"))
}

/// Whether `id` is an ID: `[a-z][a-z0-9_]*`.
fn valid_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// "; `what`: a, b, c" for up to twelve of `names`, sorted, then how many
/// more; nothing for none.
fn some_of<'a>(what: &str, names: impl Iterator<Item = &'a String>) -> String {
    let mut names: Vec<&str> = names.map(String::as_str).collect();
    names.sort_unstable();
    if names.is_empty() {
        return String::new();
    }
    let shown = names.len().min(12);
    let more = match names.len() - shown {
        0 => String::new(),
        n => format!(" and {n} more"),
    };
    format!("; {what}: {}{more}", names[..shown].join(", "))
}

/// `text`, a manifest, with `"<id>": { "path": "<path>" }` added to its
/// top-level `section` object, which is added last when the manifest has
/// none. Only the bytes of the new entry are inserted: every other byte of
/// `text` stays, so a hand-formatted manifest keeps its formatting. `None`
/// when `text` is no JSON object or `section` is no object.
pub(crate) fn insert_entry(text: &str, section: &str, id: &str, path: &str) -> Option<String> {
    let entry = format!("\"{id}\": {{ \"path\": \"{path}\" }}");
    let root = Object::read(text, skip_space(text, 0))?;
    // Only space may follow the root.
    if skip_space(text, root.close + 1) != text.len() {
        return None;
    }
    // One indent step: what the root's first member sits at, when it is on
    // a line of its own; a one-line manifest has none.
    let step = root
        .members
        .first()
        .and_then(|member| indent_of(text, member.key))
        .map(str::to_owned);
    if let Some(member) = root.members.iter().find(|member| member.name == section) {
        let object = Object::read(text, member.value)?;
        let own = indent_of(text, member.key);
        let (inner, closing) = match (&step, own) {
            (Some(step), Some(own)) => (format!("\n{own}{step}"), format!("\n{own}")),
            _ => (" ".to_owned(), " ".to_owned()),
        };
        let lined = text[object.open..object.close].contains('\n');
        return Some(match object.members.last() {
            // After its last entry; its own closing lines stay.
            Some(last) if lined => insert(text, last.end, &format!(",{inner}{entry}")),
            Some(last) => insert(text, last.end, &format!(", {entry}")),
            // An empty object: the entry and the closing indent replace
            // whatever space it held.
            None => format!(
                "{}{{{inner}{entry}{closing}}}{}",
                &text[..object.open],
                &text[object.close + 1..]
            ),
        });
    }
    let member = match &step {
        Some(step) => format!("\"{section}\": {{\n{step}{step}{entry}\n{step}}}"),
        None => format!("\"{section}\": {{ {entry} }}"),
    };
    Some(match (root.members.last(), &step) {
        (Some(last), Some(step)) => insert(text, last.end, &format!(",\n{step}{member}")),
        (Some(last), None) => insert(text, last.end, &format!(", {member}")),
        (None, _) => format!(
            "{}{{ {member} }}{}",
            &text[..root.open],
            &text[root.close + 1..]
        ),
    })
}

/// `text` with `added` put in at byte `at`.
fn insert(text: &str, at: usize, added: &str) -> String {
    format!("{}{added}{}", &text[..at], &text[at..])
}

/// One member of a JSON object in a text: its name and the bytes where its
/// key starts, its value starts and its value ends.
struct Member {
    name: String,
    key: usize,
    value: usize,
    end: usize,
}

/// A JSON object in a text: its braces' bytes and its members in order.
struct Object {
    open: usize,
    close: usize,
    members: Vec<Member>,
}

impl Object {
    /// The object whose `{` is at byte `at` of `text`; `None` when none
    /// starts there or it does not close.
    fn read(text: &str, at: usize) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.get(at) != Some(&b'{') {
            return None;
        }
        let mut members = Vec::new();
        let mut i = skip_space(text, at + 1);
        if bytes.get(i) == Some(&b'}') {
            return Some(Self {
                open: at,
                close: i,
                members,
            });
        }
        loop {
            let key = i;
            let key_end = skip_string(bytes, key)?;
            let name: String = serde_json::from_str(&text[key..key_end]).ok()?;
            i = skip_space(text, key_end);
            if bytes.get(i) != Some(&b':') {
                return None;
            }
            let value = skip_space(text, i + 1);
            let end = skip_value(bytes, value)?;
            members.push(Member {
                name,
                key,
                value,
                end,
            });
            i = skip_space(text, end);
            match bytes.get(i) {
                Some(b',') => i = skip_space(text, i + 1),
                Some(b'}') => {
                    return Some(Self {
                        open: at,
                        close: i,
                        members,
                    });
                }
                _ => return None,
            }
        }
    }
}

/// The first byte at or after `at` that is no JSON whitespace.
fn skip_space(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = at;
    while matches!(bytes.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

/// The byte after the string that starts at `at`.
fn skip_string(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes.get(at) != Some(&b'"') {
        return None;
    }
    let mut i = at + 1;
    loop {
        match bytes.get(i)? {
            b'"' => return Some(i + 1),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
}

/// The byte after the value that starts at `at`: a string, an object or an
/// array with what it nests, or a bare scalar.
fn skip_value(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at)? {
        b'"' => skip_string(bytes, at),
        b'{' | b'[' => {
            let mut depth = 0_usize;
            let mut i = at;
            loop {
                match bytes.get(i)? {
                    b'"' => {
                        i = skip_string(bytes, i)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i + 1);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        _ => {
            let mut i = at;
            while !matches!(
                bytes.get(i),
                None | Some(b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            ) {
                i += 1;
            }
            (i > at).then_some(i)
        }
    }
}

/// The spaces and tabs before byte `at` when nothing else is before it on
/// its line; `None` when it shares its line with something before it.
fn indent_of(text: &str, at: usize) -> Option<&str> {
    let line = text[..at].rfind('\n')? + 1;
    let indent = &text[line..at];
    indent
        .bytes()
        .all(|b| b == b' ' || b == b'\t')
        .then_some(indent)
}
