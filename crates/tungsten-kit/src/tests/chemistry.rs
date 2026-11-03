use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};
use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::{CommandBuffer, DataRef, DataRegistry, Schedule, Stage, Time};

use super::*;
use crate::HealthPlugin;
use crate::damage::defense::Resist;
use crate::damage::effects::{Effect, EffectRequest, Effects, EffectsPlugin, end_effect};
use crate::damage::kinds::DamageTypes;
use crate::damage::pipeline::{DamagePart, DamageRequest};
use crate::foundation::curve::Curve;
use crate::foundation::stats::{Stats, StatsPlugin};
use crate::foundation::tags::TagsPlugin;
use crate::foundation::trace::KitTrace;

const STEP: f32 = 1.0 / 60.0;

/// What a restored spawn takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Restored;

/// The tags the tests declare.
const TAGS: [&str; 9] = [
    "material.wood",
    "material.stone",
    "material.glass",
    "flammable",
    "status.burning",
    "status.armed",
    "status.cover",
    "heat",
    "team.enemy",
];

/// A World with time, a command buffer, a trace, tags, stats, health,
/// effects and `plugin`, the damage types `elemental`, `elemental.fire`
/// and `elemental.cold`, and the statuses the tests name.
fn rig_with(plugin: &ChemistryPlugin) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(TAGS)
        .build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    plugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: DamageTypes = serde_json::from_value(json!({
        "elemental": {}, "elemental.fire": {}, "elemental.cold": {}
    }))
    .unwrap();
    let data = world.resource_mut::<DataRegistry>();
    data.insert("fantasy", types);
    data.insert(
        "burning",
        Effect::new()
            .with_intensity(10.0, 0.0)
            .with_grant(["status.burning"]),
    );
    for id in [
        "scorched",
        "charred",
        "cracked",
        "shattered",
        "soaked",
        "mark",
    ] {
        data.insert(id, Effect::new());
    }
    (world, schedule)
}

fn rig() -> (World, Schedule) {
    rig_with(&ChemistryPlugin::new())
}

