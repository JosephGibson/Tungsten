use super::*;
use tungsten_core::input::GamepadButton;
use tungsten_core::ui::ui_route_system;
use tungsten_core::{Binding, GamepadId, InputState, KeyCode, Stage, Time, Transform};

use crate::foundation::trace::KitTrace;
use crate::intent::action_input::ActionInputPlugin;
use crate::react::counters::{CountersPlugin, Flags};
use crate::react::cues::{CueAction, Cues, CuesPlugin};

const E: Binding = Binding::Key {
    code: KeyCode::KeyE,
};
const PAD_WEST: Binding = Binding::GamepadButton {
    button: GamepadButton::West,
};

fn world() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(WorldLabels::default());
    let mut schedule = Schedule::new();
    InteractPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn at(x: f32, y: f32) -> Transform {
    Transform::from_position(Vec2::new(x, y))
}

/// One fixed step and one frame's presentation; the step's presses.
fn step(world: &mut World, schedule: &mut Schedule) -> Vec<Interacted> {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let sent = world
        .resource::<EventQueue<Interacted>>()
        .iter_current()
        .copied()
        .collect();
    world.flush_events();
    sent
}

fn focus(world: &World, interactor: Entity) -> Option<Entity> {
    world.get::<Interactor>(interactor).unwrap().focus()
}

fn press(world: &mut World, interactor: Entity, down: bool) {
    world
        .get_mut::<Intent>(interactor)
        .unwrap()
        .set_slot("interact", down, down);
}

/// The frame's prompts: (text, anchor, offset).
fn prompts(world: &mut World, schedule: &mut Schedule) -> Vec<(String, Vec2, Vec2)> {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    world
        .resource_mut::<WorldLabels>()
        .drain()
        .map(|label| (label.text, label.anchor, label.offset))
        .collect()
}

#[test]
fn the_nearest_interactable_in_reach_takes_the_focus() {
    let (mut world, mut schedule) = world();
    let warden = world.spawn_with((at(0.0, 0.0), Interactor::new(), Intent::default()));
    let lamp = world.spawn_with((at(0.0, 40.0), Interactable::new("Light")));
    let keeper = world.spawn_with((at(30.0, 0.0), Interactable::new("Talk")));
    let _far = world.spawn_with((at(100.0, 0.0), Interactable::new("Read")));
    step(&mut world, &mut schedule);
    assert_eq!(
        focus(&world, warden),
        Some(keeper),
        "the nearer of two in reach"
    );

    world.despawn(keeper);
    step(&mut world, &mut schedule);
    assert_eq!(focus(&world, warden), Some(lamp));

    // A radius widens the reach: 60 px off, reach 48 plus 20.
    world.despawn(lamp);
    let board = world.spawn_with((at(-60.0, 0.0), Interactable::new("Read").with_radius(20.0)));
    step(&mut world, &mut schedule);
    assert_eq!(focus(&world, warden), Some(board));

    // A tie goes to the earlier in the step's order.
    let left = world.spawn_with((at(-20.0, 0.0), Interactable::new("Talk")));
    let right = world.spawn_with((at(20.0, 0.0), Interactable::new("Talk")));
    step(&mut world, &mut schedule);
    let first = if left.id() < right.id() { left } else { right };
    assert_eq!(focus(&world, warden), Some(first));

    // Out of reach of everything: no focus.
    world.get_mut::<Transform>(warden).unwrap().position = Vec2::new(500.0, 0.0);
    step(&mut world, &mut schedule);
    assert_eq!(focus(&world, warden), None);
    assert_eq!(Interactor::new().with_reach(0.0).reach(), 0.0);
}

#[test]
fn a_failed_when_or_who_hides_an_interactable() {
    let (mut world, mut schedule) = world();
    CountersPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let warden = world.spawn_with((
        at(0.0, 0.0),
        Interactor::new(),
        Intent::default(),
        Tags::new(["team.player"]),
    ));
    let lamp = world.spawn_with((
        at(10.0, 0.0),
        Interactable::new("Light")
            .with_when(Condition::not(Condition::flag("lamp.lit").for_game())),
    ));
    let guard = world.spawn_with((
        at(20.0, 0.0),
        Interactable::new("Talk").with_who(TagFilter::new().all(["team.npc"])),
    ));
    let keeper = world.spawn_with((
        at(30.0, 0.0),
        Interactable::new("Talk").with_when(Condition::flag("trusted").for_other()),
    ));
    step(&mut world, &mut schedule);
    assert_eq!(focus(&world, warden), Some(lamp));

    world.resource_mut::<Flags>().set("lamp.lit", true);
    step(&mut world, &mut schedule);
    assert_eq!(
        focus(&world, warden),
        None,
        "the lamp lit, the guard's who fails and the keeper's when"
    );

    world.insert(warden, Flags::new().with("trusted"));
    step(&mut world, &mut schedule);
    assert_eq!(focus(&world, warden), Some(keeper), "past the guard");
    let _ = guard;
}

