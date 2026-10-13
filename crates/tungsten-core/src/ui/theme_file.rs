//! The theme asset (w01 §14, Q9): a JSON file of optional fields over
//! [`Theme::default`], read into a serde model and mapped onto a [`Theme`].
//! The manifest's `ui_themes` section names the files; [`UiThemeRegistry`]
//! holds the loaded themes by ID for a tree to apply and a reload to
//! replace. Colours are `#rrggbb` or `#rrggbbaa`, authored in sRGB and kept
//! as `[u8; 4]`; the pipelines decode them (Q8).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde::de::{self, Deserializer};
use thiserror::Error;

use crate::text::{EllipsisAt, TextAlign, TextOverflow, TextWrap};

use super::theme::{
    KindStyle, KindStyles, StyleOverrides, TextOverrides, Theme, Tokens, VisualStyle,
};

/// Why a theme file did not load.
#[derive(Debug, Error)]
pub enum ThemeFileError {
    /// The file could not be read.
    #[error("failed to read ui theme '{path}': {source}")]
    Io {
        /// The file.
        path: String,
        /// The read error.
        source: std::io::Error,
    },
    /// The file is not a theme: bad JSON, an unknown field or a bad colour.
    #[error("invalid ui theme '{path}': {source}")]
    Parse {
        /// The file.
        path: String,
        /// The parse error.
        source: serde_json::Error,
    },
}

/// A theme file: every field optional, each `Some` replacing the built-in
/// theme's value. `tokens` holds the token groups; `kinds` holds, per widget
/// kind (`panel`, `label`, `button`, `image`), a `normal` look and the
/// `focused`, `disabled`, `hovered` and `pressed` variants, each merged
/// over the kind's built-in one. An unknown field is an error.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeFile {
    tokens: TokensFile,
    kinds: KindsFile,
}

impl ThemeFile {
    /// Reads and parses `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ThemeFileError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|source| ThemeFileError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_json(&contents).map_err(|source| ThemeFileError::Parse {
            path: path.display().to_string(),
            source,
        })
    }

    /// Parses theme JSON: one object, as the file format describes.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        if !value.is_object() {
            return Err(de::Error::custom("a theme file is a JSON object"));
        }
        serde_json::from_value(value)
    }

    /// The theme: the built-in tokens under this file's, the kind styles
    /// drawn from those tokens, then each named kind's fields over them.
    #[must_use]
    pub fn into_theme(self) -> Theme {
        let mut tokens = Tokens::default();
        self.tokens.apply(&mut tokens);
        let mut kinds = KindStyles::for_tokens(&tokens);
        let KindsFile {
            panel,
            label,
            button,
            image,
        } = self.kinds;
        for (file, kind) in [
            (panel, &mut kinds.panel),
            (label, &mut kinds.label),
            (button, &mut kinds.button),
            (image, &mut kinds.image),
        ] {
            if let Some(file) = file {
                file.apply(kind);
            }
        }
        Theme { tokens, kinds }
    }
}

/// An sRGB colour parsed from `#rrggbb` or `#rrggbbaa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Color([u8; 4]);

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse_color(&text).map(Color).map_err(de::Error::custom)
    }
}

/// `#rrggbb` (opaque) or `#rrggbbaa`, case-insensitive.
pub(crate) fn parse_color(text: &str) -> Result<[u8; 4], String> {
    let invalid = || format!("colour '{text}' is not #rrggbb or #rrggbbaa");
    let hex = text.strip_prefix('#').ok_or_else(invalid)?;
    if hex.len() != 6 && hex.len() != 8 || !hex.is_ascii() {
        return Err(invalid());
    }
    let mut out = [0, 0, 0, 255];
    for (i, pair) in hex.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(pair).map_err(|_| invalid())?;
        out[i] = u8::from_str_radix(pair, 16).map_err(|_| invalid())?;
    }
    Ok(out)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TokensFile {
    colors: ColorsFile,
    spacing: SpacingFile,
    radii: RadiiFile,
    fonts: FontsFile,
    text: TextTokensFile,
}

