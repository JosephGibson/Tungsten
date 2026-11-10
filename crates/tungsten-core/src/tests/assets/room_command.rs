//! Room commands, their inverses and the journal, on the umbrella's
//! fixture room `cave`.

use serde_json::json;

use super::*;

const CAVE: &str = include_str!("../../../../tungsten/tests/fixtures/rooms/rooms/cave.json");
const CAVE_TILESET: &str =
    include_str!("../../../../tungsten/tests/fixtures/rooms/tilesets/cave.json");
const PREFABS: [&str; 4] = ["ball", "crate", "lamp", "walker"];

fn cave() -> Room {
    Room::from_json(CAVE).unwrap()
}

/// The fixture's tileset and physics layer names.
fn world() -> (Tileset, PhysicsLayers) {
    (
        Tileset::from_json(CAVE_TILESET).unwrap(),
        PhysicsLayers::new([
            ("terrain".to_owned(), 1),
            ("player".to_owned(), 2),
            ("enemy".to_owned(), 3),
        ]),
    )
}

fn batch(value: &serde_json::Value) -> Vec<RoomCommand> {
    RoomCommand::batch_from_value(value).unwrap()
}

fn object<'a>(room: &'a Room, id: &str) -> &'a RoomObject {
    room.objects.iter().find(|object| object.id == id).unwrap()
}

fn place<'a>(room: &'a Room, id: &str) -> &'a RoomPlace {
    room.places.iter().find(|place| place.id == id).unwrap()
}

