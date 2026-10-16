//! The top-down fixture: a hero on a kinematic body, a wall, slimes from a
//! nest, a trap zone, a drone on a loop and a bolt that leaves its region.

use tungsten_core::{CameraController, CameraMode, CameraState};

use super::{DTS, body_at, frames_for, recipe, spawn, tagged};
use crate::kit::{Damaged, Health, Hit, HitFlash, KitPlugins, Source, Tags, ZoneEntered, damage};

const GENRE: &str = "top_down";

#[test]
fn the_hero_slides_along_a_wall() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // The wall's left face at x 290; the hero beside it, half 10.
        spawn(&mut harness, "wall", [300.0, 0.0]);
        let hero = spawn(&mut harness, "hero", [270.0, 0.0]);
        harness.press_action("move_right");
        harness.press_action("move_down");
        let mut y = 0.0;
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            let at = body_at(&harness, hero);
            assert!(at.x <= 280.0 + 1e-3, "dt {dt}: never into the wall: {at}");
            assert!(at.y >= y, "dt {dt}: down along it");
            y = at.y;
        }
        assert!(y > 30.0, "dt {dt}: it slid: {y}");
    }
}

#[test]
fn damage_lowers_the_slimes_health_sets_its_flash_and_sends_damaged() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let slime = spawn(&mut harness, "slime", [50.0, 50.0]);
        harness.step(1);
        assert_eq!(damage(harness.world_mut(), slime, 1.0), Hit::Hurt);
        let world = harness.world();
        assert_eq!(world.get::<Health>(slime).unwrap().current(), 1.0);
        assert!(
            world.get::<HitFlash>(slime).unwrap().is_active(),
            "dt {dt}: the fixture's damage_flash material is in place"
        );
        harness.step(1);
        let sent: Vec<Damaged> = harness.events::<Damaged>().copied().collect();
        assert_eq!(sent, [Damaged::new(slime, 1.0, false)]);
    }
}

#[test]
fn the_nests_slimes_carry_the_nest_as_their_source() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let nest = spawn(&mut harness, "nest", [100.0, 100.0]);
        harness.step(frames_for(1.1, dt));
        let slimes = tagged(&harness, "slime");
        assert_eq!(slimes.len(), 2, "dt {dt}: two of the nest's count");
        for slime in slimes {
            let world = harness.world();
            assert_eq!(world.get::<Source>(slime), Some(&Source::new(nest)));
            assert!(world.get::<Tags>(slime).unwrap().has("team.enemy"));
            assert_eq!(body_at(&harness, slime), [100.0, 100.0].into());
        }
    }
}

#[test]
fn the_trap_fires_for_the_hero_and_not_for_a_slime() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let trap = spawn(&mut harness, "trap", [200.0, 0.0]);
        // A slime inside the trap from its first step: a sensor entry the
        // filter refuses.
        spawn(&mut harness, "slime", [200.0, 0.0]);
        harness.step(frames_for(0.2, dt));
        assert_eq!(harness.events::<ZoneEntered>().count(), 0, "dt {dt}");

        let hero = spawn(&mut harness, "hero", [150.0, 0.0]);
        harness.press_action("move_right");
        let mut entered = Vec::new();
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            entered.extend(harness.events::<ZoneEntered>().copied());
        }
        assert_eq!(entered.len(), 1, "dt {dt}: {entered:?}");
        assert_eq!((entered[0].zone, entered[0].entity), (trap, hero));
    }
}

#[test]
fn the_bolt_leaves_its_region() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let bolt = spawn(&mut harness, "bolt", [600.0, 100.0]);
        harness.step(1);
        assert!(harness.world().is_alive(bolt), "dt {dt}: inside at first");
        harness.step(frames_for(0.2, dt));
        assert!(
            !harness.world().is_alive(bolt),
            "dt {dt}: past x 640 at 800 px/s"
        );
    }
}

#[test]
fn the_drone_loops_its_square() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let drone = spawn(&mut harness, "drone", [0.0, 0.0]);
        let mut visited = [false; 4];
        for _ in 0..frames_for(4.0, dt) {
            harness.step(1);
            let at = body_at(&harness, drone);
            for (i, corner) in [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]
                .into_iter()
                .enumerate()
            {
                if (at - glam::Vec2::from(corner)).length() < 3.0 {
                    visited[i] = true;
                }
            }
        }
        assert_eq!(visited, [true; 4], "dt {dt}: every corner of the loop");
    }
}

#[test]
fn the_camera_follows_the_hero() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hero = spawn(&mut harness, "hero", [100.0, 100.0]);
        harness.step(1);
        assert_eq!(
            harness.world().resource::<CameraController>().mode,
            CameraMode::Follow(hero)
        );
        let before = harness.world().resource::<CameraState>().position;
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        let after = harness.world().resource::<CameraState>().position;
        assert!(after.x > before.x, "dt {dt}: the camera moved");
    }
}
