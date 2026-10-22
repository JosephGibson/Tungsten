//! The ball pit: slot 9 sets small balls down at its rate, at their size
//! and with their own clip, up to the cap; impact bursts by closing speed,
//! either side of a contact and bounded in a dense pile, the sparks'
//! rainbow and the vortex's inward spiral, and a pit of two thousand mixed
//! balls that holds.

use example_01_platformer::features::elements::systems::{ball_impacts, impact_sparks};
use example_01_platformer::features::elements::{IMPACT_BURSTS_PER_STEP, IMPACT_SPEED};
use example_01_platformer::features::level::TILE;
use example_01_platformer::features::toys::components::{Ball, BallSpawner, BlackHole, SmallBall};
use example_01_platformer::features::toys::systems::{hole_particles, remember_velocities};
use example_01_platformer::features::toys::{self, BALL_CAP, SMALL_BALL_RADIUS, SMALL_BALL_SCALE};
use example_01_platformer::shared::{BURST_CAP, Burst, spawn_burst};
use glam::Vec2;
use tungsten::core::{AnimationState, Collider, Entity, Particle, Shape, Transform};
use tungsten::testing::UiHarness;

use super::{aim_at, ball, body, bursts, choose, contact, kick, pin_camera, run};

/// A run with slot 9, the small balls, selected, aimed at the empty sky.
fn pitcher() -> UiHarness {
    let (mut harness, _) = run();
    pin_camera(&mut harness, Vec2::new(600.0, 200.0));
    aim_at(&mut harness, Vec2::new(1200.0, 500.0));
    choose(&mut harness, 8);
    harness
}

fn small_balls(harness: &UiHarness) -> Vec<Entity> {
    harness
        .world()
        .query::<(Entity, &SmallBall)>()
        .map(|(e, _)| e)
        .collect()
}

const IMPACT: &str = "ex10_small_ball_impact";

/// Despawns every burst, so a case counts its own.
fn clear_bursts(harness: &mut UiHarness) {
    let world = harness.world_mut();
    let all: Vec<Entity> = world.query::<(Entity, &Burst)>().map(|(e, _)| e).collect();
    for burst in all {
        world.despawn(burst);
    }
}

#[test]
fn impacts_require_small_ball_and_closing_speed_strictly_above_threshold() {
    let (mut harness, _) = run();
    for (velocity, small, expected) in [
        (Vec2::Y * (IMPACT_SPEED + 1.0), true, 1),
        (Vec2::Y * IMPACT_SPEED, true, 0),
        (Vec2::Y * (IMPACT_SPEED - 1.0), true, 0),
        (Vec2::new(2000.0, 20.0), true, 0),
        (-Vec2::Y * 1000.0, true, 0),
        (Vec2::Y * 1000.0, false, 0),
    ] {
        clear_bursts(&mut harness);
        let e = ball(&mut harness, Vec2::new(800.0, 400.0), small);
        kick(&mut harness, e, velocity);
        remember_velocities(harness.world_mut());
        // The solver stopped the ball: the burst reads the speed from before.
        kick(&mut harness, e, Vec2::ZERO);
        contact(&mut harness, e, None, -Vec2::Y);
        ball_impacts(harness.world_mut());
        assert_eq!(
            bursts(&harness, IMPACT),
            expected,
            "{velocity} small={small}"
        );
        ball_impacts(harness.world_mut());
        assert_eq!(bursts(&harness, IMPACT), expected, "the ball's cooldown");
        harness.world_mut().despawn(e);
    }
}

