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
}

#[test]
fn parses_partial_json() {
    let json = r#"{ "window": { "title": "Test" } }"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.window.title, "Test");
    assert_eq!(config.window.width, 1280); // default
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
