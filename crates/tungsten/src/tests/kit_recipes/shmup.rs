//! The shmup fixture: a bodiless ship under the move actions, bullets that
//! expire or leave the view, raiders from a wave on a path, and a walker
//! on a floor under the same actions; M64's fighter, drifter, asteroid,
//! mine and gun; M65's interceptor, lasers, carrier and bulwark.

use glam::Vec2;
use tungsten_core::RigidBody;
use tungsten_core::physics::{Position, PrevPosition, Velocity};
use tungsten_core::{AudioCommand, CameraController, Transform};

use super::{DTS, body_at, drawn_at, frames_for, recipe, spawn, tagged};
use crate::kit::{
    Aim, Children, CooldownElapsed, Counters, Damaged, Health, Hit, KitPlugins, LinearMotion,
    PlatformerController, ProjectileExpired, Shield, Source, Steering, Struck, TopDownMover,
    damage,
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

#[test]
fn a_fighter_held_against_the_views_edge_stays_in_and_a_drifter_tops_out() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let fighter = spawn(&mut harness, "fighter", [600.0, 300.0]);
        harness
            .world_mut()
            .get_mut::<TopDownMover>(fighter)
            .unwrap()
            .set_direction(Vec2::new(1.0, -0.2));
        let drifter = spawn(&mut harness, "drifter", [100.0, 0.0]);
        for _ in 0..frames_for(3.0, dt) {
            let before = body_at(&harness, drifter);
            harness.step(1);
            let step = body_at(&harness, drifter) - before;
            assert!(
                step.length() <= 150.0 * dt + 1e-3,
                "dt {dt}: under its cap: {step}"
            );
            // The 1280-wide view less its margin and the 24 px box.
            assert!(body_at(&harness, fighter).x <= 1248.0, "dt {dt}");
        }
        let held = body_at(&harness, fighter);
        assert_eq!(held.x, 1248.0, "dt {dt}: against the edge");
        assert!(held.y < 300.0, "dt {dt}: and still free along it: {held}");
        assert_eq!(
            harness
                .world()
                .get::<LinearMotion>(drifter)
                .unwrap()
                .velocity(),
            Vec2::new(0.0, 150.0),
            "dt {dt}: it topped out"
        );
    }
}

#[test]
fn an_asteroid_drifts_and_wraps_the_arena() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let rock = spawn(&mut harness, "asteroid", [600.0, 100.0]);
        let mut wrapped = false;
        for n in 1..=frames_for(1.0, dt) {
            harness.step(1);
            let drawn = drawn_at(&harness, rock).x;
            assert!(
                drawn > 560.0 || drawn < 40.0,
                "dt {dt}: never between: {drawn}"
            );
            wrapped |= drawn < 40.0;
            let t = n as f32 * dt;
            assert!((body_at(&harness, rock).y - (100.0 + 30.0 * t)).abs() < 1e-2);
        }
        assert!(wrapped, "dt {dt}: back in from the left");
        let prev = harness.world().get::<PrevPosition>(rock).unwrap().0;
        assert!(prev.x < 40.0, "dt {dt}: its history moved with it");
    }
}

#[test]
fn a_mine_arrives_at_the_ship_and_a_gun_aims_and_turns_to_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ship = spawn(&mut harness, "ship", [640.0, 600.0]);
        let mine = spawn(&mut harness, "mine", [200.0, 200.0]);
        let gun = spawn(&mut harness, "gun", [640.0, 100.0]);
        harness.step(frames_for(15.0, dt));
        let settled = body_at(&harness, mine).distance(body_at(&harness, ship));
        assert!(settled < 0.5, "dt {dt}: on the ship: {settled}");
        let steering = harness.world().get::<Steering>(mine).unwrap();
        assert!(steering.velocity().length() < 0.5, "dt {dt}");
        assert_eq!(steering.target_entity(), Some(ship));
        // The gun has no body: its point is its `Transform`'s.
        let origin = drawn_at(&harness, gun) + Vec2::new(8.0, 8.0);
        let toward = (body_at(&harness, ship) - origin).normalize();
        let aim = harness.world().get::<Aim>(gun).unwrap();
        assert!(
            (aim.direction().unwrap() - toward).length() < 1e-4,
            "dt {dt}"
        );
        let turned = harness.world().get::<Transform>(gun).unwrap().rotation;
        let heading = toward.to_angle() + std::f32::consts::FRAC_PI_2;
        assert!(
            (turned - heading).abs() < 1e-4,
            "dt {dt}: {turned} vs {heading}"
        );
    }
}

