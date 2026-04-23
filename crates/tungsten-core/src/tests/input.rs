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
    input.key_down(KeyCode::KeyW);
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
