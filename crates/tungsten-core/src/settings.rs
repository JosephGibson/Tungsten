//! The player's settings, kept in the game's user folder (`D-159`): the
//! display mode, resolution, vsync and frame-rate limit (`D-180`), the UI
//! scale, the text scale (M56), the master volume and each bus's volume,
//! layered between `tungsten.json` and the environment.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::audio::AudioBus;
use crate::display::{DisplayMode, Resolution};
use crate::persist::{WriteError, quarantine, write_atomic};

/// How many buses there are: [`AudioBus::ALL`]'s length.
const BUSES: usize = AudioBus::ALL.len();

/// A bus's place in [`AudioBus::ALL`].
fn bus_index(bus: AudioBus) -> usize {
    AudioBus::ALL
        .iter()
        .position(|&each| each == bus)
        .expect("every bus is in AudioBus::ALL")
}

/// The bus a settings file names, if any.
fn bus_named(name: &str) -> Option<AudioBus> {
    AudioBus::ALL.iter().copied().find(|bus| bus.name() == name)
}

/// A frame-rate limit the player chose (`D-180`). Unset, the shipped
/// `display.frame_rate_cap` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRateCap {
    /// No limit, over a shipped one.
    Uncapped,
    /// At most this many frames a second.
    Fps(NonZeroU32),
}

impl FrameRateCap {
    /// The settings file's word for [`Uncapped`](Self::Uncapped).
    const UNCAPPED: &'static str = "uncapped";

    /// The cap a settings file holds: `"uncapped"` or an integer >= 1.
    fn from_file(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::String(word) if word == Self::UNCAPPED => Some(Self::Uncapped),
            serde_json::Value::Number(number) => number
                .as_u64()
                .and_then(|fps| u32::try_from(fps).ok())
                .and_then(NonZeroU32::new)
                .map(Self::Fps),
            _ => None,
        }
    }

    fn to_file(self) -> serde_json::Value {
        match self {
            Self::Uncapped => Self::UNCAPPED.into(),
            Self::Fps(fps) => fps.get().into(),
        }
    }
}

/// Where the game's user folder is (`D-119`), set in code; never read from
/// `tungsten.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum UserFolder {
    /// `TUNGSTEN_USER_DIR` when set, else the platform's folders under `game.id`.
    #[default]
    Platform,
    /// `<dir>/settings`, `<dir>/saves` and `<dir>/logs`, as `TUNGSTEN_USER_DIR` gives.
    At(PathBuf),
    /// No folder: no log file, settings or saves.
    Off,
}

/// The settings the engine persists in the user folder: a `World` resource.
/// `None` means the player has not set it, and the shipped default applies.
///
/// A game changes a value, calls `tungsten::apply_settings` to apply it, then
/// [`save`](Self::save)s; the file holds only the values the player set:
///
/// ```json
/// {
///   "display": { "display_mode": "borderless_fullscreen", "resolution": { "width": 1920, "height": 1080 }, "vsync": true, "frame_rate_cap": 60 },
///   "ui_scale": 1.25,
///   "text_scale": 1.5,
///   "master_volume": 0.8,
///   "bus_volumes": { "music": 0.5 }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    display_mode: Option<DisplayMode>,
    resolution: Option<Resolution>,
    vsync: Option<bool>,
    frame_rate_cap: Option<FrameRateCap>,
    ui_scale: Option<f32>,
    text_scale: Option<f32>,
    master_volume: Option<f32>,
    /// By [`AudioBus::ALL`]'s order.
    bus_volumes: [Option<f32>; BUSES],
    path: Option<PathBuf>,
    load_warnings: Vec<String>,
}

/// A failed [`Settings::save`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SettingsError {
    /// Smoke mode, `UserFolder::Off`, or no `game.id` and no override.
    #[error("the game has no user folder, so settings are not saved")]
    NoFolder,
    /// The write failed; the file on disk is unchanged.
    #[error(transparent)]
    Write(#[from] WriteError),
}

