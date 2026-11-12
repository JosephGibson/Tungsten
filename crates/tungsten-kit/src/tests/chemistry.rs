use std::cell::RefCell;
use std::rc::Rc;

use glam::{Vec2, Vec3};
use serde_json::{Value, json};
use tungsten_core::physics::{
    Collider, PhysicsConfig, PhysicsPlugin, Position, RigidBodyBundle, Velocity,
};
use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::{
    CommandBuffer, DataRef, DataRegistry, LayerKind, Light, Schedule, Stage, TileChanges,
    TileIndex, TileProps, TilemapData, TilemapInstance, TilemapRegistry, Time, Transform,
};

use super::tiles::Cell;
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
const TAGS: [&str; 16] = [
    "material.wood",
    "material.stone",
    "material.glass",
    "flammable",
    "status.burning",
    "status.armed",
    "status.cover",
    "heat",
    "team.enemy",
    "ping",
    "pong",
    "node",
    "creature",
    "wet",
    "light.lamp",
    "light.doused",
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

/// [`rig_with`] with physics first, without gravity.
fn physics_rig(plugin: &ChemistryPlugin) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(TAGS)
        .build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    plugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// The frame rates the timing tests step at.
const DTS: [f32; 3] = [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0];

/// What a handler saw of each reaction: its receiver, stimulus, amount
/// and credit.
type Seen = Rc<RefCell<Vec<(Entity, String, f32, Option<Entity>)>>>;

fn seeing(seen: &Seen) -> impl Fn(&mut World, &Reaction) + 'static {
    let seen = Rc::clone(seen);
    move |_, reaction| {
        seen.borrow_mut().push((
            reaction.receiver().unwrap_or(Entity::PLACEHOLDER),
            reaction.stimulus().to_owned(),
            reaction.amount(),
            reaction.credit(),
        ));
    }
}

