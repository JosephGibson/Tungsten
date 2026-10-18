//! The top-down fixture: a hero on a kinematic body, a wall, slimes from a
//! nest, a trap zone, a drone on a loop and a bolt that leaves its region;
//! M64's seeker, coward, crates, sentry, wielder and blades, rovers,
//! brawler, pet and gem; M65's ranger and its embers, ghouls, their bursts
//! and drops, orbs and a knight; M66's fantasy damage types: a spellblade
//! and a golem, a pyromancer, its firebolts and a ward; M67's burning
//! slimes, the hero's wand and dash, the ranger's volley, ammo, levels and
//! offers, the golem's cold ladder and the ghouls' poison.

use glam::Vec2;
use tungsten_core::physics::Velocity;
use tungsten_core::{
    AnimationState, AudioCommand, CameraController, CameraMode, CameraState, Entity, Sprite,
    Transform,
};

use super::{
    DTS, body_at, drawn_at, frames_for, overlap_reversed, point_of, recipe, spawn, tagged,
    teleport, traced_steps,
};
use crate::kit::{
    Abilities, Aim, CharacterAnimator, Conversion, CooldownElapsed, Counters, DamagePart,
    DamageRequest, Damaged, DamagedPart, Effects, Facing, Health, Hit, HitFlash, KitPlugins,
    KitRng, Knockback, Level, LinearMotion, Modifier, Penetration, ProjectileExpired, Shield,
    Source, Stats, Steering, Struck, Tags, TopDownMover, TraceKind, ZoneEntered, damage,
    knock_back,
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
        // The threshold is 4 at the golem's 1.5 multiplier; a hit of 8 lands
        // 6.4 after its resistance: a full bucket chills, the next freezes,
        // and three such hits leave the golem of 20 alive.
        cold(golem, 8.0)
            .with_source(mage)
            .apply(harness.world_mut());
        harness.step(1);
        assert!(has_effect(&harness, golem, "chilled") && !has_effect(&harness, golem, "frozen"));
        cold(golem, 8.0)
            .with_source(mage)
            .apply(harness.world_mut());
        harness.step(1);
        assert!(has_effect(&harness, golem, "frozen"), "dt {dt}");
        // Up to the step the freeze expires on; that step, the rival, of a
        // lower index than the mage, lands another full bucket.
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
        harness.step(1);
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
fn a_toxic_hit_poisons_by_chance_under_a_seed() {
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
        assert_eq!(a, run(21), "dt {dt}: one seed, one outcome");
        assert_ne!(a, run(22), "dt {dt}");
        assert!(
            a.iter().any(|&p| p) && a.iter().any(|&p| !p),
            "dt {dt}: {a:?}"
        );
    }
}
