use super::*;
use crate::brick::{
    BRICK_BLACK_HOLE_PULL, BRICK_CRUSH_SPEED, BRICK_HALF, BRICK_MASS, BRICK_SHATTER_SPEED,
    BRICK_SMASH_KEEP, IronScrap, SCRAP_BLACK_HOLE_PULL, SCRAP_HALF, SCRAP_MASS, brick_impacts,
    spawn_brick,
};
use crate::gameplay::{Health, PreviousVelocity};
use crate::state::{EffectSounds, SMALL_BALL_SCALE, SmallBall};

#[test]
fn surface_blasts_throw_scraps_away_from_the_contact_and_keep_parent_velocity() {
    let center = Vec2::splat(400.0);
    let inherited = Vec2::new(120.0, -80.0);
    for direction in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        let mut world = seed_world();
        let brick = spawn_brick(&mut world, center);
        world.get_mut::<Velocity>(brick).unwrap().0 = inherited;
        crate::ice::freeze_iron(&mut world, brick);
        crate::brick::thermal_shatter(&mut world, center - direction * BRICK_HALF.x, 72.0);
        let kicks: Vec<_> = world
            .query::<(&IronScrap, &Velocity)>()
            .map(|(_, v)| v.0 - inherited)
            .collect();
        assert_eq!(kicks.len(), 16);
        assert!(kicks.iter().all(|v| v.is_finite()));
        let mean = kicks.iter().copied().sum::<Vec2>() / kicks.len() as f32;
        assert!(
            mean.dot(direction) > 300.0,
            "the {direction} surface blast did not throw the chunks away: {mean}"
        );
    }
}

#[test]
fn a_floor_level_explosion_disperses_sixteen_scraps_through_real_physics() {
    let mut harness = platformer_harness(&[
        ("move_obstacles", crate::gameplay::move_obstacles),
        ("physics_step", physics_step),
        ("brick_friction", crate::brick::brick_friction),
    ]);
    let center = Vec2::new(600.0, 500.0);
    let world = harness.world_mut();
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(600.0, 570.0)),
        Collider::aabb(Vec2::new(1200.0, 10.0)),
    ));
    let brick = spawn_brick(world, center);
    crate::ice::freeze_iron(world, brick);
    crate::fireball::explode_fireball(world, center);
    harness.step(15);
    let world = harness.world();
    let positions: Vec<_> = world
        .query::<(&IronScrap, &Position)>()
        .map(|(_, p)| p.0)
        .collect();
    assert_eq!(positions.len(), 16);
    assert!(positions.iter().all(|p| p.is_finite()));
    assert!(
        positions
            .iter()
            .filter(|p| p.distance(center) > 180.0)
            .count()
            >= 8,
        "the chunks stayed bunched around the broken block: {positions:?}"
    );
    let min_x = positions.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let max_x = positions
        .iter()
        .map(|p| p.x)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_x - min_x > 400.0,
        "the fracture did not spread across several tiles"
    );
    assert!(positions.iter().all(|p| p.y <= 560.0 - SCRAP_HALF.y + 1.0));
}

#[test]
fn frozen_iron_breaks_only_under_a_fast_head_on_iron_impact() {
    let cases = [
        (
            true,
            Vec2::X * (BRICK_SHATTER_SPEED - 1.0),
            Vec2::ZERO,
            false,
        ),
        (true, Vec2::X * BRICK_SHATTER_SPEED, Vec2::ZERO, true),
        (false, Vec2::X * 1200.0, Vec2::ZERO, false),
        (true, Vec2::Y * 1200.0, Vec2::ZERO, false),
        (true, -Vec2::X * 1200.0, Vec2::ZERO, false),
        (true, Vec2::X * 1200.0, Vec2::X * 1200.0, false),
    ];
    for (frozen, velocity, target_velocity, breaks) in cases {
        let mut world = seed_world();
        let striker = spawn_brick(&mut world, Vec2::ZERO);
        let target = spawn_brick(&mut world, Vec2::X * (BRICK_HALF.x * 2.0));
        world.insert(striker, PreviousVelocity(velocity));
        world.insert(target, PreviousVelocity(target_velocity));
        if frozen {
            crate::ice::freeze_iron(&mut world, target);
        }
        brick_impacts(&mut world);
        assert_eq!(
            !world.is_alive(target),
            breaks,
            "frozen={frozen}, velocity={velocity}, target velocity={target_velocity}"
        );
        assert!(world.is_alive(striker));
        assert_eq!(
            world.query::<(Entity, &crate::brick::IronScrap)>().count(),
            if breaks { 16 } else { 0 }
        );
        if breaks {
            assert_eq!(
                world.get::<Velocity>(striker).unwrap().0,
                velocity * BRICK_SMASH_KEEP
            );
        }
    }
}

