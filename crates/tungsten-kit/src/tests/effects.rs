use super::*;
use serde_json::json;
use tungsten_core::physics::Position;
use tungsten_core::{
    CommandBuffer, ComponentRegistry, PrefabData, PrefabRegistry, Schedule, Stage, Transform,
};

use crate::damage::health::{Damaged, HealthPlugin};
use crate::foundation::stats::StatsPlugin;
use crate::foundation::tags::TagsPlugin;
use crate::foundation::trace::KitTrace;
use crate::motion::top_down::{TopDownMover, TopDownMoverPlugin};
use crate::react::counters::{Counters, CountersPlugin};
use crate::react::cues::{CueAction, Cues, CuesPlugin};
use crate::{KIT_INTENT, KIT_REACT, KIT_RESOLVE, KIT_TICK};

const STEP: f32 = 1.0 / 60.0;

/// The status tags the tests declare.
const STATUS: [&str; 6] = [
    "status.burning",
    "status.frozen",
    "status.thawed",
    "status.marked",
    "status.other",
    "status.warm",
];

/// A World with time, a command buffer, a trace and `build`'s plugins.
fn rig_with(build: impl FnOnce(&mut Schedule, &mut World)) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// Tags, stats, health, counters, cues and effects.
fn rig() -> (World, Schedule) {
    rig_with(|schedule, world| {
        TagsPlugin::new().with_tags(STATUS).build(schedule, world);
        StatsPlugin::new()
            .with_stat("armor", f32::NEG_INFINITY, f32::INFINITY)
            .build(schedule, world);
        HealthPlugin.build(schedule, world);
        CountersPlugin.build(schedule, world);
        CuesPlugin.build(schedule, world);
        EffectsPlugin.build(schedule, world);
    })
}

/// One fixed step at the frame's `dt` (a world driven by hand reads the
/// game dt), then the command flush that ends the frame.
fn step(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        step(world, schedule, STEP);
    }
}

/// Inserts `effect` as the `effects` entry `id`.
fn insert(world: &mut World, id: &str, effect: Effect) -> DataRef<Effect> {
    world.resource_mut::<DataRegistry>().insert(id, effect);
    DataRef::by_id(id)
}

fn stat(world: &World, entity: Entity, name: &str) -> Option<f32> {
    world.get::<Stats>(entity).and_then(|stats| stats.get(name))
}

fn has_tag(world: &World, entity: Entity, tag: &str) -> bool {
    world.get::<Tags>(entity).is_some_and(|tags| tags.has(tag))
}

fn instances(world: &World, entity: Entity) -> Vec<(Option<String>, u32, Option<f32>)> {
    world.get::<Effects>(entity).map_or(Vec::new(), |effects| {
        effects
            .instances()
            .map(|i| (i.effect().id().map(str::to_owned), i.stacks(), i.left()))
            .collect()
    })
}

fn applied(world: &World) -> Vec<(Entity, u32, Option<String>)> {
    world
        .resource::<EventQueue<EffectApplied>>()
        .iter()
        .map(|event| (event.entity, event.stacks, event.id.clone()))
        .collect()
}

/// The stacks of the last `EffectApplied` sent.
fn last_applied(world: &World) -> Option<u32> {
    world
        .resource::<EventQueue<EffectApplied>>()
        .iter()
        .last()
        .map(|event| event.stacks)
}

fn expired(world: &World) -> Vec<(Entity, Option<String>, bool)> {
    world
        .resource::<EventQueue<EffectExpired>>()
        .iter()
        .map(|event| (event.entity, event.id.clone(), event.ended))
        .collect()
}

fn skipped(world: &World, entity: Entity) -> Vec<(&'static str, &'static str)> {
    world
        .resource::<KitTrace>()
        .about(entity)
        .filter_map(|entry| match entry.kind {
            TraceKind::Skipped { action, why, .. } => Some((action, why)),
            _ => None,
        })
        .collect()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn the_plugin_registers_its_kinds_queues_and_systems() {
    let (world, schedule) = rig_with(|schedule, world| EffectsPlugin.build(schedule, world));
    assert!(world.has_event::<EffectApplied>() && world.has_event::<EffectExpired>());
    assert!(world.has_resource::<EffectRequests>());
    let data = world.resource::<DataRegistry>();
    assert_eq!(data.kind_of::<Effect>(), Some(EFFECTS));
    assert_eq!(data.kind_of::<Curve>(), Some(CURVES));
    assert!(
        world
            .resource::<ComponentRegistry>()
            .names()
            .any(|n| n == EFFECTS)
    );
    assert_eq!(schedule.stage_of(EFFECT_TICK), Some(Stage::FixedUpdate));
    assert_eq!(schedule.stage_of(EFFECT_RESOLVE), Some(Stage::FixedUpdate));
    let names: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
    assert!(at(KIT_TICK) < at(EFFECT_TICK) && at(EFFECT_TICK) < at(KIT_INTENT));
    assert!(at(KIT_RESOLVE) < at(EFFECT_RESOLVE) && at(EFFECT_RESOLVE) < at(KIT_REACT));
}

#[test]
fn a_scale_curve_with_a_y_below_0_is_refused_at_load() {
    let (mut world, _) = rig();
    let bad = Effect::new().with_scale(
        DataRef::inline(Curve::new([(0.0, 1.0), (1.0, -0.5)])),
        Scaled::ALL,
    );
    let error = bad.check(&world).unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind, error.id.as_str()),
        ("scale.curve", "curve with every y at least 0", "[1, -0.5]")
    );
    world
        .resource_mut::<DataRegistry>()
        .insert("dip", Curve::new([(0.0, -1.0)]));
    let by_id = Effect::new().with_scale(DataRef::by_id("dip"), Scaled::ALL);
    assert_eq!(by_id.check(&world).unwrap_err().field, "scale.curve");
    let missing = Effect::new().with_scale(DataRef::by_id("nowhere"), Scaled::ALL);
    let error = missing.check(&world).unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind),
        ("scale.curve", "curves")
    );
    world
        .resource_mut::<DataRegistry>()
        .insert("rise", Curve::new([(0.0, 0.0), (1.0, 2.0)]));
    assert!(
        Effect::new()
            .with_scale(DataRef::by_id("rise"), Scaled::ALL)
            .check(&world)
            .is_ok()
    );
    // The scene component's check names the entry's index.
    let components = world.resource::<ComponentRegistry>();
    let error = components
        .check_in(
            EFFECTS,
            &json!([
                "rise_effect",
                { "scale": { "curve": [[0, -1]] } }
            ]),
            &world,
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("field `0`: unknown effects `rise_effect`"),
        "{error}"
    );
    let error = components
        .check_in(
            EFFECTS,
            &json!([{ "scale": { "curve": [[0, -1]] } }]),
            &world,
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("field `0.scale.curve`: unknown curve with every y at least 0"),
        "{error}"
    );
    let error = components
        .check_in(EFFECTS, &json!([{ "grant": ["status.nowhere"] }]), &world)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("field `0.grant.0`: unknown tag `status.nowhere`"),
        "{error}"
    );
    let error = components
        .check_in(EFFECTS, &json!([{ "on_expire": "gone" }]), &world)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("field `0.on_expire`: unknown effects `gone`"),
        "{error}"
    );
    assert!(
        components
            .check_in(
                EFFECTS,
                &json!([{ "duration": 1, "grant": ["status.marked"] }]),
                &world
            )
            .is_ok()
    );
}

