//! The rpg fixture, a dungeon's staples (the paper designs' G3): a bodiless
//! rogue under the move actions, a hole that spawns rats, a potion that
//! expires, and the camera on the rogue; M64's crawlers and warden; M65's
//! adventurer and its swing, a chest and its loot, gold and spikes; M67's
//! blessing and level on the adventurer; 7.10c's lantern warden, Lantern
//! Ward's: its bell, flask and run as cue events, its flask from its centre,
//! its toll's wave; its wisps, held to two by their cap, the first culled
//! with a poof; its salt, set down on a mark and refused inside a wall;
//! 7.10g's thrower, whose throw's cue plays its clip over its walk, and its
//! pole warden, whose swing turns with its four facings; a spike trap
//! switched off and on, gold that blinks faster as it goes, a rat den
//! whose first rat comes at once, a crate warden, who carries a ward crate
//! at its side as it turns and lobs it onto a mark, and a fogling whose
//! health bar waits for a hurt.

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

// 7.10c, Step 5 (place-and-caps): the warden's wisps, two at most (S-18).

#[test]
fn a_third_wisp_culls_the_first_with_a_poof() {
    use crate::kit::{Used, centre_of, use_ability};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        register_sprite(&mut harness, "wisp", 8, 8);
        register_sprite(&mut harness, "poof", 16, 16);
        let warden = spawn(&mut harness, "lantern_warden", [100.0, 100.0]);
        harness.step(1);
        let cast = |harness: &mut Harness| {
            assert_eq!(
                use_ability(harness.world_mut(), warden, "wisp"),
                Used::Used,
                "dt {dt}"
            );
            harness.step(1);
        };
        cast(&mut harness);
        let [first] = tagged(&harness, "wisp")[..] else {
            panic!("dt {dt}: one wisp");
        };
        // Sent off, so its poof shows where it went.
        harness
            .world_mut()
            .get_mut::<tungsten_core::Transform>(first)
            .unwrap()
            .position = Vec2::new(40.0, 60.0);
        let centre = centre_of(harness.world(), first).unwrap();
        assert_eq!(centre, Vec2::new(44.0, 64.0), "dt {dt}: its drawn centre");
        cast(&mut harness);
        assert_eq!(tagged(&harness, "wisp").len(), 2, "dt {dt}: two");
        assert!(tagged(&harness, "poof").is_empty(), "dt {dt}: none culled");
        cast(&mut harness);
        assert!(
            !harness.world().is_alive(first),
            "dt {dt}: the first culled"
        );
        assert_eq!(tagged(&harness, "wisp").len(), 2, "dt {dt}: still two");
        let [poof] = tagged(&harness, "poof")[..] else {
            panic!("dt {dt}: one poof");
        };
        assert_eq!(
            centre_of(harness.world(), poof),
            Some(centre),
            "dt {dt}: the poof's centre on the first wisp's"
        );
    }
}

// place-and-caps, Step 2: the warden's salt, set down on its mark (S-17).

#[test]
fn salt_is_set_down_on_its_mark_and_refused_on_a_wall() {
    use crate::kit::{Used, centre_of, use_ability};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let warden = spawn(&mut harness, "lantern_warden", [100.0, 100.0]);
        let open = spawn(&mut harness, "salt_mark", [160.0, 100.0]);
        harness.step(1);
        assert_eq!(
            use_ability(harness.world_mut(), warden, "salt"),
            Used::Used,
            "dt {dt}"
        );
        harness.step(1);
        let [salt] = tagged(&harness, "salt")[..] else {
            panic!("dt {dt}: one salt");
        };
        assert_eq!(
            centre_of(harness.world(), salt),
            centre_of(harness.world(), open),
            "dt {dt}: its centre on the mark's"
        );
        // A nearer mark inside a static wall, once the cooldown is out.
        spawn(&mut harness, "ward_wall", [130.0, 100.0]);
        spawn(&mut harness, "salt_mark", [130.0, 100.0]);
        harness.step(frames_for(2.0, dt));
        assert_eq!(
            use_ability(harness.world_mut(), warden, "salt"),
            Used::NoRoom,
            "dt {dt}: no room inside the wall"
        );
        assert!(
            harness
                .world()
                .get::<Abilities>(warden)
                .unwrap()
                .ready("salt"),
            "dt {dt}: its cooldown unspent"
        );
        harness.step(1);
        assert_eq!(tagged(&harness, "salt").len(), 1, "dt {dt}");
    }
}