/// How many reactions `seen` holds on `entity`.
fn seen_on(seen: &Seen, entity: Entity) -> usize {
    seen.borrow()
        .iter()
        .filter(|(receiver, ..)| *receiver == entity)
        .count()
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
        Receiver::Entity(held),
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

// Step 2: spread, impacts and light as stimuli.

/// A crate burning with `heat` within 40 px every 0.5 s (lit when `lit`),
/// and two boxes whose surfaces lie 39.9 and 40.1 px out, a handler
/// seeing each `heat` on wood.
fn emitting(seen: &Seen, lit: bool) -> (World, Schedule, [Entity; 3]) {
    let (mut world, schedule) =
        rig_with(&ChemistryPlugin::new().with_handler("warmed", seeing(seen)));
    world.resource_mut::<DataRegistry>().insert(
        "smouldering",
        Effect::new()
            .with_intensity(10.0, 0.0)
            .with_emit("heat", 1.0, 40.0, 0.5),
    );
    rule(
        &mut world,
        "warms",
        json!({ "stimulus": "heat", "on": { "all": ["material.wood"] }, "outputs": [{ "handler": "warmed" }] }),
    );
    let crate_ = world.spawn_with((
        Health::new(1000.0),
        Tags::new(["material.wood"]),
        Transform::from_position(Vec2::ZERO),
    ));
    let boxed = |world: &mut World, x: f32| {
        world.spawn_with((
            Health::new(1000.0),
            Tags::new(["material.wood"]),
            Position(Vec2::new(x, 0.0)),
            Collider::aabb(Vec2::splat(10.0)),
        ))
    };
    let near = boxed(&mut world, 49.9);
    let far = boxed(&mut world, 50.1);
    if lit {
        EffectRequest::new(crate_, DataRef::by_id("smouldering"))
            .with_amount(5.0)
            .apply(&mut world);
    }
    (world, schedule, [crate_, near, far])
}

#[test]
fn a_status_emits_on_its_period_within_its_radius() {
    // Over 3 s, at each frame rate: an emission every 0.5 s, the near box
    // reached, the far box and the emitter not.
    for dt in DTS {
        let seen = Seen::default();
        let (mut world, mut schedule, [crate_, near, far]) = emitting(&seen, true);
        for _ in 0..(3.0 / dt).round() as usize {
            step_at(&mut world, &mut schedule, dt);
        }
        assert_eq!(
            (
                seen_on(&seen, crate_),
                seen_on(&seen, near),
                seen_on(&seen, far)
            ),
            (0, 6, 0),
            "dt {dt}"
        );
        assert!(
            seen.borrow()
                .iter()
                .all(|(_, name, amount, _)| name == "heat" && *amount == 1.0)
        );
    }
    // The clock is kept through a save: the next emission lands on the
    // uninterrupted run's step.
    let seen = Seen::default();
    let (mut world, mut schedule, entities) = emitting(&seen, true);
    let names = ["crate", "near", "far"];
    for (entity, name) in entities.into_iter().zip(names) {
        tungsten_core::save::name(&mut world, entity, None, name).unwrap();
    }
    for _ in 0..20 {
        step(&mut world, &mut schedule);
    }
    let saved = capture_world(&world).unwrap();
    let loaded_seen = Seen::default();
    let (mut loaded, mut loaded_schedule, fresh) = emitting(&loaded_seen, false);
    for (entity, name) in fresh.into_iter().zip(names) {
        tungsten_core::save::name(&mut loaded, entity, None, name).unwrap();
    }
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    let left = loaded
        .get::<Effects>(fresh[0])
        .unwrap()
        .instances()
        .next()
        .unwrap()
        .emit_left();
    assert!(
        left.is_some_and(|left| (left - (0.5 - 20.0 * STEP)).abs() < 1e-4),
        "{left:?}"
    );
    let mut landed = (None, None);
    for frame in 1..=12 {
        step(&mut world, &mut schedule);
        step(&mut loaded, &mut loaded_schedule);
        if landed.0.is_none() && seen_on(&seen, entities[1]) == 1 {
            landed.0 = Some(frame);
        }
        if landed.1.is_none() && seen_on(&loaded_seen, fresh[1]) == 1 {
            landed.1 = Some(frame);
        }
    }
    assert_eq!(landed, (Some(10), Some(10)), "at 0.5 s both ways");
    // An instance without `emit` saves the bytes it saved before
    // emissions.
    let (mut world, _) = rig();
    world
        .resource_mut::<DataRegistry>()
        .insert("ticking", Effect::new().with_duration(2.0).with_period(0.5));
    let holder = world.spawn();
    EffectRequest::new(holder, DataRef::by_id("ticking")).apply(&mut world);
    let instance = world
        .get::<Effects>(holder)
        .unwrap()
        .instances()
        .next()
        .unwrap()
        .clone();
    assert_eq!(
        serde_json::to_string(&instance).unwrap(),
        r#"{"effect":"ticking","stacks":1,"left":2.0,"period_left":0.5,"factor":1.0,"serial":0,"granted":[]}"#
    );
}

#[test]
fn a_two_rule_cycle_ends() {
    let (mut world, mut schedule) = rig();
    for (id, from, to) in [("pings", "ping", "pong"), ("pongs", "pong", "ping")] {
        rule(
            &mut world,
            id,
            json!({
                "stimulus": from, "on": { "all": ["node"] },
                "outputs": [{ "emit": { "stimulus": to, "amount": 1, "radius": 100 } }]
            }),
        );
    }
    let node = |world: &mut World, x: f32| {
        world.spawn_with((
            Tags::new(["node"]),
            Transform::from_position(Vec2::new(x, 0.0)),
        ))
    };
    let a = node(&mut world, 0.0);
    let _b = node(&mut world, 50.0);
    let stimulus = queue::Stimulus {
        amount: 1.0,
        status_effect: 1.0,
        dot_base: 1.0,
        depth: 0,
        credit: None,
    };
    world.resource_mut::<Chemistry>().stimulate(
        &Settings::default(),
        Receiver::Entity(a),
        "ping",
        stimulus,
        0,
    );
    for _ in 0..10 {
        step(&mut world, &mut schedule);
    }
    let chemistry = chemistry(&world);
    assert_eq!(chemistry.pending(), 0, "no entry left by the tenth run");
    assert!(chemistry.emitted.is_empty());
    assert!(chemistry.cut() >= 1, "{}", chemistry.cut());
    assert_eq!(chemistry.ran(), 9, "depths 0 to 8, one a run");
}

/// 2.6's field: `count` bodiless receivers tagged `flammable` on a square
/// grid 24 px apart, `heat` on them building `burning` (max 4, decay 0.5,
/// emitting `heat` 1 within 50 px every 0.25 s) by 1.
fn field(world: &mut World, count: usize) -> Vec<Entity> {
    world.resource_mut::<DataRegistry>().insert(
        "burning",
        Effect::new()
            .with_intensity(4.0, 0.5)
            .with_grant(["status.burning"])
            .with_emit("heat", 1.0, 50.0, 0.25),
    );
    rule(
        world,
        "catches",
        json!({ "stimulus": "heat", "on": { "all": ["flammable"] }, "outputs": [{ "build": { "status": "burning", "amount": 1 } }] }),
    );
    let columns = (count as f32).sqrt().ceil() as usize;
    (0..count)
        .map(|i| {
            let at = Vec2::new((i % columns) as f32 * 24.0, (i / columns) as f32 * 24.0);
            world.spawn_with((Tags::new(["flammable"]), Transform::from_position(at)))
        })
        .collect()
}

#[test]
fn a_spread_flood_stays_under_its_bound() {
    let started = std::time::Instant::now();
    let (mut world, mut schedule) = rig();
    let receivers = field(&mut world, 12_000);
    // One receiver lit, mid-field.
    let columns = (12_000_f32).sqrt().ceil() as usize;
    let centre = receivers[(12_000 / columns / 2) * columns + columns / 2];
    EffectRequest::new(centre, DataRef::by_id("burning")).apply(&mut world);
    let mut burnt = vec![false; receivers.len()];
    let mut left = receivers.len();
    let mut runs = 0;
    while runs < 3_000 && left > 0 {
        step(&mut world, &mut schedule);
        runs += 1;
        let chemistry = chemistry(&world);
        assert!(
            chemistry.pending() <= receivers.len() && chemistry.pending() < queue::BOUND,
            "run {runs}: {} pending",
            chemistry.pending()
        );
        assert_eq!(chemistry.dropped(), 0, "run {runs}");
        if runs % 30 == 0 {
            for (i, &entity) in receivers.iter().enumerate() {
                if !burnt[i] && has(&world, entity, "burning") {
                    burnt[i] = true;
                    left -= 1;
                }
            }
        }
    }
    eprintln!(
        "a_spread_flood_stays_under_its_bound: every receiver burnt by run {runs} ({} left), {} ran, {:.1} s",
        left,
        chemistry(&world).ran(),
        started.elapsed().as_secs_f32()
    );
    assert_eq!(left, 0, "unburnt after {runs} runs");
}

/// What three passes of [`stimuli::spread`] leave in a fresh queue over
/// `receivers`, a take after each and the stamps begun again before the
/// third: the entries after each pass and each take, the queue's counts
/// and the probes the passes skipped. `probe_all` probes the index for every candidate,
/// as the pass did before it kept slots.
fn spread_passes(
    world: &mut World,
    receivers: &[Entity],
    probe_all: bool,
) -> (Vec<Vec<queue::Entry>>, [u64; 3], u64) {
    let mut queue = Chemistry::default();
    let [heat, ping, nothing] = ["heat", "ping", "nothing"].map(|name| queue.intern(name));
    // One receiver takes no entry.
    queue.hold(receivers[9]);
    world.insert_resource(queue);
    // A bound the field passes and a depth one emission passes.
    let settings = Settings {
        bound: 40,
        depth: 3,
        ..Settings::default()
    };
    let emission =
        |stimulus, at: usize, amount, radius, depth, credit: Option<usize>| stimuli::Emission {
            stimulus,
            amount,
            radius,
            point: Vec2::new((at % 6) as f32 * 24.0, (at / 6) as f32 * 24.0),
            from: Some(Receiver::Entity(receivers[at])),
            credit: credit.map(|at| receivers[at]),
            depth,
        };
    // Each stimulus between runs of the other, one past the depth and one
    // no rule answers, over the same receivers.
    let emissions = vec![
        emission(heat, 7, 1.0, 50.0, 0, Some(7)),
        emission(heat, 8, 2.5, 50.0, 1, Some(8)),
        emission(ping, 7, 3.0, 30.0, 0, None),
        emission(nothing, 7, 9.0, 80.0, 0, None),
        emission(heat, 14, 0.5, 80.0, 0, Some(14)),
        emission(heat, 14, 4.0, 80.0, 4, Some(2)),
        emission(ping, 15, 1.5, 80.0, 2, Some(15)),
        emission(heat, 21, 1.0, 60.0, 3, None),
        emission(ping, 20, 1.0, 200.0, 0, None),
        emission(heat, 28, 2.0, 200.0, 0, Some(28)),
    ];
    let mut reach = stimuli::Reach::default();
    reach.probe_all = probe_all;
    let mut entries = Vec::new();
    let mut taken = Vec::new();
    for pass in 0..3_u64 {
        reach.clear();
        // The second pass reaches a few receivers alone, so the rest keep
        // the first's stamp into the third, which begins the stamps again.
        let count = if pass == 1 { 3 } else { emissions.len() };
        let mut emitted = emissions[..count].to_vec();
        stimuli::spread(
            world,
            &settings,
            &mut emitted,
            &mut reach,
            |name| name != "nothing",
            pass,
        );
        entries.push(chemistry(world).entries());
        // A run's take frees slots the next pass fills with other pairs.
        let mut queue = world.remove_resource::<Chemistry>().unwrap();
        queue.take(world, 11, pass, &mut taken);
        world.insert_resource(queue);
        entries.push(chemistry(world).entries());
        if pass == 1 {
            reach.end_stamps();
        }
    }
    let queue = chemistry(world);
    let counts = [queue.pending() as u64, queue.dropped(), queue.cut()];
    (entries, counts, reach.skipped)
}

#[test]
fn a_pass_that_keeps_its_slots_queues_what_a_probe_a_candidate_does() {
    let (mut world, _) = rig();
    let receivers: Vec<Entity> = (0..36)
        .map(|i| {
            let at = Vec2::new((i % 6) as f32 * 24.0, (i / 6) as f32 * 24.0);
            world.spawn_with((Tags::new(["flammable"]), Transform::from_position(at)))
        })
        .collect();
    let (kept, kept_counts, skipped) = spread_passes(&mut world, &receivers, false);
    let (probed, probed_counts, none) = spread_passes(&mut world, &receivers, true);
    assert_eq!(none, 0, "the reference probes every candidate");
    assert!(skipped > 100, "{skipped} probes skipped");
    for (at, (kept, probed)) in kept.iter().zip(&probed).enumerate() {
        assert_eq!(kept, probed, "the entries at {at}");
    }
    assert_eq!(kept_counts, probed_counts, "pending, dropped and cut");
    let [pending, dropped, cut] = kept_counts;
    assert!(
        pending > 0 && dropped > 0 && cut > 0,
        "{pending} pending, {dropped} dropped, {cut} cut"
    );
    // The held receiver took no entry, kept slot or not.
    assert!(
        kept.iter()
            .flatten()
            .all(|entry| entry.receiver != Receiver::Entity(receivers[9]))
    );
}

#[test]
fn impacts_stimulate_by_closing_speed() {
    let seen = Seen::default();
    let plugin = ChemistryPlugin::new().with_handler("struck", seeing(&seen));
    let pair = |world: &mut World, y: f32, gap: f32, speed: f32| {
        let ball = world.spawn_with(RigidBodyBundle::dynamic(
            Position::new(0.0, y),
            Collider::circle(5.0),
        ));
        world.get_mut::<Velocity>(ball).unwrap().0 = Vec2::new(speed, 0.0);
        world.insert(ball, Tags::new(["material.glass"]));
        let block = world.spawn_with(RigidBodyBundle::r#static(
            Position::new(15.0 + gap, y),
            Collider::aabb(Vec2::splat(10.0)),
        ));
        world.insert(block, Tags::new(["material.glass"]));
        (ball, block)
    };
    let (mut world, mut schedule) = physics_rig(&plugin);
    rule(
        &mut world,
        "cracks",
        json!({ "stimulus": "stimulus.impact", "on": { "all": ["material.glass"] }, "min": 300, "outputs": [{ "handler": "struck" }] }),
    );
    // Touching in the world's first step, at 600 px/s.
    let (ball, block) = pair(&mut world, 0.0, 0.0, 600.0);
    let thrower = world.spawn();
    world.insert(ball, crate::Source::new(thrower));
    // A slow pair, far below.
    let (slow, slow_block) = pair(&mut world, 400.0, 20.0, 200.0);
    let mut speeds = Vec::new();
    for _ in 0..30 {
        step(&mut world, &mut schedule);
        speeds.extend(
            world
                .resource::<tungsten_core::EventQueue<ContactBegan>>()
                .iter()
                .filter(|begin| begin.a == ball || begin.b == Some(ball))
                .map(|begin| begin.speed),
        );
    }
    let first = *speeds.first().expect("the first step's begin");
    assert!((first - 600.0).abs() <= 6.0, "{first}");
    let on = |entity: Entity| -> Vec<(f32, Option<Entity>)> {
        seen.borrow()
            .iter()
            .filter(|(receiver, ..)| *receiver == entity)
            .map(|&(_, _, amount, credit)| (amount, credit))
            .collect()
    };
    assert_eq!(
        on(ball),
        [(first, Some(block))],
        "the ball, credited to the block"
    );
    assert_eq!(
        on(block),
        [(first, Some(thrower))],
        "the block, credited to the thrower"
    );
    assert!(
        on(slow).is_empty() && on(slow_block).is_empty(),
        "under the rule's min"
    );
    // A rule naming `stimulus` alone sees impacts and the light both.
    let any = Seen::default();
    let (mut world, mut schedule) =
        physics_rig(&ChemistryPlugin::new().with_handler("any", seeing(&any)));
    rule(
        &mut world,
        "everything",
        json!({ "stimulus": "stimulus", "outputs": [{ "handler": "any" }] }),
    );
    pair(&mut world, 0.0, 0.0, 600.0);
    for _ in 0..5 {
        step(&mut world, &mut schedule);
    }
    let names: std::collections::BTreeSet<String> = any
        .borrow()
        .iter()
        .map(|(_, name, ..)| name.clone())
        .collect();
    assert!(
        names.contains("stimulus.impact") && names.contains("stimulus.light"),
        "{names:?}"
    );
    // Rules that name no impact never turn the record on.
    let (mut world, mut schedule) = physics_rig(&ChemistryPlugin::new());
    rule(
        &mut world,
        "warms",
        json!({ "stimulus": "heat", "outputs": [{ "remove": "mark" }] }),
    );
    pair(&mut world, 0.0, 0.0, 600.0);
    for _ in 0..60 {
        step(&mut world, &mut schedule);
    }
    assert!(!world.has_event::<ContactBegan>());
}

