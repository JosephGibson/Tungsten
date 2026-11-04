use std::path::PathBuf;

use super::*;
use tungsten_core::assets::{AnimationData, AnimationFrame, FilterMode, TextureHandle, UvRect};
use tungsten_core::{
    CommandBuffer, DataRef, DataRegistry, MaterialUniformDefaults, Plugin, Schedule, Stage, Time,
};

use crate::{
    Effect, EffectRequest, EffectsPlugin, HealthPlugin, HitFlash, StatsPlugin, TagsPlugin, damage,
    end_effect,
};

const DT: f32 = 1.0 / 60.0;
const WHITE: [u8; 4] = [255; 4];

/// A World with a clock, a command buffer, the sprites `slime` (16 × 16),
/// `rime_0`, `rime_1` and `ember` (8 × 8) and `engine_white`, the looping
/// clips `rime` and `ember`, the materials `engine_glow` and
/// `damage_flash`, and the effect, health, stats and tags plugins.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let mut assets = AssetRegistry::new();
    let mut sprite = |id: &str, size: u32| {
        assets.register_sprite(
            id.to_string(),
            FilterMode::Nearest,
            size,
            size,
            PathBuf::from(format!("{id}.png")),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        )
    };
    sprite("slime", 16);
    let rime = [sprite("rime_0", 8), sprite("rime_1", 8)];
    let ember = sprite("ember", 8);
    sprite("engine_white", 1);
    world.insert_resource(assets);
    let mut clips = AnimationRegistry::new();
    let clip = |sprites: &[tungsten_core::SpriteAssetId]| AnimationData {
        looping: true,
        frames: sprites
            .iter()
            .map(|&sprite| AnimationFrame {
                sprite,
                duration_ms: 100,
                sound: None,
            })
            .collect(),
    };
    clips.insert("rime".to_string(), clip(&rime));
    clips.insert("ember".to_string(), clip(&[ember]));
    world.insert_resource(clips);
    let mut materials = MaterialRegistry::new();
    for (id, shader) in [("engine_glow", "glow"), ("damage_flash", "damage_flash")] {
        materials.allocate(
            id,
            PathBuf::from("manifest.json"),
            shader.to_string(),
            MaterialUniformDefaults::default(),
        );
    }
    world.insert_resource(materials);
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["status.other", "status.more"])
        .build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One frame: a step of the effects, its spawns flushed, then
/// `post_update`.
fn frame(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
    world.flush_events();
}

fn frames(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        frame(world, schedule);
    }
}

fn insert(world: &mut World, id: &str, effect: Effect) {
    world.resource_mut::<DataRegistry>().insert(id, effect);
}

/// A 16 × 16 slime sprite at `at`.
fn slime(world: &mut World, at: Vec2) -> Entity {
    let sprite = world
        .resource::<AssetRegistry>()
        .sprite_id("slime")
        .unwrap();
    world.spawn_with((
        Transform::from_position(at),
        Sprite::new(sprite),
        Visibility::default(),
    ))
}

fn apply(world: &mut World, target: Entity, id: &str, amount: f32) {
    EffectRequest::new(target, DataRef::by_id(id))
        .with_amount(amount)
        .apply(world);
}

fn color(world: &World, entity: Entity) -> [u8; 4] {
    world.get::<Sprite>(entity).unwrap().color
}

/// Every overlay: its clip, transform, depth and flip.
fn overlays(world: &World) -> Vec<(String, Transform, i32, bool)> {
    let mut found: Vec<_> = world
        .query::<(&AnimationState, &Transform, &Sprite, &SpriteFlip)>()
        .map(|(state, transform, sprite, flip)| {
            (
                state.animation_id.clone(),
                *transform,
                sprite.z_order,
                flip.0,
            )
        })
        .collect();
    found.sort_by(|a, b| a.1.position.x.total_cmp(&b.1.position.x));
    found
}

fn close(a: Vec2, b: Vec2) -> bool {
    a.distance(b) < 1e-4
}

