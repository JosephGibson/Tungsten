//! The spikes and fireballs: a touch's damage, window, knockback, flash,
//! shake and sound, three touches ending a run at Normal, and the fires'
//! trails, facing and drips.

use example_01_platformer::features::level::components::Hazard;
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AudioCommand, Bundle, CameraController, Collider, Entity, Particle, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Position, PrevPosition, RigidBodyBundle, SoundRegistry,
    Transform,
};
use tungsten::kit::{Aim, Children, Facing, Health, HitFlash, damage};
use tungsten::testing::UiHarness;

use example_01_platformer::features::elements::FIRE_CENTRE;
use example_01_platformer::features::elements::systems::fire_contacts;
use example_01_platformer::features::toys::RING_CAP;
use example_01_platformer::features::toys::components::{Ball, Ring, ShockRing};
use example_01_platformer::shared::BURST_CAP;

use super::{ball, bursts, centre, run, settle, teleport, top};

const DT: f32 = 1.0 / 60.0;
/// Where a spike's touch box sits from its sprite's top-left, and a drip's
/// emitter from its fireball's.
const SPIKES_CENTRE: Vec2 = Vec2::new(32.0, 48.0);
const DRIP_OFFSET: Vec2 = Vec2::new(32.0, 44.0);
const TRAIL_OFFSET: Vec2 = Vec2::new(32.0, 32.0);

fn health(harness: &UiHarness, player: Entity) -> f32 {
    harness.world().get::<Health>(player).unwrap().current()
}

/// The level's hazards, fires or spikes, in spawn order.
fn hazards(harness: &UiHarness, fire: bool) -> Vec<Entity> {
    harness
        .world()
        .query::<(Entity, &Hazard)>()
        .filter(|(_, hazard)| hazard.fire == fire)
        .map(|(entity, _)| entity)
        .collect()
}

/// Plays of the manifest sound `id` in the last frame.
fn plays(harness: &UiHarness, id: &str) -> usize {
    let handle = harness.world().resource::<SoundRegistry>().get_by_id(id);
    harness
        .harness()
        .audio()
        .iter()
        .filter(
            |command| matches!(command, AudioCommand::Play { handle: h, .. } if Some(*h) == handle),
        )
        .count()
}

#[test]
fn audio_controls_and_damage_flash_shake_remain_wired() {
    let (mut harness, player) = run();
    harness.step(10);
    // A body that is no hazard against the player: no hit.
    let at = centre(&harness, player) + Vec2::new(26.0, 0.0);
    harness.world_mut().spawn_with(
        RigidBodyBundle::dynamic(Position(at), Collider::circle(8.0))
            .with((SceneEntity { state_id: GAMEPLAY },)),
    );
    harness.step(5);
    let world = harness.world();
    assert_eq!(health(&harness, player), 100.0);
    assert!(!world.get::<HitFlash>(player).unwrap().is_active());
    assert_eq!(world.resource::<CameraController>().shake_trauma, 0.0);
    // A hit flashes, shakes and plays its sound.
    damage(harness.world_mut(), player, 34.0);
    harness.step(1);
    let world = harness.world();
    assert!(world.get::<HitFlash>(player).unwrap().is_active());
    assert!(world.resource::<CameraController>().shake_trauma > 0.0);
    assert_eq!(plays(&harness, "ex10_player_hit_sfx"), 1);
}

