use crate::config::{PresentModeConfig, RenderConfig, WindowConfig};
use glam::Vec2;
use serde::Deserialize;
use serde::de::Deserializer;
use serde_json::{Map, Value};
use thiserror::Error;

const DISPLAY_MODE_EXPECTED: &str = "windowed, borderless_fullscreen, or exclusive_fullscreen";
const SCALE_MODE_EXPECTED: &str = "stretch or integer";
const PRESENT_MODE_EXPECTED: &str = "auto, immediate, mailbox, fifo, auto_vsync, or auto_no_vsync";

/// How the window is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// A window at the configured resolution.
    Windowed,
    /// A borderless window covering the monitor.
    BorderlessFullscreen,
    /// The monitor's exclusive fullscreen mode.
    ExclusiveFullscreen,
}

impl DisplayMode {
    /// The config name: `windowed`, `borderless_fullscreen` or
    /// `exclusive_fullscreen`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::BorderlessFullscreen => "borderless_fullscreen",
            Self::ExclusiveFullscreen => "exclusive_fullscreen",
        }
    }

    /// The mode a config name denotes; `None` for any other string.
    #[must_use]
    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "windowed" => Some(Self::Windowed),
            "borderless_fullscreen" => Some(Self::BorderlessFullscreen),
            "exclusive_fullscreen" => Some(Self::ExclusiveFullscreen),
            _ => None,
        }
    }
}

/// How the render resolution fills the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    /// Stretched to the window.
    Stretch,
    /// Scaled by the largest whole factor that fits, pixel-exact.
    Integer,
}

impl ScaleMode {
    /// The config name: `stretch` or `integer`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Integer => "integer",
        }
    }

    /// The mode a config name denotes; `None` for any other string.
    #[must_use]
    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "stretch" => Some(Self::Stretch),
            "integer" => Some(Self::Integer),
            _ => None,
        }
    }
}

/// A render resolution in pixels; 1280 by 720 by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
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

/// The window's size in physical pixels: a `World` resource `App` inserts
/// at startup and keeps current through resizes, which extracts and
/// overlays read. In core since M38, so a plugin on core alone can read it
/// (`D-128`); the umbrella re-exports it at its old paths.
#[derive(Debug, Clone, Copy)]
pub struct WindowSize {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

/// One video mode a monitor offers (`D-184`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoMode {
    /// Its size in physical pixels.
    pub size: Resolution,
    /// Its refresh rate in millihertz: 60 Hz is 60 000.
    pub refresh_millihertz: u32,
}

impl VideoMode {
    /// A mode of `size` at `refresh_millihertz`.
    #[must_use]
    pub const fn new(size: Resolution, refresh_millihertz: u32) -> Self {
        Self {
            size,
            refresh_millihertz,
        }
    }
}

/// The monitor the window is on, as the platform reports it (`D-184`): a
/// `World` resource the app inserts once a window exists and replaces when
/// the window moves or its scale changes; absent headless and in the
/// harness.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MonitorInfo {
    /// Its current mode's size in physical pixels: the desktop, which a
    /// window never outgrows.
    pub size: Resolution,
    /// Its current refresh rate in millihertz, when the platform reports
    /// one.
    pub refresh_millihertz: Option<u32>,
    /// Every mode it reports, largest first (by area, then width, then
    /// refresh), each once; a mode with a 0 side is left out.
    pub modes: Vec<VideoMode>,
}

impl MonitorInfo {
    /// The monitor whose current mode is `size`; a refresh of 0 reads as
    /// unknown. `None` for a size with a 0 side, as a Wayland output
    /// reports before its first current mode.
    #[must_use]
    pub fn new(
        size: Resolution,
        refresh_millihertz: Option<u32>,
        modes: impl IntoIterator<Item = VideoMode>,
    ) -> Option<Self> {
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let mut modes: Vec<VideoMode> = modes
            .into_iter()
            .filter(|mode| mode.size.width > 0 && mode.size.height > 0)
            .collect();
        modes.sort_by_key(|mode| {
            std::cmp::Reverse((area(mode.size), mode.size.width, mode.refresh_millihertz))
        });
        modes.dedup();
        Some(Self {
            size,
            refresh_millihertz: refresh_millihertz.filter(|&hz| hz > 0),
            modes,
        })
    }

    /// Each distinct mode size no wider and no taller than [`size`](Self::size),
    /// and `size` itself, smallest first (by area, then width): the window
    /// sizes this monitor can show whole.
    #[must_use]
    pub fn window_sizes(&self) -> Vec<Resolution> {
        let mut sizes: Vec<Resolution> = self
            .modes
            .iter()
            .map(|mode| mode.size)
            .filter(|size| size.width <= self.size.width && size.height <= self.size.height)
            .chain(std::iter::once(self.size))
            .collect();
        sizes.sort_by_key(|size| (area(*size), size.width));
        sizes.dedup();
        sizes
    }
}

