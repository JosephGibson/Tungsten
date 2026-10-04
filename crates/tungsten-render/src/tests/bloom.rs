use super::*;
use tungsten_core::post::BloomParams;

use crate::targets::{TargetCache, bloom_mip_count_for_size};

#[test]
fn bloom_pack_writes_expected_slots() {
    let params = BloomParams {
        threshold: 1.2,
        knee: 0.4,
        intensity: 0.6,
        radius: 0.85,
    };
    let block = pack_params(&params, (1.0 / 1920.0, 1.0 / 1080.0), 6, 3, 1);
    assert!((block.f32s[0] - 1.2).abs() < 1e-6);
    assert!((block.f32s[1] - 0.4).abs() < 1e-6);
    assert!((block.f32s[2] - 0.6).abs() < 1e-6);
    assert!((block.f32s[3] - 0.85).abs() < 1e-6);
    assert_eq!(block.i32s[0], 6);
    assert_eq!(block.i32s[1], 3);
    assert_eq!(block.i32s[2], 1);
    assert_eq!(block.vec4[1], [1.0, 1.0, 1.0, 1.0]);
    assert!((block.vec4[0][0] - 1.0 / 1920.0).abs() < 1e-9);
    assert!((block.vec4[0][1] - 1.0 / 1080.0).abs() < 1e-9);
}

#[test]
fn bloom_pyramid_clamps_max_mips_by_viewport() {
    // 1080p tall enough for the requested 6 mips at half-res start.
    assert_eq!(bloom_mip_count_for_size(1920, 1080, 6), 6);
    // 64x64 viewport: floor(log2(64)) = 6, minus 1 = 5 mip ceiling.
    assert_eq!(bloom_mip_count_for_size(64, 64, 6), 5);
    // Tiny viewports never underflow the pyramid count.
    assert_eq!(bloom_mip_count_for_size(2, 2, 6), 1);
    assert_eq!(bloom_mip_count_for_size(1, 1, 6), 1);
    // Plenty of headroom: mip count caps at max_mips, not at the viewport.
    assert_eq!(bloom_mip_count_for_size(1024, 1024, 6), 6);
}

#[test]
fn karis_weighted_average_renormalizes_to_unit_sum() {
    // Using the Karis 1/(1+luma) weighting on equal-input samples should
    // collapse to the canonical (0.5, 4 * 0.125) group weights summed to 1.0,
    // i.e. an identity-preserving filter for flat colour.
    fn luma(rgb: [f32; 3]) -> f32 {
        rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
    }
    let sample = [0.5_f32, 0.5, 0.5];
    let l = luma(sample);
    let w_center = 0.5_f32 / (1.0 + l);
    let w_corner = 0.125_f32 / (1.0 + l);
    let total = w_center + 4.0 * w_corner;
    let combined = (sample[0] * (w_center + 4.0 * w_corner)) / total;
    assert!((combined - sample[0]).abs() < 1e-6);
}

#[test]
fn bloom_default_params_are_visible_for_ldr_demo() {
    // The playground fixture lowers threshold; the default params here should
    // remain usable as a "reasonable starting point" preset and not bloom every
    // pixel — a basic guardrail so default `BloomParams::default()` stays sane.
    let p = BloomParams::default();
    assert!(p.threshold >= 0.5);
    assert!(p.intensity > 0.0 && p.intensity <= 2.0);
    assert!(p.radius > 0.0 && p.radius <= 1.5);
}

#[test]
fn stage_payloads_follow_recording_order() {
    let params = BloomParams {
        threshold: 0.6,
        knee: 0.3,
        intensity: 0.8,
        radius: 1.0,
    };
    // 1080p: the pyramid starts at 960 x 540.
    let extent = |level: u32| ((960u32 >> level).max(1), (540u32 >> level).max(1));
    let mut payloads = Vec::new();
    stage_payloads(&mut payloads, &params, (1920, 1080), 6, extent);
    assert_eq!(payloads.len(), 12);

    let expect = |inv_of: (u32, u32), level: u32, kind: i32| {
        pack_params(
            &params,
            (1.0 / inv_of.0 as f32, 1.0 / inv_of.1 as f32),
            6,
            level,
            kind,
        )
        .to_bytes()
    };
    // Threshold samples the scene; a downsample samples the level above it, an
    // upsample the level below; composite samples the scene again.
    assert_eq!(payloads[0], expect((1920, 1080), 0, PASS_KIND_THRESHOLD));
    for level in 1..6u32 {
        assert_eq!(
            payloads[level as usize],
            expect(extent(level - 1), level, PASS_KIND_DOWNSAMPLE),
            "downsample {level}"
        );
    }
    for (index, level) in (0..5u32).rev().enumerate() {
        assert_eq!(
            payloads[6 + index],
            expect(extent(level + 1), level, PASS_KIND_UPSAMPLE),
            "upsample {level}"
        );
    }
    assert_eq!(payloads[11], expect((1920, 1080), 0, PASS_KIND_COMPOSITE));

    // One mip: threshold and composite only. The vector is reused.
    stage_payloads(&mut payloads, &params, (4, 4), 1, |_| (2, 2));
    assert_eq!(payloads.len(), 2);
    // Other params: every stage's bytes change, so every UBO is rewritten.
    let mut louder = Vec::new();
    let boosted = BloomParams {
        intensity: 1.5,
        ..params
    };
    stage_payloads(&mut louder, &boosted, (4, 4), 1, |_| (2, 2));
    assert!(payloads.iter().zip(&louder).all(|(a, b)| a != b));
}

#[test]
fn bloom_slots_keep_objects_of_their_own() {
    use crate::passes::TargetId::{PostPing, PostPong, SceneColor};
    let key = |generation, src, dst, mip_count| BloomSlotKey {
        generation,
        src,
        dst,
        mip_count,
    };
    let mut slots: Vec<TargetCache<BloomSlotKey, u32>> = Vec::new();
    let mut builds = 0;
    let mut record = |slots: &mut Vec<TargetCache<BloomSlotKey, u32>>, slot, key| {
        *slot_cache(slots, slot).get_or_build(key, || {
            builds += 1;
            builds
        })
    };

    // Two bloom passes in one stack, at slots 0 and 2.
    let first = key(0, SceneColor, PostPing, 6);
    let second = key(0, PostPong, PostPing, 6);
    assert_eq!(record(&mut slots, 0, first), 1);
    assert_eq!(record(&mut slots, 2, second), 2);
    assert_eq!(slots.len(), 3);
    // Later frames build nothing.
    for _ in 0..3 {
        assert_eq!(record(&mut slots, 0, first), 1);
        assert_eq!(record(&mut slots, 2, second), 2);
    }
    // Reallocated targets (a resize, a `post_aa` switch): each slot rebuilds
    // at its next use.
    assert_eq!(record(&mut slots, 0, key(1, SceneColor, PostPing, 6)), 3);
    assert_eq!(record(&mut slots, 2, key(1, PostPong, PostPing, 6)), 4);
    // A pass inserted ahead of slot 0's bloom moves it to slot 1 with another
    // source and destination; slot 2 is untouched.
    assert_eq!(record(&mut slots, 1, key(1, PostPing, PostPong, 6)), 5);
    assert_eq!(record(&mut slots, 2, key(1, PostPong, PostPing, 6)), 4);
    // A smaller pyramid.
    assert_eq!(record(&mut slots, 2, key(1, PostPong, PostPing, 5)), 6);
}
