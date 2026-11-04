use super::*;
use serde_json::json;
use tungsten_core::physics::PrevPosition;
use tungsten_core::{AssetRegistry, Bundle, ComponentRegistry, PrefabData, Schedule, Stage};

use crate::damage::effects::{Effect, Effects, EffectsPlugin};
use crate::damage::health::HealthPlugin;
use crate::foundation::stats::StatsPlugin;
use crate::foundation::tags::TagsPlugin;
use crate::foundation::trace::KitTrace;
use crate::intent::sheet::SkillLine;
use crate::motion::knockback::Knockback;
use crate::motion::linear::LinearMotionPlugin;
use crate::overlap::hit::HitShape;
use crate::react::counters::{Counters, CountersPlugin};
use crate::react::cues::{CueAction, Cues, CuesPlugin};
use crate::{KIT_INTENT, KIT_MOVE};

const STEP: f32 = 1.0 / 60.0;

/// A World with time, a command buffer, a trace, the prefabs `bolt` (with
/// a `linear_motion`) and `puff` (without), and the plugins the abilities
/// read: tags, stats (`mana`), health, linear motion, effects, counters,
/// cues and abilities.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy", "team.player", "status.swift"])
        .build(&mut schedule, &mut world);
    StatsPlugin::new()
        .with_stat("mana", 0.0, 100.0)
        .build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    CountersPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    AbilitiesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let mut prefabs = PrefabRegistry::new();
    let bolt: PrefabData = serde_json::from_value(json!({
        "tag": "bolt",
        "components": { "linear_motion": { "velocity": [100, 0] } }
    }))
    .unwrap();
    prefabs.insert("bolt".to_string(), bolt);
    let puff: PrefabData = serde_json::from_value(json!({ "tag": "puff" })).unwrap();
    prefabs.insert("puff".to_string(), puff);
    world.insert_resource(prefabs);
    (world, schedule)
}

/// One fixed step at the frame's `dt`, then the command flush that ends
/// the frame.
fn step(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
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

/// Inserts `ability` as the `abilities` entry `id`.
fn insert(world: &mut World, id: &str, ability: Ability) -> DataRef<Ability> {
    world.resource_mut::<DataRegistry>().insert(id, ability);
    DataRef::by_id(id)
}

/// An entity at `at` with `rest`.
fn at(world: &mut World, x: f32, y: f32, rest: impl Bundle) -> Entity {
    world.spawn_with(
        (
            Position(Vec2::new(x, y)),
            PrevPosition(Vec2::new(x, y)),
            Transform::from_position(Vec2::new(x, y)),
        )
            .with(rest),
    )
}

fn uses(world: &World) -> Vec<(Entity, String, Option<Entity>)> {
    world
        .resource::<EventQueue<AbilityUsed>>()
        .iter()
        .map(|event| (event.entity, event.name.clone(), event.target))
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

/// The entities tagged `bolt`, with their linear velocity.
fn bolts(world: &World) -> Vec<(Entity, Vec2)> {
    world
        .query::<(Entity, &tungsten_core::Tag, &LinearMotion)>()
        .filter(|(_, tag, _)| tag.name == "bolt")
        .map(|(entity, _, motion)| (entity, motion.velocity()))
        .collect()
}

fn mana(world: &World, entity: Entity) -> Option<f32> {
    world
        .get::<Stats>(entity)
        .and_then(|stats| stats.base("mana"))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn the_plugin_registers_its_kind_queue_and_system_in_the_intent_phase() {
    let (world, schedule) = rig();
    assert!(world.has_event::<AbilityUsed>());
    let data = world.resource::<DataRegistry>();
    assert_eq!(data.kind_of::<Ability>(), Some(ABILITIES));
    assert!(
        world
            .resource::<ComponentRegistry>()
            .names()
            .any(|n| n == ABILITIES)
    );
    let names: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
    assert!(at(KIT_INTENT) < at(ABILITY_TICK) && at(ABILITY_TICK) < at(KIT_MOVE));
}

#[test]
fn each_trigger_form_fires_as_written() {
    let (mut world, mut schedule) = rig();
    let manual = insert(
        &mut world,
        "manual",
        Ability::new().with_payload(Payload::Hitbox(0.1)),
    );
    let pressed = insert(
        &mut world,
        "pressed",
        Ability::new()
            .with_trigger(Trigger::Action("attack".to_string()))
            .with_payload(Payload::Hitbox(0.1)),
    );
    let timed = insert(
        &mut world,
        "timed",
        Ability::new()
            .with_trigger(Trigger::Interval(3.0 * STEP))
            .with_payload(Payload::Hitbox(0.1)),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(manual).with(pressed).with(timed),
            Intent::default(),
        ),
    );
    let mut used = Vec::new();
    for i in 1..=7 {
        world
            .get_mut::<Intent>(hero)
            .unwrap()
            .set_slot("attack", i == 2, i == 2);
        steps(&mut world, &mut schedule, 1);
        used.extend(uses(&world).into_iter().map(|(_, name, _)| (i, name)));
    }
    assert_eq!(
        used,
        [
            (2, "pressed".to_string()),
            (3, "timed".to_string()),
            (6, "timed".to_string()),
        ],
        "manual never fires on its own, action on the press, interval every three steps"
    );
    assert_eq!(use_ability(&mut world, hero, "manual"), Used::Used);
    // A long frame fires an interval at most once.
    let before = uses(&world).len();
    step(&mut world, &mut schedule, 10.0 * STEP);
    assert_eq!(uses(&world).len() - before, 1);
    // A held slot that was not just pressed does nothing.
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_slot("attack", true, false);
    step(&mut world, &mut schedule, STEP);
    assert!(uses(&world).iter().all(|(_, name, _)| name != "pressed"));
}

#[test]
fn a_cost_is_refused_below_the_base_and_spent_from_it() {
    let (mut world, mut schedule) = rig();
    let bolt = insert(
        &mut world,
        "bolt",
        Ability::new()
            .with_cost("mana", 4.0)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let mage = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(bolt.clone()),
            Stats::new().with("mana", 10.0),
        ),
    );
    assert_eq!(use_ability(&mut world, mage, "bolt"), Used::Used);
    assert_eq!(use_ability(&mut world, mage, "bolt"), Used::Used);
    assert_eq!(mana(&world, mage), Some(2.0));
    assert_eq!(use_ability(&mut world, mage, "bolt"), Used::NoCost);
    assert_eq!(mana(&world, mage), Some(2.0), "nothing spent");
    let poor = at(&mut world, 0.0, 0.0, (Abilities::new().with(bolt),));
    assert_eq!(
        use_ability(&mut world, poor, "bolt"),
        Used::NoCost,
        "no stat at all"
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(uses(&world).len(), 2);
}

#[test]
fn a_scaled_cost_spends_the_amount_times_the_users_stat() {
    let (mut world, _schedule) = rig();
    let bolt = insert(
        &mut world,
        "bolt",
        Ability::new()
            .with_cost("mana", 4.0)
            .with_cost_scale("spell_cost")
            .with_payload(Payload::Hitbox(0.1)),
    );
    let mage = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(bolt.clone()),
            Stats::new().with("mana", 10.0).with("spell_cost", 2.0),
        ),
    );
    let ability = world
        .resource::<DataRegistry>()
        .get::<Ability>("bolt")
        .unwrap()
        .clone();
    assert_eq!(ability.cost(), Some(("mana", 4.0)), "the data's amount");
    assert_eq!(ability.cost_scale(), Some("spell_cost"));
    assert_eq!(
        ability.cost_for(world.get::<Stats>(mage)),
        Some(("mana", 8.0))
    );
    assert_eq!(use_ability(&mut world, mage, "bolt"), Used::Used);
    assert_eq!(mana(&world, mage), Some(2.0), "twice the amount");
    // 1.9 times the amount is short of twice it.
    world
        .get_mut::<Stats>(mage)
        .unwrap()
        .set_base("mana", 4.0 * 1.9);
    assert_eq!(use_ability(&mut world, mage, "bolt"), Used::NoCost);
    assert!(close(mana(&world, mage).unwrap(), 7.6), "nothing spent");
    // A user without the scale stat pays the amount, and a negative scale
    // nothing.
    let plain = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(bolt.clone()),
            Stats::new().with("mana", 10.0),
        ),
    );
    assert_eq!(use_ability(&mut world, plain, "bolt"), Used::Used);
    assert_eq!(mana(&world, plain), Some(6.0));
    assert_eq!(ability.cost_for(None), Some(("mana", 4.0)));
    let odd = Stats::new().with("spell_cost", -1.0);
    assert_eq!(ability.cost_for(Some(&odd)), Some(("mana", 0.0)));
    // The data form, with and without the scale.
    let scaled: Ability = serde_json::from_value(json!({
        "cost": { "stat": "mana", "amount": 4, "scale": "spell_cost" },
        "payload": [{ "impulse": 1 }]
    }))
    .unwrap();
    assert_eq!(
        scaled,
        Ability::new()
            .with_cost("mana", 4.0)
            .with_cost_scale("spell_cost")
            .with_payload(Payload::Impulse(1.0))
    );
    let back = serde_json::to_value(&scaled).unwrap();
    assert_eq!(
        back["cost"],
        json!({ "stat": "mana", "amount": 4.0, "scale": "spell_cost" })
    );
    assert_eq!(serde_json::from_value::<Ability>(back).unwrap(), scaled);
    let unscaled = serde_json::to_value(
        Ability::new()
            .with_cost("mana", 4.0)
            .with_payload(Payload::Impulse(1.0)),
    )
    .unwrap();
    assert_eq!(unscaled["cost"], json!({ "stat": "mana", "amount": 4.0 }));
    assert_eq!(
        serde_json::from_value::<Ability>(unscaled)
            .unwrap()
            .cost_scale(),
        None
    );
    assert!(
        serde_json::from_value::<Ability>(json!({
            "cost": { "stat": "mana", "amount": 4, "scale": "Spell" },
            "payload": [{ "impulse": 1 }]
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| Ability::new()
            .with_cost("mana", 1.0)
            .with_cost_scale("Spell"))
        .is_err()
    );
    assert!(std::panic::catch_unwind(|| Ability::new().with_cost_scale("spell_cost")).is_err());
}

#[test]
fn a_cooldown_counts_at_the_cooldown_rate() {
    let (mut world, mut schedule) = rig();
    let swing = insert(
        &mut world,
        "swing",
        Ability::new()
            .with_cooldown(6.0 * STEP)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let slow = at(
        &mut world,
        0.0,
        0.0,
        (Abilities::new().with(swing.clone()),),
    );
    let fast = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(swing),
            Stats::new().with(Stats::COOLDOWN_RATE, 2.0),
        ),
    );
    assert_eq!(use_ability(&mut world, slow, "swing"), Used::Used);
    assert_eq!(use_ability(&mut world, fast, "swing"), Used::Used);
    assert_eq!(use_ability(&mut world, slow, "swing"), Used::NotReady);
    let mut ready_at = (None, None);
    for i in 1..=8 {
        steps(&mut world, &mut schedule, 1);
        let abilities = |entity| world.get::<Abilities>(entity).unwrap();
        if ready_at.0.is_none() && abilities(slow).ready("swing") {
            ready_at.0 = Some(i);
        }
        if ready_at.1.is_none() && abilities(fast).ready("swing") {
            ready_at.1 = Some(i);
        }
    }
    assert_eq!(ready_at, (Some(6), Some(3)), "half the steps at rate 2");
    assert!(close(
        world
            .get::<Abilities>(slow)
            .unwrap()
            .cooldown_left("swing")
            .unwrap(),
        0.0
    ));
}

