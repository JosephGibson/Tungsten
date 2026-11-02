//! Screen files (redo 7.20): a screen as one JSON object in the manifest's
//! `ui_screens`, a tree of [`widgets`](super::widgets)' builders whose nodes
//! carry a key, a layout, overrides and a named text style, their text
//! literal or bound by name. [`ScreenFile::from_json`] reads `version`
//! first, then the model through `serde_path_to_error`, so an error names
//! its field (`root.children[2].captoin`), then the structural checks with
//! their paths. [`ScreenFile::check`] holds the names a file uses to the
//! ones a game has ([`ScreenNames`]), [`ScreenFile::to_canonical`] writes
//! the form the repository keeps and [`json_schema`] describes the format.
//! [`UiScreenRegistry`] holds the loaded files by ID.
//!
//! The format is at [`SCREEN_VERSION`]: a kind or a field added later keeps
//! it, since every file of that version still reads; a changed meaning or a
//! removed field takes the next version and a refusal of the old one that
//! names it.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::path::Path;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value, json};
use thiserror::Error;

use super::theme::ColorTokens;
use super::theme_file::{Color, OverridesFile, TextFile, shared_schema_defs};

/// The screen file format's version.
pub const SCREEN_VERSION: u32 = 1;

/// Why screen JSON did not read: the field that failed, as
/// `serde_path_to_error` names it (`root.children[2].captoin`), empty for
/// the whole file, and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct ScreenError {
    /// The field's path; empty for the whole file.
    pub field: String,
    /// What is wrong.
    pub message: String,
}

impl ScreenError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ScreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.field.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.field, self.message)
        }
    }
}

/// Why a screen file did not load.
#[derive(Debug, Error)]
pub enum ScreenFileError {
    /// The file could not be read.
    #[error("failed to read ui screen '{path}': {source}")]
    Io {
        /// The file.
        path: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The file is not a screen.
    #[error("invalid ui screen '{path}': {source}")]
    Parse {
        /// The file.
        path: String,
        /// The field that failed and why.
        source: ScreenError,
    },
}

/// A screen file: its version and its root node, a `screen` (a modal scrim
/// and its card, as [`widgets::screen`](super::widgets::screen) builds) or a
/// bare `root` on a layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenFile {
    /// [`SCREEN_VERSION`].
    pub version: u32,
    pub(crate) root: Node,
}

/// What a control does when used, its `on` field: close the screen, open
/// another, run an action the game registered or write a setting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenAction {
    /// `"pop"`: closes the screen.
    Pop,
    /// `{"push": "<id>"}`: opens the screen with this `ui_screens` ID or
    /// registered name.
    Push(String),
    /// `{"action": "<name>"}`: runs the action registered under this name.
    #[serde(rename = "action")]
    Named(String),
    /// `{"setting": "<name>"}`: writes this setting from the control's
    /// value.
    Setting(ScreenSetting),
}

/// The settings a toggle or slider row writes: `vsync` from a toggle, the
/// volumes (0 to 1) and the UI and text scales (factors) from a slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenSetting {
    /// Vertical sync.
    Vsync,
    /// The master volume.
    MasterVolume,
    /// The music bus's volume.
    MusicVolume,
    /// The effects bus's volume.
    EffectsVolume,
    /// The UI scale.
    UiScale,
    /// The text scale.
    TextScale,
}

impl ScreenSetting {
    /// Every setting, in the order the docs list them.
    pub const ALL: [Self; 6] = [
        Self::Vsync,
        Self::MasterVolume,
        Self::MusicVolume,
        Self::EffectsVolume,
        Self::UiScale,
        Self::TextScale,
    ];

    /// The name a file gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vsync => "vsync",
            Self::MasterVolume => "master_volume",
            Self::MusicVolume => "music_volume",
            Self::EffectsVolume => "effects_volume",
            Self::UiScale => "ui_scale",
            Self::TextScale => "text_scale",
        }
    }

    /// Whether a toggle writes it; a slider writes the others.
    #[must_use]
    pub const fn is_toggle(self) -> bool {
        matches!(self, Self::Vsync)
    }
}

/// A node's kind: a root (`screen`, `root`), a layout node, text, a control,
/// a meter, a mount the game fills or a repeat of rows from a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Screen,
    Root,
    Group,
    Row,
    Column,
    Panel,
    Label,
    Heading,
    Display,
    Note,
    Small,
    Prompt,
    Button,
    ChoiceRow,
    ToggleRow,
    SliderRow,
    TabRail,
    Meter,
    Mount,
    Repeat,
}

/// The fields every kind takes.
const COMMON: [&str; 8] = [
    "kind",
    "key",
    "layout",
    "overrides",
    "style",
    "mono",
    "danger",
    "bind",
];

