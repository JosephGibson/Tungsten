use glam::Vec2;
mod ball_pit;
mod burning;
mod spells;
use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
use tungsten::core::{
    ActionMap, AnimationState, AudioCommand, AudioCommands, AudioHandle, Binding, CameraController,
    CameraMode, CameraState, CommandBuffer, Config, DeltaTime, EventQueue, InputState, KeyCode,
    MouseButton, TilemapInstance, TilemapRegistry, Transform, World, sync_position_to_transform,
};
use tungsten::physics::{
    Collider, CollisionEvent, PhysicsConfig, Position, RigidBody, Velocity, physics_step,
};
use tungsten::{App, WindowSize, camera_update_system};

use crate::setup::{RUNTIME_SYSTEM_ORDER, configure_platformer_camera};
use crate::state::{
    ActiveBlackHole, AudioState, BALL_RADIUS, BALL_SPAWN_JITTER, BLACK_HOLE_LIFETIME,
    BLACK_HOLE_RADIUS, Ball, BallHue, BallSpawnState, BlackHole, CurrentSprite, GRAVITY_Y,
    MAP_COLS, MAP_ROWS, PLAYER_HALF, PLAYER_SPAWN, Player, TEXT_UPDATE_INTERVAL, TILE,
    TextDisplayState, WORLD_BOUNDS_MAX, WORLD_BOUNDS_MIN,
};
use crate::systems::{
    black_hole_force_system, black_hole_lifetime_system, cursor_to_world, despawn_out_of_bounds,
    ground_detection, platformer_camera_base_zoom, player_input, rainbow_ball_hue_system,
    spawn_ball_system, spawn_black_hole_system, update_text_display,
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
        tile_width: TILE as u32,
        tile_height: TILE as u32,
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
    assert_eq!(physics.broadphase_cell_size, TILE);
    assert!(world.get_resource::<TextDisplayState>().is_some());

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    assert_eq!(player_entities.len(), 1);
    assert_eq!(world.query::<Ball>().count(), 9);
    // Seeded balls are bronze orbs: they keep their authored colours.
    assert_eq!(world.query::<BallHue>().count(), 0);
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
            "platformer_bindings",
            "update_text_display",
            "player_input",
            "lantern_input",
            "spawn_ball_system",
            "spawn_black_hole_system",
            "black_hole_force_system",
            "cast_fireball_system",
            "audio_input_system",
            "camera_zoom_input_system",
            "rainbow_ball_hue_system",
            "move_obstacles",
            "tick_ball_fire",
            "physics_step",
            "ground_detection",
            "small_ball_impacts",
            "hazard_contacts",
            "fireball_flight_system",
            "spread_ball_fire",
            "black_hole_extinguish_system",
            "black_hole_lifetime_system",
            "despawn_out_of_bounds",
            "player_presentation_system",
            "animation_system",
            "transient_emitter_cleanup",
            "sync_position_to_transform",
            "ball_fire_particles",
            "orbit_lights_system",
            "squash_stretch_trigger_system",
            "squash_stretch_tick_system",
            "scene_effects",
            "platformer_camera_base_zoom",
            "shake_tick_system",
            "camera_update_system",
        ]
    );
}

#[test]
fn extract_text_includes_debug_state_row() {
    let mut world = seed_world();
    world.insert_resource(TextDisplayState {
        fps: 60,
        contacts: 7,
        grounded: true,
        music_on: true,
        vol_pct: 50,
        zoom_pct: 125,
        ..TextDisplayState::default()
    });

    let sections = crate::extract::extract_text(&world);

    assert!(
        sections
            .iter()
            .any(|section| section.content.contains("FPS 60  Contacts 7  Grounded yes")),
        "debug state row should be visible in the example overlay"
    );
}

#[test]
fn update_text_display_refreshes_default_state_on_first_tick() {
    let mut world = seed_world();
    world.insert_resource(TextDisplayState::default());

    update_text_display(&mut world);

    let display = world.get_resource::<TextDisplayState>().unwrap();
    assert_eq!(display.fps, 60);
    assert!(
        display.timer < TEXT_UPDATE_INTERVAL,
        "first refresh should consume the primed display timer"
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
    assert!(vel.x > 0.0, "velocity.x did not increase: {vel:?}");
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
fn player_becomes_grounded_when_collision_event_lists_player_as_b() {
    let mut world = seed_world();

    let support = world.spawn();
    let player = world.spawn();
    world.insert(player, Player::default());

    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .send(CollisionEvent {
            a: support,
            b: Some(player),
            normal: Vec2::Y,
            penetration: 1.0,
        });

    ground_detection(&mut world);

    let player_state = world.get::<Player>(player).unwrap();
    assert!(
        player_state.grounded,
        "player should ground when the opposite event normal points upward for it"
    );
}

#[test]
fn exhausted_air_jump_requires_ground_contact() {
    let mut world = seed_world();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), solid_floor(8));
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

    let player = world.spawn();
    world.insert(
        player,
        Player {
            air_jump_used: true,
            ..Default::default()
        },
    );
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
        "jump fired while airborne — should be gated: {vel:?}"
    );
}

#[test]
fn shared_camera_tracks_player() {
    let mut world = seed_world();
    let player = world.spawn();
    world.insert(player, Player::default());
    // Past half-viewport so follow camera unclamps from origin.
    world.insert(player, Position(Vec2::new(1200.0, 100.0)));
    world.insert(player, Transform::from_position(Vec2::new(1200.0, 100.0)));
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
    // Seeded zoom=1.0, viewport_w=480.
    let zoom = 288.0 / (crate::state::CAMERA_ROWS * TILE);
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

    // 170 ms avoids exact-multiple floating-point cliff: floor(0.170 / 0.032) = 5.
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 0.170;
    spawn_ball_system(&mut world);

    let buffer = world
        .remove_resource::<CommandBuffer>()
        .expect("CommandBuffer present");
    world.flush(buffer);
    world.insert_resource(CommandBuffer::new());

    assert_eq!(world.query::<Ball>().count(), 5);
    // Left-click orbs carry no hue; only small marbles cycle colour.
    assert_eq!(world.query::<BallHue>().count(), 0);
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
    // Regression: no coincident spawns within one hold.
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
fn rainbow_ball_hue_system_advances_each_ball_hue() {
    let mut world = seed_world();
    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(
        ball,
        BallHue {
            hue: 0.95,
            speed: 0.2,
        },
    );
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 0.5;

    rainbow_ball_hue_system(&mut world);

    let hue = world.get::<BallHue>(ball).unwrap();
    assert!((hue.hue - 0.05).abs() < 1.0e-6);
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

    world.get_resource_mut::<DeltaTime>().unwrap().dt = 0.016;
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .mouse_down(MouseButton::Left);
    spawn_ball_system(&mut world);
    assert!(world.get_resource::<BallSpawnState>().unwrap().accumulator > 0.0);

    // Release resets held-spawn accumulator.
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
    world.insert_resource(AudioCommands::new());
    world.insert_resource(AudioState {
        sfx_handle: AudioHandle(1),
        black_hole_sfx_handle: AudioHandle(3),
        music_handle: AudioHandle(2),
        sfx_volume: 0.8,
        black_hole_sfx_volume: 0.65,
        music_volume: 0.4,
        music_playing: false,
        master_volume: 0.5,
    });
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
    let commands = world.get_resource_mut::<AudioCommands>().unwrap().drain();
    assert!(
        matches!(
            commands.as_slice(),
            [AudioCommand::Play {
                handle,
                volume,
                looping
            }] if *handle == AudioHandle(3) && (*volume - 0.65).abs() < f32::EPSILON && !*looping
        ),
        "press should play the black-hole spawn sound once"
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

    // Frame 1: press spawns and tracks.
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

    // Frame 2: hold moves and refreshes lifetime.
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

    // Frame 3: release despawns dragged entity and clears slot.
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
    world.insert(ball, Collider::circle(BALL_RADIUS));
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
    world.insert(ball, Collider::circle(BALL_RADIUS));
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
    world.insert_resource(ActiveBlackHole(Some(hole)));

    black_hole_lifetime_system(&mut world);
    assert_eq!(
        world.get_resource::<ActiveBlackHole>().unwrap().0,
        None,
        "expired active holes must clear their drag slot"
    );
    let buffer = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buffer);

    assert_eq!(world.query::<BlackHole>().count(), 0);
}

#[test]
fn despawn_out_of_bounds_culls_escaped_balls_and_keeps_in_bounds_balls() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    let inside = world.spawn();
    world.insert(inside, Ball);
    world.insert(
        inside,
        Position(Vec2::new(
            MAP_COLS as f32 * TILE * 0.5,
            MAP_ROWS as f32 * TILE * 0.5,
        )),
    );
    world.insert(inside, Collider::circle(BALL_RADIUS));

    let partly_inside = world.spawn();
    world.insert(partly_inside, Ball);
    world.insert(
        partly_inside,
        Position(Vec2::new(100.0, WORLD_BOUNDS_MAX.y + BALL_RADIUS - 1.0)),
    );
    world.insert(partly_inside, Collider::circle(BALL_RADIUS));

    let outside = world.spawn();
    world.insert(outside, Ball);
    world.insert(
        outside,
        Position(Vec2::new(100.0, WORLD_BOUNDS_MAX.y + BALL_RADIUS + 1.0)),
    );
    world.insert(outside, Collider::circle(BALL_RADIUS));

    assert_eq!(world.query::<Ball>().count(), 3);

    despawn_out_of_bounds(&mut world);
    let buffer = world.remove_resource::<CommandBuffer>().unwrap();
    assert_eq!(
        buffer.len(),
        1,
        "exactly one ball should be queued for despawn"
    );
    world.flush(buffer);

    let remaining: Vec<_> = world.query::<Ball>().map(|(e, _)| e).collect();
    assert_eq!(remaining, vec![inside, partly_inside]);
}