#[test]
fn an_all_refill_waits_for_the_last_charge_spent_in_another_step() {
    let (mut world, mut schedule) = rig();
    let all = insert(
        &mut world,
        "all",
        Ability::new()
            .with_charges(3, 2.0 * STEP, true)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let hero = at(&mut world, 0.0, 0.0, (Abilities::new().with(all),));
    steps(&mut world, &mut schedule, 1);
    let charges = |world: &World| world.get::<Abilities>(hero).unwrap().charges("all");
    let mut seen = Vec::new();
    for _ in 0..3 {
        assert_eq!(use_ability(&mut world, hero, "all"), Used::Used);
        // Three steps between uses, longer than the two-step refill.
        for _ in 0..3 {
            steps(&mut world, &mut schedule, 1);
            seen.push(charges(&world));
        }
    }
    assert_eq!(
        seen,
        [
            Some(2),
            Some(2),
            Some(2),
            Some(1),
            Some(1),
            Some(1),
            Some(0),
            Some(3),
            Some(3),
        ],
        "none back while one is left; all back two steps after the last"
    );
}

#[test]
fn charges_are_spent_and_refilled_one_by_one_or_all_at_once() {
    let (mut world, mut schedule) = rig();
    let each = insert(
        &mut world,
        "each",
        Ability::new()
            .with_charges(3, 2.0 * STEP, false)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let all = insert(
        &mut world,
        "all",
        Ability::new()
            .with_charges(3, 2.0 * STEP, true)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (Abilities::new().with(each).with(all),),
    );
    steps(&mut world, &mut schedule, 1);
    let charges = |world: &World, name: &str| world.get::<Abilities>(hero).unwrap().charges(name);
    assert_eq!(
        (charges(&world, "each"), charges(&world, "all")),
        (Some(3), Some(3))
    );
    for _ in 0..3 {
        assert_eq!(use_ability(&mut world, hero, "each"), Used::Used);
        assert_eq!(use_ability(&mut world, hero, "all"), Used::Used);
    }
    assert_eq!(use_ability(&mut world, hero, "each"), Used::NoCharge);
    assert_eq!(use_ability(&mut world, hero, "all"), Used::NoCharge);
    let mut seen = Vec::new();
    for _ in 0..6 {
        steps(&mut world, &mut schedule, 1);
        seen.push((charges(&world, "each"), charges(&world, "all")));
    }
    assert_eq!(
        seen,
        [
            (Some(0), Some(0)),
            (Some(1), Some(3)),
            (Some(1), Some(3)),
            (Some(2), Some(3)),
            (Some(2), Some(3)),
            (Some(3), Some(3)),
        ],
        "one back every two steps; all back together two steps after the last"
    );
    // A use with charges left starts the refill at once; a refill makes
    // it ready with every charge.
    assert_eq!(use_ability(&mut world, hero, "each"), Used::Used);
    steps(&mut world, &mut schedule, 2);
    assert_eq!(charges(&world, "each"), Some(3));
    use_ability(&mut world, hero, "each");
    assert!(world.get_mut::<Abilities>(hero).unwrap().refill("each"));
    assert!(!world.get_mut::<Abilities>(hero).unwrap().refill("nothing"));
    assert_eq!(
        charges(&world, "each"),
        None,
        "full, counted at the next tick"
    );
    assert!(world.get::<Abilities>(hero).unwrap().ready("each"));
    steps(&mut world, &mut schedule, 1);
    assert_eq!(charges(&world, "each"), Some(3));
}

#[test]
fn nearest_and_random_take_live_targets_only_and_random_repeats_under_a_seed() {
    let (mut world, mut schedule) = rig();
    let enemies = TagFilter::new().all(["team.enemy"]);
    let nearest = insert(
        &mut world,
        "nearest",
        Ability::new()
            .with_target(AbilityTarget::Nearest {
                filter: enemies.clone(),
                range: Some(200.0),
            })
            .with_payload(Payload::Spawn {
                prefab: "bolt".to_string(),
                speed: 100.0,
                offset: 0.0,
            })
            .with_cost("mana", 1.0),
    );
    let random = insert(
        &mut world,
        "random",
        Ability::new()
            .with_target(AbilityTarget::Random {
                filter: enemies,
                range: Some(200.0),
            })
            .with_payload(Payload::Effect(DataRef::inline(
                Effect::new()
                    .with_duration(1.0)
                    .with_grant(["status.swift"]),
            ))),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(nearest).with(random),
            Stats::new().with("mana", 10.0),
        ),
    );
    let corpse = at(
        &mut world,
        20.0,
        0.0,
        (Tags::new(["team.enemy"]), Health::new(1.0)),
    );
    crate::damage(&mut world, corpse, 5.0);
    let far = at(
        &mut world,
        0.0,
        60.0,
        (Tags::new(["team.enemy"]), Health::new(1.0)),
    );
    let out_of_range = at(&mut world, 500.0, 0.0, (Tags::new(["team.enemy"]),));
    assert_eq!(use_ability(&mut world, hero, "nearest"), Used::Used);
    steps(&mut world, &mut schedule, 1);
    let [(_, velocity)] = bolts(&world)[..] else {
        panic!("one bolt");
    };
    assert!(
        (velocity - Vec2::new(0.0, 100.0)).length() < 1e-4,
        "toward the live enemy, past the nearer corpse: {velocity}"
    );
    assert_eq!(uses(&world)[0].2, Some(far));
    assert_eq!(mana(&world, hero), Some(9.0));
    // No live target in range spends nothing.
    world.despawn(far);
    assert_eq!(use_ability(&mut world, hero, "nearest"), Used::NoTarget);
    assert_eq!(mana(&world, hero), Some(9.0));
    assert!(world.is_alive(out_of_range));
    // Random: the same seed picks the same targets.
    let mut picks = Vec::new();
    for seed in [7, 7, 8] {
        let (mut world, _) = rig();
        world.insert_resource(KitRng::seeded(seed));
        let hero = at(
            &mut world,
            0.0,
            0.0,
            (Abilities::new().with(DataRef::by_id("random")),),
        );
        insert(
            &mut world,
            "random",
            Ability::new()
                .with_target(AbilityTarget::Random {
                    filter: TagFilter::new().all(["team.enemy"]),
                    range: None,
                })
                .with_payload(Payload::Effect(DataRef::inline(
                    Effect::new().with_grant(["status.swift"]),
                ))),
        );
        let foes: Vec<Entity> = (0..5)
            .map(|i| {
                at(
                    &mut world,
                    i as f32 * 10.0,
                    0.0,
                    (Tags::new(["team.enemy"]),),
                )
            })
            .collect();
        let mut picked = Vec::new();
        for _ in 0..6 {
            assert_eq!(use_ability(&mut world, hero, "random"), Used::Used);
            let target = uses(&world).last().unwrap().2.unwrap();
            picked.push(foes.iter().position(|&foe| foe == target).unwrap());
        }
        picks.push(picked);
    }
    assert_eq!(picks[0], picks[1], "one seed, one stream");
    assert_ne!(picks[0], picks[2]);
    assert!(picks[0].iter().any(|&pick| pick != picks[0][0]), "a spread");
}

#[test]
fn aim_with_no_direction_spends_nothing_and_each_payload_lands() {
    let (mut world, mut schedule) = rig();
    let swift = DataRef::inline(
        Effect::new()
            .with_duration(1.0)
            .with_grant(["status.swift"]),
    );
    let volley = insert(
        &mut world,
        "volley",
        Ability::new()
            .with_target(AbilityTarget::Aim)
            .with_cost("mana", 1.0)
            .with_payload(Payload::Spawn {
                prefab: "bolt".to_string(),
                speed: 300.0,
                offset: 10.0,
            })
            .with_payload(Payload::Effect(swift))
            .with_payload(Payload::Impulse(50.0))
            .with_payload(Payload::Hitbox(0.5)),
    );
    let hero = at(
        &mut world,
        10.0,
        20.0,
        (
            Abilities::new().with(volley),
            Stats::new().with("mana", 5.0),
            Intent::default(),
            Knockback::new(0.2),
            Hitbox::new(HitShape::circle(4.0), 1.0),
        ),
    );
    assert_eq!(use_ability(&mut world, hero, "volley"), Used::NoTarget);
    assert_eq!(
        mana(&world, hero),
        Some(5.0),
        "nothing spent without a direction"
    );
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_aim(Some(Vec2::new(0.0, -2.0)));
    assert_eq!(use_ability(&mut world, hero, "volley"), Used::Used);
    assert_eq!(mana(&world, hero), Some(4.0));
    steps(&mut world, &mut schedule, 1);
    let [(bolt, velocity)] = bolts(&world)[..] else {
        panic!("one bolt");
    };
    assert!(
        (velocity - Vec2::new(0.0, -300.0)).length() < 1e-4,
        "{velocity}"
    );
    let spawned_at = world.get::<Transform>(bolt).unwrap().position;
    assert!(
        (spawned_at - Vec2::new(10.0, 10.0)).length() < 1.0,
        "at the point plus the offset along the aim: {spawned_at}"
    );
    assert!(
        world.get::<Effects>(hero).unwrap().instances().count() == 1,
        "the effect on the entity itself"
    );
    assert!(world.get::<Tags>(hero).unwrap().has("status.swift"));
    assert_eq!(
        world.get::<Knockback>(hero).unwrap().push(),
        Vec2::new(0.0, -50.0)
    );
    let hitbox = world.get::<Hitbox>(hero).unwrap();
    assert!(hitbox.is_armed() && hitbox.window() == Some(0.5));
    assert_eq!(uses(&world), [(hero, "volley".to_string(), None)]);
    // An Aim's direction serves too, and a missing hitbox or knockback is
    // traced, not fatal.
    let bare = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(DataRef::by_id("volley")),
            Stats::new().with("mana", 5.0),
            Aim::intent(),
            Intent::default(),
        ),
    );
    world
        .get_mut::<Intent>(bare)
        .unwrap()
        .set_aim(Some(Vec2::X));
    assert_eq!(
        use_ability(&mut world, bare, "volley"),
        Used::Used,
        "the intent's aim serves before the aim system runs"
    );
    assert_eq!(
        skipped(&world, bare),
        [("impulse", NO_KNOCKBACK), ("hitbox", NO_HITBOX)]
    );
}