#[test]
fn light_stimulates_on_its_period() {
    // Over 2 1/6 s, an integer of steps at each rate and clear of a
    // sample: at once, then every 0.25 s, 9 samples.
    let mut counts = Vec::new();
    for dt in DTS {
        let seen = Seen::default();
        let (mut world, mut schedule) =
            rig_with(&ChemistryPlugin::new().with_handler("lit", seeing(&seen)));
        rule(
            &mut world,
            "lights",
            json!({ "stimulus": "stimulus.light", "on": { "all": ["creature"] }, "outputs": [{ "handler": "lit" }] }),
        );
        world.spawn_with((
            Transform::from_position(Vec2::ZERO),
            Light::point(Vec3::ONE, 100.0),
        ));
        let lurker = world.spawn_with((
            Tags::new(["creature"]),
            Transform::from_position(Vec2::new(50.0, 0.0)),
        ));
        let stone = world.spawn_with((
            Tags::new(["material.stone"]),
            Transform::from_position(Vec2::new(10.0, 0.0)),
        ));
        for _ in 0..((2.0 + 1.0 / 6.0) / dt).round() as usize {
            step_at(&mut world, &mut schedule, dt);
        }
        let level = tungsten_core::lighting::light_level(&world, Vec2::new(50.0, 0.0));
        assert!(level > 0.0);
        assert!(
            seen.borrow()
                .iter()
                .all(|&(receiver, ref name, amount, credit)| {
                    receiver == lurker
                        && name == "stimulus.light"
                        && amount == level
                        && credit.is_none()
                }),
            "dt {dt}"
        );
        assert!(
            skips(&world, stone).is_empty(),
            "dt {dt}: the stone never sampled"
        );
        assert_eq!(chemistry(&world).ran() as usize, seen.borrow().len());
        counts.push(seen.borrow().len());
    }
    assert_eq!(counts, [9, 9, 9]);
}

// Step 3: statuses on tiles.

/// The tiles a strip's tileset holds, in order: `grass` (material
/// `grass`), `ash` (none), `stone` (material `stone`, health 3, breaking to
/// `ash`), `ember` (none), a `turf` style (its brush and the variants
/// closed (0), open right (2) and open left (8)), and `glass` (material
/// `glass`, health 1, breaking to `ash`).
const STRIP_TILES: [&str; 9] = [
    "grass",
    "ash",
    "stone",
    "ember",
    "turf",
    "turf_mid",
    "turf_right",
    "turf_left",
    "glass",
];

/// A one-row map `id` of `len` 16 px cells of `fill`, on one layer of
/// `kind`, its tileset [`STRIP_TILES`] (reversed when `reversed`, so the
/// sprites' indices differ), registered in the World's registry.
fn strip_map(world: &mut World, id: &str, len: u32, fill: &str, kind: LayerKind, reversed: bool) {
    if !world.has_resource::<TilemapRegistry>() {
        world.insert_resource(TilemapRegistry::new());
    }
    let mut map = TilemapData::new(16, 16, len, 1);
    let mut names: Vec<&str> = STRIP_TILES.to_vec();
    if reversed {
        names.reverse();
    }
    for name in &names {
        map.add_tile(*name);
    }
    let index = |name: &str| names.iter().position(|known| *known == name).unwrap() as TileIndex;
    let mut props = |name: &str, edit: &dyn Fn(&mut TileProps)| {
        let mut tile = TileProps::default();
        edit(&mut tile);
        map.set_tile_props(index(name), tile);
    };
    props("grass", &|tile| tile.material = Some("grass".into()));
    props("stone", &|tile| {
        tile.material = Some("stone".into());
        tile.health = Some(3.0);
        tile.broken = index("ash");
    });
    props("glass", &|tile| {
        tile.material = Some("glass".into());
        tile.health = Some(1.0);
        tile.broken = index("ash");
    });
    props("turf", &|tile| {
        tile.material = Some("grass".into());
        tile.style = Some("turf".into());
    });
    for (name, mask) in [("turf_mid", 0), ("turf_right", 2), ("turf_left", 8)] {
        props(name, &|tile| {
            tile.material = Some("grass".into());
            tile.style = Some("turf".into());
            tile.mask = Some(mask);
        });
    }
    let layer = map.add_layer("ground", kind);
    map.fill(layer, index(fill));
    world
        .resource_mut::<TilemapRegistry>()
        .insert(id.to_owned(), map);
}

/// A placement of map `id` at `origin`.
fn place(world: &mut World, id: &str, origin: Vec2) -> Entity {
    world.spawn_with((TilemapInstance::new(id, origin),))
}

/// The cell at `col` of map `id`'s placement at `origin`.
fn cell(id: &str, origin: Vec2, col: u32) -> Cell {
    Cell::new(Rc::from(id), origin, 0, col, 0)
}

/// The sprite map `id` holds at `col`.
fn sprite_at(world: &World, id: &str, col: u32) -> String {
    let data = world.resource::<TilemapRegistry>().get(id).unwrap();
    let tile = data.tile_at(0, col, 0).unwrap();
    data.tileset[tile as usize].clone()
}

/// Whether `cell` carries the status `id`.
fn tile_has(world: &World, cell: &Cell, id: &str) -> bool {
    world
        .resource::<TileStatuses>()
        .on(cell)
        .iter()
        .any(|instance| instance.effect().id() == Some(id))
}

/// A burning status for tiles and crates: intensity 2 that holds,
/// granting `status.burning`, sending `heat` within 20 px every 0.25 s;
/// and the rule `heat` on `grass` building it by 1.
fn burning_tiles(world: &mut World) {
    world.resource_mut::<DataRegistry>().insert(
        "burning",
        Effect::new()
            .with_intensity(2.0, 0.0)
            .with_grant(["status.burning"])
            .with_emit("heat", 1.0, 20.0, 0.25),
    );
    rule(
        world,
        "catches",
        json!({ "stimulus": "heat", "on": { "all": ["grass"] }, "outputs": [{ "build": { "status": "burning", "amount": 1 } }] }),
    );
}

#[test]
fn heat_spreads_across_material_tiles() {
    let (mut world, mut schedule) = rig();
    burning_tiles(&mut world);
    strip_map(&mut world, "strip", 10, "grass", LayerKind::Render, false);
    place(&mut world, "strip", Vec2::ZERO);
    // A burning crate 16 px left of the first cell's centre.
    let crate_ = world.spawn_with((
        Tags::new(["material.wood"]),
        Transform::from_position(Vec2::new(-8.0, 8.0)),
    ));
    EffectRequest::new(crate_, DataRef::by_id("burning")).apply(&mut world);
    let cells: Vec<Cell> = (0..10).map(|col| cell("strip", Vec2::ZERO, col)).collect();
    let mut lit_at = [None; 10];
    let mut saved = None;
    for frame in 1..=200 {
        step(&mut world, &mut schedule);
        for (i, cell) in cells.iter().enumerate() {
            if lit_at[i].is_none() && tile_has(&world, cell, "burning") {
                lit_at[i] = Some(frame);
            }
        }
        if frame == 75 {
            saved = Some(capture_world(&world).unwrap());
        }
    }
    let lit: Vec<u32> = lit_at
        .iter()
        .map(|at| at.expect("every cell burns"))
        .collect();
    assert!(
        lit.windows(2).all(|pair| pair[0] < pair[1]),
        "in turn: {lit:?}"
    );
    // Restored into a fresh World with the map placed: the cells burning
    // at the capture burn there, and the rest catch in turn.
    let (mut loaded, mut loaded_schedule) = rig();
    burning_tiles(&mut loaded);
    strip_map(&mut loaded, "strip", 10, "grass", LayerKind::Render, false);
    place(&mut loaded, "strip", Vec2::ZERO);
    restore_world(&mut loaded, &saved.unwrap(), &(Restored,)).unwrap();
    let burning_then: Vec<bool> = cells
        .iter()
        .map(|cell| tile_has(&loaded, cell, "burning"))
        .collect();
    let expected: Vec<bool> = lit.iter().map(|&at| at <= 75).collect();
    assert_eq!(burning_then, expected);
    assert!(burning_then.contains(&false), "some still to catch");
    for _ in 0..125 {
        step(&mut loaded, &mut loaded_schedule);
    }
    assert!(cells.iter().all(|cell| tile_has(&loaded, cell, "burning")));
}

