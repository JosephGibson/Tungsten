//! Canonical `input.json` names for keys, mouse buttons, and scroll directions.

use crate::input::{KeyCode, MouseButton, ScrollDirection};

const KEYCODE_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::ArrowUp, "ArrowUp"),
    (KeyCode::ArrowDown, "ArrowDown"),
    (KeyCode::ArrowLeft, "ArrowLeft"),
    (KeyCode::ArrowRight, "ArrowRight"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Escape, "Escape"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F9, "F9"),
    (KeyCode::F11, "F11"),
    (KeyCode::KeyW, "KeyW"),
    (KeyCode::KeyA, "KeyA"),
    (KeyCode::KeyS, "KeyS"),
    (KeyCode::KeyD, "KeyD"),
    (KeyCode::KeyM, "KeyM"),
    (KeyCode::KeyP, "KeyP"),
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

#[must_use]
pub fn keycode_from_str(name: &str) -> Option<KeyCode> {
    KEYCODE_NAMES
        .iter()
        .find(|(_, s)| *s == name)
        .map(|(code, _)| *code)
}

#[must_use]
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

#[must_use]
pub fn mouse_button_to_string(button: MouseButton) -> String {
    match button {
        MouseButton::Other(id) => format!("button{id}"),
        standard => MOUSE_BUTTON_NAMES
            .iter()
            .find(|(candidate, _)| *candidate == standard)
            .map_or_else(|| "button0".to_string(), |(_, name)| (*name).to_string()),
    }
}

#[must_use]
pub fn scroll_direction_from_str(name: &str) -> Option<ScrollDirection> {
    SCROLL_DIRECTION_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(direction, _)| *direction)
}

#[must_use]
pub fn scroll_direction_to_str(direction: ScrollDirection) -> &'static str {
    SCROLL_DIRECTION_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == direction)
        .map(|(_, name)| *name)
        .expect("scroll direction missing canonical name")
}

#[cfg(test)]
#[path = "../tests/input/key_serde.rs"]
mod tests;