/// One fixed step at `dt`, then the frame's command flush.
fn step_at(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

fn step(world: &mut World, schedule: &mut Schedule) {
    step_at(world, schedule, STEP);
}

/// Inserts the rule `data` as the `interactions` entry `id`.
fn rule(world: &mut World, id: &str, data: Value) {
    let rule: Interaction = serde_json::from_value(data).unwrap();
    world.resource_mut::<DataRegistry>().insert(id, rule);
}

/// A receiver with health and `tags`.
fn receiver(world: &mut World, tags: &[&str]) -> Entity {
    world.spawn_with((Health::new(1000.0), Tags::new(tags.iter().copied())))
}

/// A hit of `amount` of `kind` on `target`, landed now.
fn hit(world: &mut World, target: Entity, kind: &str, amount: f32) {
    DamageRequest::from_parts(
        target,
        [DamagePart::new(amount).with_tags(Tags::new([kind]))],
    )
    .apply(world);
}

fn has(world: &World, entity: Entity, id: &str) -> bool {
    world
        .get::<Effects>(entity)
        .is_some_and(|effects| effects.has(id))
}

fn intensity(world: &World, entity: Entity, id: &str) -> Option<f32> {
    world
        .get::<Effects>(entity)
        .and_then(|effects| effects.intensity(id))
}

fn chemistry(world: &World) -> &Chemistry {
    world.resource::<Chemistry>()
}

/// The `(action, why)` of each chemistry skip the trace holds about
/// `entity`.
fn skips(world: &World, entity: Entity) -> Vec<(&'static str, &'static str)> {
    world
        .resource::<KitTrace>()
        .about(entity)
        .filter_map(|entry| match entry.kind {
            TraceKind::Skipped {
                event: "interaction",
                action,
                why,
            } => Some((action, why)),
            _ => None,
        })
        .collect()
}

/// The `(action, rule)` of each chemistry output the trace holds about
/// `entity`.
fn ran(world: &World, entity: Entity) -> Vec<(&'static str, String)> {
    world
        .resource::<KitTrace>()
        .about(entity)
        .filter_map(|entry| match &entry.kind {
            TraceKind::Cue {
                event: "interaction",
                action,
                rule,
            } => Some((*action, rule.clone().unwrap_or_default())),
            _ => None,
        })
        .collect()
}

#[test]
fn the_plugin_registers_its_kind_queue_and_system() {
    let (world, schedule) = rig();
    assert!(world.resource::<DataRegistry>().contains_kind(INTERACTIONS));
    assert!(world.has_resource::<Chemistry>());
    let names: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
    assert!(at(DAMAGE_RESOLVE) < at(CHEMISTRY));
    assert!(at(AILMENT_FEED) < at(CHEMISTRY));
    assert!(at(CHEMISTRY) < at(crate::EFFECT_RESOLVE));
    // The kit declares its stimuli, so a rule names them anywhere.
    for name in [
        "stimulus",
        "stimulus.impact",
        "stimulus.light",
        "stimulus.blast",
    ] {
        assert_eq!(
            crate::foundation::tags::check_declared(&world, "x", name),
            Ok(()),
            "{name}"
        );
    }
}

#[test]
fn the_most_specific_rule_wins() {
    let (mut world, mut schedule) = rig();
    let outputs = |status: &str| json!([{ "apply": { "status": status } }]);
    rule(
        &mut world,
        "a_material",
        json!({ "stimulus": "elemental.fire", "on": { "all": ["material"] }, "outputs": outputs("scorched") }),
    );
    rule(
        &mut world,
        "b_wood",
        json!({ "stimulus": "elemental.fire", "on": { "all": ["material.wood"] }, "outputs": outputs("charred") }),
    );
    rule(
        &mut world,
        "c_glass",
        json!({ "stimulus": "elemental.fire", "on": { "all": ["material.glass"] }, "outputs": outputs("cracked") }),
    );
    rule(
        &mut world,
        "d_glass",
        json!({ "stimulus": "elemental.fire", "on": { "all": ["material.glass"] }, "outputs": outputs("shattered") }),
    );
    // An ancestor's rule takes its descendants, below a deeper one.
    rule(
        &mut world,
        "e_elemental",
        json!({ "stimulus": "elemental", "on": { "all": ["material.wood"] }, "outputs": outputs("soaked") }),
    );
    let crate_ = receiver(&mut world, &["material.wood"]);
    let block = receiver(&mut world, &["material.stone"]);
    let pane = receiver(&mut world, &["material.glass"]);
    for entity in [crate_, block, pane] {
        hit(&mut world, entity, "elemental.fire", 2.0);
    }
    step(&mut world, &mut schedule);
    let carried = |world: &World, entity: Entity| -> Vec<&str> {
        ["scorched", "charred", "cracked", "shattered", "soaked"]
            .into_iter()
            .filter(|id| has(world, entity, id))
            .collect()
    };
    assert_eq!(carried(&world, crate_), ["charred"], "the wood rule alone");
    assert_eq!(carried(&world, block), ["scorched"], "the material rule");
    assert_eq!(carried(&world, pane), ["cracked"], "the lower ID");
    assert_eq!(ran(&world, crate_), [("apply", "b_wood.0".to_string())]);
    hit(&mut world, crate_, "elemental.cold", 2.0);
    step(&mut world, &mut schedule);
    assert!(has(&world, crate_, "soaked"), "cold: the elemental rule");
}

#[test]
fn a_landed_part_stimulates_by_its_types() {
    let (mut world, mut schedule) = rig();
    rule(
        &mut world,
        "burns",
        json!({
            "stimulus": "elemental.fire", "on": { "all": ["flammable"] }, "min": 0.5,
            "outputs": [{ "build": { "status": "burning", "scale": 1 } }]
        }),
    );
    let crate_ = world.spawn_with((
        Health::new(1000.0),
        Tags::new(["flammable"]),
        Resist::new().with_factor("elemental.fire", 0.5),
    ));
    hit(&mut world, crate_, "elemental.fire", 6.0);
    step(&mut world, &mut schedule);
    assert_eq!(
        intensity(&world, crate_, "burning"),
        Some(3.0),
        "after the resistance"
    );
    // A periodic part stimulates nothing, nor one under the rule's `min`.
    DamageRequest::from_parts(
        crate_,
        [DamagePart::new(6.0).with_tags(Tags::new(["elemental.fire"]))],
    )
    .with_periodic(true)
    .apply(&mut world);
    step(&mut world, &mut schedule);
    hit(&mut world, crate_, "elemental.fire", 0.8);
    step(&mut world, &mut schedule);
    assert_eq!(intensity(&world, crate_, "burning"), Some(3.0));
    assert_eq!(chemistry(&world).ran(), 2, "the hit and the one under min");
    assert!(skips(&world, crate_).contains(&("all", NO_RULE)));
}

#[test]
fn one_entry_per_stimulus_and_receiver() {
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_bound(4));
    rule(
        &mut world,
        "burns",
        json!({ "stimulus": "elemental.fire", "outputs": [{ "build": { "status": "burning", "scale": 1 } }] }),
    );
    let crate_ = receiver(&mut world, &["flammable"]);
    for _ in 0..10 {
        hit(&mut world, crate_, "elemental.fire", 1.0);
    }
    step(&mut world, &mut schedule);
    assert_eq!(chemistry(&world).ran(), 1, "one entry");
    assert_eq!(intensity(&world, crate_, "burning"), Some(10.0), "summed");
    // At a bound of 4, a fifth receiver's add is dropped and counted.
    let five: Vec<Entity> = (0..5).map(|_| receiver(&mut world, &[])).collect();
    for &entity in &five {
        hit(&mut world, entity, "elemental.fire", 1.0);
    }
    step(&mut world, &mut schedule);
    assert_eq!(
        (chemistry(&world).dropped(), chemistry(&world).ran()),
        (1, 5)
    );
    assert!(!chemistry(&world).dropped_now, "warned once, then cleared");
    assert!(has(&world, five[3], "burning") && !has(&world, five[4], "burning"));
}

