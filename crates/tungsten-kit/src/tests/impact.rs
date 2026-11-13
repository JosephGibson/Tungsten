use glam::Vec2;
use serde_json::json;
use tungsten_core::physics::{Collider, Position, RigidBody, RigidBodyBundle, Velocity};
use tungsten_core::{CommandBuffer, EventQueue, Plugin, Schedule, Stage, Time};

use super::*;
use crate::damage::defense::Resist;
use crate::damage::health::{Damaged, DamagedPart, HealthPlugin};
use crate::damage::kinds::DamageTypes;
use crate::foundation::source::SourcePlugin;
use crate::foundation::stats::{Stats, StatsPlugin};
use crate::foundation::tags::TagsPlugin;
use crate::motion::knockback::{Knockback, KnockbackPlugin};
use crate::{KitRng, Penetration};

/// What a restored spawn takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Restored;

/// A World with tags, stats, health, sources, knockback and impacts, the
/// damage types `physical` and `fire`, and no physics: bodies stand where
/// a test puts them, with the velocities it gives.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["status.burning", "creature", "ghost"])
        .build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    SourcePlugin.build(&mut schedule, &mut world);
    KnockbackPlugin.build(&mut schedule, &mut world);
    ImpactDamagePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: DamageTypes = serde_json::from_value(json!({ "physical": {}, "fire": {} })).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    (world, schedule)
}

/// One fixed step of 1/60 s and the frame's flushes.
fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// A dynamic hitter of mass `mass`, a circle of radius 10 at the origin,
/// moving at `velocity`, holding `rule`.
fn hitter(world: &mut World, rule: ImpactDamage, mass: f32, velocity: Vec2) -> Entity {
    let entity = world.spawn_with(RigidBodyBundle::dynamic(
        Position(Vec2::ZERO),
        Collider::circle(10.0),
    ));
    *world.get_mut::<RigidBody>(entity).unwrap() = RigidBody::dynamic().with_mass(mass);
    world.get_mut::<Velocity>(entity).unwrap().0 = velocity;
    world.insert(entity, rule);
    entity
}

/// A target with 10,000 health and a knockback, a kinematic circle of
/// radius 10 at `at`, moving at `velocity`.
fn target(world: &mut World, at: Vec2, velocity: Vec2) -> Entity {
    let entity = world.spawn_with(RigidBodyBundle::kinematic(
        Position(at),
        Collider::circle(10.0),
    ));
    world.get_mut::<Velocity>(entity).unwrap().0 = velocity;
    world.insert(entity, Health::new(10_000.0));
    world.insert(entity, Knockback::new(1.0));
    entity
}

/// The hits the step sent: amount, source and crit.
fn hits(world: &World) -> Vec<(Entity, f32, Option<Entity>, bool)> {
    world
        .resource::<EventQueue<Damaged>>()
        .iter()
        .map(|hit| (hit.entity, hit.amount, hit.source, hit.crit))
        .collect()
}

/// The parts the step landed: amount and deepest type.
fn parts(world: &World) -> Vec<(f32, String)> {
    world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .map(|part| {
            let names = part.kinds.names(world);
            (part.amount, names.last().copied().unwrap_or("").to_owned())
        })
        .collect()
}

/// The amount one step's hit deals: `rule` on a hitter of mass 4 closing at
/// `closing` on a still target that it touches; `None` for no hit.
fn dealt(rule: ImpactDamage, closing: f32) -> Option<f32> {
    let (mut world, mut schedule) = rig();
    hitter(&mut world, rule, 4.0, Vec2::new(closing, 0.0));
    target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    step(&mut world, &mut schedule);
    let hits = hits(&world);
    assert!(hits.len() <= 1, "{hits:?}");
    hits.first().map(|hit| hit.1)
}

fn near(a: Option<f32>, b: f32) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-3)
}

