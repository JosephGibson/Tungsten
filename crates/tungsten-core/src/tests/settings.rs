use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tungsten_settings_test_{}_{}",
        std::process::id(),
        nonce
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load_text(text: &str) -> (PathBuf, Settings) {
    let path = tempdir().join(Settings::FILE);
    std::fs::write(&path, text).unwrap();
    let settings = Settings::load(&path);
    (path, settings)
}

fn is_default(settings: &Settings) -> bool {
    settings.display_mode().is_none()
        && settings.resolution().is_none()
        && settings.vsync().is_none()
        && settings.frame_rate_cap().is_none()
        && settings.ui_scale().is_none()
        && settings.master_volume().is_none()
        && AudioBus::ALL
            .iter()
            .all(|&bus| settings.bus_volume(bus).is_none())
}

#[test]
fn a_missing_file_gives_defaults_without_a_warning() {
    let path = tempdir().join(Settings::FILE);
    let mut settings = Settings::load(&path);
    assert!(is_default(&settings));
    assert_eq!(settings.path(), Some(path.as_path()));
    assert!(settings.take_load_warnings().is_empty());
    assert!(!path.exists(), "loading writes nothing");
}

#[test]
fn a_corrupt_file_falls_back_and_stays_beside_as_a_backup() {
    let corrupt = b"{ \"ui_scale\": 1.5, \"display\": ";
    for bytes in [
        &corrupt[..],
        b"\xff\xfe not utf-8",
        b"[]",
        b"{\"display\": []}",
    ] {
        let dir = tempdir();
        let path = dir.join(Settings::FILE);
        std::fs::write(&path, bytes).unwrap();
        let mut settings = Settings::load(&path);
        assert!(is_default(&settings));
        let backup = dir.join("settings.json.bad");
        assert_eq!(std::fs::read(&backup).unwrap(), bytes, "the bytes are kept");
        assert!(!path.exists());
        let warnings = settings.take_load_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains(&path.display().to_string())
                && warnings[0].contains(&backup.display().to_string()),
            "{warnings:?}"
        );
        assert!(settings.take_load_warnings().is_empty());
    }
}

#[test]
fn an_out_of_range_value_invalidates_the_file() {
    for text in [
        r#"{ "ui_scale": 0.25, "master_volume": 0.5 }"#,
        r#"{ "ui_scale": 3.5 }"#,
        r#"{ "ui_scale": 1e40 }"#,
        r#"{ "master_volume": 1.5 }"#,
        r#"{ "master_volume": -0.1 }"#,
        r#"{ "master_volume": "loud" }"#,
        r#"{ "display": { "display_mode": "fullscreen" } }"#,
        r#"{ "display": { "resolution": { "width": 0, "height": 720 } } }"#,
        r#"{ "display": { "resolution": [1920, 1080] } }"#,
        r#"{ "display": { "vsync": "yes" } }"#,
    ] {
        let (path, mut settings) = load_text(text);
        assert!(is_default(&settings), "{text}");
        assert_eq!(settings.take_load_warnings().len(), 1, "{text}");
        assert!(path.with_file_name("settings.json.bad").exists(), "{text}");
    }
}

#[test]
fn unknown_keys_are_ignored() {
    let (path, mut settings) = load_text(
        r#"{ "ui_scale": 1.5, "bus_volume": { "music": 0.2 }, "display": { "vsync": true, "hdr": true } }"#,
    );
    assert_eq!(settings.ui_scale(), Some(1.5));
    assert_eq!(settings.vsync(), Some(true));
    assert!(settings.take_load_warnings().is_empty());
    assert!(path.exists());
    assert!(!path.with_file_name("settings.json.bad").exists());
}

#[test]
fn save_writes_only_set_values_and_load_reads_them_back() {
    let path = tempdir().join("settings").join(Settings::FILE);
    let mut settings = Settings::load(&path);
    settings.set_master_volume(Some(0.0));
    settings.save().unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written, serde_json::json!({ "master_volume": 0.0 }));

    settings.set_display_mode(Some(DisplayMode::BorderlessFullscreen));
    settings.set_resolution(Some(Resolution {
        width: 1920,
        height: 1080,
    }));
    settings.set_vsync(Some(true));
    settings.set_ui_scale(Some(1.25));
    settings.set_master_volume(Some(0.8));
    settings.save().unwrap();
    let mut reloaded = Settings::load(&path);
    assert!(reloaded.take_load_warnings().is_empty());
    assert_eq!(reloaded, settings);

    settings.set_display_mode(None);
    settings.set_resolution(None);
    settings.set_vsync(None);
    settings.save().unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "ui_scale": 1.25, "master_volume": 0.8 }),
        "no empty display object"
    );
}