#[test]
fn a_stack_at_its_limit_keeps_its_count_and_its_time_follows_refresh() {
    for (refresh, left_after) in [
        (Refresh::Refresh, 1.0),
        (Refresh::Keep, 1.0 - 2.0 * STEP),
        (Refresh::Extend, 3.0 - 2.0 * STEP),
    ] {
        let (mut world, mut schedule) = rig();
        let effect = insert(
            &mut world,
            "stacking",
            Effect::new()
                .with_duration(1.0)
                .with_stack(2, refresh, Expire::Clear)
                .with_modifier("armor", Modifier::Add(1.0)),
        );
        let knight = world.spawn_with((Stats::new().with("armor", 0.0),));
        let request = EffectRequest::new(knight, effect);
        let mut counts = Vec::new();
        assert_eq!(request.clone().apply(&mut world), Applied::Applied);
        counts.push(last_applied(&world));
        steps(&mut world, &mut schedule, 1);
        assert_eq!(request.clone().apply(&mut world), Applied::Stacked);
        counts.push(last_applied(&world));
        steps(&mut world, &mut schedule, 1);
        assert_eq!(request.apply(&mut world), Applied::Stacked, "at the limit");
        counts.push(last_applied(&world));
        let [(id, stacks, left)] = &instances(&world, knight)[..] else {
            panic!("one instance");
        };
        assert_eq!((id.as_deref(), *stacks), (Some("stacking"), 2));
        assert!(close(left.unwrap(), left_after), "{refresh:?}: {left:?}");
        assert_eq!(stat(&world, knight, "armor"), Some(2.0), "per stack");
        assert_eq!(counts, [Some(1), Some(2), Some(2)]);
    }
}

#[test]
fn remove_one_drops_a_stack_and_restarts_and_clear_ends_every_stack() {
    let (mut world, mut schedule) = rig();
    let effect = insert(
        &mut world,
        "swift",
        Effect::new()
            .with_duration(2.0 * STEP)
            .with_stack(3, Refresh::Refresh, Expire::RemoveOne)
            .with_modifier(Stats::MOVE_SPEED, Modifier::Pct(0.1)),
    );
    let hero = world.spawn_with((Stats::new().with(Stats::MOVE_SPEED, 100.0),));
    for _ in 0..3 {
        EffectRequest::new(hero, effect.clone()).apply(&mut world);
    }
    assert!(close(stat(&world, hero, Stats::MOVE_SPEED).unwrap(), 130.0));
    let mut seen = Vec::new();
    let mut ended = 0;
    for _ in 0..7 {
        steps(&mut world, &mut schedule, 1);
        seen.push((
            instances(&world, hero).first().map(|(_, n, _)| *n),
            stat(&world, hero, Stats::MOVE_SPEED).unwrap().round(),
        ));
        ended += expired(&world).len();
    }
    assert_eq!(
        seen,
        [
            (Some(3), 130.0),
            (Some(2), 120.0),
            (Some(2), 120.0),
            (Some(1), 110.0),
            (Some(1), 110.0),
            (None, 100.0),
            (None, 100.0),
        ]
    );
    assert_eq!(ended, 1, "once, at the last stack's end");

    let (mut world, mut schedule) = rig();
    let effect = insert(
        &mut world,
        "swift",
        Effect::new()
            .with_duration(2.0 * STEP)
            .with_stack(3, Refresh::Refresh, Expire::Clear)
            .with_modifier(Stats::MOVE_SPEED, Modifier::Pct(0.1)),
    );
    let hero = world.spawn_with((Stats::new().with(Stats::MOVE_SPEED, 100.0),));
    for _ in 0..3 {
        EffectRequest::new(hero, effect.clone()).apply(&mut world);
    }
    steps(&mut world, &mut schedule, 2);
    assert!(instances(&world, hero).is_empty());
    assert_eq!(stat(&world, hero, Stats::MOVE_SPEED), Some(100.0));
    assert_eq!(expired(&world), [(hero, Some("swift".to_string()), false)]);
}

#[test]
fn modifiers_leave_with_their_source_and_a_games_own_stay() {
    let (mut world, _) = rig();
    let mut stats = Stats::new().with("armor", 2.0);
    stats.add_modifier("armor", "boots", Modifier::Add(1.0));
    let knight = world.spawn_with((stats, Resist::new().with_factor("status", 0.5)));
    let curse = DataRef::inline(
        Effect::new()
            .with_modifier("armor", Modifier::Mul(2.0))
            .with_resist("status", Modifier::Mul(3.0))
            .with_resist("status.burning", Modifier::Add(0.5)),
    );
    EffectRequest::new(knight, curse.clone()).apply(&mut world);
    assert_eq!(stat(&world, knight, "armor"), Some(6.0));
    let resist = world.get::<Resist>(knight).unwrap();
    assert_eq!(resist.factor(&Tags::new(["status.burning"])), 1.5 * 1.5);
    assert!(end_effect(&mut world, knight, &curse));
    assert!(!end_effect(&mut world, knight, &curse), "gone");
    assert_eq!(stat(&world, knight, "armor"), Some(3.0), "the boots stay");
    let resist = world.get::<Resist>(knight).unwrap();
    assert_eq!(resist.factor(&Tags::new(["status.burning"])), 0.5);
    assert_eq!(
        resist.base("status.burning"),
        None,
        "a tag listed by the effect alone left"
    );
    assert_eq!(expired(&world), [(knight, None, true)]);
    // A target without stats or resist takes nothing, traced once.
    let bare = world.spawn();
    EffectRequest::new(bare, curse.clone()).apply(&mut world);
    EffectRequest::new(bare, curse).apply(&mut world);
    assert_eq!(
        skipped(&world, bare),
        [("modifiers", NO_STATS), ("resist", NO_RESIST)]
    );
}

