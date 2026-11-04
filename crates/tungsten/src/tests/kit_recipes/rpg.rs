//! The rpg fixture, a dungeon's staples (the paper designs' G3): a bodiless
//! rogue under the move actions, a hole that spawns rats, a potion that
//! expires, and the camera on the rogue; M64's crawlers and warden; M65's
//! adventurer and its swing, a chest and its loot, gold and spikes; M67's
//! blessing and level on the adventurer; 7.10c's lantern warden, Lantern
//! Ward's: its bell, flask and run as cue events, its flask from its centre,
//! its toll's wave.

use glam::Vec2;
use tungsten_core::RigidBody;
use tungsten_core::{CameraController, CameraMode, CameraState, Entity, Sprite};

use super::{
    DTS, body_at, drawn_at, frames_for, point_of, recipe, register_sprite, spawn, tagged,
    traced_steps,
};
use crate::kit::{
    Abilities, Cooldown, Counters, Effects, Flags, Health, Hit, KitPlugins, KitRng, Level, Source,
    Tags, TopDownMover, damage,
};
use crate::testing::Harness;

const GENRE: &str = "rpg";

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

#[test]
fn the_delvers_turns_tick_on_its_cooldown() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let delver = spawn(&mut harness, "delver", [10.0, 10.0]);
        let turns = |harness: &super::Harness| {
            harness
                .world()
                .get::<Counters>(delver)
                .unwrap()
                .get("turns")
        };
        harness.step(frames_for(0.2, dt));
        assert_eq!(turns(&harness), 0, "dt {dt}");
        harness.step(frames_for(0.8, dt));
        assert_eq!(turns(&harness), 4, "dt {dt}: a turn each quarter second");
        harness
            .world_mut()
            .resource_mut::<tungsten_core::Time>()
            .pause();
        harness.step(frames_for(1.0, dt));
        assert_eq!(
            turns(&harness),
            4,
            "dt {dt}: a paused clock holds the turns"
        );
    }
}

#[test]
fn the_trapdoor_opens_once_its_timer_ends() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let trapdoor = spawn(&mut harness, "trapdoor", [20.0, 20.0]);
        let closed = |harness: &super::Harness| {
            harness
                .world()
                .get::<Flags>(trapdoor)
                .unwrap()
                .has("closed")
        };
        harness.step(frames_for(0.9, dt));
        assert!(closed(&harness), "dt {dt}");
        harness.step(frames_for(0.2, dt));
        assert!(!closed(&harness), "dt {dt}: it opened");
        assert_eq!(
            harness
                .world()
                .get::<Cooldown>(trapdoor)
                .unwrap()
                .remaining(),
            Some(0)
        );
        harness.step(frames_for(2.0, dt));
        assert!(!closed(&harness), "dt {dt}: and stays open");
    }
}

#[test]
fn the_slug_counts_its_death_and_goes() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let slug = spawn(&mut harness, "slug", [30.0, 30.0]);
        harness.step(1);
        assert_eq!(damage(harness.world_mut(), slug, 1.0), Hit::Killed);
        harness.step(1);
        assert!(!harness.world().is_alive(slug), "dt {dt}");
        assert_eq!(
            harness.world().resource::<Counters>().get("slugs"),
            1,
            "dt {dt}"
        );
    }
}

fn head(harness: &mut super::Harness, entity: Entity, direction: Vec2) {
    harness
        .world_mut()
        .get_mut::<TopDownMover>(entity)
        .unwrap()
        .set_direction(direction);
}

