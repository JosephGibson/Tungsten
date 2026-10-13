use super::*;
use tungsten_core::input::KeyCode;

fn world_with(input: InputState) -> World {
    let mut world = World::new();
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(UiDebug::default());
    world
}

fn debug(world: &World) -> UiDebug {
    *world.get_resource::<UiDebug>().unwrap()
}

#[test]
fn f5_flips_the_bounds_overlay_and_f6_hides_the_game_layers_on_press_edges() {
    let mut input = InputState::new();
    input.key_down(KeyCode::F5);
    let mut world = world_with(input);
    ui_debug_toggle_system(&mut world);
    assert_eq!(
        debug(&world),
        UiDebug {
            hide_game_layers: false,
            show_bounds: true
        }
    );

    // The key is still held: no second flip.
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .begin_frame();
    ui_debug_toggle_system(&mut world);
    assert!(debug(&world).show_bounds);

    let input = world.get_resource_mut::<InputState>().unwrap();
    input.key_up(KeyCode::F5);
    input.begin_frame();
    input.key_down(KeyCode::F6);
    ui_debug_toggle_system(&mut world);
    assert_eq!(
        debug(&world),
        UiDebug {
            hide_game_layers: true,
            show_bounds: true
        }
    );

    let input = world.get_resource_mut::<InputState>().unwrap();
    input.begin_frame();
    input.key_up(KeyCode::F6);
    input.begin_frame();
    input.key_down(KeyCode::F6);
    input.key_down(KeyCode::F5);
    ui_debug_toggle_system(&mut world);
    assert_eq!(debug(&world), UiDebug::default(), "both flip back");
}

#[test]
fn the_system_needs_the_input_map_and_resource() {
    let mut input = InputState::new();
    input.key_down(KeyCode::F5);
    let mut world = world_with(input);
    world.remove_resource::<UiDebug>();
    ui_debug_toggle_system(&mut world);
    assert!(
        world.get_resource::<UiDebug>().is_none(),
        "nothing inserted"
    );

    let mut world = World::new();
    world.insert_resource(UiDebug::default());
    ui_debug_toggle_system(&mut world);
    assert_eq!(debug(&world), UiDebug::default());
}