#[test]
fn the_load_refuses_a_name_twice_a_speedy_spawn_without_motion_and_bad_values() {
    let (mut world, _) = rig();
    insert(
        &mut world,
        "wand",
        Ability::new().with_payload(Payload::Hitbox(0.1)),
    );
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in(ABILITIES, &value, &world)
            .map_err(|e| e.to_string())
    };
    assert!(check(json!(["wand", { "payload": [{ "impulse": 1 }] }])).is_ok());
    assert_eq!(
        check(json!(["wand", "wand"])).unwrap_err(),
        "component `abilities`: 1: `wand` is named twice"
    );
    assert_eq!(
        check(json!(["sword"])).unwrap_err(),
        "component `abilities`, field `0`: unknown abilities `sword`"
    );
    assert_eq!(
        check(json!([{ "payload": [{ "spawn": { "prefab": "puff", "speed": 10 } }] }]))
            .unwrap_err(),
        "component `abilities`, field `0.payload.0.spawn.prefab`: unknown prefab with a `linear_motion`, for a spawn with a speed `puff`"
    );
    assert!(check(json!([{ "payload": [{ "spawn": { "prefab": "puff" } }] }])).is_ok());
    assert!(
        check(json!([{ "payload": [{ "spawn": { "prefab": "bolt", "speed": 10 } }] }])).is_ok()
    );
    assert_eq!(
        check(json!([{ "payload": [{ "spawn": { "prefab": "ghost" } }] }])).unwrap_err(),
        "component `abilities`, field `0.payload.0.spawn.prefab`: unknown prefab `ghost`"
    );
    assert_eq!(
        check(json!([{ "payload": [{ "effect": "nope" }] }])).unwrap_err(),
        "component `abilities`, field `0.payload.0.effect`: unknown effects `nope`"
    );
    assert_eq!(
        check(json!([{ "target": { "nearest": { "all": ["team.ghost"] } }, "payload": [{ "impulse": 1 }] }])).unwrap_err(),
        "component `abilities`, field `0.target.nearest.all.0`: unknown tag `team.ghost`"
    );
    assert_eq!(
        check(json!([{ "target": { "random": { "any": ["team.enemy", "team.ghost"] } }, "payload": [{ "impulse": 1 }] }])).unwrap_err(),
        "component `abilities`, field `0.target.random.any.1`: unknown tag `team.ghost`"
    );
    for bad in [
        json!({ "payload": [] }),
        json!({}),
        json!({ "trigger": { "interval": 0 }, "payload": [{ "impulse": 1 }] }),
        json!({ "trigger": { "action": "Attack" }, "payload": [{ "impulse": 1 }] }),
        json!({ "trigger": { "action": "a", "interval": 1 }, "payload": [{ "impulse": 1 }] }),
        json!({ "cost": { "stat": "mana", "amount": -1 }, "payload": [{ "impulse": 1 }] }),
        json!({ "cooldown": -1, "payload": [{ "impulse": 1 }] }),
        json!({ "charges": { "max": 0, "refill": 1 }, "payload": [{ "impulse": 1 }] }),
        json!({ "charges": { "max": 1, "refill": 0 }, "payload": [{ "impulse": 1 }] }),
        json!({ "target": { "nearest": { "none": ["boss"] } }, "payload": [{ "impulse": 1 }] }),
        json!({ "target": { "nearest": { "all": ["a"], "range": 0 } }, "payload": [{ "impulse": 1 }] }),
        json!({ "payload": [{ "impulse": -1 }] }),
        json!({ "payload": [{ "hitbox": 0 }] }),
        json!({ "payload": [{ "spawn": { "prefab": "bolt", "speed": -1 } }] }),
        json!({ "payload": [{ "spawn": "bolt" }] }),
        json!({ "extra": 1, "payload": [{ "impulse": 1 }] }),
    ] {
        assert!(
            serde_json::from_value::<Ability>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    let full: Ability = serde_json::from_value(json!({
        "trigger": { "action": "attack" },
        "cost": { "stat": "mana", "amount": 10 },
        "cooldown": 0.8,
        "charges": { "max": 6, "refill": 1.5, "all": true },
        "target": { "random": { "any": ["team.enemy"], "none": ["boss"], "range": 320 } },
        "payload": [
            { "spawn": { "prefab": "bolt", "speed": 420, "offset": 12 } },
            { "effect": "swift" },
            { "impulse": 600 },
            { "hitbox": 0.2 }
        ]
    }))
    .unwrap();
    assert_eq!(full.trigger(), &Trigger::Action("attack".to_string()));
    assert_eq!(full.cost(), Some(("mana", 10.0)));
    assert_eq!(full.charges(), Some((6, 1.5, true)));
    assert!(matches!(full.target(), AbilityTarget::Random { range: Some(r), .. } if *r == 320.0));
    let back = serde_json::to_value(&full).unwrap();
    assert_eq!(
        back["target"],
        json!({ "random": { "any": ["team.enemy"], "none": ["boss"], "range": 320.0 } })
    );
    assert_eq!(serde_json::from_value::<Ability>(back).unwrap(), full);
    assert_eq!(
        serde_json::to_value(Ability::new().with_payload(Payload::Impulse(1.0))).unwrap(),
        json!({
            "trigger": { "manual": {} }, "cost": null, "cooldown": 0.0, "charges": null,
            "target": { "self": {} }, "payload": [{ "impulse": 1.0 }]
        })
    );
    assert!(std::panic::catch_unwind(|| Ability::new().with_cooldown(-1.0)).is_err());
    assert!(std::panic::catch_unwind(|| Ability::new().with_charges(0, 1.0, false)).is_err());
    assert!(
        std::panic::catch_unwind(|| Ability::new().with_trigger(Trigger::Interval(0.0))).is_err()
    );
    assert!(
        std::panic::catch_unwind(|| Ability::new().with_payload(Payload::Hitbox(0.0))).is_err()
    );
    assert!(
        std::panic::catch_unwind(|| Ability::new().with_target(AbilityTarget::Nearest {
            filter: TagFilter::new().none(["boss"]),
            range: None,
        }))
        .is_err()
    );
}

#[test]
fn the_component_names_inline_entries_replaces_owned_names_and_saves_its_state() {
    let (mut world, mut schedule) = rig();
    let wand = insert(
        &mut world,
        "wand",
        Ability::new()
            .with_cooldown(1.0)
            .with_charges(2, 1.0, false)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let dash = Ability::new().with_payload(Payload::Hitbox(0.5));
    let mut abilities = Abilities::new()
        .with(wand.clone())
        .with(DataRef::inline(dash.clone()));
    assert_eq!(abilities.names().collect::<Vec<_>>(), ["wand", "#1"]);
    abilities.add(wand);
    assert_eq!(abilities.names().count(), 2, "an owned name is replaced");
    let hero = at(&mut world, 0.0, 0.0, (abilities,));
    assert_eq!(use_ability(&mut world, hero, "wand"), Used::Used);
    assert_eq!(use_ability(&mut world, hero, "#1"), Used::Used);
    assert_eq!(use_ability(&mut world, hero, "#2"), Used::NoAbility);
    steps(&mut world, &mut schedule, 1);
    let abilities = world.get::<Abilities>(hero).unwrap().clone();
    let saved = serde_json::to_value(&abilities).unwrap();
    assert_eq!(saved["entries"][0]["name"], "wand");
    assert_eq!(saved["entries"][0]["charges"], 1);
    assert!(close(
        saved["entries"][0]["cooldown_left"].as_f64().unwrap() as f32,
        1.0 - STEP
    ));
    assert_eq!(saved["entries"][1]["name"], "#1");
    assert_eq!(
        saved["entries"][1]["ability"]["payload"],
        json!([{ "hitbox": 0.5 }])
    );
    let read: Abilities = serde_json::from_value(saved).unwrap();
    assert_eq!(read, abilities);
    assert!(
        serde_json::from_value::<Abilities>(json!({ "entries": [
        { "name": "a", "ability": "wand" }, { "name": "a", "ability": "wand" }
    ] }))
        .is_err(),
        "twice"
    );
    assert!(
        serde_json::from_value::<Abilities>(json!({ "entries": [
        { "name": "a", "ability": "wand", "cooldown_left": -1 }
    ] }))
        .is_err()
    );
    let rows = abilities.inspect_rows();
    assert_eq!(rows.len(), 2);
    assert!(
        rows[0].1.starts_with("wand, 0.98 s, 1 charges"),
        "{}",
        rows[0].1
    );
    assert_eq!(rows[1].1, "#1, ready");
    // A scene list builds the same component.
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(
                ABILITIES,
                &json!(["wand", { "payload": [{ "impulse": 1 }] }]),
                &world
            )
            .is_ok()
    );
}

#[test]
fn the_ability_cue_action_uses_or_refills_the_subject_or_the_other() {
    let (mut world, mut schedule) = rig();
    insert(
        &mut world,
        "volley",
        Ability::new()
            .with_charges(2, 100.0, false)
            .with_payload(Payload::Hitbox(0.1)),
    );
    insert(
        &mut world,
        "swing",
        Ability::new()
            .with_cooldown(100.0)
            .with_payload(Payload::Hitbox(0.1)),
    );
    // A pickup refills its collector's volley; the collector's own cue
    // swings on `picked_up`'s... no: on `ability_used`, the counter counts.
    let ranger = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new()
                .with(DataRef::by_id("volley"))
                .with(DataRef::by_id("swing")),
            Cues::new().on("ability_used", [CueAction::counter_add("used", 1)]),
            Hitbox::new(HitShape::circle(4.0), 1.0),
        ),
    );
    steps(&mut world, &mut schedule, 1);
    use_ability(&mut world, ranger, "volley");
    use_ability(&mut world, ranger, "volley");
    assert_eq!(use_ability(&mut world, ranger, "volley"), Used::NoCharge);
    let ammo = at(
        &mut world,
        0.0,
        0.0,
        (Cues::new().on("cooldown_elapsed", [CueAction::ability("swing")]),),
    );
    // A game event with the ranger as the other refills, as a pickup's
    // `picked_up` would.
    let crate_ = at(
        &mut world,
        0.0,
        0.0,
        (Cues::new().on(
            "zone_entered",
            [
                CueAction::refill_ability("volley").for_other(),
                CueAction::ability("swing").for_other(),
            ],
        ),),
    );
    world.register_event::<crate::ZoneEntered>();
    world
        .resource_mut::<EventQueue<crate::ZoneEntered>>()
        .send(crate::ZoneEntered::new(crate_, ranger, Vec2::ZERO));
    world.register_event::<crate::CooldownElapsed>();
    world
        .resource_mut::<EventQueue<crate::CooldownElapsed>>()
        .send(crate::CooldownElapsed::new(ammo));
    let before = uses(&world).len();
    steps(&mut world, &mut schedule, 1);
    assert!(
        world.get::<Abilities>(ranger).unwrap().ready("volley"),
        "refilled by the other"
    );
    let after = uses(&world);
    assert_eq!(
        after.len() - before,
        1,
        "the ranger's swing by the crate; the ammo owns none"
    );
    assert_eq!(after.last().unwrap().0, ranger);
    assert_eq!(
        world.get::<Counters>(ranger).unwrap().get("used"),
        2,
        "the two volleys, read this step; the swing's use is counted next step"
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(world.get::<Counters>(ranger).unwrap().get("used"), 3);
    assert!(skipped(&world, ammo).contains(&("ability", "the entity owns no such ability")));
    assert_eq!(use_ability(&mut world, ranger, "swing"), Used::NotReady);
    // The data forms and the load's checks.
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in("cues", &value, &world)
            .map_err(|e| e.to_string())
    };
    assert!(check(json!({ "picked_up": [{ "ability": { "name": "volley", "op": "refill", "target": "other" } }] })).is_ok());
    assert!(check(json!({ "killed": [{ "ability": "swing" }] })).is_ok());
    assert_eq!(
        check(
            json!({ "cooldown_elapsed": [{ "ability": { "name": "swing", "target": "other" } }] })
        )
        .unwrap_err(),
        "component `cues`, field `cooldown_elapsed.0.ability.target`: unknown cue event with an other entity `cooldown_elapsed`"
    );
    assert!(
        serde_json::from_value::<CueAction>(json!({ "ability": { "name": "x", "op": "fire" } }))
            .is_err()
    );
    assert!(serde_json::from_value::<CueAction>(json!({ "ability": { "op": "use" } })).is_err());
    assert_eq!(
        serde_json::from_value::<CueAction>(
            json!({ "ability": { "name": "volley", "op": "refill" } })
        )
        .unwrap(),
        CueAction::refill_ability("volley")
    );
}

