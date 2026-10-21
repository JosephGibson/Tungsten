//! Ice: the ice spray, `cast` held on slot 3, frosts iron by the step
//! through its cone, stops at bodies, tiles and its reach, follows its
//! caster and its aim, and starts and stops between two steps; chilled and
//! frozen iron and its scraps draw tinted, their frost over the metal and
//! turning with a scrap; an escaped scrap goes; frozen iron shatters into
//! sixteen scraps at a blast, whose heat reaches by surface distance;
//! frozen iron slides further and a scrap's friction is a sixteenth; and
//! the cold pulses draw unlit, bounded, and leave.

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::elements::components::{Chill, Frozen, IcePulse};
use example_01_platformer::features::elements::draw::effects;
use example_01_platformer::features::elements::{self, FREEZE_TIME, freeze_amount};
use example_01_platformer::features::level::WORLD_MAX;
use example_01_platformer::features::level::components::SceneTime;
use example_01_platformer::features::toys::components::{IronBlock, IronScrap, Spray};
use example_01_platformer::features::toys::systems::{bounds, friction, iron_look};
use example_01_platformer::features::toys::{
    self, BLAST_RADIUS, FROZEN_FRICTION, IRON_FRICTION, IRON_HALF, IRON_MASS, MISSILE_SPEED,
    SCRAP_COUNT, SCRAP_HALF, SPRAY_RANGE,
};
use example_01_platformer::shared::BURST_CAP;
use glam::Vec2;
use tungsten::core::assets::{LayerKind, TilemapData};
use tungsten::core::{
    AssetRegistry, AudioCommand, AudioCommands, CameraState, Collider, Entity, Particle,
    ParticleBudget, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Position,
    RigidBody, Shape, SoundRegistry, TilemapInstance, TilemapRegistry, Time, Transform, Velocity,
};
use tungsten::extract_sprites_default;
use tungsten::kit::{Children, Stats, damage};
use tungsten::testing::UiHarness;

use super::{
    aim_at, body, centre, choose, contact, floor, ground, kick, mock_sprite, pin_camera, run,
    stand, steps_this_frame, teleport,
};

fn block(harness: &mut UiHarness, at: Vec2) -> Entity {
    toys::place_iron(harness.world_mut(), at)
}

fn scraps(harness: &UiHarness) -> Vec<Entity> {
    let world = harness.world();
    world
        .query::<(Entity, &IronScrap)>()
        .map(|(e, _)| e)
        .collect()
}

/// The quads a frame's default extract draws, unlit or lit, the view on
/// the test's iron.
fn drawn(harness: &mut UiHarness, lit: bool) -> Vec<tungsten::render::SpriteInstance> {
    let view = Vec2::new(900.0, 400.0) - Vec2::splat(500.0);
    harness.world_mut().resource_mut::<CameraState>().position = view;
    extract_sprites_default(harness.world())
        .into_iter()
        .filter(|b| b.lit == lit && (1..=16).contains(&b.texture.0))
        .flat_map(|b| b.instances)
        .collect()
}

