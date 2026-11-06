//! The shooter fixture, Static Front's genre (the paper designs' G3): a
//! bodiless ship under the move actions, bullets that expire or leave the
//! view, raiders from a wave on a path, and a walker on a floor under the
//! same actions; M64's fighter, drifter, asteroid, mine and gun; M65's
//! interceptor, lasers, carrier and bulwark; M66's sci-fi damage types: a
//! hacker, its payloads and a mainframe; M67's corruption ladder, the
//! ship's cannon and the turret's pulse; M74's acid named after a load, the
//! acid ladder's held rung and a game's own damage numbers; M75's drone
//! saved mid-volley; 7.10c's breacher, Static Front's: its SMG held, its
//! dodge along the move stick and its reload's sweep; a lobber's grenade
//! and its blast where it lands; a cryo trooper's cone; an escort carrier's
//! escorts, held to three by their cap; a sapper's mines, a fourth refused
//! at their cap with a blip; 7.10g's missile, turning back right side up,
//! a stray drone killed past the world's bounds, a sentry whose patrol
//! waits at each end, two turrets cycling half a second apart, a charger
//! whose ring grows over its tell, and a frigate's health bar.

use glam::Vec2;
use tungsten_core::RigidBody;
use tungsten_core::physics::{Position, PrevPosition, Velocity};
use tungsten_core::{
    AudioCommand, CameraController, DataRef, Entity, EventCursor, EventQueue, Plugin, SaveData,
    SaveSlots, Schedule, Transform, World, system,
};

use super::{DTS, body_at, drawn_at, frames_for, point_of, recipe, spawn, tagged, traced_steps};
use crate::kit::{
    Abilities, AbilityUsed, Aim, Children, ContactDamage, ContactTouch, CooldownElapsed, Counters,
    DamagePart, DamageRequest, Damaged, DamagedPart, Effect, EffectInstance, EffectRequest,
    Effects, Health, Hit, HitShape, Hitbox, Hurtbox, KitPlugins, KitTrace, LabelStyle,
    LinearMotion, Phase, PlatformerController, Projectile, ProjectileExpired, Shield, Source,
    StateMachine, StateMachinePlugin, Steering, Struck, TagFilter, Tags, TopDownMover, TraceKind,
    damage, float_label,
};
use crate::testing::Harness;

const GENRE: &str = "shooter";

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

#[test]
fn a_hackers_payload_converts_to_a_virus_and_an_exploit_immunity_holds() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let hacker = spawn(&mut harness, "hacker", [640.0, 600.0]);
        let mainframe = spawn(&mut harness, "mainframe", [640.0, 300.0]);
        step_until_hit(&mut harness, mainframe, 1.6, dt);
        // Exploit 2 and overload 8, half the overload to virus: exploit
        // immune at a base of 0, which no point pierces; overload and virus
        // 4 each at 0.5 + 0.25.
        let parts = parts_on(&harness, mainframe);
        assert!(
            parts_match(
                &parts,
                &[
                    (3.0, &["digital", "digital.overload"]),
                    (3.0, &["digital", "digital.virus"]),
                ]
            ),
            "dt {dt}: {parts:?}"
        );
        let hit = harness
            .events::<Damaged>()
            .find(|event| event.entity == mainframe)
            .copied()
            .unwrap();
        assert_eq!((hit.amount, hit.source), (6.0, Some(hacker)));
        assert!(!hit.kinds.has(harness.world(), "digital.exploit"));
        assert!(hit.kinds.has(harness.world(), "digital"));
        let world = harness.world();
        assert_eq!(world.get::<Shield>(mainframe).unwrap().current(), 0.0);
        assert_eq!(world.get::<Health>(mainframe).unwrap().current(), 96.0);
        assert_eq!(world.resource::<Counters>().get("virus_hits"), 1, "dt {dt}");
        assert_eq!(
            typed_labels(&harness),
            [
                ("3".to_string(), [0, 255, 0, 255]),
                ("3".to_string(), [0, 255, 255, 255]),
            ],
            "dt {dt}: overload in `digital`'s cyan, virus in its own green"
        );
    }
}

#[test]
fn an_untyped_laser_sends_one_part_of_no_kind_and_its_label_skips() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "interceptor", [640.0, 600.0]);
        let mainframe = spawn(&mut harness, "mainframe", [640.0, 300.0]);
        step_until_hit(&mut harness, mainframe, 1.0, dt);
        let parts: Vec<DamagedPart> = harness
            .events::<DamagedPart>()
            .filter(|part| part.entity == mainframe)
            .copied()
            .collect();
        assert_eq!(parts.len(), 1, "dt {dt}");
        assert_eq!(parts[0].amount, 1.0);
        assert!(
            parts[0].kinds.is_empty(),
            "dt {dt}: `damage.laser` is no damage type"
        );
        let world = harness.world();
        assert_eq!(
            world.get::<Shield>(mainframe).unwrap().current(),
            1.0,
            "absorbed"
        );
        assert_eq!(world.get::<Health>(mainframe).unwrap().current(), 100.0);
        assert_eq!(world.resource::<Counters>().get("virus_hits"), 0);
        assert!(typed_labels(&harness).is_empty(), "dt {dt}: no label");
        let skipped = world.resource::<KitTrace>().about(mainframe).any(|entry| {
            matches!(
                entry.kind,
                TraceKind::Skipped {
                    event: "damaged_part",
                    action: "label",
                    why: "the part's types set no label style"
                }
            )
        });
        assert!(skipped, "dt {dt}: the trace says why");
    }
}

// M67: effects, abilities and ailments.

fn digital(target: Entity, kind: &str, amount: f32) -> DamageRequest {
    DamageRequest::from_parts(
        target,
        [DamagePart::new(amount).with_tags(Tags::new([kind]))],
    )
}