/// Runs `frames` frames, collecting each one's hits and projectile ends.
fn fire(
    harness: &mut super::Harness,
    frames: u32,
) -> (
    Vec<(tungsten_core::Entity, tungsten_core::Entity)>,
    Vec<ProjectileExpired>,
) {
    let (mut hits, mut expired) = (Vec::new(), Vec::new());
    for _ in 0..frames {
        harness.step(1);
        hits.extend(
            harness
                .events::<Struck>()
                .map(|event| (event.entity, event.target)),
        );
        expired.extend(harness.events::<ProjectileExpired>().copied());
    }
    (hits, expired)
}

#[test]
fn a_laser_hits_once_and_ends_at_its_range() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // The carrier's bottom face at y 310; one laser into it, one into
        // the open.
        let carrier = spawn(&mut harness, "carrier", [100.0, 300.0]);
        let into = spawn(&mut harness, "laser", [100.0, 600.0]);
        let open = spawn(&mut harness, "laser", [400.0, 600.0]);
        let (hits, expired) = fire(&mut harness, frames_for(1.0, dt));
        assert_eq!(hits, [(into, carrier)], "dt {dt}: once");
        let at = |laser| {
            expired
                .iter()
                .find(|end| end.entity == laser)
                .unwrap()
                .position
        };
        assert!(
            at(into).distance(Vec2::new(100.0, 316.0)) < 1e-3,
            "dt {dt}: {}",
            at(into)
        );
        assert!(
            at(open).distance(Vec2::new(400.0, 100.0)) < 1e-3,
            "dt {dt}: {}",
            at(open)
        );
        assert_eq!(
            harness.world().get::<Shield>(carrier).unwrap().current(),
            1.0
        );
    }
}

#[test]
fn a_carriers_shield_recharges_after_its_delay() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let carrier = spawn(&mut harness, "carrier", [100.0, 100.0]);
        harness.step(1);
        assert_eq!(
            damage(harness.world_mut(), carrier, 2.0),
            Hit::Absorbed,
            "dt {dt}"
        );
        let shield =
            |harness: &super::Harness| harness.world().get::<Shield>(carrier).unwrap().current();
        harness.step(frames_for(1.0, dt));
        assert!(
            shield(&harness) < 1e-3,
            "dt {dt}: its delay: {}",
            shield(&harness)
        );
        harness.step(frames_for(0.5, dt));
        assert!(
            (shield(&harness) - 0.5).abs() < 1e-3,
            "dt {dt}: {}",
            shield(&harness)
        );
        harness.step(frames_for(2.0, dt));
        assert_eq!(shield(&harness), 2.0, "dt {dt}: full");
        assert_eq!(
            harness.world().get::<Health>(carrier).unwrap().current(),
            4.0
        );
    }
}

#[test]
fn a_bulwark_ignores_lasers() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let bulwark = spawn(&mut harness, "bulwark", [100.0, 300.0]);
        let laser = spawn(&mut harness, "laser", [100.0, 600.0]);
        let mut damaged = 0;
        let mut hits = Vec::new();
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            hits.extend(
                harness
                    .events::<Struck>()
                    .map(|event| (event.entity, event.target)),
            );
            damaged += harness
                .events::<Damaged>()
                .filter(|event| event.entity == bulwark)
                .count();
        }
        assert_eq!(hits, [(laser, bulwark)], "dt {dt}: hit");
        assert_eq!(damaged, 0, "dt {dt}: immune");
        assert_eq!(
            harness.world().get::<Health>(bulwark).unwrap().current(),
            3.0
        );
        assert!(
            !harness.world().is_alive(laser),
            "dt {dt}: the hit ended it"
        );
    }
}
