use crate::assets::ParticleBudget;
use crate::audio::AudioBus;
use crate::display::{DisplayConfig, DisplayMode, Resolution};
use crate::physics::PhysicsConfig;
use crate::settings::{FrameRateCap, Settings, UserFolder};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;

const DISPLAY_MODE_ENV: &str = "TUNGSTEN_DISPLAY_MODE";
const DISPLAY_RESOLUTION_ENV: &str = "TUNGSTEN_DISPLAY_RESOLUTION";
const DISPLAY_FRAME_RATE_CAP_ENV: &str = "TUNGSTEN_DISPLAY_FRAME_RATE_CAP";
const DISPLAY_PRESENT_MODE_ENV: &str = "TUNGSTEN_DISPLAY_PRESENT_MODE";
const DISPLAY_MAX_FRAME_LATENCY_ENV: &str = "TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY";
const RENDER_PRESENT_MODE_ENV: &str = "TUNGSTEN_RENDER_PRESENT_MODE";
const RENDER_MAX_FRAME_LATENCY_ENV: &str = "TUNGSTEN_RENDER_MAX_FRAME_LATENCY";
const RENDER_MSAA_ENV: &str = "TUNGSTEN_RENDER_MSAA";
const RENDER_DEPTH_ENABLED_ENV: &str = "TUNGSTEN_RENDER_DEPTH_ENABLED";
const RENDER_DEPTH_SORT_ENV: &str = "TUNGSTEN_RENDER_DEPTH_SORT";
const RENDER_POST_AA_ENV: &str = "TUNGSTEN_RENDER_POST_AA";
const RENDER_BLOOM_MAX_MIPS_ENV: &str = "TUNGSTEN_RENDER_BLOOM_MAX_MIPS";
const DISPLAY_MODE_EXPECTED: &str = "one of: windowed, borderless_fullscreen, exclusive_fullscreen";
const DISPLAY_RESOLUTION_EXPECTED: &str = "WIDTHxHEIGHT with integers >= 1";
const DISPLAY_FRAME_RATE_CAP_EXPECTED: &str = "an integer >= 0";
const PRESENT_MODE_EXPECTED: &str =
    "one of: auto, immediate, mailbox, fifo, auto_vsync, auto_no_vsync";
const MAX_FRAME_LATENCY_EXPECTED: &str = "an integer >= 1";
const MSAA_EXPECTED: &str = "one of: 1, 2, 4, 8";
const DEPTH_ENABLED_EXPECTED: &str = "one of: true, false, 1, 0";
const DEPTH_SORT_EXPECTED: &str = "one of: cpu_stable, gpu_depth";
const POST_AA_EXPECTED: &str = "one of: off, smaa_low, smaa_medium, smaa_high, smaa_ultra";
const BLOOM_MAX_MIPS_EXPECTED: &str = "an integer in 1..=8";
const GAME_ID_EXPECTED: &str = "1-64 ASCII letters, digits, '.', '_' or '-', starting with a letter or digit, not ending in '.', and not a Windows device name";
const LOGGING_LEVEL_EXPECTED: &str = "one of: off, error, warn, info, debug, trace";
const FIXED_STEP_HZ_EXPECTED: &str = "an integer >= 30";
const MAX_STEPS_PER_FRAME_EXPECTED: &str = "an integer >= 1";
const VOLUME_EXPECTED: &str = "a number from 0 to 1";
const GRAVITY_EXPECTED: &str = "two finite numbers, px/s² with y down";
const BROADPHASE_CELL_EXPECTED: &str = "a finite number of pixels above 0";
const GAME_ID_MAX_LEN: usize = 64;
/// The lowest step rate: a longer step than physics' default
/// `max_step_dt`, 1/30 s, would run physics slow against the game (`D-094`).
const MIN_FIXED_STEP_HZ: u32 = 30;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid config in '{path}': {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("invalid env override {var}='{value}': expected {expected}")]
    InvalidEnvOverride {
        var: &'static str,
        value: String,
        expected: &'static str,
    },
    /// A value the engine does not support, read from the config file or,
    /// with an empty `path`, set in code.
    #[error("invalid {field}='{value}'{}: expected {expected}", in_file(.path))]
    InvalidValue {
        path: String,
        field: &'static str,
        value: String,
        expected: &'static str,
    },
}

impl ConfigError {
    /// Names the file a value came from, for a check that does not know it.
    fn with_path(self, file: &Path) -> Self {
        match self {
            Self::InvalidValue {
                field,
                value,
                expected,
                ..
            } => Self::InvalidValue {
                path: file.display().to_string(),
                field,
                value,
                expected,
            },
            other => other,
        }
    }
}

