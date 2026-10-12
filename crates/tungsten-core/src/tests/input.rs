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

fn recorded(input: &InputState) -> Vec<RawInput> {
    input.raw_events().iter().map(|event| event.input).collect()
}

fn key(code: KeyCode, pressed: bool, repeat: bool, modifiers: Modifiers) -> RawInput {
    RawInput::Key {
        code,
        pressed,
        repeat,
        modifiers,
    }
}

#[test]
fn a_focus_loss_releases_held_input_in_both_views_and_records_only_the_focus() {
    let mut input = InputState::new();
    input.key_down(KeyCode::KeyA);
    input.mouse_down(MouseButton::Left);
    input.set_modifiers(Modifiers::SHIFT);
    input.begin_frame();
    input.end_fixed_step();

    input.set_focused(false);

    assert!(!input.focused());
    assert!(!input.is_pressed(KeyCode::KeyA));
    assert!(!input.is_mouse_pressed(MouseButton::Left));
    assert_eq!(input.modifiers(), Modifiers::empty());
    assert!(input.just_released(KeyCode::KeyA));
    assert!(input.mouse_just_released(MouseButton::Left));
    input.set_fixed_view(true);
    assert!(input.just_released(KeyCode::KeyA));
    assert!(input.mouse_just_released(MouseButton::Left));
    input.set_fixed_view(false);
    assert_eq!(recorded(&input), [RawInput::Focus(false)]);

    input.set_focused(true);
    assert!(input.focused());
    assert_eq!(
        recorded(&input),
        [RawInput::Focus(false), RawInput::Focus(true)]
    );
}

#[test]
fn the_stream_keeps_arrival_order_and_seq_counts_across_frames() {
    let mut input = InputState::new();
    input.key_down(KeyCode::KeyA);
    input.mouse_down(MouseButton::Left);
    input.update_cursor_position(1.0, 2.0);
    input.add_scroll_line_delta(0.0, 1.0);
    input.set_modifiers(Modifiers::CONTROL);
    input.mouse_up(MouseButton::Left);
    input.add_scroll_pixel_delta(3.0, -4.0);
    input.key_up(KeyCode::KeyA);

    let seqs: Vec<u64> = input.raw_events().iter().map(|event| event.seq).collect();
    assert_eq!(seqs, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        recorded(&input),
        [
            key(KeyCode::KeyA, true, false, Modifiers::empty()),
            RawInput::MouseButton {
                button: MouseButton::Left,
                pressed: true,
            },
            RawInput::PointerMoved { x: 1.0, y: 2.0 },
            RawInput::Wheel {
                delta: WheelDelta::Lines(0.0, 1.0),
            },
            RawInput::Modifiers(Modifiers::CONTROL),
            RawInput::MouseButton {
                button: MouseButton::Left,
                pressed: false,
            },
            RawInput::Wheel {
                delta: WheelDelta::Pixels(3.0, -4.0),
            },
            key(KeyCode::KeyA, false, false, Modifiers::CONTROL),
        ]
    );

    input.begin_frame();
    input.key_down(KeyCode::KeyB);
    assert_eq!(input.raw_events()[0].seq, 8);
}

#[test]
fn only_adjacent_moves_coalesce() {
    let mut input = InputState::new();
    input.update_cursor_position(1.0, 1.0);
    input.update_cursor_position(2.0, 2.0);
    input.update_cursor_position(3.0, 3.0);
    input.mouse_down(MouseButton::Left);
    input.update_cursor_position(4.0, 4.0);
    input.pointer_left();
    input.update_cursor_position(5.0, 5.0);
    input.update_cursor_position(6.0, 6.0);

    let events = input.raw_events();
    let seqs: Vec<u64> = events.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, [0, 1, 2, 3, 4]);
    assert_eq!(
        recorded(&input),
        [
            RawInput::PointerMoved { x: 3.0, y: 3.0 },
            RawInput::MouseButton {
                button: MouseButton::Left,
                pressed: true,
            },
            RawInput::PointerMoved { x: 4.0, y: 4.0 },
            RawInput::PointerLeft,
            RawInput::PointerMoved { x: 6.0, y: 6.0 },
        ]
    );
}

#[test]
fn a_press_of_a_held_key_records_a_repeat_and_changes_nothing() {
    let mut input = InputState::new();
    input.key_down(KeyCode::KeyW);
    input.begin_frame();
    input.key_down(KeyCode::KeyW);
    assert!(input.is_pressed(KeyCode::KeyW));
    assert!(!input.just_pressed(KeyCode::KeyW));
    assert_eq!(
        recorded(&input),
        [key(KeyCode::KeyW, true, true, Modifiers::empty())]
    );

    input.key_up(KeyCode::KeyS);
    assert_eq!(
        recorded(&input).len(),
        1,
        "a key not held records no release"
    );
}

