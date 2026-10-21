use std::path::PathBuf;

use super::*;
use tungsten_core::{
    CommandBuffer, MaterialRegistry, MaterialUniformDefaults, Plugin, PrefabData, PrefabRegistry,
    Schedule, Sprite, SpriteAssetId, Stage, Time, World, system,
};

use crate::damage::health::{DamagedPart, HIT_FLASH_MATERIAL, HealthPlugin, HitFlash};
use crate::damage::kinds::DamageTypes;
use crate::foundation::phases::Phase;
use crate::foundation::stats::Modifier;
use crate::foundation::trace::KitTrace;
use crate::motion::knockback::Knockback;
use crate::react::counters::{Counters, CountersPlugin};
use crate::react::cues::{CUES, CueAction, Cues, CuesPlugin};
use crate::{Conversion, Penetration, Resist, Shield};

const STEP: f32 = 1.0 / 60.0;

/// A World with health, counters and cues, a command buffer, a trace, the
/// prefab `puff` and the flash material.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut prefabs = PrefabRegistry::new();
    let puff: PrefabData = serde_json::from_value(serde_json::json!({ "tag": "puff" })).unwrap();
    prefabs.insert("puff".to_string(), puff);
    world.insert_resource(prefabs);
    let mut materials = MaterialRegistry::new();
    materials.allocate(
        HIT_FLASH_MATERIAL,
        PathBuf::from("manifest.json"),
        "damage_flash".to_string(),
        MaterialUniformDefaults::default(),
    );
    world.insert_resource(materials);
    let mut schedule = Schedule::new();
    HealthPlugin.build(&mut schedule, &mut world);
    CountersPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One fixed step of 1/60 s, then the command flush that ends the frame.
fn frame(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(STEP);
    world.resource_mut::<Time>().enter_fixed_step();
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

fn sent(world: &World) -> Vec<Damaged> {
    world
        .resource::<tungsten_core::EventQueue<Damaged>>()
        .iter()
        .copied()
        .collect()
}

fn game(world: &World, name: &str) -> i64 {
    world.resource::<Counters>().get(name)
}

/// Each part sent so far: its amount and its kinds by name.
fn parts_sent(world: &World) -> Vec<(f32, Vec<&str>)> {
    world
        .resource::<tungsten_core::EventQueue<DamagedPart>>()
        .iter()
        .map(|part| (part.amount, part.kinds.names(world)))
        .collect()
}

/// Declares the damage types `names` in a `damage_types` entry inserted in
/// code.
fn declare(world: &mut World, names: &[&str]) {
    let entry: serde_json::Map<String, serde_json::Value> = names
        .iter()
        .map(|name| ((*name).to_owned(), serde_json::json!({})))
        .collect();
    let types: DamageTypes = serde_json::from_value(serde_json::Value::Object(entry)).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
}

fn typed(amount: f32, names: &[&str]) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new(names.iter().copied()))
}

#[test]
fn a_periodic_hit_passes_the_window_and_starts_none() {
    let (mut world, mut schedule) = rig();
    let slime = world.spawn_with((
        Health::new(10.0).with_invulnerability(0.5),
        HitFlash::new([1.0; 4], 0.2),
        Sprite::new(SpriteAssetId::new(0)),
        Knockback::new(0.1),
    ));
    let mage = world.spawn();
    // A plain hit opens the window; inside it, a plain hit is ignored and a
    // periodic one lands, starting neither the window over nor the flash.
    assert_eq!(crate::damage(&mut world, slime, 1.0), Hit::Hurt);
    assert_eq!(crate::damage(&mut world, slime, 1.0), Hit::Ignored);
    frame(&mut world, &mut schedule);
    let window_before = world.get::<Health>(slime).unwrap();
    assert!(window_before.is_invulnerable());
    let left_before = serde_json::to_value(window_before).unwrap()["window_left"]
        .as_f64()
        .unwrap();
    // The flash has run out of its first frame; clear its state for the check.
    schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    let tick = DamageRequest::new(slime, 2.0)
        .with_source(mage)
        .with_periodic(true)
        .with_knockback(glam::Vec2::X);
    assert!(tick.is_periodic() && !DamageRequest::new(slime, 1.0).is_periodic());
    assert_eq!(tick.apply(&mut world), Hit::Hurt);
    let health = world.get::<Health>(slime).unwrap();
    assert_eq!(health.current(), 7.0);
    let left_after = serde_json::to_value(health).unwrap()["window_left"]
        .as_f64()
        .unwrap();
    assert_eq!(left_after, left_before, "the window is not started over");
    let flash = world.get::<HitFlash>(slime).unwrap();
    assert!(flash.is_active(), "the first hit's flash still runs");
    assert_eq!(
        serde_json::to_value(world.get::<Health>(slime).unwrap()).unwrap()["window_left"],
        serde_json::json!(left_after)
    );
    let events = sent(&world);
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0],
        Damaged::new(slime, 1.0, false),
        "a plain hit's event as before"
    );
    assert!(!events[0].periodic);
    assert_eq!(
        (events[1].amount, events[1].source, events[1].periodic),
        (2.0, Some(mage), true)
    );
    let parts = world.resource::<tungsten_core::EventQueue<DamagedPart>>();
    let periodic: Vec<bool> = parts.iter().map(|part| part.periodic).collect();
    assert_eq!(periodic, [false, true]);
    assert_eq!(
        world.get::<Knockback>(slime).unwrap().push(),
        glam::Vec2::X,
        "a periodic request's own knockback still pushes"
    );
    // Once the window has passed, a periodic hit still starts none.
    for _ in 0..30 {
        frame(&mut world, &mut schedule);
    }
    assert!(!world.get::<Health>(slime).unwrap().is_invulnerable());
    world.get_mut::<HitFlash>(slime).unwrap();
    for _ in 0..3 {
        world.resource_mut::<Time>().advance_frame(0.1);
        schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    }
    assert!(!world.get::<HitFlash>(slime).unwrap().is_active());
    assert_eq!(
        DamageRequest::new(slime, 1.0)
            .with_periodic(true)
            .apply(&mut world),
        Hit::Hurt
    );
    let health = world.get::<Health>(slime).unwrap();
    assert!(!health.is_invulnerable(), "no window started");
    assert!(
        !world.get::<HitFlash>(slime).unwrap().is_active(),
        "no flash started"
    );
    assert_eq!(
        crate::damage(&mut world, slime, 1.0),
        Hit::Hurt,
        "so a plain hit lands at once"
    );
}