// 7.10a: skills, their levels and lines, and the delivery a spawn carries.

#[test]
fn an_entry_takes_a_level_in_data_and_a_skilled_ability_round_trips() {
    let (mut world, _) = rig();
    let skilled = json!({
        "tags": ["status.swift"],
        "skill": { "bases": { "cost": 3 }, "lines": [{ "stat": "cost", "modifier": { "mul": 2 } }] },
        "payload": [{ "spawn": { "prefab": "puff" } }]
    });
    let ability: Ability = serde_json::from_value(skilled.clone()).unwrap();
    assert_eq!(ability.tags(), ["status.swift"]);
    assert_eq!(ability.skill().unwrap().cost(), Some(3.0));
    let back = serde_json::to_value(&ability).unwrap();
    assert_eq!(serde_json::from_value::<Ability>(back).unwrap(), ability);
    let plain = serde_json::to_value(Ability::new().with_payload(Payload::Impulse(1.0))).unwrap();
    assert!(
        plain.get("tags").is_none() && plain.get("skill").is_none(),
        "as before 7.10a"
    );
    world.resource_mut::<DataRegistry>().insert("zap", ability);
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| components.check_in(ABILITIES, &value, &world);
    assert!(check(json!([{ "id": "zap", "level": 10 }])).is_ok());
    for bad in [
        json!([{ "id": "zap", "level": 41 }]),
        json!([{ "id": "zap", "level": 0 }]),
        json!([{ "id": "zap", "level": 2, "spare": 1 }]),
        json!([{ "id": "zap", "level": 2 }, "zap"]),
    ] {
        assert!(check(bad.clone()).is_err(), "{bad}");
    }
    let prefab: PrefabData = serde_json::from_value(
        json!({ "components": { "abilities": [{ "id": "zap", "level": 10 }, "zap_b"] } }),
    )
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("zap_b", Ability::new().with_payload(Payload::Impulse(1.0)));
    world
        .resource_mut::<PrefabRegistry>()
        .insert("mage".to_string(), prefab);
    let mage = world.spawn_prefab("mage", [0.0, 0.0]).unwrap();
    let abilities = world.get::<Abilities>(mage).unwrap();
    assert_eq!(
        (
            abilities.level("zap"),
            abilities.level("zap_b"),
            abilities.level("nope")
        ),
        (Some(10), Some(1), None)
    );
}