#[test]
fn frozen_iron_and_scraps_render_and_escaped_scraps_are_cleaned_up() {
    let (mut harness, _) = run();
    let centre = Vec2::new(900.0, 400.0);
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    let quarters = ["0_0", "0_1", "1_0", "1_1"];
    for (i, q) in quarters.iter().enumerate() {
        mock_sprite(
            assets,
            &format!("ex10_iron_brick_big_{q}"),
            i as u32 + 1,
            true,
        );
        mock_sprite(
            assets,
            &format!("ex10_iron_frost_big_{q}"),
            i as u32 + 5,
            false,
        );
    }
    for i in 0..4 {
        mock_sprite(assets, &format!("ex10_iron_scrap_{i}"), i + 9, true);
        mock_sprite(assets, &format!("ex10_iron_scrap_frost_{i}"), i + 13, false);
    }
    let iron = block(&mut harness, centre);
    // Its art pieces land with the step's spawns.
    harness.step(1);
    assert_eq!(harness.world().get::<Children>(iron).unwrap().len(), 8);
    harness.world_mut().insert(
        iron,
        Chill {
            amount: 0.5,
            held: true,
        },
    );
    iron_look(harness.world_mut());
    assert!(
        drawn(&mut harness, true)
            .iter()
            .all(|i| i.color == [177, 225, 255, 255])
    );
    assert!(drawn(&mut harness, false).iter().all(|i| i.color[3] == 127));
    elements::freeze_iron(harness.world_mut(), iron);
    assert!(!harness.world().has::<Chill>(iron));
    iron_look(harness.world_mut());
    assert_eq!(
        drawn(&mut harness, true).len() + drawn(&mut harness, false).len(),
        8
    );
    assert!(
        drawn(&mut harness, true)
            .iter()
            .all(|i| i.color == [100, 195, 255, 255])
    );
    assert_eq!(
        drawn(&mut harness, false).len(),
        4,
        "the coating shows without the scene's light"
    );
    elements::thermal_shatter(harness.world_mut(), centre, BLAST_RADIUS);
    // The scraps' frost lands with the next step's spawns.
    harness.step(1);
    iron_look(harness.world_mut());
    let metal = drawn(&mut harness, true);
    assert_eq!(metal.len(), SCRAP_COUNT);
    assert!(metal.iter().all(|i| i.size == [32.0, 32.0]));
    assert!(
        drawn(&mut harness, false).is_empty(),
        "warm scraps show no frost"
    );
    let turns = |harness: &mut UiHarness, time: f32| {
        harness.world_mut().insert_resource(SceneTime(time));
        iron_look(harness.world_mut());
        let world = harness.world();
        world
            .query::<(&IronScrap, &tungsten::core::Transform)>()
            .map(|(_, t)| t.rotation)
            .collect::<Vec<f32>>()
    };
    let early = turns(&mut harness, 0.0);
    let tumbling = turns(&mut harness, 0.25);
    assert!(
        early
            .iter()
            .zip(&tumbling)
            .all(|(a, b)| (a - b).abs() > 0.5)
    );
    assert_eq!(
        turns(&mut harness, 0.25),
        tumbling,
        "the same instant, the same turn"
    );
    let refrozen = scraps(&harness)[0];
    elements::freeze_iron(harness.world_mut(), refrozen);
    iron_look(harness.world_mut());
    let coating = drawn(&mut harness, false);
    assert_eq!(coating.len(), 1);
    let metal = drawn(&mut harness, true);
    let under = metal
        .iter()
        .find(|i| i.position == coating[0].position)
        .unwrap();
    assert_eq!(
        under.rotation, coating[0].rotation,
        "frost turns with its metal"
    );
    let escaped = scraps(&harness)[1];
    harness.world_mut().get_mut::<Position>(escaped).unwrap().0 = WORLD_MAX + Vec2::splat(100.0);
    bounds(harness.world_mut());
    assert!(!harness.world().is_alive(escaped));
    assert_eq!(scraps(&harness).len(), SCRAP_COUNT - 1);
}

#[test]
fn thermal_contact_uses_surface_distance_and_leaves_warm_and_distant_iron_intact() {
    let (mut harness, _) = run();
    let o = Vec2::new(1200.0, 400.0);
    let edge = block(&mut harness, o + Vec2::new(IRON_HALF.x + BLAST_RADIUS, 0.0));
    let distant = block(
        &mut harness,
        o - Vec2::new(IRON_HALF.x + BLAST_RADIUS + 1.0, 0.0),
    );
    let warm = block(&mut harness, o);
    elements::freeze_iron(harness.world_mut(), edge);
    elements::freeze_iron(harness.world_mut(), distant);
    toys::explode(harness.world_mut(), o);
    assert!(!harness.world().is_alive(edge));
    assert!(harness.world().is_alive(distant));
    assert!(harness.world().is_alive(warm));
    assert_eq!(scraps(&harness).len(), SCRAP_COUNT);
    toys::explode(harness.world_mut(), o);
    assert_eq!(
        scraps(&harness).len(),
        SCRAP_COUNT,
        "warm scraps do not split again"
    );
}

