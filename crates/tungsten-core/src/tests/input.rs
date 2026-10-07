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

/// One app frame after its events: `steps` fixed steps in the fixed view,
/// each read by `fixed`, then the frame view, read by `update`, then
/// `begin_frame`.
fn run_frame(
    input: &mut InputState,
    steps: u32,
    mut fixed: impl FnMut(&InputState),
    update: impl FnOnce(&InputState),
) {
    for _ in 0..steps {
        input.set_fixed_view(true);
        fixed(input);
        input.end_fixed_step();
    }
    input.set_fixed_view(false);
    update(input);
    input.begin_frame();
}

#[test]
fn a_press_in_a_frame_without_a_step_reaches_the_next_step_only() {
    let mut input = InputState::new();
    input.key_down(KeyCode::Space);
    run_frame(
        &mut input,
        0,
        |_| {},
        |i| assert!(i.just_pressed(KeyCode::Space)),
    );
    let mut seen = Vec::new();
    for _ in 0..3 {
        run_frame(
            &mut input,
            1,
            |i| seen.push(i.just_pressed(KeyCode::Space)),
            |i| assert!(!i.just_pressed(KeyCode::Space)),
        );
    }
    assert_eq!(seen, [true, false, false]);
}

#[test]
fn with_two_steps_the_first_sees_each_edge() {
    let mut input = InputState::new();
    input.key_down(KeyCode::Space);
    input.mouse_down(MouseButton::Left);
    let mut seen = Vec::new();
    run_frame(
        &mut input,
        2,
        |i| {
            seen.push((
                i.just_pressed(KeyCode::Space),
                i.mouse_just_pressed(MouseButton::Left),
            ));
        },
        |i| assert!(i.just_pressed(KeyCode::Space)),
    );
    assert_eq!(seen, [(true, true), (false, false)]);

    input.key_up(KeyCode::Space);
    input.mouse_up(MouseButton::Left);
    seen.clear();
    run_frame(
        &mut input,
        2,
        |i| {
            seen.push((
                i.just_released(KeyCode::Space),
                i.mouse_just_released(MouseButton::Left),
            ));
        },
        |i| assert!(i.just_released(KeyCode::Space)),
    );
    assert_eq!(seen, [(true, true), (false, false)]);
}

#[test]
fn the_frame_view_reads_as_before_around_the_steps() {
    let mut input = InputState::new();
    input.key_down(KeyCode::Space);
    input.mouse_down(MouseButton::Left);
    input.set_fixed_view(true);
    assert!(input.just_pressed(KeyCode::Space));
    input.end_fixed_step();
    assert!(!input.just_pressed(KeyCode::Space));
    assert!(!input.mouse_just_pressed(MouseButton::Left));
    assert!(input.is_pressed(KeyCode::Space), "levels stay per frame");
    assert!(input.is_mouse_pressed(MouseButton::Left));
    input.set_fixed_view(false);
    assert!(input.just_pressed(KeyCode::Space));
    assert!(input.mouse_just_pressed(MouseButton::Left));
    input.begin_frame();
    assert!(!input.just_pressed(KeyCode::Space));
    assert!(input.is_pressed(KeyCode::Space));
}

#[test]
fn an_action_map_reads_the_view_its_input_is_in() {
    let mut map = ActionMap::new();
    map.replace_bindings(
        "jump",
        vec![Binding::Key {
            code: KeyCode::Space,
        }],
    );
    let mut input = InputState::new();
    input.key_down(KeyCode::Space);
    input.set_fixed_view(true);
    assert!(map.just_pressed(&input, "jump"));
    input.end_fixed_step();
    assert!(!map.just_pressed(&input, "jump"));
    assert!(map.is_pressed(&input, "jump"));
    input.set_fixed_view(false);
    assert!(map.just_pressed(&input, "jump"));
}

#[test]
fn a_scroll_notch_and_its_release_reach_one_step() {
    let mut input = InputState::new();
    input.add_scroll_line_delta(0.0, 1.0);
    run_frame(
        &mut input,
        0,
        |_| {},
        |i| assert!(i.scroll_just_pressed(ScrollDirection::Up)),
    );
    let mut seen = Vec::new();
    for _ in 0..2 {
        run_frame(
            &mut input,
            2,
            |i| {
                seen.push((
                    i.scroll_just_pressed(ScrollDirection::Up),
                    i.scroll_just_released(ScrollDirection::Up),
                ));
            },
            |_| {},
        );
    }
    assert_eq!(
        seen,
        [(true, true), (false, false), (false, false), (false, false)]
    );
}

#[test]
fn cursor_and_scroll_deltas_stay_per_frame_in_the_fixed_view() {
    let mut input = InputState::new();
    input.update_cursor_position(10.0, 20.0);
    input.update_cursor_position(14.0, 25.0);
    input.add_scroll_line_delta(0.0, 1.0);
    input.set_fixed_view(true);
    assert_eq!(input.cursor_delta(), (4.0, 5.0));
    assert_eq!(input.scroll_line_delta(), (0.0, 1.0));
    input.end_fixed_step();
    assert_eq!(input.cursor_delta(), (4.0, 5.0));
    assert_eq!(input.scroll_line_delta(), (0.0, 1.0));
    input.set_fixed_view(false);
    input.begin_frame();
    input.set_fixed_view(true);
    assert_eq!(input.cursor_delta(), (0.0, 0.0));
    assert_eq!(input.scroll_line_delta(), (0.0, 0.0));
}