#[test]
fn a_tag_two_instances_grant_leaves_with_the_second_to_expire_in_both_orders() {
    for (first, second) in [(1, 3), (3, 1)] {
        let (mut world, mut schedule) = rig();
        let short = insert(
            &mut world,
            "short",
            Effect::new()
                .with_duration(first as f32 * STEP)
                .with_grant(["status.marked"]),
        );
        let long = insert(
            &mut world,
            "long",
            Effect::new()
                .with_duration(second as f32 * STEP)
                .with_grant(["status.marked", "status.other"]),
        );
        let golem = world.spawn();
        EffectRequest::new(golem, short).apply(&mut world);
        EffectRequest::new(golem, long).apply(&mut world);
        let effects = world.get::<Effects>(golem).unwrap();
        assert_eq!(
            effects.granted().collect::<Vec<_>>(),
            [("status.marked", 2), ("status.other", 1)]
        );
        let mut marked = Vec::new();
        for _ in 0..4 {
            steps(&mut world, &mut schedule, 1);
            marked.push((
                has_tag(&world, golem, "status.marked"),
                has_tag(&world, golem, "status.other"),
            ));
        }
        // `marked` stays while either instance runs; `other` leaves with
        // the one that granted it.
        let expected: Vec<(bool, bool)> = (1..=4)
            .map(|at| (at < first.max(second), at < second))
            .collect();
        assert_eq!(marked, expected, "{first} then {second}");
        assert_eq!(world.get::<Effects>(golem).unwrap().granted().count(), 0);
    }
    // A tag the entity carried before stays.
    let (mut world, mut schedule) = rig();
    let mark = Effect::new()
        .with_duration(STEP)
        .with_grant(["status.marked"]);
    let veteran = world.spawn_with((Tags::new(["status.marked", "team.x"]),));
    EffectRequest::new(veteran, DataRef::inline(mark)).apply(&mut world);
    steps(&mut world, &mut schedule, 2);
    assert!(instances(&world, veteran).is_empty());
    assert!(has_tag(&world, veteran, "status.marked") && has_tag(&world, veteran, "team.x"));
}

#[test]
fn a_blocking_tag_refuses_the_effect() {
    let (mut world, _) = rig();
    let frost = DataRef::inline(
        Effect::new()
            .with_duration(1.0)
            .with_grant(["status.frozen"])
            .with_blocked_by(["status.warm"]),
    );
    let cold = world.spawn();
    let warm = world.spawn_with((Tags::new(["status.warm"]),));
    assert_eq!(
        EffectRequest::new(cold, frost.clone()).apply(&mut world),
        Applied::Applied
    );
    assert_eq!(
        EffectRequest::new(warm, frost).apply(&mut world),
        Applied::Blocked
    );
    assert!(!has_tag(&world, warm, "status.frozen"));
    assert!(world.get::<Effects>(warm).is_none(), "nothing inserted");
    assert_eq!(skipped(&world, warm), [("apply", BLOCKED)]);
    assert_eq!(applied(&world).len(), 1);
}

#[test]
fn restore_stops_at_the_max_stat() {
    let (mut world, mut schedule) = rig();
    let regen = DataRef::inline(
        Effect::new()
            .with_period(0.1)
            .with_restore("mana", 1.5, "max_mana"),
    );
    let mage = world.spawn_with((Stats::new().with("mana", 95.0).with("max_mana", 100.0),));
    let full = world.spawn_with((Stats::new().with("mana", 120.0).with("max_mana", 100.0),));
    let mute = world.spawn_with((Stats::new().with("max_mana", 100.0),));
    for entity in [mage, full, mute] {
        EffectRequest::new(entity, regen.clone()).apply(&mut world);
    }
    let base = |world: &World, entity: Entity| world.get::<Stats>(entity).unwrap().base("mana");
    steps(&mut world, &mut schedule, 12);
    assert_eq!(base(&world, mage), Some(98.0), "two payloads in 0.2 s");
    steps(&mut world, &mut schedule, 12);
    assert_eq!(
        base(&world, mage),
        Some(100.0),
        "capped at max_mana's value"
    );
    steps(&mut world, &mut schedule, 60);
    assert_eq!(base(&world, mage), Some(100.0));
    assert_eq!(
        base(&world, full),
        Some(120.0),
        "a base above the cap is left alone"
    );
    assert_eq!(base(&world, mute), None, "a stat it lacks is not added");
    assert!(skipped(&world, mute).contains(&("restore", NO_RESTORE)));
    // The cap is the max stat's value, its modifiers counted.
    world
        .get_mut::<Stats>(mage)
        .unwrap()
        .add_modifier("max_mana", "ring", Modifier::Add(10.0));
    steps(&mut world, &mut schedule, 60);
    assert_eq!(base(&world, mage), Some(110.0));
    // An instant restore lands once, at the application.
    let potion = DataRef::inline(
        Effect::new()
            .with_duration(0.0)
            .with_restore("mana", 30.0, "max_mana"),
    );
    world.get_mut::<Stats>(mage).unwrap().set_base("mana", 50.0);
    EffectRequest::new(mage, potion).apply(&mut world);
    assert_eq!(base(&world, mage), Some(80.0));
}

#[test]
fn a_suppressing_tag_pauses_and_resumes_a_running_effect() {
    let (mut world, mut schedule) = rig();
    let regen = DataRef::inline(
        Effect::new()
            .with_duration(2.0)
            .with_period(0.1)
            .with_restore("mana", 1.0, "max_mana")
            .with_grant(["status.warm"])
            .with_modifier("armor", Modifier::Add(1.0))
            .with_suppressed_by(["status.frozen"]),
    );
    let stats = || {
        Stats::new()
            .with("mana", 0.0)
            .with("max_mana", 100.0)
            .with("armor", 0.0)
    };
    let held = world.spawn_with((stats(),));
    let free = world.spawn_with((stats(),));
    let frozen = world.spawn_with((stats(), Tags::new(["status.frozen"])));
    for entity in [held, free] {
        EffectRequest::new(entity, regen.clone()).apply(&mut world);
    }
    assert_eq!(
        EffectRequest::new(frozen, regen).apply(&mut world),
        Applied::Applied,
        "an application while held still applies"
    );
    let mana = |world: &World, entity: Entity| world.get::<Stats>(entity).unwrap().base("mana");
    let clock = |world: &World, entity: Entity| {
        world
            .get::<Effects>(entity)
            .and_then(|effects| effects.instances().next())
            .map(|instance| (instance.left(), instance.period_left))
    };
    let mut ended = [None, None];
    let mut during = None;
    for n in 0..200 {
        if n == 15 {
            world.get_mut::<Tags>(held).unwrap().insert("status.frozen");
            during = clock(&world, held);
        }
        if n == 45 {
            assert_eq!(clock(&world, held), during, "its time and period held");
            assert_eq!(mana(&world, held), Some(2.0), "no payload while held");
            assert!(has_tag(&world, held, "status.warm"), "its tags stay");
            assert_eq!(stat(&world, held, "armor"), Some(1.0), "its modifiers stay");
            world.get_mut::<Tags>(held).unwrap().remove("status.frozen");
        }
        step(&mut world, &mut schedule, STEP);
        for (k, entity) in [held, free].into_iter().enumerate() {
            if ended[k].is_none() && instances(&world, entity).is_empty() {
                ended[k] = Some(n);
            }
        }
    }
    let (held_end, free_end) = (ended[0].unwrap(), ended[1].unwrap());
    assert_eq!(held_end, free_end + 30, "it ends 30 held steps later");
    assert_eq!(mana(&world, held), mana(&world, free), "the same payloads");
    assert_eq!(mana(&world, frozen), Some(0.0));
    assert_eq!(clock(&world, frozen), Some((Some(2.0), Some(0.1))));
    let held_lines = |entity| {
        skipped(&world, entity)
            .into_iter()
            .filter(|&(action, why)| (action, why) == ("tick", SUPPRESSED))
            .count()
    };
    assert_eq!(held_lines(held), 1, "traced once per suppression");
    assert_eq!(held_lines(frozen), 1);
    assert_eq!(held_lines(free), 0);
    // Each suppressing tag is checked at load.
    let undeclared = Effect::new().with_suppressed_by(["status.nope"]);
    assert!(undeclared.check(&world).is_err());
    assert!(
        Effect::new()
            .with_suppressed_by(["status.frozen"])
            .check(&world)
            .is_ok()
    );
}

