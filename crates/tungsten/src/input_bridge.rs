#[cfg(any(feature = "gamepad", test))]
use tungsten_core::input::{GamepadAxis, GamepadButton, GamepadId};
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

/// A gamepad event in the engine's terms, which the backend translates
/// `gilrs`'s into (`D-220`), so applying it needs no device.
#[cfg(any(feature = "gamepad", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PadEvent {
    /// The pad connected.
    Connected,
    /// The pad disconnected.
    Disconnected,
    /// A button press or release.
    Button {
        button: GamepadButton,
        pressed: bool,
    },
    /// A stick axis moved.
    Axis { axis: GamepadAxis, value: f32 },
}

/// Applies one pad event to `input` (`D-220`): a connection or
/// disconnection always, a button or axis event only while the window has
/// the focus, so an unfocused game reads no pad.
#[cfg(any(feature = "gamepad", test))]
pub(crate) fn apply_pad_event(input: &mut InputState, pad: GamepadId, event: PadEvent) {
    match event {
        PadEvent::Connected => input.gamepad_connected(pad),
        PadEvent::Disconnected => input.gamepad_disconnected(pad),
        _ if !input.focused() => {}
        PadEvent::Button {
            button,
            pressed: true,
        } => input.gamepad_button_down(pad, button),
        PadEvent::Button {
            button,
            pressed: false,
        } => input.gamepad_button_up(pad, button),
        PadEvent::Axis { axis, value } => input.set_gamepad_axis(pad, axis, value),
    }
}

/// Applies the frame's pad events in order; smoke mode drops every one, so
/// captures never read a pad.
#[cfg(any(feature = "gamepad", test))]
pub(crate) fn apply_pad_events(
    input: &mut InputState,
    events: impl IntoIterator<Item = (GamepadId, PadEvent)>,
    smoke: bool,
) {
    for (pad, event) in events {
        if !smoke {
            apply_pad_event(input, pad, event);
        }
    }
}

/// The engine's button for a `gilrs` one, by position; `None` for one the
/// engine does not name (`C`, `Z`, unknown). gilrs's first triggers are the
/// shoulders and its second the triggers.
#[cfg(feature = "gamepad")]
pub(crate) fn translate_pad_button(button: gilrs::Button) -> Option<GamepadButton> {
    use gilrs::Button as B;
    Some(match button {
        B::South => GamepadButton::South,
        B::East => GamepadButton::East,
        B::North => GamepadButton::North,
        B::West => GamepadButton::West,
        B::LeftTrigger => GamepadButton::LeftShoulder,
        B::RightTrigger => GamepadButton::RightShoulder,
        B::LeftTrigger2 => GamepadButton::LeftTrigger,
        B::RightTrigger2 => GamepadButton::RightTrigger,
        B::Select => GamepadButton::Select,
        B::Start => GamepadButton::Start,
        B::Mode => GamepadButton::Mode,
        B::LeftThumb => GamepadButton::LeftStick,
        B::RightThumb => GamepadButton::RightStick,
        B::DPadUp => GamepadButton::DPadUp,
        B::DPadDown => GamepadButton::DPadDown,
        B::DPadLeft => GamepadButton::DPadLeft,
        B::DPadRight => GamepadButton::DPadRight,
        B::C | B::Z | B::Unknown => return None,
    })
}

/// The engine's axis for a `gilrs` stick axis; `None` for the trigger axes,
/// the d-pad axes (gilrs's default filters turn those into buttons) and
/// unknown ones.
#[cfg(feature = "gamepad")]
pub(crate) fn translate_pad_axis(axis: gilrs::Axis) -> Option<GamepadAxis> {
    use gilrs::Axis as A;
    match axis {
        A::LeftStickX => Some(GamepadAxis::LeftStickX),
        A::LeftStickY => Some(GamepadAxis::LeftStickY),
        A::RightStickX => Some(GamepadAxis::RightStickX),
        A::RightStickY => Some(GamepadAxis::RightStickY),
        A::LeftZ | A::RightZ | A::DPadX | A::DPadY | A::Unknown => None,
    }
}