impl Kind {
    const ALL: [Self; 20] = [
        Self::Screen,
        Self::Root,
        Self::Group,
        Self::Row,
        Self::Column,
        Self::Panel,
        Self::Label,
        Self::Heading,
        Self::Display,
        Self::Note,
        Self::Small,
        Self::Prompt,
        Self::Button,
        Self::ChoiceRow,
        Self::ToggleRow,
        Self::SliderRow,
        Self::TabRail,
        Self::Meter,
        Self::Mount,
        Self::Repeat,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Root => "root",
            Self::Group => "group",
            Self::Row => "row",
            Self::Column => "column",
            Self::Panel => "panel",
            Self::Label => "label",
            Self::Heading => "heading",
            Self::Display => "display",
            Self::Note => "note",
            Self::Small => "small",
            Self::Prompt => "prompt",
            Self::Button => "button",
            Self::ChoiceRow => "choice_row",
            Self::ToggleRow => "toggle_row",
            Self::SliderRow => "slider_row",
            Self::TabRail => "tab_rail",
            Self::Meter => "meter",
            Self::Mount => "mount",
            Self::Repeat => "repeat",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The fields this kind takes beside [`COMMON`]: the table both the
    /// structural check and the schema read.
    const fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Screen => &["width", "children", "card", "over"],
            Self::Root => &["layer", "modal", "children"],
            Self::Group | Self::Row | Self::Column | Self::Panel => &["children"],
            Self::Label | Self::Display | Self::Note | Self::Small => &["text", "field"],
            Self::Heading => &["text", "field", "centred"],
            Self::Prompt => &["template"],
            Self::Button => &["text", "field", "children", "on"],
            Self::ChoiceRow => &["caption", "text", "field", "on"],
            Self::ToggleRow => &["caption", "on"],
            Self::SliderRow => &["caption", "range", "readout", "on"],
            Self::TabRail => &["tabs"],
            Self::Meter => &["width", "height", "color"],
            Self::Mount => &["mount"],
            Self::Repeat => &["rows", "row"],
        }
    }

    /// The fields this kind needs.
    const fn required(self) -> &'static [&'static str] {
        match self {
            Self::Screen => &["width"],
            Self::Meter => &["width", "height"],
            Self::Root => &["layer"],
            Self::Display => &["style"],
            Self::Prompt => &["template"],
            Self::ChoiceRow | Self::ToggleRow => &["caption"],
            Self::SliderRow => &["caption", "range"],
            Self::TabRail => &["tabs"],
            Self::Mount => &["mount"],
            Self::Repeat => &["key", "rows", "row"],
            _ => &[],
        }
    }

    const fn is_root(self) -> bool {
        matches!(self, Self::Screen | Self::Root)
    }

    /// Whether it holds text from exactly one of `text`, `bind.text` and
    /// `field`.
    const fn holds_text(self) -> bool {
        matches!(
            self,
            Self::Label | Self::Heading | Self::Display | Self::Note | Self::Small | Self::Button
        )
    }

    /// Whether `bind.<binding>` writes to it; `visible` and `enabled` write
    /// to every kind.
    fn binds(self, binding: &str) -> bool {
        match binding {
            "text" => self.holds_text() || self == Self::ChoiceRow,
            "value" => matches!(self, Self::ToggleRow | Self::SliderRow | Self::TabRail),
            "fill" => self == Self::Meter,
            _ => true,
        }
    }
}

/// A node of a screen file. The kind decides which fields it takes
/// ([`Kind::fields`]); every other field defaults.
#[allow(clippy::struct_excessive_bools)] // Independent flags of different kinds; no two are a state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Node {
    pub(crate) kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) layout: Option<LayoutFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) overrides: Option<OverridesFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) style: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) mono: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) danger: bool,
    #[serde(default, skip_serializing_if = "Bind::is_empty")]
    pub(crate) bind: Bind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) on: Option<ScreenAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) field: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) children: Vec<Node>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) width: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) height: Option<Length>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) card: Option<CardFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) over: Vec<Node>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) layer: Option<LayerName>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) modal: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) centred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) range: Option<RangeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) readout: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tabs: Vec<TabFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) rows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) row: Option<Box<Node>>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference.
fn is_false(value: &bool) -> bool {
    !*value
}

/// A node's bindings by kind, each a name the game registers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Bind {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) visible: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) enabled: Option<String>,
}

impl Bind {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Each binding set, by its field's name.
    fn named(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("text", &self.text),
            ("value", &self.value),
            ("fill", &self.fill),
            ("visible", &self.visible),
            ("enabled", &self.enabled),
        ]
        .into_iter()
        .filter_map(|(field, name)| name.as_deref().map(|name| (field, name)))
    }
}

/// A `screen`'s card: its layout and overrides over the builder's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CardFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) layout: Option<LayoutFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overrides: Option<OverridesFile>,
}

/// A bare root's layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LayerName {
    Game,
    GamePopup,
}

/// A slider row's range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RangeFile {
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) step: f32,
}

/// A tab rail's tab: its key and caption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TabFile {
    pub(crate) key: String,
    pub(crate) caption: String,
}

/// A layout's fields over the builder's layout; each unset one keeps it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LayoutFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) direction: Option<DirectionName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) width: Option<SizingFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) height: Option<SizingFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) min_width: Option<LimitFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) min_height: Option<LimitFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_width: Option<LimitFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_height: Option<LimitFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) padding: Option<EdgesFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) margin: Option<EdgesFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) gap: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) align_items: Option<AlignName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) align_self: Option<AlignName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) justify_content: Option<JustifyName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) anchor: Option<AnchorFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DirectionName {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AlignName {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JustifyName {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// An anchor out of the parent's flow: the corner and an `[x, y]` offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnchorFile {
    pub(crate) corner: CornerName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) offset: Option<[Length; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CornerName {
    TopStart,
    TopCenter,
    TopEnd,
    CenterStart,
    Center,
    CenterEnd,
    BottomStart,
    BottomCenter,
    BottomEnd,
}

/// A spacing token's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spacing {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl Spacing {
    const ALL: [Self; 5] = [Self::Xs, Self::Sm, Self::Md, Self::Lg, Self::Xl];

    const fn name(self) -> &'static str {
        match self {
            Self::Xs => "spacing.xs",
            Self::Sm => "spacing.sm",
            Self::Md => "spacing.md",
            Self::Lg => "spacing.lg",
            Self::Xl => "spacing.xl",
        }
    }
}

/// A length: UI units, a size name (the theme's `sizes` over the widgets'
/// defaults) or a spacing token.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Length {
    Units(f32),
    Size(String),
    Spacing(Spacing),
}

