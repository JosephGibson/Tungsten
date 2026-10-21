use super::*;
use serde_json::json;
use tungsten_core::{CommandBuffer, ComponentRegistry, DataRegistry, Plugin, Stage};

use crate::damage::effects::{Effects, EffectsPlugin};
use crate::damage::health::HealthPlugin;
use crate::damage::kinds::DamageTypes;
use crate::damage::pipeline::{DamagePart, DamageRequest};
use crate::foundation::stats::StatsPlugin;
use crate::foundation::tags::TagsPlugin;
use crate::foundation::trace::KitTrace;
use crate::{EFFECT_RESOLVE, EFFECT_TICK, KIT_INTENT, KIT_REACT};

const STEP: f32 = 1.0 / 60.0;

/// A World with tags, stats (`ailment_threshold_mult` declared), health
/// and effects, built in `effects_first` order, with the fantasy types and
/// their ladders: `elemental` (shocked, any fill), `elemental.fire`
/// (burning at half), `elemental.cold` (chilled at half, frozen at full
/// when chilled), `toxic` (poisoned, chance one half), `elemental.lightning`
/// (no ladder of its own).
fn rig_with(effects_first: bool) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags([
            "team.enemy",
            "elemental.fire",
            "elemental.cold",
            "elemental.lightning",
            "toxic",
            "status.chilled",
            "status.frozen",
            "status.burning",
            "status.poisoned",
            "status.shocked",
        ])
        .build(&mut schedule, &mut world);
    let stats = StatsPlugin::new().with_stat("ailment_threshold_mult", 0.0, 10.0);
    HealthPlugin.build(&mut schedule, &mut world);
    if effects_first {
        EffectsPlugin.build(&mut schedule, &mut world);
        stats.build(&mut schedule, &mut world);
    } else {
        stats.build(&mut schedule, &mut world);
        EffectsPlugin.build(&mut schedule, &mut world);
    }
    schedule.resolve().unwrap();
    let data = world.resource_mut::<DataRegistry>();
    for (id, effect) in [
        (
            "shocked",
            Effect::new()
                .with_duration(1.0)
                .with_grant(["status.shocked"]),
        ),
        (
            "burning",
            Effect::new()
                .with_duration(3.0)
                .with_grant(["status.burning"]),
        ),
        (
            "chilled",
            Effect::new()
                .with_duration(4.0)
                .with_grant(["status.chilled"]),
        ),
        (
            "frozen",
            Effect::new()
                .with_duration(2.0)
                .with_grant(["status.frozen"]),
        ),
        (
            "poisoned",
            Effect::new()
                .with_duration(3.0)
                .with_grant(["status.poisoned"]),
        ),
    ] {
        data.insert(id, effect);
    }
    let types: DamageTypes = serde_json::from_value(json!({
        "elemental": { "ailments": [{ "effect": "shocked" }] },
        "elemental.fire": { "ailments": [{ "effect": "burning", "needs": 0.5, "window": 4 }] },
        "elemental.cold": { "ailments": [
            { "effect": "chilled", "needs": 0.5, "window": 4 },
            { "effect": "frozen", "needs": 1, "requires": ["status.chilled"], "window": 4 }
        ] },
        "elemental.lightning": {},
        "toxic": { "ailments": [{ "effect": "poisoned", "chance": 0.5 }] }
    }))
    .unwrap();
    data.insert("fantasy", types);
    (world, schedule)
}

fn rig() -> (World, Schedule) {
    rig_with(false)
}

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

/// A golem of 100 health with a flat threshold of 10.
fn spawn_golem(world: &mut World) -> Entity {
    world.spawn_with((
        Health::new(100.0),
        AilmentThreshold::new(10.0),
        Tags::new(["team.enemy"]),
    ))
}

fn typed(amount: f32, name: &str) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new([name]))
}

/// Queues a hit of `amount` of `kind` on `target` for the step's resolve.
fn hit(world: &mut World, target: Entity, kind: &str, amount: f32) {
    assert!(
        DamageRequest::from_parts(target, [typed(amount, kind)]).queue(world),
        "the queue"
    );
}