#[test]
fn a_corrupted_mainframe_takes_more_digital_damage_until_its_wither_expires() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let mainframe = spawn(&mut harness, "mainframe", [640.0, 300.0]);
        harness.step(1);
        // The threshold is 20 plus a tenth of 100; a corruption hit of 60
        // lands 30 at the half resistance: a full bucket corrupts.
        assert_eq!(
            digital(mainframe, "digital.corruption", 60.0).apply(harness.world_mut()),
            Hit::Hurt
        );
        harness.step(1);
        let world = harness.world();
        assert!(
            world.get::<Effects>(mainframe).unwrap().has("corrupted"),
            "dt {dt}"
        );
        assert!(
            world
                .get::<Tags>(mainframe)
                .unwrap()
                .has("status.corrupted")
        );
        let health = world.get::<Health>(mainframe).unwrap().current();
        assert_eq!(health, 72.0, "dt {dt}: 30, two of it into the shield");
        // Withered, the digital factor doubles: an overload of 10 lands whole.
        digital(mainframe, "digital.overload", 10.0).apply(harness.world_mut());
        assert_eq!(
            harness.world().get::<Health>(mainframe).unwrap().current(),
            62.0,
            "dt {dt}"
        );
        harness.step(frames_for(3.1, dt));
        assert!(
            !harness
                .world()
                .get::<Effects>(mainframe)
                .unwrap()
                .has("corrupted")
        );
        digital(mainframe, "digital.overload", 10.0).apply(harness.world_mut());
        assert_eq!(
            harness.world().get::<Health>(mainframe).unwrap().current(),
            57.0,
            "dt {dt}: halved again"
        );
    }
}

#[test]
fn the_ships_cannon_fires_on_fire_and_reloads() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let ship = spawn(&mut harness, "ship", [640.0, 600.0]);
        harness.step(1);
        // A press every tenth of a second, the cannon's cooldown: four
        // charges, then none until one comes back half a second later.
        for _ in 0..5 {
            harness.press_action("fire");
            harness.step(1);
            harness.release_action("fire");
            harness.step(frames_for(0.1, dt) - 1);
        }
        assert_eq!(
            traced_steps(&harness, ship, "ability_used").len(),
            4,
            "dt {dt}: four charges: {:?}",
            traced_steps(&harness, ship, "ability_used")
        );
        // The first bullet has already flown out of the view and gone; the
        // rest carry the ship as their source.
        let bullets = tagged(&harness, "bullet");
        assert!(bullets.len() >= 3, "dt {dt}: {}", bullets.len());
        for bullet in bullets {
            assert_eq!(
                harness.world().get::<Source>(bullet),
                Some(&Source::new(ship))
            );
        }
        let abilities = harness.world().get::<Abilities>(ship).unwrap();
        assert_eq!(abilities.charges("cannon"), Some(0), "dt {dt}");
        harness.step(frames_for(0.5, dt));
        assert!(
            harness
                .world()
                .get::<Abilities>(ship)
                .unwrap()
                .ready("cannon"),
            "dt {dt}"
        );
        harness.press_action("fire");
        harness.step(1);
        harness.release_action("fire");
        assert_eq!(
            traced_steps(&harness, ship, "ability_used").len(),
            5,
            "dt {dt}: reloaded"
        );
    }
}

#[test]
fn the_turrets_pulse_hits_once_per_window() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let turret = spawn(&mut harness, "turret", [200.0, 100.0]);
        // A player's hurtbox inside the pulse's circle, clear of the
        // turret's bullets, which fly down from it.
        let target = harness.world_mut().spawn_with((
            Position(Vec2::new(230.0, 100.0)),
            PrevPosition(Vec2::new(230.0, 100.0)),
            Transform::from_position(Vec2::new(230.0, 100.0)),
            Hurtbox::new(HitShape::circle(4.0)),
            Health::new(10.0),
            Tags::new(["team.player"]),
        ));
        harness.step(frames_for(2.5, dt));
        let hits = traced_steps(&harness, target, "damaged");
        assert_eq!(hits.len(), 2, "dt {dt}: once per window: {hits:?}");
        // Fixed steps, sixty a second at either frame length.
        let expected = [61_u64, 121];
        assert!(
            hits.iter()
                .zip(expected)
                .all(|(&hit, at)| hit.abs_diff(at) <= 1),
            "dt {dt}: on the interval: {hits:?} vs {expected:?}"
        );
        assert_eq!(
            harness.world().get::<Health>(target).unwrap().current(),
            8.0
        );
        assert_eq!(traced_steps(&harness, turret, "ability_used").len(), 2);
    }
}

// M74: an effect's source named after a load, a rung a tag skips, and a
// game's own damage numbers.

/// A game's save of one target's effects, with the game's own ID for their
/// source: the serials the drone applied.
#[derive(serde::Serialize, serde::Deserialize)]
struct Etched {
    effects: Effects,
    from_drone: Vec<u64>,
}

impl SaveData for Etched {
    const SCHEMA: u32 = 1;
}

#[test]
fn a_drones_acid_named_after_a_load_credits_the_drone() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let drone = harness.world_mut().spawn_with((Tags::new(["team.enemy"]),));
        let mainframe = spawn(&mut harness, "mainframe", [640.0, 300.0]);
        harness.step(1);
        // Acid eating in, 2 each half second for two seconds.
        let acid = Effect::new()
            .with_duration(2.0)
            .with_period(0.5)
            .with_typed_damage("biological.acid", 2.0);
        EffectRequest::new(mainframe, DataRef::inline(acid))
            .with_source(drone)
            .apply(harness.world_mut());
        // The game's save holds the effects and its own ID for the source.
        let effects = harness.world().get::<Effects>(mainframe).unwrap().clone();
        let from_drone = effects
            .instances()
            .filter(|instance| instance.source() == Some(drone))
            .map(EffectInstance::serial)
            .collect();
        let dir = std::env::temp_dir().join(format!(
            "tungsten_recipe_acid_{}_{}",
            std::process::id(),
            frames_for(1.0, dt)
        ));
        let slots = SaveSlots::new(Some(dir.clone()));
        slots
            .save(
                "acid",
                &Etched {
                    effects,
                    from_drone,
                },
            )
            .unwrap();
        let loaded: Etched = slots.load("acid").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // A save holds no entity: its load names the source again.
        let mut effects = loaded.effects;
        assert!(
            effects
                .instances()
                .all(|instance| instance.source().is_none()),
            "dt {dt}"
        );
        for serial in loaded.from_drone {
            assert!(effects.set_source(serial, Some(drone)), "dt {dt}");
        }
        harness.world_mut().insert(mainframe, effects);
        let mut ticks = Vec::new();
        for _ in 0..frames_for(1.1, dt) {
            harness.step(1);
            ticks.extend(
                harness
                    .events::<Damaged>()
                    .filter(|hit| hit.entity == mainframe && hit.periodic)
                    .map(|hit| {
                        (
                            hit.source,
                            hit.kinds.has(harness.world(), "biological.acid"),
                        )
                    }),
            );
        }
        assert_eq!(
            ticks,
            [(Some(drone), true); 2],
            "dt {dt}: an acid tick each half second, each the drone's"
        );
    }
}