fn in_file(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else {
        format!(" in '{path}'")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_vsync")]
    pub vsync: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: default_title(),
            width: default_width(),
            height: default_height(),
            vsync: default_vsync(),
        }
    }
}

fn default_title() -> String {
    "Tungsten".to_string()
}
fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_vsync() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentModeConfig {
    Auto,
    Immediate,
    Mailbox,
    Fifo,
    AutoVsync,
    AutoNoVsync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("expected {PRESENT_MODE_EXPECTED}")]
pub struct ParsePresentModeConfigError;

impl PresentModeConfig {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Immediate => "immediate",
            Self::Mailbox => "mailbox",
            Self::Fifo => "fifo",
            Self::AutoVsync => "auto_vsync",
            Self::AutoNoVsync => "auto_no_vsync",
        }
    }
}

impl FromStr for PresentModeConfig {
    type Err = ParsePresentModeConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "immediate" => Ok(Self::Immediate),
            "mailbox" => Ok(Self::Mailbox),
            "fifo" => Ok(Self::Fifo),
            "auto_vsync" => Ok(Self::AutoVsync),
            "auto_no_vsync" => Ok(Self::AutoNoVsync),
            _ => Err(ParsePresentModeConfigError),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepthSortMode {
    #[default]
    CpuStable,
    GpuDepth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("expected {DEPTH_SORT_EXPECTED}")]
pub struct ParseDepthSortModeError;

impl DepthSortMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpuStable => "cpu_stable",
            Self::GpuDepth => "gpu_depth",
        }
    }
}

impl FromStr for DepthSortMode {
    type Err = ParseDepthSortModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cpu_stable" => Ok(Self::CpuStable),
            "gpu_depth" => Ok(Self::GpuDepth),
            _ => Err(ParseDepthSortModeError),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PostAaMode {
    #[default]
    Off,
    SmaaLow,
    SmaaMedium,
    SmaaHigh,
    SmaaUltra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("expected {POST_AA_EXPECTED}")]
pub struct ParsePostAaModeError;

impl PostAaMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::SmaaLow => "smaa_low",
            Self::SmaaMedium => "smaa_medium",
            Self::SmaaHigh => "smaa_high",
            Self::SmaaUltra => "smaa_ultra",
        }
    }

    #[must_use]
    pub const fn is_smaa(self) -> bool {
        matches!(
            self,
            Self::SmaaLow | Self::SmaaMedium | Self::SmaaHigh | Self::SmaaUltra
        )
    }
}

impl FromStr for PostAaMode {
    type Err = ParsePostAaModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "off" => Ok(Self::Off),
            "smaa_low" => Ok(Self::SmaaLow),
            "smaa_medium" => Ok(Self::SmaaMedium),
            "smaa_high" => Ok(Self::SmaaHigh),
            "smaa_ultra" => Ok(Self::SmaaUltra),
            _ => Err(ParsePostAaModeError),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenderConfig {
    /// The colour the scene clears to, RGBA from 0 to 1 in linear light,
    /// which the frame encodes to sRGB: the default,
    /// `[0.05, 0.05, 0.08, 1.0]`, clears to `[63, 63, 80]`.
    #[serde(default = "default_clear_color")]
    pub clear_color: [f64; 4],
    pub max_frame_latency: Option<u32>,
    pub present_mode: Option<PresentModeConfig>,
    #[serde(default = "default_msaa")]
    pub msaa: u32,
    #[serde(default = "default_depth_enabled")]
    pub depth_enabled: bool,
    #[serde(default)]
    pub depth_sort: DepthSortMode,
    #[serde(default)]
    pub post_aa: PostAaMode,
    #[serde(default = "default_bloom_max_mips")]
    pub bloom_max_mips: u32,
    /// Let text fall back to the system's fonts after the manifest's
    /// (`D-116`). Off by default: text draws only with packaged faces.
    #[serde(default)]
    pub system_fonts: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            clear_color: default_clear_color(),
            max_frame_latency: None,
            present_mode: None,
            msaa: default_msaa(),
            depth_enabled: default_depth_enabled(),
            depth_sort: DepthSortMode::default(),
            post_aa: PostAaMode::default(),
            bloom_max_mips: default_bloom_max_mips(),
            system_fonts: false,
        }
    }
}

fn default_clear_color() -> [f64; 4] {
    [0.05, 0.05, 0.08, 1.0]
}

