use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle};
use tungsten_core::{Bundle, ComponentRegistry, Stage, Time, Transform};

use crate::{TagFilter, Tags, TagsPlugin, spawn_child};

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);
    SteeringPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize, dt: f32) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(dt);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn hero_target() -> Target {
    Target::nearest(TagFilter::new().all(["team.player"]))
}

fn hero(world: &mut World, at: Vec2) -> Entity {
    world.spawn_with((Tags::new(["team.player"]), Position(at)))
}

fn seeker(world: &mut World, steering: Steering, at: Vec2) -> Entity {
    world.spawn_with((steering, Position(at), PrevPosition(at)))
}

#[test]
fn a_seeker_turns_by_its_acceleration_and_tops_out() {
    let (mut world, mut schedule) = rig();
    let hero = hero(&mut world, Vec2::new(1000.0, 0.0));
    let chaser = seeker(
        &mut world,
        Steering::seek(hero_target(), 90.0).with_acceleration(600.0),
        Vec2::ZERO,
    );
    steps(&mut world, &mut schedule, 1, DT);
    let steering = world.get::<Steering>(chaser).unwrap();
    assert!(
        (steering.velocity().x - 10.0).abs() < 1e-4,
        "600 px/s² for a step"
    );
    assert_eq!(steering.target_entity(), Some(hero));
    steps(&mut world, &mut schedule, 20, DT);
    assert!((world.get::<Steering>(chaser).unwrap().velocity().x - 90.0).abs() < 1e-4);
}

#[test]
fn a_coward_flees_inside_its_radius_and_brakes_outside() {
    let (mut world, mut schedule) = rig();
    hero(&mut world, Vec2::ZERO);
    let coward = seeker(
        &mut world,
        Steering::flee(hero_target(), 80.0).with_radius(50.0),
        Vec2::new(20.0, 0.0),
    );
    steps(&mut world, &mut schedule, 1, DT);
    assert_eq!(
        world.get::<Steering>(coward).unwrap().velocity(),
        Vec2::new(80.0, 0.0)
    );
    steps(&mut world, &mut schedule, 60, DT);
    let x = position(&world, coward).x;
    assert!((50.0..52.0).contains(&x), "it stopped at the radius: {x}");
    assert_eq!(
        world.get::<Steering>(coward).unwrap().velocity(),
        Vec2::ZERO
    );
}

#[test]
fn arriving_inside_less_than_a_steps_travel_lands_and_stays_at_either_step() {
    for dt in [DT, 2.0 * DT] {
        let (mut world, mut schedule) = rig();
        hero(&mut world, Vec2::new(200.0, 50.0));
        let mine = seeker(
            &mut world,
            Steering::arrive(hero_target(), 120.0).with_radius(0.5),
            Vec2::ZERO,
        );
        steps(&mut world, &mut schedule, (4.0 / dt) as usize, dt);
        assert!((position(&world, mine) - Vec2::new(200.0, 50.0)).length() < 1e-3);
        for _ in 0..30 {
            steps(&mut world, &mut schedule, 1, dt);
            assert!((position(&world, mine) - Vec2::new(200.0, 50.0)).length() < 1e-3);
        }
    }
}

#[test]
fn a_full_speed_switch_to_arrive_beside_the_target_stops_on_it() {
    for dt in [DT, 2.0 * DT] {
        let (mut world, mut schedule) = rig();
        hero(&mut world, Vec2::new(1.0, 0.0));
        let mut fast = Steering::arrive(hero_target(), 100.0).with_acceleration(60.0);
        fast.velocity = Vec2::new(100.0, 0.0);
        let mine = seeker(&mut world, fast, Vec2::ZERO);
        steps(&mut world, &mut schedule, 1, dt);
        assert!(
            (position(&world, mine).x - 1.0).abs() < 1e-4,
            "an emergency stop on it"
        );
        steps(&mut world, &mut schedule, 10, dt);
        assert!((position(&world, mine).x - 1.0).abs() < 1e-4);
        assert!(world.get::<Steering>(mine).unwrap().velocity().length() < 1e-3);
    }
}

