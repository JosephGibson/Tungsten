use std::path::PathBuf;

use super::*;
use crate::{
    DAMAGE_RESOLVE, KIT_DESPAWN, KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE,
    KIT_TICK, SHIELD_TICK,
};
use tungsten_core::EventQueue;
use tungsten_core::Stage;
use tungsten_core::physics::{PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin};
use tungsten_core::{MaterialUniformDefaults, SpriteAssetId};

const STEP: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A rig whose `MaterialRegistry` holds `damage_flash` and another
/// material; returns their IDs.
fn flash_rig() -> (World, Schedule, MaterialAssetId, MaterialAssetId) {
    let (mut world, schedule) = rig();
    let mut materials = MaterialRegistry::new();
    let glow = materials.allocate(
        "glow",
        PathBuf::from("manifest.json"),
        "glow".to_string(),
        MaterialUniformDefaults::default(),
    );
    let flash = materials.allocate(
        HIT_FLASH_MATERIAL,
        PathBuf::from("manifest.json"),
        "damage_flash".to_string(),
        MaterialUniformDefaults::default(),
    );
    world.insert_resource(materials);
    (world, schedule, flash, glow)
}

fn fixed_steps(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(STEP);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

fn post_update(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

fn sent(world: &World) -> Vec<Damaged> {
    world
        .resource::<EventQueue<Damaged>>()
        .iter()
        .copied()
        .collect()
}

#[test]
fn the_plugin_registers_its_queues_and_its_systems() {
    let (world, schedule) = rig();
    assert!(world.has_event::<Damaged>() && world.has_event::<Healed>());
    assert!(world.has_event::<DamagedPart>());
    assert!(world.has_resource::<crate::damage::kinds::DamageTypeTable>());
    assert!(world.has_resource::<Requests>());
    assert_eq!(schedule.stage_of(HEALTH_TICK), Some(Stage::FixedUpdate));
    assert_eq!(schedule.stage_of(HIT_FLASH_TICK), Some(Stage::PostUpdate));
    assert_eq!(schedule.stage_of(DAMAGE_RESOLVE), Some(Stage::FixedUpdate));
    assert_eq!(
        schedule.len(),
        12,
        "the seven anchors, the health and shield ticks, the resolve, the flash and the kit's despawn"
    );
}

#[test]
fn damage_hurts_kills_and_ignores() {
    let (mut world, _) = rig();
    let target = world.spawn_with((Health::new(3.0),));
    let bystander = world.spawn();
    assert_eq!(damage(&mut world, target, 0.0), Hit::Ignored);
    assert_eq!(damage(&mut world, bystander, 1.0), Hit::Ignored);
    assert_eq!(damage(&mut world, target, 1.25), Hit::Hurt);
    assert_eq!(world.get::<Health>(target).unwrap().current(), 1.75);
    assert_eq!(damage(&mut world, target, 10.0), Hit::Killed);
    let health = world.get::<Health>(target).unwrap();
    assert_eq!((health.current(), health.max()), (0.0, 3.0));
    assert!(health.is_dead());
    assert_eq!(damage(&mut world, target, 1.0), Hit::Ignored, "the dead");
    assert_eq!(
        sent(&world),
        [
            Damaged::new(target, 1.25, false),
            Damaged::new(target, 10.0, true)
        ]
    );
}

#[test]
fn without_the_plugin_damage_sends_nothing() {
    let mut world = World::new();
    let target = world.spawn_with((Health::new(1.0),));
    assert_eq!(damage(&mut world, target, 0.5), Hit::Hurt);
    assert!(!world.has_resource::<EventQueue<Damaged>>());
}

#[test]
fn the_window_ignores_hits_until_its_steps_have_passed() {
    let (mut world, mut schedule) = rig();
    let target = world.spawn_with((Health::new(10.0).with_invulnerability(0.5),));
    assert_eq!(damage(&mut world, target, 1.0), Hit::Hurt);
    assert!(world.get::<Health>(target).unwrap().is_invulnerable());
    fixed_steps(&mut world, &mut schedule, 29);
    assert_eq!(damage(&mut world, target, 1.0), Hit::Ignored);
    fixed_steps(&mut world, &mut schedule, 1);
    assert!(!world.get::<Health>(target).unwrap().is_invulnerable());
    assert_eq!(damage(&mut world, target, 1.0), Hit::Hurt);
}

#[test]
fn a_paused_clock_holds_the_window() {
    let (mut world, mut schedule) = rig();
    let target = world.spawn_with((Health::new(10.0).with_invulnerability(0.1),));
    damage(&mut world, target, 1.0);
    world.resource_mut::<Time>().pause();
    fixed_steps(&mut world, &mut schedule, 20);
    assert!(world.get::<Health>(target).unwrap().is_invulnerable());
}

#[test]
fn heal_fills_up_to_max_and_the_dead_stay_dead() {
    let (mut world, _) = rig();
    let target = world.spawn_with((Health::new(4.0),));
    damage(&mut world, target, 3.0);
    world.get_mut::<Health>(target).unwrap().heal(1.5);
    assert_eq!(world.get::<Health>(target).unwrap().current(), 2.5);
    world.get_mut::<Health>(target).unwrap().heal(10.0);
    assert_eq!(world.get::<Health>(target).unwrap().current(), 4.0);
    damage(&mut world, target, 4.0);
    world.get_mut::<Health>(target).unwrap().heal(1.0);
    assert!(world.get::<Health>(target).unwrap().is_dead());
}

#[test]
fn a_new_max_keeps_the_fraction_and_the_dead_stay_dead() {
    let mut health = Health::new(5.0);
    health.set_max(8.0);
    assert_eq!((health.current(), health.max()), (8.0, 8.0));
    health.current = 2.0;
    health.set_max(4.0);
    assert_eq!((health.current(), health.max()), (1.0, 4.0));
    for ignored in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        health.set_max(ignored);
        assert_eq!(health.max(), 4.0);
    }
    health.current = 0.0;
    health.set_max(10.0);
    assert!(health.is_dead());
}

#[test]
fn a_flash_takes_the_material_and_block_then_restores_both() {
    let (mut world, mut schedule, flash, glow) = flash_rig();
    let own = UniformOverrideBlock::from_slots(
        [[0.1, 0.2, 0.3, 0.4], [0.5; 4], [0.0; 4], [0.0; 4]],
        [0.7, 0.8, 0.0, 0.0],
        [3, 0, 0, 0],
    );
    let target = world.spawn_with((
        Health::new(5.0),
        HitFlash::new([1.0, 0.0, 0.0, 1.0], 0.3),
        Sprite::new(SpriteAssetId::new(0)).with_material(glow),
        own,
    ));
    damage(&mut world, target, 1.0);
    assert!(world.get::<HitFlash>(target).unwrap().is_active());
    assert_eq!(
        world.get::<Sprite>(target).unwrap().material_id,
        Some(flash)
    );
    let block = *world.get::<UniformOverrideBlock>(target).unwrap();
    assert_eq!(block.vec4[0], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(block.f32s[0], 1.0);
    assert_eq!(block.vec4[1], [0.5; 4], "the rest of the block stays");

    let mut amounts = Vec::new();
    for _ in 0..3 {
        post_update(&mut world, &mut schedule, 0.1);
        amounts.push(world.get::<UniformOverrideBlock>(target).unwrap().f32s[0]);
    }
    assert_eq!(amounts[0], 1.0);
    assert!((amounts[1] - 2.0 / 3.0).abs() < 1e-5, "{amounts:?}");
    assert!((amounts[2] - 1.0 / 3.0).abs() < 1e-5, "{amounts:?}");
    post_update(&mut world, &mut schedule, 0.1);
    assert_eq!(world.get::<UniformOverrideBlock>(target), Some(&own));
    assert_eq!(world.get::<Sprite>(target).unwrap().material_id, Some(glow));
    assert!(!world.get::<HitFlash>(target).unwrap().is_active());
}

#[test]
fn a_flash_on_a_sprite_without_a_block_removes_the_one_it_inserted() {
    let (mut world, mut schedule, flash, _) = flash_rig();
    let target = world.spawn_with((
        Health::new(5.0),
        HitFlash::new([1.0; 4], 0.1),
        Sprite::new(SpriteAssetId::new(0)),
    ));
    damage(&mut world, target, 1.0);
    assert_eq!(
        world.get::<Sprite>(target).unwrap().material_id,
        Some(flash)
    );
    assert!(world.has::<UniformOverrideBlock>(target));
    post_update(&mut world, &mut schedule, 0.05);
    post_update(&mut world, &mut schedule, 0.05);
    assert!(world.has::<UniformOverrideBlock>(target));
    post_update(&mut world, &mut schedule, 0.05);
    assert!(!world.has::<UniformOverrideBlock>(target));
    assert_eq!(world.get::<Sprite>(target).unwrap().material_id, None);
}

#[test]
fn a_hit_during_a_flash_starts_it_over_and_keeps_the_first_state() {
    let (mut world, mut schedule, _, glow) = flash_rig();
    let target = world.spawn_with((
        Health::new(5.0),
        HitFlash::new([1.0; 4], 0.2),
        Sprite::new(SpriteAssetId::new(0)).with_material(glow),
    ));
    damage(&mut world, target, 1.0);
    post_update(&mut world, &mut schedule, 0.1);
    post_update(&mut world, &mut schedule, 0.1);
    damage(&mut world, target, 1.0);
    post_update(&mut world, &mut schedule, 0.1);
    assert_eq!(
        world.get::<UniformOverrideBlock>(target).unwrap().f32s[0],
        1.0
    );
    post_update(&mut world, &mut schedule, 0.1);
    post_update(&mut world, &mut schedule, 0.1);
    assert_eq!(world.get::<Sprite>(target).unwrap().material_id, Some(glow));
    assert!(!world.has::<UniformOverrideBlock>(target));
}

#[test]
fn the_flash_overrides_a_game_write_to_its_block_while_it_runs() {
    let (mut world, mut schedule, _, _) = flash_rig();
    let target = world.spawn_with((
        Health::new(5.0),
        HitFlash::new([0.0, 1.0, 0.0, 1.0], 0.2),
        Sprite::new(SpriteAssetId::new(0)),
    ));
    damage(&mut world, target, 1.0);
    world.get_mut::<UniformOverrideBlock>(target).unwrap().vec4[0] = [9.0; 4];
    post_update(&mut world, &mut schedule, 0.1);
    assert_eq!(
        world.get::<UniformOverrideBlock>(target).unwrap().vec4[0],
        [0.0, 1.0, 0.0, 1.0]
    );
}

#[test]
fn without_the_material_nothing_flashes() {
    let (mut world, mut schedule) = rig();
    let target = world.spawn_with((
        Health::new(5.0),
        HitFlash::new([1.0; 4], 0.2),
        Sprite::new(SpriteAssetId::new(0)),
    ));
    assert_eq!(damage(&mut world, target, 1.0), Hit::Hurt);
    assert!(!world.get::<HitFlash>(target).unwrap().is_active());
    assert!(world.get::<HitFlash>(target).unwrap().missing);
    post_update(&mut world, &mut schedule, 0.1);
    assert!(!world.get::<HitFlash>(target).unwrap().missing);
    assert!(!world.has::<UniformOverrideBlock>(target));
    assert_eq!(world.get::<Sprite>(target).unwrap().material_id, None);
}

#[test]
fn a_flash_without_a_sprite_does_nothing() {
    let (mut world, _, _, _) = flash_rig();
    let target = world.spawn_with((Health::new(5.0), HitFlash::new([1.0; 4], 0.2)));
    damage(&mut world, target, 1.0);
    assert!(!world.get::<HitFlash>(target).unwrap().is_active());
    assert!(!world.has::<UniformOverrideBlock>(target));
}

#[test]
fn the_window_count_runs_first_after_the_snapshot() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.names(Stage::FixedUpdate).collect::<Vec<_>>(),
        [
            PHYSICS_PREV_SNAPSHOT,
            KIT_TICK,
            HEALTH_TICK,
            SHIELD_TICK,
            KIT_INTENT,
            KIT_MOVE,
            PHYSICS_STEP,
            KIT_OVERLAP,
            KIT_RESOLVE,
            DAMAGE_RESOLVE,
            KIT_REACT,
            KIT_DESPAWN
        ]
    );
}

