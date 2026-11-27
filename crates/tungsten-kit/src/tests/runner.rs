use std::collections::BTreeMap;

use serde_json::{Value, json};
use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::{CommandBuffer, Schedule, Stage, Time, Transform};

use super::*;
use crate::damage::effects::{Effect, Effects};
use crate::foundation::plugins::KitPlugins;
use crate::foundation::trace::{KitTrace, TraceKind};
use crate::intent::action_input::Intent;
use crate::overlap::interact::{Interactable, Interactor};
use crate::react::counters::{Counters, Flags};
use crate::react::cues::{CueAction, Cues};
use crate::story::dialogue::Node;

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;

/// A World with every kit plugin, a clock, commands, a trace and the
/// speakers `keeper` and `warden`; its keeper and its warden.
fn rig() -> (World, Schedule, Entity, Entity) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let data = world.resource_mut::<DataRegistry>();
    data.insert("keeper", Speaker::new("Old Tamsin").with_pace(30.0));
    data.insert("warden", Speaker::new("Warden"));
    let keeper = world.spawn();
    let warden = world.spawn();
    (world, schedule, keeper, warden)
}

/// `nodes` as the `dialogue` entry `id`, entered at `start`.
fn add(world: &mut World, id: &str, start: &str, nodes: Value) {
    let mut value = json!({ "speaker": "keeper", "start": start });
    value["nodes"] = nodes;
    let conversation: Conversation =
        serde_json::from_value(value).unwrap_or_else(|e| panic!("{id}: {e}"));
    world
        .resource_mut::<DataRegistry>()
        .insert(id, conversation);
}

fn now(world: &World) -> Option<&Talk> {
    world.resource::<Dialogue>().current()
}

/// The shown line's speaker name and text.
fn line(world: &World) -> Option<(String, String)> {
    now(world)
        .and_then(Talk::line)
        .map(|line| (line.name().to_owned(), line.text().to_owned()))
}

fn text(world: &World) -> Option<String> {
    line(world).map(|(_, text)| text)
}

fn offered(world: &World) -> Vec<(usize, String)> {
    now(world).map_or_else(Vec::new, |talk| {
        talk.choices()
            .map(|(index, text)| (index, text.to_owned()))
            .collect()
    })
}

fn game_flag(world: &World, name: &str) -> bool {
    world.resource::<Flags>().has(name)
}

fn ended(world: &World) -> usize {
    world
        .resource::<EventQueue<DialogueEnded>>()
        .iter_current()
        .count()
}

#[test]
fn a_talk_walks_its_lines_and_skips_a_failed_when() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "evening",
        "greet",
        json!({ "greet": { "lines": [
            "Evening, warden.",
            { "text": "Lamps lit already?", "when": { "flag": { "name": "lamp.lit", "target": "game" } } },
            { "speaker": "warden", "text": "Evening." }
        ] } }),
    );
    assert!(talk(&mut world, "evening", keeper, warden));
    let talk_now = now(&world).unwrap();
    assert_eq!(
        (
            talk_now.conversation(),
            talk_now.node(),
            talk_now.speaker(),
            talk_now.listener()
        ),
        ("evening", "greet", keeper, warden)
    );
    assert_eq!(now(&world).unwrap().line().unwrap().pace(), 30.0);
    assert_eq!(
        line(&world),
        Some(("Old Tamsin".into(), "Evening, warden.".into()))
    );
    assert!(Dialogue::advance(&mut world));
    assert_eq!(
        line(&world),
        Some(("Warden".into(), "Evening.".into())),
        "the lamp line skipped"
    );
    assert!(Dialogue::advance(&mut world));
    assert!(now(&world).is_none(), "no exit: the end");
    assert_eq!(ended(&world), 1);
    assert!(!Dialogue::advance(&mut world), "nothing runs");
    assert!(
        !talk(&mut world, "nowhere", keeper, warden),
        "an unknown ID"
    );
    let started = world
        .resource::<EventQueue<DialogueStarted>>()
        .iter_current()
        .count();
    assert_eq!(started, 1);
}

#[test]
fn a_choice_runs_its_list_before_the_next_node_reads_it() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "offer",
        "offer",
        json!({
            "offer": { "lines": ["The lamp is out."], "choices": [
                { "text": "I'll light it.", "do": [{ "flag": { "name": "lamp.promised", "target": "game" } }], "next": "thanks" },
                { "text": "Not tonight.", "back": true }
            ] },
            "thanks": { "lines": [
                { "text": "You promised.", "when": { "flag": { "name": "lamp.promised", "target": "game" } } },
                "Off you go."
            ] }
        }),
    );
    talk(&mut world, "offer", keeper, warden);
    assert!(!Dialogue::choose(&mut world, 0), "no choice shows yet");
    Dialogue::advance(&mut world);
    assert_eq!(
        offered(&world),
        [(0, "I'll light it.".into()), (1, "Not tonight.".into())]
    );
    assert_eq!(now(&world).unwrap().back(), Some(1));
    assert!(now(&world).unwrap().line().is_none());
    assert!(!Dialogue::advance(&mut world), "refused while choosing");
    assert!(!Dialogue::choose(&mut world, 5), "not offered");
    assert!(Dialogue::choose(&mut world, 0));
    assert_eq!(text(&world).as_deref(), Some("You promised."));
}