#[test]
fn with_no_target_it_brakes() {
    let (mut world, mut schedule) = rig();
    let mut drifting = Steering::seek(hero_target(), 90.0).with_acceleration(300.0);
    drifting.velocity = Vec2::new(0.0, 60.0);
    let idle = seeker(&mut world, drifting, Vec2::ZERO);
    steps(&mut world, &mut schedule, 6, DT);
    assert!((world.get::<Steering>(idle).unwrap().velocity().y - 30.0).abs() < 1e-3);
    steps(&mut world, &mut schedule, 10, DT);
    assert_eq!(world.get::<Steering>(idle).unwrap().velocity(), Vec2::ZERO);
    assert_eq!(world.get::<Steering>(idle).unwrap().target_entity(), None);
}

#[test]
fn a_dynamic_seeker_keeps_the_walls_push_and_a_kinematic_one_is_driven() {
    let (mut world, mut schedule) = rig();
    hero(&mut world, Vec2::new(300.0, 0.0));
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(100.0, 0.0)),
        Collider::aabb(Vec2::new(10.0, 200.0)),
    ));
    let brute = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(8.0))
            .with((Steering::seek(hero_target(), 90.0).with_acceleration(900.0),)),
    );
    let drone = world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(Vec2::new(0.0, 300.0)),
            Collider::aabb(Vec2::splat(4.0)),
        )
        .with((Steering::seek(hero_target(), 60.0),)),
    );
    for _ in 0..180 {
        steps(&mut world, &mut schedule, 1, DT);
        assert!(
            position(&world, brute).x <= 90.0 + 0.5,
            "{}",
            position(&world, brute)
        );
    }
    let toward = (Vec2::new(300.0, 0.0) - position(&world, drone)).normalize();
    assert!((world.get::<Velocity>(drone).unwrap().0 - toward * 60.0).length() < 1e-2);
}

#[test]
fn a_static_body_and_a_child_are_skipped() {
    let (mut world, mut schedule) = rig();
    hero(&mut world, Vec2::new(300.0, 0.0));
    let post = world.spawn_with(
        RigidBodyBundle::r#static(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(4.0)))
            .with((Steering::seek(hero_target(), 60.0),)),
    );
    let parent = world.spawn_with((Transform::default(),));
    let pod = spawn_child(
        &mut world,
        parent,
        Vec2::ONE,
        (Steering::seek(hero_target(), 60.0),),
    );
    steps(&mut world, &mut schedule, 5, DT);
    assert_eq!(position(&world, post), Vec2::ZERO);
    assert_eq!(world.get::<Steering>(pod).unwrap().velocity(), Vec2::ZERO);
}

#[test]
fn a_stat_stopped_save_loads_and_the_scene_refuses_a_zero_top_speed() {
    let mut stopped = Steering::seek(Target::source(), 90.0);
    stopped.set_max_speed(0.0);
    stopped.set_max_speed(f32::NAN);
    assert_eq!(stopped.max_speed, 0.0);
    let saved = serde_json::to_value(&stopped).unwrap();
    assert_eq!(serde_json::from_value::<Steering>(saved).unwrap(), stopped);
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let target = json!({ "nearest": { "all": ["team.player"] } });
    assert!(
        components
            .check_in(
                "steering",
                &json!({ "target": target, "max_speed": 90 }),
                &world
            )
            .is_ok()
    );
    for refused in [
        json!({ "target": target, "max_speed": 0 }),
        json!({ "target": target, "max_speed": 90, "mode": "chase" }),
        json!({ "target": target, "max_speed": 90, "radius": 0 }),
        json!({ "max_speed": 90 }),
    ] {
        assert!(
            components.check_in("steering", &refused, &world).is_err(),
            "{refused}"
        );
    }
    for refused in [
        json!({ "target": target, "max_speed": -1 }),
        json!({ "target": target, "max_speed": 9, "acceleration": 0 }),
    ] {
        assert!(
            serde_json::from_value::<Steering>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "max_speed must be finite and > 0")]
fn a_zero_top_speed_panics() {
    let _ = Steering::seek(Target::source(), 0.0);
}