#[test]
fn the_cap_bounds_a_run_oldest_first() {
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_cap(2));
    rule(
        &mut world,
        "marks",
        json!({ "stimulus": "elemental.fire", "outputs": [{ "build": { "status": "burning", "amount": 1 } }] }),
    );
    // Five entries at once: the shallower first, then save order.
    let five: Vec<Entity> = (0..5).map(|_| receiver(&mut world, &[])).collect();
    for (i, &entity) in five.iter().enumerate() {
        let depth = u8::from(i != 4);
        DamageRequest::from_parts(
            entity,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_depth(depth)
        .apply(&mut world);
    }
    let marked = |world: &World| -> Vec<usize> {
        (0..5).filter(|&i| has(world, five[i], "burning")).collect()
    };
    step(&mut world, &mut schedule);
    assert_eq!(marked(&world), [0, 4], "two a run: depth 0, then the first");
    step(&mut world, &mut schedule);
    assert_eq!(marked(&world), [0, 1, 2, 4]);
    step(&mut world, &mut schedule);
    assert_eq!(marked(&world), [0, 1, 2, 3, 4]);

    // A, B and C at once, A and B hit again each step: C runs second.
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_cap(2));
    rule(
        &mut world,
        "marks",
        json!({ "stimulus": "elemental.fire", "outputs": [{ "build": { "status": "burning", "amount": 1 } }] }),
    );
    let [a, b, c] = [0, 1, 2].map(|_| receiver(&mut world, &[]));
    for entity in [a, b, c] {
        hit(&mut world, entity, "elemental.fire", 1.0);
    }
    step(&mut world, &mut schedule);
    assert!(has(&world, a, "burning") && has(&world, b, "burning") && !has(&world, c, "burning"));
    hit(&mut world, a, "elemental.fire", 1.0);
    hit(&mut world, b, "elemental.fire", 1.0);
    step(&mut world, &mut schedule);
    assert!(
        has(&world, c, "burning"),
        "the waiting entry before fresh work"
    );
}