#[test]
fn crawlers_sort_by_depth_as_they_pass_and_a_warden_stays_in_its_room() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let still = spawn(&mut harness, "crawler", [100.0, 100.0]);
        let passing = spawn(&mut harness, "crawler", [100.0, 40.0]);
        head(&mut harness, passing, Vec2::Y);
        let z = |harness: &super::Harness, entity| {
            harness.world().get::<Sprite>(entity).unwrap().z_order
        };
        harness.step(1);
        assert!(
            z(&harness, passing) < z(&harness, still),
            "dt {dt}: above, behind"
        );
        harness.step(frames_for(4.0, dt));
        assert!(body_at(&harness, passing).y > 100.0, "dt {dt}: it passed");
        assert!(
            z(&harness, passing) > z(&harness, still),
            "dt {dt}: below, in front"
        );

        // The room is 64..256 by 64..192, the warden 16 px.
        let warden = spawn(&mut harness, "warden", [200.0, 100.0]);
        head(&mut harness, warden, Vec2::new(-1.0, 1.0));
        harness.step(frames_for(4.0, dt));
        assert_eq!(body_at(&harness, warden), Vec2::new(64.0, 176.0), "dt {dt}");
        head(&mut harness, warden, Vec2::new(1.0, -1.0));
        harness.step(frames_for(4.0, dt));
        assert_eq!(body_at(&harness, warden), Vec2::new(240.0, 64.0), "dt {dt}");
    }
}

#[test]
fn a_chest_rolls_its_loot_when_broken() {
    let mut counts = Vec::new();
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let adventurer = spawn(&mut harness, "adventurer", [32.0, 32.0]);
        // Inside the swing's box, 12 px to the adventurer's right; the
        // gold it drops out of the adventurer's reach.
        let chest = spawn(&mut harness, "chest", [46.0, 32.0]);
        let draws = harness.world().resource::<KitRng>().draws();
        harness.step(frames_for(0.75, dt));
        assert!(
            !harness.world().is_alive(chest),
            "dt {dt}: broken by the swing"
        );
        assert_eq!(
            harness.world().resource::<KitRng>().draws() - draws,
            2,
            "dt {dt}: the table's two rolls"
        );
        let gold = tagged(&harness, "gold");
        assert!(gold.len() <= 2, "dt {dt}");
        for &piece in &gold {
            assert!(point_of(&harness, piece).distance(Vec2::new(46.0, 32.0)) < 1e-3);
        }
        assert_eq!(
            harness
                .world()
                .get::<Counters>(adventurer)
                .unwrap()
                .get("gold"),
            0
        );
        counts.push(gold.len());
    }
    assert_eq!(counts[0], counts[1], "one seed, one roll at each dt");
    assert!(counts[0] >= 1, "the default seed draws gold: {counts:?}");
}

#[test]
fn gold_counts_once() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let adventurer = spawn(&mut harness, "adventurer", [32.0, 32.0]);
        let gold = spawn(&mut harness, "gold", [32.0, 32.0]);
        harness.step(frames_for(0.25, dt));
        assert!(!harness.world().is_alive(gold), "dt {dt}");
        assert_eq!(
            harness
                .world()
                .get::<Counters>(adventurer)
                .unwrap()
                .get("gold"),
            1,
            "dt {dt}: once"
        );
    }
}

#[test]
fn spikes_hurt_the_adventurer_each_interval() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "spikes", [0.0, 0.0]);
        let adventurer = spawn(&mut harness, "adventurer", [5.0, 0.0]);
        harness.step(frames_for(62.0 / 60.0, dt));
        assert_eq!(
            traced_steps(&harness, adventurer, "damaged"),
            [1, 31, 61],
            "dt {dt}"
        );
        assert_eq!(
            harness.world().get::<Health>(adventurer).unwrap().current(),
            7.0
        );
    }
}

// M67: the adventurer's inline effect and its level.

