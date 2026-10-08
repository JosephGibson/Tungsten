use glam::Vec2;
use tungsten::core::{CameraState, Entity, InputState, MouseButton, World};
use tungsten::physics::{Collider, Position};

use super::seed_world;
use crate::burning::{BallBurn, ignite};
use crate::fireball::{
    FIREBALL_SPEED, FireballMissile, cast_fireball_system, fireball_flight_system, spawn_fireball,
};
use crate::gameplay::Explosion;
use crate::state::{
    BALL_RADIUS, BLACK_HOLE_RADIUS, Ball, BlackHole, Player, SMALL_BALL_SCALE, SmallBall,
};
use crate::systems::black_hole_extinguish_system;

fn small_ball(world: &mut World, at: Vec2) -> tungsten::core::Entity {
    let e = world.spawn();
    world.insert(e, Ball);
    world.insert(e, SmallBall::default());
    world.insert(e, Position(at));
    world.insert(e, Collider::circle(BALL_RADIUS * SMALL_BALL_SCALE));
    e
}

#[test]
fn mouse4_casts_a_fireball_toward_the_cursor() {
    let mut world = seed_world();
    crate::setup::platformer_bindings(&mut world);
    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, Position(Vec2::new(100.0, 100.0)));
    {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::ZERO;
        camera.zoom = 1.0;
    }
    let target = Vec2::new(400.0, 500.0);
    {
        let input = world.get_resource_mut::<InputState>().unwrap();
        input.update_cursor_position(target.x, target.y);
        input.mouse_down(MouseButton::Other(4));
    }

    cast_fireball_system(&mut world);

    let missiles: Vec<_> = world
        .query::<(Entity, &FireballMissile)>()
        .map(|(e, m)| (e, *m))
        .collect();
    assert_eq!(missiles.len(), 1);
    let (missile, state) = missiles[0];
    let aim = (target - Vec2::new(100.0, 100.0)).normalize();
    assert!(state.velocity.normalize().dot(aim) > 0.999);
    assert!((state.velocity.length() - FIREBALL_SPEED).abs() < 1.0);
    let distance = |world: &World| world.get::<Position>(missile).unwrap().0.distance(target);
    let before = distance(&world);
    fireball_flight_system(&mut world);
    assert!(distance(&world) < before);
}

#[test]
fn fireball_contact_explodes_and_ignites_small_balls() {
    let mut world = seed_world();
    let ball = small_ball(&mut world, Vec2::new(200.0, 0.0));
    let missile = spawn_fireball(&mut world, Vec2::ZERO, Vec2::new(FIREBALL_SPEED, 0.0));

    for _ in 0..30 {
        fireball_flight_system(&mut world);
    }

    assert!(!world.is_alive(missile));
    assert_eq!(world.query::<(Entity, &Explosion)>().count(), 1);
    assert!(
        world
            .get::<BallBurn>(ball)
            .is_some_and(|b| b.remaining > 0.0)
    );
}

#[test]
fn black_hole_leaves_burning_balls_inside_its_radius_spent() {
    let mut world = seed_world();
    let hole = world.spawn();
    world.insert(hole, BlackHole { remaining: 2.0 });
    world.insert(hole, Position(Vec2::ZERO));
    let inside = small_ball(&mut world, Vec2::new(BLACK_HOLE_RADIUS * 0.5, 0.0));
    let outside = small_ball(&mut world, Vec2::new(BLACK_HOLE_RADIUS * 1.5, 0.0));
    ignite(&mut world, inside);
    ignite(&mut world, outside);

    black_hole_extinguish_system(&mut world);
    ignite(&mut world, inside);

    assert_eq!(world.get::<BallBurn>(inside).unwrap().remaining, 0.0);
    assert!(world.get::<BallBurn>(outside).unwrap().remaining > 0.0);
}

#[test]
fn a_blast_pushes_dynamic_bodies_away_by_distance_and_mass() {
    use crate::brick::BRICK_MASS;
    use crate::fireball::{FIREBALL_PUSH_RADIUS, FIREBALL_PUSH_SPEED, explode_fireball};
    use tungsten::physics::{RigidBody, RigidBodyBundle, Velocity, physics_step};
    let mut world = seed_world();
    let mut body = |at: Vec2, mass: f32| {
        world.spawn_with(
            RigidBodyBundle::dynamic(Position(at), Collider::circle(BALL_RADIUS))
                .with_body(RigidBody::dynamic().with_mass(mass)),
        )
    };
    let near = body(Vec2::new(40.0, 0.0), 1.0);
    let far = body(Vec2::new(120.0, 0.0), 1.0);
    let heavy = body(Vec2::new(-40.0, 0.0), BRICK_MASS);
    let beyond = body(Vec2::new(0.0, FIREBALL_PUSH_RADIUS + 1.0), 1.0);

    explode_fireball(&mut world, Vec2::ZERO);

    let velocity = |e| world.get::<Velocity>(e).unwrap().0;
    let falloff = 1.0 - 40.0 / FIREBALL_PUSH_RADIUS;
    assert!((velocity(near) - Vec2::new(FIREBALL_PUSH_SPEED * falloff, 0.0)).length() < 1e-3);
    assert!(velocity(far).x > 0.0 && velocity(far).x < velocity(near).x);
    assert!((velocity(heavy).x + velocity(near).x / BRICK_MASS).abs() < 1e-3);
    assert_eq!(velocity(beyond), Vec2::ZERO);
    // The step carries the push into the body's position.
    physics_step(&mut world);
    assert!(world.get::<Position>(near).unwrap().0.x > 40.0);
}