#[test]
fn despawn_out_of_bounds_resets_escaped_player_to_spawn() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(
        player,
        Position(Vec2::new(100.0, WORLD_BOUNDS_MAX.y + PLAYER_HALF.y + 50.0)),
    );
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, Velocity(Vec2::new(25.0, 900.0)));

    despawn_out_of_bounds(&mut world);

    let pos = world.get::<Position>(player).unwrap().0;
    let vel = world.get::<Velocity>(player).unwrap().0;
    assert_eq!(pos, PLAYER_SPAWN, "player not reset to spawn");
    assert_eq!(vel, Vec2::ZERO, "player velocity not cleared on reset");
}

#[test]
fn despawn_out_of_bounds_keeps_player_while_collider_overlaps_bounds() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    let player = world.spawn();
    world.insert(player, Player::default());
    let start = Vec2::new(WORLD_BOUNDS_MIN.x - PLAYER_HALF.x + 1.0, PLAYER_SPAWN.y);
    world.insert(player, Position(start));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, Velocity(Vec2::new(25.0, 900.0)));

    despawn_out_of_bounds(&mut world);

    let pos = world.get::<Position>(player).unwrap().0;
    let vel = world.get::<Velocity>(player).unwrap().0;
    assert_eq!(pos, start, "overlapping player should not reset yet");
    assert_eq!(
        vel,
        Vec2::new(25.0, 900.0),
        "overlapping player velocity must not change"
    );
}

#[test]
fn despawn_out_of_bounds_is_noop_for_in_bounds_player() {
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());

    let player = world.spawn();
    world.insert(player, Player::default());
    let start = Vec2::new(
        f32::midpoint(WORLD_BOUNDS_MIN.x, WORLD_BOUNDS_MAX.x),
        f32::midpoint(WORLD_BOUNDS_MIN.y, WORLD_BOUNDS_MAX.y),
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

fn asset_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(relative)
}

fn real_level_world() -> World {
    let mut world = seed_world();
    let map = TilemapData::load(asset_path("tilemaps/level.tmj")).unwrap();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), map);
    let entity = world.spawn();
    world.insert(entity, TilemapInstance::new("ex10_level", Vec2::ZERO));
    crate::gameplay::spawn_platform_colliders(&mut world);
    world
}

fn spawn_test_player(world: &mut World, position: Vec2) -> tungsten::core::Entity {
    let entity = world.spawn();
    world.insert(entity, Player::default());
    world.insert(entity, crate::state::PlayerPresentation::default());
    world.insert(entity, Position(position));
    world.insert(entity, Transform::from_position(position));
    world.insert(entity, Velocity(Vec2::ZERO));
    world.insert(entity, Collider::aabb(PLAYER_HALF));
    world.insert(entity, RigidBody::dynamic().with_restitution(0.0));
    world.insert(
        entity,
        AnimationState::new(crate::state::PLAYER_ANIMATION_ID),
    );
    world.insert(
        entity,
        CurrentSprite(crate::state::PLAYER_START_SPRITE_ID.into()),
    );
    entity
}

fn physics_frame(world: &mut World) {
    player_input(world);
    physics_step(world);
    ground_detection(world);
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .flush();
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .begin_frame();
}

#[test]
fn real_map_dimensions_spawn_and_fall_reset() {
    let mut world = real_level_world();
    let map = world
        .get_resource::<TilemapRegistry>()
        .unwrap()
        .get("ex10_level")
        .unwrap();
    assert_eq!(
        (map.width, map.height, map.tile_width, map.tile_height),
        (184, 50, 64, 64)
    );
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    for _ in 0..60 {
        physics_frame(&mut world);
    }
    assert!(world.get::<Player>(player).unwrap().grounded);
    assert!((world.get::<Position>(player).unwrap().0 - PLAYER_SPAWN).length() < 1.0);
    world.get_mut::<Position>(player).unwrap().0.y = crate::state::KILL_Y + 1.0;
    world
        .get_mut::<crate::state::PlayerPresentation>(player)
        .unwrap()
        .pending_effect = Some(crate::state::PlayerEffect::Land);
    despawn_out_of_bounds(&mut world);
    assert_eq!(world.get::<Position>(player).unwrap().0, PLAYER_SPAWN);
    assert!(!world.get::<Player>(player).unwrap().grounded);
    let p = world
        .get::<crate::state::PlayerPresentation>(player)
        .unwrap();
    assert!(p.suppress_landing && p.pending_effect.is_none());
}