#[test]
fn frozen_iron_slides_with_reduced_friction_and_scraps_can_refreeze() {
    let (mut harness, _) = run();
    let o = Vec2::new(900.0, 400.0);
    let warm = block(&mut harness, o);
    let cold = block(&mut harness, o + Vec2::new(400.0, 0.0));
    elements::freeze_iron(harness.world_mut(), cold);
    for e in [warm, cold] {
        kick(&mut harness, e, Vec2::new(-100.0, 0.0));
        contact(&mut harness, e, None, -Vec2::Y);
    }
    let dt = harness.world().resource::<Time>().delta();
    friction(harness.world_mut());
    let vx = |harness: &UiHarness, e| harness.world().get::<Velocity>(e).unwrap().0.x;
    assert!((vx(&harness, warm) + 100.0 - IRON_FRICTION * dt).abs() < 0.001);
    assert!((vx(&harness, cold) + 100.0 - FROZEN_FRICTION * dt).abs() < 0.001);
    toys::explode(harness.world_mut(), o + Vec2::new(400.0, 0.0));
    let scrap = scraps(&harness)[0];
    elements::freeze_iron(harness.world_mut(), scrap);
    assert_eq!(freeze_amount(harness.world(), scrap), 1.0);
    let at = {
        let world = harness.world();
        world.get::<Position>(scrap).unwrap().0 + Vec2::splat(16.0)
    };
    toys::explode(harness.world_mut(), at);
    assert!(!harness.world().is_alive(scrap));
    assert_eq!(scraps(&harness).len(), SCRAP_COUNT - 1);
}

#[test]
fn scrap_friction_is_one_sixteenth_of_full_iron_with_the_same_frozen_reduction() {
    let (mut harness, _) = run();
    let o = Vec2::new(900.0, 400.0);
    let iron = block(&mut harness, o);
    elements::freeze_iron(harness.world_mut(), iron);
    elements::thermal_shatter(harness.world_mut(), o, BLAST_RADIUS);
    let pieces = scraps(&harness);
    let (warm, cold) = (pieces[0], pieces[1]);
    elements::freeze_iron(harness.world_mut(), cold);
    for (index, scrap) in [warm, cold].into_iter().enumerate() {
        kick(&mut harness, scrap, Vec2::new(100.0, 0.0));
        // A support contact may name the scrap on either side.
        if index == 0 {
            contact(&mut harness, scrap, None, -Vec2::Y);
        } else {
            contact(&mut harness, warm, Some(scrap), Vec2::Y);
        }
    }
    let dt = harness.world().resource::<Time>().delta();
    friction(harness.world_mut());
    let vx = |harness: &UiHarness, e| harness.world().get::<Velocity>(e).unwrap().0.x;
    assert!((vx(&harness, warm) - (100.0 - IRON_FRICTION / 16.0 * dt)).abs() < 0.001);
    assert!((vx(&harness, cold) - (100.0 - FROZEN_FRICTION / 16.0 * dt)).abs() < 0.001);
}

#[test]
fn cold_pulses_draw_without_scene_lighting_stay_bounded_and_expire() {
    let (mut harness, _) = run();
    let at = Vec2::new(900.0, 400.0);
    harness.world_mut().resource_mut::<CameraState>().position = at - Vec2::splat(300.0);
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    mock_sprite(assets, "ex10_ice_ring", 71, false);
    mock_sprite(assets, "ex10_flame_glow", 72, false);
    for _ in 0..64 {
        elements::ice_pulse(harness.world_mut(), at, 128.0, true);
    }
    assert_eq!(harness.world().query::<&IcePulse>().count(), BURST_CAP);
    let cues = effects(harness.world());
    assert!(cues.iter().all(|batch| !batch.lit));
    assert_eq!(cues.iter().map(|b| b.instances.len()).sum::<usize>(), 32);
    harness.step(1);
    let shatter = harness
        .world()
        .resource::<SoundRegistry>()
        .get_by_id("ex10_ice_shatter_sfx");
    let plays = harness
        .harness()
        .audio()
        .iter()
        .filter(|c| matches!(c, AudioCommand::Play { handle, .. } if Some(*handle) == shatter))
        .count();
    assert_eq!(plays, 1, "a group shares one crack");
    harness.step(30);
    assert_eq!(harness.world().query::<&IcePulse>().count(), 0);
}

/// The top of the spray tests' ground, in the level's empty sky.
const TOP: f32 = 1000.0;
/// The cold spray's three particle emitters' most alive, together.
const SPRAY_PARTICLES: usize = 144 + 48 + 100;

