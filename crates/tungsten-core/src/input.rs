use std::collections::HashSet;

use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

pub mod action_map;
pub mod key_serde;

pub use action_map::{ActionMap, ActionMapError, Binding};

/// Winit-like key codes without core depending on winit.
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
    F9,
    F11,
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    KeyB,
    KeyC,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyP,
    KeyU,
    KeyV,
    KeyY,
    Tab,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Equal,
    Minus,
    BracketLeft,
    BracketRight,
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

/// Keyboard/mouse/scroll state with per-frame edges.
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

    cursor_position: Option<(f32, f32)>,
    cursor_delta: (f32, f32),
    scroll_line_delta: (f32, f32),
    scroll_pixel_delta: (f32, f32),
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
            cursor_position: None,
            cursor_delta: (0.0, 0.0),
            scroll_line_delta: (0.0, 0.0),
            scroll_pixel_delta: (0.0, 0.0),
        }
    }

    /// Clear per-frame edges and deltas.
    pub fn begin_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_just_pressed.clear();
        self.mouse_just_released.clear();

        self.scroll_just_pressed.clear();
        self.scroll_just_released.clear();
        self.scroll_just_released
            .extend(self.scroll_pressed.drain());

        self.cursor_delta = (0.0, 0.0);
        self.scroll_line_delta = (0.0, 0.0);
        self.scroll_pixel_delta = (0.0, 0.0);
    }

    pub fn key_down(&mut self, key: KeyCode) {
        if self.pressed.insert(key) {
            self.just_pressed.insert(key);
        }
    }

    pub fn key_up(&mut self, key: KeyCode) {
        if self.pressed.remove(&key) {
            self.just_released.insert(key);
        }
    }

    pub fn mouse_down(&mut self, button: MouseButton) {
        if self.mouse_pressed.insert(button) {
            self.mouse_just_pressed.insert(button);
        }
    }

    pub fn mouse_up(&mut self, button: MouseButton) {
        if self.mouse_pressed.remove(&button) {
            self.mouse_just_released.insert(button);
        }
    }

    pub fn update_cursor_position(&mut self, x: f32, y: f32) {
        if let Some((prev_x, prev_y)) = self.cursor_position {
            self.cursor_delta.0 += x - prev_x;
            self.cursor_delta.1 += y - prev_y;
        }
        self.cursor_position = Some((x, y));
    }

    pub fn add_scroll_line_delta(&mut self, x: f32, y: f32) {
        self.scroll_line_delta.0 += x;
        self.scroll_line_delta.1 += y;
        self.register_scroll_direction(y);
    }

    pub fn add_scroll_pixel_delta(&mut self, x: f32, y: f32) {
        self.scroll_pixel_delta.0 += x;
        self.scroll_pixel_delta.1 += y;
        self.register_scroll_direction(y);
    }

    #[must_use]
    pub fn is_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    #[must_use]
    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    #[must_use]
    pub fn just_released(&self, key: KeyCode) -> bool {
        self.just_released.contains(&key)
    }

    #[must_use]
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    #[must_use]
    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_just_pressed.contains(&button)
    }

    #[must_use]
    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        self.mouse_just_released.contains(&button)
    }

    #[must_use]
    pub fn is_scroll_active(&self, direction: ScrollDirection) -> bool {
        self.scroll_pressed.contains(&direction)
    }

    #[must_use]
    pub fn scroll_just_pressed(&self, direction: ScrollDirection) -> bool {
        self.scroll_just_pressed.contains(&direction)
    }

    #[must_use]
    pub fn scroll_just_released(&self, direction: ScrollDirection) -> bool {
        self.scroll_just_released.contains(&direction)
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
        if self.scroll_pressed.insert(direction) {
            self.scroll_just_pressed.insert(direction);
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