impl Length {
    /// A name: a spacing token's, else a size's.
    fn named(name: &str) -> Result<Self, String> {
        if name.starts_with("spacing.") {
            let spacing = Spacing::ALL.into_iter().find(|each| each.name() == name);
            return spacing.map(Self::Spacing).ok_or_else(|| {
                format!(
                    "'{name}' is not one of {}",
                    Spacing::ALL.map(Spacing::name).join(", ")
                )
            });
        }
        if name.is_empty() {
            return Err("a size name is not empty".into());
        }
        Ok(Self::Size(name.to_owned()))
    }

    /// The size name it gives, if any.
    fn size(&self) -> Option<&str> {
        match self {
            Self::Size(name) => Some(name),
            _ => None,
        }
    }
}

/// `value` as a finite `f32`.
fn units(value: f64) -> Result<f32, String> {
    let units = value as f32;
    if units.is_finite() {
        Ok(units)
    } else {
        Err(format!("{value} is not a finite number"))
    }
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Units(units) => serializer.serialize_f32(*units),
            Self::Size(name) => serializer.serialize_str(name),
            Self::Spacing(spacing) => serializer.serialize_str(spacing.name()),
        }
    }
}

/// Reads a number or a string through `number` and `text`, as the length
/// forms all do.
struct Forms<T> {
    expecting: &'static str,
    number: fn(f64) -> Result<T, String>,
    text: fn(&str) -> Result<T, String>,
}

impl<T> Visitor<'_> for Forms<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.expecting)
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<T, E> {
        (self.number)(value).map_err(E::custom)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<T, E> {
        self.visit_f64(value as f64)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<T, E> {
        self.visit_f64(value as f64)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<T, E> {
        (self.text)(value).map_err(E::custom)
    }
}

const LENGTH: &str = "a number, a size name or spacing.xs to spacing.xl";

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Forms {
            expecting: LENGTH,
            number: |value| units(value).map(Self::Units),
            text: Self::named,
        })
    }
}

/// An axis's size: a length, `auto`, `fill` or `<n>%`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SizingFile {
    Auto,
    Fill,
    Percent(f32),
    Length(Length),
}

impl SizingFile {
    fn named(text: &str) -> Result<Self, String> {
        Ok(match text {
            "auto" => Self::Auto,
            "fill" => Self::Fill,
            _ => match text.strip_suffix('%') {
                Some(number) => {
                    let percent = number.parse::<f32>().ok().filter(|p| p.is_finite());
                    let percent = percent.filter(|p| *p >= 0.0);
                    Self::Percent(percent.ok_or_else(|| format!("'{text}' is not a percentage"))?)
                }
                None => Self::Length(Length::named(text)?),
            },
        })
    }
}

impl Serialize for SizingFile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Fill => serializer.serialize_str("fill"),
            Self::Percent(percent) => serializer.serialize_str(&format!("{percent}%")),
            Self::Length(length) => length.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SizingFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Forms {
            expecting: "a length, auto, fill or a percentage",
            number: |value| units(value).map(|units| Self::Length(Length::Units(units))),
            text: Self::named,
        })
    }
}

/// A `min_*` or `max_*`: a length or `none`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LimitFile {
    None,
    Length(Length),
}

impl Serialize for LimitFile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::None => serializer.serialize_str("none"),
            Self::Length(length) => length.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for LimitFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Forms {
            expecting: "a length or none",
            number: |value| units(value).map(|units| Self::Length(Length::Units(units))),
            text: |text| match text {
                "none" => Ok(Self::None),
                _ => Length::named(text).map(Self::Length),
            },
        })
    }
}

/// Edges: one length for every side, or the sides by name, `x` and `y`
/// setting two at once and a named side over them; an unset side keeps the
/// builder's.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EdgesFile {
    All(Length),
    Sides(Box<SidesFile>),
}

/// Edges' sides by name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SidesFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) top: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) right: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bottom: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) left: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) x: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) y: Option<Length>,
}

impl SidesFile {
    fn lengths(&self) -> impl Iterator<Item = (&'static str, &Length)> {
        [
            ("top", &self.top),
            ("right", &self.right),
            ("bottom", &self.bottom),
            ("left", &self.left),
            ("x", &self.x),
            ("y", &self.y),
        ]
        .into_iter()
        .filter_map(|(side, length)| length.as_ref().map(|length| (side, length)))
    }
}

impl Serialize for EdgesFile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All(length) => length.serialize(serializer),
            Self::Sides(sides) => sides.serialize(serializer),
        }
    }
}

struct EdgesVisitor;

impl<'de> Visitor<'de> for EdgesVisitor {
    type Value = EdgesFile;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a length, or an object of top, right, bottom, left, x and y")
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<EdgesFile, E> {
        units(value)
            .map(|units| EdgesFile::All(Length::Units(units)))
            .map_err(E::custom)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<EdgesFile, E> {
        self.visit_f64(value as f64)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<EdgesFile, E> {
        self.visit_f64(value as f64)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<EdgesFile, E> {
        Length::named(value).map(EdgesFile::All).map_err(E::custom)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<EdgesFile, A::Error> {
        let sides = SidesFile::deserialize(de::value::MapAccessDeserializer::new(map))?;
        Ok(EdgesFile::Sides(Box::new(sides)))
    }
}

impl<'de> Deserialize<'de> for EdgesFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(EdgesVisitor)
    }
}

impl LayoutFile {
    /// Each size name the layout gives, with its field's path under it.
    fn sizes(&self) -> Vec<(String, &str)> {
        fn length<'a>(out: &mut Vec<(String, &'a str)>, field: String, length: &'a Length) {
            if let Some(name) = length.size() {
                out.push((field, name));
            }
        }
        let mut out = Vec::new();
        for (field, sizing) in [("width", &self.width), ("height", &self.height)] {
            if let Some(SizingFile::Length(each)) = sizing {
                length(&mut out, field.into(), each);
            }
        }
        for (field, limit) in [
            ("min_width", &self.min_width),
            ("min_height", &self.min_height),
            ("max_width", &self.max_width),
            ("max_height", &self.max_height),
        ] {
            if let Some(LimitFile::Length(each)) = limit {
                length(&mut out, field.into(), each);
            }
        }
        for (field, edges) in [("padding", &self.padding), ("margin", &self.margin)] {
            match edges {
                Some(EdgesFile::All(each)) => length(&mut out, field.into(), each),
                Some(EdgesFile::Sides(sides)) => {
                    for (side, each) in sides.lengths() {
                        length(&mut out, format!("{field}.{side}"), each);
                    }
                }
                None => {}
            }
        }
        if let Some(gap) = &self.gap {
            length(&mut out, "gap".into(), gap);
        }
        if let Some(offset) = self
            .anchor
            .as_ref()
            .and_then(|anchor| anchor.offset.as_ref())
        {
            for (at, each) in offset.iter().enumerate() {
                length(&mut out, format!("anchor.offset[{at}]"), each);
            }
        }
        out
    }
}

