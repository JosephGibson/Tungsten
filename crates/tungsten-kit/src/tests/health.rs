use std::path::PathBuf;

use super::*;
use tungsten_core::physics::PhysicsPlugin;
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
fn the_plugin_registers_its_queue_and_both_systems() {
    let (world, schedule) = rig();
    assert!(world.has_event::<Damaged>());
    assert_eq!(schedule.stage_of(HEALTH_TICK), Some(Stage::FixedUpdate));
    assert_eq!(schedule.stage_of(HIT_FLASH_TICK), Some(Stage::PostUpdate));
    assert_eq!(schedule.len(), 2);
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
        [PHYSICS_PREV_SNAPSHOT, HEALTH_TICK, PHYSICS_STEP]
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
