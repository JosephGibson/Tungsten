use std::collections::HashSet;

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

    pub cursor_position: Option<(f32, f32)>,
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
            cursor_position: None,
        }
    }

    /// Clear per-frame edge state. Call at the start of each frame
    /// before processing new events.
    pub fn begin_frame(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
        self.mouse_just_pressed.clear();
        self.mouse_just_released.clear();
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
}
