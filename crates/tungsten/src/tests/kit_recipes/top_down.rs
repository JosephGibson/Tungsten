//! The top-down fixture: a hero on a kinematic body, a wall, slimes from a
//! nest, a trap zone, a drone on a loop and a bolt that leaves its region;
//! M64's seeker, coward, crates, sentry, wielder and blades, rovers,
//! brawler, pet and gem; M65's ranger and its embers, ghouls, their bursts
//! and drops, orbs and a knight; M66's fantasy damage types: a spellblade
//! and a golem, a pyromancer, its firebolts and a ward; M67's burning
//! slimes, the hero's wand and dash, the ranger's volley, ammo, levels and
//! offers, the golem's cold builds and the ghouls' poison; M74's burn named
//! after a load, the golem's frost stream and a game's own damage numbers;
//! M75's guard saved mid-swing; M76's arrow through hedges, saved
//! mid-flight; 7.10a's veteran with its scoped lines, a focused effect and
//! a difficulty world source, a bulwark braced on its front, and a
//! marksman whose embers crit from its stats; 7.10m's skiff pulled along a
//! wall, an ox that stacks its knockback and a wisp steering its mover;
//! 7.10b's trooper, suppressed by the rounds that pass it; its chemistry's
//! wooden crates, lit by fire, doused by cold and setting each other
//! alight, a lurker a lamp lights, a grass strip a crate sets burning, a
//! lamp a bucket's water douses for 3 s, and a bomb whose fuse ends in a
//! blast among crates and a target; 7.10p's practice dummy, whose burn's
//! ticks and a blow pass a rule's `periodic` apart, and a campfire whose
//! crackle loops by a listener following the hero past it, a storm whose
//! recipe system plays the `thunder` world cue each second, and a
//! shield-bearer whose whole blocks flash the shield's colour, and a
//! kindled lantern whose look tints it over a colour tween and glows, and
//! a blazing crate whose look lights a lurker and crackles, and a coffer
//! whose one game event registration brings `opened` and `emptied` in by
//! name; 7.10g's ghoul scout, whose alert clip shows by its animator's rule
//! while a game's `state.alerted` tag is on, and its mark by its looks,
//! mirrored as it turns; a watchman facing the point it heard, hit shapes
//! drawn on a key, a ghoul guard its death policy disarms, a nest that
//! keeps one slime, across a save and a load, and a brazier whose phases a
//! timed state machine cycles; 7.13c's barrel bomb, whose blast hurts it
//! too, at its centre and as it goes off, a crate knocked into a ranger,
//! credited to its brawler, a stone bolt ending at a rolling ball, and a
//! crystal on a dynamic body drawn in by the ranger's magnet.

use glam::Vec2;
use tungsten_core::physics::{Collider, Position, PrevPosition, RigidBodyBundle, Velocity};
use tungsten_core::{
    AnimationState, AudioCommand, Bundle, CameraController, CameraState, DataRef, Entity,
    EventCursor, EventQueue, Plugin, SaveData, SaveSlots, Schedule, Sprite, Transform, World,
    system,
};

use super::{
    DTS, body_at, drawn_at, frames_for, overlap_reversed, point_of, recipe, spawn, tagged,
    teleport, traced_steps,
};
use crate::kit::{
    Abilities, Aim, CharacterAnimator, ContactDamage, ContactTouch, Conversion, CooldownElapsed,
    Counters, DamagePart, DamageRequest, Damaged, DamagedPart, EffectInstance, EffectRequest,
    Effects, Facing, Health, Hit, HitFlash, HitShape, Hitbox, Hurtbox, KitPlugins, KitRng,
    Knockback, LabelStyle, Level, Line, LinearMotion, Modifier, Penetration, Phase,
    ProjectileExpired, Read, Shield, Source, StateMachine, StateMachinePlugin, Stats, Steering,
    Struck, TagFilter, TagSet, Tags, TopDownMover, TraceKind, WorldSource, WorldSources,
    ZoneEntered, damage, float_label, knock_back,
};
use crate::testing::Harness;
use tungsten_core::PluginSet;

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
            harness.world().resource::<CameraController>().rig.target(),
            Some(hero)
        );
        let before = harness.world().resource::<CameraState>().position;
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        let after = harness.world().resource::<CameraState>().position;
        assert!(after.x > before.x, "dt {dt}: the camera moved");
    }
}

// 7.18, Step 2: rigs as data.

/// The camera's last frame.
fn camera_frame(harness: &Harness) -> tungsten_core::CameraFrame {
    harness.world().resource::<CameraController>().frame()
}

#[test]
fn a_roamers_camera_holds_in_its_dead_zone_then_follows() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let roamer = spawn(&mut harness, "roamer", [300.0, 300.0]);
        harness.step(1);
        let controller = harness.world().resource::<CameraController>();
        assert_eq!(controller.winner(), Some(roamer), "dt {dt}: its own rig");
        let start = camera_frame(&harness);
        // 540 world units tall in a 720 px window: a view of 960 by 540.
        assert!(
            (start.size - Vec2::new(960.0, 540.0)).length() < 1e-3,
            "dt {dt}"
        );
        let anchor = drawn_at(&harness, roamer) + Vec2::splat(10.0);
        assert!(
            (start.anchor.unwrap() - anchor).length() < 1e-3,
            "dt {dt}: its offset"
        );
        let zone = start.dead_zone.unwrap();
        assert!((zone.max - zone.min - Vec2::new(400.0, 240.0)).length() < 1e-3);
        // Half a second right: still inside the dead zone, the view holds.
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        let held = camera_frame(&harness);
        assert!(
            held.anchor.unwrap().x > anchor.x + 50.0,
            "dt {dt}: it roamed"
        );
        assert_eq!(held.corner, start.corner, "dt {dt}: the view held");
        // Past the zone's right side the view follows, the anchor on it.
        harness.step(frames_for(1.5, dt));
        let moved = camera_frame(&harness);
        assert!(moved.corner.x > start.corner.x, "dt {dt}: it followed");
        assert_eq!(moved.corner.y, start.corner.y, "dt {dt}");
        let zone = moved.dead_zone.unwrap();
        assert!(
            (moved.anchor.unwrap().x - zone.max.x).abs() < 1e-2,
            "dt {dt}: the anchor at the zone's edge"
        );
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

fn z(harness: &Harness, entity: Entity) -> i32 {
    harness.world().get::<Sprite>(entity).unwrap().z_order
}

fn walk(harness: &mut Harness, entity: Entity, direction: Vec2) {
    harness
        .world_mut()
        .get_mut::<TopDownMover>(entity)
        .unwrap()
        .set_direction(direction);
}

#[test]
fn a_seeker_chases_faces_walks_four_ways_and_sorts_by_depth_and_a_coward_flees() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hero = spawn(&mut harness, "hero", [400.0, 300.0]);
        let seeker = spawn(&mut harness, "seeker", [100.0, 300.0]);
        let above = spawn(&mut harness, "crate", [250.0, 280.0]);
        let below = spawn(&mut harness, "crate", [250.0, 320.0]);
        let coward = spawn(&mut harness, "coward", [450.0, 300.0]);
        let clip = |harness: &Harness| {
            harness
                .world()
                .get::<AnimationState>(seeker)
                .unwrap()
                .animation_id
                .clone()
        };
        harness.step(frames_for(1.0, dt));
        assert!(body_at(&harness, seeker).x > 130.0, "dt {dt}: it chases");
        assert_eq!(
            harness.world().get::<Facing>(seeker).unwrap().direction(),
            Vec2::X
        );
        assert_eq!(clip(&harness), "seeker_walk_right", "dt {dt}");
        assert!(
            z(&harness, seeker) > z(&harness, above) && z(&harness, seeker) < z(&harness, below),
            "dt {dt}: in front of the crate above it, behind the one below"
        );
        let fled = body_at(&harness, coward).distance(body_at(&harness, hero));
        assert!(
            (96.0..100.0).contains(&fled),
            "dt {dt}: out of its radius: {fled}"
        );
        assert_eq!(
            harness.world().get::<Steering>(coward).unwrap().velocity(),
            Vec2::ZERO
        );

        for (to, walk_clip) in [
            (Vec2::new(0.0, -300.0), "seeker_walk_up"),
            (Vec2::new(0.0, 300.0), "seeker_walk_down"),
            (Vec2::new(-300.0, 0.0), "seeker_walk_left"),
        ] {
            let at = body_at(&harness, seeker) + to;
            teleport(&mut harness, hero, at);
            harness.step(frames_for(0.5, dt));
            assert_eq!(clip(&harness), walk_clip, "dt {dt}");
        }
        // Its target gone, it brakes and idles.
        harness.world_mut().despawn(hero);
        harness.step(frames_for(0.5, dt));
        assert_eq!(clip(&harness), "seeker_idle", "dt {dt}");
        assert_eq!(
            harness
                .world()
                .get::<CharacterAnimator>(seeker)
                .unwrap()
                .state(),
            "idle"
        );
    }
}

#[test]
fn a_sentry_watches_the_nearest_hero_in_range_and_a_wielders_blades_circle_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let sentry = spawn(&mut harness, "sentry", [0.0, 0.0]);
        let hero = spawn(&mut harness, "hero", [100.0, 0.0]);
        let wielder = spawn(&mut harness, "wielder", [300.0, 300.0]);
        walk(&mut harness, wielder, Vec2::X);
        harness.step(1);
        let aim = harness.world().get::<Aim>(sentry).unwrap();
        assert_eq!(
            (aim.direction(), aim.target_entity()),
            (Some(Vec2::X), Some(hero))
        );
        teleport(&mut harness, hero, Vec2::new(0.0, 500.0));
        harness.step(1);
        let aim = harness.world().get::<Aim>(sentry).unwrap();
        assert_eq!(
            (aim.direction(), aim.target_entity()),
            (Some(Vec2::X), None),
            "dt {dt}: out of range, it holds"
        );

        harness.step(frames_for(1.2, dt));
        let blades = tagged(&harness, "blade");
        assert_eq!(blades.len(), 2, "dt {dt}");
        for &blade in &blades {
            assert_eq!(
                harness.world().get::<Source>(blade).unwrap().entity(),
                wielder
            );
        }
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            let centre = body_at(&harness, wielder);
            for &blade in &blades {
                let radius = body_at(&harness, blade).distance(centre);
                assert!((radius - 40.0).abs() < 1e-2, "dt {dt}: {radius}");
            }
        }
        let centre = body_at(&harness, wielder);
        let apart = (body_at(&harness, blades[0]) - centre)
            .angle_to(body_at(&harness, blades[1]) - centre)
            .abs();
        assert!(
            (apart - std::f32::consts::PI).abs() < 0.3,
            "dt {dt}: opposite: {apart}"
        );
        assert!(centre.x > 400.0, "dt {dt}: the wielder moved");
    }
}

#[test]
fn a_rover_wraps_through_a_side_tunnel_and_pushes_slide_along_walls() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let rover = spawn(&mut harness, "rover", [600.0, 200.0]);
        walk(&mut harness, rover, Vec2::X);
        let pushed = spawn(&mut harness, "rover", [300.0, 400.0]);
        // A wall whose left face is at x 890, the brawler's right face at 880.
        spawn(&mut harness, "wall", [900.0, 300.0]);
        let brawler = spawn(&mut harness, "brawler", [870.0, 300.0]);
        harness.step(2);
        assert!(knock_back(
            harness.world_mut(),
            pushed,
            Vec2::new(-400.0, 0.0)
        ));
        assert!(knock_back(
            harness.world_mut(),
            brawler,
            Vec2::new(600.0, -300.0)
        ));
        let mut wrapped = false;
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            let drawn = drawn_at(&harness, rover).x;
            assert!(
                drawn > 560.0 || drawn < 80.0,
                "dt {dt}: never between: {drawn}"
            );
            wrapped |= drawn < 80.0;
            // Its centre at most 10 px on, its right face on the wall's.
            assert!(
                body_at(&harness, brawler).x <= 880.0 + 1e-2,
                "dt {dt}: never into it"
            );
        }
        assert!(wrapped, "dt {dt}: through the tunnel");
        assert!((body_at(&harness, rover).y - 200.0).abs() < 1e-3);
        assert!(
            (body_at(&harness, pushed).x - 260.0).abs() < 1e-2,
            "dt {dt}: 40 px back"
        );
        let slid = body_at(&harness, brawler);
        assert!(
            (slid.x - 880.0).abs() < 0.05,
            "dt {dt}: against the wall: {slid}"
        );
        assert!(slid.y < 280.0, "dt {dt}: it slid up the wall: {slid}");
    }
}

#[test]
fn a_pet_trails_the_hero_bobbing_and_a_gem_bobs_and_pulses() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hero = spawn(&mut harness, "hero", [200.0, 200.0]);
        let pet = spawn(&mut harness, "pet", [0.0, 0.0]);
        let gem = spawn(&mut harness, "gem", [500.0, 300.0]);
        harness.step(frames_for(2.0, dt));
        let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
        let (mut small, mut large) = (f32::INFINITY, f32::NEG_INFINITY);
        let gem_start = body_at(&harness, gem);
        for _ in 0..frames_for(1.2, dt) {
            harness.step(1);
            let off = body_at(&harness, pet) - body_at(&harness, hero) - Vec2::new(-20.0, 0.0);
            assert!(off.x.abs() < 0.5, "dt {dt}: beside the hero: {off}");
            low = low.min(off.y);
            high = high.max(off.y);
            let scale = harness.world().get::<Transform>(gem).unwrap().scale.x;
            small = small.min(scale);
            large = large.max(scale);
        }
        assert!(
            high - low > 2.0 && low > -5.0 && high < 5.0,
            "dt {dt}: {low}..{high}"
        );
        assert!(small >= 0.9 - 1e-4 && large <= 1.1 + 1e-4 && large - small > 0.15);
        assert!(
            body_at(&harness, gem).distance(gem_start) < 1e-3,
            "dt {dt}: a whole period"
        );
        harness.press_action("move_right");
        harness.step(frames_for(0.5, dt));
        let trail = body_at(&harness, hero).x - 20.0 - body_at(&harness, pet).x;
        assert!(trail > 5.0, "dt {dt}: it lags the running hero by {trail}");
    }
}