/// A run with the player on the sky's ground at `x`, the camera held over
/// it and slot 3, the ice spray, selected.
fn sprayer(x: f32) -> (UiHarness, Entity) {
    let (mut harness, player) = run();
    ground(&mut harness, TOP);
    stand(&mut harness, player, x, TOP);
    harness.step(1);
    pin_camera(&mut harness, Vec2::new(x - 600.0, TOP - 500.0));
    choose(&mut harness, 2);
    (harness, player)
}

/// Iron set down on the sky's ground, its centre at `x`.
fn iron(harness: &mut UiHarness, x: f32) -> Entity {
    block(harness, Vec2::new(x, TOP - IRON_HALF.y))
}

/// Aims at `at` and holds `cast`.
fn spray_at(harness: &mut UiHarness, at: Vec2) {
    aim_at(harness, at);
    harness.harness_mut().press_action("cast");
}

fn release(harness: &mut UiHarness) {
    harness.harness_mut().release_action("cast");
}

/// The player's spray, while one runs.
fn spray(harness: &UiHarness) -> Option<Spray> {
    harness.world().query::<&Spray>().next().copied()
}

fn sprays(harness: &UiHarness) -> usize {
    harness.world().query::<&Spray>().count()
}

fn frozen(harness: &UiHarness, iron: Entity) -> bool {
    harness.world().has::<Frozen>(iron)
}

/// The particles of `emitters`.
fn particles(harness: &UiHarness, emitters: &[Option<Entity>; 3]) -> usize {
    harness
        .world()
        .query::<&Particle>()
        .filter(|p| p.emitter.is_some_and(|e| emitters.contains(&Some(e))))
        .count()
}

/// The live ones of `emitters`.
fn live(harness: &UiHarness, emitters: &[Option<Entity>; 3]) -> usize {
    let world = harness.world();
    emitters
        .iter()
        .flatten()
        .filter(|e| world.is_alive(**e))
        .count()
}

/// The last frame's plays of the manifest sound `id`, and of it looping.
fn plays(harness: &UiHarness, id: &str) -> usize {
    played(harness, id, false)
}

fn loops(harness: &UiHarness, id: &str) -> usize {
    played(harness, id, true)
}

fn played(harness: &UiHarness, id: &str, looped: bool) -> usize {
    let sound = harness.world().resource::<SoundRegistry>().get_by_id(id);
    harness
        .harness()
        .audio()
        .iter()
        .filter(|command| {
            matches!(command, AudioCommand::Play { handle, looping, .. }
                if Some(*handle) == sound && *looping == looped)
        })
        .count()
}

/// The last frame's stops of `spray`'s loop.
fn stops(harness: &UiHarness, spray: Spray) -> usize {
    harness
        .harness()
        .audio()
        .iter()
        .filter(|command| {
            matches!(command, AudioCommand::StopVoice { voice, .. } if Some(*voice) == spray.voice)
        })
        .count()
}

const START: &str = "ex10_ice_beam_sfx";
const LOOP: &str = "ex10_ice_spray_sfx";
const FREEZE: &str = "ex10_ice_freeze_sfx";
const END: &str = "ex10_ice_end_sfx";
const SHATTER: &str = "ex10_ice_shatter_sfx";

