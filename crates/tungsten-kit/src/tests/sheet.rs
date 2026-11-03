use serde_json::json;
use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::{CommandBuffer, EventQueue, PrefabRegistry, Schedule, Time};

use super::*;
use crate::{
    DamageRequest, Damaged, DamagedPart, Health, KitPlugins, KitRng, Resist, TagsPlugin, damage,
};

/// Spawns the empty prefab `id`, named `id` for a save.
fn placed(world: &mut World, id: &str) -> Entity {
    let entity = world.spawn_prefab(id, [0.0, 0.0]).unwrap();
    tungsten_core::save::name(world, entity, None, id).unwrap();
    entity
}

/// What a restored spawn takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Loaded;

/// The skill's damage types and tags the tests name.
const TAGS: [&str; 11] = [
    "damage.fire",
    "damage.cold",
    "skill.spell",
    "skill.attack",
    "skill.projectile",
    "skill.fire",
    "skill.melee",
    "status.chilled",
    "status.marked",
    "team.player",
    "team.enemy",
];

/// A World with every kit plugin, the tags declared and the damage types
/// `damage.fire` and `damage.cold`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let mut prefabs = PrefabRegistry::new();
    for id in ["bolt", "target", "caster"] {
        prefabs.insert(
            id.to_string(),
            serde_json::from_value(json!({ "tag": id })).unwrap(),
        );
    }
    world.insert_resource(prefabs);
    let mut schedule = Schedule::new();
    KitPlugins::set()
        .without::<TagsPlugin>()
        .with(TagsPlugin::new().with_tags(TAGS))
        .build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "damage.fire": {}, "damage.cold": {} })).unwrap();
    let data = world.resource_mut::<DataRegistry>();
    data.insert("types", types);
    data.insert("double_by_level", Curve::new([(1.0, 20.0), (11.0, 40.0)]));
    (world, schedule)
}

/// Cinder: fire 16 to 24 and cold 4 to 8, spells, cost 12 of mana.
fn cinder() -> Ability {
    Ability::new()
        .with_cost("mana", 5.0)
        .with_cooldown(2.0)
        .with_tags(["skill.spell"])
        .with_payload(crate::Payload::Hitbox(0.1))
        .with_skill(
            Skill::new()
                .with_damage("damage.fire", 16.0, 24.0)
                .with_damage("damage.cold", 4.0, 8.0)
                .with_crit_chance(0.25)
                .with_cost(12.0)
                .with_cooldown(0.5),
        )
}

/// A caster owning `ability` as `cinder`, with mana and its stats.
fn caster(world: &mut World, ability: Ability, stats: Stats) -> Entity {
    world
        .resource_mut::<DataRegistry>()
        .insert("cinder", ability);
    world.spawn_with((
        stats.with("mana", 100.0),
        Tags::new(["team.player"]),
        Abilities::new().with(DataRef::by_id("cinder")),
    ))
}

fn sheet_of(world: &World, holder: Entity) -> Sheet {
    let ability = world
        .resource::<DataRegistry>()
        .get::<Ability>("cinder")
        .unwrap()
        .clone();
    resolve(world, holder, "cinder", &ability, ability.skill().unwrap())
}