impl Settings {
    /// The file's name in the user folder's `settings` folder.
    pub const FILE: &'static str = "settings.json";
    /// The UI scales a player can set.
    pub const UI_SCALE_RANGE: RangeInclusive<f32> = 0.5..=3.0;
    /// The text scales a player can set (M56, Q13); the UI tree itself
    /// takes 0.25..=4.0.
    pub const TEXT_SCALE_RANGE: RangeInclusive<f32> = 0.75..=2.0;

    /// Reads `path`. A missing file gives defaults; an unreadable, corrupt or
    /// invalid one gives defaults and is renamed to `settings.json.bad`.
    /// Never fails: warnings wait in [`take_load_warnings`](Self::take_load_warnings),
    /// as `Config::load`'s do.
    ///
    /// Invalid means not UTF-8 JSON, not an object, a known key of the wrong
    /// type, or a value no setter would keep. Unknown keys are ignored, so an
    /// older build reads a newer file, and so are unknown buses in
    /// `bus_volumes`, whatever their values. A file that cannot be read for
    /// another reason than its absence (permission denied) is left where it
    /// is.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut settings = Self {
            path: Some(path.clone()),
            ..Self::default()
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return settings,
            Err(error) => {
                settings.load_warnings.push(format!(
                    "failed to read settings '{}': {error}; using defaults",
                    path.display()
                ));
                return settings;
            }
        };
        match parse(&bytes) {
            Ok(file) => settings.apply_file(file),
            Err(reason) => {
                let warning = match quarantine(&path) {
                    Ok(backup) => format!(
                        "settings '{}' are invalid ({reason}); using defaults, the file kept as '{}'",
                        path.display(),
                        backup.display()
                    ),
                    Err(error) => format!(
                        "settings '{}' are invalid ({reason}); using defaults, and the file could not be moved aside: {error}",
                        path.display()
                    ),
                };
                settings.load_warnings.push(warning);
            }
        }
        settings
    }

    /// Takes the warnings [`load`](Self::load) kept, for the caller to log;
    /// `App::new` logs them. Later calls return none.
    pub fn take_load_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.load_warnings)
    }

    /// The file [`save`](Self::save) writes; `None` without a user folder.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The display mode the player set.
    #[must_use]
    pub fn display_mode(&self) -> Option<DisplayMode> {
        self.display_mode
    }

    /// Sets or, with `None`, unsets the display mode.
    pub fn set_display_mode(&mut self, mode: Option<DisplayMode>) {
        self.display_mode = mode;
    }

    /// The render resolution the player set.
    #[must_use]
    pub fn resolution(&self) -> Option<Resolution> {
        self.resolution
    }

    /// Sets or unsets the resolution; one with a zero side is ignored.
    pub fn set_resolution(&mut self, resolution: Option<Resolution>) {
        match resolution {
            Some(resolution) if resolution.width == 0 || resolution.height == 0 => {}
            other => self.resolution = other,
        }
    }

    /// The vsync the player set. Set, it clears a shipped present mode, so
    /// `auto` follows it (`D-159`).
    #[must_use]
    pub fn vsync(&self) -> Option<bool> {
        self.vsync
    }

    /// Sets or unsets vsync.
    pub fn set_vsync(&mut self, vsync: Option<bool>) {
        self.vsync = vsync;
    }

    /// The frame-rate limit the player set (`D-180`), which replaces the
    /// shipped `display.frame_rate_cap`; [`FrameRateCap::Uncapped`] lifts it.
    #[must_use]
    pub fn frame_rate_cap(&self) -> Option<FrameRateCap> {
        self.frame_rate_cap
    }

    /// Sets or, with `None`, unsets the frame-rate limit, so the shipped one
    /// applies again.
    pub fn set_frame_rate_cap(&mut self, cap: Option<FrameRateCap>) {
        self.frame_rate_cap = cap;
    }

    /// The UI scale the player set, which `ScaleFactor::get` multiplies the
    /// OS scale by.
    #[must_use]
    pub fn ui_scale(&self) -> Option<f32> {
        self.ui_scale
    }

    /// Sets or unsets the UI scale, clamped to [`UI_SCALE_RANGE`](Self::UI_SCALE_RANGE);
    /// a value that is not finite is ignored.
    pub fn set_ui_scale(&mut self, scale: Option<f32>) {
        match scale {
            Some(scale) if !scale.is_finite() => {}
            Some(scale) => {
                let range = Self::UI_SCALE_RANGE;
                self.ui_scale = Some(scale.clamp(*range.start(), *range.end()));
            }
            None => self.ui_scale = None,
        }
    }

    /// The text scale the player set (M56), which the UI tree multiplies
    /// every label's text size and line height by.
    #[must_use]
    pub fn text_scale(&self) -> Option<f32> {
        self.text_scale
    }

    /// Sets or unsets the text scale, clamped to
    /// [`TEXT_SCALE_RANGE`](Self::TEXT_SCALE_RANGE); a value that is not
    /// finite is ignored.
    pub fn set_text_scale(&mut self, scale: Option<f32>) {
        match scale {
            Some(scale) if !scale.is_finite() => {}
            Some(scale) => {
                let range = Self::TEXT_SCALE_RANGE;
                self.text_scale = Some(scale.clamp(*range.start(), *range.end()));
            }
            None => self.text_scale = None,
        }
    }

    /// The master volume the player set, 0 to 1.
    #[must_use]
    pub fn master_volume(&self) -> Option<f32> {
        self.master_volume
    }

    /// Sets or unsets the master volume, clamped to 0..=1; a value that is
    /// not finite is ignored.
    pub fn set_master_volume(&mut self, volume: Option<f32>) {
        match volume {
            Some(volume) if !volume.is_finite() => {}
            Some(volume) => self.master_volume = Some(volume.clamp(0.0, 1.0)),
            None => self.master_volume = None,
        }
    }

    /// The volume the player set for `bus`, 0 to 1.
    #[must_use]
    pub fn bus_volume(&self, bus: AudioBus) -> Option<f32> {
        self.bus_volumes[bus_index(bus)]
    }

    /// Sets or unsets `bus`'s volume, clamped to 0..=1; a value that is not
    /// finite is ignored. The file holds only the buses set.
    pub fn set_bus_volume(&mut self, bus: AudioBus, volume: Option<f32>) {
        let slot = &mut self.bus_volumes[bus_index(bus)];
        match volume {
            Some(volume) if !volume.is_finite() => {}
            Some(volume) => *slot = Some(volume.clamp(0.0, 1.0)),
            None => *slot = None,
        }
    }

    /// Writes the set values through [`write_atomic`].
    ///
    /// # Errors
    ///
    /// [`SettingsError::NoFolder`] without a user folder, or the write's
    /// error; the file on disk is then unchanged.
    pub fn save(&self) -> Result<(), SettingsError> {
        let path = self.path.as_deref().ok_or(SettingsError::NoFolder)?;
        let display = DisplayFile {
            display_mode: self.display_mode.map(|mode| mode.as_str().to_string()),
            resolution: self.resolution.map(|r| ResolutionFile {
                width: r.width,
                height: r.height,
            }),
            vsync: self.vsync,
            frame_rate_cap: self.frame_rate_cap.map(FrameRateCap::to_file),
        };
        let buses: BTreeMap<String, f32> = AudioBus::ALL
            .iter()
            .filter_map(|&bus| Some((bus.name().to_string(), self.bus_volume(bus)?)))
            .collect();
        let file = SettingsFile {
            display: (display != DisplayFile::default()).then_some(display),
            ui_scale: self.ui_scale,
            text_scale: self.text_scale,
            master_volume: self.master_volume,
            bus_volumes: (!buses.is_empty()).then_some(buses),
        };
        let mut json = serde_json::to_string_pretty(&file)
            .expect("settings serialize: plain fields with string keys");
        json.push('\n');
        write_atomic(path, json.as_bytes())?;
        Ok(())
    }

    fn apply_file(&mut self, file: SettingsFile) {
        if let Some(display) = file.display {
            self.display_mode = display
                .display_mode
                .as_deref()
                .and_then(DisplayMode::from_str_name);
            self.resolution = display.resolution.map(|r| Resolution {
                width: r.width,
                height: r.height,
            });
            self.vsync = display.vsync;
            self.frame_rate_cap = display
                .frame_rate_cap
                .as_ref()
                .and_then(FrameRateCap::from_file);
        }
        self.ui_scale = file.ui_scale;
        self.text_scale = file.text_scale;
        self.master_volume = file.master_volume;
        for (name, volume) in file.bus_volumes.unwrap_or_default() {
            if let Some(bus) = bus_named(&name) {
                self.bus_volumes[bus_index(bus)] = Some(volume);
            }
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display: Option<DisplayFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ui_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    master_volume: Option<f32>,
    /// The known buses' volumes by name.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "bus_volumes_file"
    )]
    bus_volumes: Option<BTreeMap<String, f32>>,
}