#[test]
#[should_panic(expected = "amount must be finite and >= 0")]
fn a_negative_amount_panics() {
    let mut world = World::new();
    let target = world.spawn_with((Health::new(1.0),));
    damage(&mut world, target, -1.0);
}

#[test]
#[should_panic(expected = "max must be finite and > 0")]
fn a_zero_max_panics() {
    let _ = Health::new(0.0);
}

#[test]
#[should_panic(expected = "seconds must be finite and >= 0")]
fn a_negative_window_panics() {
    let _ = Health::new(1.0).with_invulnerability(-0.5);
}

#[test]
#[should_panic(expected = "with_despawn_on_death: seconds must be finite and >= 0")]
fn a_negative_death_delay_panics() {
    let _ = Health::new(1.0).with_despawn_on_death(-0.5);
}

#[test]
#[should_panic(expected = "amount must be finite and >= 0")]
fn a_nan_heal_panics() {
    Health::new(1.0).heal(f32::NAN);
}

#[test]
#[should_panic(expected = "color must be finite")]
fn a_non_finite_flash_color_panics() {
    let _ = HitFlash::new([f32::NAN, 0.0, 0.0, 1.0], 0.1);
}

#[test]
#[should_panic(expected = "seconds must be finite and > 0")]
fn a_zero_flash_panics() {
    let _ = HitFlash::new([1.0; 4], 0.0);
}

