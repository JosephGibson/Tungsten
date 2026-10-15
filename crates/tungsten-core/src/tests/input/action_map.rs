use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::input::GamepadId;

const SAMPLE_JSON: &str = r#"{
    "actions": {
        "move_left":   [{ "kind": "key", "code": "ArrowLeft" }, { "kind": "key", "code": "KeyA" }],
        "jump":        [{ "kind": "key", "code": "Enter" }, { "kind": "mouse", "button": "button4" }],
        "zoom_in":     [{ "kind": "scroll", "direction": "up" }],
        "fire":        [{ "kind": "mouse", "button": "left" }]
    }
}"#;

const CHECKED_IN_INPUT_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../input.json"));

#[test]
fn default_map_has_platformer_and_engine_actions() {
    let map = ActionMap::default_map();
    for action in [
        "move_left",
        "move_right",
        "jump",
        "audio_toggle_music",
        "audio_stop_all",
        "volume_preset_low",
        "volume_preset_mid",
        "volume_preset_high",
        "zoom_in",
        "zoom_out",
        "engine_toggle_physics_debug",
        "engine_toggle_systems_overlay",
        "engine_toggle_inspector",
        "engine_toggle_hud",
        "engine_toggle_ui_bounds",
        "engine_toggle_ui",
        "engine_toggle_vsync",
        "engine_toggle_fullscreen",
        "engine_exit",
        "state_start",
        "state_pause",
        "state_back",
    ] {
        assert!(
            !map.bindings(action).is_empty(),
            "default map missing action '{action}'"
        );
    }
    assert!(map.bindings("jump").contains(&Binding::Mouse {
        button: MouseButton::Left
    }));
    assert!(map.bindings("zoom_in").contains(&Binding::Scroll {
        direction: ScrollDirection::Up
    }));
}

#[test]
fn default_map_binds_the_ui_actions() {
    let map = ActionMap::default_map();
    let keys = |codes: &[KeyCode]| {
        codes
            .iter()
            .map(|&code| Binding::Key { code })
            .collect::<Vec<_>>()
    };
    // The keys lead; the gamepad's follow (M54).
    let leading = |action: &str, codes: &[KeyCode]| {
        assert_eq!(
            &map.bindings(action)[..codes.len()],
            keys(codes),
            "{action}"
        );
    };
    leading(UI_ACCEPT, &[KeyCode::Enter, KeyCode::Space]);
    leading(UI_CANCEL, &[KeyCode::Escape]);
    leading(UI_NEXT, &[KeyCode::Tab]);
    leading(UI_UP, &[KeyCode::ArrowUp]);
    leading(UI_DOWN, &[KeyCode::ArrowDown]);
    leading(UI_LEFT, &[KeyCode::ArrowLeft]);
    leading(UI_RIGHT, &[KeyCode::ArrowRight]);
    assert!(
        map.bindings(UI_PREV)
            .iter()
            .all(|binding| !matches!(binding, Binding::Key { .. })),
        "no key: Shift with ui_next goes back"
    );
    assert!(
        map.iter().any(|(action, _)| action == UI_PREV),
        "listed, so a rebind screen can offer it"
    );
    assert_eq!(UI_ACCEPT, "ui_accept");
    assert_eq!(UI_PREV, "ui_prev");
}

#[test]
fn unknown_action_returns_false() {
    let map = ActionMap::default_map();
    let input = InputState::new();
    assert!(!map.is_pressed(&input, "dance"));
    assert!(!map.just_pressed(&input, "dance"));
    assert!(!map.just_released(&input, "dance"));
    assert!(map.bindings("dance").is_empty());
}

#[test]
fn merged_with_defaults_preserves_user_overrides() {
    let loaded: ActionMap =
        ActionMap::from_json(SAMPLE_JSON, Path::new("<test>")).expect("parse sample");
    let merged = ActionMap::merged_with_defaults(loaded);

    let jump = merged.bindings("jump");
    assert_eq!(
        jump,
        &[
            Binding::Key {
                code: KeyCode::Enter
            },
            Binding::Mouse {
                button: MouseButton::Other(4)
            }
        ]
    );

    assert!(
        merged
            .bindings("engine_toggle_hud")
            .contains(&Binding::Key { code: KeyCode::F4 })
    );

    assert_eq!(
        merged.bindings("fire"),
        &[Binding::Mouse {
            button: MouseButton::Left
        }]
    );
}

