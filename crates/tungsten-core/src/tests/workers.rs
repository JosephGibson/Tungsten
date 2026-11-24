use std::ops::Range;
use std::panic::{AssertUnwindSafe, RefUnwindSafe, UnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{SystemWorkers, forks, map};
use crate::ecs::World;
use crate::rng::Pcg32;

/// The participant counts the tests fork at, as the step's lockstep tests do.
const COUNTS: [usize; 5] = [2, 3, 4, 6, 12];
const CHUNKS: [usize; 6] = [1, 2, 3, 7, 16, 64];

/// A map's shared items, whether each item takes a microsecond, and how many
/// chunks the worker threads ran.
struct Values {
    items: Vec<u64>,
    slow: bool,
    on_workers: AtomicUsize,
}

impl Values {
    fn new(items: Vec<u64>, slow: bool) -> Self {
        Self {
            items,
            slow,
            on_workers: AtomicUsize::new(0),
        }
    }
}

/// Whether this is one of the map's worker threads.
fn on_worker() -> bool {
    std::thread::current()
        .name()
        .is_some_and(|name| name.starts_with("tungsten-map-"))
}

/// Item `i`'s outputs: `i % 3` of them, from the item's value, so items give
/// none, one or two and a merge that loses or moves one shows.
fn spread(values: &Values, range: Range<usize>, out: &mut Vec<u64>) {
    if on_worker() {
        values.on_workers.fetch_add(1, Ordering::Relaxed);
    }
    for i in range {
        if values.slow {
            let start = Instant::now();
            while start.elapsed() < Duration::from_micros(1) {
                std::hint::spin_loop();
            }
        }
        for k in 0..(i % 3) as u64 {
            out.push(values.items[i].wrapping_mul(0x9e37_79b9).wrapping_add(k));
        }
    }
}

/// A second output type through the same pool: the even items with their
/// index.
fn pairs(values: &Values, range: Range<usize>, out: &mut Vec<(u32, u64)>) {
    out.extend(
        range
            .filter(|i| i % 2 == 0)
            .map(|i| (i as u32, values.items[i])),
    );
}

fn items(rng: &mut Pcg32, len: usize) -> Vec<u64> {
    (0..len).map(|_| u64::from(rng.next_u32())).collect()
}

#[test]
fn a_map_equals_one_serial_call_at_every_count_and_chunk() {
    let mut rng = Pcg32::seeded(0x5eed);
    let lens: Vec<usize> = (0..24)
        .map(|_| rng.next_u32() as usize % 1_500)
        .chain([0, 1, 2, 63, 64, 65])
        .collect();
    for participants in COUNTS {
        for chunk in CHUNKS {
            // One pool for every length and both output types: a pool is
            // made once, at its first fork.
            let mut workers = SystemWorkers::forced(participants, chunk);
            for &len in &lens {
                let values = Values::new(items(&mut rng, len), false);
                let (mut want, mut want_pairs) = (vec![7], vec![(7, 7)]);
                spread(&values, 0..len, &mut want);
                pairs(&values, 0..len, &mut want_pairs);
                let (mut got, mut got_pairs) = (vec![7], vec![(7, 7)]);
                let values = workers.map(values, len, 4_096, usize::MAX, &mut got, spread);
                let back = workers.map(values, len, 4_096, usize::MAX, &mut got_pairs, pairs);
                assert_eq!(
                    (got, got_pairs),
                    (want, want_pairs),
                    "{len} items, {participants} participants, chunks of {chunk}"
                );
                assert_eq!(back.items.len(), len, "the shared value comes back whole");
            }
            // Items of a microsecond each: the workers wake within the
            // caller's share and take chunks of their own.
            let values = Values::new(items(&mut rng, 1_000), true);
            let mut want = Vec::new();
            spread(&values, 0..1_000, &mut want);
            let mut got = Vec::new();
            let back = workers.map(values, 1_000, 4_096, usize::MAX, &mut got, spread);
            assert_eq!(
                got, want,
                "slow items, {participants} participants, chunks of {chunk}"
            );
            assert!(
                back.on_workers.load(Ordering::Relaxed) > 0,
                "{participants} participants, chunks of {chunk}: no worker took a chunk"
            );
        }
    }
}

#[test]
fn a_map_without_workers_with_one_participant_or_under_its_minimum_runs_serially() {
    let values = || Values::new((0..5_000).collect(), false);
    let mut want = Vec::new();
    spread(&values(), 0..5_000, &mut want);

    // No resource: one serial call.
    let mut world = World::new();
    assert!(!forks(&world, 5_000, 64, 0));
    let mut got = Vec::new();
    let back = map(&mut world, values(), 5_000, 64, 0, &mut got, spread);
    assert_eq!(got, want);
    assert_eq!(back.on_workers.load(Ordering::Relaxed), 0);

    // One participant, a length under the minimum and a single chunk: no
    // thread starts.
    for (mut workers, chunk, min) in [
        (SystemWorkers::new(1), 64, 0),
        (SystemWorkers::new(4), 64, 5_001),
        (SystemWorkers::new(4), 5_000, 0),
    ] {
        assert!(!workers.forks(5_000, chunk, min), "{workers:?} would fork");
        let mut got = Vec::new();
        workers.map(values(), 5_000, chunk, min, &mut got, spread);
        assert_eq!(got, want);
        assert!(!workers.started(), "{workers:?} started its threads");
    }

    // At its minimum, with chunks to share, it forks and agrees.
    let mut workers = SystemWorkers::new(4);
    assert!(workers.forks(5_000, 64, 5_000));
    let mut got = Vec::new();
    workers.map(values(), 5_000, 64, 5_000, &mut got, spread);
    assert_eq!(got, want);
    assert!(workers.started());

    // Through the World's resource.
    world.insert_resource(SystemWorkers::new(3));
    assert!(forks(&world, 5_000, 64, 0));
    let mut got = Vec::new();
    map(&mut world, values(), 5_000, 64, 0, &mut got, spread);
    assert_eq!(got, want);
    assert!(world.get_resource::<SystemWorkers>().unwrap().started());
}

#[test]
fn new_clamps_its_count_and_forced_its_chunk() {
    assert_eq!(SystemWorkers::new(0).participants(), 1);
    assert_eq!(SystemWorkers::new(64).participants(), SystemWorkers::MAX);
    let mut workers = SystemWorkers::forced(3, 0);
    let values = Values::new((0..10).collect(), false);
    let mut want = Vec::new();
    spread(&values, 0..10, &mut want);
    let mut got = Vec::new();
    workers.map(values, 10, 64, usize::MAX, &mut got, spread);
    assert_eq!(got, want);
    assert!(
        workers.started(),
        "a forced chunk of 0 is 1, so ten items fork"
    );
}

/// A share that fails on a worker thread, while the caller's waits (at most
/// two seconds) for a worker to have claimed a chunk.
struct Trap {
    claimed: AtomicBool,
}

impl Trap {
    fn new() -> Self {
        Self {
            claimed: AtomicBool::new(false),
        }
    }
}

fn trap(shared: &Trap, range: Range<usize>, out: &mut Vec<usize>) {
    if on_worker() {
        shared.claimed.store(true, Ordering::Release);
        panic!("a worker's chunk fails");
    }
    let start = Instant::now();
    while !shared.claimed.load(Ordering::Acquire) && start.elapsed() < Duration::from_secs(2) {
        std::thread::yield_now();
    }
    out.extend(range);
}

fn fails_at_item_40(_: &Trap, range: Range<usize>, out: &mut Vec<usize>) {
    for i in range {
        assert!(i != 40, "item 40 fails");
        out.push(i);
    }
}

fn message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
        })
        .unwrap_or_default()
}