#[test]
fn frozen_iron_shatters_once_from_solver_contacts_in_either_pair_order() {
    for reversed in [false, true] {
        let mut world = seed_world();
        load_presentation_assets(&mut world);
        let striker = spawn_brick(&mut world, Vec2::ZERO);
        let target = spawn_brick(&mut world, Vec2::X * 400.0);
        crate::ice::freeze_iron(&mut world, target);
        world.insert(striker, PreviousVelocity(Vec2::X * BRICK_SHATTER_SPEED));
        world.insert(target, PreviousVelocity(Vec2::ZERO));
        // The solver has stopped the striker and pushed the target clear.
        // Duplicate substep contacts must not duplicate the sixteen scraps.
        for _ in 0..3 {
            world
                .get_resource_mut::<EventQueue<CollisionEvent>>()
                .unwrap()
                .send(CollisionEvent {
                    a: if reversed { target } else { striker },
                    b: Some(if reversed { striker } else { target }),
                    normal: if reversed { Vec2::X } else { -Vec2::X },
                    penetration: 0.0,
                });
        }
        brick_impacts(&mut world);
        brick_impacts(&mut world);
        assert!(!world.is_alive(target));
        assert!(world.is_alive(striker));
        assert_eq!(
            world.query::<(Entity, &crate::brick::IronScrap)>().count(),
            16
        );
        assert_eq!(
            world
                .query::<(Entity, &crate::state::TransientEmitter)>()
                .count(),
            1
        );
    }
}

#[test]
fn a_real_falling_iron_block_shatters_a_frozen_block_on_the_floor() {
    let mut harness = platformer_harness(&[
        ("move_obstacles", crate::gameplay::move_obstacles),
        ("physics_step", physics_step),
        ("brick_impacts", brick_impacts),
    ]);
    let world = harness.world_mut();
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(400.0, 570.0)),
        Collider::aabb(Vec2::new(200.0, 10.0)),
    ));
    let target = spawn_brick(world, Vec2::new(400.0, 500.0));
    crate::ice::freeze_iron(world, target);
    let striker = spawn_brick(world, Vec2::new(400.0, 300.0));
    world.get_mut::<Velocity>(striker).unwrap().0 = Vec2::Y * 1200.0;
    harness.step(10);
    assert!(!harness.world().is_alive(target));
    assert!(harness.world().is_alive(striker));
    assert_eq!(
        harness
            .world()
            .query::<(Entity, &crate::brick::IronScrap)>()
            .count(),
        16
    );
}

#[test]
fn a_fast_brick_hurts_the_player_and_smashes_a_ball_and_a_resting_one_does_neither() {
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, Vec2::ZERO);
    world.insert(player, Health::default());
    let radius = BALL_RADIUS * SMALL_BALL_SCALE;
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::new(400.0, 0.0)), Collider::circle(radius))
            .with((Ball, SmallBall::default())),
    );
    // One brick resting on the player's head, one on the ball.
    let bricks = [
        spawn_brick(&mut world, Vec2::new(0.0, -(PLAYER_HALF.y + BRICK_HALF.y))),
        spawn_brick(&mut world, Vec2::new(400.0, -(radius + BRICK_HALF.y))),
    ];
    let drive = |world: &mut World, speed: f32| {
        for brick in bricks {
            world.insert(brick, PreviousVelocity(Vec2::new(0.0, speed)));
        }
        brick_impacts(world);
    };

    drive(&mut world, 0.0);
    drive(&mut world, BRICK_CRUSH_SPEED - 1.0);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 3);
    assert!(world.is_alive(ball));

    drive(&mut world, BRICK_CRUSH_SPEED + 40.0);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    assert!(!world.is_alive(ball));
}