fn default_msaa() -> u32 {
    1
}

fn default_depth_enabled() -> bool {
    true
}

fn default_bloom_max_mips() -> u32 {
    6
}

#[must_use]
pub const fn is_supported_msaa(sample_count: u32) -> bool {
    matches!(sample_count, 1 | 2 | 4 | 8)
}

#[must_use]
pub const fn is_supported_bloom_max_mips(n: u32) -> bool {
    n >= 1 && n <= 8
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_level")]
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_level(),
        }
    }
}

fn default_level() -> String {
    "info".to_string()
}

/// The game's identity (`D-119`).
#[derive(Debug, Clone, Default, Deserialize)]
#[non_exhaustive]
pub struct GameConfig {
    /// Names the game's per-user folder, which holds its logs and crash
    /// files: 1–64 ASCII letters, digits, `.`, `_` or `-`, starting with a
    /// letter or digit, and not a Windows device name such as `con`. Without
    /// one the game has no user folder.
    pub id: Option<String>,
    /// Free text that crash files print.
    pub version: Option<String>,
    /// Where the user folder is, set in code (`D-159`): the platform's by
    /// default, a fixed folder, or none, as a test that must not read the
    /// developer's own settings sets it.
    #[serde(skip)]
    pub user_folder: UserFolder,
}

impl GameConfig {
    /// Checks `id`, which becomes a folder name. `Config::load` and
    /// `App::new` both call this, since a config built in code skips the
    /// load.
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidValue`] for field `game.id`, with an empty
    /// `path`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match &self.id {
            Some(id) if !is_valid_game_id(id) => Err(ConfigError::InvalidValue {
                path: String::new(),
                field: "game.id",
                value: id.clone(),
                expected: GAME_ID_EXPECTED,
            }),
            _ => Ok(()),
        }
    }
}

fn is_valid_game_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() <= GAME_ID_MAX_LEN
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        // Windows drops a trailing dot, so `mygame.` would share `mygame`'s
        // folder (`D-219`).
        && bytes.last() != Some(&b'.')
        && !is_windows_device_name(id)
}

/// Windows reserves these names in any case and with any extension, so
/// `con.txt` names the console too.
fn is_windows_device_name(id: &str) -> bool {
    let stem = id.split('.').next().unwrap_or(id).to_ascii_lowercase();
    match stem.as_bytes() {
        b"con" | b"prn" | b"aux" | b"nul" => true,
        [b'c', b'o', b'm', digit] | [b'l', b'p', b't', digit] => matches!(digit, b'1'..=b'9'),
        _ => false,
    }
}

/// The fixed step: `tungsten.json`'s `time` section, which `App::new`
/// applies to [`Time`](crate::time::Time).
///
/// ```json
/// "time": { "fixed_step_hz": 60, "max_steps_per_frame": 2, "interpolate": true }
/// ```
///
/// Every field has its default, so a file without the section, or with an
/// empty one, steps at 60 Hz, two steps a frame at most, interpolating.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct TimeConfig {
    /// Fixed steps per game second: 60 unless set, the smoke pin. At least
    /// 30, since physics clamps a step to 1/30 s by default.
    pub fixed_step_hz: u32,
    /// The most fixed steps a frame runs: 2 unless set, at least 1.
    pub max_steps_per_frame: u32,
    /// Whether bodies with a previous position are drawn interpolated: on
    /// unless set.
    pub interpolate: bool,
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            fixed_step_hz: 60,
            max_steps_per_frame: 2,
            interpolate: true,
        }
    }
}

impl TimeConfig {
    /// Checks the step rate and the bound, which `Time`'s setters would
    /// refuse. `Config::load` and `App::new` both call this, since a config
    /// built in code skips the load.
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidValue`] with an empty `path` for field
    /// `time.fixed_step_hz` below 30 or `time.max_steps_per_frame` of 0.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.fixed_step_hz < MIN_FIXED_STEP_HZ {
            return Err(ConfigError::InvalidValue {
                path: String::new(),
                field: "time.fixed_step_hz",
                value: self.fixed_step_hz.to_string(),
                expected: FIXED_STEP_HZ_EXPECTED,
            });
        }
        if self.max_steps_per_frame == 0 {
            return Err(ConfigError::InvalidValue {
                path: String::new(),
                field: "time.max_steps_per_frame",
                value: self.max_steps_per_frame.to_string(),
                expected: MAX_STEPS_PER_FRAME_EXPECTED,
            });
        }
        Ok(())
    }
}

/// The `audio` section of `tungsten.json` (`D-184`): the volumes a player
/// who set none hears, each 0 to 1, which an unset setting returns to.
///
/// ```json
/// "audio": { "master_volume": 0.8, "bus_volumes": { "music": 0.8, "effects": 0.8 } }
/// ```
///
/// A file without the section, or a bus left out, plays at 1. An unknown
/// key or bus name fails the parse.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct AudioConfig {
    /// `audio.master_volume`: 1 unless set.
    pub master_volume: f32,
    /// `audio.bus_volumes`, by bus name; a bus left out is 1.
    pub bus_volumes: HashMap<AudioBus, f32>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            bus_volumes: HashMap::new(),
        }
    }
}