#[test]
fn a_suppressed_key_ignores_presses_until_its_release() {
    let mut input = InputState::new();
    input.suppress_until_release(KeyCode::KeyD);
    input.key_down(KeyCode::KeyD);
    input.key_down(KeyCode::KeyD);
    assert!(!input.is_pressed(KeyCode::KeyD));
    assert!(!input.just_pressed(KeyCode::KeyD));
    input.key_up(KeyCode::KeyD);
    assert!(!input.just_released(KeyCode::KeyD));
    assert!(input.raw_events().is_empty());

    input.key_down(KeyCode::KeyD);
    assert!(input.is_pressed(KeyCode::KeyD));
    assert!(input.just_pressed(KeyCode::KeyD));
    assert_eq!(
        recorded(&input),
        [key(KeyCode::KeyD, true, false, Modifiers::empty())]
    );

    input.suppress_until_release(KeyCode::KeyD);
    input.key_up(KeyCode::KeyD);
    assert!(
        input.just_released(KeyCode::KeyD),
        "a held key is not marked"
    );
}

#[test]
fn a_focus_loss_clears_the_suppression_marks() {
    let mut input = InputState::new();
    input.suppress_until_release(KeyCode::KeyD);
    input.set_focused(false);
    input.set_focused(true);
    input.key_down(KeyCode::KeyD);
    assert!(input.is_pressed(KeyCode::KeyD));
    assert!(input.just_pressed(KeyCode::KeyD));
}

#[test]
fn begin_frame_clears_the_stream_and_the_fixed_view_reads_it() {
    let mut input = InputState::new();
    input.key_down(KeyCode::Space);
    input.set_fixed_view(true);
    assert_eq!(input.raw_events().len(), 1);
    input.end_fixed_step();
    assert_eq!(input.raw_events().len(), 1);
    input.set_fixed_view(false);
    input.begin_frame();
    assert!(input.raw_events().is_empty());
}

#[test]
fn pointer_leave_keeps_the_position_and_the_next_move_adds_no_delta() {
    let mut input = InputState::new();
    assert!(!input.pointer_in_window());
    input.update_cursor_position(10.0, 20.0);
    assert!(input.pointer_in_window());

    input.pointer_left();
    assert!(!input.pointer_in_window());
    assert_eq!(input.cursor_position(), Some((10.0, 20.0)));
    assert_eq!(
        recorded(&input),
        [
            RawInput::PointerMoved { x: 10.0, y: 20.0 },
            RawInput::PointerLeft,
        ]
    );

    input.begin_frame();
    input.update_cursor_position(50.0, 60.0);
    assert!(input.pointer_in_window());
    assert_eq!(input.cursor_position(), Some((50.0, 60.0)));
    assert_eq!(input.cursor_delta(), (0.0, 0.0));
    input.update_cursor_position(52.0, 61.0);
    assert_eq!(input.cursor_delta(), (2.0, 1.0));
}

#[test]
fn modifiers_combine_and_answer_per_key() {
    let set = Modifiers::SHIFT.union(Modifiers::SUPER);
    assert!(set.shift());
    assert!(set.super_key());
    assert!(!set.control());
    assert!(!set.alt());
    assert!(set.contains(Modifiers::SHIFT));
    assert!(set.contains(Modifiers::empty()));
    assert!(!set.contains(Modifiers::SHIFT.union(Modifiers::ALT)));
    assert_eq!(Modifiers::default(), Modifiers::empty());
    assert!(Modifiers::CONTROL.control());
    assert!(Modifiers::ALT.alt());
}

#[test]
fn key_events_carry_the_modifiers_held() {
    let mut input = InputState::new();
    input.set_modifiers(Modifiers::CONTROL.union(Modifiers::SHIFT));
    input.key_down(KeyCode::KeyZ);
    assert_eq!(
        input.modifiers(),
        Modifiers::CONTROL.union(Modifiers::SHIFT)
    );
    assert_eq!(
        recorded(&input),
        [
            RawInput::Modifiers(Modifiers::CONTROL.union(Modifiers::SHIFT)),
            key(
                KeyCode::KeyZ,
                true,
                false,
                Modifiers::CONTROL.union(Modifiers::SHIFT)
            ),
        ]
    );
}