fn has(world: &World, entity: Entity, id: &str) -> bool {
    world
        .get::<Effects>(entity)
        .is_some_and(|effects| effects.has(id))
}

fn stacks(world: &World, entity: Entity, id: &str) -> u32 {
    world
        .get::<Effects>(entity)
        .map_or(0, |effects| effects.stacks(id))
}

fn fill(world: &World, entity: Entity, name: &str) -> f32 {
    world
        .get::<AilmentBuckets>(entity)
        .map_or(0.0, |buckets| buckets.fill(name))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_feed_and_the_drain_sit_in_their_phases_with_their_edges() {
    let (world, schedule) = rig();
    assert!(
        world
            .resource::<ComponentRegistry>()
            .names()
            .any(|n| n == AILMENT_THRESHOLD)
    );
    let names: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
    assert!(at(STATS_APPLY) < at(AILMENT_TICK) && at(EFFECT_TICK) < at(AILMENT_TICK));
    assert!(at(AILMENT_TICK) < at(KIT_INTENT));
    assert!(at(DAMAGE_RESOLVE) < at(AILMENT_FEED) && at(AILMENT_FEED) < at(EFFECT_RESOLVE));
    assert!(at(EFFECT_RESOLVE) < at(KIT_REACT));
}

#[test]
fn the_ladder_climbs_from_the_top_rung_down_and_the_first_rung_empties_the_bucket() {
    let (mut world, mut schedule) = rig();
    let golem = spawn_golem(&mut world);
    // A full bucket with no `status.chilled`: the frozen rung fails its
    // `requires`, the chilled rung takes it and empties the bucket.
    hit(&mut world, golem, "elemental.cold", 10.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, golem, "chilled") && !has(&world, golem, "frozen"));
    assert_eq!(fill(&world, golem, "elemental.cold"), 0.0, "emptied");
    // The next full bucket, `status.chilled` granted, freezes.
    hit(&mut world, golem, "elemental.cold", 10.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, golem, "frozen"));
    assert!(world.get::<Tags>(golem).unwrap().has("status.frozen"));
    assert_eq!(
        stacks(&world, golem, "chilled"),
        1,
        "the chilled stack is untouched"
    );
    // A half bucket with `status.chilled`: frozen needs a full one, so
    // chilled again.
    let applied_before = world.resource::<KitTrace>().about(golem).count();
    hit(&mut world, golem, "elemental.cold", 5.0);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(fill(&world, golem, "elemental.cold"), 0.0);
    assert!(world.resource::<KitTrace>().about(golem).count() > applied_before);
    let ailments: Vec<f32> = world
        .resource::<KitTrace>()
        .about(golem)
        .filter_map(|entry| match entry.kind {
            TraceKind::Event {
                name: "ailment",
                amount,
                ..
            } => amount,
            _ => None,
        })
        .collect();
    assert_eq!(ailments, [10.0, 10.0, 5.0]);
    // Below the half, the bucket keeps its fill and nothing applies.
    let fresh = spawn_golem(&mut world);
    hit(&mut world, fresh, "elemental.cold", 4.0);
    steps(&mut world, &mut schedule, 1);
    assert!(world.get::<Effects>(fresh).is_none());
    assert_eq!(
        fill(&world, fresh, "elemental.cold"),
        4.0,
        "filled after this step's tick, so no drain yet"
    );
    steps(&mut world, &mut schedule, 1);
    assert!(close(
        fill(&world, fresh, "elemental.cold"),
        4.0 - 10.0 / 4.0 * STEP
    ));
    // The request carries the part's source and the fill over the
    // threshold as its scale.
    let mage = world.spawn();
    let scaled = spawn_golem(&mut world);
    assert!(
        DamageRequest::from_parts(scaled, [typed(15.0, "elemental.cold")])
            .with_source(mage)
            .queue(&mut world)
    );
    steps(&mut world, &mut schedule, 1);
    let instance = world
        .get::<Effects>(scaled)
        .unwrap()
        .instances()
        .next()
        .unwrap();
    assert_eq!(instance.source(), Some(mage));
    assert_eq!(instance.factor(), 1.0, "the effect has no scale curve");
}