#[test]
fn each_verb_then_its_inverse_gives_the_room_back_byte_for_byte() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let commands = [
        json!({ "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19], "on": "floor" }, "index": 1 } }),
        json!({ "move": { "id": "floor", "by": [0, -1] } }),
        json!({ "set": { "id": "ball_1", "path": "set.rigid_body.shape.radius", "value": 5 } }),
        json!({ "delete": { "id": "floor" } }),
        json!({ "duplicate": { "id": "crate_1", "as": "crate_2", "by": [1.5, 0] } }),
        json!({ "paint": { "layer": "art", "rect": [15, 18, 3, 2], "tile": "plank" } }),
        json!({ "erase": { "layer": "solid", "cells": [[1, 20], [2, 20]] } }),
        json!({ "link": { "id": "exit", "to": null } }),
        json!({ "rename": { "id": "player_start", "to": "spawn" } }),
    ];
    let mut verbs = Vec::new();
    for command in commands {
        let mut room = cave();
        let before = room.to_canonical();
        let applied = batch(&json!([command]));
        verbs.push(applied[0].verb());
        let inverse = room.apply(&applied, &context).unwrap();
        assert_ne!(room.to_canonical(), before, "{command} changes the room");
        // The inverse reads back from JSON, as the command line's journal
        // keeps it.
        let text = serde_json::to_string(&inverse).unwrap();
        let inverse = RoomCommand::parse_batch(&text).unwrap();
        room.apply(&inverse, &context).unwrap();
        assert_eq!(room.to_canonical(), before, "{command}'s inverse: {text}");
    }
    assert_eq!(
        verbs,
        [
            "place",
            "move",
            "set",
            "delete",
            "duplicate",
            "paint",
            "erase",
            "link",
            "rename"
        ]
    );
}

#[test]
fn a_batch_with_one_bad_command_changes_nothing_and_names_it() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let before = room.to_canonical();
    let err = room
        .apply(
            &batch(&json!([
                { "move": { "id": "ball_1", "by": [1, 0] } },
                { "move": { "id": "crate_9", "by": [1, 0] } }
            ])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[1].move.id: no entry `crate_9`; did you mean `crate_1`?"
    );
    assert_eq!(room.to_canonical(), before, "the first move is not kept");
    // A batch that does not read names its command and field.
    let err =
        RoomCommand::parse_batch(r#"[{ "move": { "id": "a", "by": [1, 0] } }, { "mvoe": {} }]"#)
            .unwrap_err();
    assert!(err.field.starts_with("commands[1]"), "{err}");
    let err =
        RoomCommand::parse_batch(r#"[{ "move": { "id": "a", "bye": [1, 0] } }]"#).unwrap_err();
    assert_eq!(err.field, "commands[0].move.bye");
}

#[test]
fn moving_a_shape_carries_the_objects_on_it() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    room.apply(
        &batch(&json!([{ "move": { "id": "floor", "to": [0, 19] } }])),
        &context,
    )
    .unwrap();
    assert_eq!(room.shapes[0].rect, Some([0, 19, 40, 4]));
    assert_eq!(object(&room, "crate_1").at, [5.0, 18.0], "on the floor");
    assert_eq!(object(&room, "lamp_1").at, [8.0, 19.0], "on nothing");
    // A fraction of a cell is no shape's move; a shape past the room's
    // edge fails.
    let err = room
        .apply(
            &batch(&json!([{ "move": { "id": "floor", "by": [0.5, 0] } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].move.by: a shape moves by whole cells"
    );
    let err = room
        .apply(
            &batch(&json!([{ "move": { "id": "floor", "by": [0, 3] } }])),
            &context,
        )
        .unwrap_err();
    assert!(
        err.to_string()
            .starts_with("commands[0].move: shapes[0].rect"),
        "{err}"
    );
}

#[test]
fn rename_carries_the_rooms_own_references() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    room.apply(
        &batch(&json!([
            { "link": { "id": "arena", "to": "cave.exit" } },
            { "rename": { "id": "floor", "to": "ground" } },
            { "rename": { "id": "player_start", "to": "spawn" } },
            { "rename": { "id": "exit", "to": "door" } }
        ])),
        &context,
    )
    .unwrap();
    assert_eq!(object(&room, "crate_1").on.as_deref(), Some("ground"));
    let route = place(&room, "main").route.clone().unwrap();
    assert_eq!(route[0], RouteStop::Place("spawn".into()));
    assert_eq!(route[2], RouteStop::Place("door".into()));
    assert_eq!(
        place(&room, "arena").to.as_deref(),
        Some("cave.door"),
        "a link to its own place"
    );
    assert_eq!(
        place(&room, "door").to.as_deref(),
        Some("hall.west_door"),
        "another room's place stays"
    );
    let err = room
        .apply(
            &batch(&json!([{ "rename": { "id": "door", "to": "ground" } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].rename.to: `ground` is an entry's ID already"
    );
}

#[test]
fn deleting_a_shape_clears_on_and_its_inverse_restores_it() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let before = room.clone();
    let inverse = room
        .apply(&batch(&json!([{ "delete": { "id": "floor" } }])), &context)
        .unwrap();
    assert!(room.shapes.iter().all(|shape| shape.id != "floor"));
    assert_eq!(object(&room, "crate_1").on, None, "it lost its shape");
    assert_eq!(
        inverse.len(),
        2,
        "the shape back at its index, then `on`: {inverse:?}"
    );
    room.apply(&inverse, &context).unwrap();
    assert_eq!(room, before);
    assert_eq!(room.shapes[0].id, "floor", "at its index");
}

#[test]
fn painting_null_removes_overrides() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let before = room.to_canonical();
    let inverse = room
        .apply(
            &batch(
                &json!([{ "paint": { "layer": "art", "cells": [[16, 19], [3, 3]], "tile": null } }]),
            ),
            &context,
        )
        .unwrap();
    assert!(
        room.layers[0].cells.is_empty(),
        "the arch's override is gone, the shapes show"
    );
    assert_eq!(
        inverse,
        batch(&json!([{ "paint": { "layer": "art", "cells": [[16, 19]], "tile": "arch" } }]))
    );
    room.apply(&inverse, &context).unwrap();
    assert_eq!(room.to_canonical(), before);
    // Erase writes empty overrides, which null removes in turn.
    room.apply(
        &batch(&json!([{ "erase": { "layer": "art", "rect": [0, 0, 2, 1] } }])),
        &context,
    )
    .unwrap();
    assert_eq!(room.layers[0].cells.get("1,0"), Some(&None));
    let err = room
        .apply(
            &batch(&json!([{ "paint": { "layer": "art", "cells": [[0, 0]], "tile": "gravel" } }])),
            &context,
        )
        .unwrap_err();
    assert!(err.to_string().contains("no tile `gravel`"), "{err}");
}

#[test]
fn place_refuses_an_unknown_prefab_with_the_nearest_id() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let err = room
        .apply(
            &batch(&json!([{ "place": { "objects": { "id": "crate_2", "prefab": "crat", "at": [6, 19] } } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].place.objects.prefab: unknown prefab `crat`; did you mean `crate`?"
    );
    let err = room
        .apply(
            &batch(&json!([{ "set": { "id": "crate_1", "path": "prefab", "value": "lamb" } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].set.value: unknown prefab `lamb`; did you mean `lamp`?"
    );
    let err = room
        .apply(
            &batch(&json!([{ "place": { "objects": { "id": "floor", "prefab": "crate", "at": [6, 19] } } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].place: objects[4].id: `floor` is also shapes[0]'s ID"
    );
}

#[test]
fn a_set_past_the_room_fails_and_a_resize_that_cuts_a_shape_fails() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let before = room.clone();
    let err = room
        .apply(
            &batch(&json!([{ "set": { "id": "ball_1", "path": "at", "value": [99, 0] } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].set: objects[2].at: [99, 0] is past the room's 40 \u{d7} 24 cells"
    );
    let err = room
        .apply(
            &batch(&json!([{ "set": { "path": "size", "value": [40, 22] } }])),
            &context,
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].set: shapes[0].rect: cell (0, 22) is past the room's 40 \u{d7} 22 cells"
    );
    assert_eq!(room, before);
    // A resize that keeps everything inside applies, and an optional field
    // goes with `null`.
    room.apply(
        &batch(&json!([
            { "set": { "path": "size", "value": [40, 30] } },
            { "set": { "path": "ambient", "value": null } },
            { "set": { "path": "budget.lights", "value": 4 } }
        ])),
        &context,
    )
    .unwrap();
    assert_eq!((room.size, room.ambient), ([40, 30], None));
    assert_eq!(room.budget.unwrap().lights, Some(4));
    let err = room
        .apply(
            &batch(&json!([{ "set": { "path": "cell", "value": [8, 8] } }])),
            &context,
        )
        .unwrap_err();
    assert!(
        err.to_string()
            .starts_with("commands[0].set.path: the room's own fields"),
        "{err}"
    );
}

#[test]
fn the_journal_undoes_and_redoes_in_order_and_keeps_64() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let mut journal = RoomJournal::new();
    let x = |i: usize| i as f32 * 0.25;
    for i in 0..70 {
        let applied = batch(&json!([{ "move": { "id": "ball_1", "to": [x(i), 12] } }]));
        let inverse = room.apply(&applied, &context).unwrap();
        journal.record("cave", applied, inverse);
    }
    assert_eq!(journal.undo_len("cave"), RoomJournal::LIMIT);
    let ball = |room: &Room| object(room, "ball_1").at[0];
    assert_eq!(ball(&room), x(69));
    for _ in 0..3 {
        assert!(journal.undo("cave", &mut room, &context).unwrap());
    }
    assert_eq!(ball(&room), x(66));
    assert_eq!(journal.redo_len("cave"), 3);
    for _ in 0..2 {
        assert!(journal.redo("cave", &mut room, &context).unwrap());
    }
    assert_eq!(ball(&room), x(68));
    // A new batch empties the redo list.
    let applied = batch(&json!([{ "move": { "id": "ball_1", "by": [0, 1] } }]));
    let inverse = room.apply(&applied, &context).unwrap();
    journal.record("cave", applied, inverse);
    assert_eq!(journal.redo_len("cave"), 0);
    assert!(!journal.redo("cave", &mut room, &context).unwrap());
    // Undone to the oldest kept, batch 6, the six before it dropped: the
    // state batch 5 left.
    while journal.undo("cave", &mut room, &context).unwrap() {}
    assert_eq!(ball(&room), x(5));
    assert_eq!(journal.undo_len("cave"), 0);
    assert_eq!(journal.redo_len("cave"), RoomJournal::LIMIT);
}

#[test]
fn an_undo_that_no_longer_validates_fails_and_changes_nothing() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = cave();
    let mut journal = RoomJournal::new();
    let applied = batch(
        &json!([{ "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19] } } }]),
    );
    let inverse = room.apply(&applied, &context).unwrap();
    journal.record("cave", applied, inverse);
    // Edited outside the journal: the placed crate is gone.
    room.objects.retain(|object| object.id != "crate_2");
    let before = room.clone();
    let err = journal.undo("cave", &mut room, &context).unwrap_err();
    assert_eq!(
        err.to_string(),
        "commands[0].delete.id: no entry `crate_2`; did you mean `crate_1`?"
    );
    assert_eq!(room, before);
    assert_eq!(journal.undo_len("cave"), 1, "the entry stays");
    assert_eq!(journal.redo_len("cave"), 0);
}

/// `cave` with every object made by the generator `chain`.
fn marked() -> Room {
    let mut room = cave();
    for object in &mut room.objects {
        object.generator = Some("chain".to_owned());
    }
    room
}

fn mark<'a>(room: &'a Room, id: &str) -> Option<&'a str> {
    object(room, id).generator.as_deref()
}

#[test]
fn a_hand_move_set_or_rename_detaches_and_its_inverse_marks_again() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    for (command, id) in [
        (
            json!({ "move": { "id": "crate_1", "by": [1, 0] } }),
            "crate_1",
        ),
        (
            json!({ "move": { "id": "ball_1", "to": [3, 4] } }),
            "ball_1",
        ),
        (
            json!({ "set": { "id": "ball_1", "path": "set.rigid_body.shape.radius", "value": 5 } }),
            "ball_1",
        ),
        (
            json!({ "set": { "id": "crate_1", "path": "on", "value": null } }),
            "crate_1",
        ),
        (
            json!({ "rename": { "id": "lamp_1", "to": "lamp_9" } }),
            "lamp_9",
        ),
    ] {
        let mut room = marked();
        let before = room.to_canonical();
        let inverse = room.apply(&batch(&json!([command])), &context).unwrap();
        assert_eq!(mark(&room, id), None, "{command} detaches `{id}`");
        assert!(
            room.objects
                .iter()
                .filter(|object| object.id != id)
                .all(|object| object.generator.is_some()),
            "{command} detaches nothing else"
        );
        let text = serde_json::to_string(&inverse).unwrap();
        room.apply(&RoomCommand::parse_batch(&text).unwrap(), &context)
            .unwrap();
        assert_eq!(room.to_canonical(), before, "{command}'s inverse: {text}");
    }
}

#[test]
fn a_shapes_move_keeps_the_marks_of_the_objects_on_it() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = marked();
    room.apply(
        &batch(&json!([
            { "move": { "id": "floor", "by": [0, -1] } },
            { "rename": { "id": "floor", "to": "ground" } }
        ])),
        &context,
    )
    .unwrap();
    assert_eq!(object(&room, "crate_1").at, [5.0, 18.0], "carried");
    assert_eq!(object(&room, "crate_1").on.as_deref(), Some("ground"));
    assert_eq!(
        mark(&room, "crate_1"),
        Some("chain"),
        "the carry is no hand edit"
    );
}

#[test]
fn a_shape_deleted_or_moved_under_marked_objects_undoes_and_redoes_whole() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    for command in [
        json!({ "delete": { "id": "floor" } }),
        json!({ "move": { "id": "floor", "by": [0, -1] } }),
    ] {
        let mut room = marked();
        let mut journal = RoomJournal::new();
        let before = room.to_canonical();
        let applied = batch(&json!([command]));
        let inverse = room.apply(&applied, &context).unwrap();
        journal.record("cave", applied, inverse);
        let after = room.to_canonical();
        assert_eq!(mark(&room, "crate_1"), Some("chain"), "{command}");
        for _ in 0..2 {
            assert!(journal.undo("cave", &mut room, &context).unwrap());
            assert_eq!(room.to_canonical(), before, "{command} undone");
            assert!(journal.redo("cave", &mut room, &context).unwrap());
            assert_eq!(room.to_canonical(), after, "{command} redone");
        }
    }
}

#[test]
fn each_verb_then_its_inverse_over_marked_objects_gives_the_text_back() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let commands = [
        json!({ "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19], "on": "floor", "generator": "chain" }, "index": 1 } }),
        json!({ "move": { "id": "floor", "by": [0, -1] } }),
        json!({ "move": { "id": "crate_1", "to": [7, 19] } }),
        json!({ "set": { "id": "ball_1", "path": "set.rigid_body.shape.radius", "value": 5 } }),
        json!({ "set": { "id": "ball_1", "path": "generator", "value": "other" } }),
        json!({ "delete": { "id": "floor" } }),
        json!({ "delete": { "id": "crate_1" } }),
        json!({ "duplicate": { "id": "crate_1", "as": "crate_2", "by": [1.5, 0] } }),
        json!({ "paint": { "layer": "art", "rect": [15, 18, 3, 2], "tile": "plank" } }),
        json!({ "erase": { "layer": "solid", "cells": [[1, 20], [2, 20]] } }),
        json!({ "link": { "id": "exit", "to": null } }),
        json!({ "rename": { "id": "crate_1", "to": "box" } }),
        json!({ "rename": { "id": "floor", "to": "ground" } }),
    ];
    for command in commands {
        let mut room = marked();
        let before = room.to_canonical();
        let inverse = room.apply(&batch(&json!([command])), &context).unwrap();
        assert_ne!(room.to_canonical(), before, "{command} changes the room");
        let text = serde_json::to_string(&inverse).unwrap();
        room.apply(&RoomCommand::parse_batch(&text).unwrap(), &context)
            .unwrap();
        assert_eq!(room.to_canonical(), before, "{command}'s inverse: {text}");
    }
    // A batch moving one object twice gives it back once, marked.
    let mut room = marked();
    let before = room.to_canonical();
    let inverse = room
        .apply(
            &batch(&json!([
                { "move": { "id": "crate_1", "by": [1, 0] } },
                { "move": { "id": "crate_1", "by": [1, 0] } }
            ])),
            &context,
        )
        .unwrap();
    room.apply(&inverse, &context).unwrap();
    assert_eq!(room.to_canonical(), before);
}

#[test]
fn a_duplicates_copy_is_unmarked_and_set_generator_marks_and_detaches() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let mut room = marked();
    room.apply(
        &batch(&json!([{ "duplicate": { "id": "crate_1", "as": "crate_2", "by": [1, 0] } }])),
        &context,
    )
    .unwrap();
    assert_eq!(
        mark(&room, "crate_1"),
        Some("chain"),
        "the original keeps it"
    );
    assert_eq!(mark(&room, "crate_2"), None, "the copy is the author's");
    let at = object(&room, "crate_2").at;
    room.apply(
        &batch(&json!([
            { "set": { "id": "crate_2", "path": "generator", "value": "chain" } },
            { "set": { "id": "crate_1", "path": "generator", "value": null } }
        ])),
        &context,
    )
    .unwrap();
    assert_eq!(mark(&room, "crate_2"), Some("chain"));
    assert_eq!(
        object(&room, "crate_2").at,
        at,
        "a mark changes nothing else"
    );
    assert_eq!(mark(&room, "crate_1"), None);
    // A mark is an ID.
    let err = room
        .apply(
            &batch(&json!([{ "set": { "id": "crate_2", "path": "generator", "value": "Chain" } }])),
            &context,
        )
        .unwrap_err();
    assert!(
        err.to_string()
            .starts_with("commands[0].set: objects[1].generator: `Chain` is not"),
        "{err}"
    );
}

#[test]
fn a_batch_that_is_no_array_shows_one() {
    let err =
        RoomCommand::parse_batch(r#"{ "move": { "id": "crate_1", "by": [1, 0] } }"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        r#"a batch is a JSON array of commands, as `[{ "move": { "id": "crate_1", "by": [1, 0] } }]`"#
    );
    // The example it shows is a batch.
    let example = err.message.split('`').nth(1).unwrap();
    assert_eq!(RoomCommand::parse_batch(example).unwrap().len(), 1);
}

#[test]
fn a_refusal_carries_its_reason() {
    let (tileset, layers) = world();
    let context = RoomContext {
        room: "cave",
        tileset: &tileset,
        layers: &layers,
        prefabs: &PREFABS,
    };
    let refuse =
        |commands: serde_json::Value| cave().apply(&batch(&commands), &context).unwrap_err();
    // A command's own error: the reason is its message.
    let err = refuse(json!([{ "move": { "id": "crate_9", "by": [1, 0] } }]));
    assert_eq!(err.field, "commands[0].move.id");
    assert_eq!(err.reason, err.message);
    assert!(err.reason.starts_with("no entry `crate_9`"), "{err}");
    // A check of the whole room: the message keeps the room's path, the
    // reason drops it.
    let err = refuse(json!([{ "move": { "id": "crate_1", "by": [0, 40] } }]));
    assert_eq!(err.field, "commands[0].move");
    assert_eq!(
        err.message,
        "objects[0].at: [5, 59] is past the room's 40 \u{d7} 24 cells"
    );
    assert_eq!(err.reason, "[5, 59] is past the room's 40 \u{d7} 24 cells");
    assert!(err.to_string().ends_with(&err.reason));
    // A batch that does not read: the reason is the reader's.
    let err = RoomCommand::parse_batch(r#"[{ "mov": {} }]"#).unwrap_err();
    assert_eq!(err.reason, err.message);
}

#[test]
fn every_command_sums_up_in_a_line() {
    let lines: Vec<String> = batch(&json!([
        { "place": { "objects": { "id": "crate_2", "prefab": "crate", "at": [6, 19] } } },
        { "place": { "shapes": { "id": "step", "rect": [1, 2, 3, 4], "tile": "rock", "layers": ["art"] } } },
        { "place": { "places": { "id": "door", "point": [2.5, 19] } } },
        { "place": { "places": { "id": "zone", "rect": [1, 2, 3, 4] } } },
        { "place": { "places": { "id": "path", "route": [[1, 2], "door"] } } },
        { "place": { "layers": { "name": "back" } } },
        { "move": { "id": "crate_1", "by": [1, 0] } },
        { "move": { "id": "crate_1", "to": [8, 12.5] } },
        { "set": { "id": "ball_1", "path": "set.health.max", "value": 50 } },
        { "set": { "id": "ball_1", "path": "set.health.max", "value": null } },
        { "set": { "layer": "art", "path": "z", "value": 2 } },
        { "set": { "path": "size", "value": [40, 30] } },
        { "delete": { "id": "crate_1" } },
        { "delete": { "layer": "art" } },
        { "duplicate": { "id": "crate_1", "as": "crate_2" } },
        { "duplicate": { "id": "crate_1", "as": "crate_2", "by": [1, 0] } },
        { "paint": { "layer": "solid", "rect": [4, 12, 2, 2], "tile": "rock" } },
        { "paint": { "layer": "solid", "cells": [[4, 12]], "tile": null } },
        { "erase": { "layer": "solid", "cells": [[4, 12], [5, 12]] } },
        { "link": { "id": "exit", "to": "hall.west_door" } },
        { "link": { "id": "exit", "to": null } },
        { "rename": { "id": "crate_1", "to": "big_crate" } },
    ]))
    .iter()
    .map(RoomCommand::summary)
    .collect();
    assert_eq!(
        lines,
        [
            "place object `crate_2` (`crate`) at 6, 19",
            "place shape `step`",
            "place point `door` at 2.5, 19",
            "place area `zone` at 1, 2, 3 by 4",
            "place route `path`",
            "place layer `back` (render)",
            "move `crate_1` by 1, 0",
            "move `crate_1` to 8, 12.5",
            "set `ball_1`'s `set.health.max` to 50",
            "clear `ball_1`'s `set.health.max`",
            "set layer `art`'s `z` to 2",
            "set the room's `size` to [40,30]",
            "delete `crate_1`",
            "delete layer `art`",
            "copy `crate_1` as `crate_2`",
            "copy `crate_1` as `crate_2`, 1, 0 from it",
            "paint 4 cells of `solid` with `rock`",
            "clear 1 cell of `solid`'s overrides",
            "erase 2 cells of `solid`",
            "link `exit` to `hall.west_door`",
            "unlink `exit`",
            "rename `crate_1` to `big_crate`",
        ]
    );
}

#[test]
fn the_batch_schema_names_every_verb() {
    let schema = batch_schema();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["type"], "array");
    let commands = schema["items"]["oneOf"].as_array().unwrap();
    let described = |value: &serde_json::Value| {
        value["description"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    };
    let mut verbs = Vec::new();
    for command in commands {
        let verb = command["required"][0].as_str().unwrap();
        assert!(described(command), "`{verb}` is described");
        // Its example reads as a batch of that verb.
        let example = &command["examples"][0];
        let read = RoomCommand::batch_from_value(&json!([example]))
            .unwrap_or_else(|err| panic!("`{verb}`'s example: {err}"));
        assert_eq!(read[0].verb(), verb);
        // Every field it takes is described.
        let fields = command["properties"][verb]["properties"]
            .as_object()
            .unwrap();
        for (name, field) in fields {
            assert!(described(field), "`{verb}.{name}` is described");
        }
        verbs.push(verb);
    }
    assert_eq!(
        verbs,
        [
            "place",
            "move",
            "set",
            "delete",
            "duplicate",
            "paint",
            "erase",
            "link",
            "rename"
        ]
    );
    // The entries a place takes are the room schema's.
    assert_eq!(schema["$defs"], crate::assets::room::json_schema()["$defs"]);
    for entry in ["object", "shape", "place", "layer"] {
        assert!(schema["$defs"][entry].is_object(), "{entry}");
    }
}
