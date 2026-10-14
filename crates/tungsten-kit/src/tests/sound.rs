use super::*;
use tungsten_core::{AudioCommand, SoundData};

/// A hit with the sound it names.
struct Hit {
    sound: Option<&'static str>,
}

fn sound() -> SoundData {
    SoundData {
        samples: vec![0.0; 4],
        sample_rate: 44_100,
        channels: 1,
    }
}

/// A World with the `thud` (volume 0.5) and `clang` (volume 0.8) sounds.
fn world() -> (World, AudioHandle, AudioHandle) {
    let mut sounds = SoundRegistry::new();
    let thud = sounds.register("thud".to_string(), sound(), 0.5, false);
    let clang = sounds.register("clang".to_string(), sound(), 0.8, true);
    let mut world = World::new();
    world.insert_resource(sounds);
    world.insert_resource(AudioCommands::new());
    (world, thud, clang)
}

fn send(world: &mut World, sound: Option<&'static str>) {
    world.resource_mut::<EventQueue<Hit>>().send(Hit { sound });
}

/// One frame: `update`, `post_update`, the event flush; returns what played.
fn frame(world: &mut World, schedule: &mut Schedule) -> Vec<(AudioHandle, f32, bool)> {
    schedule.run_stage(Stage::Update, world, |_, _| {});
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    world.flush_events();
    world
        .resource_mut::<AudioCommands>()
        .drain()
        .into_iter()
        .map(|command| match command {
            AudioCommand::Play {
                handle,
                volume,
                looping,
            } => (handle, volume, looping),
            other => panic!("unexpected {other:?}"),
        })
        .collect()
}

fn build(world: &mut World, plugins: &[SoundOnEvent<Hit>]) -> Schedule {
    let mut schedule = Schedule::new();
    for plugin in plugins {
        plugin.build(&mut schedule, world);
    }
    schedule.resolve().unwrap();
    schedule
}

#[test]
fn the_plugin_registers_the_queue_and_its_system_under_the_name() {
    let (mut world, _, _) = world();
    world.register_event::<Hit>();
    let schedule = build(&mut world, &[SoundOnEvent::new("hit_sound", "thud")]);
    assert!(world.has_event::<Hit>());
    assert_eq!(world.registered_event_count(), 1);
    assert_eq!(schedule.stage_of("hit_sound"), Some(Stage::PostUpdate));
    assert_eq!(schedule.len(), 1);
}

#[test]
fn each_event_plays_at_the_manifest_volume_never_looping() {
    let (mut world, thud, _) = world();
    let mut schedule = build(&mut world, &[SoundOnEvent::new("hit_sound", "thud")]);
    send(&mut world, None);
    send(&mut world, None);
    assert_eq!(
        frame(&mut world, &mut schedule),
        [(thud, 0.5, false), (thud, 0.5, false)]
    );
    assert_eq!(frame(&mut world, &mut schedule), []);
}

#[test]
fn an_event_sent_before_plays_that_frame_and_one_sent_after_the_next() {
    let (mut world, thud, clang) = world();
    let mut schedule = Schedule::new();
    schedule.add(
        Stage::Update,
        system("early", |world: &mut World| send(world, Some("thud"))),
    );
    SoundOnEvent::<Hit>::select("hit_sound", |hit| hit.sound).build(&mut schedule, &mut world);
    schedule.add(
        Stage::PostUpdate,
        system("late", |world: &mut World| send(world, Some("clang"))).after("hit_sound"),
    );
    schedule.resolve().unwrap();
    assert_eq!(frame(&mut world, &mut schedule), [(thud, 0.5, false)]);
    // The late hit of the frame before, then this frame's early one.
    assert_eq!(
        frame(&mut world, &mut schedule),
        [(clang, 0.8, false), (thud, 0.5, false)]
    );
    assert_eq!(
        frame(&mut world, &mut schedule),
        [(clang, 0.8, false), (thud, 0.5, false)]
    );
}

#[test]
fn select_picks_per_event_and_none_plays_nothing() {
    let (mut world, thud, clang) = world();
    let mut schedule = build(
        &mut world,
        &[SoundOnEvent::select("hit_sound", |hit: &Hit| hit.sound)],
    );
    send(&mut world, Some("clang"));
    send(&mut world, None);
    send(&mut world, Some("thud"));
    assert_eq!(
        frame(&mut world, &mut schedule),
        [(clang, 0.8, false), (thud, 0.5, false)]
    );
}

#[test]
fn an_unknown_sound_plays_nothing_and_the_rest_still_play() {
    let (mut world, thud, _) = world();
    let mut schedule = build(
        &mut world,
        &[SoundOnEvent::select("hit_sound", |hit: &Hit| hit.sound)],
    );
    send(&mut world, Some("missing"));
    send(&mut world, Some("thud"));
    send(&mut world, Some("missing"));
    assert_eq!(frame(&mut world, &mut schedule), [(thud, 0.5, false)]);
    send(&mut world, Some("missing"));
    assert_eq!(frame(&mut world, &mut schedule), []);
}

#[test]
fn without_a_registry_or_commands_nothing_plays_and_nothing_replays() {
    let (mut world, thud, _) = world();
    let mut schedule = build(&mut world, &[SoundOnEvent::new("hit_sound", "thud")]);
    let sounds = world.remove_resource::<SoundRegistry>().unwrap();
    send(&mut world, None);
    assert_eq!(frame(&mut world, &mut schedule), []);
    world.insert_resource(sounds);
    assert_eq!(frame(&mut world, &mut schedule), []);

    let commands = world.remove_resource::<AudioCommands>().unwrap();
    send(&mut world, None);
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    world.flush_events();
    world.insert_resource(commands);
    assert_eq!(frame(&mut world, &mut schedule), []);
    send(&mut world, None);
    assert_eq!(frame(&mut world, &mut schedule), [(thud, 0.5, false)]);
}

#[test]
fn two_plugins_with_two_names_each_play_once() {
    let (mut world, thud, clang) = world();
    let mut schedule = build(
        &mut world,
        &[
            SoundOnEvent::new("hit_thud", "thud"),
            SoundOnEvent::new("hit_clang", "clang"),
        ],
    );
    send(&mut world, None);
    assert_eq!(
        frame(&mut world, &mut schedule),
        [(thud, 0.5, false), (clang, 0.8, false)]
    );
    assert_eq!(frame(&mut world, &mut schedule), []);
}

#[test]
fn a_frame_without_the_queue_does_nothing() {
    let (mut world, _, _) = world();
    let mut schedule = build(&mut world, &[SoundOnEvent::new("hit_sound", "thud")]);
    world.remove_resource::<EventQueue<Hit>>();
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    assert!(world.resource_mut::<AudioCommands>().drain().is_empty());
}

#[test]
fn debug_names_the_system_and_the_sound() {
    let every = SoundOnEvent::<Hit>::new("hit_sound", "thud");
    assert_eq!(
        format!("{every:?}"),
        r#"SoundOnEvent { name: "hit_sound", sound: "thud" }"#
    );
    let select = SoundOnEvent::<Hit>::select("hit_sound", |hit| hit.sound).clone();
    assert_eq!(
        format!("{select:?}"),
        r#"SoundOnEvent { name: "hit_sound", sound: "selected per event" }"#
    );
}
