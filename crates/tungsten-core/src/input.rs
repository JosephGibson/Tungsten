use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser};

pub mod action_map;
pub mod key_serde;

pub use action_map::{ActionMap, ActionMapError, Binding};

/// Winit-like key codes without core depending on winit.
///
/// A named variant's `input.json` name is its winit variant name (`D-045`,
/// `D-144`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Space,
    Enter,
    Escape,
    Backspace,
    F1,
    F2,
    F3,
    F4,
    /// The F5 key.
    F5,
    /// The F6 key.
    F6,
    /// The F7 key.
    F7,
    /// The F8 key.
    F8,
    F9,
    /// The F10 key.
    F10,
    F11,
    /// The F12 key.
    F12,
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    KeyF,
    KeyB,
    KeyC,
    /// The E key.
    KeyE,
    /// The G key.
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    /// The O key.
    KeyO,
    KeyP,
    /// The Q key.
    KeyQ,
    KeyR,
    /// The T key.
    KeyT,
    KeyU,
    KeyV,
    /// The X key.
    KeyX,
    KeyY,
    /// The Z key.
    KeyZ,
    Tab,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    /// The 4 key above the letters.
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    /// The 9 key above the letters.
    Digit9,
    Equal,
    Minus,
    BracketLeft,
    BracketRight,
    /// The Home key.
    Home,
    /// The End key.
    End,
    /// The Delete key, which deletes forward.
    Delete,
    /// The Page Up key.
    PageUp,
    /// The Page Down key.
    PageDown,
    /// The left Shift key.
    ShiftLeft,
    /// The right Shift key.
    ShiftRight,
    /// The left Control key.
    ControlLeft,
    /// The right Control key.
    ControlRight,
    /// The left Alt key (Option on macOS).
    AltLeft,
    /// The right Alt key (Option on macOS, AltGr on some layouts).
    AltRight,
    /// The left Super key: the Windows key, or Command on macOS.
    SuperLeft,
    /// The right Super key: the Windows key, or Command on macOS.
    SuperRight,
    /// A key with no variant. Its number is stable within one build only,
    /// and it never serializes.
    Other(u32),
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Discrete scroll direction for action bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// The modifier keys held: a set of [`SHIFT`](Self::SHIFT),
/// [`CONTROL`](Self::CONTROL), [`ALT`](Self::ALT) and [`SUPER`](Self::SUPER),
/// each for either side's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    bits: u8,
}

impl Modifiers {
    /// Either Shift key.
    pub const SHIFT: Self = Self { bits: 1 };
    /// Either Control key.
    pub const CONTROL: Self = Self { bits: 1 << 1 };
    /// Either Alt key (Option on macOS).
    pub const ALT: Self = Self { bits: 1 << 2 };
    /// Either Super key: the Windows key, or Command on macOS.
    pub const SUPER: Self = Self { bits: 1 << 3 };

    /// The set with no modifier in it.
    #[must_use]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    /// The modifiers in either set.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// Whether every modifier in `other` is in this set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.bits & other.bits == other.bits
    }

    /// Whether Shift is held.
    #[must_use]
    pub const fn shift(self) -> bool {
        self.contains(Self::SHIFT)
    }

    /// Whether Control is held.
    #[must_use]
    pub const fn control(self) -> bool {
        self.contains(Self::CONTROL)
    }

    /// Whether Alt is held.
    #[must_use]
    pub const fn alt(self) -> bool {
        self.contains(Self::ALT)
    }

    /// Whether Super is held (`super` is a keyword).
    #[must_use]
    pub const fn super_key(self) -> bool {
        self.contains(Self::SUPER)
    }
}

/// A gamepad, as the backend numbers it: stable while it stays connected
/// (M54).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GamepadId(u32);

impl GamepadId {
    /// The gamepad numbered `id`.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A gamepad button by its position, as the standard layout names it, so one
/// `input.json` serves every pad family (Q10 of M54). Its `input.json` name
/// is the variant name.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    /// The bottom face button: A on Xbox, Cross on PlayStation.
    South,
    /// The right face button: B on Xbox, Circle on PlayStation.
    East,
    /// The top face button: Y on Xbox, Triangle on PlayStation.
    North,
    /// The left face button: X on Xbox, Square on PlayStation.
    West,
    /// The left shoulder button (LB, L1).
    LeftShoulder,
    /// The right shoulder button (RB, R1).
    RightShoulder,
    /// The left trigger (LT, L2), as a button.
    LeftTrigger,
    /// The right trigger (RT, R2), as a button.
    RightTrigger,
    /// The left centre button: View, Back or Share.
    Select,
    /// The right centre button: Menu, Start or Options.
    Start,
    /// The logo button: Guide or PS.
    Mode,
    /// The left stick pressed in.
    LeftStick,
    /// The right stick pressed in.
    RightStick,
    /// The d-pad's up.
    DPadUp,
    /// The d-pad's down.
    DPadDown,
    /// The d-pad's left.
    DPadLeft,
    /// The d-pad's right.
    DPadRight,
}