#[test]
fn an_acid_stream_builds_through_a_blocked_rung_to_its_top() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let mainframe = spawn(&mut harness, "mainframe", [640.0, 300.0]);
        harness.step(1);
        // Acid 4 a frame against the threshold of 30: `eaten` takes the
        // bucket at half, then its `blocked_by` skips it while the mainframe
        // is eaten, so the acid fills on to the top rung's whole threshold.
        let tagged_with = |harness: &Harness, tag: &str| {
            harness
                .world()
                .get::<Tags>(mainframe)
                .is_some_and(|tags| tags.has(tag))
        };
        let (mut eaten_at, mut dissolved_at) = (None, None);
        for frame in 0..frames_for(1.5, dt) {
            DamageRequest::from_parts(
                mainframe,
                [DamagePart::new(4.0).with_tags(Tags::new(["biological.acid"]))],
            )
            .queue(harness.world_mut());
            harness.step(1);
            if eaten_at.is_none() && tagged_with(&harness, "status.eaten") {
                eaten_at = Some(frame);
            }
            if tagged_with(&harness, "status.dissolved") {
                dissolved_at = Some(frame);
                break;
            }
        }
        let eaten_at = eaten_at.expect("eaten");
        let dissolved_at = dissolved_at.unwrap_or_else(|| panic!("dt {dt}: never dissolved"));
        assert!(
            eaten_at < dissolved_at,
            "dt {dt}: eaten at {eaten_at}, dissolved at {dissolved_at}"
        );
        assert!(tagged_with(&harness, "status.eaten"), "dt {dt}: it held");
    }
}

/// Gold, the game's crit colour, and the virus type's green.
const GOLD: [u8; 4] = [255, 218, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

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

#[test]
fn a_crits_parts_float_gold_from_the_games_code() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set().with(GameNumbers), dt);
        // No cues of its own: the game's numbers alone.
        let target = harness.world_mut().spawn_with((Health::new(100.0),));
        let hacker = harness.world_mut().spawn();
        harness.step(1);
        // One step: a crit of virus 4 and exploit 2, and a plain virus 3.
        let part = |amount: f32, kind: &str| DamagePart::new(amount).with_tags(Tags::new([kind]));
        let world = harness.world_mut();
        DamageRequest::from_parts(
            target,
            [part(4.0, "digital.virus"), part(2.0, "digital.exploit")],
        )
        .with_source(hacker)
        .with_position(Vec2::new(640.0, 200.0))
        .with_crit(true)
        .queue(world);
        DamageRequest::from_parts(target, [part(3.0, "digital.virus")])
            .with_position(Vec2::new(680.0, 200.0))
            .queue(world);
        harness.step(1);
        assert_eq!(
            typed_labels(&harness),
            [("3".to_string(), GREEN), ("6".to_string(), GOLD)],
            "dt {dt}: the plain part in the virus style, the crit's two summed in gold"
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

// M75: a drone saved mid-volley: its state machine, its piercing beam's
// passed targets, its ram's touches and its mover's turn, the targets named
// again after the load by the game's own IDs.

/// The dts M75's save recipes run at: one step a frame, two, and under one.
const SAVE_DTS: [f32; 3] = [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0];

/// A drone's states: its approach, its volley, which turns it down and
/// fires a beam, and its retreat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Drone {
    Approach,
    Volley,
    Retreat,
}

fn drone_approach(world: &mut World, drone: Entity) {
    let machine = world.get_mut::<StateMachine<Drone>>(drone).unwrap();
    if machine.elapsed() > 0.2 {
        machine.go(Drone::Volley);
    }
}

/// The volley's beam: a piercing bolt four times as long as its step.
fn drone_volley(world: &mut World, drone: Entity) {
    world
        .get_mut::<TopDownMover>(drone)
        .unwrap()
        .set_direction(Vec2::Y);
    let at = world.get::<Position>(drone).unwrap().0;
    world.spawn_with((
        Position(at),
        PrevPosition(at),
        Transform::from_position(at),
        LinearMotion::new(Vec2::new(0.0, 600.0)),
        Hitbox::new(HitShape::aabb(Vec2::new(3.0, 40.0)), 1.0)
            .with_filter(TagFilter::new().all(["team.player"])),
        Projectile::new().with_pierce(3).with_range(600.0),
    ));
}

fn drone_retreat(world: &mut World, drone: Entity) {
    world
        .get_mut::<TopDownMover>(drone)
        .unwrap()
        .set_direction(-Vec2::Y);
    world.resource_mut::<DroneTrail>().retreats += 1;
}

/// One fixed step of the drone and its beam, after the kit's react phase.
#[derive(Debug, Clone, PartialEq)]
struct DroneStep {
    /// State, previous, pending, held and elapsed.
    machine: (Drone, Option<Drone>, Option<Drone>, bool, f32),
    /// The step's hits: (hitter, target).
    struck: Vec<(Entity, Entity)>,
    at: Vec2,
    beam: Option<Vec2>,
}

#[derive(Default)]
struct DroneTrail {
    steps: Vec<DroneStep>,
    retreats: u32,
}

/// The game's drone: its machine's hooks and its trail.
struct DroneGame;

impl Plugin for DroneGame {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        StateMachinePlugin::<Drone>::new("drone_states")
            .on_update(Drone::Approach, drone_approach)
            .on_enter(Drone::Volley, drone_volley)
            .on_enter(Drone::Retreat, drone_retreat)
            .build(schedule, world);
        world.insert_resource(DroneTrail::default());
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
            let Some((drone, machine)) = world
                .query::<(Entity, &StateMachine<Drone>)>()
                .next()
                .map(|(drone, m)| {
                    let at = (
                        m.state(),
                        m.previous(),
                        m.pending(),
                        m.is_held(),
                        m.elapsed(),
                    );
                    (drone, at)
                })
            else {
                return;
            };
            let at = world.get::<Position>(drone).unwrap().0;
            let beam = world
                .query::<(&Projectile, &Position)>()
                .next()
                .map(|(_, at)| at.0);
            world.resource_mut::<DroneTrail>().steps.push(DroneStep {
                machine,
                struck,
                at,
                beam,
            });
        };
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(system("drone_trail", run)),
        );
    }
}

