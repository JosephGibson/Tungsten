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
/// `rime_0`, `rime_1` and `ember` (8 × 8), `pin` (8 × 8, anchored at
/// (2, 6)) and `engine_white`, the looping clips `rime`, `ember` and `pin`,
/// the materials `engine_glow` and `damage_flash`, and the effect, health,
/// stats and tags plugins.
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
    let pin = sprite("pin", 8);
    sprite("engine_white", 1);
    assets.set_sprite_frame("pin", None, Some([2.0, 6.0]));
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
    clips.insert("pin".to_string(), clip(&[pin]));
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

/// A slime at `at` wearing `looks`, with tags.
fn tagged_slime(world: &mut World, at: Vec2, looks: Looks) -> Entity {
    let slime = slime(world, at);
    world.insert(slime, Tags::new(Vec::<String>::new()));
    world.insert(slime, looks);
    slime
}

fn set_tag(world: &mut World, holder: Entity, tag: &str, on: bool) {
    let tags = world.get_mut::<Tags>(holder).unwrap();
    if on {
        tags.insert(tag);
    } else {
        tags.remove(tag);
    }
}

/// Every glow: its transform, colour and depth.
fn glows(world: &World) -> Vec<(Transform, [u8; 4], i32)> {
    let material = world.resource::<MaterialRegistry>().get("engine_glow");
    world
        .query::<(&Transform, &Sprite)>()
        .filter(|(_, sprite)| sprite.material_id == material)
        .map(|(transform, sprite)| (*transform, sprite.color, sprite.z_order))
        .collect()
}

#[test]
fn a_tag_look_shows_while_its_filter_passes() {
    use tungsten_core::ComponentRegistry;
    let (mut world, mut schedule) = rig();
    let mark = Look::new()
        .with_tint([0.5, 1.0, 1.0, 1.0])
        .with_overlay(
            Overlay::new("rime")
                .with_offset(Vec2::new(4.0, -12.0))
                .with_z(3),
        )
        .with_glow([1.0, 0.5, 0.0, 1.0], 6.0);
    let looks = Looks::new().with(TagLook::new(TagFilter::new().all(["status.other"]), mark));
    let slime = tagged_slime(&mut world, Vec2::new(100.0, 100.0), looks);
    frame(&mut world, &mut schedule);
    assert!(overlays(&world).is_empty() && glows(&world).is_empty());
    assert_eq!(color(&world, slime), WHITE, "untagged, nothing shows");
    set_tag(&mut world, slime, "status.other", true);
    frame(&mut world, &mut schedule);
    let shown = overlays(&world);
    assert_eq!(shown.len(), 1);
    let (clip, transform, z, flipped) = &shown[0];
    assert_eq!((clip.as_str(), *z, *flipped), ("rime", 3, false));
    // Centre (108, 108) plus (4, -12), less the 8 px rime's half.
    assert!(
        close(transform.position, Vec2::new(108.0, 92.0)),
        "{transform:?}"
    );
    assert_eq!(glows(&world).len(), 1);
    assert_eq!(
        color(&world, slime),
        [128, 255, 255, 255],
        "tinted at a share of 1"
    );
    for _ in 0..3 {
        frame(&mut world, &mut schedule);
    }
    assert_eq!(overlays(&world).len(), 1, "kept, not spawned again");
    set_tag(&mut world, slime, "status.other", false);
    frame(&mut world, &mut schedule);
    assert!(overlays(&world).is_empty() && glows(&world).is_empty());
    assert_eq!(color(&world, slime), WHITE, "its own colour back");
    // A despawning holder's parts go.
    set_tag(&mut world, slime, "status.other", true);
    frame(&mut world, &mut schedule);
    world.insert(slime, Despawning);
    frame(&mut world, &mut schedule);
    assert!(overlays(&world).is_empty());

    // The load checks the tags and IDs and refuses a light or a sound.
    LooksPlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in("looks", &value, &world)
            .map_err(|error| error.to_string())
    };
    check(serde_json::json!([{ "when": { "all": ["status.other"] }, "look": { "overlay": { "animation": "rime" } }, "grow": 0.5 }]))
        .unwrap();
    for (value, needle) in [
        (
            serde_json::json!([{ "when": { "all": ["status.unheard"] }, "look": {} }]),
            "0.when.all.0",
        ),
        (
            serde_json::json!([{ "when": {}, "look": { "overlay": { "animation": "rim" } } }]),
            "0.look.overlay.animation",
        ),
        (
            serde_json::json!([{ "when": {}, "look": { "light": { "color": [1, 1, 1, 1], "radius": 8 } } }]),
            "look.light",
        ),
        (
            serde_json::json!([{ "when": {}, "look": {}, "grow": -1 }]),
            "grow",
        ),
    ] {
        let error = check(value).unwrap_err();
        assert!(error.contains(needle), "{error}");
    }
    assert!(
        std::panic::catch_unwind(|| {
            TagLook::new(TagFilter::new(), Look::new().with_sound("crackle", 1.0))
        })
        .is_err()
    );
}