#[test]
fn an_entry_decides_when_it_runs() {
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_cap(1));
    rule(
        &mut world,
        "wood",
        json!({
            "stimulus": "elemental.fire", "on": { "all": ["material.wood"] },
            "outputs": [{ "build": { "status": "burning", "amount": 1 } }, { "remove": "mark" }]
        }),
    );
    // A backlog ahead of four receivers, so each entry waits a run.
    let first = receiver(&mut world, &["material.wood"]);
    let gone = receiver(&mut world, &["material.wood"]);
    let despawning = receiver(&mut world, &["material.wood"]);
    let changed = receiver(&mut world, &["material.wood"]);
    let dead = receiver(&mut world, &["material.wood"]);
    EffectRequest::new(dead, DataRef::by_id("mark")).apply(&mut world);
    for entity in [first, gone, despawning, changed, dead] {
        hit(&mut world, entity, "elemental.fire", 1.0);
    }
    step(&mut world, &mut schedule);
    assert!(has(&world, first, "burning"));
    world.despawn(gone);
    world.insert(despawning, crate::lifecycle::ended::Despawning);
    world
        .get_mut::<Tags>(changed)
        .unwrap()
        .remove("material.wood");
    world.get_mut::<Health>(dead).unwrap().hurt(5000.0, false);
    for _ in 0..4 {
        step(&mut world, &mut schedule);
    }
    assert_eq!(chemistry(&world).ran(), 5);
    assert!(!has(&world, despawning, "burning"));
    assert_eq!(skips(&world, despawning), [("all", GONE)]);
    assert!(!has(&world, changed, "burning"));
    assert_eq!(skips(&world, changed), [("all", NO_RULE)]);
    // The dead still react: the build refused, the remove run.
    assert!(!has(&world, dead, "burning") && !has(&world, dead, "mark"));
    assert!(skips(&world, dead).contains(&("build", outputs::DEAD)));
    assert!(ran(&world, dead).contains(&("remove", "wood.1".to_string())));
}

#[test]
fn a_reaction_outlives_its_receivers_death() {
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_cap(1));
    rule(
        &mut world,
        "wood",
        json!({ "stimulus": "elemental.fire", "on": { "all": ["material.wood"] }, "outputs": [{ "remove": "mark" }] }),
    );
    // A backlog of two, then three receivers killed in one step with a
    // death delay of 0.
    let backlog = [0, 1].map(|_| receiver(&mut world, &["material.wood"]));
    for entity in backlog {
        hit(&mut world, entity, "elemental.fire", 1.0);
    }
    step(&mut world, &mut schedule);
    let doomed = [0, 1, 2].map(|_| {
        let entity = world.spawn_with((
            Health::new(1.0).with_despawn_on_death(0.0),
            Tags::new(["material.wood"]),
        ));
        EffectRequest::new(entity, DataRef::by_id("mark")).apply(&mut world);
        entity
    });
    for entity in doomed {
        hit(&mut world, entity, "elemental.fire", 5.0);
    }
    let mut alive_until = [0; 3];
    for frame in 1..=10 {
        step(&mut world, &mut schedule);
        for (i, &entity) in doomed.iter().enumerate() {
            if world.is_alive(entity) {
                alive_until[i] = frame;
            }
        }
    }
    // Each reaction ran in its turn, its receiver held until then.
    for &entity in &doomed {
        assert!(!world.is_alive(entity), "despawned after its reaction");
    }
    assert!(
        alive_until[0] < alive_until[1] && alive_until[1] < alive_until[2],
        "{alive_until:?}"
    );
    assert_eq!(chemistry(&world).ran(), 5);
    assert_eq!(chemistry(&world).pending(), 0);
    // A held receiver takes no new entry, so its hold ends.
    let held = world.spawn_with((Health::new(1.0), Tags::new(["material.wood"])));
    let chemistry_mut = world.resource_mut::<Chemistry>();
    chemistry_mut.hold(held);
    chemistry_mut.stimulate(
        &Settings::default(),
        held,
        "elemental.fire",
        queue::Stimulus {
            amount: 1.0,
            status_effect: 1.0,
            dot_base: 1.0,
            depth: 0,
            credit: None,
        },
        0,
    );
    assert_eq!(chemistry(&world).pending(), 0);
    assert!(!chemistry(&world).holds(held));
}