#[test]
fn a_node_list_runs_as_it_is_entered_with_the_speaker_as_subject() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "greet",
        "greet",
        json!({
            "greet": { "do": [
                { "counter": { "name": "greeted", "add": 1 } },
                { "flag": { "name": "met.keeper", "target": "other" } }
            ], "lines": ["Evening."], "next": "again" },
            "again": { "do": [{ "counter": { "name": "greeted", "add": 1 } }], "lines": ["Still here?"] }
        }),
    );
    talk(&mut world, "greet", keeper, warden);
    let count = |world: &World| {
        world
            .get::<Counters>(keeper)
            .map_or(0, |c| c.get("greeted"))
    };
    assert_eq!(count(&world), 1, "on entering, before the line shows");
    assert!(world.get::<Flags>(warden).unwrap().has("met.keeper"));
    Dialogue::advance(&mut world);
    assert_eq!(count(&world), 2);
}

#[test]
fn a_branch_takes_its_first_holding_entry() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "fork",
        "fork",
        json!({
            "fork": { "branch": [
                { "when": { "flag": { "name": "night", "target": "game" } }, "next": "night" },
                { "when": { "counter": { "name": "visits", "min": 2, "target": "other" } }, "next": "regular" },
                { "next": "day" }
            ] },
            "night": { "lines": ["Late, isn't it?"] },
            "regular": { "lines": ["You again."] },
            "day": { "lines": ["Good day."] }
        }),
    );
    talk(&mut world, "fork", keeper, warden);
    assert_eq!(text(&world).as_deref(), Some("Good day."));
    Dialogue::end(&mut world);
    world.insert(warden, Counters::new().with("visits", 2));
    talk(&mut world, "fork", keeper, warden);
    assert_eq!(text(&world).as_deref(), Some("You again."));
    Dialogue::end(&mut world);
    world.resource_mut::<Flags>().set("night", true);
    talk(&mut world, "fork", keeper, warden);
    assert_eq!(
        text(&world).as_deref(),
        Some("Late, isn't it?"),
        "the first that holds"
    );
}

#[test]
fn a_second_talk_is_refused_while_one_runs() {
    let (mut world, mut schedule, keeper, warden) = rig();
    add(
        &mut world,
        "a",
        "a",
        json!({ "a": { "lines": ["First."] } }),
    );
    add(
        &mut world,
        "b",
        "b",
        json!({ "b": { "lines": ["Second."] } }),
    );
    assert!(talk(&mut world, "a", keeper, warden));
    assert!(!talk(&mut world, "b", warden, keeper));
    assert_eq!(text(&world).as_deref(), Some("First."));
    // The cue action is skipped and traced while it runs: the warden
    // presses at the keeper, whose `interacted` cue starts `b`.
    let dialogue: CueAction = serde_json::from_value(json!({ "dialogue": "b" })).unwrap();
    world.insert(keeper, Transform::from_position(Vec2::ZERO));
    world.insert(keeper, Interactable::new("Talk"));
    world.insert(keeper, Cues::new().on("interacted", [dialogue]));
    world.insert(warden, Transform::from_position(Vec2::new(10.0, 0.0)));
    world.insert(warden, Interactor::new());
    let mut intent = Intent::default();
    intent.set_slot("interact", true, true);
    world.insert(warden, intent);
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert_eq!(text(&world).as_deref(), Some("First."));
    let skipped = world.resource::<KitTrace>().iter().any(|entry| {
        matches!(entry.kind, TraceKind::Skipped { action: "dialogue", why, .. } if why == "a conversation runs")
    });
    assert!(skipped);
    let dead = world.spawn();
    world.despawn(dead);
    Dialogue::end(&mut world);
    assert!(!talk(&mut world, "a", keeper, dead), "a dead listener");
}

#[test]
fn a_dead_listener_ends_the_talk() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "a",
        "a",
        json!({ "a": { "lines": ["One.", "Two."] } }),
    );
    talk(&mut world, "a", keeper, warden);
    world.despawn(warden);
    assert!(!Dialogue::advance(&mut world));
    assert!(now(&world).is_none());
    assert_eq!(ended(&world), 1);
    assert!(!Dialogue::choose(&mut world, 0));
}

#[test]
fn a_flag_from_a_choice_reads_back_at_once_while_the_clock_holds() {
    let (mut world, _, keeper, warden) = rig();
    world.resource_mut::<Time>().set_paused(true);
    add(
        &mut world,
        "ask",
        "ask",
        json!({
            "ask": { "choices": [
                { "text": "Who keeps the lamps?", "when": { "not": { "flag": { "name": "asked", "target": "game" } } },
                  "do": [{ "flag": { "name": "asked", "target": "game" } }, { "counter": { "name": "asks", "add": 1, "target": "other" } }], "next": "ask" },
                { "text": "Goodbye." }
            ] }
        }),
    );
    talk(&mut world, "ask", keeper, warden);
    assert_eq!(offered(&world).len(), 2);
    Dialogue::choose(&mut world, 0);
    assert!(
        game_flag(&world, "asked"),
        "written at the call, no step run"
    );
    assert_eq!(world.get::<Counters>(warden).unwrap().get("asks"), 1);
    assert_eq!(
        offered(&world),
        [(1, "Goodbye.".into())],
        "the asked choice gone"
    );
}

