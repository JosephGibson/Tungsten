use crate::config::{PresentModeConfig, RenderConfig, WindowConfig};
use serde::de::Deserializer;
use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

const DISPLAY_MODE_EXPECTED: &str = "windowed, borderless_fullscreen, or exclusive_fullscreen";
const SCALE_MODE_EXPECTED: &str = "stretch or integer";
const PRESENT_MODE_EXPECTED: &str = "auto, immediate, mailbox, fifo, auto_vsync, or auto_no_vsync";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Windowed,
    BorderlessFullscreen,
    ExclusiveFullscreen,
}

impl DisplayMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::BorderlessFullscreen => "borderless_fullscreen",
            Self::ExclusiveFullscreen => "exclusive_fullscreen",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "windowed" => Some(Self::Windowed),
            "borderless_fullscreen" => Some(Self::BorderlessFullscreen),
            "exclusive_fullscreen" => Some(Self::ExclusiveFullscreen),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    Stretch,
    Integer,
}

impl ScaleMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Integer => "integer",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "stretch" => Some(Self::Stretch),
            "integer" => Some(Self::Integer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayState {
    pub resolution: Resolution,
    pub display_mode: DisplayMode,
    pub vsync: bool,
    pub present_mode: Option<PresentModeConfig>,
    pub max_frame_latency: Option<u32>,
    pub scale_mode: ScaleMode,
    pub frame_rate_cap: Option<u32>,
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            resolution: Resolution::default(),
            display_mode: DisplayMode::Windowed,
            vsync: false,
            present_mode: None,
            max_frame_latency: None,
            scale_mode: ScaleMode::Stretch,
            frame_rate_cap: None,
        }
    }
}

