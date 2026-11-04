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

// Step 8: particles at a point, a light and a sound loop.

/// The rig with the particle config `sparks`, the slime's `fuse` point at
/// (12, 2), and the cue plugin with audio, the looping sound `hum` and a
/// listener at the origin.
fn rig_with_parts() -> (World, Schedule) {
    use std::sync::Arc;
    use tungsten_core::{AudioCommands, ParticleConfig, SoundData};

    let (mut world, _) = rig();
    let sparks: ParticleConfig = serde_json::from_value(serde_json::json!({
        "sprite": "rime_0",
        "max_alive": 64,
        "emission": { "kind": "continuous", "rate_hz": 30.0 },
        "lifetime": { "min": 0.5, "max": 0.5 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 0.0, "max": 0.0 } }
    }))
    .unwrap();
    let mut configs = ParticleConfigRegistry::new();
    configs.register(
        "sparks".into(),
        PathBuf::from("sparks.json"),
        Arc::new(sparks),
    );
    world.insert_resource(configs);
    world
        .resource_mut::<AssetRegistry>()
        .set_sprite_points("slime", [("fuse".to_string(), [12.0, 2.0])].into());
    let mut sounds = SoundRegistry::new();
    let hum = SoundData {
        samples: vec![0.0; 4],
        sample_rate: 44_100,
        channels: 1,
    };
    sounds.register("hum".to_string(), hum, 1.0, true);
    world.insert_resource(sounds);
    world.insert_resource(AudioCommands::new());
    world.insert_resource(crate::CueListener::new(100.0, 500.0));
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["status.other", "status.more"])
        .build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    crate::CuesPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn emitter(world: &World) -> Option<(Vec2, f32)> {
    world
        .query::<(&tungsten_core::ParticleEmitter, &Transform)>()
        .map(|(emitter, transform)| (transform.position, emitter.rate_scale))
        .next()
}

#[test]
fn look_particles_follow_a_named_point() {
    let (mut world, mut schedule) = rig_with_parts();
    let half = |look: Look| {
        Effect::new()
            .with_stack(2, crate::Refresh::Refresh, crate::Expire::Clear)
            .with_look(look)
    };
    insert(
        &mut world,
        "fused",
        half(Look::new().with_particles_at("sparks", "fuse")),
    );
    insert(
        &mut world,
        "aimless",
        half(Look::new().with_particles_at("sparks", "spout")),
    );
    let keg = slime(&mut world, Vec2::new(100.0, 50.0));
    apply(&mut world, keg, "fused", 1.0);
    frame(&mut world, &mut schedule);
    let (at, rate) = emitter(&world).unwrap();
    assert!(close(at, Vec2::new(112.0, 52.0)), "{at}");
    assert_eq!(rate, 0.5, "the share: one stack of two");
    // Mirrored with its holder, then turned with it about its centre.
    world.insert(keg, SpriteFlip(true));
    frame(&mut world, &mut schedule);
    assert!(close(emitter(&world).unwrap().0, Vec2::new(104.0, 52.0)));
    world.insert(keg, SpriteFlip(false));
    world.get_mut::<Transform>(keg).unwrap().rotation = std::f32::consts::FRAC_PI_2;
    frame(&mut world, &mut schedule);
    assert!(close(emitter(&world).unwrap().0, Vec2::new(114.0, 62.0)));
    // A point the frame lacks: the drawn centre.
    assert!(end_effect(&mut world, keg, &DataRef::by_id("fused")));
    world.get_mut::<Transform>(keg).unwrap().rotation = 0.0;
    apply(&mut world, keg, "aimless", 1.0);
    frame(&mut world, &mut schedule);
    let emitters: Vec<_> = world
        .query::<&tungsten_core::ParticleEmitter>()
        .map(|emitter| emitter.rate_scale)
        .collect();
    assert_eq!(emitters.len(), 1, "the old emitter went with its look");
    assert!(close(emitter(&world).unwrap().0, Vec2::new(108.0, 58.0)));
}

/// One fixed step of 1/60 s, its spawns flushed.
fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

#[test]
fn a_look_lights_its_holder() {
    use tungsten_core::Light;
    use tungsten_core::physics::Position;

    let (mut world, mut schedule) = rig_with_parts();
    let glowing = Effect::new()
        .with_intensity(4.0, 1.0)
        .with_look(Look::new().with_light([1.0, 0.5, 0.25, 1.0], 40.0, 2.0));
    insert(&mut world, "glowing", glowing);
    let holder = world.spawn_with((Position(Vec2::new(100.0, 50.0)),));
    apply(&mut world, holder, "glowing", 4.0);
    let light = |world: &World| -> Vec<(Vec2, f32)> {
        world
            .query::<(&Transform, &Light)>()
            .map(|(transform, light)| (transform.position, light.intensity))
            .collect()
    };
    step(&mut world, &mut schedule);
    let lit = light(&world);
    assert_eq!(lit.len(), 1, "spawned at once");
    assert!(close(lit[0].0, Vec2::new(100.0, 50.0)));
    assert!((lit[0].1 - 2.0 * (4.0 - DT) / 4.0).abs() < 1e-4, "{lit:?}");
    // Moved and dimmed each step as the intensity wears.
    world.get_mut::<Position>(holder).unwrap().0 = Vec2::new(140.0, 50.0);
    for _ in 0..59 {
        step(&mut world, &mut schedule);
    }
    let lit = light(&world);
    assert!(close(lit[0].0, Vec2::new(140.0, 50.0)));
    assert!((lit[0].1 - 1.5).abs() < 1e-3, "a second worn: {lit:?}");
    assert!(end_effect(&mut world, holder, &DataRef::by_id("glowing")));
    step(&mut world, &mut schedule);
    assert!(
        light(&world).is_empty(),
        "gone the step after the look ends"
    );
}

