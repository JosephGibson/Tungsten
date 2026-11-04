use super::*;
use glam::Vec2;
use serde_json::{Value, json};
use tungsten_core::physics::{Collider, Position};
use tungsten_core::save::{self, capture_world, restore_world};
use tungsten_core::{CommandBuffer, DataRef, DataRegistry, PrefabData, Stage, Tag, Transform};

use crate::foundation::tags::TagsPlugin;
use crate::foundation::trace::{KitTrace, TraceKind};
use crate::intent::ability::{Abilities, AbilitiesPlugin, Ability, Payload, Trigger};
use crate::intent::action_input::Intent;
use crate::lifecycle::spawner::{Spawner, SpawnerPlugin};
use crate::react::counters::{Counters, CountersPlugin};
use crate::react::cues::{ABILITY_USED, CueAction, Cues, CuesPlugin};
use crate::react::world_cues::{WorldCue, WorldCueActions};

/// A frame's time, exact in binary so timers count whole steps.
const STEP: f32 = 1.0 / 64.0;

fn prefab(world: &mut World, id: &str, components: Value) {
    let mut value = json!({ "tag": id });
    value["components"] = components;
    let data: PrefabData =
        serde_json::from_value(value).unwrap_or_else(|err| panic!("{id}: {err}"));
    world
        .resource_mut::<PrefabRegistry>()
        .insert(id.to_string(), data);
}

/// A World with a clock, a command buffer, a trace, the tags, counter,
/// cue, ability, spawner and cap plugins, and the prefabs `salt` (group
/// `salt`, two, the oldest ending past them, its `culled` counted for the
/// game), `mine` (group `mines`, two, refused past them) and `plain`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.insert_resource(PrefabRegistry::new());
    prefab(
        &mut world,
        "salt",
        json!({
            "cap": { "group": "salt", "max": 2 },
            "cues": { "culled": [{ "counter": { "name": "culled", "add": 1, "target": "game" } }] }
        }),
    );
    prefab(
        &mut world,
        "mine",
        json!({ "cap": { "group": "mines", "max": 2, "past": "refuse" } }),
    );
    prefab(&mut world, "plain", json!({}));
    let mut schedule = Schedule::new();
    TagsPlugin::new().build(&mut schedule, &mut world);
    CountersPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    AbilitiesPlugin.build(&mut schedule, &mut world);
    SpawnerPlugin.build(&mut schedule, &mut world);
    CapPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A kit spawn of `id` from `from`, as a capped route makes one: prepared,
/// admitted, then queued for the flush; whether it was admitted.
fn kit_spawn(world: &mut World, id: &str, from: Entity) -> bool {
    let prepared = world.prepare_prefab(id, [0.0, 0.0]).unwrap();
    let Ok(prepared) = admit(world, id, Some(from), prepared) else {
        return false;
    };
    world
        .resource_mut::<CommandBuffer>()
        .spawn_prefab_from(from, prepared);
    true
}