fn sums(part: &DeliveryPart) -> (f64, f64) {
    let (_, factor, product, ..) = part.hit.sums().numbers();
    (factor - 1.0, product)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn skill_level_adds_add_lines_in_context_and_clamps_to_1_through_40() {
    let (mut world, _) = rig();
    let ability = cinder().with_skill(
        Skill::new()
            .with_damage("damage.fire", 10.0, 30.0)
            .with_level_curve("damage", DataRef::by_id("double_by_level")),
    );
    let mut stats = Stats::new();
    let spells = TagFilter::new().all(["skill.spell"]);
    let attacks = TagFilter::new().all(["skill.attack"]);
    stats.add_line(
        Stats::SKILL_LEVEL,
        "passive:lore",
        Line::new(Modifier::Add(3.0)).with(spells),
    );
    stats.add_line(
        Stats::SKILL_LEVEL,
        "passive:drill",
        Line::new(Modifier::Add(9.0)).with(attacks),
    );
    stats.add_line(
        Stats::SKILL_LEVEL,
        "passive:odd",
        Line::new(Modifier::Mul(10.0)),
    );
    let holder = caster(&mut world, ability, stats);
    world
        .get_mut::<Abilities>(holder)
        .unwrap()
        .set_level("cinder", 3);
    let sheet = sheet_of(&world, holder);
    assert_eq!(
        sheet.delivery.level(),
        6.0,
        "3 and the spell's 3; the attack's and a `mul` none"
    );
    assert_eq!(
        sheet.delivery.parts[0].range,
        [15.0, 45.0],
        "the curve's 30 at 6: × 1.5"
    );
    world
        .get_mut::<Abilities>(holder)
        .unwrap()
        .set_level("cinder", 40);
    assert_eq!(sheet_of(&world, holder).delivery.level(), 40.0, "clamped");
    let mut low = Stats::new();
    low.add_line(Stats::SKILL_LEVEL, "curse", Line::new(Modifier::Add(-9.0)));
    world.insert(holder, low.with("mana", 100.0));
    world
        .get_mut::<Abilities>(holder)
        .unwrap()
        .set_level("cinder", 2);
    assert_eq!(
        sheet_of(&world, holder).delivery.level(),
        1.0,
        "clamped below"
    );
}

#[test]
fn a_level_curve_scales_a_range_about_its_midpoint() {
    let (mut world, _) = rig();
    let ability = cinder().with_skill(
        Skill::new()
            .with_damage("damage.fire", 16.0, 24.0)
            .with_level_curve("damage", DataRef::by_id("double_by_level")),
    );
    let holder = caster(&mut world, ability, Stats::new());
    let at = |world: &World| sheet_of(world, holder).delivery.parts[0].range;
    assert_eq!(
        at(&world),
        [16.0, 24.0],
        "level 1: the curve's 20 is the midpoint"
    );
    world
        .get_mut::<Abilities>(holder)
        .unwrap()
        .set_level("cinder", 11);
    assert_eq!(at(&world), [32.0, 48.0], "level 11: 40, the spread kept");
}

#[test]
fn a_skill_local_line_reaches_only_its_skill_and_added_adds_to_its_types_range() {
    let (mut world, _) = rig();
    let holder = caster(&mut world, cinder(), Stats::new());
    world
        .resource_mut::<DataRegistry>()
        .insert("other", cinder());
    world
        .get_mut::<Abilities>(holder)
        .unwrap()
        .add(DataRef::by_id("other"));
    let abilities = world.get_mut::<Abilities>(holder).unwrap();
    abilities.add_skill_lines(
        "cinder",
        "support:twin",
        [
            SkillLine::stat("damage", Line::new(Modifier::Mul(0.8))),
            SkillLine::added("damage.fire", 4.0, 6.0),
            SkillLine::added("damage.cold", 1.0, 1.0).with(TagFilter::new().all(["skill.attack"])),
        ],
    );
    let sheet = sheet_of(&world, holder);
    let fire = &sheet.delivery.parts[1];
    assert_eq!(
        (fire.kind.as_str(), fire.range),
        ("damage.fire", [20.0, 30.0])
    );
    assert_eq!(
        sheet.delivery.parts[0].range,
        [4.0, 8.0],
        "the attack's `added` skipped"
    );
    assert!(close(sums(fire).1, 0.8));
    let other = world
        .resource::<DataRegistry>()
        .get::<Ability>("other")
        .unwrap()
        .clone();
    let theirs = resolve(&world, holder, "other", &other, other.skill().unwrap());
    assert_eq!(
        theirs.delivery.parts[1].range,
        [16.0, 24.0],
        "the other skill takes none"
    );
    assert!(close(sums(&theirs.delivery.parts[1]).1, 1.0));
    let abilities = world.get_mut::<Abilities>(holder).unwrap();
    assert_eq!(abilities.remove_skill_lines("cinder", "support:twin"), 3);
    assert_eq!(
        sheet_of(&world, holder).delivery.parts[1].range,
        [16.0, 24.0]
    );
}

#[test]
fn the_sheets_cost_and_cooldown_replace_the_abilitys() {
    let (mut world, _) = rig();
    let mut stats = Stats::new();
    stats.add_line(Stats::COST, "item:9", Line::new(Modifier::Mul(1.5)));
    let holder = caster(&mut world, cinder(), stats);
    let sheet = sheet_of(&world, holder);
    assert_eq!((sheet.cost, sheet.cooldown), (Some(18.0), 0.5));
    assert_eq!(
        crate::use_ability(&mut world, holder, "cinder"),
        crate::Used::Used
    );
    let stats = world.get::<Stats>(holder).unwrap();
    assert_eq!(
        stats.base("mana"),
        Some(82.0),
        "the sheet's 12 × 1.5, not the ability's 5"
    );
    let left = world
        .get::<Abilities>(holder)
        .unwrap()
        .cooldown_left("cinder");
    assert_eq!(left, Some(0.5));
    // Without its own, the ability's.
    let plain = Ability::new()
        .with_cost("mana", 5.0)
        .with_cooldown(2.0)
        .with_payload(crate::Payload::Hitbox(0.1))
        .with_skill(Skill::new().with_damage("damage.fire", 1.0, 1.0));
    world.resource_mut::<DataRegistry>().insert("cinder", plain);
    assert_eq!(
        (
            sheet_of(&world, holder).cost,
            sheet_of(&world, holder).cooldown
        ),
        (Some(7.5), 2.0)
    );
}

#[test]
fn a_two_part_sheet_hit_draws_twice_both_parts_at_one_u_the_crit_draw_second() {
    let (mut world, _) = rig();
    let holder = caster(&mut world, cinder(), Stats::new());
    let target = world.spawn_with((Health::new(1000.0),));
    let delivery = sheet_of(&world, holder).delivery;
    let request = DamageRequest::from_delivery(&world, target, &delivery);
    let mids: Vec<f32> = request
        .parts()
        .iter()
        .map(crate::DamagePart::amount)
        .collect();
    assert_eq!(mids, [6.0, 20.0], "the midpoints until the draw");
    world.insert_resource(KitRng::seeded(11));
    let mut stream = KitRng::seeded(11);
    let (u, roll) = (stream.unit(), stream.unit());
    request.apply(&mut world);
    assert_eq!(world.resource::<KitRng>().draws(), 2);
    let crit = roll < 0.25;
    let times = if crit { 1.5 } else { 1.0 };
    let parts: Vec<f32> = world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .map(|part| part.amount)
        .collect();
    let expect = |min: f32, max: f32| {
        ((f64::from(min) + f64::from(u) * f64::from(max - min)) * times) as f32
    };
    assert_eq!(
        parts,
        [expect(4.0, 8.0), expect(16.0, 24.0)],
        "u {u}, crit draw {roll}"
    );
    assert_eq!(
        world
            .resource::<EventQueue<Damaged>>()
            .iter()
            .next()
            .unwrap()
            .crit,
        crit
    );
}

#[test]
fn a_deferred_line_folds_in_at_the_hit_against_the_target_and_the_state() {
    let (mut world, _) = rig();
    let mut stats = Stats::new();
    let chilled = TagFilter::new().all(["status.chilled"]);
    let chained = TagFilter::new().all(["delivery.chained"]);
    stats.add_line(
        Stats::DAMAGE,
        "passive:cold_snap",
        Line::new(Modifier::Pct(0.5)).against(chilled.clone()),
    );
    stats.add_line(
        Stats::DAMAGE,
        "support:ricochet",
        Line::new(Modifier::Mul(0.5)).with(chained),
    );
    stats.add_line(
        Stats::CRIT_CHANCE,
        "passive:exploit",
        Line::new(Modifier::Add(1.0)).against(chilled),
    );
    let holder = caster(
        &mut world,
        cinder().with_skill(Skill::new().with_damage("damage.fire", 10.0, 10.0)),
        stats,
    );
    let delivery = sheet_of(&world, holder).delivery;
    assert_eq!(delivery.deferred.len(), 3);
    let warm = world.spawn_with((Health::new(1000.0),));
    let cold = world.spawn_with((Health::new(1000.0), Tags::new(["status.chilled"])));
    world.insert_resource(KitRng::seeded(1));
    let hit = |world: &mut World, target, delivery: &Delivery| {
        DamageRequest::from_delivery(world, target, delivery).apply(world);
        let sent = world
            .resource::<EventQueue<Damaged>>()
            .iter()
            .last()
            .copied()
            .unwrap();
        (sent.amount, sent.crit)
    };
    let (amount, _) = hit(&mut world, warm, &delivery);
    assert!(amount == 10.0 || amount == 15.0, "{amount}");
    assert_eq!(
        hit(&mut world, cold, &delivery),
        (22.5, true),
        "15 × 1.5, a sure crit against the chill"
    );
    let chain = delivery.clone().with_state(Tags::new(["delivery.chained"]));
    assert_eq!(hit(&mut world, cold, &chain), (11.25, true));
}

#[test]
fn a_line_on_what_a_use_spends_may_not_wait_for_the_hit() {
    let marked = TagFilter::new().all(["status.marked"]);
    for bad in [
        json!({ "stat": "cost", "modifier": { "mul": 2 }, "against": { "all": ["status.marked"] } }),
        json!({ "stat": "skill_level", "modifier": { "add": 1 }, "with": { "all": ["delivery.chained"] } }),
        json!({ "stat": "cooldown", "modifier": { "mul": 2 }, "against": { "none": ["boss"] } }),
        json!({ "added": { "damage.fire": [1, 2] }, "against": { "all": ["status.marked"] } }),
        json!({ "added": { "damage.fire": [3, 2] } }),
        json!({ "pen": { "damage.fire": 0.1 }, "added": { "damage.fire": [1, 2] } }),
    ] {
        assert!(
            serde_json::from_value::<SkillLine>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let skill = json!({ "lines": [{ "stat": "cost", "modifier": { "mul": 2 }, "against": { "all": ["x"] } }] });
    assert!(serde_json::from_value::<Skill>(skill).is_err());
    let stats = json!({ "cost": { "base": 1, "lines": [{ "modifier": { "mul": 2 }, "against": { "all": ["x"] } }] } });
    assert!(serde_json::from_value::<Stats>(stats).is_err());
    // A damage line, a crit line and a `pen` line may.
    for good in [
        json!({ "stat": "damage", "modifier": { "mul": 2 }, "against": { "all": ["status.marked"] } }),
        json!({ "stat": "crit_chance", "modifier": { "add": 0.5 }, "with": { "all": ["delivery.forked"] } }),
        json!({ "pen": { "damage.fire": 0.1 }, "against": { "all": ["status.marked"] } }),
    ] {
        assert!(
            serde_json::from_value::<SkillLine>(good.clone()).is_ok(),
            "{good}"
        );
    }
    let mut stats = Stats::new();
    let refused = std::panic::catch_unwind(move || {
        stats.add_line(
            Stats::COST,
            "x",
            Line::new(Modifier::Mul(2.0)).against(marked.clone()),
        );
    });
    assert!(refused.is_err(), "by `add_line`");
    let refused = std::panic::catch_unwind(|| {
        let mut abilities = Abilities::new();
        abilities.add_skill_lines(
            "cinder",
            "support:x",
            [SkillLine::added("damage.fire", 1.0, 2.0).against(TagFilter::new().all(["boss"]))],
        );
    });
    assert!(refused.is_err(), "by `add_skill_lines`");
}

#[test]
fn a_deliverys_hit_reads_the_holders_tags_at_the_use_for_every_against() {
    let (mut world, _) = rig();
    let mut stats = Stats::new();
    let marked = TagFilter::new().all(["status.marked"]);
    stats.add_line(
        Stats::DAMAGE,
        "passive:x",
        Line::new(Modifier::Mul(2.0)).while_holder(marked.clone()),
    );
    let holder = caster(
        &mut world,
        cinder().with_skill(Skill::new().with_damage("damage.fire", 10.0, 10.0)),
        stats,
    );
    world
        .get_mut::<Tags>(holder)
        .unwrap()
        .insert("status.marked");
    let delivery = sheet_of(&world, holder).delivery;
    world
        .get_mut::<Tags>(holder)
        .unwrap()
        .remove("status.marked");
    // A defender whose lines read the attacker: its `damage_taken` and its
    // resistance, against the marked.
    let mut taken = Stats::new().with(Stats::DAMAGE_TAKEN, 1.0);
    taken.add_line(
        Stats::DAMAGE_TAKEN,
        "base",
        Line::new(Modifier::Mul(0.5)).against(marked.clone()),
    );
    let mut resist = Resist::new();
    resist.add_line(
        "damage.fire",
        "ward",
        Line::new(Modifier::Mul(0.5)).against(marked),
    );
    let target = world.spawn_with((Health::new(1000.0), taken, resist));
    world.insert_resource(KitRng::seeded(1));
    DamageRequest::from_delivery(&world, target, &delivery).apply(&mut world);
    let amount = world
        .resource::<EventQueue<Damaged>>()
        .iter()
        .last()
        .unwrap()
        .amount;
    let crit = if amount > 5.0 { 1.5 } else { 1.0 };
    assert_eq!(
        amount,
        (5.0 * crit) as f32,
        "20 at the use, the holder marked then: × 0.5 × 0.5"
    );
}

#[test]
fn a_delivery_whose_holder_is_gone_hits_with_no_source_and_its_damage() {
    let (mut world, mut schedule) = rig();
    let holder = caster(
        &mut world,
        cinder().with_skill(Skill::new().with_damage("damage.fire", 10.0, 10.0)),
        Stats::new(),
    );
    let delivery = sheet_of(&world, holder).delivery;
    let bolt = placed(&mut world, "bolt");
    world.insert(bolt, delivery.clone());
    let target = placed(&mut world, "target");
    world.insert(target, Health::new(1000.0));
    world.despawn(holder);
    let saved = capture_world(&world).unwrap();
    let hit = |world: &mut World, target: Entity, delivery: &Delivery| {
        world.insert_resource(KitRng::seeded(4));
        DamageRequest::from_delivery(world, target, delivery).apply(world);
        let sent = world
            .resource::<EventQueue<Damaged>>()
            .iter()
            .last()
            .copied()
            .unwrap();
        (sent.amount, sent.source)
    };
    let before = hit(&mut world, target, &delivery);
    assert_eq!(before.1, None, "no source once the holder is gone");
    let (mut loaded, _) = rig();
    let (bolt, target) = (placed(&mut loaded, "bolt"), placed(&mut loaded, "target"));
    restore_world(&mut loaded, &saved, &(Loaded,)).unwrap();
    let back = loaded.get::<Delivery>(bolt).unwrap().clone();
    assert_eq!(back.holder(), None);
    assert_eq!(
        hit(&mut loaded, target, &back),
        before,
        "the same damage after a load"
    );
    let _ = &mut schedule;
}

#[test]
fn a_saved_delivery_loads_with_its_holder_and_resolves_the_same() {
    let (mut world, _) = rig();
    let mut stats = Stats::new();
    stats.add_line(
        Stats::DAMAGE,
        "passive:x",
        Line::new(Modifier::Pct(0.5)).against(TagFilter::new().all(["status.chilled"])),
    );
    let holder = caster(&mut world, cinder(), stats);
    tungsten_core::save::name(&mut world, holder, None, "caster").unwrap();
    let delivery = sheet_of(&world, holder).delivery.armed(3);
    let bolt = placed(&mut world, "bolt");
    world.insert(bolt, delivery.clone());
    let saved = capture_world(&world).unwrap();
    let (mut loaded, _) = rig();
    loaded
        .resource_mut::<DataRegistry>()
        .insert("cinder", cinder());
    let (caster_b, bolt_b) = (placed(&mut loaded, "caster"), placed(&mut loaded, "bolt"));
    restore_world(&mut loaded, &saved, &(Loaded,)).unwrap();
    let back = loaded.get::<Delivery>(bolt_b).unwrap().clone();
    assert_eq!(back.holder(), Some(caster_b));
    let mut expected = delivery.clone();
    expected.holder = Some(caster_b);
    assert_eq!(back, expected, "every snapshot field");
    let cold =
        |world: &mut World| world.spawn_with((Health::new(1000.0), Tags::new(["status.chilled"])));
    let (a, b) = (cold(&mut world), cold(&mut loaded));
    for (world, target, delivery) in [(&mut world, a, &delivery), (&mut loaded, b, &back)] {
        world.insert_resource(KitRng::seeded(9));
        DamageRequest::from_delivery(world, target, delivery).apply(world);
    }
    let amount = |world: &World| {
        world
            .resource::<EventQueue<Damaged>>()
            .iter()
            .last()
            .unwrap()
            .amount
    };
    assert_eq!(amount(&world), amount(&loaded));
}

#[test]
fn an_ability_without_a_skill_uses_as_before() {
    let (mut world, _) = rig();
    let plain = Ability::new()
        .with_cost("mana", 5.0)
        .with_cooldown(2.0)
        .with_payload(crate::Payload::Hitbox(0.1));
    let holder = caster(&mut world, plain, Stats::new());
    assert_eq!(
        crate::use_ability(&mut world, holder, "cinder"),
        crate::Used::Used
    );
    assert_eq!(world.get::<Stats>(holder).unwrap().base("mana"), Some(95.0));
    assert_eq!(
        world
            .get::<Abilities>(holder)
            .unwrap()
            .cooldown_left("cinder"),
        Some(2.0)
    );
    assert!(world.get::<Delivery>(holder).is_none());
    damage(&mut world, holder, 0.0);
}

// 7.10a, Step 6: conversion and extra with history; §9.1.

/// A skill of `damage` (type to range) and `lines`, its sheet resolved for
/// a holder with `stats`.
fn sheet_with(
    world: &mut World,
    damage: &[(&str, f32, f32)],
    lines: Vec<SkillLine>,
    stats: Stats,
) -> Delivery {
    let mut skill = Skill::new();
    for &(kind, min, max) in damage {
        skill = skill.with_damage(kind, min, max);
    }
    for line in lines {
        skill = skill.with_line(line);
    }
    let holder = caster(world, cinder().with_skill(skill), stats);
    sheet_of(world, holder).delivery
}

/// The parts' types, histories and ranges.
fn shape(delivery: &Delivery) -> Vec<(String, Vec<String>, [f32; 2])> {
    delivery
        .parts
        .iter()
        .map(|part| (part.kind.clone(), part.history.clone(), part.range))
        .collect()
}

fn fire() -> String {
    "damage.fire".to_string()
}

fn cold() -> String {
    "damage.cold".to_string()
}

#[test]
fn a_converted_share_keeps_its_history_and_never_converts_again() {
    let (mut world, _) = rig();
    let delivery = sheet_with(
        &mut world,
        &[("damage.fire", 10.0, 20.0)],
        vec![
            SkillLine::convert("damage.fire", "damage.cold", 0.5),
            SkillLine::convert("damage.cold", "damage.fire", 1.0),
        ],
        Stats::new(),
    );
    assert_eq!(
        shape(&delivery),
        [
            (fire(), vec![], [5.0, 10.0]),
            (cold(), vec![fire()], [5.0, 10.0])
        ],
        "one pass: the cold share stays cold"
    );
}

#[test]
fn shares_past_one_normalize_and_equal_parts_merge() {
    let (mut world, _) = rig();
    let delivery = sheet_with(
        &mut world,
        &[("damage.fire", 10.0, 10.0)],
        vec![
            SkillLine::convert("damage.fire", "damage.cold", 0.75),
            SkillLine::convert("damage.fire", "damage.cold", 0.75),
        ],
        Stats::new(),
    );
    assert_eq!(
        shape(&delivery),
        [(cold(), vec![fire()], [10.0, 10.0])],
        "1.5 normalized, merged"
    );
    // A native cold part and a converted one stay apart; two natives
    // merge.
    let delivery = sheet_with(
        &mut world,
        &[("damage.fire", 10.0, 10.0), ("damage.cold", 2.0, 2.0)],
        vec![
            SkillLine::convert("damage.fire", "damage.cold", 0.5),
            SkillLine::added("damage.cold", 1.0, 1.0),
        ],
        Stats::new(),
    );
    assert_eq!(
        shape(&delivery),
        [
            (cold(), vec![], [3.0, 3.0]),
            (fire(), vec![], [5.0, 5.0]),
            (cold(), vec![fire()], [5.0, 5.0]),
        ]
    );
}

#[test]
fn extra_parts_come_from_every_part_carrying_from_and_make_none() {
    let (mut world, _) = rig();
    let delivery = sheet_with(
        &mut world,
        &[("damage.fire", 10.0, 10.0)],
        vec![
            SkillLine::convert("damage.fire", "damage.cold", 0.5),
            SkillLine::extra("damage.fire", "damage.cold", 0.5),
            SkillLine::extra("damage.cold", "damage.fire", 0.5),
        ],
        Stats::new(),
    );
    // Fire to cold from the fire part (merging into the converted cold) and
    // from the converted cold (fire by its history); cold to fire from the
    // converted cold alone, none from the extra cold.
    let both = vec![cold(), fire()];
    assert_eq!(
        shape(&delivery),
        [
            (fire(), vec![], [5.0, 5.0]),
            (cold(), vec![fire()], [7.5, 7.5]),
            (cold(), both.clone(), [2.5, 2.5]),
            (fire(), both, [2.5, 2.5]),
        ]
    );
}

#[test]
fn a_line_reaches_a_part_by_its_history_and_pen_by_its_type() {
    let (mut world, _) = rig();
    let mut stats = Stats::new();
    let by_fire = TagFilter::new().any(["damage.fire"]);
    stats.add_line(
        Stats::DAMAGE,
        "item:searing",
        Line::new(Modifier::Pct(0.5)).with(by_fire.clone()),
    );
    let delivery = sheet_with(
        &mut world,
        &[("damage.fire", 10.0, 10.0), ("damage.cold", 4.0, 4.0)],
        vec![
            SkillLine::convert("damage.fire", "damage.cold", 0.5),
            SkillLine::pen("damage.cold", 0.25).with(by_fire.clone()),
        ],
        stats,
    );
    let parts: Vec<(String, Vec<String>, f64, f32)> = delivery
        .parts
        .iter()
        .map(|part| {
            (
                part.kind.clone(),
                part.history.clone(),
                sums(part).0,
                part.pen,
            )
        })
        .collect();
    assert_eq!(
        parts,
        [
            (cold(), vec![], 0.0, 0.0),
            (fire(), vec![], 0.5, 0.0),
            (cold(), vec![fire()], 0.5, 0.25),
        ],
        "searing reaches the converted cold by its history; the cold `pen` with fire only that one"
    );
    // A defender's `damage_taken` so scoped halves the fire and the
    // converted cold, not the native cold.
    let mut taken = Stats::new().with(Stats::DAMAGE_TAKEN, 1.0);
    taken.add_line(
        Stats::DAMAGE_TAKEN,
        "base",
        Line::new(Modifier::Mul(0.5)).with(by_fire),
    );
    let target = world.spawn_with((Health::new(1000.0), taken));
    world.insert_resource(KitRng::seeded(7228));
    DamageRequest::from_delivery(&world, target, &delivery).apply(&mut world);
    let landed: Vec<f32> = world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(landed[0], 4.0, "native cold, whole");
    assert!(
        (landed[1] - 3.75).abs() < 1e-4,
        "fire 5 × 1.5, halved: {landed:?}"
    );
    assert!(
        (landed[2] - 3.75).abs() < 1e-4,
        "converted cold 5 × 1.5, halved: {landed:?}"
    );
}

#[test]
fn a_convert_or_extra_line_may_not_wait_for_the_hit() {
    for bad in [
        json!({ "convert": { "from": "damage.fire", "to": "damage.cold", "pct": 0.5 }, "against": { "all": ["boss"] } }),
        json!({ "extra": { "from": "damage.fire", "to": "damage.cold", "pct": 0.5 }, "with": { "all": ["delivery.chained"] } }),
        json!({ "convert": { "from": "damage.fire", "to": "damage.fire", "pct": 0.5 } }),
        json!({ "extra": { "from": "damage.fire", "to": "damage.cold", "pct": 1.5 } }),
    ] {
        assert!(
            serde_json::from_value::<SkillLine>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let good = json!({ "extra": { "from": "damage.fire", "to": "damage.cold", "pct": 0.25 }, "with": { "all": ["skill.spell"] } });
    let line: SkillLine = serde_json::from_value(good.clone()).unwrap();
    assert_eq!(line.as_extra(), Some(("damage.fire", "damage.cold", 0.25)));
    assert_eq!(serde_json::to_value(&line).unwrap(), good);
    let refused = std::panic::catch_unwind(|| {
        SkillLine::convert("damage.fire", "damage.cold", 0.5)
            .against(TagFilter::new().all(["boss"]))
    });
    assert!(refused.is_err());
}

#[test]
fn a_request_without_a_sheet_converts_as_before() {
    let (mut world, _) = rig();
    let target = world.spawn_with((Health::new(1000.0),));
    DamageRequest::from_parts(
        target,
        [crate::DamagePart::new(8.0).with_tags(Tags::new(["damage.fire"]))],
    )
    .with_conversion(
        crate::Conversion::new()
            .with_rule("damage.fire", "damage.cold", 0.75)
            .with_rule("damage.fire", "damage.cold", 0.75),
    )
    .apply(&mut world);
    let parts: Vec<f32> = world
        .resource::<EventQueue<DamagedPart>>()
        .iter()
        .map(|part| part.amount)
        .collect();
    assert_eq!(
        parts,
        [8.0],
        "both shares normalized into one cold part, as before"
    );
}

/// §9.1's caster: Cinder Volley at fire 16 to 24 with `threefold`,
/// `ricochet` and `rimecraft` as skill-local `support:` lines, the
/// passives, the wand's and the status's lines on its stats, and the
/// rings' `pen` and, with `ring`, the unique's `extra` as holder lines.
fn nine_one(world: &mut World, ring: bool) -> Entity {
    let all = |tag: &str| TagFilter::new().all([tag]);
    let any = |tag: &str| TagFilter::new().any([tag]);
    let mut stats = Stats::new();
    for (source, modifier, filter) in [
        (
            "passive:spellcraft",
            Modifier::Pct(0.40),
            all("skill.spell"),
        ),
        (
            "item:9137 implicit",
            Modifier::Pct(0.08),
            all("skill.spell"),
        ),
        (
            "item:9137 spellwoven",
            Modifier::Pct(0.26),
            all("skill.spell"),
        ),
        ("item:9137 searing", Modifier::Pct(0.22), any("damage.fire")),
        (
            "passive:frostcraft",
            Modifier::Pct(0.20),
            any("damage.cold"),
        ),
        (
            "passive:marksman",
            Modifier::Pct(0.25),
            all("skill.projectile"),
        ),
        ("passive:brawler", Modifier::Pct(0.30), all("skill.melee")),
        ("status#41", Modifier::Mul(1.1), any("damage.fire")),
    ] {
        stats.add_line(Stats::DAMAGE, source, Line::new(modifier).with(filter));
    }
    stats.add_line(
        Stats::DAMAGE,
        "passive:cold_snap",
        Line::new(Modifier::Pct(0.20)).against(all("status.chilled")),
    );
    stats.add_line(
        Stats::CRIT_CHANCE,
        "passive:precision",
        Line::new(Modifier::Pct(0.5)),
    );
    stats.add_line(
        Stats::CRIT_MULTIPLIER,
        "passive:deadeye",
        Line::new(Modifier::Add(0.4)),
    );
    let volley = Ability::new()
        .with_cost("mana", 1.0)
        .with_tags(["skill.spell", "skill.projectile", "skill.fire"])
        .with_payload(crate::Payload::Hitbox(0.1))
        .with_skill(
            Skill::new()
                .with_damage("damage.fire", 16.0, 24.0)
                .with_crit_chance(0.06)
                .with_cost(12.0),
        );
    let holder = caster(world, volley, stats);
    let abilities = world.get_mut::<Abilities>(holder).unwrap();
    abilities.add_skill_lines(
        "cinder",
        "support:threefold",
        [
            SkillLine::stat("damage", Line::new(Modifier::Mul(0.8))),
            SkillLine::stat("cost", Line::new(Modifier::Mul(1.3))),
        ],
    );
    abilities.add_skill_lines(
        "cinder",
        "support:ricochet",
        [
            SkillLine::stat(
                "damage",
                Line::new(Modifier::Mul(0.85)).with(all("delivery.chained")),
            ),
            SkillLine::stat("cost", Line::new(Modifier::Mul(1.2))),
        ],
    );
    abilities.add_skill_lines(
        "cinder",
        "support:rimecraft",
        [
            SkillLine::convert("damage.fire", "damage.cold", 0.5),
            SkillLine::stat("cost", Line::new(Modifier::Mul(1.1))),
        ],
    );
    abilities.add_lines("item:ring", [SkillLine::pen("damage.fire", 0.10)]);
    if ring {
        abilities.add_lines(
            "item:9140",
            [SkillLine::extra("damage.cold", "damage.fire", 0.18)],
        );
    }
    holder
}

/// A chilled fen brute: fire 0.6, cold 0.75, no floors.
fn brute(world: &mut World) -> Entity {
    world.spawn_with((
        Health::new(100_000.0),
        Tags::new(["status.chilled"]),
        Resist::new()
            .with_factor("damage.fire", 0.6)
            .with_factor("damage.cold", 0.75),
    ))
}

/// The hit `delivery` lands on `target` under `seed`: its amount and crit.
fn landing(world: &mut World, target: Entity, delivery: &Delivery, seed: u64) -> (f32, bool) {
    world.insert_resource(KitRng::seeded(seed));
    DamageRequest::from_delivery(world, target, delivery).apply(world);
    let hit = world
        .resource::<EventQueue<Damaged>>()
        .iter()
        .last()
        .copied()
        .unwrap();
    (hit.amount, hit.crit)
}

/// §9.1's pinned seeds: the first draw within 10⁻⁴ of 0.5 at both, the
/// second missing a 0.09 chance at the first and making it at the second.
const PLAIN: u64 = 7228;
const CRIT: u64 = 2638;

#[test]
fn nine_one_hits_at_32_07_crits_at_60_94_chains_at_27_26_and_the_ring_adds_2_89() {
    for (seed, crit) in [(PLAIN, false), (CRIT, true)] {
        let mut rng = KitRng::seeded(seed);
        let (u, roll) = (rng.unit(), rng.unit());
        assert!(
            (u - 0.5).abs() < 1e-4 && (roll < 0.09) == crit,
            "seed {seed}: {u}, {roll}"
        );
    }
    let (mut world, _) = rig();
    let holder = nine_one(&mut world, false);
    let sheet = sheet_of(&world, holder);
    let delivery = sheet.delivery;
    let chance = delivery.crit_chance.sums().value(0.0, 1.0);
    let multiplier = delivery.crit_multiplier.sums().value(1.0, f32::INFINITY);
    assert!((chance - 0.09).abs() < 1e-6, "{chance}");
    assert!((multiplier - 1.9).abs() < 1e-6, "{multiplier}");
    let cost = sheet.cost.unwrap();
    assert!((cost - 20.59).abs() < 0.005, "12 × 1.3 × 1.2 × 1.1: {cost}");
    assert_eq!(
        shape(&delivery),
        [
            (fire(), vec![], [8.0, 12.0]),
            (cold(), vec![fire()], [8.0, 12.0])
        ],
        "half the fire to cold"
    );
    let near = |got: f32, want: f32| (got - want).abs() < 0.005;
    let target = brute(&mut world);
    let (hit, crit) = landing(&mut world, target, &delivery, PLAIN);
    assert!(near(hit, 32.0716) && !crit, "the first hit: {hit}");
    let (critical, crit) = landing(&mut world, target, &delivery, CRIT);
    assert!(near(critical, 60.94) && crit, "on a crit: {critical}");
    let chained = delivery.clone().with_state(Tags::new(["delivery.chained"]));
    let (hit_chained, _) = landing(&mut world, target, &chained, PLAIN);
    assert!(near(hit_chained, 27.26), "a chained hit: {hit_chained}");
    // Worn with the unique ring: 18 % of cold as extra fire.
    let (mut ringed, _) = rig();
    let holder = nine_one(&mut ringed, true);
    let delivery = sheet_of(&ringed, holder).delivery;
    let target = brute(&mut ringed);
    let (with_ring, _) = landing(&mut ringed, target, &delivery, PLAIN);
    assert!(
        near(with_ring - hit, 2.89),
        "the ring's extra fire: {}",
        with_ring - hit
    );
}