#[test]
fn a_body_hurts_what_it_touches_by_its_closing_speed() {
    let crate_ = || ImpactDamage::new().with_floor(2.0).with_most(120.0);
    assert_eq!(dealt(crate_(), 600.0), None, "1.6, under the floor");
    assert!(near(dealt(crate_(), 1000.0), 4.8));
    assert!(near(dealt(crate_(), 100_000.0), 120.0), "at its most");
    assert!(
        near(dealt(crate_().with_mass(8.0), 1000.0), 9.6),
        "its mass over the body's"
    );
    assert!(near(dealt(crate_().with_amount(7.0), 450.0), 7.0), "flat");
    assert_eq!(
        dealt(crate_().with_amount(7.0), 350.0),
        None,
        "under its speed"
    );
    assert_eq!(
        dealt(crate_().with_scale(1.5), 600.0),
        None,
        "the floor before the scale"
    );
    assert!(near(dealt(crate_().with_scale(1.5), 1000.0), 7.2));
    assert_eq!(dealt(crate_().with_scale(0.0), 1000.0), None, "inert");
    assert_eq!(ImpactDamage::new().amount(4.0, 1000.0), 0.002 * 4.0 * 600.0);

    // Typed, with its extra parts while the hitter's tags pass.
    let burning = || {
        crate_().with_kind("physical").with_extra(
            TagFilter::new().all(["status.burning"]),
            "fire",
            12.0,
        )
    };
    let (mut world, mut schedule) = rig();
    let body = hitter(&mut world, burning(), 4.0, Vec2::new(1000.0, 0.0));
    target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    step(&mut world, &mut schedule);
    let landed = parts(&world);
    assert_eq!(landed.len(), 1, "unlit: {landed:?}");
    assert!((landed[0].0 - 4.8).abs() < 1e-3 && landed[0].1 == "physical");
    world.insert(body, Tags::new(["status.burning"]));
    step(&mut world, &mut schedule);
    let landed = parts(&world);
    assert_eq!(landed.len(), 2, "lit: {landed:?}");
    assert!((landed[1].0 - 12.0).abs() < 1e-3 && landed[1].1 == "fire");

    // Pushes: away by a share of the closing speed to its most, along its
    // flight.
    for (rule, closing, want) in [
        (
            crate_().with_push_away(0.6, Some(900.0)),
            1000.0,
            Vec2::new(600.0, 0.0),
        ),
        (
            crate_().with_push_away(0.6, Some(900.0)),
            2000.0,
            Vec2::new(900.0, 0.0),
        ),
        (
            crate_().with_push_along(260.0),
            1000.0,
            Vec2::new(260.0, 0.0),
        ),
    ] {
        let (mut world, mut schedule) = rig();
        hitter(&mut world, rule, 4.0, Vec2::new(closing, 0.0));
        let hit = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
        step(&mut world, &mut schedule);
        let push = world.get::<Knockback>(hit).unwrap().push();
        assert!((push - want).length() < 1e-2, "{closing}: {push}");
    }

    // Credited to its credit while that lives, else to its source; never
    // the one it is credited to.
    let (mut world, mut schedule) = rig();
    let thrower = world.spawn();
    let origin = world.spawn();
    let mut rule = crate_();
    rule.set_credit(Some(thrower));
    let body = hitter(&mut world, rule, 4.0, Vec2::new(1000.0, 0.0));
    world.insert(body, Source::new(origin));
    let hit = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    step(&mut world, &mut schedule);
    assert_eq!(hits(&world)[0].2, Some(thrower));
    world.despawn(thrower);
    step(&mut world, &mut schedule);
    assert_eq!(
        hits(&world)[0].2,
        Some(origin),
        "its source once it is gone"
    );
    world
        .get_mut::<ImpactDamage>(body)
        .unwrap()
        .set_credit(Some(hit));
    step(&mut world, &mut schedule);
    assert!(hits(&world).is_empty(), "never the one it credits");

    // One hit a pair a step, a refused target, a dead one, and one moving
    // off at its speed: none.
    let (mut world, mut schedule) = rig();
    let mut picky = crate_();
    picky.set_filter(TagFilter::new().all(["creature"]));
    hitter(&mut world, picky, 4.0, Vec2::new(1000.0, 0.0));
    let beast = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    world.insert(beast, Tags::new(["creature"]));
    let wraith = target(&mut world, Vec2::new(0.0, 20.0), Vec2::ZERO);
    world.insert(wraith, Tags::new(["ghost"]));
    step(&mut world, &mut schedule);
    assert_eq!(
        hits(&world).iter().map(|hit| hit.0).collect::<Vec<_>>(),
        [beast],
        "once, and the ghost refused"
    );
    world.get_mut::<Health>(beast).unwrap().hurt(20_000.0, true);
    step(&mut world, &mut schedule);
    assert!(hits(&world).is_empty(), "the dead");
    let (mut world, mut schedule) = rig();
    hitter(&mut world, crate_(), 4.0, Vec2::new(1000.0, 0.0));
    target(&mut world, Vec2::new(20.0, 0.0), Vec2::new(1000.0, 0.0));
    step(&mut world, &mut schedule);
    assert!(hits(&world).is_empty(), "moving off at its speed");
    // Out of its margin: none; within it: one.
    for (gap, want) in [(4.0, 0), (2.5, 1)] {
        let (mut world, mut schedule) = rig();
        hitter(&mut world, crate_(), 4.0, Vec2::new(1000.0, 0.0));
        target(&mut world, Vec2::new(20.0 + gap, 0.0), Vec2::ZERO);
        step(&mut world, &mut schedule);
        assert_eq!(hits(&world).len(), want, "a gap of {gap}");
    }
}

