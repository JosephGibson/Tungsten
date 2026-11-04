use super::*;
use serde_json::{Value, json};
use tungsten_core::{
    AudioCommand, AudioCommands, CommandBuffer, DataRegistry, Entity, Plugin, PrefabData,
    PrefabRegistry, Schedule, ShakeEvent, SoundData, SoundRegistry, Stage, Tag, Time, Transform,
};

use crate::foundation::trace::KitTrace;
use crate::react::cues::check_world_cue;
use crate::{Counters, CueAction, CueEvent, CueListener, Cues, CuesOnEvent, CuesPlugin};

/// A game event about an entity, played before the world cues.
struct Rang(Entity);

#[allow(clippy::unnecessary_wraps)]
fn rang(event: &Rang) -> Option<CueEvent> {
    Some(CueEvent::of(event.0))
}

/// A World with the cue plugin, a clock, a command buffer, a trace, audio,
/// the shake queue, the prefab `puff`, the sound `blip` and a listener at
/// the origin that hears whole within 100 px and nothing from 500.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.insert_resource(AudioCommands::new());
    world.insert_resource(CueListener::new(100.0, 500.0));
    world.register_event::<ShakeEvent>();
    let mut prefabs = PrefabRegistry::new();
    let puff: PrefabData = serde_json::from_value(json!({ "tag": "puff" })).unwrap();
    prefabs.insert("puff".to_string(), puff);
    world.insert_resource(prefabs);
    let mut sounds = SoundRegistry::new();
    let blip = SoundData {
        samples: vec![0.0; 4],
        sample_rate: 44_100,
        channels: 1,
    };
    sounds.register("blip".to_string(), blip, 1.0, false);
    world.insert_resource(sounds);
    let mut schedule = Schedule::new();
    CuesPlugin.build(&mut schedule, &mut world);
    CuesOnEvent::<Rang>::new("rang", rang).build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

fn entry(value: Value) -> WorldCueActions {
    serde_json::from_value(value).unwrap()
}

fn insert(world: &mut World, name: &str, value: Value) {
    let cue = entry(value);
    check_world_cue(&cue, world).unwrap();
    world.resource_mut::<DataRegistry>().insert(name, cue);
}

fn game(world: &World, name: &str) -> i64 {
    world
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get(name))
}

#[test]
fn a_world_cue_plays_at_its_point() {
    let (mut world, mut schedule) = rig();
    insert(
        &mut world,
        "thunder",
        json!([
            { "prefab": "puff" },
            { "sound": { "id": "blip" } },
            { "shake": { "trauma": 0.8, "near": 100, "far": 500 } },
            { "counter": { "name": "strikes", "add": 1, "target": "game" } }
        ]),
    );
    WorldCue::new("thunder", Vec2::new(300.0, 0.0)).play(&mut world);
    step(&mut world, &mut schedule);
    let puffs: Vec<Vec2> = world
        .query::<(&Tag, &Transform)>()
        .map(|(_, transform)| transform.position)
        .collect();
    assert_eq!(puffs, [Vec2::new(300.0, 0.0)], "at its point");
    let played: Vec<f32> = world
        .resource_mut::<AudioCommands>()
        .drain()
        .into_iter()
        .filter_map(|command| match command {
            AudioCommand::Play { volume, .. } => Some(volume),
            _ => None,
        })
        .collect();
    assert_eq!(played, [0.25], "by the listener");
    let shakes: Vec<f32> = world
        .resource::<EventQueue<ShakeEvent>>()
        .iter()
        .map(|shake| shake.trauma_add)
        .collect();
    assert_eq!(shakes, [0.4], "by its falloff");
    assert_eq!(game(&world, "strikes"), 1);
    let traced = world
        .resource::<KitTrace>()
        .about(Entity::PLACEHOLDER)
        .count();
    assert_eq!(
        traced, 5,
        "four actions and the spawn, under the placeholder"
    );
    // Two plays in a step run in send order, after the step's occurrences.
    insert(
        &mut world,
        "first",
        json!([{ "counter": { "name": "order", "set": 1, "target": "game" } }]),
    );
    insert(
        &mut world,
        "second",
        json!([{ "counter": { "name": "order", "add": 10, "target": "game" } }]),
    );
    let bell = world
        .spawn_with((Cues::new().on("rang", [CueAction::counter_set("order", 100).for_game()]),));
    WorldCue::new("first", Vec2::ZERO).play(&mut world);
    world.resource_mut::<EventQueue<Rang>>().send(Rang(bell));
    WorldCue::new("second", Vec2::ZERO).play(&mut world);
    step(&mut world, &mut schedule);
    assert_eq!(
        game(&world, "order"),
        11,
        "the bell's 100, then 1, then +10"
    );
}

#[test]
fn a_world_cue_refuses_subject_actions() {
    let (world, _) = rig();
    for (value, field) in [
        (json!([{ "flash": {} }]), "0.flash"),
        (
            json!([{ "prefab": "puff" }, { "despawn": {} }]),
            "1.despawn",
        ),
        (json!([{ "effect": "burning" }]), "0.effect"),
        (json!([{ "loop": { "id": "blip" } }]), "0.loop"),
        (json!([{ "stop": { "id": "blip" } }]), "0.stop"),
        (json!([{ "squash": "on_hit" }]), "0.squash"),
        (json!([{ "ability": "wand" }]), "0.ability"),
        (json!([{ "label": {} }]), "0.label"),
        (
            json!([{ "counter": { "name": "hits", "add": 1 } }]),
            "0.counter.target",
        ),
        (
            json!([{ "flag": { "name": "lit", "target": "other" } }]),
            "0.flag.target",
        ),
        (json!([{ "prefab": "nope" }]), "0.prefab"),
        (json!([{ "sound": { "id": "nope" } }]), "0.sound.id"),
    ] {
        let error = check_world_cue(&entry(value.clone()), &world).unwrap_err();
        assert_eq!(error.field, field, "{value}");
    }
    let fine = entry(json!([
        { "label": { "font": "sans" } },
        { "flag": { "name": "storm", "target": "game" } }
    ]));
    assert!(check_world_cue(&fine, &world).is_ok());
}

#[test]
fn no_world_cue_queue_before_the_first_play() {
    let (mut world, mut schedule) = rig();
    for _ in 0..10 {
        step(&mut world, &mut schedule);
    }
    assert!(!world.has_event::<WorldCue>());
    WorldCue::new("thunder", Vec2::ZERO).play(&mut world);
    assert!(world.has_event::<WorldCue>());
    step(&mut world, &mut schedule);
}