impl ScreenFile {
    /// Reads and parses `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ScreenFileError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|source| ScreenFileError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&contents).map_err(|source| ScreenFileError::Parse {
            path: path.display().to_string(),
            source,
        })
    }

    /// Parses screen JSON: `version` first, so a version other than
    /// [`SCREEN_VERSION`] fails before any other field is read; then the
    /// model, an error naming its field's path; then the structural checks
    /// (each kind's fields, keys, text sources, rows), the first that fails
    /// naming its path.
    pub fn from_json(json: &str) -> Result<Self, ScreenError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|err| ScreenError::new("", format!("not JSON: {err}")))?;
        if !value.is_object() {
            return Err(ScreenError::new("", "a screen file is a JSON object"));
        }
        match value.get("version") {
            None => {
                return Err(ScreenError::new(
                    "version",
                    format!("missing; this engine reads version {SCREEN_VERSION}"),
                ));
            }
            Some(Value::Number(n)) if n.as_u64() == Some(u64::from(SCREEN_VERSION)) => {}
            Some(other) => {
                return Err(ScreenError::new(
                    "version",
                    format!(
                        "version {other} is not supported; this engine reads version {SCREEN_VERSION}"
                    ),
                ));
            }
        }
        let file: Self = serde_path_to_error::deserialize(&value).map_err(|err| {
            let field = err.path().to_string();
            let field = if field == "." { String::new() } else { field };
            ScreenError::new(field, err.into_inner().to_string())
        })?;
        if let Some(root) = value.get("root") {
            kind_fields(root, "root")?;
        }
        Checker::default().node(&file.root, "root", Place::Root)?;
        Ok(file)
    }

    /// The file in its canonical form: the model as JSON with defaults left
    /// out, keys sorted, arrays in order, two-space indents, whole numbers
    /// without a fraction and others in the shortest form that reads back
    /// as the same number, and a final newline. It reads back as the same
    /// file, and is its own canonical form.
    #[must_use]
    pub fn to_canonical(&self) -> String {
        // Through text first: a model number is an `f32`, which serde_json
        // writes in its shortest form, and which reads back as that decimal.
        let text = serde_json::to_string(self).unwrap_or_default();
        let value: Value = serde_json::from_str(&text).unwrap_or_default();
        let mut out = String::new();
        write_canonical(&value, 0, &mut out);
        out.push('\n');
        out
    }

    /// Each name the file uses that `known` lacks, with its field's path:
    /// bindings, providers, mounts and actions the game has not registered,
    /// text styles and sizes no theme names, and screens a `push` cannot
    /// open. Empty when every name is known.
    #[must_use]
    pub fn check(&self, known: &ScreenNames) -> Vec<ScreenError> {
        let mut issues = Vec::new();
        self.walk(&mut |path, name, use_| {
            let (set, what) = match use_ {
                Use::Binding("text") => (&known.text, "text binding"),
                Use::Binding("value") => (&known.value, "value binding"),
                Use::Binding("fill") => (&known.fill, "fill binding"),
                Use::Binding("visible") => (&known.visible, "visible binding"),
                Use::Binding(_) => (&known.enabled, "enabled binding"),
                Use::Rows => (&known.rows, "row provider"),
                Use::Mount => (&known.mounts, "mount"),
                Use::Action => (&known.actions, "action"),
                Use::Style => (&known.styles, "text style"),
                Use::Size => (&known.sizes, "size"),
                Use::Push => {
                    if known.refused.contains(name) {
                        issues.push(ScreenError::new(
                            path,
                            format!(
                                "screen '{name}' is a file a data screen refuses: its root is not modal on the popup layer"
                            ),
                        ));
                        return;
                    }
                    (&known.screens, "screen to push")
                }
            };
            if !set.contains(name) {
                issues.push(ScreenError::new(path, format!("no {what} '{name}'")));
            }
        });
        issues
    }

    /// The names the file uses, by kind; `refused` stays empty.
    #[must_use]
    pub fn uses(&self) -> ScreenNames {
        let mut names = ScreenNames::default();
        self.walk(&mut |_, name, use_| {
            let set = match use_ {
                Use::Binding("text") => &mut names.text,
                Use::Binding("value") => &mut names.value,
                Use::Binding("fill") => &mut names.fill,
                Use::Binding("visible") => &mut names.visible,
                Use::Binding(_) => &mut names.enabled,
                Use::Rows => &mut names.rows,
                Use::Mount => &mut names.mounts,
                Use::Action => &mut names.actions,
                Use::Style => &mut names.styles,
                Use::Size => &mut names.sizes,
                Use::Push => &mut names.screens,
            };
            set.insert(name.to_owned());
        });
        names
    }

    /// The file's keys in file order, tab keys included; the keys inside a
    /// repeat's row, which its rows qualify, are left out.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        visit(&self.root, "root", false, &mut |node, _, in_row| {
            if in_row {
                return;
            }
            keys.extend(node.key.clone());
            keys.extend(node.tabs.iter().map(|tab| tab.key.clone()));
        });
        keys
    }

    /// Why a data screen refuses to open the file, `None` when it opens
    /// it: a data screen's root is a `screen`, or a `root` that is modal on
    /// the popup layer, since screens close front-most first.
    #[must_use]
    pub fn data_screen_refusal(&self) -> Option<&'static str> {
        let root = &self.root;
        let popup = root.layer == Some(LayerName::GamePopup) && root.modal;
        (root.kind == Kind::Root && !popup).then_some("its root is not modal on the popup layer")
    }

    /// Calls `each` with every name the file uses, its field's path and its
    /// use, in file order.
    fn walk(&self, each: &mut dyn FnMut(String, &str, Use)) {
        visit(&self.root, "root", false, &mut |node, path, _| {
            if let Some(style) = &node.style {
                each(format!("{path}.style"), style, Use::Style);
            }
            for (field, name) in node.bind.named() {
                each(format!("{path}.bind.{field}"), name, Use::Binding(field));
            }
            if let Some(readout) = &node.readout {
                each(format!("{path}.readout"), readout, Use::Binding("text"));
            }
            if let Some(rows) = &node.rows {
                each(format!("{path}.rows"), rows, Use::Rows);
            }
            if let Some(mount) = &node.mount {
                each(format!("{path}.mount"), mount, Use::Mount);
            }
            match &node.on {
                Some(ScreenAction::Named(name)) => {
                    each(format!("{path}.on.action"), name, Use::Action);
                }
                Some(ScreenAction::Push(id)) => each(format!("{path}.on.push"), id, Use::Push),
                _ => {}
            }
            for (field, length) in [("width", &node.width), ("height", &node.height)] {
                if let Some(name) = length.as_ref().and_then(Length::size) {
                    each(format!("{path}.{field}"), name, Use::Size);
                }
            }
            let layouts = [
                ("layout", node.layout.as_ref()),
                (
                    "card.layout",
                    node.card.as_ref().and_then(|card| card.layout.as_ref()),
                ),
            ];
            for (field, layout) in layouts {
                for (sub, name) in layout.map(LayoutFile::sizes).unwrap_or_default() {
                    each(format!("{path}.{field}.{sub}"), name, Use::Size);
                }
            }
        });
    }
}

