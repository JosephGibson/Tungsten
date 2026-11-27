use serde_json::{Value, json};
use tungsten_core::save::{self as saves, capture_world, restore_world};
use tungsten_core::{CommandBuffer, PrefabRegistry, Stage, Time};

use super::*;
use crate::foundation::plugins::KitPlugins;
use crate::foundation::trace::{KitTrace, TraceKind};
use crate::react::counters::Flags;
use crate::react::cues::{MAX_LISTS, MAX_QUEST_DEPTH};

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn parse(value: Value) -> Result<Quest, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn add(world: &mut World, id: &str, value: Value) {
    let quest = parse(value).unwrap_or_else(|e| panic!("{id}: {e}"));
    world.resource_mut::<DataRegistry>().insert(id, quest);
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// `holder`'s quest `id`: (status, stage).
fn state(world: &World, holder: Holder, id: &str) -> Option<(QuestStatus, String)> {
    quests_of(world, holder)?
        .get(id)
        .map(|state| (state.status(), state.stage().to_owned()))
}

fn active(stage: &str) -> (QuestStatus, String) {
    (QuestStatus::Active, stage.to_owned())
}

fn game_count(world: &World, name: &str) -> i64 {
    world.resource::<Counters>().get(name)
}

fn traced(world: &World, why: &str) -> usize {
    world
        .resource::<KitTrace>()
        .iter()
        .filter(|entry| matches!(entry.kind, TraceKind::Skipped { why: seen, .. } if seen == why))
        .count()
}

/// The plan's lamplight, its first stage's objective a game flag.
fn lamplight() -> Value {
    json!({
        "title": "Lamplight",
        "start": "light",
        "stages": {
            "light": { "journal": "Light the lamp by the steps.",
                       "objectives": [{ "text": "Lamp lit", "when": { "flag": { "name": "lamp.lit", "target": "game" } } }],
                       "next": "report",
                       "rewards": [{ "counter": { "name": "lit", "add": 1 } }] },
            "report": { "journal": "Tell Tamsin it burns.",
                        "rewards": [{ "counter": { "name": "coins", "add": 5, "target": "game" } }] }
        }
    })
}

fn holder(world: &mut World, quests: Value) -> Entity {
    let quests: Quests = serde_json::from_value(quests).unwrap();
    world.spawn_with((quests,))
}

#[test]
fn a_stage_completes_when_its_objectives_hold() {
    let (mut world, mut schedule) = rig();
    add(&mut world, "lamplight", lamplight());
    let warden = holder(&mut world, json!({ "lamplight": { "stage": "light" } }));
    step(&mut world, &mut schedule);
    assert_eq!(
        state(&world, Some(warden), "lamplight"),
        Some(active("light"))
    );
    world.resource_mut::<Flags>().set("lamp.lit", true);
    step(&mut world, &mut schedule);
    assert_eq!(
        state(&world, Some(warden), "lamplight"),
        Some(active("report"))
    );
    assert_eq!(
        world.get::<Counters>(warden).unwrap().get("lit"),
        1,
        "its reward"
    );
    for _ in 0..5 {
        step(&mut world, &mut schedule);
    }
    assert_eq!(
        state(&world, Some(warden), "lamplight"),
        Some(active("report")),
        "a stage without objectives waits for `advance`"
    );
}

#[test]
fn rewards_run_on_the_holder_and_skip_a_subject_on_the_games() {
    let (mut world, mut schedule) = rig();
    add(
        &mut world,
        "watch",
        json!({ "title": "Watch", "start": "night", "stages": { "night": {
            "journal": "Keep watch.",
            "objectives": [{ "text": "Dawn", "when": { "flag": { "name": "dawn", "target": "game" } } }],
            "rewards": [
                { "counter": { "name": "watched", "add": 1 } },
                { "counter": { "name": "nights", "add": 1, "target": "game" } }
            ] } } }),
    );
    let warden = holder(&mut world, json!({ "watch": { "stage": "night" } }));
    world.insert_resource(
        serde_json::from_value::<Quests>(json!({ "watch": { "stage": "night" } })).unwrap(),
    );
    world.resource_mut::<Flags>().set("dawn", true);
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Counters>(warden).unwrap().get("watched"),
        1,
        "on the holder"
    );
    assert_eq!(
        game_count(&world, "nights"),
        2,
        "both forms' game rewards ran"
    );
    assert_eq!(
        game_count(&world, "watched"),
        0,
        "the game's quest has no subject"
    );
    assert_eq!(traced(&world, "the list runs with no subject"), 1);
    assert_eq!(
        state(&world, None, "watch"),
        Some((QuestStatus::Done, "night".into()))
    );
}

