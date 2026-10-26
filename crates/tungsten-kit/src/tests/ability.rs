use super::*;
use serde_json::json;
use tungsten_core::physics::PrevPosition;
use tungsten_core::{Bundle, ComponentRegistry, PrefabData, Schedule, Stage};

use crate::damage::effects::{Effect, Effects, EffectsPlugin};
use crate::damage::health::HealthPlugin;
use crate::foundation::stats::StatsPlugin;
use crate::foundation::tags::TagsPlugin;
use crate::foundation::trace::KitTrace;
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
