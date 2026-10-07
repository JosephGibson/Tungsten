use super::*;
use tungsten::physics::PrevPosition;

#[test]
fn spawn_ball_system_spawns_at_fixed_rate_while_held() {
    let mut harness = platformer_harness(&[("spawn_ball_system", spawn_ball_system)]);
    let world = harness.world_mut();
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
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::new(0.0, 0.0);
        camera.zoom = 1.0;
    }

    harness.set_cursor(240.0, 144.0);
    harness.press_action("spawn_ball");

    // 170 ms avoids exact-multiple floating-point cliff: floor(0.170 / 0.032) = 5.
    harness.set_dt(0.170);
    harness.step(1);

    let world = harness.world();
    assert_eq!(world.query::<(Entity, &Ball)>().count(), 5);
    // Left-click orbs carry no hue; only small marbles cycle colour.
    assert_eq!(world.query::<(Entity, &BallHue)>().count(), 0);
    let center = Vec2::new(240.0, 144.0);
    let positions: Vec<Vec2> = world
        .query::<(Entity, &Ball)>()
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
    world.get_resource_mut::<Time>().unwrap().advance_frame(0.5);

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

    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(0.016);
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

    let holes: Vec<_> = world.query::<(Entity, &BlackHole)>().collect();
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
    let mut harness = platformer_harness(&[("spawn_black_hole_system", spawn_black_hole_system)]);
    let world = harness.world_mut();
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
    harness.set_cursor(50.0, 60.0);
    harness.press_action("spawn_black_hole");
    harness.step(1);
    let hole_entity = harness
        .world()
        .get_resource::<ActiveBlackHole>()
        .unwrap()
        .0
        .expect("press should register active hole");

    // Frame 2: hold moves and refreshes lifetime.
    if let Some(hole) = harness.world_mut().get_mut::<BlackHole>(hole_entity) {
        hole.remaining = 0.5;
    }
    harness.set_cursor(200.0, 150.0);
    harness.step(1);

    let world = harness.world();
    let pos = world.get::<Position>(hole_entity).unwrap().0;
    assert_eq!(pos, Vec2::new(200.0, 150.0), "hole should follow cursor");
    assert_eq!(
        world.get::<BlackHole>(hole_entity).unwrap().remaining,
        BLACK_HOLE_LIFETIME,
        "holding must refresh lifetime so the hole never expires mid-drag"
    );
    assert_eq!(
        world.query::<(Entity, &BlackHole)>().count(),
        1,
        "hold must not spawn a second hole per frame"
    );

    // Frame 3: release despawns dragged entity and clears slot.
    harness.release_action("spawn_black_hole");
    harness.step(1);
    let world = harness.world();
    assert_eq!(world.get_resource::<ActiveBlackHole>().unwrap().0, None);
    assert_eq!(
        world.query::<(Entity, &BlackHole)>().count(),
        0,
        "release must despawn the dragged hole immediately, not let it fade"
    );
}

#[test]
fn black_hole_force_system_pulls_dynamic_body_toward_hole() {
    let mut world = seed_world();
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(1.0 / 60.0);

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
    world.insert(ball, PrevPosition(Vec2::new(BLACK_HOLE_RADIUS * 0.5, 0.0)));
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
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(1.0 / 60.0);

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
    world.insert(ball, PrevPosition(Vec2::new(BLACK_HOLE_RADIUS + 10.0, 0.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(BALL_RADIUS));
    world.insert(ball, RigidBody::dynamic());

    black_hole_force_system(&mut world);
    assert_eq!(world.get::<Velocity>(ball).unwrap().0, Vec2::ZERO);
}

#[test]
fn black_hole_lifetime_system_despawns_expired_hole() {
    let mut harness =
        platformer_harness(&[("black_hole_lifetime_system", black_hole_lifetime_system)]);
    harness.set_dt(BLACK_HOLE_LIFETIME + 0.1);

    let world = harness.world_mut();
    let hole = world.spawn();
    world.insert(
        hole,
        BlackHole {
            remaining: BLACK_HOLE_LIFETIME,
        },
    );
    world.insert(hole, Position(Vec2::ZERO));
    world.insert_resource(ActiveBlackHole(Some(hole)));

    harness.step(1);
    let world = harness.world();
    assert_eq!(
        world.get_resource::<ActiveBlackHole>().unwrap().0,
        None,
        "expired active holes must clear their drag slot"
    );
    assert_eq!(world.query::<(Entity, &BlackHole)>().count(), 0);
}

/// The commands queued so far in the last frame, when `record_queued_commands` ran.
struct QueuedCommands(usize);

/// Records [`QueuedCommands`].
fn record_queued_commands(world: &mut World) {
    let queued = world.get_resource::<CommandBuffer>().unwrap().len();
    world.insert_resource(QueuedCommands(queued));
}

#[test]
fn despawn_out_of_bounds_culls_escaped_balls_and_keeps_in_bounds_balls() {
    let mut harness = platformer_harness(&[
        ("despawn_out_of_bounds", despawn_out_of_bounds),
        ("record_queued_commands", record_queued_commands),
    ]);
    let world = harness.world_mut();

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

    assert_eq!(world.query::<(Entity, &Ball)>().count(), 3);

    harness.step(1);
    let world = harness.world();
    assert_eq!(
        world.get_resource::<QueuedCommands>().unwrap().0,
        1,
        "exactly one ball should be queued for despawn"
    );

    let remaining: Vec<_> = world.query::<(Entity, &Ball)>().map(|(e, _)| e).collect();
    assert_eq!(remaining, vec![inside, partly_inside]);
}

#[test]
fn only_small_marbles_shift_hue_while_orbs_render_untinted() {
    use crate::state::{BALL_START_SPRITE_ID, SMALL_BALL_START_SPRITE_ID, SmallBall};
    let mut harness = platformer_harness(&[("spawn_ball_system", spawn_ball_system)]);
    let world = harness.world_mut();
    world.insert_resource(BallSpawnState::default());
    crate::setup::platformer_bindings(world);
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
    harness.set_dt(0.1);
    harness.step(1);
    let world = harness.world_mut();
    let balls: Vec<_> = world.query::<(Entity, &Ball)>().map(|(e, _)| e).collect();
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
    let batches = crate::extract::extract_sprites(world);
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
