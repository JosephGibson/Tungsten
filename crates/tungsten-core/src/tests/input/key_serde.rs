use super::*;

#[test]
fn keycode_names_round_trip() {
    for (code, name) in KEYCODE_NAMES {
        assert_eq!(keycode_from_str(name), Some(*code));
        assert_eq!(keycode_to_str(*code), Some(*name));
    }
}

#[test]
fn keycode_other_has_no_name() {
    assert!(keycode_to_str(KeyCode::Other(42)).is_none());
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
