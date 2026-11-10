//! `tungsten level` on a game made of the umbrella's rooms fixture.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A game folder holding the umbrella's rooms fixture as its `assets/`, a
/// `tungsten.json` naming its manifest and a `Cargo.toml`.
fn game() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_level_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/tungsten/tests/fixtures/rooms");
    copy(&fixture, &dir.join("assets"));
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"cave\"\n").unwrap();
    std::fs::write(
        dir.join("tungsten.json"),
        r#"{ "assets": { "manifest": "assets/manifest.json" } }"#,
    )
    .unwrap();
    dir.canonicalize().unwrap()
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn cave(dir: &Path) -> PathBuf {
    dir.join("assets/rooms/cave.json")
}

/// The cave file in its canonical form, so an undo's bytes are its own.
fn canonical_cave(dir: &Path) -> String {
    let text = std::fs::read_to_string(cave(dir)).unwrap();
    let canonical = Room::from_json(&text).unwrap().to_canonical();
    std::fs::write(cave(dir), &canonical).unwrap();
    canonical
}

/// `apply` with no option, writing for real.
fn apply_batch(game: &Game, room: &str, text: &str) -> anyhow::Result<String> {
    apply(game, room, text, &ApplyOptions::default(), &mut write_file)
}

fn undo(game: &Game, room: &str) -> anyhow::Result<String> {
    undo_last(game, room, false, &mut write_file)
}

fn redo(game: &Game, room: &str) -> anyhow::Result<String> {
    redo_last(game, room, false, &mut write_file)
}

/// The room file, the journal and the redo journal as they read now,
/// `None` for one that is not there.
fn apply_files(game: &Game, dir: &Path) -> [Option<String>; 3] {
    [cave(dir), game.journal("cave"), game.redo_journal("cave")]
        .map(|path| std::fs::read_to_string(path).ok())
}

