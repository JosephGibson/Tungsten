use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use super::*;
use serde_json::json;
use tungsten_core::Stage;

use crate::{Aim, spawn_child};

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    FacingPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn present(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

fn direction(world: &World, entity: Entity) -> Vec2 {
    world.get::<Facing>(entity).unwrap().direction()
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-5
}

#[test]
fn motion_reads_the_velocity_else_the_last_steps_move_and_holds_when_slow() {
    let (mut world, mut schedule) = rig();
    let runner = world.spawn_with((Facing::new(), Velocity(Vec2::new(0.0, 50.0))));
    let rover = world.spawn_with((
        Facing::new().with_min_speed(30.0),
        Position(Vec2::new(-0.25, 0.0)),
        PrevPosition(Vec2::ZERO),
    ));
    present(&mut world, &mut schedule);
    assert!(close(direction(&world, runner), Vec2::Y));
    assert_eq!(direction(&world, rover), Vec2::X, "15 px/s is under its 30");
    world.get_mut::<Position>(rover).unwrap().0 = Vec2::new(-1.0, 0.0);
    present(&mut world, &mut schedule);
    assert!(close(direction(&world, rover), Vec2::NEG_X), "60 px/s");
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(0.5, 0.0);
    present(&mut world, &mut schedule);
    assert!(
        close(direction(&world, runner), Vec2::Y),
        "under 1 px/s it keeps facing"
    );
}

#[test]
fn intent_and_aim_sources_hold_while_they_give_nothing() {
    let (mut world, mut schedule) = rig();
    let mut intent = Intent::default();
    intent.set_movement(Vec2::new(-0.2, 0.0));
    let walker = world.spawn_with((Facing::new().from_intent(), intent));
    let gunner = world.spawn_with((Facing::new().from_aim(), Aim::cursor()));
    present(&mut world, &mut schedule);
    assert!(
        close(direction(&world, walker), Vec2::NEG_X),
        "an intent's direction, any length"
    );
    assert_eq!(direction(&world, gunner), Vec2::X, "no aim yet");
    world
        .get_mut::<Intent>(walker)
        .unwrap()
        .set_movement(Vec2::ZERO);
    present(&mut world, &mut schedule);
    assert!(close(direction(&world, walker), Vec2::NEG_X));
}

#[test]
fn each_snap_rounds_the_direction() {
    let facing = Facing::new().with_min_speed(0.0);
    let input = Vec2::new(3.0, -4.0);
    assert!(close(facing.snapped(input).unwrap(), Vec2::new(0.6, -0.8)));
    assert_eq!(facing.four_way().snapped(input), Some(Vec2::NEG_Y));
    assert_eq!(
        facing.four_way().snapped(Vec2::new(4.0, 4.0)),
        Some(Vec2::X),
        "a tie goes to x"
    );
    let eight = facing.eight_way().snapped(Vec2::new(10.0, 1.0)).unwrap();
    assert!(close(eight, Vec2::X));
    let eight = facing.eight_way().snapped(input).unwrap();
    assert!(close(eight, Vec2::from_angle(-FRAC_PI_4)));
    let mut side = facing.horizontal();
    assert_eq!(side.snapped(Vec2::new(-2.0, 9.0)), Some(Vec2::NEG_X));
    side.direction = Vec2::NEG_X;
    assert_eq!(
        side.snapped(Vec2::new(0.0, 9.0)),
        Some(Vec2::NEG_X),
        "the last side"
    );
    assert_eq!(facing.snapped(Vec2::ZERO), None);
    assert_eq!(facing.snapped(Vec2::new(f32::NAN, 1.0)), None);
}

#[test]
fn turning_writes_the_rotation_and_never_the_scale() {
    let (mut world, mut schedule) = rig();
    let transform = Transform {
        scale: Vec2::new(-1.0, 2.0),
        ..Transform::default()
    };
    let ship = world.spawn_with((
        Facing::new().turning(FRAC_PI_2),
        transform,
        Velocity(Vec2::new(0.0, 80.0)),
    ));
    let still = world.spawn_with((Facing::new(), Transform::default(), Velocity(Vec2::Y * 9.0)));
    present(&mut world, &mut schedule);
    let turned = world.get::<Transform>(ship).unwrap();
    assert!(
        (turned.rotation - std::f32::consts::PI).abs() < 1e-5,
        "{}",
        turned.rotation
    );
    assert_eq!(turned.scale, Vec2::new(-1.0, 2.0));
    assert_eq!(
        world.get::<Transform>(still).unwrap().rotation,
        0.0,
        "no turn asked"
    );
    // Stopped, it keeps its heading.
    world.get_mut::<Velocity>(ship).unwrap().0 = Vec2::ZERO;
    present(&mut world, &mut schedule);
    assert!((world.get::<Transform>(ship).unwrap().rotation - std::f32::consts::PI).abs() < 1e-5);
}

#[test]
fn a_child_faces_its_intent() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::default(),));
    let mut intent = Intent::default();
    intent.set_movement(Vec2::new(0.0, -1.0));
    let head = spawn_child(
        &mut world,
        parent,
        Vec2::ZERO,
        (Facing::new().from_intent().turning(0.0), intent),
    );
    present(&mut world, &mut schedule);
    assert!(close(direction(&world, head), Vec2::NEG_Y));
    assert!((world.get::<Transform>(head).unwrap().rotation + FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn a_save_keeps_the_direction_and_refuses_bad_values() {
    let mut facing = Facing::new().horizontal().turning(0.5);
    facing.direction = Vec2::NEG_X;
    let saved = serde_json::to_value(facing).unwrap();
    assert_eq!(serde_json::from_value::<Facing>(saved).unwrap(), facing);
    for refused in [
        json!({ "min_speed": -1 }),
        json!({ "direction": [0.0, 0.0] }),
        json!({ "snap": "six" }),
        json!({ "from": "nose" }),
    ] {
        assert!(
            serde_json::from_value::<Facing>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "offset must be finite")]
fn a_turn_offset_that_is_not_finite_panics() {
    let _ = Facing::new().turning(f32::INFINITY);
}

#[test]
fn facing_with_flip_writes_the_sprite_flip() {
    use tungsten_core::components::SpriteFlip;
    let (mut world, mut schedule) = rig();
    let runner = world.spawn_with((
        Facing::new().horizontal().flipping(),
        Velocity(Vec2::new(30.0, 0.0)),
    ));
    let plain = world.spawn_with((Facing::new(), Velocity(Vec2::new(-30.0, 0.0))));
    present(&mut world, &mut schedule);
    assert!(
        world.get::<SpriteFlip>(runner).is_none(),
        "facing right inserts nothing"
    );
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(-30.0, 0.0);
    present(&mut world, &mut schedule);
    assert_eq!(world.get::<SpriteFlip>(runner), Some(&SpriteFlip(true)));
    // Straight down keeps the side; right unflips and the flip stays.
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(0.0, 30.0);
    present(&mut world, &mut schedule);
    assert_eq!(world.get::<SpriteFlip>(runner), Some(&SpriteFlip(true)));
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(30.0, 0.0);
    present(&mut world, &mut schedule);
    assert_eq!(world.get::<SpriteFlip>(runner), Some(&SpriteFlip(false)));
    assert!(world.get::<SpriteFlip>(plain).is_none(), "no flip asked");
    // From data, and a save keeps it while a facing without it saves as
    // before.
    let registry = world.resource::<tungsten_core::ComponentRegistry>();
    assert!(registry.names().any(|name| name == "facing"));
    let saved = serde_json::to_value(Facing::new().flipping()).unwrap();
    assert_eq!(saved["flip"], json!(true));
    assert_eq!(
        serde_json::from_value::<Facing>(saved).unwrap(),
        Facing::new().flipping()
    );
    assert!(
        serde_json::to_value(Facing::new())
            .unwrap()
            .get("flip")
            .is_none()
    );
}

#[test]
fn face_holds_until_the_source_turns_it() {
    let (mut world, mut schedule) = rig();
    let sentry = world.spawn_with((
        Facing::new().four_way(),
        Velocity(Vec2::ZERO),
        Transform::from_position(Vec2::new(10.0, 10.0)),
    ));
    present(&mut world, &mut schedule);
    assert_eq!(direction(&world, sentry), Vec2::X);
    // A game faces it: snapped as it snaps, held while it stands still.
    world
        .get_mut::<Facing>(sentry)
        .unwrap()
        .face(Vec2::new(-1.0, -3.0));
    assert_eq!(direction(&world, sentry), Vec2::NEG_Y);
    for _ in 0..3 {
        present(&mut world, &mut schedule);
        assert_eq!(direction(&world, sentry), Vec2::NEG_Y, "held");
    }
    // Toward a point, from its Transform without a Position.
    assert!(face_toward(&mut world, sentry, Vec2::new(-50.0, 12.0)));
    present(&mut world, &mut schedule);
    assert_eq!(direction(&world, sentry), Vec2::NEG_X);
    // A zero or non-finite direction, or its own point, faces nothing.
    for ignored in [Vec2::ZERO, Vec2::NAN, Vec2::new(f32::INFINITY, 1.0)] {
        world.get_mut::<Facing>(sentry).unwrap().face(ignored);
        assert_eq!(direction(&world, sentry), Vec2::NEG_X, "{ignored}");
    }
    assert!(!face_toward(&mut world, sentry, Vec2::new(10.0, 10.0)));
    // Its Position wins over its Transform.
    world.insert(sentry, Position(Vec2::new(-100.0, 10.0)));
    assert!(face_toward(&mut world, sentry, Vec2::new(-100.0, 90.0)));
    assert_eq!(direction(&world, sentry), Vec2::Y);
    // Moving, its source turns it again.
    world.get_mut::<Velocity>(sentry).unwrap().0 = Vec2::new(40.0, 0.0);
    present(&mut world, &mut schedule);
    assert_eq!(direction(&world, sentry), Vec2::X);
    // A face ignores the minimum speed; an entity without a facing or a
    // point faces nothing.
    let mut slow = Facing::new().with_min_speed(30.0);
    slow.face(Vec2::new(0.0, 0.01));
    assert!(close(slow.direction(), Vec2::Y));
    let bare = world.spawn_with((Transform::default(),));
    let pointless = world.spawn_with((Facing::new(),));
    assert!(!face_toward(&mut world, bare, Vec2::ONE));
    assert!(!face_toward(&mut world, pointless, Vec2::ONE));
}

#[test]
fn a_turned_mirror_keeps_the_art_upright() {
    use tungsten_core::ComponentRegistry;
    use tungsten_core::components::SpriteFlip;
    let (mut world, mut schedule) = rig();
    let missile = world.spawn_with((
        Facing::new().turning(0.0).mirroring(),
        Transform::default(),
        Velocity(Vec2::new(10.0, 0.0)),
    ));
    let seen = |world: &World| {
        let rotation = world.get::<Transform>(missile).unwrap().rotation;
        let flipped = world.get::<SpriteFlip>(missile).is_some_and(|flip| flip.0);
        (Vec2::from_angle(rotation), flipped)
    };
    let turned = |angle: f32| Vec2::from_angle(angle);
    for (velocity, rotation, flipped) in [
        (Vec2::new(10.0, 0.0), turned(0.0), false),
        // Up and to the left: flipped, turned a quarter of the way up.
        (Vec2::new(-10.0, -10.0), turned(FRAC_PI_4), true),
        (Vec2::new(-10.0, 0.0), turned(0.0), true),
        // Straight up keeps the left side: nose up, flipped.
        (Vec2::new(0.0, -10.0), turned(FRAC_PI_2), true),
        (Vec2::new(10.0, -10.0), turned(-FRAC_PI_4), false),
        // Straight down keeps the right side.
        (Vec2::new(0.0, 10.0), turned(FRAC_PI_2), false),
    ] {
        world.get_mut::<Velocity>(missile).unwrap().0 = velocity;
        present(&mut world, &mut schedule);
        let (got, side) = seen(&world);
        assert!(close(got, rotation), "{velocity}: {got} for {rotation}");
        assert_eq!(side, flipped, "{velocity}");
    }
    // Art drawn facing up: its offset turns the other way while mirrored.
    let rocket = world.spawn_with((
        Facing::new().turning(FRAC_PI_2).mirroring(),
        Transform::default(),
        Velocity(Vec2::new(-10.0, 0.0)),
    ));
    present(&mut world, &mut schedule);
    let rotation = world.get::<Transform>(rocket).unwrap().rotation;
    assert!(close(Vec2::from_angle(rotation), turned(-FRAC_PI_2)));
    // The load refuses `mirror` without `turn`, and the builder panics.
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in("facing", &json!({ "turn": true, "mirror": true }), &world)
            .is_ok()
    );
    assert!(
        components
            .check_in("facing", &json!({ "mirror": true }), &world)
            .is_err()
    );
    assert!(serde_json::from_value::<Facing>(json!({ "mirror": true })).is_err());
    let saved = serde_json::to_value(world.get::<Facing>(missile).unwrap()).unwrap();
    assert_eq!(saved["mirror"], true);
    let plain = serde_json::to_value(Facing::new().turning(0.0)).unwrap();
    assert!(
        plain.get("mirror").is_none(),
        "a plain facing keeps its bytes"
    );
    assert!(std::panic::catch_unwind(|| Facing::new().mirroring()).is_err());
}