/// The game's save of its drone and its beam in flight: the kit items, and
/// its own IDs, an index into its targets, for the entities they name.
#[derive(serde::Serialize, serde::Deserialize)]
struct DroneSave {
    machine: StateMachine<Drone>,
    mover: TopDownMover,
    ram: ContactDamage,
    ram_touches: Vec<(usize, f32, f32)>,
    beam: Hitbox,
    beam_passed: Vec<usize>,
    beam_flight: Projectile,
}

impl SaveData for DroneSave {
    const SCHEMA: u32 = 1;
}

/// A drone run at `dt` from its save point on: its trail, its retreats and
/// its targets. With `load`, the game saves it there and loads it back,
/// naming its targets again or not.
fn drone_run(dt: f32, load: Option<bool>) -> (Vec<DroneStep>, u32, Vec<Entity>) {
    let mut harness = recipe(GENRE, KitPlugins::set().with(DroneGame), dt);
    let world = harness.world_mut();
    let mut mover = TopDownMover::new(150.0).with_acceleration(450.0);
    mover.set_direction(Vec2::X);
    let drone = world.spawn_with((
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
        Transform::from_position(Vec2::ZERO),
        Tags::new(["team.enemy"]),
        mover,
        ContactDamage::new(1.0)
            .with_interval(0.2)
            .with_shape(HitShape::circle(20.0), Vec2::ZERO)
            .with_filter(TagFilter::new().all(["team.player"])),
        StateMachine::new(Drone::Approach),
    ));
    // Two targets down the beam's line, and one on the drone's path, in its
    // ram's reach from before the volley to the end.
    let targets: Vec<Entity> = [
        Vec2::new(10.0, 80.0),
        Vec2::new(10.0, 200.0),
        Vec2::new(32.0, 8.0),
    ]
    .into_iter()
    .map(|at| {
        world.spawn_with((
            Transform::from_position(at),
            Tags::new(["team.player"]),
            Hurtbox::new(HitShape::circle(8.0)),
            Health::new(1000.0),
        ))
    })
    .collect();
    // Mid-volley: the beam inside the first target, the ram between two
    // touches and the turn under way. The game stuns it with a retreat
    // asked for: a transition pending, and the machine held.
    harness.step(frames_for(0.38, dt));
    let world = harness.world_mut();
    let machine = world.get_mut::<StateMachine<Drone>>(drone).unwrap();
    assert_eq!(machine.state(), Drone::Volley, "dt {dt}");
    machine.go(Drone::Retreat);
    machine.set_held(true);
    if let Some(named_again) = load {
        let id = |entity: &Entity| targets.iter().position(|target| target == entity).unwrap();
        let (beam, beam_hitbox) = world
            .query::<(Entity, &Hitbox)>()
            .next()
            .map(|(beam, hitbox)| (beam, hitbox.clone()))
            .unwrap();
        let ram = world.get::<ContactDamage>(drone).unwrap().clone();
        assert_eq!(beam_hitbox.hit_targets(), &targets[..1], "dt {dt}");
        assert_eq!(ram.touches().count(), 1, "dt {dt}");
        let save = DroneSave {
            machine: world.get::<StateMachine<Drone>>(drone).unwrap().clone(),
            mover: world.get::<TopDownMover>(drone).unwrap().clone(),
            ram_touches: ram
                .touches()
                .map(|touch| (id(&touch.target()), touch.since_hit(), touch.since_met()))
                .collect(),
            ram,
            beam_passed: beam_hitbox.hit_targets().iter().map(id).collect(),
            beam: beam_hitbox,
            beam_flight: world.get::<Projectile>(beam).unwrap().clone(),
        };
        let dir = std::env::temp_dir().join(format!(
            "tungsten_recipe_drone_{}_{}_{named_again}",
            std::process::id(),
            frames_for(1.0, dt)
        ));
        let slots = SaveSlots::new(Some(dir.clone()));
        slots.save("drone", &save).unwrap();
        let loaded: DroneSave = slots.load("drone").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let (mut ram, mut beam_hitbox) = (loaded.ram, loaded.beam);
        assert_eq!(loaded.beam_flight.hits(), 1, "dt {dt}");
        if named_again {
            ram.set_touches(
                loaded
                    .ram_touches
                    .iter()
                    .map(|&(id, hit, met)| ContactTouch::new(targets[id], hit, met)),
            );
            beam_hitbox.set_hit_targets(loaded.beam_passed.iter().map(|&id| targets[id]));
        }
        world.insert(drone, loaded.machine);
        world.insert(drone, loaded.mover);
        world.insert(drone, ram);
        world.insert(beam, beam_hitbox);
        world.insert(beam, loaded.beam_flight);
    }
    let start = world.resource::<DroneTrail>().steps.len();
    harness.step(frames_for(0.1, dt));
    harness
        .world_mut()
        .get_mut::<StateMachine<Drone>>(drone)
        .unwrap()
        .set_held(false);
    harness.step(frames_for(0.5, dt));
    let trail = harness.world().resource::<DroneTrail>();
    (trail.steps[start..].to_vec(), trail.retreats, targets)
}

#[test]
fn a_drone_saved_mid_volley_resumes_exactly() {
    for dt in SAVE_DTS {
        let (kept, retreats, targets) = drone_run(dt, None);
        let (loaded, loaded_retreats, _) = drone_run(dt, Some(true));
        assert!(kept.len() > 30, "dt {dt}");
        assert_eq!((retreats, loaded_retreats), (1, 1), "dt {dt}: once");
        // A bodiless mover takes no still step: all of it, exactly.
        assert_eq!(loaded, kept, "dt {dt}");
        let passed = |steps: &[DroneStep]| {
            steps
                .iter()
                .flat_map(|step| step.struck.iter().map(|&(_, target)| target))
                .collect::<Vec<_>>()
        };
        let hits = passed(&kept);
        assert!(
            hits.contains(&targets[1]) && hits.contains(&targets[2]),
            "dt {dt}: the beam and the ram went on: {hits:?}"
        );
        assert!(kept.iter().any(|step| step.beam.is_some()), "dt {dt}");
        assert!(kept[0].struck.is_empty(), "dt {dt}");
        // Loaded without its targets named, the beam and the ram hit at once.
        let (unnamed, _, _) = drone_run(dt, Some(false));
        assert_eq!(unnamed[0].struck.len(), 2, "dt {dt}: {:?}", unnamed[0]);
    }
}