impl AudioConfig {
    /// The volume `bus` ships at: its entry, else 1.
    #[must_use]
    pub fn bus_volume(&self, bus: AudioBus) -> f32 {
        self.bus_volumes.get(&bus).copied().unwrap_or(1.0)
    }

    /// Checks every volume is finite and in 0..=1. `Config::load` and
    /// `App::new` both call this, since a config built in code skips the
    /// load.
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidValue`] with an empty `path` for field
    /// `audio.master_volume` or `audio.bus_volumes`, the latter's value
    /// naming the bus.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let valid = |volume: f32| volume.is_finite() && (0.0..=1.0).contains(&volume);
        if !valid(self.master_volume) {
            return Err(ConfigError::InvalidValue {
                path: String::new(),
                field: "audio.master_volume",
                value: self.master_volume.to_string(),
                expected: VOLUME_EXPECTED,
            });
        }
        for &bus in AudioBus::ALL {
            if let Some(&volume) = self.bus_volumes.get(&bus)
                && !valid(volume)
            {
                return Err(ConfigError::InvalidValue {
                    path: String::new(),
                    field: "audio.bus_volumes",
                    value: format!("{}: {volume}", bus.name()),
                    expected: VOLUME_EXPECTED,
                });
            }
        }
        Ok(())
    }
}

/// The `assets` section of `tungsten.json` (redo 7.9): the root manifest
/// the app loads, and whether edits to it and under its folder apply while
/// the game runs.
///
/// ```json
/// "assets": { "manifest": "assets/manifest.json", "hot_reload": true }
/// ```
///
/// A file without the section names no manifest, so the app loads none
/// unless the game sets its roots in code. An unknown key fails the parse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct AssetsConfig {
    /// `assets.manifest`: the root manifest, relative to the game's folder;
    /// none unless set.
    pub manifest: Option<PathBuf>,
    /// `assets.hot_reload`: whether the app watches the manifest and its
    /// folder; unset, on in debug builds.
    pub hot_reload: Option<bool>,
}

impl AssetsConfig {
    /// Whether the app watches the manifest: `hot_reload` when set, else
    /// `debug_build`.
    #[must_use]
    pub fn hot_reload_or(&self, debug_build: bool) -> bool {
        self.hot_reload.unwrap_or(debug_build)
    }
}

/// A corner of the window: where the debug HUD sits (redo 7.17).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCorner {
    /// The top left.
    TopLeft,
    /// The top right.
    TopRight,
    /// The bottom left.
    BottomLeft,
    /// The bottom right.
    BottomRight,
}

/// The pointer sprite `tungsten.json` names (redo 7.17): a sprite's
/// manifest ID, its size and its hotspot, in UI units.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiCursorConfig {
    /// The sprite's manifest ID.
    pub sprite: String,
    /// Its size.
    pub size: [f32; 2],
    /// The point of it on the pointer, from its top-left; the corner when
    /// left out.
    #[serde(default)]
    pub hotspot: [f32; 2],
}

/// `tungsten.json`'s `ui` (redo 7.17): the UI's theme, scale and floor.
/// Each field is optional, and an absent one, or the whole section, keeps
/// the engine's default.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    /// A `ui_themes` ID the app applies once it has loaded; a reload
    /// re-applies it as it does any theme asset (`D-148`).
    pub theme: Option<String>,
    /// The window height, in physical pixels, that lays the UI out at
    /// scale 1 (`D-184`).
    pub reference_height: Option<f32>,
    /// The smallest viewport the UI lays out in, in UI units, whatever the
    /// scales.
    pub min_viewport: Option<[f32; 2]>,
    /// The UI scale an unset `Settings::ui_scale` applies as.
    pub scale: Option<f32>,
    /// The corner the debug HUD sits in.
    pub hud_corner: Option<UiCorner>,
    /// The pointer sprite the app gives the tree at startup, which the game
    /// shows.
    pub cursor: Option<UiCursorConfig>,
}