#[test]
fn an_instant_effect_lands_its_payload_and_queues_its_on_expire() {
    let (mut world, mut schedule) = rig();
    let mark = insert(
        &mut world,
        "mark",
        Effect::new()
            .with_duration(5.0)
            .with_grant(["status.marked"]),
    );
    let smite = insert(
        &mut world,
        "smite",
        Effect::new()
            .with_duration(0.0)
            .with_damage(2.0)
            .with_heal(0.5)
            .with_grant(["status.other"])
            .with_modifier("armor", Modifier::Add(9.0))
            .with_on_expire(mark),
    );
    let priest = world.spawn();
    let ghoul = world.spawn_with((Health::new(10.0), Stats::new().with("armor", 0.0)));
    assert_eq!(
        EffectRequest::new(ghoul, smite)
            .with_source(priest)
            .apply(&mut world),
        Applied::Applied
    );
    assert_eq!(world.get::<Health>(ghoul).unwrap().current(), 8.5);
    assert!(world.get::<Effects>(ghoul).is_none(), "no instance");
    assert!(!has_tag(&world, ghoul, "status.other"));
    assert_eq!(stat(&world, ghoul, "armor"), Some(0.0));
    assert_eq!(applied(&world), [(ghoul, 1, Some("smite".to_string()))]);
    assert_eq!(expired(&world), [(ghoul, Some("smite".to_string()), false)]);
    let hit = world
        .resource::<EventQueue<Damaged>>()
        .iter()
        .next()
        .unwrap();
    assert_eq!(
        (hit.amount, hit.source, hit.periodic),
        (2.0, Some(priest), true)
    );
    steps(&mut world, &mut schedule, 1);
    assert!(
        has_tag(&world, ghoul, "status.marked"),
        "the next step's resolve"
    );
    let instance = world
        .get::<Effects>(ghoul)
        .unwrap()
        .instances()
        .next()
        .unwrap();
    assert_eq!(instance.source(), Some(priest), "from the same source");
}

#[test]
fn a_period_and_a_duration_ending_inside_one_step_land_inside_the_lifetime() {
    for dt in [1.0 / 60.0, 1.0 / 30.0] {
        let (mut world, mut schedule) = rig();
        let burn = DataRef::inline(
            Effect::new()
                .with_duration(0.01)
                .with_period(0.005)
                .with_damage(1.0),
        );
        let slime = world.spawn_with((Health::new(10.0),));
        EffectRequest::new(slime, burn).apply(&mut world);
        assert_eq!(
            world.get::<Health>(slime).unwrap().current(),
            10.0,
            "none at the application"
        );
        step(&mut world, &mut schedule, dt);
        assert_eq!(
            world.get::<Health>(slime).unwrap().current(),
            8.0,
            "two ticks at {dt}"
        );
        assert!(instances(&world, slime).is_empty());
        step(&mut world, &mut schedule, dt);
        assert_eq!(world.get::<Health>(slime).unwrap().current(), 8.0);
    }
    // A longer period lands once per period, at most 16 a step.
    let (mut world, mut schedule) = rig();
    let regen = DataRef::inline(Effect::new().with_period(0.1).with_heal(1.0));
    let runner = world.spawn_with((Health::new(100.0),));
    crate::damage(&mut world, runner, 50.0);
    EffectRequest::new(runner, regen).apply(&mut world);
    steps(&mut world, &mut schedule, 60);
    assert_eq!(world.get::<Health>(runner).unwrap().current(), 60.0);
    step(&mut world, &mut schedule, 10.0);
    assert_eq!(
        world.get::<Health>(runner).unwrap().current(),
        76.0,
        "capped at 16"
    );
    // The capped instance starts its next cycle whole, and its save reloads.
    let effects = world.get::<Effects>(runner).unwrap().clone();
    let saved = serde_json::to_value(&effects).unwrap();
    assert_eq!(
        saved["instances"][0]["period_left"].as_f64().unwrap() as f32,
        0.1
    );
    assert_eq!(serde_json::from_value::<Effects>(saved).unwrap(), effects);
}