/// The audio commands since the last drain: plays' volumes, volume
/// changes and stops' fades.
fn heard(world: &mut World) -> Vec<(&'static str, f32)> {
    use tungsten_core::{AudioCommand, AudioCommands};

    world
        .resource_mut::<AudioCommands>()
        .drain()
        .into_iter()
        .filter_map(|command| match command {
            AudioCommand::Play { volume, .. } => Some(("play", volume)),
            AudioCommand::SetVoiceVolume { volume, .. } => Some(("volume", volume)),
            AudioCommand::StopVoice { fade, .. } => Some(("stop", fade)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_look_loops_its_sound() {
    use tungsten_core::physics::Position;

    let (mut world, mut schedule) = rig_with_parts();
    let humming = Effect::new()
        .with_intensity(4.0, 0.0)
        .with_look(Look::new().with_sound("hum", 1.0));
    let droning = Effect::new().with_look(Look::new().with_sound("hum", 0.25));
    insert(&mut world, "humming", humming);
    insert(&mut world, "droning", droning);
    let holder = world.spawn_with((Position(Vec2::ZERO),));
    apply(&mut world, holder, "humming", 2.0);
    step(&mut world, &mut schedule);
    assert_eq!(heard(&mut world), [("play", 0.5)], "one voice at the share");
    apply(&mut world, holder, "humming", 2.0);
    step(&mut world, &mut schedule);
    assert_eq!(heard(&mut world), [("volume", 1.0)]);
    // Away from the listener, by its gain.
    world.get_mut::<Position>(holder).unwrap().0 = Vec2::new(300.0, 0.0);
    step(&mut world, &mut schedule);
    assert_eq!(heard(&mut world), [("volume", 0.25)]);
    world.get_mut::<Position>(holder).unwrap().0 = Vec2::ZERO;
    // Two looks and a cue loop on one sound share its voice at the
    // largest want; the first look's end leaves it playing.
    world.insert(
        holder,
        crate::Cues::new().on(
            "effect_applied",
            [crate::CueAction::looping("hum").with_volume(0.75)],
        ),
    );
    apply(&mut world, holder, "droning", 1.0);
    step(&mut world, &mut schedule);
    assert_eq!(heard(&mut world), [("volume", 1.0)], "the largest, 1.0");
    assert!(end_effect(&mut world, holder, &DataRef::by_id("humming")));
    step(&mut world, &mut schedule);
    assert_eq!(
        heard(&mut world),
        [("volume", 0.75)],
        "the cue's, still playing"
    );
    world.remove_component::<crate::Cues>(holder);
    world.resource_mut::<crate::react::cues::CueLoops>().let_go(
        "hum",
        crate::react::cues::Wanter::Cue(holder),
        0.1,
    );
    step(&mut world, &mut schedule);
    assert_eq!(heard(&mut world), [("volume", 0.25)], "the drone's");
    assert!(end_effect(&mut world, holder, &DataRef::by_id("droning")));
    step(&mut world, &mut schedule);
    assert_eq!(heard(&mut world), [("stop", 0.1)], "faded out at the end");
}

#[test]
fn a_look_names_only_what_the_world_holds() {
    let (world, _) = rig_with_parts();
    for (look, field) in [
        (Look::new().with_particles("nope"), "look.particles.config"),
        (
            Look::new().with_overlay(Overlay::new("nope")),
            "look.overlay.animation",
        ),
        (
            Look::new().with_overlay(Overlay::new("rime").with_material("nope")),
            "look.overlay.material",
        ),
        (Look::new().with_sound("nope", 1.0), "look.sound.id"),
    ] {
        let error = Effect::new().with_look(look).check(&world).unwrap_err();
        assert_eq!(error.field, field);
    }
    let fine = Look::new()
        .with_particles("sparks")
        .with_overlay(Overlay::new("rime").with_material("engine_glow"))
        .with_sound("hum", 1.0);
    assert!(Effect::new().with_look(fine).check(&world).is_ok());
    let banded = Effect::new()
        .with_intensity(2.0, 0.0)
        .with_band(1.0, ["status.other"])
        .with_band_look(1.0, Look::new().with_sound("nope", 1.0));
    assert_eq!(
        banded.check(&world).unwrap_err().field,
        "bands.0.look.sound.id"
    );
}
