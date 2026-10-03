use super::*;
use crate::post::smaa_luts;
use tungsten_core::config::PostAaMode;

#[test]
fn preset_mode_off_returns_none() {
    assert!(SmaaPreset::from_mode(PostAaMode::Off).is_none());
}

#[test]
fn preset_low_matches_canonical_values() {
    let p = SmaaPreset::from_mode(PostAaMode::SmaaLow).unwrap();
    assert!((p.threshold - 0.15).abs() < 1e-6);
    assert_eq!(p.max_search_steps, 4);
    assert_eq!(p.max_search_steps_diag, 0);
    assert_eq!(p.corner_rounding, u32::MAX);
}

#[test]
fn preset_medium_matches_canonical_values() {
    let p = SmaaPreset::from_mode(PostAaMode::SmaaMedium).unwrap();
    assert!((p.threshold - 0.10).abs() < 1e-6);
    assert_eq!(p.max_search_steps, 8);
    assert_eq!(p.max_search_steps_diag, 0);
    assert_eq!(p.corner_rounding, u32::MAX);
}

#[test]
fn preset_high_matches_canonical_values() {
    let p = SmaaPreset::from_mode(PostAaMode::SmaaHigh).unwrap();
    assert!((p.threshold - 0.10).abs() < 1e-6);
    assert_eq!(p.max_search_steps, 16);
    assert_eq!(p.max_search_steps_diag, 8);
    assert_eq!(p.corner_rounding, 25);
}

#[test]
fn preset_ultra_matches_canonical_values() {
    let p = SmaaPreset::from_mode(PostAaMode::SmaaUltra).unwrap();
    assert!((p.threshold - 0.05).abs() < 1e-6);
    assert_eq!(p.max_search_steps, 32);
    assert_eq!(p.max_search_steps_diag, 16);
    assert_eq!(p.corner_rounding, 25);
}

#[test]
fn preset_ubo_size_is_256() {
    assert_eq!(std::mem::size_of::<SmaaPresetUbo>(), 256);
}

#[test]
fn preset_ubo_packs_rt_metrics_from_size() {
    let preset = SmaaPreset::from_mode(PostAaMode::SmaaHigh).unwrap();
    let ubo = SmaaPresetUbo::from_preset(preset, (1920, 1080));
    assert!((ubo.rt_metrics[0] - 1.0 / 1920.0).abs() < 1e-6);
    assert!((ubo.rt_metrics[1] - 1.0 / 1080.0).abs() < 1e-6);
    assert_eq!(ubo.rt_metrics[2], 1920.0);
    assert_eq!(ubo.rt_metrics[3], 1080.0);
    assert!((ubo.threshold - 0.10).abs() < 1e-6);
    assert_eq!(ubo.max_search_steps as u32, 16);
}

#[test]
fn area_lut_byte_length() {
    assert_eq!(smaa_luts::area_bytes().len(), 160 * 560 * 2);
}

#[test]
fn search_lut_byte_length() {
    assert_eq!(smaa_luts::search_bytes().len(), 64 * 16);
}
