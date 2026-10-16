use super::*;
use serde_json::json;
use tungsten_core::physics::Position;
use tungsten_core::{Binding, ComponentRegistry, KeyCode, Stage, Time};

use crate::{TopDownMover, TopDownMoverPlugin};

fn actions() -> ActionMap {
    let mut actions = ActionMap::new();
    for (action, code) in [
        ("move_left", KeyCode::ArrowLeft),
        ("move_right", KeyCode::ArrowRight),
        ("move_up", KeyCode::ArrowUp),
        ("move_down", KeyCode::ArrowDown),
        ("aim_left", KeyCode::KeyA),
        ("aim_right", KeyCode::KeyD),
        ("aim_up", KeyCode::KeyW),
        ("aim_down", KeyCode::KeyS),
        ("jump", KeyCode::Space),
    ] {
        actions.replace_bindings(action, vec![Binding::Key { code }]);
    }
    actions
}

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(actions());
    world.insert_resource(InputState::new());
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    ActionInputPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn full() -> ActionInput {
    ActionInput::new("move_left", "move_right")
        .with_vertical("move_up", "move_down")
        .with_aim(["aim_left", "aim_right", "aim_up", "aim_down"])
        .with_slot("jump", "jump")
}

#[test]
fn the_plugin_registers_its_component_and_system() {
    let (world, schedule) = rig();
    assert_eq!(schedule.stage_of(ACTION_INPUT), Some(Stage::FixedUpdate));
    assert!(
        world
            .resource::<ComponentRegistry>()
            .contains(ACTION_INPUT_COMPONENT)
    );
}

#[test]
fn movement_aim_and_slots_follow_the_input_each_step() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((full(), Intent::default()));
    step(&mut world, &mut schedule);
    let intent = world.get::<Intent>(hero).unwrap();
    assert_eq!(intent.movement(), Vec2::ZERO);
    assert_eq!(intent.aim(), Some(Vec2::ZERO), "it aims, nowhere");
    assert!(!intent.held("jump") && !intent.pressed("jump"));

    {
        let input = world.resource_mut::<InputState>();
        input.key_down(KeyCode::ArrowRight);
        input.key_down(KeyCode::ArrowDown);
        input.key_down(KeyCode::KeyW);
        input.key_down(KeyCode::Space);
    }
    step(&mut world, &mut schedule);
    let intent = world.get::<Intent>(hero).unwrap();
    let diagonal = Vec2::splat(std::f32::consts::FRAC_1_SQRT_2);
    assert!(
        (intent.movement() - diagonal).length() < 1e-6,
        "clamped to 1"
    );
    assert_eq!(intent.aim(), Some(Vec2::new(0.0, -1.0)));
    assert!(intent.held("jump") && intent.pressed("jump"));

    // The next frame: held, no longer just pressed.
    world.resource_mut::<InputState>().begin_frame();
    step(&mut world, &mut schedule);
    let intent = world.get::<Intent>(hero).unwrap();
    assert!(intent.held("jump") && !intent.pressed("jump"));

    world
        .resource_mut::<InputState>()
        .key_up(KeyCode::ArrowRight);
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Intent>(hero).unwrap().movement(),
        Vec2::new(0.0, 1.0)
    );
}

#[test]
fn two_axes_never_move_vertically_and_no_aim_reads_none() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((
        ActionInput::new("move_left", "move_right"),
        Intent::default(),
    ));
    world
        .resource_mut::<InputState>()
        .key_down(KeyCode::ArrowDown);
    world
        .resource_mut::<InputState>()
        .key_down(KeyCode::ArrowLeft);
    step(&mut world, &mut schedule);
    let intent = world.get::<Intent>(hero).unwrap();
    assert_eq!(intent.movement(), Vec2::new(-1.0, 0.0));
    assert_eq!(intent.aim(), None);
}

#[test]
fn the_routed_input_wins_over_the_raw_input_when_present() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((full(), Intent::default()));
    world
        .resource_mut::<InputState>()
        .key_down(KeyCode::ArrowRight);
    let mut routed = RoutedInput::default();
    routed.key_down(KeyCode::ArrowLeft);
    world.insert_resource(routed);
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Intent>(hero).unwrap().movement(),
        Vec2::new(-1.0, 0.0),
        "the UI's leftovers, not the raw keys"
    );
    assert!(world.has_resource::<RoutedInput>() && world.has_resource::<InputState>());
    assert!(world.has_resource::<ActionMap>(), "put back after the pass");
}

#[test]
fn without_the_map_or_the_input_nothing_is_written() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((full(), Intent::default()));
    world
        .resource_mut::<InputState>()
        .key_down(KeyCode::ArrowRight);
    let actions = world.remove_resource::<ActionMap>().unwrap();
    step(&mut world, &mut schedule);
    assert_eq!(world.get::<Intent>(hero).unwrap().movement(), Vec2::ZERO);
    world.insert_resource(actions);
    world.remove_resource::<InputState>();
    step(&mut world, &mut schedule);
    assert_eq!(world.get::<Intent>(hero).unwrap().movement(), Vec2::ZERO);
}