#[test]
fn apply_writes_the_canonical_room_and_journals_the_inverse() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let message = apply(
        &game,
        "cave",
        r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }, { "link": { "id": "exit", "to": null } }]"#,
        &ApplyOptions {
            inverse: Some(Path::new("undo.json")),
            dry_run: false,
        },
        &mut write_file,
    )
    .unwrap();
    assert_eq!(
        message,
        "applied 2 commands to assets/rooms/cave.json:\n  move `ball_1` by 1, 0\n  unlink `exit`\nundo: tungsten level apply cave --undo"
    );
    let written = std::fs::read_to_string(cave(&dir)).unwrap();
    let room = Room::from_json(&written).unwrap();
    assert_eq!(room.to_canonical(), written, "written canonically");
    assert!(written.contains("\"at\": [31.5, 12]"), "{written}");
    let journal = std::fs::read_to_string(game.journal("cave")).unwrap();
    let lines: Vec<Value> = journal
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["after"], json!(hash(written.as_bytes())));
    assert_eq!(lines[0]["batch"][0]["move"]["id"], "ball_1");
    let inverse: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("undo.json")).unwrap()).unwrap();
    assert_eq!(inverse, lines[0]["inverse"]);
    assert_eq!(
        inverse,
        json!([
            { "link": { "id": "exit", "to": "hall.west_door" } },
            { "move": { "id": "ball_1", "to": [30.5, 12.0] } }
        ])
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_refuses_a_bad_batch_and_leaves_the_file() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let before = std::fs::read(cave(&dir)).unwrap();
    let err = apply_batch(
        &game,
        "cave",
        r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }, { "move": { "id": "bal_1", "by": [1, 0] } }]"#,
    )
    .unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "assets/rooms/cave.json: commands[1].move.id: no entry `bal_1`; did you mean `ball_1`?"
    );
    let err = apply_batch(&game, "cave", r#"{ "move": {} }"#).unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        r#"a batch is a JSON array of commands, as `[{ "move": { "id": "crate_1", "by": [1, 0] } }]`"#
    );
    assert_eq!(
        std::fs::read(cave(&dir)).unwrap(),
        before,
        "the file as it was"
    );
    assert!(!game.journal("cave").exists(), "nothing journaled");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_undo_restores_the_file_byte_for_byte() {
    let dir = game();
    let before = canonical_cave(&dir);
    let game = Game::open(&dir).unwrap();
    apply_batch(
        &game,
        "cave",
        r#"[{ "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19], "on": "floor" } } },
            { "move": { "id": "floor", "by": [0, -1] } },
            { "paint": { "layer": "art", "cells": [[3, 3]], "tile": "plank" } }]"#,
    )
    .unwrap();
    apply_batch(&game, "cave", r#"[{ "delete": { "id": "lamp_1" } }]"#).unwrap();
    assert_ne!(std::fs::read_to_string(cave(&dir)).unwrap(), before);
    assert_eq!(
        undo(&game, "cave").unwrap(),
        "undid 1 command on assets/rooms/cave.json:\n  delete `lamp_1`\nredo: tungsten level apply cave --redo"
    );
    undo(&game, "cave").unwrap();
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), before);
    assert!(
        !game.journal("cave").exists(),
        "an emptied journal is removed"
    );
    let err = undo(&game, "cave").unwrap_err();
    assert_eq!(format!("{err:#}"), "room `cave` has nothing to undo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_undo_refuses_after_an_outside_edit() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    apply_batch(
        &game,
        "cave",
        r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }]"#,
    )
    .unwrap();
    let edited = std::fs::read_to_string(cave(&dir))
        .unwrap()
        .replace("31.5", "32");
    std::fs::write(cave(&dir), &edited).unwrap();
    let journal = std::fs::read_to_string(game.journal("cave")).unwrap();
    let err = undo(&game, "cave").unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "assets/rooms/cave.json changed outside the journal since its last apply; nothing undone"
    );
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), edited);
    assert_eq!(
        std::fs::read_to_string(game.journal("cave")).unwrap(),
        journal
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn query_prints_an_entry_and_what_lies_at_a_cell() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let text = query(&game, "cave", Some("crate_1"), None, false).unwrap();
    assert!(
        text.starts_with("object crate_1 (objects[0]):\n{\n"),
        "{text}"
    );
    assert!(text.contains("\"prefab\": \"crate\""), "{text}");
    let value: Value =
        serde_json::from_str(&query(&game, "cave", Some("exit"), None, true).unwrap()).unwrap();
    assert_eq!(value["kind"], "place");
    assert_eq!(value["entry"]["to"], "hall.west_door");
    assert_eq!(
        query(&game, "cave", None, Some([5, 20]), false).unwrap(),
        "cave (5, 20):\n  art: rock (auto)\n  solid: rock (auto)\n  shapes: floor\n  objects: none\n  places: none\n"
    );
    assert_eq!(
        query(&game, "cave", None, Some([5, 19]), false).unwrap(),
        "cave (5, 19):\n  art: empty\n  solid: empty\n  shapes: none\n  objects: crate_1\n  places: none\n"
    );
    // A slope cell, a whole tile's override and the area over the arena.
    let cell: Value =
        serde_json::from_str(&query(&game, "cave", None, Some([10, 19]), true).unwrap()).unwrap();
    assert_eq!(cell["layers"][1]["tile"], "rock_45_br");
    assert_eq!(cell["layers"][1]["auto"], false);
    assert_eq!(cell["shapes"], json!(["ramp"]));
    let arch = query(&game, "cave", None, Some([16, 19]), false).unwrap();
    assert!(arch.contains("  art: arch\n"), "{arch}");
    assert!(
        arch.contains("  places: main\n"),
        "the route's stop: {arch}"
    );
    let pit = query(&game, "cave", None, Some([36, 21]), false).unwrap();
    assert!(
        pit.contains("  shapes: floor, pit\n"),
        "an eraser is over it too: {pit}"
    );
    assert!(query(&game, "cave", None, Some([40, 0]), false).is_err());
    assert!(query(&game, "cave", Some("nope"), None, false).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn describe_lists_named_things_by_kind() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    assert_eq!(
        describe(&game, Some("hall"), false).unwrap(),
        "hall (assets/rooms/hall.json): 20 \u{d7} 12 cells of 16 \u{d7} 16 px, tileset cave\n  layers (2): art (render), solid (collision)\n  shapes (1): floor\n  places (1): west_door (point)\n  links (1): west_door \u{2192} cave.exit\n"
    );
    let all = describe(&game, None, false).unwrap();
    assert!(all.starts_with("cave (assets/rooms/cave.json)"), "{all}");
    assert!(
        all.contains(
            "  objects (4): crate_1 (crate), lamp_1 (lamp), ball_1 (ball), walker_1 (walker)\n"
        ),
        "{all}"
    );
    assert!(
        all.contains(
            "  places (4): player_start (point), arena (rect), main (route), exit (point)\n"
        ),
        "{all}"
    );
    let value: Value = serde_json::from_str(&describe(&game, None, true).unwrap()).unwrap();
    let ids: Vec<&str> = value["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|room| room["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["cave", "hall"]);
    assert_eq!(
        value["rooms"][0]["links"],
        json!([{ "id": "exit", "to": "hall.west_door" }])
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_room_argument_is_an_id_or_a_path() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let by_id = game.target("cave").unwrap();
    let by_path = game.target("assets/rooms/cave.json").unwrap();
    assert_eq!(by_id.id, "cave");
    assert_eq!(by_path, by_id);
    // An unlisted room file takes its stem.
    std::fs::copy(cave(&dir), dir.join("assets/rooms/den.json")).unwrap();
    assert_eq!(game.target("assets/rooms/den.json").unwrap().id, "den");
    let err = game.target("cove").unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "no room `cove`: name a rooms ID (cave, hall) or a room file"
    );
    assert!(game.target("assets/rooms/nope.json").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn room_findings_keep_the_room_files_alone() {
    let findings = [
        check::Finding {
            file: "assets/rooms/cave.json".into(),
            message: "cave: crate_1: objects[0]: floats".into(),
        },
        check::Finding {
            file: "assets/prefabs/lamp.json".into(),
            message: "bad".into(),
        },
    ];
    let files: BTreeSet<String> = ["assets/rooms/cave.json".to_owned()].into();
    assert_eq!(
        room_findings(&findings, &files),
        ["assets/rooms/cave.json: cave: crate_1: objects[0]: floats"]
    );
}

#[test]
fn level_check_prints_every_generators_findings() {
    let report = check::parse_report(
        "#\tgenerator `chain` (version 1): seeds 0+16, 48 rooms\n\
         assets/rooms/cave.json\tcave: crate_1: objects[0]: floats\n\
         generator chain\tseed 4: chain_r2: lamp_9: objects[0]: floats\n\
         assets/rooms/hall.json\thall: west_door: places[0].to: links to `cave.exit`, which no room holds\n\
         assets/prefabs/lamp.json\tbad\n",
    );
    let files: BTreeSet<String> = ["assets/rooms/cave.json".to_owned()].into();
    let (lines, found) = check_lines(&report, &files, None);
    assert_eq!(
        lines,
        [
            "generator `chain` (version 1): seeds 0+16, 48 rooms",
            "assets/rooms/cave.json: cave: crate_1: objects[0]: floats",
            "generator chain: seed 4: chain_r2: lamp_9: objects[0]: floats",
        ],
        "the named room's and every generator's, not hall's or the prefab's"
    );
    assert_eq!(found, 2);
    let seeds = Some(check::Seeds {
        first: 0,
        count: 500,
    });
    let (lines, found) = check_lines(&check::parse_report(""), &files, seeds);
    assert_eq!(
        (lines, found),
        (vec!["no room generator registered".to_owned()], 0)
    );
}

/// A generate run's report of `rooms`, each `(id, replaces)` with a small
/// room's text, 4 objects generated and one kept, after reading
/// `templates`.
fn generation(rooms: &[&str], templates: &[&str]) -> GenerateReport {
    let rooms: Vec<Value> = rooms
        .iter()
        .map(|id| {
            json!({ "id": id, "text": small(), "replaces": false, "generated": 4,
                    "kept": ["box_1"], "dropped": [], "reshaped": false })
        })
        .collect();
    let templates: serde_json::Map<String, Value> = templates
        .iter()
        .map(|id| ((*id).to_owned(), json!("0123456789abcdef")))
        .collect();
    let report = json!({ "version": 1, "errors": [], "rooms": rooms, "generation":
        { "generator": "chain", "version": 1, "seed": 3, "prefix": "run", "templates": templates } });
    parse_generation(&report.to_string()).unwrap()
}

/// A 4 × 4 room's canonical text.
fn small() -> String {
    Room::new([16, 16], [4, 4]).to_canonical()
}

/// The manifest of `dir` with `rooms` listed too, each `(id, path)`.
fn list(dir: &Path, rooms: &[(&str, &str)]) {
    let path = dir.join("assets/manifest.json");
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (id, file) in rooms {
        manifest["rooms"][*id] = json!({ "path": file });
    }
    std::fs::write(&path, manifest.to_string()).unwrap();
}

fn files(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir.join("assets/rooms"))
        .unwrap()
        .flatten()
    {
        found.push((entry.path(), std::fs::read_to_string(entry.path()).unwrap()));
    }
    found.sort();
    found
}

#[test]
fn generate_rewrites_a_listed_room_only_with_regenerate() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let report = generation(&["cave"], &[]);
    let before = files(&dir);
    let err = finish_generation(&game, &report, None, false, &mut write_room_file).unwrap_err();
    assert_eq!(
        err.to_string(),
        "room `cave` is listed (assets/rooms/cave.json); `--regenerate` rewrites it"
    );
    assert_eq!(files(&dir), before, "nothing written");
    let (lines, written) =
        finish_generation(&game, &report, None, true, &mut write_room_file).unwrap();
    assert!(written);
    assert_eq!(
        lines,
        [
            "regenerated assets/rooms/cave.json: 4 generated, 1 kept, 0 dropped",
            "tungsten level generate: generator `chain` at seed 3, 1 room written"
        ]
    );
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), small());
}