#[test]
fn chance_draws_only_after_needs_and_requires_pass_in_event_order_under_a_seed() {
    let outcome = |seed: u64| -> (Vec<bool>, u64) {
        let (mut world, mut schedule) = rig();
        world.insert_resource(KitRng::seeded(seed));
        let targets: Vec<Entity> = (0..8).map(|_| spawn_golem(&mut world)).collect();
        for &target in &targets {
            hit(&mut world, target, "toxic", 1.0);
        }
        // A rung with `needs` on a target below it, and one whose
        // `requires` fails, draw nothing: the cold ladder's rungs want a
        // half bucket and `status.chilled`.
        let unreached = spawn_golem(&mut world);
        hit(&mut world, unreached, "elemental.cold", 1.0);
        steps(&mut world, &mut schedule, 1);
        let poisoned = targets
            .iter()
            .map(|&target| has(&world, target, "poisoned"))
            .collect();
        (poisoned, world.resource::<KitRng>().draws())
    };
    let (a, draws) = outcome(11);
    let (b, _) = outcome(11);
    let (c, _) = outcome(12);
    assert_eq!(a, b, "one seed, one outcome");
    assert_ne!(a, c);
    assert!(
        a.iter().any(|&p| p) && a.iter().any(|&p| !p),
        "a spread: {a:?}"
    );
    assert_eq!(
        draws, 8,
        "one draw per reached rung, none for the unreached"
    );
}

#[test]
fn two_full_buckets_in_one_step_chill_then_freeze_as_split_hits_do() {
    let (mut world, mut schedule) = rig();
    let batched = spawn_golem(&mut world);
    hit(&mut world, batched, "elemental.cold", 10.0);
    hit(&mut world, batched, "elemental.cold", 10.0);
    let split = spawn_golem(&mut world);
    hit(&mut world, split, "elemental.cold", 10.0);
    steps(&mut world, &mut schedule, 1);
    hit(&mut world, split, "elemental.cold", 10.0);
    steps(&mut world, &mut schedule, 1);
    for target in [batched, split] {
        assert!(has(&world, target, "chilled") && has(&world, target, "frozen"));
        let tags = world.get::<Tags>(target).unwrap();
        assert!(tags.has("status.chilled") && tags.has("status.frozen"));
        assert_eq!(fill(&world, target, "elemental.cold"), 0.0);
    }
}