/// 7.10g: the thrower walks; a throw's `ability_used` cue plays its throw
/// clip once over the walk, the walk showing again after its 300 ms.
#[test]
fn the_warden_throws_over_its_walk() {
    use tungsten_core::AnimationState;

    use crate::kit::CharacterAnimator;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let thrower = spawn(&mut harness, "thrower", [100.0, 100.0]);
        let shown = |harness: &Harness| {
            let world = harness.world();
            let state = world.get::<AnimationState>(thrower).unwrap();
            let animator = world.get::<CharacterAnimator>(thrower).unwrap();
            (state.animation_id.clone(), animator.state())
        };
        harness.press_action("move_right");
        harness.step(2);
        assert_eq!(shown(&harness), ("warden_walk".into(), "move"), "dt {dt}");
        harness.press_action("attack");
        harness.step(1);
        harness.release_action("attack");
        assert_eq!(
            shown(&harness),
            ("warden_throw".into(), "move"),
            "dt {dt}: the throw over the walk"
        );
        assert_eq!(tagged(&harness, "poof").len(), 1, "dt {dt}: its payload");
        for _ in 1..frames_for(0.3, dt) {
            harness.step(1);
            assert_eq!(shown(&harness).0, "warden_throw", "dt {dt}");
        }
        harness.step(1);
        assert_eq!(shown(&harness), ("warden_walk".into(), "move"), "dt {dt}");
    }
}

/// 7.10g: the pole warden turns by its move actions, four ways, and its
/// swing's hitbox, written facing right, turns with it: each swing strikes
/// the straw dummy ahead and no other.
#[test]
fn the_wardens_pole_swings_ahead_in_four_facings() {
    use crate::kit::Struck;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let warden = spawn(&mut harness, "pole_warden", [100.0, 100.0]);
        let dummies = [
            (
                "move_right",
                spawn(&mut harness, "straw_dummy", [126.0, 100.0]),
            ),
            (
                "move_down",
                spawn(&mut harness, "straw_dummy", [100.0, 126.0]),
            ),
            (
                "move_left",
                spawn(&mut harness, "straw_dummy", [74.0, 100.0]),
            ),
            ("move_up", spawn(&mut harness, "straw_dummy", [100.0, 74.0])),
        ];
        harness.step(1);
        for (action, ahead) in dummies {
            harness.press_action(action);
            harness.step(1);
            harness.release_action(action);
            harness.press_action("attack");
            let mut struck = Vec::new();
            for frame in 0..frames_for(0.25, dt) {
                harness.step(1);
                if frame == 0 {
                    harness.release_action("attack");
                }
                struck.extend(
                    harness
                        .events::<Struck>()
                        .filter(|hit| hit.entity == warden)
                        .map(|hit| hit.target),
                );
            }
            assert_eq!(struck, [ahead], "dt {dt}: {action}");
        }
    }
}

/// 7.10g: the spike trap hurts the warden standing in it once a second;
/// switched off, it hurts nothing, its clock held; switched on again, it
/// hurts once its interval allows, the rest of it.
#[test]
fn spikes_switch_off_and_on() {
    use crate::kit::ContactDamage;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let trap = spawn(&mut harness, "spike_trap", [300.0, 3000.0]);
        let warden = spawn(&mut harness, "hold_warden", [300.0, 3000.0]);
        let health = |harness: &Harness| harness.world().get::<Health>(warden).unwrap().current();
        let switch = |harness: &mut Harness, on: bool| {
            harness
                .world_mut()
                .get_mut::<ContactDamage>(trap)
                .unwrap()
                .set_on(on);
        };
        harness.step(1);
        assert_eq!(health(&harness), 9.0, "dt {dt}: at once");
        harness.step(frames_for(0.5, dt));
        switch(&mut harness, false);
        harness.step(frames_for(2.0, dt));
        assert_eq!(health(&harness), 9.0, "dt {dt}: off, nothing");
        switch(&mut harness, true);
        harness.step(frames_for(0.4, dt));
        assert_eq!(health(&harness), 9.0, "dt {dt}: its interval not yet");
        harness.step(frames_for(0.2, dt));
        assert_eq!(health(&harness), 8.0, "dt {dt}: the interval's rest");
    }
}

/// 7.10g: gold that lives 4 s shows its first 2 s, then blinks over its
/// last 2, hidden half of each period, faster in the last second.
#[test]
fn gold_blinks_faster_as_it_goes() {
    use tungsten_core::Visibility;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let gold = spawn(&mut harness, "fading_gold", [400.0, 3000.0]);
        let shows = |harness: &Harness| {
            harness
                .world()
                .get::<Visibility>(gold)
                .is_none_or(|visibility| visibility.visible)
        };
        for frame in 0..frames_for(2.0, dt) - 1 {
            harness.step(1);
            assert!(shows(&harness), "dt {dt}, frame {frame}: whole");
        }
        let mut hides = [0, 0];
        let mut was = shows(&harness);
        let half = frames_for(1.0, dt);
        for frame in 0..2 * half - 1 {
            harness.step(1);
            let now = shows(&harness);
            if was && !now {
                hides[(frame / half) as usize] += 1;
            }
            was = now;
        }
        assert!(
            hides[0] >= 2 && hides[1] > hides[0],
            "dt {dt}: hides by second {hides:?}"
        );
        harness.step(4);
        assert!(!harness.world().is_alive(gold), "dt {dt}: gone at its 4 s");
    }
}