#[test]
fn a_tag_look_grows_over_its_time() {
    let (mut world, mut schedule) = rig();
    let looks = Looks::new().with(
        TagLook::new(
            TagFilter::new().all(["status.other"]),
            Look::new().with_glow([1.0, 1.0, 1.0, 1.0], 4.0),
        )
        .with_grow(0.5),
    );
    let slime = tagged_slime(&mut world, Vec2::ZERO, looks);
    set_tag(&mut world, slime, "status.other", true);
    let alpha = |world: &World| glows(world)[0].1[3];
    frame(&mut world, &mut schedule);
    assert_eq!(alpha(&world), 0, "from nothing");
    // 15 more frames at 1/60 s: 0.25 s of its 0.5.
    frames(&mut world, &mut schedule, 15);
    assert_eq!(alpha(&world), 128, "half grown");
    frames(&mut world, &mut schedule, 20);
    assert_eq!(alpha(&world), 255, "whole, and held");
    // Paused, the game clock stands and the growth with it.
    set_tag(&mut world, slime, "status.other", false);
    frame(&mut world, &mut schedule);
    set_tag(&mut world, slime, "status.other", true);
    frame(&mut world, &mut schedule);
    assert_eq!(alpha(&world), 0, "its time starts again");
    world.resource_mut::<Time>().pause();
    frames(&mut world, &mut schedule, 10);
    assert_eq!(alpha(&world), 0, "held by the pause");
}

/// The rig with the facing and the character animator, which the late
/// look pass runs after.
fn rig_with_facing() -> (World, Schedule) {
    let (mut world, _) = rig();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["status.other", "status.more"])
        .build(&mut schedule, &mut world);
    StatsPlugin::new().build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    crate::FacingPlugin.build(&mut schedule, &mut world);
    crate::CharacterAnimatorPlugin.build(&mut schedule, &mut world);
    EffectsPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// The overlays playing `clip` on `holder`'s parts, not on the holder: each
/// one's point and flip.
fn marks_of(world: &World, holder: Entity, clip: &str) -> Vec<(Vec2, bool)> {
    world
        .query::<(Entity, &AnimationState, &Transform, &SpriteFlip)>()
        .filter(|(entity, state, ..)| *entity != holder && state.animation_id == clip)
        .map(|(_, _, transform, flip)| (transform.position, flip.0))
        .collect()
}

/// Whether `found` is one mark at `point`, flipped as `flipped`.
fn one_at(found: &[(Vec2, bool)], point: Vec2, flipped: bool) -> bool {
    matches!(found, [(at, flip)] if close(*at, point) && *flip == flipped)
}

