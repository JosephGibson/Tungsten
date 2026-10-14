use super::*;
use crate::config::{RenderConfig, WindowConfig};

#[test]
fn defaults_are_stable() {
    let state = DisplayState::default();
    assert_eq!(
        state,
        DisplayState {
            resolution: Resolution {
                width: 1280,
                height: 720,
            },
            display_mode: DisplayMode::Windowed,
            vsync: false,
            present_mode: None,
            max_frame_latency: None,
            scale_mode: ScaleMode::Stretch,
            frame_rate_cap: None,
        }
    );
}

#[test]
fn validation_rejects_zero_dimensions() {
    let err = DisplayState {
        resolution: Resolution {
            width: 0,
            height: 720,
        },
        ..DisplayState::default()
    }
    .validate()
    .unwrap_err();

    assert_eq!(
        err,
        DisplayValidationError::InvalidResolution {
            width: 0,
            height: 720
        }
    );
}

#[test]
fn validation_rejects_zero_frame_rate_cap() {
    let err = DisplayState {
        frame_rate_cap: Some(0),
        ..DisplayState::default()
    }
    .validate()
    .unwrap_err();

    assert_eq!(err, DisplayValidationError::InvalidFrameRateCap(0));
}

#[test]
fn unknown_enum_values_fall_back_to_safe_defaults() {
    let config: DisplayConfig = serde_json::from_str(
        r#"{
            "display_mode": "theater_mode",
            "scale_mode": "pixel_perfectish"
        }"#,
    )
    .unwrap();

    let resolved = config.resolve(&WindowConfig::default(), &RenderConfig::default());
    assert_eq!(resolved.display_mode, DisplayMode::Windowed);
    assert_eq!(resolved.scale_mode, ScaleMode::Stretch);
}

#[test]
fn invalid_resolution_falls_back_to_legacy_defaults() {
    let config: DisplayConfig = serde_json::from_str(
        r#"{
            "resolution": { "width": 0, "height": 720 }
        }"#,
    )
    .unwrap();

    let resolved = config.resolve(&WindowConfig::default(), &RenderConfig::default());
    assert_eq!(
        resolved.resolution,
        Resolution {
            width: 1280,
            height: 720
        }
    );
}

#[test]
fn resolve_honors_display_precedence_over_legacy_fields() {
    let config: DisplayConfig = serde_json::from_str(
        r#"{
            "resolution": { "width": 1600, "height": 900 },
            "vsync": true,
            "present_mode": "fifo",
            "max_frame_latency": 3,
            "frame_rate_cap": 144
        }"#,
    )
    .unwrap();

    let window = WindowConfig {
        width: 800,
        height: 600,
        vsync: false,
        ..WindowConfig::default()
    };
    let render = RenderConfig {
        present_mode: Some(PresentModeConfig::Immediate),
        max_frame_latency: Some(1),
        ..RenderConfig::default()
    };

    let resolved = config.resolve(&window, &render);

    assert_eq!(
        resolved,
        DisplayState {
            resolution: Resolution {
                width: 1600,
                height: 900,
            },
            display_mode: DisplayMode::Windowed,
            vsync: true,
            present_mode: Some(PresentModeConfig::Fifo),
            max_frame_latency: Some(3),
            scale_mode: ScaleMode::Stretch,
            frame_rate_cap: Some(144),
        }
    );
}

#[test]
fn partial_display_sections_inherit_unspecified_fields() {
    let config: DisplayConfig = serde_json::from_str(
        r#"{
            "display_mode": "borderless_fullscreen",
            "scale_mode": "integer"
        }"#,
    )
    .unwrap();

    let window = WindowConfig {
        width: 1920,
        height: 1080,
        vsync: true,
        ..WindowConfig::default()
    };
    let render = RenderConfig {
        present_mode: Some(PresentModeConfig::AutoNoVsync),
        max_frame_latency: Some(2),
        ..RenderConfig::default()
    };

    let resolved = config.resolve(&window, &render);

    assert_eq!(
        resolved,
        DisplayState {
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            display_mode: DisplayMode::BorderlessFullscreen,
            vsync: true,
            present_mode: Some(PresentModeConfig::AutoNoVsync),
            max_frame_latency: Some(2),
            scale_mode: ScaleMode::Integer,
            frame_rate_cap: None,
        }
    );
}

#[test]
fn parsing_the_section_collects_each_fallback_in_field_order() {
    let mut warnings = Vec::new();
    let config = DisplayConfig::from_json_value(
        serde_json::json!({
            "resolution": { "width": 0, "height": 720 },
            "display_mode": "theater_mode",
            "vsync": "yes",
            "frame_rate_cap": 0
        }),
        &mut warnings,
    );
    assert_eq!(config, DisplayConfig::default());
    assert_eq!(warnings.len(), 4, "{warnings:?}");
    assert!(warnings[0].starts_with("Config display.resolution=0x720 is invalid"));
    assert!(warnings[1].starts_with("Config display.display_mode='theater_mode' is invalid"));
    assert!(warnings[2].starts_with(r#"Config display.vsync="yes" is invalid"#));
    assert_eq!(
        warnings[3],
        "Config display.frame_rate_cap=0 means uncapped; using None"
    );
}

#[test]
fn a_section_that_is_not_an_object_is_one_warning() {
    let mut warnings = Vec::new();
    let config = DisplayConfig::from_json_value(serde_json::json!(42), &mut warnings);
    assert_eq!(config, DisplayConfig::default());
    assert_eq!(
        warnings,
        ["Config display section must be an object; ignoring invalid value"]
    );
}

#[test]
fn a_valid_section_collects_nothing() {
    let mut warnings = Vec::new();
    let config = DisplayConfig::from_json_value(
        serde_json::json!({ "display_mode": "borderless_fullscreen", "frame_rate_cap": 144 }),
        &mut warnings,
    );
    assert_eq!(config.display_mode, Some(DisplayMode::BorderlessFullscreen));
    assert_eq!(config.frame_rate_cap, Some(144));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn scale_factor_defaults_to_one() {
    let scale = ScaleFactor::default();
    assert_eq!(scale.os(), 1.0);
    assert_eq!(scale.get(), 1.0);
}

#[test]
fn scale_factor_set_os_sets_both_readings() {
    let mut scale = ScaleFactor::default();
    scale.set_os(1.5);
    assert_eq!(scale.os(), 1.5);
    assert_eq!(
        scale.get(),
        1.5,
        "the UI scale defaults to 1, so get is the OS scale"
    );
}

#[test]
fn get_is_the_os_scale_times_the_ui_scale() {
    let mut scale = ScaleFactor::default();
    assert_eq!(scale.ui_scale(), 1.0);
    scale.set_os(2.0);
    scale.set_ui_scale(1.25);
    assert_eq!(scale.os(), 2.0);
    assert_eq!(scale.ui_scale(), 1.25);
    assert_eq!(scale.get(), 2.5);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        scale.set_ui_scale(bad);
        assert_eq!(scale.ui_scale(), 1.25, "{bad} is ignored");
    }
    scale.set_os(1.0);
    assert_eq!(scale.get(), 1.25);
}

#[test]
fn scale_factor_ignores_values_that_are_not_finite_and_positive() {
    let mut scale = ScaleFactor::default();
    scale.set_os(2.0);
    for bad in [0.0, -0.0, -1.25, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        scale.set_os(bad);
        assert_eq!(scale.os(), 2.0, "{bad} is ignored");
    }
}
