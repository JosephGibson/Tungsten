use glam::Vec2;
use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
use tungsten::core::{
    sync_position_to_transform, ActionMap, AnimationState, Binding, CameraController, CameraMode,
    CameraState, CommandBuffer, Config, DeltaTime, EventQueue, InputState, KeyCode, MouseButton,
    TilemapInstance, TilemapRegistry, Transform, World,
};
use tungsten::physics::{
    physics_step, Collider, CollisionEvent, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten::{camera_update_system, App, WindowSize};

use crate::setup::{configure_platformer_camera, RUNTIME_SYSTEM_ORDER};
use crate::state::{
    ActiveBlackHole, Ball, BallSpawnState, BlackHole, CurrentSprite, Player, TextDisplayState,
    BALL_SPAWN_JITTER, BLACK_HOLE_LIFETIME, BLACK_HOLE_RADIUS, GRAVITY_Y, MAP_COLS, MAP_ROWS,
    PLAYER_HALF, PLAYER_SPAWN, TILE, WORLD_BOUNDS_MAX, WORLD_BOUNDS_MIN,
};
use crate::systems::{
    black_hole_force_system, black_hole_lifetime_system, cursor_to_world, despawn_out_of_bounds,
    ground_detection, platformer_camera_base_zoom, player_input, spawn_ball_system,
    spawn_black_hole_system,
};

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
    assert_eq!(physics.broadphase_cell_size, 16.0);
    assert!(world.get_resource::<TextDisplayState>().is_some());

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    assert_eq!(player_entities.len(), 1);
    assert_eq!(world.query::<Ball>().count(), 9);
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
            "spawn_ball_system",
            "spawn_black_hole_system",
            "black_hole_force_system",
            "audio_input_system",
            "camera_zoom_input_system",
            "animation_system",
            "physics_step",
            "ground_detection",
            "black_hole_lifetime_system",
            "despawn_out_of_bounds",
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
    // Place the player well past the half-viewport. With the 84x32 map, the
    // base zoom is 288/(32*TILE) ≈ 0.5625, so half the 480px window spans
    // ≈ 427 world px — anything smaller keeps the camera clamped at 0.
    world.insert(player, Position(Vec2::new(800.0, 100.0)));
    world.insert(player, Transform::from_position(Vec2::new(800.0, 100.0)));
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
fn spawn_ball_system_spawns_at_fixed_rate_while_held() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(BallSpawnState::default());

    // Bind spawn_ball to LMB for the test (engine default does not include it).
    world
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_ball",
            vec![Binding::Mouse {
                button: MouseButton::Left,
            }],
        );

    {
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.update_cursor_position(240.0, 144.0);
        input.mouse_down(MouseButton::Left);
    }

    {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::new(0.0, 0.0);
        camera.zoom = 1.0;
    }

    // One 170 ms step at a held LMB should yield floor(0.170 / 0.032) = 5 balls.
    // (Avoids the floating-point cliff at exactly 5 * interval.)
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 0.170;
    spawn_ball_system(&mut world);

    let buffer = world
        .remove_resource::<CommandBuffer>()
        .expect("CommandBuffer present");
    world.flush(buffer);
    world.insert_resource(CommandBuffer::new());

    assert_eq!(world.query::<Ball>().count(), 5);
    let center = Vec2::new(240.0, 144.0);
    let positions: Vec<Vec2> = world
        .query::<Ball>()
        .map(|(e, _)| world.get::<Position>(e).unwrap().0)
        .collect();
    for pos in &positions {
        let dist = (*pos - center).length();
        assert!(
            (dist - BALL_SPAWN_JITTER).abs() < 1.0e-3,
            "ball {pos:?} not on jitter ring (dist {dist}, expected {BALL_SPAWN_JITTER})"
        );
    }
    // Coincident spawns were the root cause of the pile-drift bug; every
    // spawn in a single hold must resolve to a distinct world position.
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            assert_ne!(
                positions[i], positions[j],
                "balls {i} and {j} coincident at {:?}",
                positions[i]
            );
        }
    }
}