#[test]
fn save_without_a_folder_is_no_folder() {
    let mut settings = Settings::default();
    settings.set_ui_scale(Some(2.0));
    assert_eq!(settings.path(), None);
    assert!(matches!(settings.save(), Err(SettingsError::NoFolder)));
}

#[test]
fn setters_clamp_and_ignore_non_finite() {
    let mut settings = Settings::default();
    settings.set_ui_scale(Some(0.1));
    assert_eq!(settings.ui_scale(), Some(0.5));
    settings.set_ui_scale(Some(9.0));
    assert_eq!(settings.ui_scale(), Some(3.0));
    settings.set_ui_scale(Some(f32::NAN));
    assert_eq!(settings.ui_scale(), Some(3.0), "NaN is ignored");
    settings.set_ui_scale(None);
    assert_eq!(settings.ui_scale(), None);

    settings.set_master_volume(Some(-1.0));
    assert_eq!(settings.master_volume(), Some(0.0));
    settings.set_master_volume(Some(2.0));
    assert_eq!(settings.master_volume(), Some(1.0));
    settings.set_master_volume(Some(f32::INFINITY));
    assert_eq!(settings.master_volume(), Some(1.0), "infinity is ignored");

    let hd = Resolution {
        width: 1920,
        height: 1080,
    };
    settings.set_resolution(Some(hd));
    settings.set_resolution(Some(Resolution {
        width: 0,
        height: 1080,
    }));
    assert_eq!(settings.resolution(), Some(hd), "a zero side is ignored");
}

#[test]
fn the_text_scale_saves_loads_clamps_and_falls_back() {
    let dir = tempdir();
    let path = dir.join(Settings::FILE);
    let mut settings = Settings::load(&path);
    assert_eq!(settings.text_scale(), None);
    settings.set_text_scale(Some(1.5));
    assert_eq!(settings.text_scale(), Some(1.5));
    settings.save().unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({ "text_scale": 1.5 }));
    assert_eq!(Settings::load(&path).text_scale(), Some(1.5));

    settings.set_text_scale(Some(9.0));
    assert_eq!(settings.text_scale(), Some(2.0));
    settings.set_text_scale(Some(0.1));
    assert_eq!(settings.text_scale(), Some(0.5));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        settings.set_text_scale(Some(value));
        assert_eq!(settings.text_scale(), Some(0.5), "{value} ignored");
    }
    settings.set_text_scale(None);
    assert_eq!(settings.text_scale(), None);

    let (_, without) = load_text(r#"{ "ui_scale": 1.5 }"#);
    assert_eq!(without.text_scale(), None, "a file without the key");
    assert_eq!(without.ui_scale(), Some(1.5));

    for text in [r#"{ "text_scale": "big" }"#, r#"{ "text_scale": 3.0 }"#] {
        let (path, mut fallen) = load_text(text);
        assert!(
            is_default(&fallen) && fallen.text_scale().is_none(),
            "{text}"
        );
        let backup = path.with_file_name("settings.json.bad");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), text);
        assert_eq!(fallen.take_load_warnings().len(), 1);
    }
}

#[test]
fn bus_volumes_save_load_and_stay_out_of_the_file_when_unset() {
    let path = tempdir().join(Settings::FILE);
    let mut settings = Settings::load(&path);
    assert_eq!(settings.bus_volume(AudioBus::Music), None);
    settings.set_bus_volume(AudioBus::Music, Some(0.5));
    settings.save().unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "bus_volumes": { "music": 0.5 } })
    );

    settings.set_bus_volume(AudioBus::Effects, Some(0.0));
    settings.set_master_volume(Some(0.8));
    settings.save().unwrap();
    let mut reloaded = Settings::load(&path);
    assert!(reloaded.take_load_warnings().is_empty());
    assert_eq!(reloaded, settings);
    assert_eq!(reloaded.bus_volume(AudioBus::Effects), Some(0.0));

    settings.set_bus_volume(AudioBus::Music, None);
    settings.set_bus_volume(AudioBus::Effects, None);
    settings.save().unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "master_volume": 0.8 }),
        "no empty bus_volumes object"
    );
}

