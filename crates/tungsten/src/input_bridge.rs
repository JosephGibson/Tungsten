use tungsten_core::input::{InputState, KeyCode, Modifiers, MouseButton};
use tungsten_core::ui::CursorIcon;
use tungsten_core::{ScaleFactor, World};
use winit::event::ElementState;
use winit::keyboard::{KeyCode as WinitKeyCode, ModifiersState, PhysicalKey};

/// The winit code of each named `KeyCode`, whose `input.json` name is that
/// winit variant's name (`D-045`, `D-144`).
const KEYS: &[(WinitKeyCode, KeyCode)] = &[
    (WinitKeyCode::ArrowUp, KeyCode::ArrowUp),
    (WinitKeyCode::ArrowDown, KeyCode::ArrowDown),
    (WinitKeyCode::ArrowLeft, KeyCode::ArrowLeft),
    (WinitKeyCode::ArrowRight, KeyCode::ArrowRight),
    (WinitKeyCode::Space, KeyCode::Space),
    (WinitKeyCode::Enter, KeyCode::Enter),
    (WinitKeyCode::Escape, KeyCode::Escape),
    (WinitKeyCode::Backspace, KeyCode::Backspace),
    (WinitKeyCode::F1, KeyCode::F1),
    (WinitKeyCode::F2, KeyCode::F2),
    (WinitKeyCode::F3, KeyCode::F3),
    (WinitKeyCode::F4, KeyCode::F4),
    (WinitKeyCode::F5, KeyCode::F5),
    (WinitKeyCode::F6, KeyCode::F6),
    (WinitKeyCode::F7, KeyCode::F7),
    (WinitKeyCode::F8, KeyCode::F8),
    (WinitKeyCode::F9, KeyCode::F9),
    (WinitKeyCode::F10, KeyCode::F10),
    (WinitKeyCode::F11, KeyCode::F11),
    (WinitKeyCode::F12, KeyCode::F12),
    (WinitKeyCode::KeyW, KeyCode::KeyW),
    (WinitKeyCode::KeyA, KeyCode::KeyA),
    (WinitKeyCode::KeyS, KeyCode::KeyS),
    (WinitKeyCode::KeyD, KeyCode::KeyD),
    (WinitKeyCode::KeyF, KeyCode::KeyF),
    (WinitKeyCode::KeyB, KeyCode::KeyB),
    (WinitKeyCode::KeyC, KeyCode::KeyC),
    (WinitKeyCode::KeyE, KeyCode::KeyE),
    (WinitKeyCode::KeyG, KeyCode::KeyG),
    (WinitKeyCode::KeyH, KeyCode::KeyH),
    (WinitKeyCode::KeyI, KeyCode::KeyI),
    (WinitKeyCode::KeyJ, KeyCode::KeyJ),
    (WinitKeyCode::KeyK, KeyCode::KeyK),
    (WinitKeyCode::KeyL, KeyCode::KeyL),
    (WinitKeyCode::KeyM, KeyCode::KeyM),
    (WinitKeyCode::KeyN, KeyCode::KeyN),
    (WinitKeyCode::KeyO, KeyCode::KeyO),
    (WinitKeyCode::KeyP, KeyCode::KeyP),
    (WinitKeyCode::KeyQ, KeyCode::KeyQ),
    (WinitKeyCode::KeyR, KeyCode::KeyR),
    (WinitKeyCode::KeyT, KeyCode::KeyT),
    (WinitKeyCode::KeyU, KeyCode::KeyU),
    (WinitKeyCode::KeyV, KeyCode::KeyV),
    (WinitKeyCode::KeyX, KeyCode::KeyX),
    (WinitKeyCode::KeyY, KeyCode::KeyY),
    (WinitKeyCode::KeyZ, KeyCode::KeyZ),
    (WinitKeyCode::Tab, KeyCode::Tab),
    (WinitKeyCode::Digit0, KeyCode::Digit0),
    (WinitKeyCode::Digit1, KeyCode::Digit1),
    (WinitKeyCode::Digit2, KeyCode::Digit2),
    (WinitKeyCode::Digit3, KeyCode::Digit3),
    (WinitKeyCode::Digit4, KeyCode::Digit4),
    (WinitKeyCode::Digit5, KeyCode::Digit5),
    (WinitKeyCode::Digit6, KeyCode::Digit6),
    (WinitKeyCode::Digit7, KeyCode::Digit7),
    (WinitKeyCode::Digit8, KeyCode::Digit8),
    (WinitKeyCode::Digit9, KeyCode::Digit9),
    (WinitKeyCode::Equal, KeyCode::Equal),
    (WinitKeyCode::Minus, KeyCode::Minus),
    (WinitKeyCode::BracketLeft, KeyCode::BracketLeft),
    (WinitKeyCode::BracketRight, KeyCode::BracketRight),
    (WinitKeyCode::Home, KeyCode::Home),
    (WinitKeyCode::End, KeyCode::End),
    (WinitKeyCode::Delete, KeyCode::Delete),
    (WinitKeyCode::PageUp, KeyCode::PageUp),
    (WinitKeyCode::PageDown, KeyCode::PageDown),
    (WinitKeyCode::ShiftLeft, KeyCode::ShiftLeft),
    (WinitKeyCode::ShiftRight, KeyCode::ShiftRight),
    (WinitKeyCode::ControlLeft, KeyCode::ControlLeft),
    (WinitKeyCode::ControlRight, KeyCode::ControlRight),
    (WinitKeyCode::AltLeft, KeyCode::AltLeft),
    (WinitKeyCode::AltRight, KeyCode::AltRight),
    (WinitKeyCode::SuperLeft, KeyCode::SuperLeft),
    (WinitKeyCode::SuperRight, KeyCode::SuperRight),
];