/// The frame's fixed step, without its flush.
fn fixed(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

/// Ends the frame: its command and event flushes.
fn flush(world: &mut World) {
    world.resource_mut::<Time>().leave_fixed_steps();
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// Starts a frame.
fn frame(world: &mut World) {
    world.resource_mut::<Time>().advance_frame(STEP);
}

/// A frame of one fixed step.
fn step(world: &mut World, schedule: &mut Schedule) {
    frame(world);
    fixed(world, schedule);
    flush(world);
}

/// The live entities tagged `name`, by index.
fn tagged(world: &World, name: &str) -> Vec<Entity> {
    let mut found: Vec<Entity> = world
        .query::<(Entity, &Tag)>()
        .filter(|(_, tag)| tag.name == name)
        .map(|(entity, _)| entity)
        .collect();
    found.sort_by_key(|entity| entity.id());
    found
}

/// The despawns a cap's ending traced.
fn culled(world: &World) -> Vec<Entity> {
    world
        .resource::<KitTrace>()
        .iter()
        .filter(|entry| matches!(entry.kind, TraceKind::Despawn { by } if by == CAP))
        .map(|entry| entry.entity)
        .collect()
}

/// How many skips about `entity` were traced for `why`.
fn skipped(world: &World, entity: Entity, why: &str) -> usize {
    world
        .resource::<KitTrace>()
        .about(entity)
        .filter(|entry| matches!(entry.kind, TraceKind::Skipped { why: w, .. } if w == why))
        .count()
}

/// The game counter `name`.
fn game(world: &World, name: &str) -> i64 {
    world.resource::<Counters>().get(name)
}

/// Inserts the world cue `name` with `actions`.
fn world_cue(world: &mut World, name: &str, actions: Value) {
    let actions: WorldCueActions = serde_json::from_value(actions).unwrap();
    world.resource_mut::<DataRegistry>().insert(name, actions);
}

#[test]
fn past_its_cap_the_oldest_is_culled_or_the_spawn_refused() {
    let (mut world, mut schedule) = rig();
    let caster = world.spawn_with((Transform::default(),));
    for _ in 0..2 {
        assert!(kit_spawn(&mut world, "salt", caster));
        step(&mut world, &mut schedule);
    }
    let first = tagged(&world, "salt")[0];
    assert!(world.get::<Source>(first).is_some(), "a kit spawn's source");
    assert!(world.get::<Admitted>(first).is_some(), "and its stamp");
    assert!(kit_spawn(&mut world, "salt", caster), "the third admitted");
    step(&mut world, &mut schedule);
    let salts = tagged(&world, "salt");
    assert_eq!(salts.len(), 2);
    assert!(!salts.contains(&first), "the oldest ended");
    assert_eq!(culled(&world), [first]);
    assert_eq!(
        game(&world, "culled"),
        1,
        "its `culled` cue ran before the despawn"
    );
    // The fourth takes the first's freed index, so the lowest index is the
    // youngest: each later admission still ends the oldest by admission.
    let (second, third) = (salts[0], salts[1]);
    assert!(kit_spawn(&mut world, "salt", caster));
    step(&mut world, &mut schedule);
    let fourth = tagged(&world, "salt")
        .into_iter()
        .find(|&salt| salt != third)
        .unwrap();
    assert_eq!(fourth.id(), first.id(), "the first's index reused");
    assert_eq!(culled(&world), [first, second]);
    assert!(kit_spawn(&mut world, "salt", caster));
    step(&mut world, &mut schedule);
    assert_eq!(culled(&world), [first, second, third], "by admission");
    assert!(world.is_alive(fourth));
    assert_eq!(game(&world, "culled"), 3);
    for _ in 0..3 {
        kit_spawn(&mut world, "mine", caster);
        step(&mut world, &mut schedule);
    }
    assert_eq!(tagged(&world, "mine").len(), 2, "the third refused");
    let prepared = world.prepare_prefab("mine", [0.0, 0.0]).unwrap();
    assert_eq!(
        admit(&mut world, "mine", Some(caster), prepared).err(),
        Some(FULL)
    );
    assert_eq!(culled(&world).len(), 3, "a refusal ends nothing");
}

#[test]
fn a_map_placed_instance_does_not_count() {
    let (mut world, mut schedule) = rig();
    let caster = world.spawn_with((Transform::default(),));
    let placed = world.spawn_prefab("salt", [0.0, 0.0]).unwrap();
    assert!(
        world.get::<Source>(placed).is_none() && world.get::<Admitted>(placed).is_none(),
        "spawned with no source and no stamp"
    );
    for _ in 0..2 {
        assert!(kit_spawn(&mut world, "salt", caster));
        step(&mut world, &mut schedule);
    }
    assert_eq!(tagged(&world, "salt").len(), 3, "the placed one aside");
    let oldest = tagged(&world, "salt")
        .into_iter()
        .find(|&salt| salt != placed)
        .unwrap();
    assert!(kit_spawn(&mut world, "salt", caster));
    step(&mut world, &mut schedule);
    assert!(world.is_alive(placed), "never a victim");
    assert_eq!(culled(&world), [oldest]);
}

#[test]
fn spawns_queued_before_one_flush_keep_the_cap() {
    for two_steps in [false, true] {
        let (mut world, mut schedule) = rig();
        let caster = world.spawn_with((Transform::default(),));
        for id in ["salt", "mine"] {
            assert!(kit_spawn(&mut world, id, caster));
        }
        step(&mut world, &mut schedule);
        let live = tagged(&world, "salt")[0];
        // Three requests of each before the frame's one flush, in one step
        // or with the frame's second step between them.
        frame(&mut world);
        let mut admitted = Vec::new();
        for i in 0..3 {
            if two_steps && i == 2 {
                fixed(&mut world, &mut schedule);
            }
            admitted.push((
                kit_spawn(&mut world, "salt", caster),
                kit_spawn(&mut world, "mine", caster),
            ));
        }
        fixed(&mut world, &mut schedule);
        flush(&mut world);
        assert_eq!(
            admitted,
            [(true, true), (true, false), (false, false)],
            "two steps {two_steps}: `oldest` admits two, ending the live one for the second, \
             and refuses a third with only queued spawns left; `refuse` admits one"
        );
        assert_eq!(
            culled(&world),
            [live],
            "two steps {two_steps}: one victim, once"
        );
        assert!(!world.is_alive(live));
        assert_eq!(tagged(&world, "salt").len(), 2);
        assert_eq!(tagged(&world, "mine").len(), 2);
    }
}

#[test]
fn a_spawn_that_fails_to_prepare_culls_nothing() {
    let (mut world, mut schedule) = rig();
    // In the salt group, with a component no plugin registers.
    prefab(
        &mut world,
        "broken",
        json!({ "cap": { "group": "salt", "max": 2 }, "nonesuch": {} }),
    );
    world_cue(&mut world, "crack", json!([{ "prefab": "broken" }]));
    let caster = world.spawn_with((Transform::default(),));
    for _ in 0..2 {
        assert!(kit_spawn(&mut world, "salt", caster));
        step(&mut world, &mut schedule);
    }
    let salts = tagged(&world, "salt");
    // A spawner and a world cue spawn the broken prefab in one frame.
    world.spawn_with((Transform::default(), Spawner::new("broken", STEP)));
    WorldCue::new("crack", Vec2::ZERO).play(&mut world);
    frame(&mut world);
    fixed(&mut world, &mut schedule);
    let failed = world
        .resource::<KitTrace>()
        .iter()
        .filter(|entry| matches!(&entry.kind, TraceKind::SpawnFailed { prefab, .. } if prefab == "broken"))
        .count();
    assert_eq!(failed, 2, "both routes tried");
    assert!(culled(&world).is_empty(), "nothing culled");
    // Nothing counted: a salt admitted this frame ends the oldest alone.
    assert!(kit_spawn(&mut world, "salt", caster));
    fixed(&mut world, &mut schedule);
    flush(&mut world);
    assert_eq!(culled(&world), [salts[0]]);
    assert_eq!(tagged(&world, "salt").len(), 2);
}

#[test]
fn a_spawn_whose_source_goes_at_the_flush_culls_nothing() {
    let (mut world, mut schedule) = rig();
    let caster = world.spawn_with((Transform::default(),));
    for _ in 0..2 {
        assert!(kit_spawn(&mut world, "salt", caster));
        step(&mut world, &mut schedule);
    }
    // A spawner ended in the frame's first fixed step falls due in its
    // second, its despawn queued ahead of the spawn.
    let nest = world.spawn_with((Transform::default(), Spawner::new("salt", 2.0 * STEP)));
    frame(&mut world);
    end(&mut world, nest, "test");
    fixed(&mut world, &mut schedule);
    assert!(world.has::<Despawning>(nest));
    fixed(&mut world, &mut schedule);
    flush(&mut world);
    assert!(!world.is_alive(nest));
    assert_eq!(skipped(&world, nest, LEAVING), 1, "refused at its cap");
    assert!(culled(&world).is_empty());
    assert_eq!(tagged(&world, "salt").len(), 2);

    // A receiver whose replacement is queued: a spawn from it, and a second
    // replacement of it, before one flush.
    let receiver = world.spawn_with((Transform::default(),));
    frame(&mut world);
    crate::chemistry::becoming::replace(&mut world, receiver, "plain", None).unwrap();
    assert!(
        !kit_spawn(&mut world, "salt", receiver),
        "its source leaves"
    );
    assert_eq!(
        crate::chemistry::becoming::replace(&mut world, receiver, "salt", None),
        Err(LEAVING)
    );
    fixed(&mut world, &mut schedule);
    flush(&mut world);
    assert!(culled(&world).is_empty());
    assert_eq!(tagged(&world, "salt").len(), 2);
    assert_eq!(tagged(&world, "plain").len(), 1, "the first replacement");
}

#[test]
fn a_sourceless_kit_spawn_counts_toward_its_cap() {
    let (mut world, mut schedule) = rig();
    world_cue(&mut world, "storm", json!([{ "prefab": "salt" }]));
    for _ in 0..2 {
        WorldCue::new("storm", Vec2::ZERO).play(&mut world);
        step(&mut world, &mut schedule);
    }
    let salts = tagged(&world, "salt");
    assert_eq!(salts.len(), 2);
    assert!(
        salts
            .iter()
            .all(|&salt| world.get::<Source>(salt).is_none()
                && world.get::<Admitted>(salt).is_some()),
        "no source, but stamped"
    );
    WorldCue::new("storm", Vec2::ZERO).play(&mut world);
    step(&mut world, &mut schedule);
    assert_eq!(culled(&world), [salts[0]]);
    assert_eq!(tagged(&world, "salt").len(), 2);
    assert_eq!(game(&world, "culled"), 1, "its `culled` ran that step");
}

/// A restore's marker.
#[derive(Debug, Clone, Copy)]
struct Probe;

/// A World with the cap and spawner plugins alone, a clock, a command
/// buffer and `salt` capped at three.
fn save_rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.insert_resource(PrefabRegistry::new());
    prefab(
        &mut world,
        "salt",
        json!({ "cap": { "group": "salt", "max": 3 } }),
    );
    let mut schedule = Schedule::new();
    CapPlugin.build(&mut schedule, &mut world);
    SpawnerPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A nest spawning three salts, named in the save.
fn nest(world: &mut World) -> Entity {
    let nest = world.spawn_with((
        Position(Vec2::ZERO),
        Spawner::new("salt", STEP).with_count(3),
    ));
    save::name(world, nest, None, "nest").unwrap();
    nest
}

#[test]
fn a_restored_group_counts_oldest_first() {
    let (mut a, mut schedule_a) = save_rig();
    nest(&mut a);
    for _ in 0..3 {
        step(&mut a, &mut schedule_a);
    }
    assert_eq!(tagged(&a, "salt").len(), 3);
    let saved = capture_world(&a).unwrap();

    let (mut b, mut schedule_b) = save_rig();
    // Churn, so the restored salts take other indices.
    let spare: Vec<Entity> = (0..20).map(|_| b.spawn()).collect();
    for (index, entity) in spare.into_iter().enumerate() {
        if index % 3 != 1 {
            b.despawn(entity);
        }
    }
    let nest_b = nest(&mut b);
    let report = restore_world(&mut b, &saved, &(Probe,)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let mut restored = tagged(&b, "salt");
    assert_eq!(restored.len(), 3);
    assert!(
        restored
            .iter()
            .all(|&salt| b.get::<Source>(salt).is_some() && b.get::<Admitted>(salt).is_none()),
        "restored with their source, unstamped"
    );
    restored.sort_by_key(|&salt| save::order_key(&b, salt));
    assert_ne!(
        restored[0],
        *restored.iter().min_by_key(|salt| salt.id()).unwrap(),
        "the churn put the lowest save ID off the lowest index"
    );
    // A new admission ends the lowest save ID.
    frame(&mut b);
    assert!(kit_spawn(&mut b, "salt", nest_b));
    fixed(&mut b, &mut schedule_b);
    flush(&mut b);
    assert_eq!(culled(&b), [restored[0]]);
    // The next ends the next restored one before the new admission.
    frame(&mut b);
    assert!(kit_spawn(&mut b, "salt", nest_b));
    fixed(&mut b, &mut schedule_b);
    flush(&mut b);
    assert_eq!(culled(&b), [restored[0], restored[1]]);
}

#[test]
fn a_refused_spawner_spawn_keeps_its_count() {
    let (mut world, mut schedule) = rig();
    let nest = world.spawn_with((
        Transform::default(),
        Spawner::new("mine", STEP).with_count(5),
    ));
    let remaining = |world: &World| world.get::<Spawner>(nest).unwrap().remaining();
    step(&mut world, &mut schedule);
    step(&mut world, &mut schedule);
    assert_eq!(
        (tagged(&world, "mine").len(), remaining(&world)),
        (2, Some(3))
    );
    step(&mut world, &mut schedule);
    assert_eq!(
        (tagged(&world, "mine").len(), remaining(&world)),
        (2, Some(3)),
        "refused while full, given back"
    );
    assert_eq!(skipped(&world, nest, FULL), 1);
    let mines = tagged(&world, "mine");
    world.despawn(mines[0]);
    step(&mut world, &mut schedule);
    assert_eq!(
        (tagged(&world, "mine").len(), remaining(&world)),
        (2, Some(2))
    );
    let mut spawned = 3;
    for _ in 0..8 {
        for mine in tagged(&world, "mine") {
            world.despawn(mine);
        }
        let before = remaining(&world).unwrap();
        step(&mut world, &mut schedule);
        spawned += before - remaining(&world).unwrap();
    }
    assert_eq!((spawned, remaining(&world)), (5, Some(0)), "five in all");
}

#[test]
fn a_become_takes_its_receivers_place_in_the_group() {
    let (mut world, mut schedule) = rig();
    prefab(
        &mut world,
        "brine",
        json!({ "cap": { "group": "salt", "max": 2 } }),
    );
    let caster = world.spawn_with((Transform::default(),));
    for _ in 0..2 {
        assert!(kit_spawn(&mut world, "salt", caster));
        step(&mut world, &mut schedule);
    }
    let [older, receiver] = tagged(&world, "salt")[..] else {
        panic!("two salts");
    };
    // The receiver becomes brine in the frame a salt is admitted.
    frame(&mut world);
    crate::chemistry::becoming::replace(&mut world, receiver, "brine", None).unwrap();
    assert!(kit_spawn(&mut world, "salt", caster));
    fixed(&mut world, &mut schedule);
    flush(&mut world);
    assert_eq!(culled(&world), [older], "the receiver no victim");
    assert!(!world.is_alive(receiver));
    assert_eq!(
        (tagged(&world, "salt").len(), tagged(&world, "brine").len()),
        (1, 1),
        "the group at its max"
    );

    // A replacement its `refuse` cap refuses leaves the receiver as it is.
    for _ in 0..2 {
        assert!(kit_spawn(&mut world, "mine", caster));
        step(&mut world, &mut schedule);
    }
    let plain = world.spawn_prefab("plain", [0.0, 0.0]).unwrap();
    frame(&mut world);
    assert_eq!(
        crate::chemistry::becoming::replace(&mut world, plain, "mine", None),
        Err(FULL)
    );
    fixed(&mut world, &mut schedule);
    flush(&mut world);
    assert!(world.is_alive(plain));
    assert_eq!(tagged(&world, "mine").len(), 2);
    assert_eq!(culled(&world), [older]);
}

#[test]
fn the_oldest_end_until_the_group_has_room() {
    let (mut world, mut schedule) = rig();
    let caster = world.spawn_with((Transform::default(),));
    // A game's own spawns from a source, which no cap admitted.
    for _ in 0..4 {
        let prepared = world.prepare_prefab("salt", [0.0, 0.0]).unwrap();
        world
            .resource_mut::<CommandBuffer>()
            .spawn_prefab_from(caster, prepared);
    }
    step(&mut world, &mut schedule);
    let own = tagged(&world, "salt");
    assert_eq!(own.len(), 4);
    assert!(kit_spawn(&mut world, "salt", caster));
    step(&mut world, &mut schedule);
    assert_eq!(culled(&world), own[..3], "three end for the one admitted");
    assert_eq!(game(&world, "culled"), 3, "a `culled` for each");
    assert_eq!(tagged(&world, "salt").len(), 2);
}

#[test]
fn a_culled_cue_runs_before_its_victim_goes() {
    let (mut world, mut schedule) = rig();
    // A wisp holds one; its `culled` counts, spawns a poof at the event's
    // point and uses its `dud`, refused for its cost.
    prefab(
        &mut world,
        "wisp",
        json!({
            "cap": { "group": "wisps", "max": 1 },
            "cues": {
                "culled": [
                    { "counter": { "name": "culled", "add": 1, "target": "game" } },
                    { "prefab": "poof" },
                    { "ability": "dud" }
                ],
                "ability_refused": [{ "counter": { "name": "refused", "add": 1, "target": "game" } }]
            }
        }),
    );
    prefab(&mut world, "poof", json!({}));
    world_cue(&mut world, "storm", json!([{ "prefab": "wisp" }]));
    let data = world.resource_mut::<DataRegistry>();
    data.insert(
        "cast",
        Ability::new()
            .with_trigger(Trigger::Action("cast".to_string()))
            .with_payload(Payload::Spawn {
                prefab: "wisp".to_string(),
                speed: 0.0,
                offset: 0.0,
            }),
    );
    data.insert(
        "ring",
        Ability::new().with_trigger(Trigger::Action("ring".to_string())),
    );
    data.insert("dud", Ability::new().with_cost("mana", 1.0));
    let caster = world.spawn_with((
        Transform::default(),
        Abilities::new().with(DataRef::by_id("cast")),
        Intent::default(),
    ));
    // A bell whose use spawns a wisp through its own cue.
    let bell = world.spawn_with((
        Transform::default(),
        Abilities::new().with(DataRef::by_id("ring")),
        Intent::default(),
        Cues::new().on(ABILITY_USED, [CueAction::prefab("wisp")]),
    ));
    // A step, `slot` pressed on `user` alone.
    let step_with =
        |world: &mut World, schedule: &mut Schedule, pressed: Option<(Entity, &str)>| {
            for user in [caster, bell] {
                let mut intent = Intent::default();
                if let Some((by, slot)) = pressed
                    && by == user
                {
                    intent.set_slot(slot, true, true);
                }
                world.insert(user, intent);
            }
            step(world, schedule);
        };
    let poofs = |world: &World| -> Vec<Vec2> {
        tagged(world, "poof")
            .into_iter()
            .map(|poof| world.get::<Transform>(poof).unwrap().position)
            .collect()
    };
    step_with(&mut world, &mut schedule, Some((caster, "cast")));
    let mut expected = Vec::new();
    for (round, route) in ["a spawn payload", "a prefab cue action", "a world cue"]
        .into_iter()
        .enumerate()
    {
        // The survivor's centre: a box below its point.
        let [victim] = tagged(&world, "wisp")[..] else {
            panic!("{route}: one wisp");
        };
        let at = Vec2::new(10.0 * (round as f32 + 1.0), 20.0);
        world.insert(victim, Position(at));
        world.insert(
            victim,
            Collider::aabb(Vec2::new(4.0, 4.0)).with_offset(Vec2::new(0.0, -6.0)),
        );
        let pressed = match round {
            0 => {
                world.insert(victim, Abilities::new().with(DataRef::by_id("dud")));
                Some((caster, "cast"))
            }
            1 => Some((bell, "ring")),
            _ => {
                WorldCue::new("storm", Vec2::ZERO).play(&mut world);
                None
            }
        };
        step_with(&mut world, &mut schedule, pressed);
        assert!(!world.is_alive(victim), "{route}: culled");
        assert_eq!(tagged(&world, "wisp").len(), 1, "{route}: its successor");
        assert_eq!(
            game(&world, "culled"),
            round as i64 + 1,
            "{route}: once, that step"
        );
        expected.push(at + Vec2::new(0.0, -6.0));
        assert_eq!(poofs(&world), expected, "{route}: at its centre");
        assert_eq!(
            game(&world, "refused"),
            1,
            "{route}: the first victim's `ability` refusal cued that step"
        );
    }
}
