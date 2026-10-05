//! The style model (`D-125`, w01 §14): theme tokens, per-kind defaults with
//! state variants and per-node overrides. Non-text properties never inherit;
//! text properties pass down the tree. No selectors.

use crate::text::{TextAlign, TextOverflow, TextStyle, TextWrap};

use super::node::WidgetKind;

/// Named values the kind styles draw from. Plain data: a game edits a copy
/// and hands it to [`UiTree::set_theme`](super::UiTree::set_theme).
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Colours, spacing, radii, fonts and text sizes.
    pub tokens: Tokens,
    /// One style per widget kind.
    pub kinds: KindStyles,
}

impl Default for Theme {
    /// The built-in theme: a dark surface, the `sans` and `mono` families.
    fn default() -> Self {
        let tokens = Tokens::default();
        let kinds = KindStyles::for_tokens(&tokens);
        Self { tokens, kinds }
    }
}

impl Theme {
    /// The text every root starts from: the body font at the base size,
    /// regular weight, the text colour.
    #[must_use]
    pub fn base_text(&self) -> TextStyle {
        TextStyle::new(
            self.tokens.fonts.body.clone(),
            self.tokens.text.size,
            self.tokens.text.line_height,
        )
        .with_color(self.tokens.colors.text)
    }

    /// The style of `kind`.
    #[must_use]
    pub fn kind(&self, kind: WidgetKind) -> &KindStyle {
        match kind {
            WidgetKind::Panel => &self.kinds.panel,
            WidgetKind::Label => &self.kinds.label,
            WidgetKind::Button => &self.kinds.button,
        }
    }
}

/// The theme's token groups.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tokens {
    /// RGBA colours.
    pub colors: ColorTokens,
    /// Spacing steps.
    pub spacing: SpacingTokens,
    /// Corner radii.
    pub radii: RadiusTokens,
    /// Font families.
    pub fonts: FontTokens,
    /// Text sizes.
    pub text: TextTokens,
}

/// RGBA colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTokens {
    /// The window behind every root.
    pub background: [u8; 4],
    /// Panels.
    pub surface: [u8; 4],
    /// Panel and button borders.
    pub border: [u8; 4],
    /// Body text.
    pub text: [u8; 4],
    /// Secondary text.
    pub text_muted: [u8; 4],
    /// Buttons and highlights.
    pub accent: [u8; 4],
    /// Text on the accent.
    pub accent_text: [u8; 4],
    /// The focus ring.
    pub focus: [u8; 4],
    /// Text of a disabled widget.
    pub disabled_text: [u8; 4],
}

impl Default for ColorTokens {
    fn default() -> Self {
        Self {
            background: [18, 18, 22, 255],
            surface: [32, 33, 40, 255],
            border: [70, 72, 84, 255],
            text: [235, 235, 240, 255],
            text_muted: [160, 162, 175, 255],
            accent: [86, 156, 255, 255],
            accent_text: [255, 255, 255, 255],
            focus: [255, 196, 64, 255],
            disabled_text: [120, 122, 135, 255],
        }
    }
}

/// Spacing steps in UI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingTokens {
    /// Extra small.
    pub xs: f32,
    /// Small.
    pub sm: f32,
    /// Medium.
    pub md: f32,
    /// Large.
    pub lg: f32,
    /// Extra large.
    pub xl: f32,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
        }
    }
}

/// Corner radii in UI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusTokens {
    /// Square corners.
    pub none: f32,
    /// Small.
    pub sm: f32,
    /// Medium.
    pub md: f32,
    /// Large.
    pub lg: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            none: 0.0,
            sm: 2.0,
            md: 4.0,
            lg: 8.0,
        }
    }
}

/// Manifest `font_families` IDs (`D-115`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontTokens {
    /// Body text.
    pub body: String,
    /// Headings.
    pub heading: String,
    /// Monospaced text.
    pub mono: String,
}

impl Default for FontTokens {
    /// `sans`, `sans` and `mono`, the root manifest's families.
    fn default() -> Self {
        Self {
            body: "sans".to_owned(),
            heading: "sans".to_owned(),
            mono: "mono".to_owned(),
        }
    }
}

/// Base text sizes in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextTokens {
    /// Body size.
    pub size: f32,
    /// Body line height.
    pub line_height: f32,
    /// Heading size.
    pub heading_size: f32,
}

impl Default for TextTokens {
    fn default() -> Self {
        Self {
            size: 16.0,
            line_height: 20.0,
            heading_size: 24.0,
        }
    }
}

/// One [`KindStyle`] per [`WidgetKind`].
#[derive(Debug, Clone, PartialEq)]
pub struct KindStyles {
    /// Panels, roots included.
    pub panel: KindStyle,
    /// Labels.
    pub label: KindStyle,
    /// Buttons.
    pub button: KindStyle,
}