fn route_jump_possible(
    from: &crate::level_layout::RoutePlatform,
    to: &crate::level_layout::RoutePlatform,
    dt: f32,
) -> bool {
    // Search launch points on the authored platform, using ordinary input and
    // real map collision. No teleport, impulse tweak or collider substitution
    // occurs during any tested traversal segment.
    let goals = [
        to.left + 0.75,
        to.right - 0.75,
        f32::midpoint(to.left, to.right),
    ];
    let mut starts: Vec<_> = (0..((from.right - from.left) * 2.0) as u32)
        .map(|i| from.left + 0.35 + i as f32 * 0.5)
        .collect();
    starts.sort_by(|a, b| {
        let midpoint = f32::midpoint(to.left, to.right);
        (a - midpoint).abs().total_cmp(&(b - midpoint).abs())
    });
    for start in starts {
        for goal in goals {
            for jump in [true, false] {
                let mut world = real_level_world();
                world.get_resource_mut::<DeltaTime>().unwrap().dt = dt;
                let player = spawn_test_player(
                    &mut world,
                    Vec2::new(start * TILE, from.row * TILE - PLAYER_HALF.y - 0.5),
                );
                for _ in 0..4 {
                    physics_frame(&mut world);
                }
                let initial = world.get::<Position>(player).unwrap().0;
                if !world.get::<Player>(player).unwrap().grounded
                    || (initial.y + PLAYER_HALF.y - from.row * TILE).abs() > 1.0
                {
                    continue;
                }
                for tick in 0..(2.5 / dt) as usize {
                    let position = world.get::<Position>(player).unwrap().0;
                    let dx = goal * TILE - position.x;
                    let input = world.get_resource_mut::<InputState>().unwrap();
                    input.key_up(KeyCode::KeyD);
                    input.key_up(KeyCode::KeyA);
                    input.key_up(KeyCode::Space);
                    if dx.abs() > crate::state::PLAYER_MOVE_SPEED * dt * 0.6 {
                        input.key_down(if dx > 0.0 {
                            KeyCode::KeyD
                        } else {
                            KeyCode::KeyA
                        });
                    }
                    if tick == 0 && jump {
                        input.key_down(KeyCode::Space);
                    }
                    physics_frame(&mut world);
                    let position = world.get::<Position>(player).unwrap().0;
                    if crate::level_layout::HAZARDS
                        .iter()
                        .filter(|h| !h.fire)
                        .any(|h| {
                            let delta =
                                (position - Vec2::from_array(h.motion.position) * TILE).abs();
                            delta.cmplt(PLAYER_HALF + Vec2::new(27.0, 14.0)).all()
                        })
                    {
                        break;
                    }
                    if tick > 3
                        && world.get::<Player>(player).unwrap().grounded
                        && (position.y + PLAYER_HALF.y - to.row * TILE).abs() < 1.0
                        && position.x >= to.left * TILE + PLAYER_HALF.x
                        && position.x <= to.right * TILE - PLAYER_HALF.x
                    {
                        return true;
                    }
                    if position.y > crate::state::KILL_Y {
                        break;
                    }
                }
            }
        }
    }
    false
}

#[test]
fn authored_routes_and_recovery_shelves_traverse_with_real_physics() {
    use crate::level_layout::{PLATFORMS, ROUTES};
    let mut tested = std::collections::HashSet::new();
    for &(route, names) in ROUTES {
        for pair in names.windows(2) {
            if !tested.insert((pair[0], pair[1])) {
                continue;
            }
            let from = PLATFORMS.iter().find(|p| p.name == pair[0]).unwrap();
            let to = PLATFORMS.iter().find(|p| p.name == pair[1]).unwrap();
            assert!(
                route_jump_possible(from, to, 1.0 / 60.0),
                "{route}: {} -> {} is not traversable",
                from.name,
                to.name
            );
        }
    }
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

fn load_presentation_assets(world: &mut World) {
    use tungsten::core::{
        AnimationData, AnimationRegistry, ParticleActive, ParticleBudget, ParticleConfig,
        ParticleConfigRegistry,
    };
    let mut animations = AnimationRegistry::new();
    for name in [
        "player_idle",
        "player_walk",
        "player_jump",
        "player_fall",
        "player_land",
        "ball_spin",
        "ball_small_spin",
        "torch_flicker",
        "waterfall_flow",
        "vines_sway",
        "fireball",
    ] {
        animations.insert(
            format!("ex10_{name}"),
            AnimationData::load(asset_path(&format!("animations/{name}.json"))).unwrap(),
        );
    }
    world.insert_resource(animations);
    let mut particles = ParticleConfigRegistry::new();
    for name in [
        "jump_puff",
        "double_jump",
        "landing_dust",
        "torch_embers",
        "waterfall_spray",
        "wind_motes",
        "black_hole",
        "ball_explosion",
        "small_ball_impact",
        "fire_trail",
        "ball_burn",
        "ball_burn_embers",
    ] {
        let path = asset_path(&format!("particles/{name}.json"));
        let config = ParticleConfig::load(&path).unwrap();
        particles.register(format!("ex10_{name}"), path, config);
    }
    world.insert_resource(particles);
    world.insert_resource(ParticleActive::default());
    world.insert_resource(ParticleBudget { global_cap: 2048 });
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(crate::state::EffectSequence::default());
}

fn particle_frame(world: &mut World) {
    use tungsten::particles::{
        particle_count_refresh_system, particle_emit_system, particle_tick_system,
    };
    crate::systems::transient_emitter_cleanup(world);
    particle_count_refresh_system(world);
    particle_emit_system(world);
    particle_tick_system(world);
    let commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(commands);
    world.insert_resource(CommandBuffer::new());
}

#[test]
fn animation_transitions_hold_finish_face_and_interrupt_landing() {
    use crate::state::PlayerPresentation;
    use crate::systems::{animation_system, player_presentation_system};
    let mut world = real_level_world();
    load_presentation_assets(&mut world);
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    for _ in 0..5 {
        physics_frame(&mut world);
        player_presentation_system(&mut world);
        animation_system(&mut world);
    }
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_idle"
    );
    assert_eq!(
        world.query::<crate::state::TransientEmitter>().count(),
        0,
        "spawn settles silently"
    );
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::KeyA);
    physics_frame(&mut world);
    player_presentation_system(&mut world);
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_walk"
    );
    assert!(world.get::<PlayerPresentation>(player).unwrap().facing_left);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_up(KeyCode::KeyA);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::Space);
    physics_frame(&mut world);
    player_presentation_system(&mut world);
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_jump"
    );
    assert!(
        !world.get::<Player>(player).unwrap().grounded,
        "previous floor contact must not ground the launch"
    );
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_up(KeyCode::Space);
    for _ in 0..13 {
        physics_frame(&mut world);
        player_presentation_system(&mut world);
        animation_system(&mut world);
    }
    assert!(
        world.get::<AnimationState>(player).unwrap().finished,
        "non-looping rise holds last pose"
    );
    let mut saw_fall = false;
    let mut saw_land = false;
    for _ in 0..65 {
        physics_frame(&mut world);
        player_presentation_system(&mut world);
        animation_system(&mut world);
        let clip = world
            .get::<AnimationState>(player)
            .unwrap()
            .animation_id
            .as_str();
        saw_fall |= clip == "ex10_player_fall";
        if clip == "ex10_player_land" {
            saw_land = true;
            break;
        }
    }
    assert!(saw_fall && saw_land);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::Space);
    physics_frame(&mut world);
    player_presentation_system(&mut world);
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_jump"
    );
    assert_eq!(
        world
            .get::<PlayerPresentation>(player)
            .unwrap()
            .landing_lock,
        0.0
    );
}