#[test]
fn set_tile_writes_a_cell_and_reverts() {
    // A cell's `on_end` sets `ash` for 5 s.
    let scorched = |world: &mut World| {
        world.resource_mut::<DataRegistry>().insert(
            "scorched",
            serde_json::from_value::<Effect>(json!({
                "duration": 0.1, "on_end": [{ "set_tile": { "tile": "ash", "revert": 5 } }]
            }))
            .unwrap(),
        );
    };
    let (mut world, mut schedule) = rig();
    scorched(&mut world);
    strip_map(&mut world, "strip", 3, "grass", LayerKind::Render, false);
    place(&mut world, "strip", Vec2::ZERO);
    let target = cell("strip", Vec2::ZERO, 1);
    // Each journal entry read from the cursor before it: the map after
    // both is the map before, which one cursor reads as unchanged.
    fn journaled(world: &World, cursor: &tungsten_core::MapCursor) -> usize {
        match world
            .resource::<TilemapRegistry>()
            .changes_since("strip", cursor)
        {
            Some(TileChanges::Cells(cells)) => cells.count(),
            other => panic!("{other:?}"),
        }
    }
    let mut cursor = world.resource::<TilemapRegistry>().cursor("strip").unwrap();
    assert!(tiles::apply(
        &mut world,
        &target,
        &DataRef::by_id("scorched"),
        1.0,
        1.0,
        None,
        0
    ));
    let (mut written, mut entries) = (None, 0);
    for frame in 1..=400 {
        step(&mut world, &mut schedule);
        let now = sprite_at(&world, "strip", 1);
        match (written, now.as_str()) {
            (None, "ash") => {
                written = Some(frame);
                entries += journaled(&world, &cursor);
                cursor = world.resource::<TilemapRegistry>().cursor("strip").unwrap();
            }
            (Some(at), "grass") => {
                assert_eq!(frame - at, 300, "5 s later, within a step");
                entries += journaled(&world, &cursor);
                break;
            }
            _ => {}
        }
    }
    assert_eq!(sprite_at(&world, "strip", 1), "grass");
    assert_eq!(entries, 2, "the write and the revert");

    // Through a save between, into a map whose tileset is in another
    // order: the old tile by sprite ID.
    let (mut world, mut schedule) = rig();
    strip_map(&mut world, "strip", 3, "grass", LayerKind::Render, false);
    place(&mut world, "strip", Vec2::ZERO);
    tiles::set_tile(&mut world, &target, "ash", Some(5.0)).unwrap();
    for _ in 0..60 {
        step(&mut world, &mut schedule);
    }
    let saved = capture_world(&world).unwrap();
    let (mut loaded, mut loaded_schedule) = rig();
    strip_map(&mut loaded, "strip", 3, "grass", LayerKind::Render, true);
    place(&mut loaded, "strip", Vec2::ZERO);
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    // The area's restore writes the edited cell back by sprite ID.
    let ash = STRIP_TILES.len() as TileIndex - 2;
    loaded
        .resource_mut::<TilemapRegistry>()
        .set_tile("strip", 0, 1, 0, ash);
    assert_eq!(sprite_at(&loaded, "strip", 1), "ash");
    for _ in 0..239 {
        step(&mut loaded, &mut loaded_schedule);
    }
    assert_eq!(sprite_at(&loaded, "strip", 1), "ash", "4.98 s");
    step(&mut loaded, &mut loaded_schedule);
    assert_eq!(sprite_at(&loaded, "strip", 1), "grass", "5 s");

    // An auto cell of a styled strip reverts to an auto cell, so a
    // neighbour's later edit re-picks it, with and without a save between.
    for through_save in [false, true] {
        let (mut world, mut schedule) = rig();
        strip_map(&mut world, "turf", 5, "turf", LayerKind::Render, false);
        place(&mut world, "turf", Vec2::ZERO);
        assert_eq!(sprite_at(&world, "turf", 2), "turf_mid");
        let middle = cell("turf", Vec2::ZERO, 2);
        tiles::set_tile(&mut world, &middle, "ash", Some(1.0)).unwrap();
        assert_eq!(
            (sprite_at(&world, "turf", 1), sprite_at(&world, "turf", 3)),
            ("turf_right".to_string(), "turf_left".to_string()),
            "its neighbours open toward it"
        );
        if through_save {
            for _ in 0..10 {
                step(&mut world, &mut schedule);
            }
            let saved = capture_world(&world).unwrap();
            let (mut loaded, loaded_schedule) = rig();
            strip_map(&mut loaded, "turf", 5, "turf", LayerKind::Render, false);
            place(&mut loaded, "turf", Vec2::ZERO);
            restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
            loaded
                .resource_mut::<TilemapRegistry>()
                .set_tile("turf", 0, 2, 0, 1);
            world = loaded;
            schedule = loaded_schedule;
        }
        for _ in 0..70 {
            step(&mut world, &mut schedule);
        }
        assert_eq!(
            sprite_at(&world, "turf", 2),
            "turf_mid",
            "save {through_save}"
        );
        assert!(
            world
                .resource::<TilemapRegistry>()
                .get("turf")
                .unwrap()
                .is_auto(0, 2, 0)
        );
        // `stone` beside it opens its right side: the auto-tiler re-picks it.
        world
            .resource_mut::<TilemapRegistry>()
            .set_tile("turf", 0, 3, 0, 2);
        assert_eq!(
            sprite_at(&world, "turf", 2),
            "turf_right",
            "save {through_save}"
        );
    }

    // A game's write of `stone` before the revert runs out drops it.
    let (mut world, mut schedule) = rig();
    strip_map(&mut world, "strip", 3, "grass", LayerKind::Render, false);
    place(&mut world, "strip", Vec2::ZERO);
    tiles::set_tile(&mut world, &target, "ash", Some(5.0)).unwrap();
    for _ in 0..120 {
        step(&mut world, &mut schedule);
    }
    world
        .resource_mut::<TilemapRegistry>()
        .set_tile("strip", 0, 1, 0, 2);
    for _ in 0..300 {
        step(&mut world, &mut schedule);
    }
    assert_eq!(
        sprite_at(&world, "strip", 1),
        "stone",
        "the game's write stands"
    );

    // Two placements' overlapping reverts on one map cell end on the
    // original tile, through a save.
    let (mut world, mut schedule) = rig();
    strip_map(&mut world, "strip", 3, "grass", LayerKind::Render, false);
    let far = Vec2::new(500.0, 0.0);
    place(&mut world, "strip", Vec2::ZERO);
    place(&mut world, "strip", far);
    tiles::set_tile(&mut world, &target, "ash", Some(5.0)).unwrap();
    for _ in 0..120 {
        step(&mut world, &mut schedule);
    }
    tiles::set_tile(&mut world, &cell("strip", far, 1), "ember", Some(5.0)).unwrap();
    for _ in 0..60 {
        step(&mut world, &mut schedule);
    }
    let saved = capture_world(&world).unwrap();
    let (mut loaded, mut loaded_schedule) = rig();
    strip_map(&mut loaded, "strip", 3, "grass", LayerKind::Render, false);
    place(&mut loaded, "strip", Vec2::ZERO);
    place(&mut loaded, "strip", far);
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    loaded
        .resource_mut::<TilemapRegistry>()
        .set_tile("strip", 0, 1, 0, 3);
    for _ in 0..239 {
        step(&mut loaded, &mut loaded_schedule);
    }
    assert_eq!(sprite_at(&loaded, "strip", 1), "ember");
    step(&mut loaded, &mut loaded_schedule);
    assert_eq!(
        sprite_at(&loaded, "strip", 1),
        "grass",
        "the original, not ash"
    );
}

#[test]
fn two_placements_of_a_map_keep_apart() {
    let (mut world, mut schedule) = rig();
    burning_tiles(&mut world);
    strip_map(&mut world, "pair", 4, "grass", LayerKind::Render, false);
    let far = Vec2::new(1000.0, 0.0);
    place(&mut world, "pair", Vec2::ZERO);
    place(&mut world, "pair", far);
    // A crate beside the near placement's first cell, burning once.
    let crate_ = world.spawn_with((Transform::from_position(Vec2::new(-8.0, 8.0)),));
    EffectRequest::new(crate_, DataRef::by_id("burning")).apply(&mut world);
    for _ in 0..16 {
        step(&mut world, &mut schedule);
    }
    assert!(tile_has(&world, &cell("pair", Vec2::ZERO, 0), "burning"));
    assert!(
        !tile_has(&world, &cell("pair", far, 0), "burning"),
        "the far placement's own"
    );
    let saved = capture_world(&world).unwrap();
    let (mut loaded, _) = rig();
    burning_tiles(&mut loaded);
    strip_map(&mut loaded, "pair", 4, "grass", LayerKind::Render, false);
    place(&mut loaded, "pair", Vec2::ZERO);
    place(&mut loaded, "pair", far);
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    assert!(tile_has(&loaded, &cell("pair", Vec2::ZERO, 0), "burning"));
    assert!(!tile_has(&loaded, &cell("pair", far, 0), "burning"));

    // An impact on the far placement's tile stimulates its own cell.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&seen);
    let (mut world, mut schedule) = physics_rig(&ChemistryPlugin::new().with_handler(
        "struck",
        move |_, reaction: &Reaction| {
            let cell = reaction.cell().unwrap();
            noted.borrow_mut().push((cell.origin, cell.col));
        },
    ));
    rule(
        &mut world,
        "cracks",
        json!({ "stimulus": "stimulus.impact", "on": { "all": ["grass"] }, "outputs": [{ "handler": "struck" }] }),
    );
    strip_map(&mut world, "pair", 4, "grass", LayerKind::Collision, false);
    place(&mut world, "pair", Vec2::ZERO);
    place(&mut world, "pair", far);
    let ball = world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(1000.0 + 24.0, -20.0),
        Collider::circle(5.0),
    ));
    world.get_mut::<Velocity>(ball).unwrap().0 = Vec2::new(0.0, 400.0);
    for _ in 0..20 {
        step(&mut world, &mut schedule);
    }
    let seen = seen.borrow();
    assert!(seen.contains(&(far, 1)), "the struck cell: {seen:?}");
    assert!(seen.iter().all(|(origin, _)| *origin == far), "{seen:?}");
}

#[test]
fn damage_breaks_a_cell_at_its_health() {
    let (mut world, mut schedule) = rig();
    rule(
        &mut world,
        "chips",
        json!({ "stimulus": "heat", "on": { "all": ["stone"] }, "outputs": [{ "damage": 2 }] }),
    );
    strip_map(&mut world, "wall", 3, "stone", LayerKind::Render, false);
    place(&mut world, "wall", Vec2::ZERO);
    let block = cell("wall", Vec2::ZERO, 1);
    let heat = |world: &mut World| {
        let settings = Settings::default();
        let stimulus = queue::Stimulus {
            amount: 1.0,
            status_effect: 1.0,
            dot_base: 1.0,
            depth: 0,
            credit: None,
        };
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        chemistry.stimulate(
            &settings,
            Receiver::Cell(block.clone()),
            "heat",
            stimulus,
            run,
        );
    };
    heat(&mut world);
    step(&mut world, &mut schedule);
    let registry = world.resource::<TilemapRegistry>();
    assert_eq!(registry.health("wall", 0, 1, 0), Some(1.0), "3 less 2");
    assert_eq!(sprite_at(&world, "wall", 1), "stone");
    heat(&mut world);
    step(&mut world, &mut schedule);
    assert_eq!(sprite_at(&world, "wall", 1), "ash", "broken at the next");
}

