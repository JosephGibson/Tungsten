//! The hazards: lava's surface, which holds the player, its touch, which
//! burns and throws on every touch, hurting once a window, and a dying touch
//! starting the death screen; a hit's flash, shake and sound; and the fire
//! hazards the level's data may place, which burst fast balls they sweep.

use example_01_platformer::features::combat::BURNING;
use example_01_platformer::features::enemies::{self, components::EnemyKind};
use example_01_platformer::features::level::components::{Doused, Hazard};
use example_01_platformer::features::player::PLAYER_HALF;
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shell::game_over::GAME_OVER;
use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AudioCommand, Bundle, CameraController, Collider, Entity, Position, PrevPosition,
    RigidBodyBundle, SoundRegistry, Transform,
};
use tungsten::kit::{ContactDamage, DamageRequest, Damaged, Health, HitFlash, Tags, damage, heal};
use tungsten::testing::UiHarness;

use example_01_platformer::features::elements::FIRE_CENTRE;
use example_01_platformer::features::elements::systems::fire_contacts;
use example_01_platformer::features::toys::RING_CAP;
use example_01_platformer::features::toys::components::{Ball, Ring, ShockRing};
use example_01_platformer::shared::BURST_CAP;

use example_01_platformer::features::level::lava::Lava;
use tungsten::core::RigidBody;

use super::{
    aim_at, ball, body, bursts, calm, centre, choose, fall_dead, pin_camera, run, stand, teleport,
    top,
};

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
fn a_burn_tick_plays_no_hurt_and_a_killing_one_plays_the_death() {
    let (mut harness, player) = run();
    harness.step(10);
    let tick = |harness: &mut UiHarness, amount: f32| {
        DamageRequest::new(player, amount)
            .with_periodic(true)
            .queue(harness.world_mut());
        harness.step(1);
    };
    tick(&mut harness, 5.0);
    assert_eq!(health(&harness, player), 95.0);
    assert_eq!(
        plays(&harness, "ex10_player_hit_sfx"),
        0,
        "a tick plays no hurt"
    );
    let world = harness.world();
    assert_eq!(
        world.resource::<CameraController>().shake_trauma,
        0.0,
        "nor shakes"
    );
    tick(&mut harness, 200.0);
    assert!(harness.world().get::<Health>(player).unwrap().is_dead());
    assert_eq!(
        (
            plays(&harness, "ex10_player_hit_sfx"),
            plays(&harness, "ex10_player_death_sfx")
        ),
        (0, 1),
        "the killing tick plays the death in place of the hurt"
    );
    assert!(harness.world().resource::<CameraController>().shake_trauma > 0.0);
}

/// The skin of `basin`, the lava's top under which nothing that stands on
/// it sinks: its corner's row plus the skin's 6 px.
fn skin(harness: &UiHarness, basin: Entity) -> f32 {
    harness.world().get::<Transform>(basin).unwrap().position.y + 6.0
}

/// The non-periodic hits the player took in the last frame.
fn blows(harness: &UiHarness, player: Entity) -> Vec<f32> {
    harness
        .harness()
        .events::<Damaged>()
        .filter(|hit| hit.entity == player && !hit.periodic)
        .map(|hit| hit.amount)
        .collect()
}

#[test]
fn hazards_damage_once_per_cooldown_and_the_last_heart_starts_the_death_screen() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.step(10);
    let lake = lava(&harness)[0];
    let x = harness.world().get::<Transform>(lake).unwrap().position.x + 96.0;
    let surface = skin(&harness, lake);
    stand(&mut harness, player, x, surface);
    harness.step(1);
    assert_eq!(blows(&harness, player), [16.0], "lava's bite");
    for _ in 0..10 {
        stand(&mut harness, player, x, surface);
        harness.step(1);
        assert!(blows(&harness, player).is_empty(), "once per window");
    }
    // Away until the burn and the window run out, brought low, then a touch
    // that kills.
    let away = Vec2::new(x - 4.0 * 64.0, surface - 2.0 * 64.0);
    teleport(&mut harness, player, away);
    harness.step(300);
    heal(harness.world_mut(), player, 500.0);
    damage(harness.world_mut(), player, 90.0);
    harness.step(70);
    stand(&mut harness, player, x, surface);
    harness.step(2);
    assert!(harness.world().get::<Health>(player).unwrap().is_dead());
    fall_dead(&mut harness);
    assert_eq!(top(&harness), Some(GAME_OVER));
}

