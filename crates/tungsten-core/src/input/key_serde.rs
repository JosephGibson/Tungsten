//! Canonical `input.json` names for keys, mouse buttons, scroll directions,
//! gamepad buttons, gamepad axes and axis directions, and the labels a
//! prompt shows for them (M56).

use std::borrow::Cow;

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

/// What a prompt shows for a named key: the physical key's US label
/// (winit 0.30 has no keymap query, so an AZERTY player sees US labels).
const KEYCODE_LABELS: &[(KeyCode, &str)] = &[
    (KeyCode::ArrowUp, "Up"),
    (KeyCode::ArrowDown, "Down"),
    (KeyCode::ArrowLeft, "Left"),
    (KeyCode::ArrowRight, "Right"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Escape, "Esc"),
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
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::Equal, "="),
    (KeyCode::Minus, "-"),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::PageUp, "Page Up"),
    (KeyCode::PageDown, "Page Down"),
    (KeyCode::ShiftLeft, "Left Shift"),
    (KeyCode::ShiftRight, "Right Shift"),
    (KeyCode::ControlLeft, "Left Ctrl"),
    (KeyCode::ControlRight, "Right Ctrl"),
    (KeyCode::AltLeft, "Left Alt"),
    (KeyCode::AltRight, "Right Alt"),
    (KeyCode::SuperLeft, "Left Super"),
    (KeyCode::SuperRight, "Right Super"),
];

/// Xbox-style pad labels (Q7 of M56): `A` for the south button, as SDL's
/// reference layout; a Nintendo pad reads swapped.
const GAMEPAD_BUTTON_LABELS: &[(GamepadButton, &str)] = &[
    (GamepadButton::South, "A"),
    (GamepadButton::East, "B"),
    (GamepadButton::North, "Y"),
    (GamepadButton::West, "X"),
    (GamepadButton::LeftShoulder, "LB"),
    (GamepadButton::RightShoulder, "RB"),
    (GamepadButton::LeftTrigger, "LT"),
    (GamepadButton::RightTrigger, "RT"),
    (GamepadButton::Select, "Select"),
    (GamepadButton::Start, "Start"),
    (GamepadButton::Mode, "Home"),
    (GamepadButton::LeftStick, "LS"),
    (GamepadButton::RightStick, "RS"),
    (GamepadButton::DPadUp, "D-Pad Up"),
    (GamepadButton::DPadDown, "D-Pad Down"),
    (GamepadButton::DPadLeft, "D-Pad Left"),
    (GamepadButton::DPadRight, "D-Pad Right"),
];

/// A key's prompt label: `W`, `Space`, `Esc`, `Left Shift`, `Page Up`; a
/// key with no variant reads `Key <n>`.
#[must_use]
pub fn key_label(code: KeyCode) -> Cow<'static, str> {
    match code {
        KeyCode::Other(n) => Cow::Owned(format!("Key {n}")),
        named => KEYCODE_LABELS
            .iter()
            .find(|(candidate, _)| *candidate == named)
            .map_or(Cow::Borrowed("Key"), |(_, label)| Cow::Borrowed(*label)),
    }
}

/// A mouse button's prompt label: `Left Mouse`, `Right Mouse`, `Middle
/// Mouse`, `Mouse <n>`.
#[must_use]
pub fn mouse_button_label(button: MouseButton) -> Cow<'static, str> {
    match button {
        MouseButton::Left => Cow::Borrowed("Left Mouse"),
        MouseButton::Right => Cow::Borrowed("Right Mouse"),
        MouseButton::Middle => Cow::Borrowed("Middle Mouse"),
        MouseButton::Other(n) => Cow::Owned(format!("Mouse {n}")),
    }
}

/// A wheel direction's prompt label: `Wheel Up` or `Wheel Down`.
#[must_use]
pub fn scroll_direction_label(direction: ScrollDirection) -> &'static str {
    match direction {
        ScrollDirection::Up => "Wheel Up",
        ScrollDirection::Down => "Wheel Down",
    }
}

/// A pad button's prompt label, Xbox-style: `A`, `LB`, `D-Pad Up`.
#[must_use]
pub fn gamepad_button_label(button: GamepadButton) -> &'static str {
    GAMEPAD_BUTTON_LABELS
        .iter()
        .find(|(candidate, _)| *candidate == button)
        .map(|(_, label)| *label)
        .expect("gamepad button missing label")
}

/// A stick direction's prompt label: `Left Stick Up`, `Right Stick Left`.
/// Negative is left on x and down on y.
#[must_use]
pub fn stick_label(axis: GamepadAxis, direction: AxisDirection) -> &'static str {
    use AxisDirection::{Negative, Positive};
    match (axis, direction) {
        (GamepadAxis::LeftStickX, Negative) => "Left Stick Left",
        (GamepadAxis::LeftStickX, Positive) => "Left Stick Right",
        (GamepadAxis::LeftStickY, Negative) => "Left Stick Down",
        (GamepadAxis::LeftStickY, Positive) => "Left Stick Up",
        (GamepadAxis::RightStickX, Negative) => "Right Stick Left",
        (GamepadAxis::RightStickX, Positive) => "Right Stick Right",
        (GamepadAxis::RightStickY, Negative) => "Right Stick Down",
        (GamepadAxis::RightStickY, Positive) => "Right Stick Up",
    }
}

#[cfg(test)]
#[path = "../tests/input/key_serde.rs"]
mod tests;