#[test]
fn a_scaled_duration_and_modifier_land_and_a_huge_factor_clamps() {
    let (mut world, _) = rig();
    world
        .resource_mut::<DataRegistry>()
        .insert("ramp", Curve::new([(0.0, 1.0), (1.0, 2.0), (2.0, 1e30)]));
    let scaled = insert(
        &mut world,
        "scaled",
        Effect::new()
            .with_duration(3.0)
            .with_modifier("armor", Modifier::Add(10.0))
            .with_scale(DataRef::by_id("ramp"), Scaled::ALL),
    );
    let knight = world.spawn_with((Stats::new().with("armor", 0.0),));
    EffectRequest::new(knight, scaled.clone())
        .with_scale(1.0)
        .apply(&mut world);
    assert_eq!(instances(&world, knight)[0].2, Some(6.0));
    assert_eq!(stat(&world, knight, "armor"), Some(20.0));
    let instance = world
        .get::<Effects>(knight)
        .unwrap()
        .instances()
        .next()
        .unwrap();
    assert_eq!(instance.factor(), 2.0);
    EffectRequest::new(knight, scaled)
        .with_scale(2.0)
        .apply(&mut world);
    let factor = f64::from(1e30_f32);
    assert_eq!(instances(&world, knight)[0].2, Some((3.0 * factor) as f32));
    assert_eq!(stat(&world, knight, "armor"), Some((10.0 * factor) as f32));
    // Past f32::MAX, the product clamps and the stat stays finite.
    let huge = insert(
        &mut world,
        "huge",
        Effect::new()
            .with_modifier("armor", Modifier::Add(1e10))
            .with_scale(DataRef::by_id("ramp"), Scaled::NONE.modifiers()),
    );
    let giant = world.spawn_with((Stats::new().with("armor", 0.0),));
    EffectRequest::new(giant, huge)
        .with_scale(2.0)
        .apply(&mut world);
    assert_eq!(stat(&world, giant, "armor"), Some(f32::MAX));
    // A scale that reaches the duration only leaves the modifier as written.
    let half = DataRef::inline(
        Effect::new()
            .with_duration(2.0)
            .with_modifier("armor", Modifier::Add(10.0))
            .with_scale(DataRef::by_id("ramp"), Scaled::NONE.duration()),
    );
    let squire = world.spawn_with((Stats::new().with("armor", 0.0),));
    EffectRequest::new(squire, half)
        .with_scale(1.0)
        .apply(&mut world);
    assert_eq!(instances(&world, squire)[0].2, Some(4.0));
    assert_eq!(stat(&world, squire, "armor"), Some(10.0));
    // A duration scaled to 0 is instant: the payload lands, no instance stays.
    world
        .resource_mut::<DataRegistry>()
        .insert("flat", Curve::new([(0.0, 0.0), (1.0, 1.0)]));
    let brief = DataRef::inline(
        Effect::new()
            .with_duration(1.0)
            .with_damage(2.0)
            .with_modifier("armor", Modifier::Add(10.0))
            .with_grant(["status.marked"])
            .with_scale(DataRef::by_id("flat"), Scaled::ALL),
    );
    let page = world.spawn_with((Health::new(10.0), Stats::new().with("armor", 0.0)));
    assert_eq!(
        EffectRequest::new(page, brief)
            .with_scale(0.0)
            .apply(&mut world),
        Applied::Applied
    );
    assert!(world.get::<Effects>(page).is_none(), "no instance");
    assert_eq!(stat(&world, page, "armor"), Some(0.0));
    assert!(!has_tag(&world, page, "status.marked"));
    assert_eq!(
        world.get::<Health>(page).unwrap().current(),
        10.0,
        "the payload scaled to 0 too"
    );
    assert_eq!(expired(&world).last(), Some(&(page, None, false)));
}

#[test]
fn a_lethal_listed_effect_despawns_at_the_same_step_under_both_plugin_orders() {
    let doomed: PrefabData = serde_json::from_value(json!({
        "tag": "doomed",
        "components": {
            "health": { "max": 1, "despawn_on_death": 0 },
            "effects": [{ "damage": 5 }]
        }
    }))
    .unwrap();
    let mut gone_at = Vec::new();
    for effects_first in [true, false] {
        let (mut world, mut schedule) = rig_with(|schedule, world| {
            if effects_first {
                EffectsPlugin.build(schedule, world);
                HealthPlugin.build(schedule, world);
            } else {
                HealthPlugin.build(schedule, world);
                EffectsPlugin.build(schedule, world);
            }
        });
        let mut prefabs = PrefabRegistry::new();
        prefabs.insert("doomed".to_string(), doomed.clone());
        world.insert_resource(prefabs);
        let prepared = world.prepare_prefab("doomed", [0.0, 0.0]).unwrap();
        let entity = prepared.spawn(&mut world);
        assert_eq!(world.get::<Effects>(entity).unwrap().pending().len(), 1);
        let mut at = None;
        for i in 1..=5 {
            steps(&mut world, &mut schedule, 1);
            if !world.is_alive(entity) {
                at = Some(i);
                break;
            }
        }
        gone_at.push(at);
    }
    assert_eq!(
        gone_at,
        [Some(2), Some(2)],
        "effects first, then health first"
    );
}

#[test]
fn a_k_step_move_speed_effect_moves_a_bodiless_mover_for_k_steps_under_both_orders() {
    for effects_first in [true, false] {
        for k in [1_u32, 3] {
            let (mut world, mut schedule) = rig_with(|schedule, world| {
                if effects_first {
                    EffectsPlugin.build(schedule, world);
                    StatsPlugin::new().build(schedule, world);
                } else {
                    StatsPlugin::new().build(schedule, world);
                    EffectsPlugin.build(schedule, world);
                }
                TopDownMoverPlugin.build(schedule, world);
            });
            let mut mover = TopDownMover::new(100.0);
            mover.set_direction(glam::Vec2::X);
            let hero = world.spawn_with((
                Position(glam::Vec2::ZERO),
                Transform::default(),
                mover,
                Stats::new().with(Stats::MOVE_SPEED, 100.0),
            ));
            let haste = DataRef::inline(
                Effect::new()
                    .with_duration(k as f32 * STEP)
                    .with_modifier(Stats::MOVE_SPEED, Modifier::Pct(1.0)),
            );
            assert!(EffectRequest::new(hero, haste).queue(&mut world));
            let mut moved = Vec::new();
            let mut last = 0.0;
            for _ in 0..(k + 3) {
                steps(&mut world, &mut schedule, 1);
                let x = world.get::<Position>(hero).unwrap().0.x;
                moved.push(((x - last) / STEP).round());
                last = x;
            }
            let mut expected = vec![100.0];
            expected.extend(std::iter::repeat_n(200.0, k as usize));
            expected.extend([100.0, 100.0]);
            assert_eq!(moved, expected, "k = {k}, effects first = {effects_first}");
        }
    }
}

#[test]
fn a_thaw_grants_before_a_lower_index_sources_freeze_on_the_same_step_in_both_orders() {
    for target_first in [true, false] {
        let (mut world, mut schedule) = rig();
        let thawed = insert(
            &mut world,
            "thawed",
            Effect::new()
                .with_duration(1.0)
                .with_grant(["status.thawed"]),
        );
        let frozen = insert(
            &mut world,
            "frozen",
            Effect::new()
                .with_duration(2.0 * STEP)
                .with_grant(["status.frozen"])
                .with_blocked_by(["status.thawed"])
                .with_on_expire(thawed),
        );
        let (golem, mage) = if target_first {
            let golem = world.spawn();
            (golem, world.spawn())
        } else {
            let mage = world.spawn();
            (world.spawn(), mage)
        };
        let later = world.spawn();
        EffectRequest::new(golem, frozen.clone())
            .with_source(later)
            .apply(&mut world);
        steps(&mut world, &mut schedule, 1);
        assert!(has_tag(&world, golem, "status.frozen"));
        // The step the freeze expires on, a lower-index source asks again.
        assert!(
            EffectRequest::new(golem, frozen)
                .with_source(mage)
                .queue(&mut world)
        );
        steps(&mut world, &mut schedule, 1);
        assert!(
            has_tag(&world, golem, "status.thawed"),
            "target first: {target_first}"
        );
        assert!(!has_tag(&world, golem, "status.frozen"));
        let effects = world.get::<Effects>(golem).unwrap();
        assert!(effects.has("thawed") && !effects.has("frozen"));
        assert_eq!(skipped(&world, golem), [("apply", BLOCKED)]);
        let thaw = effects.instances().next().unwrap();
        assert_eq!(thaw.source(), Some(later), "the freeze's source");
    }
}

