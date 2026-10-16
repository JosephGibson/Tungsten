//! The grid fixture: a bodiless rogue under the move actions, a hole that
//! spawns rats, a potion that expires, and the camera on the rogue.

use tungsten_core::RigidBody;
use tungsten_core::{CameraController, CameraMode, CameraState};

use super::{DTS, body_at, frames_for, recipe, spawn, tagged};
use crate::kit::{Health, KitPlugins, Source, Tags};

const GENRE: &str = "grid";

#[test]
fn the_rogue_moves_by_position() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let rogue = spawn(&mut harness, "rogue", [64.0, 64.0]);
        assert!(!harness.world().has::<RigidBody>(rogue));
        harness.press_action("move_down");
        harness.step(frames_for(0.5, dt));
        let at = body_at(&harness, rogue);
        assert!(at.y > 64.0 + 20.0, "dt {dt}: 64 px/s: {at}");
        assert!((at.x - 64.0).abs() < 1e-3);
    }
}

#[test]
fn the_hole_spawns_rats() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hole = spawn(&mut harness, "hole", [96.0, 96.0]);
        harness.step(frames_for(1.1, dt));
        let rats = tagged(&harness, "rat");
        assert_eq!(rats.len(), 2, "dt {dt}");
        for rat in rats {
            let world = harness.world();
            assert!(world.get::<Tags>(rat).unwrap().has("team.enemy"));
            assert_eq!(world.get::<Health>(rat).unwrap().max(), 1.0);
            assert_eq!(world.get::<Source>(rat), Some(&Source::new(hole)));
        }
    }
}

#[test]
fn the_potion_expires() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let potion = spawn(&mut harness, "potion", [32.0, 32.0]);
        harness.step(frames_for(0.3, dt));
        assert!(harness.world().is_alive(potion), "dt {dt}: 0.3 of 0.5 s");
        harness.step(frames_for(0.3, dt));
        assert!(!harness.world().is_alive(potion), "dt {dt}: 0.6 s");
    }
}

#[test]
fn the_camera_follows_the_rogue() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let rogue = spawn(&mut harness, "rogue", [64.0, 64.0]);
        harness.step(1);
        assert_eq!(
            harness.world().resource::<CameraController>().mode,
            CameraMode::Follow(rogue)
        );
        let before = harness.world().resource::<CameraState>().position;
        harness.press_action("move_right");
        harness.step(frames_for(1.0, dt));
        let after = harness.world().resource::<CameraState>().position;
        assert!(after.x > before.x, "dt {dt}: the camera moved");
    }
}