/// A stick axis, from −1 to 1: x grows to the right and y upward. Its
/// `input.json` name is the variant name.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    /// The left stick's x.
    LeftStickX,
    /// The left stick's y.
    LeftStickY,
    /// The right stick's x.
    RightStickX,
    /// The right stick's y.
    RightStickY,
}

impl GamepadAxis {
    /// The axis's slot in a pad's values.
    fn index(self) -> usize {
        match self {
            Self::LeftStickX => 0,
            Self::LeftStickY => 1,
            Self::RightStickX => 2,
            Self::RightStickY => 3,
        }
    }
}

/// One direction of a stick axis; `input.json` names them `negative` and
/// `positive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisDirection {
    /// Toward −1: left on x, down on y.
    Negative,
    /// Toward 1: right on x, up on y.
    Positive,
}

impl AxisDirection {
    const BOTH: [Self; 2] = [Self::Negative, Self::Positive];

    /// How far `value` goes this way: `value` for `Positive`, `-value` for
    /// `Negative`.
    #[must_use]
    pub fn along(self, value: f32) -> f32 {
        match self {
            Self::Negative => -value,
            Self::Positive => value,
        }
    }
}

/// A stick direction presses when the value along it reaches this, after
/// the backend's deadzone (Q8 of M54).
pub const STICK_PRESS: f32 = 0.5;
/// A pressed stick direction releases when the value along it falls below
/// this. [`ActionMap::value`] reads an axis binding from here: 0 at it, 1 at
/// full tilt.
pub const STICK_RELEASE: f32 = 0.35;

/// A scroll amount as the window reports it: lines from a notched wheel or
/// pixels from a touchpad. A positive `y` scrolls up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelDelta {
    /// Columns and lines.
    Lines(f32, f32),
    /// Physical pixels.
    Pixels(f32, f32),
}

/// One input event as the window delivered it, before any routing: what
/// [`InputState::raw_events`] records (`D-144`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum RawInput {
    /// A key press or release.
    #[non_exhaustive]
    Key {
        /// The key.
        code: KeyCode,
        /// Pressed, or released.
        pressed: bool,
        /// A press of a key already held, such as the OS's auto-repeat; it
        /// changes no state.
        repeat: bool,
        /// The modifiers held when it arrived.
        modifiers: Modifiers,
    },
    /// A mouse button press or release.
    #[non_exhaustive]
    MouseButton {
        /// The button.
        button: MouseButton,
        /// Pressed, or released.
        pressed: bool,
    },
    /// The pointer moved, to physical pixels from the window's top-left
    /// corner. Adjacent moves coalesce into the first one's record.
    #[non_exhaustive]
    PointerMoved {
        /// The new x position.
        x: f32,
        /// The new y position.
        y: f32,
    },
    /// The pointer left the window.
    PointerLeft,
    /// The wheel or touchpad scrolled.
    #[non_exhaustive]
    Wheel {
        /// How far.
        delta: WheelDelta,
    },
    /// The modifiers held changed to this set.
    Modifiers(Modifiers),
    /// The window gained (`true`) or lost (`false`) the keyboard focus.
    Focus(bool),
    /// A gamepad button press or release.
    #[non_exhaustive]
    GamepadButton {
        /// The pad.
        pad: GamepadId,
        /// The button.
        button: GamepadButton,
        /// Pressed, or released.
        pressed: bool,
    },
    /// A stick axis moved to `value`. Adjacent moves of one pad's axis
    /// coalesce into the first one's record unless either crossed a stick
    /// threshold: a crossing's record keeps its value, so a release or press
    /// always reaches a reader that derives them from the values.
    #[non_exhaustive]
    GamepadAxis {
        /// The pad.
        pad: GamepadId,
        /// The axis.
        axis: GamepadAxis,
        /// The new value, −1 to 1.
        value: f32,
    },
    /// A stick direction crossed [`STICK_PRESS`] (pressed) or
    /// [`STICK_RELEASE`] (released). It comes right before the
    /// [`GamepadAxis`](Self::GamepadAxis) record of the move that crossed.
    #[non_exhaustive]
    GamepadStick {
        /// The pad.
        pad: GamepadId,
        /// The axis.
        axis: GamepadAxis,
        /// The direction along it.
        direction: AxisDirection,
        /// Pressed, or released.
        pressed: bool,
    },
    /// A gamepad connected (`true`) or disconnected (`false`).
    #[non_exhaustive]
    Gamepad {
        /// The pad.
        pad: GamepadId,
        /// Connected, or disconnected.
        connected: bool,
    },
}

