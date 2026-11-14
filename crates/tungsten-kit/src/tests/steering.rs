use super::*;
use serde_json::{Value, json};
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

/// A world with the physics, a top-down mover's plugin and steering's, the
/// two built in either order.
fn mover_rig(steering_first: bool) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    if steering_first {
        SteeringPlugin.build(&mut schedule, &mut world);
        crate::TopDownMoverPlugin.build(&mut schedule, &mut world);
    } else {
        crate::TopDownMoverPlugin.build(&mut schedule, &mut world);
        SteeringPlugin.build(&mut schedule, &mut world);
    }
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A mover of speed 120 at `at`, on a kinematic body of half 10 or bodiless,
/// steered to arrive at `point`.
fn steered_mover(
    world: &mut World,
    at: Vec2,
    point: Vec2,
    body: bool,
    mover: crate::TopDownMover,
) -> Entity {
    let steering = Steering::arrive(Target::point(point), 120.0)
        .with_acceleration(600.0)
        .with_radius(32.0);
    if body {
        world.spawn_with(
            RigidBodyBundle::kinematic(Position(at), Collider::aabb(Vec2::splat(10.0)))
                .with((mover, steering)),
        )
    } else {
        world.spawn_with((
            mover,
            steering,
            Position(at),
            PrevPosition(at),
            Velocity(Vec2::ZERO),
        ))
    }
}

#[test]
fn steering_drives_a_top_down_mover_by_its_walls() {
    // An arrive at a point behind a wall whose left face is at x 100: the
    // mover's casts stop it at the wall and it slides along it toward the
    // point's height.
    let (mut world, mut schedule) = mover_rig(true);
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(110.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 500.0)),
    ));
    let entity = steered_mover(
        &mut world,
        Vec2::new(0.0, 0.0),
        Vec2::new(200.0, 80.0),
        true,
        crate::TopDownMover::new(120.0),
    );
    steps(&mut world, &mut schedule, 120, DT);
    let at = position(&world, entity);
    assert!(at.x <= 90.0 && at.x > 89.9, "at the wall: {at}");
    assert!(at.y > 40.0, "slid along it: {at}");

    // In the open it lands on the point and stops, on a body and bodiless,
    // and so does a mover with rates of its own moving at 100 px/s when the
    // steering starts; the body's `Velocity` is the mover's.
    for body in [true, false] {
        for rated in [false, true] {
            let (mut world, mut schedule) = mover_rig(true);
            let point = Vec2::new(300.0, -60.0);
            let mover = if rated {
                let mut mover = crate::TopDownMover::new(120.0)
                    .with_acceleration(60.0)
                    .with_deceleration(60.0);
                mover.set_direction(Vec2::new(100.0 / 120.0, 0.0));
                mover
            } else {
                crate::TopDownMover::new(120.0)
            };
            let entity = if rated {
                let entity = if body {
                    world.spawn_with(
                        RigidBodyBundle::kinematic(
                            Position::new(0.0, 0.0),
                            Collider::aabb(Vec2::splat(10.0)),
                        )
                        .with((mover,)),
                    )
                } else {
                    world.spawn_with((
                        mover,
                        Position::new(0.0, 0.0),
                        PrevPosition(Vec2::ZERO),
                        Velocity(Vec2::ZERO),
                    ))
                };
                steps(&mut world, &mut schedule, 120, DT);
                let moving = world.get::<crate::TopDownMover>(entity).unwrap().velocity();
                assert!((moving.x - 100.0).abs() < 1e-3, "{moving}");
                let steering = Steering::arrive(Target::point(point), 120.0)
                    .with_acceleration(600.0)
                    .with_radius(32.0);
                world.insert(entity, steering);
                entity
            } else {
                steered_mover(&mut world, Vec2::ZERO, point, body, mover)
            };
            steps(&mut world, &mut schedule, 600, DT);
            let at = position(&world, entity);
            let at_label = format!("body {body}, rated {rated}: {at}");
            assert!(at.distance(point) < 1e-3, "{at_label}");
            let velocity = world.get::<crate::TopDownMover>(entity).unwrap().velocity();
            assert!(velocity.length() < 1e-2, "{at_label}: {velocity}");
            if body {
                assert_eq!(
                    world.get::<Velocity>(entity).unwrap().0,
                    velocity,
                    "{at_label}"
                );
            }
        }
    }
}