#[test]
fn hazards_damage_once_per_cooldown_and_the_last_heart_starts_the_death_screen() {
    let (mut harness, player) = run();
    harness.step(10);
    let spike = hazards(&harness, false)[0];
    let touch = harness.world().get::<Transform>(spike).unwrap().position + SPIKES_CENTRE;
    // Standing on the teeth, a little above their centre.
    let onto = |harness: &mut UiHarness| teleport(harness, player, touch - Vec2::new(0.0, 20.0));
    onto(&mut harness);
    harness.step(1);
    assert_eq!(health(&harness, player), 66.0);
    harness.step(1);
    assert!(
        centre(&harness, player).y < touch.y - 20.0,
        "the touch knocks the player up and away"
    );
    for _ in 0..10 {
        onto(&mut harness);
        harness.step(1);
    }
    assert_eq!(health(&harness, player), 66.0, "once per window");
    // Away until the window runs out, then two more touches, each a new
    // contact, which hits at once.
    let away = touch - Vec2::new(4.0 * 64.0, 0.0);
    for left in [32.0, 0.0] {
        teleport(&mut harness, player, away);
        harness.step(70);
        onto(&mut harness);
        harness.step(1);
        assert_eq!(health(&harness, player).max(0.0), left);
    }
    harness.step(2);
    assert!(harness.world().get::<Health>(player).unwrap().is_dead());
    settle(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
}

/// Each fire's child emitter of the particle config `name`, in fire order:
/// a fire's trail at its centre and its drips underneath.
fn fire_children(harness: &UiHarness, fires: &[Entity], name: &str) -> Vec<Entity> {
    let world = harness.world();
    let config = world.resource::<ParticleConfigRegistry>().id_for_name(name);
    fires
        .iter()
        .map(|fire| {
            let children = world.get::<Children>(*fire).expect("its emitters");
            let found: Vec<Entity> = children
                .iter()
                .filter(|child| {
                    world
                        .get::<ParticleEmitter>(*child)
                        .is_some_and(|emitter| Some(emitter.config) == config)
                })
                .collect();
            assert_eq!(found.len(), 1, "one {name} on each fire");
            found[0]
        })
        .collect()
}

#[test]
fn moving_fire_emits_a_bounded_particle_trail() {
    let (mut harness, _) = run();
    let fires = hazards(&harness, true);
    assert_eq!(fires.len(), 6);
    harness.step(1);
    let trails = fire_children(&harness, &fires, "ex10_fire_trail");
    harness.step(120);
    let world = harness.world();
    let trail = world
        .query::<&Particle>()
        .filter(|particle| {
            particle
                .emitter
                .is_some_and(|emitter| trails.contains(&emitter))
        })
        .count();
    assert!(trail > 60 && trail <= 240, "{trail} trail particles");
    for (fire, emitter) in fires.iter().zip(&trails) {
        let state = world.get::<ParticleEmitterState>(*emitter).unwrap();
        assert!(state.active_count <= 40);
        let at = world.get::<Transform>(*emitter).unwrap().position;
        let centre = world.get::<Transform>(*fire).unwrap().position + TRAIL_OFFSET;
        assert!(
            (at - centre).length() < 1e-3,
            "the trail rides the fire's centre"
        );
    }
}

#[test]
fn fireballs_face_their_travel_and_drag_anchored_drips() {
    let (mut harness, _) = run();
    harness.step(1);
    let fires = hazards(&harness, true);
    let drips = fire_children(&harness, &fires, "ex10_fireball_drips");
    let mut seen = vec![[false; 2]; fires.len()];
    for _ in 0..450 {
        harness.step(1);
        let world = harness.world();
        for (i, fire) in fires.iter().enumerate() {
            let left = world.get::<Facing>(*fire).unwrap().direction().x < 0.0;
            if world.has::<Aim>(*fire) {
                // One that only rises and falls watches the player, who
                // waits to its left.
                assert!(left, "fire {i} looks away from the player");
                continue;
            }
            let moved = world.get::<Position>(*fire).unwrap().0
                - world.get::<PrevPosition>(*fire).unwrap().0;
            if moved.length() / DT >= 1.0 && moved.x != 0.0 {
                assert_eq!(left, moved.x < 0.0, "fire {i} faces against its travel");
            }
            seen[i][usize::from(left)] = true;
        }
        for (fire, drip) in fires.iter().zip(&drips) {
            let expected = world.get::<Transform>(*fire).unwrap().position + DRIP_OFFSET;
            let at = world.get::<Transform>(*drip).unwrap().position;
            assert!((at - expected).length() < 1e-3, "{at} vs {expected}");
        }
    }
    // Over a full period every horizontal mover turns both ways.
    for (i, fire) in fires.iter().enumerate() {
        if !harness.world().has::<Aim>(*fire) {
            assert_eq!(seen[i], [true, true], "fire {i}");
        }
    }
}

#[test]
fn a_hit_plays_its_sound_and_immunity_ignores_repeat_hits_until_it_runs_out() {
    let (mut harness, player) = run();
    harness.step(10);
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 66.0);
    harness.step(1);
    assert_eq!(plays(&harness, "ex10_player_hit_sfx"), 1);
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 66.0);
    // The window counts down by itself; a hit just inside it still misses.
    harness.step((1.1 / DT) as u32 - 3);
    assert!(
        harness
            .world()
            .get::<Health>(player)
            .unwrap()
            .is_invulnerable()
    );
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 66.0);
    harness.step(3);
    assert!(
        !harness
            .world()
            .get::<Health>(player)
            .unwrap()
            .is_invulnerable()
    );
    damage(harness.world_mut(), player, 34.0);
    assert_eq!(health(&harness, player), 32.0);
}

