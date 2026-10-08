use super::*;
use crate::brick::{BRICK_CRUSH_SPEED, BRICK_HALF, brick_impacts, spawn_brick};
use crate::gameplay::{Health, PreviousVelocity};
use crate::state::{SMALL_BALL_SCALE, SmallBall};

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