/// The card's recipe, cues and cue rules alone: the ranger's cooldown fires
/// an ember from it, whose hit kills a ghoul; the combat rule counts the
/// kill and floats the damage, the ghoul's `killed` cues burst, roll its
/// drop and count the kill on the ranger, its kill's other entity, and the
/// ghoul goes the step after its kill.
fn the_card_recipe(plugins: fn() -> PluginSet) {
    for dt in DTS {
        let mut harness = recipe(GENRE, plugins(), dt);
        let ranger = spawn(&mut harness, "ranger", [100.0, 100.0]);
        let ghoul = spawn(&mut harness, "ghoul", [200.0, 100.0]);
        let mut killed = None;
        for _ in 0..frames_for(1.5, dt) {
            harness.step(1);
            killed = harness
                .events::<Damaged>()
                .find(|event| event.entity == ghoul && event.killed)
                .copied();
            if killed.is_some() {
                break;
            }
        }
        let killed = killed.unwrap_or_else(|| panic!("dt {dt}: the ember killed no ghoul"));
        assert_eq!(killed.source, Some(ranger), "dt {dt}: the ember's source");
        assert_eq!(killed.amount, 2.0);
        assert!(
            tagged(&harness, "ember")
                .iter()
                .any(|&ember| harness.world().get::<Source>(ember) == Some(&Source::new(ranger))),
            "dt {dt}: the ember came from the ranger's cue"
        );
        let world = harness.world();
        assert_eq!(
            world.resource::<Counters>().get("kills"),
            1,
            "dt {dt}: the combat rule"
        );
        assert_eq!(
            world.get::<Counters>(ranger).unwrap().get("kills"),
            1,
            "dt {dt}: the kill's other entity"
        );
        let at = Vec2::new(200.0, 100.0);
        for name in ["ghoul_burst", "orb"] {
            let spawned = tagged(&harness, name);
            assert_eq!(spawned.len(), 1, "dt {dt}: one {name}");
            assert!(
                point_of(&harness, spawned[0]).distance(at) < 1e-3,
                "dt {dt}: {name} at the hit"
            );
        }
        assert!(
            labels(&harness).contains(&"2".to_string()),
            "dt {dt}: the damage floated"
        );
        if dt == DTS[0] {
            assert!(world.is_alive(ghoul), "dt {dt}: kept for its cues' step");
        }
        harness.step(1);
        assert!(
            !harness.world().is_alive(ghoul),
            "dt {dt}: gone the step after its kill"
        );
    }
}

#[test]
fn a_cue_driven_kill_counts_bursts_labels_and_rolls_a_drop() {
    the_card_recipe(KitPlugins::set);
}

#[test]
fn the_card_recipe_holds_with_the_overlap_plugins_reversed() {
    the_card_recipe(overlap_reversed);
}

#[test]
fn an_ember_pierces_two_ghouls_and_expires_at_its_range() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let first = spawn(&mut harness, "ghoul", [100.0, 0.0]);
        let second = spawn(&mut harness, "ghoul", [150.0, 0.0]);
        let ember = spawn(&mut harness, "ember", [0.0, 0.0]);
        let (mut struck, mut expired) = (Vec::new(), Vec::new());
        for _ in 0..frames_for(1.0, dt) {
            harness.step(1);
            struck.extend(
                harness
                    .events::<Struck>()
                    .map(|event| (event.entity, event.target)),
            );
            expired.extend(harness.events::<ProjectileExpired>().copied());
        }
        assert_eq!(struck, [(ember, first), (ember, second)], "dt {dt}");
        assert_eq!(expired.len(), 1, "dt {dt}");
        assert!(
            expired[0].position.distance(Vec2::new(400.0, 0.0)) < 1e-3,
            "dt {dt}: at its range: {}",
            expired[0].position
        );
        let world = harness.world();
        assert!(!world.is_alive(first) && !world.is_alive(second) && !world.is_alive(ember));
    }
}

#[test]
fn a_knights_shield_takes_a_hit_then_its_resist_halves_fire() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let knight = spawn(&mut harness, "knight", [200.0, 0.0]);
        harness.step(1);
        assert_eq!(
            damage(harness.world_mut(), knight, 2.0),
            Hit::Absorbed,
            "dt {dt}"
        );
        assert_eq!(
            harness.world().get::<Shield>(knight).unwrap().current(),
            0.0
        );
        spawn(&mut harness, "ember", [0.0, 0.0]);
        let mut amounts = Vec::new();
        for _ in 0..frames_for(0.6, dt) {
            harness.step(1);
            amounts.extend(
                harness
                    .events::<Damaged>()
                    .filter(|event| event.entity == knight)
                    .map(|event| event.amount),
            );
        }
        assert_eq!(
            amounts,
            [2.0, 1.0],
            "dt {dt}: whole into the shield, then fire halved"
        );
        assert_eq!(
            harness.world().get::<Health>(knight).unwrap().current(),
            5.0
        );
    }
}

#[test]
fn an_orb_drifts_to_the_ranger_and_counts_its_xp() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ranger = spawn(&mut harness, "ranger", [0.0, 0.0]);
        let orb = spawn(&mut harness, "orb", [80.0, 0.0]);
        harness.step(1);
        let x = body_at(&harness, orb).x;
        assert!(x < 80.0, "dt {dt}: drifting in: {x}");
        harness.step(frames_for(0.5, dt));
        assert!(!harness.world().is_alive(orb), "dt {dt}: collected");
        assert_eq!(
            harness.world().get::<Counters>(ranger).unwrap().get("xp"),
            1
        );
    }
}

#[test]
fn a_ghouls_touch_hurts_the_ranger_each_interval() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // Touching, the ghoul to the ranger's left, so the ranger's embers
        // fly away from it.
        spawn(&mut harness, "ghoul", [0.0, 0.0]);
        let ranger = spawn(&mut harness, "ranger", [15.0, 0.0]);
        harness.step(frames_for(62.0 / 60.0, dt));
        assert_eq!(
            traced_steps(&harness, ranger, "damaged"),
            [1, 31, 61],
            "dt {dt}"
        );
        assert_eq!(
            harness.world().get::<Health>(ranger).unwrap().current(),
            2.0
        );
    }
}

/// The world labels the last frame drew: each text with its colour, sorted.
fn typed_labels(harness: &Harness) -> Vec<(String, [u8; 4])> {
    let mut drawn: Vec<(String, [u8; 4])> = harness
        .draw()
        .unwrap()
        .world_labels
        .iter()
        .map(|run| (run.content.clone(), run.color))
        .collect();
    drawn.sort();
    drawn
}

/// The parts of `entity`'s hits the last frame sent: each amount with its
/// kinds by name.
fn parts_on(harness: &Harness, entity: Entity) -> Vec<(f32, Vec<String>)> {
    harness
        .events::<DamagedPart>()
        .filter(|part| part.entity == entity)
        .map(|part| {
            let kinds = part.kinds.names(harness.world());
            (part.amount, kinds.into_iter().map(str::to_owned).collect())
        })
        .collect()
}

/// Steps until `entity` is hit, at most `seconds` of game time; the frame
/// that hit it is the last.
fn step_until_hit(harness: &mut Harness, entity: Entity, seconds: f32, dt: f32) {
    for _ in 0..frames_for(seconds, dt) {
        harness.step(1);
        if harness
            .events::<DamagedPart>()
            .any(|part| part.entity == entity)
        {
            return;
        }
    }
    panic!("dt {dt}: no hit within {seconds} s");
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

/// `parts` match `expected` amount for amount, within 1e-4, kinds exactly.
fn parts_match(parts: &[(f32, Vec<String>)], expected: &[(f32, &[&str])]) -> bool {
    parts.len() == expected.len()
        && parts
            .iter()
            .zip(expected)
            .all(|((amount, kinds), (want, names))| {
                close(*amount, *want) && kinds.iter().map(String::as_str).eq(names.iter().copied())
            })
}

const RED: [u8; 4] = [255, 0, 0, 255];
const CYAN: [u8; 4] = [0, 255, 255, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

#[test]
fn a_spellblades_typed_hit_converts_penetrates_resists_and_labels_each_part() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let golem = spawn(&mut harness, "golem", [300.0, 300.0]);
        spawn(&mut harness, "spellblade", [300.0, 300.0]);
        step_until_hit(&mut harness, golem, 0.1, dt);
        // Fire 4 and slash 8, half the slash to cold: fire 4 at 0.8 + 0.25
        // capped at 1 and 0.5 + 0.25, slash 4 at 0.5, cold 4 at 0.8.
        let parts = parts_on(&harness, golem);
        assert!(
            parts_match(
                &parts,
                &[
                    (3.0, &["elemental", "elemental.fire"]),
                    (2.0, &["physical", "physical.slash"]),
                    (3.2, &["elemental", "elemental.cold"]),
                ]
            ),
            "dt {dt}: {parts:?}"
        );
        let hits: Vec<Damaged> = harness
            .events::<Damaged>()
            .filter(|event| event.entity == golem)
            .copied()
            .collect();
        assert_eq!(hits.len(), 1, "dt {dt}: one Damaged");
        assert!(close(hits[0].amount, 8.2), "dt {dt}: {}", hits[0].amount);
        assert_eq!(
            hits[0].kinds.names(harness.world()),
            [
                "elemental",
                "elemental.cold",
                "elemental.fire",
                "physical",
                "physical.slash"
            ]
        );
        let health = harness.world().get::<Health>(golem).unwrap().current();
        assert!(close(health, 11.8), "dt {dt}: {health}");
        assert_eq!(
            harness.world().resource::<Counters>().get("elemental_hits"),
            2,
            "dt {dt}: the rule's `any: [elemental]` passes fire and cold"
        );
        assert_eq!(
            typed_labels(&harness),
            [
                ("2".to_string(), CYAN),
                ("3".to_string(), RED),
                ("3.2".to_string(), BLUE),
                ("8.2".to_string(), WHITE),
            ],
            "dt {dt}: each part in its type's style, the hit in the combat rule's"
        );
    }
}

#[test]
fn a_pyromancers_bolts_take_its_penetration_and_conversion() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let pyromancer = spawn(&mut harness, "pyromancer", [100.0, 300.0]);
        let ward = spawn(&mut harness, "ward", [200.0, 300.0]);
        step_until_hit(&mut harness, ward, 1.5, dt);
        // Its conversion halves fire 8 into toxic 4, to which the ward is
        // immune; fire 4 takes its 0.5 points over the ward's 0.25.
        let parts = parts_on(&harness, ward);
        assert!(
            parts_match(&parts, &[(3.0, &["elemental", "elemental.fire"])]),
            "dt {dt}: {parts:?}"
        );
        let hit = harness
            .events::<Damaged>()
            .find(|event| event.entity == ward)
            .copied()
            .unwrap();
        assert_eq!((hit.amount, hit.source), (3.0, Some(pyromancer)));
        assert_eq!(harness.world().get::<Health>(ward).unwrap().current(), 7.0);
        assert_eq!(
            typed_labels(&harness),
            [("3".to_string(), RED), ("3".to_string(), WHITE)],
            "dt {dt}"
        );
        // Without its components the next bolt deals fire 8 at 0.25.
        let world = harness.world_mut();
        world.remove_component::<Penetration>(pyromancer);
        world.remove_component::<Conversion>(pyromancer);
        harness.step(1);
        step_until_hit(&mut harness, ward, 1.5, dt);
        let parts = parts_on(&harness, ward);
        assert!(
            parts_match(&parts, &[(2.0, &["elemental", "elemental.fire"])]),
            "dt {dt}: {parts:?}"
        );
    }
}

// M67: effects, abilities, levels and ailments.

fn cold(target: Entity, amount: f32) -> DamageRequest {
    DamageRequest::from_parts(
        target,
        [DamagePart::new(amount).with_tags(Tags::new(["elemental.cold"]))],
    )
}

fn has_effect(harness: &Harness, entity: Entity, id: &str) -> bool {
    harness
        .world()
        .get::<Effects>(entity)
        .is_some_and(|effects| effects.has(id))
}

#[test]
fn a_burning_hit_slows_the_slime_and_its_fire_ticks_kill_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let slime = spawn(&mut harness, "slime", [50.0, 50.0]);
        let torch = harness.world_mut().spawn();
        harness.step(1);
        // A fire part on the flammable slime: the elements rule's `effect`
        // action queues `burning` from the hitter, which lands the step
        // after the cue.
        DamageRequest::from_parts(
            slime,
            [DamagePart::new(0.5).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_source(torch)
        .apply(harness.world_mut());
        harness.step(2);
        let world = harness.world();
        assert!(has_effect(&harness, slime, "burning"), "dt {dt}");
        let instance = world
            .get::<Effects>(slime)
            .unwrap()
            .instances()
            .next()
            .unwrap();
        assert_eq!(instance.source(), Some(torch));
        assert_eq!(
            world.get::<Stats>(slime).unwrap().get("move_speed"),
            Some(45.0),
            "dt {dt}: a quarter slower"
        );
        assert!(world.get::<Tags>(slime).unwrap().has("status.burning"));
        assert_eq!(world.get::<Health>(slime).unwrap().current(), 1.5);
        // Fire ticks every half second: the second one kills, which the
        // combat rule counts.
        harness.step(frames_for(1.1, dt));
        let world = harness.world();
        assert!(world.get::<Health>(slime).unwrap().is_dead(), "dt {dt}");
        assert_eq!(world.resource::<Counters>().get("kills"), 1, "dt {dt}");
        let ticks: Vec<f32> = world
            .resource::<crate::kit::KitTrace>()
            .about(slime)
            .filter_map(|entry| match entry.kind {
                TraceKind::Event {
                    name: "damaged",
                    amount,
                    ..
                } => amount,
                _ => None,
            })
            .collect();
        assert_eq!(ticks, [0.5, 1.0, 1.0], "dt {dt}: the hit, then two ticks");
    }
}

#[test]
fn a_practice_dummys_burn_ticks_and_a_blow_pass_their_rules_apart() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let dummy = spawn(&mut harness, "practice_dummy", [50.0, 50.0]);
        let torch = harness.world_mut().spawn();
        harness.step(1);
        // One blow, then a burn: the `ticks` rules count each apart.
        damage(harness.world_mut(), dummy, 1.0);
        EffectRequest::new(dummy, DataRef::by_id("burning"))
            .with_source(torch)
            .apply(harness.world_mut());
        harness.step(frames_for(4.0, dt));
        let counters = harness.world().resource::<Counters>();
        assert_eq!(
            (counters.get("blows"), counters.get("ticks")),
            (1, 6),
            "dt {dt}: the blow, then a tick each 0.5 s over the burn's 3 s"
        );
    }
}