#[test]
fn a_tag_look_mirrors_by_facing() {
    let (mut world, mut schedule) = rig_with_facing();
    let mark = || {
        Looks::new().with(TagLook::new(
            TagFilter::new(),
            Look::new().with_overlay(Overlay::new("rime").with_offset(Vec2::new(6.0, 0.0))),
        ))
    };
    // A status overlay with the same offset, which follows the holder as it
    // is drawn.
    let marked = Effect::new().with_look(
        Look::new().with_overlay(Overlay::new("ember").with_offset(Vec2::new(6.0, 0.0))),
    );
    insert(&mut world, "marked", marked);
    let face = |world: &mut World, holder: Entity, direction: Vec2| {
        world
            .get_mut::<crate::Facing>(holder)
            .unwrap()
            .face(direction);
    };

    // A facing that neither turns nor flips mirrors the mark by its side.
    let ghoul = tagged_slime(&mut world, Vec2::new(100.0, 0.0), mark());
    world.insert(ghoul, crate::Facing::new());
    face(&mut world, ghoul, Vec2::NEG_X);
    frame(&mut world, &mut schedule);
    // Centre (108, 8) less 6, less the 8 px rime's half: its left, mirrored.
    let found = marks_of(&world, ghoul, "rime");
    assert!(one_at(&found, Vec2::new(98.0, 4.0), true), "{found:?}");
    face(&mut world, ghoul, Vec2::new(0.5, -1.0));
    frame(&mut world, &mut schedule);
    let found = marks_of(&world, ghoul, "rime");
    assert!(
        one_at(&found, Vec2::new(110.0, 4.0), false),
        "right of the vertical, as written: {found:?}"
    );
    face(&mut world, ghoul, Vec2::NEG_X);
    frame(&mut world, &mut schedule);
    face(&mut world, ghoul, Vec2::Y);
    frame(&mut world, &mut schedule);
    let found = marks_of(&world, ghoul, "rime");
    assert!(
        one_at(&found, Vec2::new(98.0, 4.0), true),
        "straight down, its last side kept: {found:?}"
    );
    world.despawn(ghoul);

    // A turning facing pointed left turns the holder by π: the mark turns
    // with it to its left, unmirrored, where the status overlay lands.
    let turner = tagged_slime(&mut world, Vec2::new(100.0, 0.0), mark());
    world.insert(turner, crate::Facing::new().turning(0.0));
    face(&mut world, turner, Vec2::NEG_X);
    apply(&mut world, turner, "marked", 1.0);
    frame(&mut world, &mut schedule);
    let rotation = world.get::<Transform>(turner).unwrap().rotation;
    assert!((rotation - std::f32::consts::PI).abs() < 1e-6, "{rotation}");
    let (tag, status) = (
        marks_of(&world, turner, "rime"),
        marks_of(&world, turner, "ember"),
    );
    assert!(one_at(&tag, Vec2::new(98.0, 4.0), false), "{tag:?}");
    assert!(one_at(&status, tag[0].0, false), "{status:?}");
    world.despawn(turner);

    // A mirroring facing pointed left flips the holder upright: the mark
    // mirrors with it.
    let mirror = tagged_slime(&mut world, Vec2::new(100.0, 0.0), mark());
    world.insert(mirror, crate::Facing::new().turning(0.0).mirroring());
    face(&mut world, mirror, Vec2::NEG_X);
    apply(&mut world, mirror, "marked", 1.0);
    frame(&mut world, &mut schedule);
    let (tag, status) = (
        marks_of(&world, mirror, "rime"),
        marks_of(&world, mirror, "ember"),
    );
    assert!(one_at(&tag, Vec2::new(98.0, 4.0), true), "{tag:?}");
    assert!(one_at(&status, tag[0].0, true), "{status:?}");
    world.despawn(mirror);

    // A passive facing over an animator drawing one set mirrored: the drawn
    // flip first, kept by the animator while it faces straight down.
    let walker = tagged_slime(&mut world, Vec2::new(100.0, 0.0), mark());
    world.insert(walker, crate::Facing::new());
    world.insert(
        walker,
        crate::CharacterAnimator::new(crate::Clips::mirrored("ember")),
    );
    face(&mut world, walker, Vec2::NEG_X);
    apply(&mut world, walker, "marked", 1.0);
    frame(&mut world, &mut schedule);
    // The animator shows its 8 px ember, flipped: centre (104, 4) less 6.
    assert!(world.get::<SpriteFlip>(walker).is_some_and(|flip| flip.0));
    for heading in ["left", "straight down"] {
        let (tag, status) = (
            marks_of(&world, walker, "rime"),
            marks_of(&world, walker, "ember"),
        );
        assert!(
            one_at(&tag, Vec2::new(94.0, 0.0), true),
            "{heading}: {tag:?}"
        );
        assert!(one_at(&status, tag[0].0, true), "{heading}: {status:?}");
        face(&mut world, walker, Vec2::Y);
        frame(&mut world, &mut schedule);
    }
    world.despawn(walker);

    // Without a facing, its drawn flip mirrors it.
    let flipped_one = tagged_slime(&mut world, Vec2::new(100.0, 0.0), mark());
    world.insert(flipped_one, SpriteFlip(true));
    frames(&mut world, &mut schedule, 2);
    let found = marks_of(&world, flipped_one, "rime");
    assert!(one_at(&found, Vec2::new(98.0, 4.0), true), "{found:?}");
}