#[test]
fn health_is_data_within_its_bounds() {
    let mut health = Health::new(5.0).with_invulnerability(0.5);
    health.current = 2.0;
    let value = serde_json::to_value(&health).unwrap();
    let back: Health = serde_json::from_value(value).unwrap();
    assert_eq!((back.current(), back.max()), (2.0, 5.0));
    assert!(!back.is_invulnerable());

    assert!(
        !value_has_policy(&serde_json::to_value(&health).unwrap()),
        "a health without the policy saves as before"
    );
    let mut dying = Health::new(5.0).with_despawn_on_death(0.25);
    assert!(dying.hurt(5.0, false));
    let value = serde_json::to_value(&dying).unwrap();
    assert_eq!(value["despawn_on_death"], 0.25);
    assert_eq!(value["death_left"], 0.25);
    let back: Health = serde_json::from_value(value).unwrap();
    assert_eq!(
        (back.despawn_on_death(), back.death_left),
        (Some(0.25), Some(0.25))
    );

    let refused = [
        serde_json::json!({ "current": 5.0, "max": -1.0 }),
        serde_json::json!({ "current": 6.0, "max": 5.0 }),
        serde_json::json!({ "current": -1.0, "max": 5.0 }),
        serde_json::json!({ "current": 1.0, "max": 5.0, "invulnerability": -0.5 }),
        serde_json::json!({ "current": 1.0, "max": 5.0, "window_left": f64::NAN }),
        serde_json::json!({ "current": 1.0, "max": 5.0, "lives": 3 }),
        serde_json::json!({ "current": 1.0, "max": 5.0, "despawn_on_death": -1.0 }),
        serde_json::json!({ "current": 1.0, "max": 5.0, "death_left": -0.5 }),
    ];
    for value in refused {
        assert!(
            serde_json::from_value::<Health>(value.clone()).is_err(),
            "{value}"
        );
    }
    let mut revived: Health =
        serde_json::from_value(serde_json::json!({ "current": 0.0, "max": 5.0 })).unwrap();
    assert!(revived.is_dead());
    revived.heal(3.0);
    assert!(revived.is_dead(), "the dead stay dead");
}