#[test]
fn one_stage_a_quest_a_step() {
    let (mut world, mut schedule) = rig();
    let always = json!({ "text": "Always", "when": { "not": { "flag": "never" } } });
    add(
        &mut world,
        "road",
        json!({ "title": "Road", "start": "a", "stages": {
            "a": { "journal": "a", "objectives": [always.clone()], "next": "b" },
            "b": { "journal": "b", "objectives": [always.clone()], "next": "c" },
            "c": { "journal": "c", "objectives": [always] } } }),
    );
    let walker = holder(&mut world, json!({ "road": { "stage": "a" } }));
    step(&mut world, &mut schedule);
    assert_eq!(state(&world, Some(walker), "road"), Some(active("b")));
    step(&mut world, &mut schedule);
    assert_eq!(state(&world, Some(walker), "road"), Some(active("c")));
    step(&mut world, &mut schedule);
    assert_eq!(
        state(&world, Some(walker), "road"),
        Some((QuestStatus::Done, "c".into()))
    );
}

#[test]
fn a_self_advancing_reward_completes_the_next_stage_once() {
    let (mut world, _) = rig();
    let advance = json!({ "quest": { "id": "chain", "op": "advance", "target": "game" } });
    add(
        &mut world,
        "chain",
        json!({ "title": "Chain", "start": "a", "stages": {
            "a": { "journal": "a", "next": "b", "rewards": [{ "counter": { "name": "a", "add": 1, "target": "game" } }, advance] },
            "b": { "journal": "b", "next": "c", "rewards": [{ "counter": { "name": "b", "add": 1, "target": "game" } }] },
            "c": { "journal": "c" } } }),
    );
    assert!(act(&mut world, None, "chain", &QuestOp::Start));
    assert!(act(&mut world, None, "chain", &QuestOp::Advance));
    assert_eq!(
        state(&world, None, "chain"),
        Some(active("c")),
        "a's reward completed b"
    );
    assert_eq!(
        (game_count(&world, "a"), game_count(&world, "b")),
        (1, 1),
        "each once"
    );
}

#[test]
fn rewards_that_advance_each_other_stop_at_the_depth_bound() {
    let (mut world, _) = rig();
    // Two long quests whose every stage advances the other.
    for (id, other) in [("ping", "pong"), ("pong", "ping")] {
        let mut stages = serde_json::Map::new();
        for i in 0..20 {
            let mut stage = json!({ "journal": format!("{i}"), "rewards": [
                { "counter": { "name": "completed", "add": 1, "target": "game" } },
                { "quest": { "id": other, "op": "advance", "target": "game" } }
            ] });
            if i < 19 {
                stage["next"] = json!(format!("s{}", i + 1));
            }
            stages.insert(format!("s{i}"), stage);
        }
        add(
            &mut world,
            id,
            json!({ "title": id, "start": "s0", "stages": stages }),
        );
    }
    act(&mut world, None, "ping", &QuestOp::Start);
    act(&mut world, None, "pong", &QuestOp::Start);
    act(&mut world, None, "ping", &QuestOp::Advance);
    assert_eq!(
        game_count(&world, "completed"),
        i64::from(MAX_QUEST_DEPTH) + 1,
        "the first completion and one a depth below the bound"
    );
    assert_eq!(traced(&world, "a quest action 8 lists deep"), 1);
}

