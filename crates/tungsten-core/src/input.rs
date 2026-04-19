use std::collections::HashSet;

use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

pub mod action_map;
pub mod key_serde;

pub use action_map::{ActionMap, ActionMapError, Binding};

/// Keyboard key codes, matching winit's `KeyCode` variants we actually use.
/// Kept separate from winit so tungsten-core doesn't depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Space,
    Enter,
    Escape,
    F4,
    F9,
    F11,
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    KeyM,
    KeyV,
    Digit1,
    Digit2,
    Digit3,
    Equal,
    Minus,
    Other(u32),
}

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Discrete scroll directions exposed to the action map. Wheel motion is still
/// available as raw line/pixel deltas on `InputState`; these directions exist
/// so scroll-up / scroll-down can participate in the same boolean action path
/// as keys and mouse buttons.
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

/// Resource tracking keyboard and mouse state with edge detection.
/// Inserted as a Resource in the World. Updated each frame by the app layer.
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

    /// Clear per-frame edge state. Call at the start of each frame
    /// before processing new events.
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

    // --- Query methods ---

    pub fn is_pressed(&self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }

    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed.contains(&key)
    }

    pub fn just_released(&self, key: KeyCode) -> bool {
        self.just_released.contains(&key)
    }

    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_just_pressed.contains(&button)
    }

    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        self.mouse_just_released.contains(&button)
    }

    pub fn is_scroll_active(&self, direction: ScrollDirection) -> bool {
        self.scroll_pressed.contains(&direction)
    }

    pub fn scroll_just_pressed(&self, direction: ScrollDirection) -> bool {
        self.scroll_just_pressed.contains(&direction)
    }

    pub fn scroll_just_released(&self, direction: ScrollDirection) -> bool {
        self.scroll_just_released.contains(&direction)
    }

    pub fn cursor_position(&self) -> Option<(f32, f32)> {
        self.cursor_position
    }

    pub fn cursor_delta(&self) -> (f32, f32) {
        self.cursor_delta
    }

    pub fn scroll_line_delta(&self) -> (f32, f32) {
        self.scroll_line_delta
    }

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
mod tests {
    use super::*;

    #[test]
    fn key_press_and_release() {
        let mut input = InputState::new();
        input.key_down(KeyCode::Space);
        assert!(input.is_pressed(KeyCode::Space));
        assert!(input.just_pressed(KeyCode::Space));

        input.begin_frame();
        assert!(input.is_pressed(KeyCode::Space));
        assert!(!input.just_pressed(KeyCode::Space));

        input.key_up(KeyCode::Space);
        assert!(!input.is_pressed(KeyCode::Space));
        assert!(input.just_released(KeyCode::Space));
    }

    #[test]
    fn mouse_press_and_release() {
        let mut input = InputState::new();
        input.mouse_down(MouseButton::Left);
        assert!(input.is_mouse_pressed(MouseButton::Left));
        assert!(input.mouse_just_pressed(MouseButton::Left));

        input.begin_frame();
        assert!(!input.mouse_just_pressed(MouseButton::Left));

        input.mouse_up(MouseButton::Left);
        assert!(input.mouse_just_released(MouseButton::Left));
    }

    #[test]
    fn duplicate_key_down_does_not_re_trigger() {
        let mut input = InputState::new();
        input.key_down(KeyCode::KeyW);
        input.begin_frame();
        input.key_down(KeyCode::KeyW); // still held, no new press
        assert!(!input.just_pressed(KeyCode::KeyW));
        assert!(input.is_pressed(KeyCode::KeyW));
    }

    #[test]
    fn cursor_delta_accumulates_within_a_frame_and_resets_next_frame() {
        let mut input = InputState::new();
        input.update_cursor_position(10.0, 20.0);
        input.update_cursor_position(14.0, 25.0);
        input.update_cursor_position(20.0, 35.0);

        assert_eq!(input.cursor_position(), Some((20.0, 35.0)));
        assert_eq!(input.cursor_delta(), (10.0, 15.0));

        input.begin_frame();
        assert_eq!(input.cursor_delta(), (0.0, 0.0));
        assert_eq!(input.cursor_position(), Some((20.0, 35.0)));
    }

    #[test]
    fn scroll_delta_and_edges_reset_across_frames() {
        let mut input = InputState::new();
        input.add_scroll_line_delta(0.0, 1.0);
        input.add_scroll_pixel_delta(2.0, -4.0);

        assert_eq!(input.scroll_line_delta(), (0.0, 1.0));
        assert_eq!(input.scroll_pixel_delta(), (2.0, -4.0));
        assert!(input.is_scroll_active(ScrollDirection::Up));
        assert!(input.scroll_just_pressed(ScrollDirection::Up));
        assert!(input.is_scroll_active(ScrollDirection::Down));
        assert!(input.scroll_just_pressed(ScrollDirection::Down));

        input.begin_frame();

        assert_eq!(input.scroll_line_delta(), (0.0, 0.0));
        assert_eq!(input.scroll_pixel_delta(), (0.0, 0.0));
        assert!(!input.is_scroll_active(ScrollDirection::Up));
        assert!(input.scroll_just_released(ScrollDirection::Up));
        assert!(!input.is_scroll_active(ScrollDirection::Down));
        assert!(input.scroll_just_released(ScrollDirection::Down));
    }
}