#[test]
fn a_tile_without_a_material_takes_nothing() {
    let (mut world, mut schedule) = rig();
    burning_tiles(&mut world);
    rule(
        &mut world,
        "anything",
        json!({ "stimulus": "heat", "outputs": [{ "remove": "mark" }] }),
    );
    strip_map(&mut world, "ashes", 4, "ash", LayerKind::Render, false);
    place(&mut world, "ashes", Vec2::ZERO);
    let crate_ = world.spawn_with((Transform::from_position(Vec2::new(-8.0, 8.0)),));
    EffectRequest::new(crate_, DataRef::by_id("burning")).apply(&mut world);
    for _ in 0..40 {
        step(&mut world, &mut schedule);
    }
    assert_eq!(chemistry(&world).ran(), 0, "no cell received");
    assert!(world.resource::<TileStatuses>().is_empty());
}

#[test]
fn an_impact_on_a_tile_stimulates_its_cell() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&seen);
    let (mut world, mut schedule) = physics_rig(&ChemistryPlugin::new().with_handler(
        "struck",
        move |_, reaction: &Reaction| {
            let cell = reaction.cell().unwrap();
            noted.borrow_mut().push((
                cell.map.to_owned(),
                cell.col,
                reaction.amount(),
                reaction.credit(),
            ));
        },
    ));
    rule(
        &mut world,
        "cracks",
        json!({ "stimulus": "stimulus.impact", "on": { "all": ["grass"] }, "outputs": [{ "handler": "struck" }] }),
    );
    strip_map(&mut world, "floor", 4, "grass", LayerKind::Collision, false);
    place(&mut world, "floor", Vec2::ZERO);
    let ball = world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(40.0, -20.0),
        Collider::circle(5.0),
    ));
    world.get_mut::<Velocity>(ball).unwrap().0 = Vec2::new(0.0, 400.0);
    for _ in 0..20 {
        step(&mut world, &mut schedule);
    }
    // The physics begins a contact with each cell whose face the ball's
    // reach spans: the struck cell and its neighbours, each stimulated by
    // the closing speed, credited to the ball.
    let seen = seen.borrow();
    let cols: Vec<u32> = seen.iter().map(|(_, col, ..)| *col).collect();
    assert_eq!(cols, [1, 2, 3], "the struck cell 2 among them");
    for (map, _, amount, credit) in seen.iter() {
        assert_eq!((map.as_str(), *credit), ("floor", Some(ball)));
        assert!((amount - 400.0).abs() <= 4.0, "{amount}");
    }
}

#[test]
fn tile_statuses_drop_with_their_placement() {
    let (mut world, mut schedule) = rig();
    burning_tiles(&mut world);
    strip_map(&mut world, "strip", 2, "grass", LayerKind::Render, false);
    let near = place(&mut world, "strip", Vec2::ZERO);
    let moved = place(&mut world, "strip", Vec2::new(500.0, 0.0));
    let held = place(&mut world, "strip", Vec2::new(900.0, 0.0));
    for origin in [Vec2::ZERO, Vec2::new(500.0, 0.0), Vec2::new(900.0, 0.0)] {
        assert!(tiles::apply(
            &mut world,
            &cell("strip", origin, 0),
            &DataRef::by_id("burning"),
            1.0,
            1.0,
            None,
            0
        ));
    }
    step(&mut world, &mut schedule);
    world.despawn(near);
    world.get_mut::<TilemapInstance>(moved).unwrap().origin = Vec2::new(600.0, 0.0);
    step(&mut world, &mut schedule);
    let statuses = world.resource::<TileStatuses>();
    assert!(
        statuses.on(&cell("strip", Vec2::ZERO, 0)).is_empty(),
        "gone"
    );
    assert!(
        statuses
            .on(&cell("strip", Vec2::new(500.0, 0.0), 0))
            .is_empty(),
        "moved"
    );
    assert!(
        !statuses
            .on(&cell("strip", Vec2::new(900.0, 0.0), 0))
            .is_empty(),
        "kept"
    );
    let _ = held;
}

// Step 4: `become` keeping identity.

/// [`rig_with`] with the source derivation and the prefabs `lamp`
/// (`light.lamp`) and `lamp_doused` (`light.doused`).
fn become_rig(plugin: &ChemistryPlugin) -> (World, Schedule) {
    let (mut world, mut schedule) = rig_with(plugin);
    crate::SourcePlugin.build(&mut schedule, &mut world);
    let mut prefabs = tungsten_core::PrefabRegistry::new();
    for (id, tag) in [("lamp", "light.lamp"), ("lamp_doused", "light.doused")] {
        prefabs.insert(
            id.to_string(),
            serde_json::from_value(json!({ "tag": id, "components": { "tags": [tag] } })).unwrap(),
        );
    }
    world.insert_resource(prefabs);
    rule(
        &mut world,
        "douses",
        json!({
            "stimulus": "wet", "on": { "all": ["light.lamp"] },
            "outputs": [{ "become": { "prefab": "lamp_doused", "revert": { "after": 3, "to": "lamp" } } }]
        }),
    );
    (world, schedule)
}

/// The entity tagged `tag`, when exactly one is.
fn tagged_one(world: &World, tag: &str) -> Option<Entity> {
    let found: Vec<Entity> = world
        .query::<(Entity, &tungsten_core::Tag)>()
        .filter(|(_, found)| found.name == tag)
        .map(|(entity, _)| entity)
        .collect();
    (found.len() == 1).then(|| found[0])
}

#[test]
fn become_replaces_and_reverts() {
    let wet = queue::Stimulus {
        amount: 1.0,
        status_effect: 1.0,
        dot_base: 1.0,
        depth: 0,
        credit: None,
    };
    // At `depth`: the take order runs the shallower entry first.
    let stimulate = |world: &mut World, entity: Entity, name: &str, depth: u8| {
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        let stimulus = queue::Stimulus { depth, ..wet };
        chemistry.stimulate(
            &Settings::default(),
            Receiver::Entity(entity),
            name,
            stimulus,
            run,
        );
    };
    let lit = |world: &mut World, owner: Option<Entity>| {
        let lamp = world.spawn_prefab("lamp", [40.0, 40.0]).unwrap();
        world.insert(lamp, Health::new(10.0));
        if let Some(owner) = owner {
            world.insert(lamp, crate::Source::new(owner));
        }
        tungsten_core::save::name(world, lamp, None, "hall_lamp").unwrap();
        lamp
    };
    for owned in [true, false] {
        let (mut world, mut schedule) = become_rig(&ChemistryPlugin::new().with_cap(1));
        let owner = world.spawn();
        let lamp = lit(&mut world, owned.then_some(owner));
        let id = world.get::<tungsten_core::save::Saved>(lamp).unwrap().id();
        EffectRequest::new(lamp, DataRef::by_id("mark")).apply(&mut world);
        // `wet` first, then an entry that runs after the flush.
        stimulate(&mut world, lamp, "wet", 0);
        stimulate(&mut world, lamp, "heat", 1);
        step(&mut world, &mut schedule);
        assert!(!world.is_alive(lamp));
        let doused = tagged_one(&world, "lamp_doused").expect("the doused lamp");
        assert_eq!(
            world.get::<Transform>(doused).unwrap().position,
            Vec2::new(40.0, 40.0),
            "at its point"
        );
        let saved = world.get::<tungsten_core::save::Saved>(doused).unwrap();
        assert_eq!((saved.id(), saved.prefab()), (id, Some("lamp_doused")));
        let source = world
            .get::<crate::Source>(doused)
            .map(|source| source.entity());
        assert_eq!(
            source,
            Some(if owned { owner } else { lamp }),
            "owned {owned}"
        );
        assert!(!has(&world, doused, "mark"), "statuses are not carried");
        step(&mut world, &mut schedule);
        assert_eq!(
            skips(&world, lamp),
            [("all", GONE)],
            "the old entity's entry"
        );
        // `lamp` again 3 s after the flush, within a step: frame counts the
        // steps since it.
        let mut back = None;
        for frame in 2..=200 {
            step(&mut world, &mut schedule);
            if back.is_none() && tagged_one(&world, "lamp").is_some() {
                back = Some(frame);
            }
        }
        assert_eq!(back, Some(180), "owned {owned}");
        let relit = tagged_one(&world, "lamp").unwrap();
        let saved = world.get::<tungsten_core::save::Saved>(relit).unwrap();
        assert_eq!((saved.id(), saved.prefab()), (id, Some("lamp")));
        assert!(tagged_one(&world, "lamp_doused").is_none());
    }
    // Through a save between: the doused lamp comes back with its revert,
    // turning back on the uninterrupted run's step.
    let (mut world, mut schedule) = become_rig(&ChemistryPlugin::new());
    let lamp = lit(&mut world, None);
    stimulate(&mut world, lamp, "wet", 0);
    step(&mut world, &mut schedule);
    for _ in 0..60 {
        step(&mut world, &mut schedule);
    }
    let saved = capture_world(&world).unwrap();
    let (mut loaded, mut loaded_schedule) = become_rig(&ChemistryPlugin::new());
    let fresh = lit(&mut loaded, None);
    restore_world(&mut loaded, &saved, &(Restored,)).unwrap();
    assert!(!loaded.is_alive(fresh), "the named lamp reads as gone");
    assert!(tagged_one(&loaded, "lamp_doused").is_some());
    let mut back = (None, None);
    for frame in 1..=200 {
        step(&mut world, &mut schedule);
        step(&mut loaded, &mut loaded_schedule);
        if back.0.is_none() && tagged_one(&world, "lamp").is_some() {
            back.0 = Some(frame);
        }
        if back.1.is_none() && tagged_one(&loaded, "lamp").is_some() {
            back.1 = Some(frame);
        }
    }
    assert_eq!(back, (Some(120), Some(120)), "at 3 s both ways");
}

