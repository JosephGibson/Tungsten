use super::*;

#[test]
fn cursor_to_world_inverts_camera_translation_and_zoom() {
    let mut camera = CameraState::new();
    camera.position = Vec2::new(100.0, 50.0);
    camera.zoom = 2.0;
    let world_pos = cursor_to_world(Vec2::new(40.0, 20.0), &camera)
        .expect("non-rotated camera should invert cleanly");
    assert_eq!(world_pos, Vec2::new(120.0, 60.0));
}

#[test]
fn camera_clamps_both_axes_and_keeps_cursor_inverse_after_resize_and_zoom() {
    for (width, height) in [(1920, 1080), (960, 720), (2560, 720)] {
        for multiplier in [0.35, 1.0, 3.0] {
            let mut world = seed_world();
            world.insert_resource(WindowSize { width, height });
            let player = spawn_test_player(&mut world, Vec2::new(20000.0, 20000.0));
            configure_platformer_camera(&mut world, player);
            world
                .get_resource_mut::<CameraController>()
                .unwrap()
                .zoom_multiplier = multiplier;
            platformer_camera_base_zoom(&mut world);
            camera_update_system(&mut world);
            let camera = world.get_resource::<CameraState>().unwrap();
            let zoom = height as f32 / (crate::state::CAMERA_ROWS * TILE) * multiplier;
            assert!((camera.zoom - zoom).abs() < 0.0001);
            let maximum = Vec2::new(
                MAP_COLS as f32 * TILE - width as f32 / zoom,
                MAP_ROWS as f32 * TILE - height as f32 / zoom,
            )
            .max(Vec2::ZERO);
            assert!((camera.position - maximum).length() < 0.01);
            let cursor = Vec2::new(width as f32 * 0.37, height as f32 * 0.71);
            let point = cursor_to_world(cursor, camera).unwrap();
            assert!(((point - camera.position) * zoom - cursor).length() < 0.01);
            world.get_mut::<Transform>(player).unwrap().position = Vec2::splat(-1000.0);
            platformer_camera_base_zoom(&mut world);
            camera_update_system(&mut world);
            assert_eq!(
                world.get_resource::<CameraState>().unwrap().position,
                Vec2::ZERO
            );
        }
    }
}

#[test]
fn zoom_input_reaches_and_clamps_expanded_range() {
    let mut harness = platformer_harness(&[(
        "camera_zoom_input_system",
        crate::systems::camera_zoom_input_system,
    )]);
    for (key, expected) in [(KeyCode::Minus, 0.35), (KeyCode::Equal, 3.0)] {
        for _ in 0..30 {
            let input = input_mut(&mut harness);
            input.key_up(key);
            input.key_down(key);
            harness.step(1);
        }
        assert!(
            (harness
                .world()
                .get_resource::<CameraController>()
                .unwrap()
                .zoom_multiplier
                - expected)
                .abs()
                < 0.001
        );
    }
}

#[test]
fn parallax_covers_camera_extremes_resizes_zoom_and_shake() {
    use tungsten::core::AssetRegistry;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let mut assets = world.remove_resource::<AssetRegistry>().unwrap();
    for (id, name) in ["ex10_sky", "ex10_distant_ridges", "ex10_near_woodland"]
        .iter()
        .enumerate()
    {
        mock_sprite(&mut assets, name, id as u32, false);
    }
    world.insert_resource(assets);
    for (width, height) in [(1920, 1080), (960, 720), (3840, 720)] {
        world.insert_resource(WindowSize { width, height });
        for multiplier in [0.35, 1.0, 3.0] {
            for position in [
                Vec2::splat(-16.0),
                Vec2::new(MAP_COLS as f32 * TILE, MAP_ROWS as f32 * TILE),
            ] {
                let camera = world.get_resource_mut::<CameraState>().unwrap();
                camera.position = position;
                camera.zoom = height as f32 / (crate::state::CAMERA_ROWS * TILE) * multiplier;
                let (min, max) = camera.visible_world_aabb(width as f32, height as f32);
                let batches = crate::extract::extract_sprites(&world);
                let sky = &batches[0].instances[0];
                let origin = Vec2::from_array(sky.position);
                let end = origin + Vec2::from_array(sky.size);
                assert!(
                    origin.cmple(min).all() && end.cmpge(max).all(),
                    "sky coverage at {width}x{height}, {multiplier}, {position:?}"
                );
            }
        }
    }
}