#[test]
fn an_out_of_range_bus_volume_sets_the_file_aside_and_an_unknown_bus_is_ignored() {
    for text in [
        r#"{ "bus_volumes": { "music": 1.5 } }"#,
        r#"{ "bus_volumes": { "effects": -0.1, "music": 0.5 } }"#,
        r#"{ "bus_volumes": { "music": "loud" } }"#,
        r#"{ "bus_volumes": { "music": null } }"#,
        r#"{ "bus_volumes": [0.5] }"#,
        r#"{ "bus_volumes": 0.5 }"#,
    ] {
        let (path, mut settings) = load_text(text);
        assert!(is_default(&settings), "{text}");
        assert_eq!(settings.take_load_warnings().len(), 1, "{text}");
        let backup = path.with_file_name("settings.json.bad");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), text);
    }
    let (path, mut settings) = load_text(
        r#"{ "bus_volumes": { "music": 0.25, "voice": 7, "ambience": "loud" }, "master_volume": 0.5 }"#,
    );
    assert!(settings.take_load_warnings().is_empty());
    assert_eq!(settings.bus_volume(AudioBus::Music), Some(0.25));
    assert_eq!(settings.bus_volume(AudioBus::Effects), None);
    assert_eq!(settings.master_volume(), Some(0.5));
    assert!(path.exists() && !path.with_file_name("settings.json.bad").exists());
}

#[test]
fn the_bus_volume_setters_clamp_and_ignore_non_finite() {
    let mut settings = Settings::default();
    settings.set_bus_volume(AudioBus::Music, Some(-1.0));
    assert_eq!(settings.bus_volume(AudioBus::Music), Some(0.0));
    settings.set_bus_volume(AudioBus::Music, Some(2.0));
    assert_eq!(settings.bus_volume(AudioBus::Music), Some(1.0));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        settings.set_bus_volume(AudioBus::Music, Some(value));
        assert_eq!(
            settings.bus_volume(AudioBus::Music),
            Some(1.0),
            "{value} ignored"
        );
    }
    assert_eq!(
        settings.bus_volume(AudioBus::Effects),
        None,
        "one bus at a time"
    );
    settings.set_bus_volume(AudioBus::Music, None);
    assert_eq!(settings.bus_volume(AudioBus::Music), None);
}

fn fps(n: u32) -> FrameRateCap {
    FrameRateCap::Fps(NonZeroU32::new(n).unwrap())
}

#[test]
fn player_frame_cap_round_trips_a_number_and_uncapped() {
    for (cap, json) in [
        (fps(60), serde_json::json!(60)),
        (FrameRateCap::Uncapped, serde_json::json!("uncapped")),
    ] {
        let path = tempdir().join(Settings::FILE);
        let mut settings = Settings::load(&path);
        settings.set_frame_rate_cap(Some(cap));
        settings.save().unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            written,
            serde_json::json!({ "display": { "frame_rate_cap": json } })
        );
        let mut reloaded = Settings::load(&path);
        assert!(reloaded.take_load_warnings().is_empty());
        assert_eq!(reloaded.frame_rate_cap(), Some(cap));
        assert_eq!(reloaded, settings);

        settings.set_frame_rate_cap(None);
        settings.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{}\n",
            "unset, absent"
        );
    }
}

#[test]
fn player_frame_cap_any_other_value_quarantines_the_file() {
    for value in ["0", "-1", r#""fast""#, "59.5", "4294967296", "true", "[60]"] {
        let text = format!(r#"{{ "ui_scale": 1.5, "display": {{ "frame_rate_cap": {value} }} }}"#);
        let (path, mut settings) = load_text(&text);
        assert!(is_default(&settings), "{text}");
        let warnings = settings.take_load_warnings();
        assert_eq!(warnings.len(), 1, "{text}");
        assert!(warnings[0].contains("frame_rate_cap"), "{warnings:?}");
        assert!(path.with_file_name("settings.json.bad").exists(), "{text}");
    }
    let (_, mut settings) = load_text(r#"{ "display": { "frame_rate_cap": null } }"#);
    assert_eq!(
        settings.frame_rate_cap(),
        None,
        "null is unset, as elsewhere"
    );
    assert!(settings.take_load_warnings().is_empty());
}