#[test]
fn generate_writes_new_rooms_and_prints_their_manifest_lines() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let report = generation(&["run_r0", "run_r1"], &["hall"]);
    let (lines, _) = finish_generation(&game, &report, None, false, &mut write_room_file).unwrap();
    assert_eq!(
        lines,
        [
            "wrote assets/rooms/run_r0.json: 4 generated",
            "wrote assets/rooms/run_r1.json: 4 generated",
            "add to the manifest's rooms:",
            r#"  "run_r0": { "path": "rooms/run_r0.json" }"#,
            r#"  "run_r1": { "path": "rooms/run_r1.json" }"#,
            "tungsten level generate: generator `chain` at seed 3, 2 rooms written"
        ]
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("assets/rooms/run_r1.json")).unwrap(),
        small()
    );
    // Under `--into`, a folder made for them, the path from the manifest's.
    let (lines, _) = finish_generation(
        &game,
        &generation(&["run_r2"], &[]),
        Some(Path::new("levels")),
        false,
        &mut write_room_file,
    )
    .unwrap();
    assert_eq!(lines[0], "wrote levels/run_r2.json: 4 generated");
    assert_eq!(
        lines[2],
        r#"  "run_r2": { "path": "../levels/run_r2.json" }"#
    );
    assert!(dir.join("levels/run_r2.json").is_file());
    // A folder outside the game's.
    let err = finish_generation(
        &game,
        &generation(&["run_r3"], &[]),
        Some(Path::new("../elsewhere")),
        false,
        &mut write_room_file,
    )
    .unwrap_err();
    assert!(
        err.to_string().ends_with("is outside the game's folder"),
        "{err}"
    );
}