/// A [`RawInput`] and its place in arrival order.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct RawEvent {
    /// Counts up across frames for the life of the [`InputState`].
    pub seq: u64,
    /// The event.
    pub input: RawInput,
}

impl Serialize for KeyCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match key_serde::keycode_to_str(*self) {
            Some(name) => serializer.serialize_str(name),
            None => Err(ser::Error::custom(format!(
                "KeyCode variant {self:?} has no canonical string name"
            ))),
        }
    }
}

impl<'de> Deserialize<'de> for KeyCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        key_serde::keycode_from_str(&raw)
            .ok_or_else(|| de::Error::custom(format!("unknown key name '{raw}'")))
    }
}

impl Serialize for MouseButton {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&key_serde::mouse_button_to_string(*self))
    }
}

impl<'de> Deserialize<'de> for MouseButton {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        key_serde::mouse_button_from_str(&raw)
            .ok_or_else(|| de::Error::custom(format!("unknown mouse button name '{raw}'")))
    }
}

impl Serialize for GamepadButton {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(key_serde::gamepad_button_to_str(*self))
    }
}

impl<'de> Deserialize<'de> for GamepadButton {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        key_serde::gamepad_button_from_str(&raw)
            .ok_or_else(|| de::Error::custom(format!("unknown gamepad button '{raw}'")))
    }
}

impl Serialize for GamepadAxis {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(key_serde::gamepad_axis_to_str(*self))
    }
}

impl<'de> Deserialize<'de> for GamepadAxis {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        key_serde::gamepad_axis_from_str(&raw)
            .ok_or_else(|| de::Error::custom(format!("unknown gamepad axis '{raw}'")))
    }
}

impl Serialize for AxisDirection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(key_serde::axis_direction_to_str(*self))
    }
}

impl<'de> Deserialize<'de> for AxisDirection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        key_serde::axis_direction_from_str(&raw)
            .ok_or_else(|| de::Error::custom(format!("unknown axis direction '{raw}'")))
    }
}

impl Serialize for ScrollDirection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(key_serde::scroll_direction_to_str(*self))
    }
}

impl<'de> Deserialize<'de> for ScrollDirection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        key_serde::scroll_direction_from_str(&raw)
            .ok_or_else(|| de::Error::custom(format!("unknown scroll direction '{raw}'")))
    }
}

/// Keyboard, mouse, scroll and gamepad state with per-frame edges, and the
/// frame's raw input in arrival order.
///
/// Edges have two views. The frame view, the default, holds the presses and
/// releases since the last [`begin_frame`](Self::begin_frame). The fixed
/// view, which the app turns on while `fixed_update` runs
/// ([`set_fixed_view`](Self::set_fixed_view)), holds them since the last
/// [`end_fixed_step`](Self::end_fixed_step): a frame that runs no fixed step
/// keeps its edges for the next step, and a frame that runs two shows each
/// edge to the first. So `just_pressed` sees each press once in either stage.
/// A frame with no step whose game clock stood still while real time ran (a
/// pause, or a scale of 0) drops them instead, so a press made on a stopped
/// clock reaches no step (`D-146`).
/// Level queries, the cursor, the deltas and
/// [`raw_events`](Self::raw_events) stay per frame.
///
/// Gamepads (M54): each connected pad keeps its buttons, its stick
/// directions and its axes. The plain queries read any pad, which is what
/// [`ActionMap`] does; the `_on` queries read one. A stick direction presses
/// when the value along it reaches [`STICK_PRESS`] and releases below
/// [`STICK_RELEASE`]. Input from a pad not yet connected connects it.
///
/// Each mutator applies its state and records its [`RawEvent`], so input fed
/// through these methods, the window's or a test's, reaches the stream as
/// given (`D-144`).
#[derive(Debug, Clone)]
pub struct InputState {
    pressed: HashSet<KeyCode>,
    just_pressed: HashSet<KeyCode>,
    just_released: HashSet<KeyCode>,

    mouse_pressed: HashSet<MouseButton>,
    mouse_just_pressed: HashSet<MouseButton>,
    mouse_just_released: HashSet<MouseButton>,

    scroll_pressed: HashSet<ScrollDirection>,
    scroll_just_pressed: HashSet<ScrollDirection>,
    scroll_just_released: HashSet<ScrollDirection>,

    /// Connected pads, by ID.
    pads: Vec<PadState>,
    pad_edges: PadEdges,
    /// The last record is a move of this pad's axis that crossed nothing, so
    /// the next such move may coalesce into it.
    open_axis: Option<(GamepadId, GamepadAxis)>,