/// `tungsten.json`'s `physics` (redo 7.12, S-85): the World's gravity and
/// its broadphase's cell, which `App::new` puts into the World's
/// `PhysicsConfig`; a game's plugin may set them again after.
///
/// ```json
/// "physics": { "gravity": [0, 900], "broadphase_cell": 32 }
/// ```
///
/// A file without the section, or a field left out, keeps the engine's
/// default. An unknown key fails the parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhysicsSection {
    /// `physics.gravity`: every dynamic body's acceleration in px/s², y
    /// down; `[0, 0]` unless set.
    pub gravity: [f32; 2],
    /// `physics.broadphase_cell`: the broadphase grid's cell in pixels; 32
    /// unless set, finite and above 0.
    pub broadphase_cell: f32,
}

impl Default for PhysicsSection {
    fn default() -> Self {
        let physics = PhysicsConfig::default();
        Self {
            gravity: physics.gravity.to_array(),
            broadphase_cell: physics.broadphase_cell_size,
        }
    }
}

impl PhysicsSection {
    /// Checks the gravity is finite and the cell finite and above 0.
    /// `Config::load` and `App::new` both call this, since a config built
    /// in code skips the load.
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidValue`] with an empty `path` for field
    /// `physics.gravity` or `physics.broadphase_cell`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let [x, y] = self.gravity;
        if !(x.is_finite() && y.is_finite()) {
            return Err(ConfigError::InvalidValue {
                path: String::new(),
                field: "physics.gravity",
                value: format!("[{x}, {y}]"),
                expected: GRAVITY_EXPECTED,
            });
        }
        let cell = self.broadphase_cell;
        if !(cell.is_finite() && cell > 0.0) {
            return Err(ConfigError::InvalidValue {
                path: String::new(),
                field: "physics.broadphase_cell",
                value: cell.to_string(),
                expected: BROADPHASE_CELL_EXPECTED,
            });
        }
        Ok(())
    }
}

/// `tungsten.json`'s `particles` (redo 7.12, S-85): the cap on live
/// particles across every emitter, which `App::new` puts into the World's
/// `ParticleBudget` (`D-014`); a game's plugin may set it again after.
///
/// ```json
/// "particles": { "budget": 4000 }
/// ```
///
/// A file without the section keeps the engine's 10,000. An unknown key
/// fails the parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParticlesSection {
    /// `particles.budget`: the most particles alive at once; 10,000 unless
    /// set.
    pub budget: u32,
}

impl Default for ParticlesSection {
    fn default() -> Self {
        Self {
            budget: ParticleBudget::DEFAULT_GLOBAL_CAP,
        }
    }
}

/// Top-level engine configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    /// The game's identity and user folder (`D-119`).
    #[serde(default)]
    pub game: GameConfig,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    /// The fixed step and interpolation.
    #[serde(default)]
    pub time: TimeConfig,
    /// The volumes the game ships (`D-184`).
    #[serde(default)]
    pub audio: AudioConfig,
    /// The root manifest and its hot reload (redo 7.9).
    #[serde(default)]
    pub assets: AssetsConfig,
    /// The UI's theme, scale and floor (redo 7.17).
    #[serde(default)]
    pub ui: UiConfig,
    /// The World's gravity and broadphase cell (redo 7.12).
    #[serde(default)]
    pub physics: PhysicsSection,
    /// The cap on live particles (redo 7.12).
    #[serde(default)]
    pub particles: ParticlesSection,
    /// What `load` would have logged before any logger exists (`D-119`).
    #[serde(skip)]
    load_warnings: Vec<String>,
    /// Set by `load`, so `apply_settings` applies the environment overrides
    /// again only to a config they applied to.
    #[serde(skip)]
    env_overrides: bool,
}

