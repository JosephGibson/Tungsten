use glam::Vec2;

use super::*;
use crate::ecs::Entity;

const DT: f32 = 1.0 / 60.0;

/// A body at `cur` whose last step started at `prev`, drawn at the origin.
fn body_with_history(world: &mut World, prev: Vec2, cur: Vec2) -> Entity {
    world.spawn_with((
        Position(cur),
        PrevPosition(prev),
        Transform::from_position(Vec2::ZERO),
    ))
}

fn body_without_history(world: &mut World, cur: Vec2) -> Entity {
    world.spawn_with((Position(cur), Transform::from_position(Vec2::ZERO)))
}

/// A world whose clock sits `alpha` of a step past its last step.
fn world_at_alpha(alpha: f32) -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT * alpha);
    assert_eq!(time.alpha(), alpha);
    world.insert_resource(time);
    world
}

fn drawn(world: &World, entity: Entity) -> Vec2 {
    world.get::<Transform>(entity).unwrap().position
}

#[test]
fn the_snapshot_copies_position_into_prev_position() {
    let mut world = World::new();
    let body = body_with_history(&mut world, Vec2::ZERO, Vec2::new(3.0, 4.0));
    let bare = body_without_history(&mut world, Vec2::new(5.0, 6.0));
    physics_prev_snapshot(&mut world);
    assert_eq!(
        world.get::<PrevPosition>(body),
        Some(&PrevPosition(Vec2::new(3.0, 4.0)))
    );
    assert_eq!(world.get::<Position>(body), Some(&Position::new(3.0, 4.0)));
    assert!(!world.has::<PrevPosition>(bare));
    assert_eq!(world.get::<Position>(bare), Some(&Position::new(5.0, 6.0)));
}

#[test]
fn the_sync_lerps_a_body_with_history_and_copies_one_without() {
    let mut world = world_at_alpha(0.25);
    let body = body_with_history(&mut world, Vec2::new(0.0, 8.0), Vec2::new(4.0, 0.0));
    let bare = body_without_history(&mut world, Vec2::new(5.0, 6.0));
    physics_sync(&mut world);
    assert_eq!(drawn(&world, body), Vec2::new(1.0, 6.0));
    assert_eq!(drawn(&world, bare), Vec2::new(5.0, 6.0));
    assert_eq!(world.get::<Position>(body), Some(&Position::new(4.0, 0.0)));
    assert_eq!(world.get::<Position>(bare), Some(&Position::new(5.0, 6.0)));
    assert_eq!(
        world.get::<PrevPosition>(body),
        Some(&PrevPosition(Vec2::new(0.0, 8.0)))
    );
}

#[test]
fn at_alpha_zero_the_sync_draws_the_previous_position() {
    let mut world = world_at_alpha(0.0);
    let body = body_with_history(&mut world, Vec2::new(1.5, -2.5), Vec2::new(9.0, 9.0));
    physics_sync(&mut world);
    assert_eq!(drawn(&world, body), Vec2::new(1.5, -2.5));
}

#[test]
fn with_interpolation_off_or_without_time_the_sync_copies() {
    let mut off = world_at_alpha(0.25);
    off.get_resource_mut::<Time>()
        .unwrap()
        .set_interpolate(false);
    let mut timeless = World::new();
    for world in [&mut off, &mut timeless] {
        let body = body_with_history(world, Vec2::ZERO, Vec2::new(4.0, 2.0));
        physics_sync(world);
        assert_eq!(drawn(world, body), Vec2::new(4.0, 2.0));
        assert_eq!(world.get::<Position>(body), Some(&Position::new(4.0, 2.0)));
    }
}

#[test]
fn physics_plugin_alone_resolves_to_the_snapshot_the_step_and_the_sync() {
    let mut schedule = Schedule::new();
    let mut world = World::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.resolved_text(),
        "startup: -\npre_update: -\nfixed_update: physics_prev_snapshot, physics_step\n\
         update: -\npost_update: physics_sync\n"
    );
}
