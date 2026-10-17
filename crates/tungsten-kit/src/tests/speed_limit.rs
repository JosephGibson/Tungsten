use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle};
use tungsten_core::{Bundle, PhysicsConfig, Stage, Time};

use crate::{
    Knockback, KnockbackPlugin, LinearMotionPlugin, PlatformerControllerPlugin, SteeringPlugin,
    Target, knock_back,
};

const DT: f32 = 1.0 / 60.0;

fn rig(gravity: f32) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, gravity),
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    PlatformerControllerPlugin.build(&mut schedule, &mut world);
    SteeringPlugin.build(&mut schedule, &mut world);
    KnockbackPlugin.build(&mut schedule, &mut world);
    SpeedLimitPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

#[test]
fn the_length_is_capped_first_then_each_axis() {
    let limit = SpeedLimit::speed(100.0).with_x(30.0);
    let capped = limit.cap(Vec2::new(200.0, 200.0));
    assert!((capped - Vec2::new(30.0, 100.0 * std::f32::consts::FRAC_1_SQRT_2)).length() < 1e-4);
    assert_eq!(
        SpeedLimit::y(5.0).cap(Vec2::new(-80.0, -9.0)),
        Vec2::new(-80.0, -5.0)
    );
    assert_eq!(
        SpeedLimit::x(5.0).cap(Vec2::new(1.0, 2.0)),
        Vec2::new(1.0, 2.0)
    );
}

#[test]
fn a_kinematic_and_a_bodiless_step_never_pass_the_cap() {
    let (mut world, mut schedule) = rig(0.0);
    let fast = LinearMotion::new(Vec2::new(500.0, 0.0)).with_acceleration(Vec2::new(0.0, 900.0));
    let cart = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(4.0)))
            .with((fast.clone(), SpeedLimit::speed(240.0))),
    );
    let rock = world.spawn_with((
        fast,
        SpeedLimit::x(120.0).with_y(60.0),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    for _ in 0..60 {
        let (cart_at, rock_at) = (position(&world, cart), position(&world, rock));
        step(&mut world, &mut schedule);
        let cart_move = position(&world, cart) - cart_at;
        assert!(cart_move.length() <= 240.0 * DT + 1e-4, "{cart_move}");
        let rock_move = position(&world, rock) - rock_at;
        assert!(
            rock_move.x <= 120.0 * DT + 1e-4 && rock_move.y <= 60.0 * DT + 1e-4,
            "{rock_move}"
        );
    }
    assert!(world.get::<LinearMotion>(cart).unwrap().velocity().length() <= 240.0 + 1e-3);
    assert_eq!(
        world.get::<LinearMotion>(rock).unwrap().velocity(),
        Vec2::new(120.0, 60.0)
    );
}

#[test]
fn a_falling_platformer_and_its_own_velocity_top_out() {
    let (mut world, mut schedule) = rig(2000.0);
    let acrobat = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(10.0, 20.0)))
            .with((
                PlatformerController::new(200.0, 600.0),
                SpeedLimit::y(500.0),
            )),
    );
    for _ in 0..60 {
        let at = position(&world, acrobat);
        step(&mut world, &mut schedule);
        assert!(position(&world, acrobat).y - at.y <= 500.0 * DT + 1e-3);
    }
    let controller = world.get_mut::<PlatformerController>(acrobat).unwrap();
    assert!((controller.velocity_mut().y - 500.0).abs() < 1e-3);
}

#[test]
fn a_steerings_own_velocity_is_capped() {
    let (mut world, mut schedule) = rig(0.0);
    let probe = world.spawn_with((
        Steering::seek(Target::point(Vec2::new(1000.0, 0.0)), 200.0),
        SpeedLimit::speed(50.0),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Steering>(probe).unwrap().velocity(),
        Vec2::new(50.0, 0.0)
    );
    assert!((position(&world, probe).x - 50.0 * DT).abs() < 1e-5);
}

#[test]
fn a_dynamic_body_in_free_fall_enters_and_leaves_each_step_under_its_cap() {
    let (mut world, mut schedule) = rig(1200.0);
    let crate_ = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(8.0)))
            .with((SpeedLimit::y(300.0), Knockback::new(0.1))),
    );
    for n in 0..90 {
        if n == 30 {
            // Knocked down far past its cap.
            assert!(knock_back(&mut world, crate_, Vec2::new(0.0, 2000.0)));
        }
        let at = position(&world, crate_);
        step(&mut world, &mut schedule);
        assert!(
            world.get::<Velocity>(crate_).unwrap().0.y <= 300.0 + 1e-3,
            "it leaves capped"
        );
        let moved = position(&world, crate_).y - at.y;
        assert!(moved <= (300.0 + 1200.0 * DT) * DT + 1e-3, "{n}: {moved}");
    }
}

#[test]
fn a_save_round_trips_and_needs_a_cap() {
    let limit = SpeedLimit::speed(600.0).with_x(240.0);
    assert_eq!(
        serde_json::to_value(limit).unwrap(),
        json!({ "speed": 600.0, "x": 240.0 })
    );
    for refused in [json!({}), json!({ "x": 0 }), json!({ "speed": 1, "z": 2 })] {
        assert!(
            serde_json::from_value::<SpeedLimit>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "speed must be finite and > 0")]
fn a_cap_of_zero_panics() {
    let _ = SpeedLimit::speed(1.0).with_y(0.0);
}