#[test]
fn generate_refuses_every_write_for_an_existing_unlisted_file() {
    let dir = game();
    std::fs::write(dir.join("assets/rooms/run_r1.json"), "{}").unwrap();
    let game = Game::open(&dir).unwrap();
    let before = files(&dir);
    let err = finish_generation(
        &game,
        &generation(&["run_r0", "run_r1"], &[]),
        None,
        false,
        &mut write_room_file,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "room `run_r1`'s file 'assets/rooms/run_r1.json' exists; a new room is written over no file"
    );
    assert_eq!(files(&dir), before, "not even run_r0");
}

#[test]
fn generate_refuses_a_file_another_listed_room_or_a_template_names() {
    // `twin` lists `cave`'s file: regenerating `cave` would rewrite it.
    let dir = game();
    list(&dir, &[("twin", "rooms/cave.json")]);
    let game = Game::open(&dir).unwrap();
    let before = files(&dir);
    let err = finish_generation(
        &game,
        &generation(&["cave"], &[]),
        None,
        true,
        &mut write_room_file,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "room `cave`'s file 'assets/rooms/cave.json' is also room `twin`'s, which the generation does not write; nothing written"
    );
    // `run_r0` lists the template `hall`'s file.
    list(&dir, &[("run_r0", "rooms/hall.json")]);
    let game = Game::open(&dir).unwrap();
    let err = finish_generation(
        &game,
        &generation(&["run_r0"], &["hall"]),
        None,
        true,
        &mut write_room_file,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "room `run_r0`'s file 'assets/rooms/hall.json' is also template `hall`'s, which the generation does not write; nothing written"
    );
    assert_eq!(files(&dir), before, "no file changed");
}

