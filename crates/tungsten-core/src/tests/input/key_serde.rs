use super::*;

#[test]
fn keycode_names_round_trip() {
    for (code, name) in KEYCODE_NAMES {
        assert_eq!(keycode_from_str(name), Some(*code));
        assert_eq!(keycode_to_str(*code), Some(*name));
    }
    // One row per named variant; the window bridge's key table pins the same
    // count, so every name has a winit key.
    assert_eq!(KEYCODE_NAMES.len(), 74);
}

#[test]
fn each_new_key_name_round_trips_through_json() {
    let new = [
        (KeyCode::Home, "Home"),
        (KeyCode::End, "End"),
        (KeyCode::Delete, "Delete"),
        (KeyCode::PageUp, "PageUp"),
        (KeyCode::PageDown, "PageDown"),
        (KeyCode::ShiftLeft, "ShiftLeft"),
        (KeyCode::ShiftRight, "ShiftRight"),
        (KeyCode::ControlLeft, "ControlLeft"),
        (KeyCode::ControlRight, "ControlRight"),
        (KeyCode::AltLeft, "AltLeft"),
        (KeyCode::AltRight, "AltRight"),
        (KeyCode::SuperLeft, "SuperLeft"),
        (KeyCode::SuperRight, "SuperRight"),
        (KeyCode::KeyE, "KeyE"),
        (KeyCode::KeyG, "KeyG"),
        (KeyCode::KeyO, "KeyO"),
        (KeyCode::KeyQ, "KeyQ"),
        (KeyCode::KeyT, "KeyT"),
        (KeyCode::KeyX, "KeyX"),
        (KeyCode::KeyZ, "KeyZ"),
        (KeyCode::Digit4, "Digit4"),
        (KeyCode::Digit9, "Digit9"),
        (KeyCode::F5, "F5"),
        (KeyCode::F6, "F6"),
        (KeyCode::F7, "F7"),
        (KeyCode::F8, "F8"),
        (KeyCode::F10, "F10"),
        (KeyCode::F12, "F12"),
    ];
    for (code, name) in new {
        let json = format!("\"{name}\"");
        assert_eq!(serde_json::to_string(&code).unwrap(), json);
        assert_eq!(serde_json::from_str::<KeyCode>(&json).unwrap(), code);
    }
}

#[test]
fn keycode_other_has_no_name() {
    assert!(keycode_to_str(KeyCode::Other(42)).is_none());
}

#[test]
fn keycode_other_fails_to_serialize() {
    assert!(serde_json::to_string(&KeyCode::Other(42)).is_err());
}

#[test]
fn keycode_unknown_name_returns_none() {
    assert!(keycode_from_str("Dance").is_none());
}

#[test]
fn mouse_button_names_round_trip() {
    for (button, name) in MOUSE_BUTTON_NAMES {
        assert_eq!(mouse_button_from_str(name), Some(*button));
        assert_eq!(mouse_button_to_string(*button), *name);
    }
}

#[test]
fn mouse_button_other_round_trips_through_button_prefix() {
    assert_eq!(
        mouse_button_from_str("button4"),
        Some(MouseButton::Other(4))
    );
    assert_eq!(mouse_button_to_string(MouseButton::Other(4)), "button4");
}

#[test]
fn mouse_button_zero_is_rejected() {
    assert!(mouse_button_from_str("button0").is_none());
}

#[test]
fn scroll_direction_round_trips() {
    for (direction, name) in SCROLL_DIRECTION_NAMES {
        assert_eq!(scroll_direction_from_str(name), Some(*direction));
        assert_eq!(scroll_direction_to_str(*direction), *name);
    }
}
