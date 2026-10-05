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

/// The checked-in `tungsten.json` sets both display pacing fields like this.
const DISPLAY_PACING_JSON: &str =
    r#"{ "display": { "present_mode": "auto", "max_frame_latency": 1 } }"#;

#[test]
fn display_pacing_overrides_win_over_display_values_from_the_file() {
    let mut config: Config = serde_json::from_str(DISPLAY_PACING_JSON).unwrap();
    config.apply_display_present_mode_override("fifo").unwrap();
    config
        .apply_display_max_frame_latency_override("2")
        .unwrap();

    let resolved = config.display.resolve(&config.window, &config.render);
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::Fifo));
    assert_eq!(resolved.max_frame_latency, Some(2));
}

#[test]
fn render_pacing_overrides_lose_to_display_values_from_the_file() {
    let mut config: Config = serde_json::from_str(DISPLAY_PACING_JSON).unwrap();
    config.apply_present_mode_override("fifo").unwrap();
    config.apply_max_frame_latency_override("3").unwrap();

    let resolved = config.display.resolve(&config.window, &config.render);
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::Auto));
    assert_eq!(resolved.max_frame_latency, Some(1));
}

#[test]
fn invalid_display_pacing_overrides_name_var_and_value() {
    let mut config = Config::default();

    match config
        .apply_display_present_mode_override("triple-buffer")
        .unwrap_err()
    {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, DISPLAY_PRESENT_MODE_ENV);
            assert_eq!(value, "triple-buffer");
            assert_eq!(expected, PRESENT_MODE_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }

    match config
        .apply_display_max_frame_latency_override("0")
        .unwrap_err()
    {
        ConfigError::InvalidEnvOverride {
            var,
            value,
            expected,
        } => {
            assert_eq!(var, DISPLAY_MAX_FRAME_LATENCY_ENV);
            assert_eq!(value, "0");
            assert_eq!(expected, MAX_FRAME_LATENCY_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(config.display.present_mode.is_none());
    assert!(config.display.max_frame_latency.is_none());
}

#[test]
fn missing_file_returns_defaults() {
    let config = Config::load("/nonexistent/path/tungsten.json").unwrap();
    assert_eq!(config.window.title, "Tungsten");
}

#[test]
fn system_fonts_default_off_and_parse_on() {
    assert!(!RenderConfig::default().system_fonts);
    let parsed: RenderConfig = serde_json::from_str("{}").unwrap();
    assert!(!parsed.system_fonts);
    let parsed: RenderConfig = serde_json::from_str(r#"{ "system_fonts": true }"#).unwrap();
    assert!(parsed.system_fonts);
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

#[test]
fn file_msaa_error_is_not_an_env_override() {
    // B7: an unsupported `render.msaa` read from the file names the file and
    // the field, not an env override nobody set.
    let path = std::env::temp_dir().join(format!("tungsten-b7-{}.json", std::process::id()));
    std::fs::write(&path, r#"{"render": {"msaa": 3}}"#).unwrap();
    let err = Config::load(&path).unwrap_err();
    let _ = std::fs::remove_file(&path);
    let message = err.to_string();
    match err {
        ConfigError::InvalidValue {
            path: file,
            field,
            value,
            expected,
        } => {
            assert_eq!(file, path.display().to_string());
            assert_eq!(field, "render.msaa");
            assert_eq!(value, "3");
            assert_eq!(expected, MSAA_EXPECTED);
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(message.contains(&path.display().to_string()), "{message}");
    assert!(!message.contains("env override"), "{message}");
}

/// Loads `json` from a temp file named after `tag`, then removes the file.
fn load_json(tag: &str, json: &str) -> (std::path::PathBuf, Result<Config, ConfigError>) {
    let path = std::env::temp_dir().join(format!("tungsten-{tag}-{}.json", std::process::id()));
    std::fs::write(&path, json).unwrap();
    let result = Config::load(&path);
    let _ = std::fs::remove_file(&path);
    (path, result)
}

fn game_with_id(id: &str) -> GameConfig {
    GameConfig {
        id: Some(id.to_string()),
        version: None,
    }
}

fn game_id_json(id: &str) -> String {
    serde_json::json!({ "game": { "id": id } }).to_string()
}

#[test]
fn game_section_defaults_to_no_id() {
    assert!(Config::default().game.id.is_none());
    let config: Config = serde_json::from_str("{}").unwrap();
    assert!(config.game.id.is_none());
    assert!(config.game.version.is_none());
    assert!(config.game.validate().is_ok());
    let config: Config =
        serde_json::from_str(r#"{ "game": { "id": "my-game", "version": "1.2 beta" } }"#).unwrap();
    assert_eq!(config.game.id.as_deref(), Some("my-game"));
    assert_eq!(config.game.version.as_deref(), Some("1.2 beta"));
}

#[test]
fn game_ids_that_pass_validate_and_load() {
    for (n, id) in ["my-game", "com.example.game"].into_iter().enumerate() {
        assert!(game_with_id(id).validate().is_ok(), "{id}");
        let (_, loaded) = load_json(&format!("game-id-ok-{n}"), &game_id_json(id));
        assert_eq!(loaded.unwrap().game.id.as_deref(), Some(id));
    }
}

#[test]
fn game_ids_that_fail_name_the_field_through_validate_and_load() {
    let too_long = "a".repeat(65);
    let bad = [
        "",
        too_long.as_str(),
        "../x",
        "/abs",
        ".x",
        "a/b",
        "con.txt",
    ];
    for (n, id) in bad.into_iter().enumerate() {
        match game_with_id(id).validate().unwrap_err() {
            ConfigError::InvalidValue {
                path, field, value, ..
            } => {
                assert_eq!(field, "game.id", "{id}");
                assert_eq!(value, id);
                assert!(path.is_empty(), "{path}");
            }
            other => panic!("unexpected error for '{id}': {other}"),
        }
        let (file, loaded) = load_json(&format!("game-id-bad-{n}"), &game_id_json(id));
        match loaded.unwrap_err() {
            ConfigError::InvalidValue { path, field, .. } => {
                assert_eq!(field, "game.id", "{id}");
                assert_eq!(path, file.display().to_string());
            }
            other => panic!("unexpected error for '{id}': {other}"),
        }
    }
}

#[test]
fn game_id_length_limit_is_64() {
    assert!(game_with_id(&"a".repeat(64)).validate().is_ok());
    assert!(game_with_id(&"a".repeat(65)).validate().is_err());
}

#[test]
fn windows_device_names_fail_in_any_case_and_with_any_extension() {
    for id in [
        "CON",
        "prn",
        "Aux",
        "nul.tar.gz",
        "com1",
        "COM9.log",
        "lpt1",
        "Lpt9",
    ] {
        assert!(game_with_id(id).validate().is_err(), "{id}");
    }
    for id in ["com0", "lpt10", "console", "nul-game", "auxiliary", "comx"] {
        assert!(game_with_id(id).validate().is_ok(), "{id}");
    }
}

#[test]
fn game_id_error_set_in_code_names_no_file() {
    let message = game_with_id("../x").validate().unwrap_err().to_string();
    assert!(
        message.starts_with("invalid game.id='../x': expected"),
        "{message}"
    );
}

#[test]
fn logging_level_must_name_a_level() {
    let (file, loaded) = load_json("level-bad", r#"{ "logging": { "level": "verbose" } }"#);
    match loaded.unwrap_err() {
        ConfigError::InvalidValue {
            path,
            field,
            value,
            expected,
        } => {
            assert_eq!(field, "logging.level");
            assert_eq!(value, "verbose");
            assert_eq!(expected, LOGGING_LEVEL_EXPECTED);
            assert_eq!(path, file.display().to_string());
        }
        other => panic!("unexpected error: {other}"),
    }
    let (_, loaded) = load_json("level-upper", r#"{ "logging": { "level": "WARN" } }"#);
    assert_eq!(loaded.unwrap().logging.level, "WARN");
}

#[test]
fn a_missing_file_warning_comes_back_from_take_load_warnings() {
    let mut config = Config::load("/nonexistent/path/tungsten.json").unwrap();
    let warnings = config.take_load_warnings();
    assert_eq!(
        warnings,
        ["Config file '/nonexistent/path/tungsten.json' not found, using defaults"]
    );
    assert!(config.take_load_warnings().is_empty());
}

#[test]
fn a_display_fallback_comes_back_from_take_load_warnings() {
    let json = r#"{ "display": { "display_mode": "theater_mode", "scale_mode": "integer" } }"#;
    let (_, loaded) = load_json("display-fallback", json);
    let mut config = loaded.unwrap();
    assert!(config.display.display_mode.is_none());
    assert_eq!(
        config.display.scale_mode,
        Some(crate::display::ScaleMode::Integer)
    );
    let warnings = config.take_load_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("Config display.display_mode='theater_mode' is invalid"),
        "{warnings:?}"
    );
}

#[test]
fn a_legacy_conflict_comes_back_from_take_load_warnings_before_fallbacks() {
    let json = r#"{
        "window": { "vsync": false },
        "display": { "vsync": true, "frame_rate_cap": 0 }
    }"#;
    let (_, loaded) = load_json("legacy-conflict", json);
    let mut config = loaded.unwrap();
    assert_eq!(config.display.vsync, Some(true));
    assert_eq!(
        config.take_load_warnings(),
        [
            "Config display.vsync overrides legacy window.vsync",
            "Config display.frame_rate_cap=0 means uncapped; using None",
        ]
    );
}

#[test]
fn configs_not_from_load_keep_no_warnings() {
    assert!(Config::default().take_load_warnings().is_empty());
    let mut parsed: Config =
        serde_json::from_str(r#"{ "display": { "display_mode": "theater_mode" } }"#).unwrap();
    assert!(parsed.take_load_warnings().is_empty());
}