#[test]
fn an_overlay_scales_by_its_share() {
    let (mut world, mut schedule) = rig();
    // A status's: half its intensity, halfway from 0.5 to 1.5.
    let swell = Effect::new()
        .with_intensity(4.0, 0.0)
        .with_look(Look::new().with_overlay(Overlay::new("rime").with_scale(0.5, 1.5)));
    insert(&mut world, "swell", swell);
    let target = slime(&mut world, Vec2::new(100.0, 50.0));
    apply(&mut world, target, "swell", 2.0);
    frame(&mut world, &mut schedule);
    let (_, transform, _, _) = overlays(&world)[0].clone();
    assert_eq!(transform.scale, Vec2::ONE);
    // Its 8 px frame scaled 1 about the centre (108, 58).
    assert!(
        close(transform.position, Vec2::new(104.0, 54.0)),
        "{transform:?}"
    );
    assert!(end_effect(&mut world, target, &DataRef::by_id("swell")));
    frame(&mut world, &mut schedule);
    // A tag look's: from 0.25 to 1 over its 0.5 s.
    let ring = Looks::new().with(
        TagLook::new(
            TagFilter::new(),
            Look::new().with_overlay(Overlay::new("rime").with_scale(0.25, 1.0)),
        )
        .with_grow(0.5),
    );
    tagged_slime(&mut world, Vec2::new(300.0, 50.0), ring);
    let scale = |world: &World| overlays(world)[0].1.scale.x;
    frame(&mut world, &mut schedule);
    assert!((scale(&world) - 0.25).abs() < 1e-5, "{}", scale(&world));
    frames(&mut world, &mut schedule, 15);
    assert!((scale(&world) - 0.625).abs() < 1e-4, "{}", scale(&world));
    frames(&mut world, &mut schedule, 20);
    assert!((scale(&world) - 1.0).abs() < 1e-5, "{}", scale(&world));
    // An anchored frame: its drawn centre on the holder's centre (508, 58)
    // plus its offset as it grows from 0.5 to 1.5, and mirrored by a flip.
    let pinned = Looks::new().with(
        TagLook::new(
            TagFilter::new(),
            Look::new().with_overlay(
                Overlay::new("pin")
                    .with_offset(Vec2::new(4.0, -2.0))
                    .with_scale(0.5, 1.5),
            ),
        )
        .with_grow(0.5),
    );
    let holder = tagged_slime(&mut world, Vec2::new(500.0, 50.0), pinned);
    let pin = |world: &World| {
        let (transform, flip) = world
            .query::<(&AnimationState, &Transform, &SpriteFlip)>()
            .find(|(state, ..)| state.animation_id == "pin")
            .map(|(_, transform, flip)| (*transform, flip.0))
            .unwrap();
        let assets = world.resource::<AssetRegistry>();
        let asset = assets.sprite(assets.sprite_id("pin").unwrap()).unwrap();
        (
            sprite_centre(asset, &transform, flip),
            transform.scale.x,
            flip,
        )
    };
    for (count, grown) in [(1, 0.5), (15, 1.0), (20, 1.5)] {
        frames(&mut world, &mut schedule, count);
        let (centre, scale, _) = pin(&world);
        assert!(close(centre, Vec2::new(512.0, 56.0)), "{grown}: {centre}");
        assert!((scale - grown).abs() < 1e-4, "{grown}: {scale}");
    }
    world.insert(holder, SpriteFlip(true));
    frame(&mut world, &mut schedule);
    let (centre, _, flipped) = pin(&world);
    assert!(close(centre, Vec2::new(504.0, 56.0)) && flipped, "{centre}");
    // The data form: a default scale writes nothing; a negative is refused.
    let plain = serde_json::to_value(Look::new().with_overlay(Overlay::new("rime"))).unwrap();
    assert!(plain["overlay"].get("scale").is_none(), "its bytes kept");
    let grown = Look::new().with_overlay(Overlay::new("rime").with_scale(0.25, 1.0));
    let data = serde_json::to_value(&grown).unwrap();
    assert_eq!(data["overlay"]["scale"], serde_json::json!([0.25, 1.0]));
    assert_eq!(serde_json::from_value::<Look>(data).unwrap(), grown);
    let refused = serde_json::json!({ "overlay": { "animation": "rime", "scale": [-1.0, 1.0] } });
    assert!(serde_json::from_value::<Look>(refused).is_err());
}

