use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

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
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|e| ConfigError::Parse {
                path: path.display().to_string(),
                source: e,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn!("Config file '{}' not found, using defaults", path.display());
                Ok(Config::default())
            }
            Err(e) => Err(ConfigError::Io {
                path: path.display().to_string(),
                source: e,
            }),
        }
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
    fn missing_file_returns_defaults() {
        let config = Config::load("/nonexistent/path/tungsten.json").unwrap();
        assert_eq!(config.window.title, "Tungsten");
    }
}
