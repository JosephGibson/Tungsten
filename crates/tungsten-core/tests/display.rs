use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tungsten_core::config::PresentModeConfig;
use tungsten_core::{Config, DisplayMode, Resolution, ScaleMode};

static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const DISPLAY_MODE_ENV: &str = "TUNGSTEN_DISPLAY_MODE";
const DISPLAY_RESOLUTION_ENV: &str = "TUNGSTEN_DISPLAY_RESOLUTION";
const DISPLAY_FRAME_RATE_CAP_ENV: &str = "TUNGSTEN_DISPLAY_FRAME_RATE_CAP";
const RENDER_PRESENT_MODE_ENV: &str = "TUNGSTEN_RENDER_PRESENT_MODE";
const RENDER_MAX_FRAME_LATENCY_ENV: &str = "TUNGSTEN_RENDER_MAX_FRAME_LATENCY";

fn write_temp_config(json: &str) -> PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "tungsten-display-config-{}-{}-{}.json",
        std::process::id(),
        nanos,
        id
    ));
    fs::write(&path, json).expect("temp config should be writable");
    path
}

fn clear_display_env() {
    for var in [
        DISPLAY_MODE_ENV,
        DISPLAY_RESOLUTION_ENV,
        DISPLAY_FRAME_RATE_CAP_ENV,
        RENDER_PRESENT_MODE_ENV,
        RENDER_MAX_FRAME_LATENCY_ENV,
    ] {
        unsafe {
            std::env::remove_var(var);
        }
    }
}

#[test]
fn full_config_parse_with_display_section() {
    let config: Config = serde_json::from_str(
        r#"{
            "window": { "title": "Test" },
            "display": {
                "resolution": { "width": 1600, "height": 900 },
                "display_mode": "borderless_fullscreen",
                "vsync": true,
                "present_mode": "fifo",
                "max_frame_latency": 3,
                "scale_mode": "integer",
                "frame_rate_cap": 144
            }
        }"#,
    )
    .unwrap();

    let resolved = config.display.resolve(&config.window, &config.render);
    assert_eq!(
        resolved.resolution,
        Resolution {
            width: 1600,
            height: 900
        }
    );
    assert_eq!(resolved.display_mode, DisplayMode::BorderlessFullscreen);
    assert!(resolved.vsync);
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::Fifo));
    assert_eq!(resolved.max_frame_latency, Some(3));
    assert_eq!(resolved.scale_mode, ScaleMode::Integer);
    assert_eq!(resolved.frame_rate_cap, Some(144));
}

#[test]
fn partial_display_section_falls_back_to_legacy_values() {
    let config: Config = serde_json::from_str(
        r#"{
            "window": { "width": 1920, "height": 1080, "vsync": true },
            "render": { "present_mode": "auto_no_vsync", "max_frame_latency": 2 },
            "display": { "scale_mode": "integer" }
        }"#,
    )
    .unwrap();

    let resolved = config.display.resolve(&config.window, &config.render);
    assert_eq!(
        resolved.resolution,
        Resolution {
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(resolved.display_mode, DisplayMode::Windowed);
    assert!(resolved.vsync);
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::AutoNoVsync));
    assert_eq!(resolved.max_frame_latency, Some(2));
    assert_eq!(resolved.scale_mode, ScaleMode::Integer);
    assert_eq!(resolved.frame_rate_cap, None);
}

#[test]
fn legacy_only_config_still_resolves_correctly() {
    let config: Config = serde_json::from_str(
        r#"{
            "window": { "width": 1024, "height": 768, "vsync": true },
            "render": { "present_mode": "mailbox", "max_frame_latency": 4 }
        }"#,
    )
    .unwrap();

    let resolved = config.display.resolve(&config.window, &config.render);
    assert_eq!(
        resolved.resolution,
        Resolution {
            width: 1024,
            height: 768
        }
    );
    assert_eq!(resolved.display_mode, DisplayMode::Windowed);
    assert!(resolved.vsync);
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::Mailbox));
    assert_eq!(resolved.max_frame_latency, Some(4));
    assert_eq!(resolved.scale_mode, ScaleMode::Stretch);
    assert_eq!(resolved.frame_rate_cap, None);
}

#[test]
fn explicit_display_values_prefer_display_over_legacy() {
    let config: Config = serde_json::from_str(
        r#"{
            "window": { "width": 1024, "height": 768, "vsync": false },
            "render": { "present_mode": "mailbox", "max_frame_latency": 4 },
            "display": {
                "resolution": { "width": 2560, "height": 1440 },
                "vsync": true,
                "present_mode": "fifo",
                "max_frame_latency": 2
            }
        }"#,
    )
    .unwrap();

    let resolved = config.display.resolve(&config.window, &config.render);
    assert_eq!(
        resolved.resolution,
        Resolution {
            width: 2560,
            height: 1440
        }
    );
    assert!(resolved.vsync);
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::Fifo));
    assert_eq!(resolved.max_frame_latency, Some(2));
}

#[test]
fn env_overrides_apply_on_top_of_file_config() {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    clear_display_env();

    unsafe {
        std::env::set_var(DISPLAY_MODE_ENV, "borderless_fullscreen");
        std::env::set_var(DISPLAY_RESOLUTION_ENV, "1440x900");
        std::env::set_var(DISPLAY_FRAME_RATE_CAP_ENV, "165");
        std::env::set_var(RENDER_PRESENT_MODE_ENV, "fifo");
        std::env::set_var(RENDER_MAX_FRAME_LATENCY_ENV, "3");
    }

    let path = write_temp_config(
        r#"{
            "window": { "width": 800, "height": 600, "vsync": false },
            "render": { "present_mode": "immediate", "max_frame_latency": 1 },
            "display": {
                "display_mode": "windowed",
                "resolution": { "width": 1280, "height": 720 },
                "frame_rate_cap": 60
            }
        }"#,
    );

    let config = Config::load(&path).unwrap();
    let resolved = config.display.resolve(&config.window, &config.render);

    assert_eq!(resolved.display_mode, DisplayMode::BorderlessFullscreen);
    assert_eq!(
        resolved.resolution,
        Resolution {
            width: 1440,
            height: 900
        }
    );
    assert_eq!(resolved.frame_rate_cap, Some(165));
    assert_eq!(resolved.present_mode, Some(PresentModeConfig::Fifo));
    assert_eq!(resolved.max_frame_latency, Some(3));

    clear_display_env();
    let _ = fs::remove_file(path);
}