// Step 5: the blast.

/// The damage each entity took, by its `DamagedPart`s this step: `(amount,
/// type, source, depth)`.
fn parts_on(world: &World, entity: Entity) -> Vec<(f32, String, Option<Entity>, u8)> {
    world
        .resource::<tungsten_core::EventQueue<crate::DamagedPart>>()
        .iter()
        .filter(|part| part.entity == entity)
        .map(|part| {
            // Its deepest type: the kinds list the ancestors too.
            let names = part.kinds.names(world);
            let deepest = names
                .iter()
                .max_by_key(|name| name.split('.').count())
                .map_or(String::new(), |name| (*name).to_owned());
            (part.amount, deepest, part.source, part.depth)
        })
        .collect()
}

#[test]
fn a_blast_hurts_and_pushes_by_its_falloff() {
    use tungsten_core::physics::{BodyKind, RigidBody};

    use crate::{Knockback, Line, TagFilter};

    let (mut world, mut schedule) = physics_rig(&ChemistryPlugin::new());
    let types: DamageTypes = serde_json::from_value(json!({
        "elemental": {}, "elemental.fire": {}, "elemental.cold": {}
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("fantasy", types);
    rule(
        &mut world,
        "blows",
        json!({
            "stimulus": "heat", "on": { "all": ["node"] },
            "outputs": [{ "blast": {
                "radius": 100, "damage": { "elemental.fire": 8, "elemental.cold": 8 },
                "context": ["team.enemy"], "push": 600, "impulse": 300
            } }]
        }),
    );
    // The keg at the origin, bodiless, with health and a knockback.
    let keg = world.spawn_with((
        Health::new(100.0),
        Tags::new(["node"]),
        Knockback::new(1.0),
        Transform::from_position(Vec2::ZERO),
    ));
    // Targets on circle bodies of radius 10 that take no impulse
    // (kinematic, no mass), their surfaces 0, 50 and 100 px out.
    let target = |world: &mut World, at: Vec2| {
        world.spawn_with(RigidBodyBundle::kinematic(
            Position(at),
            Collider::circle(10.0),
        ))
    };
    let near = target(&mut world, Vec2::ZERO);
    let mid = target(&mut world, Vec2::new(60.0, 0.0));
    let edge = target(&mut world, Vec2::new(0.0, 110.0));
    for entity in [near, mid, edge] {
        world.insert(entity, Health::new(100.0));
        world.insert(entity, Knockback::new(1.0));
    }
    // One at 50 px whose fire factor halves in the blast's context.
    let guarded = target(&mut world, Vec2::new(0.0, -60.0));
    world.insert(guarded, Health::new(100.0));
    let mut resist = Resist::new();
    resist.add_line(
        "elemental.fire",
        "blast_guard",
        Line::new(crate::Modifier::Mul(0.5)).with(TagFilter::new().all(["team.enemy"])),
    );
    world.insert(guarded, resist);
    // Dynamic bodies of mass 1 and 2 at 50 px; a static body, a sensor and
    // a kinematic body without a mass inside the radius.
    let one = world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(-60.0, 0.0),
        Collider::circle(10.0),
    ));
    let two = world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(0.0, 160.0),
        Collider::circle(10.0),
    ));
    *world.get_mut::<RigidBody>(two).unwrap() = RigidBody::dynamic().with_mass(2.0);
    world.get_mut::<Position>(two).unwrap().0 = Vec2::new(42.426_407, 42.426_407);
    let fixed = world.spawn_with(RigidBodyBundle::r#static(
        Position::new(-50.0, -50.0),
        Collider::circle(10.0),
    ));
    let sensor = world.spawn_with(RigidBodyBundle::r#static(
        Position::new(50.0, -50.0),
        Collider::circle(10.0),
    ));
    *world.get_mut::<RigidBody>(sensor).unwrap() = RigidBody::sensor();
    let massless = world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(-50.0, 50.0),
        Collider::circle(10.0),
    ));
    // A bodiless target and a body, each with health and no tags.
    let bodiless = world.spawn_with((
        Health::new(100.0),
        Transform::from_position(Vec2::new(30.0, 30.0)),
    ));
    let unbodied = target(&mut world, Vec2::new(-30.0, -30.0));
    world.insert(unbodied, Health::new(100.0));
    let _ = BodyKind::Dynamic;
    // A step for the physics index, then the stimulus from the chain's
    // source.
    step(&mut world, &mut schedule);
    let source = world.spawn();
    let stimulus = queue::Stimulus {
        amount: 1.0,
        status_effect: 1.0,
        dot_base: 1.0,
        depth: 0,
        credit: Some(source),
    };
    {
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        chemistry.stimulate(
            &Settings::default(),
            Receiver::Entity(keg),
            "heat",
            stimulus,
            run,
        );
    }
    let velocity = |world: &World, entity: Entity| world.get::<Velocity>(entity).map(|v| v.0);
    let before: Vec<Option<Vec2>> = [fixed, sensor, massless]
        .iter()
        .map(|&e| velocity(&world, e))
        .collect();
    step(&mut world, &mut schedule);
    // The impulse lands at once: 300 × 0.5 over the mass, away.
    let pushed = |world: &World, entity: Entity| velocity(world, entity).unwrap();
    assert!(
        (pushed(&world, one) - Vec2::new(-150.0, 0.0)).length() < 1e-2,
        "{}",
        pushed(&world, one)
    );
    let away = Vec2::new(1.0, 1.0).normalize() * 75.0;
    assert!(
        (pushed(&world, two) - away).length() < 1e-2,
        "{}",
        pushed(&world, two)
    );
    let after: Vec<Option<Vec2>> = [fixed, sensor, massless]
        .iter()
        .map(|&e| velocity(&world, e))
        .collect();
    assert_eq!(
        before, after,
        "the static body, the sensor and the massless kinematic one"
    );
    // The damage at the next step's resolve: typed, credited, a reaction
    // deep, by the falloff.
    assert!(parts_on(&world, near).is_empty(), "not resolved yet");
    step(&mut world, &mut schedule);
    let typed = |amount: f32| {
        vec![
            (amount, "elemental.cold".to_string(), Some(source), 1),
            (amount, "elemental.fire".to_string(), Some(source), 1),
        ]
    };
    let sorted = |mut parts: Vec<(f32, String, Option<Entity>, u8)>| {
        parts.sort_by(|a, b| a.1.cmp(&b.1));
        parts
    };
    assert_eq!(sorted(parts_on(&world, near)), typed(8.0));
    assert_eq!(sorted(parts_on(&world, mid)), typed(4.0));
    assert!(parts_on(&world, edge).is_empty(), "at the radius: nothing");
    let health = |world: &World, entity: Entity| world.get::<Health>(entity).unwrap().current();
    assert!(
        (health(&world, guarded) - (100.0 - 2.0 - 4.0)).abs() < 1e-4,
        "the context halves its fire"
    );
    assert!(
        (health(&world, bodiless) - (100.0 - 2.0 * 8.0 * (1.0 - 42.426_407 / 100.0))).abs() < 1e-3
    );
    let unbodied_d = 42.426_407_f32 - 10.0;
    assert!(
        (health(&world, unbodied) - (100.0 - 2.0 * 8.0 * (1.0 - unbodied_d / 100.0))).abs() < 1e-3
    );
    let push = |world: &World, entity: Entity| world.get::<Knockback>(entity).unwrap().push();
    assert!(
        (push(&world, near) - Vec2::new(0.0, -600.0)).length() < 1e-3,
        "straight up"
    );
    assert!((push(&world, mid) - Vec2::new(300.0, 0.0)).length() < 1e-3);
    assert_eq!(push(&world, edge), Vec2::ZERO);
    // The keg neither hurt nor pushed.
    assert_eq!(
        (health(&world, keg), push(&world, keg)),
        (100.0, Vec2::ZERO)
    );
}

#[test]
fn a_chain_of_kegs_ends_at_the_depth_limit() {
    let notes = Notes::default();
    let (mut world, mut schedule) =
        rig_with(&ChemistryPlugin::new().with_handler("noted", noting(&notes)));
    world
        .resource_mut::<DataRegistry>()
        .insert("armed", Effect::new().with_grant(["status.armed"]));
    rule(
        &mut world,
        "goes_off",
        json!({
            "stimulus": "stimulus.blast", "on": { "all": ["status.armed"] },
            "outputs": [
                { "remove": "armed" }, { "handler": "noted" },
                { "blast": { "radius": 50, "damage": 1, "push": 100 } }
            ]
        }),
    );
    // Ten kegs 40 px apart, health 1, kept when dead, armed.
    let kegs: Vec<Entity> = (0..10)
        .map(|i| {
            let keg = world.spawn_with((
                Health::new(1.0),
                Tags::new(["node"]),
                crate::Knockback::new(1.0),
                Transform::from_position(Vec2::new(i as f32 * 40.0, 0.0)),
            ));
            EffectRequest::new(keg, DataRef::by_id("armed")).apply(&mut world);
            keg
        })
        .collect();
    let blast = queue::Stimulus {
        amount: 1.0,
        status_effect: 1.0,
        dot_base: 1.0,
        depth: 0,
        credit: None,
    };
    {
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        chemistry.stimulate(
            &Settings::default(),
            Receiver::Entity(kegs[0]),
            "stimulus.blast",
            blast,
            run,
        );
    }
    let mut per_step = Vec::new();
    for frame in 0..14 {
        step(&mut world, &mut schedule);
        per_step.push(notes.borrow().len());
        if frame == 0 {
            // Its own blast neither hurts nor pushes the first keg.
            let first = kegs[0];
            assert!(!world.get::<Health>(first).unwrap().is_dead());
            assert_eq!(
                world.get::<crate::Knockback>(first).unwrap().push(),
                Vec2::ZERO
            );
        }
    }
    let went_off: Vec<u8> = notes.borrow().iter().map(|(_, _, depth)| *depth).collect();
    assert_eq!(
        went_off,
        (0..=8).collect::<Vec<u8>>(),
        "the first nine, depths 0 to 8"
    );
    assert!(
        per_step.windows(2).all(|pair| pair[1] - pair[0] <= 1),
        "a run apart: {per_step:?}"
    );
    assert!(has(&world, kegs[9], "armed"), "the tenth never goes off");
    assert!(chemistry(&world).cut() >= 1, "the tenth cut and counted");
    assert_eq!(chemistry(&world).pending(), 0);
}

