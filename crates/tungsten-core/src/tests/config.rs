use super::*;

#[test]
fn defaults_are_sane() {
    let config = Config::default();
    assert_eq!(config.window.title, "Tungsten");
    assert_eq!(config.window.width, 1280);
    assert_eq!(config.window.height, 720);
    assert!(!config.window.vsync);
    assert!(config.display.resolution.is_none());
    assert!(config.display.display_mode.is_none());
    assert!(config.display.frame_rate_cap.is_none());
    assert!(config.render.max_frame_latency.is_none());
    assert!(config.render.present_mode.is_none());
    assert_eq!(config.render.msaa, 1);
    assert!(config.render.depth_enabled);
    assert_eq!(config.render.depth_sort, DepthSortMode::CpuStable);
    assert_eq!(config.render.post_aa, PostAaMode::Off);
    assert_eq!(config.render.bloom_max_mips, 6);
}

#[test]
fn render_config_defaults_from_empty_json() {
    let parsed: RenderConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(parsed.msaa, 1);
    assert!(parsed.depth_enabled);
    assert_eq!(parsed.depth_sort, DepthSortMode::CpuStable);
    assert_eq!(parsed.post_aa, PostAaMode::Off);
    assert_eq!(parsed.bloom_max_mips, 6);
}

#[test]
fn post_aa_mode_default_is_off() {
    assert_eq!(PostAaMode::default(), PostAaMode::Off);
}

#[test]
fn post_aa_mode_from_str_parses_all_modes() {
    assert_eq!(PostAaMode::from_str("off").unwrap(), PostAaMode::Off);
    assert_eq!(
        PostAaMode::from_str("smaa_low").unwrap(),
        PostAaMode::SmaaLow
    );
    assert_eq!(
        PostAaMode::from_str("smaa_medium").unwrap(),
        PostAaMode::SmaaMedium
    );
    assert_eq!(
        PostAaMode::from_str("smaa_high").unwrap(),
        PostAaMode::SmaaHigh
    );
    assert_eq!(
        PostAaMode::from_str("smaa_ultra").unwrap(),
        PostAaMode::SmaaUltra
    );
}

#[test]
fn post_aa_mode_is_smaa_helper() {
    assert!(!PostAaMode::Off.is_smaa());
    assert!(PostAaMode::SmaaLow.is_smaa());
    assert!(PostAaMode::SmaaMedium.is_smaa());
    assert!(PostAaMode::SmaaHigh.is_smaa());
    assert!(PostAaMode::SmaaUltra.is_smaa());
}

#[test]
fn render_config_parses_post_aa_smaa_high() {
    let json = r#"{ "post_aa": "smaa_high" }"#;
    let parsed: RenderConfig = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.post_aa, PostAaMode::SmaaHigh);
}

#[test]
fn render_config_rejects_unknown_post_aa() {
    let json = r#"{ "post_aa": "junk" }"#;
    let err = serde_json::from_str::<RenderConfig>(json).unwrap_err();
    assert!(err.is_data());
}

#[test]
fn post_aa_override_accepts_all_modes() {
    for value in ["off", "smaa_low", "smaa_medium", "smaa_high", "smaa_ultra"] {
        let mut config = Config::default();
        config.apply_post_aa_override(value).unwrap();
        assert_eq!(config.render.post_aa, PostAaMode::from_str(value).unwrap());
    }
}

#[test]
fn post_aa_override_rejects_unknown() {
    let mut config = Config::default();
    let err = config.apply_post_aa_override("junk").unwrap_err();
    match err {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, RENDER_POST_AA_ENV);
            assert_eq!(value, "junk");
            assert_eq!(expected, POST_AA_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn render_config_parses_depth_sort_gpu_depth() {
    let json = r#"{ "depth_sort": "gpu_depth" }"#;
    let parsed: RenderConfig = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.depth_sort, DepthSortMode::GpuDepth);
}

#[test]
fn render_config_parses_depth_sort_cpu_stable() {
    let json = r#"{ "depth_sort": "cpu_stable" }"#;
    let parsed: RenderConfig = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.depth_sort, DepthSortMode::CpuStable);
}

#[test]
fn render_config_rejects_unknown_depth_sort() {
    let json = r#"{ "depth_sort": "painters" }"#;
    let err = serde_json::from_str::<RenderConfig>(json).unwrap_err();
    assert!(err.is_data());
}

#[test]
fn msaa_override_accepts_supported_values() {
    for value in ["1", "2", "4", "8"] {
        let mut config = Config::default();
        config.apply_msaa_override(value).unwrap();
        assert_eq!(config.render.msaa, value.parse::<u32>().unwrap());
    }
}

