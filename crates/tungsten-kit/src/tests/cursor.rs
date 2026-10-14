use super::*;

fn world_with(camera: CameraState) -> World {
    let mut world = World::new();
    world.insert_resource(camera);
    world.insert_resource(InputState::new());
    world
}

#[test]
fn none_without_a_cursor_an_input_state_or_a_camera() {
    let mut world = world_with(CameraState::new());
    assert_eq!(cursor_to_world(&world), None);

    world
        .resource_mut::<InputState>()
        .update_cursor_position(10.0, 20.0);
    let input = world.remove_resource::<InputState>().unwrap();
    assert_eq!(cursor_to_world(&world), None);

    world.insert_resource(input);
    world.remove_resource::<CameraState>();
    assert_eq!(cursor_to_world(&world), None);
}

#[test]
fn the_default_camera_maps_pixels_to_world_units() {
    let mut world = world_with(CameraState::new());
    world
        .resource_mut::<InputState>()
        .update_cursor_position(10.0, 20.0);
    assert_eq!(cursor_to_world(&world), Some(Vec2::new(10.0, 20.0)));
}

#[test]
fn a_moved_and_zoomed_camera_maps_through_its_inverse() {
    let mut camera = CameraState::new();
    camera.position = Vec2::new(100.0, 50.0);
    camera.zoom = 2.0;
    let mut world = world_with(camera);
    world
        .resource_mut::<InputState>()
        .update_cursor_position(60.0, 40.0);
    assert_eq!(cursor_to_world(&world), Some(Vec2::new(130.0, 70.0)));
}

#[test]
fn a_rotated_camera_inverts_where_the_platformer_copy_refused() {
    let mut camera = CameraState::new();
    camera.position = Vec2::new(100.0, 50.0);
    camera.zoom = 2.0;
    camera.rotation = 0.5;
    let mut world = world_with(camera);
    // (130, 70) draws at (71.83, 6.34) under this camera.
    world
        .resource_mut::<InputState>()
        .update_cursor_position(71.83, 6.34);
    let point = cursor_to_world(&world).unwrap();
    assert!(
        (point - Vec2::new(130.0, 70.0)).length() < 0.01,
        "{point:?}"
    );
}

#[test]
fn the_last_position_maps_after_the_pointer_leaves() {
    let mut world = world_with(CameraState::new());
    world
        .resource_mut::<InputState>()
        .update_cursor_position(5.0, 6.0);
    world.resource_mut::<InputState>().pointer_left();
    assert!(!world.resource::<InputState>().pointer_in_window());
    assert_eq!(cursor_to_world(&world), Some(Vec2::new(5.0, 6.0)));
}
