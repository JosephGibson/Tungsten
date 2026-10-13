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

/// Keyboard/mouse/scroll state with per-frame edges, and the frame's raw
/// input in arrival order.
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

        self.cursor_delta = (0.0, 0.0);
        self.scroll_line_delta = (0.0, 0.0);
        self.scroll_pixel_delta = (0.0, 0.0);
        self.raw_events.clear();
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
    /// key and mouse button, with release edges in both views and no
    /// recorded releases, and clears the modifiers and every
    /// [`suppress_until_release`](Self::suppress_until_release) mark, since
    /// the window may never hear those releases (`D-144`).
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
            self.modifiers = Modifiers::empty();
            self.suppressed.clear();
        }
        self.record(RawInput::Focus(focused));
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

#[cfg(test)]
#[path = "tests/input.rs"]
mod tests;
