//! The counter guards. The full pit's (redo 7.3): a New Game, 2,000 small
//! balls poured and settled, the six enemies woken about the player, the
//! pit's counts asserted, then a 60-frame window, its counts pinned in
//! `guards/pit.json`. The tour's: the `e01-play` benchmark row's own script
//! from the registry, run from the title with its window's 1,200 frames
//! counted and pinned in `guards/play.json`.

use example_01_platformer::features::run::pit::{self, PitCounts};
use serde_json::{Value, json};
use tungsten::core::script::Script;
use tungsten::testing::CounterGuard;

use super::{harness, run};

const BALLS: usize = 2_000;
const SETTLE: u32 = 120;
const WINDOW: u32 = 60;

#[test]
fn the_full_pit_holds_its_counts() {
    let (mut harness, _) = run();
    assert_eq!(pit::fill(harness.world_mut(), BALLS), BALLS);
    harness.step(SETTLE);
    pit::wake_foes(harness.world_mut()).expect("the six");
    harness.step(1);
    let PitCounts {
        balls,
        foes_alive,
        foes_awake,
    } = pit::counts(harness.world());
    assert_eq!(balls, BALLS, "the pit");
    assert_eq!(foes_alive, 6, "the six live");
    assert!(foes_awake >= 5, "{foes_awake} awake");

    let reading = CounterGuard::measure(harness.harness_mut(), 0, WINDOW);
    reading.check(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/game/guards/pit.json"
    ));
}

/// The `e01-play` row's script as the benchmark runner composes it
/// (`scripts/perf.py`'s `compose_script`): the registry's steps, an
/// `until: "end"` held through the last frame, and its asserts on that
/// frame. With it, the row's warm-up.
fn tour() -> (Script, u32) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/perf-rows.json");
    let text = std::fs::read_to_string(path).expect("the benchmark registry reads");
    let registry: Value = serde_json::from_str(&text).expect("the benchmark registry parses");
    let row = registry["rows"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["name"] == "e01-play"))
        .expect("the registry holds e01-play");
    let warmup = row["warmup"].as_u64().expect("a warm-up");
    let last = warmup + row["frames"].as_u64().expect("a window");
    let mut steps = row["launch"]["script"]["steps"]
        .as_array()
        .expect("the row's steps")
        .clone();
    for step in &mut steps {
        if step["until"] == "end" {
            step["until"] = json!(last);
        }
    }
    let asserts = row["launch"]["script"]["asserts"].as_array();
    for assert in asserts.into_iter().flatten() {
        steps.push(json!({ "frame": last, "assert": assert }));
    }
    let script = json!({
        "version": 1,
        "frames": last,
        "resolution": row["resolution"],
        "steps": steps,
    });
    let script = Script::parse(&script.to_string()).expect("the row's script parses");
    (script, warmup as u32)
}

#[test]
fn the_tour_holds_its_counts() {
    let (script, warmup) = tour();
    let mut harness = harness();
    let (reading, report) =
        CounterGuard::measure_script(harness.harness_mut(), &script, warmup + 1);
    // The row's own asserts: the last area's name, the wizard alive, the
    // run on top.
    assert!(
        report.passed,
        "the tour: {:?} {:?} {:?}",
        report.asserts, report.failed_steps, report.unreached
    );
    reading.check(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/game/guards/play.json"
    ));
}