#[test]
fn an_effect_from_a_choice_waits_for_the_next_step() {
    let (mut world, mut schedule, keeper, warden) = rig();
    world
        .resource_mut::<DataRegistry>()
        .insert("warmed", Effect::new().with_duration(5.0));
    add(
        &mut world,
        "warm",
        "warm",
        json!({ "warm": { "choices": [{ "text": "Warm me.", "do": [{ "effect": { "id": "warmed", "target": "other" } }] }] } }),
    );
    talk(&mut world, "warm", keeper, warden);
    Dialogue::choose(&mut world, 0);
    let warmed = |world: &World| {
        world
            .get::<Effects>(warden)
            .is_some_and(|e| e.has("warmed"))
    };
    assert!(!warmed(&world), "queued for the effect resolve");
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert!(warmed(&world), "applied at the next fixed step");
}

#[test]
fn the_jump_bound_ends_a_runaway_inline_talk() {
    let (mut world, _, keeper, warden) = rig();
    // Two nodes that show nothing, jumping to each other: past the parse's
    // checks, as only data built in Rust can be.
    let node = |next: &str| -> Node {
        serde_json::from_value(json!({ "do": [{ "counter": { "name": "entered", "add": 1, "target": "game" } }], "next": next })).unwrap()
    };
    let nodes = BTreeMap::from([("a".to_owned(), node("b")), ("b".to_owned(), node("a"))]);
    let runaway = Conversation::unchecked("keeper", "a", nodes);
    assert!(start_talk(
        &mut world,
        "runaway".into(),
        runaway,
        keeper,
        warden
    ));
    assert!(now(&world).is_none(), "ended");
    assert_eq!(ended(&world), 1);
    assert_eq!(
        world.resource::<Counters>().get("entered"),
        MAX_ENTRIES as i64,
        "64 nodes entered, each list run"
    );
}

#[test]
fn a_list_started_inside_a_list_runs_before_the_call_returns() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "inner",
        "hi",
        json!({ "hi": { "do": [{ "counter": { "name": "n", "add": 10, "target": "game" } }], "lines": ["Hi."] } }),
    );
    let outer: Vec<CueAction> = serde_json::from_value(json!([
        { "dialogue": "inner" },
        { "counter": { "name": "n", "set": 1, "target": "game" } }
    ]))
    .unwrap();
    run_list(&mut world, &outer, Some(keeper), Some(warden), Vec2::ZERO);
    assert_eq!(
        world.resource::<Counters>().get("n"),
        11,
        "the outer list first, then the node's, before the call returned"
    );
    assert_eq!(text(&world).as_deref(), Some("Hi."));
}

#[test]
fn a_call_drains_at_most_64_lists() {
    let (mut world, _, keeper, warden) = rig();
    // Each talk's node shows nothing and ends; its list, run after it,
    // starts the next talk.
    add(
        &mut world,
        "again",
        "again",
        json!({ "again": { "do": [
            { "counter": { "name": "lists", "add": 1, "target": "game" } },
            { "dialogue": "again" }
        ] } }),
    );
    let first: Vec<CueAction> = serde_json::from_value(json!([{ "dialogue": "again" }])).unwrap();
    run_list(&mut world, &first, Some(keeper), Some(warden), Vec2::ZERO);
    assert_eq!(
        world.resource::<Counters>().get("lists"),
        63,
        "the call's own list and 63 it started"
    );
    let dropped = world.resource::<KitTrace>().iter().any(|entry| {
        matches!(entry.kind, TraceKind::Skipped { why, .. } if why == "one call ran its 64 lists")
    });
    assert!(dropped);
    // The next call starts afresh.
    run_list(&mut world, &first, Some(keeper), Some(warden), Vec2::ZERO);
    assert_eq!(world.resource::<Counters>().get("lists"), 126);
}

#[test]
fn a_restore_ends_a_running_talk_without_its_lists() {
    let (mut world, _, keeper, warden) = rig();
    add(
        &mut world,
        "a",
        "a",
        json!({ "a": { "lines": ["One."], "next": "b" }, "b": { "do": [{ "flag": { "name": "b.entered", "target": "game" } }], "lines": ["Two."] } }),
    );
    let saved = capture_world(&world).unwrap();
    talk(&mut world, "a", keeper, warden);
    restore_world(&mut world, &saved, &(Restored,)).unwrap();
    assert!(now(&world).is_none(), "the restore ended it");
    assert_eq!(ended(&world), 1);
    assert!(!game_flag(&world, "b.entered"), "no list ran");
    assert!(!Dialogue::advance(&mut world));
}