#[test]
fn one_press_sends_one_interacted_cue() {
    let (mut world, mut schedule) = world();
    CountersPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    world.insert_resource(KitTrace::default());
    let warden = world.spawn_with((at(0.0, 0.0), Interactor::new(), Intent::default()));
    let notice = world.spawn_with((
        at(16.0, 8.0),
        Interactable::new("Read"),
        Cues::new().on(
            "interacted",
            [
                CueAction::counter_add("reads", 1).for_other(),
                CueAction::flag("read", true),
            ],
        ),
    ));
    let reads = |world: &World| {
        world
            .get::<crate::react::counters::Counters>(warden)
            .map_or(0, |counters| counters.get("reads"))
    };

    step(&mut world, &mut schedule);
    assert_eq!(reads(&world), 0, "no press yet");
    press(&mut world, warden, true);
    let sent = step(&mut world, &mut schedule);
    assert_eq!(
        sent,
        [Interacted::new(notice, warden, Vec2::new(16.0, 8.0))]
    );
    assert_eq!(reads(&world), 1, "the interactor is the cue's other");
    assert!(world.get::<Flags>(notice).unwrap().has("read"));
    // Held, not pressed again: nothing more.
    world
        .get_mut::<Intent>(warden)
        .unwrap()
        .set_slot("interact", true, false);
    assert!(step(&mut world, &mut schedule).is_empty());
    assert_eq!(reads(&world), 1);
    // A press with no focus sends nothing.
    world.get_mut::<Transform>(warden).unwrap().position = Vec2::new(400.0, 0.0);
    press(&mut world, warden, true);
    assert!(step(&mut world, &mut schedule).is_empty());
    assert_eq!(reads(&world), 1);
    let traced = world
        .resource::<KitTrace>()
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind,
                TraceKind::Event {
                    name: "interacted",
                    ..
                }
            )
        })
        .count();
    assert_eq!(traced, 1);
}

#[test]
fn the_prompt_reads_the_binding_for_the_last_device() {
    let (mut world, mut schedule) = world();
    ActionInputPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let mut actions = ActionMap::new();
    actions.replace_bindings("use", vec![E, PAD_WEST]);
    world.insert_resource(actions);
    world.insert_resource(InputState::new());
    let warden = world.spawn_with((
        at(0.0, 0.0),
        Interactor::new(),
        ActionInput::new("left", "right").with_slot("interact", "use"),
        Intent::default(),
    ));
    let keeper = world.spawn_with((
        at(20.0, 0.0),
        Interactable::new("Talk").with_offset(Vec2::new(0.0, -24.0)),
    ));
    assert!(
        prompts(&mut world, &mut schedule).is_empty(),
        "no focus yet"
    );
    step(&mut world, &mut schedule);
    let keys = format!("{} Talk", E.label());
    assert_eq!(
        prompts(&mut world, &mut schedule),
        [(keys.clone(), Vec2::new(20.0, 0.0), Vec2::new(0.0, -24.0))]
    );

    // A pad press routed: the pad's binding.
    world
        .resource_mut::<InputState>()
        .gamepad_button_down(GamepadId::new(0), GamepadButton::South);
    ui_route_system(&mut world);
    assert_eq!(
        world.resource::<UiRouter>().last_device(),
        InputDevice::Gamepad
    );
    let pad = format!("{} Talk", PAD_WEST.label());
    assert_ne!(pad, keys);
    assert_eq!(prompts(&mut world, &mut schedule)[0].0, pad);

    // Rebound: the prompt follows the map.
    world
        .resource_mut::<ActionMap>()
        .replace_bindings("use", vec![E]);
    assert_eq!(
        prompts(&mut world, &mut schedule)[0].0,
        keys,
        "no pad binding"
    );

    // No slot binding: the slot's own name as the action, unbound.
    world.remove_component::<ActionInput>(warden);
    world.get_mut::<Interactor>(warden).unwrap().prompt = None;
    assert_eq!(prompts(&mut world, &mut schedule)[0].0, "Unbound Talk");

    // The focus gone: no prompt.
    world.despawn(keeper);
    assert!(prompts(&mut world, &mut schedule).is_empty());
}

#[test]
fn a_world_without_an_interactor_queries_nothing() {
    let (mut world, mut schedule) = world();
    let lamp = world.spawn_with((at(0.0, 0.0), Interactable::new("Light")));
    let walker = world.spawn_with((at(4.0, 0.0), Intent::default()));
    press(&mut world, walker, true);
    for _ in 0..3 {
        assert!(step(&mut world, &mut schedule).is_empty());
        assert!(prompts(&mut world, &mut schedule).is_empty());
    }
    assert!(!world.has_any::<Interactor>());
    // The gate is the holder's presence: the scratch lists stay empty.
    let _ = lamp;

    // An interactor arriving is served from its first step.
    world.insert(walker, Interactor::new());
    step(&mut world, &mut schedule);
    assert_eq!(focus(&world, walker), Some(lamp));
}