#[test]
fn holding_f_gradually_freezes_iron_with_particles_and_a_single_audio_loop() {
    // `cast` held on slot 3 for F.
    let (mut harness, player) = sprayer(300.0);
    let near = iron(&mut harness, 600.0);
    let behind = iron(&mut harness, 850.0);
    let eye = centre(&harness, player);
    spray_at(&mut harness, Vec2::new(850.0, eye.y));
    harness.step(1);
    assert!(!frozen(&harness, near));
    assert!(freeze_amount(harness.world(), near) > 0.0);
    assert_eq!(freeze_amount(harness.world(), behind), 0.0);
    let beam = spray(&harness).unwrap();
    assert!(
        (beam.end.x - (600.0 - IRON_HALF.x - 4.0)).abs() < 0.001,
        "the beam ends on the near face: {}",
        beam.end.x
    );
    assert_eq!(sprays(&harness), 1);
    assert_eq!((plays(&harness, START), loops(&harness, LOOP)), (1, 1));
    for _ in 0..44 {
        harness.step(1);
        assert_eq!(plays(&harness, START) + loops(&harness, LOOP), 0);
    }
    assert!((freeze_amount(harness.world(), near) - 0.5).abs() < 0.001);
    assert!(!frozen(&harness, near));
    elements::thermal_shatter(
        harness.world_mut(),
        Vec2::new(600.0, TOP - IRON_HALF.y),
        BLAST_RADIUS,
    );
    assert!(
        harness.world().is_alive(near),
        "partial frost is not brittle yet"
    );
    assert!(particles(&harness, &beam.emitters) > 50);
    harness.step(44);
    assert!(!frozen(&harness, near));
    harness.step(1);
    assert!(frozen(&harness, near));
    assert!(!frozen(&harness, behind));
    assert_eq!(plays(&harness, FREEZE), 1);
    for _ in 0..60 {
        harness.step(1);
        let again = plays(&harness, START) + loops(&harness, LOOP) + plays(&harness, FREEZE);
        assert_eq!(again, 0, "no repeated start or freeze sounds");
    }
    assert_eq!(sprays(&harness), 1);
    release(&mut harness);
    let (mut stopped, mut ended) = (0, 0);
    for _ in 0..2 {
        harness.step(1);
        stopped += stops(&harness, beam);
        ended += plays(&harness, END);
    }
    assert_eq!(
        (stopped, ended),
        (1, 1),
        "the loop stops and the end sounds"
    );
    harness.step(120);
    assert_eq!(sprays(&harness), 0);
    assert_eq!(live(&harness, &beam.emitters), 0);
    assert_eq!(particles(&harness, &beam.emitters), 0);
    harness.step(1);
    assert_eq!(sprays(&harness), 0, "a released channel stays stopped");
    spray_at(&mut harness, Vec2::new(850.0, eye.y));
    harness.step(1);
    assert_eq!(sprays(&harness), 1);
}

#[test]
fn beam_stops_on_colliders_and_tiles_and_has_a_finite_range() {
    for blocker in 0..3 {
        let (mut harness, player) = sprayer(300.0);
        let target = iron(&mut harness, 600.0);
        let eye = centre(&harness, player);
        match blocker {
            0 => {
                floor(
                    &mut harness,
                    Vec2::new(450.0, TOP - 80.0),
                    Vec2::new(2.0, 80.0),
                );
            }
            1 => {
                // One solid tile between them.
                let mut wall = TilemapData::new(64, 64, 1, 1);
                let tile = wall.add_tile("ex10_ground");
                let layer = wall.add_layer("collision", LayerKind::Collision);
                wall.set_tile(layer, 0, 0, tile);
                let world = harness.world_mut();
                world
                    .resource_mut::<TilemapRegistry>()
                    .insert("wall".into(), wall);
                world.spawn_with((TilemapInstance::new("wall", Vec2::new(448.0, TOP - 64.0)),));
            }
            _ => {
                let x = eye.x + SPRAY_RANGE + IRON_HALF.x + 10.0;
                let world = harness.world_mut();
                world.get_mut::<Position>(target).unwrap().0.x = x - IRON_HALF.x;
            }
        }
        spray_at(&mut harness, Vec2::new(1600.0, eye.y));
        harness.step((FREEZE_TIME * 60.0) as u32);
        assert!(!frozen(&harness, target), "blocker {blocker}");
        assert_eq!(
            freeze_amount(harness.world(), target),
            0.0,
            "blocker {blocker}"
        );
    }
    // A round body blocks the ray too, a diagonal shot's.
    let (mut harness, player) = sprayer(300.0);
    let eye = centre(&harness, player);
    let low = TOP + 300.0;
    floor(
        &mut harness,
        Vec2::new(600.0, low + 50.0),
        Vec2::new(200.0, 50.0),
    );
    let brick = block(&mut harness, Vec2::new(eye.x + 300.0, low - IRON_HALF.y));
    let ball = floor(&mut harness, eye + Vec2::splat(150.0), Vec2::ZERO);
    harness.world_mut().insert(ball, Collider::circle(80.0));
    spray_at(&mut harness, eye + Vec2::splat(300.0));
    harness.step((FREEZE_TIME * 60.0) as u32);
    assert!(!frozen(&harness, brick));
    assert_eq!(freeze_amount(harness.world(), brick), 0.0);
    assert!(!frozen(&harness, ball));
}