#[test]
fn spawn_ball_system_resets_accumulator_on_release() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(BallSpawnState::default());
    world
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_ball",
            vec![Binding::Mouse {
                button: MouseButton::Left,
            }],
        );

    // Half a spawn interval while held — not enough to fire yet.
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 0.016;
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .mouse_down(MouseButton::Left);
    spawn_ball_system(&mut world);
    assert!(world.get_resource::<BallSpawnState>().unwrap().accumulator > 0.0);

    // Release: accumulator must snap back to zero so a later press starts fresh.
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .mouse_up(MouseButton::Left);
    spawn_ball_system(&mut world);
    assert_eq!(
        world.get_resource::<BallSpawnState>().unwrap().accumulator,
        0.0
    );
}

#[test]
fn spawn_black_hole_system_creates_attractor_at_cursor_on_right_click() {
    let mut world = seed_world();
    world.insert_resource(ActiveBlackHole::default());
    world
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_black_hole",
            vec![Binding::Mouse {
                button: MouseButton::Right,
            }],
        );

    {
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.update_cursor_position(120.0, 80.0);
        input.mouse_down(MouseButton::Right);
    }
    {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::ZERO;
        camera.zoom = 1.0;
    }

    spawn_black_hole_system(&mut world);

    let holes: Vec<_> = world.query::<BlackHole>().collect();
    assert_eq!(holes.len(), 1);
    let (hole_entity, hole) = holes[0];
    assert_eq!(hole.remaining, BLACK_HOLE_LIFETIME);
    let pos = world.get::<Position>(hole_entity).unwrap().0;
    assert_eq!(pos, Vec2::new(120.0, 80.0));
    assert_eq!(
        world.get_resource::<ActiveBlackHole>().unwrap().0,
        Some(hole_entity),
        "press must record the dragged entity"
    );
}

#[test]
fn spawn_black_hole_system_drags_active_hole_to_cursor_while_held() {
    let mut world = seed_world();
    world.insert_resource(ActiveBlackHole::default());
    world
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_black_hole",
            vec![Binding::Mouse {
                button: MouseButton::Right,
            }],
        );
    {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::ZERO;
        camera.zoom = 1.0;
    }

    // Frame 1: press at (50, 60) — spawns and tracks the hole.
    {
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.update_cursor_position(50.0, 60.0);
        input.mouse_down(MouseButton::Right);
    }
    spawn_black_hole_system(&mut world);
    let hole_entity = world
        .get_resource::<ActiveBlackHole>()
        .unwrap()
        .0
        .expect("press should register active hole");

    // Frame 2: still held, cursor moved to (200, 150), and age it a bit.
    if let Some(hole) = world.get_mut::<BlackHole>(hole_entity) {
        hole.remaining = 0.5;
    }
    {
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.begin_frame();
        input.update_cursor_position(200.0, 150.0);
    }
    spawn_black_hole_system(&mut world);

    let pos = world.get::<Position>(hole_entity).unwrap().0;
    assert_eq!(pos, Vec2::new(200.0, 150.0), "hole should follow cursor");
    assert_eq!(
        world.get::<BlackHole>(hole_entity).unwrap().remaining,
        BLACK_HOLE_LIFETIME,
        "holding must refresh lifetime so the hole never expires mid-drag"
    );
    assert_eq!(
        world.query::<BlackHole>().count(),
        1,
        "hold must not spawn a second hole per frame"
    );

    // Frame 3: release — the dragged hole despawns immediately and the
    // active slot clears so later presses start fresh.
    {
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.begin_frame();
        input.mouse_up(MouseButton::Right);
    }
    spawn_black_hole_system(&mut world);
    assert_eq!(world.get_resource::<ActiveBlackHole>().unwrap().0, None);
    assert_eq!(
        world.query::<BlackHole>().count(),
        0,
        "release must despawn the dragged hole immediately, not let it fade"
    );
}