#[test]
fn a_campfires_crackle_rises_and_pans_as_the_hero_walks_past() {
    use crate::kit::CueListener;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hero = spawn(&mut harness, "hero", [0.0, 0.0]);
        let fire = spawn(&mut harness, "campfire", [300.0, 0.0]);
        harness.world_mut().insert_resource(
            CueListener::new(50.0, 400.0)
                .with_pan(200.0)
                .following(hero),
        );
        harness.step(1);
        // The fire's burn starts its loop; the hero walks right, past it.
        EffectRequest::new(fire, DataRef::by_id("burning")).apply(harness.world_mut());
        harness.press_action("move_right");
        let (mut plays, mut volumes, mut pans, mut stops) = (0, Vec::new(), Vec::new(), Vec::new());
        for _ in 0..frames_for(3.4, dt) {
            harness.step(1);
            for command in harness.audio() {
                match command {
                    AudioCommand::Play {
                        looping: true,
                        volume,
                        ..
                    } => {
                        plays += 1;
                        volumes.push(*volume);
                    }
                    AudioCommand::SetVoiceVolume { volume, .. } => volumes.push(*volume),
                    AudioCommand::SetVoicePan { pan, .. } => pans.push(pan.get()),
                    AudioCommand::StopVoice { fade, .. } => stops.push(*fade),
                    _ => {}
                }
            }
        }
        assert_eq!(plays, 1, "dt {dt}: one voice");
        assert!(volumes[0] < 0.2, "dt {dt}: far at first: {volumes:?}");
        let loudest = volumes.iter().copied().fold(0.0, f32::max);
        assert!(
            loudest > 1.0 - 1.0 / 64.0,
            "dt {dt}: whole beside it, within a command's step: {loudest}"
        );
        let peak = volumes.iter().position(|&v| v == loudest).unwrap();
        assert!(
            volumes[..=peak].windows(2).all(|w| w[1] >= w[0]),
            "dt {dt}: rising as it nears: {volumes:?}"
        );
        assert!(pans[0] > 0.5, "dt {dt}: the fire to its right: {pans:?}");
        assert!(
            pans.windows(2).all(|w| w[1] <= w[0]) && *pans.last().unwrap() < 0.0,
            "dt {dt}: panning across as it passes: {pans:?}"
        );
        assert_eq!(stops, [0.5], "dt {dt}: the burn's end stops it");
    }
}

/// A recipe's storm: the `thunder` world cue played at (200, 0) by name
/// each game second.
struct Storm;

impl Plugin for Storm {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        let mut clock = 0.0_f32;
        let storm = system("recipe_storm", move |world: &mut World| {
            clock += world.resource::<tungsten_core::Time>().delta();
            if clock >= 1.0 - 1e-4 {
                clock -= 1.0;
                crate::kit::WorldCue::new("thunder", Vec2::new(200.0, 0.0)).play(world);
            }
        });
        schedule.add(
            tungsten_core::Stage::FixedUpdate,
            Phase::React.join(storm.before(crate::kit::CUES)),
        );
    }
}

#[test]
fn a_storms_thunder_world_cue_strikes_each_second_by_its_falloff() {
    use crate::kit::{CueListener, KitTrace};
    use tungsten_core::ShakeEvent;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set().with(Storm), dt);
        let hero = spawn(&mut harness, "hero", [0.0, 0.0]);
        harness
            .world_mut()
            .insert_resource(CueListener::new(100.0, 600.0).following(hero));
        let mut shakes = Vec::new();
        for _ in 0..frames_for(2.5, dt) {
            harness.step(1);
            shakes.extend(
                harness
                    .world()
                    .resource::<EventQueue<ShakeEvent>>()
                    .iter()
                    .map(|shake| shake.trauma_add),
            );
        }
        let world = harness.world();
        assert_eq!(world.resource::<Counters>().get("strikes"), 2, "dt {dt}");
        let blasts = world
            .resource::<KitTrace>()
            .about(Entity::PLACEHOLDER)
            .filter(
                |entry| matches!(&entry.kind, TraceKind::Spawn { prefab, .. } if prefab == "blast"),
            )
            .count();
        assert_eq!(blasts, 2, "dt {dt}: a blast each, with no source");
        assert_eq!(shakes.len(), 2, "dt {dt}");
        assert!(
            shakes.iter().all(|&trauma| (trauma - 0.48).abs() < 1e-5),
            "dt {dt}: 200 px off, near 100, far 600: {shakes:?}"
        );
    }
}

#[test]
fn a_shieldbearers_whole_blocks_flash_its_shield_colour_until_it_breaks() {
    use tungsten_core::UniformOverrideBlock;

    const BLUE: [f32; 4] = [0.25, 0.5, 1.0, 1.0];
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let bearer = spawn(&mut harness, "shieldbearer", [100.0, 100.0]);
        harness.step(1);
        let mut seen = Vec::new();
        for _ in 0..3 {
            damage(harness.world_mut(), bearer, 20.0);
            harness.step(1);
            let world = harness.world();
            seen.push((
                world.get::<UniformOverrideBlock>(bearer).unwrap().vec4[0],
                world.resource::<Counters>().get("blocked"),
                world.get::<Health>(bearer).unwrap().current(),
            ));
            harness.step(frames_for(0.3, dt));
        }
        assert_eq!(
            seen,
            [(BLUE, 1, 100.0), (BLUE, 2, 100.0), ([1.0; 4], 2, 90.0)],
            "dt {dt}: two whole blocks, then the shield breaks under the third"
        );
    }
}

#[test]
fn a_kindled_lanterns_tint_rides_its_colour_tween_and_glows_under_it() {
    use tungsten_core::{Easing, MaterialRegistry, Tween, TweenChannel, Visibility};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let sprite = super::register_sprite(&mut harness, "lantern", 16, 16);
        if harness
            .world()
            .resource::<tungsten_core::AssetRegistry>()
            .get_sprite("engine_white")
            .is_none()
        {
            super::register_sprite(&mut harness, "engine_white", 1, 1);
        }
        let tween = || {
            Tween::new(2.0, Easing::Linear).with_channel(TweenChannel::ColorG { from: 255, to: 0 })
        };
        // The lantern and a control on the same tween, without the look.
        let spawn = |harness: &mut Harness, x: f32| {
            harness.world_mut().spawn_with((
                Transform::from_position(Vec2::new(x, 100.0)),
                Sprite::new(sprite),
                Visibility::default(),
                tween(),
            ))
        };
        let lantern = spawn(&mut harness, 100.0);
        spawn(&mut harness, 300.0);
        EffectRequest::new(lantern, DataRef::by_id("kindled")).apply(harness.world_mut());
        let glow = harness
            .world()
            .resource::<MaterialRegistry>()
            .get("engine_glow");
        let (mut lit_frames, mut after) = (0, 0);
        for _ in 0..frames_for(2.0, dt) {
            harness.step(1);
            let draw = harness.draw().unwrap();
            let at = |x: f32| {
                draw.sprites
                    .iter()
                    .filter(|batch| batch.material_id.is_none())
                    .flat_map(|batch| &batch.instances)
                    .find(|instance| instance.position == [x, 100.0])
                    .map(|instance| instance.color)
                    .unwrap()
            };
            let (lit, base) = (at(100.0), at(300.0));
            let glows: Vec<_> = draw
                .sprites
                .iter()
                .filter(|batch| batch.material_id == glow)
                .flat_map(|batch| &batch.instances)
                .collect();
            if has_effect(&harness, lantern, "kindled") {
                let tinted = [
                    base[0],
                    (f32::from(base[1]) * 0.5).round() as u8,
                    (f32::from(base[2]) * 0.25).round() as u8,
                    base[3],
                ];
                assert_eq!(lit, tinted, "dt {dt}: the tween's colour taken to the tint");
                assert_eq!(glows.len(), 1, "dt {dt}");
                assert_eq!(
                    glows[0].position,
                    [96.0, 96.0],
                    "dt {dt}: 12 px about its centre"
                );
                lit_frames += 1;
            } else {
                assert_eq!(lit, base, "dt {dt}: the tween's colour stands");
                assert!(glows.is_empty(), "dt {dt}");
                after += 1;
            }
        }
        assert!(
            lit_frames > 0 && after > 0,
            "dt {dt}: {lit_frames} and {after}"
        );
    }
}

#[test]
fn a_blazing_crates_light_reaches_a_lurker_alike_at_any_frame_rate_and_crackles() {
    let mut taken = Vec::new();
    for dt in [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0] {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let crate_ = spawn(&mut harness, "crate", [100.0, 100.0]);
        let lurker = spawn(&mut harness, "lurker", [150.0, 100.0]);
        EffectRequest::new(crate_, DataRef::by_id("blazing")).apply(harness.world_mut());
        let (mut plays, mut stops) = (0, 0);
        for _ in 0..frames_for(3.5, dt) {
            harness.step(1);
            for command in harness.audio() {
                match command {
                    AudioCommand::Play { looping: true, .. } => plays += 1,
                    AudioCommand::StopVoice { .. } => stops += 1,
                    _ => {}
                }
            }
        }
        assert_eq!(
            (plays, stops),
            (1, 1),
            "dt {dt}: one crackle, ended with the blaze"
        );
        taken.push(traced_steps(&harness, lurker, "effect_applied"));
    }
    assert!(!taken[0].is_empty(), "the light reached it: {:?}", taken[0]);
    assert_eq!(taken[0], taken[1], "dt 1/30, two fixed steps a frame");
    assert_eq!(taken[0], taken[2], "dt 1/144");
}

/// The game's chest event: a coffer opened, or emptied by its last take.
struct ChestEvent {
    chest: Entity,
    emptied: bool,
}

#[allow(clippy::unnecessary_wraps)]
fn chest_event(event: &ChestEvent) -> Option<crate::kit::CueEvent> {
    let name = if event.emptied { "emptied" } else { "opened" };
    Some(crate::kit::CueEvent::of(event.chest).named(name))
}

#[test]
fn a_coffers_one_registration_brings_opened_and_emptied_in_by_name() {
    use crate::kit::{CueAction, Cues, CuesOnEvent};

    for dt in DTS {
        let plugins = KitPlugins::set().with(CuesOnEvent::<ChestEvent>::with_names(
            &["opened", "emptied"],
            chest_event,
        ));
        let mut harness = recipe(GENRE, plugins, dt);
        let coffer = harness.world_mut().spawn_with((
            Transform::from_position(Vec2::new(40.0, 60.0)),
            Cues::new()
                .on("opened", [CueAction::counter_add("opened", 1).for_game()])
                .on(
                    "emptied",
                    [
                        CueAction::counter_add("emptied", 1).for_game(),
                        CueAction::despawn(),
                    ],
                ),
        ));
        harness.step(1);
        for emptied in [false, false, true] {
            harness
                .world_mut()
                .resource_mut::<EventQueue<ChestEvent>>()
                .send(ChestEvent {
                    chest: coffer,
                    emptied,
                });
        }
        harness.step(1);
        let counters = harness.world().resource::<Counters>();
        assert_eq!(
            (counters.get("opened"), counters.get("emptied")),
            (2, 1),
            "dt {dt}"
        );
        assert!(!harness.world().is_alive(coffer), "dt {dt}: emptied");
    }
}

#[test]
fn the_heros_wand_bolts_the_nearest_slime_on_its_interval_and_its_dash_pushes_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hero = spawn(&mut harness, "hero", [100.0, 0.0]);
        let near = spawn(&mut harness, "slime", [200.0, 0.0]);
        spawn(&mut harness, "slime", [100.0, 300.0]);
        harness.step(frames_for(0.45, dt));
        assert!(tagged(&harness, "ember").is_empty(), "dt {dt}: not yet");
        harness.step(frames_for(0.1, dt));
        let embers = tagged(&harness, "ember");
        assert_eq!(embers.len(), 1, "dt {dt}: one on the interval");
        let world = harness.world();
        let velocity = world.get::<LinearMotion>(embers[0]).unwrap().velocity();
        assert!(
            (velocity - Vec2::new(420.0, 0.0)).length() < 1e-3,
            "dt {dt}: toward the near slime at the wand's speed: {velocity}"
        );
        assert_eq!(world.get::<Source>(embers[0]), Some(&Source::new(hero)));
        let used = traced_steps(&harness, hero, "ability_used");
        assert_eq!(used.len(), 1);
        let _ = near;
        // The dash: the hero moving right aims right, and the dash slot
        // pushes it that way through its knockback.
        harness.press_action("move_right");
        harness.step(1);
        harness.press_action("dash");
        harness.step(1);
        harness.release_action("dash");
        // The push falls to 0 over the knockback's 0.2 s: a step in, most
        // of its 600 is left, along the aim.
        let push = harness.world().get::<Knockback>(hero).unwrap().push();
        assert!(push.x > 400.0 && push.y.abs() < 1e-3, "dt {dt}: {push}");
        let x = body_at(&harness, hero).x;
        harness.step(frames_for(0.2, dt));
        assert!(
            body_at(&harness, hero).x > x + 50.0,
            "dt {dt}: thrown forward"
        );
        assert!(
            !harness
                .world()
                .get::<Abilities>(hero)
                .unwrap()
                .ready("dash"),
            "dt {dt}: on cooldown"
        );
    }
}

#[test]
fn the_rangers_volley_empties_its_charges_and_an_ammo_orb_refills_them() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ranger = spawn(&mut harness, "ranger", [0.0, 0.0]);
        spawn(&mut harness, "golem", [300.0, 0.0]);
        harness.step(1);
        for _ in 0..4 {
            harness.press_action("attack");
            harness.step(1);
            harness.release_action("attack");
            harness.step(1);
        }
        assert_eq!(
            traced_steps(&harness, ranger, "ability_used").len(),
            3,
            "dt {dt}: three charges"
        );
        let abilities = harness.world().get::<Abilities>(ranger).unwrap();
        assert_eq!(abilities.charges("volley"), Some(0));
        assert!(!abilities.ready("volley"));
        let embers = tagged(&harness, "ember");
        assert_eq!(embers.len(), 3, "dt {dt}");
        for ember in embers {
            let velocity = harness
                .world()
                .get::<LinearMotion>(ember)
                .unwrap()
                .velocity();
            assert!(
                (velocity - Vec2::new(500.0, 0.0)).length() < 1e-3,
                "dt {dt}"
            );
        }
        // The ammo on the ranger: its `picked_up` cue refills the
        // collector's volley.
        let ammo = spawn(&mut harness, "ammo", [0.0, 0.0]);
        harness.step(2);
        assert!(!harness.world().is_alive(ammo), "dt {dt}: collected");
        let abilities = harness.world().get::<Abilities>(ranger).unwrap();
        assert!(abilities.ready("volley"), "dt {dt}");
        assert_eq!(abilities.charges("volley"), Some(3));
        harness.press_action("attack");
        harness.step(1);
        harness.release_action("attack");
        assert_eq!(traced_steps(&harness, ranger, "ability_used").len(), 4);
    }
}