#[test]
fn a_look_tints_by_its_share() {
    let (mut world, mut schedule) = rig();
    let chill = Effect::new()
        .with_intensity(4.0, 0.0)
        .with_look(Look::new().with_tint([0.5, 0.5, 1.0, 1.0]));
    insert(&mut world, "chill", chill);
    let target = slime(&mut world, Vec2::ZERO);
    apply(&mut world, target, "chill", 2.0);
    frame(&mut world, &mut schedule);
    assert_eq!(
        color(&world, target),
        [191, 191, 255, 255],
        "halfway at 2 of 4"
    );
    apply(&mut world, target, "chill", 2.0);
    frame(&mut world, &mut schedule);
    assert_eq!(color(&world, target), [128, 128, 255, 255], "whole at 4");
    // Two tints multiply: a plain effect of one stack, its limit 1.
    let warm = Effect::new().with_look(Look::new().with_tint([1.0, 0.5, 0.5, 1.0]));
    insert(&mut world, "warm", warm);
    apply(&mut world, target, "warm", 1.0);
    frame(&mut world, &mut schedule);
    assert_eq!(color(&world, target), [128, 64, 128, 255]);
    assert!(end_effect(&mut world, target, &DataRef::by_id("chill")));
    assert!(end_effect(&mut world, target, &DataRef::by_id("warm")));
    frame(&mut world, &mut schedule);
    assert_eq!(
        color(&world, target),
        WHITE,
        "its own colour back at the end"
    );
    // A plain effect's share: its stacks over its limit.
    let dusk = Effect::new()
        .with_stack(2, crate::Refresh::Refresh, crate::Expire::Clear)
        .with_look(Look::new().with_tint([0.0, 0.0, 0.0, 1.0]));
    insert(&mut world, "dusk", dusk);
    apply(&mut world, target, "dusk", 1.0);
    frame(&mut world, &mut schedule);
    assert_eq!(
        color(&world, target),
        [128, 128, 128, 255],
        "one stack of two"
    );
}

#[test]
fn an_overlay_rides_its_holder() {
    let (mut world, mut schedule) = rig();
    let rimed = Effect::new().with_look(
        Look::new().with_overlay(Overlay::new("rime").with_offset(Vec2::new(4.0, -2.0))),
    );
    insert(&mut world, "rimed", rimed);
    let first = slime(&mut world, Vec2::new(100.0, 50.0));
    world.get_mut::<Sprite>(first).unwrap().z_order = 5;
    apply(&mut world, first, "rimed", 1.0);
    frame(&mut world, &mut schedule);
    let shown = overlays(&world);
    assert_eq!(shown.len(), 1);
    let (clip, transform, z, flip) = &shown[0];
    assert_eq!(
        (clip.as_str(), *z, *flip),
        ("rime", 6, false),
        "one above in z"
    );
    // At the drawn centre (108, 58) plus its offset, an 8 px clip's centre.
    assert!(
        close(transform.position, Vec2::new(108.0, 52.0)),
        "{transform:?}"
    );
    // Mirrored by the flip, turned with the rotation.
    world.insert(first, SpriteFlip(true));
    frame(&mut world, &mut schedule);
    let (_, transform, _, flip) = &overlays(&world)[0];
    assert!(
        *flip && close(transform.position, Vec2::new(100.0, 52.0)),
        "{transform:?}"
    );
    world.insert(first, SpriteFlip(false));
    world.get_mut::<Transform>(first).unwrap().rotation = std::f32::consts::FRAC_PI_2;
    frame(&mut world, &mut schedule);
    let (_, transform, _, _) = &overlays(&world)[0];
    assert!(
        close(transform.position, Vec2::new(106.0, 58.0)),
        "{transform:?}"
    );
    assert_eq!(transform.rotation, std::f32::consts::FRAC_PI_2);
    // Holders do not play in step.
    let second = slime(&mut world, Vec2::new(300.0, 50.0));
    let third = slime(&mut world, Vec2::new(500.0, 50.0));
    apply(&mut world, second, "rimed", 1.0);
    apply(&mut world, third, "rimed", 1.0);
    frame(&mut world, &mut schedule);
    let phases: Vec<(usize, u32)> = world
        .query::<&AnimationState>()
        .map(|state| (state.frame_index, state.accumulated_ms as u32))
        .collect();
    assert_eq!(phases.len(), 3);
    assert!(
        phases.windows(2).any(|pair| pair[0] != pair[1]),
        "{phases:?}"
    );
    // Gone with the status, with a despawning holder and with a gone one.
    assert!(end_effect(&mut world, first, &DataRef::by_id("rimed")));
    world.insert(second, crate::lifecycle::ended::Despawning);
    world.despawn(third);
    frame(&mut world, &mut schedule);
    assert!(overlays(&world).is_empty(), "{:?}", overlays(&world));
}