#[test]
fn a_blast_reaches_cells() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&seen);
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_handler(
        "struck",
        move |_, reaction: &Reaction| {
            let cell = reaction.cell().unwrap();
            noted.borrow_mut().push((cell.col, reaction.amount()));
        },
    ));
    rule(
        &mut world,
        "blows",
        json!({ "stimulus": "heat", "on": { "all": ["node"] }, "outputs": [{ "blast": { "radius": 40 } }] }),
    );
    rule(
        &mut world,
        "shatters",
        json!({ "stimulus": "stimulus.blast", "on": { "all": ["glass"] }, "outputs": [{ "handler": "struck" }, { "damage": 1 }] }),
    );
    strip_map(&mut world, "panes", 8, "glass", LayerKind::Render, false);
    place(&mut world, "panes", Vec2::ZERO);
    // A keg at cell 2's centre: cells 0 to 4 lie within 40 px.
    let keg = world.spawn_with((
        Tags::new(["node"]),
        Transform::from_position(Vec2::new(40.0, 8.0)),
    ));
    {
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        let stimulus = queue::Stimulus {
            amount: 1.0,
            status_effect: 1.0,
            dot_base: 1.0,
            depth: 0,
            credit: None,
        };
        chemistry.stimulate(
            &Settings::default(),
            Receiver::Entity(keg),
            "heat",
            stimulus,
            run,
        );
    }
    for _ in 0..3 {
        step(&mut world, &mut schedule);
    }
    let mut seen = seen.borrow().clone();
    seen.sort_by_key(|(col, _)| *col);
    let falloffs: Vec<(u32, f32)> = [(0, 0.2), (1, 0.6), (2, 1.0), (3, 0.6), (4, 0.2)].to_vec();
    assert_eq!(seen.len(), falloffs.len(), "{seen:?}");
    for ((col, amount), (want_col, want)) in seen.iter().zip(&falloffs) {
        assert_eq!(col, want_col);
        assert!((amount - want).abs() < 1e-5, "{col}: {amount}");
    }
    let broken: Vec<u32> = (0..8)
        .filter(|&col| sprite_at(&world, "panes", col) == "ash")
        .collect();
    assert_eq!(broken, [0, 1, 2, 3, 4]);
}

#[test]
fn a_capped_spawn_output_culls_the_oldest() {
    let (mut world, mut schedule) = rig();
    crate::CapPlugin.build(&mut schedule, &mut world);
    let mut prefabs = tungsten_core::PrefabRegistry::new();
    prefabs.insert(
        "salt".to_string(),
        serde_json::from_value(
            json!({ "tag": "salt", "components": { "cap": { "group": "salt", "max": 2 } } }),
        )
        .unwrap(),
    );
    world.insert_resource(prefabs);
    rule(
        &mut world,
        "salts",
        json!({ "stimulus": "heat", "outputs": [{ "spawn": "salt" }] }),
    );
    strip_map(&mut world, "floor", 3, "grass", LayerKind::Render, false);
    place(&mut world, "floor", Vec2::ZERO);
    let vat = world.spawn_with((Transform::from_position(Vec2::new(0.0, -40.0)),));
    let heat = |world: &mut World, receiver: Receiver| {
        let stimulus = queue::Stimulus {
            amount: 1.0,
            status_effect: 1.0,
            dot_base: 1.0,
            depth: 0,
            credit: None,
        };
        let chemistry = world.resource_mut::<Chemistry>();
        let run = chemistry.run;
        chemistry.stimulate(&Settings::default(), receiver, "heat", stimulus, run);
    };
    let salts = |world: &World| -> Vec<Entity> {
        let mut found: Vec<Entity> = world
            .query::<(Entity, &tungsten_core::Tag)>()
            .filter(|(_, tag)| tag.name == "salt")
            .map(|(entity, _)| entity)
            .collect();
        found.sort_by_key(|entity| entity.id());
        found
    };
    let culled = |world: &World| -> Vec<Entity> {
        world
            .resource::<KitTrace>()
            .iter()
            .filter(|entry| matches!(entry.kind, TraceKind::Despawn { by } if by == "cap"))
            .map(|entry| entry.entity)
            .collect()
    };
    // An entity's spawns, from it: the third ends the first.
    let mut spawned = Vec::new();
    for _ in 0..3 {
        heat(&mut world, Receiver::Entity(vat));
        step(&mut world, &mut schedule);
        for salt in salts(&world) {
            if !spawned.contains(&salt) {
                spawned.push(salt);
            }
        }
    }
    assert_eq!(spawned.len(), 3);
    assert_eq!(culled(&world), [spawned[0]]);
    assert_eq!(salts(&world).len(), 2);
    // A tile cell's, sourceless, counted all the same: each ends the oldest.
    for col in [0, 2] {
        heat(&mut world, Receiver::Cell(cell("floor", Vec2::ZERO, col)));
        step(&mut world, &mut schedule);
    }
    assert_eq!(culled(&world), spawned);
    let left = salts(&world);
    assert_eq!(left.len(), 2);
    assert!(
        left.iter()
            .all(|&salt| world.get::<crate::Source>(salt).is_none()),
        "the cells' spawns"
    );
}

#[test]
fn a_build_is_relative_to_its_receivers_per_stat() {
    let (mut world, mut schedule) = rig();
    rule(
        &mut world,
        "burns",
        json!({
            "stimulus": "elemental.fire",
            "outputs": [{ "build": { "status": "burning", "amount": 4, "per": "toughness" } }]
        }),
    );
    let tough = world.spawn_with((Health::new(1000.0), Stats::new().with("toughness", 40.0)));
    let plain = receiver(&mut world, &[]);
    let broken = world.spawn_with((Health::new(1000.0), Stats::new().with("toughness", 0.0)));
    for entity in [tough, plain, broken] {
        hit(&mut world, entity, "elemental.fire", 1.0);
    }
    step(&mut world, &mut schedule);
    assert_eq!(intensity(&world, tough, "burning"), Some(0.1), "a fortieth");
    assert_eq!(
        intensity(&world, plain, "burning"),
        Some(4.0),
        "whole without the stat"
    );
    assert_eq!(intensity(&world, broken, "burning"), None, "refused at 0");
    assert!(skips(&world, broken).contains(&("build", outputs::BAD_PER)));
    // The load refuses a name that is not a stat's.
    let bad = json!({
        "stimulus": "elemental.fire",
        "outputs": [{ "build": { "status": "burning", "per": "Tough ness" } }]
    });
    assert!(serde_json::from_value::<Interaction>(bad).is_err());
}

/// What a handler saw of each reaction: its receiver, stimulus, amount,
/// credit and depth.
type Stimuli = Rc<RefCell<Vec<(Entity, String, f32, Option<Entity>, u8)>>>;

#[test]
fn a_games_stimulus_reaches_its_rule_as_a_part_does() {
    let stimuli = Stimuli::default();
    let saw = Rc::clone(&stimuli);
    let (mut world, mut schedule) = rig_with(&ChemistryPlugin::new().with_handler(
        "seen",
        move |_, reaction: &Reaction| {
            saw.borrow_mut().push((
                reaction.receiver().unwrap_or(Entity::PLACEHOLDER),
                reaction.stimulus().to_owned(),
                reaction.amount(),
                reaction.credit(),
                reaction.depth(),
            ));
        },
    ));
    let seen = json!([{ "handler": "seen" }]);
    rule(
        &mut world,
        "warms",
        json!({ "stimulus": "heat", "outputs": seen }),
    );
    rule(
        &mut world,
        "burns",
        json!({ "stimulus": "elemental.fire", "outputs": seen }),
    );
    // A body without health, credited, at depth 0.
    let wizard = world.spawn();
    let stone = world.spawn_with((Tags::new(["material.stone"]),));
    assert!(stimulate(&mut world, stone, "heat", 2.0, Some(wizard)));
    step(&mut world, &mut schedule);
    assert_eq!(
        stimuli.borrow().as_slice(),
        [(stone, "heat".to_owned(), 2.0, Some(wizard), 0)]
    );
    // A damage type merges with a landed part of it on the same receiver:
    // one entry, the amounts summed, the first credit kept.
    stimuli.borrow_mut().clear();
    let ran = chemistry(&world).ran();
    let target = receiver(&mut world, &[]);
    assert!(stimulate(
        &mut world,
        target,
        "elemental.fire",
        1.5,
        Some(wizard)
    ));
    hit(&mut world, target, "elemental.fire", 3.0);
    step(&mut world, &mut schedule);
    assert_eq!(chemistry(&world).ran() - ran, 1, "one entry");
    assert_eq!(
        stimuli.borrow().as_slice(),
        [(target, "elemental.fire".to_owned(), 4.5, Some(wizard), 0)]
    );
    // Refused: an undeclared stimulus, an amount below 0 or not finite, a
    // gone receiver, and a World without chemistry; one no rule answers is
    // not queued.
    assert!(!stimulate(&mut world, stone, "frost", 1.0, None));
    assert!(world.resource::<Chemistry>().refused_now, "warned once");
    assert!(!stimulate(&mut world, stone, "heat", -1.0, None));
    assert!(!stimulate(&mut world, stone, "heat", f32::NAN, None));
    let gone = world.spawn();
    world.despawn(gone);
    assert!(!stimulate(&mut world, gone, "heat", 1.0, None));
    assert!(!stimulate(&mut World::new(), stone, "heat", 1.0, None));
    assert!(
        !stimulate(&mut world, stone, "ping", 1.0, None),
        "unanswered"
    );
    assert_eq!(chemistry(&world).pending(), 0);
    step(&mut world, &mut schedule);
    assert!(
        !world.resource::<Chemistry>().refused_now,
        "cleared by the run"
    );
}

