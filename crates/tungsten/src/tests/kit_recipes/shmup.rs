//! The shmup fixture: a bodiless ship under the move actions, bullets that
//! expire or leave the view, raiders from a wave on a path, and a walker
//! on a floor under the same actions.

use tungsten_core::RigidBody;
use tungsten_core::physics::{Position, Velocity};
use tungsten_core::{AudioCommand, CameraController};

use super::{DTS, body_at, drawn_at, frames_for, recipe, spawn, tagged};
use crate::kit::{
    Children, CooldownElapsed, Counters, Hit, KitPlugins, PlatformerController, Source, damage,
};

const GENRE: &str = "shmup";

#[test]
fn the_ship_moves_by_position_and_draws_by_transform() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ship = spawn(&mut harness, "ship", [640.0, 600.0]);
        assert!(!harness.world().has::<RigidBody>(ship), "bodiless");
        harness.press_action("move_left");
        harness.step(frames_for(0.5, dt));
        let at = body_at(&harness, ship);
        assert!(at.x < 640.0 - 100.0, "dt {dt}: 300 px/s: {at}");
        let drawn = drawn_at(&harness, ship);
        assert!(
            (drawn.x - at.x).abs() <= 300.0 * dt + 1e-3,
            "dt {dt}: drawn within a step of the body: {drawn} vs {at}"
        );
        assert!((drawn.y - 600.0).abs() < 1e-3);
    }
}

#[test]
fn bullets_expire_and_leave_the_view() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let flying = spawn(&mut harness, "bullet", [640.0, 300.0]);
        let stuck = spawn(&mut harness, "bullet", [640.0, 300.0]);
        harness.world_mut().get_mut::<Velocity>(stuck).unwrap().0 = glam::Vec2::ZERO;
        harness.step(frames_for(0.5, dt));
        assert!(
            !harness.world().is_alive(flying),
            "dt {dt}: 900 px/s up leaves the view's 32 px margin"
        );
        assert!(harness.world().is_alive(stuck), "dt {dt}: 0.5 of 1.5 s");
        harness.step(frames_for(1.2, dt));
        assert!(!harness.world().is_alive(stuck), "dt {dt}: expired");
    }
}

#[test]
fn the_waves_raiders_follow_their_path() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "wave", [300.0, 0.0]);
        harness.step(frames_for(0.8, dt));
        let raiders = tagged(&harness, "raider");
        assert_eq!(raiders.len(), 3, "dt {dt}: the wave's three");
        let before: Vec<f32> = raiders
            .iter()
            .map(|&raider| body_at(&harness, raider).y)
            .collect();
        harness.step(frames_for(0.5, dt));
        for (i, &raider) in raiders.iter().enumerate() {
            let at = body_at(&harness, raider);
            assert!(at.y > before[i] + 50.0, "dt {dt}: down its path: {at}");
            assert!((at.x - 300.0).abs() < 1e-3);
        }
        assert!(
            harness.world().get::<Position>(raiders[0]).unwrap().0.y
                > harness.world().get::<Position>(raiders[2]).unwrap().0.y,
            "dt {dt}: the first spawned is furthest along"
        );
    }
}

#[test]
fn the_walker_runs_on_its_floor() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // The floor's top at y 392; the walker's feet on it.
        spawn(&mut harness, "floor", [0.0, 400.0]);
        let walker = spawn(&mut harness, "walker", [0.0, 380.0]);
        harness.step(2);
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        let at = body_at(&harness, walker);
        assert!(at.x > 30.0, "dt {dt}: it ran: {at}");
        assert!((at.y - 380.0).abs() < 0.1, "dt {dt}: on the floor: {at}");
        assert!(
            harness
                .world()
                .get::<PlatformerController>(walker)
                .unwrap()
                .grounded()
        );
    }
}

#[test]
fn the_turret_fires_at_jittered_intervals_with_a_sound() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let turret = spawn(&mut harness, "turret", [200.0, 300.0]);
        let mut shots = Vec::new();
        for frame in 0..frames_for(2.0, dt) {
            harness.step(1);
            let fired = harness
                .events::<CooldownElapsed>()
                .filter(|event| event.entity == turret)
                .count();
            let played = harness
                .audio()
                .iter()
                .filter(|command| matches!(command, AudioCommand::Play { .. }))
                .count();
            assert_eq!(played, fired, "dt {dt}: a sound per shot");
            if fired > 0 {
                shots.push(frame);
                let bullets = tagged(&harness, "bullet");
                assert!(
                    bullets
                        .iter()
                        .any(|&bullet| harness.world().get::<Source>(bullet)
                            == Some(&Source::new(turret))),
                    "dt {dt}: the shot's bullet"
                );
            }
        }
        assert!(
            (4..=5).contains(&shots.len()),
            "dt {dt}: 0.4 to 0.5 s apart: {shots:?}"
        );
        let (low, high) = (frames_for(0.4, dt) - 1, frames_for(0.5, dt) + 1);
        for pair in shots.windows(2) {
            assert!(
                (low..=high).contains(&(pair[1] - pair[0])),
                "dt {dt}: {shots:?}"
            );
        }
    }
}

#[test]
fn the_gunships_wings_follow_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let gunship = spawn(&mut harness, "gunship", [100.0, 100.0]);
        harness.step(2);
        let wings: Vec<_> = harness
            .world()
            .get::<Children>(gunship)
            .unwrap()
            .iter()
            .collect();
        assert_eq!(wings, tagged(&harness, "wing"), "dt {dt}");
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            let at = drawn_at(&harness, gunship);
            for (&wing, dx) in wings.iter().zip([-24.0, 24.0]) {
                let offset = drawn_at(&harness, wing) - at;
                assert!(
                    (offset - glam::Vec2::new(dx, 0.0)).length() < 1e-3,
                    "dt {dt}: {offset}"
                );
            }
        }
        assert!(
            body_at(&harness, gunship).x > 150.0,
            "dt {dt}: it flew its path"
        );
    }
}

#[test]
fn a_score_rule_counts_each_kill_and_shakes_the_camera() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let wings = [
            spawn(&mut harness, "wing", [300.0, 300.0]),
            spawn(&mut harness, "wing", [340.0, 300.0]),
        ];
        harness.step(1);
        assert_eq!(
            harness.world().resource::<CameraController>().shake_trauma,
            0.0
        );
        for wing in wings {
            assert_eq!(damage(harness.world_mut(), wing, 1.0), Hit::Killed);
        }
        harness.step(1);
        assert_eq!(
            harness.world().resource::<Counters>().get("score"),
            2,
            "dt {dt}"
        );
        let trauma = harness.world().resource::<CameraController>().shake_trauma;
        assert!(trauma > 0.5, "dt {dt}: two kills of 0.3 each: {trauma}");
        harness.step(1);
        assert_eq!(
            harness.world().resource::<Counters>().get("score"),
            2,
            "dt {dt}: once each"
        );
    }
}