#[test]
fn a_tag_look_goes_with_its_holder() {
    let (mut world, mut schedule) = rig();
    let mark = || {
        Looks::new().with(TagLook::new(
            TagFilter::new(),
            Look::new()
                .with_overlay(Overlay::new("rime"))
                .with_glow([1.0, 0.5, 0.0, 1.0], 6.0),
        ))
    };
    let first = tagged_slime(&mut world, Vec2::ZERO, mark());
    let second = tagged_slime(&mut world, Vec2::new(100.0, 0.0), mark());
    frame(&mut world, &mut schedule);
    let parts = |world: &World| (overlays(world).len(), glows(world).len());
    assert_eq!(parts(&world), (2, 2));
    // Its looks taken while another holder lives: its parts go.
    world.remove_component::<Looks>(first);
    frame(&mut world, &mut schedule);
    assert_eq!(parts(&world), (1, 1), "the other's kept");
    // The last holder despawned: no holder lives, so the pass reads no
    // query and lets every kept part go.
    world.despawn(second);
    assert!(!world.has_any::<Looks>());
    frame(&mut world, &mut schedule);
    assert_eq!(parts(&world), (0, 0));
    assert_eq!(world.query::<&Sprite>().count(), 1, "the first slime alone");
    // Its looks back: shown again.
    world.insert(first, mark());
    frame(&mut world, &mut schedule);
    assert_eq!(parts(&world), (1, 1));
}

#[test]
fn tag_and_status_tints_multiply() {
    let (mut world, mut schedule) = rig();
    let chill = Effect::new().with_look(Look::new().with_tint([0.5, 0.5, 1.0, 1.0]));
    insert(&mut world, "chill", chill);
    let looks = Looks::new().with(TagLook::new(
        TagFilter::new().all(["status.other"]),
        Look::new().with_tint([1.0, 0.5, 0.5, 1.0]),
    ));
    let slime = tagged_slime(&mut world, Vec2::ZERO, looks);
    apply(&mut world, slime, "chill", 1.0);
    set_tag(&mut world, slime, "status.other", true);
    frame(&mut world, &mut schedule);
    assert_eq!(
        color(&world, slime),
        [128, 64, 128, 255],
        "both, multiplied"
    );
    frames(&mut world, &mut schedule, 3);
    assert_eq!(
        color(&world, slime),
        [128, 64, 128, 255],
        "once a frame, over its own colour"
    );
    assert!(end_effect(&mut world, slime, &DataRef::by_id("chill")));
    frame(&mut world, &mut schedule);
    assert_eq!(
        color(&world, slime),
        [255, 128, 128, 255],
        "the tag's alone"
    );
    apply(&mut world, slime, "chill", 1.0);
    set_tag(&mut world, slime, "status.other", false);
    frame(&mut world, &mut schedule);
    assert_eq!(
        color(&world, slime),
        [128, 128, 255, 255],
        "the status's alone"
    );
    assert!(end_effect(&mut world, slime, &DataRef::by_id("chill")));
    frame(&mut world, &mut schedule);
    assert_eq!(color(&world, slime), WHITE, "its own colour back");
}

/// The parts showing sprite `id` that are not `holder`: each one's
/// transform, flip, depth and whether it plays a clip.
fn shown_sprites(world: &World, holder: Entity, id: &str) -> Vec<(Transform, bool, i32, bool)> {
    let sprite = world.resource::<AssetRegistry>().sprite_id(id).unwrap();
    world
        .query::<(Entity, &Transform, &Sprite, &SpriteFlip)>()
        .filter(|(entity, _, shown, _)| *entity != holder && shown.asset_id == sprite)
        .map(|(entity, transform, shown, flip)| {
            (
                *transform,
                flip.0,
                shown.z_order,
                world.has::<AnimationState>(entity),
            )
        })
        .collect()
}