#[test]
fn a_kinematic_hitter_hurts_with_its_mass() {
    for (mass, want) in [(Some(6.0), Some(7.2)), (None, None)] {
        let (mut world, mut schedule) = rig();
        let body = world.spawn_with(RigidBodyBundle::kinematic(
            Position(Vec2::ZERO),
            Collider::circle(10.0),
        ));
        world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(1000.0, 0.0);
        let rule = ImpactDamage::new();
        world.insert(
            body,
            match mass {
                Some(mass) => rule.with_mass(mass),
                None => rule,
            },
        );
        target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
        step(&mut world, &mut schedule);
        let dealt = hits(&world).first().map(|hit| hit.1);
        match want {
            Some(want) => assert!(near(dealt, want), "{dealt:?}"),
            None => assert_eq!(dealt, None, "a kinematic body without a mass"),
        }
    }
}

/// The first seed whose first draws are `rolls`, each under 0.5 or not.
fn seed_rolling(rolls: &[bool]) -> u64 {
    (0..10_000)
        .find(|&seed| {
            let mut rng = KitRng::seeded(seed);
            rolls.iter().all(|&crit| (rng.unit() < 0.5) == crit)
        })
        .unwrap()
}

#[test]
fn impacts_build_in_save_id_order_and_roll_at_build() {
    let (mut world, mut schedule) = rig();
    let anchor = world.spawn();
    tungsten_core::save::name(&mut world, anchor, None, "anchor").unwrap();
    let kept = |world: &mut World, at: Vec2| {
        let id = tungsten_core::save::take_id(world).unwrap();
        let entity = target(world, at, Vec2::ZERO);
        world.insert(entity, tungsten_core::save::Saved::kept(id));
        entity
    };
    // Two targets saved in the order of their IDs, the later at the lower
    // index; a hitter closing on both.
    let x0 = world.spawn();
    let first = kept(&mut world, Vec2::new(20.0, 0.0));
    world.despawn(x0);
    let second = kept(&mut world, Vec2::new(14.0, 14.0));
    assert!(second.id() < first.id());
    let lucky = world.spawn_with((Stats::new()
        .with(Stats::CRIT_CHANCE, 0.5)
        .with(Stats::CRIT_MULTIPLIER, 2.0),));
    let mut rule = ImpactDamage::new();
    rule.set_credit(Some(lucky));
    hitter(&mut world, rule, 4.0, Vec2::new(1000.0, 1000.0));
    world.insert_resource(KitRng::seeded(seed_rolling(&[true, false])));
    step(&mut world, &mut schedule);
    let order: Vec<(Entity, bool)> = hits(&world).iter().map(|hit| (hit.0, hit.3)).collect();
    assert_eq!(
        order,
        [(first, true), (second, false)],
        "the first by its ID, its crit drawn first"
    );
    assert_eq!(
        world.resource::<KitRng>().draws(),
        2,
        "drawn at the build, none at the resolve"
    );
}

