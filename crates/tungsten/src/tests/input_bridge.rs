use std::collections::HashSet;

use super::*;
use tungsten_core::input::{RawInput, key_serde};
use winit::keyboard::NativeKeyCode;

fn world() -> World {
    let mut world = World::new();
    world.insert_resource(InputState::new());
    world.insert_resource(ScaleFactor::default());
    world
}

fn input(world: &World) -> &InputState {
    world.get_resource::<InputState>().unwrap()
}

fn press(world: &mut World, code: WinitKeyCode, repeat: bool, is_synthetic: bool) {
    apply_key(
        world,
        PhysicalKey::Code(code),
        ElementState::Pressed,
        repeat,
        is_synthetic,
    );
}

fn release(world: &mut World, code: WinitKeyCode, is_synthetic: bool) {
    apply_key(
        world,
        PhysicalKey::Code(code),
        ElementState::Released,
        false,
        is_synthetic,
    );
}

#[test]
fn each_new_winit_code_maps_to_its_variant() {
    let new = [
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
        (WinitKeyCode::KeyE, KeyCode::KeyE),
        (WinitKeyCode::KeyG, KeyCode::KeyG),
        (WinitKeyCode::KeyO, KeyCode::KeyO),
        (WinitKeyCode::KeyQ, KeyCode::KeyQ),
        (WinitKeyCode::KeyT, KeyCode::KeyT),
        (WinitKeyCode::KeyX, KeyCode::KeyX),
        (WinitKeyCode::KeyZ, KeyCode::KeyZ),
        (WinitKeyCode::Digit4, KeyCode::Digit4),
        (WinitKeyCode::Digit9, KeyCode::Digit9),
        (WinitKeyCode::F5, KeyCode::F5),
        (WinitKeyCode::F6, KeyCode::F6),
        (WinitKeyCode::F7, KeyCode::F7),
        (WinitKeyCode::F8, KeyCode::F8),
        (WinitKeyCode::F10, KeyCode::F10),
        (WinitKeyCode::F12, KeyCode::F12),
    ];
    for (winit, code) in new {
        assert_eq!(translate_key(PhysicalKey::Code(winit)), code);
    }
}

#[test]
fn every_key_name_is_its_winit_codes_debug_name() {
    let mut named = HashSet::new();
    for &(winit, code) in KEYS {
        assert_eq!(
            key_serde::keycode_to_str(code),
            Some(format!("{winit:?}").as_str())
        );
        assert_eq!(translate_key(PhysicalKey::Code(winit)), code);
        assert!(named.insert(code), "{code:?} has two rows");
    }
    // `key_serde`'s test pins its table at the same count, so every name has
    // a row here.
    assert_eq!(named.len(), 74);
}

#[test]
fn unnamed_and_unidentified_keys_stay_other() {
    for winit in [WinitKeyCode::NumpadAdd, WinitKeyCode::Insert] {
        assert!(matches!(
            translate_key(PhysicalKey::Code(winit)),
            KeyCode::Other(_)
        ));
    }
    assert_eq!(
        translate_key(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)),
        KeyCode::Other(0)
    );
}

#[test]
fn a_synthetic_release_is_dropped() {
    let mut world = world();
    press(&mut world, WinitKeyCode::KeyA, false, false);
    release(&mut world, WinitKeyCode::KeyA, true);
    assert!(input(&world).is_pressed(KeyCode::KeyA));
    assert_eq!(input(&world).raw_events().len(), 1);
}

#[test]
fn a_synthetic_press_keeps_its_key_released_until_it_is_released() {
    let mut world = world();
    press(&mut world, WinitKeyCode::KeyW, false, true);
    // X11's first auto-repeat after focus returns is not flagged `repeat`.
    press(&mut world, WinitKeyCode::KeyW, false, false);
    press(&mut world, WinitKeyCode::KeyW, true, false);
    assert!(!input(&world).is_pressed(KeyCode::KeyW));
    assert!(!input(&world).just_pressed(KeyCode::KeyW));
    release(&mut world, WinitKeyCode::KeyW, false);
    assert!(!input(&world).just_released(KeyCode::KeyW));
    assert!(input(&world).raw_events().is_empty());

    press(&mut world, WinitKeyCode::KeyW, false, false);
    assert!(input(&world).is_pressed(KeyCode::KeyW));
    assert!(input(&world).just_pressed(KeyCode::KeyW));
}

#[test]
fn a_repeat_of_a_key_not_held_is_dropped() {
    let mut world = world();
    press(&mut world, WinitKeyCode::KeyD, true, false);
    assert!(!input(&world).is_pressed(KeyCode::KeyD));
    assert!(input(&world).raw_events().is_empty());

    press(&mut world, WinitKeyCode::KeyD, false, false);
    press(&mut world, WinitKeyCode::KeyD, true, false);
    let repeats: Vec<bool> = input(&world)
        .raw_events()
        .iter()
        .map(|event| matches!(event.input, RawInput::Key { repeat: true, .. }))
        .collect();
    assert_eq!(repeats, [false, true], "a held key's repeat is recorded");
}

#[test]
fn a_focus_loss_releases_held_keys() {
    let mut world = world();
    press(&mut world, WinitKeyCode::ArrowLeft, false, false);
    apply_focus(&mut world, false);
    assert!(!input(&world).focused());
    assert!(!input(&world).is_pressed(KeyCode::ArrowLeft));
    assert!(input(&world).just_released(KeyCode::ArrowLeft));
    assert!(matches!(
        input(&world).raw_events().last().unwrap().input,
        RawInput::Focus(false)
    ));
}

#[test]
fn modifiers_are_translated() {
    assert_eq!(
        translate_modifiers(ModifiersState::SHIFT | ModifiersState::SUPER),
        Modifiers::SHIFT.union(Modifiers::SUPER)
    );
    assert_eq!(
        translate_modifiers(ModifiersState::CONTROL | ModifiersState::ALT),
        Modifiers::CONTROL.union(Modifiers::ALT)
    );
    assert_eq!(
        translate_modifiers(ModifiersState::empty()),
        Modifiers::empty()
    );

    let mut world = world();
    apply_modifiers(&mut world, ModifiersState::CONTROL);
    assert_eq!(input(&world).modifiers(), Modifiers::CONTROL);
}

#[test]
fn pointer_leave_reaches_the_input_state() {
    let mut world = world();
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .update_cursor_position(3.0, 4.0);
    apply_pointer_left(&mut world);
    assert!(!input(&world).pointer_in_window());
    assert_eq!(input(&world).cursor_position(), Some((3.0, 4.0)));
}

#[test]
fn the_scale_factor_reaches_the_resource() {
    let mut world = world();
    apply_scale_factor(&mut world, 1.75);
    let scale = world.get_resource::<ScaleFactor>().unwrap();
    assert_eq!(scale.os(), 1.75);
    assert_eq!(scale.get(), 1.75);

    apply_scale_factor(&mut world, 0.0);
    assert_eq!(world.get_resource::<ScaleFactor>().unwrap().os(), 1.75);
}
