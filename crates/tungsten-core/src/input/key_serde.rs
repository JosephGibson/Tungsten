//! Canonical `input.json` names for keys, mouse buttons, scroll directions,
//! gamepad buttons, gamepad axes and axis directions.

use crate::input::{
    AxisDirection, GamepadAxis, GamepadButton, KeyCode, MouseButton, ScrollDirection,
};

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
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::KeyW, "KeyW"),
    (KeyCode::KeyA, "KeyA"),
    (KeyCode::KeyS, "KeyS"),
    (KeyCode::KeyD, "KeyD"),
    (KeyCode::KeyF, "KeyF"),
    (KeyCode::KeyB, "KeyB"),
    (KeyCode::KeyC, "KeyC"),
    (KeyCode::KeyE, "KeyE"),
    (KeyCode::KeyG, "KeyG"),
    (KeyCode::KeyH, "KeyH"),
    (KeyCode::KeyI, "KeyI"),
    (KeyCode::KeyJ, "KeyJ"),
    (KeyCode::KeyK, "KeyK"),
    (KeyCode::KeyL, "KeyL"),
    (KeyCode::KeyM, "KeyM"),
    (KeyCode::KeyN, "KeyN"),
    (KeyCode::KeyO, "KeyO"),
    (KeyCode::KeyP, "KeyP"),
    (KeyCode::KeyQ, "KeyQ"),
    (KeyCode::KeyR, "KeyR"),
    (KeyCode::KeyT, "KeyT"),
    (KeyCode::KeyU, "KeyU"),
    (KeyCode::KeyV, "KeyV"),
    (KeyCode::KeyX, "KeyX"),
    (KeyCode::KeyY, "KeyY"),
    (KeyCode::KeyZ, "KeyZ"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Digit0, "Digit0"),
    (KeyCode::Digit1, "Digit1"),
    (KeyCode::Digit2, "Digit2"),
    (KeyCode::Digit3, "Digit3"),
    (KeyCode::Digit4, "Digit4"),
    (KeyCode::Digit5, "Digit5"),
    (KeyCode::Digit6, "Digit6"),
    (KeyCode::Digit7, "Digit7"),
    (KeyCode::Digit8, "Digit8"),
    (KeyCode::Digit9, "Digit9"),
    (KeyCode::Equal, "Equal"),
    (KeyCode::Minus, "Minus"),
    (KeyCode::BracketLeft, "BracketLeft"),
    (KeyCode::BracketRight, "BracketRight"),
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
];

const MOUSE_BUTTON_NAMES: &[(MouseButton, &str)] = &[
    (MouseButton::Left, "left"),
    (MouseButton::Right, "right"),
    (MouseButton::Middle, "middle"),
];

const SCROLL_DIRECTION_NAMES: &[(ScrollDirection, &str)] =
    &[(ScrollDirection::Up, "up"), (ScrollDirection::Down, "down")];

/// Positional names, the variant names (Q10 of M54).
const GAMEPAD_BUTTON_NAMES: &[(GamepadButton, &str)] = &[
    (GamepadButton::South, "South"),
    (GamepadButton::East, "East"),
    (GamepadButton::North, "North"),
    (GamepadButton::West, "West"),
    (GamepadButton::LeftShoulder, "LeftShoulder"),
    (GamepadButton::RightShoulder, "RightShoulder"),
    (GamepadButton::LeftTrigger, "LeftTrigger"),
    (GamepadButton::RightTrigger, "RightTrigger"),
    (GamepadButton::Select, "Select"),
    (GamepadButton::Start, "Start"),
    (GamepadButton::Mode, "Mode"),
    (GamepadButton::LeftStick, "LeftStick"),
    (GamepadButton::RightStick, "RightStick"),
    (GamepadButton::DPadUp, "DPadUp"),
    (GamepadButton::DPadDown, "DPadDown"),
    (GamepadButton::DPadLeft, "DPadLeft"),
    (GamepadButton::DPadRight, "DPadRight"),
];

const GAMEPAD_AXIS_NAMES: &[(GamepadAxis, &str)] = &[
    (GamepadAxis::LeftStickX, "LeftStickX"),
    (GamepadAxis::LeftStickY, "LeftStickY"),
    (GamepadAxis::RightStickX, "RightStickX"),
    (GamepadAxis::RightStickY, "RightStickY"),
];

const AXIS_DIRECTION_NAMES: &[(AxisDirection, &str)] = &[
    (AxisDirection::Negative, "negative"),
    (AxisDirection::Positive, "positive"),
];

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

    // `button0` too: `mouse_button_to_string` writes it, so every saved
    // button loads (Core F4).
    name.strip_prefix("button")
        .and_then(|rest| rest.parse::<u16>().ok())
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

/// The gamepad button named `name` in `input.json`, such as `South`.
#[must_use]
pub fn gamepad_button_from_str(name: &str) -> Option<GamepadButton> {
    GAMEPAD_BUTTON_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(button, _)| *button)
}

/// `button`'s `input.json` name.
#[must_use]
pub fn gamepad_button_to_str(button: GamepadButton) -> &'static str {
    GAMEPAD_BUTTON_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == button)
        .map(|(_, name)| *name)
        .expect("gamepad button missing canonical name")
}

/// The gamepad axis named `name` in `input.json`, such as `LeftStickX`.
#[must_use]
pub fn gamepad_axis_from_str(name: &str) -> Option<GamepadAxis> {
    GAMEPAD_AXIS_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(axis, _)| *axis)
}

/// `axis`'s `input.json` name.
#[must_use]
pub fn gamepad_axis_to_str(axis: GamepadAxis) -> &'static str {
    GAMEPAD_AXIS_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == axis)
        .map(|(_, name)| *name)
        .expect("gamepad axis missing canonical name")
}

/// The axis direction named `name` in `input.json`: `negative` or
/// `positive`.
#[must_use]
pub fn axis_direction_from_str(name: &str) -> Option<AxisDirection> {
    AXIS_DIRECTION_NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(direction, _)| *direction)
}

/// `direction`'s `input.json` name.
#[must_use]
pub fn axis_direction_to_str(direction: AxisDirection) -> &'static str {
    AXIS_DIRECTION_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == direction)
        .map(|(_, name)| *name)
        .expect("axis direction missing canonical name")
}

#[cfg(test)]
#[path = "../tests/input/key_serde.rs"]
mod tests;