/// Adds a whole fire penetration to every hit it prepares.
fn piercing(_: &World, hit: &mut DamageRequest) {
    let taken = std::mem::replace(hit, DamageRequest::new(hit.target(), 0.0));
    *hit = taken.with_penetration(Penetration::new().with_points("fire", 1.0));
}

#[test]
fn an_impact_lands_now_or_queued_and_reports_its_outcome() {
    let impacted = |world: &World| -> Vec<(Entity, Entity, Option<Hit>)> {
        world
            .resource::<EventQueue<Impacted>>()
            .iter()
            .map(|event| (event.hitter, event.target, event.outcome))
            .collect()
    };
    // Queued: no outcome, the hit at the step's resolve.
    let (mut world, mut schedule) = rig();
    let body = hitter(&mut world, ImpactDamage::new(), 4.0, Vec2::new(1000.0, 0.0));
    let hit = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    step(&mut world, &mut schedule);
    assert_eq!(impacted(&world), [(body, hit, None)]);
    assert_eq!(hits(&world).len(), 1, "landed at the resolve");
    // Now: its outcome, a kill among them.
    let (mut world, mut schedule) = rig();
    let body = hitter(
        &mut world,
        ImpactDamage::new().landing_now(),
        4.0,
        Vec2::new(1000.0, 0.0),
    );
    let hit = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    let frail = target(&mut world, Vec2::new(0.0, -20.0), Vec2::ZERO);
    *world.get_mut::<Health>(frail).unwrap() = Health::new(1.0);
    world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(1000.0, -1000.0);
    step(&mut world, &mut schedule);
    let events = impacted(&world);
    assert!(events.contains(&(body, hit, Some(Hit::Hurt))), "{events:?}");
    assert!(
        events.contains(&(body, frail, Some(Hit::Killed))),
        "{events:?}"
    );
    // The named preparation runs on each hit: the fire resisted by half
    // lands whole; an unknown name refuses the hit.
    let (mut world, mut schedule) = rig();
    crate::add_hit_prep(&mut world, "piercing", piercing);
    let rule = ImpactDamage::new()
        .with_kind("fire")
        .with_amount(10.0)
        .with_prepare("piercing");
    hitter(&mut world, rule, 4.0, Vec2::new(1000.0, 0.0));
    let guarded = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
    world.insert(guarded, Resist::new().with_factor("fire", 0.5));
    step(&mut world, &mut schedule);
    assert!(
        (hits(&world)[0].1 - 10.0).abs() < 1e-4,
        "{:?}",
        hits(&world)
    );
    let unknown: Result<ImpactDamage, _> = serde_json::from_value(json!({ "prepare": "missing" }));
    let err = format!("{:?}", unknown.unwrap().check(&world).unwrap_err());
    assert!(err.contains("missing"), "{err}");
}

#[test]
fn impact_damage_keeps_nothing_without_a_holder() {
    use tungsten_core::physics::{PhysicsConfig, PhysicsPlugin};

    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    crate::HitPlugin.build(&mut schedule, &mut world);
    ImpactDamagePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    // Bodies thrown at targets every step, none holding a rule.
    let mut bodies = Vec::new();
    for i in 0..4 {
        let y = i as f32 * 40.0;
        let body = world.spawn_with(RigidBodyBundle::dynamic(
            Position(Vec2::new(0.0, y)),
            Collider::circle(5.0),
        ));
        bodies.push(body);
        let wall = world.spawn_with(RigidBodyBundle::r#static(
            Position(Vec2::new(30.0, y)),
            Collider::circle(10.0),
        ));
        world.insert(wall, Health::new(100.0));
    }
    let mut struck = 0;
    let mut impacts = 0;
    for _ in 0..60 {
        for &body in &bodies {
            world.get_mut::<Velocity>(body).unwrap().0 = Vec2::new(900.0, 0.0);
        }
        step(&mut world, &mut schedule);
        struck += world.resource::<EventQueue<crate::Struck>>().iter().count();
        impacts += world.resource::<EventQueue<Impacted>>().iter().count();
    }
    assert!(world.get_resource::<ImpactRecord>().is_none(), "no record");
    assert_eq!((struck, impacts), (0, 0));
    assert!(
        world
            .query::<(Entity, &Health)>()
            .all(|(_, health)| health.current() == 100.0)
    );
}