/// How a file uses a name.
#[derive(Debug, Clone, Copy)]
enum Use {
    Binding(&'static str),
    Rows,
    Mount,
    Action,
    Style,
    Size,
    Push,
}

/// Calls `each` with every node under `node`, `node` included, its path and
/// whether it sits inside a repeat's row, in file order.
fn visit<'a>(node: &'a Node, path: &str, in_row: bool, each: &mut dyn FnMut(&'a Node, &str, bool)) {
    each(node, path, in_row);
    for (list, nodes) in [("children", &node.children), ("over", &node.over)] {
        for (at, child) in nodes.iter().enumerate() {
            visit(child, &format!("{path}.{list}[{at}]"), in_row, each);
        }
    }
    if let Some(row) = &node.row {
        visit(row, &format!("{path}.row"), true, each);
    }
}

/// Fails the first field a node's kind does not take, with its path, in
/// the JSON as written, so a field set to its default is caught too.
fn kind_fields(node: &Value, path: &str) -> Result<(), ScreenError> {
    let Some(object) = node.as_object() else {
        return Ok(());
    };
    let Some(kind) = object
        .get("kind")
        .and_then(Value::as_str)
        .and_then(Kind::named)
    else {
        return Ok(());
    };
    for field in object.keys() {
        if !COMMON.contains(&field.as_str()) && !kind.fields().contains(&field.as_str()) {
            return Err(ScreenError::new(
                format!("{path}.{field}"),
                format!("a {} takes no {field}", kind.name()),
            ));
        }
    }
    for list in ["children", "over"] {
        if let Some(Value::Array(nodes)) = object.get(list) {
            for (at, child) in nodes.iter().enumerate() {
                kind_fields(child, &format!("{path}.{list}[{at}]"))?;
            }
        }
    }
    match object.get("row") {
        Some(row) => kind_fields(row, &format!("{path}.row")),
        None => Ok(()),
    }
}

/// Where a node sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Root,
    Child,
    InRow,
}

/// The structural checks: keys unique in the file (a repeat row's within
/// the row), each kind's required fields, text sources and bindings, and the
/// values the model alone cannot hold to.
#[derive(Default)]
struct Checker {
    keys: HashSet<String>,
    row_keys: HashSet<String>,
}