impl TokensFile {
    fn apply(self, tokens: &mut Tokens) {
        let colors = &mut tokens.colors;
        for (value, slot) in [
            (self.colors.background, &mut colors.background),
            (self.colors.surface, &mut colors.surface),
            (self.colors.border, &mut colors.border),
            (self.colors.text, &mut colors.text),
            (self.colors.text_muted, &mut colors.text_muted),
            (self.colors.accent, &mut colors.accent),
            (self.colors.accent_text, &mut colors.accent_text),
            (self.colors.focus, &mut colors.focus),
            (self.colors.disabled_text, &mut colors.disabled_text),
        ] {
            if let Some(Color(color)) = value {
                *slot = color;
            }
        }
        let spacing = &mut tokens.spacing;
        for (value, slot) in [
            (self.spacing.xs, &mut spacing.xs),
            (self.spacing.sm, &mut spacing.sm),
            (self.spacing.md, &mut spacing.md),
            (self.spacing.lg, &mut spacing.lg),
            (self.spacing.xl, &mut spacing.xl),
        ] {
            if let Some(value) = value {
                *slot = value;
            }
        }
        let radii = &mut tokens.radii;
        for (value, slot) in [
            (self.radii.none, &mut radii.none),
            (self.radii.sm, &mut radii.sm),
            (self.radii.md, &mut radii.md),
            (self.radii.lg, &mut radii.lg),
        ] {
            if let Some(value) = value {
                *slot = value;
            }
        }
        let fonts = &mut tokens.fonts;
        for (value, slot) in [
            (self.fonts.body, &mut fonts.body),
            (self.fonts.heading, &mut fonts.heading),
            (self.fonts.mono, &mut fonts.mono),
        ] {
            if let Some(value) = value {
                *slot = value;
            }
        }
        let text = &mut tokens.text;
        for (value, slot) in [
            (self.text.size, &mut text.size),
            (self.text.line_height, &mut text.line_height),
            (self.text.heading_size, &mut text.heading_size),
        ] {
            if let Some(value) = value {
                *slot = value;
            }
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ColorsFile {
    background: Option<Color>,
    surface: Option<Color>,
    border: Option<Color>,
    text: Option<Color>,
    text_muted: Option<Color>,
    accent: Option<Color>,
    accent_text: Option<Color>,
    focus: Option<Color>,
    disabled_text: Option<Color>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SpacingFile {
    xs: Option<f32>,
    sm: Option<f32>,
    md: Option<f32>,
    lg: Option<f32>,
    xl: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RadiiFile {
    none: Option<f32>,
    sm: Option<f32>,
    md: Option<f32>,
    lg: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FontsFile {
    body: Option<String>,
    heading: Option<String>,
    mono: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TextTokensFile {
    size: Option<f32>,
    line_height: Option<f32>,
    heading_size: Option<f32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct KindsFile {
    panel: Option<KindFile>,
    label: Option<KindFile>,
    button: Option<KindFile>,
    image: Option<KindFile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct KindFile {
    normal: OverridesFile,
    focused: OverridesFile,
    disabled: OverridesFile,
    hovered: OverridesFile,
    pressed: OverridesFile,
}

impl KindFile {
    fn apply(self, kind: &mut KindStyle) {
        self.normal.apply_visual(&mut kind.normal);
        self.focused.apply_overrides(&mut kind.focused);
        self.disabled.apply_overrides(&mut kind.disabled);
        self.hovered.apply_overrides(&mut kind.hovered);
        self.pressed.apply_overrides(&mut kind.pressed);
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OverridesFile {
    background: Option<Color>,
    border: Option<Color>,
    border_width: Option<f32>,
    corner_radius: Option<f32>,
    text: TextFile,
}

impl OverridesFile {
    fn apply_visual(self, visual: &mut VisualStyle) {
        if let Some(Color(background)) = self.background {
            visual.background = background;
        }
        if let Some(Color(border)) = self.border {
            visual.border = border;
        }
        if let Some(border_width) = self.border_width {
            visual.border_width = border_width;
        }
        if let Some(corner_radius) = self.corner_radius {
            visual.corner_radius = corner_radius;
        }
        self.text.apply(&mut visual.text);
    }

    fn apply_overrides(self, overrides: &mut StyleOverrides) {
        if let Some(Color(background)) = self.background {
            overrides.background = Some(background);
        }
        if let Some(Color(border)) = self.border {
            overrides.border = Some(border);
        }
        if let Some(border_width) = self.border_width {
            overrides.border_width = Some(border_width);
        }
        if let Some(corner_radius) = self.corner_radius {
            overrides.corner_radius = Some(corner_radius);
        }
        self.text.apply(&mut overrides.text);
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TextFile {
    family: Option<String>,
    size: Option<f32>,
    line_height: Option<f32>,
    weight: Option<u16>,
    italic: Option<bool>,
    color: Option<Color>,
    align: Option<AlignFile>,
    wrap: Option<WrapFile>,
    overflow: Option<OverflowFile>,
}

impl TextFile {
    fn apply(self, text: &mut TextOverrides) {
        if let Some(family) = self.family {
            text.family = Some(family);
        }
        if let Some(size) = self.size {
            text.size = Some(size);
        }
        if let Some(line_height) = self.line_height {
            text.line_height = Some(line_height);
        }
        if let Some(weight) = self.weight {
            text.weight = Some(weight);
        }
        if let Some(italic) = self.italic {
            text.italic = Some(italic);
        }
        if let Some(Color(color)) = self.color {
            text.color = Some(color);
        }
        if let Some(align) = self.align {
            text.align = Some(align.into());
        }
        if let Some(wrap) = self.wrap {
            text.wrap = Some(wrap.into());
        }
        if let Some(overflow) = self.overflow {
            text.overflow = Some(overflow.into());
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AlignFile {
    Start,
    Center,
    End,
    Justified,
}

impl From<AlignFile> for TextAlign {
    fn from(align: AlignFile) -> Self {
        match align {
            AlignFile::Start => Self::Start,
            AlignFile::Center => Self::Center,
            AlignFile::End => Self::End,
            AlignFile::Justified => Self::Justified,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WrapFile {
    None,
    Glyph,
    Word,
    WordOrGlyph,
}

impl From<WrapFile> for TextWrap {
    fn from(wrap: WrapFile) -> Self {
        match wrap {
            WrapFile::None => Self::None,
            WrapFile::Glyph => Self::Glyph,
            WrapFile::Word => Self::Word,
            WrapFile::WordOrGlyph => Self::WordOrGlyph,
        }
    }
}

/// `"clip"`, or `{ "ellipsis": { "at": "end", "lines": 2 } }` with `lines`
/// optional.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OverflowFile {
    Clip,
    Ellipsis {
        at: EllipsisAtFile,
        #[serde(default)]
        lines: Option<u32>,
    },
}

impl From<OverflowFile> for TextOverflow {
    fn from(overflow: OverflowFile) -> Self {
        match overflow {
            OverflowFile::Clip => Self::Clip,
            OverflowFile::Ellipsis { at, lines } => Self::Ellipsis {
                at: at.into(),
                lines,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EllipsisAtFile {
    Start,
    Middle,
    End,
}

impl From<EllipsisAtFile> for EllipsisAt {
    fn from(at: EllipsisAtFile) -> Self {
        match at {
            EllipsisAtFile::Start => Self::Start,
            EllipsisAtFile::Middle => Self::Middle,
            EllipsisAtFile::End => Self::End,
        }
    }
}

/// The loaded `ui_themes` by ID: a `World` resource the asset loader fills
/// and a theme reload replaces entries in, which a tree applies through
/// [`UiTree::set_theme_asset`](super::UiTree::set_theme_asset).
#[derive(Debug, Default)]
pub struct UiThemeRegistry {
    themes: HashMap<String, Theme>,
}

impl UiThemeRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `theme` under `id`, returning the theme it replaced.
    pub fn insert(&mut self, id: impl Into<String>, theme: Theme) -> Option<Theme> {
        self.themes.insert(id.into(), theme)
    }

    /// The theme loaded as `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Theme> {
        self.themes.get(id)
    }

    /// Whether `id` is loaded.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.themes.contains_key(id)
    }

    /// The loaded IDs, sorted.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.themes.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }

    /// How many themes are loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.themes.len()
    }

    /// Whether no theme is loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }
}

#[cfg(test)]
#[path = "../tests/ui/theme_file.rs"]
mod tests;