#[test]
fn a_share_that_panics_fails_the_map_and_the_next_map_forks_afresh() {
    for participants in [2, 4] {
        let mut workers = SystemWorkers::forced(participants, 4);
        // A worker's share fails: the map panics with the workers' message
        // instead of hanging or returning short.
        let failed = catch_unwind(AssertUnwindSafe(|| {
            workers.map(Trap::new(), 64, 4, 0, &mut Vec::new(), trap);
        }));
        let payload = failed.expect_err("a worker's panic returned");
        assert_eq!(
            message(payload.as_ref()),
            "a worker's share of a system's map panicked",
            "{participants} participants"
        );
        assert!(!workers.started(), "the failed fork dropped its pool");
        assert!(
            workers.exits().load(Ordering::Relaxed) >= participants - 1,
            "the failed pool's threads ended"
        );
        // Wherever the share fails, the map panics; and the same workers map
        // again on fresh threads.
        let failed = catch_unwind(AssertUnwindSafe(|| {
            workers.map(Trap::new(), 200, 4, 0, &mut Vec::new(), fails_at_item_40);
        }));
        assert!(
            failed.is_err(),
            "{participants} participants: the map returned"
        );
        let values = Values::new((0..500).collect(), false);
        let mut want = Vec::new();
        spread(&values, 0..500, &mut want);
        let mut got = Vec::new();
        workers.map(values, 500, 4, 0, &mut got, spread);
        assert_eq!(got, want, "{participants} participants after a failed map");
        assert!(workers.started());
    }
}

#[test]
fn dropping_the_workers_ends_their_threads() {
    let mut workers = SystemWorkers::forced(6, 8);
    let values = Values::new((0..1_000).collect(), false);
    let mut out = Vec::new();
    workers.map(values, 1_000, 8, 0, &mut out, spread);
    let exits = workers.exits();
    assert_eq!(exits.load(Ordering::Relaxed), 0, "the pool's threads run");
    // Dropping closes every worker's channel and joins each thread, which
    // has ended by then.
    drop(workers);
    assert_eq!(exits.load(Ordering::Relaxed), 5);
}

#[test]
fn the_workers_keep_their_auto_traits() {
    fn auto_traits<T: Send + Sync + UnwindSafe + RefUnwindSafe>() {}
    auto_traits::<SystemWorkers>();
}