fn area(size: Resolution) -> u64 {
    u64::from(size.width) * u64::from(size.height)
}

/// The display's scale factor: a `World` resource `App` inserts at 1.0 and
/// keeps current from the window. [`os`](Self::os) is the OS scale, physical
/// pixels per logical unit, [`ui_scale`](Self::ui_scale) the player's
/// setting, and [`get`](Self::get) the factor UI layout uses (`D-127`,
/// `D-144`, `D-159`). A game may size the UI from the window instead
/// (`D-184`): a [reference height](Self::set_reference_height) and a
/// [minimum viewport](Self::set_min_viewport), read against the
/// [window size](Self::set_window_size) the app keeps here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor {
    os: f32,
    ui: f32,
    reference: Option<f32>,
    floor: Option<Vec2>,
    window: (u32, u32),
    /// What `get` returns, recomputed whenever an input changes.
    factor: f32,
}

impl ScaleFactor {
    /// The factor UI layout uses: the base times the player's UI scale, the
    /// base being the OS scale or, with a reference height and a window
    /// size, the larger of the OS scale and the window's height over the
    /// reference; then, with a minimum viewport and a window size, at most
    /// the window's width over the floor's and its height over the floor's.
    /// Without either, the OS scale times the UI scale. Cached, so a call
    /// costs a read.
    #[must_use]
    pub fn get(&self) -> f32 {
        self.factor
    }

    /// The OS scale: physical pixels per logical unit.
    #[must_use]
    pub fn os(&self) -> f32 {
        self.os
    }

    /// Sets the OS scale; a value that is not finite or not positive is
    /// ignored.
    pub fn set_os(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.os = scale;
            self.refresh();
        }
    }

    /// The player's UI scale, from `Settings`: 1.0 unless set.
    #[must_use]
    pub fn ui_scale(&self) -> f32 {
        self.ui
    }

    /// Sets the player's UI scale; a value that is not finite or not
    /// positive is ignored.
    pub fn set_ui_scale(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.ui = scale;
            self.refresh();
        }
    }

    /// Sizes the UI from the window's height: the window's height over
    /// `height` replaces the OS scale in [`get`](Self::get) when larger, so
    /// 720.0 lays a 2160-pixel window out at 3. `None`, the default, is the
    /// OS scale alone. A value that is not finite or not positive is
    /// ignored.
    pub fn set_reference_height(&mut self, height: Option<f32>) {
        match height {
            Some(height) if !(height.is_finite() && height > 0.0) => {}
            height => {
                self.reference = height;
                self.refresh();
            }
        }
    }

    /// The reference height [`set_reference_height`](Self::set_reference_height)
    /// set; `None` by default.
    #[must_use]
    pub fn reference_height(&self) -> Option<f32> {
        self.reference
    }

    /// The smallest viewport, in UI units, the factor lays a window out in:
    /// [`get`](Self::get) never rises past the window's width over
    /// `floor.x` or its height over `floor.y`, whatever the scales ask.
    /// `None`, the default, sets no floor. A floor with a side that is not
    /// finite or not positive is ignored.
    pub fn set_min_viewport(&mut self, floor: Option<Vec2>) {
        match floor {
            Some(floor) if !(floor.is_finite() && floor.x > 0.0 && floor.y > 0.0) => {}
            floor => {
                self.floor = floor;
                self.refresh();
            }
        }
    }

    /// The floor [`set_min_viewport`](Self::set_min_viewport) set; `None`
    /// by default.
    #[must_use]
    pub fn min_viewport(&self) -> Option<Vec2> {
        self.floor
    }

    /// The window's size in physical pixels, which the reference height and
    /// the floor read: the app sets it wherever it sets `WindowSize`, and
    /// `UiHarness` before each frame and each layout. 0×0, the default, or
    /// any 0 side leaves both out.
    pub fn set_window_size(&mut self, width: u32, height: u32) {
        self.window = (width, height);
        self.refresh();
    }

    fn refresh(&mut self) {
        let (width, height) = self.window;
        let sized = width > 0 && height > 0;
        let (width, height) = (width as f32, height as f32);
        let mut base = self.os;
        if let Some(reference) = self.reference
            && sized
        {
            base = base.max(height / reference);
        }
        let mut factor = base * self.ui;
        if let Some(floor) = self.floor
            && sized
        {
            factor = factor.min(width / floor.x).min(height / floor.y);
        }
        self.factor = factor;
    }
}