    fixed: FixedEdges,
    fixed_view: bool,

    cursor_position: Option<(f32, f32)>,
    cursor_delta: (f32, f32),
    scroll_line_delta: (f32, f32),
    scroll_pixel_delta: (f32, f32),

    focused: bool,
    modifiers: Modifiers,
    pointer_in_window: bool,
    /// Keys whose presses are ignored until their release.
    suppressed: HashSet<KeyCode>,
    raw_events: Vec<RawEvent>,
    next_seq: u64,
}

/// The fixed view's edges: every press and release since the last fixed
/// step ended.
#[derive(Debug, Clone, Default)]
struct FixedEdges {
    just_pressed: HashSet<KeyCode>,
    just_released: HashSet<KeyCode>,
    mouse_just_pressed: HashSet<MouseButton>,
    mouse_just_released: HashSet<MouseButton>,
    scroll_just_pressed: HashSet<ScrollDirection>,
    scroll_just_released: HashSet<ScrollDirection>,
    pad: PadEdges,
}

/// One connected pad: held buttons and stick directions, and its axes by
/// `GamepadAxis::index`.
#[derive(Debug, Clone)]
struct PadState {
    id: GamepadId,
    buttons: Vec<GamepadButton>,
    sticks: Vec<(GamepadAxis, AxisDirection)>,
    axes: [f32; 4],
}

/// One view's gamepad edges, each with its pad.
#[derive(Debug, Clone, Default)]
struct PadEdges {
    pressed: Vec<(GamepadId, GamepadButton)>,
    released: Vec<(GamepadId, GamepadButton)>,
    stick_pressed: Vec<(GamepadId, GamepadAxis, AxisDirection)>,
    stick_released: Vec<(GamepadId, GamepadAxis, AxisDirection)>,
}

impl PadEdges {
    fn clear(&mut self) {
        self.pressed.clear();
        self.released.clear();
        self.stick_pressed.clear();
        self.stick_released.clear();
    }
}