#[test]
fn an_impact_rule_reads_its_data_and_saves_its_credit_and_scale() {
    let read: ImpactDamage = serde_json::from_value(json!({
        "speed": 400, "per_mass": 0.002, "most": 120, "floor": 2, "kind": "physical",
        "extra": { "when": { "all": ["status.burning"] }, "damage": { "fire": 12 } },
        "push": { "away": 0.6, "most": 900 }, "margin": 3, "land": "now", "prepare": "armour"
    }))
    .unwrap();
    let built = ImpactDamage::new()
        .with_most(120.0)
        .with_floor(2.0)
        .with_kind("physical")
        .with_extra(TagFilter::new().all(["status.burning"]), "fire", 12.0)
        .with_push_away(0.6, Some(900.0))
        .landing_now()
        .with_prepare("armour");
    assert_eq!(read, built);
    let mut thrown = built.clone();
    thrown.set_scale(1.5);
    let saved = serde_json::to_value(&thrown).unwrap();
    assert_eq!(saved["scale"], json!(1.5));
    assert!(saved.get("margin").is_none(), "its defaults left out");
    let again: ImpactDamage = serde_json::from_value(saved).unwrap();
    assert_eq!(again, thrown);
    for bad in [
        json!({ "speed": -1 }),
        json!({ "most": 0 }),
        json!({ "push": { "away": -1 } }),
        json!({ "land": "later" }),
        json!({ "kind": "Fire" }),
        json!({ "credit": 3 }),
    ] {
        assert!(
            serde_json::from_value::<ImpactDamage>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    // Saved with the World: its credit by save ID.
    let (mut world, _) = rig();
    let thrower = world.spawn();
    tungsten_core::save::name(&mut world, thrower, None, "thrower").unwrap();
    let body = world.spawn();
    tungsten_core::save::name(&mut world, body, None, "body").unwrap();
    let mut rule = ImpactDamage::new();
    rule.set_credit(Some(thrower));
    rule.set_scale(2.0);
    world.insert(body, rule);
    let save = tungsten_core::save::capture_world(&world).unwrap();
    let text = serde_json::to_value(&save).unwrap().to_string();
    assert!(
        text.contains("impact_damage") && text.contains("credit"),
        "{text}"
    );
    let (mut other, _) = rig();
    let thrower_b = other.spawn();
    tungsten_core::save::name(&mut other, thrower_b, None, "thrower").unwrap();
    let body_b = other.spawn();
    tungsten_core::save::name(&mut other, body_b, None, "body").unwrap();
    tungsten_core::save::restore_world(&mut other, &save, &(Restored,)).unwrap();
    let loaded = other.get::<ImpactDamage>(body_b).unwrap();
    assert_eq!((loaded.credit(), loaded.scale()), (Some(thrower_b), 2.0));
}

// 7.13c's foes and cues, Step 1: the record before the movers, and a dead
// credit.

#[test]
fn the_impact_record_keeps_what_a_body_carries_into_the_step() {
    use tungsten_core::physics::{PhysicsConfig, PhysicsPlugin, PrevPosition};

    use crate::motion::path::{PathFollower, PathFollowerPlugin};
    use crate::motion::top_down::{TopDownMover, TopDownMoverPlugin};

    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    KnockbackPlugin.build(&mut schedule, &mut world);
    TopDownMoverPlugin.build(&mut schedule, &mut world);
    PathFollowerPlugin.build(&mut schedule, &mut world);
    ImpactDamagePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    // A bodiless walker heading +x at 200 px/s.
    let mut mover = TopDownMover::new(200.0);
    mover.set_direction(Vec2::X);
    let walker = world.spawn_with((
        Position(Vec2::new(0.0, -300.0)),
        PrevPosition(Vec2::new(0.0, -300.0)),
        Velocity(Vec2::ZERO),
        mover,
        Health::new(100.0),
    ));
    // A kinematic body standing still, to be knocked.
    let knocked = world.spawn_with(RigidBodyBundle::kinematic(
        Position(Vec2::new(0.0, 300.0)),
        Collider::circle(10.0),
    ));
    world.insert(knocked, Health::new(100.0));
    world.insert(knocked, Knockback::new(0.2));
    // A kinematic hitter of mass 6 whose path stops at a target's face:
    // 10 px a step for ten steps, then the last 5.
    let hitter = world.spawn_with(RigidBodyBundle::kinematic(
        Position(Vec2::new(0.0, 600.0)),
        Collider::circle(10.0),
    ));
    world.insert(hitter, ImpactDamage::new().with_mass(6.0));
    world.insert(
        hitter,
        PathFollower::new([Vec2::new(0.0, 600.0), Vec2::new(105.0, 600.0)], 600.0),
    );
    let face = target(&mut world, Vec2::new(125.0, 600.0), Vec2::ZERO);
    for _ in 0..3 {
        step(&mut world, &mut schedule);
    }
    let velocity = |world: &World, entity| world.get::<Velocity>(entity).unwrap().0;
    assert_eq!(velocity(&world, walker), Vec2::new(200.0, 0.0));
    // The walker turned this step: the record keeps what it carried in.
    world
        .get_mut::<TopDownMover>(walker)
        .unwrap()
        .set_direction(Vec2::NEG_X);
    assert!(crate::knock_back(
        &mut world,
        knocked,
        Vec2::new(300.0, 0.0)
    ));
    step(&mut world, &mut schedule);
    assert_eq!(velocity(&world, walker), Vec2::new(-200.0, 0.0));
    assert_eq!(step_velocity(&world, walker), Some(Vec2::new(200.0, 0.0)));
    let shoved = velocity(&world, knocked);
    assert!(shoved.x > 0.0, "knocked this step: {shoved}");
    assert_eq!(step_velocity(&world, knocked), Some(Vec2::ZERO));
    // The next step's record sees this step's shove.
    step(&mut world, &mut schedule);
    assert_eq!(step_velocity(&world, knocked), Some(shoved));
    // The hitter's last step of 5 px brings it to the target's face at
    // 300 px/s, under the rule's 400; it carried 600 into that step, which
    // the hit takes: 0.002 × 6 × (600 − 400).
    let mut hit = Vec::new();
    for _ in 0..12 {
        step(&mut world, &mut schedule);
        hit.extend(
            hits(&world)
                .into_iter()
                .filter(|hit| hit.0 == face)
                .map(|hit| hit.1),
        );
    }
    assert_eq!(hit.len(), 1, "{hit:?}");
    assert!(
        (hit[0] - 2.4).abs() < 1e-3,
        "at its approach speed: {hit:?}"
    );
}

#[test]
fn a_dead_credit_credits_nobody() {
    let lucky = |world: &mut World| {
        world.spawn_with((
            Health::new(100.0),
            Stats::new()
                .with(Stats::CRIT_CHANCE, 0.5)
                .with(Stats::CRIT_MULTIPLIER, 2.0),
        ))
    };
    // A thrown body whose thrower died mid-flight, against one alive: each
    // hit's source, and the draws.
    for dies in [false, true] {
        let (mut world, mut schedule) = rig();
        let thrower = lucky(&mut world);
        let mut rule = ImpactDamage::new();
        rule.set_credit(Some(thrower));
        hitter(&mut world, rule, 4.0, Vec2::new(1000.0, 0.0));
        let hit = target(&mut world, Vec2::new(20.0, 0.0), Vec2::ZERO);
        if dies {
            crate::kill(&mut world, thrower);
        }
        world.insert_resource(KitRng::seeded(7));
        step(&mut world, &mut schedule);
        let got: Vec<_> = hits(&world)
            .into_iter()
            .filter(|got| got.0 == hit)
            .map(|got| got.2)
            .collect();
        let want = if dies { None } else { Some(thrower) };
        assert_eq!(got, [want], "dies {dies}");
        assert_eq!(
            world.resource::<KitRng>().draws(),
            u64::from(!dies),
            "dies {dies}"
        );
    }
    // A bolt whose shooter died before its end: its blast from nobody.
    for dies in [false, true] {
        let (mut world, mut schedule) = bolt_rig();
        let shooter = lucky(&mut world);
        let near = world.spawn_with((
            tungsten_core::Transform::from_position(Vec2::new(30.0, 20.0)),
            Health::new(100.0),
        ));
        let bolt = world.spawn_with((
            Position(Vec2::ZERO),
            tungsten_core::physics::PrevPosition(Vec2::ZERO),
            crate::LinearMotion::new(Vec2::new(600.0, 0.0)),
            crate::Hitbox::new(crate::HitShape::circle(2.0), 0.0),
            crate::Projectile::new()
                .with_range(30.0)
                .with_end(crate::Blast::new(50.0).with_damage(10.0)),
            Source::new(shooter),
        ));
        step(&mut world, &mut schedule);
        if dies {
            crate::kill(&mut world, shooter);
        }
        world.insert_resource(KitRng::seeded(7));
        let mut got = Vec::new();
        for _ in 0..4 {
            step(&mut world, &mut schedule);
            got.extend(
                hits(&world)
                    .into_iter()
                    .filter(|got| got.0 == near)
                    .map(|got| got.2),
            );
        }
        assert!(!world.is_alive(bolt), "dies {dies}: it ended");
        let want = if dies { None } else { Some(shooter) };
        assert_eq!(got, [want], "dies {dies}");
        assert_eq!(
            world.resource::<KitRng>().draws(),
            u64::from(!dies),
            "dies {dies}"
        );
    }
    // A thrower an earlier `now` impact of the same pass kills: its throw's
    // hit from nobody, and the dead thrower hit no more. Its touch sorts
    // first, by the thrower's lower index.
    let (mut world, mut schedule) = rig();
    let thrower = lucky(&mut world);
    world.insert(thrower, Position(Vec2::new(20.0, 0.0)));
    world.insert(thrower, Collider::circle(10.0));
    *world.get_mut::<Health>(thrower).unwrap() = Health::new(1.0);
    let mark = target(&mut world, Vec2::new(20.0, 100.0), Vec2::ZERO);
    let rock = |world: &mut World, at: Vec2, velocity: Vec2, rule: ImpactDamage| {
        let body = hitter(world, rule, 4.0, velocity);
        world.get_mut::<Position>(body).unwrap().0 = at;
        body
    };
    let first = rock(
        &mut world,
        Vec2::ZERO,
        Vec2::new(1000.0, 0.0),
        ImpactDamage::new().landing_now(),
    );
    let mut thrown = ImpactDamage::new().landing_now();
    thrown.set_credit(Some(thrower));
    rock(
        &mut world,
        Vec2::new(0.0, 100.0),
        Vec2::new(1000.0, 0.0),
        thrown,
    );
    rock(
        &mut world,
        Vec2::new(40.0, 0.0),
        Vec2::new(-1000.0, 0.0),
        ImpactDamage::new().landing_now(),
    );
    world.insert_resource(KitRng::seeded(7));
    step(&mut world, &mut schedule);
    assert!(world.get::<Health>(thrower).unwrap().is_dead());
    let got: Vec<(Entity, Option<Entity>)> =
        hits(&world).iter().map(|got| (got.0, got.2)).collect();
    assert_eq!(
        got,
        [(thrower, None), (mark, None)],
        "the throw from nobody"
    );
    let impacted: Vec<(Entity, Entity)> = world
        .resource::<EventQueue<Impacted>>()
        .iter()
        .map(|event| (event.hitter, event.target))
        .collect();
    assert_eq!(impacted.iter().filter(|(_, at)| *at == thrower).count(), 1);
    assert_eq!(impacted[0], (first, thrower));
    assert_eq!(world.resource::<KitRng>().draws(), 0, "no crit drawn");
}

/// A World with physics without gravity, tags, stats, health, sources,
/// linear motion, chemistry and projectiles.
fn bolt_rig() -> (World, Schedule) {
    use tungsten_core::physics::{PhysicsConfig, PhysicsPlugin};

    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(tungsten_core::TilemapRegistry::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TagsPlugin::new().build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    SourcePlugin.build(&mut schedule, &mut world);
    crate::LinearMotionPlugin.build(&mut schedule, &mut world);
    crate::ChemistryPlugin::new().build(&mut schedule, &mut world);
    crate::ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}