// 7.10c, Step 2: Static Front's breacher (SF2, SF6, SF25).

/// The uses of `name` by `breacher` in the last frame.
fn used(harness: &Harness, breacher: Entity, name: &str) -> usize {
    harness
        .events::<AbilityUsed>()
        .filter(|used| used.entity == breacher && used.name == name)
        .count()
}

/// The breacher, aiming at a wing to its left.
fn breacher(dt: f32) -> (Harness, Entity) {
    let mut harness = recipe(GENRE, KitPlugins::set(), dt);
    let breacher = spawn(&mut harness, "breacher", [0.0, 0.0]);
    spawn(&mut harness, "wing", [-200.0, 0.0]);
    harness.step(1);
    (harness, breacher)
}

/// The velocities of the entities tagged `tag`.
fn velocities(harness: &Harness, tag: &str) -> Vec<Vec2> {
    tagged(harness, tag)
        .iter()
        .map(|&entity| {
            harness
                .world()
                .get::<LinearMotion>(entity)
                .unwrap()
                .velocity()
        })
        .collect()
}

#[test]
fn the_smg_fires_five_rounds_in_half_a_second_held() {
    for dt in DTS {
        let (mut harness, breacher) = breacher(dt);
        harness.press_action("fire");
        let mut rounds = 0;
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            rounds += used(&harness, breacher, "smg");
        }
        assert_eq!(rounds, 5, "dt {dt}: a round each 0.1 s held");
        let flown = velocities(&harness, "round");
        assert!(
            flown.len() == 5
                && flown
                    .iter()
                    .all(|v| (*v - Vec2::new(-900.0, 0.0)).length() < 1e-2),
            "dt {dt}: each along the aim, at the wing: {flown:?}"
        );
        harness.release_action("fire");
        for _ in 0..frames_for(0.5, dt) {
            harness.step(1);
            rounds += used(&harness, breacher, "smg");
        }
        assert_eq!(rounds, 5, "dt {dt}: none after the release");
    }
}

#[test]
fn the_dodge_follows_the_move_stick_not_the_aim() {
    for dt in DTS {
        let (mut harness, breacher) = breacher(dt);
        harness.press_action("move_right");
        harness.press_action("dash");
        harness.step(1);
        harness.release_action("dash");
        let aim = harness
            .world()
            .get::<Aim>(breacher)
            .and_then(Aim::direction)
            .unwrap();
        assert!(aim.x < -0.99, "dt {dt}: aiming at the wing: {aim}");
        let [along] = velocities(&harness, "dodge_dust")[..] else {
            panic!("dt {dt}: one dodge");
        };
        assert!(
            (along - Vec2::new(400.0, 0.0)).length() < 1e-2,
            "dt {dt}: along the move stick: {along}"
        );
        // At rest, past its cooldown and the dust's life, it takes the aim.
        harness.release_action("move_right");
        harness.step(frames_for(1.0, dt));
        harness.press_action("dash");
        harness.step(1);
        let [still] = velocities(&harness, "dodge_dust")[..] else {
            panic!("dt {dt}: one dodge at rest");
        };
        assert!(
            (still - Vec2::new(-400.0, 0.0)).length() < 1e-2,
            "dt {dt}: the aim at rest: {still}"
        );
    }
}

#[test]
fn the_reload_sweep_reads_the_refill() {
    for dt in DTS {
        let (mut harness, breacher) = breacher(dt);
        let smg = |harness: &Harness| {
            let abilities = harness.world().get::<Abilities>(breacher).unwrap();
            (abilities.charges("smg"), abilities.refill_left("smg"))
        };
        harness.press_action("fire");
        let mut rounds = 0;
        for _ in 0..frames_for(4.0, dt) {
            if smg(&harness).0 == Some(0) {
                break;
            }
            harness.step(1);
            rounds += used(&harness, breacher, "smg");
        }
        harness.release_action("fire");
        assert_eq!(rounds, 24, "dt {dt}: the magazine");
        // From 1.6 after the last round, down each frame, to every round back.
        let first = smg(&harness).1.unwrap();
        assert!(
            first <= 1.6 && first >= 1.6 - dt - 1e-4,
            "dt {dt}: the sweep starts at the refill: {first}"
        );
        let (mut left, mut frames) = (first, 0);
        loop {
            harness.step(1);
            frames += 1;
            assert!(frames <= frames_for(2.0, dt), "dt {dt}: no reload");
            match smg(&harness) {
                (_, Some(now)) => {
                    assert!(
                        now < left,
                        "dt {dt}: the sweep runs down: {now} after {left}"
                    );
                    left = now;
                }
                (charges, None) => {
                    assert_eq!(charges, Some(24), "dt {dt}: every round back");
                    break;
                }
            }
        }
        let elapsed = frames as f32 * dt;
        assert!(
            (elapsed - first).abs() <= dt + 1e-4,
            "dt {dt}: back after {elapsed} s of {first}"
        );
    }
}

// 7.10c, Step 3: a dry fire on `no_charge` (SF6).

#[test]
fn a_dry_fire_clicks_on_no_charge() {
    for dt in DTS {
        let (mut harness, breacher) = breacher(dt);
        let clicks = |harness: &Harness| {
            harness
                .audio()
                .iter()
                .filter(|command| matches!(command, AudioCommand::Play { .. }))
                .count()
        };
        harness.press_action("fire");
        let mut rounds = 0;
        for _ in 0..frames_for(4.0, dt) {
            if rounds == 24 {
                break;
            }
            harness.step(1);
            rounds += used(&harness, breacher, "smg");
            assert_eq!(clicks(&harness), 0, "dt {dt}: no click while rounds remain");
        }
        assert_eq!(rounds, 24, "dt {dt}: the magazine");
        // Held through the reload: a click once the 0.1 s cooldown is out,
        // then each 0.25 s, the rule's `min_interval`.
        let mut heard = Vec::new();
        for frame in 0..frames_for(1.5, dt) {
            harness.step(1);
            assert_eq!(used(&harness, breacher, "smg"), 0, "dt {dt}: reloading");
            for _ in 0..clicks(&harness) {
                heard.push(frame);
            }
        }
        assert_eq!(heard.len(), 6, "dt {dt}: {heard:?}");
        // Fifteen steps apart, a frame's two steps rounding at dt 1/30.
        let steps = (dt * 60.0).round() as u32;
        assert!(
            heard
                .windows(2)
                .all(|pair| (14..=16).contains(&((pair[1] - pair[0]) * steps))),
            "dt {dt}: each 0.25 s: {heard:?}"
        );
    }
}

