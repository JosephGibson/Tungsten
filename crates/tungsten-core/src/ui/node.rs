//! Node data (`D-125`): the closed kind, layer, role and visibility enums
//! (closed as `Easing` is, `D-054`) and the record behind every
//! [`WidgetId`].

use crate::text::{StyledText, TextNodeId};

use super::WidgetId;
use super::geometry::Rect;
use super::style::LayoutStyle;
use super::theme::{ResolvedStyle, StyleOverrides};

/// The layer a root draws and hit-tests on, low to high (w01 §6).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UiLayer {
    /// Game HUD and menus.
    #[default]
    Game,
    /// Game dialogs and popups, above the game layer.
    GamePopup,
    /// Debug views, above every game layer.
    Debug,
    /// Debug popups, the top layer.
    DebugPopup,
}

/// What a widget is. `Image` joins at M2, `Toggle` and `Slider` at BC.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    /// A box that lays out its children.
    Panel,
    /// A text node.
    Label,
    /// A focusable, interactive panel holding one `Label`.
    Button,
}

impl WidgetKind {
    /// The role a new widget of this kind starts with.
    #[must_use]
    pub const fn default_role(self) -> Role {
        match self {
            Self::Panel => Role::None,
            Self::Label => Role::Label,
            Self::Button => Role::Button,
        }
    }
}

/// The accessibility role kept in node data from M1 (w01 §12). Nothing
/// consumes it yet.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Role {
    /// No role.
    #[default]
    None,
    /// A grouping of widgets.
    Group,
    /// A top-level window.
    Window,
    /// A modal or popup dialog.
    Dialog,
    /// Static text.
    Label,
    /// An activatable control.
    Button,
}

/// Whether a widget takes part in layout, paint and hit testing.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Laid out, painted and hit-testable.
    #[default]
    Visible,
    /// Laid out and keeps its box; neither painted nor hit-testable, and
    /// skipped by focus traversal.
    Hidden,
    /// Leaves layout: no box, no paint, no hits; its subtree with it.
    Collapsed,
}

impl Visibility {
    /// Whether the widget is painted and hit-testable.
    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// A label's text and its flattened form, for the accessible name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextContent {
    pub(crate) text: StyledText,
    pub(crate) flat: String,
}

impl TextContent {
    pub(crate) fn new(text: StyledText) -> Self {
        let flat = text.spans.iter().map(|span| span.text.as_str()).collect();
        Self { text, flat }
    }
}

/// The record behind a [`WidgetId`].
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)] // One behaviour flag each (w01 §14); no two are a state machine.
pub(crate) struct Node {
    pub(crate) kind: WidgetKind,
    /// `None` for a root.
    pub(crate) parent: Option<WidgetId>,
    /// The root this node belongs to; itself for a root.
    pub(crate) root: WidgetId,
    pub(crate) layer: UiLayer,
    pub(crate) children: Vec<WidgetId>,
    /// Set for a `Label`.
    pub(crate) text: Option<TextContent>,
    pub(crate) layout_style: LayoutStyle,
    pub(crate) visibility: Visibility,
    pub(crate) role: Role,
    pub(crate) accessible_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) interactive: bool,
    pub(crate) focusable: bool,
    pub(crate) focus_scope: bool,
    pub(crate) hit_testable: bool,
    /// The node's own style overrides, applied after the kind's variant.
    pub(crate) overrides: StyleOverrides,
    /// The last resolution; `None` before the first.
    pub(crate) resolved: Option<ResolvedStyle>,
    /// Whether the next resolution pass must resolve this node.
    pub(crate) style_dirty: bool,
    /// Roots: whether any node under this root is style-dirty.
    pub(crate) style_dirty_subtree: bool,
    /// Labels: the text node the store shapes; `None` before the first layout.
    pub(crate) text_id: Option<TextNodeId>,
    /// Labels: whether the text node must be set again (new, changed text or
    /// changed resolved text style).
    pub(crate) text_stale: bool,
    /// The committed box; `None` before the first layout or when collapsed.
    pub(crate) rect: Option<Rect>,
    /// Roots: their place in insertion order.
    pub(crate) root_seq: u64,
    /// Roots: whether the next layout pass must lay this root out.
    pub(crate) layout_dirty: bool,
}

impl Node {
    pub(crate) fn new(
        kind: WidgetKind,
        parent: Option<WidgetId>,
        root: WidgetId,
        layer: UiLayer,
        layout_style: LayoutStyle,
    ) -> Self {
        let is_button = matches!(kind, WidgetKind::Button);
        Self {
            kind,
            parent,
            root,
            layer,
            children: Vec::new(),
            text: None,
            layout_style,
            visibility: Visibility::Visible,
            role: kind.default_role(),
            accessible_name: None,
            enabled: true,
            interactive: is_button,
            focusable: is_button,
            focus_scope: false,
            hit_testable: true,
            overrides: StyleOverrides::default(),
            resolved: None,
            style_dirty: true,
            style_dirty_subtree: true,
            text_id: None,
            text_stale: true,
            rect: None,
            root_seq: 0,
            layout_dirty: true,
        }
    }
}