#[test]
fn jump_landing_bursts_fire_once_stay_at_event_and_cleanup() {
    use crate::state::TransientEmitter;
    let mut world = real_level_world();
    load_presentation_assets(&mut world);
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    for _ in 0..5 {
        physics_frame(&mut world);
        crate::systems::player_presentation_system(&mut world);
    }
    let takeoff = world.get::<Position>(player).unwrap().0 + Vec2::new(0.0, PLAYER_HALF.y);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::Space);
    physics_frame(&mut world);
    crate::systems::player_presentation_system(&mut world);
    let emitter = world.query::<TransientEmitter>().next().unwrap().0;
    let origin = world.get::<Transform>(emitter).unwrap().position;
    assert_eq!(
        origin, takeoff,
        "jump puff belongs at the pre-physics feet position"
    );
    assert_eq!(world.query::<TransientEmitter>().count(), 1);
    crate::systems::transient_emitter_cleanup(&mut world);
    assert!(
        world.get::<TransientEmitter>(emitter).is_some(),
        "must survive first emission tick"
    );
    particle_frame(&mut world);
    assert_eq!(world.query::<tungsten::core::Particle>().count(), 8);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_up(KeyCode::Space);
    let mut landing_emitters = std::collections::HashSet::new();
    for _ in 0..100 {
        physics_frame(&mut world);
        crate::systems::player_presentation_system(&mut world);
        for (entity, _) in world.query::<TransientEmitter>() {
            if entity != emitter {
                landing_emitters.insert(entity);
            }
        }
        if let Some(t) = world.get::<Transform>(emitter) {
            assert_eq!(t.position, origin);
        }
        particle_frame(&mut world);
    }
    assert!(world.get::<Player>(player).unwrap().grounded);
    assert_eq!(landing_emitters.len(), 1);
    assert_eq!(world.query::<TransientEmitter>().count(), 0);
    assert_eq!(world.query::<tungsten::core::Particle>().count(), 0);
}

#[test]
fn particle_caps_and_ambient_placements_remain_bounded() {
    use tungsten::core::{
        Particle, ParticleBudget, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState,
    };
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let registry = world.get_resource::<ParticleConfigRegistry>().unwrap();
    let ambient_max: u32 = crate::level_layout::EMITTERS
        .iter()
        .map(|p| {
            registry
                .get(registry.id_for_name(p.config).unwrap())
                .unwrap()
                .max_alive
        })
        .sum();
    assert!(ambient_max <= 256);
    assert_eq!(
        world.query::<crate::state::AmbientEmitter>().count(),
        crate::level_layout::EMITTERS.len()
    );
    for _ in 0..30 {
        crate::systems::spawn_transient_effect(&mut world, "ex10_landing_dust", Vec2::ZERO);
    }
    assert_eq!(
        world.query::<crate::state::TransientEmitter>().count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
    let config = world
        .get_resource::<ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_black_hole")
        .unwrap();
    for i in 0..3 {
        let emitter = world.spawn();
        world.insert(emitter, Transform::default());
        world.insert(emitter, ParticleEmitter::with_seed(config, i));
        world.insert(emitter, ParticleEmitterState::default());
    }
    for _ in 0..240 {
        particle_frame(&mut world);
        assert!(
            world.query::<Particle>().count()
                <= world.get_resource::<ParticleBudget>().unwrap().global_cap as usize
        );
    }
    assert_eq!(world.query::<crate::state::TransientEmitter>().count(), 0);
}

fn mock_sprite(assets: &mut tungsten::core::AssetRegistry, name: &str, handle: u32, lit: bool) {
    use tungsten::core::assets::UvRect;
    use tungsten::core::{FilterMode, TextureHandle};
    assets.register_sprite(
        name.into(),
        FilterMode::Nearest,
        64,
        64,
        std::path::PathBuf::new(),
        TextureHandle(handle),
        UvRect::FULL,
        None,
        None,
        lit.then_some(TextureHandle(handle)),
    );
}

#[test]
fn extraction_keeps_tile_stages_props_actors_foreground_and_cursor_in_order() {
    use crate::state::AnimatedProp;
    use tungsten::core::{AssetRegistry, InputState};
    let mut world = seed_world();
    let mut assets = AssetRegistry::new();
    for (id, name) in [
        "back",
        "decoration",
        "terrain",
        "foreground",
        "actor",
        "prop_back",
        "prop_world",
        "ex10_cursor",
    ]
    .iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, id as u32 + 1, false);
    }
    world.insert_resource(assets);
    let layers = [
        ("background", 0),
        ("decorations", 1),
        ("terrain", 2),
        ("foreground", 3),
        ("collision", 2),
    ]
    .into_iter()
    .map(|(name, tile)| TilemapLayer {
        name: name.into(),
        kind: if name == "collision" {
            LayerKind::Collision
        } else {
            LayerKind::Render
        },
        tiles: vec![tile],
    })
    .collect();
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "test".into(),
        TilemapData {
            width: 1,
            height: 1,
            tile_width: 64,
            tile_height: 64,
            tileset: ["back", "decoration", "terrain", "foreground"]
                .map(str::to_owned)
                .into(),
            layers,
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("test", Vec2::ZERO));
    let player = spawn_test_player(&mut world, Vec2::new(32.0, 32.0));
    world.insert(player, CurrentSprite("actor".into()));
    for (name, depth) in [
        ("prop_back", crate::level_layout::PropDepth::Back),
        ("prop_world", crate::level_layout::PropDepth::World),
    ] {
        let entity = world.spawn();
        world.insert(entity, AnimatedProp(depth));
        world.insert(entity, Transform::default());
        world.insert(entity, CurrentSprite(name.into()));
    }
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .update_cursor_position(10.0, 10.0);
    let extracted = crate::extract::extract_sprites(&world);
    let handles: Vec<_> = extracted.iter().map(|b| b.texture.0).collect();
    assert_eq!(handles, vec![1, 2, 6, 3, 7, 5, 4, 8]);
    // Moving the view culls all offscreen tile layers and props.
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::splat(1000.0);
    assert!(
        crate::extract::extract_tile_layers(
            &world,
            &[
                "background",
                "decorations",
                "terrain",
                "foreground",
                "collision"
            ]
        )
        .is_empty()
    );
}

#[test]
fn all_player_orb_frames_keep_lighting_material_facing_and_bottom_anchor() {
    use crate::state::{LightingFixture, LightingFixtureMode, PlayerMaterial, PlayerPresentation};
    use tungsten::core::{AnimationRegistry, AssetRegistry, MaterialAssetId, UniformOverrideBlock};
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let mut assets = AssetRegistry::new();
    let manifest =
        tungsten::core::assets::manifest::ResolvedManifest::load(asset_path("manifest.json"))
            .unwrap();
    let frames: Vec<_> = world
        .get_resource::<AnimationRegistry>()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.starts_with("ex10_player") || *name == "ex10_ball_spin")
        .flat_map(|(_, clip)| clip.frames.iter().map(|f| f.sprite.clone()))
        .collect();
    for (i, name) in frames.iter().enumerate() {
        let sprite = manifest.sprites.get(name).unwrap();
        assert!(
            sprite.normal_path.is_some() && sprite.emissive_path.is_some(),
            "{name}"
        );
        mock_sprite(&mut assets, name, i as u32, true);
    }
    world.insert_resource(assets);
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    world.insert(
        player,
        PlayerMaterial {
            material_id: MaterialAssetId(0),
        },
    );
    world.insert(player, UniformOverrideBlock::default());
    world.get_mut::<Transform>(player).unwrap().scale = Vec2::new(1.16, 0.86);
    for name in frames.iter().filter(|name| name.starts_with("ex10_player")) {
        world.insert(player, CurrentSprite(name.clone()));
        for mode in [LightingFixtureMode::On, LightingFixtureMode::Off] {
            world.insert_resource(LightingFixture { mode });
            for left in [false, true] {
                world
                    .get_mut::<PlayerPresentation>(player)
                    .unwrap()
                    .facing_left = left;
                let batches = crate::extract::extract_sprites(&world);
                assert_eq!(batches.len(), 1);
                let batch = &batches[0];
                let sprite = &batch.instances[0];
                assert_eq!(batch.lit, mode == LightingFixtureMode::On);
                assert_eq!(
                    batch.material_id.is_some(),
                    mode == LightingFixtureMode::Off
                );
                assert_eq!(sprite.uv_size[0] < 0.0, left);
                assert!((sprite.position[1] + 61.0 * 0.86 - 128.0).abs() < 0.001);
            }
        }
    }
    world.despawn(player);
    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(ball, Position(Vec2::new(200.0, 200.0)));
    world.insert_resource(LightingFixture {
        mode: LightingFixtureMode::On,
    });
    for name in frames.iter().filter(|name| name.starts_with("ex10_ball")) {
        world.insert(ball, CurrentSprite(name.clone()));
        let batches = crate::extract::extract_sprites(&world);
        assert!(batches[0].lit);
        assert_eq!(batches[0].instances[0].position, [184.0, 184.0]);
    }
    // A ball outside the view is not extracted.
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::splat(1000.0);
    assert!(crate::extract::extract_sprites(&world).is_empty());
}