#[test]
fn the_adventurers_inline_effect_expires_and_its_level_grants_an_ability() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let adventurer = spawn(&mut harness, "adventurer", [32.0, 32.0]);
        harness.step(1);
        let world = harness.world();
        assert!(
            world.get::<Tags>(adventurer).unwrap().has("status.blessed"),
            "dt {dt}"
        );
        let effects = world.get::<Effects>(adventurer).unwrap();
        assert_eq!(effects.instances().count(), 1);
        assert!(
            effects
                .instances()
                .next()
                .unwrap()
                .effect()
                .as_inline()
                .is_some()
        );
        harness.step(frames_for(2.05, dt));
        let world = harness.world();
        assert!(
            !world.get::<Tags>(adventurer).unwrap().has("status.blessed"),
            "dt {dt}"
        );
        assert_eq!(
            world
                .get::<Effects>(adventurer)
                .unwrap()
                .instances()
                .count(),
            0
        );
        // One gold reaches level 2; the draw offers the blessing or the
        // swing ability, so raise the count until the ability comes up.
        let gold = spawn(&mut harness, "gold", [32.0, 32.0]);
        harness.step(frames_for(0.25, dt));
        assert!(!harness.world().is_alive(gold), "dt {dt}");
        let level = harness.world().get::<Level>(adventurer).unwrap();
        assert_eq!((level.level(), level.offers().len()), (2, 1), "dt {dt}");
        let mut chosen = false;
        for (level, gold) in [(2, 1), (3, 15)] {
            if gold > 1 {
                harness
                    .world_mut()
                    .get_mut::<Counters>(adventurer)
                    .unwrap()
                    .set("gold", gold);
                harness.step(1);
            }
            let found = harness.world().get::<Level>(adventurer).unwrap();
            assert_eq!(found.level(), level, "dt {dt}");
            if found.offers()[0].ability().is_some() {
                assert!(
                    harness
                        .world_mut()
                        .get_mut::<Level>(adventurer)
                        .unwrap()
                        .choose(0)
                );
                chosen = true;
                break;
            }
        }
        assert!(chosen, "dt {dt}: the ability was offered by level 3");
        harness.step(1);
        let abilities = harness.world().get::<Abilities>(adventurer).unwrap();
        assert_eq!(
            abilities.names().collect::<Vec<_>>(),
            ["#0"],
            "dt {dt}: the inline ability"
        );
        assert!(abilities.ready("#0"));
    }
}

// 7.10c, Step 3: Lantern Ward's warden (LW14, LW28) and its ability events.

/// The lantern warden, a step in.
fn warden(dt: f32) -> (Harness, Entity) {
    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    let warden = spawn(&mut harness, "lantern_warden", [100.0, 100.0]);
    harness.step(1);
    (harness, warden)
}

/// `entity`'s counter `name`.
fn counter(harness: &Harness, entity: Entity, name: &str) -> i64 {
    harness.world().get::<Counters>(entity).unwrap().get(name)
}

/// Presses `action` for a frame, then lets it go.
fn tap(harness: &mut Harness, action: &str) {
    harness.press_action(action);
    harness.step(1);
    harness.release_action(action);
}

#[test]
fn the_bell_rings_for_the_bell_alone() {
    for dt in DTS {
        let (mut harness, warden) = warden(dt);
        tap(&mut harness, "fire");
        assert_eq!(counter(&harness, warden, "rings"), 1, "dt {dt}: the bell");
        assert_eq!(tagged(&harness, "noise").len(), 1, "dt {dt}: its noise");
        tap(&mut harness, "attack");
        assert_eq!(tagged(&harness, "oil_flask").len(), 1, "dt {dt}: a flask");
        assert_eq!(
            counter(&harness, warden, "rings"),
            1,
            "dt {dt}: the flask rings nothing"
        );
        harness.step(frames_for(2.0, dt));
        tap(&mut harness, "fire");
        assert_eq!(counter(&harness, warden, "rings"), 2, "dt {dt}: again");
    }
}

#[test]
fn the_wardens_run_cues_its_hold_and_release() {
    for dt in DTS {
        let (mut harness, warden) = warden(dt);
        let running = |harness: &Harness| {
            let world = harness.world();
            (
                world.get::<Flags>(warden).unwrap().has("running"),
                world.get::<Tags>(warden).unwrap().has("state.running"),
            )
        };
        assert_eq!(running(&harness), (false, false));
        harness.press_action("dash");
        harness.step(1);
        assert_eq!(running(&harness), (true, true), "dt {dt}: the hold's cue");
        harness.step(frames_for(0.5, dt));
        assert_eq!(running(&harness), (true, true), "dt {dt}: held");
        harness.release_action("dash");
        harness.step(1);
        assert!(!running(&harness).0, "dt {dt}: the release's cue");
        harness.step(frames_for(0.2, dt));
        assert_eq!(
            running(&harness),
            (false, false),
            "dt {dt}: the run's effect ran out"
        );
    }
}