impl DisplayState {
    pub fn validate(&self) -> Result<(), DisplayValidationError> {
        if self.resolution.width == 0 || self.resolution.height == 0 {
            return Err(DisplayValidationError::InvalidResolution {
                width: self.resolution.width,
                height: self.resolution.height,
            });
        }

        if matches!(self.frame_rate_cap, Some(0)) {
            return Err(DisplayValidationError::InvalidFrameRateCap(0));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayConfig {
    pub resolution: Option<Resolution>,
    pub display_mode: Option<DisplayMode>,
    pub vsync: Option<bool>,
    pub present_mode: Option<PresentModeConfig>,
    pub max_frame_latency: Option<u32>,
    pub scale_mode: Option<ScaleMode>,
    pub frame_rate_cap: Option<u32>,
}

impl DisplayConfig {
    pub fn resolve(&self, window: &WindowConfig, render: &RenderConfig) -> DisplayState {
        let mut resolved = DisplayState {
            resolution: Resolution {
                width: window.width,
                height: window.height,
            },
            display_mode: DisplayMode::Windowed,
            vsync: window.vsync,
            present_mode: render.present_mode,
            max_frame_latency: render.max_frame_latency,
            scale_mode: ScaleMode::Stretch,
            frame_rate_cap: None,
        };

        if let Some(resolution) = self.resolution {
            resolved.resolution = resolution;
        }
        if let Some(display_mode) = self.display_mode {
            resolved.display_mode = display_mode;
        }
        if let Some(vsync) = self.vsync {
            resolved.vsync = vsync;
        }
        if let Some(present_mode) = self.present_mode {
            resolved.present_mode = Some(present_mode);
        }
        if let Some(max_frame_latency) = self.max_frame_latency {
            resolved.max_frame_latency = Some(max_frame_latency);
        }
        if let Some(scale_mode) = self.scale_mode {
            resolved.scale_mode = scale_mode;
        }
        if let Some(frame_rate_cap) = self.frame_rate_cap {
            resolved.frame_rate_cap = Some(frame_rate_cap);
        }

        resolved
    }

    fn from_json_value(value: Value) -> Self {
        let Value::Object(map) = value else {
            log::warn!("Config display section must be an object; ignoring invalid value");
            return Self::default();
        };

        Self::from_json_object(&map)
    }

    fn from_json_object(map: &Map<String, Value>) -> Self {
        Self {
            resolution: map.get("resolution").and_then(parse_resolution),
            display_mode: map.get("display_mode").and_then(parse_display_mode),
            vsync: map
                .get("vsync")
                .and_then(|value| parse_bool_field("display.vsync", value)),
            present_mode: map.get("present_mode").and_then(parse_present_mode),
            max_frame_latency: map
                .get("max_frame_latency")
                .and_then(|value| parse_optional_positive_u32("display.max_frame_latency", value)),
            scale_mode: map.get("scale_mode").and_then(parse_scale_mode),
            frame_rate_cap: map.get("frame_rate_cap").and_then(parse_frame_rate_cap),
        }
    }
}

impl<'de> Deserialize<'de> for DisplayConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_json_value(Value::deserialize(deserializer)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DisplayValidationError {
    #[error("invalid resolution {width}x{height}; expected width >= 1 and height >= 1")]
    InvalidResolution { width: u32, height: u32 },
    #[error("invalid frame_rate_cap '{0}'; expected >= 1")]
    InvalidFrameRateCap(u32),
}

fn parse_display_mode(value: &Value) -> Option<DisplayMode> {
    parse_named_enum(
        "display.display_mode",
        value,
        DisplayMode::from_str_name,
        DISPLAY_MODE_EXPECTED,
    )
}

fn parse_scale_mode(value: &Value) -> Option<ScaleMode> {
    parse_named_enum(
        "display.scale_mode",
        value,
        ScaleMode::from_str_name,
        SCALE_MODE_EXPECTED,
    )
}

fn parse_present_mode(value: &Value) -> Option<PresentModeConfig> {
    parse_named_enum(
        "display.present_mode",
        value,
        |raw| raw.parse::<PresentModeConfig>().ok(),
        PRESENT_MODE_EXPECTED,
    )
}

fn parse_named_enum<T>(
    field_name: &str,
    value: &Value,
    parser: impl Fn(&str) -> Option<T>,
    expected: &str,
) -> Option<T> {
    match value {
        Value::Null => None,
        Value::String(raw) => match parser(raw) {
            Some(parsed) => Some(parsed),
            None => {
                log::warn!(
                    "Config {}='{}' is invalid; expected {}; falling back",
                    field_name,
                    raw,
                    expected
                );
                None
            }
        },
        other => {
            log::warn!(
                "Config {}={} is invalid; expected {}; falling back",
                field_name,
                describe_json_value(other),
                expected
            );
            None
        }
    }
}

fn parse_bool_field(field_name: &str, value: &Value) -> Option<bool> {
    match value {
        Value::Null => None,
        Value::Bool(parsed) => Some(*parsed),
        other => {
            log::warn!(
                "Config {}={} is invalid; expected true or false; falling back",
                field_name,
                describe_json_value(other)
            );
            None
        }
    }
}

fn parse_optional_positive_u32(field_name: &str, value: &Value) -> Option<u32> {
    match value {
        Value::Null => None,
        Value::Number(number) => match number.as_u64().and_then(|raw| u32::try_from(raw).ok()) {
            Some(0) => {
                log::warn!(
                    "Config {}=0 is invalid; expected an integer >= 1; falling back",
                    field_name
                );
                None
            }
            Some(parsed) => Some(parsed),
            None => {
                log::warn!(
                    "Config {}={} is invalid; expected an integer >= 1; falling back",
                    field_name,
                    describe_json_value(value)
                );
                None
            }
        },
        other => {
            log::warn!(
                "Config {}={} is invalid; expected an integer >= 1; falling back",
                field_name,
                describe_json_value(other)
            );
            None
        }
    }
}

fn parse_frame_rate_cap(value: &Value) -> Option<u32> {
    match value {
        Value::Null => None,
        Value::Number(number) => match number.as_u64().and_then(|raw| u32::try_from(raw).ok()) {
            Some(0) => {
                log::warn!("Config display.frame_rate_cap=0 means uncapped; using None");
                None
            }
            Some(parsed) => Some(parsed),
            None => {
                log::warn!(
                    "Config display.frame_rate_cap={} is invalid; expected null or an integer >= 1; falling back",
                    describe_json_value(value)
                );
                None
            }
        },
        other => {
            log::warn!(
                "Config display.frame_rate_cap={} is invalid; expected null or an integer >= 1; falling back",
                describe_json_value(other)
            );
            None
        }
    }
}

fn parse_resolution(value: &Value) -> Option<Resolution> {
    let Value::Object(map) = value else {
        if !value.is_null() {
            log::warn!(
                "Config display.resolution={} is invalid; expected {{\"width\":<u32>,\"height\":<u32>}} with both >= 1; falling back",
                describe_json_value(value)
            );
        }
        return None;
    };

    let width = parse_u32_member("display.resolution.width", map.get("width"));
    let height = parse_u32_member("display.resolution.height", map.get("height"));

    match (width, height) {
        (Some(width), Some(height)) if width >= 1 && height >= 1 => {
            Some(Resolution { width, height })
        }
        (Some(width), Some(height)) => {
            log::warn!(
                "Config display.resolution={}x{} is invalid; expected width >= 1 and height >= 1; falling back",
                width,
                height
            );
            None
        }
        _ => {
            log::warn!(
                "Config display.resolution is invalid; expected {{\"width\":<u32>,\"height\":<u32>}} with both >= 1; falling back"
            );
            None
        }
    }
}

fn parse_u32_member(field_name: &str, value: Option<&Value>) -> Option<u32> {
    match value {
        Some(Value::Number(number)) => number.as_u64().and_then(|raw| u32::try_from(raw).ok()),
        Some(Value::Null) | None => None,
        Some(other) => {
            log::warn!(
                "Config {}={} is invalid; expected an integer >= 0",
                field_name,
                describe_json_value(other)
            );
            None
        }
    }
}

fn describe_json_value(value: &Value) -> String {
    match serde_json::to_string(value) {
        Ok(rendered) => rendered,
        Err(_) => "<unprintable>".to_string(),
    }
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