#[test]
fn a_look_overlay_shows_a_plain_sprite() {
    use tungsten_core::ComponentRegistry;
    let (mut world, mut schedule) = rig();
    // A status's: the 8 px ember, still, at the drawn centre (108, 58)
    // plus (4, -2), one above in z.
    let marked = Effect::new().with_look(
        Look::new().with_overlay(Overlay::sprite("ember").with_offset(Vec2::new(4.0, -2.0))),
    );
    insert(&mut world, "marked", marked);
    let holder = slime(&mut world, Vec2::new(100.0, 50.0));
    world.get_mut::<Sprite>(holder).unwrap().z_order = 5;
    apply(&mut world, holder, "marked", 1.0);
    frame(&mut world, &mut schedule);
    let shown = shown_sprites(&world, holder, "ember");
    assert!(
        matches!(shown.as_slice(), [(transform, false, 6, false)] if close(transform.position, Vec2::new(108.0, 52.0))),
        "{shown:?}"
    );
    assert!(overlays(&world).is_empty(), "no clip plays");
    frames(&mut world, &mut schedule, 3);
    assert_eq!(shown_sprites(&world, holder, "ember").len(), 1, "kept");
    // Mirrored with its holder.
    world.insert(holder, SpriteFlip(true));
    frame(&mut world, &mut schedule);
    let shown = shown_sprites(&world, holder, "ember");
    assert!(
        matches!(shown.as_slice(), [(transform, true, ..)] if close(transform.position, Vec2::new(100.0, 52.0))),
        "{shown:?}"
    );
    assert!(end_effect(&mut world, holder, &DataRef::by_id("marked")));
    frame(&mut world, &mut schedule);
    assert!(
        shown_sprites(&world, holder, "ember").is_empty(),
        "gone with it"
    );
    // A tag look's too.
    let looks = Looks::new().with(TagLook::new(
        TagFilter::new().all(["status.other"]),
        Look::new().with_overlay(Overlay::sprite("ember")),
    ));
    let tagged = tagged_slime(&mut world, Vec2::new(300.0, 50.0), looks);
    set_tag(&mut world, tagged, "status.other", true);
    frame(&mut world, &mut schedule);
    let shown = shown_sprites(&world, tagged, "ember");
    assert!(
        matches!(shown.as_slice(), [(transform, ..)] if close(transform.position, Vec2::new(304.0, 54.0))),
        "{shown:?}"
    );

    // The data form: exactly one of `animation` and `sprite`.
    let data = serde_json::json!({ "overlay": { "sprite": "ember", "offset": [4, -2] } });
    let look: Look = serde_json::from_value(data).unwrap();
    assert_eq!(
        look,
        Look::new().with_overlay(Overlay::sprite("ember").with_offset(Vec2::new(4.0, -2.0)))
    );
    let written = serde_json::to_value(&look).unwrap();
    assert_eq!(written["overlay"]["sprite"], "ember");
    assert!(written["overlay"].get("animation").is_none());
    assert_eq!(serde_json::from_value::<Look>(written).unwrap(), look);
    for refused in [
        serde_json::json!({ "overlay": { "animation": "rime", "sprite": "ember" } }),
        serde_json::json!({ "overlay": { "offset": [1, 1] } }),
    ] {
        let error = serde_json::from_value::<Look>(refused.clone()).unwrap_err();
        assert!(
            error.to_string().contains("exactly one"),
            "{refused}: {error}"
        );
    }
    // An unknown sprite is refused at the load, naming its field.
    let error = Effect::new()
        .with_look(Look::new().with_overlay(Overlay::sprite("nope")))
        .check(&world)
        .unwrap_err();
    assert_eq!(error.field, "look.overlay.sprite");
    assert!(
        Effect::new()
            .with_look(Look::new().with_overlay(Overlay::sprite("ember")))
            .check(&world)
            .is_ok()
    );
    LooksPlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    let check = |value: serde_json::Value| {
        components
            .check_in("looks", &value, &world)
            .map_err(|error| error.to_string())
    };
    check(serde_json::json!([{ "when": {}, "look": { "overlay": { "sprite": "ember" } } }]))
        .unwrap();
    let error =
        check(serde_json::json!([{ "when": {}, "look": { "overlay": { "sprite": "embr" } } }]))
            .unwrap_err();
    assert!(error.contains("0.look.overlay.sprite"), "{error}");
}

