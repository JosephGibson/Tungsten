use serde::Deserialize;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

const RENDER_PRESENT_MODE_ENV: &str = "TUNGSTEN_RENDER_PRESENT_MODE";
const RENDER_MAX_FRAME_LATENCY_ENV: &str = "TUNGSTEN_RENDER_MAX_FRAME_LATENCY";
const PRESENT_MODE_EXPECTED: &str =
    "one of: auto, immediate, mailbox, fifo, auto_vsync, auto_no_vsync";
const MAX_FRAME_LATENCY_EXPECTED: &str = "an integer >= 1";

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

#[derive(Debug, Clone, Deserialize)]
pub struct RenderConfig {
    #[serde(default = "default_clear_color")]
    pub clear_color: [f64; 4],
    pub max_frame_latency: Option<u32>,
    pub present_mode: Option<PresentModeConfig>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            clear_color: default_clear_color(),
            max_frame_latency: None,
            present_mode: None,
        }
    }
}

fn default_clear_color() -> [f64; 4] {
    [0.05, 0.05, 0.08, 1.0]
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

/// Top-level engine configuration, loaded from `tungsten.json`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    /// Load config from the given path. Falls back to defaults with a warning
    /// if the file is missing. Fatals on invalid JSON, naming the bad field.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let mut config = match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|e| ConfigError::Parse {
                path: path.display().to_string(),
                source: e,
            })?,
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
        if let Ok(value) = std::env::var(RENDER_PRESENT_MODE_ENV) {
            self.apply_present_mode_override(&value)?;
        }
        if let Ok(value) = std::env::var(RENDER_MAX_FRAME_LATENCY_ENV) {
            self.apply_max_frame_latency_override(&value)?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let config = Config::default();
        assert_eq!(config.window.title, "Tungsten");
        assert_eq!(config.window.width, 1280);
        assert_eq!(config.window.height, 720);
        assert!(!config.window.vsync);
        assert!(config.render.max_frame_latency.is_none());
        assert!(config.render.present_mode.is_none());
    }

    #[test]
    fn parses_partial_json() {
        let json = r#"{ "window": { "title": "Test" } }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.window.title, "Test");
        assert_eq!(config.window.width, 1280); // default
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
    fn missing_file_returns_defaults() {
        let config = Config::load("/nonexistent/path/tungsten.json").unwrap();
        assert_eq!(config.window.title, "Tungsten");
    }
}
