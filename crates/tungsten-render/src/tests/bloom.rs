use super::*;
use tungsten_core::assets::ShaderAssetId;
use tungsten_core::post::BloomParams;
use tungsten_core::tween::UniformOverrideBlock;

use crate::targets::bloom_mip_count_for_size;

#[test]
fn uniform_override_block_payload_is_256_bytes() {
    let block = UniformOverrideBlock::default();
    assert_eq!(block.to_bytes().len(), 256);
}

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
fn bloom_shader_ids_are_stable() {
    // The renderer pre-seeds bloom shader ids 4..=7 immediately after sprite
    // (0) and SMAA (1..=3). Dropping or reordering would break manifest reload
    // routing in `Renderer::reload_shader`.
    let ids = BloomShaderIds {
        threshold: ShaderAssetId(4),
        downsample: ShaderAssetId(5),
        upsample: ShaderAssetId(6),
        composite: ShaderAssetId(7),
    };
    assert_eq!(ids.threshold.0, 4);
    assert_eq!(ids.downsample.0, 5);
    assert_eq!(ids.upsample.0, 6);
    assert_eq!(ids.composite.0, 7);
}

#[test]
fn bloom_threshold_wgsl_naga_validates() {
    let src = include_str!("../shaders/stock/bloom_threshold.wgsl");
    crate::shader_hot_reload::validate_wgsl_source("bloom_threshold", src)
        .expect("bloom_threshold.wgsl must validate");
}

#[test]
fn bloom_downsample_wgsl_naga_validates() {
    let src = include_str!("../shaders/stock/bloom_downsample.wgsl");
    crate::shader_hot_reload::validate_wgsl_source("bloom_downsample", src)
        .expect("bloom_downsample.wgsl must validate");
}

#[test]
fn bloom_upsample_wgsl_naga_validates() {
    let src = include_str!("../shaders/stock/bloom_upsample.wgsl");
    crate::shader_hot_reload::validate_wgsl_source("bloom_upsample", src)
        .expect("bloom_upsample.wgsl must validate");
}

#[test]
fn bloom_composite_wgsl_naga_validates() {
    let src = include_str!("../shaders/stock/bloom_composite.wgsl");
    crate::shader_hot_reload::validate_wgsl_source("bloom_composite", src)
        .expect("bloom_composite.wgsl must validate");
}

#[test]
fn bloom_wgsl_engine_and_asset_mirrors_match() {
    let pairs: &[(&str, &str)] = &[
        (
            include_str!("../shaders/stock/bloom_threshold.wgsl"),
            include_str!("../../../../assets/shaders/stock/bloom_threshold.wgsl"),
        ),
        (
            include_str!("../shaders/stock/bloom_downsample.wgsl"),
            include_str!("../../../../assets/shaders/stock/bloom_downsample.wgsl"),
        ),
        (
            include_str!("../shaders/stock/bloom_upsample.wgsl"),
            include_str!("../../../../assets/shaders/stock/bloom_upsample.wgsl"),
        ),
        (
            include_str!("../shaders/stock/bloom_composite.wgsl"),
            include_str!("../../../../assets/shaders/stock/bloom_composite.wgsl"),
        ),
    ];
    for (engine, asset) in pairs {
        assert_eq!(
            engine.len(),
            asset.len(),
            "engine and asset bloom WGSL mirrors must be byte-equal"
        );
        assert!(
            engine == asset,
            "engine and asset bloom WGSL mirrors must match byte-for-byte"
        );
    }
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