impl Checker {
    fn node(&mut self, node: &Node, path: &str, place: Place) -> Result<(), ScreenError> {
        let kind = node.kind;
        let fail = |field: &str, message: String| {
            Err(ScreenError::new(format!("{path}.{field}"), message))
        };
        if place == Place::Root && !kind.is_root() {
            return fail("kind", "a file's root is a screen or a root".into());
        }
        if place != Place::Root && kind.is_root() {
            return fail("kind", format!("a {} is only a file's root", kind.name()));
        }
        if place == Place::InRow && kind == Kind::Repeat {
            return fail("kind", "a repeat cannot sit inside a repeat's row".into());
        }
        for field in kind.required() {
            if !node.has(field) {
                return fail(field, format!("a {} needs its {field}", kind.name()));
            }
        }
        if let Some(key) = &node.key {
            self.key(key, &format!("{path}.key"), place)?;
        }
        let sources = [
            node.text.is_some(),
            node.bind.text.is_some(),
            node.field.is_some(),
        ];
        let sources = sources.into_iter().filter(|set| *set).count();
        if (kind.holds_text() && sources != 1) || (kind == Kind::ChoiceRow && sources > 1) {
            let exactly = if kind.holds_text() {
                "exactly"
            } else {
                "at most"
            };
            return fail(
                "text",
                format!(
                    "a {} takes {exactly} one of text, bind.text and field",
                    kind.name()
                ),
            );
        }
        if node.field.is_some() && place != Place::InRow {
            return fail("field", "a field is read only inside a repeat's row".into());
        }
        for (binding, name) in node.bind.named() {
            if !kind.binds(binding) {
                return fail(
                    &format!("bind.{binding}"),
                    format!("a {} takes no {binding} binding", kind.name()),
                );
            }
            not_empty(name, &format!("{path}.bind.{binding}"))?;
        }
        for (field, name) in [
            ("style", &node.style),
            ("field", &node.field),
            ("template", &node.template),
            ("readout", &node.readout),
            ("mount", &node.mount),
            ("rows", &node.rows),
        ] {
            if let Some(name) = name {
                not_empty(name, &format!("{path}.{field}"))?;
            }
        }
        action_check(node, path)?;
        if let Some(range) = node.range {
            range_check(range, &format!("{path}.range"))?;
        }
        for (at, tab) in node.tabs.iter().enumerate() {
            self.key(&tab.key, &format!("{path}.tabs[{at}].key"), place)?;
        }
        for (field, overrides) in [
            ("overrides", node.overrides.as_ref()),
            (
                "card.overrides",
                node.card.as_ref().and_then(|card| card.overrides.as_ref()),
            ),
        ] {
            if let Some(overrides) = overrides {
                overrides_check(overrides, &format!("{path}.{field}"))?;
            }
        }
        if let Some(color) = &node.color {
            color_check(color, &format!("{path}.color"))?;
        }
        let child = if place == Place::Root {
            Place::Child
        } else {
            place
        };
        for (list, nodes) in [("children", &node.children), ("over", &node.over)] {
            for (at, each) in nodes.iter().enumerate() {
                self.node(each, &format!("{path}.{list}[{at}]"), child)?;
            }
        }
        if let Some(row) = &node.row {
            self.row_keys.clear();
            self.node(row, &format!("{path}.row"), Place::InRow)?;
        }
        Ok(())
    }

    /// A key: a name without a `/`, unique in the file, or within its row
    /// inside a repeat's row.
    fn key(&mut self, key: &str, path: &str, place: Place) -> Result<(), ScreenError> {
        not_empty(key, path)?;
        if key.contains('/') {
            return Err(ScreenError::new(
                path,
                format!("key '{key}' holds a '/', which only the keys a repeat builds hold"),
            ));
        }
        let (keys, scope) = match place {
            Place::InRow => (&mut self.row_keys, "this repeat's row"),
            _ => (&mut self.keys, "this file"),
        };
        if !keys.insert(key.to_owned()) {
            return Err(ScreenError::new(
                path,
                format!("key '{key}' is used twice in {scope}"),
            ));
        }
        Ok(())
    }
}

/// An `on` action: a setting only on the row kind that writes it, and names
/// that are not empty.
fn action_check(node: &Node, path: &str) -> Result<(), ScreenError> {
    match &node.on {
        Some(ScreenAction::Setting(setting)) => {
            let fits = match node.kind {
                Kind::ToggleRow => setting.is_toggle(),
                Kind::SliderRow => !setting.is_toggle(),
                _ => false,
            };
            if !fits {
                let row = if setting.is_toggle() {
                    "toggle_row"
                } else {
                    "slider_row"
                };
                return Err(ScreenError::new(
                    format!("{path}.on.setting"),
                    format!("setting '{}' is written by a {row}", setting.name()),
                ));
            }
            Ok(())
        }
        Some(ScreenAction::Push(id)) => not_empty(id, &format!("{path}.on.push")),
        Some(ScreenAction::Named(name)) => not_empty(name, &format!("{path}.on.action")),
        Some(ScreenAction::Pop) | None => Ok(()),
    }
}

impl Node {
    /// Whether `field` is set.
    fn has(&self, field: &str) -> bool {
        match field {
            "width" => self.width.is_some(),
            "height" => self.height.is_some(),
            "layer" => self.layer.is_some(),
            "style" => self.style.is_some(),
            "template" => self.template.is_some(),
            "caption" => self.caption.is_some(),
            "range" => self.range.is_some(),
            "tabs" => !self.tabs.is_empty(),
            "mount" => self.mount.is_some(),
            "key" => self.key.is_some(),
            "rows" => self.rows.is_some(),
            "row" => self.row.is_some(),
            _ => true,
        }
    }
}

fn not_empty(name: &str, path: &str) -> Result<(), ScreenError> {
    if name.is_empty() {
        Err(ScreenError::new(path, "a name is not empty"))
    } else {
        Ok(())
    }
}

/// A range: finite, `min` below `max`, `step` above 0.
fn range_check(range: RangeFile, path: &str) -> Result<(), ScreenError> {
    let RangeFile { min, max, step } = range;
    let message = if ![min, max, step].iter().all(|value| value.is_finite()) {
        "a range's numbers are finite".to_owned()
    } else if min >= max {
        format!("min {min} is not below max {max}")
    } else if step <= 0.0 {
        format!("step {step} is not above 0")
    } else {
        return Ok(());
    };
    Err(ScreenError::new(path, message))
}

/// A colour's token name, when it gives one, among [`ColorTokens::NAMES`].
fn color_check(color: &Color, path: &str) -> Result<(), ScreenError> {
    match color {
        Color::Token(name) if !ColorTokens::NAMES.contains(&name.as_str()) => {
            Err(ScreenError::new(
                path,
                format!(
                    "colour '{name}' is not #rrggbb, #rrggbbaa or a colour token's name ({})",
                    ColorTokens::NAMES.join(", ")
                ),
            ))
        }
        _ => Ok(()),
    }
}