#[test]
fn steering_runs_before_the_mover_either_way_round() {
    // The mover steers its entity at its own step, so the two plugins built
    // in both orders move a steered mover alike, bit for bit, along a wall
    // and onto its point.
    let run = |steering_first: bool| {
        let (mut world, mut schedule) = mover_rig(steering_first);
        world.spawn_with(RigidBodyBundle::r#static(
            Position::new(110.0, 0.0),
            Collider::aabb(Vec2::new(10.0, 40.0)),
        ));
        let entity = steered_mover(
            &mut world,
            Vec2::ZERO,
            Vec2::new(200.0, 80.0),
            true,
            crate::TopDownMover::new(120.0).with_acceleration(300.0),
        );
        (0..480)
            .map(|_| {
                steps(&mut world, &mut schedule, 1, DT);
                position(&world, entity)
            })
            .collect::<Vec<_>>()
    };
    let (first, second) = (run(true), run(false));
    assert_eq!(first, second);
    assert!(
        first.last().unwrap().distance(Vec2::new(200.0, 80.0)) < 1e-3,
        "{:?}",
        first.last()
    );
}

/// A world with the physics and the top-down mover's, steering's, the
/// oscillator's and the knockback's plugins.
fn drivers_rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    crate::TopDownMoverPlugin.build(&mut schedule, &mut world);
    SteeringPlugin.build(&mut schedule, &mut world);
    crate::OscillatorPlugin.build(&mut schedule, &mut world);
    crate::KnockbackPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// `steering`, switched off.
fn off(mut steering: Steering) -> Steering {
    steering.set_on(false);
    steering
}

fn velocity(world: &World, entity: Entity) -> Vec2 {
    world.get::<Velocity>(entity).unwrap().0
}

/// A bodiless mover of speed 120 at the origin with `steering`.
fn bodiless_mover(world: &mut World, mover: crate::TopDownMover, steering: Steering) -> Entity {
    world.spawn_with((
        mover,
        steering,
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
        Velocity(Vec2::ZERO),
    ))
}

#[test]
fn a_steerings_pace_scales_its_arrive() {
    let point = Vec2::new(300.0, 0.0);
    let run = |pace: f32| {
        let (mut world, mut schedule) = mover_rig(true);
        let mut steering = Steering::arrive(Target::point(point), 120.0).with_radius(32.0);
        steering.set_pace(pace);
        assert_eq!(steering.pace(), pace);
        let entity = bodiless_mover(&mut world, crate::TopDownMover::new(120.0), steering);
        steps(&mut world, &mut schedule, 1, DT);
        let first = position(&world, entity);
        let direction = world
            .get::<crate::TopDownMover>(entity)
            .unwrap()
            .direction();
        steps(&mut world, &mut schedule, 1200, DT);
        (first, direction, position(&world, entity))
    };
    let (full, full_direction, full_end) = run(1.0);
    let (half, half_direction, half_end) = run(0.5);
    assert!((full.x - 2.0).abs() < 1e-4, "120 px/s for a step: {full}");
    assert!(
        (half.x - full.x / 2.0).abs() < 1e-4 && half.y == 0.0,
        "half as far a step: {half} against {full}"
    );
    assert!((full_direction.length() - 1.0).abs() < 1e-6);
    assert!(
        (half_direction.length() - 0.5).abs() < 1e-6,
        "half as long: {half_direction}"
    );
    assert!(full_end.distance(point) < 1e-3, "{full_end}");
    assert!(
        half_end.distance(point) < 1e-3,
        "lands all the same: {half_end}"
    );
}

