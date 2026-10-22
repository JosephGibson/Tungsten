//! The hazards: the lava's touch, its damage, window and knockback, three
//! touches ending a run at Normal, never doused; a hit's flash, shake and
//! sound; and the fire hazards the level's data may place, which burst fast
//! balls they sweep.

use example_01_platformer::features::level::components::{Doused, Hazard};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AudioCommand, Bundle, CameraController, Collider, Entity, Position, PrevPosition,
    RigidBodyBundle, SoundRegistry, Transform,
};
use tungsten::kit::{ContactDamage, Health, HitFlash, damage};
use tungsten::testing::UiHarness;

use example_01_platformer::features::elements::FIRE_CENTRE;
use example_01_platformer::features::elements::systems::fire_contacts;
use example_01_platformer::features::toys::RING_CAP;
use example_01_platformer::features::toys::components::{Ball, Ring, ShockRing};
use example_01_platformer::shared::BURST_CAP;

use super::{ball, bursts, centre, run, settle, teleport, top};

const DT: f32 = 1.0 / 60.0;

fn health(harness: &UiHarness, player: Entity) -> f32 {
    harness.world().get::<Health>(player).unwrap().current()
}

/// The level's lava basins, in spawn order.
fn lava(harness: &UiHarness) -> Vec<Entity> {
    harness
        .world()
        .query::<(Entity, &Hazard)>()
        .filter(|(_, hazard)| hazard.lava)
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
    let lake = lava(&harness)[0];
    // The lake's skin over its first lava column.
    let touch = harness.world().get::<Transform>(lake).unwrap().position + Vec2::new(32.0, 28.0);
    // Standing in the lava, a little above the skin's middle.
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
    let away = touch - Vec2::new(4.0 * 64.0, 2.0 * 64.0);
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

#[test]
fn lava_touches_as_a_fire_does_and_cold_never_douses_it() {
    let (mut harness, player) = run();
    harness.step(10);
    let basins = lava(&harness);
    assert_eq!(basins.len(), 2, "the lake and the shelf's pool");
    let basin = basins[0];
    let corner = harness.world().get::<Transform>(basin).unwrap().position;
    // On the lake's skin, over its first lava column.
    let onto = corner + Vec2::new(32.0, 8.0);
    teleport(&mut harness, player, onto);
    harness.step(1);
    assert_eq!(health(&harness, player), 66.0, "a fire's damage");
    harness.step(1);
    assert!(
        centre(&harness, player).y < onto.y,
        "the touch knocks the player up and away"
    );
    // Cold puts a fire out; lava stays hot.
    example_01_platformer::features::elements::douse_hazard(harness.world_mut(), basin);
    let world = harness.world();
    assert!(!world.has::<Doused>(basin) && world.has::<ContactDamage>(basin));
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
        Hazard {
            fire: true,
            lava: false,
        },
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