#[test]
fn a_one_part_hit_resolves_as_before() {
    let (mut world, _) = rig();
    let plain = world.spawn_with((Health::new(10.0),));
    let request = DamageRequest::new(plain, 3.0);
    assert_eq!(request.parts(), [DamagePart::new(3.0)]);
    assert_eq!((request.amount(), request.tags()), (3.0, &Tags::default()));
    assert_eq!(request.apply(&mut world), Hit::Hurt);
    assert_eq!(world.get::<Health>(plain).unwrap().current(), 7.0);

    let golem = world.spawn_with((
        Health::new(10.0),
        Resist::new()
            .with_factor("damage", 0.5)
            .with_factor("damage.fire", 0.5)
            .with_factor("damage.poison", 0.0),
    ));
    let fire = DamageRequest::new(golem, 6.0).with_tags(Tags::new(["damage.fire"]));
    assert_eq!(
        fire.parts()[0].tags(),
        &Tags::default(),
        "it takes the request's"
    );
    assert_eq!(fire.apply(&mut world), Hit::Hurt);
    let poison = DamageRequest::new(golem, 6.0).with_tags(Tags::new(["damage.poison"]));
    assert_eq!(poison.apply(&mut world), Hit::Ignored, "immune");
    assert_eq!(
        DamageRequest::new(golem, 0.0).apply(&mut world),
        Hit::Ignored
    );
    world.insert(plain, Resist::new().with_factor("tiny", 1e-30));
    let tiny = DamageRequest::new(plain, 1e-30).with_tags(Tags::new(["tiny"]));
    assert_eq!(tiny.apply(&mut world), Hit::Ignored, "resisted to 0");

    let levels: Vec<String> = (1..=13)
        .map(|n| {
            (0..n)
                .map(|i| format!("l{i}"))
                .collect::<Vec<_>>()
                .join(".")
        })
        .collect();
    let mut resist = Resist::new();
    for level in &levels {
        resist = resist.with_factor(level, 1000.0);
    }
    let titan = world.spawn_with((Health::new(f32::MAX), resist));
    let deep = DamageRequest::new(titan, 1.0).with_tags(Tags::new([levels[12].clone()]));
    assert_eq!(deep.apply(&mut world), Hit::Killed, "clamped to f32::MAX");

    assert_eq!(
        sent(&world),
        [
            Damaged::new(plain, 3.0, false),
            Damaged::new(golem, 1.5, false),
            Damaged::new(titan, f32::MAX, true),
        ],
        "0.85's events, value for value, with no kind"
    );
    assert_eq!(
        parts_sent(&world),
        [(3.0, vec![]), (1.5, vec![]), (f32::MAX, vec![])],
        "one part each, of no kind"
    );
    assert_eq!(world.get::<Health>(golem).unwrap().current(), 8.5);
}