#[test]
fn a_band_look_replaces_its_effects() {
    let (mut world, mut schedule) = rig();
    let chill = Effect::new()
        .with_intensity(4.0, 1.0)
        .with_band(1.0, ["status.other"])
        .with_band(3.0, ["status.more"])
        .with_band_look(1.0, Look::new().with_overlay(Overlay::new("rime")))
        .with_band_look(3.0, Look::new().with_overlay(Overlay::new("ember")));
    insert(&mut world, "chill", chill);
    let target = slime(&mut world, Vec2::ZERO);
    apply(&mut world, target, "chill", 3.5);
    frame(&mut world, &mut schedule);
    let clips = |world: &World| -> Vec<String> {
        overlays(world).into_iter().map(|(clip, ..)| clip).collect()
    };
    assert_eq!(clips(&world), ["ember"], "band 2's at its threshold");
    frames(&mut world, &mut schedule, 60);
    assert_eq!(clips(&world), ["rime"], "worn below 3: band 1's");
    frames(&mut world, &mut schedule, 120);
    assert!(clips(&world).is_empty(), "worn out");
}

#[test]
fn a_glow_sits_under_its_holder() {
    let (mut world, mut schedule) = rig();
    let lit = Effect::new()
        .with_stack(2, crate::Refresh::Refresh, crate::Expire::Clear)
        .with_look(Look::new().with_glow([1.0, 0.5, 0.0, 1.0], 10.0));
    insert(&mut world, "lit", lit);
    let target = slime(&mut world, Vec2::new(100.0, 50.0));
    world.get_mut::<Sprite>(target).unwrap().z_order = 5;
    apply(&mut world, target, "lit", 1.0);
    frame(&mut world, &mut schedule);
    let glow_material = world.resource::<MaterialRegistry>().get("engine_glow");
    let glows: Vec<(Transform, [u8; 4], i32)> = world
        .query::<(&Transform, &Sprite)>()
        .filter(|(_, sprite)| sprite.material_id == glow_material)
        .map(|(transform, sprite)| (*transform, sprite.color, sprite.z_order))
        .collect();
    assert_eq!(glows.len(), 1);
    let (transform, color, z) = glows[0];
    assert!(
        close(transform.position, Vec2::new(98.0, 48.0)),
        "{transform:?}"
    );
    assert_eq!(transform.scale, Vec2::splat(20.0), "10 px about the centre");
    assert_eq!(
        (color, z),
        ([255, 128, 0, 128], 4),
        "half: one stack of two"
    );
}

#[test]
fn looks_come_back_after_a_load() {
    use tungsten_core::save::{self, capture_world, restore_world};

    #[derive(Debug, Clone, Copy)]
    struct Marker;

    let rimed = Effect::new()
        .with_duration(10.0)
        .with_look(Look::new().with_overlay(Overlay::new("rime")));
    let (mut world, mut schedule) = rig();
    insert(&mut world, "rimed", rimed.clone());
    let target = slime(&mut world, Vec2::ZERO);
    save::name(&mut world, target, None, "slime").unwrap();
    apply(&mut world, target, "rimed", 1.0);
    frame(&mut world, &mut schedule);
    assert_eq!(overlays(&world).len(), 1);
    let saved = capture_world(&world).unwrap();
    // A fresh World, its run spawned, the capture restored.
    let (mut fresh, mut fresh_schedule) = rig();
    insert(&mut fresh, "rimed", rimed);
    let again = slime(&mut fresh, Vec2::ZERO);
    save::name(&mut fresh, again, None, "slime").unwrap();
    restore_world(&mut fresh, &saved, &(Marker,)).unwrap();
    assert!(overlays(&fresh).is_empty(), "parts are unsaved");
    frame(&mut fresh, &mut fresh_schedule);
    assert_eq!(overlays(&fresh).len(), 1, "back after one step");
}

#[test]
fn the_tint_shows_between_flashes() {
    let (mut world, mut schedule) = rig();
    let chill = Effect::new()
        .with_intensity(4.0, 0.0)
        .with_look(Look::new().with_tint([0.5, 0.5, 1.0, 1.0]));
    insert(&mut world, "chill", chill);
    let target = slime(&mut world, Vec2::ZERO);
    world.insert(target, crate::Health::new(1000.0));
    world.insert(target, HitFlash::new([1.0; 4], 0.12).with_min_interval(0.5));
    apply(&mut world, target, "chill", 4.0);
    let flash = world.resource::<MaterialRegistry>().get("damage_flash");
    let mut off = 0;
    for frame_index in 0..120 {
        if frame_index % 6 == 0 {
            damage(&mut world, target, 1.0);
        }
        frame(&mut world, &mut schedule);
        assert_eq!(
            color(&world, target),
            [128, 128, 255, 255],
            "frame {frame_index}"
        );
        off += u32::from(world.get::<Sprite>(target).unwrap().material_id != flash);
    }
    assert!(off >= 60, "off {off} of 120 frames");
}
