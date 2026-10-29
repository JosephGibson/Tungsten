//! The full pit's counter guard (redo 7.3): a New Game, 2,000 small balls
//! poured and settled, the six enemies woken about the player, the pit's
//! counts asserted, then a 60-frame window, its counts pinned in
//! `guards/pit.json`.

use example_01_platformer::features::run::pit::{self, PitCounts};
use tungsten::testing::CounterGuard;

use super::run;

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