#[test]
fn a_hit_and_a_kill_go_in_the_trace() {
    use crate::foundation::trace::{KitTrace, TraceKind};

    let (mut world, _) = rig();
    world.insert_resource(KitTrace::default());
    let target = world.spawn_with((Health::new(2.0),));
    damage(&mut world, target, 1.0);
    damage(&mut world, target, 0.0);
    damage(&mut world, target, 3.0);
    let names: Vec<(&str, Option<f32>)> = world
        .resource::<KitTrace>()
        .about(target)
        .map(|entry| match entry.kind {
            TraceKind::Event { name, amount, .. } => (name, amount),
            ref other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        names,
        [
            ("damaged_part", Some(1.0)),
            ("damaged", Some(1.0)),
            ("damaged_part", Some(3.0)),
            ("damaged", Some(3.0)),
            ("killed", Some(3.0))
        ],
        "each part before its hit; an ignored hit records nothing"
    );
}

/// Whether a saved health names the death policy.
fn value_has_policy(value: &serde_json::Value) -> bool {
    value.get("despawn_on_death").is_some() || value.get("death_left").is_some()
}

#[test]
fn the_scene_form_takes_the_death_policy() {
    let (world, _) = rig();
    let components = world.resource::<tungsten_core::ComponentRegistry>();
    let check = |value| components.check_in(HEALTH, &value, &world);
    assert!(check(serde_json::json!({ "max": 5, "despawn_on_death": 0 })).is_ok());
    assert!(check(serde_json::json!({ "max": 5, "despawn_on_death": -1 })).is_err());
    let rows = Health::new(2.0).with_despawn_on_death(0.1).inspect_rows();
    assert_eq!(
        rows[2],
        ("death", "despawns 0.10 s after a kill".to_string())
    );
    assert_eq!(Health::new(2.0).inspect_rows()[2].1, "kept");
}

#[test]
fn a_hit_on_a_shield_still_starts_the_window_and_sends_damaged() {
    let (mut world, _) = rig();
    let target = world.spawn_with((
        Health::new(5.0).with_invulnerability(0.5),
        crate::Shield::new(3.0),
    ));
    assert_eq!(damage(&mut world, target, 2.0), Hit::Absorbed);
    assert!(world.get::<Health>(target).unwrap().is_invulnerable());
    assert_eq!(
        sent(&world),
        [Damaged::new(target, 2.0, false).with_absorbed(2.0)],
        "the shield took it whole"
    );
    let queue = world.resource::<EventQueue<Damaged>>();
    assert_eq!(queue.iter().count(), 1);
}

// 7.10p: the flash's interval and the shield's take.

#[test]
fn a_flash_waits_its_interval() {
    for (interval, starts_every) in [(0.5, 30), (0.0, 6)] {
        let (mut world, mut schedule, flash_material, _) = flash_rig();
        let flash = HitFlash::new([1.0; 4], 0.12).with_min_interval(interval);
        let target = world.spawn_with((
            Health::new(1000.0),
            flash,
            Sprite::new(SpriteAssetId::new(0)),
        ));
        // A blow every 0.1 s for 2 s, at frames of 1/60 s.
        let (mut starts, mut off) = (Vec::new(), 0);
        for frame in 0..120 {
            if frame % 6 == 0 {
                damage(&mut world, target, 1.0);
                if world.get::<HitFlash>(target).unwrap().fresh {
                    starts.push(frame);
                }
            }
            post_update(&mut world, &mut schedule, STEP);
            if world.get::<Sprite>(target).unwrap().material_id != Some(flash_material) {
                off += 1;
            }
        }
        let expected: Vec<i32> = (0..120).step_by(starts_every).collect();
        assert_eq!(starts, expected, "interval {interval}");
        if interval > 0.0 {
            assert!(
                off >= 60,
                "off {off} of 120 frames: the sprite shows between"
            );
        } else {
            assert_eq!(off, 0, "every blow restarts it, as before");
        }
    }
}

#[test]
fn a_whole_shield_hit_flashes_its_shield_colour() {
    const WHITE: [f32; 4] = [1.0; 4];
    const BLUE: [f32; 4] = [0.25, 0.5, 1.0, 1.0];
    for shield_colour in [Some(BLUE), None] {
        let (mut world, mut schedule, _, _) = flash_rig();
        let mut flash = HitFlash::new(WHITE, 0.1);
        if let Some(colour) = shield_colour {
            flash = flash.with_shield(colour);
        }
        let knight = world.spawn_with((
            Health::new(100.0),
            Shield::new(50.0),
            flash,
            Sprite::new(SpriteAssetId::new(0)),
        ));
        let shown = |world: &World| world.get::<UniformOverrideBlock>(knight).unwrap().vec4[0];
        assert_eq!(damage(&mut world, knight, 30.0), Hit::Absorbed);
        assert_eq!(
            world.get::<Health>(knight).unwrap().current(),
            100.0,
            "whole"
        );
        assert_eq!(shown(&world), shield_colour.unwrap_or(WHITE));
        post_update(&mut world, &mut schedule, STEP);
        assert_eq!(
            shown(&world),
            shield_colour.unwrap_or(WHITE),
            "the tick keeps it"
        );
        for _ in 0..12 {
            post_update(&mut world, &mut schedule, STEP);
        }
        damage(&mut world, knight, 80.0);
        assert_eq!(world.get::<Health>(knight).unwrap().current(), 40.0);
        assert_eq!(shown(&world), WHITE, "a hit that hurt");
    }
}

#[test]
fn a_disarming_death_switches_off_the_touch_and_the_hitbox() {
    use crate::{ContactDamage, HitShape, Hitbox};
    let (mut world, _) = rig();
    let armed = || {
        (
            ContactDamage::new(1.0),
            Hitbox::new(HitShape::circle(4.0), 1.0),
        )
    };
    let ghoul = world.spawn_with((Health::new(2.0).disarming(), armed().0, armed().1));
    let plain = world.spawn_with((Health::new(2.0), armed().0, armed().1));
    assert!(world.get::<Health>(ghoul).unwrap().disarms());
    for target in [ghoul, plain] {
        assert_eq!(damage(&mut world, target, 2.0), Hit::Killed);
    }
    let on = |world: &World, entity: Entity| {
        (
            world.get::<ContactDamage>(entity).unwrap().is_on(),
            world.get::<Hitbox>(entity).unwrap().is_on(),
        )
    };
    assert_eq!(on(&world, ghoul), (false, false), "disarmed at its kill");
    assert_eq!(on(&world, plain), (true, true), "a plain death keeps them");
    // A heal brings no one back, so nothing switches on again.
    crate::heal(&mut world, ghoul, 2.0);
    assert_eq!(on(&world, ghoul), (false, false));
    // The scene names it; a save writes it only when set.
    let components = world.resource::<tungsten_core::ComponentRegistry>();
    let scene = serde_json::json!({ "max": 2, "disarm": true });
    assert!(components.check_in("health", &scene, &world).is_ok());
    let saved = serde_json::to_value(world.get::<Health>(ghoul).unwrap()).unwrap();
    assert_eq!(saved["disarm"], true);
    assert!(serde_json::from_value::<Health>(saved).unwrap().disarms());
    let plain = serde_json::to_value(world.get::<Health>(plain).unwrap()).unwrap();
    assert!(
        plain.get("disarm").is_none(),
        "a plain health keeps its bytes"
    );
}

#[test]
fn a_disarming_death_stops_and_locks_the_controller() {
    use glam::Vec2;
    use tungsten_core::physics::{
        Collider, PhysicsConfig, PhysicsPlugin, Position, RigidBodyBundle, Velocity,
    };
    use tungsten_core::{Bundle, TilemapRegistry};

    use crate::{PlatformerController, PlatformerControllerPlugin};

    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, 2000.0),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    PlatformerControllerPlugin.build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let hero = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(8.0, 16.0)))
            .with((
                PlatformerController::new(200.0, 600.0),
                Health::new(1.0).disarming(),
            )),
    );
    world
        .get_mut::<PlatformerController>(hero)
        .unwrap()
        .set_run(1.0);
    fixed_steps(&mut world, &mut schedule, 10);
    let running = world.get::<Velocity>(hero).unwrap().0;
    assert!(
        running.x > 0.0 && running.y > 0.0,
        "running and falling: {running}"
    );
    assert_eq!(damage(&mut world, hero, 1.0), Hit::Killed);
    world
        .get_mut::<PlatformerController>(hero)
        .unwrap()
        .set_run(-1.0);
    world
        .get_mut::<PlatformerController>(hero)
        .unwrap()
        .press_jump();
    let x = world.get::<Position>(hero).unwrap().0.x;
    fixed_steps(&mut world, &mut schedule, 10);
    let velocity = world.get::<Velocity>(hero).unwrap().0;
    assert_eq!(velocity.x, 0.0, "its run stopped, the input locked");
    assert!(velocity.y > running.y, "its fall kept and growing");
    assert_eq!(world.get::<Position>(hero).unwrap().0.x, x, "straight down");
}