impl Default for ScaleFactor {
    fn default() -> Self {
        Self {
            os: 1.0,
            ui: 1.0,
            reference: None,
            floor: None,
            window: (0, 0),
            factor: 1.0,
        }
    }
}

/// The resolved display settings: what [`DisplayConfig::resolve`] produces
/// at startup and a `World` resource the app keeps current as hotkeys and
/// pending requests change them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayState {
    /// The render resolution.
    pub resolution: Resolution,
    /// How the window is presented.
    pub display_mode: DisplayMode,
    /// Whether presentation waits for the vertical blank.
    pub vsync: bool,
    /// The swapchain present mode, when the config names one.
    pub present_mode: Option<PresentModeConfig>,
    /// The swapchain's maximum frame latency, when the config sets one.
    pub max_frame_latency: Option<u32>,
    /// How the render resolution fills the window.
    pub scale_mode: ScaleMode,
    /// The frame-rate cap in frames per second, when one is set.
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
    /// Checks the state is one the engine can run with.
    ///
    /// # Errors
    ///
    /// A zero width or height, or a frame-rate cap of zero.
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

/// The `display` section of `tungsten.json`, every field optional; a value
/// that fails to parse is dropped with a warning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayConfig {
    /// `display.resolution`.
    pub resolution: Option<Resolution>,
    /// `display.display_mode`.
    pub display_mode: Option<DisplayMode>,
    /// `display.vsync`.
    pub vsync: Option<bool>,
    /// `display.present_mode`.
    pub present_mode: Option<PresentModeConfig>,
    /// `display.max_frame_latency`.
    pub max_frame_latency: Option<u32>,
    /// `display.scale_mode`.
    pub scale_mode: Option<ScaleMode>,
    /// `display.frame_rate_cap`.
    pub frame_rate_cap: Option<u32>,
}

impl DisplayConfig {
    /// The state this section gives: each set field, else the `window`
    /// section's size and vsync, the `render` section's present mode and
    /// frame latency, windowed, stretched and uncapped.
    #[must_use]
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

    /// Parses the display section, pushing a message to `warnings` for each
    /// value that falls back.
    pub(crate) fn from_json_value(value: Value, warnings: &mut Vec<String>) -> Self {
        let Value::Object(map) = value else {
            warnings
                .push("Config display section must be an object; ignoring invalid value".into());
            return Self::default();
        };

        Self::from_json_object(&map, warnings)
    }

    fn from_json_object(map: &Map<String, Value>, warnings: &mut Vec<String>) -> Self {
        Self {
            resolution: map
                .get("resolution")
                .and_then(|value| parse_resolution(value, warnings)),
            display_mode: map
                .get("display_mode")
                .and_then(|value| parse_display_mode(value, warnings)),
            vsync: map
                .get("vsync")
                .and_then(|value| parse_bool_field("display.vsync", value, warnings)),
            present_mode: map
                .get("present_mode")
                .and_then(|value| parse_present_mode(value, warnings)),
            max_frame_latency: map.get("max_frame_latency").and_then(|value| {
                parse_optional_positive_u32("display.max_frame_latency", value, warnings)
            }),
            scale_mode: map
                .get("scale_mode")
                .and_then(|value| parse_scale_mode(value, warnings)),
            frame_rate_cap: map
                .get("frame_rate_cap")
                .and_then(|value| parse_frame_rate_cap(value, warnings)),
        }
    }
}

/// Logs each fallback; `Config::load` parses the section apart and keeps
/// them for `Config::take_load_warnings` instead (`D-119`).
impl<'de> Deserialize<'de> for DisplayConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut warnings = Vec::new();
        let config = Self::from_json_value(Value::deserialize(deserializer)?, &mut warnings);
        for warning in warnings {
            log::warn!("{warning}");
        }
        Ok(config)
    }
}

/// A display state the engine cannot run with ([`DisplayState::validate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DisplayValidationError {
    /// A zero width or height.
    #[error("invalid resolution {width}x{height}; expected width >= 1 and height >= 1")]
    InvalidResolution {
        /// The width given.
        width: u32,
        /// The height given.
        height: u32,
    },
    /// A frame-rate cap of zero.
    #[error("invalid frame_rate_cap '{0}'; expected >= 1")]
    InvalidFrameRateCap(u32),
}

fn parse_display_mode(value: &Value, warnings: &mut Vec<String>) -> Option<DisplayMode> {
    parse_named_enum(
        "display.display_mode",
        value,
        DisplayMode::from_str_name,
        DISPLAY_MODE_EXPECTED,
        warnings,
    )
}

fn parse_scale_mode(value: &Value, warnings: &mut Vec<String>) -> Option<ScaleMode> {
    parse_named_enum(
        "display.scale_mode",
        value,
        ScaleMode::from_str_name,
        SCALE_MODE_EXPECTED,
        warnings,
    )
}