#[test]
fn levels_and_placed_lines_save_with_the_abilities_and_leave_with_their_source() {
    let mut abilities = Abilities::new()
        .with(DataRef::by_id("zap"))
        .with_level("zap", 7);
    abilities.add_skill_lines(
        "zap",
        "support:twin",
        [SkillLine::added("damage.fire", 1.0, 2.0)],
    );
    abilities.add_lines("item:9", [SkillLine::pen("damage.fire", 0.1)]);
    let saved = serde_json::to_value(&abilities).unwrap();
    assert_eq!(saved["levels"], json!([["zap", 7]]));
    assert_eq!(saved["skill_lines"][0][1], json!("support:twin"));
    assert_eq!(saved["lines"][0][0], json!("item:9"));
    assert_eq!(
        serde_json::from_value::<Abilities>(saved).unwrap(),
        abilities
    );
    assert_eq!(abilities.remove_skill_lines("zap", "support:twin"), 1);
    assert_eq!(abilities.remove_lines("item:9"), 1);
    abilities.set_level("zap", 1);
    assert_eq!(
        serde_json::to_value(&abilities)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        1,
        "only the entries again"
    );
}

#[test]
fn a_skills_spawn_carries_its_delivery_and_a_plain_one_none() {
    let (mut world, mut schedule) = rig();
    let skilled = Ability::new()
        .with_payload(Payload::Spawn {
            prefab: "puff".to_string(),
            speed: 0.0,
            offset: 0.0,
        })
        .with_skill(crate::Skill::new().with_damage("damage.fire", 3.0, 5.0));
    let plain = Ability::new().with_payload(Payload::Spawn {
        prefab: "bolt".to_string(),
        speed: 0.0,
        offset: 0.0,
    });
    let caster = world.spawn_with((
        Transform::from_position(Vec2::ZERO),
        Abilities::new()
            .with(DataRef::inline(skilled))
            .with(DataRef::inline(plain)),
    ));
    assert_eq!(use_ability(&mut world, caster, "#0"), Used::Used);
    assert_eq!(use_ability(&mut world, caster, "#1"), Used::Used);
    step(&mut world, &mut schedule, STEP);
    let carried: Vec<(String, Option<Entity>)> = world
        .query::<(&tungsten_core::Tag, Option<&crate::Delivery>)>()
        .map(|(tag, delivery)| (tag.name.clone(), delivery.and_then(crate::Delivery::holder)))
        .collect();
    assert!(
        carried.contains(&("puff".to_string(), Some(caster))),
        "{carried:?}"
    );
    assert!(carried.contains(&("bolt".to_string(), None)), "{carried:?}");
}

// 7.10c, Step 2: a held trigger, a direction and the refill.

#[test]
fn a_held_trigger_uses_each_cooldown_while_its_slot_is_held() {
    let (mut world, mut schedule) = rig();
    let smg = insert(
        &mut world,
        "smg",
        Ability::new()
            .with_trigger(Trigger::Held("fire".to_string()))
            .with_cooldown(6.0 * STEP)
            .with_payload(Payload::Hitbox(0.05)),
    );
    let burst = insert(
        &mut world,
        "burst",
        Ability::new()
            .with_trigger(Trigger::Held("burst".to_string()))
            .with_charges(2, 100.0, false)
            .with_payload(Payload::Hitbox(0.05)),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (Abilities::new().with(smg).with(burst), Intent::default()),
    );
    let mut used = Vec::new();
    for i in 1..=40 {
        // Held from step 1 through step 30, pressed at the first.
        let intent = world.get_mut::<Intent>(hero).unwrap();
        intent.set_slot("fire", i <= 30, i == 1);
        intent.set_slot("burst", i <= 30, i == 1);
        steps(&mut world, &mut schedule, 1);
        used.extend(uses(&world).into_iter().map(|(_, name, _)| (i, name)));
    }
    let at_steps = |name: &str| -> Vec<usize> {
        used.iter()
            .filter(|(_, used)| used == name)
            .map(|&(i, _)| i)
            .collect()
    };
    assert_eq!(
        at_steps("smg"),
        [1, 7, 13, 19, 25],
        "a use each 0.1 s while held, none after the release"
    );
    assert_eq!(
        at_steps("burst"),
        [1, 2],
        "a use each step held until the charges run out"
    );
    // The data form.
    let held: Ability = serde_json::from_value(json!({
        "trigger": { "held": "fire" },
        "payload": [{ "hitbox": 0.1 }]
    }))
    .unwrap();
    assert_eq!(held.trigger(), &Trigger::Held("fire".to_string()));
    assert_eq!(
        serde_json::to_value(&held).unwrap()["trigger"],
        json!({ "held": "fire" })
    );
    assert!(
        serde_json::from_value::<Ability>(json!({
            "trigger": { "held": "Fire" },
            "payload": [{ "hitbox": 0.1 }]
        }))
        .is_err()
    );
}

#[test]
fn a_movement_direction_follows_the_intent_else_the_aim() {
    let (mut world, mut schedule) = rig();
    let bolt = || Payload::Spawn {
        prefab: "bolt".to_string(),
        speed: 100.0,
        offset: 0.0,
    };
    let dodge = insert(
        &mut world,
        "dodge",
        Ability::new()
            .with_direction(Direction::Movement)
            .with_payload(bolt())
            .with_payload(Payload::Impulse(50.0)),
    );
    let shot = insert(&mut world, "shot", Ability::new().with_payload(bolt()));
    let seek = insert(
        &mut world,
        "seek",
        Ability::new()
            .with_direction(Direction::Movement)
            .with_target(AbilityTarget::Nearest {
                filter: TagFilter::new().all(["team.enemy"]),
                range: None,
            })
            .with_payload(bolt()),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(dodge).with(shot).with(seek),
            Intent::default(),
            Knockback::new(0.2),
        ),
    );
    at(&mut world, 0.0, 50.0, (Tags::new(["team.enemy"]),));
    // Each use's new bolt's velocity.
    let fired = |world: &mut World, schedule: &mut Schedule, name: &str| -> Vec2 {
        let before: Vec<Entity> = bolts(world).iter().map(|&(bolt, _)| bolt).collect();
        assert_eq!(use_ability(world, hero, name), Used::Used, "{name}");
        steps(world, schedule, 1);
        bolts(world)
            .into_iter()
            .find(|(bolt, _)| !before.contains(bolt))
            .map(|(_, velocity)| velocity)
            .unwrap()
    };
    let near = |a: Vec2, b: Vec2| (a - b).length() < 1e-4;
    // Moving right while aiming up: the dodge goes right, the shot up.
    {
        let intent = world.get_mut::<Intent>(hero).unwrap();
        intent.set_movement(Vec2::X);
        intent.set_aim(Some(Vec2::NEG_Y));
    }
    let dodged = fired(&mut world, &mut schedule, "dodge");
    assert!(near(dodged, Vec2::new(100.0, 0.0)), "{dodged}");
    assert_eq!(
        world.get::<Knockback>(hero).unwrap().push(),
        Vec2::new(50.0, 0.0),
        "the impulse along the move stick"
    );
    let shot = fired(&mut world, &mut schedule, "shot");
    assert!(near(shot, Vec2::new(0.0, -100.0)), "{shot}");
    // A nearest target keeps the direction to its target.
    let sought = fired(&mut world, &mut schedule, "seek");
    assert!(near(sought, Vec2::new(0.0, 100.0)), "{sought}");
    // At rest the dodge takes the aim's direction.
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_movement(Vec2::ZERO);
    let still = fired(&mut world, &mut schedule, "dodge");
    assert!(near(still, Vec2::new(0.0, -100.0)), "{still}");
    // With neither, no direction: no use.
    world.get_mut::<Intent>(hero).unwrap().set_aim(None);
    assert_eq!(use_ability(&mut world, hero, "dodge"), Used::NoTarget);
    // The data form: `movement` written, `aim` left out.
    let dodge: Ability = serde_json::from_value(json!({
        "direction": "movement",
        "payload": [{ "impulse": 600 }]
    }))
    .unwrap();
    assert_eq!(dodge.direction(), Direction::Movement);
    assert_eq!(
        serde_json::to_value(&dodge).unwrap()["direction"],
        json!("movement")
    );
    let aimed = Ability::new().with_payload(Payload::Impulse(600.0));
    assert_eq!(aimed.direction(), Direction::Aim);
    assert!(
        serde_json::to_value(&aimed)
            .unwrap()
            .get("direction")
            .is_none()
    );
    assert!(
        serde_json::from_value::<Ability>(json!({
            "direction": "behind",
            "payload": [{ "impulse": 600 }]
        }))
        .is_err()
    );
}

