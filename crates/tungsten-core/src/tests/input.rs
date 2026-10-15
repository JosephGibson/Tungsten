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

#[test]
fn routed_input_reads_as_an_input_state_of_its_own() {
    let map = ActionMap::default_map();
    let raw = InputState::new();
    let mut routed = RoutedInput::default();
    assert!(routed.focused());
    routed.key_down(KeyCode::Space);
    assert!(
        map.just_pressed(&routed, "jump"),
        "the action map reads it through Deref"
    );
    assert!(!map.is_pressed(&raw, "jump"), "it shares nothing");
    routed.set_fixed_view(true);
    assert!(map.just_pressed(&routed, "jump"), "its fixed view");
    routed.end_fixed_step();
    assert!(!map.just_pressed(&routed, "jump"));
    routed.set_fixed_view(false);
    assert!(
        map.just_pressed(&routed, "jump"),
        "the frame view keeps its edge"
    );
    routed.begin_frame();
    assert!(!map.just_pressed(&routed, "jump"));
    assert!(map.is_pressed(&routed, "jump"));
    assert_eq!(routed.raw_events(), &[]);
}

// ---- gamepads (M54) ----

const PAD: GamepadId = GamepadId::new(0);
const PAD_B: GamepadId = GamepadId::new(3);

/// The frame's records, without their sequence numbers.
fn records(input: &InputState) -> Vec<RawInput> {
    input.raw_events().iter().map(|event| event.input).collect()
}

#[test]
fn gamepad_button_edges_in_both_views() {
    let mut input = InputState::new();
    input.gamepad_connected(PAD);
    input.gamepad_button_down(PAD, GamepadButton::South);
    assert!(input.is_gamepad_pressed(GamepadButton::South));
    assert!(input.is_gamepad_pressed_on(PAD, GamepadButton::South));
    assert!(!input.is_gamepad_pressed_on(PAD_B, GamepadButton::South));
    assert!(input.gamepad_just_pressed(GamepadButton::South));
    input.set_fixed_view(true);
    assert!(
        input.gamepad_just_pressed(GamepadButton::South),
        "fixed view"
    );
    input.set_fixed_view(false);
    // A second press of a held button records nothing.
    input.gamepad_button_down(PAD, GamepadButton::South);
    assert_eq!(
        records(&input),
        [
            RawInput::Gamepad {
                pad: PAD,
                connected: true
            },
            RawInput::GamepadButton {
                pad: PAD,
                button: GamepadButton::South,
                pressed: true
            },
        ]
    );

    // The frame view clears at the frame; the fixed view at the step.
    input.begin_frame();
    assert!(!input.gamepad_just_pressed(GamepadButton::South));
    input.set_fixed_view(true);
    assert!(input.gamepad_just_pressed(GamepadButton::South));
    input.end_fixed_step();
    assert!(!input.gamepad_just_pressed(GamepadButton::South));
    input.set_fixed_view(false);

    input.gamepad_button_up(PAD, GamepadButton::South);
    assert!(!input.is_gamepad_pressed(GamepadButton::South));
    assert!(input.gamepad_just_released(GamepadButton::South));
    input.set_fixed_view(true);
    assert!(input.gamepad_just_released(GamepadButton::South));
    input.set_fixed_view(false);
    // A release of a button not held records nothing.
    input.gamepad_button_up(PAD, GamepadButton::East);
    assert_eq!(records(&input).len(), 1);
    assert_eq!(input.gamepads().collect::<Vec<_>>(), [PAD]);
}

#[test]
fn stick_directions_press_with_hysteresis() {
    let mut input = InputState::new();
    let x = GamepadAxis::LeftStickX;
    let held = |input: &InputState| {
        (
            input.is_stick_pressed(x, AxisDirection::Negative),
            input.is_stick_pressed(x, AxisDirection::Positive),
        )
    };
    input.set_gamepad_axis(PAD, x, 0.49);
    assert_eq!(held(&input), (false, false), "below the press threshold");
    assert_eq!(
        input.gamepads().collect::<Vec<_>>(),
        [PAD],
        "input connects"
    );
    input.set_gamepad_axis(PAD, x, STICK_PRESS);
    assert_eq!(held(&input), (false, true));
    assert!(input.stick_just_pressed(x, AxisDirection::Positive));
    input.begin_frame();
    input.set_gamepad_axis(PAD, x, 0.36);
    assert_eq!(
        held(&input),
        (false, true),
        "held down to the release threshold"
    );
    input.set_gamepad_axis(PAD, x, 0.34);
    assert_eq!(held(&input), (false, false));
    assert!(input.stick_just_released(x, AxisDirection::Positive));
    assert_eq!(input.gamepad_axis(x), 0.34);
    assert_eq!(input.gamepad_axis_on(PAD, x), 0.34);

    // Full right to full left in one move releases one way and presses the
    // other, both recorded before the axis record.
    input.set_gamepad_axis(PAD, x, 1.0);
    input.begin_frame();
    input.set_gamepad_axis(PAD, x, -1.5);
    assert_eq!(held(&input), (true, false));
    assert_eq!(input.gamepad_axis(x), -1.0, "clamped");
    assert_eq!(
        records(&input),
        [
            RawInput::GamepadStick {
                pad: PAD,
                axis: x,
                direction: AxisDirection::Negative,
                pressed: true
            },
            RawInput::GamepadStick {
                pad: PAD,
                axis: x,
                direction: AxisDirection::Positive,
                pressed: false
            },
            RawInput::GamepadAxis {
                pad: PAD,
                axis: x,
                value: -1.0
            },
        ]
    );

    // Any pad: the farthest value, and a held direction on either.
    input.set_gamepad_axis(PAD_B, x, 0.6);
    assert_eq!(input.gamepad_axis(x), -1.0);
    assert!(input.is_stick_pressed(x, AxisDirection::Positive));
    assert_eq!(input.gamepads().collect::<Vec<_>>(), [PAD, PAD_B]);
}