#[test]
fn a_multi_part_hit_starts_one_window_and_sends_one_damaged_after_its_parts() {
    let (mut world, _) = rig();
    declare(&mut world, &["fire", "ice"]);
    let archer = world.spawn();
    let target = world.spawn_with((
        Health::new(20.0).with_invulnerability(0.5),
        Shield::new(1.0),
        HitFlash::new([1.0; 4], 0.2),
        Sprite::new(SpriteAssetId::new(0)),
    ));
    let hit = DamageRequest::from_parts(target, [typed(3.0, &["fire"]), typed(2.0, &["ice"])])
        .with_source(archer)
        .with_position(Vec2::new(4.0, 5.0))
        .with_crit(true);
    assert_eq!(hit.amount(), 5.0);
    assert_eq!(hit.apply(&mut world), Hit::Hurt);
    let health = world.get::<Health>(target).unwrap();
    assert_eq!(health.current(), 16.0, "5, of which the shield took 1");
    assert!(health.is_invulnerable(), "one window");
    assert!(world.get::<HitFlash>(target).unwrap().is_active());
    assert_eq!(
        parts_sent(&world),
        [(3.0, vec!["fire"]), (2.0, vec!["ice"])]
    );
    let parts: Vec<DamagedPart> = world
        .resource::<tungsten_core::EventQueue<DamagedPart>>()
        .iter()
        .copied()
        .collect();
    assert!(
        parts
            .iter()
            .all(|part| part.source == Some(archer) && part.position == Some(Vec2::new(4.0, 5.0)))
    );
    let damaged = sent(&world);
    assert_eq!(damaged.len(), 1, "one Damaged for the hit");
    assert_eq!(
        (
            damaged[0].amount,
            damaged[0].crit,
            damaged[0].kinds.names(&world)
        ),
        (5.0, true, vec!["fire", "ice"])
    );
    let story: Vec<String> = world
        .resource::<KitTrace>()
        .about(target)
        .map(|entry| {
            let line = entry.to_string();
            let head = format!("{target} ");
            line.split_once(&head).unwrap().1.to_owned()
        })
        .collect();
    assert_eq!(
        story,
        [
            format!("damaged_part 3 with {archer}"),
            format!("damaged_part 2 with {archer}"),
            format!("damaged 5 with {archer}")
        ]
    );
    let again = DamageRequest::from_parts(target, [typed(3.0, &["fire"])]);
    assert_eq!(again.apply(&mut world), Hit::Ignored, "inside the window");
}

#[test]
fn a_zero_part_and_a_modifier_made_zero_factor_meet_no_product() {
    let (mut world, _) = rig();
    // 103 levels of 1000: a product of 1e309, past f64::MAX, which 13
    // levels never reach.
    let levels: Vec<String> = (1..=103)
        .map(|n| {
            (0..n)
                .map(|i| format!("l{i}"))
                .collect::<Vec<_>>()
                .join(".")
        })
        .collect();
    let deepest = levels.last().unwrap().as_str();
    let mut resist = Resist::new();
    for level in &levels {
        resist = resist.with_factor(level, 1000.0);
    }
    let product = resist.product_with(&Tags::new([deepest]), 0.0).unwrap();
    assert!(product.is_infinite(), "the levels overflow f64: {product}");
    // A 104th entry, sorting after the levels, whose factor a modifier
    // makes 0.
    resist.add_modifier("m", "ward", Modifier::Mul(0.0));
    let titan = world.spawn_with((Health::new(100.0), resist));
    let zero_beside =
        DamageRequest::from_parts(titan, [typed(0.0, &[deepest]), typed(2.0, &["safe"])]);
    assert_eq!(zero_beside.apply(&mut world), Hit::Hurt);
    let warded_beside =
        DamageRequest::from_parts(titan, [typed(1.0, &[deepest, "m"]), typed(2.0, &["safe"])]);
    assert_eq!(warded_beside.apply(&mut world), Hit::Hurt);
    let amounts: Vec<f32> = sent(&world).iter().map(|event| event.amount).collect();
    assert_eq!(amounts, [2.0, 2.0], "each drop met no product");
    assert!(
        parts_sent(&world)
            .iter()
            .all(|(amount, _)| amount.is_finite() && *amount != f32::MAX && *amount == 2.0)
    );
    assert_eq!(world.get::<Health>(titan).unwrap().current(), 96.0);
}