#[test]
fn a_failed_write_puts_back_the_files_written_before_it() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let before = files(&dir);
    let mut calls = 0;
    let mut failing = |path: &Path, text: &str| -> anyhow::Result<()> {
        calls += 1;
        if calls == 2 {
            anyhow::bail!("the disk is full");
        }
        write_room_file(path, text)
    };
    let err = finish_generation(
        &game,
        &generation(&["cave", "run_r0", "run_r1"], &[]),
        None,
        true,
        &mut failing,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot write 'assets/rooms/run_r0.json': the disk is full; 1 file written before it put back"
    );
    assert_eq!(files(&dir), before, "cave as it was, no new file");
}

#[test]
fn a_report_error_writes_nothing_and_fails() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let report = parse_generation(
        &json!({ "version": 1, "errors": [{ "file": "assets/rooms/hall.json", "message": "room 'hall': not JSON" }] })
            .to_string(),
    )
    .unwrap();
    let before = files(&dir);
    let (lines, written) =
        finish_generation(&game, &report, None, true, &mut write_room_file).unwrap();
    assert!(!written, "exit 1");
    assert_eq!(
        lines,
        [
            "assets/rooms/hall.json: room 'hall': not JSON",
            "tungsten level generate: 1 error; nothing written"
        ]
    );
    assert_eq!(files(&dir), before);
    assert!(
        parse_generation(r#"{ "version": 2 }"#)
            .unwrap_err()
            .to_string()
            .starts_with("the generation's report is version 2")
    );
}

const NUDGE: &str = r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }]"#;
const PLACE: &str = r#"[{ "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19] } } },
    { "paint": { "layer": "art", "rect": [3, 3, 2, 1], "tile": "plank" } }]"#;

