use tungsten_core::input::{KeyCode, MouseButton};
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

pub fn translate_key(key: PhysicalKey) -> KeyCode {
    match key {
        PhysicalKey::Code(code) => match code {
            WinitKeyCode::ArrowUp => KeyCode::ArrowUp,
            WinitKeyCode::ArrowDown => KeyCode::ArrowDown,
            WinitKeyCode::ArrowLeft => KeyCode::ArrowLeft,
            WinitKeyCode::ArrowRight => KeyCode::ArrowRight,
            WinitKeyCode::Space => KeyCode::Space,
            WinitKeyCode::Enter => KeyCode::Enter,
            WinitKeyCode::Escape => KeyCode::Escape,
            WinitKeyCode::F9 => KeyCode::F9,
            WinitKeyCode::F11 => KeyCode::F11,
            WinitKeyCode::KeyW => KeyCode::KeyW,
            WinitKeyCode::KeyA => KeyCode::KeyA,
            WinitKeyCode::KeyS => KeyCode::KeyS,
            WinitKeyCode::KeyD => KeyCode::KeyD,
            WinitKeyCode::KeyM => KeyCode::KeyM,
            WinitKeyCode::KeyV => KeyCode::KeyV,
            WinitKeyCode::Digit1 => KeyCode::Digit1,
            WinitKeyCode::Digit2 => KeyCode::Digit2,
            WinitKeyCode::Digit3 => KeyCode::Digit3,
            WinitKeyCode::Equal => KeyCode::Equal,
            WinitKeyCode::Minus => KeyCode::Minus,
            other => KeyCode::Other(other as u32),
        },
        PhysicalKey::Unidentified(_) => KeyCode::Other(0),
    }
}

pub fn translate_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Other(id) => MouseButton::Other(id),
        _ => MouseButton::Other(0),
    }
}