#[test]
fn conversion_normalizes_past_one_and_never_chains() {
    let (mut world, _) = rig();
    declare(&mut world, &["fire", "ice", "acid", "zap"]);
    let target = world.spawn_with((Health::new(1000.0),));
    let past_one = Conversion::new()
        .with_rule("fire", "ice", 0.75)
        .with_rule("fire", "acid", 0.75)
        .with_rule("ice", "zap", 1.0);
    DamageRequest::from_parts(target, [typed(10.0, &["fire"])])
        .with_conversion(past_one)
        .apply(&mut world);
    assert_eq!(
        parts_sent(&world),
        [(5.0, vec!["ice"]), (5.0, vec!["acid"])],
        "normalized to 1, no remainder, the ice share not converted again"
    );
    let exactly_one = Conversion::new().with_rule("fire", "ice", 1.0);
    DamageRequest::from_parts(target, [typed(4.0, &["fire"])])
        .with_conversion(exactly_one)
        .apply(&mut world);
    assert_eq!(
        parts_sent(&world)[2],
        (4.0, vec!["ice"]),
        "no remainder at 1"
    );
    let some = Conversion::new().with_rule("fire", "ice", 0.25);
    DamageRequest::from_parts(target, [typed(4.0, &["fire"])])
        .with_conversion(some)
        .apply(&mut world);
    assert_eq!(
        parts_sent(&world)[3..],
        [(3.0, vec!["fire"]), (1.0, vec!["ice"])],
        "the remainder before its shares"
    );
    // A share is its rule's type alone: the request's tags stay behind.
    let shared = Conversion::new().with_rule("fire", "ice", 0.5);
    DamageRequest::new(target, 2.0)
        .with_tags(Tags::new(["fire", "acid"]))
        .with_conversion(shared)
        .apply(&mut world);
    assert_eq!(
        parts_sent(&world)[5..],
        [(1.0, vec!["acid", "fire"]), (1.0, vec!["ice"])]
    );
}

#[test]
fn shares_that_land_on_one_type_merge_in_first_appearance_order() {
    let (mut world, _) = rig();
    declare(&mut world, &["slash", "fire"]);
    let target = world.spawn_with((Health::new(1000.0),));
    DamageRequest::from_parts(target, [typed(10.0, &["slash"]), typed(5.0, &["fire"])])
        .with_conversion(Conversion::new().with_rule("slash", "fire", 0.5))
        .apply(&mut world);
    assert_eq!(
        parts_sent(&world),
        [(5.0, vec!["slash"]), (10.0, vec!["fire"])],
        "slash 10 and fire 5, half the slash to fire: slash 5 and fire 10"
    );
    // Untyped parts take the request's types and merge as one.
    DamageRequest::from_parts(target, [DamagePart::new(1.0), DamagePart::new(2.0)])
        .with_tags(Tags::new(["fire"]))
        .apply(&mut world);
    assert_eq!(parts_sent(&world)[2..], [(3.0, vec!["fire"])]);
    assert_eq!(sent(&world)[1].amount, 3.0);
}

#[test]
fn penetration_never_pierces_zero_or_raises_a_factor_above_one() {
    let (mut world, _) = rig();
    let target = world.spawn_with((
        Health::new(1000.0),
        Resist::new()
            .with_factor("fire", 0.5)
            .with_factor("ice", 0.0)
            .with_factor("acid", 1.5),
    ));
    let pen = Penetration::new()
        .with_points("fire", 0.75)
        .with_points("ice", 1.0)
        .with_points("acid", 1.0);
    let hit = DamageRequest::from_parts(
        target,
        [
            typed(4.0, &["fire"]),
            typed(4.0, &["ice"]),
            typed(4.0, &["acid"]),
        ],
    )
    .with_penetration(pen);
    assert_eq!(hit.apply(&mut world), Hit::Hurt);
    let amounts: Vec<f32> = parts_sent(&world).iter().map(|part| part.0).collect();
    assert_eq!(
        amounts,
        [4.0, 6.0],
        "fire capped at a factor of 1, ice immune, acid left at 1.5"
    );
}

#[test]
fn the_source_lends_its_penetration_and_conversion_after_the_hits_own() {
    let (mut world, _) = rig();
    declare(&mut world, &["fire", "acid", "ice"]);
    let mage = world.spawn_with((
        Penetration::new().with_points("fire", 0.25),
        Conversion::new().with_rule("fire", "ice", 0.25),
    ));
    let target = world.spawn_with((Health::new(1000.0), Resist::new().with_factor("fire", 0.25)));
    let own = || {
        DamageRequest::from_parts(target, [typed(8.0, &["fire"])])
            .with_penetration(Penetration::new().with_points("fire", 0.25))
            .with_conversion(Conversion::new().with_rule("fire", "acid", 0.25))
    };
    own().with_source(mage).apply(&mut world);
    assert_eq!(
        parts_sent(&world),
        [(3.0, vec!["fire"]), (2.0, vec!["acid"]), (2.0, vec!["ice"])],
        "fire 4 at 0.25 + 0.5 points; the hit's share before the source's"
    );
    own().apply(&mut world);
    assert_eq!(
        parts_sent(&world)[3..],
        [(3.0, vec!["fire"]), (2.0, vec!["acid"])],
        "fire 6 at 0.25 + 0.25 points, without the source"
    );
}