#[test]
fn focus_loss_releases_gamepad_input() {
    let mut input = InputState::new();
    let mut map = ActionMap::new();
    map.replace_bindings(
        "move_right",
        vec![Binding::GamepadAxis {
            axis: GamepadAxis::LeftStickX,
            direction: AxisDirection::Positive,
        }],
    );
    map.replace_bindings(
        "jump",
        vec![Binding::GamepadButton {
            button: GamepadButton::South,
        }],
    );
    input.gamepad_connected(PAD);
    input.gamepad_button_down(PAD, GamepadButton::South);
    input.set_gamepad_axis(PAD, GamepadAxis::LeftStickX, 1.0);
    assert_eq!(map.value(&input, "move_right"), 1.0);
    input.begin_frame();

    input.set_focused(false);
    assert!(!input.is_gamepad_pressed(GamepadButton::South));
    assert!(input.gamepad_just_released(GamepadButton::South));
    assert!(!input.is_stick_pressed(GamepadAxis::LeftStickX, AxisDirection::Positive));
    assert!(input.stick_just_released(GamepadAxis::LeftStickX, AxisDirection::Positive));
    input.set_fixed_view(true);
    assert!(
        input.gamepad_just_released(GamepadButton::South),
        "fixed view"
    );
    input.set_fixed_view(false);
    assert_eq!(input.gamepad_axis(GamepadAxis::LeftStickX), 0.0);
    assert_eq!(map.value(&input, "move_right"), 0.0);
    assert_eq!(map.value(&input, "jump"), 0.0);
    assert!(map.just_released(&input, "jump"));
    // No recorded releases; the pad stays connected.
    assert_eq!(records(&input), [RawInput::Focus(false)]);
    assert_eq!(input.gamepads().collect::<Vec<_>>(), [PAD]);
}

#[test]
fn disconnect_releases_the_pads_input() {
    let mut input = InputState::new();
    input.gamepad_connected(PAD);
    input.gamepad_connected(PAD_B);
    input.gamepad_button_down(PAD, GamepadButton::North);
    input.gamepad_button_down(PAD_B, GamepadButton::West);
    input.set_gamepad_axis(PAD, GamepadAxis::RightStickY, -0.9);
    input.begin_frame();

    input.gamepad_disconnected(PAD);
    assert_eq!(input.gamepads().collect::<Vec<_>>(), [PAD_B]);
    assert!(!input.is_gamepad_pressed(GamepadButton::North));
    assert!(input.gamepad_just_released(GamepadButton::North));
    assert!(input.stick_just_released(GamepadAxis::RightStickY, AxisDirection::Negative));
    assert_eq!(input.gamepad_axis(GamepadAxis::RightStickY), 0.0);
    assert_eq!(input.gamepad_axis_on(PAD, GamepadAxis::RightStickY), 0.0);
    assert!(
        input.is_gamepad_pressed_on(PAD_B, GamepadButton::West),
        "the other pad holds"
    );
    assert_eq!(
        records(&input),
        [RawInput::Gamepad {
            pad: PAD,
            connected: false
        }]
    );
}

#[test]
fn adjacent_axis_moves_coalesce() {
    let mut input = InputState::new();
    let x = GamepadAxis::LeftStickX;
    let y = GamepadAxis::LeftStickY;
    let axis = |pad, axis, value| RawInput::GamepadAxis { pad, axis, value };
    input.set_gamepad_axis(PAD, x, 0.1);
    input.set_gamepad_axis(PAD, x, 0.2);
    input.set_gamepad_axis(PAD, x, 0.3);
    assert_eq!(
        records(&input),
        [axis(PAD, x, 0.3)],
        "one record, the last value"
    );

    // Another axis, or another pad, starts a record of its own.
    input.set_gamepad_axis(PAD, y, 0.1);
    input.set_gamepad_axis(PAD_B, y, 0.1);
    input.set_gamepad_axis(PAD_B, y, 0.2);
    assert_eq!(
        records(&input)[1..],
        [axis(PAD, y, 0.1), axis(PAD_B, y, 0.2)]
    );

    // A crossing move starts a record and keeps its value; the next move
    // starts another.
    input.begin_frame();
    input.set_gamepad_axis(PAD, x, 0.6);
    input.set_gamepad_axis(PAD, x, 0.45);
    input.set_gamepad_axis(PAD, x, 0.4);
    let press = RawInput::GamepadStick {
        pad: PAD,
        axis: x,
        direction: AxisDirection::Positive,
        pressed: true,
    };
    assert_eq!(
        records(&input),
        [press, axis(PAD, x, 0.6), axis(PAD, x, 0.4)]
    );

    // Something recorded in between ends the run.
    input.begin_frame();
    input.set_gamepad_axis(PAD, x, 0.41);
    input.key_down(KeyCode::KeyA);
    input.set_gamepad_axis(PAD, x, 0.42);
    assert_eq!(records(&input).len(), 3);
}