/// A `gilrs` event in the engine's terms; `None` for one the engine skips:
/// an unmapped button or axis, an analog button value, a repeat, a dropped
/// event or a force feedback one.
#[cfg(feature = "gamepad")]
pub(crate) fn translate_pad_event(event: gilrs::EventType) -> Option<PadEvent> {
    use gilrs::EventType as E;
    match event {
        E::ButtonPressed(button, _) => Some(PadEvent::Button {
            button: translate_pad_button(button)?,
            pressed: true,
        }),
        E::ButtonReleased(button, _) => Some(PadEvent::Button {
            button: translate_pad_button(button)?,
            pressed: false,
        }),
        E::AxisChanged(axis, value, _) => Some(PadEvent::Axis {
            axis: translate_pad_axis(axis)?,
            value,
        }),
        E::Connected => Some(PadEvent::Connected),
        E::Disconnected => Some(PadEvent::Disconnected),
        _ => None,
    }
}

/// The engine's ID for a `gilrs` pad.
#[cfg(feature = "gamepad")]
fn pad_id(id: gilrs::GamepadId) -> GamepadId {
    GamepadId::new(usize::from(id) as u32)
}

/// The gamepad backend (`D-220`): `gilrs` with force feedback off, whose
/// one backend thread watches for hotplug; its events reach the main thread
/// through `gilrs`'s own channel, and `run_frame` drains them first.
#[cfg(feature = "gamepad")]
pub(crate) struct GamepadBackend {
    gilrs: gilrs::Gilrs,
}

#[cfg(feature = "gamepad")]
impl GamepadBackend {
    /// Builds `gilrs` with force feedback off, whose server would spawn a
    /// second thread, and reports the pads connected now, except in smoke
    /// mode. A failed build logs one warning and gives `None`: the game runs
    /// without pads.
    ///
    /// gilrs's default filters are off, so its per-axis dead zone does not
    /// stack on core's radial one (`StickResponse`, P7-12); [`drain`](Self::drain)
    /// runs the other two, and so updates gilrs's state itself.
    pub(crate) fn start(input: &mut InputState, smoke: bool) -> Option<Self> {
        let gilrs = match gilrs::GilrsBuilder::new()
            .with_force_feedback(false)
            .with_default_filters(false)
            .set_update_state(false)
            .build()
        {
            Ok(gilrs) => gilrs,
            Err(error) => {
                log::warn!("gamepad backend failed to start: {error}; running without gamepads");
                return None;
            }
        };
        let connected: Vec<GamepadId> = gilrs.gamepads().map(|(id, _)| pad_id(id)).collect();
        log::info!("gamepad backend started: {} connected", connected.len());
        apply_pad_events(
            input,
            connected.into_iter().map(|pad| (pad, PadEvent::Connected)),
            smoke,
        );
        Some(Self { gilrs })
    }

    /// Applies every pending event in arrival order (`apply_pad_events`),
    /// each through gilrs's d-pad filter (its axes to buttons) and jitter
    /// filter, then into gilrs's state, as its default filters do without
    /// the dead zone; a filtered event comes back `Dropped`, which
    /// [`translate_pad_event`] skips.
    pub(crate) fn drain(&mut self, input: &mut InputState, smoke: bool) {
        use gilrs::Filter;
        use gilrs::ev::filter::{Jitter, axis_dpad_to_button};

        let gilrs = &mut self.gilrs;
        let jitter = Jitter::new();
        let events = std::iter::from_fn(|| {
            let event = gilrs
                .next_event()
                .filter_ev(&axis_dpad_to_button, gilrs)
                .filter_ev(&jitter, gilrs)?;
            gilrs.update(&event);
            Some(event)
        })
        .filter_map(|event| {
            translate_pad_event(event.event).map(|translated| (pad_id(event.id), translated))
        });
        apply_pad_events(input, events, smoke);
    }
}

#[cfg(test)]
#[path = "tests/input_bridge.rs"]
mod tests;