// 7.10c, Step 4: Static Front's lobber, its grenade's arc kept (S-53, S-186).

#[test]
fn a_lobbed_grenade_lands_on_the_breacher() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let breacher = spawn(&mut harness, "breacher", [300.0, 0.0]);
        spawn(&mut harness, "lobber", [0.0, 0.0]);
        let target = point_of(&harness, breacher);
        let mut thrown = None;
        for _ in 0..frames_for(1.5, dt) {
            harness.step(1);
            if let Some(&grenade) = tagged(&harness, "grenade").first() {
                thrown = Some(grenade);
                break;
            }
        }
        let grenade = thrown.unwrap_or_else(|| panic!("dt {dt}: a lob each second"));
        assert_eq!(
            harness
                .world()
                .get::<LinearMotion>(grenade)
                .unwrap()
                .acceleration(),
            Vec2::new(0.0, 600.0),
            "dt {dt}: its prefab's arc kept"
        );
        // 300 px at 375 px/s: a 0.8 s flight, 48 fixed steps, rising 48 px.
        let steps = (dt * 60.0).round() as u32;
        let mut highest = 0.0_f32;
        for _ in 0..48 / steps {
            harness.step(1);
            highest = highest.min(point_of(&harness, grenade).y);
        }
        let landed = point_of(&harness, grenade);
        assert!(
            (landed - target).length() < 0.5,
            "dt {dt}: on the breacher at its last step: {landed}"
        );
        assert!(highest < -45.0, "dt {dt}: an arc: {highest}");
    }
}

// 7.10c, Step 6: Static Front's cryo sprayer, a cone stopped by walls (SF30, S-15).

#[test]
fn the_cryo_sprayer_hits_the_drones_its_rays_reach() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        spawn(&mut harness, "cryo_trooper", [0.0, 0.0]);
        // Seven rays at 0, ±4.7, ±9.3 and ±14 degrees along +x; the
        // bulkhead covers y 30 to 70 at x 150, across the upper ray.
        let near = spawn(&mut harness, "drone", [200.0, 10.0]);
        let far = spawn(&mut harness, "drone", [320.0, -30.0]);
        let walled = spawn(&mut harness, "drone", [260.0, 60.0]);
        spawn(&mut harness, "bulkhead", [150.0, 50.0]);
        harness.step(1);
        harness.press_action("fire");
        harness.step(1);
        harness.release_action("fire");
        let mut struck: Vec<Entity> = harness.events::<Struck>().map(|hit| hit.target).collect();
        struck.sort_by_key(|entity| entity.id());
        assert_eq!(
            struck,
            [near, far],
            "dt {dt}: each once, none behind the wall"
        );
        harness.step(frames_for(0.2, dt));
        let health = |entity: Entity| harness.world().get::<Health>(entity).unwrap().current();
        assert_eq!(
            [near, far, walled].map(health),
            [17.0, 17.0, 20.0],
            "dt {dt}: 3 each where the rays reach"
        );
    }
}

// 7.10c, Step 8: Static Front's lobber, its grenade's blast where it lands
// (S-73).

#[test]
fn the_lobbers_grenade_blasts_where_it_lands() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        // The floor's top at y 8, under the breacher the lob lands on.
        let floor = spawn(&mut harness, "floor", [300.0, 16.0]);
        spawn(&mut harness, "breacher", [300.0, 0.0]);
        let lobber = spawn(&mut harness, "lobber", [0.0, 0.0]);
        let near = spawn(&mut harness, "sandbag", [340.0, 0.0]);
        let far = spawn(&mut harness, "sandbag", [420.0, 0.0]);
        let mut hits = Vec::new();
        let mut collect = |harness: &Harness| {
            hits.extend(
                harness
                    .events::<Damaged>()
                    .filter(|hit| hit.entity == near || hit.entity == far)
                    .map(|hit| (hit.entity, hit.amount, hit.source)),
            );
        };
        let mut landed = None;
        for _ in 0..frames_for(2.5, dt) {
            harness.step(1);
            collect(&harness);
            if let Some(expired) = harness.events::<ProjectileExpired>().next().copied() {
                let played = harness
                    .audio()
                    .iter()
                    .filter(|command| matches!(command, AudioCommand::Play { .. }))
                    .count();
                landed = Some((expired, played));
                break;
            }
        }
        let (expired, played) = landed.unwrap_or_else(|| panic!("dt {dt}: the grenade lands"));
        // Past the breacher's point, on the floor's top.
        let at = expired.position;
        assert!(
            (at.y - 8.0).abs() < 1e-3 && at.x > 305.0 && at.x < 320.0,
            "dt {dt}: on the floor: {at}"
        );
        assert_eq!(
            expired.wall.and_then(|wall| wall.entity),
            Some(floor),
            "dt {dt}"
        );
        // Its look, the `expired` cue: a burst where it landed and a blip.
        let bursts = tagged(&harness, "burst");
        assert_eq!(bursts.len(), 1, "dt {dt}");
        assert!(
            (point_of(&harness, bursts[0]) - at).length() < 1e-3,
            "dt {dt}"
        );
        assert_eq!(played, 1, "dt {dt}: a blip");
        // Its blast, at the next step's resolve: 20 at 1 - d / 64 on the
        // near sandbag, from the lobber; the far one out of its reach.
        for _ in 0..2 {
            harness.step(1);
            collect(&harness);
        }
        let d = (Vec2::new(340.0, 0.0) - at).length();
        let want = 20.0 * (1.0 - d / 64.0);
        assert_eq!(hits.len(), 1, "dt {dt}: {hits:?}");
        assert_eq!((hits[0].0, hits[0].2), (near, Some(lobber)), "dt {dt}");
        assert!(
            (hits[0].1 - want).abs() < 1e-3,
            "dt {dt}: {hits:?} of {want}"
        );
        let health = |entity: Entity| harness.world().get::<Health>(entity).unwrap().current();
        assert!((health(near) - (40.0 - want)).abs() < 1e-3, "dt {dt}");
        assert_eq!(health(far), 40.0, "dt {dt}");
    }
}

// 7.10c, Step 5 (place-and-caps): a carrier's escorts, three at most (S-18).

