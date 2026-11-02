//! The theme asset (w01 §14, Q9): a JSON file of optional fields over
//! [`Theme::default`], read into a serde model and mapped onto a [`Theme`].
//! The manifest's `ui_themes` section names the files; [`UiThemeRegistry`]
//! holds the loaded themes by ID for a tree to apply and a reload to
//! replace. Colours are `#rrggbb` or `#rrggbbaa`, authored in sRGB and kept
//! as `[u8; 4]`; the pipelines decode them (Q8). Any colour field may name a
//! colour token instead, resolved once the file's tokens are known.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;
use serde::de::{self, Deserializer};
use thiserror::Error;

use crate::text::{EllipsisAt, TextAlign, TextOverflow, TextWrap};

use super::theme::{
    ColorTokens, KindStyle, KindStyles, StyleOverrides, TextOverrides, Theme, ThemeNames,
    ThemeSounds, Tokens, VisualStyle,
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
    /// The file is not a theme: bad JSON, an unknown field, a bad colour or
    /// an unknown token name.
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
/// kind (`panel`, `label`, `button`, `image`, `toggle`, `slider`), a
/// `normal` look and the `focused`, `disabled`, `hovered` and `pressed`
/// variants, each merged over the kind's built-in one; `text_styles` holds
/// text objects by name, each merged over the built-in style of its name;
/// `sizes` holds sizes by name in UI units, finite and at least 0; `sounds`
/// holds the `focus`, `activate` and `back` sounds' manifest IDs. A colour
/// field takes `#rrggbb`, `#rrggbbaa` or a colour token's name
/// ([`ColorTokens::NAMES`]), read from the tokens the file makes; a name in
/// `tokens.colors` reads the file's literal colours. An unknown field or
/// token name is an error, the name's with its field path.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeFile {
    tokens: TokensFile,
    kinds: KindsFile,
    text_styles: BTreeMap<String, TextFile>,
    sizes: BTreeMap<String, f32>,
    sounds: SoundsFile,
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

    /// Parses theme JSON: one object, as the file format describes, its
    /// token names resolved.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        if !value.is_object() {
            return Err(de::Error::custom("a theme file is a JSON object"));
        }
        let mut file: Self = serde_json::from_value(value)?;
        file.resolve_names().map_err(de::Error::custom)?;
        Ok(file)
    }

    /// Checks the sizes and turns every colour token's name into the
    /// token's colour: the token fields' first, against the file's literal
    /// colours, then the rest against the tokens the file makes.
    fn resolve_names(&mut self) -> Result<(), String> {
        for (name, size) in &self.sizes {
            if !(size.is_finite() && *size >= 0.0) {
                return Err(format!(
                    "sizes.{name}: {size} is not a finite number at or above 0"
                ));
            }
        }
        let literal = self.tokens.colors.tokens();
        for (name, value) in self.tokens.colors.fields() {
            resolve(value, &literal, &|| format!("tokens.colors.{name}"))?;
        }
        let tokens = self.tokens.colors.tokens();
        for (kind, file) in self.kinds.fields() {
            let Some(file) = file else {
                continue;
            };
            for (state, overrides) in file.fields() {
                let path = |field: &str| format!("kinds.{kind}.{state}.{field}");
                resolve(&mut overrides.background, &tokens, &|| path("background"))?;
                resolve(&mut overrides.border, &tokens, &|| path("border"))?;
                overrides
                    .text
                    .resolve(&tokens, &|field| path(&format!("text.{field}")))?;
            }
        }
        for (name, text) in &mut self.text_styles {
            text.resolve(&tokens, &|field| format!("text_styles.{name}.{field}"))?;
        }
        Ok(())
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
            toggle,
            slider,
        } = self.kinds;
        for (file, kind) in [
            (panel, &mut kinds.panel),
            (label, &mut kinds.label),
            (button, &mut kinds.button),
            (image, &mut kinds.image),
            (toggle, &mut kinds.toggle),
            (slider, &mut kinds.slider),
        ] {
            if let Some(file) = file {
                file.apply(kind);
            }
        }
        let mut names = ThemeNames::for_tokens(&tokens);
        for (name, file) in self.text_styles {
            file.apply(&mut names.text_styles.entry(name).or_default().text);
        }
        names.sizes = self.sizes;
        names.sounds = ThemeSounds {
            focus: self.sounds.focus,
            activate: self.sounds.activate,
            back: self.sounds.back,
        };
        Theme {
            tokens,
            kinds,
            names: Box::new(names),
        }
    }
}

/// A colour field's value: an sRGB colour parsed from `#rrggbb` or
/// `#rrggbbaa`, or a colour token's name, which
/// [`ThemeFile::from_json`] turns into the token's colour.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Color {
    Rgba([u8; 4]),
    Token(String),
}