#[test]
fn the_ranger_levels_on_gems_and_a_chosen_offer_grants_swift() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ranger = spawn(&mut harness, "ranger", [0.0, 0.0]);
        harness.step(1);
        assert_eq!(harness.world().get::<Level>(ranger).unwrap().level(), 1);
        // The orbs' xp, as their cues count it: enough for the next level,
        // then the next, until the draw offers `swift`.
        let mut swift_at = None;
        for (level, xp) in [(2, 10), (3, 25), (4, 50)] {
            harness
                .world_mut()
                .get_mut::<Counters>(ranger)
                .unwrap()
                .set("xp", xp);
            harness.step(1);
            let found = harness.world().get::<Level>(ranger).unwrap();
            assert_eq!(found.level(), level, "dt {dt}");
            assert_eq!(found.offers().len(), 2, "dt {dt}: two of three");
            swift_at = found
                .offers()
                .iter()
                .position(|offer| offer.effect().and_then(|e| e.id()) == Some("swift"));
            if swift_at.is_some() {
                break;
            }
        }
        let at = swift_at.expect("swift was offered by the fourth level");
        assert!(
            harness
                .world_mut()
                .get_mut::<Level>(ranger)
                .unwrap()
                .choose(at)
        );
        harness.step(2);
        assert!(has_effect(&harness, ranger, "swift"), "dt {dt}");
        assert!(
            harness
                .world()
                .get::<Level>(ranger)
                .unwrap()
                .offers()
                .is_empty(),
            "dt {dt}: the choice cleared the offers"
        );
        let ups = traced_steps(&harness, ranger, "level_up");
        assert!(!ups.is_empty());
    }
}

#[test]
fn cold_hits_chill_then_freeze_the_golem_and_a_thawed_golem_resists_the_next_freeze() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let rival = harness.world_mut().spawn();
        let golem = spawn(&mut harness, "golem", [0.0, 0.0]);
        let mage = harness.world_mut().spawn();
        harness.step(1);
        // The golem's `status_threshold` is 6; a hit of 8 lands 6.4 after its
        // resistance: a full chill build chills, its band's effect at
        // chemistry's run after the hit's, the next hit's freeze build
        // freezes, and three such hits leave the golem of 20 alive.
        let two_steps = frames_for(2.0 / 60.0, dt);
        cold(golem, 8.0)
            .with_source(mage)
            .apply(harness.world_mut());
        harness.step(two_steps);
        assert!(has_effect(&harness, golem, "chilled") && !has_effect(&harness, golem, "frozen"));
        cold(golem, 8.0)
            .with_source(mage)
            .apply(harness.world_mut());
        harness.step(two_steps);
        assert!(has_effect(&harness, golem, "frozen"), "dt {dt}");
        // Up to the step the freeze expires on; that step, the rival, of a
        // lower index than the mage, lands another full build, whose
        // freeze its band applies the step after, under the thaw.
        loop {
            let left = harness
                .world()
                .get::<Effects>(golem)
                .unwrap()
                .instances()
                .find(|i| i.effect().id() == Some("frozen"))
                .unwrap()
                .left()
                .unwrap();
            if left <= dt * 1.001 {
                break;
            }
            harness.step(1);
        }
        assert!(
            cold(golem, 8.0)
                .with_source(rival)
                .queue(harness.world_mut())
        );
        harness.step(two_steps);
        let world = harness.world();
        let effects = world.get::<Effects>(golem).unwrap();
        assert!(effects.has("thawed") && !effects.has("frozen"), "dt {dt}");
        assert!(effects.has("chilled"), "dt {dt}: the chill still runs");
        let tags = world.get::<Tags>(golem).unwrap();
        assert!(tags.has("status.thawed") && !tags.has("status.frozen"));
        let blocked = world
            .resource::<crate::kit::KitTrace>()
            .about(golem)
            .filter(|entry| {
                matches!(
                    entry.kind,
                    TraceKind::Skipped {
                        event: "effect",
                        action: "apply",
                        ..
                    }
                )
            })
            .count();
        assert_eq!(blocked, 1, "dt {dt}: the rival's freeze was refused");
    }
}

#[test]
fn a_toxic_hit_poisons_a_susceptible_ghoul_whatever_the_seed() {
    // 7.13a: the ladder's chance went with it; the rule applies its poison
    // on each hit, so a seed changes nothing.
    for dt in DTS {
        let run = |seed: u64| -> Vec<bool> {
            let mut harness = recipe(GENRE, KitPlugins::set(), dt);
            harness.world_mut().insert_resource(KitRng::seeded(seed));
            let ghouls: Vec<Entity> = (0..8)
                .map(|i| spawn(&mut harness, "ghoul", [i as f32 * 100.0, -500.0]))
                .collect();
            harness.step(1);
            for &ghoul in &ghouls {
                DamageRequest::from_parts(
                    ghoul,
                    [DamagePart::new(0.5).with_tags(Tags::new(["toxic"]))],
                )
                .apply(harness.world_mut());
            }
            harness.step(1);
            ghouls
                .iter()
                .map(|&ghoul| has_effect(&harness, ghoul, "poisoned"))
                .collect()
        };
        let a = run(21);
        assert_eq!(a, run(22), "dt {dt}: no draw");
        assert!(a.iter().all(|&p| p), "dt {dt}: {a:?}");
    }
}

// M72: stamina that sprinting pauses.

#[test]
fn a_sprinters_stamina_regenerates_and_a_sprint_spends_and_pauses_it() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let sprinter = spawn(&mut harness, "sprinter", [0.0, 0.0]);
        harness.step(2);
        let stamina = |harness: &Harness| {
            let stats = harness.world().get::<Stats>(sprinter).unwrap();
            stats.base("stamina").unwrap()
        };
        // 2 each 0.1 s: 20 a second.
        harness.step(frames_for(1.0, dt));
        assert!(
            (stamina(&harness) - 70.0).abs() <= 2.0,
            "dt {dt}: {}",
            stamina(&harness)
        );
        harness.press_action("move_right");
        harness.press_action("dash");
        harness.step(1);
        harness.release_action("dash");
        harness.step(1);
        let sprint = stamina(&harness);
        assert!(sprint < 62.0, "dt {dt}: the sprint cost 10: {sprint}");
        let speed = harness.world().get::<Velocity>(sprinter).unwrap().0.x;
        assert!(
            (speed - 180.0).abs() < 1e-3,
            "dt {dt}: half again as fast: {speed}"
        );
        harness.step(frames_for(0.4, dt));
        assert_eq!(stamina(&harness), sprint, "dt {dt}: held while sprinting");
        harness.step(frames_for(0.5, dt));
        assert!(stamina(&harness) > sprint + 3.0, "dt {dt}: it resumed");
        harness.step(frames_for(4.0, dt));
        assert_eq!(stamina(&harness), 100.0, "dt {dt}: capped at max_stamina");
    }
}

// M74: an effect's source named after a load, a rung a tag skips, and a
// game's own damage numbers.

/// A game's save of one target's effects, with the game's own ID for their
/// source: the serials the pyromancer applied.
#[derive(serde::Serialize, serde::Deserialize)]
struct Burned {
    effects: Effects,
    from_pyromancer: Vec<u64>,
}

impl SaveData for Burned {
    const SCHEMA: u32 = 1;
}

#[test]
fn a_burn_named_after_a_load_credits_the_pyromancer() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // Its bolts fly right along its own row, clear of the golem.
        let pyromancer = spawn(&mut harness, "pyromancer", [100.0, 1000.0]);
        let golem = spawn(&mut harness, "golem", [0.0, 0.0]);
        harness.step(1);
        EffectRequest::new(golem, DataRef::by_id("burning"))
            .with_source(pyromancer)
            .apply(harness.world_mut());
        // The game's save holds the effects and its own ID for the source.
        let effects = harness.world().get::<Effects>(golem).unwrap().clone();
        let from_pyromancer = effects
            .instances()
            .filter(|instance| instance.source() == Some(pyromancer))
            .map(EffectInstance::serial)
            .collect();
        let dir = std::env::temp_dir().join(format!(
            "tungsten_recipe_burn_{}_{}",
            std::process::id(),
            frames_for(1.0, dt)
        ));
        let slots = SaveSlots::new(Some(dir.clone()));
        slots
            .save(
                "burn",
                &Burned {
                    effects,
                    from_pyromancer,
                },
            )
            .unwrap();
        let loaded: Burned = slots.load("burn").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // A save holds no entity: its load names the source again.
        let mut effects = loaded.effects;
        assert!(
            effects
                .instances()
                .all(|instance| instance.source().is_none()),
            "dt {dt}"
        );
        for serial in loaded.from_pyromancer {
            assert!(effects.set_source(serial, Some(pyromancer)), "dt {dt}");
        }
        harness.world_mut().insert(golem, effects);
        let mut ticks = Vec::new();
        for _ in 0..frames_for(1.1, dt) {
            harness.step(1);
            ticks.extend(
                harness
                    .events::<Damaged>()
                    .filter(|hit| hit.entity == golem && hit.periodic)
                    .map(|hit| hit.source),
            );
        }
        assert_eq!(
            ticks,
            [Some(pyromancer); 2],
            "dt {dt}: a fire tick each half second, each the pyromancer's"
        );
    }
}

#[test]
fn a_steady_frost_stream_chills_then_freezes_the_golem() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let golem = spawn(&mut harness, "golem", [0.0, 0.0]);
        harness.step(1);
        // A spray's pulses, 1 a frame at the golem's 0.8: the chilled rung
        // takes the bucket at half the threshold of 6, then its
        // `blocked_by` skips it while the golem is chilled, so the cold
        // fills on to the frozen rung's whole threshold.
        let (mut chilled_at, mut frozen_at) = (None, None);
        for frame in 0..frames_for(1.0, dt) {
            cold(golem, 1.25).queue(harness.world_mut());
            harness.step(1);
            if chilled_at.is_none() && has_effect(&harness, golem, "chilled") {
                chilled_at = Some(frame);
            }
            if has_effect(&harness, golem, "frozen") {
                frozen_at = Some(frame);
                break;
            }
        }
        let chilled_at = chilled_at.expect("chilled");
        let frozen_at = frozen_at.unwrap_or_else(|| panic!("dt {dt}: never frozen"));
        assert!(
            (3..5).contains(&chilled_at) && (6..9).contains(&(frozen_at - chilled_at)),
            "dt {dt}: three pulses to chill, six more to freeze: {chilled_at}, {frozen_at}"
        );
        assert!(has_effect(&harness, golem, "chilled"), "dt {dt}: it held");
    }
}

/// Gold, the game's crit colour.
const GOLD: [u8; 4] = [255, 218, 0, 255];

/// A game's own damage numbers, as its Rust floats them in the react
/// phase: each part in its type's style, but one target's crit parts in a
/// step summed into one gold number, larger, where the first landed.
struct GameNumbers;

impl Plugin for GameNumbers {
    fn build(&self, schedule: &mut Schedule, _: &mut World) {
        let mut cursor = EventCursor::<DamagedPart>::default();
        let gold = LabelStyle::new("sans")
            .with_size(24.0)
            .with_color([1.0, 0.7, 0.0, 1.0]);
        let run = move |world: &mut World| {
            let Some(queue) = world.get_resource::<EventQueue<DamagedPart>>() else {
                return;
            };
            let parts: Vec<DamagedPart> = cursor.read(queue).copied().collect();
            let mut crits: Vec<(Entity, Vec2, f32)> = Vec::new();
            for part in parts {
                let at = part.position.unwrap_or_default();
                if part.crit {
                    match crits.iter_mut().find(|(entity, ..)| *entity == part.entity) {
                        Some((_, _, sum)) => *sum += part.amount,
                        None => crits.push((part.entity, at, part.amount)),
                    }
                } else if let Some(style) = part.kinds.label_style(world) {
                    float_label(world, &style, at, part.amount);
                }
            }
            for (_, at, sum) in crits {
                float_label(world, &gold, at, sum);
            }
        };
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(system("game_numbers", run)),
        );
    }
}

fn typed_part(amount: f32, kind: &str) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new([kind]))
}

#[test]
fn a_crit_floats_gold_and_larger_from_the_games_code() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set().with(GameNumbers), dt);
        // No cues of its own: the game's numbers alone.
        let target = harness.world_mut().spawn_with((Health::new(100.0),));
        let pyromancer = harness.world_mut().spawn();
        harness.step(1);
        // One step: a crit of fire 4 and cold 2, and a plain fire 3.
        let world = harness.world_mut();
        DamageRequest::from_parts(
            target,
            [
                typed_part(4.0, "elemental.fire"),
                typed_part(2.0, "elemental.cold"),
            ],
        )
        .with_source(pyromancer)
        .with_position(Vec2::new(40.0, 20.0))
        .with_crit(true)
        .queue(world);
        DamageRequest::from_parts(target, [typed_part(3.0, "elemental.fire")])
            .with_position(Vec2::new(80.0, 20.0))
            .queue(world);
        harness.step(1);
        assert_eq!(
            typed_labels(&harness),
            [("3".to_string(), RED), ("6".to_string(), GOLD)],
            "dt {dt}: the plain part in fire's style, the crit's two summed in gold"
        );
        let mut sizes: Vec<(String, f32)> = harness
            .draw()
            .unwrap()
            .world_labels
            .iter()
            .map(|run| (run.content.clone(), run.font_size))
            .collect();
        sizes.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            sizes,
            [("3".to_string(), 16.0), ("6".to_string(), 24.0)],
            "dt {dt}"
        );
    }
}

/// Each label a frame drew that no earlier frame drew, appended to `seen`
/// as text, size and opaque colour: the floats in their order.
fn note_labels(harness: &Harness, seen: &mut Vec<(String, f32, [u8; 4])>) {
    for run in &harness.draw().unwrap().world_labels {
        // Its colour as floated: the fade leaves the RGB.
        let [r, g, b, _] = run.color;
        let label = (run.content.clone(), run.font_size, [r, g, b, 255]);
        if !seen.contains(&label) {
            seen.push(label);
        }
    }
}

#[test]
fn a_numbered_targets_numbers_merge_tick_and_crit_by_its_cue_rule() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // `numbers.json`: merge over 0.25 s, ticks at 10, crits gold at 24.
        let target = harness
            .world_mut()
            .spawn_with((Health::new(100.0), Tags::new(["numbered"])));
        harness.step(1);
        let mut seen = Vec::new();
        let frames: [&[(f32, bool, bool)]; 3] = [
            &[(4.0, false, false), (1.0, true, false)],
            &[(2.0, false, false), (1.0, true, false), (3.0, false, true)],
            &[(1.5, false, false), (1.0, true, false)],
        ];
        for frame in frames {
            let world = harness.world_mut();
            for &(amount, periodic, crit) in frame {
                let mut parts = vec![typed_part(amount, "elemental.fire")];
                if crit {
                    parts.push(typed_part(2.0, "elemental.cold"));
                }
                DamageRequest::from_parts(target, parts)
                    .with_position(Vec2::new(40.0, 20.0))
                    .with_periodic(periodic)
                    .with_crit(crit)
                    .queue(world);
            }
            harness.step(1);
            note_labels(&harness, &mut seen);
        }
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            note_labels(&harness, &mut seen);
        }
        let label = |text: &str, size: f32, color: [u8; 4]| (text.to_string(), size, color);
        assert_eq!(
            seen,
            [
                label("4", 16.0, RED),
                label("5", 24.0, GOLD),
                label("3.5", 16.0, RED),
                label("3", 10.0, RED),
            ],
            "dt {dt}: fire's first at once, the crit's fire and cold as one, then the window's plain parts and its ticks"
        );
    }
}

