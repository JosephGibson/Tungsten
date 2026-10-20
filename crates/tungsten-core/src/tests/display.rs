use super::*;
use crate::config::{RenderConfig, WindowConfig};
use glam::Vec2;

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

fn res(width: u32, height: u32) -> Resolution {
    Resolution { width, height }
}

fn mode(width: u32, height: u32, hz: u32) -> VideoMode {
    VideoMode::new(res(width, height), hz * 1000)
}

#[test]
fn monitor_info_drops_repeats_and_sorts_largest_first() {
    let monitor = MonitorInfo::new(
        res(1920, 1080),
        Some(60_000),
        [
            mode(1280, 720, 60),
            mode(1920, 1080, 60),
            mode(1920, 1080, 144),
            mode(1280, 720, 60),
            mode(0, 720, 60),
            mode(2560, 1080, 60),
            mode(1920, 1200, 60),
        ],
    )
    .unwrap();
    assert_eq!(monitor.size, res(1920, 1080));
    assert_eq!(monitor.refresh_millihertz, Some(60_000));
    assert_eq!(
        monitor.modes,
        [
            mode(2560, 1080, 60),
            mode(1920, 1200, 60),
            mode(1920, 1080, 144),
            mode(1920, 1080, 60),
            mode(1280, 720, 60),
        ],
        "largest area first, then width, then refresh; repeats and a 0 side dropped"
    );
    let unknown = MonitorInfo::new(res(800, 600), Some(0), []).unwrap();
    assert_eq!(
        unknown.refresh_millihertz, None,
        "a refresh of 0 is unknown"
    );
}

#[test]
fn window_sizes_keep_modes_inside_the_current_size_smallest_first() {
    // An X11 monitor lists modes past its current one (4096×2160 here).
    let monitor = MonitorInfo::new(
        res(3840, 2160),
        Some(60_000),
        [
            mode(4096, 2160, 60),
            mode(3840, 2160, 60),
            mode(3840, 2160, 30),
            mode(2560, 1440, 60),
            mode(1920, 1080, 60),
            mode(1600, 1200, 60),
            mode(1280, 720, 60),
            mode(800, 600, 60),
        ],
    )
    .unwrap();
    assert_eq!(
        monitor.window_sizes(),
        [
            res(800, 600),
            res(1280, 720),
            res(1600, 1200),
            res(1920, 1080),
            res(2560, 1440),
            res(3840, 2160),
        ]
    );
    // A Wayland output often lists only its current mode, or none.
    let one = MonitorInfo::new(res(1920, 1080), None, [mode(1920, 1080, 60)]).unwrap();
    assert_eq!(one.window_sizes(), [res(1920, 1080)]);
    let none = MonitorInfo::new(res(1366, 768), None, []).unwrap();
    assert_eq!(none.window_sizes(), [res(1366, 768)]);
}

#[test]
fn a_monitor_with_a_zero_side_is_none() {
    assert_eq!(MonitorInfo::new(res(0, 0), Some(60_000), []), None);
    assert_eq!(MonitorInfo::new(res(1920, 0), None, []), None);
    assert_eq!(
        MonitorInfo::new(res(0, 1080), None, [mode(1920, 1080, 60)]),
        None
    );
}

#[test]
fn scale_factor_defaults_to_os_times_ui() {
    let mut scale = ScaleFactor::default();
    assert_eq!(scale.reference_height(), None);
    assert_eq!(scale.min_viewport(), None);
    scale.set_os(1.75);
    scale.set_ui_scale(2.0);
    scale.set_window_size(3840, 2160);
    assert_eq!(
        scale.get(),
        3.5,
        "without a reference or a floor the window size changes nothing"
    );
    scale.set_window_size(0, 0);
    assert_eq!(scale.get(), 3.5);
}