#[test]
fn load_parses_sample_input_json() {
    let map = ActionMap::from_json(SAMPLE_JSON, Path::new("<sample>")).unwrap();
    assert_eq!(
        map.bindings("move_left"),
        &[
            Binding::Key {
                code: KeyCode::ArrowLeft
            },
            Binding::Key {
                code: KeyCode::KeyA
            }
        ]
    );
    assert_eq!(
        map.bindings("jump"),
        &[
            Binding::Key {
                code: KeyCode::Enter
            },
            Binding::Mouse {
                button: MouseButton::Other(4)
            }
        ]
    );
    assert_eq!(
        map.bindings("zoom_in"),
        &[Binding::Scroll {
            direction: ScrollDirection::Up
        }]
    );
}

#[test]
fn checked_in_input_json_loads() {
    let map = ActionMap::from_json(CHECKED_IN_INPUT_JSON, Path::new("input.json")).unwrap();
    assert_eq!(
        map.bindings("playground_cycle_post_aa"),
        &[Binding::Key { code: KeyCode::Tab }]
    );
    assert_eq!(
        map.bindings("playground_post_aa_off"),
        &[Binding::Key {
            code: KeyCode::Digit0
        }]
    );
    assert_eq!(
        map.bindings("playground_post_aa_ultra"),
        &[Binding::Key {
            code: KeyCode::Digit8
        }]
    );
}

#[test]
fn load_invalid_json_is_error() {
    let err = ActionMap::from_json("{ not json", Path::new("<bad>")).unwrap_err();
    assert!(matches!(err, ActionMapError::Parse { .. }));
}

#[test]
fn query_respects_multiple_bindings() {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.key_down(KeyCode::KeyA);
    assert!(map.is_pressed(&input, "move_left"));
    input.key_up(KeyCode::KeyA);
    input.begin_frame();
    input.key_down(KeyCode::ArrowLeft);
    assert!(map.is_pressed(&input, "move_left"));
}

#[test]
fn edge_query_just_pressed_any_binding() {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.key_down(KeyCode::ArrowLeft);
    assert!(map.just_pressed(&input, "move_left"));
    input.begin_frame();
    assert!(!map.just_pressed(&input, "move_left"));
}

#[test]
fn mouse_button_binding_dispatches() {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.mouse_down(MouseButton::Left);
    assert!(map.is_pressed(&input, "jump"));
    assert!(map.just_pressed(&input, "jump"));
    input.begin_frame();
    input.mouse_up(MouseButton::Left);
    assert!(map.just_released(&input, "jump"));
}

#[test]
fn scroll_binding_dispatches() {
    let map = ActionMap::default_map();
    let mut input = InputState::new();
    input.add_scroll_line_delta(0.0, 1.0);
    assert!(map.is_pressed(&input, "zoom_in"));
    assert!(map.just_pressed(&input, "zoom_in"));
    input.begin_frame();
    assert!(!map.is_pressed(&input, "zoom_in"));
    assert!(map.just_released(&input, "zoom_in"));
}

#[test]
fn replace_bindings_rebinds_runtime() {
    let mut map = ActionMap::default_map();
    map.replace_bindings(
        "jump",
        vec![
            Binding::Key {
                code: KeyCode::Enter,
            },
            Binding::Mouse {
                button: MouseButton::Other(5),
            },
        ],
    );
    let mut input = InputState::new();
    input.key_down(KeyCode::Enter);
    assert!(map.is_pressed(&input, "jump"));
    input.key_up(KeyCode::Enter);
    input.mouse_down(MouseButton::Other(5));
    assert!(map.is_pressed(&input, "jump"));
    input.mouse_up(MouseButton::Other(5));
    input.key_down(KeyCode::Space);
    assert!(!map.is_pressed(&input, "jump"));
}

#[test]
fn duplicate_bindings_are_deduplicated() {
    let dupes = r#"{ "actions": { "jump": [
        { "kind": "key", "code": "Space" },
        { "kind": "key", "code": "Space" },
        { "kind": "scroll", "direction": "up" },
        { "kind": "scroll", "direction": "up" }
    ] } }"#;
    let map = ActionMap::from_json(dupes, Path::new("<dupe>")).unwrap();
    assert_eq!(
        map.bindings("jump"),
        &[
            Binding::Key {
                code: KeyCode::Space
            },
            Binding::Scroll {
                direction: ScrollDirection::Up
            }
        ]
    );
}