// M75: a guard saved mid-swing: its state machine, its sword's activation,
// its spiked shield's touches and its mover's turn, the targets named again
// after the load by the game's own IDs.

/// The dts M75's save recipes run at: one step a frame, two, and under one.
const SAVE_DTS: [f32; 3] = [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0];

/// A guard's states: its patrol, its swing, which turns it down and opens
/// its sword's window, and its rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Guard {
    Patrol,
    Swing,
    Rest,
}

fn guard_patrol(world: &mut World, guard: Entity) {
    let machine = world.get_mut::<StateMachine<Guard>>(guard).unwrap();
    if machine.elapsed() > 0.24 {
        machine.go(Guard::Swing);
    }
}

fn guard_swing(world: &mut World, guard: Entity) {
    world
        .get_mut::<TopDownMover>(guard)
        .unwrap()
        .set_direction(Vec2::Y);
    world.get_mut::<Hitbox>(guard).unwrap().arm();
}

fn guard_rest(world: &mut World, _: Entity) {
    world.resource_mut::<GuardTrail>().rests += 1;
}

/// One fixed step of the guard, after the kit's react phase.
#[derive(Debug, Clone, PartialEq)]
struct GuardStep {
    /// State, previous, pending, held and elapsed.
    machine: (Guard, Option<Guard>, Option<Guard>, bool, f32),
    /// The step's hits: (hitter, target).
    struck: Vec<(Entity, Entity)>,
    at: Vec2,
    velocity: Vec2,
}

#[derive(Default)]
struct GuardTrail {
    steps: Vec<GuardStep>,
    rests: u32,
}

/// The game's guard: its machine's hooks and its trail.
struct GuardGame;

impl Plugin for GuardGame {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        StateMachinePlugin::<Guard>::new("guard_states")
            .on_update(Guard::Patrol, guard_patrol)
            .on_enter(Guard::Swing, guard_swing)
            .on_enter(Guard::Rest, guard_rest)
            .build(schedule, world);
        world.insert_resource(GuardTrail::default());
        let mut cursor = EventCursor::<Struck>::default();
        let run = move |world: &mut World| {
            let struck = world
                .get_resource::<EventQueue<Struck>>()
                .map(|queue| {
                    cursor
                        .read(queue)
                        .map(|hit| (hit.entity, hit.target))
                        .collect()
                })
                .unwrap_or_default();
            let Some((guard, machine)) = world
                .query::<(Entity, &StateMachine<Guard>)>()
                .next()
                .map(|(guard, m)| {
                    let at = (
                        m.state(),
                        m.previous(),
                        m.pending(),
                        m.is_held(),
                        m.elapsed(),
                    );
                    (guard, at)
                })
            else {
                return;
            };
            let at = world.get::<Position>(guard).unwrap().0;
            let velocity = world.get::<Velocity>(guard).unwrap().0;
            world.resource_mut::<GuardTrail>().steps.push(GuardStep {
                machine,
                struck,
                at,
                velocity,
            });
        };
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(system("guard_trail", run)),
        );
    }
}

/// The game's save of its guard: the four kit items, and its own IDs, an
/// index into its targets, for the entities they name.
#[derive(serde::Serialize, serde::Deserialize)]
struct GuardSave {
    machine: StateMachine<Guard>,
    mover: TopDownMover,
    sword: Hitbox,
    sword_hit: Vec<usize>,
    shield: ContactDamage,
    shield_touches: Vec<(usize, f32, f32)>,
}

impl SaveData for GuardSave {
    const SCHEMA: u32 = 1;
}

/// A guard run at `dt` from its save point on: its trail, its rests and
/// where it was at the save. With `load`, the game saves it there and
/// loads it back, naming its targets again or not.
fn guard_run(dt: f32, load: Option<bool>) -> (Vec<GuardStep>, u32, Vec2) {
    let mut harness = recipe(GENRE, KitPlugins::set().with(GuardGame), dt);
    let world = harness.world_mut();
    let player = TagFilter::new().all(["team.player"]);
    let mut mover = TopDownMover::new(40.0).with_acceleration(100.0);
    mover.set_direction(Vec2::X);
    let guard = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(10.0))).with((
            PrevPosition(Vec2::ZERO),
            Transform::from_position(Vec2::ZERO),
            Tags::new(["team.enemy"]),
            mover,
            Hitbox::new(HitShape::circle(30.0), 1.0)
                .with_window(0.6)
                .with_filter(player.clone()),
            ContactDamage::new(1.0)
                .with_interval(0.25)
                .with_shape(HitShape::circle(24.0), Vec2::ZERO)
                .with_filter(player),
            StateMachine::new(Guard::Patrol),
        )),
    );
    // Two targets in both its reaches all along its path.
    let targets: Vec<Entity> = [Vec2::new(-6.0, 12.0), Vec2::new(4.0, 12.0)]
        .into_iter()
        .map(|at| {
            world.spawn_with((
                Transform::from_position(at),
                Tags::new(["team.player"]),
                Hurtbox::new(HitShape::circle(4.0)),
                Health::new(1000.0),
            ))
        })
        .collect();
    // Mid-swing: the window open on both targets hit, the shield between
    // two touches and the turn under way. The game stuns it with a rest
    // asked for: a transition pending, and the machine held.
    harness.step(frames_for(0.55, dt));
    let world = harness.world_mut();
    let machine = world.get_mut::<StateMachine<Guard>>(guard).unwrap();
    assert_eq!(machine.state(), Guard::Swing, "dt {dt}");
    machine.go(Guard::Rest);
    machine.set_held(true);
    let saved_at = world.get::<Position>(guard).unwrap().0;
    if let Some(named_again) = load {
        let id = |entity: &Entity| targets.iter().position(|target| target == entity).unwrap();
        let sword = world.get::<Hitbox>(guard).unwrap().clone();
        let shield = world.get::<ContactDamage>(guard).unwrap().clone();
        assert!(
            sword.is_armed() && sword.hit_targets().len() == 2,
            "dt {dt}"
        );
        assert_eq!(shield.touches().count(), 2, "dt {dt}");
        let save = GuardSave {
            machine: world.get::<StateMachine<Guard>>(guard).unwrap().clone(),
            mover: world.get::<TopDownMover>(guard).unwrap().clone(),
            sword_hit: sword.hit_targets().iter().map(id).collect(),
            sword,
            shield_touches: shield
                .touches()
                .map(|touch| (id(&touch.target()), touch.since_hit(), touch.since_met()))
                .collect(),
            shield,
        };
        let dir = std::env::temp_dir().join(format!(
            "tungsten_recipe_guard_{}_{}_{named_again}",
            std::process::id(),
            frames_for(1.0, dt)
        ));
        let slots = SaveSlots::new(Some(dir.clone()));
        slots.save("guard", &save).unwrap();
        let loaded: GuardSave = slots.load("guard").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let (mut sword, mut shield) = (loaded.sword, loaded.shield);
        assert!(sword.hit_targets().is_empty() && shield.touches().count() == 0);
        if named_again {
            sword.set_hit_targets(loaded.sword_hit.iter().map(|&id| targets[id]));
            shield.set_touches(
                loaded
                    .shield_touches
                    .iter()
                    .map(|&(id, hit, met)| ContactTouch::new(targets[id], hit, met)),
            );
        }
        world.insert(guard, loaded.machine);
        world.insert(guard, loaded.mover);
        world.insert(guard, sword);
        world.insert(guard, shield);
    }
    let start = world.resource::<GuardTrail>().steps.len();
    harness.step(frames_for(0.1, dt));
    harness
        .world_mut()
        .get_mut::<StateMachine<Guard>>(guard)
        .unwrap()
        .set_held(false);
    harness.step(frames_for(0.5, dt));
    let trail = harness.world().resource::<GuardTrail>();
    (trail.steps[start..].to_vec(), trail.rests, saved_at)
}

#[test]
fn a_guard_saved_mid_swing_resumes_exactly() {
    for dt in SAVE_DTS {
        let (kept, rests, saved_at) = guard_run(dt, None);
        let (loaded, loaded_rests, _) = guard_run(dt, Some(true));
        assert!(kept.len() > 30 && kept.len() == loaded.len(), "dt {dt}");
        assert_eq!(
            (rests, loaded_rests),
            (1, 1),
            "dt {dt}: the rest entered once"
        );
        for (n, (kept, loaded)) in kept.iter().zip(&loaded).enumerate() {
            assert_eq!(loaded.machine, kept.machine, "dt {dt} step {n}");
            assert_eq!(loaded.struck, kept.struck, "dt {dt} step {n}");
        }
        // The rest waited for the hold, then came at the same step.
        let rest_at = |steps: &[GuardStep]| {
            steps
                .iter()
                .position(|step| step.machine.0 == Guard::Rest)
                .unwrap()
        };
        assert!(rest_at(&kept) >= 5, "dt {dt}");
        assert!(
            kept.iter().any(|step| !step.struck.is_empty()),
            "dt {dt}: the shield's touches went on"
        );
        // Its mover's still first step, then its path a step behind.
        assert_eq!(
            (loaded[0].at, loaded[0].velocity),
            (saved_at, Vec2::ZERO),
            "dt {dt}"
        );
        for (n, (kept, loaded)) in kept.iter().zip(&loaded[1..]).enumerate() {
            assert!(
                loaded.velocity.distance(kept.velocity) < 1e-3
                    && loaded.at.distance(kept.at) < 1e-3,
                "dt {dt} step {n}: {kept:?} against {loaded:?}"
            );
        }
        assert!(
            kept[0].velocity.x > 1.0 && kept[0].velocity.y > 1.0,
            "dt {dt}: mid-turn"
        );
        // Loaded without its targets named, the sword and the shield hit at once.
        let (unnamed, _, _) = guard_run(dt, Some(false));
        let first_hits = |steps: &[GuardStep]| {
            steps
                .iter()
                .find(|step| !step.struck.is_empty())
                .map(|step| step.struck.len())
        };
        assert_eq!(first_hits(&unnamed), Some(4), "dt {dt}");
        assert!(kept[0].struck.is_empty(), "dt {dt}");
        assert_eq!(unnamed[0].struck.len(), 4, "dt {dt}");
    }
}

// M76: walls by their physics layers. An arrow passes hedges in layer 3
// and ends at a stone wall; saved between the hedges, it resumes with the
// same meetings.

/// A wall as a meeting names it: an entity or a tile, its normal, its
/// layers.
type Wall = (bool, Vec2, u32);

/// What a shot met: "passed" or "expired", where, and the wall.
type Meeting = (&'static str, Vec2, Option<Wall>);

/// Runs `frames` frames, collecting each one's meetings.
fn meetings(harness: &mut Harness, frames: u32) -> Vec<Meeting> {
    use crate::kit::{ProjectilePassed, WallContact};

    let wall_of = |wall: WallContact| (wall.entity.is_some(), wall.normal, wall.layers);
    let mut out = Vec::new();
    for _ in 0..frames {
        harness.step(1);
        out.extend(
            harness
                .events::<ProjectilePassed>()
                .map(|event| ("passed", event.position, Some(wall_of(event.wall)))),
        );
        out.extend(
            harness
                .events::<ProjectileExpired>()
                .map(|event| ("expired", event.position, event.wall.map(wall_of))),
        );
    }
    out
}

/// The garden: hedges whose left faces are at x 90 and 190, and the stone
/// wall's at x 290.
fn garden(dt: f32) -> Harness {
    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    spawn(&mut harness, "hedge", [100.0, 100.0]);
    spawn(&mut harness, "hedge", [200.0, 100.0]);
    spawn(&mut harness, "wall", [300.0, 0.0]);
    harness
}

/// The game's save of its arrow in flight.
#[derive(serde::Serialize, serde::Deserialize)]
struct ArrowSave {
    projectile: crate::kit::Projectile,
    motion: LinearMotion,
    at: [f32; 2],
}

impl SaveData for ArrowSave {
    const SCHEMA: u32 = 1;
}

/// The arrow's meetings at `dt` before its save point and after it; with
/// `load`, the game saves it there and loads it into a fresh garden.
fn arrow_run(dt: f32, load: bool) -> (Vec<Meeting>, Vec<Meeting>) {
    let mut harness = garden(dt);
    let arrow = spawn(&mut harness, "arrow", [0.0, 100.0]);
    let before = meetings(&mut harness, frames_for(0.25, dt));
    if load {
        let world = harness.world();
        let at = body_at(&harness, arrow);
        let save = ArrowSave {
            projectile: world.get::<crate::kit::Projectile>(arrow).unwrap().clone(),
            motion: world.get::<LinearMotion>(arrow).unwrap().clone(),
            at: [at.x, at.y],
        };
        let dir = std::env::temp_dir().join(format!(
            "tungsten_recipe_arrow_{}_{}",
            std::process::id(),
            frames_for(1.0, dt)
        ));
        let slots = SaveSlots::new(Some(dir.clone()));
        slots.save("arrow", &save).unwrap();
        let loaded: ArrowSave = slots.load("arrow").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        harness = garden(dt);
        let arrow = spawn(&mut harness, "arrow", loaded.at);
        let world = harness.world_mut();
        world.insert(arrow, loaded.projectile);
        world.insert(arrow, loaded.motion);
    }
    let after = meetings(&mut harness, frames_for(0.5, dt));
    (before, after)
}

#[test]
fn an_arrow_passes_hedges_and_ends_at_a_stone_wall_and_resumes_after_a_load() {
    let left = Vec2::new(-1.0, 0.0);
    let hedge = Some((true, left, 1 << 3));
    for dt in DTS {
        let (before, after) = arrow_run(dt, false);
        let close = |got: &[Meeting], want: &[(&str, f32, Option<Wall>)]| {
            got.len() == want.len()
                && got.iter().zip(want).all(|(a, b)| {
                    a.0 == b.0 && a.1.distance(Vec2::new(b.1, 100.0)) < 1e-3 && a.2 == b.2
                })
        };
        assert!(
            close(&before, &[("passed", 88.0, hedge)]),
            "dt {dt}: {before:?}"
        );
        assert!(
            close(
                &after,
                &[
                    ("passed", 188.0, hedge),
                    ("expired", 288.0, Some((true, left, 1)))
                ]
            ),
            "dt {dt}: {after:?}"
        );
        let (loaded_before, loaded_after) = arrow_run(dt, true);
        assert_eq!(loaded_before, before, "dt {dt}");
        assert_eq!(loaded_after.len(), after.len(), "dt {dt}: {loaded_after:?}");
        for (loaded, kept) in loaded_after.iter().zip(&after) {
            assert!(
                loaded.0 == kept.0 && loaded.2 == kept.2 && loaded.1.distance(kept.1) < 1e-3,
                "dt {dt}: {loaded:?} against {kept:?}"
            );
        }
    }
}

// 7.10a: scoped stat lines, context reads and world sources.

/// `entity`'s `damage` read in the context of one damage type.
fn damage_in(harness: &Harness, entity: Entity, kind: &str) -> Option<f32> {
    let context = [TagSet::Name(kind)];
    harness
        .world()
        .get::<Stats>(entity)
        .unwrap()
        .read(Stats::DAMAGE, &Read::new(&context))
}

#[test]
fn a_veterans_own_line_and_a_focused_effect_read_in_and_out_of_context() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let veteran = spawn(&mut harness, "veteran", [2000.0, 2000.0]);
        harness.step(1);
        let get = |harness: &Harness| {
            harness
                .world()
                .get::<Stats>(veteran)
                .unwrap()
                .get(Stats::DAMAGE)
        };
        assert_eq!(get(&harness), Some(4.0), "dt {dt}: its own line is scoped");
        assert_eq!(damage_in(&harness, veteran, "elemental.fire"), Some(6.0));
        assert_eq!(damage_in(&harness, veteran, "elemental.cold"), Some(4.0));

        let focused = DataRef::by_id("focused");
        assert!(EffectRequest::new(veteran, focused).queue(harness.world_mut()));
        harness.step(1);
        assert_eq!(
            damage_in(&harness, veteran, "elemental.cold"),
            Some(8.0),
            "dt {dt}"
        );
        assert_eq!(damage_in(&harness, veteran, "elemental.fire"), Some(6.0));
        assert_eq!(
            get(&harness),
            Some(4.0),
            "dt {dt}: the effect's line is scoped too"
        );
        harness.step(frames_for(2.1, dt));
        assert_eq!(
            damage_in(&harness, veteran, "elemental.cold"),
            Some(4.0),
            "dt {dt}: it left"
        );
    }
}