/// A bare fire hazard whose touch box sits about `centre` now and about
/// `before` as the step began.
fn crossing_fire(harness: &mut UiHarness, centre: Vec2, before: Vec2) -> Entity {
    let (at, prev) = (centre - FIRE_CENTRE, before - FIRE_CENTRE);
    harness.world_mut().spawn_with((
        Hazard { fire: true },
        Position(at),
        PrevPosition(prev),
        Transform::from_position(at),
        SceneEntity { state_id: GAMEPLAY },
    ))
}

#[test]
fn moving_fire_sweeps_fast_balls_once_and_explosions_do_not_hurt_players() {
    let (mut harness, player) = run();
    let o = Vec2::new(900.0, 500.0);
    teleport(&mut harness, player, o + Vec2::new(0.0, 100.0));
    for _ in 0..2 {
        crossing_fire(
            &mut harness,
            o - Vec2::new(0.0, 100.0),
            o + Vec2::new(0.0, 100.0),
        );
    }
    // The ball and both fires cross between steps.
    let heavy = ball(&mut harness, o, false);
    let world = harness.world_mut();
    let offset = world.get::<Collider>(heavy).unwrap().offset;
    world.get_mut::<Position>(heavy).unwrap().0 = o + Vec2::new(100.0, 0.0) - offset;
    world.get_mut::<PrevPosition>(heavy).unwrap().0 = o - Vec2::new(100.0, 0.0) - offset;
    fire_contacts(harness.world_mut());
    assert!(!harness.world().is_alive(heavy));
    assert_eq!(
        bursts(&harness, "ex10_ball_explosion"),
        1,
        "one burst for one ball"
    );
    assert_eq!(ball_rings(&harness), 1);
    // The burst beside the player hurts nothing.
    assert_eq!(health(&harness, player), 100.0);
    harness.step(150);
    assert_eq!(bursts(&harness, "ex10_ball_explosion"), 0);
    assert_eq!(ball_rings(&harness), 0);
}

fn ball_rings(harness: &UiHarness) -> usize {
    harness
        .world()
        .query::<&ShockRing>()
        .filter(|ring| ring.kind == Ring::Ball)
        .count()
}

#[test]
fn explosion_bursts_and_rings_stay_bounded_under_dense_ball_contact() {
    let (mut harness, _) = run();
    let o = Vec2::new(900.0, 500.0);
    crossing_fire(&mut harness, o, o);
    for _ in 0..100 {
        ball(&mut harness, o, false);
    }
    fire_contacts(harness.world_mut());
    assert_eq!(harness.world().query::<&Ball>().count(), 0);
    assert_eq!(ball_rings(&harness), RING_CAP);
    assert_eq!(bursts(&harness, "ex10_ball_explosion"), BURST_CAP);
}