#[test]
fn binding_round_trip_json() {
    let bindings = vec![
        Binding::Key {
            code: KeyCode::ArrowLeft,
        },
        Binding::Mouse {
            button: MouseButton::Other(4),
        },
        Binding::Scroll {
            direction: ScrollDirection::Down,
        },
    ];
    let json = serde_json::to_string(&bindings).unwrap();
    let parsed: Vec<Binding> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, bindings);
}

#[test]
fn persist_round_trips_rebound_actions_and_preserves_other_top_level_fields() {
    let dir = tempdir();
    let path = dir.join("input.json");
    std::fs::write(
        &path,
        "{\n  \"note\": \"keep\",\n  \"actions\": {\n    \"jump\": [{ \"kind\": \"key\", \"code\": \"Space\" }]\n  }\n}\n",
    )
    .unwrap();

    let mut map = ActionMap::load(&path).unwrap();
    map.replace_bindings(
        "jump",
        vec![Binding::Key {
            code: KeyCode::Enter,
        }],
    );
    map.persist().unwrap();

    let persisted = std::fs::read_to_string(&path).unwrap();
    assert!(persisted.contains("\"note\": \"keep\""));
    assert!(persisted.contains("\"Enter\""));

    let reparsed = ActionMap::load(&path).unwrap();
    assert_eq!(
        reparsed.bindings("jump"),
        &[Binding::Key {
            code: KeyCode::Enter
        }]
    );
}

#[test]
fn persist_can_create_missing_input_json_from_defaults() {
    let dir = tempdir();
    let path = dir.join("input.json");
    let mut map = ActionMap::default_map();
    map.set_source_path(&path);
    map.persist().unwrap();

    let persisted = std::fs::read_to_string(&path).unwrap();
    assert!(persisted.contains("\"engine_toggle_hud\""));
    assert!(persisted.contains("\"zoom_in\""));
}

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tungsten_action_map_test_{}_{}",
        std::process::id(),
        nonce
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn persist_preserves_existing_temporary_files_and_cleans_failed_writes() {
    let dir = tempdir();
    let stale = dir.join(format!(".input.json.{}.0.tmp", std::process::id()));
    std::fs::write(&stale, "another writer").unwrap();
    let path = dir.join("input.json");
    ActionMap::default_map().persist_to(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&stale).unwrap(), "another writer");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

    // A directory cannot be replaced by a file; our temporary file is removed.
    let destination = dir.join("directory");
    std::fs::create_dir(&destination).unwrap();
    assert!(ActionMap::default_map().persist_to(&destination).is_err());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_utf8_byte_order_mark_is_skipped_and_kept_on_persist() {
    // An editor that writes a byte-order mark would otherwise make the
    // player's input.json fatal at the next start (`D-008`).
    let dir = tempdir();
    let path = dir.join("input.json");
    std::fs::write(&path, format!("\u{feff}{SAMPLE_JSON}")).unwrap();
    let mut map = ActionMap::load(&path).expect("a byte-order mark is skipped");
    assert!(!map.bindings("jump").is_empty());
    map.replace_bindings(
        "jump",
        vec![Binding::Key {
            code: KeyCode::KeyZ,
        }],
    );
    map.persist().unwrap();
    let persisted = std::fs::read_to_string(&path).unwrap();
    assert!(persisted.starts_with('\u{feff}'), "{persisted:?}");
    let reloaded = ActionMap::load(&path).unwrap();
    assert_eq!(
        reloaded.bindings("jump"),
        &[Binding::Key {
            code: KeyCode::KeyZ
        }]
    );
}

#[test]
fn a_key_with_no_name_is_skipped_and_the_rest_saved() {
    let dir = tempdir();
    let path = dir.join("input.json");
    let mut map = ActionMap::default_map();
    map.set_source_path(&path);
    map.replace_bindings_and_persist(
        "jump",
        vec![
            Binding::Key {
                code: KeyCode::Other(42),
            },
            Binding::Key {
                code: KeyCode::Enter,
            },
            Binding::Mouse {
                button: MouseButton::Other(0),
            },
        ],
    )
    .expect("an unnamed key no longer fails the whole map");
    let reloaded = ActionMap::merged_with_defaults(ActionMap::load(&path).unwrap());
    assert_eq!(
        reloaded.bindings("jump"),
        &[
            Binding::Key {
                code: KeyCode::Enter
            },
            Binding::Mouse {
                button: MouseButton::Other(0)
            }
        ]
    );
    assert_eq!(
        reloaded.bindings("move_left"),
        ActionMap::default_map().bindings("move_left")
    );
}

