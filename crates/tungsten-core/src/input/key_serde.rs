//! Stable string names for `KeyCode` / `MouseButton` used by `input.json`
//! parsing and serde helpers. Names track the `winit::keyboard::KeyCode`
//! variant names exactly (e.g. `"ArrowLeft"`, `"KeyA"`, `"Space"`) so a
//! user writing `input.json` can rely on a single canonical spelling.

use crate::input::{KeyCode, MouseButton, ScrollDirection};

const KEYCODE_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::ArrowUp, "ArrowUp"),
    (KeyCode::ArrowDown, "ArrowDown"),
    (KeyCode::ArrowLeft, "ArrowLeft"),
    (KeyCode::ArrowRight, "ArrowRight"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Escape, "Escape"),
    (KeyCode::F4, "F4"),
    (KeyCode::F9, "F9"),
    (KeyCode::F11, "F11"),
    (KeyCode::KeyW, "KeyW"),
    (KeyCode::KeyA, "KeyA"),
    (KeyCode::KeyS, "KeyS"),
    (KeyCode::KeyD, "KeyD"),
    (KeyCode::KeyM, "KeyM"),
    (KeyCode::KeyV, "KeyV"),
    (KeyCode::Digit1, "Digit1"),
    (KeyCode::Digit2, "Digit2"),
    (KeyCode::Digit3, "Digit3"),
    (KeyCode::Equal, "Equal"),
    (KeyCode::Minus, "Minus"),
];

const MOUSE_BUTTON_NAMES: &[(MouseButton, &str)] = &[
    (MouseButton::Left, "left"),
    (MouseButton::Right, "right"),
    (MouseButton::Middle, "middle"),
];

const SCROLL_DIRECTION_NAMES: &[(ScrollDirection, &str)] =
    &[(ScrollDirection::Up, "up"), (ScrollDirection::Down, "down")];

pub fn keycode_from_str(name: &str) -> Option<KeyCode> {
    KEYCODE_NAMES
        .iter()
        .find(|(_, s)| *s == name)
        .map(|(code, _)| *code)
}

pub fn keycode_to_str(code: KeyCode) -> Option<&'static str> {
    KEYCODE_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, name)| *name)
}

pub fn mouse_button_from_str(name: &str) -> Option<MouseButton> {
    if let Some(button) = MOUSE_BUTTON_NAMES
        .iter()
        .find(|(_, s)| *s == name)
        .map(|(button, _)| *button)
    {
        return Some(button);
    }

    name.strip_prefix("button")
        .and_then(|rest| rest.parse::<u16>().ok())
        .filter(|id| *id > 0)
        .map(MouseButton::Other)
}

pub fn mouse_button_to_string(button: MouseButton) -> String {
    match button {
        MouseButton::Other(id) => format!("button{id}"),
        standard => MOUSE_BUTTON_NAMES
            .iter()
            .find(|(candidate, _)| *candidate == standard)
            .map(|(_, name)| (*name).to_string())
            .unwrap_or_else(|| "button0".to_string()),
    }
}

pub fn scroll_direction_from_str(name: &str) -> Option<ScrollDirection> {
    SCROLL_DIRECTION_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(direction, _)| *direction)
}

pub fn scroll_direction_to_str(direction: ScrollDirection) -> &'static str {
    SCROLL_DIRECTION_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == direction)
        .map(|(_, name)| *name)
        .expect("scroll direction missing canonical name")
}

#[cfg(test)]
mod tests {
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
}