impl KindStyles {
    /// The built-in kind styles drawn from `tokens`: panels on the surface
    /// with a border, labels transparent, buttons on the accent with bold
    /// accent text, a focus-coloured border when focused and muted text when
    /// disabled.
    #[must_use]
    pub fn for_tokens(tokens: &Tokens) -> Self {
        let transparent = [0, 0, 0, 0];
        Self {
            panel: KindStyle::plain(VisualStyle {
                background: tokens.colors.surface,
                border: tokens.colors.border,
                border_width: 1.0,
                corner_radius: tokens.radii.md,
                text: TextOverrides::default(),
            }),
            label: KindStyle::plain(VisualStyle {
                background: transparent,
                border: transparent,
                border_width: 0.0,
                corner_radius: tokens.radii.none,
                text: TextOverrides::default(),
            }),
            button: KindStyle {
                normal: VisualStyle {
                    background: tokens.colors.accent,
                    border: tokens.colors.border,
                    border_width: 1.0,
                    corner_radius: tokens.radii.md,
                    text: TextOverrides {
                        color: Some(tokens.colors.accent_text),
                        weight: Some(600),
                        ..TextOverrides::default()
                    },
                },
                focused: StyleOverrides {
                    border: Some(tokens.colors.focus),
                    border_width: Some(2.0),
                    ..StyleOverrides::default()
                },
                disabled: StyleOverrides {
                    background: Some(tokens.colors.surface),
                    text: TextOverrides {
                        color: Some(tokens.colors.disabled_text),
                        ..TextOverrides::default()
                    },
                    ..StyleOverrides::default()
                },
                hovered: StyleOverrides {
                    background: Some(lighten(tokens.colors.accent, 24)),
                    ..StyleOverrides::default()
                },
                pressed: StyleOverrides {
                    background: Some(darken(tokens.colors.accent, 32)),
                    ..StyleOverrides::default()
                },
            },
        }
    }
}

fn lighten(color: [u8; 4], by: u8) -> [u8; 4] {
    let [r, g, b, a] = color;
    [
        r.saturating_add(by),
        g.saturating_add(by),
        b.saturating_add(by),
        a,
    ]
}

fn darken(color: [u8; 4], by: u8) -> [u8; 4] {
    let [r, g, b, a] = color;
    [
        r.saturating_sub(by),
        g.saturating_sub(by),
        b.saturating_sub(by),
        a,
    ]
}

/// A kind's look: its normal visual and the overrides each state applies.
/// `hovered` and `pressed` are defined now and resolved from M3.
#[derive(Debug, Clone, PartialEq)]
pub struct KindStyle {
    /// The enabled, unfocused look.
    pub normal: VisualStyle,
    /// Applied while the widget has keyboard focus.
    pub focused: StyleOverrides,
    /// Applied while the widget is disabled; wins over `focused`.
    pub disabled: StyleOverrides,
    /// Applied while the pointer is over the widget (M3).
    pub hovered: StyleOverrides,
    /// Applied while the widget is pressed (M3).
    pub pressed: StyleOverrides,
}

impl KindStyle {
    /// `normal` with no state variants.
    #[must_use]
    pub fn plain(normal: VisualStyle) -> Self {
        Self {
            normal,
            focused: StyleOverrides::default(),
            disabled: StyleOverrides::default(),
            hovered: StyleOverrides::default(),
            pressed: StyleOverrides::default(),
        }
    }

    /// The overrides `state` applies; none for `Normal`.
    #[must_use]
    pub fn variant(&self, state: WidgetState) -> Option<&StyleOverrides> {
        match state {
            WidgetState::Normal => None,
            WidgetState::Focused => Some(&self.focused),
            WidgetState::Disabled => Some(&self.disabled),
        }
    }
}

/// A complete non-text look plus the text overrides a kind applies to the
/// text under it.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualStyle {
    /// Fill colour.
    pub background: [u8; 4],
    /// Border colour.
    pub border: [u8; 4],
    /// Border width in UI units; zero for none.
    pub border_width: f32,
    /// Corner radius in UI units.
    pub corner_radius: f32,
    /// Text changes for this widget and everything under it.
    pub text: TextOverrides,
}

/// Optional changes to a [`VisualStyle`]: a state variant's, or a node's own
/// through [`UiTree::set_overrides`](super::UiTree::set_overrides).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleOverrides {
    /// Fill colour.
    pub background: Option<[u8; 4]>,
    /// Border colour.
    pub border: Option<[u8; 4]>,
    /// Border width.
    pub border_width: Option<f32>,
    /// Corner radius.
    pub corner_radius: Option<f32>,
    /// Text changes for this widget and everything under it.
    pub text: TextOverrides,
}

