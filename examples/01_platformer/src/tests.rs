use glam::Vec2;
use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
use tungsten::core::{
    sync_position_to_transform, ActionMap, AnimationState, CameraController, CameraMode,
    CameraState, Config, DeltaTime, EventQueue, InputState, KeyCode, TilemapInstance,
    TilemapRegistry, Transform, World,
};
use tungsten::physics::{
    physics_step, Collider, CollisionEvent, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten::{camera_update_system, App, WindowSize};

use crate::setup::{configure_platformer_camera, RUNTIME_SYSTEM_ORDER};
use crate::state::{
    Ball, CurrentSprite, Player, TextDisplayState, GRAVITY_Y, MAP_COLS, MAP_ROWS, PLAYER_HALF, TILE,
};
use crate::systems::{ground_detection, platformer_camera_base_zoom, player_input};

fn seed_world() -> World {
    let mut world = World::new();
    world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
    world.insert_resource(InputState::new());
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(EventQueue::<CollisionEvent>::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY_Y),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(CameraState::new());
    world.insert_resource(CameraController::default());
    world.insert_resource(WindowSize {
        width: 480,
        height: 288,
    });
    world
}

fn solid_floor(width: u32) -> TilemapData {
    let mut tiles = vec![-1i32; (width as usize) * 2];
    for x in 0..width as usize {
        tiles[width as usize + x] = 0;
    }
    TilemapData {
        tile_width: 16,
        tile_height: 16,
        width,
        height: 2,
        tileset: vec!["ex10_ground".into()],
        layers: vec![TilemapLayer {
            name: "collision".into(),
            kind: LayerKind::Collision,
            tiles,
        }],
    }
}

#[test]
fn configure_app_seeds_expected_bootstrap_state() {
    let mut app = App::new(Config::default()).expect("App::new failed");
    crate::setup::configure_app(&mut app);

    let world = app.world_mut();
    let physics = world.get_resource::<PhysicsConfig>().unwrap();
    assert_eq!(physics.gravity, Vec2::new(0.0, GRAVITY_Y));
    assert_eq!(physics.broadphase_cell_size, 32.0);
    assert!(world.get_resource::<TextDisplayState>().is_some());

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    assert_eq!(player_entities.len(), 1);
    assert_eq!(world.query::<Ball>().count(), 8);
    assert_eq!(world.query::<TilemapInstance>().count(), 1);

    let player = player_entities[0];
    assert!(world.get::<AnimationState>(player).is_some());
    assert!(world.get::<CurrentSprite>(player).is_some());

    let controller = world.get_resource::<CameraController>().unwrap();
    assert!(matches!(controller.mode, CameraMode::Follow(entity) if entity == player));
}

#[test]
fn runtime_system_order_matches_expected_pipeline() {
    let names: Vec<_> = RUNTIME_SYSTEM_ORDER.iter().map(|(name, _)| *name).collect();

    assert_eq!(
        names,
        vec![
            "update_text_display",
            "player_input",
            "audio_input_system",
            "camera_zoom_input_system",
            "animation_system",
            "physics_step",
            "ground_detection",
            "sync_position_to_transform",
            "platformer_camera_base_zoom",
            "camera_update_system",
        ]
    );
}

#[test]
fn player_moves_right_on_d() {
    let mut world = seed_world();
    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, Position(Vec2::new(100.0, 100.0)));
    world.insert(player, Transform::from_position(Vec2::new(100.0, 100.0)));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic());
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::KeyD);

    player_input(&mut world);

    let vel = world.get::<Velocity>(player).unwrap().0;
    assert!(vel.x > 0.0, "velocity.x did not increase: {:?}", vel);
}

#[test]
fn player_becomes_grounded_after_falling_onto_tilemap() {
    let mut world = seed_world();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), solid_floor(8));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, Position(Vec2::new(40.0, 8.0)));
    world.insert(player, Transform::from_position(Vec2::new(40.0, 8.0)));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic());

    for _ in 0..20 {
        player_input(&mut world);
        physics_step(&mut world);
        ground_detection(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    let p = world.get::<Player>(player).unwrap();
    assert!(p.grounded, "player did not become grounded");
}

#[test]
fn jump_impulse_only_applies_when_grounded() {
    let mut world = seed_world();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), solid_floor(8));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

    let player = world.spawn();
    world.insert(player, Player { grounded: false });
    world.insert(player, Position(Vec2::new(40.0, 40.0)));
    world.insert(player, Transform::from_position(Vec2::new(40.0, 40.0)));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic());
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::Space);

    player_input(&mut world);

    let vel = world.get::<Velocity>(player).unwrap().0;
    assert!(
        vel.y >= 0.0,
        "jump fired while airborne — should be gated: {:?}",
        vel
    );
}

#[test]
fn shared_camera_tracks_player() {
    let mut world = seed_world();
    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, Position(Vec2::new(300.0, 100.0)));
    world.insert(player, Transform::from_position(Vec2::new(300.0, 100.0)));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic());
    configure_platformer_camera(&mut world, player);

    sync_position_to_transform(&mut world);
    platformer_camera_base_zoom(&mut world);
    camera_update_system(&mut world);

    let cam = world.get_resource::<CameraState>().unwrap();
    assert!(
        cam.position.x > 0.0,
        "camera did not follow player: {:?}",
        cam.position
    );
}

#[test]
fn camera_clamped_at_right_boundary() {
    let mut world = seed_world();
    let player = world.spawn();
    world.insert(player, Player::default());
    // Place the player far past the right edge of the map.
    world.insert(player, Position(Vec2::new(9999.0, 100.0)));
    world.insert(player, Transform::from_position(Vec2::new(9999.0, 100.0)));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic());
    configure_platformer_camera(&mut world, player);

    sync_position_to_transform(&mut world);
    platformer_camera_base_zoom(&mut world);
    camera_update_system(&mut world);

    let cam = world.get_resource::<CameraState>().unwrap();
    // The shared camera path derives zoom from window.height / map_h.
    // seed_world uses 480x288, map_h = 288, so zoom = 1.0 and viewport_w = 480.
    let zoom = 288.0 / (MAP_ROWS as f32 * TILE);
    let max_x = (MAP_COLS as f32 * TILE - 480.0 / zoom).max(0.0);
    assert!(
        cam.position.x <= max_x,
        "camera not clamped: {} > {}",
        cam.position.x,
        max_x
    );
}