#[test]
fn black_hole_force_system_pulls_dynamic_body_toward_hole() {
    let mut world = seed_world();
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 1.0 / 60.0;

    let hole = world.spawn();
    world.insert(
        hole,
        BlackHole {
            remaining: BLACK_HOLE_LIFETIME,
        },
    );
    world.insert(hole, Position(Vec2::new(0.0, 0.0)));

    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(ball, Position(Vec2::new(BLACK_HOLE_RADIUS * 0.5, 0.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(6.0));
    world.insert(ball, RigidBody::dynamic());

    black_hole_force_system(&mut world);

    let vel = world.get::<Velocity>(ball).unwrap().0;
    assert!(
        vel.x < 0.0,
        "ball should accelerate toward the hole (-x), got {vel:?}"
    );
    assert_eq!(vel.y, 0.0);
}

#[test]
fn black_hole_force_system_ignores_bodies_outside_radius() {
    let mut world = seed_world();
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 1.0 / 60.0;

    let hole = world.spawn();
    world.insert(
        hole,
        BlackHole {
            remaining: BLACK_HOLE_LIFETIME,
        },
    );
    world.insert(hole, Position(Vec2::ZERO));

    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(ball, Position(Vec2::new(BLACK_HOLE_RADIUS + 10.0, 0.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(6.0));
    world.insert(ball, RigidBody::dynamic());

    black_hole_force_system(&mut world);
    assert_eq!(world.get::<Velocity>(ball).unwrap().0, Vec2::ZERO);
}

#[test]
fn black_hole_lifetime_system_despawns_expired_hole() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());
    world.get_resource_mut::<DeltaTime>().unwrap().dt = BLACK_HOLE_LIFETIME + 0.1;

    let hole = world.spawn();
    world.insert(
        hole,
        BlackHole {
            remaining: BLACK_HOLE_LIFETIME,
        },
    );
    world.insert(hole, Position(Vec2::ZERO));

    black_hole_lifetime_system(&mut world);
    let buffer = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buffer);

    assert_eq!(world.query::<BlackHole>().count(), 0);
}

#[test]
fn despawn_out_of_bounds_culls_escaped_balls_and_keeps_in_bounds_balls() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    // In-bounds ball (centre of map).
    let inside = world.spawn();
    world.insert(inside, Ball);
    world.insert(
        inside,
        Position(Vec2::new(
            MAP_COLS as f32 * TILE * 0.5,
            MAP_ROWS as f32 * TILE * 0.5,
        )),
    );

    // Out-of-bounds ball (far below the map, as if it escaped the floor).
    let outside = world.spawn();
    world.insert(outside, Ball);
    world.insert(
        outside,
        Position(Vec2::new(100.0, WORLD_BOUNDS_MAX.y + 1.0)),
    );

    assert_eq!(world.query::<Ball>().count(), 2);

    despawn_out_of_bounds(&mut world);
    let buffer = world.remove_resource::<CommandBuffer>().unwrap();
    assert_eq!(
        buffer.len(),
        1,
        "exactly one ball should be queued for despawn"
    );
    world.flush(buffer);

    let remaining: Vec<_> = world.query::<Ball>().map(|(e, _)| e).collect();
    assert_eq!(remaining, vec![inside]);
}

#[test]
fn despawn_out_of_bounds_resets_escaped_player_to_spawn() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    let player = world.spawn();
    world.insert(player, Player::default());
    // Below the world-bounds lower edge — simulates falling through the map.
    world.insert(
        player,
        Position(Vec2::new(100.0, WORLD_BOUNDS_MAX.y + 50.0)),
    );
    world.insert(player, Velocity(Vec2::new(25.0, 900.0)));

    despawn_out_of_bounds(&mut world);

    let pos = world.get::<Position>(player).unwrap().0;
    let vel = world.get::<Velocity>(player).unwrap().0;
    assert_eq!(pos, PLAYER_SPAWN, "player not reset to spawn");
    assert_eq!(vel, Vec2::ZERO, "player velocity not cleared on reset");
}

#[test]
fn despawn_out_of_bounds_is_noop_for_in_bounds_player() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    let player = world.spawn();
    world.insert(player, Player::default());
    let start = Vec2::new(
        (WORLD_BOUNDS_MIN.x + WORLD_BOUNDS_MAX.x) * 0.5,
        (WORLD_BOUNDS_MIN.y + WORLD_BOUNDS_MAX.y) * 0.5,
    );
    world.insert(player, Position(start));
    world.insert(player, Velocity(Vec2::new(42.0, -17.0)));

    despawn_out_of_bounds(&mut world);

    let pos = world.get::<Position>(player).unwrap().0;
    let vel = world.get::<Velocity>(player).unwrap().0;
    assert_eq!(pos, start, "in-bounds player should not move");
    assert_eq!(
        vel,
        Vec2::new(42.0, -17.0),
        "in-bounds velocity must not change"
    );
}