#[test]
fn fireball_contact_shatters_frozen_iron_into_sixteen_physical_scraps() {
    let (mut harness, player) = sprayer(300.0);
    let at = Vec2::new(600.0, TOP - IRON_HALF.y);
    let brick = block(&mut harness, at);
    let eye = centre(&harness, player);
    spray_at(&mut harness, Vec2::new(600.0, eye.y));
    harness.step((FREEZE_TIME * 60.0) as u32);
    assert!(frozen(&harness, brick));
    release(&mut harness);
    harness.step(1);
    // Where it rests, settled into the ground.
    let rested = body(&harness, brick);
    let missile = toys::fire_missile(
        harness.world_mut(),
        Vec2::new(450.0, eye.y),
        Vec2::X * MISSILE_SPEED,
    )
    .unwrap();
    for _ in 0..10 {
        harness.step(1);
        if !harness.world().is_alive(missile) {
            break;
        }
    }
    let world = harness.world();
    assert!(!world.is_alive(missile));
    assert!(!world.is_alive(brick));
    assert_eq!(world.query::<&IronBlock>().count(), 0);
    let pieces: Vec<(Entity, IronScrap, Vec2, f32)> = world
        .query::<(Entity, &IronScrap, &Position, &RigidBody)>()
        .map(|(e, scrap, p, body)| (e, *scrap, p.0, 1.0 / body.inv_mass))
        .collect();
    assert_eq!(pieces.len(), SCRAP_COUNT);
    let mass: f32 = pieces.iter().map(|piece| piece.3).sum();
    assert!((mass - IRON_MASS).abs() < 0.001);
    for &(e, scrap, p, mass) in &pieces {
        assert!((mass - IRON_MASS / 16.0).abs() < 0.001);
        assert!(scrap.variant < 4);
        let collider = world.get::<Collider>(e).unwrap();
        assert_eq!(
            collider.shape,
            Shape::Aabb {
                half_extents: SCRAP_HALF
            }
        );
        let v = world.get::<Velocity>(e).unwrap().0;
        assert!(v.is_finite() && v.length() > 100.0);
        let piece = p + collider.offset;
        assert!(
            (piece - rested)
                .abs()
                .cmple(IRON_HALF - SCRAP_HALF + 0.01)
                .all()
        );
        assert!(!world.has::<Frozen>(e));
    }
}

#[test]
fn repeated_channels_and_long_holds_keep_particles_bounded_and_stop_on_death() {
    let (mut harness, player) = sprayer(300.0);
    // Mana for ten seconds of spraying past the channels.
    let stats = harness.world_mut().get_mut::<Stats>(player).unwrap();
    stats.set_base("max_mana", 1000.0);
    stats.set_base("mana", 1000.0);
    let eye = centre(&harness, player);
    let far = Vec2::new(1500.0, eye.y);
    let mut last = None;
    for _ in 0..20 {
        spray_at(&mut harness, far);
        harness.step(12);
        let channel = spray(&harness).unwrap();
        assert_eq!(live(&harness, &channel.emitters), 3);
        release(&mut harness);
        harness.step(1);
        assert_eq!(sprays(&harness), 0);
        assert_eq!(live(&harness, &channel.emitters), 0);
        let cap = harness.world().resource::<ParticleBudget>().global_cap as usize;
        assert!(harness.world().query::<&Particle>().count() <= cap);
        last = Some(channel);
    }
    harness.step(120);
    assert_eq!(
        particles(&harness, &last.unwrap().emitters),
        0,
        "they drain"
    );
    spray_at(&mut harness, far);
    harness.step(600);
    let long = spray(&harness).unwrap();
    assert_eq!(sprays(&harness), 1);
    assert_eq!(live(&harness, &long.emitters), 3);
    assert!(particles(&harness, &long.emitters) <= SPRAY_PARTICLES);
    assert_eq!(
        harness.world().resource::<ParticleBudget>().global_cap,
        2048
    );
    assert!(
        harness
            .world()
            .query::<&example_01_platformer::shared::Burst>()
            .count()
            <= BURST_CAP
    );
    damage(harness.world_mut(), player, 1000.0);
    harness.step(1);
    assert_eq!(sprays(&harness), 0);
    assert_eq!(live(&harness, &long.emitters), 0);
}