#[test]
fn a_reference_height_takes_the_larger_of_os_and_window() {
    let mut scale = ScaleFactor::default();
    scale.set_reference_height(Some(720.0));
    assert_eq!(scale.reference_height(), Some(720.0));
    assert_eq!(scale.get(), 1.0, "no window size yet: the OS scale");
    scale.set_window_size(3840, 2160);
    assert_eq!(scale.get(), 3.0, "a 2160-pixel window at OS 1");
    scale.set_os(1.75);
    assert_eq!(scale.get(), 3.0, "the window's 3 beats OS 1.75");
    scale.set_window_size(1280, 720);
    assert_eq!(scale.get(), 1.75, "OS 1.75 beats the window's 1");
    scale.set_ui_scale(0.5);
    assert_eq!(scale.get(), 0.875, "the UI scale multiplies the base");
    scale.set_os(1.0);
    scale.set_window_size(1920, 1080);
    assert_eq!(scale.get(), 0.75);
    scale.set_reference_height(None);
    assert_eq!(scale.get(), 0.5, "None is the OS scale alone again");
    scale.set_reference_height(Some(720.0));
    scale.set_window_size(1920, 0);
    assert_eq!(
        scale.get(),
        0.5,
        "a window with a 0 side leaves the term out"
    );
}

#[test]
fn the_floor_holds_the_factor_down_on_either_axis() {
    let mut scale = ScaleFactor::default();
    scale.set_reference_height(Some(720.0));
    scale.set_min_viewport(Some(Vec2::new(731.0, 411.0)));
    assert_eq!(scale.min_viewport(), Some(Vec2::new(731.0, 411.0)));
    scale.set_os(1.75);
    scale.set_ui_scale(3.0);
    scale.set_window_size(1280, 720);
    let viewport = |scale: &ScaleFactor, w: f32, h: f32| (w / scale.get(), h / scale.get());
    let (w, h) = viewport(&scale, 1280.0, 720.0);
    assert!(
        (w - 731.0).abs() < 1e-3 && h >= 411.0,
        "the width binds: {w}×{h}"
    );
    scale.set_os(1.0);
    scale.set_window_size(3840, 2160);
    let (w, h) = viewport(&scale, 3840.0, 2160.0);
    assert!((w - 731.0).abs() < 1e-3 && h >= 411.0, "{w}×{h}");
    // A wide window: the height binds.
    scale.set_window_size(2000, 822);
    let (w, h) = viewport(&scale, 2000.0, 822.0);
    assert!(
        (h - 411.0).abs() < 1e-3 && w >= 731.0,
        "the height binds: {w}×{h}"
    );
    // Below the floor's pull the scales stand.
    scale.set_ui_scale(1.0);
    scale.set_window_size(3840, 2160);
    assert_eq!(scale.get(), 3.0);
    // A floor without a reference height holds the OS scale down too.
    let mut plain = ScaleFactor::default();
    plain.set_min_viewport(Some(Vec2::new(731.0, 411.0)));
    plain.set_os(2.0);
    plain.set_window_size(1280, 720);
    assert!((plain.get() - 1280.0 / 731.0).abs() < 1e-6);
    plain.set_min_viewport(None);
    assert_eq!(plain.get(), 2.0);
}

#[test]
fn bad_reference_or_floor_values_are_ignored() {
    let mut scale = ScaleFactor::default();
    scale.set_reference_height(Some(720.0));
    scale.set_min_viewport(Some(Vec2::new(731.0, 411.0)));
    for bad in [0.0, -720.0, f32::NAN, f32::INFINITY] {
        scale.set_reference_height(Some(bad));
        assert_eq!(scale.reference_height(), Some(720.0), "{bad} is ignored");
    }
    for bad in [
        Vec2::new(0.0, 411.0),
        Vec2::new(731.0, -1.0),
        Vec2::new(f32::NAN, 411.0),
        Vec2::new(731.0, f32::INFINITY),
    ] {
        scale.set_min_viewport(Some(bad));
        assert_eq!(
            scale.min_viewport(),
            Some(Vec2::new(731.0, 411.0)),
            "{bad} is ignored"
        );
    }
    scale.set_window_size(3840, 2160);
    assert_eq!(scale.get(), 3.0);
}