#[test]
fn parallax_covers_camera_extremes_resizes_zoom_and_shake() {
    use tungsten::core::AssetRegistry;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let mut assets = AssetRegistry::new();
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

#[test]
fn landing_clip_completes_without_resetting_idle_and_props_stay_synchronized() {
    use crate::systems::{animation_system, player_presentation_system};
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    world.get_mut::<Player>(player).unwrap().grounded = true;
    world
        .get_mut::<crate::state::PlayerPresentation>(player)
        .unwrap()
        .pending_effect = Some(crate::state::PlayerEffect::Land);
    player_presentation_system(&mut world);
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_land"
    );
    for _ in 0..20 {
        player_presentation_system(&mut world);
        animation_system(&mut world);
    }
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_idle"
    );
    let before = world.get::<AnimationState>(player).unwrap().accumulated_ms;
    player_presentation_system(&mut world);
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().accumulated_ms,
        before
    );
    let waterfalls: Vec<_> = world
        .query::<AnimationState>()
        .filter(|(_, a)| a.animation_id == "ex10_waterfall_flow")
        .collect();
    assert_eq!(waterfalls.len(), 48);
    let first = waterfalls[0].1;
    assert!(waterfalls.iter().all(
        |(_, a)| a.frame_index == first.frame_index && a.accumulated_ms == first.accumulated_ms
    ));
}

#[test]
fn audio_controls_and_damage_flash_shake_remain_wired() {
    use tungsten::core::{MaterialAssetId, ShakeEvent, Tween, UniformOverrideBlock};
    let mut world = seed_world();
    world.insert_resource(AudioCommands::new());
    world.insert_resource(AudioState {
        sfx_handle: AudioHandle(1),
        black_hole_sfx_handle: AudioHandle(2),
        music_handle: AudioHandle(3),
        sfx_volume: 1.0,
        black_hole_sfx_volume: 0.65,
        music_volume: 0.4,
        music_playing: false,
        master_volume: 0.5,
    });
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::KeyM);
    crate::systems::audio_input_system(&mut world);
    assert!(world.get_resource::<AudioState>().unwrap().music_playing);
    assert!(matches!(
        world.get_resource_mut::<AudioCommands>().unwrap().drain()[0],
        AudioCommand::Play { looping: true, .. }
    ));
    let input = world.get_resource_mut::<InputState>().unwrap();
    input.begin_frame();
    input.key_up(KeyCode::KeyM);
    input.key_down(KeyCode::KeyS);
    crate::systems::audio_input_system(&mut world);
    assert!(!world.get_resource::<AudioState>().unwrap().music_playing);
    world.insert_resource(EventQueue::<ShakeEvent>::new());
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    world.insert(
        player,
        crate::state::PlayerMaterial {
            material_id: MaterialAssetId(0),
        },
    );
    world.insert(player, UniformOverrideBlock::default());
    let ball = world.spawn();
    world.insert(ball, Ball);
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .send(CollisionEvent {
            a: player,
            b: Some(ball),
            normal: Vec2::X,
            penetration: 1.0,
        });
    world.insert(player, crate::gameplay::Health::default());
    crate::gameplay::hazard_contacts(&mut world);
    assert_eq!(
        world.get::<crate::gameplay::Health>(player).unwrap().hearts,
        3
    );
    assert!(world.get::<Tween>(player).is_none());
    assert!(
        world
            .get_resource::<EventQueue<ShakeEvent>>()
            .unwrap()
            .is_empty()
    );
    crate::gameplay::damage_player(&mut world, player, PLAYER_SPAWN + Vec2::X);
    assert!(world.get::<Tween>(player).is_some());
    assert!(
        !world
            .get_resource::<EventQueue<ShakeEvent>>()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn hazards_damage_once_per_cooldown_and_restore_health_on_respawn() {
    use crate::gameplay::*;
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    let spike = world.spawn();
    world.insert(spike, Hazard { fire: false });
    world.insert(spike, Position(Vec2::new(100.0, 100.0)));
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    assert!(world.get::<Velocity>(player).unwrap().0.y < 0.0);
    for _ in 0..10 {
        hazard_contacts(&mut world);
    }
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    world.get_mut::<Health>(player).unwrap().immunity = 0.0;
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 1);
    world.get_mut::<Health>(player).unwrap().immunity = 0.0;
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 3);
    assert_eq!(world.get::<Position>(player).unwrap().0, PLAYER_SPAWN);
    assert_eq!(world.get::<Velocity>(player).unwrap().0, Vec2::ZERO);
    assert!(world.get::<Health>(player).unwrap().immunity > 1.0);
}

#[test]
fn moving_fire_sweeps_fast_balls_once_and_explosions_do_not_hurt_players() {
    use crate::gameplay::*;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 200.0));
    world.insert(player, Health::default());
    for _ in 0..2 {
        let fire = world.spawn();
        world.insert(fire, Hazard { fire: true });
        world.insert(fire, Position(Vec2::new(100.0, 100.0)));
        world.insert(fire, PreviousPosition(Vec2::new(100.0, 300.0)));
    }
    // Both the ball and moving fire cross between frames.
    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(ball, PreviousPosition(Vec2::new(0.0, 200.0)));
    world.insert(ball, Position(Vec2::new(200.0, 200.0)));
    hazard_contacts(&mut world);
    assert!(world.get::<Ball>(ball).is_none());
    assert_eq!(world.query::<Explosion>().count(), 1);
    assert_eq!(world.query::<crate::state::TransientEmitter>().count(), 1);
    // The crossing fire hit the player once. Neither overlapping fire nor the
    // ball burst can add another hit during immunity.
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    for e in world.query_entities::<Hazard>() {
        world.despawn(e);
    }
    world.get_mut::<Health>(player).unwrap().immunity = 0.0;
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    for _ in 0..150 {
        move_obstacles(&mut world);
        particle_frame(&mut world);
    }
    assert_eq!(world.query::<Explosion>().count(), 0);
    assert_eq!(world.query::<crate::state::TransientEmitter>().count(), 0);
}