/// The rig's cold ladder alone, its chilled rung `blocked_by` its own tag
/// when `blocked`.
fn cold_ladder(world: &mut World, blocked: bool) {
    let mut chilled = json!({ "effect": "chilled", "needs": 0.5, "window": 4 });
    if blocked {
        chilled["blocked_by"] = json!(["status.chilled"]);
    }
    let types: DamageTypes = serde_json::from_value(json!({
        "elemental.cold": { "ailments": [
            chilled,
            { "effect": "frozen", "needs": 1, "requires": ["status.chilled"], "window": 4 }
        ] }
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("fantasy", types);
}

#[test]
fn a_steady_stream_climbs_past_a_blocked_rung() {
    // A spray's pulses, each a tenth of the threshold: the chilled rung
    // passes at half and empties the bucket every time, so without
    // `blocked_by` the stream never fills a bucket to the frozen rung.
    let stream = |blocked: bool| {
        let (mut world, mut schedule) = rig();
        cold_ladder(&mut world, blocked);
        let golem = spawn_golem(&mut world);
        let mut frozen_at = None;
        for step in 0..60 {
            hit(&mut world, golem, "elemental.cold", 1.0);
            steps(&mut world, &mut schedule, 1);
            if frozen_at.is_none() && has(&world, golem, "frozen") {
                frozen_at = Some(step);
            }
        }
        (has(&world, golem, "chilled"), frozen_at)
    };
    let (chilled, frozen_at) = stream(false);
    assert!(chilled && frozen_at.is_none(), "chilled, never frozen");
    let (chilled, frozen_at) = stream(true);
    assert!(chilled, "the chill holds");
    let frozen_at = frozen_at.expect("the cold climbs past the held chill to frozen");
    assert!(
        (14..20).contains(&frozen_at),
        "about five pulses to chill, then ten more to freeze: step {frozen_at}"
    );
}

#[test]
fn a_pending_grant_skips_a_blocked_rung() {
    let (mut world, mut schedule) = rig();
    let types: DamageTypes = serde_json::from_value(json!({
        "elemental.fire": { "ailments": [
            { "effect": "burning", "needs": 0.5, "window": 4, "blocked_by": ["status.frozen"] }
        ] },
        "elemental.cold": { "ailments": [
            { "effect": "frozen", "needs": 0.5, "window": 4, "blocked_by": ["status.burning"] }
        ] }
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("fantasy", types);
    // Cold, then fire, in one step: the freeze the feed queued for the
    // first part skips the second's burning, whose bucket keeps its fill.
    let golem = spawn_golem(&mut world);
    hit(&mut world, golem, "elemental.cold", 10.0);
    hit(&mut world, golem, "elemental.fire", 10.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, golem, "frozen") && !has(&world, golem, "burning"));
    assert_eq!(fill(&world, golem, "elemental.fire"), 10.0, "filling on");
    // Fire, then cold: the other way round.
    let other = spawn_golem(&mut world);
    hit(&mut world, other, "elemental.fire", 10.0);
    hit(&mut world, other, "elemental.cold", 10.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, other, "burning") && !has(&world, other, "frozen"));
    assert_eq!(fill(&world, other, "elemental.cold"), 10.0);
    // Carried now, the freeze skips the next step's fire too.
    hit(&mut world, golem, "elemental.fire", 10.0);
    steps(&mut world, &mut schedule, 1);
    assert!(!has(&world, golem, "burning"));
    assert!(world.get::<Tags>(golem).unwrap().has("status.frozen"));
    // A rung blocked by an ancestor meets a pending grant under it, as it
    // meets a carried one.
    let types: DamageTypes = serde_json::from_value(json!({
        "elemental.cold": { "ailments": [{ "effect": "frozen", "needs": 0.5, "window": 4 }] },
        "toxic": { "ailments": [{ "effect": "poisoned", "blocked_by": ["status"] }] }
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("fantasy", types);
    let third = spawn_golem(&mut world);
    hit(&mut world, third, "elemental.cold", 10.0);
    hit(&mut world, third, "toxic", 1.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, third, "frozen") && !has(&world, third, "poisoned"));
}

#[test]
fn an_inherited_type_feeds_its_ancestors_bucket_and_a_periodic_part_feeds_nothing() {
    let (mut world, mut schedule) = rig();
    let golem = spawn_golem(&mut world);
    // Lightning sets no ladder: its ancestor `elemental`'s takes the part,
    // and the bucket is named for the ancestor.
    hit(&mut world, golem, "elemental.lightning", 1.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, golem, "shocked"));
    assert_eq!(fill(&world, golem, "elemental.lightning"), 0.0);
    let buckets = world.get::<AilmentBuckets>(golem).unwrap();
    assert_eq!(
        buckets.iter().count(),
        0,
        "the shocked rung emptied `elemental`"
    );
    // A periodic part feeds nothing; an untyped one neither.
    let other = spawn_golem(&mut world);
    assert!(
        DamageRequest::from_parts(other, [typed(4.0, "elemental.cold")])
            .with_periodic(true)
            .queue(&mut world)
    );
    assert!(DamageRequest::new(other, 4.0).queue(&mut world));
    steps(&mut world, &mut schedule, 1);
    assert!(world.get::<AilmentBuckets>(other).is_none());
    assert_eq!(world.get::<Health>(other).unwrap().current(), 92.0);
}

#[test]
fn buckets_drain_by_the_first_needs_rungs_window_and_follow_a_raised_max_health() {
    for (dt, count) in [(1.0 / 60.0, 60), (1.0 / 30.0, 30)] {
        let (mut world, mut schedule) = rig();
        // The threshold is the full health: 100, draining over the cold
        // ladder's 4 s window at 25 a second.
        let golem = world.spawn_with((
            Health::new(100.0),
            AilmentThreshold::new(0.0).with_of_max_health(1.0),
        ));
        hit(&mut world, golem, "elemental.cold", 40.0);
        steps(&mut world, &mut schedule, 1);
        assert!(world.get::<Effects>(golem).is_none(), "below the half");
        for _ in 0..count {
            step(&mut world, &mut schedule, dt);
        }
        assert!(
            close(fill(&world, golem, "elemental.cold"), 40.0 - 25.0),
            "at dt {dt}: {}",
            fill(&world, golem, "elemental.cold")
        );
        for _ in 0..count {
            step(&mut world, &mut schedule, dt);
        }
        assert_eq!(
            fill(&world, golem, "elemental.cold"),
            0.0,
            "drained away and gone"
        );
        assert_eq!(
            world.get::<AilmentBuckets>(golem).unwrap().iter().count(),
            0
        );
    }
    // A `max_health` stat raised mid-run moves the threshold the same step
    // under both plugin orders, since the drain reads the max the stat
    // wrote.
    let mut fills = Vec::new();
    for effects_first in [true, false] {
        let (mut world, mut schedule) = rig_with(effects_first);
        let golem = world.spawn_with((
            Health::new(100.0),
            AilmentThreshold::new(0.0).with_of_max_health(1.0),
            Stats::new().with(Stats::MAX_HEALTH, 100.0),
        ));
        hit(&mut world, golem, "elemental.cold", 40.0);
        steps(&mut world, &mut schedule, 1);
        world
            .get_mut::<Stats>(golem)
            .unwrap()
            .set_base(Stats::MAX_HEALTH, 200.0);
        steps(&mut world, &mut schedule, 1);
        fills.push(fill(&world, golem, "elemental.cold"));
    }
    assert!(close(fills[0], fills[1]), "{fills:?}");
    assert!(
        close(fills[0], 40.0 - 50.0 * STEP),
        "the raised max drains at the new rate the same step: {fills:?}"
    );
}

#[test]
fn a_threshold_of_0_or_none_skips_the_needs_rungs_and_the_stat_multiplies() {
    let (mut world, mut schedule) = rig();
    let bare = world.spawn_with((Health::new(100.0),));
    let zero = world.spawn_with((Health::new(100.0), AilmentThreshold::new(0.0)));
    for target in [bare, zero] {
        hit(&mut world, target, "elemental.cold", 100.0);
        hit(&mut world, target, "toxic", 1.0);
    }
    world.insert_resource(KitRng::seeded(1));
    steps(&mut world, &mut schedule, 1);
    for target in [bare, zero] {
        assert!(!has(&world, target, "chilled"), "needs rungs skipped");
        assert_eq!(
            fill(&world, target, "elemental.cold"),
            100.0,
            "nothing drains either"
        );
    }
    // Rungs without `needs` still apply by their chance, with scale 1.
    let poisoned = [bare, zero]
        .iter()
        .filter(|&&t| has(&world, t, "poisoned"))
        .count();
    assert!(poisoned <= 2);
    // The multiplier: twice the threshold needs twice the fill.
    let tough = world.spawn_with((
        Health::new(100.0),
        AilmentThreshold::new(10.0),
        Stats::new().with("ailment_threshold_mult", 2.0),
    ));
    hit(&mut world, tough, "elemental.cold", 9.0);
    steps(&mut world, &mut schedule, 1);
    assert!(!has(&world, tough, "chilled"), "under half of 20");
    hit(&mut world, tough, "elemental.cold", 11.0);
    steps(&mut world, &mut schedule, 1);
    assert!(has(&world, tough, "chilled"), "past half now");
    assert!(
        !has(&world, tough, "frozen"),
        "the drain kept it under the full 20"
    );
    let frail = world.spawn_with((
        Health::new(100.0),
        AilmentThreshold::new(10.0),
        Stats::new().with("ailment_threshold_mult", 0.0),
    ));
    hit(&mut world, frail, "elemental.cold", 100.0);
    steps(&mut world, &mut schedule, 1);
    assert!(
        !has(&world, frail, "chilled"),
        "a multiplier of 0 is a threshold of 0"
    );
}

#[test]
fn the_buckets_save_as_fills_and_show_as_rows_and_the_threshold_round_trips() {
    let (mut world, mut schedule) = rig();
    let golem = spawn_golem(&mut world);
    hit(&mut world, golem, "elemental.cold", 4.0);
    hit(&mut world, golem, "elemental.fire", 2.0);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(
        serde_json::to_value(world.get::<AilmentBuckets>(golem).unwrap()).unwrap(),
        json!({ "elemental.cold": 4.0, "elemental.fire": 2.0 }),
        "whole after the hit's step"
    );
    steps(&mut world, &mut schedule, 1);
    let buckets = world.get::<AilmentBuckets>(golem).unwrap().clone();
    let saved = serde_json::to_value(&buckets).unwrap();
    let cold = saved["elemental.cold"].as_f64().unwrap() as f32;
    let fire = saved["elemental.fire"].as_f64().unwrap() as f32;
    assert!(
        close(cold, 4.0 - 2.5 * STEP) && close(fire, 2.0 - 2.5 * STEP),
        "{saved}"
    );
    assert_eq!(
        serde_json::from_value::<AilmentBuckets>(saved).unwrap(),
        buckets
    );
    let rows = buckets.inspect_rows();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].1.starts_with("elemental.cold 3.9"), "{}", rows[0].1);
    assert!(serde_json::from_value::<AilmentBuckets>(json!({ "Bad": 1 })).is_err());
    assert!(serde_json::from_value::<AilmentBuckets>(json!({ "toxic": 0 })).is_err());
    let threshold = AilmentThreshold::new(20.0).with_of_max_health(0.25);
    let saved = serde_json::to_value(threshold).unwrap();
    assert_eq!(saved, json!({ "flat": 20.0, "of_max_health": 0.25 }));
    assert_eq!(
        serde_json::from_value::<AilmentThreshold>(saved).unwrap(),
        threshold
    );
    assert!(serde_json::from_value::<AilmentThreshold>(json!({ "flat": -1 })).is_err());
    assert_eq!(
        threshold.inspect_rows(),
        [("ailment threshold", "20 + 0.25 of max health".to_string())]
    );
    assert!(std::panic::catch_unwind(|| AilmentThreshold::new(-1.0)).is_err());
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(
                AILMENT_THRESHOLD,
                &json!({ "flat": 20, "of_max_health": 0.1 }),
                &world
            )
            .is_ok()
    );
    assert!(
        components
            .check_in(
                AILMENT_THRESHOLD,
                &json!({ "flat": 20, "extra": 1 }),
                &world
            )
            .is_err()
    );
}

#[test]
fn a_rung_refuses_bad_values_and_builds_as_its_data_form() {
    for bad in [
        json!({ "effect": "x", "needs": 0, "window": 1 }),
        json!({ "effect": "x", "needs": 1 }),
        json!({ "effect": "x", "window": 0 }),
        json!({ "effect": "x", "chance": 0 }),
        json!({ "effect": "x", "chance": 1.5 }),
        json!({ "effect": "x", "requires": ["Bad"] }),
        json!({ "needs": 1, "window": 1 }),
        json!({ "effect": "x", "extra": 1 }),
    ] {
        assert!(
            serde_json::from_value::<AilmentRung>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let rung: AilmentRung = serde_json::from_value(json!({
        "effect": "frozen", "needs": 1, "requires": ["status.chilled"], "chance": 0.5, "window": 4
    }))
    .unwrap();
    assert_eq!(
        rung,
        AilmentRung::new(DataRef::by_id("frozen"))
            .with_needs(1.0, 4.0)
            .with_requires(["status.chilled"])
            .with_chance(0.5)
    );
    let back = serde_json::to_value(&rung).unwrap();
    assert_eq!(back["requires"], json!(["status.chilled"]));
    assert_eq!(serde_json::from_value::<AilmentRung>(back).unwrap(), rung);
    let plain = AilmentRung::new(DataRef::inline(Effect::new()));
    assert_eq!(
        (plain.needs(), plain.window(), plain.chance()),
        (None, None, 1.0)
    );
    assert!(plain.requires().is_empty() && plain.effect().as_inline().is_some());
    assert!(
        std::panic::catch_unwind(|| AilmentRung::new(DataRef::by_id("x")).with_chance(0.0))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| AilmentRung::new(DataRef::by_id("x")).with_needs(1.0, 0.0))
            .is_err()
    );
}