/// The `difficulty` world source: `mul 1.5` on the enemies' health and
/// damage.
fn hard() -> WorldSource {
    WorldSource::new(TagFilter::new().all(["team.enemy"]))
        .with_line(Stats::MAX_HEALTH, Line::new(Modifier::Mul(1.5)))
        .with_line(Stats::DAMAGE, Line::new(Modifier::Mul(1.5)))
}

#[test]
fn a_difficulty_source_reaches_the_living_a_spawn_and_leaves_when_removed() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let veteran = spawn(&mut harness, "veteran", [2000.0, 2000.0]);
        let brute = spawn(&mut harness, "brute", [2400.0, 2000.0]);
        harness.step(1);
        let max = |harness: &Harness, entity| harness.world().get::<Health>(entity).unwrap().max();
        WorldSources::set(harness.world_mut(), "difficulty", hard());
        harness.step(1);
        assert_eq!(
            max(&harness, veteran),
            12.0,
            "dt {dt}: the living, at the next step"
        );
        assert_eq!(damage_in(&harness, veteran, "elemental.fire"), Some(9.0));
        assert_eq!(
            harness.world().get::<Stats>(brute).unwrap().get("armor"),
            Some(2.0),
            "dt {dt}: a stat no line names"
        );

        let late = spawn(&mut harness, "veteran", [2800.0, 2000.0]);
        harness.step(1);
        let health = harness.world().get::<Health>(late).unwrap();
        assert_eq!(
            (health.current(), health.max()),
            (12.0, 12.0),
            "dt {dt}: in its first step"
        );

        assert!(WorldSources::remove(harness.world_mut(), "difficulty"));
        harness.step(1);
        assert_eq!(max(&harness, veteran), 8.0, "dt {dt}: it left");
        assert_eq!(max(&harness, late), 8.0);
        assert_eq!(damage_in(&harness, late, "elemental.fire"), Some(6.0));
    }
}

#[test]
fn a_bulwark_takes_front_hits_at_a_quarter_and_rear_hits_whole() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let bulwark = spawn(&mut harness, "bulwark", [3000.0, 3000.0]);
        spawn(&mut harness, "post", [3200.0, 3000.0]);
        harness.step(2);
        let aim = harness.world().get::<Aim>(bulwark).unwrap().direction();
        assert_eq!(
            aim,
            Some(Vec2::X),
            "dt {dt}: it aims at the post on its right"
        );
        let slash = |direction: Vec2| {
            DamageRequest::from_parts(bulwark, [typed_part(8.0, "physical.slash")])
                .with_direction(direction)
        };
        let health = |harness: &Harness| harness.world().get::<Health>(bulwark).unwrap().current();
        slash(Vec2::NEG_X).apply(harness.world_mut());
        assert_eq!(health(&harness), 38.0, "dt {dt}: from the front, a quarter");
        slash(Vec2::X).apply(harness.world_mut());
        assert_eq!(health(&harness), 30.0, "dt {dt}: from behind, whole");
        DamageRequest::from_parts(bulwark, [typed_part(8.0, "elemental.fire")])
            .with_direction(Vec2::NEG_X)
            .apply(harness.world_mut());
        assert_eq!(
            health(&harness),
            22.0,
            "dt {dt}: its brace is physical only"
        );
    }
}

/// The first seed whose first draws are `rolls`, each under 0.5 or not.
fn seed_for(rolls: &[bool]) -> u64 {
    (0..10_000)
        .find(|&seed| {
            let mut rng = KitRng::seeded(seed);
            rolls.iter().all(|&crit| (rng.unit() < 0.5) == crit)
        })
        .unwrap()
}

#[test]
fn a_marksmans_embers_crit_from_its_stats_at_a_pinned_seed() {
    let seed = seed_for(&[true, false]);
    assert_eq!(seed, 2, "pinned: crit, then none");
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness.world_mut().insert_resource(KitRng::seeded(seed));
        let marksman = spawn(&mut harness, "marksman", [5000.0, 5000.0]);
        let veteran = spawn(&mut harness, "veteran", [5100.0, 5000.0]);
        let mut hits = Vec::new();
        for _ in 0..frames_for(1.3, dt) {
            harness.step(1);
            hits.extend(
                harness
                    .events::<Damaged>()
                    .filter(|hit| hit.entity == veteran)
                    .map(|hit| (hit.amount, hit.crit, hit.source)),
            );
        }
        assert_eq!(
            hits,
            [(6.0, true, Some(marksman)), (2.0, false, Some(marksman))],
            "dt {dt}: its first ember crits for 2 × 3, its second lands whole"
        );
        assert_eq!(
            harness.world().resource::<KitRng>().draws(),
            2,
            "dt {dt}: a draw a hit"
        );
    }
}

#[test]
fn a_casters_bolt_carries_its_skills_snapshot_converted_and_hits_after_the_caster_is_gone() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness
            .world_mut()
            .insert_resource(crate::kit::KitExplain::new());
        let caster = spawn(&mut harness, "caster", [6000.0, 6000.0]);
        let veteran = spawn(&mut harness, "veteran", [6100.0, 6000.0]);
        harness.step(1);
        assert_eq!(
            crate::kit::use_ability(harness.world_mut(), caster, "cinder"),
            crate::kit::Used::Used,
            "dt {dt}"
        );
        harness.step(1);
        let bolts: Vec<Option<Entity>> = harness
            .world()
            .query::<&crate::kit::Delivery>()
            .map(crate::kit::Delivery::holder)
            .collect();
        assert_eq!(
            bolts,
            [Some(caster)],
            "dt {dt}: the bolt carries the snapshot"
        );
        harness.world_mut().despawn(caster);
        let (mut hits, mut parts) = (Vec::new(), Vec::new());
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            hits.extend(
                harness
                    .events::<Damaged>()
                    .filter(|hit| hit.entity == veteran)
                    .map(|hit| (hit.amount, hit.source)),
            );
            parts.extend(parts_on(&harness, veteran));
        }
        assert_eq!(
            hits,
            [(3.0, None)],
            "dt {dt}: the snapshot's damage, with no source"
        );
        assert_eq!(
            parts,
            [
                (
                    1.5,
                    vec!["elemental".to_string(), "elemental.fire".to_string()]
                ),
                (
                    1.5,
                    vec!["elemental".to_string(), "elemental.cold".to_string()]
                ),
            ],
            "dt {dt}: half its fire converted to cold at the use"
        );
        // Its why-record: each part with where it came from, and the sheet
        // its bolt carried.
        let hit = harness
            .world()
            .resource::<crate::kit::KitExplain>()
            .last_hit()
            .unwrap();
        let parts: Vec<(&str, f32)> = hit
            .parts
            .iter()
            .map(|part| (part.tags[0].as_str(), part.after))
            .collect();
        assert_eq!(
            parts,
            [("elemental.fire", 1.5), ("elemental.cold", 1.5)],
            "dt {dt}"
        );
        assert_eq!(
            hit.parts[1].origins[0].steps,
            [crate::kit::OriginStep::Converted {
                share: 0.5,
                to: "elemental.cold".to_string(),
                source: "skill:cinder".to_string(),
            }],
            "dt {dt}"
        );
        let sheet = hit.sheet.as_deref().unwrap();
        assert_eq!((sheet.holder, sheet.ability.as_str()), (caster, "cinder"));
        assert!(
            hit.to_string()
                .contains("cold part  1.50   from fire 3–3 at u "),
            "dt {dt}: {hit}"
        );
    }
}

#[test]
fn a_skiff_slides_along_a_wall_under_a_steady_pull_and_an_ox_stacks_its_pushes() {
    for dt in DTS {
        // A pull of (120, 80) px/s pushed each frame into a wall whose left
        // face is at x 290: the skiff stays out of the wall and slides down.
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "wall", [300.0, 0.0]);
        let skiff = spawn(&mut harness, "skiff", [270.0, 0.0]);
        harness.step(2);
        let mut y = body_at(&harness, skiff).y;
        for frame in 0..frames_for(0.5, dt) {
            harness
                .world_mut()
                .get_mut::<TopDownMover>(skiff)
                .unwrap()
                .push(Vec2::new(120.0, 80.0));
            harness.step(1);
            let at = body_at(&harness, skiff);
            assert!(
                at.x <= 280.0 + 1e-3,
                "dt {dt}, frame {frame}: into the wall: {at}"
            );
            assert!(at.y > y, "dt {dt}, frame {frame}: down along it: {at}");
            y = at.y;
        }
        // Each push lasts one fixed step: the frame's first, so a frame of
        // two steps moves by half its pull.
        let fixed = harness
            .world()
            .resource::<tungsten_core::Time>()
            .fixed_step();
        let pulled = 80.0 * fixed * frames_for(0.5, dt) as f32;
        assert!((y - pulled).abs() < 1e-2, "dt {dt}: {y}, pulled {pulled}");

        // The ox stacks: a second push 0.05 s after the first starts from
        // the first's value then.
        let ox = spawn(&mut harness, "ox", [100.0, 100.0]);
        harness.step(1);
        assert!(knock_back(harness.world_mut(), ox, Vec2::new(-400.0, 0.0)));
        harness.step(frames_for(0.05, dt));
        let first = harness.world().get::<Knockback>(ox).unwrap().push();
        assert!(knock_back(harness.world_mut(), ox, Vec2::new(0.0, -200.0)));
        let second = harness.world().get::<Knockback>(ox).unwrap().push();
        assert!(
            second.abs_diff_eq(first + Vec2::new(0.0, -200.0), 1e-3),
            "dt {dt}: {first} then {second}"
        );
    }
}

#[test]
fn a_wisp_steers_its_mover_along_a_wall_and_lands_on_a_point() {
    for dt in DTS {
        // Arriving at a point behind a wall whose left face is at x 290:
        // the mover's casts stop the wisp at the wall and it slides along
        // it to the point's height.
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "wall", [300.0, 0.0]);
        let wisp = spawn(&mut harness, "wisp", [200.0, 0.0]);
        harness.step(frames_for(3.0, dt));
        let at = body_at(&harness, wisp);
        assert!(
            at.x <= 282.0 + 1e-3 && at.x > 281.9,
            "dt {dt}: at the wall: {at}"
        );
        assert!(at.y > 40.0 && at.y <= 60.0, "dt {dt}: slid along it: {at}");
        let world = harness.world();
        assert_eq!(
            world.get::<Velocity>(wisp).unwrap().0,
            world.get::<TopDownMover>(wisp).unwrap().velocity(),
            "dt {dt}: the body moves by the mover"
        );

        // A point the game sets in the open: it lands on it.
        let point = Vec2::new(150.0, -40.0);
        harness
            .world_mut()
            .get_mut::<Steering>(wisp)
            .unwrap()
            .set_target(crate::kit::Target::point(point));
        harness.step(frames_for(6.0, dt));
        let at = body_at(&harness, wisp);
        assert!(at.distance(point) < 1e-3, "dt {dt}: on the point: {at}");
    }
}

/// The trooper's `suppressed` intensity, and whether it carries `tag`.
fn suppression(harness: &Harness, trooper: Entity) -> Option<f32> {
    harness
        .world()
        .get::<Effects>(trooper)
        .and_then(|effects| effects.intensity("suppressed"))
}

#[test]
fn suppression_builds_bands_and_decays() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let trooper = spawn(&mut harness, "trooper", [300.0, 200.0]);
        harness.step(1);
        assert_eq!(suppression(&harness, trooper), None, "dt {dt}");
        // A round every quarter second through the trooper's near-miss
        // zone: each entry builds `suppressed` by 1, which wears 0.5 a
        // second, its bands granting cover at 3 and pinned at its max of 5.
        let carries = |harness: &Harness, tag: &str| {
            harness
                .world()
                .get::<Tags>(trooper)
                .is_some_and(|tags| tags.has(tag))
        };
        // The highest intensity, the frame of the last build and the last
        // reading.
        let mut watch = (0.0_f32, 0_u32, 0.0_f32);
        let check = |harness: &Harness, frame: u32, watch: &mut (f32, u32, f32)| {
            let now = suppression(harness, trooper).unwrap_or(0.0);
            assert_eq!(
                carries(harness, "status.cover"),
                now >= 3.0,
                "dt {dt}: {now}"
            );
            assert_eq!(
                carries(harness, "status.pinned"),
                now >= 5.0,
                "dt {dt}: {now}"
            );
            watch.0 = watch.0.max(now);
            if now > watch.2 {
                watch.1 = frame;
            }
            watch.2 = now;
        };
        let mut frame = 0;
        for _ in 0..8 {
            spawn(&mut harness, "round", [240.0, 200.0]);
            for _ in 0..frames_for(0.25, dt) {
                harness.step(1);
                frame += 1;
                check(&harness, frame, &mut watch);
            }
        }
        // At its max of 5; at two steps a frame, a build in the first step
        // is worn by the second before the frame ends.
        assert!(watch.0 >= 5.0 - 0.5 * dt, "dt {dt}: peak {}", watch.0);
        if dt == DTS[0] {
            assert_eq!(watch.0, 5.0, "pinned at one step a frame");
        }
        // The fire stops: pinned leaves at once, cover 4 s on, and the
        // status runs out at 0, 10 s after its last build at 5.
        while suppression(&harness, trooper).is_some() {
            harness.step(1);
            frame += 1;
            check(&harness, frame, &mut watch);
            assert!(frame < frames_for(20.0, dt), "dt {dt}: still suppressed");
        }
        let lasted = (frame - watch.1) as f32 * dt;
        assert!((lasted - 10.0).abs() <= dt + 1e-3, "dt {dt}: {lasted} s");
        assert!(!carries(&harness, "status.cover") && !carries(&harness, "status.pinned"));
    }
}