#[test]
fn spray_aim_tracks_the_caster_and_partial_frost_thaws_when_aim_moves_away() {
    let (mut harness, player) = sprayer(600.0);
    let right = iron(&mut harness, 850.0);
    let left = iron(&mut harness, 350.0);
    let eye = centre(&harness, player);
    spray_at(&mut harness, Vec2::new(1000.0, eye.y));
    harness.step(45);
    assert!((freeze_amount(harness.world(), right) - 0.5).abs() < 0.001);
    assert_eq!(freeze_amount(harness.world(), left), 0.0);
    // The aim swings back as the courier steps aside: the spray follows.
    aim_at(&mut harness, Vec2::new(200.0, eye.y));
    teleport(&mut harness, player, eye + Vec2::X * 10.0);
    harness.step(1);
    let beam = spray(&harness).unwrap();
    assert!(beam.end.x < beam.start.x - 150.0);
    assert!(
        (beam.start.x - (eye.x + 10.0 - 24.0)).abs() < 0.1,
        "the muzzle leaves the moved courier: {}",
        beam.start.x
    );
    harness.step(89);
    assert!(frozen(&harness, left));
    assert!(!frozen(&harness, right));
    assert!((freeze_amount(harness.world(), right) - 0.275).abs() < 0.001);
    release(&mut harness);
    harness.step(120);
    assert!(!harness.world().has::<Chill>(right));
    assert!(frozen(&harness, left), "completed freezing persists");
}

#[test]
fn release_death_and_owner_removal_stop_the_loop_even_without_a_fixed_step() {
    for stop in 0..3 {
        let (mut harness, player) = sprayer(300.0);
        let eye = centre(&harness, player);
        spray_at(&mut harness, Vec2::new(600.0, eye.y));
        harness.step(1);
        let sprayed = spray(&harness).unwrap();
        harness.world_mut().resource_mut::<Time>().pause();
        harness.harness_mut().set_dt(1.0 / 144.0);
        match stop {
            0 => release(&mut harness),
            1 => {
                damage(harness.world_mut(), player, 1000.0);
            }
            _ => harness.world_mut().despawn(player),
        }
        harness.step(1);
        assert_eq!(steps_this_frame(&harness), 0);
        assert_eq!(sprays(&harness), 0, "stop {stop}");
        assert_eq!(live(&harness, &sprayed.emitters), 0);
        assert_eq!(stops(&harness, sprayed), 1, "stop {stop}: the loop stops");
        let ended = harness
            .harness()
            .events::<Sounded>()
            .filter(|sounded| sounded.sound == Sound::ColdEnded)
            .count();
        assert_eq!(
            ended,
            usize::from(stop == 0),
            "only a release sounds its end; death and the owner's loss stop quietly"
        );
    }
}

#[test]
fn freezing_uses_fixed_exposure_at_30_60_and_144_hz_and_stops_when_paused() {
    for fps in [30, 60, 144] {
        let (mut harness, player) = sprayer(300.0);
        let target = iron(&mut harness, 550.0);
        let eye = centre(&harness, player);
        spray_at(&mut harness, Vec2::new(550.0, eye.y));
        harness.harness_mut().set_dt(1.0 / fps as f32);
        harness.step(fps);
        let amount = freeze_amount(harness.world(), target);
        assert!(
            (amount - 1.0 / FREEZE_TIME).abs() < 0.012,
            "{fps} Hz: {amount}"
        );
        assert!(!frozen(&harness, target));
        harness.world_mut().resource_mut::<Time>().pause();
        harness.step(fps);
        assert_eq!(freeze_amount(harness.world(), target), amount);
        harness.world_mut().resource_mut::<Time>().resume();
        harness.step(fps / 2 + 1);
        assert!(frozen(&harness, target), "{fps} Hz did not complete");
    }
}

