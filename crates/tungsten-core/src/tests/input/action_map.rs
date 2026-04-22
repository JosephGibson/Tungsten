use super::*;

const SAMPLE_JSON: &str = r#"{
    "actions": {
        "move_left":   [{ "kind": "key", "code": "ArrowLeft" }, { "kind": "key", "code": "KeyA" }],
        "jump":        [{ "kind": "key", "code": "Enter" }, { "kind": "mouse", "button": "button4" }],
        "zoom_in":     [{ "kind": "scroll", "direction": "up" }],
        "fire":        [{ "kind": "mouse", "button": "left" }]
    }
}"#;

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

    assert!(merged
        .bindings("engine_toggle_hud")
        .contains(&Binding::Key { code: KeyCode::F4 }));

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