pub fn translate_key(key: PhysicalKey) -> KeyCode {
    match key {
        PhysicalKey::Code(code) => KEYS
            .iter()
            .find(|(winit, _)| *winit == code)
            .map_or(KeyCode::Other(code as u32), |&(_, key)| key),
        PhysicalKey::Unidentified(_) => KeyCode::Other(0),
    }
}

pub fn translate_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Other(4),
        winit::event::MouseButton::Forward => MouseButton::Other(5),
        winit::event::MouseButton::Other(id) => MouseButton::Other(id),
    }
}

pub fn translate_modifiers(state: ModifiersState) -> Modifiers {
    let mut held = Modifiers::empty();
    for (winit, modifier) in [
        (ModifiersState::SHIFT, Modifiers::SHIFT),
        (ModifiersState::CONTROL, Modifiers::CONTROL),
        (ModifiersState::ALT, Modifiers::ALT),
        (ModifiersState::SUPER, Modifiers::SUPER),
    ] {
        if state.contains(winit) {
            held = held.union(modifier);
        }
    }
    held
}

/// A key event, from its parts, since a test cannot build winit's
/// `KeyEvent`. A synthetic release is dropped. A synthetic press, which winit
/// sends for each key still held when focus returns, marks the key so it
/// stays released until it is pressed again, X11's auto-repeat included. A
/// press flagged `repeat` of a key not held is dropped: the engine never saw
/// its first press (`D-144`).
pub fn apply_key(
    world: &mut World,
    key: PhysicalKey,
    state: ElementState,
    repeat: bool,
    is_synthetic: bool,
) {
    let Some(input) = world.get_resource_mut::<InputState>() else {
        return;
    };
    let code = translate_key(key);
    match state {
        ElementState::Released if !is_synthetic => input.key_up(code),
        ElementState::Released => {}
        ElementState::Pressed if is_synthetic => input.suppress_until_release(code),
        ElementState::Pressed if repeat && !input.is_pressed(code) => {}
        ElementState::Pressed => input.key_down(code),
    }
}

/// `Focused`: a focus loss releases held keys and buttons.
pub fn apply_focus(world: &mut World, focused: bool) {
    if let Some(input) = world.get_resource_mut::<InputState>() {
        input.set_focused(focused);
    }
}

/// `ModifiersChanged`.
pub fn apply_modifiers(world: &mut World, state: ModifiersState) {
    if let Some(input) = world.get_resource_mut::<InputState>() {
        input.set_modifiers(translate_modifiers(state));
    }
}

/// `CursorLeft`.
pub fn apply_pointer_left(world: &mut World) {
    if let Some(input) = world.get_resource_mut::<InputState>() {
        input.pointer_left();
    }
}

/// The window's cursor for a UI [`CursorIcon`] (`D-149`); one added to
/// core before this mapping shows the default arrow.
pub fn cursor_icon(icon: CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as Window;
    match icon {
        CursorIcon::Pointer => Window::Pointer,
        CursorIcon::Text => Window::Text,
        CursorIcon::Move => Window::Move,
        CursorIcon::NotAllowed => Window::NotAllowed,
        CursorIcon::EwResize => Window::EwResize,
        CursorIcon::NsResize => Window::NsResize,
        CursorIcon::NwseResize => Window::NwseResize,
        CursorIcon::NeswResize => Window::NeswResize,
        _ => Window::Default,
    }
}

/// The window's scale factor, at startup and on `ScaleFactorChanged`.
pub fn apply_scale_factor(world: &mut World, scale_factor: f64) {
    if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
        scale.set_os(scale_factor as f32);
    }
}

#[cfg(test)]
#[path = "tests/input_bridge.rs"]
mod tests;