#[test]
fn a_build_crossing_two_bands_in_one_run_enters_both() {
    let calls = Rc::new(Calls::default());
    let (mut world, mut schedule) =
        rig_with(&ChemistryPlugin::new().with_handler("entered", counting(&calls, "entered")));
    let ladder: Effect = serde_json::from_value(json!({
        "intensity": { "max": 5, "decay": 0 },
        "bands": [
            { "at": 1, "enter": [{ "handler": "entered" }] },
            { "at": 2, "enter": [{ "handler": "entered" }] }
        ]
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("ladder", ladder);
    rule(
        &mut world,
        "climbs",
        json!({ "stimulus": "heat", "outputs": [{ "build": { "status": "ladder", "amount": 3 } }] }),
    );
    let stone = world.spawn();
    assert!(stimulate(&mut world, stone, "heat", 1.0, None));
    step(&mut world, &mut schedule);
    assert_eq!(intensity(&world, stone, "ladder"), Some(3.0));
    step(&mut world, &mut schedule);
    assert_eq!(
        calls.0.borrow().as_slice(),
        ["entered:ladder#enter.0", "entered:ladder#enter.1"]
    );
}

/// The queue's pending entries and the entries the runs took.
fn queue_counts(world: &World) -> (usize, u64) {
    let chemistry = chemistry(world);
    (chemistry.pending(), chemistry.ran())
}

/// The rule that sets a receiver burning, at a cap of 1, with `names`
/// named in order.
fn burning_rig<const N: usize>(names: [&str; N]) -> (World, Schedule, [Entity; N]) {
    let wood = json!({ "stimulus": "elemental.fire", "outputs": [{ "build": { "status": "burning", "amount": 1 } }] });
    let (mut world, schedule) = rig_with(&ChemistryPlugin::new().with_cap(1));
    rule(&mut world, "wood", wood);
    let named = names.map(|name| {
        let entity = receiver(&mut world, &[]);
        tungsten_core::save::name(&mut world, entity, None, name).unwrap();
        entity
    });
    (world, schedule, named)
}

/// A queue that holds an entry on a receiver despawned before the capture,
/// ahead of a younger one on `live`: two hits at depth 0 and one at depth 1
/// on `doomed`, unnamed, a run each; then a hit on `live`, whose run takes
/// the second of depth 0; then `doomed` despawned.
fn dead_ahead() -> (World, Schedule, Entity) {
    let (mut world, mut schedule, [a, b, live]) = burning_rig(["a", "b", "live"]);
    let doomed = receiver(&mut world, &[]);
    for (target, depth) in [(a, 0), (b, 0), (doomed, 1)] {
        DamageRequest::from_parts(
            target,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_depth(depth)
        .apply(&mut world);
    }
    step(&mut world, &mut schedule);
    hit(&mut world, live, "elemental.fire", 1.0);
    step(&mut world, &mut schedule);
    assert_eq!(queue_counts(&world), (2, 2), "doomed's, then live's");
    world.despawn(doomed);
    (world, schedule, live)
}

#[test]
fn a_dead_receivers_entry_keeps_its_place_under_the_cap_through_a_load() {
    let (mut a, mut schedule_a, live_a) = dead_ahead();
    let saved = capture_world(&a).unwrap();
    let text = serde_json::to_value(&saved).unwrap();
    let entries = &text["resources"]["chemistry_queue"]["entries"];
    assert_eq!(entries[0]["dead"], json!(true), "{entries}");
    assert_eq!(entries[1].get("dead"), None, "{entries}");

    let (mut b, mut schedule_b, [_, _, live_b]) = burning_rig(["a", "b", "live"]);
    restore_world(&mut b, &saved, &(Restored,)).unwrap();
    assert_eq!(queue_counts(&b), queue_counts(&a), "the dead entry kept");
    assert_eq!(
        serde_json::to_value(capture_world(&b).unwrap()).unwrap(),
        text,
        "a load saves what it loaded"
    );
    for run in 0..3 {
        step(&mut a, &mut schedule_a);
        step(&mut b, &mut schedule_b);
        assert_eq!(queue_counts(&b), queue_counts(&a), "run {run}");
        assert_eq!(
            has(&b, live_b, "burning"),
            has(&a, live_a, "burning"),
            "run {run}"
        );
    }
    assert!(has(&b, live_b, "burning"), "after the dead entry's run");
}

/// Kept receivers: `x1` takes the first ID at a high index, `x2` the next
/// at the index `x0` freed; each takes a hit behind one at depth 0.
fn crossed(ids: Option<[tungsten_core::save::SaveId; 2]>) -> (World, [Entity; 2]) {
    let (mut world, mut schedule, [anchor]) = burning_rig(["anchor"]);
    let kept = |world: &mut World, id: Option<tungsten_core::save::SaveId>| {
        let id = id.unwrap_or_else(|| tungsten_core::save::take_id(world).unwrap());
        let entity = receiver(world, &[]);
        world.insert(entity, tungsten_core::save::Saved::kept(id));
        entity
    };
    let receivers = match ids {
        None => {
            let x0 = world.spawn();
            let x1 = kept(&mut world, None);
            world.despawn(x0);
            let x2 = kept(&mut world, None);
            assert!(x2.id() < x1.id(), "the later ID at the lower index");
            [x1, x2]
        }
        Some([id1, id2]) => {
            let x1 = kept(&mut world, Some(id1));
            let x2 = kept(&mut world, Some(id2));
            [x1, x2]
        }
    };
    if ids.is_none() {
        hit(&mut world, anchor, "elemental.fire", 1.0);
        for (&target, amount) in receivers.iter().zip([1.0, 2.0]) {
            DamageRequest::from_parts(
                target,
                [DamagePart::new(amount).with_tags(Tags::new(["elemental.fire"]))],
            )
            .with_depth(1)
            .apply(&mut world);
        }
        step(&mut world, &mut schedule);
        assert_eq!(queue_counts(&world), (2, 1));
    }
    (world, receivers)
}

#[test]
fn a_saved_queue_orders_its_receivers_by_save_id_not_index() {
    let (a, [x1, x2]) = crossed(None);
    let id = |entity| a.get::<tungsten_core::save::Saved>(entity).unwrap().id();
    let saved = capture_world(&a).unwrap();
    let queue = |save: &tungsten_core::save::WorldSave| {
        serde_json::to_value(save).unwrap()["resources"]["chemistry_queue"].clone()
    };
    let refs = |save: &tungsten_core::save::WorldSave| {
        serde_json::to_value(save).unwrap()["resource_refs"]["chemistry_queue"].clone()
    };
    assert_eq!(
        queue(&saved)["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["raw"].as_f64().unwrap())
            .collect::<Vec<_>>(),
        [1.0, 2.0],
        "x1's first, by its ID"
    );
    assert_eq!(
        refs(&saved),
        json!({ "entries.0.receiver": [id(x1).0], "entries.1.receiver": [id(x2).0] })
    );

    let (mut b, _) = crossed(Some([id(x1), id(x2)]));
    restore_world(&mut b, &saved, &(Restored,)).unwrap();
    let again = capture_world(&b).unwrap();
    assert_eq!(queue(&again), queue(&saved));
    assert_eq!(refs(&again), refs(&saved));
}

#[test]
fn a_loaded_queue_without_references_counts_its_dead_entries() {
    // One entry, on a receiver despawned before the capture and with no
    // credit, so the save writes no reference and its load no reference
    // pass.
    let (mut a, mut schedule_a, [first]) = burning_rig(["first"]);
    let doomed = receiver(&mut a, &[]);
    for (target, depth) in [(first, 0), (doomed, 1)] {
        DamageRequest::from_parts(
            target,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_depth(depth)
        .apply(&mut a);
    }
    step(&mut a, &mut schedule_a);
    a.despawn(doomed);
    let saved = capture_world(&a).unwrap();
    assert!(
        serde_json::to_value(&saved).unwrap()["resource_refs"]
            .get("chemistry_queue")
            .is_none()
    );
    let (mut b, mut schedule_b, _) = burning_rig(["first"]);
    restore_world(&mut b, &saved, &(Restored,)).unwrap();
    assert_eq!(queue_counts(&b), queue_counts(&a));
    assert_eq!(queue_counts(&b), (1, 1));
    step(&mut a, &mut schedule_a);
    step(&mut b, &mut schedule_b);
    assert_eq!(queue_counts(&b), (0, 2), "taken, skipped and counted");
    assert_eq!(queue_counts(&b), queue_counts(&a));
}
