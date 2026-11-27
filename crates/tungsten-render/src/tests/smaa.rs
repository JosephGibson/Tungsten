use super::*;
use tungsten_core::config::PostAaMode;

#[test]
fn preset_mode_off_returns_none() {
    assert!(SmaaPreset::from_mode(PostAaMode::Off).is_none());
}

#[test]
fn each_preset_matches_its_canonical_values() {
    for (mode, threshold, steps, diag, corner) in [
        (PostAaMode::SmaaLow, 0.15, 4, 0, u32::MAX),
        (PostAaMode::SmaaMedium, 0.10, 8, 0, u32::MAX),
        (PostAaMode::SmaaHigh, 0.10, 16, 8, 25),
        (PostAaMode::SmaaUltra, 0.05, 32, 16, 25),
    ] {
        let p = SmaaPreset::from_mode(mode).unwrap();
        assert!(
            (p.threshold - threshold).abs() < 1e-6,
            "{mode:?}: threshold {}",
            p.threshold
        );
        assert_eq!(p.max_search_steps, steps, "{mode:?}: max_search_steps");
        assert_eq!(
            p.max_search_steps_diag, diag,
            "{mode:?}: max_search_steps_diag"
        );
        assert_eq!(p.corner_rounding, corner, "{mode:?}: corner_rounding");
    }
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
