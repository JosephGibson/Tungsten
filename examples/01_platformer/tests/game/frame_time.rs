//! C1's frame-time reading of the acceptance game (M76): a New Game at
//! Normal, the ball pit full under the rope bridge and six enemies awake
//! about the player on the bridge's east deck, its frames timed on the harness, which
//! runs the real frame body with dt pinned and copies the extract where
//! render would draw. Ignored: a release build runs it in one sitting
//! (`perf-runs/<UTC>-e01d-frame/README.md`).

use example_01_platformer::features::enemies;
use example_01_platformer::features::enemies::components::{EnemyKind, EnemyState};
use example_01_platformer::features::level::TILE;
use example_01_platformer::features::toys::{self, BALL_CAP};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shared::SaveIds;
use example_01_platformer::shell::Difficulty;
use glam::Vec2;
use serde::Deserialize;
use tungsten::FrameTimings;
use tungsten::core::Entity;
use tungsten::kit::{Health, StateMachine, Stats};
use tungsten::testing::UiHarness;

use super::{kind_body, player_half};
use super::{player_entity, run, teleport, top};

const DT: f32 = 1.0 / 60.0;
/// Frames the poured balls settle over, the warm-up before the windows,
/// and each window's frames and their count.
const SETTLE: u32 = 600;
const WARM_UP: u32 = 180;
const WINDOW: usize = 300;
const WINDOWS: usize = 5;
/// The participants' durability for the capture alone: every attack lands,
/// the troll's slam on enemies too, but nothing dies and the work holds.
const DURABLE: f32 = 1_000_000.0;
/// The deck the player stands on, its middle: over the pit.
const DECK: &str = "bridge_b";

#[derive(Deserialize)]
struct Source {
    pit: Pit,
    platforms: Vec<Platform>,
}

#[derive(Deserialize)]
struct Pit {
    left: f32,
    right: f32,
    floor: f32,
}

#[derive(Deserialize)]
struct Platform {
    name: String,
    left: f32,
    right: f32,
    row: f32,
}

/// An enemy of `kind`, awake, its body's centre at `centre`, at Normal.
fn enemy(harness: &mut UiHarness, kind: EnemyKind, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let entity =
        enemies::spawn(world, kind, centre - kind_body(kind).centre, id).expect("its prefab");
    enemies::scale(world, entity, Difficulty::Normal);
    entity
}

/// A window's p50, p95, p99 and max of `samples`.
fn quantiles(samples: &mut [f32]) -> [f32; 4] {
    samples.sort_by(f32::total_cmp);
    let at = |q: f32| samples[((samples.len() - 1) as f32 * q).round() as usize];
    [at(0.50), at(0.95), at(0.99), samples[samples.len() - 1]]
}

fn line(name: &str, samples: &mut [f32]) -> String {
    let [p50, p95, p99, max] = quantiles(samples);
    format!("{name} p50 {p50:.3} p95 {p95:.3} p99 {p99:.3} max {max:.3}")
}