/// 7.10g: the rat den's `first: 0` spawns its first rat at once, the next
/// an interval later.
#[test]
fn a_rat_hole_spawns_at_once() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "rat_den", [600.0, 3000.0]);
        harness.step(1);
        assert_eq!(tagged(&harness, "rat").len(), 1, "dt {dt}: at once");
        harness.step(frames_for(1.9, dt));
        assert_eq!(tagged(&harness, "rat").len(), 1, "dt {dt}");
        harness.step(frames_for(0.2, dt));
        assert_eq!(tagged(&harness, "rat").len(), 2, "dt {dt}: an interval on");
    }
}

/// 7.10g: a warden carries a ward crate at its side, held on its left
/// while it faces left, and lobs it onto a mark: the crate's own body
/// back, its centre on the mark once the lob's half second is out.
#[test]
fn the_warden_lobs_a_crate_onto_its_mark() {
    use tungsten_core::physics::{BodyKind, PhysicsConfig};

    use crate::kit::{Hold, carried_by, carry, lob_velocity, release};

    let side = Vec2::new(12.0, -6.0);
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let warden = spawn(&mut harness, "crate_warden", [100.0, 100.0]);
        let crate_ = spawn(&mut harness, "ward_crate", [60.0, 160.0]);
        let mark = spawn(&mut harness, "salt_mark", [220.0, 80.0]);
        harness.step(1);
        assert!(
            carry(harness.world_mut(), warden, crate_, Hold::new(side)),
            "dt {dt}"
        );
        let held = |harness: &Harness| point_of(harness, crate_) - point_of(harness, warden);
        harness.step(1);
        assert!(
            (held(&harness) - side).length() < 1e-3,
            "dt {dt}: at its side"
        );

        // Facing left, it holds the crate on its left.
        harness.press_action("move_left");
        harness.step(frames_for(0.25, dt));
        harness.release_action("move_left");
        harness.step(1);
        assert!(
            (held(&harness) - Vec2::new(-12.0, -6.0)).length() < 1e-3,
            "dt {dt}: mirrored: {}",
            held(&harness)
        );
        assert!(point_of(&harness, warden).x < 90.0, "dt {dt}: it walked");

        // Facing right again, it lobs the crate onto the mark.
        harness.press_action("move_right");
        harness.step(1);
        harness.release_action("move_right");
        harness.step(1);
        assert!((held(&harness) - side).length() < 1e-3, "dt {dt}");
        let from = point_of(&harness, crate_);
        let to = point_of(&harness, mark);
        let gravity = harness.world().resource::<PhysicsConfig>().gravity;
        let launch = lob_velocity(from, to, 0.5, gravity);
        assert_eq!(release(harness.world_mut(), warden, launch), Some(crate_));
        harness.step(frames_for(0.5, dt));
        let world = harness.world();
        assert_eq!(carried_by(world, warden), None, "dt {dt}");
        assert_eq!(
            world.get::<RigidBody>(crate_).unwrap().kind,
            BodyKind::Dynamic,
            "dt {dt}: its own body"
        );
        let landed = point_of(&harness, crate_);
        assert!(
            (landed - to).length() < 0.5,
            "dt {dt}: on the mark: {landed}"
        );
    }
}

/// 7.10g: the fogling's bar, `full: false`, stays hidden at full health and
/// shows its share from the first hurt.
#[test]
fn a_foglings_bar_waits_for_a_hurt() {
    use glam::Vec2 as V;
    use tungsten_core::{Transform, Visibility};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let white = register_sprite(&mut harness, "engine_white", 1, 1);
        let fogling = spawn(&mut harness, "fogling", [500.0, 3000.0]);
        let bar = |harness: &Harness| {
            let mut parts: Vec<(i32, V, bool)> = harness
                .world()
                .query::<(&Transform, &Sprite, &Visibility)>()
                .filter(|(_, sprite, _)| sprite.asset_id == white)
                .map(|(transform, sprite, visibility)| {
                    (sprite.z_order, transform.scale, visibility.visible)
                })
                .collect();
            parts.sort_by_key(|part| part.0);
            parts
        };
        harness.step(1);
        let parts = bar(&harness);
        assert_eq!(parts.len(), 2, "dt {dt}");
        assert!(parts.iter().all(|part| !part.2), "dt {dt}: hidden at full");
        damage(harness.world_mut(), fogling, 2.5);
        harness.step(1);
        let parts = bar(&harness);
        assert!(parts.iter().all(|part| part.2), "dt {dt}: shown once hurt");
        assert_eq!(parts[1].1, V::new(22.5, 2.0), "dt {dt}: 30 px at 0.75");
    }
}
