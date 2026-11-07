//! The view: the camera prefabs' numbers, the follow clamped to the map at
//! every window and zoom, with the cursor's mapping its inverse, the zoom
//! actions' range, and the sky covering the view at the camera's extremes.

use example_01_platformer::features::level::components::Zoom;
use example_01_platformer::features::level::draw;
use example_01_platformer::features::level::title::{TITLE_CAMERA, sky_view};
use example_01_platformer::features::level::{CAMERA_ROWS, MAP_COLS, MAP_ROWS, TILE};
use example_01_platformer::features::player::{ZOOM_MAX, ZOOM_MIN};
use glam::Vec2;
use tungsten::WindowSize;
use tungsten::core::{
    AssetRegistry, CameraBounds, CameraController, CameraRig, CameraState, FixedRig, FollowRig,
    RigBlend, RigKind, Transform,
};

use super::{mock_sprite, run};

#[test]
fn the_camera_prefabs_restate_the_map_and_the_sky() {
    let (run, player) = run();
    let world = run.world();
    let view = CAMERA_ROWS * TILE;
    let map = Vec2::new(MAP_COLS as f32, MAP_ROWS as f32) * TILE;
    let expected = CameraRig::follow(FollowRig::default())
        .with_view_height(view)
        .with_bounds(CameraBounds::new(Vec2::ZERO, map));
    let rig = *world.get::<CameraRig>(player).expect("the player's rig");
    assert_eq!(
        rig.kind, expected.kind,
        "a snapping follow of the player's point"
    );
    assert_eq!(
        (rig.view_height, rig.bounds, rig.blend),
        (
            Some(view),
            Some(CameraBounds::new(Vec2::ZERO, map)),
            RigBlend::CUT
        )
    );
    assert_eq!(world.resource::<CameraController>().winner(), Some(player));
    // The title's: a fixed rig at the sky's corner, the base zoom's height.
    let mut title = super::harness();
    title.step(2);
    let world = title.world_mut();
    let (corner, size) = sky_view(world);
    let camera = world
        .prepare_prefab(TITLE_CAMERA, Vec2::ZERO)
        .unwrap()
        .spawn(world);
    let rig = *world.get::<CameraRig>(camera).unwrap();
    assert_eq!(rig.kind, RigKind::Fixed(FixedRig::Corner(corner)));
    assert_eq!((rig.view_height, rig.bounds), (Some(size.y), None));
    assert_eq!(size.y, view);
}

#[test]
fn camera_clamps_both_axes_and_keeps_cursor_inverse_after_resize_and_zoom() {
    let (mut harness, player) = run();
    // A bare entity the player's rig follows, which no rule kills off the
    // map.
    let world = harness.world_mut();
    let probe = world.spawn_with((Transform::from_position(Vec2::splat(40_000.0)),));
    if let RigKind::Follow(follow) = &mut world.get_mut::<CameraRig>(player).unwrap().kind {
        follow.target = Some(probe);
    }
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
    let (mut harness, player) = run();
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
        let zoom = world.resource::<CameraState>().zoom;
        assert!((zoom - base * expected).abs() < 1e-3, "{zoom}");
        let rig = world.get::<CameraRig>(player).unwrap();
        assert_eq!(rig.view_height, Some(CAMERA_ROWS * TILE));
        assert!((rig.zoom - expected).abs() < 1e-3, "the player's rig");
    }
}

#[test]
fn zoom_returns_to_one() {
    let (mut harness, _) = run();
    let press = |harness: &mut tungsten::testing::UiHarness, action: &str| {
        harness.harness_mut().press_action(action);
        harness.step(1);
        harness.harness_mut().release_action(action);
        harness.step(1);
        harness.world().resource::<Zoom>().0
    };
    for _ in 0..10 {
        press(&mut harness, "zoom_out");
    }
    let back: Vec<f32> = (0..3).map(|_| press(&mut harness, "zoom_in")).collect();
    assert_eq!(back, [0.5, 0.75, 1.0], "on the 0.25 grid from the floor");
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