impl Config {
    /// Load config; missing file falls back to defaults.
    ///
    /// Warnings (a missing file, legacy fields that the display section
    /// overrides, display values and a zero `render.max_frame_latency` that
    /// fall back) are kept, not logged:
    /// [`Config::take_load_warnings`] returns them.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let mut config = match std::fs::read_to_string(path) {
            Ok(contents) => {
                // An editor's byte-order mark is not JSON.
                let json = contents.strip_prefix('\u{feff}').unwrap_or(&contents);
                let mut raw: Value =
                    serde_json::from_str(json).map_err(|e| ConfigError::Parse {
                        path: path.display().to_string(),
                        source: e,
                    })?;
                let mut warnings = Vec::new();
                display_conflicts(&raw, &mut warnings);
                // Parsed apart, so its fallbacks are kept rather than logged.
                let display = raw.as_object_mut().and_then(|map| map.remove("display"));
                let mut parsed: Config =
                    serde_json::from_value(raw).map_err(|e| ConfigError::Parse {
                        path: path.display().to_string(),
                        source: e,
                    })?;
                if let Some(display) = display {
                    parsed.display = DisplayConfig::from_json_value(display, &mut warnings);
                }
                // As `display.max_frame_latency` does: the surface refuses 0.
                if parsed.render.max_frame_latency == Some(0) {
                    warnings.push(
                        "Config render.max_frame_latency=0 is invalid; expected an integer >= 1; \
                         falling back"
                            .into(),
                    );
                    parsed.render.max_frame_latency = None;
                }
                parsed.load_warnings = warnings;
                if !is_supported_msaa(parsed.render.msaa) {
                    return Err(ConfigError::InvalidValue {
                        path: path.display().to_string(),
                        field: "render.msaa",
                        value: parsed.render.msaa.to_string(),
                        expected: MSAA_EXPECTED,
                    });
                }
                if !is_supported_bloom_max_mips(parsed.render.bloom_max_mips) {
                    return Err(ConfigError::InvalidValue {
                        path: path.display().to_string(),
                        field: "render.bloom_max_mips",
                        value: parsed.render.bloom_max_mips.to_string(),
                        expected: BLOOM_MAX_MIPS_EXPECTED,
                    });
                }
                parsed.game.validate().map_err(|e| e.with_path(path))?;
                parsed.time.validate().map_err(|e| e.with_path(path))?;
                parsed.audio.validate().map_err(|e| e.with_path(path))?;
                parsed.physics.validate().map_err(|e| e.with_path(path))?;
                if parsed.logging.level.parse::<log::LevelFilter>().is_err() {
                    return Err(ConfigError::InvalidValue {
                        path: path.display().to_string(),
                        field: "logging.level",
                        value: parsed.logging.level.clone(),
                        expected: LOGGING_LEVEL_EXPECTED,
                    });
                }
                parsed
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut config = Config::default();
                config.load_warnings.push(format!(
                    "Config file '{}' not found, using defaults",
                    path.display()
                ));
                config
            }
            Err(e) => Err(ConfigError::Io {
                path: path.display().to_string(),
                source: e,
            })?,
        };

        config.env_overrides = true;
        config.apply_env_overrides_from_env()?;
        Ok(config)
    }

    /// Lays the player's [`Settings`] over this config (`D-159`): each set
    /// display value replaces the file's, a set vsync also clears both
    /// present modes, so `auto` follows it, and [`FrameRateCap::Uncapped`]
    /// clears the file's frame-rate cap (`D-180`). A config [`load`](Self::load)
    /// built then takes its environment overrides again, so they stay last;
    /// one built in code stays free of them.
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidEnvOverride`], as `load` returns it.
    pub fn apply_settings(&mut self, settings: &Settings) -> Result<(), ConfigError> {
        self.apply_settings_with(settings, env_var)
    }

    fn apply_settings_with(
        &mut self,
        settings: &Settings,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<(), ConfigError> {
        if let Some(mode) = settings.display_mode() {
            self.display.display_mode = Some(mode);
        }
        if let Some(resolution) = settings.resolution() {
            self.display.resolution = Some(resolution);
        }
        if let Some(vsync) = settings.vsync() {
            self.display.vsync = Some(vsync);
            self.display.present_mode = None;
            self.render.present_mode = None;
        }
        if let Some(cap) = settings.frame_rate_cap() {
            self.display.frame_rate_cap = match cap {
                FrameRateCap::Uncapped => None,
                FrameRateCap::Fps(fps) => Some(fps.get()),
            };
        }
        if self.env_overrides {
            self.apply_env_overrides(env)?;
        }
        Ok(())
    }

    /// Takes the warnings [`Config::load`] kept, for the caller to log once
    /// a logger is installed; `App::new` logs them. Later calls return none.
    pub fn take_load_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.load_warnings)
    }

    fn apply_env_overrides_from_env(&mut self) -> Result<(), ConfigError> {
        self.apply_env_overrides(env_var)
    }

    /// `env` looks a variable up, so tests need no `set_var`.
    fn apply_env_overrides(
        &mut self,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<(), ConfigError> {
        if let Some(value) = env(DISPLAY_MODE_ENV) {
            self.apply_display_mode_override(&value)?;
        }
        if let Some(value) = env(DISPLAY_RESOLUTION_ENV) {
            self.apply_display_resolution_override(&value)?;
        }
        if let Some(value) = env(DISPLAY_FRAME_RATE_CAP_ENV) {
            self.apply_display_frame_rate_cap_override(&value)?;
        }
        if let Some(value) = env(DISPLAY_PRESENT_MODE_ENV) {
            self.apply_display_present_mode_override(&value)?;
        }
        if let Some(value) = env(DISPLAY_MAX_FRAME_LATENCY_ENV) {
            self.apply_display_max_frame_latency_override(&value)?;
        }
        if let Some(value) = env(RENDER_PRESENT_MODE_ENV) {
            self.apply_present_mode_override(&value)?;
        }
        if let Some(value) = env(RENDER_MAX_FRAME_LATENCY_ENV) {
            self.apply_max_frame_latency_override(&value)?;
        }
        if let Some(value) = env(RENDER_MSAA_ENV) {
            self.apply_msaa_override(&value)?;
        }
        if let Some(value) = env(RENDER_DEPTH_ENABLED_ENV) {
            self.apply_depth_enabled_override(&value)?;
        }
        if let Some(value) = env(RENDER_DEPTH_SORT_ENV) {
            self.apply_depth_sort_override(&value)?;
        }
        if let Some(value) = env(RENDER_POST_AA_ENV) {
            self.apply_post_aa_override(&value)?;
        }
        if let Some(value) = env(RENDER_BLOOM_MAX_MIPS_ENV) {
            self.apply_bloom_max_mips_override(&value)?;
        }
        Ok(())
    }

    fn apply_msaa_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = value
            .parse::<u32>()
            .ok()
            .filter(|v| is_supported_msaa(*v))
            .ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: RENDER_MSAA_ENV,
                value: value.to_string(),
                expected: MSAA_EXPECTED,
            })?;
        self.render.msaa = parsed;
        Ok(())
    }

    fn apply_depth_enabled_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = match value {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => {
                return Err(ConfigError::InvalidEnvOverride {
                    var: RENDER_DEPTH_ENABLED_ENV,
                    value: value.to_string(),
                    expected: DEPTH_ENABLED_EXPECTED,
                });
            }
        };
        self.render.depth_enabled = parsed;
        Ok(())
    }

    fn apply_depth_sort_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed =
            DepthSortMode::from_str(value).map_err(|_| ConfigError::InvalidEnvOverride {
                var: RENDER_DEPTH_SORT_ENV,
                value: value.to_string(),
                expected: DEPTH_SORT_EXPECTED,
            })?;
        self.render.depth_sort = parsed;
        Ok(())
    }

    fn apply_post_aa_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = PostAaMode::from_str(value).map_err(|_| ConfigError::InvalidEnvOverride {
            var: RENDER_POST_AA_ENV,
            value: value.to_string(),
            expected: POST_AA_EXPECTED,
        })?;
        self.render.post_aa = parsed;
        Ok(())
    }

    fn apply_bloom_max_mips_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = value
            .parse::<u32>()
            .ok()
            .filter(|v| is_supported_bloom_max_mips(*v))
            .ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: RENDER_BLOOM_MAX_MIPS_ENV,
                value: value.to_string(),
                expected: BLOOM_MAX_MIPS_EXPECTED,
            })?;
        self.render.bloom_max_mips = parsed;
        Ok(())
    }

    fn apply_display_mode_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed =
            DisplayMode::from_str_name(value).ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: DISPLAY_MODE_ENV,
                value: value.to_string(),
                expected: DISPLAY_MODE_EXPECTED,
            })?;
        self.display.display_mode = Some(parsed);
        Ok(())
    }

    fn apply_display_resolution_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed =
            parse_resolution_override(value).ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: DISPLAY_RESOLUTION_ENV,
                value: value.to_string(),
                expected: DISPLAY_RESOLUTION_EXPECTED,
            })?;
        self.display.resolution = Some(parsed);
        Ok(())
    }

    fn apply_display_frame_rate_cap_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = value
            .parse::<u32>()
            .ok()
            .ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: DISPLAY_FRAME_RATE_CAP_ENV,
                value: value.to_string(),
                expected: DISPLAY_FRAME_RATE_CAP_EXPECTED,
            })?;
        self.display.frame_rate_cap = if parsed == 0 { None } else { Some(parsed) };
        Ok(())
    }

    /// Unlike the `render.*` override, this one wins over a `display.present_mode`
    /// set in the config file.
    fn apply_display_present_mode_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed =
            PresentModeConfig::from_str(value).map_err(|_| ConfigError::InvalidEnvOverride {
                var: DISPLAY_PRESENT_MODE_ENV,
                value: value.to_string(),
                expected: PRESENT_MODE_EXPECTED,
            })?;
        self.display.present_mode = Some(parsed);
        Ok(())
    }

    /// Unlike the `render.*` override, this one wins over a
    /// `display.max_frame_latency` set in the config file.
    fn apply_display_max_frame_latency_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = value
            .parse::<u32>()
            .ok()
            .filter(|v| *v >= 1)
            .ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: DISPLAY_MAX_FRAME_LATENCY_ENV,
                value: value.to_string(),
                expected: MAX_FRAME_LATENCY_EXPECTED,
            })?;
        self.display.max_frame_latency = Some(parsed);
        Ok(())
    }

    fn apply_present_mode_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed =
            PresentModeConfig::from_str(value).map_err(|_| ConfigError::InvalidEnvOverride {
                var: RENDER_PRESENT_MODE_ENV,
                value: value.to_string(),
                expected: PRESENT_MODE_EXPECTED,
            })?;
        self.render.present_mode = Some(parsed);
        Ok(())
    }

    fn apply_max_frame_latency_override(&mut self, value: &str) -> Result<(), ConfigError> {
        let parsed = value
            .parse::<u32>()
            .ok()
            .filter(|v| *v >= 1)
            .ok_or_else(|| ConfigError::InvalidEnvOverride {
                var: RENDER_MAX_FRAME_LATENCY_ENV,
                value: value.to_string(),
                expected: MAX_FRAME_LATENCY_EXPECTED,
            })?;
        self.render.max_frame_latency = Some(parsed);
        Ok(())
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn parse_resolution_override(value: &str) -> Option<Resolution> {
    let (width, height) = value.split_once('x')?;
    let width = width.parse::<u32>().ok()?;
    let height = height.parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    Some(Resolution { width, height })
}

fn display_conflicts(raw: &Value, warnings: &mut Vec<String>) {
    let Some(display) = raw.get("display").and_then(Value::as_object) else {
        return;
    };
    let window = raw.get("window").and_then(Value::as_object);
    let render = raw.get("render").and_then(Value::as_object);

    if let Some(display_resolution) = display.get("resolution").and_then(parse_raw_resolution)
        && let Some(window) = window
    {
        let width = window.get("width").and_then(raw_u32);
        let height = window.get("height").and_then(raw_u32);
        if width.is_some_and(|legacy| legacy != display_resolution.width)
            || height.is_some_and(|legacy| legacy != display_resolution.height)
        {
            warnings.push(
                "Config display.resolution overrides legacy window.width/window.height".into(),
            );
        }
    }

    if let Some(display_vsync) = display.get("vsync").and_then(Value::as_bool)
        && let Some(legacy_vsync) = window
            .and_then(|window| window.get("vsync"))
            .and_then(Value::as_bool)
        && legacy_vsync != display_vsync
    {
        warnings.push("Config display.vsync overrides legacy window.vsync".into());
    }

    if let Some(display_present_mode) = display
        .get("present_mode")
        .and_then(Value::as_str)
        .filter(|value| PresentModeConfig::from_str(value).is_ok())
        && let Some(legacy_present_mode) = render
            .and_then(|render| render.get("present_mode"))
            .and_then(Value::as_str)
        && legacy_present_mode != display_present_mode
    {
        warnings.push("Config display.present_mode overrides legacy render.present_mode".into());
    }

    if let Some(display_latency) = display
        .get("max_frame_latency")
        .and_then(raw_u32)
        .filter(|value| *value >= 1)
        && let Some(legacy_latency) = render
            .and_then(|render| render.get("max_frame_latency"))
            .and_then(raw_u32)
        && legacy_latency != display_latency
    {
        warnings.push(
            "Config display.max_frame_latency overrides legacy render.max_frame_latency".into(),
        );
    }
}

fn parse_raw_resolution(value: &Value) -> Option<Resolution> {
    let map = value.as_object()?;
    let width = map.get("width").and_then(raw_u32)?;
    let height = map.get("height").and_then(raw_u32)?;
    if width == 0 || height == 0 {
        return None;
    }
    Some(Resolution { width, height })
}

fn raw_u32(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|raw| u32::try_from(raw).ok())
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