#[test]
fn a_hit_whose_every_part_drops_is_ignored_with_no_side_effect() {
    let (mut world, _) = rig();
    let target = world.spawn_with((
        Health::new(5.0).with_invulnerability(1.0),
        Shield::new(2.0),
        HitFlash::new([1.0; 4], 0.2),
        Sprite::new(SpriteAssetId::new(0)),
        Knockback::new(0.2),
        Resist::new()
            .with_factor("fire", 0.0)
            .with_factor("ice", 0.0),
    ));
    let dropped = DamageRequest::from_parts(
        target,
        [
            typed(3.0, &["fire"]),
            typed(2.0, &["ice"]),
            DamagePart::new(0.0),
        ],
    )
    .with_knockback(Vec2::new(100.0, 0.0));
    assert_eq!(dropped.apply(&mut world), Hit::Ignored);
    assert_eq!(
        DamageRequest::from_parts(target, []).apply(&mut world),
        Hit::Ignored,
        "no part"
    );
    let health = world.get::<Health>(target).unwrap();
    assert!(health.current() == 5.0 && !health.is_invulnerable());
    assert_eq!(world.get::<Shield>(target).unwrap().current(), 2.0);
    assert!(!world.get::<HitFlash>(target).unwrap().is_active());
    assert!(!world.get::<Knockback>(target).unwrap().is_active());
    assert!(sent(&world).is_empty() && parts_sent(&world).is_empty());
    assert_eq!(world.resource::<KitTrace>().about(target).count(), 0);
}

#[test]
fn an_amount_of_zero_changes_nothing_immediate_or_queued() {
    let (mut world, mut schedule) = rig();
    let target = world.spawn_with((
        Health::new(5.0).with_invulnerability(1.0),
        Shield::new(2.0).with_recharge(1.0, 0.0),
        HitFlash::new([1.0; 4], 0.2),
        Sprite::new(SpriteAssetId::new(0)),
    ));
    world.get_mut::<Shield>(target).unwrap().restore(0.0);
    assert_eq!(
        DamageRequest::new(target, 0.0).apply(&mut world),
        Hit::Ignored
    );
    // Resisted to 0: the same.
    world.insert(target, Resist::new().with_factor("tiny", 1e-30));
    let tiny = DamageRequest::new(target, 1e-30).with_tags(Tags::new(["tiny"]));
    assert_eq!(tiny.clone().apply(&mut world), Hit::Ignored);
    assert!(DamageRequest::new(target, 0.0).queue(&mut world));
    assert!(tiny.queue(&mut world));
    frame(&mut world, &mut schedule);
    let health = world.get::<Health>(target).unwrap();
    assert_eq!(health.current(), 5.0);
    assert!(!health.is_invulnerable(), "no window");
    assert!(
        !world.get::<HitFlash>(target).unwrap().is_active(),
        "no flash"
    );
    assert_eq!(world.get::<Shield>(target).unwrap().current(), 2.0);
    assert!(sent(&world).is_empty(), "no event");
    assert_eq!(
        world.resource::<KitTrace>().about(target).count(),
        0,
        "nothing traced"
    );
}

#[test]
fn an_immunity_and_a_shield_hold_their_steps_and_their_outcomes() {
    let (mut world, _) = rig();
    let golem = world.spawn_with((
        Health::new(10.0),
        Shield::new(2.0),
        Resist::new()
            .with_factor("damage", 0.5)
            .with_factor("damage.fire", 0.5)
            .with_factor("damage.poison", 0.0),
    ));
    let fire = |amount| DamageRequest::new(golem, amount).with_tags(Tags::new(["damage.fire"]));
    assert_eq!(
        fire(4.0).apply(&mut world),
        Hit::Absorbed,
        "1 of the shield's 2"
    );
    assert_eq!(world.get::<Shield>(golem).unwrap().current(), 1.0);
    assert_eq!(fire(8.0).apply(&mut world), Hit::Hurt, "2: 1 off each");
    assert_eq!(world.get::<Health>(golem).unwrap().current(), 9.0);
    let poison = DamageRequest::new(golem, 100.0).with_tags(Tags::new(["damage.poison.slow"]));
    assert_eq!(
        poison.apply(&mut world),
        Hit::Ignored,
        "immune, by an ancestor"
    );
    assert_eq!(
        sent(&world).iter().map(|e| e.amount).collect::<Vec<_>>(),
        [1.0, 2.0],
        "the amounts after the resistances"
    );
}