#[test]
fn fire_lights_a_crate_and_cold_puts_it_out() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let crate_ = spawn(&mut harness, "wooden_crate", [300.0, 100.0]);
        let torch = harness.world_mut().spawn();
        harness.step(1);
        let ablaze = |harness: &Harness| {
            harness
                .world()
                .get::<Effects>(crate_)
                .and_then(|effects| effects.intensity("ablaze"))
        };
        let burning = |harness: &Harness| {
            harness
                .world()
                .get::<Tags>(crate_)
                .is_some_and(|tags| tags.has("status.burning"))
        };
        // A fire part of 3 on the wooden crate: the wood rule builds
        // `ablaze` by it, which wears 0.5 a second.
        DamageRequest::from_parts(
            crate_,
            [DamagePart::new(3.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .with_source(torch)
        .apply(harness.world_mut());
        harness.step(1);
        let lit = ablaze(&harness).unwrap();
        assert!((lit - 3.0).abs() <= 0.5 * dt + 1e-4, "dt {dt}: {lit}");
        assert!(burning(&harness), "dt {dt}");
        let instance = harness
            .world()
            .get::<Effects>(crate_)
            .unwrap()
            .instances()
            .next()
            .unwrap()
            .clone();
        assert_eq!(instance.source(), Some(torch), "dt {dt}: credited");
        // Cold on the burning crate: the douse rule removes it.
        DamageRequest::from_parts(
            crate_,
            [DamagePart::new(1.0).with_tags(Tags::new(["elemental.cold"]))],
        )
        .apply(harness.world_mut());
        harness.step(1);
        assert_eq!(ablaze(&harness), None, "dt {dt}: out");
        assert!(!burning(&harness), "dt {dt}");
    }
}

#[test]
fn burning_crates_set_each_other_alight() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let lit = spawn(&mut harness, "wooden_crate", [300.0, 100.0]);
        let near = spawn(&mut harness, "wooden_crate", [340.0, 100.0]);
        let far = spawn(&mut harness, "wooden_crate", [500.0, 100.0]);
        harness.step(1);
        let burning = |harness: &Harness, entity: Entity| {
            harness
                .world()
                .get::<Tags>(entity)
                .is_some_and(|tags| tags.has("status.burning"))
        };
        DamageRequest::from_parts(
            lit,
            [DamagePart::new(3.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .apply(harness.world_mut());
        harness.step(1);
        assert!(
            burning(&harness, lit) && !burning(&harness, near),
            "dt {dt}"
        );
        // `ablaze` sends `heat` within 48 px every 0.5 s: the crate 40 px
        // off catches, the one 200 px off does not.
        harness.step(frames_for(0.5, dt));
        assert!(burning(&harness, near), "dt {dt}: caught");
        assert!(!burning(&harness, far), "dt {dt}: out of reach");
        // Each keeps the other alight past the first's own 6 s.
        harness.step(frames_for(8.0, dt));
        assert!(burning(&harness, lit) && burning(&harness, near), "dt {dt}");
        assert!(!burning(&harness, far), "dt {dt}");
    }
}

#[test]
fn a_lamp_lights_a_lurker() {
    use glam::Vec3;
    use tungsten_core::Light;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let lurker = spawn(&mut harness, "lurker", [100.0, 100.0]);
        let hidden = spawn(&mut harness, "lurker", [600.0, 100.0]);
        // A lamp 40 px from the first: chemistry samples the light every
        // 0.25 s, building `lit` by its luma.
        harness.world_mut().spawn_with((
            Transform::from_position(Vec2::new(140.0, 100.0)),
            Light::point(Vec3::ONE, 120.0),
        ));
        let lit = |harness: &Harness, entity: Entity| {
            harness
                .world()
                .get::<Tags>(entity)
                .is_some_and(|tags| tags.has("status.lit"))
        };
        harness.step(1);
        assert!(lit(&harness, lurker), "dt {dt}: sampled at once");
        assert!(!lit(&harness, hidden), "dt {dt}: in the dark");
        harness.step(frames_for(2.0, dt));
        assert!(lit(&harness, lurker) && !lit(&harness, hidden), "dt {dt}");
    }
}

#[test]
fn a_grass_strip_burns_across_from_a_crate() {
    use crate::kit::{TileCell, TileStatuses};
    use tungsten_core::{LayerKind, TileProps, TilemapData, TilemapInstance, TilemapRegistry};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // Six 32 px grass cells from x 332, their centres 348 to 508 at
        // y 100, built in code with their material.
        let mut map = TilemapData::new(32, 32, 6, 1);
        let grass = map.add_tile("grass");
        let mut props = TileProps::default();
        props.material = Some("grass".into());
        map.set_tile_props(grass, props);
        let layer = map.add_layer("meadow", LayerKind::Render);
        map.fill(layer, grass);
        let origin = Vec2::new(332.0, 84.0);
        let world = harness.world_mut();
        world
            .resource_mut::<TilemapRegistry>()
            .insert("meadow".into(), map);
        world.spawn_with((TilemapInstance::new("meadow", origin),));
        // A crate 48 px from the first cell, lit by fire: `ablaze` sends
        // `heat` within 48 px every 0.5 s, which builds it on grass too.
        let crate_ = spawn(&mut harness, "wooden_crate", [300.0, 100.0]);
        harness.step(1);
        DamageRequest::from_parts(
            crate_,
            [DamagePart::new(3.0).with_tags(Tags::new(["elemental.fire"]))],
        )
        .apply(harness.world_mut());
        let burning = |harness: &Harness, col: u32| {
            let cell = TileCell {
                map: "meadow",
                origin,
                layer: 0,
                col,
                row: 0,
            };
            harness
                .world()
                .resource::<TileStatuses>()
                .carries(cell, "ablaze")
        };
        let mut lit = [None; 6];
        for frame in 0..frames_for(4.0, dt) {
            harness.step(1);
            for (col, at) in lit.iter_mut().enumerate() {
                if at.is_none() && burning(&harness, col as u32) {
                    *at = Some(frame);
                }
            }
        }
        let lit: Vec<u32> = lit
            .iter()
            .map(|at| at.unwrap_or_else(|| panic!("dt {dt}: every cell burns")))
            .collect();
        assert!(
            lit.windows(2).all(|pair| pair[0] < pair[1]),
            "dt {dt}: in turn {lit:?}"
        );
    }
}

#[test]
fn a_lamp_doused_by_water_relights() {
    let tagged_one = |harness: &Harness, tag: &str| -> Option<Entity> {
        let found: Vec<Entity> = harness
            .world()
            .query::<(Entity, &tungsten_core::Tag)>()
            .filter(|(_, found)| found.name == tag)
            .map(|(entity, _)| entity)
            .collect();
        (found.len() == 1).then(|| found[0])
    };
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let lamp = spawn(&mut harness, "lamp", [200.0, 100.0]);
        // A bucket 30 px off sends `stimulus.wet` within 40 px every 0.5 s
        // for 0.6 s: `douses_lamps` makes the lamp `lamp_doused` for 3 s.
        let bucket = harness
            .world_mut()
            .spawn_with((Transform::from_position(Vec2::new(230.0, 100.0)),));
        crate::kit::EffectRequest::new(bucket, DataRef::by_id("drenched"))
            .apply(harness.world_mut());
        harness.step(frames_for(0.6, dt));
        assert!(!harness.world().is_alive(lamp), "dt {dt}: replaced");
        let doused = tagged_one(&harness, "lamp_doused").expect("doused");
        assert_eq!(
            drawn_at(&harness, doused),
            Vec2::new(200.0, 100.0),
            "dt {dt}"
        );
        assert!(tagged_one(&harness, "lamp").is_none(), "dt {dt}");
        harness.step(frames_for(3.0, dt));
        assert!(tagged_one(&harness, "lamp").is_some(), "dt {dt}: relit");
        assert!(tagged_one(&harness, "lamp_doused").is_none(), "dt {dt}");
    }
}

#[test]
fn a_bombs_blast_hurts_crates_and_a_target() {
    use crate::kit::Knockback;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // The bomb's `fuse` runs 0.5 s; its end blasts 12 within 64 px,
        // pushing 300, both falling off to 0 at the radius.
        spawn(&mut harness, "powder_bomb", [300.0, 300.0]);
        let near = spawn(&mut harness, "wooden_crate", [332.0, 300.0]);
        let far = spawn(&mut harness, "wooden_crate", [300.0, 400.0]);
        let target = spawn(&mut harness, "blast_dummy", [300.0, 252.0]);
        harness.step(frames_for(0.5, dt) + 3);
        let world = harness.world();
        let health = |entity| world.get::<Health>(entity).unwrap().current();
        assert!(
            (health(near) - (100.0 - 6.0)).abs() < 1e-3,
            "dt {dt}: {}",
            health(near)
        );
        assert_eq!(health(far), 100.0, "dt {dt}: out of reach");
        assert!(
            (health(target) - (50.0 - 3.0)).abs() < 1e-3,
            "dt {dt}: {}",
            health(target)
        );
        let push = world.get::<Knockback>(target).unwrap().push();
        assert!(
            push.y < 0.0 && push.x.abs() < 1e-3,
            "dt {dt}: away from the bomb, {push}"
        );
    }
}

// 7.10c, Step 4: a drop rolled onto the dead brute's centre (S-88).

#[test]
fn a_drop_rolls_onto_the_dead_brutes_centre() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "ogre", [7000.0, 7000.0]);
        harness.step(1);
        // Its hurtbox, and so its hit, 16 px right of and below its point.
        let centre = Vec2::new(7016.0, 7016.0);
        spawn(&mut harness, "cleaver", [centre.x, centre.y]);
        let mut dropped = None;
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            if let Some(&tooth) = tagged(&harness, "tooth").first() {
                dropped = Some(tooth);
                break;
            }
        }
        let tooth = dropped.unwrap_or_else(|| panic!("dt {dt}: a drop"));
        assert_eq!(
            crate::kit::centre_of(harness.world(), tooth),
            Some(centre),
            "dt {dt}: its collider's centre on the brute's"
        );
        assert_eq!(
            body_at(&harness, tooth),
            centre - Vec2::splat(4.0),
            "dt {dt}: its point 4 px up and left"
        );
    }
}

// 7.10c, Step 7: a chain bolt conducting along pylons (S-5).

#[test]
fn a_chain_bolt_jumps_three_conductors_and_ends_at_the_frozen_one() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let caller = spawn(&mut harness, "stormcaller", [8000.0, 8000.0]);
        let pylons: Vec<Entity> = [100.0, 200.0, 300.0]
            .map(|x| spawn(&mut harness, "pylon", [8000.0 + x, 8000.0]))
            .into();
        let frozen = spawn(&mut harness, "frozen_pylon", [8400.0, 8000.0]);
        harness.step(1);
        assert_eq!(
            crate::kit::use_ability(harness.world_mut(), caller, "chain_bolt"),
            crate::kit::Used::Used
        );
        // One strike a fixed step, each 0.7 of the last; read per step.
        let mut struck = Vec::new();
        let steps = (dt * 60.0).round() as u32;
        for _ in 0..frames_for(0.25, dt) {
            harness.step(1);
            let mut this: Vec<(Entity, f32)> = harness
                .events::<Damaged>()
                .map(|hit| (hit.entity, hit.amount))
                .collect();
            this.sort_by_key(|&(entity, _)| entity.id());
            struck.extend(this);
        }
        let amounts: Vec<f32> = struck.iter().map(|&(_, amount)| amount).collect();
        assert_eq!(
            struck.iter().map(|&(entity, _)| entity).collect::<Vec<_>>(),
            pylons,
            "dt {dt}: the three in order, one a step ({steps} a frame), none on the frozen"
        );
        for (got, want) in amounts.iter().zip([30.0, 21.0, 14.7]) {
            assert!(
                (got - want).abs() < 1e-3,
                "dt {dt}: weaker each: {amounts:?}"
            );
        }
        assert_eq!(
            harness.world().get::<Health>(frozen).unwrap().current(),
            100.0,
            "dt {dt}: the frozen one ends it unstruck"
        );
    }
}

/// 7.10g: the ghoul scout walks its beat in its walk clip; a game's
/// `state.alerted` tag shows its alert clip by its animator's rule while it
/// walks, and the walk once the tag goes.
#[test]
fn an_alerted_ghoul_shows_its_alert_clip() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let scout = spawn(&mut harness, "ghoul_scout", [0.0, 3000.0]);
        let shown = |harness: &Harness| {
            let world = harness.world();
            let state = world.get::<AnimationState>(scout).unwrap();
            let animator = world.get::<CharacterAnimator>(scout).unwrap();
            (state.animation_id.clone(), animator.state())
        };
        harness.step(2);
        assert_eq!(shown(&harness), ("ghoul_walk".into(), "move"), "dt {dt}");
        let alert = |harness: &mut Harness, on: bool| {
            let tags = harness.world_mut().get_mut::<Tags>(scout).unwrap();
            if on {
                tags.insert("state.alerted");
            } else {
                tags.remove("state.alerted");
            }
        };
        alert(&mut harness, true);
        harness.step(1);
        assert_eq!(shown(&harness), ("ghoul_alert".into(), "move"), "dt {dt}");
        harness.step(frames_for(0.5, dt));
        assert_eq!(shown(&harness).0, "ghoul_alert", "dt {dt}: while alerted");
        alert(&mut harness, false);
        harness.step(1);
        assert_eq!(shown(&harness), ("ghoul_walk".into(), "move"), "dt {dt}");
    }
}

/// 7.10g: the game hears a noise and faces the watchman toward it; standing
/// still, it holds that way, turned eight ways, until its motion turns it.
#[test]
fn a_sentry_faces_what_it_heard() {
    use crate::kit::face_toward;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let watchman = spawn(&mut harness, "watchman", [0.0, 5000.0]);
        harness.step(1);
        let faced = |harness: &Harness| {
            let world = harness.world();
            let direction = world.get::<Facing>(watchman).unwrap().direction();
            let rotation = world.get::<Transform>(watchman).unwrap().rotation;
            (direction, Vec2::from_angle(rotation))
        };
        assert_eq!(faced(&harness).0, Vec2::X, "dt {dt}");
        // A noise up and to its left, heard.
        assert!(face_toward(
            harness.world_mut(),
            watchman,
            Vec2::new(-40.0, 4960.0)
        ));
        harness.step(1);
        let up_left = Vec2::new(-1.0, -1.0).normalize();
        let (direction, nose) = faced(&harness);
        assert!(
            (direction - up_left).length() < 1e-5,
            "dt {dt}: {direction}"
        );
        assert!((nose - up_left).length() < 1e-5, "dt {dt}: turned to it");
        harness.step(frames_for(1.0, dt));
        assert!(
            (faced(&harness).0 - up_left).length() < 1e-5,
            "dt {dt}: held"
        );
        // Moving, its motion turns it.
        harness.world_mut().get_mut::<Velocity>(watchman).unwrap().0 = Vec2::new(0.0, 40.0);
        harness.step(1);
        let way = faced(&harness).0;
        assert!(
            (way - Vec2::Y).length() < 1e-5,
            "dt {dt}: its motion's way {way}"
        );
    }
}