#[test]
fn refill_left_reads_a_running_refill_alone() {
    let (mut world, mut schedule) = rig();
    let each = insert(
        &mut world,
        "each",
        Ability::new()
            .with_charges(3, 2.0 * STEP, false)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let all = insert(
        &mut world,
        "all",
        Ability::new()
            .with_charges(3, 2.0 * STEP, true)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let plain = insert(
        &mut world,
        "plain",
        Ability::new()
            .with_cooldown(1.0)
            .with_payload(Payload::Hitbox(0.1)),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (Abilities::new().with(each).with(all).with(plain),),
    );
    let refill =
        |world: &World, name: &str| world.get::<Abilities>(hero).unwrap().refill_left(name);
    assert_eq!(
        (refill(&world, "each"), refill(&world, "all")),
        (None, None),
        "full, not yet counted"
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(
        (refill(&world, "each"), refill(&world, "all")),
        (None, None),
        "every charge"
    );
    // One spent: the one-at-a-time refill runs, a partial `all` magazine's
    // waits for its last charge.
    assert_eq!(use_ability(&mut world, hero, "each"), Used::Used);
    assert_eq!(use_ability(&mut world, hero, "all"), Used::Used);
    assert!(refill(&world, "each").is_some_and(|left| close(left, 2.0 * STEP)));
    assert_eq!(refill(&world, "all"), None, "charges left");
    steps(&mut world, &mut schedule, 1);
    assert!(refill(&world, "each").is_some_and(|left| close(left, STEP)));
    steps(&mut world, &mut schedule, 1);
    assert_eq!(refill(&world, "each"), None, "its charge back, full");
    // The magazine emptied: its refill runs from the last charge.
    assert_eq!(use_ability(&mut world, hero, "all"), Used::Used);
    assert_eq!(refill(&world, "all"), None, "one left");
    assert_eq!(use_ability(&mut world, hero, "all"), Used::Used);
    assert!(refill(&world, "all").is_some_and(|left| close(left, 2.0 * STEP)));
    steps(&mut world, &mut schedule, 1);
    assert!(refill(&world, "all").is_some_and(|left| close(left, STEP)));
    steps(&mut world, &mut schedule, 1);
    assert_eq!(refill(&world, "all"), None);
    assert_eq!(
        world.get::<Abilities>(hero).unwrap().charges("all"),
        Some(3)
    );
    // Without charges, or not owned: none.
    assert_eq!(use_ability(&mut world, hero, "plain"), Used::Used);
    assert_eq!(refill(&world, "plain"), None);
    assert_eq!(refill(&world, "sword"), None);
}

// 7.10c, Step 3: the ability cue events.

#[test]
fn a_refusal_carries_its_reason() {
    use crate::react::cues::KitCues;
    let (mut world, mut schedule) = rig();
    let hitbox = || Payload::Hitbox(0.1);
    let cool = insert(
        &mut world,
        "cool",
        Ability::new().with_cooldown(1.0).with_payload(hitbox()),
    );
    let spent = insert(
        &mut world,
        "spent",
        Ability::new()
            .with_charges(1, 100.0, false)
            .with_payload(hitbox()),
    );
    let dear = insert(
        &mut world,
        "dear",
        Ability::new()
            .with_cost("mana", 50.0)
            .with_payload(hitbox()),
    );
    let aimed = insert(
        &mut world,
        "aimed",
        Ability::new()
            .with_target(AbilityTarget::Aim)
            .with_payload(hitbox()),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new()
                .with(cool)
                .with(spent)
                .with(dear)
                .with(aimed),
            Stats::new().with("mana", 5.0),
            Cues::new().on(ABILITY_REFUSED, [CueAction::counter_add("refused", 1)]),
            Counters::new(),
        ),
    );
    for name in ["cool", "spent"] {
        assert_eq!(use_ability(&mut world, hero, name), Used::Used);
    }
    for (name, used) in [
        ("cool", Used::NotReady),
        ("spent", Used::NoCharge),
        ("dear", Used::NoCost),
        ("aimed", Used::NoTarget),
        ("sword", Used::NoAbility),
    ] {
        assert_eq!(use_ability(&mut world, hero, name), used, "{name}");
    }
    let abilities = world.get::<Abilities>(hero).unwrap();
    let pushed: Vec<(&str, Option<&str>, u8)> = world
        .resource::<KitCues>()
        .0
        .iter()
        .map(|cue| {
            (
                cue.event,
                abilities.name_at(usize::from(cue.entry)),
                cue.reason,
            )
        })
        .collect();
    assert_eq!(
        pushed,
        [
            (ABILITY_REFUSED, Some("cool"), 1),
            (ABILITY_REFUSED, Some("spent"), 2),
            (ABILITY_REFUSED, Some("dear"), 3),
            (ABILITY_REFUSED, Some("aimed"), 4),
        ],
        "each owned ability's refusal with its reason; none for a name it does not own"
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(world.get::<Counters>(hero).unwrap().get("refused"), 4);
    assert!(
        world.resource::<KitCues>().0.is_empty(),
        "the cue system drained them"
    );
}

#[test]
fn an_entry_turns_ready_once_a_cooldown() {
    let (mut world, mut schedule) = rig();
    let hitbox = || Payload::Hitbox(0.1);
    let cool = insert(
        &mut world,
        "cool",
        Ability::new()
            .with_cooldown(3.0 * STEP)
            .with_payload(hitbox()),
    );
    let shot = insert(
        &mut world,
        "shot",
        Ability::new()
            .with_charges(1, 2.0 * STEP, false)
            .with_payload(hitbox()),
    );
    let free = insert(&mut world, "free", Ability::new().with_payload(hitbox()));
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(cool).with(shot).with(free),
            Cues::new().on(ABILITY_READY, [CueAction::counter_add("ready", 1)]),
            Counters::new(),
        ),
    );
    let ready = |world: &World| world.get::<Counters>(hero).unwrap().get("ready");
    steps(&mut world, &mut schedule, 5);
    assert_eq!(
        ready(&world),
        0,
        "an entry starts ready: a spawn sends none"
    );
    for name in ["cool", "shot", "free"] {
        assert_eq!(use_ability(&mut world, hero, name), Used::Used);
    }
    let mut seen = Vec::new();
    for _ in 0..6 {
        steps(&mut world, &mut schedule, 1);
        seen.push(ready(&world));
    }
    assert_eq!(
        seen,
        [0, 1, 2, 2, 2, 2],
        "the shot's charge back at step 2, the cooldown out at step 3; the free one never waits"
    );
    // A refill makes it ready at once, which no counting sees.
    assert_eq!(use_ability(&mut world, hero, "cool"), Used::Used);
    assert!(world.get_mut::<Abilities>(hero).unwrap().refill("cool"));
    steps(&mut world, &mut schedule, 4);
    assert_eq!(ready(&world), 2);
}

#[test]
fn a_held_trigger_cues_its_hold_and_release() {
    let (mut world, mut schedule) = rig();
    let run = insert(
        &mut world,
        "run",
        Ability::new()
            .with_trigger(Trigger::Held("run".to_string()))
            .with_payload(Payload::Hitbox(0.05)),
    );
    let dash = insert(
        &mut world,
        "dash",
        Ability::new()
            .with_trigger(Trigger::Action("run".to_string()))
            .with_payload(Payload::Hitbox(0.05)),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (
            Abilities::new().with(run).with(dash),
            Intent::default(),
            Cues::new()
                .on(ABILITY_HELD, [CueAction::counter_add("held", 1)])
                .on(ABILITY_RELEASED, [CueAction::counter_add("released", 1)]),
            Counters::new(),
        ),
    );
    let counts = |world: &World| {
        let counters = world.get::<Counters>(hero).unwrap();
        (counters.get("held"), counters.get("released"))
    };
    let mut seen = Vec::new();
    for i in 1..=7 {
        world
            .get_mut::<Intent>(hero)
            .unwrap()
            .set_slot("run", (2..=4).contains(&i), i == 2);
        steps(&mut world, &mut schedule, 1);
        seen.push(counts(&world));
    }
    assert_eq!(
        seen,
        [(0, 0), (1, 0), (1, 0), (1, 0), (1, 1), (1, 1), (1, 1)],
        "down at step 2 and up at step 5, once each; the action trigger on the slot cues no hold"
    );
    // The flag is unsaved, so a load sees a held slot's hold begin again.
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_slot("run", true, true);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(counts(&world), (2, 1));
    let saved = serde_json::to_value(world.get::<Abilities>(hero).unwrap()).unwrap();
    assert!(!saved.to_string().contains("held"), "{saved}");
    *world.get_mut::<Abilities>(hero).unwrap() = serde_json::from_value(saved).unwrap();
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_slot("run", true, false);
    steps(&mut world, &mut schedule, 1);
    assert_eq!(counts(&world), (3, 1));
}

// 7.10c, Step 4: spawns by their centre, their motion kept, a lob.

/// Registers sprite `id`, `size` source pixels, anchored at `anchor` when
/// given, on the World's asset registry.
fn sprite(world: &mut World, id: &str, size: [u32; 2], anchor: Option<[f32; 2]>) {
    use std::path::PathBuf;
    use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
    if !world.has_resource::<AssetRegistry>() {
        world.insert_resource(AssetRegistry::new());
    }
    let assets = world.resource_mut::<AssetRegistry>();
    assets.register_sprite(
        id.to_string(),
        FilterMode::Nearest,
        size[0],
        size[1],
        PathBuf::from(format!("{id}.png")),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
    assets.set_sprite_frame(id, None, anchor);
}

/// Adds prefab `id` as `data` to the World's registry.
fn prefab(world: &mut World, id: &str, data: serde_json::Value) {
    let data: PrefabData = serde_json::from_value(data).unwrap();
    world
        .resource_mut::<PrefabRegistry>()
        .insert(id.to_string(), data);
}

/// The entities tagged `name`, by index.
fn tagged(world: &World, name: &str) -> Vec<Entity> {
    let mut found: Vec<Entity> = world
        .query::<(Entity, &tungsten_core::Tag)>()
        .filter(|(_, tag)| tag.name == name)
        .map(|(entity, _)| entity)
        .collect();
    found.sort_by_key(|entity| entity.id());
    found
}

/// The command flush that lands queued spawns.
fn flush(world: &mut World) {
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

/// A spawn of `prefab` with no speed or offset, centred or not.
fn spawn_of(prefab: &str, centred: bool, offset: f32) -> Payload {
    if centred {
        Payload::SpawnWith(Box::new(
            SpawnWith::new(prefab).with_offset(offset).centred(),
        ))
    } else {
        Payload::Spawn {
            prefab: prefab.to_string(),
            speed: 0.0,
            offset,
        }
    }
}

#[test]
fn a_centred_spawn_puts_the_prefabs_centre_on_the_users() {
    use tungsten_core::physics::Collider;
    let (mut world, mut schedule) = rig();
    sprite(&mut world, "mote", [8, 6], None);
    prefab(
        &mut world,
        "mote",
        json!({ "tag": "mote", "sprite": { "asset_id": "mote" } }),
    );
    let centred = insert(
        &mut world,
        "centred",
        Ability::new().with_payload(spawn_of("mote", true, 0.0)),
    );
    let ahead = insert(
        &mut world,
        "ahead",
        Ability::new().with_payload(spawn_of("mote", true, 10.0)),
    );
    let plain = insert(
        &mut world,
        "plain",
        Ability::new().with_payload(spawn_of("mote", false, 0.0)),
    );
    // A body whose collider sits 8 px above its point.
    let hero = world.spawn_with((
        Position(Vec2::new(10.0, 20.0)),
        Collider::aabb(Vec2::splat(4.0)).with_offset(Vec2::new(0.0, -8.0)),
        Abilities::new().with(centred).with(ahead).with(plain),
        Intent::default(),
    ));
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_aim(Some(Vec2::X));
    let mut placed = Vec::new();
    for name in ["centred", "ahead", "plain"] {
        let before = tagged(&world, "mote");
        assert_eq!(use_ability(&mut world, hero, name), Used::Used);
        steps(&mut world, &mut schedule, 1);
        let mote = tagged(&world, "mote")
            .into_iter()
            .find(|mote| !before.contains(mote))
            .unwrap();
        placed.push((
            world.get::<Transform>(mote).unwrap().position,
            centre_of(&world, mote).unwrap(),
        ));
    }
    assert_eq!(
        placed,
        [
            (Vec2::new(6.0, 9.0), Vec2::new(10.0, 12.0)),
            (Vec2::new(16.0, 9.0), Vec2::new(20.0, 12.0)),
            (Vec2::new(10.0, 20.0), Vec2::new(14.0, 23.0)),
        ],
        "its sprite's centre on the user's collider centre, then 10 px on along the aim; \
         uncentred, its point on the user's point"
    );
}

#[test]
fn a_drawn_centre_reads_the_anchor_scale_flip_and_turn() {
    use tungsten_core::Sprite;
    use tungsten_core::components::SpriteFlip;
    let (mut world, mut schedule) = rig();
    sprite(&mut world, "banner", [16, 8], Some([4.0, 8.0]));
    let banner = world
        .resource::<AssetRegistry>()
        .sprite_id("banner")
        .unwrap();
    // Scaled 2, turned a quarter and flipped: the flipped anchor (12, 8),
    // scaled to (24, 16), sits on the point; the centre (16, 8) lies
    // (−8, −8) from it, turned a quarter to (8, −8).
    let mut transform = Transform::from_position(Vec2::new(100.0, 50.0));
    transform.scale = Vec2::splat(2.0);
    transform.rotation = std::f32::consts::FRAC_PI_2;
    let flag = world.spawn_with((transform, Sprite::new(banner), SpriteFlip(true)));
    let centre = centre_of(&world, flag).unwrap();
    assert!(
        (centre - Vec2::new(108.0, 42.0)).length() < 1e-4,
        "{centre}"
    );
    // Unscaled, unturned, unflipped: the anchor (4, 8) on the point, the
    // centre (8, 4) at (4, −4) from it.
    let plain = world.spawn_with((
        Transform::from_position(Vec2::new(100.0, 50.0)),
        Sprite::new(banner),
    ));
    assert_eq!(centre_of(&world, plain), Some(Vec2::new(104.0, 46.0)));
    // A bodiless prefab anchored so: a centred spawn puts its point 4 px
    // left of and below the user's centre.
    prefab(
        &mut world,
        "banner",
        json!({ "tag": "banner", "sprite": { "asset_id": "banner" } }),
    );
    let raise = insert(
        &mut world,
        "raise",
        Ability::new().with_payload(spawn_of("banner", true, 0.0)),
    );
    let bearer = at(&mut world, 30.0, 30.0, (Abilities::new().with(raise),));
    assert_eq!(use_ability(&mut world, bearer, "raise"), Used::Used);
    steps(&mut world, &mut schedule, 1);
    let [raised] = tagged(&world, "banner")[..] else {
        panic!("one banner");
    };
    assert_eq!(
        world.get::<Transform>(raised).unwrap().position,
        Vec2::new(26.0, 34.0)
    );
    assert_eq!(centre_of(&world, raised), Some(Vec2::new(30.0, 30.0)));
}

#[test]
fn a_spawn_keeps_its_prefabs_acceleration() {
    let (mut world, mut schedule) = rig();
    prefab(
        &mut world,
        "shell",
        json!({ "tag": "shell", "components": { "linear_motion": { "velocity": [0, 0], "acceleration": [0, 300] } } }),
    );
    let fired = |prefab: &str| Payload::Spawn {
        prefab: prefab.to_string(),
        speed: 200.0,
        offset: 0.0,
    };
    let shell = insert(
        &mut world,
        "shell",
        Ability::new()
            .with_target(AbilityTarget::Aim)
            .with_payload(fired("shell")),
    );
    let bolt = insert(
        &mut world,
        "bolt",
        Ability::new()
            .with_target(AbilityTarget::Aim)
            .with_payload(fired("bolt")),
    );
    let hero = at(
        &mut world,
        0.0,
        0.0,
        (Abilities::new().with(shell).with(bolt), Intent::default()),
    );
    world
        .get_mut::<Intent>(hero)
        .unwrap()
        .set_aim(Some(Vec2::X));
    for name in ["shell", "bolt"] {
        assert_eq!(use_ability(&mut world, hero, name), Used::Used);
    }
    steps(&mut world, &mut schedule, 1);
    let [thrown] = tagged(&world, "shell")[..] else {
        panic!("one shell");
    };
    let motion = world.get::<LinearMotion>(thrown).unwrap();
    assert_eq!(
        (motion.velocity(), motion.acceleration()),
        (Vec2::new(200.0, 0.0), Vec2::new(0.0, 300.0)),
        "its speed along the aim, its prefab's acceleration kept"
    );
    let [(_, velocity)] = bolts(&world)[..] else {
        panic!("one bolt");
    };
    assert_eq!(velocity, Vec2::new(200.0, 0.0));
    let [bolt] = tagged(&world, "bolt")[..] else {
        panic!("one bolt");
    };
    assert_eq!(
        world.get::<LinearMotion>(bolt).unwrap().acceleration(),
        Vec2::ZERO,
        "a prefab without one gets none"
    );
}

/// A World with a clock at fixed step `dt`, a command buffer, a trace,
/// gravity 600 px/s² down, the physics and the tags (`team.enemy`), linear
/// motion and ability plugins.
fn physics_rig(dt: f32) -> (World, Schedule) {
    use tungsten_core::physics::{PhysicsConfig, PhysicsPlugin};
    let mut world = World::new();
    let mut time = Time::new();
    time.set_fixed_step(dt);
    world.insert_resource(time);
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.insert_resource(PrefabRegistry::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, 600.0),
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.enemy"])
        .build(&mut schedule, &mut world);
    PhysicsPlugin.build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    AbilitiesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

#[test]
fn a_lob_lands_on_the_aims_point_by_its_body_kind() {
    let target = Vec2::new(300.0, -40.0);
    let dynamic = |scale: Option<f32>| {
        let mut body = json!({ "kind": "dynamic", "shape": { "kind": "circle", "radius": 2 } });
        if let Some(scale) = scale {
            body["gravity_scale"] = json!(scale);
        }
        body
    };
    let linear = json!({ "velocity": [0, 0], "acceleration": [0, 300] });
    let kinds = [
        ("stone", json!({ "rigid_body": dynamic(Some(2.0)) })),
        ("shell", json!({ "linear_motion": linear })),
        (
            "both",
            json!({ "rigid_body": dynamic(None), "linear_motion": linear }),
        ),
    ];
    for dt in [STEP, 2.0 * STEP] {
        for (id, components) in &kinds {
            let (mut world, mut schedule) = physics_rig(dt);
            prefab(
                &mut world,
                id,
                json!({ "tag": id, "components": components }),
            );
            let lob = insert(
                &mut world,
                "lob",
                Ability::new()
                    .with_target(AbilityTarget::Nearest {
                        filter: TagFilter::new().all(["team.enemy"]),
                        range: None,
                    })
                    .with_payload(Payload::SpawnWith(Box::new(
                        SpawnWith::new(*id).with_lob(Lob::new(400.0, [0.2, 2.0])),
                    ))),
            );
            let thrower = at(&mut world, 0.0, 0.0, (Abilities::new().with(lob),));
            at(&mut world, target.x, target.y, (Tags::new(["team.enemy"]),));
            assert_eq!(use_ability(&mut world, thrower, "lob"), Used::Used);
            flush(&mut world);
            let [thrown] = tagged(&world, id)[..] else {
                panic!("{id}: one");
            };
            // 302.6 px at 400 px/s: 45 steps at 1/60 s, 23 at 1/30 s.
            let flight = ((target.length() / 400.0).clamp(0.2, 2.0) / dt).round() as usize;
            for _ in 0..flight {
                step(&mut world, &mut schedule, dt);
            }
            let landed = point_of(&world, thrown).unwrap();
            assert!(
                (landed - target).length() < 0.5,
                "{id} at dt {dt}: {landed} after {flight} steps"
            );
        }
    }
    // The load refuses a lob beside a speed, and on a prefab that holds
    // neither a dynamic body nor a `linear_motion`.
    assert!(
        serde_json::from_value::<Ability>(json!({
            "payload": [{ "spawn": { "prefab": "shell", "speed": 100, "lob": { "speed": 400, "time": [0.2, 2] } } }]
        }))
        .is_err()
    );
    let (mut world, _) = physics_rig(STEP);
    prefab(&mut world, "puff", json!({ "tag": "puff" }));
    let still = Ability::new().with_payload(Payload::SpawnWith(Box::new(
        SpawnWith::new("puff").with_lob(Lob::new(400.0, [0.2, 2.0])),
    )));
    let error = still.check(&world).unwrap_err();
    assert_eq!(error.field, "payload.0.spawn.prefab", "{error:?}");
}

// 7.10c, Step 7: conduction.

/// The filter of conductors.
fn conductive() -> TagFilter {
    TagFilter::new().all(["conductive"])
}

/// A conductor at `x, y` with 100 health and `more` tags.
fn conductor(world: &mut World, x: f32, y: f32, more: &[&str]) -> Entity {
    let mut tags = vec!["conductive"];
    tags.extend_from_slice(more);
    at(world, x, y, (Tags::new(tags), Health::new(100.0)))
}

/// A caster at the origin whose `zap` fires on the slot `zap`, conducting
/// `conduct` from the nearest conductor.
fn zapper(world: &mut World, conduct: Conduct) -> Entity {
    let zap = insert(
        world,
        "zap",
        Ability::new()
            .with_trigger(Trigger::Action("zap".to_string()))
            .with_target(AbilityTarget::Nearest {
                filter: conductive(),
                range: None,
            })
            .with_payload(Payload::Conduct(Box::new(conduct))),
    );
    at(
        world,
        0.0,
        0.0,
        (Abilities::new().with(zap), Intent::default()),
    )
}

/// `count` steps, `zap` pressed on `caster` at the first when `press`:
/// each step's damage, by target.
fn zap_steps(
    world: &mut World,
    schedule: &mut Schedule,
    caster: Entity,
    count: usize,
    press: bool,
) -> Vec<Vec<(Entity, f32)>> {
    (1..=count)
        .map(|i| {
            if world.is_alive(caster) {
                world.get_mut::<Intent>(caster).unwrap().set_slot(
                    "zap",
                    press && i == 1,
                    press && i == 1,
                );
            }
            steps(world, schedule, 1);
            world
                .resource::<EventQueue<crate::Damaged>>()
                .iter()
                .map(|hit| (hit.entity, hit.amount))
                .collect()
        })
        .collect()
}

#[test]
fn a_conduction_jumps_to_the_nearest_unchained_receiver_one_a_step() {
    let (mut world, mut schedule) = rig();
    let caster = zapper(
        &mut world,
        Conduct::new(32.0, 120.0, conductive()).with_keep(0.5),
    );
    let first = conductor(&mut world, 100.0, 0.0, &[]);
    let near = conductor(&mut world, 150.0, 80.0, &[]);
    let last = conductor(&mut world, 200.0, 0.0, &[]);
    let away = conductor(&mut world, 900.0, 0.0, &[]);
    assert_eq!(
        zap_steps(&mut world, &mut schedule, caster, 5, true),
        [
            vec![(first, 32.0)],
            vec![(near, 16.0)],
            vec![(last, 8.0)],
            vec![],
            vec![],
        ],
        "the target, then 94 px on to the nearer, then the last, halving; none in reach after"
    );
    assert_eq!(world.get::<Health>(away).unwrap().current(), 100.0);
    // Equally near, the lower in save order goes first.
    let (mut world, mut schedule) = rig();
    let caster = zapper(
        &mut world,
        Conduct::new(10.0, 120.0, conductive()).with_jumps(1),
    );
    let first = conductor(&mut world, 100.0, 0.0, &[]);
    let lower = conductor(&mut world, 100.0, 90.0, &[]);
    let higher = conductor(&mut world, 100.0, -90.0, &[]);
    let struck = zap_steps(&mut world, &mut schedule, caster, 3, true);
    assert_eq!(struck[1].len(), 1);
    assert_eq!(struck[1][0].0, lower, "{struck:?}");
    assert_eq!(world.get::<Health>(higher).unwrap().current(), 100.0);
    let _ = first;
}

#[test]
fn a_conduction_ends_at_an_immune_or_stopping_receiver_and_at_its_jumps() {
    use crate::damage::defense::Resist;
    let bolt = || {
        Conduct::new(20.0, 120.0, conductive())
            .with_tags(["damage.lightning"])
            .with_stop(TagFilter::new().any(["status.frozen"]))
    };
    for case in ["immune", "frozen", "jumps"] {
        let (mut world, mut schedule) = rig();
        let conduct = if case == "jumps" {
            bolt().with_jumps(1)
        } else {
            bolt()
        };
        let caster = zapper(&mut world, conduct);
        let first = conductor(&mut world, 100.0, 0.0, &[]);
        let second = match case {
            "frozen" => conductor(&mut world, 200.0, 0.0, &["status.frozen"]),
            _ => conductor(&mut world, 200.0, 0.0, &[]),
        };
        if case == "immune" {
            world.insert(second, Resist::new().with_factor("damage.lightning", 0.0));
        }
        let third = conductor(&mut world, 300.0, 0.0, &[]);
        let struck: Vec<Entity> = zap_steps(&mut world, &mut schedule, caster, 5, true)
            .into_iter()
            .flatten()
            .map(|(entity, _)| entity)
            .collect();
        let expected = if case == "jumps" {
            vec![first, second]
        } else {
            vec![first]
        };
        assert_eq!(struck, expected, "{case}");
        assert_eq!(
            world.get::<Health>(third).unwrap().current(),
            100.0,
            "{case}"
        );
    }
    // A frozen target takes no strike at all.
    let (mut world, mut schedule) = rig();
    let caster = zapper(&mut world, bolt());
    conductor(&mut world, 100.0, 0.0, &["status.frozen"]);
    assert!(
        zap_steps(&mut world, &mut schedule, caster, 3, true)
            .iter()
            .all(Vec::is_empty)
    );
    // The load refuses a conduction on a `self` or `aim` target.
    for target in [json!({ "self": {} }), json!({ "aim": {} })] {
        assert!(
            serde_json::from_value::<Ability>(json!({
                "target": target,
                "payload": [{ "conduct": { "damage": 5, "reach": 100, "receivers": { "all": ["conductive"] } } }]
            }))
            .is_err()
        );
    }
}

/// What a restored entity takes, as a state's marker.
#[derive(Debug, Clone, Copy)]
struct Restored;

#[test]
fn a_chain_ends_when_its_user_or_last_receiver_goes() {
    use tungsten_core::save::{capture_world, name, restore_world};
    let bolt = || Conduct::new(20.0, 120.0, conductive());
    for gone in ["user", "receiver"] {
        let (mut world, mut schedule) = rig();
        let caster = zapper(&mut world, bolt());
        let first = conductor(&mut world, 100.0, 0.0, &[]);
        let next = conductor(&mut world, 200.0, 0.0, &[]);
        assert_eq!(
            zap_steps(&mut world, &mut schedule, caster, 1, true),
            [vec![(first, 20.0)]]
        );
        world.despawn(if gone == "user" { caster } else { first });
        assert!(
            zap_steps(&mut world, &mut schedule, caster, 4, false)
                .iter()
                .all(Vec::is_empty),
            "{gone} gone: no strike after"
        );
        assert_eq!(world.get::<Health>(next).unwrap().current(), 100.0);
    }
    // A capture restored into the same World, as a load's new run.
    let (mut world, mut schedule) = rig();
    let caster = zapper(&mut world, bolt());
    name(&mut world, caster, None, "caster").unwrap();
    let first = conductor(&mut world, 100.0, 0.0, &[]);
    let next = conductor(&mut world, 200.0, 0.0, &[]);
    zap_steps(&mut world, &mut schedule, caster, 1, true);
    let saved = capture_world(&world).unwrap();
    world.despawn(caster);
    let caster = zapper(&mut world, bolt());
    name(&mut world, caster, None, "caster").unwrap();
    restore_world(&mut world, &saved, &(Restored,)).unwrap();
    for _ in 0..4 {
        steps(&mut world, &mut schedule, 1);
        assert!(
            world
                .resource::<EventQueue<crate::Damaged>>()
                .iter()
                .all(|hit| hit.entity != next),
            "the chain ended with its run"
        );
    }
    let _ = first;
}