fn parse_present_mode(value: &Value, warnings: &mut Vec<String>) -> Option<PresentModeConfig> {
    parse_named_enum(
        "display.present_mode",
        value,
        |raw| raw.parse::<PresentModeConfig>().ok(),
        PRESENT_MODE_EXPECTED,
        warnings,
    )
}

fn parse_named_enum<T>(
    field_name: &str,
    value: &Value,
    parser: impl Fn(&str) -> Option<T>,
    expected: &str,
    warnings: &mut Vec<String>,
) -> Option<T> {
    match value {
        Value::Null => None,
        Value::String(raw) => {
            if let Some(parsed) = parser(raw) {
                Some(parsed)
            } else {
                warnings.push(format!(
                    "Config {field_name}='{raw}' is invalid; expected {expected}; falling back"
                ));
                None
            }
        }
        other => {
            warnings.push(format!(
                "Config {}={} is invalid; expected {}; falling back",
                field_name,
                describe_json_value(other),
                expected
            ));
            None
        }
    }
}

fn parse_bool_field(field_name: &str, value: &Value, warnings: &mut Vec<String>) -> Option<bool> {
    match value {
        Value::Null => None,
        Value::Bool(parsed) => Some(*parsed),
        other => {
            warnings.push(format!(
                "Config {}={} is invalid; expected true or false; falling back",
                field_name,
                describe_json_value(other)
            ));
            None
        }
    }
}

fn parse_optional_positive_u32(
    field_name: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    match value {
        Value::Null => None,
        Value::Number(number) => match number.as_u64().and_then(|raw| u32::try_from(raw).ok()) {
            Some(0) => {
                warnings.push(format!(
                    "Config {field_name}=0 is invalid; expected an integer >= 1; falling back"
                ));
                None
            }
            Some(parsed) => Some(parsed),
            None => {
                warnings.push(format!(
                    "Config {}={} is invalid; expected an integer >= 1; falling back",
                    field_name,
                    describe_json_value(value)
                ));
                None
            }
        },
        other => {
            warnings.push(format!(
                "Config {}={} is invalid; expected an integer >= 1; falling back",
                field_name,
                describe_json_value(other)
            ));
            None
        }
    }
}

fn parse_frame_rate_cap(value: &Value, warnings: &mut Vec<String>) -> Option<u32> {
    match value {
        Value::Null => None,
        Value::Number(number) => match number.as_u64().and_then(|raw| u32::try_from(raw).ok()) {
            Some(0) => {
                warnings.push("Config display.frame_rate_cap=0 means uncapped; using None".into());
                None
            }
            Some(parsed) => Some(parsed),
            None => {
                warnings.push(format!(
                    "Config display.frame_rate_cap={} is invalid; expected null or an integer >= 1; falling back",
                    describe_json_value(value)
                ));
                None
            }
        },
        other => {
            warnings.push(format!(
                "Config display.frame_rate_cap={} is invalid; expected null or an integer >= 1; falling back",
                describe_json_value(other)
            ));
            None
        }
    }
}

fn parse_resolution(value: &Value, warnings: &mut Vec<String>) -> Option<Resolution> {
    let Value::Object(map) = value else {
        if !value.is_null() {
            warnings.push(format!(
                "Config display.resolution={} is invalid; expected {{\"width\":<u32>,\"height\":<u32>}} with both >= 1; falling back",
                describe_json_value(value)
            ));
        }
        return None;
    };

    let width = parse_u32_member("display.resolution.width", map.get("width"), warnings);
    let height = parse_u32_member("display.resolution.height", map.get("height"), warnings);

    match (width, height) {
        (Some(width), Some(height)) if width >= 1 && height >= 1 => {
            Some(Resolution { width, height })
        }
        (Some(width), Some(height)) => {
            warnings.push(format!(
                "Config display.resolution={width}x{height} is invalid; expected width >= 1 and height >= 1; falling back"
            ));
            None
        }
        _ => {
            warnings.push(
                "Config display.resolution is invalid; expected {\"width\":<u32>,\"height\":<u32>} with both >= 1; falling back"
                    .into(),
            );
            None
        }
    }
}

fn parse_u32_member(
    field_name: &str,
    value: Option<&Value>,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    match value {
        Some(Value::Number(number)) => number.as_u64().and_then(|raw| u32::try_from(raw).ok()),
        Some(Value::Null) | None => None,
        Some(other) => {
            warnings.push(format!(
                "Config {}={} is invalid; expected an integer >= 0",
                field_name,
                describe_json_value(other)
            ));
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
#[path = "tests/display.rs"]
mod tests;