#[test]
fn a_carrier_launches_no_more_than_its_cap() {
    use crate::kit::Spawner;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let carrier = spawn(&mut harness, "escort_carrier", [200.0, 100.0]);
        let remaining =
            |harness: &Harness| harness.world().get::<Spawner>(carrier).unwrap().remaining();
        // Due at 0.25, 0.5 and 0.75 s: three.
        harness.step(frames_for(0.9, dt));
        assert_eq!(tagged(&harness, "escort").len(), 3, "dt {dt}: three live");
        assert_eq!(remaining(&harness), Some(2), "dt {dt}");
        // The fourth, due at 1 s, refused while full and kept in the count.
        harness.step(frames_for(0.25, dt));
        assert_eq!(tagged(&harness, "escort").len(), 3, "dt {dt}: refused");
        assert_eq!(remaining(&harness), Some(2), "dt {dt}: kept");
        // Two destroyed, gone 0.1 s after: two more, then none.
        for escort in tagged(&harness, "escort").into_iter().take(2) {
            damage(harness.world_mut(), escort, 1.0);
        }
        harness.step(frames_for(1.0, dt));
        assert_eq!(tagged(&harness, "escort").len(), 3, "dt {dt}: two more");
        assert_eq!(remaining(&harness), Some(0), "dt {dt}: five in all");
        harness.step(frames_for(1.0, dt));
        assert_eq!(tagged(&harness, "escort").len(), 3, "dt {dt}: then none");
    }
}

// place-and-caps, Step 2: a sapper's mines, three at most (S-17, S-18).

#[test]
fn a_fourth_mine_is_refused_at_its_cap() {
    use crate::kit::{Used, use_ability};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let sapper = spawn(&mut harness, "sapper", [100.0, 100.0]);
        spawn(&mut harness, "wing", [300.0, 100.0]);
        harness.step(1);
        let blips = |harness: &Harness| {
            harness
                .audio()
                .iter()
                .filter(|command| matches!(command, AudioCommand::Play { .. }))
                .count()
        };
        for laid in 1..=3 {
            assert_eq!(
                use_ability(harness.world_mut(), sapper, "lay_mine"),
                Used::Used,
                "dt {dt}: mine {laid}"
            );
            harness.step(frames_for(0.3, dt));
        }
        assert_eq!(tagged(&harness, "claymore").len(), 3, "dt {dt}: three laid");
        assert_eq!(
            use_ability(harness.world_mut(), sapper, "lay_mine"),
            Used::Capped,
            "dt {dt}: the fourth"
        );
        assert!(
            harness
                .world()
                .get::<Abilities>(sapper)
                .unwrap()
                .ready("lay_mine"),
            "dt {dt}: nothing spent"
        );
        harness.step(1);
        assert_eq!(blips(&harness), 1, "dt {dt}: its blip");
        assert_eq!(tagged(&harness, "claymore").len(), 3, "dt {dt}");
    }
}

/// 7.10g: a missile drawn facing right turns back under its acceleration:
/// its facing turns it with the heading, and past the vertical its mirror
/// flips it so the art stays right side up.
#[test]
fn a_missile_turns_back_right_side_up() {
    use tungsten_core::components::SpriteFlip;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let missile = spawn(&mut harness, "missile", [0.0, 4000.0]);
        let seen = |harness: &Harness| {
            let world = harness.world();
            let heading = world.get::<Velocity>(missile).unwrap().0;
            let rotation = world.get::<Transform>(missile).unwrap().rotation;
            let flipped = world.get::<SpriteFlip>(missile).is_some_and(|flip| flip.0);
            (heading, Vec2::from_angle(rotation), flipped)
        };
        harness.step(frames_for(0.1, dt));
        let (heading, nose, flipped) = seen(&harness);
        assert!(heading.x > 0.0 && !flipped, "dt {dt}: outbound, unflipped");
        assert!(
            (nose - heading.normalize()).length() < 1e-3,
            "dt {dt}: turned to its heading"
        );
        harness.step(frames_for(0.9, dt));
        let (heading, nose, flipped) = seen(&harness);
        assert!(heading.x < 0.0 && flipped, "dt {dt}: back, flipped");
        assert!(nose.x > 0.0, "dt {dt}: right side up, not upside down");
        let mirrored = Vec2::new(-heading.x, heading.y).normalize();
        assert!(
            (Vec2::new(nose.x, -nose.y) - mirrored).length() < 1e-3,
            "dt {dt}: the flipped nose along the heading"
        );
    }
}

/// 7.10g: the stray drone flies off the arena's right edge; past the
/// `WorldBounds` it is killed, and its death policy takes it at once.
#[test]
fn a_drone_past_the_world_dies() {
    use crate::kit::WorldBounds;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        harness
            .world_mut()
            .insert_resource(WorldBounds::new(Vec2::ZERO, Vec2::new(640.0, 360.0)));
        let drone = spawn(&mut harness, "stray_drone", [600.0, 100.0]);
        let mut killed_at = None;
        let mut last = Vec2::ZERO;
        for frame in 0..frames_for(1.0, dt) {
            harness.step(1);
            if let Some(position) = harness.world().get::<Position>(drone) {
                last = position.0;
            }
            if harness
                .events::<Damaged>()
                .any(|hit| hit.entity == drone && hit.killed)
            {
                killed_at = Some(frame);
                break;
            }
        }
        assert!(killed_at.is_some(), "dt {dt}: killed past the edge");
        assert!(last.x > 630.0, "dt {dt}: at the edge: {last}");
        harness.step(2);
        assert!(
            !harness.world().is_alive(drone),
            "dt {dt}: its policy took it"
        );
    }
}

/// 7.10g: the patrol sentry holds 1 s at each end of its beat: still for
/// its first second, out over the next, held at the far end its third,
/// and back.
#[test]
fn a_sentry_waits_at_each_end() {
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let sentry = spawn(&mut harness, "patrol_sentry", [0.0, 6000.0]);
        let x = |harness: &Harness| body_at(harness, sentry).x;
        harness.step(frames_for(0.9, dt));
        assert_eq!(x(&harness), 0.0, "dt {dt}: waiting at its start");
        harness.step(frames_for(0.6, dt));
        assert!(
            (x(&harness) - 60.0).abs() < 4.0,
            "dt {dt}: half way out: {}",
            x(&harness)
        );
        harness.step(frames_for(1.0, dt));
        assert!((x(&harness) - 120.0).abs() < 1e-3, "dt {dt}: arrived");
        harness.step(frames_for(0.4, dt));
        assert!((x(&harness) - 120.0).abs() < 1e-3, "dt {dt}: still waiting");
        harness.step(frames_for(0.4, dt));
        assert!(x(&harness) < 119.0, "dt {dt}: and back: {}", x(&harness));
    }
}