/// An overrides object: its colours' token names known, and no field only a
/// theme's named styles take.
fn overrides_check(overrides: &OverridesFile, path: &str) -> Result<(), ScreenError> {
    for (field, color) in [
        ("background", &overrides.background),
        ("border", &overrides.border),
        ("text.color", &overrides.text.color),
    ] {
        if let Some(color) = color {
            color_check(color, &format!("{path}.{field}"))?;
        }
    }
    if let Some(field) = TextFile::named_only(&overrides.text) {
        return Err(ScreenError::new(
            format!("{path}.text.{field}"),
            format!("only a theme's named styles take {field}"),
        ));
    }
    Ok(())
}

/// Writes `value` canonically at `indent` spaces.
fn write_canonical(value: &Value, indent: usize, out: &mut String) {
    let pad = |out: &mut String, indent: usize| out.extend(std::iter::repeat_n(' ', indent));
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(on) => out.push_str(if *on { "true" } else { "false" }),
        Value::Number(number) => out.push_str(&canonical_number(number)),
        Value::String(text) => out.push_str(&Value::String(text.clone()).to_string()),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push_str("[\n");
            for (at, item) in items.iter().enumerate() {
                pad(out, indent + 2);
                write_canonical(item, indent + 2, out);
                out.push_str(if at + 1 < items.len() { ",\n" } else { "\n" });
            }
            pad(out, indent);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push_str("{\n");
            for (at, key) in keys.iter().enumerate() {
                pad(out, indent + 2);
                out.push_str(&Value::String((*key).clone()).to_string());
                out.push_str(": ");
                write_canonical(&map[*key], indent + 2, out);
                out.push_str(if at + 1 < keys.len() { ",\n" } else { "\n" });
            }
            pad(out, indent);
            out.push('}');
        }
    }
}

/// A whole number without a fraction, any other in the shortest form that
/// reads back as the same `f64`.
fn canonical_number(number: &serde_json::Number) -> String {
    if number.is_i64() || number.is_u64() {
        return number.to_string();
    }
    let value = number.as_f64().unwrap_or_default();
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Names a screen file may use, by kind: what [`ScreenFile::check`] holds a
/// file to, and what [`ScreenFile::uses`] reports a file using.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScreenNames {
    /// Text bindings: `bind.text` and slider rows' `readout`.
    pub text: BTreeSet<String>,
    /// Value bindings: `bind.value`.
    pub value: BTreeSet<String>,
    /// Meter fill bindings: `bind.fill`.
    pub fill: BTreeSet<String>,
    /// Visibility bindings: `bind.visible`.
    pub visible: BTreeSet<String>,
    /// Enabled-state bindings: `bind.enabled`.
    pub enabled: BTreeSet<String>,
    /// Row providers: a repeat's `rows`.
    pub rows: BTreeSet<String>,
    /// Mounts: a mount node's `mount`.
    pub mounts: BTreeSet<String>,
    /// Actions: `on`'s `action`.
    pub actions: BTreeSet<String>,
    /// Text styles: built in or in a loaded theme's `text_styles`.
    pub styles: BTreeSet<String>,
    /// Size names: a loaded theme's `sizes` or the widgets' defaults.
    pub sizes: BTreeSet<String>,
    /// Screens a `push` opens: `ui_screens` IDs and registered screens.
    pub screens: BTreeSet<String>,
    /// `ui_screens` IDs a data screen refuses to open, which no `push` may
    /// name; [`ScreenFile::uses`] leaves it empty.
    pub refused: BTreeSet<String>,
}

/// The loaded `ui_screens` by ID: a `World` resource the asset loader fills,
/// only when a manifest lists screens, and a reload replaces entries in.
#[derive(Debug, Default)]
pub struct UiScreenRegistry {
    screens: BTreeMap<String, ScreenFile>,
    interned: HashSet<&'static str>,
}

impl UiScreenRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `file` under `id`, returning the file it replaced.
    pub fn insert(&mut self, id: impl Into<String>, file: ScreenFile) -> Option<ScreenFile> {
        self.screens.insert(id.into(), file)
    }

    /// The file loaded as `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ScreenFile> {
        self.screens.get(id)
    }

    /// Whether `id` is loaded.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.screens.contains_key(id)
    }

    /// The loaded IDs, sorted.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.screens.keys().map(String::as_str).collect()
    }

    /// How many files are loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.screens.len()
    }

    /// Whether no file is loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    /// The loaded ID `id` as a `&'static str`, a data screen's state ID:
    /// each ID leaks once, so the set is bounded by the manifests' entries.
    /// `None` when `id` is not loaded.
    pub fn intern(&mut self, id: &str) -> Option<&'static str> {
        if !self.contains(id) {
            return None;
        }
        if let Some(&interned) = self.interned.get(id) {
            return Some(interned);
        }
        let interned: &'static str = Box::leak(id.to_owned().into_boxed_str());
        self.interned.insert(interned);
        Some(interned)
    }
}

