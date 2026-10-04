use super::*;

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
    let mut harness = platformer_harness(&PHYSICS_SYSTEMS);
    let world = harness.world_mut();
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

    harness.step(20);

    let p = harness.world().get::<Player>(player).unwrap();
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

#[test]
fn real_map_dimensions_spawn_and_fall_reset() {
    let mut harness = platformer_harness(&PHYSICS_SYSTEMS);
    seed_level(harness.world_mut());
    let map = harness
        .world()
        .get_resource::<TilemapRegistry>()
        .unwrap()
        .get("ex10_level")
        .unwrap();
    assert_eq!(
        (map.width, map.height, map.tile_width, map.tile_height),
        (184, 50, 64, 64)
    );
    let player = spawn_test_player(harness.world_mut(), PLAYER_SPAWN);
    harness.step(60);
    let world = harness.world_mut();
    assert!(world.get::<Player>(player).unwrap().grounded);
    assert!((world.get::<Position>(player).unwrap().0 - PLAYER_SPAWN).length() < 1.0);
    world.get_mut::<Position>(player).unwrap().0.y = crate::state::KILL_Y + 1.0;
    world
        .get_mut::<crate::state::PlayerPresentation>(player)
        .unwrap()
        .pending_effect = Some(crate::state::PlayerEffect::Land);
    despawn_out_of_bounds(world);
    assert_eq!(world.get::<Position>(player).unwrap().0, PLAYER_SPAWN);
    assert!(!world.get::<Player>(player).unwrap().grounded);
    let p = world
        .get::<crate::state::PlayerPresentation>(player)
        .unwrap();
    assert!(p.suppress_landing && p.pending_effect.is_none());
}

/// The player's state right after `player_input`, in the last frame.
#[derive(Clone, Copy)]
struct AfterInput {
    air_jump_used: bool,
    pending_effect: Option<crate::state::PlayerEffect>,
    velocity_y: f32,
}

/// Records [`AfterInput`] for the one player.
fn record_after_input(world: &mut World) {
    let player = world.query::<Player>().next().unwrap().0;
    let after = AfterInput {
        air_jump_used: world.get::<Player>(player).unwrap().air_jump_used,
        pending_effect: world
            .get::<crate::state::PlayerPresentation>(player)
            .unwrap()
            .pending_effect,
        velocity_y: world.get::<Velocity>(player).unwrap().0.y,
    };
    world.insert_resource(after);
}

#[test]
fn double_jump_needs_a_fresh_press_emits_once_and_refills_on_landing() {
    use crate::state::PlayerEffect;
    use crate::systems::player_presentation_system;
    use tungsten::core::{ParticleConfigRegistry, ParticleEmitter};
    let mut harness = platformer_harness(&[
        ("player_input", player_input),
        ("record_after_input", record_after_input),
        ("physics_step", physics_step),
        ("ground_detection", ground_detection),
        ("player_presentation_system", player_presentation_system),
    ]);
    seed_level(harness.world_mut());
    load_presentation_assets(harness.world_mut());
    let player = spawn_test_player(harness.world_mut(), PLAYER_SPAWN);
    harness.step(10);
    assert!(harness.world().get::<Player>(player).unwrap().grounded);
    input_mut(&mut harness).key_down(KeyCode::Space);
    harness.step(1);
    assert!(!harness.world().get::<Player>(player).unwrap().air_jump_used);
    harness.step(10);
    assert!(
        !harness.world().get::<Player>(player).unwrap().air_jump_used,
        "holding space consumed the second jump"
    );
    let input = input_mut(&mut harness);
    input.key_up(KeyCode::Space);
    input.key_down(KeyCode::Space);
    let origin = harness.world().get::<Position>(player).unwrap().0 + Vec2::new(0.0, PLAYER_HALF.y);
    harness.step(1);
    let after = *harness.world().get_resource::<AfterInput>().unwrap();
    assert!(after.air_jump_used);
    assert_eq!(after.pending_effect, Some(PlayerEffect::DoubleJump));
    assert_eq!(after.velocity_y, -crate::state::PLAYER_JUMP_IMPULSE);
    let world = harness.world();
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
    harness.world_mut().get_mut::<Velocity>(player).unwrap().0.y = -300.0;
    let input = input_mut(&mut harness);
    input.key_up(KeyCode::Space);
    input.key_down(KeyCode::Space);
    harness.step(1);
    assert_eq!(
        harness
            .world()
            .get_resource::<AfterInput>()
            .unwrap()
            .velocity_y,
        -300.0
    );
    assert_eq!(
        harness
            .world()
            .query::<ParticleEmitter>()
            .filter(|(_, p)| p.config == config)
            .count(),
        1
    );
    input_mut(&mut harness).key_up(KeyCode::Space);
    harness.step(150);
    let world = harness.world_mut();
    assert!(world.get::<Player>(player).unwrap().grounded);
    assert!(!world.get::<Player>(player).unwrap().air_jump_used);
    world.get_mut::<Player>(player).unwrap().air_jump_used = true;
    crate::systems::respawn_player(world, player);
    assert!(!world.get::<Player>(player).unwrap().air_jump_used);
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
