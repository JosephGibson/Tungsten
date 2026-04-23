use super::*;

#[test]
fn splitmix64_reference_vectors() {
    // SplitMix64 byte-pattern guard for inputs 0..=3.
    assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
    assert_eq!(splitmix64(1), 0x910A_2DEC_8902_5CC1);
    assert_eq!(splitmix64(2), 0x9758_35DE_1C97_56CE);
    assert_eq!(splitmix64(3), 0x1D0B_14E4_DB01_8FED);
}

#[test]
fn pcg32_same_seed_same_sequence() {
    let mut a = Pcg32::seeded(42);
    let mut b = Pcg32::seeded(42);
    for _ in 0..16 {
        assert_eq!(a.next_u32(), b.next_u32());
    }
}

#[test]
fn pcg32_different_seeds_diverge_quickly() {
    let mut a = Pcg32::seeded(1);
    let mut b = Pcg32::seeded(2);
    let mut diffs = 0;
    for _ in 0..32 {
        if a.next_u32() != b.next_u32() {
            diffs += 1;
        }
    }
    assert!(
        diffs >= 30,
        "consecutive seeds should produce independent streams (diffs={diffs})"
    );
}

#[test]
fn next_f32_unit_is_bounded() {
    let mut rng = Pcg32::seeded(7);
    for _ in 0..10_000 {
        let v = rng.next_f32_unit();
        assert!((0.0..1.0).contains(&v), "f32_unit out of range: {v}");
    }
}

#[test]
fn next_range_distribution_mean_within_tolerance() {
    let mut rng = Pcg32::seeded(0xdead_beef);
    let (lo, hi) = (-3.0f32, 5.0f32);
    const N: u32 = 10_000;
    let mut sum = 0.0f64;
    for _ in 0..N {
        let v = rng.next_range(lo, hi);
        assert!((lo..hi).contains(&v), "out of range: {v}");
        sum += f64::from(v);
    }
    let mean = sum / f64::from(N);
    let expected = f64::from(f32::midpoint(lo, hi));
    assert!(
        (mean - expected).abs() < 0.25,
        "mean {mean} far from expected {expected}"
    );
}

#[test]
fn next_unit_vec2_length_is_one() {
    let mut rng = Pcg32::seeded(99);
    for _ in 0..256 {
        let v = rng.next_unit_vec2();
        let len = v.length();
        assert!((len - 1.0).abs() < 1.0e-5, "|v| = {len}");
    }
}