impl Color {
    /// The colour; a name left unresolved, which no parsed file holds, reads
    /// transparent.
    fn rgba(&self) -> [u8; 4] {
        match self {
            Self::Rgba(rgba) => *rgba,
            Self::Token(_) => [0; 4],
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.starts_with('#') {
            parse_color(&text)
                .map(Color::Rgba)
                .map_err(de::Error::custom)
        } else {
            Ok(Color::Token(text))
        }
    }
}

/// Turns `color`, when it names a token, into that token's colour in
/// `tokens`; an unknown name is an error naming `path`.
fn resolve(
    color: &mut Option<Color>,
    tokens: &ColorTokens,
    path: &dyn Fn() -> String,
) -> Result<(), String> {
    let Some(Color::Token(name)) = color else {
        return Ok(());
    };
    let Some(rgba) = tokens.named(name) else {
        return Err(format!(
            "{}: colour '{name}' is not #rrggbb, #rrggbbaa or a colour token's name ({})",
            path(),
            ColorTokens::NAMES.join(", ")
        ));
    };
    *color = Some(Color::Rgba(rgba));
    Ok(())
}

/// A `line_height`: finite and above zero, since the text engine cannot lay a
/// line out at zero (cosmic-text asserts it). `null` is no value.
fn line_height<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f32>, D::Error> {
    match Option::<f32>::deserialize(deserializer)? {
        Some(value) if !(value.is_finite() && value > 0.0) => Err(de::Error::custom(format!(
            "line_height {value} is not a finite number above 0"
        ))),
        value => Ok(value),
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
        tokens.colors = self.colors.tokens();
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
    danger: Option<Color>,
    scrim: Option<Color>,
    raised: Option<Color>,
}

impl ColorsFile {
    /// Each field by its token's name.
    fn fields(&mut self) -> [(&'static str, &mut Option<Color>); 12] {
        [
            ("background", &mut self.background),
            ("surface", &mut self.surface),
            ("border", &mut self.border),
            ("text", &mut self.text),
            ("text_muted", &mut self.text_muted),
            ("accent", &mut self.accent),
            ("accent_text", &mut self.accent_text),
            ("focus", &mut self.focus),
            ("disabled_text", &mut self.disabled_text),
            ("danger", &mut self.danger),
            ("scrim", &mut self.scrim),
            ("raised", &mut self.raised),
        ]
    }

    /// The colour tokens: the built-in ones under this file's colours, an
    /// unset scrim the background's and an unset raised fill the surface.
    /// Names not yet resolved are left out.
    fn tokens(&self) -> ColorTokens {
        let mut tokens = ColorTokens::default();
        let mut file = self.clone();
        let (scrim, raised) = (self.scrim.is_some(), self.raised.is_some());
        for (name, value) in file.fields() {
            if let Some(Color::Rgba(rgba)) = value
                && let Some(slot) = token_slot(&mut tokens, name)
            {
                *slot = *rgba;
            }
        }
        if !scrim {
            tokens.scrim = ColorTokens::scrim_of(tokens.background);
        }
        if !raised {
            tokens.raised = tokens.surface;
        }
        tokens
    }
}

/// The token named `name` in `tokens`.
fn token_slot<'a>(tokens: &'a mut ColorTokens, name: &str) -> Option<&'a mut [u8; 4]> {
    Some(match name {
        "background" => &mut tokens.background,
        "surface" => &mut tokens.surface,
        "border" => &mut tokens.border,
        "text" => &mut tokens.text,
        "text_muted" => &mut tokens.text_muted,
        "accent" => &mut tokens.accent,
        "accent_text" => &mut tokens.accent_text,
        "focus" => &mut tokens.focus,
        "disabled_text" => &mut tokens.disabled_text,
        "danger" => &mut tokens.danger,
        "scrim" => &mut tokens.scrim,
        "raised" => &mut tokens.raised,
        _ => return None,
    })
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
    #[serde(deserialize_with = "line_height")]
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
    toggle: Option<KindFile>,
    slider: Option<KindFile>,
}

impl KindsFile {
    /// Each kind by its name.
    fn fields(&mut self) -> [(&'static str, &mut Option<KindFile>); 6] {
        [
            ("panel", &mut self.panel),
            ("label", &mut self.label),
            ("button", &mut self.button),
            ("image", &mut self.image),
            ("toggle", &mut self.toggle),
            ("slider", &mut self.slider),
        ]
    }
}

/// The `sounds` object: the menu sounds' manifest IDs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SoundsFile {
    focus: Option<String>,
    activate: Option<String>,
    back: Option<String>,
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
    /// Each state's overrides by its name.
    fn fields(&mut self) -> [(&'static str, &mut OverridesFile); 5] {
        [
            ("normal", &mut self.normal),
            ("focused", &mut self.focused),
            ("disabled", &mut self.disabled),
            ("hovered", &mut self.hovered),
            ("pressed", &mut self.pressed),
        ]
    }

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
        if let Some(background) = &self.background {
            visual.background = background.rgba();
        }
        if let Some(border) = &self.border {
            visual.border = border.rgba();
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
        if let Some(background) = &self.background {
            overrides.background = Some(background.rgba());
        }
        if let Some(border) = &self.border {
            overrides.border = Some(border.rgba());
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
    #[serde(deserialize_with = "line_height")]
    line_height: Option<f32>,
    weight: Option<u16>,
    italic: Option<bool>,
    color: Option<Color>,
    align: Option<AlignFile>,
    wrap: Option<WrapFile>,
    overflow: Option<OverflowFile>,
    letter_spacing: Option<f32>,
}

impl TextFile {
    /// Turns the colour fields' token names into colours, an unknown one an
    /// error at `path` of the field.
    fn resolve(
        &mut self,
        tokens: &ColorTokens,
        path: &dyn Fn(&str) -> String,
    ) -> Result<(), String> {
        resolve(&mut self.color, tokens, &|| path("color"))
    }

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
        if let Some(color) = &self.color {
            text.color = Some(color.rgba());
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
        if let Some(letter_spacing) = self.letter_spacing {
            text.letter_spacing = Some(letter_spacing);
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