#[test]
fn an_action_left_with_no_saveable_binding_is_left_out() {
    let dir = tempdir();
    let path = dir.join("input.json");
    let mut map = ActionMap::default_map();
    map.replace_bindings(
        "jump",
        vec![Binding::Key {
            code: KeyCode::Other(42),
        }],
    );
    map.replace_bindings("ui_prev", Vec::new());
    map.persist_to(&path).unwrap();
    let persisted = std::fs::read_to_string(&path).unwrap();
    assert!(!persisted.contains("\"jump\""), "{persisted}");
    assert!(
        persisted.contains("\"ui_prev\": []"),
        "an action disabled on purpose stays disabled"
    );
    let reloaded = ActionMap::merged_with_defaults(ActionMap::load(&path).unwrap());
    assert_eq!(
        reloaded.bindings("jump"),
        ActionMap::default_map().bindings("jump"),
        "a restart falls back to the layer below rather than disabling it"
    );
}

const SHIPPED_JSON: &str = r#"{
    "note": "shipped",
    "actions": {
        "jump": [{ "kind": "key", "code": "Space" }],
        "fire": [{ "kind": "mouse", "button": "left" }]
    }
}"#;

/// The shipped file in `dir` and the map the app builds from it.
fn shipped_in(dir: &Path) -> (PathBuf, ActionMap) {
    let path = dir.join("input.json");
    std::fs::write(&path, SHIPPED_JSON).unwrap();
    let map = ActionMap::merged_with_defaults(ActionMap::load(&path).unwrap());
    (path, map)
}

fn key(code: KeyCode) -> Binding {
    Binding::Key { code }
}

#[test]
fn a_user_layer_overlays_per_action_and_saves_only_the_difference() {
    let dir = tempdir();
    let (_, shipped) = shipped_in(&dir);
    let user = dir.join("user").join("input.json");
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    std::fs::write(
        &user,
        r#"{ "actions": { "jump": [{ "kind": "key", "code": "KeyZ" }],
            "ui_prev": [{ "kind": "gamepad_button", "button": "LeftShoulder" }] } }"#,
    )
    .unwrap();

    let mut map = shipped.with_user_layer(Some(&user));
    assert!(map.take_load_warnings().is_empty());
    assert_eq!(map.user_layer_path(), Some(user.as_path()));
    assert_eq!(map.bindings("jump"), &[key(KeyCode::KeyZ)]);
    assert_eq!(
        map.bindings("fire"),
        &[Binding::Mouse {
            button: MouseButton::Left
        }],
        "the shipped layer shows through"
    );
    assert_eq!(
        map.bindings("move_left"),
        ActionMap::default_map().bindings("move_left")
    );

    map.replace_bindings_and_persist("move_left", vec![key(KeyCode::KeyQ)])
        .unwrap();
    let saved = ActionMap::load(&user).unwrap();
    let mut names: Vec<&str> = saved.iter().map(|(action, _)| action).collect();
    names.sort_unstable();
    assert_eq!(names, ["jump", "move_left"], "only the player's changes");

    map.replace_bindings_and_persist("jump", vec![key(KeyCode::Space)])
        .unwrap();
    let saved = ActionMap::load(&user).unwrap();
    assert!(
        saved.bindings("jump").is_empty() && !saved.iter().any(|(a, _)| a == "jump"),
        "an action set back to the shipped binding leaves the file"
    );
}

#[test]
fn persist_with_a_user_layer_leaves_the_shipped_file_alone() {
    let dir = tempdir();
    let (shipped_path, shipped) = shipped_in(&dir);
    let user = dir.join("settings").join("input.json");
    let mut map = shipped.with_user_layer(Some(&user));
    map.replace_bindings_and_persist("fire", vec![key(KeyCode::KeyF)])
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&shipped_path).unwrap(),
        SHIPPED_JSON,
        "the shipped file's bytes are unchanged"
    );
    let saved = ActionMap::load(&user).unwrap();
    assert_eq!(saved.bindings("fire"), &[key(KeyCode::KeyF)]);
    assert_eq!(saved.iter().count(), 1);

    let (_, shipped) = shipped_in(&dir);
    let restarted = shipped.with_user_layer(Some(&user));
    assert_eq!(restarted.bindings("fire"), &[key(KeyCode::KeyF)]);
    assert_eq!(restarted.bindings("jump"), &[key(KeyCode::Space)]);
}