#[test]
fn an_overlay_bobs_and_keeps_its_frame_unflipped() {
    let (mut world, mut schedule) = rig_with_facing();
    let glyph = || {
        Looks::new().with(TagLook::new(
            TagFilter::new(),
            Look::new().with_overlay(
                Overlay::sprite("ember")
                    .with_offset(Vec2::new(6.0, -12.0))
                    .with_bob(2.0, 0.5)
                    .unflipped(),
            ),
        ))
    };
    // Facing left under a facing that neither turns nor flips: the offset
    // mirrored, the frame not. Centre (108, 108) plus (-6, -12), less the
    // 8 px ember's half.
    let ghoul = tagged_slime(&mut world, Vec2::new(100.0, 100.0), glyph());
    world.insert(ghoul, crate::Facing::new());
    world
        .get_mut::<crate::Facing>(ghoul)
        .unwrap()
        .face(Vec2::NEG_X);
    let at = |world: &World| {
        let shown = shown_sprites(world, ghoul, "ember");
        assert!(
            matches!(shown.as_slice(), [(_, false, ..)]),
            "one, unflipped: {shown:?}"
        );
        shown[0].0.position
    };
    frame(&mut world, &mut schedule);
    assert!(close(at(&world), Vec2::new(98.0, 92.0)), "{}", at(&world));
    // 15 frames on, a quarter second from its show, half its 0.5 s: risen
    // its whole 2 px; at 0.5 s back down.
    frames(&mut world, &mut schedule, 15);
    assert!(
        at(&world).distance(Vec2::new(98.0, 90.0)) < 1e-3,
        "{}",
        at(&world)
    );
    frames(&mut world, &mut schedule, 15);
    assert!(
        at(&world).distance(Vec2::new(98.0, 92.0)) < 1e-3,
        "{}",
        at(&world)
    );
    // Paused, the game clock stands and the bob with it.
    frames(&mut world, &mut schedule, 15);
    world.resource_mut::<Time>().pause();
    frames(&mut world, &mut schedule, 7);
    assert!(
        at(&world).distance(Vec2::new(98.0, 90.0)) < 1e-3,
        "{}",
        at(&world)
    );
    world.resource_mut::<Time>().resume();
    world.despawn(ghoul);

    // A drawn flip mirrors the offset of an anchored clip kept unflipped:
    // its unflipped frame's drawn centre lands on (104, 6) + (-4, -2).
    let pinned = tagged_slime(
        &mut world,
        Vec2::new(96.0, -2.0),
        Looks::new().with(TagLook::new(
            TagFilter::new(),
            Look::new().with_overlay(
                Overlay::new("pin")
                    .with_offset(Vec2::new(4.0, -2.0))
                    .unflipped(),
            ),
        )),
    );
    world.insert(pinned, SpriteFlip(true));
    frames(&mut world, &mut schedule, 2);
    let (transform, flipped) = world
        .query::<(&AnimationState, &Transform, &SpriteFlip)>()
        .find(|(state, ..)| state.animation_id == "pin")
        .map(|(_, transform, flip)| (*transform, flip.0))
        .unwrap();
    let assets = world.resource::<AssetRegistry>();
    let asset = assets.sprite(assets.sprite_id("pin").unwrap()).unwrap();
    let centre = sprite_centre(asset, &transform, false);
    assert!(!flipped && close(centre, Vec2::new(100.0, 4.0)), "{centre}");
    world.despawn(pinned);

    // A status's bob counts from its apply.
    let pulse =
        Effect::new().with_look(Look::new().with_overlay(Overlay::new("rime").with_bob(4.0, 1.0)));
    insert(&mut world, "pulse", pulse);
    let target = slime(&mut world, Vec2::new(300.0, 50.0));
    apply(&mut world, target, "pulse", 1.0);
    frame(&mut world, &mut schedule);
    let y = |world: &World| overlays(world)[0].1.position.y;
    assert!((y(&world) - 54.0).abs() < 1e-4, "at rest: {}", y(&world));
    frames(&mut world, &mut schedule, 30);
    assert!(
        (y(&world) - 50.0).abs() < 1e-3,
        "4 px up at 0.5 s: {}",
        y(&world)
    );

    // The data form: a bob and a flip written, the defaults read back.
    let data = serde_json::json!({
        "overlay": { "sprite": "ember", "bob": { "height": 2, "seconds": 0.5 }, "flip": false }
    });
    let look: Look = serde_json::from_value(data).unwrap();
    assert_eq!(
        look,
        Look::new().with_overlay(Overlay::sprite("ember").with_bob(2.0, 0.5).unflipped())
    );
    assert_eq!(
        serde_json::from_value::<Look>(serde_json::to_value(&look).unwrap()).unwrap(),
        look
    );
    let plain = serde_json::to_value(Look::new().with_overlay(Overlay::new("rime"))).unwrap();
    assert!(plain["overlay"].get("bob").is_none());
    assert_eq!(plain["overlay"]["flip"], true);
    for refused in [
        serde_json::json!({ "overlay": { "sprite": "ember", "bob": { "height": 2, "seconds": 0 } } }),
        serde_json::json!({ "overlay": { "sprite": "ember", "bob": { "height": -1, "seconds": 1 } } }),
        serde_json::json!({ "overlay": { "sprite": "ember", "bob": { "height": 2 } } }),
    ] {
        assert!(
            serde_json::from_value::<Look>(refused.clone()).is_err(),
            "{refused}"
        );
    }
    assert!(std::panic::catch_unwind(|| Overlay::sprite("ember").with_bob(1.0, 0.0)).is_err());
}