#[test]
fn a_reward_cycle_that_fans_out_stops_at_the_call_bound() {
    let (mut world, _) = rig();
    let mut stages = serde_json::Map::new();
    for i in 0..300 {
        let mut stage = json!({ "journal": "on", "rewards": [
            { "counter": { "name": "lists", "add": 1, "target": "game" } },
            { "quest": { "id": "fan", "op": "advance", "target": "game" } },
            { "quest": { "id": "fan", "op": "advance", "target": "game" } }
        ] });
        if i < 299 {
            stage["next"] = json!(format!("s{}", i + 1));
        }
        stages.insert(format!("s{i}"), stage);
    }
    add(
        &mut world,
        "fan",
        json!({ "title": "Fan", "start": "s0", "stages": stages }),
    );
    act(&mut world, None, "fan", &QuestOp::Start);
    act(&mut world, None, "fan", &QuestOp::Advance);
    assert_eq!(
        game_count(&world, "lists"),
        MAX_LISTS as i64,
        "64 lists and no more"
    );
    assert_eq!(traced(&world, "one call ran its 64 lists"), 1);
}

#[test]
fn a_reward_or_objective_naming_other_is_refused() {
    let (mut world, _) = rig();
    let reward = parse(
        json!({ "title": "x", "start": "a", "stages": { "a": { "journal": "a",
        "rewards": [{ "flag": { "name": "thanked", "target": "other" } }] } } }),
    )
    .unwrap();
    let error = check_quest(&reward, &world).unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind),
        (
            "stages.a.rewards.0.flag.target",
            "cue event with an other entity"
        )
    );
    let objective = parse(json!({ "title": "x", "start": "a", "stages": { "a": { "journal": "a",
        "objectives": [{ "text": "x", "when": { "all": [{ "flag": "a" }, { "flag": { "name": "b", "target": "other" } }] } }] } } }))
    .unwrap();
    let error = check_quest(&objective, &world).unwrap_err();
    assert_eq!(error.field, "stages.a.objectives.0.when");
    let ability = parse(
        json!({ "title": "x", "start": "a", "stages": { "a": { "journal": "a",
        "rewards": [{ "ability": "wand" }] } } }),
    )
    .unwrap();
    assert!(
        check_quest(&ability, &world).is_err(),
        "a story list refuses `ability`"
    );
    add(&mut world, "fine", lamplight());
    let fine = world
        .resource::<DataRegistry>()
        .get::<Quest>("fine")
        .unwrap()
        .clone();
    assert!(check_quest(&fine, &world).is_ok());
}