#[test]
fn explosion_bursts_and_rings_stay_bounded_under_dense_ball_contact() {
    use crate::gameplay::*;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let fire = world.spawn();
    world.insert(fire, Hazard { fire: true });
    world.insert(fire, Position(Vec2::ZERO));
    for _ in 0..100 {
        let ball = world.spawn();
        world.insert(ball, Ball);
        world.insert(ball, Position(Vec2::ZERO));
    }
    hazard_contacts(&mut world);
    assert_eq!(world.query::<Ball>().count(), 0);
    assert_eq!(
        world.query::<Explosion>().count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
    assert_eq!(
        world.query::<crate::state::TransientEmitter>().count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
}

#[test]
fn prescribed_platforms_carry_supported_players_and_release_jumps() {
    use crate::gameplay::*;
    let mut world = seed_world();
    spawn_obstacles(&mut world);
    let (platform, half) = world
        .query::<MovingPlatform>()
        .next()
        .map(|(e, p)| (e, p.half_width))
        .unwrap();
    assert!(half > PLAYER_HALF.x);
    let original = world.get::<Position>(platform).unwrap().0;
    let player = spawn_test_player(
        &mut world,
        original - Vec2::new(0.0, crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y),
    );
    move_obstacles(&mut world);
    let delta = world.get::<Position>(platform).unwrap().0 - original;
    assert!(delta.length() > 0.0);
    assert!(
        (world.get::<Position>(player).unwrap().0
            - (original + delta
                - Vec2::new(0.0, crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y)))
        .length()
            < 0.01
    );
    let before = world.get::<Position>(player).unwrap().0;
    world.get_mut::<Velocity>(player).unwrap().0.y = -crate::state::PLAYER_JUMP_IMPULSE;
    move_obstacles(&mut world);
    assert_eq!(world.get::<Position>(player).unwrap().0, before);
}

#[test]
fn zoom_input_reaches_and_clamps_expanded_range() {
    let mut world = seed_world();
    for (key, expected) in [(KeyCode::Minus, 0.35), (KeyCode::Equal, 3.0)] {
        for _ in 0..30 {
            let input = world.get_resource_mut::<InputState>().unwrap();
            input.key_up(key);
            input.begin_frame();
            input.key_down(key);
            crate::systems::camera_zoom_input_system(&mut world);
        }
        assert!(
            (world
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
fn platforms_support_riders_through_a_complete_cycle_with_real_physics() {
    use crate::gameplay::*;
    let mut world = seed_world();
    spawn_obstacles(&mut world);
    let platform = world.query::<MovingPlatform>().next().unwrap().0;
    let start = world.get::<Position>(platform).unwrap().0;
    let player = spawn_test_player(
        &mut world,
        start - Vec2::new(0.0, crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y),
    );
    for _ in 0..400 {
        player_input(&mut world);
        move_obstacles(&mut world);
        physics_step(&mut world);
        ground_detection(&mut world);
        let feet = world.get::<Position>(player).unwrap().0.y + PLAYER_HALF.y;
        let deck =
            world.get::<Position>(platform).unwrap().0.y - crate::level_layout::DECK_DEPTH * 0.5;
        assert!(
            (feet - deck).abs() < 1.0,
            "rider separated from deck: {feet} vs {deck}"
        );
    }
}

#[test]
fn every_platform_sweep_clears_terrain_and_player_headroom() {
    let world = real_level_world();
    let map = world
        .get_resource::<TilemapRegistry>()
        .unwrap()
        .get("ex10_level")
        .unwrap();
    let collision = map
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::Collision)
        .unwrap();
    for placement in crate::level_layout::MOVING_PLATFORMS {
        for step in 0..200 {
            let m = placement.motion;
            let pos = (Vec2::from_array(m.position)
                + Vec2::from_array(m.travel) * (step as f32 / 200.0 * std::f32::consts::TAU).sin())
                * TILE;
            let min = pos
                - Vec2::new(
                    placement.half_width * TILE,
                    crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y * 2.0,
                );
            let max = pos
                + Vec2::new(
                    placement.half_width * TILE,
                    crate::level_layout::DECK_DEPTH * 0.5,
                );
            for row in (min.y / TILE).floor() as u32..(max.y / TILE).ceil() as u32 {
                for col in (min.x / TILE).floor() as u32..(max.x / TILE).ceil() as u32 {
                    assert!(
                        collision.tiles[(row * map.width + col) as usize] < 0,
                        "platform at {pos:?} intersects tile {col},{row}"
                    );
                }
            }
        }
    }
}

#[test]
fn cloud_parallax_and_vortex_instances_animate_without_world_mutation() {
    use tungsten::core::AssetRegistry;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let mut assets = AssetRegistry::new();
    for (i, name) in [
        "ex10_clouds_far",
        "ex10_vortex",
        "ex10_vortex_core",
        "ex10_spark",
    ]
    .iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, i as u32, false);
    }
    world.insert_resource(assets);
    let hole = world.spawn();
    world.insert(hole, BlackHole { remaining: 2.0 });
    world.insert(hole, Position(Vec2::new(300.0, 200.0)));
    world.insert_resource(crate::gameplay::SceneTime(0.0));
    let first = crate::extract::extract_sprites(&world);
    world
        .get_resource_mut::<crate::gameplay::SceneTime>()
        .unwrap()
        .0 = 0.25;
    let second = crate::extract::extract_sprites(&world);
    assert_ne!(
        first[0].instances[0].position,
        second[0].instances[0].position
    );
    assert!(
        second
            .iter()
            .flat_map(|b| &b.instances)
            .any(|i| i.rotation.abs() > 0.5)
    );
    assert_eq!(
        world.get::<Position>(hole).unwrap().0,
        Vec2::new(300.0, 200.0)
    );
    assert_eq!(world.query::<BlackHole>().count(), 1);
}

#[test]
fn thin_platform_underside_and_edges_match_visible_art() {
    let mut world = real_level_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::ZERO;
    // Below the transparent portion of the bridge tile, horizontal movement
    // must stay unobstructed. This used to collide with a full 64px tile.
    let player = spawn_test_player(&mut world, Vec2::new(23.5 * TILE, 30.0 * TILE + 56.0));
    world.get_mut::<Velocity>(player).unwrap().0 = Vec2::new(200.0, 0.0);
    for _ in 0..45 {
        physics_step(&mut world);
    }
    assert!(world.get::<Position>(player).unwrap().0.x > 25.5 * TILE);
    assert!((world.get::<Position>(player).unwrap().0.y - (30.0 * TILE + 56.0)).abs() < 0.5);
    // A rising player contacts the visible underside at row*64+23, not +64.
    world.insert(player, Position(Vec2::new(27.0 * TILE, 31.0 * TILE + 30.0)));
    world.insert(player, Velocity(Vec2::new(0.0, -240.0)));
    for _ in 0..40 {
        physics_step(&mut world);
    }
    let head = world.get::<Position>(player).unwrap().0.y - PLAYER_HALF.y;
    assert!(
        (head - (30.0 * TILE + crate::level_layout::DECK_DEPTH)).abs() < 1.0,
        "head stopped at {head}"
    );
}

#[test]
fn player_lantern_toggle_controls_halo_and_native_light_and_tracks_facing() {
    use crate::gameplay::*;
    use tungsten::core::{AssetRegistry, Light};
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    spawn_obstacles(&mut world);
    scene_effects(&mut world);
    let center = glow_center(&world, player, Vec2::ZERO).unwrap();
    assert!(center.x > PLAYER_SPAWN.x && center.y < PLAYER_SPAWN.y + PLAYER_HALF.y);
    world
        .get_mut::<crate::state::PlayerPresentation>(player)
        .unwrap()
        .facing_left = true;
    let left = glow_center(&world, player, Vec2::ZERO).unwrap();
    assert!((left.x + center.x - 2.0 * PLAYER_SPAWN.x).abs() < 0.001);
    world.get_resource_mut::<CameraState>().unwrap().position =
        PLAYER_SPAWN - Vec2::new(200.0, 150.0);
    let mut assets = AssetRegistry::new();
    mock_sprite(&mut assets, "ex10_halo", 901, false);
    mock_sprite(&mut assets, "ex10_flame_glow", 902, false);
    world.insert_resource(assets);
    let on = crate::extract::extract_sprites(&world)
        .iter()
        .map(|b| b.instances.len())
        .sum::<usize>();
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::KeyL);
    lantern_input(&mut world);
    scene_effects(&mut world);
    assert!(glow_center(&world, player, Vec2::ZERO).is_none());
    assert_eq!(
        world
            .query::<Light>()
            .filter(|(_, l)| l.intensity == 0.0)
            .count(),
        1
    );
    let off = crate::extract::extract_sprites(&world)
        .iter()
        .map(|b| b.instances.len())
        .sum::<usize>();
    assert_eq!(on - off, 2);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .begin_frame();
    lantern_input(&mut world);
    assert!(
        !world.get::<PlayerLantern>(player).unwrap().enabled,
        "held key must not flicker"
    );
    let input = world.get_resource_mut::<InputState>().unwrap();
    input.key_up(KeyCode::KeyL);
    input.key_down(KeyCode::KeyL);
    lantern_input(&mut world);
    scene_effects(&mut world);
    assert!(world.get::<PlayerLantern>(player).unwrap().enabled);
    assert!(world.query::<Light>().all(|(_, l)| l.intensity > 0.0));
}

#[test]
fn moving_fire_emits_a_bounded_particle_trail() {
    use crate::gameplay::*;
    use tungsten::core::{Particle, ParticleEmitter, ParticleEmitterState};
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    spawn_obstacles(&mut world);
    let flames: Vec<_> = world
        .query::<Hazard>()
        .filter(|(_, h)| h.fire)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(flames.len(), 6);
    assert!(
        flames
            .iter()
            .all(|e| world.get::<ParticleEmitter>(*e).is_some())
    );
    for _ in 0..120 {
        move_obstacles(&mut world);
        sync_position_to_transform(&mut world);
        particle_frame(&mut world);
    }
    assert!(world.query::<Particle>().count() > 60);
    assert!(world.query::<Particle>().count() <= 240);
    for e in flames {
        assert!(world.get::<ParticleEmitterState>(e).unwrap().active_count <= 40);
    }
}

#[test]
fn double_jump_needs_a_fresh_press_emits_once_and_refills_on_landing() {
    use crate::state::{PlayerEffect, PlayerPresentation};
    use crate::systems::player_presentation_system;
    use tungsten::core::{ParticleConfigRegistry, ParticleEmitter};
    let mut world = real_level_world();
    load_presentation_assets(&mut world);
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    for _ in 0..10 {
        physics_frame(&mut world);
    }
    assert!(world.get::<Player>(player).unwrap().grounded);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::Space);
    physics_frame(&mut world);
    player_presentation_system(&mut world);
    assert!(!world.get::<Player>(player).unwrap().air_jump_used);
    for _ in 0..10 {
        physics_frame(&mut world);
        player_presentation_system(&mut world);
    }
    assert!(
        !world.get::<Player>(player).unwrap().air_jump_used,
        "holding space consumed the second jump"
    );
    let input = world.get_resource_mut::<InputState>().unwrap();
    input.key_up(KeyCode::Space);
    input.key_down(KeyCode::Space);
    let origin = world.get::<Position>(player).unwrap().0 + Vec2::new(0.0, PLAYER_HALF.y);
    player_input(&mut world);
    assert!(world.get::<Player>(player).unwrap().air_jump_used);
    assert_eq!(
        world
            .get::<PlayerPresentation>(player)
            .unwrap()
            .pending_effect,
        Some(PlayerEffect::DoubleJump)
    );
    assert_eq!(
        world.get::<Velocity>(player).unwrap().0.y,
        -crate::state::PLAYER_JUMP_IMPULSE
    );
    player_presentation_system(&mut world);
    let config = world
        .get_resource::<ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_double_jump")
        .unwrap();
    let bursts: Vec<_> = world
        .query::<ParticleEmitter>()
        .filter(|(_, p)| p.config == config)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(bursts.len(), 1);
    assert_eq!(world.get::<Transform>(bursts[0]).unwrap().position, origin);
    // A third fresh press must neither boost velocity nor emit another burst.
    world.get_mut::<Velocity>(player).unwrap().0.y = -300.0;
    let input = world.get_resource_mut::<InputState>().unwrap();
    input.begin_frame();
    input.key_up(KeyCode::Space);
    input.key_down(KeyCode::Space);
    player_input(&mut world);
    player_presentation_system(&mut world);
    assert_eq!(world.get::<Velocity>(player).unwrap().0.y, -300.0);
    assert_eq!(
        world
            .query::<ParticleEmitter>()
            .filter(|(_, p)| p.config == config)
            .count(),
        1
    );
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_up(KeyCode::Space);
    for _ in 0..150 {
        physics_frame(&mut world);
        player_presentation_system(&mut world);
    }
    assert!(world.get::<Player>(player).unwrap().grounded);
    assert!(!world.get::<Player>(player).unwrap().air_jump_used);
    world.get_mut::<Player>(player).unwrap().air_jump_used = true;
    crate::systems::respawn_player(&mut world, player);
    assert!(!world.get::<Player>(player).unwrap().air_jump_used);
}

#[test]
fn hearts_reflect_every_hp_value_immediately_at_constant_screen_size() {
    use tungsten::core::AssetRegistry;
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    world.insert(player, crate::gameplay::Health::default());
    let mut assets = AssetRegistry::new();
    mock_sprite(&mut assets, "ex10_heart_full", 801, false);
    mock_sprite(&mut assets, "ex10_heart_empty", 802, false);
    world.insert_resource(assets);
    for zoom in [0.35, 1.0, 3.0] {
        world.get_resource_mut::<CameraState>().unwrap().zoom = zoom;
        world.get_resource_mut::<CameraState>().unwrap().position = Vec2::new(800.0, 1500.0);
        for hp in 0..=3 {
            world
                .get_mut::<crate::gameplay::Health>(player)
                .unwrap()
                .hearts = hp;
            let batches = crate::extract::extract_sprites(&world);
            assert_eq!(batches.iter().map(|b| b.instances.len()).sum::<usize>(), 3);
            assert_eq!(
                batches
                    .iter()
                    .filter(|b| b.texture.0 == 801)
                    .map(|b| b.instances.len())
                    .sum::<usize>(),
                hp as usize
            );
            for (index, heart) in batches.iter().flat_map(|b| &b.instances).enumerate() {
                assert!((heart.size[0] * zoom - 40.0).abs() < 0.001);
                let screen = (Vec2::from_array(heart.position) - Vec2::new(800.0, 1500.0)) * zoom;
                assert!((screen - Vec2::new(16.0 + index as f32 * 40.0, 86.0)).length() < 0.001);
            }
        }
    }
}

#[test]
fn midnight_uses_restrained_stock_post_passes_and_round_moon_at_all_aspects() {
    use tungsten::core::{
        AssetRegistry,
        post::{PostPass, PostStack},
    };
    let mut app = App::new(Config::default()).unwrap();
    crate::setup::configure_app(&mut app);
    let stack = app.world_mut().get_resource::<PostStack>().unwrap();
    assert!(
        matches!(stack.0.as_slice(),[PostPass::Bloom(b),PostPass::Vignette(v)] if b.intensity<=0.3 && v.strength<=0.2)
    );
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let mut assets = AssetRegistry::new();
    mock_sprite(&mut assets, "ex10_sky", 700, false);
    mock_sprite(&mut assets, "ex10_moon", 701, false);
    world.insert_resource(assets);
    for (width, height) in [(1920, 1080), (800, 800), (3840, 720)] {
        world.insert_resource(WindowSize { width, height });
        let batches = crate::extract::extract_sprites(&world);
        let moon = &batches
            .iter()
            .find(|b| b.texture.0 == 701)
            .unwrap()
            .instances[0];
        assert_eq!(moon.size[0], moon.size[1]);
        assert_eq!(moon.rotation, 0.0);
    }
}

#[test]
fn aerial_jump_plays_the_tuck_clip_until_the_ascent_ends() {
    use crate::state::{PlayerEffect, PlayerPresentation};
    use crate::systems::player_presentation_system;
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    let clip = |world: &World| world.get::<PlayerPresentation>(player).unwrap().clip;
    let launch = |world: &mut World, effect| {
        world.get_mut::<Velocity>(player).unwrap().0.y = -500.0;
        world
            .get_mut::<PlayerPresentation>(player)
            .unwrap()
            .pending_effect = Some(effect);
    };
    world.get_mut::<Player>(player).unwrap().grounded = false;
    launch(&mut world, PlayerEffect::DoubleJump);
    player_presentation_system(&mut world);
    assert_eq!(clip(&world), "ex10_player_double_jump");
    player_presentation_system(&mut world);
    assert_eq!(clip(&world), "ex10_player_double_jump");
    world.get_mut::<Velocity>(player).unwrap().0.y = 40.0;
    player_presentation_system(&mut world);
    assert_eq!(clip(&world), "ex10_player_fall");
    launch(&mut world, PlayerEffect::Jump);
    player_presentation_system(&mut world);
    assert_eq!(clip(&world), "ex10_player_jump");
}

#[test]
fn fireballs_face_their_travel_and_drag_anchored_drips() {
    use crate::gameplay::{
        EmitterAnchor, FIREBALL_DRIP_OFFSET, Hazard, fireball_faces_left, motion_velocity,
        move_obstacles, spawn_obstacles,
    };
    use tungsten::core::{ParticleConfig, ParticleConfigRegistry, ParticleEmitter};
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let path = asset_path("particles/fireball_drips.json");
    let config = ParticleConfig::load(&path).unwrap();
    world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register("ex10_fireball_drips".into(), path, config);
    spawn_test_player(&mut world, PLAYER_SPAWN);
    spawn_obstacles(&mut world);
    let fires: Vec<_> = world
        .query::<Hazard>()
        .filter(|(_, h)| h.fire)
        .map(|(e, _)| e)
        .collect();
    let drips: Vec<_> = world
        .query::<EmitterAnchor>()
        .map(|(e, a)| (e, a.parent))
        .collect();
    assert_eq!(drips.len(), fires.len());
    assert!(
        drips
            .iter()
            .all(|(e, parent)| fires.contains(parent) && world.get::<ParticleEmitter>(*e).is_some())
    );
    let mut seen = vec![[false; 2]; fires.len()];
    for _ in 0..450 {
        move_obstacles(&mut world);
        for (i, fire) in fires.iter().enumerate() {
            let velocity = motion_velocity(&world, *fire).unwrap();
            let left = fireball_faces_left(&world, *fire);
            if velocity.x == 0.0 {
                // Vertical movers watch the player, who waits to their left.
                assert!(left);
            } else {
                assert_eq!(left, velocity.x < 0.0);
            }
            seen[i][usize::from(left)] = true;
        }
        for (drip, parent) in &drips {
            let expected = world.get::<Position>(*parent).unwrap().0 + FIREBALL_DRIP_OFFSET;
            assert_eq!(world.get::<Transform>(*drip).unwrap().position, expected);
        }
    }
    // Over a full period every horizontal mover turns both ways.
    for (i, fire) in fires.iter().enumerate() {
        if motion_velocity(&world, *fire).unwrap().x != 0.0 {
            assert_eq!(seen[i], [true, true]);
        }
    }
}

#[test]
fn only_small_marbles_shift_hue_while_orbs_render_untinted() {
    use crate::state::{BALL_START_SPRITE_ID, SMALL_BALL_START_SPRITE_ID, SmallBall};
    let mut world = seed_world();
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(BallSpawnState::default());
    crate::setup::platformer_bindings(&mut world);
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
        input.mouse_down(MouseButton::Middle);
    }
    world.get_resource_mut::<DeltaTime>().unwrap().dt = 0.1;
    spawn_ball_system(&mut world);
    let commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(commands);
    let balls: Vec<_> = world.query::<Ball>().map(|(e, _)| e).collect();
    let (small, orbs): (Vec<_>, Vec<_>) = balls
        .iter()
        .partition(|e| world.get::<SmallBall>(**e).is_some());
    assert!(!small.is_empty() && !orbs.is_empty());
    assert!(small.iter().all(|e| world.get::<BallHue>(*e).is_some()));
    assert!(orbs.iter().all(|e| world.get::<BallHue>(*e).is_none()));

    let mut assets = tungsten::core::AssetRegistry::new();
    mock_sprite(&mut assets, BALL_START_SPRITE_ID, 1, true);
    mock_sprite(&mut assets, SMALL_BALL_START_SPRITE_ID, 2, true);
    world.insert_resource(assets);
    let batches = crate::extract::extract_sprites(&world);
    let colors = |texture: u32| -> Vec<[u8; 4]> {
        batches
            .iter()
            .filter(|b| b.texture.0 == texture)
            .flat_map(|b| b.instances.iter().map(|i| i.color))
            .collect()
    };
    assert_eq!(colors(1).len(), orbs.len());
    assert!(colors(1).iter().all(|c| *c == [255; 4]));
    assert_eq!(colors(2).len(), small.len());
    assert!(colors(2).iter().all(|c| *c != [255; 4]));
}

