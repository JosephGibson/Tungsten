//! The shmup fixture: a bodiless ship under the move actions, bullets that
//! expire or leave the view, raiders from a wave on a path, and a walker
//! on a floor under the same actions.

use tungsten_core::RigidBody;
use tungsten_core::physics::{Position, Velocity};

use super::{DTS, body_at, drawn_at, frames_for, recipe, spawn, tagged};
use crate::kit::{KitPlugins, PlatformerController};

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