#[test]
fn start_advance_fail_and_stage_act_at_once() {
    let (mut world, _) = rig();
    add(&mut world, "lamplight", lamplight());
    let warden = world.spawn();
    let at = |world: &World| state(world, Some(warden), "lamplight");
    assert!(
        !act(&mut world, Some(warden), "nowhere", &QuestOp::Start),
        "an unknown ID"
    );
    assert!(
        !act(&mut world, Some(warden), "lamplight", &QuestOp::Advance),
        "not taken"
    );
    assert!(act(&mut world, Some(warden), "lamplight", &QuestOp::Start));
    assert_eq!(at(&world), Some(active("light")));
    assert!(
        !act(&mut world, Some(warden), "lamplight", &QuestOp::Start),
        "a taken quest stays"
    );
    assert!(act(
        &mut world,
        Some(warden),
        "lamplight",
        &QuestOp::Stage("report".into())
    ));
    assert_eq!(at(&world), Some(active("report")));
    assert_eq!(
        world.get::<Counters>(warden).map_or(0, |c| c.get("lit")),
        0,
        "no rewards"
    );
    assert!(act(
        &mut world,
        Some(warden),
        "lamplight",
        &QuestOp::Advance
    ));
    assert_eq!(at(&world), Some((QuestStatus::Done, "report".into())));
    assert_eq!(game_count(&world, "coins"), 5, "report's reward");
    assert!(
        !act(&mut world, Some(warden), "lamplight", &QuestOp::Fail),
        "done stays done"
    );

    let keeper = world.spawn();
    act(&mut world, Some(keeper), "lamplight", &QuestOp::Start);
    assert!(act(&mut world, Some(keeper), "lamplight", &QuestOp::Fail));
    assert_eq!(
        state(&world, Some(keeper), "lamplight"),
        Some((QuestStatus::Failed, "light".into()))
    );

    // The cue action, on the event's other entity.
    let cue: CueAction =
        serde_json::from_value(json!({ "quest": { "id": "lamplight", "target": "other" } }))
            .unwrap();
    let rookie = world.spawn();
    run_list(&mut world, &[cue], Some(keeper), Some(rookie), Vec2::ZERO);
    assert_eq!(
        state(&world, Some(rookie), "lamplight"),
        Some(active("light"))
    );
    for bad in [
        json!({ "quest": { "id": "x", "op": "fail", "stage": "a" } }),
        json!({ "quest": { "id": "x", "op": "finish" } }),
    ] {
        assert!(
            serde_json::from_value::<CueAction>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_quest_condition_reads_status_and_stage() {
    let (mut world, _) = rig();
    add(&mut world, "lamplight", lamplight());
    let warden = world.spawn();
    let holds = |world: &World, value: Value| {
        serde_json::from_value::<Condition>(value)
            .unwrap()
            .holds(world, Some(warden), None)
    };
    assert!(holds(
        &world,
        json!({ "quest": { "id": "lamplight", "is": "new" } })
    ));
    assert!(
        !holds(&world, json!({ "quest": { "id": "lamplight" } })),
        "active by default"
    );
    act(&mut world, Some(warden), "lamplight", &QuestOp::Start);
    assert!(holds(
        &world,
        json!({ "quest": { "id": "lamplight", "stage": "light" } })
    ));
    assert!(!holds(
        &world,
        json!({ "quest": { "id": "lamplight", "stage": "report" } })
    ));
    assert!(!holds(
        &world,
        json!({ "quest": { "id": "lamplight", "is": "new" } })
    ));
    act(&mut world, Some(warden), "lamplight", &QuestOp::Fail);
    assert!(holds(
        &world,
        json!({ "quest": { "id": "lamplight", "is": "failed" } })
    ));
    assert!(!holds(
        &world,
        json!({ "quest": { "id": "lamplight", "is": "done" } })
    ));
    act(&mut world, None, "lamplight", &QuestOp::Start);
    assert!(holds(
        &world,
        json!({ "quest": { "id": "lamplight", "target": "game" } })
    ));

    let error = serde_json::from_value::<Condition>(json!({ "quest": { "id": "nowhere" } }))
        .unwrap()
        .check(&world, "when")
        .unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind),
        ("when.quest.id", "quest")
    );
    let error = serde_json::from_value::<Condition>(
        json!({ "quest": { "id": "lamplight", "stage": "gone" } }),
    )
    .unwrap()
    .check(&world, "when")
    .unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind),
        ("when.quest.stage", "quest stage")
    );
    assert!(
        serde_json::from_value::<Condition>(
            json!({ "quest": { "id": "lamplight", "is": "done", "stage": "light" } })
        )
        .is_err(),
        "a stage reads an active quest"
    );
}

#[test]
fn the_parse_refuses_an_unknown_unreachable_or_cyclic_stage() {
    let stages = |stages: Value| json!({ "title": "x", "start": "a", "stages": stages });
    for (value, message) in [
        (
            json!({ "title": "x", "start": "zz", "stages": { "a": { "journal": "a" } } }),
            "start: unknown stage `zz`",
        ),
        (
            stages(json!({ "a": { "journal": "a", "next": "b" } })),
            "stages.a.next: unknown stage `b`",
        ),
        (
            stages(json!({ "a": { "journal": "a" }, "lost": { "journal": "?" } })),
            "stages.lost: unreachable",
        ),
        (
            stages(
                json!({ "a": { "journal": "a", "next": "b" }, "b": { "journal": "b", "next": "a" } }),
            ),
            "a cycle of stages",
        ),
        (stages(json!({})), "at least one stage"),
        (
            stages(json!({ "a": { "journal": "a", "reward": [] } })),
            "unknown field `reward`",
        ),
    ] {
        let error = parse(value).unwrap_err();
        assert!(error.contains(message), "{message}: {error}");
    }
    assert!(parse(lamplight()).is_ok());
}