#[test]
fn a_steering_switched_off_leaves_its_mover_and_its_body_alone() {
    let (mut world, mut schedule) = drivers_rig();
    let point = Vec2::new(300.0, 0.0);
    let arrive = || Steering::arrive(Target::point(point), 120.0).with_radius(32.0);
    // A mover whose steering is off moves by the direction the game set.
    let mut mover = crate::TopDownMover::new(120.0);
    mover.set_direction(Vec2::new(0.0, 0.5));
    let flyer = bodiless_mover(&mut world, mover, off(arrive()));
    // A body without a mover keeps the velocity another system wrote.
    let raft = world.spawn_with(
        RigidBodyBundle::kinematic(Position::new(0.0, 100.0), Collider::aabb(Vec2::splat(8.0)))
            .with((off(arrive()),)),
    );
    world.get_mut::<Velocity>(raft).unwrap().0 = Vec2::new(0.0, 30.0);
    steps(&mut world, &mut schedule, 10, DT);
    assert!(
        position(&world, flyer).distance(Vec2::new(0.0, 10.0)) < 1e-3,
        "by its own direction: {}",
        position(&world, flyer)
    );
    assert_eq!(velocity(&world, raft), Vec2::new(0.0, 30.0));
    assert!(position(&world, raft).distance(Vec2::new(0.0, 105.0)) < 1e-3);
    for entity in [flyer, raft] {
        let steering = world.get::<Steering>(entity).unwrap();
        assert!(!steering.is_on());
        assert_eq!(steering.velocity(), Vec2::ZERO, "it wrote nothing");
    }
    // On again, it steers both at the point.
    for entity in [flyer, raft] {
        world.get_mut::<Steering>(entity).unwrap().set_on(true);
    }
    let towards = |world: &World, entity: Entity| (point - position(world, entity)).normalize();
    let (flyer_way, raft_way) = (towards(&world, flyer), towards(&world, raft));
    steps(&mut world, &mut schedule, 1, DT);
    let direction = world.get::<crate::TopDownMover>(flyer).unwrap().direction();
    assert!(direction.distance(flyer_way) < 1e-5, "{direction}");
    assert!(
        (velocity(&world, raft) - raft_way * 120.0).length() < 1e-3,
        "{}",
        velocity(&world, raft)
    );
}

#[test]
fn steered_direction_is_the_movers_next_direction() {
    let (mut world, mut schedule) = mover_rig(true);
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(110.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 40.0)),
    ));
    let entity = steered_mover(
        &mut world,
        Vec2::ZERO,
        Vec2::new(200.0, 80.0),
        true,
        crate::TopDownMover::new(120.0).with_acceleration(300.0),
    );
    for step in 0..240 {
        let next = crate::steered_direction(&world, entity, DT).expect("steered");
        let velocity = world.get::<Steering>(entity).unwrap().velocity();
        assert_eq!(
            crate::steered_direction(&world, entity, DT),
            Some(next),
            "step {step}: reading it records nothing"
        );
        assert_eq!(world.get::<Steering>(entity).unwrap().velocity(), velocity);
        steps(&mut world, &mut schedule, 1, DT);
        let held = world
            .get::<crate::TopDownMover>(entity)
            .unwrap()
            .direction();
        assert_eq!(held, next, "step {step}");
    }
    // Off, without a steering, or a steering without a mover: none.
    world.get_mut::<Steering>(entity).unwrap().set_on(false);
    assert_eq!(crate::steered_direction(&world, entity, DT), None);
    world.remove_component::<Steering>(entity);
    assert_eq!(crate::steered_direction(&world, entity, DT), None);
    let seeker = seeker(
        &mut world,
        Steering::seek(Target::point(Vec2::X), 90.0),
        Vec2::ZERO,
    );
    assert_eq!(crate::steered_direction(&world, seeker, DT), None);
}