#[test]
fn lava_throws_the_player_up_and_out_and_never_lets_them_sink() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position.x
    };
    basins.sort_by(|a, b| corner(&harness, *a).total_cmp(&corner(&harness, *b)));
    // The lake's two pools, 4 columns each, and the shelf's pool, 3.
    assert_eq!(basins.len(), 3);
    for (basin, columns) in basins.into_iter().zip([4.0, 4.0, 3.0]) {
        let left = harness.world().get::<Transform>(basin).unwrap().position.x;
        let right = left + columns * 64.0;
        let top = skin(&harness, basin);
        for (x, bank) in [(left + 30.0, -1.0), (right - 30.0, 1.0)] {
            // On the near bank, out of the lava, until the window runs out.
            let shore = Vec2::new(left - 96.0, top - 3.0 * PLAYER_HALF.y);
            teleport(&mut harness, player, shore);
            heal(harness.world_mut(), player, 500.0);
            harness.step(70);
            stand(&mut harness, player, x, top);
            harness.step(1);
            assert_eq!(
                blows(&harness, player),
                [16.0],
                "{basin:?} at {x}: the bite"
            );
            let tags = harness.world().get::<Tags>(player).unwrap();
            assert!(tags.has(BURNING), "{basin:?} at {x}: the bite burns");
            let start = centre(&harness, player);
            harness.step(8);
            let at = centre(&harness, player);
            assert!(
                at.y < start.y - 30.0,
                "{basin:?} at {x}: thrown up, {start} to {at}"
            );
            assert!(
                (at.x - start.x) * bank > 0.0,
                "{basin:?} at {x}: toward its nearer bank"
            );
            // Inside the window: thrown again, unhurt.
            stand(&mut harness, player, x, top);
            harness.step(1);
            assert!(
                blows(&harness, player).is_empty(),
                "{basin:?} at {x}: the window"
            );
            harness.step(6);
            let again = centre(&harness, player).y + PLAYER_HALF.y;
            assert!(again < top - 30.0, "{basin:?} at {x}: thrown again");
            // Never below the skin, whatever it lands on, for two seconds.
            for frame in 0..120 {
                harness.step(1);
                let at = centre(&harness, player);
                let feet = at.y + PLAYER_HALF.y;
                let over = (left..right).contains(&at.x);
                assert!(
                    !over || feet <= top + 1.0,
                    "{basin:?} frame {frame}: sank to {feet} under {top}"
                );
            }
        }
    }
    // A walker falls in: lava's surface holds the player alone.
    let lake = lava(&harness)[1];
    let middle = harness.world().get::<Transform>(lake).unwrap().position + Vec2::new(128.0, -96.0);
    let crawler = enemies::summon(harness.world_mut(), EnemyKind::Crawler, middle).unwrap();
    harness.step(60);
    let low = enemies::centre(harness.world(), crawler).map_or(f32::MAX, |at| at.y);
    assert!(
        low > skin(&harness, lake),
        "the crawler sank below the skin: {low}"
    );
}

