//! The top-down fixture: a hero on a kinematic body, a wall, slimes from a
//! nest, a trap zone, a drone on a loop and a bolt that leaves its region.

use tungsten_core::physics::Velocity;
use tungsten_core::{AudioCommand, CameraController, CameraMode, CameraState};

use super::{DTS, body_at, frames_for, recipe, spawn, tagged};
use crate::kit::{
    CooldownElapsed, Counters, Damaged, Health, Hit, HitFlash, KitPlugins, KitRng, Modifier,
    Source, Stats, Tags, TopDownMover, ZoneEntered, damage,
};
use crate::testing::Harness;

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

/// The texts of the world labels the last frame drew.
fn labels(harness: &Harness) -> Vec<String> {
    harness
        .draw()
        .unwrap()
        .world_labels
        .iter()
        .map(|run| run.content.clone())
        .collect()
}

#[test]
fn a_cue_rule_counts_kills_and_shows_damage_with_no_prefab_field() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let slimes: Vec<_> = [[50.0, 50.0], [100.0, 50.0], [150.0, 50.0]]
            .into_iter()
            .map(|at| spawn(&mut harness, "slime", at))
            .collect();
        harness.step(1);
        for &slime in &slimes {
            assert_eq!(damage(harness.world_mut(), slime, 1.0), Hit::Hurt);
        }
        harness.step(1);
        assert_eq!(
            labels(&harness),
            ["1", "1", "1"],
            "dt {dt}: each hit's amount"
        );
        let kills = |harness: &Harness| harness.world().resource::<Counters>().get("kills");
        assert_eq!(kills(&harness), 0);
        for &slime in &slimes {
            assert_eq!(damage(harness.world_mut(), slime, 1.5), Hit::Killed);
        }
        harness.step(1);
        assert_eq!(kills(&harness), 3, "dt {dt}: each kill once");
        let mut shown = labels(&harness);
        shown.sort();
        assert_eq!(shown, ["1", "1", "1", "1.5", "1.5", "1.5"], "dt {dt}");
        assert_eq!(damage(harness.world_mut(), slimes[0], 1.0), Hit::Ignored);
        harness.step(1);
        assert_eq!(kills(&harness), 3, "dt {dt}: the dead are not killed twice");
        assert!(
            slimes
                .iter()
                .all(|&slime| !harness.world().has::<Counters>(slime))
        );
    }
}

#[test]
fn the_brutes_speed_follows_a_modifier_and_its_hits_count_flash_and_sound() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let brute = spawn(&mut harness, "brute", [100.0, 100.0]);
        harness
            .world_mut()
            .get_mut::<TopDownMover>(brute)
            .unwrap()
            .set_direction(glam::Vec2::X);
        harness.step(frames_for(0.25, dt));
        let speed = |harness: &Harness| harness.world().get::<Velocity>(brute).unwrap().0.x;
        assert!(
            (speed(&harness) - 80.0).abs() < 1e-3,
            "dt {dt}: {}",
            speed(&harness)
        );
        harness
            .world_mut()
            .get_mut::<Stats>(brute)
            .unwrap()
            .add_modifier(Stats::MOVE_SPEED, "rage", Modifier::Pct(0.5));
        harness.step(2);
        assert!(
            (speed(&harness) - 120.0).abs() < 1e-3,
            "dt {dt}: {}",
            speed(&harness)
        );
        assert_eq!(
            harness.world().get::<Stats>(brute).unwrap().get("armor"),
            Some(2.0)
        );

        let played = |harness: &Harness| {
            harness
                .audio()
                .iter()
                .filter(|command| matches!(command, AudioCommand::Play { .. }))
                .count()
        };
        damage(harness.world_mut(), brute, 1.0);
        harness.step(1);
        assert_eq!(played(&harness), 1, "dt {dt}: the hit's sound");
        assert!(harness.world().get::<HitFlash>(brute).unwrap().is_active());
        damage(harness.world_mut(), brute, 1.0);
        harness.step(1);
        assert_eq!(played(&harness), 0, "dt {dt}: within its min_interval");
        let hits = harness.world().get::<Counters>(brute).unwrap().get("hits");
        assert_eq!(hits, 2, "dt {dt}");
    }
}

#[test]
fn the_bomb_bursts_once_after_its_fuse() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let bomb = spawn(&mut harness, "bomb", [200.0, 200.0]);
        harness.step(frames_for(0.45, dt));
        assert!(
            tagged(&harness, "blast").is_empty(),
            "dt {dt}: the fuse burns"
        );
        harness.step(frames_for(0.15, dt));
        let blasts = tagged(&harness, "blast");
        assert_eq!(blasts.len(), 1, "dt {dt}");
        assert_eq!(
            super::drawn_at(&harness, blasts[0]),
            glam::Vec2::new(200.0, 200.0)
        );
        assert_eq!(
            harness.world().get::<Source>(blasts[0]),
            Some(&Source::new(bomb))
        );
        assert!(
            !harness.world().is_alive(bomb),
            "dt {dt}: it went with its burst"
        );
        harness.step(frames_for(1.0, dt));
        assert!(
            tagged(&harness, "blast").is_empty(),
            "dt {dt}: once, and the blast expired"
        );
    }
}

/// The frames on which the hopper hopped over three seconds under `seed`.
fn hops(seed: u64, dt: f32) -> Vec<u32> {
    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    harness.world_mut().resource_mut::<KitRng>().reseed(seed);
    let hopper = spawn(&mut harness, "hopper", [0.0, 0.0]);
    (0..frames_for(3.0, dt))
        .filter(|_| {
            harness.step(1);
            harness
                .events::<CooldownElapsed>()
                .any(|event| event.entity == hopper)
        })
        .collect()
}

#[test]
fn the_hoppers_jittered_hops_repeat_under_a_seed() {
    for dt in DTS {
        let first = hops(5, dt);
        assert_eq!(first, hops(5, dt), "dt {dt}: one seed, one run");
        assert_ne!(first, hops(6, dt), "dt {dt}");
        assert!((6..=12).contains(&first.len()), "dt {dt}: {first:?}");
        let (low, high) = (frames_for(0.25, dt) - 1, frames_for(0.5, dt) + 1);
        for pair in first.windows(2) {
            let gap = pair[1] - pair[0];
            assert!((low..=high).contains(&gap), "dt {dt}: {first:?}");
        }
    }
}
