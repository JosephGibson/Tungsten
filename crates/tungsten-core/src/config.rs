use crate::display::{DisplayConfig, DisplayMode, Resolution};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

const DISPLAY_MODE_ENV: &str = "TUNGSTEN_DISPLAY_MODE";
const DISPLAY_RESOLUTION_ENV: &str = "TUNGSTEN_DISPLAY_RESOLUTION";
const DISPLAY_FRAME_RATE_CAP_ENV: &str = "TUNGSTEN_DISPLAY_FRAME_RATE_CAP";
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

/// Top-level engine configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    /// Load config; missing file falls back to defaults.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let mut config = match std::fs::read_to_string(path) {
            Ok(contents) => {
                let raw: Value =
                    serde_json::from_str(&contents).map_err(|e| ConfigError::Parse {
                        path: path.display().to_string(),
                        source: e,
                    })?;
                warn_display_conflicts(&raw);
                let parsed: Config =
                    serde_json::from_value(raw).map_err(|e| ConfigError::Parse {
                        path: path.display().to_string(),
                        source: e,
                    })?;
                if !is_supported_msaa(parsed.render.msaa) {
                    return Err(ConfigError::InvalidEnvOverride {
                        var: "render.msaa",
                        value: parsed.render.msaa.to_string(),
                        expected: MSAA_EXPECTED,
                    });
                }
                if !is_supported_bloom_max_mips(parsed.render.bloom_max_mips) {
                    return Err(ConfigError::InvalidEnvOverride {
                        var: "render.bloom_max_mips",
                        value: parsed.render.bloom_max_mips.to_string(),
                        expected: BLOOM_MAX_MIPS_EXPECTED,
                    });
                }
                parsed
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn!("Config file '{}' not found, using defaults", path.display());
                Config::default()
            }
            Err(e) => Err(ConfigError::Io {
                path: path.display().to_string(),
                source: e,
            })?,
        };

        config.apply_env_overrides_from_env()?;
        Ok(config)
    }

    fn apply_env_overrides_from_env(&mut self) -> Result<(), ConfigError> {
        if let Ok(value) = std::env::var(DISPLAY_MODE_ENV) {
            self.apply_display_mode_override(&value)?;
        }
        if let Ok(value) = std::env::var(DISPLAY_RESOLUTION_ENV) {
            self.apply_display_resolution_override(&value)?;
        }
        if let Ok(value) = std::env::var(DISPLAY_FRAME_RATE_CAP_ENV) {
            self.apply_display_frame_rate_cap_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_PRESENT_MODE_ENV) {
            self.apply_present_mode_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_MAX_FRAME_LATENCY_ENV) {
            self.apply_max_frame_latency_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_MSAA_ENV) {
            self.apply_msaa_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_DEPTH_ENABLED_ENV) {
            self.apply_depth_enabled_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_DEPTH_SORT_ENV) {
            self.apply_depth_sort_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_POST_AA_ENV) {
            self.apply_post_aa_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_BLOOM_MAX_MIPS_ENV) {
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
                })
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

fn parse_resolution_override(value: &str) -> Option<Resolution> {
    let (width, height) = value.split_once('x')?;
    let width = width.parse::<u32>().ok()?;
    let height = height.parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    Some(Resolution { width, height })
}

fn warn_display_conflicts(raw: &Value) {
    let Some(display) = raw.get("display").and_then(Value::as_object) else {
        return;
    };
    let window = raw.get("window").and_then(Value::as_object);
    let render = raw.get("render").and_then(Value::as_object);

    if let Some(display_resolution) = display.get("resolution").and_then(parse_raw_resolution) {
        if let Some(window) = window {
            let width = window.get("width").and_then(raw_u32);
            let height = window.get("height").and_then(raw_u32);
            if width.is_some_and(|legacy| legacy != display_resolution.width)
                || height.is_some_and(|legacy| legacy != display_resolution.height)
            {
                log::warn!("Config display.resolution overrides legacy window.width/window.height");
            }
        }
    }

    if let Some(display_vsync) = display.get("vsync").and_then(Value::as_bool) {
        if let Some(legacy_vsync) = window
            .and_then(|window| window.get("vsync"))
            .and_then(Value::as_bool)
        {
            if legacy_vsync != display_vsync {
                log::warn!("Config display.vsync overrides legacy window.vsync");
            }
        }
    }

    if let Some(display_present_mode) = display
        .get("present_mode")
        .and_then(Value::as_str)
        .filter(|value| PresentModeConfig::from_str(value).is_ok())
    {
        if let Some(legacy_present_mode) = render
            .and_then(|render| render.get("present_mode"))
            .and_then(Value::as_str)
        {
            if legacy_present_mode != display_present_mode {
                log::warn!("Config display.present_mode overrides legacy render.present_mode");
            }
        }
    }

    if let Some(display_latency) = display
        .get("max_frame_latency")
        .and_then(raw_u32)
        .filter(|value| *value >= 1)
    {
        if let Some(legacy_latency) = render
            .and_then(|render| render.get("max_frame_latency"))
            .and_then(raw_u32)
        {
            if legacy_latency != display_latency {
                log::warn!(
                    "Config display.max_frame_latency overrides legacy render.max_frame_latency"
                );
            }
        }
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