/// Every emitter's point, by x.
fn emitters(world: &World) -> Vec<Vec2> {
    let mut found: Vec<Vec2> = world
        .query::<(&tungsten_core::ParticleEmitter, &Transform)>()
        .map(|(_, transform)| transform.position)
        .collect();
    found.sort_by(|a, b| a.x.total_cmp(&b.x));
    found
}

#[test]
fn look_particles_take_a_mirrored_offset() {
    let (mut world, mut schedule) = rig_with_parts();
    insert(
        &mut world,
        "fused",
        Effect::new().with_look(
            Look::new()
                .with_particles_at("sparks", "fuse")
                .with_particles_offset(Vec2::new(3.0, -1.0)),
        ),
    );
    // The fuse at (112, 52), plus (3, -1).
    let keg = slime(&mut world, Vec2::new(100.0, 50.0));
    apply(&mut world, keg, "fused", 1.0);
    frame(&mut world, &mut schedule);
    assert!(
        close(emitters(&world)[0], Vec2::new(115.0, 51.0)),
        "{:?}",
        emitters(&world)
    );
    // Mirrored with the fuse: (104, 52) less 3.
    world.insert(keg, SpriteFlip(true));
    frame(&mut world, &mut schedule);
    assert!(
        close(emitters(&world)[0], Vec2::new(101.0, 51.0)),
        "{:?}",
        emitters(&world)
    );
    // Turned with it: the fuse at (114, 62), the offset turned to (1, 3).
    world.insert(keg, SpriteFlip(false));
    world.get_mut::<Transform>(keg).unwrap().rotation = std::f32::consts::FRAC_PI_2;
    frame(&mut world, &mut schedule);
    assert!(
        close(emitters(&world)[0], Vec2::new(115.0, 65.0)),
        "{:?}",
        emitters(&world)
    );
    assert!(end_effect(&mut world, keg, &DataRef::by_id("fused")));
    frame(&mut world, &mut schedule);
    assert!(emitters(&world).is_empty());

    // A tag look's, from the drawn centre (308, 58), mirrored by the side
    // of a facing that neither turns nor flips its holder, its last side
    // kept while it points straight down.
    let looks = Looks::new().with(TagLook::new(
        TagFilter::new(),
        Look::new()
            .with_particles("sparks")
            .with_particles_offset(Vec2::new(5.0, 2.0)),
    ));
    let walker = tagged_slime(&mut world, Vec2::new(300.0, 50.0), looks);
    world.insert(walker, crate::Facing::new());
    let face = |world: &mut World, direction: Vec2| {
        world
            .get_mut::<crate::Facing>(walker)
            .unwrap()
            .face(direction);
    };
    for (direction, at) in [
        (Vec2::X, Vec2::new(313.0, 60.0)),
        (Vec2::NEG_X, Vec2::new(303.0, 60.0)),
        (Vec2::Y, Vec2::new(303.0, 60.0)),
    ] {
        face(&mut world, direction);
        frames(&mut world, &mut schedule, 2);
        assert!(
            close(emitters(&world)[0], at),
            "{direction}: {:?}",
            emitters(&world)
        );
    }

    // The data form, and a zero offset written as such.
    let data = serde_json::json!({ "particles": { "config": "sparks", "offset": [3, -1] } });
    let look: Look = serde_json::from_value(data).unwrap();
    assert_eq!(
        look,
        Look::new()
            .with_particles("sparks")
            .with_particles_offset(Vec2::new(3.0, -1.0))
    );
    assert_eq!(
        serde_json::from_value::<Look>(serde_json::to_value(&look).unwrap()).unwrap(),
        look
    );
    assert!(
        std::panic::catch_unwind(|| Look::new().with_particles_offset(Vec2::ONE)).is_err(),
        "no particles to offset"
    );
}