#[test]
fn redo_applies_what_undo_took_back() {
    let dir = game();
    let before = canonical_cave(&dir);
    let game = Game::open(&dir).unwrap();
    apply_batch(&game, "cave", NUDGE).unwrap();
    apply_batch(&game, "cave", PLACE).unwrap();
    let applied = apply_files(&game, &dir);
    undo(&game, "cave").unwrap();
    undo(&game, "cave").unwrap();
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), before);
    assert_eq!(journal_lines(&game.redo_journal("cave")).len(), 2);
    // The newest undone batch first, as a stack.
    assert_eq!(
        redo(&game, "cave").unwrap(),
        "redid 1 command on assets/rooms/cave.json:\n  move `ball_1` by 1, 0\nundo: tungsten level apply cave --undo"
    );
    assert_eq!(
        redo(&game, "cave").unwrap(),
        "redid 2 commands on assets/rooms/cave.json:\n  place object `crate_2` (`crate`) at 6, 19\n  paint 2 cells of `art` with `plank`\nundo: tungsten level apply cave --undo"
    );
    assert_eq!(
        apply_files(&game, &dir),
        applied,
        "the room and its journal as applied"
    );
    let err = redo(&game, "cave").unwrap_err();
    assert_eq!(format!("{err:#}"), "room `cave` has nothing to redo");
    // And the redone batches undo again.
    undo(&game, "cave").unwrap();
    undo(&game, "cave").unwrap();
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn redo_follows_an_undo_of_a_file_that_was_not_canonical() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    // The fixture's file is hand-formatted: its bytes are not its
    // canonical form, which an undo writes.
    let original = std::fs::read_to_string(cave(&dir)).unwrap();
    let canonical = Room::from_json(&original).unwrap().to_canonical();
    assert_ne!(original, canonical, "the fixture is not canonical");
    apply_batch(&game, "cave", NUDGE).unwrap();
    let applied = std::fs::read_to_string(cave(&dir)).unwrap();
    undo(&game, "cave").unwrap();
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), canonical);
    redo(&game, "cave").unwrap();
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), applied);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_apply_clears_the_redo_journal() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    apply_batch(&game, "cave", NUDGE).unwrap();
    undo(&game, "cave").unwrap();
    assert!(game.redo_journal("cave").exists());
    apply_batch(&game, "cave", PLACE).unwrap();
    assert!(
        !game.redo_journal("cave").exists(),
        "a new batch ends the redos"
    );
    let err = redo(&game, "cave").unwrap_err();
    assert_eq!(format!("{err:#}"), "room `cave` has nothing to redo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn redo_refuses_a_file_edited_since() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    apply_batch(&game, "cave", NUDGE).unwrap();
    undo(&game, "cave").unwrap();
    let edited = std::fs::read_to_string(cave(&dir))
        .unwrap()
        .replace("30.5", "29");
    std::fs::write(cave(&dir), &edited).unwrap();
    let before = apply_files(&game, &dir);
    let err = redo(&game, "cave").unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "assets/rooms/cave.json changed since its undo; nothing redone"
    );
    assert_eq!(apply_files(&game, &dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dry_run_writes_nothing() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let before = apply_files(&game, &dir);
    let dry = ApplyOptions {
        inverse: Some(Path::new("undo.json")),
        dry_run: true,
    };
    assert_eq!(
        apply(&game, "cave", NUDGE, &dry, &mut write_file).unwrap(),
        "would apply 1 command to assets/rooms/cave.json:\n  move `ball_1` by 1, 0\ndry run: nothing written"
    );
    assert_eq!(apply_files(&game, &dir), before);
    assert!(!dir.join("undo.json").exists());
    // A batch that does not apply fails a dry run as it fails an apply.
    let err = apply(
        &game,
        "cave",
        r#"[{ "move": { "id": "bal_1", "by": [1, 0] } }]"#,
        &dry,
        &mut write_file,
    )
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("did you mean `ball_1`?"),
        "{err:#}"
    );
    // An undo and a redo dry-run the same way.
    apply_batch(&game, "cave", NUDGE).unwrap();
    let applied = apply_files(&game, &dir);
    assert_eq!(
        undo_last(&game, "cave", true, &mut write_file).unwrap(),
        "would undo 1 command on assets/rooms/cave.json:\n  move `ball_1` by 1, 0\ndry run: nothing written"
    );
    assert_eq!(apply_files(&game, &dir), applied);
    undo(&game, "cave").unwrap();
    let undone = apply_files(&game, &dir);
    assert_eq!(
        redo_last(&game, "cave", true, &mut write_file).unwrap(),
        "would redo 1 command on assets/rooms/cave.json:\n  move `ball_1` by 1, 0\ndry run: nothing written"
    );
    assert_eq!(apply_files(&game, &dir), undone);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_apply_reports_each_command() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    // A placed object and an override are the game's to check, so the
    // report names the check.
    assert_eq!(
        apply_batch(&game, "cave", PLACE).unwrap(),
        "applied 2 commands to assets/rooms/cave.json:\n  place object `crate_2` (`crate`) at 6, 19\n  paint 2 cells of `art` with `plank`\nundo: tungsten level apply cave --undo\ncheck: tungsten level check cave (the level rules and the game's components)"
    );
    let set = apply_batch(
        &game,
        "cave",
        r#"[{ "set": { "id": "crate_2", "path": "set.rigid_body.mass", "value": 2 } }]"#,
    )
    .unwrap();
    assert!(
        set.ends_with(
            "check: tungsten level check cave (the level rules and the game's components)"
        ),
        "{set}"
    );
    // Terrain and moves alone are checked whole by the apply.
    let moved = apply_batch(&game, "cave", NUDGE).unwrap();
    assert!(!moved.contains("check:"), "{moved}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_inverse_file_that_is_the_room_or_a_journal_is_refused() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    // A journal that exists, so each path resolves to its file.
    apply_batch(&game, "cave", NUDGE).unwrap();
    undo(&game, "cave").unwrap();
    let before = apply_files(&game, &dir);
    for (inverse, also) in [
        ("assets/rooms/cave.json", "the room's file"),
        ("assets/rooms/../rooms/cave.json", "the room's file"),
        ("target/tungsten/journal/cave.jsonl", "the journal"),
        (
            "target/tungsten/journal/cave.redo.jsonl",
            "the redo journal",
        ),
    ] {
        let options = ApplyOptions {
            inverse: Some(Path::new(inverse)),
            dry_run: false,
        };
        let err = apply(&game, "cave", NUDGE, &options, &mut write_file).unwrap_err();
        let message = format!("{err:#}");
        assert!(
            message.starts_with("`--inverse`'s file '")
                && message.ends_with(&format!("' is also {also}; nothing written")),
            "{inverse}: {message}"
        );
        assert_eq!(apply_files(&game, &dir), before, "{inverse}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_write_that_fails_puts_back_the_files_before_it() {
    let dir = game();
    canonical_cave(&dir);
    let game = Game::open(&dir).unwrap();
    // The state before each command: one batch journaled, one undone, so
    // all three files exist and each command rewrites all three.
    apply_batch(&game, "cave", NUDGE).unwrap();
    apply_batch(&game, "cave", PLACE).unwrap();
    undo(&game, "cave").unwrap();
    let before = apply_files(&game, &dir);
    type Command = fn(&Game, &mut Writer<'_>) -> anyhow::Result<String>;
    let commands: [(&str, Command, usize); 3] = [
        (
            "apply",
            |game, write| {
                let options = ApplyOptions {
                    inverse: Some(Path::new("undo.json")),
                    dry_run: false,
                };
                apply(game, "cave", NUDGE, &options, write)
            },
            4,
        ),
        (
            "undo",
            |game, write| undo_last(game, "cave", false, write),
            3,
        ),
        (
            "redo",
            |game, write| redo_last(game, "cave", false, write),
            3,
        ),
    ];
    for (name, command, writes) in commands {
        for failing in 0..writes {
            let mut calls = 0;
            let mut write = |path: &Path, text: Option<&str>| {
                calls += 1;
                if calls - 1 == failing {
                    anyhow::bail!("the disk is full");
                }
                write_file(path, text)
            };
            let err = command(&game, &mut write).unwrap_err();
            let message = format!("{err:#}");
            assert!(
                message.starts_with("cannot write ")
                    && message.ends_with(": the disk is full; nothing changed"),
                "{name}, write {failing}: {message}"
            );
            assert_eq!(apply_files(&game, &dir), before, "{name}, write {failing}");
            assert!(!dir.join("undo.json").exists(), "{name}, write {failing}");
        }
    }
    // With no write failing, each lands.
    for (name, command, _) in commands {
        let mut write = |path: &Path, text: Option<&str>| write_file(path, text);
        command(&game, &mut write).unwrap_or_else(|err| panic!("{name}: {err:#}"));
        if name != "apply" {
            continue;
        }
        // The apply emptied the redo journal; give the undo and the redo
        // after it something to do.
        assert!(!game.redo_journal("cave").exists());
    }
    let _ = std::fs::remove_dir_all(&dir);
}