#[test]
fn immunity_comes_before_a_product_past_f32_max_which_clamps() {
    let (mut world, _) = rig();
    // Thirteen levels of 1000: a product of 1e39, past f32::MAX.
    let levels: Vec<String> = (1..=13)
        .map(|n| {
            (0..n)
                .map(|i| format!("l{i}"))
                .collect::<Vec<_>>()
                .join(".")
        })
        .collect();
    let deepest = levels.last().unwrap().clone();
    let mut resist = Resist::new();
    for level in &levels {
        resist = resist.with_factor(level, 1000.0);
    }
    let titan = world.spawn_with((Health::new(f32::MAX), resist.clone()));
    let request = DamageRequest::new(titan, 1.0).with_tags(Tags::new([deepest.clone()]));
    assert_eq!(request.clone().apply(&mut world), Hit::Killed);
    let event = sent(&world)[0];
    assert!(event.amount.is_finite() && event.amount == f32::MAX);
    assert_eq!(resist.factor(&Tags::new([deepest.clone()])), f32::MAX);

    let immune = resist.with_factor("ward", 0.0);
    let warded = world.spawn_with((Health::new(5.0), immune.clone()));
    let request = DamageRequest::new(warded, 1.0).with_tags(Tags::new([deepest, "ward".into()]));
    assert_eq!(request.apply(&mut world), Hit::Ignored);
    assert_eq!(world.get::<Health>(warded).unwrap().current(), 5.0);
}

#[test]
fn each_outcome_and_each_ignored_target() {
    let (mut world, _) = rig();
    let target = world.spawn_with((Health::new(3.0).with_invulnerability(0.5),));
    let plain = world.spawn();
    let gone = world.spawn_with((Health::new(3.0),));
    world.despawn(gone);
    let ending = world.spawn_with((Health::new(3.0), Despawning));
    for ignored in [plain, gone, ending] {
        assert_eq!(
            DamageRequest::new(ignored, 1.0).apply(&mut world),
            Hit::Ignored
        );
    }
    assert_eq!(DamageRequest::new(target, 1.0).apply(&mut world), Hit::Hurt);
    assert_eq!(
        DamageRequest::new(target, 1.0).apply(&mut world),
        Hit::Ignored,
        "inside its window"
    );
    let dying = world.spawn_with((Health::new(1.0),));
    assert_eq!(
        DamageRequest::new(dying, 5.0).apply(&mut world),
        Hit::Killed
    );
    assert_eq!(
        DamageRequest::new(dying, 5.0).apply(&mut world),
        Hit::Ignored,
        "the dead"
    );
}

#[test]
fn damaged_carries_the_source_the_position_and_the_crit() {
    let (mut world, _) = rig();
    let archer = world.spawn();
    let target = world.spawn_with((Health::new(5.0),));
    DamageRequest::new(target, 2.0)
        .with_source(archer)
        .with_position(Vec2::new(3.0, 4.0))
        .with_crit(true)
        .apply(&mut world);
    assert_eq!(
        sent(&world),
        [Damaged::new(target, 2.0, false)
            .with_source(archer)
            .with_position(Vec2::new(3.0, 4.0))
            .with_crit(true)]
    );
    assert_eq!(
        crate::damage(&mut world, archer, 1.0),
        Hit::Ignored,
        "no health"
    );
}

#[test]
fn a_crit_requests_parts_carry_the_flag() {
    let (mut world, _) = rig();
    declare(&mut world, &["fire", "ice"]);
    let archer = world.spawn();
    let target = world.spawn_with((Health::new(100.0),));
    let parts = || [typed(3.0, &["fire"]), typed(2.0, &["ice"])];
    DamageRequest::from_parts(target, parts())
        .with_source(archer)
        .with_crit(true)
        .apply(&mut world);
    DamageRequest::from_parts(target, parts())
        .with_periodic(true)
        .apply(&mut world);
    DamageRequest::from_parts(target, parts()).apply(&mut world);
    let flags: Vec<(f32, bool, bool)> = world
        .resource::<tungsten_core::EventQueue<DamagedPart>>()
        .iter()
        .map(|part| (part.amount, part.crit, part.periodic))
        .collect();
    assert_eq!(
        flags,
        [
            (3.0, true, false),
            (2.0, true, false),
            (3.0, false, true),
            (2.0, false, true),
            (3.0, false, false),
            (2.0, false, false)
        ],
        "every part of the crit, none of the periodic or the plain hit"
    );
    let crits: Vec<bool> = sent(&world).iter().map(|hit| hit.crit).collect();
    assert_eq!(crits, [true, false, false]);
    assert!(DamagedPart::new(target, 1.0).with_crit(true).crit);
    assert!(!DamagedPart::new(target, 1.0).crit);
}

#[test]
fn the_knockback_pushes_on_a_hurt_or_an_absorbed_hit_only() {
    let (mut world, _) = rig();
    let push = Vec2::new(-300.0, 0.0);
    let hurt = world.spawn_with((Health::new(5.0), Knockback::new(0.2)));
    let shielded = world.spawn_with((Health::new(5.0), Shield::new(5.0), Knockback::new(0.2)));
    let killed = world.spawn_with((Health::new(1.0), Knockback::new(0.2)));
    for (target, outcome) in [
        (hurt, Hit::Hurt),
        (shielded, Hit::Absorbed),
        (killed, Hit::Killed),
    ] {
        let request = DamageRequest::new(target, 2.0).with_knockback(push);
        assert_eq!(request.apply(&mut world), outcome);
    }
    assert!(world.get::<Knockback>(hurt).unwrap().is_active());
    assert!(world.get::<Knockback>(shielded).unwrap().is_active());
    assert!(!world.get::<Knockback>(killed).unwrap().is_active());
}