#[test]
fn an_off_steering_is_no_driver() {
    let (mut world, mut schedule) = drivers_rig();
    let to = Vec2::new(500.0, 0.0);
    let seek = || Steering::seek(Target::point(to), 90.0);
    // A lift whose oscillator drives its velocity beside an off steering.
    let lift = world.spawn_with(
        RigidBodyBundle::kinematic(
            Position::new(0.0, 200.0),
            Collider::aabb(Vec2::new(40.0, 8.0)),
        )
        .with((
            crate::Oscillator::position(Vec2::new(0.0, 24.0), 2.0),
            off(seek()),
        )),
    );
    // A crate pushed back beside an off steering: it slides, then its
    // knockback's settling step writes zero.
    let crate_ = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(8.0)))
            .with((crate::Knockback::new(0.2), off(seek()))),
    );
    assert!(crate::knock_back(
        &mut world,
        crate_,
        Vec2::new(0.0, -300.0)
    ));
    steps(&mut world, &mut schedule, 1, DT);
    let lifted = velocity(&world, lift);
    assert!(
        lifted.x == 0.0 && lifted.y.abs() > 1.0,
        "the oscillator drives it: {lifted}"
    );
    assert!(velocity(&world, crate_).y < 0.0, "pushed");
    steps(&mut world, &mut schedule, 12, DT);
    assert!(
        (position(&world, crate_).y + 30.0).abs() < 1e-2,
        "{}",
        position(&world, crate_)
    );
    assert_eq!(velocity(&world, crate_), Vec2::ZERO, "settled to zero");
    // Switched on, the steering drives each again, the oscillator aside.
    for entity in [lift, crate_] {
        world.get_mut::<Steering>(entity).unwrap().set_on(true);
    }
    let ways = [lift, crate_].map(|entity| (to - position(&world, entity)).normalize() * 90.0);
    steps(&mut world, &mut schedule, 1, DT);
    for (entity, way) in [lift, crate_].into_iter().zip(ways) {
        let moving = velocity(&world, entity);
        assert!((moving - way).length() < 1e-3, "{moving} against {way}");
    }
}

#[test]
fn pace_and_on_save_and_load() {
    let mut steering = Steering::arrive(Target::point(Vec2::new(5.0, 6.0)), 90.0);
    let plain = serde_json::to_value(&steering).unwrap();
    assert!(
        plain.get("pace").is_none() && plain.get("on").is_none(),
        "left out at their defaults: {plain}"
    );
    let older: Steering = serde_json::from_value(plain).unwrap();
    assert_eq!(older, steering, "a save without them loads as before");
    assert_eq!((older.pace(), older.is_on()), (1.0, true));
    steering.set_pace(0.4);
    steering.set_on(false);
    let saved = serde_json::to_value(&steering).unwrap();
    assert_eq!(saved.get("on"), Some(&json!(false)));
    let back: Steering = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(back, steering);
    assert_eq!((back.pace(), back.is_on()), (0.4, false));
    for pace in [json!(0.0), json!(1.5), json!(-0.5)] {
        let mut refused = saved.clone();
        refused["pace"] = pace;
        assert!(
            serde_json::from_value::<Steering>(refused.clone()).is_err(),
            "{refused}"
        );
    }
    // The scene takes both, its pace in (0, 1].
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let target = json!({ "point": [0, 0] });
    let scene = |extra: Value| {
        let mut def = json!({ "target": target, "max_speed": 90, "mode": "arrive" });
        def.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        components.check_in("steering", &def, &world)
    };
    assert!(scene(json!({ "pace": 0.5, "on": false })).is_ok());
    assert!(scene(json!({ "pace": 1 })).is_ok());
    for refused in [
        json!({ "pace": 0 }),
        json!({ "pace": 1.01 }),
        json!({ "on": 1 }),
    ] {
        assert!(scene(refused.clone()).is_err(), "{refused}");
    }
}