#[test]
fn a_corrupt_user_layer_falls_back_with_a_backup() {
    let dir = tempdir();
    let (_, shipped) = shipped_in(&dir);
    let user_dir = dir.join("settings");
    std::fs::create_dir_all(&user_dir).unwrap();
    let user = user_dir.join("input.json");
    let corrupt = r#"{ "actions": { "jump": [{ "kind": "key", "code": "Dance" }] } }"#;
    std::fs::write(&user, corrupt).unwrap();

    let mut map = shipped.with_user_layer(Some(&user));
    assert_eq!(map.bindings("jump"), &[key(KeyCode::Space)]);
    let backup = user_dir.join("input.json.bad");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), corrupt);
    assert!(!user.exists());
    let warnings = map.take_load_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains(&backup.display().to_string()),
        "{warnings:?}"
    );

    map.replace_bindings_and_persist("jump", vec![key(KeyCode::KeyX)])
        .unwrap();
    assert_eq!(
        ActionMap::load(&user).unwrap().bindings("jump"),
        &[key(KeyCode::KeyX)]
    );
}

#[test]
fn no_user_layer_persist_is_missing_source_path() {
    let dir = tempdir();
    let (shipped_path, shipped) = shipped_in(&dir);
    let mut map = shipped.with_user_layer(None);
    assert_eq!(map.user_layer_path(), None);
    let error = map
        .replace_bindings_and_persist("jump", vec![key(KeyCode::KeyX)])
        .unwrap_err();
    assert!(
        matches!(error, ActionMapError::MissingSourcePath),
        "{error}"
    );
    assert_eq!(
        map.bindings("jump"),
        &[key(KeyCode::KeyX)],
        "still in memory"
    );
    assert_eq!(
        std::fs::read_to_string(&shipped_path).unwrap(),
        SHIPPED_JSON
    );
}

// ---- gamepads (M54) ----