/// A turret's phases, which `two_turrets_cycle_half_a_second_apart` times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Cycle {
    Charge,
    Fire,
}

/// 7.10g: the turrets charge 1 s and fire 0.5 s on timed transitions; the
/// second starts half a second into its charge, so it fires half a second
/// before the first, every cycle.
#[test]
fn two_turrets_cycle_half_a_second_apart() {
    use crate::kit::StateMachinePlugin;
    for dt in DTS {
        let cycle = StateMachinePlugin::<Cycle>::new("turret_cycle")
            .after(Cycle::Charge, 1.0, Cycle::Fire)
            .after(Cycle::Fire, 0.5, Cycle::Charge);
        let mut harness = recipe(GENRE, KitPlugins::set().with(cycle), dt);
        let first = spawn(&mut harness, "cycle_turret", [0.0, 7000.0]);
        let second = spawn(&mut harness, "cycle_turret", [100.0, 7000.0]);
        harness
            .world_mut()
            .insert(first, StateMachine::new(Cycle::Charge));
        harness
            .world_mut()
            .insert(second, StateMachine::new(Cycle::Charge).starting_at(0.5));
        let state = |harness: &Harness, turret| {
            harness
                .world()
                .get::<StateMachine<Cycle>>(turret)
                .unwrap()
                .state()
        };
        let mut fired = [Vec::new(), Vec::new()];
        let mut was = [Cycle::Charge; 2];
        for frame in 0..frames_for(4.0, dt) {
            harness.step(1);
            for (i, turret) in [first, second].into_iter().enumerate() {
                let now = state(&harness, turret);
                if now == Cycle::Fire && was[i] == Cycle::Charge {
                    fired[i].push(frame);
                }
                was[i] = now;
            }
        }
        let apart = frames_for(0.5, dt);
        assert_eq!(fired[0].len(), 2, "dt {dt}: {fired:?}");
        for (first_fired, second_fired) in fired[0].iter().zip(&fired[1]) {
            assert_eq!(first_fired - second_fired, apart, "dt {dt}: {fired:?}");
        }
        assert_eq!(
            fired[1][1] - fired[1][0],
            frames_for(1.5, dt),
            "dt {dt}: a 1.5 s cycle"
        );
    }
}

/// 7.10g: the charger's tell is a game's `state.charging` tag; its looks
/// show a ring that grows from a quarter of its size to its whole over the
/// tell's 0.5 s, and goes when the tag does.
#[test]
fn a_chargers_ring_grows_over_its_tell() {
    use tungsten_core::AnimationState;
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let charger = spawn(&mut harness, "charger", [800.0, 3000.0]);
        let ring = |harness: &Harness| {
            harness
                .world()
                .query::<(&AnimationState, &Transform)>()
                .find(|(state, _)| state.animation_id == "charge_ring")
                .map(|(_, transform)| transform.scale.x)
        };
        let charging = |harness: &mut Harness, on: bool| {
            let tags = harness.world_mut().get_mut::<Tags>(charger).unwrap();
            if on {
                tags.insert("state.charging");
            } else {
                tags.remove("state.charging");
            }
        };
        harness.step(1);
        assert_eq!(ring(&harness), None, "dt {dt}: no tell, no ring");
        charging(&mut harness, true);
        harness.step(1);
        assert_eq!(ring(&harness), Some(0.25), "dt {dt}: from a quarter");
        harness.step(frames_for(0.2, dt));
        let grown = ring(&harness).unwrap();
        assert!(
            (grown - 0.55).abs() < 1e-3,
            "dt {dt}: 0.2 s of 0.5: {grown}"
        );
        harness.step(frames_for(0.3, dt));
        assert!(
            (ring(&harness).unwrap() - 1.0).abs() < 1e-4,
            "dt {dt}: whole"
        );
        harness.step(frames_for(0.5, dt));
        assert!(
            (ring(&harness).unwrap() - 1.0).abs() < 1e-4,
            "dt {dt}: held"
        );
        charging(&mut harness, false);
        harness.step(1);
        assert_eq!(ring(&harness), None, "dt {dt}: gone with the tag");
    }
}

/// 7.10g: the frigate's health bar sits over its point, its fill falling
/// with each hit, and goes with the frigate at its death.
#[test]
fn a_frigates_bar_falls_and_goes() {
    use tungsten_core::{Sprite, Visibility};
    for dt in DTS {
        let mut harness = recipe(GENRE, KitPlugins::set(), dt);
        let white = super::register_sprite(&mut harness, "engine_white", 1, 1);
        let frigate = spawn(&mut harness, "frigate", [400.0, 3000.0]);
        let bar = |harness: &Harness| {
            let mut parts: Vec<(i32, Vec2, Vec2, bool)> = harness
                .world()
                .query::<(&Transform, &Sprite, &Visibility)>()
                .filter(|(_, sprite, _)| sprite.asset_id == white)
                .map(|(transform, sprite, visibility)| {
                    (
                        sprite.z_order,
                        transform.position,
                        transform.scale,
                        visibility.visible,
                    )
                })
                .collect();
            parts.sort_by_key(|part| part.0);
            parts
        };
        harness.step(1);
        let parts = bar(&harness);
        assert_eq!(parts.len(), 2, "dt {dt}: a back and a fill");
        assert!(parts.iter().all(|part| part.3), "dt {dt}: shown");
        // No sprite: centred over its point, 6 px over it, 6 tall.
        assert_eq!(parts[0].1, Vec2::new(368.0, 2988.0), "dt {dt}");
        assert_eq!(parts[1].2, Vec2::new(62.0, 4.0), "dt {dt}: full");
        damage(harness.world_mut(), frigate, 10.0);
        harness.step(1);
        assert_eq!(bar(&harness)[1].2, Vec2::new(31.0, 4.0), "dt {dt}: half");
        damage(harness.world_mut(), frigate, 10.0);
        let mut frames = 0;
        while !bar(&harness).is_empty() {
            assert!(frames < 4, "dt {dt}: the bar goes with its frigate");
            harness.step(1);
            frames += 1;
        }
        assert!(!harness.world().is_alive(frigate), "dt {dt}");
    }
}
