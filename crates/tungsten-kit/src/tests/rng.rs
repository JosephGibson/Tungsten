use super::*;

#[test]
fn one_seed_gives_one_stream_and_a_reseed_starts_it_over() {
    let mut a = KitRng::seeded(42);
    let mut b = KitRng::seeded(42);
    let first: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
    assert_eq!(first, (0..8).map(|_| b.next_u32()).collect::<Vec<_>>());
    assert_ne!(first[0], KitRng::seeded(43).next_u32());
    assert_eq!(a.draws(), 8);
    a.reseed(42);
    assert_eq!((a.seed(), a.draws(), a.next_u32()), (42, 0, first[0]));
}

#[test]
fn the_default_is_the_default_seed() {
    let mut default = KitRng::default();
    let mut seeded = KitRng::seeded(KitRng::DEFAULT_SEED);
    assert_eq!(default.seed(), KitRng::DEFAULT_SEED);
    assert_eq!(default.next_u32(), seeded.next_u32());
}

#[test]
fn draws_stay_in_their_ranges() {
    let mut rng = KitRng::seeded(7);
    for _ in 0..1000 {
        assert!((0.0..1.0).contains(&rng.unit()));
        assert!((-2.0..=3.0).contains(&rng.range(-2.0, 3.0)));
        assert!(rng.index(3) < 3);
    }
    assert_eq!(rng.range(5.0, 5.0), 5.0);
    assert_eq!(rng.draws(), 3001);
}

#[test]
fn a_range_across_the_whole_f32_line_stays_finite() {
    let mut rng = KitRng::seeded(3);
    for _ in 0..1000 {
        let value = rng.range(-f32::MAX, f32::MAX);
        assert!(value.is_finite(), "{value}");
    }
    assert_eq!(KitRng::seeded(5).range(f32::MAX, f32::MAX), f32::MAX);
}

#[test]
fn every_index_comes_up() {
    let mut rng = KitRng::seeded(9);
    let mut seen = [false; 5];
    for _ in 0..200 {
        seen[rng.index(5)] = true;
    }
    assert_eq!(seen, [true; 5]);
}

#[test]
#[should_panic(expected = "no index below 0")]
fn an_index_below_zero_panics() {
    let _ = KitRng::seeded(1).index(0);
}

#[test]
#[should_panic(expected = "not a finite, ordered range")]
fn a_reversed_range_panics() {
    let _ = KitRng::seeded(1).range(2.0, 1.0);
}
