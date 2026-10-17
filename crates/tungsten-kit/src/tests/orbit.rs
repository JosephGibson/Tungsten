use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten_core::{Bundle, ComponentRegistry, Stage, Time, Transform};

use crate::{LinearMotion, LinearMotionPlugin, Source, TagFilter, Tags, TagsPlugin, spawn_child};

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
    OrbitPlugin.build(&mut schedule, &mut world);
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

#[test]
fn without_a_centre_it_circles_its_first_point_at_its_speed() {
    let (mut world, mut schedule) = rig();
    let pivot = Vec2::new(100.0, 100.0);
    let ball = world.spawn_with((Orbit::new(64.0, PI), Position(pivot), PrevPosition(pivot)));
    for n in 1..=120 {
        steps(&mut world, &mut schedule, 1);
        let angle = PI * n as f32 * DT;
        let expected = pivot + 64.0 * Vec2::from_angle(angle);
        assert!(close(position(&world, ball), expected), "{n}");
    }
    let orbit = world.get::<Orbit>(ball).unwrap();
    assert!(
        orbit.angle() < 1e-3 || (TAU - orbit.angle()) < 1e-3,
        "{}",
        orbit.angle()
    );
    assert_eq!(orbit.circle(), Some((pivot, 64.0)));
}

#[test]
fn it_keeps_its_radius_from_a_moving_centre_and_circles_the_last_one_when_it_goes() {
    let (mut world, mut schedule) = rig();
    let wielder = world.spawn_with((
        Tags::new(["team.player"]),
        LinearMotion::new(Vec2::new(60.0, 20.0)),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    let blade = world.spawn_with((
        Orbit::new(40.0, 2.0 * PI).around(Target::source()),
        Source::new(wielder),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    for _ in 0..45 {
        steps(&mut world, &mut schedule, 1);
        let gap = position(&world, blade).distance(position(&world, wielder));
        assert!((gap - 40.0).abs() < 1e-3, "{gap}");
    }
    let last = position(&world, wielder);
    world.despawn(wielder);
    steps(&mut world, &mut schedule, 10);
    assert!((position(&world, blade).distance(last) - 40.0).abs() < 1e-3);
}

#[test]
fn a_kinematic_orbiter_drives_its_velocity_onto_the_circle() {
    let (mut world, mut schedule) = rig();
    let centre = Vec2::new(-50.0, 20.0);
    let platform = world.spawn_with(
        RigidBodyBundle::kinematic(Position(centre), Collider::aabb(Vec2::new(30.0, 6.0))).with((
            Orbit::new(80.0, -1.0)
                .with_angle(FRAC_PI_2)
                .with_offset(Vec2::new(0.0, 5.0)),
        )),
    );
    steps(&mut world, &mut schedule, 30);
    // Counter-clockwise on screen from straight below the offset centre.
    let angle = FRAC_PI_2 - 0.5;
    let expected = centre + Vec2::new(0.0, 5.0) + 80.0 * Vec2::from_angle(angle);
    assert!(
        close(position(&world, platform), expected),
        "{}",
        position(&world, platform)
    );
    assert!(world.get::<Velocity>(platform).unwrap().0.length() > 0.0);
}

#[test]
fn a_point_centre_and_a_nearest_one_place_it_too() {
    let (mut world, mut schedule) = rig();
    let moon = world.spawn_with((
        Orbit::new(10.0, 0.0).around(Target::point(Vec2::new(5.0, 5.0))),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    world.spawn_with((Tags::new(["team.player"]), Position(Vec2::new(300.0, 0.0))));
    let guard = world.spawn_with((
        Orbit::new(20.0, 0.0)
            .around(Target::nearest(TagFilter::new().all(["team.player"])))
            .with_angle(PI),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    steps(&mut world, &mut schedule, 1);
    assert!(close(position(&world, moon), Vec2::new(15.0, 5.0)));
    assert!(close(position(&world, guard), Vec2::new(280.0, 0.0)));
}

#[test]
fn a_dynamic_body_and_a_child_are_skipped() {
    let (mut world, mut schedule) = rig();
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(4.0))
            .with((Orbit::new(10.0, 1.0),)),
    );
    let parent = world.spawn_with((Transform::default(),));
    let pod = spawn_child(&mut world, parent, Vec2::ONE, (Orbit::new(10.0, 1.0),));
    steps(&mut world, &mut schedule, 5);
    assert_eq!(position(&world, ball), Vec2::ZERO);
    assert_eq!(world.get::<Orbit>(ball).unwrap().circle(), None);
    assert_eq!(
        world.get::<Orbit>(pod).unwrap().angle(),
        0.0,
        "a skipped orbit holds"
    );
}

#[test]
fn the_angle_stays_in_a_turn_and_a_save_keeps_the_centre() {
    let orbit = Orbit::new(3.0, 1.0).with_angle(-FRAC_PI_2);
    assert!((orbit.angle() - 1.5 * PI).abs() < 1e-5);
    let mut circling = Orbit::new(3.0, 1.0).around(Target::source());
    circling.centre_at = Some(Vec2::new(7.0, 8.0));
    let saved = serde_json::to_value(&circling).unwrap();
    assert_eq!(serde_json::from_value::<Orbit>(saved).unwrap(), circling);
    for refused in [
        json!({ "radius": -1, "speed": 1 }),
        json!({ "radius": 1 }),
        json!({ "radius": 1, "speed": 1, "offset": [1e39, 0] }),
    ] {
        assert!(
            serde_json::from_value::<Orbit>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
fn the_scene_component_checks_its_centres_tags() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in("orbit", &json!({ "radius": 48, "speed": 3 }), &world)
            .is_ok()
    );
    let error = components
        .check_in(
            "orbit",
            &json!({ "radius": 48, "speed": 3, "center": { "nearest": { "any": ["team.plyer"] } } }),
            &world,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("center.nearest.any.0"),
        "{error}"
    );
}

#[test]
#[should_panic(expected = "radius must be finite and >= 0")]
fn a_negative_radius_panics() {
    let _ = Orbit::new(-1.0, 1.0);
}