#[test]
#[ignore = "C1's capture: a release build runs it in one sitting"]
fn a_full_pit_with_enemies_awake() {
    let text = std::fs::read_to_string("tools/level.json").expect("tools/level.json reads");
    let level: Source = serde_json::from_str(&text).expect("tools/level.json parses");
    let deck = level
        .platforms
        .iter()
        .find(|p| p.name == DECK)
        .expect("the deck");
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(DT);
    // The pit full: the cap's small balls in a grid over its interior, clear
    // of the climbing shelves up its middle, settled.
    let pit = &level.pit;
    let shelves: Vec<&Platform> = level
        .platforms
        .iter()
        .filter(|p| p.name.starts_with("pit_shelf"))
        .collect();
    let world = harness.world_mut();
    let across = (((pit.right - pit.left) * TILE - 16.0) / 16.0) as usize;
    let mut slot = 0;
    while toys::ball_count(world) < BALL_CAP {
        let at = Vec2::new(
            pit.left * TILE + 8.0 + (slot % across) as f32 * 16.0,
            pit.floor * TILE - 8.0 - (slot / across) as f32 * 16.0,
        );
        slot += 1;
        let on_shelf = shelves.iter().any(|p| {
            at.x > p.left * TILE - 8.0
                && at.x < p.right * TILE + 8.0
                && at.y > p.row * TILE - 8.0
                && at.y < p.row * TILE + 32.0
        });
        if !on_shelf {
            toys::spawn_ball(world, at, true);
        }
    }
    harness.step(SETTLE);
    // The player on the deck's middle, six enemies awake within 600 px.
    let middle = Vec2::new(
        f32::midpoint(deck.left, deck.right) * TILE,
        deck.row * TILE - player_half().y - 0.5,
    );
    teleport(&mut harness, player, middle);
    harness.step(1);
    let surface = deck.row * TILE;
    let walk =
        |kind: EnemyKind, dx: f32| Vec2::new(middle.x + dx, surface - kind_body(kind).half.y - 0.5);
    // The walkers on the deck and the east rim beyond the chute, never over
    // the gap or a chute.
    let cast = [
        (EnemyKind::Wisp, middle + Vec2::new(-220.0, -200.0)),
        (EnemyKind::Crawler, walk(EnemyKind::Crawler, -150.0)),
        (EnemyKind::Beetle, walk(EnemyKind::Beetle, 544.0)),
        (EnemyKind::Bat, middle + Vec2::new(120.0, -260.0)),
        (EnemyKind::Troll, walk(EnemyKind::Troll, 700.0)),
        (EnemyKind::Wisp, middle + Vec2::new(260.0, -180.0)),
    ];
    let six: Vec<Entity> = cast
        .into_iter()
        .map(|(kind, centre)| enemy(&mut harness, kind, centre))
        .collect();
    let world = harness.world_mut();
    for &entity in six.iter().chain([&player]) {
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            stats.set_base(Stats::MAX_HEALTH, DURABLE);
        }
    }
    // The stat reaches `Health` at the next step; then fill it.
    harness.step(1);
    let world = harness.world_mut();
    for &entity in six.iter().chain([&player]) {
        if let Some(health) = world.get_mut::<Health>(entity) {
            let max = health.max();
            health.heal(max);
        }
    }
    println!(
        "frame_time scenario: balls {}, enemies {} awake within 600 px, player at {:?} on {DECK}, dt 1/60",
        toys::ball_count(harness.world()),
        six.len(),
        middle
    );
    harness.step(WARM_UP);
    let mut medians = Vec::new();
    for window in 0..WINDOWS {
        let (mut total, mut update, mut extract, mut physics) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for _ in 0..WINDOW {
            harness.step(1);
            let timings = harness.world().resource::<FrameTimings>();
            total.push(timings.total_ms);
            update.push(timings.update_ms);
            extract.push(timings.extract_ms);
            physics.push(
                timings
                    .system_timings
                    .iter()
                    .find(|(name, _)| name == "physics_step")
                    .map_or(0.0, |(_, ms)| *ms),
            );
        }
        let world = harness.world();
        assert_eq!(
            toys::ball_count(world),
            BALL_CAP,
            "window {window}: the pit"
        );
        let alive = six
            .iter()
            .filter(|&&e| world.get::<Health>(e).is_some_and(|h| !h.is_dead()))
            .count();
        let awake = six
            .iter()
            .filter(|&&e| {
                world
                    .get::<StateMachine<EnemyState>>(e)
                    .is_some_and(|machine| !machine.is_held())
            })
            .count();
        assert_eq!(alive, 6, "window {window}: the six live");
        assert!(awake >= 5, "window {window}: {awake} awake");
        assert!(
            world
                .get::<Health>(player_entity(&harness))
                .is_some_and(|h| !h.is_dead()),
            "window {window}: the player lives"
        );
        assert_eq!(top(&harness), Some(GAMEPLAY));
        medians.push(quantiles(&mut total.clone())[0]);
        println!(
            "frame_time window {window}: {} | {} | {} | {} | awake {awake}",
            line("total", &mut total),
            line("update", &mut update),
            line("extract", &mut extract),
            line("physics_step", &mut physics)
        );
    }
    medians.sort_by(f32::total_cmp);
    println!(
        "frame_time median of the windows' total p50: {:.3} ms",
        medians[WINDOWS / 2]
    );
}
