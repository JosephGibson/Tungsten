//! C1's frame-time reading of the acceptance game (M76): a New Game at
//! Normal, the ball pit full under the rope bridge and six enemies awake
//! about the player on the bridge's east deck, its frames timed on the harness, which
//! runs the real frame body with dt pinned and copies the extract where
//! render would draw. Ignored: a release build runs it in one sitting
//! (`perf-runs/<UTC>-e01d-frame/README.md`). The scene is the game's own
//! full pit ([`pit`]), which the `e01-pit` benchmark row scripts.

use example_01_platformer::features::level::{PIT, PIT_DECK, PIT_SHELVES, TileSpan};
use example_01_platformer::features::run::pit::{self, PitCounts};
use example_01_platformer::features::toys::BALL_CAP;
use example_01_platformer::game::GAMEPLAY;
use serde::Deserialize;
use tungsten::FrameTimings;
use tungsten::kit::Health;

use super::{player_entity, run, top};

const DT: f32 = 1.0 / 60.0;
/// Frames the poured balls settle over, the warm-up before the windows,
/// and each window's frames and their count.
const SETTLE: u32 = 600;
const WARM_UP: u32 = 180;
const WINDOW: usize = 300;
const WINDOWS: usize = 5;
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

impl Platform {
    fn span(&self) -> TileSpan {
        TileSpan {
            left: self.left,
            right: self.right,
            row: self.row,
        }
    }
}

#[test]
fn the_pit_constants_restate_level_json() {
    let text = std::fs::read_to_string("tools/level.json").expect("tools/level.json reads");
    let level: Source = serde_json::from_str(&text).expect("tools/level.json parses");
    let pit = &level.pit;
    assert_eq!(
        PIT,
        TileSpan {
            left: pit.left,
            right: pit.right,
            row: pit.floor,
        }
    );
    let shelves: Vec<TileSpan> = level
        .platforms
        .iter()
        .filter(|p| p.name.starts_with("pit_shelf"))
        .map(Platform::span)
        .collect();
    assert_eq!(shelves, PIT_SHELVES);
    let deck = level.platforms.iter().find(|p| p.name == DECK);
    assert_eq!(deck.map(Platform::span), Some(PIT_DECK));
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
    let (mut harness, player) = run();
    harness.harness_mut().set_dt(DT);
    // The pit full, settled; then the player on the deck's middle, six
    // enemies awake within 600 px, and the seven durable from the next step.
    pit::fill(harness.world_mut(), BALL_CAP);
    harness.step(SETTLE);
    let six = pit::wake_foes(harness.world_mut()).expect("the six");
    harness.step(1);
    let middle = example_01_platformer::features::player::centre(harness.world(), player);
    println!(
        "frame_time scenario: balls {}, enemies {} awake within 600 px, player at {middle:?} on {DECK}, dt 1/60",
        pit::counts(harness.world()).balls,
        six.len(),
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
        let PitCounts {
            balls,
            foes_alive: alive,
            foes_awake: awake,
        } = pit::counts(world);
        assert_eq!(balls, BALL_CAP, "window {window}: the pit");
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
