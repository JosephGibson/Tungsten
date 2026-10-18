use std::path::PathBuf;

use super::*;
use tungsten_core::{
    CommandBuffer, MaterialRegistry, MaterialUniformDefaults, Plugin, PrefabData, PrefabRegistry,
    Schedule, Sprite, SpriteAssetId, Stage, Time, World, system,
};

use crate::damage::health::{HIT_FLASH_MATERIAL, HealthPlugin, HitFlash};
use crate::foundation::phases::Phase;
use crate::foundation::trace::KitTrace;
use crate::motion::knockback::Knockback;
use crate::react::counters::{Counters, CountersPlugin};
use crate::react::cues::{CUES, CueAction, Cues, CuesPlugin};
use crate::{Resist, Shield};

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