#[test]
fn relative_body_impacts_work_on_b_side_and_dense_contacts_remain_bounded() {
    let (mut harness, _) = run();
    let a = ball(&mut harness, Vec2::new(800.0, 400.0), false);
    let b = ball(&mut harness, Vec2::new(830.0, 400.0), true);
    kick(&mut harness, a, Vec2::X * 300.0);
    kick(&mut harness, b, -Vec2::X * 300.0);
    remember_velocities(harness.world_mut());
    contact(&mut harness, a, Some(b), -Vec2::X);
    ball_impacts(harness.world_mut());
    assert_eq!(bursts(&harness, IMPACT), 1);
    clear_bursts(&mut harness);
    let mut pile = vec![a, b];
    for i in 0..200 {
        let e = ball(
            &mut harness,
            Vec2::new(
                200.0 + (i % 40) as f32 * 20.0,
                300.0 + (i / 40) as f32 * 20.0,
            ),
            true,
        );
        pile.push(e);
        kick(&mut harness, e, Vec2::Y * 1000.0);
        // Repeated substep contacts take no extra burst.
        for _ in 0..8 {
            contact(&mut harness, e, None, -Vec2::Y);
        }
    }
    remember_velocities(harness.world_mut());
    ball_impacts(harness.world_mut());
    assert_eq!(bursts(&harness, IMPACT), IMPACT_BURSTS_PER_STEP);
    for _ in 0..20 {
        ball_impacts(harness.world_mut());
    }
    assert_eq!(bursts(&harness, IMPACT), BURST_CAP);
    // The balls go, so no landing of theirs bursts again, and the bursts drain.
    for e in pile {
        harness.world_mut().despawn(e);
    }
    harness.step(60);
    assert_eq!(harness.world().query::<&Burst>().count(), 0);
}

#[test]
fn impact_particles_span_rainbow_and_vortex_births_spiral_inward() {
    let (mut harness, _) = run();
    let at = Vec2::new(800.0, 400.0);
    let burst = spawn_burst(harness.world_mut(), IMPACT, at).unwrap();
    // Emitted at the frame's end, coloured as newborns the next.
    harness.step(1);
    impact_sparks(harness.world_mut());
    let colors: Vec<[f32; 4]> = harness
        .world()
        .query::<&Particle>()
        .filter(|p| p.emitter == Some(burst))
        .map(|p| p.base_rgba)
        .collect();
    assert!(!colors.is_empty());
    for channel in 0..3 {
        assert!(colors.iter().any(|c| c[channel] == 1.0), "{channel} full");
        assert!(colors.iter().any(|c| c[channel] < 0.2), "{channel} empty");
    }
    let centre = Vec2::new(1400.0, 600.0);
    let hole = toys::place_black_hole(harness.world_mut(), centre);
    harness.step(2);
    let world = harness.world();
    let mut checked = 0;
    for (particle, transform) in world.query::<(&Particle, &Transform)>() {
        // Placed on the ring as newborns, then a frame's flight: this frame's
        // newborns wait for the next.
        if particle.emitter != Some(hole) || particle.age == 0.0 || particle.age > 0.05 {
            continue;
        }
        let delta = transform.position - centre;
        assert!(
            (100.0..200.0).contains(&delta.length()),
            "{}",
            delta.length()
        );
        assert!(delta.dot(particle.velocity) < 0.0, "falls inward");
        assert!(delta.perp_dot(particle.velocity) > 0.0, "orbits");
        checked += 1;
    }
    assert!(checked > 0);
    // The discrete step keeps shrinking the orbit near the core.
    let tracked = world
        .query::<(Entity, &Particle)>()
        .find(|(_, p)| p.emitter == Some(hole))
        .unwrap()
        .0;
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
        harness.harness_mut().set_dt(dt);
        harness.step(1);
        let world = harness.world_mut();
        assert!(world.has::<BlackHole>(hole));
        let particle = world.get_mut::<Particle>(tracked).unwrap();
        particle.age = 0.1;
        particle.lifetime = 2.0;
        world.get_mut::<Transform>(tracked).unwrap().position = centre + Vec2::new(30.0, 0.0);
        hole_particles(world);
        let velocity = world.get::<Particle>(tracked).unwrap().velocity;
        let next = Vec2::new(30.0, 0.0) + velocity * dt;
        assert!((next.length() - (30.0 - 105.0 * dt)).abs() < 0.001, "{dt}");
    }
}

/// The ball pit's interior, `tools/level.json`'s `pit`: its columns and
/// rows, from the rim's row down to the floor's.
#[derive(serde::Deserialize)]
struct Pit {
    left: f32,
    right: f32,
    rim: f32,
    floor: f32,
}

/// A platform of `tools/level.json`'s, as far as these tests read it.
#[derive(serde::Deserialize)]
struct Platform {
    name: String,
    left: f32,
    right: f32,
    row: f32,
}

#[derive(serde::Deserialize)]
struct Source {
    pit: Pit,
    platforms: Vec<Platform>,
}

fn source() -> Source {
    let text = std::fs::read_to_string("tools/level.json").expect("tools/level.json reads");
    serde_json::from_str(&text).expect("tools/level.json parses")
}