#[test]
fn black_holes_move_iron_more_slowly_than_balls_and_skip_static_bodies() {
    let mut world = seed_world();
    let at = Vec2::new(120.0, 0.0);
    let brick = spawn_brick(&mut world, at);
    let ball = world.spawn_with(RigidBodyBundle::dynamic(
        Position(at),
        Collider::circle(BALL_RADIUS),
    ));
    let fixed = world.spawn();
    world.insert(fixed, RigidBody::r#static());
    world.insert(fixed, Velocity(Vec2::ZERO));
    world.insert(fixed, Position(at));
    let hole = world.spawn();
    world.insert(hole, BlackHole { remaining: 2.0 });
    world.insert(hole, Position(Vec2::ZERO));

    black_hole_force_system(&mut world);

    let pull = world.get::<Velocity>(ball).unwrap().0;
    assert!(pull.x < 0.0);
    assert!(
        (world.get::<Velocity>(brick).unwrap().0 - pull * BRICK_BLACK_HOLE_PULL).length() < 0.001
    );
    assert_eq!(
        world.get::<RigidBody>(brick).unwrap().inv_mass,
        1.0 / BRICK_MASS
    );
    assert_eq!(world.get::<Velocity>(fixed).unwrap().0, Vec2::ZERO);
}

#[test]
fn black_holes_gather_warm_and_frozen_scraps_as_readily_as_balls() {
    let mut world = seed_world();
    let at = Vec2::new(120.0, 0.0);
    let brick = spawn_brick(&mut world, at);
    let ball = world.spawn_with(RigidBodyBundle::dynamic(
        Position(at),
        Collider::circle(BALL_RADIUS),
    ));
    let scraps = [false, true].map(|frozen| {
        let e = world.spawn_with(
            RigidBodyBundle::dynamic(Position(at), Collider::aabb(SCRAP_HALF))
                .with_body(RigidBody::dynamic().with_mass(SCRAP_MASS))
                .with((IronScrap { variant: 0 },)),
        );
        if frozen {
            crate::ice::freeze_iron(&mut world, e);
        }
        e
    });
    let hole = world.spawn();
    world.insert(hole, BlackHole { remaining: 2.0 });
    world.insert(hole, Position(Vec2::ZERO));
    black_hole_force_system(&mut world);
    let ball_pull = world.get::<Velocity>(ball).unwrap().0;
    let brick_pull = world.get::<Velocity>(brick).unwrap().0;
    assert!(ball_pull.x < 0.0);
    for scrap in scraps {
        let velocity = world.get::<Velocity>(scrap).unwrap().0;
        assert!((velocity - ball_pull * SCRAP_BLACK_HOLE_PULL).length() < 0.001);
        assert!(velocity.length() > brick_pull.length() * 4.9);
        assert_eq!(
            world.get::<RigidBody>(scrap).unwrap().inv_mass,
            1.0 / SCRAP_MASS
        );
    }
}

#[test]
fn faster_slams_crush_more_marble_contacts_keep_momentum_and_throttle_audio() {
    fn slam(speed: f32) -> (usize, Vec2, usize, usize) {
        let mut world = seed_world();
        load_presentation_assets(&mut world);
        world.insert_resource(AudioCommands::new());
        let crush = AudioHandle(5);
        world.insert_resource(EffectSounds {
            cast: (AudioHandle(1), 1.0),
            ice_cast: (AudioHandle(6), 1.0),
            ice_loop: (AudioHandle(7), 1.0),
            ice_freeze: (AudioHandle(8), 1.0),
            ice_end: (AudioHandle(10), 1.0),
            ice_shatter: (AudioHandle(11), 1.0),
            blast: (AudioHandle(2), 1.0),
            extinguish: (AudioHandle(3), 1.0),
            hit: (AudioHandle(4), 1.0),
            crush: (crush, 0.65),
            extinguish_cooldown: 0.0,
            crush_cooldown: 0.0,
        });
        let brick = spawn_brick(&mut world, Vec2::ZERO);
        world.insert(brick, PreviousVelocity(Vec2::Y * speed));
        // Post-resolution positions no longer touch the brick. Duplicate
        // substep contacts and both pair orderings must still crush once.
        for index in 0..32 {
            let ball = world.spawn_with(
                RigidBodyBundle::dynamic(
                    Position(Vec2::new(index as f32 * 15.0, 150.0)),
                    Collider::circle(7.5),
                )
                .with((Ball, SmallBall::default())),
            );
            let (a, b, normal) = if index % 2 == 0 {
                (brick, ball, -Vec2::Y)
            } else {
                (ball, brick, Vec2::Y)
            };
            for _ in 0..2 {
                world
                    .get_resource_mut::<EventQueue<CollisionEvent>>()
                    .unwrap()
                    .send(CollisionEvent {
                        a,
                        b: Some(b),
                        normal,
                        penetration: 1.0,
                    });
            }
        }
        let large = world.spawn_with(
            RigidBodyBundle::dynamic(
                Position(Vec2::new(0.0, BRICK_HALF.y + BALL_RADIUS)),
                Collider::circle(BALL_RADIUS),
            )
            .with((Ball,)),
        );
        brick_impacts(&mut world);
        let smashed = 32 - world.query::<(Entity, &SmallBall)>().count();
        let velocity = world.get::<Velocity>(brick).unwrap().0;
        let bursts = world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count();
        let sound_count = world
            .get_resource_mut::<AudioCommands>()
            .unwrap()
            .drain()
            .into_iter()
            .filter(|cmd| matches!(cmd, AudioCommand::Play { handle, .. } if *handle == crush))
            .count();
        assert!(world.is_alive(large));
        // A following physics step in the same short crunch keeps crushing,
        // but shares the audio voice.
        brick_impacts(&mut world);
        assert!(
            world
                .get_resource_mut::<AudioCommands>()
                .unwrap()
                .drain()
                .is_empty()
        );
        (smashed, velocity, bursts, sound_count)
    }
    let slow = slam(BRICK_CRUSH_SPEED);
    let fast = slam(1440.0);
    assert_eq!(slow.0, 4);
    assert_eq!(fast.0, 22);
    assert_eq!(fast.1, Vec2::Y * 1440.0 * BRICK_SMASH_KEEP);
    assert_eq!(fast.2, 4);
    assert_eq!(fast.3, 1);
}

#[test]
fn a_fast_drop_plows_through_a_real_physics_pile() {
    let mut harness = platformer_harness(&[
        ("move_obstacles", crate::gameplay::move_obstacles),
        ("physics_step", physics_step),
        ("brick_impacts", brick_impacts),
    ]);
    set_dt(&mut harness, 1.0 / 60.0);
    let world = harness.world_mut();
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(400.0, 630.0)));
    world.insert(floor, Collider::aabb(Vec2::new(200.0, 10.0)));
    world.insert(floor, RigidBody::r#static());
    for row in 0..9 {
        for col in 0..7 {
            let at = Vec2::new(400.0 + (col as f32 - 3.0) * 15.2, 612.5 - row as f32 * 15.2);
            world.spawn_with(
                RigidBodyBundle::dynamic(Position(at), Collider::circle(7.5))
                    .with((Ball, SmallBall::default())),
            );
        }
    }
    // Settle the pile before introducing the falling weight.
    harness.step(60);
    let world = harness.world_mut();
    let brick = spawn_brick(world, Vec2::new(400.0, 300.0));
    world.get_mut::<Velocity>(brick).unwrap().0 = Vec2::Y * 1800.0;
    let before = world.query::<(Entity, &SmallBall)>().count();
    harness.step(30);
    let world = harness.world();
    let smashed = before - world.query::<(Entity, &SmallBall)>().count();
    assert!(
        smashed >= 21,
        "the fast drop only crushed {smashed} of {before} marbles"
    );
    assert!(world.get::<Position>(brick).unwrap().0.y > 400.0);
}