#[test]
fn glows_draw_through_soft_materials_without_absorbing_other_sprites() {
    use crate::gameplay::Glow;
    use tungsten::core::{AssetRegistry, MaterialRegistry, MaterialUniformDefaults};
    let mut world = seed_world();
    let mut materials = MaterialRegistry::new();
    let halo = materials.allocate(
        "ex10_soft_halo",
        "halo".into(),
        "ex10_soft_glow".into(),
        MaterialUniformDefaults::default(),
    );
    world.insert_resource(materials);
    let mut assets = AssetRegistry::new();
    // Both sprites share one atlas page, as packed sprites do.
    mock_sprite(&mut assets, "ex10_halo", 7, false);
    mock_sprite(&mut assets, "ex10_lantern", 7, false);
    world.insert_resource(assets);
    for x in [100.0, 180.0] {
        let lamp = world.spawn();
        world.insert(lamp, Transform::from_position(Vec2::new(x, 100.0)));
        world.insert(
            lamp,
            Glow {
                offset: Vec2::ZERO,
                radius: 40.0,
                color: [255, 190, 100, 255],
            },
        );
    }
    let batches = crate::extract::extract_sprites(&world);
    let glow_batches: Vec<_> = batches
        .iter()
        .filter(|b| b.material_id == Some(halo))
        .collect();
    assert_eq!(glow_batches.len(), 1);
    assert_eq!(glow_batches[0].instances.len(), 2);
    assert!(
        batches
            .iter()
            .filter(|b| b.material_id.is_none() && b.texture.0 == 7)
            .all(|b| b.instances.iter().all(|i| i.size != [80.0, 80.0]))
    );
}