#[test]
fn gamepad_bindings_round_trip_through_json() {
    let json = r#"{
    "actions": {
        "jump": [{ "kind": "gamepad_button", "button": "South" }],
        "move_left": [
            { "kind": "gamepad_axis", "axis": "LeftStickX", "direction": "negative" },
            { "kind": "gamepad_button", "button": "DPadLeft" }
        ]
    }
}"#;
    let map = ActionMap::from_json(json, Path::new("pad.json")).unwrap();
    assert_eq!(
        map.bindings("jump"),
        &[Binding::GamepadButton {
            button: GamepadButton::South
        }]
    );
    assert_eq!(
        map.bindings("move_left"),
        &[
            Binding::GamepadAxis {
                axis: GamepadAxis::LeftStickX,
                direction: AxisDirection::Negative
            },
            Binding::GamepadButton {
                button: GamepadButton::DPadLeft
            }
        ]
    );
    let rendered = serde_json::to_string(map.bindings("move_left")).unwrap();
    assert_eq!(
        rendered,
        r#"[{"kind":"gamepad_axis","axis":"LeftStickX","direction":"negative"},{"kind":"gamepad_button","button":"DPadLeft"}]"#
    );
    let back: Vec<Binding> = serde_json::from_str(&rendered).unwrap();
    assert_eq!(back, map.bindings("move_left"));

    // Every name loads back to its button, axis or direction.
    for button in [
        GamepadButton::South,
        GamepadButton::East,
        GamepadButton::North,
        GamepadButton::West,
        GamepadButton::LeftShoulder,
        GamepadButton::RightShoulder,
        GamepadButton::LeftTrigger,
        GamepadButton::RightTrigger,
        GamepadButton::Select,
        GamepadButton::Start,
        GamepadButton::Mode,
        GamepadButton::LeftStick,
        GamepadButton::RightStick,
        GamepadButton::DPadUp,
        GamepadButton::DPadDown,
        GamepadButton::DPadLeft,
        GamepadButton::DPadRight,
    ] {
        let name = serde_json::to_string(&button).unwrap();
        assert_eq!(name.trim_matches('"'), format!("{button:?}"));
        assert_eq!(
            serde_json::from_str::<GamepadButton>(&name).unwrap(),
            button
        );
    }
    for axis in [
        GamepadAxis::LeftStickX,
        GamepadAxis::LeftStickY,
        GamepadAxis::RightStickX,
        GamepadAxis::RightStickY,
    ] {
        let name = serde_json::to_string(&axis).unwrap();
        assert_eq!(serde_json::from_str::<GamepadAxis>(&name).unwrap(), axis);
    }
    assert!(serde_json::from_str::<GamepadButton>(r#""A""#).is_err());
    assert!(serde_json::from_str::<AxisDirection>(r#""up""#).is_err());
}

#[test]
fn value_reads_a_sticks_magnitude() {
    let mut map = ActionMap::new();
    map.replace_bindings(
        "move_right",
        vec![
            Binding::Key {
                code: KeyCode::KeyD,
            },
            Binding::GamepadAxis {
                axis: GamepadAxis::LeftStickX,
                direction: AxisDirection::Positive,
            },
        ],
    );
    let pad = GamepadId::new(0);
    let mut input = InputState::new();
    let value = |input: &InputState| map.value(input, "move_right");
    assert_eq!(value(&input), 0.0);
    for (tilt, expected) in [
        (0.2, 0.0),
        (0.35, 0.0),
        (0.675, 0.5),
        (1.0, 1.0),
        (-1.0, 0.0),
    ] {
        input.set_gamepad_axis(pad, GamepadAxis::LeftStickX, tilt);
        assert!(
            (value(&input) - expected).abs() < 1e-6,
            "tilt {tilt}: {}",
            value(&input)
        );
    }
    // A held key is full speed, whatever the stick says.
    input.set_gamepad_axis(pad, GamepadAxis::LeftStickX, 0.5);
    input.key_down(KeyCode::KeyD);
    assert_eq!(value(&input), 1.0);
    input.key_up(KeyCode::KeyD);
    // The farthest pad that way counts.
    input.set_gamepad_axis(GamepadId::new(1), GamepadAxis::LeftStickX, -1.0);
    assert!((value(&input) - 0.15 / 0.65).abs() < 1e-6);
    assert_eq!(map.value(&input, "unknown"), 0.0);
}

#[test]
fn default_map_binds_ui_actions_on_a_gamepad() {
    let map = ActionMap::default_map();
    let pad = |button| Binding::GamepadButton { button };
    let stick = |axis, direction| Binding::GamepadAxis { axis, direction };
    let has = |action: &str, binding: Binding| {
        assert!(
            map.bindings(action).contains(&binding),
            "{action}: {binding:?}"
        );
    };
    has(UI_ACCEPT, pad(GamepadButton::South));
    has(UI_CANCEL, pad(GamepadButton::East));
    has(UI_NEXT, pad(GamepadButton::RightShoulder));
    has(UI_PREV, pad(GamepadButton::LeftShoulder));
    for (action, button, axis, direction) in [
        (
            UI_UP,
            GamepadButton::DPadUp,
            GamepadAxis::LeftStickY,
            AxisDirection::Positive,
        ),
        (
            UI_DOWN,
            GamepadButton::DPadDown,
            GamepadAxis::LeftStickY,
            AxisDirection::Negative,
        ),
        (
            UI_LEFT,
            GamepadButton::DPadLeft,
            GamepadAxis::LeftStickX,
            AxisDirection::Negative,
        ),
        (
            UI_RIGHT,
            GamepadButton::DPadRight,
            GamepadAxis::LeftStickX,
            AxisDirection::Positive,
        ),
    ] {
        has(action, pad(button));
        has(action, stick(axis, direction));
    }

    // A press reads through the action as a key's does.
    let mut input = InputState::new();
    input.gamepad_button_down(GamepadId::new(2), GamepadButton::South);
    assert!(map.just_pressed(&input, UI_ACCEPT));
    assert!(map.is_pressed(&input, UI_ACCEPT));
    input.set_gamepad_axis(GamepadId::new(2), GamepadAxis::LeftStickY, -0.8);
    assert!(map.just_pressed(&input, UI_DOWN));
    assert!(!map.is_pressed(&input, UI_UP));
}