#[test]
fn queued_requests_resolve_sorted_by_target_then_source_with_none_first() {
    let (mut world, mut schedule) = rig();
    let a = world.spawn_with((Health::new(100.0),));
    let b = world.spawn_with((Health::new(100.0),));
    let (s1, s2) = (world.spawn(), world.spawn());
    for request in [
        DamageRequest::new(b, 1.0).with_source(s2),
        DamageRequest::new(a, 2.0).with_source(s2),
        DamageRequest::new(b, 3.0),
        DamageRequest::new(a, 4.0).with_source(s1),
        DamageRequest::new(a, 5.0),
    ] {
        assert!(request.queue(&mut world));
    }
    assert!(sent(&world).is_empty(), "nothing before the resolve");
    world.resource_mut::<Time>().advance_frame(STEP);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    let order: Vec<(Entity, f32)> = sent(&world)
        .iter()
        .map(|event| (event.entity, event.amount))
        .collect();
    assert_eq!(
        order,
        [(a, 5.0), (a, 4.0), (a, 2.0), (b, 3.0), (b, 1.0)],
        "a's none, s1, s2, then b's none, s2"
    );
}

#[test]
fn a_request_queued_while_paused_waits_for_the_next_step() {
    let (mut world, mut schedule) = rig();
    let target = world.spawn_with((Health::new(5.0),));
    world.resource_mut::<Time>().pause();
    assert!(DamageRequest::new(target, 2.0).queue(&mut world));
    assert!(HealRequest::new(target, 1.0).queue(&mut world));
    // Paused frames run no step; their flushes drop held events, not
    // requests.
    for _ in 0..3 {
        world.resource_mut::<Time>().advance_frame(STEP);
        world.flush_events();
    }
    assert_eq!(world.get::<Health>(target).unwrap().current(), 5.0);
    world.resource_mut::<Time>().set_paused(false);
    frame(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Health>(target).unwrap().current(),
        4.0,
        "2 off, then 1 back"
    );
}

#[test]
fn a_heal_is_capped_at_max_refused_on_the_dead_and_sends_healed() {
    let (mut world, _) = rig();
    let cleric = world.spawn();
    let knight = world.spawn_with((Health::new(10.0), Shield::new(4.0)));
    crate::damage(&mut world, knight, 7.0);
    assert_eq!(
        world.get::<Health>(knight).unwrap().current(),
        7.0,
        "4 shield, 3 health"
    );
    assert_eq!(heal(&mut world, knight, 0.0), 0.0);
    assert_eq!(
        HealRequest::new(knight, 5.0)
            .with_source(cleric)
            .apply(&mut world),
        3.0
    );
    assert_eq!(world.get::<Health>(knight).unwrap().current(), 10.0);
    assert_eq!(
        world.get::<Shield>(knight).unwrap().current(),
        0.0,
        "no heal fills it"
    );
    assert_eq!(heal(&mut world, knight, 1.0), 0.0, "full");
    let dead = world.spawn_with((Health::new(1.0),));
    crate::damage(&mut world, dead, 1.0);
    assert_eq!(heal(&mut world, dead, 1.0), 0.0, "the dead");
    let healed: Vec<Healed> = world
        .resource::<tungsten_core::EventQueue<Healed>>()
        .iter()
        .copied()
        .collect();
    assert_eq!(healed, [Healed::new(knight, 3.0).with_source(cleric)]);
    let traced: Vec<String> = world
        .resource::<KitTrace>()
        .about(knight)
        .map(ToString::to_string)
        .filter(|line| line.contains("healed"))
        .collect();
    assert_eq!(traced.len(), 1, "{traced:?}");
}

/// Kills `target` after the cues of the step `kill_at` names, in the react
/// phase: a kill that lands after `cues` has read the step's events.
fn late_killer(schedule: &mut Schedule, target: Entity, kill_at: u32) {
    let mut steps = 0;
    schedule.add(
        Phase::React.stage(),
        Phase::React
            .join(system("late_killer", move |world: &mut World| {
                steps += 1;
                if steps == kill_at {
                    crate::damage(world, target, 100.0);
                }
            }))
            .after(CUES),
    );
    schedule.resolve().unwrap();
}