#[test]
fn pit_contains_two_thousand_mixed_balls_and_fast_wall_impacts() {
    let level = source();
    let pit = level.pit;
    // The climbing shelves in the pit, each a tile deep with a margin round
    // it: no ball starts inside one.
    let shelves: Vec<(Vec2, Vec2)> = level
        .platforms
        .iter()
        .filter(|p| p.name.starts_with("pit_shelf"))
        .map(|p| {
            (
                Vec2::new(p.left * TILE - 20.0, p.row * TILE - 20.0),
                Vec2::new(p.right * TILE + 20.0, p.row * TILE + 44.0),
            )
        })
        .collect();
    let across = (((pit.right - pit.left) * TILE - 96.0) / 31.0) as usize;
    let (mut harness, _) = run();
    let mut placed = Vec::new();
    let mut slot = 0;
    while placed.len() < 2048 {
        let at = Vec2::new(
            pit.left * TILE + 48.0 + (slot % across) as f32 * 31.0,
            pit.floor * TILE - 16.0 - (slot / across) as f32 * 31.0,
        );
        slot += 1;
        if shelves
            .iter()
            .any(|(min, max)| at.cmpge(*min).all() && at.cmple(*max).all())
        {
            continue;
        }
        let i = placed.len();
        let small = i % 2 == 0;
        let e = ball(&mut harness, at, small);
        if i < 2 {
            kick(
                &mut harness,
                e,
                Vec2::X * if small { -1800.0 } else { 1800.0 },
            );
        }
        placed.push(e);
    }
    for _ in 0..180 {
        harness.step(1);
        for &e in &placed {
            let p = body(&harness, e);
            // Containment, allowing the solver's shallow contact penetration:
            // the narrow pit's pile stands twice as deep, and its foot presses
            // a ball up to its radius into the floor (`D-094`); one through
            // the floor would fall on past half a tile.
            assert!(
                p.x >= pit.left * TILE && p.x <= pit.right * TILE,
                "wall escape {p}"
            );
            assert!(
                p.y >= pit.rim * TILE && p.y <= (pit.floor + 0.5) * TILE,
                "vertical escape {p}"
            );
        }
    }
    assert_eq!(harness.world().query::<&Ball>().count(), 2048);
}

#[test]
fn middle_mouse_has_five_times_the_rate_half_size_and_dedicated_animation() {
    // `cast` held on slot 9 for the middle mouse button. The orbs got no
    // slot, and a frame runs at most two steps, so a long frame owes two
    // steps' balls.
    for (dt, frames, expected) in [
        (0.001, 322, 49),
        (1.0 / 60.0, 20, 52),
        (1.0 / 30.0, 10, 52),
        (0.161, 1, 5),
    ] {
        let mut harness = pitcher();
        harness.harness_mut().set_dt(dt);
        harness.harness_mut().press_action("cast");
        harness.step(frames);
        let small = small_balls(&harness);
        assert_eq!(small.len(), expected, "dt={dt}");
        let world = harness.world();
        for ball in &small {
            assert_eq!(
                world.get::<Collider>(*ball).unwrap().shape,
                Shape::Circle {
                    radius: SMALL_BALL_RADIUS
                }
            );
            let clip = &world.get::<AnimationState>(*ball).unwrap().animation_id;
            assert_eq!(clip, "ex10_ball_small_spin");
            let scale = world.get::<Transform>(*ball).unwrap().scale;
            assert_eq!(scale, Vec2::splat(SMALL_BALL_SCALE));
        }
        // The release's step forgets what was owed.
        harness.harness_mut().release_action("cast");
        harness.harness_mut().set_dt(1.0 / 60.0);
        harness.step(1);
        assert_eq!(harness.world().resource::<BallSpawner>().owed, 0.0);
        assert_eq!(small_balls(&harness).len(), expected);
    }
}

#[test]
fn spawning_stops_at_the_ball_cap() {
    let mut harness = pitcher();
    let live = toys::ball_count(harness.world());
    for _ in 0..BALL_CAP - live - 1 {
        harness.world_mut().spawn_with((Ball,));
    }
    // One frame long enough for five more.
    harness.harness_mut().set_dt(0.161);
    harness.harness_mut().press_action("cast");
    // The one ball left in the budget goes out, then nothing.
    for _ in 0..2 {
        harness.step(1);
        assert_eq!(harness.world().query::<&Ball>().count(), BALL_CAP);
    }
}