#[test]
fn the_flask_chimes_when_ready() {
    for dt in DTS {
        let (mut harness, warden) = warden(dt);
        tap(&mut harness, "attack");
        assert_eq!(tagged(&harness, "oil_flask").len(), 1, "dt {dt}: thrown");
        let mut frames = 0;
        while counter(&harness, warden, "chimes") == 0 {
            harness.step(1);
            frames += 1;
            assert!(frames <= frames_for(2.0, dt), "dt {dt}: no chime");
        }
        let after = frames as f32 * dt;
        assert!(
            (after - 1.0).abs() <= dt + 1e-4,
            "dt {dt}: its 1 s cooldown out: {after}"
        );
        // Once; the bell turning ready chimes nothing.
        tap(&mut harness, "fire");
        harness.step(frames_for(3.0, dt));
        assert_eq!(counter(&harness, warden, "chimes"), 1, "dt {dt}: once");
    }
}

// 7.10c, Step 4: the warden's flask from its centre (LW14, S-88).

#[test]
fn a_flask_leaves_from_the_wardens_centre() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        register_sprite(&mut harness, "lantern_warden", 16, 32);
        register_sprite(&mut harness, "oil_flask", 8, 8);
        let warden = spawn(&mut harness, "lantern_warden", [100.0, 100.0]);
        harness.step(1);
        let centre = crate::kit::centre_of(harness.world(), warden).unwrap();
        assert_eq!(centre, Vec2::new(108.0, 116.0), "dt {dt}: its drawn centre");
        tap(&mut harness, "attack");
        let [flask] = tagged(&harness, "oil_flask")[..] else {
            panic!("dt {dt}: one flask");
        };
        assert_eq!(
            drawn_at(&harness, flask),
            Vec2::new(104.0, 112.0),
            "dt {dt}: its point 4 px up and left of the warden's centre"
        );
        assert_eq!(
            crate::kit::centre_of(harness.world(), flask),
            Some(centre),
            "dt {dt}: its centre on the warden's"
        );
    }
}

// 7.10c, Step 6: the warden's toll, a sweeping cone (S-15).

#[test]
fn the_bells_wave_pushes_a_crate_and_knocks_back_once() {
    use tungsten_core::physics::Velocity;
    for dt in DTS {
        let (mut harness, warden) = warden(dt);
        let fogborn = spawn(&mut harness, "fogborn", [200.0, 100.0]);
        let crate_ = spawn(&mut harness, "ward_crate", [150.0, 120.0]);
        harness.step(1);
        assert_eq!(
            crate::kit::use_ability(harness.world_mut(), warden, "toll"),
            crate::kit::Used::Used
        );
        let mut hits = 0;
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            hits += harness
                .events::<crate::kit::Damaged>()
                .filter(|hit| hit.entity == fogborn)
                .count();
        }
        let velocity = |entity: Entity| harness.world().get::<Velocity>(entity).unwrap().0;
        let pushed = velocity(crate_);
        // Nine rays every 7.5 degrees along +x, toward the fogborn: the
        // crate, 21.8 degrees off, met by the 22.5-degree ray.
        let along = glam::Vec2::from_angle(22.5_f32.to_radians()) * 400.0;
        assert!(
            (pushed - along).length() < 0.5,
            "dt {dt}: the crate pushed once along its ray: {pushed}"
        );
        assert!(
            (velocity(fogborn) - glam::Vec2::new(400.0, 0.0)).length() < 0.5,
            "dt {dt}: the fogborn knocked back once: {}",
            velocity(fogborn)
        );
        assert_eq!(hits, 1, "dt {dt}: and hit once");
        assert_eq!(
            harness.world().get::<Health>(fogborn).unwrap().current(),
            9.0
        );
    }
}
