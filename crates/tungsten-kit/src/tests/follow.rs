use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten_core::{Bundle, ComponentRegistry, Stage, Time, Transform};

use crate::{
    LinearMotion, LinearMotionPlugin, OscillatorPlugin, Source, TagFilter, Tags, TagsPlugin,
    spawn_child,
};

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    OscillatorPlugin.build(&mut schedule, &mut world);
    FollowPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(DT);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-3
}

fn hero(world: &mut World, at: Vec2, velocity: Vec2) -> Entity {
    world.spawn_with((
        Tags::new(["team.player"]),
        LinearMotion::new(velocity),
        Position(at),
        PrevPosition(at),
    ))
}

fn follower(world: &mut World, follow: Follow, at: Vec2) -> Entity {
    world.spawn_with((follow, Position(at), PrevPosition(at)))
}

fn nearest_hero() -> Target {
    Target::nearest(TagFilter::new().all(["team.player"]))
}

#[test]
fn at_lag_zero_it_sits_on_a_moving_target_plus_its_offset_every_step() {
    let (mut world, mut schedule) = rig();
    let hero = hero(&mut world, Vec2::new(100.0, 0.0), Vec2::new(90.0, -30.0));
    let pet = follower(
        &mut world,
        Follow::new(nearest_hero()).with_offset(Vec2::new(-20.0, 0.0)),
        Vec2::ZERO,
    );
    for _ in 0..30 {
        steps(&mut world, &mut schedule, 1);
        let goal = position(&world, hero) + Vec2::new(-20.0, 0.0);
        assert!(
            close(position(&world, pet), goal),
            "{} vs {goal}",
            position(&world, pet)
        );
    }
    assert_eq!(world.get::<Follow>(pet).unwrap().followed(), Some(hero));
}

#[test]
fn with_a_lag_the_gap_shrinks_by_its_factor_each_step() {
    let (mut world, mut schedule) = rig();
    hero(&mut world, Vec2::new(100.0, 0.0), Vec2::ZERO);
    let pet = follower(
        &mut world,
        Follow::new(nearest_hero()).with_lag(0.25),
        Vec2::ZERO,
    );
    let factor = (-DT / 0.25).exp();
    let mut gap = 100.0;
    for _ in 0..20 {
        steps(&mut world, &mut schedule, 1);
        gap *= factor;
        assert!((100.0 - position(&world, pet).x - gap).abs() < 1e-3);
    }
}

#[test]
fn a_top_speed_caps_each_step_and_no_target_holds() {
    let (mut world, mut schedule) = rig();
    let pet = follower(
        &mut world,
        Follow::new(nearest_hero()).with_max_speed(120.0),
        Vec2::ZERO,
    );
    steps(&mut world, &mut schedule, 5);
    assert_eq!(position(&world, pet), Vec2::ZERO, "no hero: it holds");
    assert_eq!(world.get::<Follow>(pet).unwrap().followed(), None);
    hero(&mut world, Vec2::new(100.0, 0.0), Vec2::ZERO);
    steps(&mut world, &mut schedule, 30);
    assert!(
        close(position(&world, pet), Vec2::new(60.0, 0.0)),
        "{}",
        position(&world, pet)
    );
}

#[test]
fn a_kinematic_follower_drives_its_velocity_onto_the_goal_and_zero_while_it_holds() {
    let (mut world, mut schedule) = rig();
    let hero = hero(&mut world, Vec2::new(40.0, 0.0), Vec2::new(0.0, 60.0));
    let drone = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(4.0)))
            .with((Follow::new(nearest_hero()),)),
    );
    for _ in 0..10 {
        steps(&mut world, &mut schedule, 1);
        assert!(close(position(&world, drone), position(&world, hero)));
    }
    world.despawn(hero);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(world.get::<Velocity>(drone).unwrap().0, Vec2::ZERO);
}

#[test]
fn an_oscillator_on_the_follower_bobs_it_about_its_goal() {
    let (mut world, mut schedule) = rig();
    let hero = hero(&mut world, Vec2::new(10.0, 10.0), Vec2::new(30.0, 0.0));
    let pet = world.spawn_with((
        Follow::new(nearest_hero()),
        Oscillator::position(Vec2::new(0.0, 4.0), 1.0),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    for n in 1..=90 {
        steps(&mut world, &mut schedule, 1);
        let t = n as f32 * DT;
        let bob = Vec2::new(0.0, 4.0 * (std::f32::consts::TAU * t).sin());
        let goal = position(&world, hero) + bob;
        assert!(
            close(position(&world, pet), goal),
            "{n}: {} vs {goal}",
            position(&world, pet)
        );
    }
}

#[test]
fn a_source_target_follows_the_spawn_chains_origin() {
    let (mut world, mut schedule) = rig();
    let owner = world.spawn_with((Position(Vec2::new(64.0, 32.0)),));
    let pet = world.spawn_with((
        Follow::new(Target::source()).with_offset(Vec2::new(0.0, -24.0)),
        Source::new(owner),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    steps(&mut world, &mut schedule, 1);
    assert_eq!(position(&world, pet), Vec2::new(64.0, 8.0));
    assert_eq!(
        world.get::<Follow>(pet).unwrap().goal(),
        Some(Vec2::new(64.0, 8.0))
    );
}

#[test]
fn a_dynamic_body_and_a_child_are_skipped() {
    let (mut world, mut schedule) = rig();
    hero(&mut world, Vec2::new(100.0, 0.0), Vec2::ZERO);
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(4.0))
            .with((Follow::new(nearest_hero()),)),
    );
    let parent = world.spawn_with((Transform::default(),));
    let pod = spawn_child(
        &mut world,
        parent,
        Vec2::ONE,
        (Follow::new(nearest_hero()),),
    );
    steps(&mut world, &mut schedule, 5);
    assert_eq!(position(&world, ball), Vec2::ZERO);
    assert_eq!(world.get::<Follow>(ball).unwrap().followed(), None);
    assert_eq!(world.get::<Follow>(pod).unwrap().followed(), None);
}

#[test]
fn the_scene_component_checks_its_tags_and_a_save_refuses_bad_numbers() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(
                "follow",
                &json!({ "target": { "nearest": { "all": ["team.player"] } } }),
                &world
            )
            .is_ok()
    );
    let error = components
        .check_in(
            "follow",
            &json!({ "target": { "nearest": { "all": ["team.plyer"] } } }),
            &world,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("target.nearest.all.0"),
        "{error}"
    );
    let follow = Follow::new(Target::point(Vec2::ONE))
        .with_offset(Vec2::new(1.0, 2.0))
        .with_lag(0.5)
        .with_max_speed(9.0);
    let saved = serde_json::to_value(&follow).unwrap();
    assert_eq!(serde_json::from_value::<Follow>(saved).unwrap(), follow);
    for refused in [
        json!({ "target": { "source": {} }, "lag": -1 }),
        json!({ "target": { "source": {} }, "max_speed": 0 }),
        json!({ "target": { "source": {} }, "offset": [1e39, 0] }),
        json!({ "offset": [0, 0] }),
    ] {
        assert!(
            serde_json::from_value::<Follow>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "lag must be finite and >= 0")]
fn a_negative_lag_panics() {
    let _ = Follow::new(Target::source()).with_lag(-0.1);
}