#[test]
fn merged_hits_keep_their_own_status_effects() {
    let (mut world, mut schedule) = rig();
    rule(
        &mut world,
        "builds",
        json!({
            "stimulus": "elemental.fire", "min": 2,
            "outputs": [{ "build": { "status": "burning", "scale": 1 } }]
        }),
    );
    let strong = world.spawn_with((Stats::new().with(Stats::STATUS_EFFECT, 2.0),));
    let plain = world.spawn();
    let crate_ = receiver(&mut world, &[]);
    for source in [strong, plain] {
        DamageRequest::from_parts(
            crate_,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_source(source)
        .apply(&mut world);
    }
    step(&mut world, &mut schedule);
    assert_eq!(
        intensity(&world, crate_, "burning"),
        Some(3.0),
        "1 × 2 + 1 × 1, and the raw 2 passes `min`"
    );
    let credited = world
        .get::<Effects>(crate_)
        .unwrap()
        .instances()
        .next()
        .unwrap()
        .source();
    assert_eq!(credited, Some(strong), "the first credit");
}

/// Counts the calls of each handler a test registers.
#[derive(Default)]
struct Calls(RefCell<Vec<String>>);

fn counting(calls: &Rc<Calls>, name: &'static str) -> impl Fn(&mut World, &Reaction) + 'static {
    let calls = Rc::clone(calls);
    move |_, reaction| {
        calls
            .0
            .borrow_mut()
            .push(format!("{name}:{}", reaction.rule()));
    }
}

/// What a handler saw of each reaction: its rule, credit and depth.
type Notes = Rc<RefCell<Vec<(String, Option<Entity>, u8)>>>;

fn noting(notes: &Notes) -> impl Fn(&mut World, &Reaction) + 'static {
    let notes = Rc::clone(notes);
    move |_, reaction| {
        notes.borrow_mut().push((
            reaction.rule().to_owned(),
            reaction.credit(),
            reaction.depth(),
        ));
    }
}

#[test]
fn a_status_ends_with_its_outputs() {
    let calls = Rc::new(Calls::default());
    let notes = Notes::default();
    let (mut world, mut schedule) = rig_with(
        &ChemistryPlugin::new()
            .with_handler("ended", counting(&calls, "ended"))
            .with_handler("entered", counting(&calls, "entered"))
            .with_handler("noted", noting(&notes)),
    );
    let ends = json!([{ "handler": "ended" }]);
    let status = |extra: Value| -> Effect {
        let mut data = json!({ "on_end": ends.clone() });
        for (key, value) in extra.as_object().unwrap() {
            data[key] = value.clone();
        }
        serde_json::from_value(data).unwrap()
    };
    let data = world.resource_mut::<DataRegistry>();
    data.insert("timed", status(json!({ "duration": 0.1 })));
    data.insert(
        "worn",
        status(json!({ "intensity": { "max": 1, "decay": 10 } })),
    );
    data.insert(
        "fuelled",
        status(json!({ "fuel": { "stat": "oil", "rate": 100 } })),
    );
    data.insert("removed", status(json!({})));
    data.insert("ended", status(json!({})));
    rule(
        &mut world,
        "strips",
        json!({ "stimulus": "elemental.cold", "outputs": [{ "remove": "removed" }] }),
    );
    let holder = world.spawn_with((Health::new(1000.0), Stats::new().with("oil", 1.0)));
    for id in ["timed", "worn", "fuelled", "removed", "ended"] {
        EffectRequest::new(holder, DataRef::by_id(id)).apply(&mut world);
    }
    assert!(end_effect(&mut world, holder, &DataRef::by_id("ended")));
    hit(&mut world, holder, "elemental.cold", 1.0);
    for _ in 0..12 {
        step(&mut world, &mut schedule);
    }
    let mut seen = calls.0.borrow().clone();
    seen.sort();
    assert_eq!(
        seen,
        [
            "ended:ended#on_end",
            "ended:fuelled#on_end",
            "ended:removed#on_end",
            "ended:timed#on_end",
            "ended:worn#on_end"
        ],
        "once each"
    );
    // A band's `enter` runs each time the intensity reaches it from below.
    calls.0.borrow_mut().clear();
    let banded: Effect = serde_json::from_value(json!({
        "intensity": { "max": 5, "decay": 30 },
        "bands": [{ "at": 2, "grant": ["status.cover"], "enter": [{ "handler": "entered" }] }]
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("banded", banded);
    let build = |world: &mut World, amount: f32| {
        EffectRequest::new(holder, DataRef::by_id("banded"))
            .with_amount(amount)
            .apply(world);
    };
    build(&mut world, 3.0);
    build(&mut world, 1.0);
    step(&mut world, &mut schedule);
    assert_eq!(calls.0.borrow().len(), 1, "once on the way up");
    for _ in 0..6 {
        step(&mut world, &mut schedule);
    }
    build(&mut world, 3.0);
    step(&mut world, &mut schedule);
    assert_eq!(
        calls.0.borrow().as_slice(),
        ["entered:banded#enter.0", "entered:banded#enter.0"],
        "again after falling below"
    );
    // An instant effect lands and ends at once: a duration of 0, and one
    // a curve scales to 0, each run `on_end` once, credited to the
    // request's source at its depth.
    let noted = json!([{ "handler": "noted" }]);
    let data = world.resource_mut::<DataRegistry>();
    data.insert("zero", Curve::new([(0.0, 0.0), (1.0, 0.0)]));
    data.insert(
        "instant",
        serde_json::from_value::<Effect>(json!({ "duration": 0, "on_end": noted })).unwrap(),
    );
    data.insert(
        "scaled_out",
        serde_json::from_value::<Effect>(json!({
            "duration": 1, "scale": { "curve": "zero", "into": ["duration"] }, "on_end": noted
        }))
        .unwrap(),
    );
    let source = world.spawn();
    for id in ["instant", "scaled_out"] {
        let applied = EffectRequest::new(holder, DataRef::by_id(id))
            .with_source(source)
            .at_depth(3)
            .apply(&mut world);
        assert_eq!(applied, crate::Applied::Applied, "{id}");
        assert!(!has(&world, holder, id), "{id} ended as it landed");
    }
    for _ in 0..3 {
        step(&mut world, &mut schedule);
    }
    assert_eq!(
        notes.borrow().as_slice(),
        [
            ("instant#on_end".to_string(), Some(source), 3),
            ("scaled_out#on_end".to_string(), Some(source), 3)
        ]
    );
}

#[test]
fn an_on_expire_cycle_ends_at_the_depth_limit() {
    let notes = Notes::default();
    let (mut world, mut schedule) =
        rig_with(&ChemistryPlugin::new().with_handler("noted", noting(&notes)));
    // Instant `a` expires into instant `b`, whose `on_end` applies `a`.
    let a: Effect = serde_json::from_value(json!({ "duration": 0, "on_expire": "b" })).unwrap();
    let b: Effect = serde_json::from_value(json!({
        "duration": 0, "on_end": [{ "handler": "noted" }, { "apply": { "status": "a" } }]
    }))
    .unwrap();
    let data = world.resource_mut::<DataRegistry>();
    data.insert("a", a);
    data.insert("b", b);
    let holder = receiver(&mut world, &[]);
    EffectRequest::new(holder, DataRef::by_id("a")).apply(&mut world);
    let mut depths = Vec::new();
    for _ in 0..40 {
        step(&mut world, &mut schedule);
        depths.push(notes.borrow().len());
    }
    let seen: Vec<u8> = notes.borrow().iter().map(|(_, _, depth)| *depth).collect();
    assert_eq!(
        seen,
        (0..=8).collect::<Vec<u8>>(),
        "a link a run, from 0 to 8"
    );
    // A run each: the count grows by one a step until the limit.
    assert!(
        depths.windows(2).all(|pair| pair[1] - pair[0] <= 1),
        "{depths:?}"
    );
    assert_eq!(chemistry(&world).cut(), 1, "the link past 8 cut");
    assert_eq!(chemistry(&world).pending(), 0);
}

#[test]
fn a_reactions_damage_carries_its_depth() {
    let (mut world, mut schedule) = rig();
    rule(
        &mut world,
        "feeds",
        json!({ "stimulus": "elemental.fire", "outputs": [{ "damage": { "elemental.fire": 1 } }] }),
    );
    let crate_ = receiver(&mut world, &[]);
    hit(&mut world, crate_, "elemental.fire", 1.0);
    for _ in 0..30 {
        step(&mut world, &mut schedule);
    }
    let health = world.get::<Health>(crate_).unwrap().current();
    assert_eq!(
        health,
        1000.0 - 10.0,
        "the hit and nine answers, depths 1 to 9"
    );
    assert_eq!(chemistry(&world).cut(), 1, "the tenth past the limit");
    assert_eq!(chemistry(&world).pending(), 0);
    assert_eq!(std::mem::size_of::<DamageRequest>(), 152);
}

#[test]
fn handlers_run_by_name_and_an_unknown_one_fails_the_load() {
    let calls = Rc::new(Calls::default());
    let (mut world, mut schedule) =
        rig_with(&ChemistryPlugin::new().with_handler("douse", counting(&calls, "douse")));
    rule(
        &mut world,
        "wet",
        json!({ "stimulus": "elemental.cold", "outputs": [{ "handler": "douse" }] }),
    );
    let crate_ = receiver(&mut world, &[]);
    hit(&mut world, crate_, "elemental.cold", 1.0);
    step(&mut world, &mut schedule);
    assert_eq!(calls.0.borrow().as_slice(), ["douse:wet"]);
    let known: Interaction = serde_json::from_value(
        json!({ "stimulus": "heat", "outputs": [{ "handler": "douse" }, { "spawn": "nowhere" }] }),
    )
    .unwrap();
    let err = format!("{:?}", known.check(&world).unwrap_err());
    assert!(err.contains("nowhere"), "{err}");
    let unknown: Interaction = serde_json::from_value(
        json!({ "stimulus": "heat", "outputs": [{ "handler": "missing" }] }),
    )
    .unwrap();
    let err = format!("{:?}", unknown.check(&world).unwrap_err());
    assert!(err.contains("missing"), "{err}");
    let undeclared: Interaction =
        serde_json::from_value(json!({ "stimulus": "smoke", "outputs": [] })).unwrap();
    assert!(
        undeclared.check(&world).is_err(),
        "a stimulus no tag or type declares"
    );
}

#[test]
fn outputs_read_their_data_and_refuse_bad_values() {
    for good in [
        json!({ "build": { "status": "burning", "amount": 1.0 } }),
        json!({ "build": { "status": "burning", "scale": 0.5 } }),
        json!({ "apply": { "status": "burning", "dot": 0.25, "lines": [{ "stat": "armor", "modifier": { "add": 1.0 } }] } }),
        json!({ "remove": "burning" }),
        json!({ "consume": { "status": "burning", "amount": 1.0 } }),
        json!({ "damage": 2.0 }),
        json!({ "damage": { "elemental.fire": 2.0 } }),
        json!({ "spawn": "puff" }),
        json!({ "handler": "douse" }),
    ] {
        let output: Output = serde_json::from_value(good.clone()).unwrap();
        assert_eq!(serde_json::to_value(&output).unwrap(), good, "{good}");
    }
    for bad in [
        json!({ "build": { "status": "burning", "amount": 1, "scale": 1 } }),
        json!({ "build": { "status": "burning", "amount": -1 } }),
        json!({ "apply": { "status": "burning", "dot": -1 } }),
        json!({ "consume": { "status": "burning", "amount": 0 } }),
        json!({ "remove": "burning", "spawn": "puff" }),
        json!({}),
        json!({ "explode": 1 }),
    ] {
        assert!(
            serde_json::from_value::<Output>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    for bad in [
        json!({ "stimulus": "Heat", "outputs": [] }),
        json!({ "stimulus": "heat", "min": -1, "outputs": [] }),
        json!({ "stimulus": "heat" }),
        json!({ "stimulus": "heat", "outputs": [], "extra": 1 }),
    ] {
        assert!(
            serde_json::from_value::<Interaction>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_queue_round_trips_a_save() {
    let wood = json!({ "stimulus": "elemental.fire", "outputs": [{ "build": { "status": "burning", "amount": 1 } }] });
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_cap(1));
    rule(&mut world, "wood", wood.clone());
    let named = |world: &mut World, name: &str| {
        let entity = receiver(world, &[]);
        tungsten_core::save::name(world, entity, None, name).unwrap();
        entity
    };
    let [first, kept, lost, other, torch, ash] =
        ["first", "kept", "lost", "other", "torch", "ash"].map(|name| named(&mut world, name));
    // `first` at depth 0 runs at once; the rest wait behind the cap.
    for (target, source, depth) in [
        (first, torch, 0),
        (kept, ash, 1),
        (lost, torch, 1),
        (other, torch, 1),
    ] {
        DamageRequest::from_parts(
            target,
            [DamagePart::new(2.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_source(source)
        .with_depth(depth)
        .apply(&mut world);
    }
    step(&mut world, &mut schedule);
    assert!(has(&world, first, "burning"));
    assert_eq!(chemistry(&world).pending(), 3);
    let saved = capture_world(&world).unwrap();

    // A World without `lost` and `ash`, built at a cap of 1, a bound of 4
    // and a depth limit of 2: `lost`'s entry goes, and `kept`'s credit with
    // `ash`; the load keeps the World's settings, not the saved World's.
    let tight = || {
        ChemistryPlugin::new()
            .with_cap(1)
            .with_bound(4)
            .with_depth(2)
    };
    let (mut loaded, mut loaded_schedule) = rig_with(&tight());
    rule(&mut loaded, "wood", wood.clone());
    let _spare = loaded.spawn();
    let [kept_b, other_b, torch_b, _first_b] =
        ["kept", "other", "torch", "first"].map(|name| named(&mut loaded, name));
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    assert_eq!(chemistry(&loaded).pending(), 2, "the gone receiver dropped");
    assert_eq!(chemistry(&loaded).ran(), 1);
    // Three fresh hits and one three reactions deep: the third fresh add
    // passes the bound of 4, the deep one the depth limit of 2.
    let fresh = [0, 1, 2].map(|_| receiver(&mut loaded, &[]));
    for &target in &fresh {
        hit(&mut loaded, target, "elemental.fire", 1.0);
    }
    let deep = receiver(&mut loaded, &[]);
    DamageRequest::from_parts(
        deep,
        [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
    )
    .with_depth(3)
    .apply(&mut loaded);
    step(&mut loaded, &mut loaded_schedule);
    let counts = |world: &World| {
        let chemistry = chemistry(world);
        (
            chemistry.pending(),
            chemistry.dropped(),
            chemistry.cut(),
            chemistry.ran(),
        )
    };
    assert_eq!(
        counts(&loaded),
        (3, 1, 1, 2),
        "one entry a run, one dropped, one cut"
    );
    step(&mut loaded, &mut loaded_schedule);
    assert_eq!(counts(&loaded).3, 3, "one entry a run");
    let source = |world: &World, entity: Entity| {
        world
            .get::<Effects>(entity)
            .unwrap()
            .instances()
            .next()
            .unwrap()
            .source()
    };
    assert_eq!(
        source(&loaded, kept_b),
        None,
        "a gone credit credits no one"
    );
    assert_eq!(
        source(&loaded, other_b),
        Some(torch_b),
        "credited by save ID"
    );
    for _ in 0..4 {
        step(&mut loaded, &mut loaded_schedule);
    }
    assert!(has(&loaded, fresh[0], "burning") && has(&loaded, fresh[1], "burning"));
    assert!(!has(&loaded, fresh[2], "burning") && !has(&loaded, deep, "burning"));
    let _ = (kept, lost, other, ash);

    // Six saved entries, one three deep, restored into the tight World:
    // its first run cuts the deep one and drops the youngest past the
    // bound, each counted, before it takes any.
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_cap(1));
    rule(&mut world, "wood", wood.clone());
    let names = ["r0", "r1", "r2", "r3", "r4", "r5", "r6"];
    let receivers = names.map(|name| named(&mut world, name));
    for (i, &target) in receivers.iter().enumerate() {
        let depth = match i {
            0 => 0,
            3 => 3,
            _ => 1,
        };
        DamageRequest::from_parts(
            target,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_depth(depth)
        .apply(&mut world);
    }
    step(&mut world, &mut schedule);
    assert!(has(&world, receivers[0], "burning"), "the shallowest ran");
    assert_eq!(counts(&world), (6, 0, 0, 1));
    let saved = capture_world(&world).unwrap();
    let (mut loaded, mut loaded_schedule) = rig_with(&tight());
    rule(&mut loaded, "wood", wood);
    let restored = names.map(|name| named(&mut loaded, name));
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    assert_eq!(counts(&loaded), (6, 0, 0, 1), "checked at the first run");
    step(&mut loaded, &mut loaded_schedule);
    assert_eq!(counts(&loaded), (3, 1, 1, 2), "cut, dropped, then one run");
    for _ in 0..4 {
        step(&mut loaded, &mut loaded_schedule);
    }
    // The youngest is the last in the take order: by save ID here.
    let mut waiting = vec![1, 2, 4, 5, 6];
    waiting.sort_by_key(|&i| tungsten_core::save::order_key(&loaded, restored[i]));
    let youngest = waiting.pop().unwrap();
    waiting.sort_unstable();
    let burnt: Vec<usize> = (1..7)
        .filter(|&i| has(&loaded, restored[i], "burning"))
        .collect();
    assert_eq!(
        burnt, waiting,
        "the deep r3 cut, the youngest r{youngest} dropped"
    );
}
