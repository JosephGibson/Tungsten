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

#[test]
fn apply_writes_the_canonical_room_and_journals_the_inverse() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    let message = apply(
        &game,
        "cave",
        r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }, { "link": { "id": "exit", "to": null } }]"#,
        Some(Path::new("undo.json")),
    )
    .unwrap();
    assert_eq!(
        message,
        "applied 2 commands to assets/rooms/cave.json; `tungsten level apply cave --undo` undoes it"
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
    let err = apply(
        &game,
        "cave",
        r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }, { "move": { "id": "bal_1", "by": [1, 0] } }]"#,
        None,
    )
    .unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "assets/rooms/cave.json: commands[1].move.id: no entry `bal_1`; did you mean `ball_1`?"
    );
    let err = apply(&game, "cave", r#"{ "move": {} }"#, None).unwrap_err();
    assert_eq!(format!("{err:#}"), "a batch is a JSON array of commands");
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
    apply(
        &game,
        "cave",
        r#"[{ "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19], "on": "floor" } } },
            { "move": { "id": "floor", "by": [0, -1] } },
            { "paint": { "layer": "art", "cells": [[3, 3]], "tile": "plank" } }]"#,
        None,
    )
    .unwrap();
    apply(&game, "cave", r#"[{ "delete": { "id": "lamp_1" } }]"#, None).unwrap();
    assert_ne!(std::fs::read_to_string(cave(&dir)).unwrap(), before);
    assert_eq!(
        undo_last(&game, "cave").unwrap(),
        "undid the last batch on assets/rooms/cave.json"
    );
    undo_last(&game, "cave").unwrap();
    assert_eq!(std::fs::read_to_string(cave(&dir)).unwrap(), before);
    assert_eq!(std::fs::read_to_string(game.journal("cave")).unwrap(), "");
    let err = undo_last(&game, "cave").unwrap_err();
    assert_eq!(format!("{err:#}"), "room `cave` has nothing to undo");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_undo_refuses_after_an_outside_edit() {
    let dir = game();
    let game = Game::open(&dir).unwrap();
    apply(
        &game,
        "cave",
        r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }]"#,
        None,
    )
    .unwrap();
    let edited = std::fs::read_to_string(cave(&dir))
        .unwrap()
        .replace("31.5", "32");
    std::fs::write(cave(&dir), &edited).unwrap();
    let journal = std::fs::read_to_string(game.journal("cave")).unwrap();
    let err = undo_last(&game, "cave").unwrap_err();
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