/// The JSON Schema (draft 2020-12) of a screen file: every object closed
/// (`additionalProperties: false`), one `$defs` entry per node kind chosen
/// by its `kind`, the overrides and text objects shared with the theme
/// file's schema, enums for kinds, layers, alignments, corners and
/// settings, and the game's names as strings.
#[must_use]
pub fn json_schema() -> Value {
    let mut defs = shared_schema_defs();
    let reference = |name: &str| json!({ "$ref": format!("#/$defs/{name}") });
    let children = Kind::ALL.into_iter().filter(|kind| !kind.is_root());
    defs.insert(
        "root_node".into(),
        json!({ "oneOf": [reference("screen"), reference("root")] }),
    );
    defs.insert(
        "node".into(),
        json!({ "oneOf": children.map(|kind| reference(kind.name())).collect::<Vec<_>>() }),
    );
    for kind in Kind::ALL {
        let mut properties = Map::new();
        properties.insert("kind".into(), json!({ "const": kind.name() }));
        for field in COMMON.iter().skip(1).chain(kind.fields()) {
            properties.insert((*field).into(), field_schema(field));
        }
        let mut required = vec!["kind"];
        required.extend(kind.required());
        defs.insert(
            kind.name().into(),
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": required,
                "properties": properties,
            }),
        );
    }
    let name = json!({ "type": "string", "minLength": 1 });
    let length = json!({
        "anyOf": [
            { "type": "number" },
            { "enum": Spacing::ALL.map(Spacing::name) },
            name,
        ],
    });
    defs.insert("length".into(), length);
    defs.insert(
        "sizing".into(),
        json!({ "anyOf": [
            reference("length"),
            { "enum": ["auto", "fill"] },
            { "type": "string", "pattern": "^[0-9]+(\\.[0-9]+)?%$" },
        ] }),
    );
    defs.insert(
        "limit".into(),
        json!({ "anyOf": [reference("length"), { "const": "none" }] }),
    );
    let sides =
        ["top", "right", "bottom", "left", "x", "y"].map(|side| (side, reference("length")));
    defs.insert(
        "edges".into(),
        json!({ "anyOf": [reference("length"), closed(&sides, &[])] }),
    );
    let aligns = json!({ "enum": ["start", "center", "end", "stretch"] });
    defs.insert(
        "layout".into(),
        closed(
            &[
                ("direction", json!({ "enum": ["row", "column"] })),
                ("width", reference("sizing")),
                ("height", reference("sizing")),
                ("min_width", reference("limit")),
                ("min_height", reference("limit")),
                ("max_width", reference("limit")),
                ("max_height", reference("limit")),
                ("padding", reference("edges")),
                ("margin", reference("edges")),
                ("gap", reference("length")),
                ("align_items", aligns.clone()),
                ("align_self", aligns),
                (
                    "justify_content",
                    json!({ "enum": ["start", "center", "end", "space_between", "space_around", "space_evenly"] }),
                ),
                ("anchor", reference("anchor")),
            ],
            &[],
        ),
    );
    let corners = [
        "top_start",
        "top_center",
        "top_end",
        "center_start",
        "center",
        "center_end",
        "bottom_start",
        "bottom_center",
        "bottom_end",
    ];
    defs.insert(
        "anchor".into(),
        closed(
            &[
                ("corner", json!({ "enum": corners })),
                (
                    "offset",
                    json!({ "type": "array", "prefixItems": [reference("length"), reference("length")], "minItems": 2, "maxItems": 2 }),
                ),
            ],
            &["corner"],
        ),
    );
    let bindings = ["text", "value", "fill", "visible", "enabled"].map(|each| (each, name.clone()));
    defs.insert("bind".into(), closed(&bindings, &[]));
    defs.insert(
        "card".into(),
        closed(
            &[
                ("layout", reference("layout")),
                ("overrides", reference("overrides")),
            ],
            &[],
        ),
    );
    let number = json!({ "type": "number" });
    defs.insert(
        "range".into(),
        closed(
            &[
                ("min", number.clone()),
                ("max", number),
                ("step", json!({ "type": "number", "exclusiveMinimum": 0 })),
            ],
            &["min", "max", "step"],
        ),
    );
    defs.insert(
        "tab".into(),
        closed(
            &[
                ("key", json!({ "type": "string", "pattern": "^[^/]+$" })),
                ("caption", json!({ "type": "string" })),
            ],
            &["key", "caption"],
        ),
    );
    let action = |field: &str, value: Value| closed(&[(field, value)], &[field]);
    defs.insert(
        "action".into(),
        json!({ "oneOf": [
            { "const": "pop" },
            action("push", name.clone()),
            action("action", name),
            action("setting", json!({ "enum": ScreenSetting::ALL.map(ScreenSetting::name) })),
        ] }),
    );
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Tungsten UI screen",
        "description": "A screen file, listed in a manifest's ui_screens.",
        "type": "object",
        "additionalProperties": false,
        "required": ["version", "root"],
        "properties": {
            "version": { "const": SCREEN_VERSION },
            "root": reference("root_node"),
        },
        "$defs": defs,
    })
}

/// A node field's schema.
fn field_schema(field: &str) -> Value {
    let reference = |name: &str| json!({ "$ref": format!("#/$defs/{name}") });
    match field {
        "key" => json!({ "type": "string", "pattern": "^[^/]+$" }),
        "style" | "field" | "template" | "readout" | "mount" | "rows" => {
            json!({ "type": "string", "minLength": 1 })
        }
        "text" | "caption" => json!({ "type": "string" }),
        "mono" | "danger" | "modal" | "centred" => json!({ "type": "boolean" }),
        "children" | "over" => json!({ "type": "array", "items": reference("node") }),
        "tabs" => json!({ "type": "array", "minItems": 1, "items": reference("tab") }),
        "width" | "height" => reference("length"),
        "layer" => json!({ "enum": ["game", "game_popup"] }),
        "row" => reference("node"),
        "on" => reference("action"),
        "color" => reference("color"),
        other => reference(other),
    }
}

/// A closed object of `properties`, `required` among them.
pub(crate) fn closed(properties: &[(&str, Value)], required: &[&str]) -> Value {
    let properties: Map<String, Value> = properties
        .iter()
        .map(|(name, schema)| ((*name).to_owned(), schema.clone()))
        .collect();
    let mut object = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
    });
    if !required.is_empty() {
        object["required"] = json!(required);
    }
    object
}

#[cfg(test)]
#[path = "../tests/ui/screen_file.rs"]
mod tests;