/// `bus_volumes`: an object whose known buses each hold a number in 0..=1;
/// an unknown bus is ignored, whatever its value.
fn bus_volumes_file<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, f32>>, D::Error> {
    let Some(entries) = Option::<BTreeMap<String, serde_json::Value>>::deserialize(deserializer)?
    else {
        return Ok(None);
    };
    let mut volumes = BTreeMap::new();
    for (name, value) in entries {
        if bus_named(&name).is_none() {
            continue;
        }
        let volume = value
            .as_f64()
            .filter(|volume| (0.0..=1.0).contains(volume))
            .ok_or_else(|| {
                serde::de::Error::custom(format!("bus_volumes.{name} {value} is outside 0..=1"))
            })?;
        volumes.insert(name, volume as f32);
    }
    Ok(Some(volumes))
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct DisplayFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resolution: Option<ResolutionFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vsync: Option<bool>,
    /// `"uncapped"` or an integer >= 1, checked by [`parse`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame_rate_cap: Option<serde_json::Value>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ResolutionFile {
    width: u32,
    height: u32,
}

/// The file, or why the whole of it falls back.
fn parse(bytes: &[u8]) -> Result<SettingsFile, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("not UTF-8: {error}"))?;
    let json = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: serde_json::Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    // A derived struct also reads a JSON array; the file's objects must be
    // objects.
    let display = value.get("display");
    let resolution = display.and_then(|display| display.get("resolution"));
    if !value.is_object()
        || display.is_some_and(|v| !v.is_object() && !v.is_null())
        || resolution.is_some_and(|v| !v.is_object() && !v.is_null())
    {
        return Err("expected an object of objects".into());
    }
    let file: SettingsFile = serde_json::from_value(value).map_err(|error| error.to_string())?;
    if let Some(display) = &file.display {
        if let Some(mode) = &display.display_mode
            && DisplayMode::from_str_name(mode).is_none()
        {
            return Err(format!(
                "display.display_mode '{mode}' is not windowed, borderless_fullscreen or exclusive_fullscreen"
            ));
        }
        if let Some(r) = &display.resolution
            && (r.width == 0 || r.height == 0)
        {
            return Err(format!(
                "display.resolution {}x{} has a zero side",
                r.width, r.height
            ));
        }
        if let Some(cap) = &display.frame_rate_cap
            && FrameRateCap::from_file(cap).is_none()
        {
            return Err(format!(
                "display.frame_rate_cap {cap} is not \"uncapped\" or an integer >= 1"
            ));
        }
    }
    if let Some(scale) = file.ui_scale
        && !(scale.is_finite() && Settings::UI_SCALE_RANGE.contains(&scale))
    {
        return Err(format!("ui_scale {scale} is outside 0.5..=3"));
    }
    if let Some(scale) = file.text_scale
        && !(scale.is_finite() && Settings::TEXT_SCALE_RANGE.contains(&scale))
    {
        return Err(format!("text_scale {scale} is outside 0.75..=2"));
    }
    if let Some(volume) = file.master_volume
        && !(volume.is_finite() && (0.0..=1.0).contains(&volume))
    {
        return Err(format!("master_volume {volume} is outside 0..=1"));
    }
    Ok(file)
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