#[test]
fn a_reloaded_definition_releases_the_tag_the_instance_granted_and_tunes_its_payload() {
    let (mut world, mut schedule) = rig();
    let mark = insert(
        &mut world,
        "mark",
        Effect::new()
            .with_duration(3.0 * STEP)
            .with_period(STEP)
            .with_damage(1.0)
            .with_grant(["status.marked"]),
    );
    let golem = world.spawn_with((Health::new(100.0),));
    EffectRequest::new(golem, mark).apply(&mut world);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(world.get::<Health>(golem).unwrap().current(), 99.0);
    // The reload: another tag, a stronger tick.
    insert(
        &mut world,
        "mark",
        Effect::new()
            .with_duration(3.0 * STEP)
            .with_period(STEP)
            .with_damage(5.0)
            .with_grant(["status.other"]),
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(
        world.get::<Health>(golem).unwrap().current(),
        94.0,
        "tuned live"
    );
    assert!(has_tag(&world, golem, "status.marked"));
    steps(&mut world, &mut schedule, 1);
    assert!(instances(&world, golem).is_empty());
    assert!(
        !has_tag(&world, golem, "status.marked"),
        "released as placed"
    );
    assert!(!has_tag(&world, golem, "status.other"), "never granted");
}

#[test]
fn apply_returns_its_outcome_at_once_and_queue_waits_for_the_resolve() {
    let (mut world, mut schedule) = rig();
    let slow = insert(&mut world, "slow", Effect::new().with_duration(1.0));
    let slime = world.spawn_with((Health::new(1.0),));
    assert_eq!(
        EffectRequest::new(slime, slow.clone()).apply(&mut world),
        Applied::Applied
    );
    assert_eq!(
        EffectRequest::new(slime, slow.clone()).apply(&mut world),
        Applied::Stacked
    );
    let nobody = world.spawn();
    world.despawn(nobody);
    assert_eq!(
        EffectRequest::new(nobody, slow.clone()).apply(&mut world),
        Applied::Ignored
    );
    assert_eq!(
        EffectRequest::new(slime, DataRef::by_id("nothing")).apply(&mut world),
        Applied::Ignored
    );
    crate::damage(&mut world, slime, 5.0);
    assert_eq!(
        EffectRequest::new(slime, slow.clone()).apply(&mut world),
        Applied::Ignored,
        "dead"
    );
    assert_eq!(
        skipped(&world, slime),
        [("apply", NO_EFFECT), ("apply", GONE)]
    );
    let other = world.spawn();
    assert!(EffectRequest::new(other, slow.clone()).queue(&mut world));
    assert!(
        world.get::<Effects>(other).is_none(),
        "nothing until the resolve"
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(instances(&world, other).len(), 1);
    // Without the plugin's queue, nothing queues.
    let (mut bare, _) = rig_with(|_, _| {});
    let lone = bare.spawn();
    assert!(!EffectRequest::new(lone, slow).queue(&mut bare));
    let request = EffectRequest::new(lone, DataRef::by_id("x"))
        .with_source(other)
        .with_scale(2.5);
    assert_eq!(
        (request.target(), request.source(), request.scale()),
        (lone, Some(other), 2.5)
    );
    assert_eq!(request.effect().id(), Some("x"));
}

#[test]
fn the_component_saves_its_instances_and_table_without_the_source() {
    let (mut world, mut schedule) = rig();
    let mark = insert(
        &mut world,
        "mark",
        Effect::new()
            .with_duration(2.0)
            .with_period(0.5)
            .with_grant(["status.marked"]),
    );
    let mage = world.spawn();
    let golem = world.spawn_with((Tags::new(["status.marked"]),));
    EffectRequest::new(golem, mark)
        .with_source(mage)
        .with_scale(1.0)
        .apply(&mut world);
    EffectRequest::new(
        golem,
        DataRef::inline(Effect::new().with_grant(["status.other"])),
    )
    .apply(&mut world);
    steps(&mut world, &mut schedule, 1);
    let effects = world.get::<Effects>(golem).unwrap().clone();
    let saved = serde_json::to_value(&effects).unwrap();
    assert_eq!(saved["next_serial"], 2);
    assert_eq!(saved["instances"][0]["effect"], "mark");
    assert_eq!(saved["instances"][0]["granted"], json!(["status.marked"]));
    assert_eq!(
        saved["instances"][1]["effect"]["grant"],
        json!(["status.other"])
    );
    assert_eq!(
        saved["granted"],
        json!([
            { "tag": "status.marked", "owners": 1, "prior": true },
            { "tag": "status.other", "owners": 1, "prior": false }
        ])
    );
    assert!(saved["instances"][0].get("source").is_none());
    let read: Effects = serde_json::from_value(saved).unwrap();
    let instance = read.instances().next().unwrap();
    assert_eq!(instance.source(), None, "an entity is not data");
    assert_eq!(instance.stacks(), 1);
    assert!(close(instance.left().unwrap(), 2.0 - STEP));
    assert_eq!(
        read.granted().collect::<Vec<_>>(),
        [("status.marked", 1), ("status.other", 1)]
    );
    assert!(
        serde_json::from_value::<Effects>(json!({ "instances": [{ "effect": "x", "stacks": 0 }] }))
            .is_err()
    );
    assert!(
        serde_json::from_value::<Effects>(json!({ "instances": [{ "effect": "x", "left": -1 }] }))
            .is_err()
    );
    assert!(
        serde_json::from_value::<Effects>(
            json!({ "granted": [{ "tag": "Bad", "owners": 1, "prior": false }] })
        )
        .is_err()
    );
    let rows = effects.inspect_rows();
    assert_eq!(rows[0].0, "effect");
    assert!(
        rows[0]
            .1
            .starts_with("mark ×1, 1.98 s, next payload 0.48 s, from "),
        "{}",
        rows[0].1
    );
    assert_eq!(rows[1].1, "inline ×1, infinite");
    assert_eq!(
        rows[2],
        ("granted", "status.marked (1), status.other (1)".to_string())
    );
    let pending = Effects::new().with(DataRef::by_id("regen"));
    assert_eq!(pending.inspect_rows(), [("pending", "regen".to_string())]);
    assert_eq!(pending.pending().len(), 1);
    assert!(!pending.has("regen") && pending.stacks("regen") == 0);
}

#[test]
fn set_source_restores_a_loaded_instances_source_by_serial() {
    let (mut world, mut schedule) = rig();
    let mark = insert(&mut world, "mark", Effect::new().with_duration(2.0));
    let burn = insert(
        &mut world,
        "burn",
        Effect::new()
            .with_duration(1.0)
            .with_period(0.5)
            .with_damage(2.0)
            .with_on_expire(mark),
    );
    let mage = world.spawn();
    let golem = world.spawn_with((Health::new(100.0),));
    EffectRequest::new(golem, burn)
        .with_source(mage)
        .apply(&mut world);
    // A save and a load: the instance comes back with no source.
    let saved = serde_json::to_value(world.get::<Effects>(golem).unwrap()).unwrap();
    let mut loaded: Effects = serde_json::from_value(saved).unwrap();
    let serial = loaded.instances().next().unwrap().serial();
    assert_eq!(loaded.instances().next().unwrap().source(), None);
    let before = loaded.clone();
    assert!(
        !loaded.set_source(serial + 7, Some(mage)),
        "no such instance"
    );
    assert_eq!(loaded, before, "an unknown serial changes nothing");
    assert!(loaded.set_source(serial, Some(mage)));
    let (named, was) = (
        loaded.instances().next().unwrap(),
        before.instances().next().unwrap(),
    );
    assert_eq!(named.source(), Some(mage));
    assert_eq!(
        (named.stacks(), named.left(), named.factor(), named.serial()),
        (was.stacks(), was.left(), was.factor(), was.serial()),
        "only the source changes"
    );
    world.insert(golem, loaded);
    // Its ticks, and the `on_expire` it hands on, come from the mage.
    let mut ticks = Vec::new();
    for _ in 0..62 {
        steps(&mut world, &mut schedule, 1);
        ticks.extend(
            world
                .resource::<EventQueue<Damaged>>()
                .iter()
                .map(|hit| (hit.source, hit.periodic)),
        );
    }
    assert!(!ticks.is_empty());
    assert!(
        ticks.iter().all(|&tick| tick == (Some(mage), true)),
        "{ticks:?}"
    );
    let effects = world.get_mut::<Effects>(golem).unwrap();
    let next = effects.instances().next().unwrap();
    assert_eq!(
        (next.effect().id(), next.source()),
        (Some("mark"), Some(mage))
    );
    let serial = next.serial();
    assert!(effects.set_source(serial, None), "cleared");
    assert_eq!(effects.instances().next().unwrap().source(), None);
}

#[test]
fn the_data_form_round_trips_and_refuses_bad_values() {
    let effect: Effect = serde_json::from_value(json!({
        "duration": 3, "period": 0.5,
        "damage": { "elemental.fire": 1 }, "heal": 0,
        "stack": { "limit": 3, "refresh": "extend", "expire": "remove_one" },
        "modifiers": [{ "stat": "move_speed", "modifier": { "pct": -0.2 } }],
        "resist": [{ "tag": "digital", "modifier": { "add": 0.25 } }],
        "grant": ["status.burning"], "blocked_by": ["status.wet"],
        "restore": { "stat": "mana", "amount": 1.5, "max": "max_mana" },
        "suppressed_by": ["casting", "casting"],
        "scale": { "curve": "burn_scale", "into": ["duration", "periodic", "modifiers"] },
        "on_expire": "thawed"
    }))
    .unwrap();
    assert_eq!(
        effect,
        Effect::new()
            .with_duration(3.0)
            .with_period(0.5)
            .with_typed_damage("elemental.fire", 1.0)
            .with_stack(3, Refresh::Extend, Expire::RemoveOne)
            .with_modifier("move_speed", Modifier::Pct(-0.2))
            .with_resist("digital", Modifier::Add(0.25))
            .with_grant(["status.burning"])
            .with_blocked_by(["status.wet"])
            .with_restore("mana", 1.5, "max_mana")
            .with_suppressed_by(["casting"])
            .with_scale(DataRef::by_id("burn_scale"), Scaled::ALL)
            .with_on_expire(DataRef::by_id("thawed"))
    );
    assert_eq!(effect.restore(), Some(("mana", 1.5, "max_mana")));
    assert_eq!(effect.suppressed_by(), ["casting"]);
    let back = serde_json::to_value(&effect).unwrap();
    assert_eq!(serde_json::from_value::<Effect>(back).unwrap(), effect);
    assert_eq!(
        serde_json::to_value(Effect::new()).unwrap(),
        json!({
            "duration": null, "period": null, "damage": 0.0, "heal": 0.0,
            "stack": { "limit": 1, "refresh": "refresh", "expire": "clear" },
            "modifiers": [], "resist": [], "grant": [], "blocked_by": [],
            "restore": null, "suppressed_by": [],
            "scale": null, "on_expire": null
        })
    );
    for bad in [
        json!({ "duration": -1 }),
        json!({ "period": 0 }),
        json!({ "heal": -1 }),
        json!({ "damage": -1 }),
        json!({ "stack": { "limit": 0 } }),
        json!({ "modifiers": [{ "stat": "Move", "modifier": { "add": 1 } }] }),
        json!({ "modifiers": [{ "stat": "move_speed", "modifier": { "add": 1, "pct": 1 } }] }),
        json!({ "resist": [{ "tag": "Digital", "modifier": { "add": 1 } }] }),
        json!({ "grant": ["Bad Tag"] }),
        json!({ "scale": { "curve": "x", "into": ["heal"] } }),
        json!({ "scale": { "into": [] } }),
        json!({ "extra": 1 }),
        json!({ "restore": { "stat": "mana", "amount": 1 } }),
        json!({ "restore": { "amount": 1, "max": "max_mana" } }),
        json!({ "restore": { "stat": "mana", "amount": 0, "max": "max_mana" } }),
        json!({ "restore": { "stat": "mana", "amount": -1, "max": "max_mana" } }),
        json!({ "restore": { "stat": "Mana", "amount": 1, "max": "max_mana" } }),
        json!({ "restore": { "stat": "mana", "amount": 1, "max": "max mana" } }),
        json!({ "restore": { "stat": "mana", "amount": 1, "max": "max_mana", "per": 1 } }),
        json!({ "suppressed_by": ["Bad Tag"] }),
    ] {
        assert!(
            serde_json::from_value::<Effect>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let scaled: Effect = serde_json::from_value(json!({ "scale": { "curve": [[0, 1]] } })).unwrap();
    assert_eq!(
        scaled.scale().unwrap().1,
        Scaled::ALL,
        "every target by default"
    );
    let none: Effect =
        serde_json::from_value(json!({ "scale": { "curve": "x", "into": [] } })).unwrap();
    assert_eq!(none.scale().unwrap().1, Scaled::NONE);
    // The builders refuse what the data refuses.
    assert!(std::panic::catch_unwind(|| Effect::new().with_duration(-1.0)).is_err());
    assert!(std::panic::catch_unwind(|| Effect::new().with_period(0.0)).is_err());
    assert!(
        std::panic::catch_unwind(|| Effect::new().with_stack(0, Refresh::Keep, Expire::Clear))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| Effect::new().with_modifier("Move", Modifier::Add(1.0)))
            .is_err()
    );
    assert!(std::panic::catch_unwind(|| Effect::new().with_typed_damage("x", -1.0)).is_err());
    for (stat, amount, max) in [
        ("Mana", 1.0, "max_mana"),
        ("mana", 1.0, "max mana"),
        ("mana", 0.0, "max_mana"),
        ("mana", f32::NAN, "max_mana"),
        ("mana", f32::INFINITY, "max_mana"),
    ] {
        assert!(
            std::panic::catch_unwind(|| Effect::new().with_restore(stat, amount, max)).is_err(),
            "{stat} {amount} {max}"
        );
    }
    assert!(std::panic::catch_unwind(|| Effect::new().with_suppressed_by(["Bad Tag"])).is_err());
    assert_eq!(
        Effect::new()
            .with_suppressed_by(["b", "a"])
            .with_suppressed_by(["b"])
            .suppressed_by(),
        ["a", "b"]
    );
    let typed = Effect::new()
        .with_damage(4.0)
        .with_typed_damage("a", 1.0)
        .with_typed_damage("b", 2.0)
        .with_typed_damage("a", 3.0);
    assert_eq!(typed.damage(), 5.0, "the number gave way; `a` replaced");
    assert_eq!(typed.with_damage(1.0).damage(), 1.0);
    // A typed part of 0 is no part, so every built effect reloads.
    let zeroed = Effect::new()
        .with_typed_damage("a", 0.0)
        .with_typed_damage("b", 1.0)
        .with_typed_damage("b", 0.0);
    assert_eq!(serde_json::to_value(&zeroed).unwrap()["damage"], json!(0.0));
    let saved = serde_json::to_value(&zeroed).unwrap();
    assert_eq!(serde_json::from_value::<Effect>(saved).unwrap(), zeroed);
    let partly = Effect::new()
        .with_typed_damage("a", 0.0)
        .with_typed_damage("b", 1.0);
    assert_eq!(
        serde_json::to_value(&partly).unwrap()["damage"],
        json!({ "b": 1.0 })
    );
    let effect = Effect::new()
        .with_grant(["b", "a", "b"])
        .with_blocked_by(["z"])
        .with_heal(2.0);
    assert_eq!(effect.grant(), ["a", "b"]);
    assert_eq!(
        (effect.blocked_by(), effect.heal()),
        (&["z".to_string()][..], 2.0)
    );
    assert_eq!(
        (effect.refresh(), effect.expire(), effect.stack_limit()),
        (Refresh::Refresh, Expire::Clear, 1)
    );
    assert_eq!(effect.modifiers().count() + effect.resist().count(), 0);
    assert!(effect.on_expire().is_none() && effect.period().is_none());
}

#[test]
fn the_effect_cue_action_queues_on_the_subject_or_the_other() {
    let (mut world, mut schedule) = rig();
    let mark = insert(
        &mut world,
        "mark",
        Effect::new()
            .with_duration(1.0)
            .with_grant(["status.marked"]),
    );
    let burn = insert(
        &mut world,
        "burn",
        Effect::new()
            .with_duration(1.0)
            .with_grant(["status.burning"]),
    );
    // A hit on the golem: its own cue marks it (from the source), and the
    // source's cue burns the golem (the other).
    let golem = world.spawn_with((
        Health::new(10.0),
        Cues::new().on("damaged", [CueAction::effect(mark.clone())]),
    ));
    let mage = world.spawn_with((Cues::new().on(
        "effect_applied",
        [CueAction::counter_add("applied", 1).for_game()],
    ),));
    crate::DamageRequest::new(golem, 1.0)
        .with_source(mage)
        .apply(&mut world);
    steps(&mut world, &mut schedule, 1);
    assert!(
        !has_tag(&world, golem, "status.marked"),
        "queued by the cues, after the resolve"
    );
    steps(&mut world, &mut schedule, 1);
    assert!(has_tag(&world, golem, "status.marked"));
    let instance = world
        .get::<Effects>(golem)
        .unwrap()
        .instances()
        .next()
        .unwrap();
    assert_eq!(
        instance.source(),
        Some(mage),
        "from the event's other entity"
    );
    // `effect_applied` on the golem counts, with the stacks as the amount.
    let counted = world.spawn_with((Cues::new()
        .on("effect_applied", [CueAction::counter_add("applied", 1)])
        .on("effect_expired", [CueAction::counter_add("expired", 1)]),));
    EffectRequest::new(counted, burn.clone()).apply(&mut world);
    EffectRequest::new(counted, burn.clone()).apply(&mut world);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(world.get::<Counters>(counted).unwrap().get("applied"), 2);
    assert!(end_effect(&mut world, counted, &burn));
    steps(&mut world, &mut schedule, 1);
    assert_eq!(world.get::<Counters>(counted).unwrap().get("expired"), 1);
    // `for_other` puts it on the other, from the subject.
    let hitter = world
        .spawn_with((Cues::new().on("effect_applied", [CueAction::effect(burn).for_other()]),));
    let victim = world.spawn();
    EffectRequest::new(hitter, mark)
        .with_source(victim)
        .apply(&mut world);
    steps(&mut world, &mut schedule, 2);
    assert!(has_tag(&world, victim, "status.burning"));
    let instance = world
        .get::<Effects>(victim)
        .unwrap()
        .instances()
        .next()
        .unwrap();
    assert_eq!(instance.source(), Some(hitter));
    // The load checks the effect and the target.
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in("cues", &value, &world)
            .map_err(|e| e.to_string())
    };
    assert!(check(json!({ "damaged": [{ "effect": "mark" }] })).is_ok());
    assert!(
        check(json!({ "damaged": [{ "effect": { "id": { "duration": 1 }, "target": "other" } }] }))
            .is_ok()
    );
    assert_eq!(
        check(json!({ "damaged": [{ "effect": "nope" }] })).unwrap_err(),
        "component `cues`, field `damaged.0.effect`: unknown effects `nope`"
    );
    assert_eq!(
        check(json!({ "cooldown_elapsed": [{ "effect": { "id": "mark", "target": "other" } }] }))
            .unwrap_err(),
        "component `cues`, field `cooldown_elapsed.0.effect.target`: unknown cue event with an other entity `cooldown_elapsed`"
    );
    assert!(
        check(json!({ "damaged": [{ "effect": { "id": { "grant": ["status.nowhere"] } } }] }))
            .is_err()
    );
    assert!(
        serde_json::from_value::<CueAction>(json!({ "effect": { "duration": 1 } })).is_err(),
        "the definition sits under `id`"
    );
    assert!(
        serde_json::from_value::<CueAction>(json!({ "effect": { "id": "x", "target": "game" } }))
            .is_err()
    );
}