#[test]
fn cone_frosts_iron_beside_the_centerline_and_spray_particles_follow_aim() {
    let (mut harness, player) = sprayer(300.0);
    let side = iron(&mut harness, 580.0);
    let eye = centre(&harness, player);
    // The aim passes over the block; the cone's lower edge meets it.
    let up = 24f32.to_radians();
    spray_at(&mut harness, eye + Vec2::new(up.cos(), -up.sin()) * 400.0);
    harness.step(90);
    assert!(frozen(&harness, side), "the side of the cone frosts iron");
    let beam = spray(&harness).unwrap();
    let world = harness.world();
    let configs = world.resource::<ParticleConfigRegistry>();
    let config = configs.id_for_name("ex10_ice_contact").unwrap();
    let contact = beam
        .emitters
        .into_iter()
        .flatten()
        .find(|e| {
            world
                .get::<ParticleEmitter>(*e)
                .is_some_and(|em| em.config == config)
        })
        .unwrap();
    assert!(!world.get::<ParticleEmitterState>(contact).unwrap().drained);
    let at = world.get::<Transform>(contact).unwrap().position;
    let line = eye.y - (at.x - eye.x) * up.tan();
    assert!(
        at.x < 580.0 && at.y > line,
        "the contact sits on the block's face, below the aim: {at}"
    );
    // Aimed up into empty sky, the old particles drain.
    spray_at(&mut harness, eye - Vec2::Y * 700.0);
    harness.step(120);
    let world = harness.world();
    let snow = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name("ex10_ice_beam")
        .unwrap();
    let velocities: Vec<Vec2> = world
        .query::<&Particle>()
        .filter(|p| {
            p.age > 0.0
                && p.emitter.is_some_and(|e| {
                    world
                        .get::<ParticleEmitter>(e)
                        .is_some_and(|emitter| emitter.config == snow)
                })
        })
        .map(|p| p.velocity)
        .collect();
    assert!(!velocities.is_empty());
    assert!(
        velocities
            .iter()
            .all(|v| v.y < -500.0 && v.x.abs() < -v.y * 0.26),
        "snow travels inside the upward cone"
    );
}

#[test]
fn high_refresh_press_starts_the_spray_before_the_next_physics_step() {
    let (mut harness, player) = sprayer(300.0);
    let target = iron(&mut harness, 550.0);
    let eye = centre(&harness, player);
    harness.harness_mut().set_dt(1.0 / 144.0);
    spray_at(&mut harness, Vec2::new(550.0, eye.y));
    harness.step(1);
    assert_eq!(steps_this_frame(&harness), 0);
    let beam = spray(&harness).unwrap();
    assert_eq!(sprays(&harness), 1);
    assert!(particles(&harness, &beam.emitters) > 0);
    assert_eq!(
        freeze_amount(harness.world(), target),
        0.0,
        "exposure still waits for simulation"
    );
    // Its loop at once; its start sound, a cue, with the next step.
    assert_eq!(loops(&harness, LOOP), 1);
    release(&mut harness);
    harness.step(1);
    assert_eq!(steps_this_frame(&harness), 0);
    assert_eq!(sprays(&harness), 0);
    assert_eq!(stops(&harness, beam), 1);
    assert_eq!(freeze_amount(harness.world(), target), 0.0);
}

#[test]
fn simultaneous_freezes_and_thermal_fractures_share_audio_but_keep_individual_cues() {
    let (mut harness, player) = sprayer(300.0);
    let lower = iron(&mut harness, 550.0);
    let upper = block(&mut harness, Vec2::new(550.0, TOP - 3.0 * IRON_HALF.y));
    let eye = centre(&harness, player);
    let up = 20f32.to_radians();
    spray_at(&mut harness, eye + Vec2::new(up.cos(), -up.sin()) * 400.0);
    let mut freezes = 0;
    for _ in 0..90 {
        harness.step(1);
        freezes += plays(&harness, FREEZE);
    }
    assert!(frozen(&harness, upper));
    assert!(frozen(&harness, lower));
    assert_eq!(freezes, 1, "the freezes share one sound");
    let pulses = |harness: &UiHarness, shatter: bool| {
        harness
            .world()
            .query::<&IcePulse>()
            .filter(|p| p.shatter == shatter)
            .count()
    };
    assert_eq!(pulses(&harness, false), 2);
    elements::thermal_shatter(
        harness.world_mut(),
        Vec2::new(550.0, TOP - 2.0 * IRON_HALF.y),
        BLAST_RADIUS,
    );
    assert_eq!(harness.world().query::<&IronScrap>().count(), 32);
    assert_eq!(pulses(&harness, true), 2);
    harness.world_mut().resource_mut::<AudioCommands>().drain();
    harness.step(1);
    assert_eq!(plays(&harness, SHATTER), 1, "the fractures share one crack");
    release(&mut harness);
    harness.step(120);
    assert_eq!(harness.world().query::<&IcePulse>().count(), 0);
}
