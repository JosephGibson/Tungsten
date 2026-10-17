use super::*;
use serde_json::json;
use tungsten_core::physics::Position;
use tungsten_core::{CameraState, ComponentRegistry, InputState, Stage, Time, Transform};

use crate::{TagFilter, Tags, TagsPlugin, spawn_child};

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);
    AimPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn run(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn direction(world: &World, entity: Entity) -> Option<Vec2> {
    world.get::<Aim>(entity).unwrap().direction()
}

fn close(a: Option<Vec2>, b: Vec2) -> bool {
    a.is_some_and(|a| (a - b).length() < 1e-5)
}

#[test]
fn the_intents_aim_is_kept_while_it_gives_none() {
    let (mut world, mut schedule) = rig();
    let mut intent = Intent::default();
    intent.set_aim(Some(Vec2::new(0.0, -0.5)));
    let hero = world.spawn_with((Aim::intent(), intent, Position(Vec2::ZERO)));
    run(&mut world, &mut schedule);
    assert!(
        close(direction(&world, hero), Vec2::NEG_Y),
        "a unit direction"
    );
    world.get_mut::<Intent>(hero).unwrap().set_aim(None);
    run(&mut world, &mut schedule);
    assert!(
        close(direction(&world, hero), Vec2::NEG_Y),
        "a released stick keeps aiming"
    );
}

#[test]
fn the_cursor_is_aimed_at_from_the_point_plus_the_offset_under_a_moved_camera() {
    let (mut world, mut schedule) = rig();
    let mut camera = CameraState::new();
    camera.position = Vec2::new(1000.0, 500.0);
    world.insert_resource(camera);
    let mut input = InputState::new();
    input.update_cursor_position(100.0, 0.0);
    world.insert_resource(input);
    let turret = world.spawn_with((
        Aim::cursor().with_offset(Vec2::new(16.0, 16.0)),
        Position(Vec2::new(1000.0, 500.0)),
    ));
    run(&mut world, &mut schedule);
    let expected = (Vec2::new(1100.0, 500.0) - Vec2::new(1016.0, 516.0)).normalize();
    assert!(close(direction(&world, turret), expected));
}

#[test]
fn movement_aims_along_the_intent_else_the_velocity() {
    let (mut world, mut schedule) = rig();
    let mut intent = Intent::default();
    intent.set_movement(Vec2::new(0.0, 3.0));
    let walker = world.spawn_with((Aim::movement(), intent, Position(Vec2::ZERO)));
    let drifter = world.spawn_with((
        Aim::movement(),
        Position(Vec2::ZERO),
        tungsten_core::physics::Velocity(Vec2::new(-4.0, 0.0)),
    ));
    let still = world.spawn_with((Aim::movement(), Position(Vec2::ZERO)));
    run(&mut world, &mut schedule);
    assert!(close(direction(&world, walker), Vec2::Y));
    assert!(close(direction(&world, drifter), Vec2::NEG_X));
    assert_eq!(
        direction(&world, still),
        None,
        "none until a source gives one"
    );
}

#[test]
fn a_target_in_range_is_watched_and_held_when_it_leaves() {
    let (mut world, mut schedule) = rig();
    let watch = Aim::at(Target::nearest(TagFilter::new().all(["team.player"])).with_range(100.0));
    let sentry = world.spawn_with((watch, Position(Vec2::ZERO)));
    let hero = world.spawn_with((Tags::new(["team.player"]), Position(Vec2::new(0.0, 60.0))));
    run(&mut world, &mut schedule);
    assert!(close(direction(&world, sentry), Vec2::Y));
    assert_eq!(
        world.get::<Aim>(sentry).unwrap().target_entity(),
        Some(hero)
    );
    assert_eq!(
        world.get::<Aim>(sentry).unwrap().ray(),
        Some((Vec2::ZERO, Vec2::new(0.0, 60.0))),
        "the ray ends at the target"
    );
    world.get_mut::<Position>(hero).unwrap().0 = Vec2::new(500.0, 0.0);
    run(&mut world, &mut schedule);
    let aim = world.get::<Aim>(sentry).unwrap();
    assert!(close(aim.direction(), Vec2::Y), "it holds");
    assert_eq!(aim.target_entity(), None);
    assert_eq!(
        aim.ray(),
        Some((Vec2::ZERO, Vec2::new(0.0, 32.0))),
        "32 px along it"
    );
}

#[test]
fn a_child_aims_from_its_drawn_point() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::from_position(Vec2::new(10.0, 10.0)),));
    let barrel = spawn_child(
        &mut world,
        parent,
        Vec2::new(5.0, 0.0),
        (Aim::at(Target::point(Vec2::new(15.0, 50.0))),),
    );
    run(&mut world, &mut schedule);
    assert!(close(direction(&world, barrel), Vec2::Y));
}

#[test]
fn the_scene_component_needs_a_target_to_aim_at_one_and_checks_its_tags() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    assert!(components.check_in("aim", &json!({}), &world).is_ok());
    assert!(
        components
            .check_in("aim", &json!({ "from": "target" }), &world)
            .is_err()
    );
    let error = components
        .check_in(
            "aim",
            &json!({ "from": "cursor", "target": { "nearest": { "all": ["team.plyer"] } } }),
            &world,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("target.nearest.all.0"),
        "{error}"
    );
}

#[test]
fn a_save_keeps_the_direction_and_refuses_what_is_not_one() {
    let mut aim = Aim::at(Target::point(Vec2::ONE)).with_offset(Vec2::new(4.0, 4.0));
    aim.direction = Some(Vec2::X);
    let saved = serde_json::to_value(&aim).unwrap();
    assert_eq!(serde_json::from_value::<Aim>(saved).unwrap(), aim);
    for refused in [
        json!({ "from": "target" }),
        json!({ "direction": [2.0, 0.0] }),
        json!({ "from": "eyes" }),
        json!({ "offset": [1e39, 0] }),
    ] {
        assert!(
            serde_json::from_value::<Aim>(refused.clone()).is_err(),
            "{refused}"
        );
    }
    let unused: Aim =
        serde_json::from_value(json!({ "from": "intent", "target": { "source": {} } })).unwrap();
    assert_eq!(
        unused,
        Aim::intent(),
        "a target with another source is accepted, unused"
    );
}