/// 7.10g: with `KitDebug`'s key on `dash`, a press draws the hit shapes, the
/// trace's log left off; the next press hides them.
#[test]
fn hit_shapes_draw_on_a_key_without_the_trace() {
    use crate::kit::KitDebug;
    let mut harness = recipe(GENRE, KitPlugins::set(), DTS[0]);
    spawn(&mut harness, "watchman", [0.0, 5000.0]);
    harness
        .world_mut()
        .resource_mut::<KitDebug>()
        .set_key(Some("dash"));
    let lines = |harness: &Harness| harness.draw().unwrap().debug_lines.len();
    harness.step(1);
    assert_eq!(lines(&harness), 0, "off until the key");
    harness.press_action("dash");
    harness.step(1);
    harness.release_action("dash");
    assert!(lines(&harness) > 0, "the watchman's hurtbox drawn");
    let debug = harness.world().resource::<KitDebug>();
    assert!(debug.shapes() && !debug.trace(), "the shapes alone");
    harness.step(1);
    assert!(lines(&harness) > 0, "held on");
    harness.press_action("dash");
    harness.step(1);
    harness.release_action("dash");
    assert_eq!(lines(&harness), 0, "the next press hides them");
}

/// 7.10g: the ghoul guard's touch and swing hurt the bystander in it; an
/// arrow kills the guard, and its death policy disarms it: its body stays
/// its second, harming nothing more.
#[test]
fn a_dead_ghoul_harms_nothing() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let guard = spawn(&mut harness, "ghoul_guard", [0.0, 6000.0]);
        let bystander = spawn(&mut harness, "bystander", [0.0, 6000.0]);
        harness.world_mut().get_mut::<Hitbox>(guard).unwrap().arm();
        let health =
            |harness: &Harness| harness.world().get::<Health>(bystander).unwrap().current();
        harness.step(frames_for(0.2, dt));
        assert_eq!(health(&harness), 98.0, "dt {dt}: its touch and its swing");
        spawn(&mut harness, "arrow", [-60.0, 6000.0]);
        harness.step(frames_for(0.2, dt));
        assert!(
            harness.world().get::<Health>(guard).unwrap().is_dead(),
            "dt {dt}: the arrow kills it"
        );
        let after = health(&harness);
        harness.world_mut().get_mut::<Hitbox>(guard).unwrap().arm();
        harness.step(frames_for(0.6, dt));
        assert_eq!(
            health(&harness),
            after,
            "dt {dt}: disarmed, it harms nothing"
        );
        assert!(harness.world().is_alive(guard), "dt {dt}: its body kept");
        let guard_parts = harness.world().get::<ContactDamage>(guard).unwrap();
        assert!(!guard_parts.is_on(), "dt {dt}");
        harness.step(frames_for(0.5, dt));
        assert!(
            !harness.world().is_alive(guard),
            "dt {dt}: gone after its second"
        );
    }
}

/// 7.10g: the slime nest keeps one slime: its first at once, none more
/// while it lives, the next 3 s after it dies; and the same after the
/// World is saved and loaded with the slime alive.
#[test]
fn a_nest_keeps_one_slime() {
    use tungsten_core::save::{self, capture_world, restore_world};

    #[derive(Debug, Clone, Copy)]
    struct Restored;

    let level = |dt: f32| {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let nest = spawn(&mut harness, "slime_nest", [0.0, 7000.0]);
        save::name(harness.world_mut(), nest, None, "nest").unwrap();
        harness
    };
    let live = |harness: &Harness| -> Vec<Entity> {
        tagged(harness, "slime")
            .into_iter()
            .filter(|&slime| {
                !harness
                    .world()
                    .get::<Health>(slime)
                    .is_some_and(Health::is_dead)
            })
            .collect()
    };
    for dt in DTS {
        let mut kept = level(dt);
        kept.step(1);
        assert_eq!(live(&kept).len(), 1, "dt {dt}: its first at once");
        kept.step(frames_for(4.0, dt));
        assert_eq!(live(&kept).len(), 1, "dt {dt}: one while it lives");
        let saved = capture_world(kept.world()).unwrap();
        let mut loaded = level(dt);
        restore_world(loaded.world_mut(), &saved, &(Restored,)).unwrap();
        assert_eq!(live(&loaded).len(), 1, "dt {dt}: its slime came back");
        for harness in [&mut kept, &mut loaded] {
            let slime = live(harness)[0];
            damage(harness.world_mut(), slime, 2.0);
            harness.step(frames_for(2.9, dt));
            assert!(live(harness).is_empty(), "dt {dt}: not before 3 s");
            harness.step(frames_for(0.2, dt));
            assert_eq!(live(harness).len(), 1, "dt {dt}: 3 s after it died");
            harness.step(frames_for(4.0, dt));
            assert_eq!(live(harness).len(), 1, "dt {dt}: and held at one");
        }
    }
}

/// A brazier's phases, which `a_brazier_cycles_its_phases` times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Brazier {
    Cold,
    Kindling,
    Burning,
    Embers,
}

/// 7.10g: the brazier's phases run on timed transitions alone: cold 1 s,
/// kindling 0.5 s, burning 2 s, embers 1 s, and cold again.
#[test]
fn a_brazier_cycles_its_phases() {
    let phases = StateMachinePlugin::<Brazier>::new("brazier_phases")
        .after(Brazier::Cold, 1.0, Brazier::Kindling)
        .after(Brazier::Kindling, 0.5, Brazier::Burning)
        .after(Brazier::Burning, 2.0, Brazier::Embers)
        .after(Brazier::Embers, 1.0, Brazier::Cold);
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set().with(phases.clone()), dt);
        let brazier = spawn(&mut harness, "brazier", [0.0, 8000.0]);
        harness
            .world_mut()
            .insert(brazier, StateMachine::new(Brazier::Cold));
        let mut entered = Vec::new();
        let mut was = Brazier::Cold;
        for frame in 0..frames_for(5.0, dt) {
            harness.step(1);
            let now = harness
                .world()
                .get::<StateMachine<Brazier>>(brazier)
                .unwrap()
                .state();
            if now != was {
                entered.push((now, frame + 1));
                was = now;
            }
        }
        let at = |seconds: f32| frames_for(seconds, dt) + 1;
        assert_eq!(
            entered,
            [
                (Brazier::Kindling, at(1.0)),
                (Brazier::Burning, at(1.5)),
                (Brazier::Embers, at(3.5)),
                (Brazier::Cold, at(4.5)),
            ],
            "dt {dt}"
        );
    }
}

/// 7.10g: the alerted ghoul scout wears its mark, an overlay its `looks`
/// show while `state.alerted` is on, beside its head on the side it faces,
/// mirrored as its beat turns it back; the mark goes with the tag.
#[test]
fn an_alerted_ghoul_wears_its_mark() {
    use tungsten_core::components::SpriteFlip;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let scout = spawn(&mut harness, "ghoul_scout", [0.0, 3000.0]);
        let marks = |harness: &Harness| -> Vec<(Vec2, bool)> {
            let world = harness.world();
            world
                .query::<(Entity, &AnimationState, &Transform)>()
                .filter(|(_, state, _)| state.animation_id == "ghoul_mark")
                .map(|(mark, _, transform)| {
                    let flipped = world.get::<SpriteFlip>(mark).is_some_and(|flip| flip.0);
                    (transform.position, flipped)
                })
                .collect()
        };
        let side = |harness: &Harness| {
            harness
                .world()
                .get::<Facing>(scout)
                .unwrap()
                .direction()
                .x
                .signum()
        };
        harness.step(2);
        assert!(marks(&harness).is_empty(), "dt {dt}: unalerted, no mark");
        harness
            .world_mut()
            .get_mut::<Tags>(scout)
            .unwrap()
            .insert("state.alerted");
        harness.step(1);
        let worn = |harness: &Harness| {
            let found = marks(harness);
            assert_eq!(found.len(), 1, "dt {dt}: one mark");
            let point = drawn_at(harness, scout);
            let (at, flipped) = found[0];
            (at - point, flipped)
        };
        let first = side(&harness);
        let (offset, flipped) = worn(&harness);
        assert_eq!(offset, Vec2::new(6.0 * first, -14.0), "dt {dt}");
        assert_eq!(flipped, first < 0.0, "dt {dt}");
        // Its beat turns it back within a few seconds: the mark mirrors.
        let mut frames = 0;
        while side(&harness) == first {
            assert!(frames < frames_for(6.0, dt), "dt {dt}: the beat turns");
            harness.step(1);
            frames += 1;
        }
        let (offset, flipped) = worn(&harness);
        assert_eq!(offset, Vec2::new(-6.0 * first, -14.0), "dt {dt}: mirrored");
        assert_eq!(flipped, first > 0.0, "dt {dt}");
        harness
            .world_mut()
            .get_mut::<Tags>(scout)
            .unwrap()
            .remove("state.alerted");
        harness.step(1);
        assert!(marks(&harness).is_empty(), "dt {dt}: gone with the tag");
    }
}

// 7.13c, Step 1: a barrel bomb's blast, which hurts the barrel too, lands
// at its centre and resolves as it goes off.

#[test]
fn a_barrel_bombs_blast_hurts_it_lands_at_its_centre_and_at_once() {
    use crate::kit::EffectExpired;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // Its fuse runs 0.5 s; its end blasts 12 within 64 px, pushing 300,
        // the barrel itself among its targets at distance 0.
        let barrel = spawn(&mut harness, "barrel_bomb", [300.0, 300.0]);
        let target = spawn(&mut harness, "blast_dummy", [300.0, 252.0]);
        let mut ended = None;
        let mut hurt = None;
        for frame in 0..frames_for(1.0, dt) {
            harness.step(1);
            if harness
                .events::<EffectExpired>()
                .any(|expired| expired.entity == barrel)
            {
                ended.get_or_insert(frame);
            }
            if let Some(hit) = harness.events::<Damaged>().find(|hit| hit.entity == target) {
                hurt.get_or_insert((frame, hit.position));
            }
        }
        let (frame, position) = hurt.unwrap_or_else(|| panic!("dt {dt}: the target hurt"));
        assert_eq!(Some(frame), ended, "dt {dt}: in the frame its fuse ends");
        assert_eq!(
            position,
            Some(Vec2::new(300.0, 300.0)),
            "dt {dt}: at its centre"
        );
        let world = harness.world();
        let health = |entity| world.get::<Health>(entity).unwrap().current();
        assert!(
            (health(target) - (50.0 - 3.0)).abs() < 1e-3,
            "dt {dt}: {}",
            health(target)
        );
        assert!(
            (health(barrel) - (30.0 - 12.0)).abs() < 1e-3,
            "dt {dt}: the barrel its own 12: {}",
            health(barrel)
        );
    }
}

// 7.13c, Step 2: a crate knocked into a ranger, credited to its brawler,
// and a stone bolt ending at a rolling ball.

#[test]
fn a_knocked_crate_hurts_a_ranger_credited_to_its_brawler() {
    use crate::kit::ImpactDamage;
    use tungsten_core::physics::Velocity;

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let brawler = harness.world_mut().spawn();
        let ranger = spawn(&mut harness, "ranger", [300.0, 100.0]);
        let crate_ = spawn(&mut harness, "hurled_crate", [250.0, 100.0]);
        let world = harness.world_mut();
        world.get_mut::<Velocity>(crate_).unwrap().0 = Vec2::new(900.0, 0.0);
        world
            .get_mut::<ImpactDamage>(crate_)
            .unwrap()
            .set_credit(Some(brawler));
        let mut hits = Vec::new();
        for _ in 0..frames_for(0.3, dt) {
            harness.step(1);
            hits.extend(
                harness
                    .events::<Damaged>()
                    .filter(|hit| hit.entity == ranger)
                    .map(|hit| (hit.amount, hit.source)),
            );
        }
        // 0.002 a unit of its mass of 4 a px/s past 400.
        assert_eq!(hits.len(), 1, "dt {dt}: {hits:?}");
        assert!((hits[0].0 - 4.0).abs() < 1e-3, "dt {dt}: {hits:?}");
        assert_eq!(hits[0].1, Some(brawler), "dt {dt}");
    }
}

#[test]
fn a_stone_bolt_ends_at_a_rolling_ball() {
    use tungsten_core::physics::{RigidBody, Velocity};

    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // A ball in layer 0 the bolt passes, then one in its body walls
        // rolling at it.
        let bystander = spawn(&mut harness, "rolling_ball", [160.0, 100.0]);
        harness
            .world_mut()
            .get_mut::<RigidBody>(bystander)
            .unwrap()
            .layers = 1;
        let ball = spawn(&mut harness, "rolling_ball", [300.0, 100.0]);
        harness.world_mut().get_mut::<Velocity>(ball).unwrap().0 = Vec2::new(-200.0, 0.0);
        let bolt = spawn(&mut harness, "stone_bolt", [100.0, 100.0]);
        let mut ended = None;
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            if let Some(expired) = harness
                .events::<ProjectileExpired>()
                .find(|expired| expired.entity == bolt)
                .copied()
            {
                ended = Some(expired);
                break;
            }
        }
        let ended = ended.unwrap_or_else(|| panic!("dt {dt}: the bolt ends"));
        assert_eq!(
            ended.wall.and_then(|wall| wall.entity),
            Some(ball),
            "dt {dt}: at the rolling ball, past the other"
        );
    }
}

#[test]
fn a_crystal_on_a_dynamic_body_is_drawn_to_the_ranger() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ranger = spawn(&mut harness, "ranger", [0.0, 0.0]);
        let crystal = spawn(&mut harness, "crystal", [80.0, 0.0]);
        harness.step(2);
        let x = body_at(&harness, crystal).x;
        assert!(x < 80.0 && x > 0.0, "dt {dt}: drawn in: {x}");
        harness.step(frames_for(0.5, dt));
        assert!(!harness.world().is_alive(crystal), "dt {dt}: collected");
        assert_eq!(
            harness.world().get::<Counters>(ranger).unwrap().get("xp"),
            1,
            "dt {dt}"
        );
    }
}
