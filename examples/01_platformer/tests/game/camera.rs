//! The view: the follow clamped to the map at every window and zoom, with
//! the cursor's mapping its inverse, the zoom actions' range, and the sky
//! covering the view at the camera's extremes.

use example_01_platformer::features::level::components::Zoom;
use example_01_platformer::features::level::draw;
use example_01_platformer::features::level::{CAMERA_ROWS, MAP_COLS, MAP_ROWS, TILE};
use example_01_platformer::features::player::{ZOOM_MAX, ZOOM_MIN};
use glam::Vec2;
use tungsten::WindowSize;
use tungsten::core::{AssetRegistry, CameraController, CameraState, Transform};
use tungsten::kit::CameraTarget;

use super::{mock_sprite, run};

#[test]
fn camera_clamps_both_axes_and_keeps_cursor_inverse_after_resize_and_zoom() {
    let (mut harness, player) = run();
    // A bare target the camera follows, which no rule kills off the map.
    let world = harness.world_mut();
    world.remove_component::<CameraTarget>(player);
    let probe = world.spawn_with((
        Transform::from_position(Vec2::splat(40_000.0)),
        CameraTarget,
    ));
    for (width, height) in [(1920, 1080), (960, 720), (2560, 720)] {
        for multiplier in [0.35, 1.0, 3.0] {
            let world = harness.world_mut();
            world.insert_resource(WindowSize { width, height });
            world.resource_mut::<Zoom>().0 = multiplier;
            world.get_mut::<Transform>(probe).unwrap().position = Vec2::splat(40_000.0);
            harness.step(1);
            let camera = *harness.world().resource::<CameraState>();
            let zoom = height as f32 / (CAMERA_ROWS * TILE) * multiplier;
            assert!(
                (camera.zoom - zoom).abs() < 1e-4,
                "{} vs {zoom}",
                camera.zoom
            );
            let maximum = Vec2::new(
                MAP_COLS as f32 * TILE - width as f32 / zoom,
                MAP_ROWS as f32 * TILE - height as f32 / zoom,
            )
            .max(Vec2::ZERO);
            assert!(
                (camera.position - maximum).length() < 0.01,
                "{width}x{height} at {multiplier}"
            );
            let cursor = Vec2::new(width as f32 * 0.37, height as f32 * 0.71);
            let point = camera.screen_to_world(cursor);
            assert!(((point - camera.position) * zoom - cursor).length() < 0.01);
            harness
                .world_mut()
                .get_mut::<Transform>(probe)
                .unwrap()
                .position = Vec2::splat(-1000.0);
            harness.step(1);
            assert_eq!(
                harness.world().resource::<CameraState>().position,
                Vec2::ZERO
            );
        }
    }
}

#[test]
fn zoom_input_reaches_and_clamps_expanded_range() {
    let (mut harness, _) = run();
    for (action, expected) in [("zoom_out", ZOOM_MIN), ("zoom_in", ZOOM_MAX)] {
        for _ in 0..30 {
            harness.harness_mut().press_action(action);
            harness.step(1);
            harness.harness_mut().release_action(action);
            harness.step(1);
        }
        let world = harness.world();
        assert!((world.resource::<Zoom>().0 - expected).abs() < 1e-3);
        let height = world.resource::<WindowSize>().height as f32;
        let base = height / (CAMERA_ROWS * TILE);
        let multiplier = world.resource::<CameraController>().zoom_multiplier;
        assert!((multiplier - base * expected).abs() < 1e-3, "{multiplier}");
    }
}

#[test]
fn parallax_covers_camera_extremes_resizes_zoom_and_shake() {
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let assets = world.resource_mut::<AssetRegistry>();
    for (handle, name) in ["ex10_sky", "ex10_distant_ridges", "ex10_near_woodland"]
        .iter()
        .enumerate()
    {
        mock_sprite(assets, name, 900 + handle as u32, false);
    }
    for (width, height) in [(1920, 1080), (960, 720), (3840, 720)] {
        world.insert_resource(WindowSize { width, height });
        for multiplier in [0.35, 1.0, 3.0] {
            for position in [
                Vec2::splat(-16.0),
                Vec2::new(MAP_COLS as f32 * TILE, MAP_ROWS as f32 * TILE),
            ] {
                let camera = world.resource_mut::<CameraState>();
                camera.position = position;
                camera.zoom = height as f32 / (CAMERA_ROWS * TILE) * multiplier;
                let (min, max) = camera.visible_world_aabb(width as f32, height as f32);
                let batches = draw::backdrop(world);
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