impl InputState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
            mouse_pressed: HashSet::new(),
            mouse_just_pressed: HashSet::new(),
            mouse_just_released: HashSet::new(),
            scroll_pressed: HashSet::new(),
            scroll_just_pressed: HashSet::new(),
            scroll_just_released: HashSet::new(),
            pads: Vec::new(),
            pad_edges: PadEdges::default(),
            open_axis: None,
            fixed: FixedEdges::default(),
            fixed_view: false,
            cursor_position: None,
            cursor_delta: (0.0, 0.0),
            scroll_line_delta: (0.0, 0.0),
            scroll_pixel_delta: (0.0, 0.0),
            focused: true,
            modifiers: Modifiers::empty(),
            pointer_in_window: false,
            suppressed: HashSet::new(),
            raw_events: Vec::new(),
            next_seq: 0,
        }
    }

    /// Clear per-frame edges, deltas and raw events. The fixed view keeps its
    /// edges until a fixed step ends; a scroll notch's release enters both
    /// views.
    pub fn begin_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_just_pressed.clear();
        self.mouse_just_released.clear();

        self.scroll_just_pressed.clear();
        self.scroll_just_released.clear();
        for direction in self.scroll_pressed.drain() {
            self.scroll_just_released.insert(direction);
            self.fixed.scroll_just_released.insert(direction);
        }

        self.pad_edges.clear();

        self.cursor_delta = (0.0, 0.0);
        self.scroll_line_delta = (0.0, 0.0);
        self.scroll_pixel_delta = (0.0, 0.0);
        self.raw_events.clear();
        self.open_axis = None;
    }

    /// Turns the fixed view on or off: while it is on, `just_pressed`,
    /// `just_released` and the mouse and scroll edge queries answer with the
    /// edges since the last [`end_fixed_step`](Self::end_fixed_step), and
    /// [`ActionMap`] with them. The app turns it on around each fixed step.
    pub fn set_fixed_view(&mut self, on: bool) {
        self.fixed_view = on;
    }

    /// Clears the fixed view's edges: the app calls it after each fixed
    /// step, so the next step sees only what arrived since, and in a frame
    /// with no step while the game clock stands still.
    pub fn end_fixed_step(&mut self) {
        let fixed = &mut self.fixed;
        fixed.just_pressed.clear();
        fixed.just_released.clear();
        fixed.mouse_just_pressed.clear();
        fixed.mouse_just_released.clear();
        fixed.scroll_just_pressed.clear();
        fixed.scroll_just_released.clear();
        fixed.pad.clear();
    }

    /// A press: holds `key` and records it; a press of a held key records a
    /// repeat and changes nothing. A key marked by
    /// [`suppress_until_release`](Self::suppress_until_release) ignores it.
    pub fn key_down(&mut self, key: KeyCode) {
        if self.suppressed.contains(&key) {
            return;
        }
        let repeat = !self.pressed.insert(key);
        if !repeat {
            self.just_pressed.insert(key);
            self.fixed.just_pressed.insert(key);
        }
        self.record(RawInput::Key {
            code: key,
            pressed: true,
            repeat,
            modifiers: self.modifiers,
        });
    }

    /// A release: releases a held `key` and records it. A release of a key
    /// not held records nothing, and one of a suppressed key clears its mark.
    pub fn key_up(&mut self, key: KeyCode) {
        if self.suppressed.remove(&key) {
            return;
        }
        if self.pressed.remove(&key) {
            self.just_released.insert(key);
            self.fixed.just_released.insert(key);
            self.record(RawInput::Key {
                code: key,
                pressed: false,
                repeat: false,
                modifiers: self.modifiers,
            });
        }
    }

    pub fn mouse_down(&mut self, button: MouseButton) {
        if self.mouse_pressed.insert(button) {
            self.mouse_just_pressed.insert(button);
            self.fixed.mouse_just_pressed.insert(button);
            self.record(RawInput::MouseButton {
                button,
                pressed: true,
            });
        }
    }

    pub fn mouse_up(&mut self, button: MouseButton) {
        if self.mouse_pressed.remove(&button) {
            self.mouse_just_released.insert(button);
            self.fixed.mouse_just_released.insert(button);
            self.record(RawInput::MouseButton {
                button,
                pressed: false,
            });
        }
    }

    /// The pointer moved to `x`, `y`. The first move after
    /// [`pointer_left`](Self::pointer_left) sets the position without a
    /// delta; a move right after a move updates that one's record.
    pub fn update_cursor_position(&mut self, x: f32, y: f32) {
        if self.pointer_in_window
            && let Some((prev_x, prev_y)) = self.cursor_position
        {
            self.cursor_delta.0 += x - prev_x;
            self.cursor_delta.1 += y - prev_y;
        }
        self.cursor_position = Some((x, y));
        self.pointer_in_window = true;
        if let Some(RawInput::PointerMoved {
            x: last_x,
            y: last_y,
        }) = self.raw_events.last_mut().map(|event| &mut event.input)
        {
            *last_x = x;
            *last_y = y;
        } else {
            self.record(RawInput::PointerMoved { x, y });
        }
    }

    pub fn add_scroll_line_delta(&mut self, x: f32, y: f32) {
        self.scroll_line_delta.0 += x;
        self.scroll_line_delta.1 += y;
        self.register_scroll_direction(y);
        self.record(RawInput::Wheel {
            delta: WheelDelta::Lines(x, y),
        });
    }

    pub fn add_scroll_pixel_delta(&mut self, x: f32, y: f32) {
        self.scroll_pixel_delta.0 += x;
        self.scroll_pixel_delta.1 += y;
        self.register_scroll_direction(y);
        self.record(RawInput::Wheel {
            delta: WheelDelta::Pixels(x, y),
        });
    }

    /// Sets the keyboard focus and records it. Losing it releases every held
    /// key, mouse button, gamepad button and stick direction, with release
    /// edges in both views and no recorded releases, zeroes every gamepad
    /// axis, and clears the modifiers and every
    /// [`suppress_until_release`](Self::suppress_until_release) mark, since
    /// the window may never hear those releases (`D-144`). The pads stay
    /// connected.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            for key in self.pressed.drain() {
                self.just_released.insert(key);
                self.fixed.just_released.insert(key);
            }
            for button in self.mouse_pressed.drain() {
                self.mouse_just_released.insert(button);
                self.fixed.mouse_just_released.insert(button);
            }
            for index in 0..self.pads.len() {
                self.release_pad(index);
            }
            self.modifiers = Modifiers::empty();
            self.suppressed.clear();
        }
        self.record(RawInput::Focus(focused));
    }

    /// Connects `pad` and records it; a pad already connected records only.
    pub fn gamepad_connected(&mut self, pad: GamepadId) {
        self.pad_index(pad);
        self.record(RawInput::Gamepad {
            pad,
            connected: true,
        });
    }

    /// Disconnects `pad` and records it: its held buttons and stick
    /// directions release, with edges in both views and no recorded
    /// releases, and its axes read 0.
    pub fn gamepad_disconnected(&mut self, pad: GamepadId) {
        if let Some(index) = self.pads.iter().position(|state| state.id == pad) {
            self.release_pad(index);
            self.pads.remove(index);
        }
        self.record(RawInput::Gamepad {
            pad,
            connected: false,
        });
    }

    /// A press of `button` on `pad`: holds and records it; a press of a held
    /// button changes and records nothing.
    pub fn gamepad_button_down(&mut self, pad: GamepadId, button: GamepadButton) {
        let index = self.pad_index(pad);
        let state = &mut self.pads[index];
        if state.buttons.contains(&button) {
            return;
        }
        state.buttons.push(button);
        self.pad_edges.pressed.push((pad, button));
        self.fixed.pad.pressed.push((pad, button));
        self.record(RawInput::GamepadButton {
            pad,
            button,
            pressed: true,
        });
    }

    /// A release of `button` on `pad`: releases and records it when held.
    pub fn gamepad_button_up(&mut self, pad: GamepadId, button: GamepadButton) {
        let Some(state) = self.pads.iter_mut().find(|state| state.id == pad) else {
            return;
        };
        let Some(at) = state.buttons.iter().position(|held| *held == button) else {
            return;
        };
        state.buttons.swap_remove(at);
        self.pad_edges.released.push((pad, button));
        self.fixed.pad.released.push((pad, button));
        self.record(RawInput::GamepadButton {
            pad,
            button,
            pressed: false,
        });
    }

    /// Moves `pad`'s `axis` to `value`, clamped to −1..1, and records it. A
    /// direction whose value crosses [`STICK_PRESS`] presses and one that
    /// falls below [`STICK_RELEASE`] releases, each with edges in both views
    /// and a [`RawInput::GamepadStick`] record before the axis record. A move
    /// that crosses nothing, right after a move of the same axis that crossed
    /// nothing, updates that one's record.
    pub fn set_gamepad_axis(&mut self, pad: GamepadId, axis: GamepadAxis, value: f32) {
        let value = if value.is_nan() {
            0.0
        } else {
            value.clamp(-1.0, 1.0)
        };
        let index = self.pad_index(pad);
        self.pads[index].axes[axis.index()] = value;
        let mut crossed = false;
        for direction in AxisDirection::BOTH {
            let along = direction.along(value);
            let state = &mut self.pads[index];
            let held = state.sticks.iter().position(|s| *s == (axis, direction));
            let pressed = match held {
                None if along >= STICK_PRESS => {
                    state.sticks.push((axis, direction));
                    self.pad_edges.stick_pressed.push((pad, axis, direction));
                    self.fixed.pad.stick_pressed.push((pad, axis, direction));
                    true
                }
                Some(at) if along < STICK_RELEASE => {
                    state.sticks.swap_remove(at);
                    self.pad_edges.stick_released.push((pad, axis, direction));
                    self.fixed.pad.stick_released.push((pad, axis, direction));
                    false
                }
                _ => continue,
            };
            crossed = true;
            self.record(RawInput::GamepadStick {
                pad,
                axis,
                direction,
                pressed,
            });
        }
        if !crossed
            && self.open_axis == Some((pad, axis))
            && let Some(RawInput::GamepadAxis { value: last, .. }) =
                self.raw_events.last_mut().map(|event| &mut event.input)
        {
            *last = value;
            return;
        }
        self.record(RawInput::GamepadAxis { pad, axis, value });
        if !crossed {
            self.open_axis = Some((pad, axis));
        }
    }

    /// Sets the modifiers held and records them.
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
        self.record(RawInput::Modifiers(modifiers));
    }

    /// The pointer left the window: records it. The cursor position stays
    /// the last one, and the next move sets it without a delta.
    pub fn pointer_left(&mut self) {
        self.pointer_in_window = false;
        self.record(RawInput::PointerLeft);
    }

    /// Ignores `key`'s presses, recording nothing, until its next release,
    /// which clears the mark and records nothing either. Does nothing to a
    /// held key. The window bridge marks a key still held when focus
    /// returns, so it stays released until it is pressed again (`D-144`).
    pub fn suppress_until_release(&mut self, key: KeyCode) {
        if !self.pressed.contains(&key) {
            self.suppressed.insert(key);
        }
    }

    #[must_use]
    pub fn is_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    /// Pressed since the last frame, or since the last fixed step while the
    /// fixed view is on.
    #[must_use]
    pub fn just_pressed(&self, key: KeyCode) -> bool {
        if self.fixed_view {
            self.fixed.just_pressed.contains(&key)
        } else {
            self.just_pressed.contains(&key)
        }
    }

    /// Released since the last frame, or since the last fixed step while the
    /// fixed view is on.
    #[must_use]
    pub fn just_released(&self, key: KeyCode) -> bool {
        if self.fixed_view {
            self.fixed.just_released.contains(&key)
        } else {
            self.just_released.contains(&key)
        }
    }

    #[must_use]
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    /// As [`just_pressed`](Self::just_pressed), for a mouse button.
    #[must_use]
    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        if self.fixed_view {
            self.fixed.mouse_just_pressed.contains(&button)
        } else {
            self.mouse_just_pressed.contains(&button)
        }
    }

    /// As [`just_released`](Self::just_released), for a mouse button.
    #[must_use]
    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        if self.fixed_view {
            self.fixed.mouse_just_released.contains(&button)
        } else {
            self.mouse_just_released.contains(&button)
        }
    }

    #[must_use]
    pub fn is_scroll_active(&self, direction: ScrollDirection) -> bool {
        self.scroll_pressed.contains(&direction)
    }

    /// As [`just_pressed`](Self::just_pressed), for a scroll notch.
    #[must_use]
    pub fn scroll_just_pressed(&self, direction: ScrollDirection) -> bool {
        if self.fixed_view {
            self.fixed.scroll_just_pressed.contains(&direction)
        } else {
            self.scroll_just_pressed.contains(&direction)
        }
    }

    /// As [`just_released`](Self::just_released), for a scroll notch, which
    /// releases at the end of the frame it arrived in.
    #[must_use]
    pub fn scroll_just_released(&self, direction: ScrollDirection) -> bool {
        if self.fixed_view {
            self.fixed.scroll_just_released.contains(&direction)
        } else {
            self.scroll_just_released.contains(&direction)
        }
    }

    #[must_use]
    pub fn cursor_position(&self) -> Option<(f32, f32)> {
        self.cursor_position
    }

    #[must_use]
    pub fn cursor_delta(&self) -> (f32, f32) {
        self.cursor_delta
    }

    #[must_use]
    pub fn scroll_line_delta(&self) -> (f32, f32) {
        self.scroll_line_delta
    }

    #[must_use]
    pub fn scroll_pixel_delta(&self) -> (f32, f32) {
        self.scroll_pixel_delta
    }

    /// The connected gamepads, in ID order.
    pub fn gamepads(&self) -> impl Iterator<Item = GamepadId> + '_ {
        self.pads.iter().map(|state| state.id)
    }

    /// Whether `button` is held on any pad.
    #[must_use]
    pub fn is_gamepad_pressed(&self, button: GamepadButton) -> bool {
        self.pads
            .iter()
            .any(|state| state.buttons.contains(&button))
    }

    /// Whether `button` is held on `pad`.
    #[must_use]
    pub fn is_gamepad_pressed_on(&self, pad: GamepadId, button: GamepadButton) -> bool {
        self.pad(pad)
            .is_some_and(|state| state.buttons.contains(&button))
    }

    /// As [`just_pressed`](Self::just_pressed), for `button` on any pad.
    #[must_use]
    pub fn gamepad_just_pressed(&self, button: GamepadButton) -> bool {
        self.view_pad_edges()
            .pressed
            .iter()
            .any(|(_, pressed)| *pressed == button)
    }

    /// As [`just_released`](Self::just_released), for `button` on any pad.
    #[must_use]
    pub fn gamepad_just_released(&self, button: GamepadButton) -> bool {
        self.view_pad_edges()
            .released
            .iter()
            .any(|(_, released)| *released == button)
    }

    /// Whether `axis` is held toward `direction` on any pad: pressed at
    /// [`STICK_PRESS`], released below [`STICK_RELEASE`].
    #[must_use]
    pub fn is_stick_pressed(&self, axis: GamepadAxis, direction: AxisDirection) -> bool {
        self.pads
            .iter()
            .any(|state| state.sticks.contains(&(axis, direction)))
    }

    /// As [`just_pressed`](Self::just_pressed), for a stick direction on any
    /// pad.
    #[must_use]
    pub fn stick_just_pressed(&self, axis: GamepadAxis, direction: AxisDirection) -> bool {
        self.view_pad_edges()
            .stick_pressed
            .iter()
            .any(|&(_, a, d)| (a, d) == (axis, direction))
    }

    /// As [`just_released`](Self::just_released), for a stick direction on
    /// any pad.
    #[must_use]
    pub fn stick_just_released(&self, axis: GamepadAxis, direction: AxisDirection) -> bool {
        self.view_pad_edges()
            .stick_released
            .iter()
            .any(|&(_, a, d)| (a, d) == (axis, direction))
    }

    /// `axis`'s value, −1 to 1, on the pad where it is farthest from 0; 0
    /// with no pad.
    #[must_use]
    pub fn gamepad_axis(&self, axis: GamepadAxis) -> f32 {
        self.pads
            .iter()
            .map(|state| state.axes[axis.index()])
            .fold(0.0, |best, value| {
                if value.abs() > best.abs() {
                    value
                } else {
                    best
                }
            })
    }

    /// `axis`'s value on `pad`, −1 to 1; 0 for a pad not connected.
    #[must_use]
    pub fn gamepad_axis_on(&self, pad: GamepadId, axis: GamepadAxis) -> f32 {
        self.pad(pad).map_or(0.0, |state| state.axes[axis.index()])
    }

    /// The farthest any pad's `axis` goes toward `direction`, 0 when none
    /// does.
    pub(crate) fn stick_reach(&self, axis: GamepadAxis, direction: AxisDirection) -> f32 {
        self.pads
            .iter()
            .map(|state| direction.along(state.axes[axis.index()]))
            .fold(0.0, f32::max)
    }

    /// The frame's input in arrival order: every event recorded since the
    /// last [`begin_frame`](Self::begin_frame), not routed. The fixed view
    /// does not change it.
    #[must_use]
    pub fn raw_events(&self) -> &[RawEvent] {
        &self.raw_events
    }

    /// Whether the window has the keyboard focus; true until
    /// [`set_focused`](Self::set_focused) says otherwise.
    #[must_use]
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// The modifiers held.
    #[must_use]
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Whether the pointer is over the window: false before the first move
    /// and from [`pointer_left`](Self::pointer_left) until the next.
    #[must_use]
    pub fn pointer_in_window(&self) -> bool {
        self.pointer_in_window
    }

    fn record(&mut self, input: RawInput) {
        self.raw_events.push(RawEvent {
            seq: self.next_seq,
            input,
        });
        self.next_seq += 1;
        self.open_axis = None;
    }

    fn pad(&self, pad: GamepadId) -> Option<&PadState> {
        self.pads.iter().find(|state| state.id == pad)
    }

    /// `pad`'s slot, connecting it when it is not.
    fn pad_index(&mut self, pad: GamepadId) -> usize {
        match self.pads.binary_search_by_key(&pad, |state| state.id) {
            Ok(index) => index,
            Err(index) => {
                self.pads.insert(
                    index,
                    PadState {
                        id: pad,
                        buttons: Vec::new(),
                        sticks: Vec::new(),
                        axes: [0.0; 4],
                    },
                );
                index
            }
        }
    }

    /// Releases the pad at `index`'s buttons and stick directions, with
    /// edges in both views and no records, and zeroes its axes.
    fn release_pad(&mut self, index: usize) {
        let state = &mut self.pads[index];
        let pad = state.id;
        for button in state.buttons.drain(..) {
            self.pad_edges.released.push((pad, button));
            self.fixed.pad.released.push((pad, button));
        }
        for (axis, direction) in state.sticks.drain(..) {
            self.pad_edges.stick_released.push((pad, axis, direction));
            self.fixed.pad.stick_released.push((pad, axis, direction));
        }
        state.axes = [0.0; 4];
    }

    /// The pad edges of the view in use.
    fn view_pad_edges(&self) -> &PadEdges {
        if self.fixed_view {
            &self.fixed.pad
        } else {
            &self.pad_edges
        }
    }

    fn register_scroll_direction(&mut self, y: f32) {
        let direction = if y > 0.0 {
            Some(ScrollDirection::Up)
        } else if y < 0.0 {
            Some(ScrollDirection::Down)
        } else {
            None
        };
        let Some(direction) = direction else {
            return;
        };
        self.scroll_just_released.remove(&direction);
        self.fixed.scroll_just_released.remove(&direction);
        if self.scroll_pressed.insert(direction) {
            self.scroll_just_pressed.insert(direction);
            self.fixed.scroll_just_pressed.insert(direction);
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// The input the UI did not take (`D-149`): an [`InputState`] that only the
/// UI routing system feeds, through the same mutators, in arrival order. A
/// press the UI takes never reaches it, nor does that key's or button's
/// release, and a press it got always gets its release, so no key sticks on
/// either side; moves, pointer leave, modifiers and focus always reach it.
/// [`ActionMap`] reads it through `Deref`, and its frame and fixed views
/// behave as [`InputState`]'s. Its frame starts at each routing, so a reader
/// that runs before the routing sees the previous routing's edges. It is
/// opt-in: [`InputState`] stays the raw input, which the engine's own toggles
/// read.
#[derive(Debug, Clone, Default)]
pub struct RoutedInput(InputState);

impl std::ops::Deref for RoutedInput {
    type Target = InputState;

    fn deref(&self) -> &InputState {
        &self.0
    }
}

impl std::ops::DerefMut for RoutedInput {
    fn deref_mut(&mut self) -> &mut InputState {
        &mut self.0
    }
}

#[cfg(test)]
#[path = "tests/input.rs"]
mod tests;