#[test]
fn a_mover_takes_the_intent_as_its_direction() {
    let (mut world, mut schedule) = {
        let mut world = World::new();
        world.insert_resource(actions());
        world.insert_resource(InputState::new());
        world.insert_resource(Time::new());
        let mut schedule = Schedule::new();
        ActionInputPlugin.build(&mut schedule, &mut world);
        TopDownMoverPlugin.build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        (world, schedule)
    };
    let hero = world.spawn_with((
        full(),
        Intent::default(),
        TopDownMover::new(120.0),
        Position::new(0.0, 0.0),
    ));
    world
        .resource_mut::<InputState>()
        .key_down(KeyCode::ArrowRight);
    for _ in 0..30 {
        step(&mut world, &mut schedule);
    }
    let at = world.get::<Position>(hero).unwrap().0;
    assert!((at.x - 60.0).abs() < 0.01, "bodiless, at its speed: {at}");
    assert_eq!(
        world.get::<TopDownMover>(hero).unwrap().direction(),
        Vec2::X
    );
}

#[test]
fn the_load_checks_every_action_against_the_map() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in(ACTION_INPUT_COMPONENT, &value, &world)
            .map_err(|error| error.to_string())
    };
    assert_eq!(
        check(json!({ "move": ["move_left", "move_right"] })),
        Ok(())
    );
    assert_eq!(
        check(json!({
            "move": ["move_left", "move_right", "move_up", "move_down"],
            "aim": ["aim_left", "aim_right", "aim_up", "aim_down"],
            "slots": { "jump": "jump" }
        })),
        Ok(())
    );
    let error = check(json!({ "move": ["move_left", "move_rigth"] })).unwrap_err();
    assert!(
        error.contains("move.1") && error.contains("move_rigth"),
        "{error}"
    );
    let error = check(json!({ "move": ["move_left", "move_right"], "slots": { "jump": "jmp" } }))
        .unwrap_err();
    assert!(
        error.contains("slots.jump") && error.contains("jmp"),
        "{error}"
    );
    let error = check(json!({ "move": ["move_left", "move_right"], "aim": ["aim_left", "aim_right", "aim_up", "ami_down"] }))
        .unwrap_err();
    assert!(error.contains("aim.3"), "{error}");
    let error = check(json!({ "move": ["move_left", "move_right", "move_up"] })).unwrap_err();
    assert!(error.contains("2 actions"), "{error}");
    assert!(check(json!({ "move": ["move_left", "move_right"], "fire": "fire" })).is_err());

    let mut bare = World::new();
    let mut schedule = Schedule::new();
    ActionInputPlugin.build(&mut schedule, &mut bare);
    assert!(
        bare.resource::<ComponentRegistry>()
            .check_in(
                ACTION_INPUT_COMPONENT,
                &json!({ "move": ["a", "b"] }),
                &bare
            )
            .is_ok(),
        "no map, no check"
    );
}

#[test]
fn action_input_is_data_in_the_scene_form() {
    let input = full();
    let value = serde_json::to_value(&input).unwrap();
    assert_eq!(
        value,
        json!({
            "move": ["move_left", "move_right", "move_up", "move_down"],
            "aim": ["aim_left", "aim_right", "aim_up", "aim_down"],
            "slots": { "jump": "jump" }
        })
    );
    assert_eq!(serde_json::from_value::<ActionInput>(value).unwrap(), input);
    let short = ActionInput::new("move_left", "move_right");
    assert_eq!(
        serde_json::to_value(&short).unwrap(),
        json!({ "move": ["move_left", "move_right"], "aim": null, "slots": {} })
    );
    let replaced = short.with_slot("jump", "jump").with_slot("jump", "fire");
    assert_eq!(
        serde_json::to_value(&replaced).unwrap()["slots"],
        json!({ "jump": "fire" })
    );
}

#[test]
fn an_intent_clamps_its_directions_and_keeps_its_slots() {
    let mut intent = Intent::default();
    intent.set_movement(Vec2::new(3.0, 4.0));
    assert!((intent.movement() - Vec2::new(0.6, 0.8)).length() < 1e-6);
    intent.set_movement(Vec2::new(f32::NAN, 0.0));
    assert_eq!(intent.movement(), Vec2::ZERO);
    intent.set_aim(Some(Vec2::new(0.0, 5.0)));
    assert_eq!(intent.aim(), Some(Vec2::new(0.0, 1.0)));
    intent.set_slot("fire", true, false);
    intent.set_slot("dash", false, true);
    intent.set_slot("fire", true, true);
    assert!(intent.held("fire") && intent.pressed("fire"));
    assert!(!intent.held("dash") && intent.pressed("dash"));
    assert!(!intent.held("jump"));

    let value = serde_json::to_value(&intent).unwrap();
    assert_eq!(value["movement"], json!([0.0, 0.0]));
    assert_eq!(value["aim"], json!([0.0, 1.0]));
    assert_eq!(serde_json::from_value::<Intent>(value).unwrap(), intent);
}

#[test]
fn an_intent_read_from_data_is_clamped_and_its_slots_sorted() {
    let intent: Intent = serde_json::from_value(json!({
        "movement": [10.0, 0.0],
        "aim": [0.0, -3.0],
        "slots": [
            { "name": "jump", "held": true, "pressed": false },
            { "name": "fire", "held": false, "pressed": true }
        ]
    }))
    .unwrap();
    assert_eq!(
        intent.movement(),
        Vec2::new(1.0, 0.0),
        "clamped to length 1"
    );
    assert_eq!(intent.aim(), Some(Vec2::new(0.0, -1.0)));
    assert!(
        intent.held("jump") && intent.pressed("fire"),
        "found whatever the order"
    );
    let error = serde_json::from_value::<Intent>(json!({
        "movement": [0.0, 0.0],
        "slots": [
            { "name": "jump", "held": true, "pressed": false },
            { "name": "jump", "held": false, "pressed": false }
        ]
    }))
    .unwrap_err();
    assert!(error.to_string().contains("named twice"), "{error}");
}