#[test]
fn msaa_override_rejects_unsupported_values() {
    let mut config = Config::default();
    let err = config.apply_msaa_override("3").unwrap_err();
    match err {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, RENDER_MSAA_ENV);
            assert_eq!(value, "3");
            assert_eq!(expected, MSAA_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn depth_enabled_override_accepts_boolish_strings() {
    let mut config = Config::default();
    config.apply_depth_enabled_override("false").unwrap();
    assert!(!config.render.depth_enabled);
    config.apply_depth_enabled_override("1").unwrap();
    assert!(config.render.depth_enabled);
    config.apply_depth_enabled_override("0").unwrap();
    assert!(!config.render.depth_enabled);
    config.apply_depth_enabled_override("true").unwrap();
    assert!(config.render.depth_enabled);
}

#[test]
fn depth_sort_override_parses_both_modes() {
    let mut config = Config::default();
    config.apply_depth_sort_override("gpu_depth").unwrap();
    assert_eq!(config.render.depth_sort, DepthSortMode::GpuDepth);
    config.apply_depth_sort_override("cpu_stable").unwrap();
    assert_eq!(config.render.depth_sort, DepthSortMode::CpuStable);
}

#[test]
fn depth_sort_override_rejects_unknown() {
    let mut config = Config::default();
    let err = config.apply_depth_sort_override("painters").unwrap_err();
    match err {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, RENDER_DEPTH_SORT_ENV);
            assert_eq!(value, "painters");
            assert_eq!(expected, DEPTH_SORT_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn parses_partial_json() {
    let json = r#"{ "window": { "title": "Test" } }"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.window.title, "Test");
    assert_eq!(config.window.width, 1280);
    assert!(config.display.resolution.is_none());
    assert!(config.render.max_frame_latency.is_none());
    assert!(config.render.present_mode.is_none());
}

#[test]
fn parses_render_present_mode_and_latency() {
    let json = r#"{
        "render": {
            "max_frame_latency": 3,
            "present_mode": "auto_no_vsync"
        }
    }"#;

    let config: Config = serde_json::from_str(json).unwrap();

    assert_eq!(config.render.max_frame_latency, Some(3));
    assert_eq!(
        config.render.present_mode,
        Some(PresentModeConfig::AutoNoVsync)
    );
}

#[test]
fn present_mode_from_str_accepts_supported_values() {
    assert_eq!(
        PresentModeConfig::from_str("auto").unwrap(),
        PresentModeConfig::Auto
    );
    assert_eq!(
        PresentModeConfig::from_str("immediate").unwrap(),
        PresentModeConfig::Immediate
    );
    assert_eq!(
        PresentModeConfig::from_str("mailbox").unwrap(),
        PresentModeConfig::Mailbox
    );
    assert_eq!(
        PresentModeConfig::from_str("fifo").unwrap(),
        PresentModeConfig::Fifo
    );
    assert_eq!(
        PresentModeConfig::from_str("auto_vsync").unwrap(),
        PresentModeConfig::AutoVsync
    );
    assert_eq!(
        PresentModeConfig::from_str("auto_no_vsync").unwrap(),
        PresentModeConfig::AutoNoVsync
    );
}

#[test]
fn present_mode_override_updates_render_config() {
    let mut config = Config::default();
    config.apply_present_mode_override("mailbox").unwrap();
    assert_eq!(config.render.present_mode, Some(PresentModeConfig::Mailbox));
}

#[test]
fn invalid_present_mode_override_names_var_and_value() {
    let mut config = Config::default();
    let err = config
        .apply_present_mode_override("triple-buffer")
        .unwrap_err();

    match err {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, RENDER_PRESENT_MODE_ENV);
            assert_eq!(value, "triple-buffer");
            assert_eq!(expected, PRESENT_MODE_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn max_frame_latency_override_updates_render_config() {
    let mut config = Config::default();
    config.apply_max_frame_latency_override("3").unwrap();
    assert_eq!(config.render.max_frame_latency, Some(3));
}

#[test]
fn max_frame_latency_override_rejects_zero() {
    let mut config = Config::default();
    let err = config.apply_max_frame_latency_override("0").unwrap_err();

    match err {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, RENDER_MAX_FRAME_LATENCY_ENV);
            assert_eq!(value, "0");
            assert_eq!(expected, MAX_FRAME_LATENCY_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn display_mode_override_updates_display_config() {
    let mut config = Config::default();
    config
        .apply_display_mode_override("borderless_fullscreen")
        .unwrap();
    assert_eq!(
        config.display.display_mode,
        Some(DisplayMode::BorderlessFullscreen)
    );
}

#[test]
fn display_resolution_override_updates_display_config() {
    let mut config = Config::default();
    config
        .apply_display_resolution_override("1600x900")
        .unwrap();
    assert_eq!(
        config.display.resolution,
        Some(Resolution {
            width: 1600,
            height: 900
        })
    );
}

#[test]
fn display_frame_rate_cap_override_allows_uncapped_zero() {
    let mut config = Config::default();
    config.apply_display_frame_rate_cap_override("0").unwrap();
    assert_eq!(config.display.frame_rate_cap, None);
}

#[test]
fn missing_file_returns_defaults() {
    let config = Config::load("/nonexistent/path/tungsten.json").unwrap();
    assert_eq!(config.window.title, "Tungsten");
}

#[test]
fn bloom_max_mips_default_is_six() {
    let parsed: RenderConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(parsed.bloom_max_mips, 6);
}

#[test]
fn bloom_max_mips_parses_in_range() {
    for n in 1u32..=8 {
        let json = format!(r#"{{ "bloom_max_mips": {n} }}"#);
        let parsed: RenderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bloom_max_mips, n);
    }
}

#[test]
fn bloom_max_mips_env_override() {
    let mut config = Config::default();
    config.apply_bloom_max_mips_override("3").unwrap();
    assert_eq!(config.render.bloom_max_mips, 3);
}

#[test]
fn bloom_max_mips_rejects_zero_and_nine() {
    let mut config = Config::default();
    for bad in ["0", "9", "junk"] {
        let err = config.apply_bloom_max_mips_override(bad).unwrap_err();
        match err {
            ConfigError::InvalidEnvOverride {
                var,
                value,
                expected,
            } => {
                assert_eq!(var, RENDER_BLOOM_MAX_MIPS_ENV);
                assert_eq!(value, bad);
                assert_eq!(expected, BLOOM_MAX_MIPS_EXPECTED);
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}

#[test]
fn is_supported_bloom_max_mips_matches_expected_range() {
    assert!(!is_supported_bloom_max_mips(0));
    assert!(is_supported_bloom_max_mips(1));
    assert!(is_supported_bloom_max_mips(8));
    assert!(!is_supported_bloom_max_mips(9));
}