#[test]
fn lava_bites_with_fire_and_the_fires_douse_leaves_it_hot() {
    let (mut harness, player) = run();
    harness.step(10);
    let basins = lava(&harness);
    assert_eq!(basins.len(), 3, "the lake's two pools and the shelf's");
    let basin = basins[0];
    let corner = harness.world().get::<Transform>(basin).unwrap().position;
    // On the lake's skin, over its first lava column.
    let top = skin(&harness, basin);
    stand(&mut harness, player, corner.x + 32.0, top);
    harness.step(1);
    assert_eq!(health(&harness, player), 84.0, "16 fire");
    harness.step(1);
    assert!(
        centre(&harness, player).y < top - PLAYER_HALF.y,
        "the touch knocks the player up and away"
    );
    // The douse puts a fire out; lava stays hot (the spray crusts it).
    example_01_platformer::features::elements::douse_hazard(harness.world_mut(), basin);
    let world = harness.world();
    assert!(!world.has::<Doused>(basin) && world.has::<ContactDamage>(basin));
}

/// Whether `basin` has a crust set.
fn crusted(harness: &UiHarness, basin: Entity) -> bool {
    harness
        .world()
        .get::<Lava>(basin)
        .is_some_and(|lava| lava.crust.is_some())
}

#[test]
fn the_ice_spray_crusts_a_basin_that_holds_the_player_and_melts_to_throw_them() {
    let (mut harness, player) = run();
    calm(&mut harness);
    harness.step(10);
    let mut basins = lava(&harness);
    let corner = |harness: &UiHarness, basin: Entity| {
        harness.world().get::<Transform>(basin).unwrap().position
    };
    basins.sort_by(|a, b| corner(&harness, *a).x.total_cmp(&corner(&harness, *b).x));
    let pool = basins[0];
    let at = corner(&harness, pool);
    let surface = skin(&harness, pool);
    // On the bank beside the pool, the ice spray aimed at its surface.
    stand(&mut harness, player, at.x - 40.0, at.y - 64.0);
    harness.step(2);
    pin_camera(&mut harness, at - Vec2::new(500.0, 400.0));
    choose(&mut harness, 2);
    // The cursor held on the pool's surface each frame, as the camera
    // settles.
    let target = Vec2::new(at.x + 96.0, surface + 2.0);
    let spray = |harness: &mut UiHarness, steps: u32| {
        harness.harness_mut().press_action("cast");
        for _ in 0..steps {
            aim_at(harness, target);
            harness.step(1);
        }
        harness.harness_mut().release_action("cast");
        harness.step(1);
    };
    spray(&mut harness, 18);
    assert!(!crusted(&harness, pool), "0.3 s of spray sets no crust");
    harness.step(90);
    spray(&mut harness, 33);
    assert!(crusted(&harness, pool), "0.5 s sets it");
    let world = harness.world();
    assert!(!world.has::<ContactDamage>(pool), "its touch held aside");
    assert_eq!(
        world.get::<RigidBody>(pool).unwrap().layers,
        1,
        "solid as rock"
    );
    // The player stands on it untouched, and a ball rests on it.
    stand(&mut harness, player, at.x + 96.0, surface);
    let rock = ball(&mut harness, Vec2::new(at.x + 200.0, surface - 40.0), false);
    for _ in 0..60 {
        harness.step(1);
        assert!(
            blows(&harness, player).is_empty(),
            "the crust bites nothing"
        );
    }
    let feet = centre(&harness, player).y + PLAYER_HALF.y;
    assert!(
        (feet - surface).abs() < 2.0,
        "standing on the crust: {feet} on {surface}"
    );
    assert!(body(&harness, rock).y < surface, "the ball rests on it");
    // It melts within its six seconds, throwing the player and dropping the ball.
    let mut thrown = false;
    for _ in 0..400 {
        harness.step(1);
        thrown |= blows(&harness, player) == [16.0];
        if !crusted(&harness, pool) {
            for _ in 0..4 {
                harness.step(1);
                thrown |= blows(&harness, player) == [16.0];
            }
            break;
        }
    }
    assert!(!crusted(&harness, pool), "melted");
    assert!(thrown, "the melt bites the player on it");
    harness.step(10);
    assert!(
        centre(&harness, player).y + PLAYER_HALF.y < surface - 20.0,
        "thrown up"
    );
    harness.step(60);
    assert!(body(&harness, rock).y > surface + 32.0, "the ball fell in");
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