#[test]
fn both_forms_save_round_trip() {
    // The holder is a spawn from a named source, as a run's spawns save.
    let rig_with_prefab = || {
        let (mut world, schedule) = rig();
        add(&mut world, "lamplight", lamplight());
        let mut prefabs = PrefabRegistry::new();
        prefabs.insert(
            "holder".to_owned(),
            serde_json::from_value(
                json!({ "components": { "quests": { "lamplight": { "stage": "report" } } } }),
            )
            .unwrap(),
        );
        world.insert_resource(prefabs);
        let source = world.spawn_with((tungsten_core::Transform::default(),));
        saves::name(&mut world, source, None, "source").unwrap();
        (world, schedule, source)
    };
    let (mut world, _, source) = rig_with_prefab();
    let prepared = world.prepare_prefab("holder", [8.0, 8.0]).unwrap();
    world
        .resource_mut::<CommandBuffer>()
        .spawn_prefab_from(source, prepared);
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    act(&mut world, None, "lamplight", &QuestOp::Start);
    act(&mut world, None, "lamplight", &QuestOp::Fail);
    let saved = capture_world(&world).unwrap();

    let (mut fresh, ..) = rig_with_prefab();
    restore_world(&mut fresh, &saved, &(Restored,)).unwrap();
    let restored: Vec<Entity> = fresh
        .query::<(Entity, &Quests)>()
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(restored.len(), 1, "the holder spawned again");
    assert_eq!(
        state(&fresh, Some(restored[0]), "lamplight"),
        Some(active("report"))
    );
    assert_eq!(
        state(&fresh, None, "lamplight"),
        Some((QuestStatus::Failed, "light".into())),
        "the game's"
    );
}

#[test]
fn a_vanished_stage_restores_at_the_start() {
    let (mut world, _) = rig();
    add(&mut world, "lamplight", lamplight());
    act(
        &mut world,
        None,
        "lamplight",
        &QuestOp::Stage("report".into()),
    );
    let saved = capture_world(&world).unwrap();
    // The data changes: `report` is gone.
    add(
        &mut world,
        "lamplight",
        json!({ "title": "Lamplight", "start": "light", "stages": { "light": { "journal": "Light it." } } }),
    );
    restore_world(&mut world, &saved, &(Restored,)).unwrap();
    assert_eq!(state(&world, None, "lamplight"), Some(active("light")));
}

#[test]
fn a_world_without_quests_queries_nothing() {
    let (mut world, mut schedule) = rig();
    add(&mut world, "lamplight", lamplight());
    world.resource_mut::<Flags>().set("lamp.lit", true);
    for _ in 0..3 {
        step(&mut world, &mut schedule);
    }
    assert!(!world.has_any::<Quests>());
    assert!(!world.has_resource::<Quests>(), "nothing took one");
    // A done game quest leaves the gate shut too.
    act(
        &mut world,
        None,
        "lamplight",
        &QuestOp::Stage("report".into()),
    );
    act(&mut world, None, "lamplight", &QuestOp::Advance);
    assert!(!world.resource::<Quests>().any_active());
    step(&mut world, &mut schedule);
    // A holder arriving is served from its next step.
    let warden = holder(&mut world, json!({ "lamplight": { "stage": "light" } }));
    step(&mut world, &mut schedule);
    assert_eq!(
        state(&world, Some(warden), "lamplight"),
        Some(active("report"))
    );
}