impl StyleOverrides {
    /// Applies the non-text overrides to `visual`; the text ones are applied
    /// separately, since they inherit.
    pub fn apply_visual(&self, visual: &mut VisualStyle) {
        if let Some(background) = self.background {
            visual.background = background;
        }
        if let Some(border) = self.border {
            visual.border = border;
        }
        if let Some(border_width) = self.border_width {
            visual.border_width = border_width;
        }
        if let Some(corner_radius) = self.corner_radius {
            visual.corner_radius = corner_radius;
        }
    }
}

/// Optional text properties; each `Some` replaces the inherited value. The
/// only properties that pass down the tree (w01 §14).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextOverrides {
    /// A `font_families` ID.
    pub family: Option<String>,
    /// Font size in pixels.
    pub size: Option<f32>,
    /// Line height in pixels.
    pub line_height: Option<f32>,
    /// Weight, 100–900.
    pub weight: Option<u16>,
    /// Italic.
    pub italic: Option<bool>,
    /// RGBA colour.
    pub color: Option<[u8; 4]>,
    /// Line alignment.
    pub align: Option<TextAlign>,
    /// Line breaking.
    pub wrap: Option<TextWrap>,
    /// Text that does not fit.
    pub overflow: Option<TextOverflow>,
}

impl TextOverrides {
    /// Whether every property is left as inherited.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Applies the set properties to `style`.
    pub fn apply(&self, style: &mut TextStyle) {
        if let Some(family) = &self.family {
            style.family.clone_from(family);
        }
        if let Some(size) = self.size {
            style.size = size;
        }
        if let Some(line_height) = self.line_height {
            style.line_height = line_height;
        }
        if let Some(weight) = self.weight {
            style.weight = weight;
        }
        if let Some(italic) = self.italic {
            style.italic = italic;
        }
        if let Some(color) = self.color {
            style.color = color;
        }
        if let Some(align) = self.align {
            style.layout.align = align;
        }
        if let Some(wrap) = self.wrap {
            style.layout.wrap = wrap;
        }
        if let Some(overflow) = self.overflow {
            style.layout.overflow = overflow;
        }
    }
}

/// The state a widget's style resolves for. `Disabled` wins over `Focused`,
/// since a disabled widget holds no focus. `Hovered` and `Pressed` join at
/// M3.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WidgetState {
    /// Enabled, unfocused.
    #[default]
    Normal,
    /// Holding keyboard focus.
    Focused,
    /// Disabled.
    Disabled,
}

/// Rises on every [`UiTree::set_theme`](super::UiTree::set_theme); part of
/// the resolution cache key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThemeEpoch(u64);

impl ThemeEpoch {
    /// The epoch after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A widget's look after resolution: the kind's normal visual, then the
/// state variant, then the node's own overrides; text inherited down the
/// tree with the same three layers applied at every node.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedStyle {
    /// Fill colour.
    pub background: [u8; 4],
    /// Border colour.
    pub border: [u8; 4],
    /// Border width in UI units.
    pub border_width: f32,
    /// Corner radius in UI units.
    pub corner_radius: f32,
    /// The text style a label under this widget shapes with.
    pub text: TextStyle,
}

impl ResolvedStyle {
    /// Whether `other` lays text out differently: any text property but the
    /// colour.
    #[must_use]
    pub fn text_measures_differently(&self, other: &Self) -> bool {
        let (a, b) = (&self.text, &other.text);
        a.family != b.family
            || a.weight != b.weight
            || a.italic != b.italic
            || a.size != b.size
            || a.line_height != b.line_height
            || a.layout != b.layout
    }
}

/// Resolves one widget: `kind`'s normal visual, then the `state` variant,
/// then the node's own `overrides`, each layer's non-text properties
/// replacing and its text overrides applied to `parent_text` in order.
#[must_use]
pub fn resolve(
    theme: &Theme,
    kind: WidgetKind,
    state: WidgetState,
    overrides: &StyleOverrides,
    parent_text: &TextStyle,
) -> ResolvedStyle {
    let kind_style = theme.kind(kind);
    let mut visual = kind_style.normal.clone();
    let mut text = parent_text.clone();
    visual.text.apply(&mut text);
    if let Some(variant) = kind_style.variant(state) {
        variant.apply_visual(&mut visual);
        variant.text.apply(&mut text);
    }
    overrides.apply_visual(&mut visual);
    overrides.text.apply(&mut text);
    ResolvedStyle {
        background: visual.background,
        border: visual.border,
        border_width: visual.border_width,
        corner_radius: visual.corner_radius,
        text,
    }
}