#[test]
fn a_kill_after_the_cues_keeps_its_killed_cues_and_despawns_the_step_after() {
    let (mut world, mut schedule) = rig();
    let ghoul = world.spawn_with((
        Health::new(1.0).with_despawn_on_death(0.0),
        Cues::new().on("killed", [CueAction::counter_add("kills", 1).for_game()]),
    ));
    late_killer(&mut schedule, ghoul, 1);
    frame(&mut world, &mut schedule);
    assert!(world.get::<Health>(ghoul).unwrap().is_dead());
    assert!(world.is_alive(ghoul), "the kill landed after the cues");
    assert_eq!(game(&world, "kills"), 0);
    frame(&mut world, &mut schedule);
    assert_eq!(game(&world, "kills"), 1, "its killed cues ran first");
    assert!(!world.is_alive(ghoul), "then the despawn took it");
}

#[test]
fn the_death_policy_despawns_after_the_killed_cues_at_zero_and_after_its_delay() {
    for (delay, steps_after) in [(0.0_f32, 1_usize), (0.1, 6)] {
        let (mut world, mut schedule) = rig();
        let ghoul = world.spawn_with((
            Health::new(1.0).with_despawn_on_death(delay),
            tungsten_core::Transform::from_position(Vec2::new(7.0, 8.0)),
            Cues::new().on(
                "killed",
                [
                    CueAction::counter_add("kills", 1).for_game(),
                    CueAction::prefab("puff"),
                ],
            ),
        ));
        assert!(DamageRequest::new(ghoul, 5.0).queue(&mut world));
        frame(&mut world, &mut schedule);
        assert_eq!(
            game(&world, "kills"),
            1,
            "delay {delay}: the cues in the kill's step"
        );
        let mut alive_for = 0;
        while world.is_alive(ghoul) {
            frame(&mut world, &mut schedule);
            alive_for += 1;
            assert!(alive_for <= 20, "delay {delay}: never despawned");
        }
        assert_eq!(alive_for, steps_after, "delay {delay}");
        assert_eq!(
            world.query::<&tungsten_core::Tag>().count(),
            1,
            "delay {delay}: the puff survived"
        );
        let despawns: Vec<String> = world
            .resource::<KitTrace>()
            .iter()
            .map(ToString::to_string)
            .filter(|line| line.contains("despawned by health"))
            .collect();
        assert_eq!(despawns.len(), 1, "delay {delay}: traced once");
    }
}

#[test]
fn without_the_plugin_a_request_cannot_queue() {
    let mut world = World::new();
    let target = world.spawn_with((Health::new(1.0),));
    assert!(!DamageRequest::new(target, 1.0).queue(&mut world));
    assert!(!HealRequest::new(target, 1.0).queue(&mut world));
    assert_eq!(DamageRequest::new(target, 0.5).apply(&mut world), Hit::Hurt);
}

#[test]
fn a_request_reads_back_what_it_was_built_with() {
    let mut world = World::new();
    let (target, source) = (world.spawn(), world.spawn());
    let request = DamageRequest::new(target, 2.5)
        .with_source(source)
        .with_tags(Tags::new(["damage.fire"]))
        .with_position(Vec2::new(1.0, 2.0))
        .with_knockback(Vec2::new(-3.0, 0.0))
        .with_crit(true);
    assert_eq!((request.target(), request.amount()), (target, 2.5));
    assert_eq!(request.source(), Some(source));
    assert!(request.tags().has("damage"));
    assert_eq!(request.position(), Some(Vec2::new(1.0, 2.0)));
    assert_eq!(request.knockback(), Some(Vec2::new(-3.0, 0.0)));
    assert!(request.is_crit());
    let plain = DamageRequest::new(target, 1.0);
    assert_eq!(
        (
            plain.source(),
            plain.position(),
            plain.knockback(),
            plain.is_crit()
        ),
        (None, None, None, false)
    );
    let heal = HealRequest::new(target, 3.0).with_source(source);
    assert_eq!(
        (heal.target(), heal.amount(), heal.source()),
        (target, 3.0, Some(source))
    );
}

#[test]
#[should_panic(expected = "amount must be finite and >= 0")]
fn a_negative_request_panics() {
    let mut world = World::new();
    let target = world.spawn();
    let _ = DamageRequest::new(target, -1.0);
}

#[test]
#[should_panic(expected = "position must be finite")]
fn a_non_finite_position_panics() {
    let mut world = World::new();
    let target = world.spawn();
    let _ = DamageRequest::new(target, 1.0).with_position(Vec2::new(f32::NAN, 0.0));
}

#[test]
#[should_panic(expected = "velocity must be finite")]
fn a_non_finite_knockback_panics() {
    let mut world = World::new();
    let target = world.spawn();
    let _ = DamageRequest::new(target, 1.0).with_knockback(Vec2::new(f32::INFINITY, 0.0));
}

#[test]
#[should_panic(expected = "amount must be finite and >= 0")]
fn a_nan_heal_request_panics() {
    let mut world = World::new();
    let target = world.spawn();
    let _ = HealRequest::new(target, f32::NAN);
}
