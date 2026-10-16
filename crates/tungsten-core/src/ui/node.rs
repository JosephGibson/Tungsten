//! Node data (`D-125`): the closed kind, layer, role, visibility and cursor
//! enums (closed as `Easing` is, `D-054`) and the record behind every
//! [`WidgetId`].

use glam::Vec2;

use crate::assets::SpriteAssetId;
use crate::text::{StyledText, TextNodeId};

use super::WidgetId;
use super::control::Control;
use super::geometry::Rect;
use super::scroll::ScrollState;
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

impl UiLayer {
    /// Whether roots on this layer take the tree's debug scale (M58):
    /// `Debug` and `DebugPopup`.
    pub(crate) const fn is_debug(self) -> bool {
        matches!(self, Self::Debug | Self::DebugPopup)
    }
}

/// What a widget is.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    /// A box that lays out its children.
    Panel,
    /// A text node.
    Label,
    /// A focusable, interactive panel holding one `Label`.
    Button,
    /// A leaf drawing one sprite, stretched into its box, or nine-sliced
    /// when the sprite has insets (M2).
    Image,
    /// A checkbox: a focusable, interactive leaf that is on or off (M55).
    /// Its caption is a sibling `Label`.
    Toggle,
    /// A horizontal slider: a focusable, interactive leaf holding a value in
    /// a [`SliderRange`](super::SliderRange) (M55).
    Slider,
    /// A leaf drawing coloured triangles, [`UiVertex`] positions as fractions
    /// of its box (M57). It takes the panel's kind style for its own box and
    /// has no natural size.
    Mesh,
}

impl WidgetKind {
    /// The role a new widget of this kind starts with.
    #[must_use]
    pub const fn default_role(self) -> Role {
        match self {
            Self::Panel => Role::None,
            Self::Label => Role::Label,
            Self::Button => Role::Button,
            Self::Image | Self::Mesh => Role::Image,
            Self::Toggle => Role::CheckBox,
            Self::Slider => Role::Slider,
        }
    }

    /// Whether widgets of this kind hold no children: labels, images,
    /// toggles, sliders and meshes.
    #[must_use]
    pub const fn is_leaf(self) -> bool {
        matches!(
            self,
            Self::Label | Self::Image | Self::Toggle | Self::Slider | Self::Mesh
        )
    }

    /// Whether widgets of this kind start focusable and interactive:
    /// buttons, toggles and sliders.
    const fn is_control(self) -> bool {
        matches!(self, Self::Button | Self::Toggle | Self::Slider)
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
    /// A picture.
    Image,
    /// A control that is on or off.
    CheckBox,
    /// A control that picks a value in a range.
    Slider,
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

/// A system cursor a widget asks for while the pointer is over it
/// (`D-149`). The umbrella maps each to the window's own; game cursors,
/// cursor visibility and grab are not here.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    /// The platform's arrow.
    #[default]
    Default,
    /// A hand, for something to click.
    Pointer,
    /// A text beam, for text to select or edit.
    Text,
    /// Four arrows, for something to move.
    Move,
    /// A crossed circle, for an action not available.
    NotAllowed,
    /// A horizontal resize arrow, east to west.
    EwResize,
    /// A vertical resize arrow, north to south.
    NsResize,
    /// A diagonal resize arrow, top left to bottom right.
    NwseResize,
    /// A diagonal resize arrow, top right to bottom left.
    NeswResize,
}

/// One vertex of a [`WidgetKind::Mesh`] (M57): every three make a triangle.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiVertex {
    /// Where the vertex sits as fractions of the widget's box, 0 to 1 from
    /// its top-left corner; values outside place it outside the box, where
    /// the clips cut it.
    pub position: Vec2,
    /// Its colour, sRGB with straight alpha; the triangle blends between its
    /// three.
    pub color: [u8; 4],
}

impl UiVertex {
    /// A vertex at `position`, in box fractions, of `color`.
    #[must_use]
    pub const fn new(position: Vec2, color: [u8; 4]) -> Self {
        Self { position, color }
    }
}

/// A soft shadow a widget paints under its box (M58), set with
/// [`UiTree::set_shadow`](super::UiTree::set_shadow): the box moved by
/// `offset` and filled with `color`, ringed `blur` wide by a band that fades
/// to transparent. Lengths are in the widget's UI units. Paint only: hit
/// testing ignores it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiShadow {
    /// The colour under the box, sRGB with straight alpha; the ring fades
    /// from it to transparent.
    pub color: [u8; 4],
    /// The ring's width; a value that is not finite and positive paints no
    /// shadow.
    pub blur: f32,
    /// How far the shadow sits from the box.
    pub offset: Vec2,
}

impl UiShadow {
    /// A shadow of `color`, ringed `blur` wide, moved by `offset`.
    #[must_use]
    pub const fn new(color: [u8; 4], blur: f32, offset: Vec2) -> Self {
        Self {
            color,
            blur,
            offset,
        }
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

/// An image's sprite, the size an `Auto` box measures and its tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageContent {
    pub(crate) sprite: SpriteAssetId,
    /// The size an `Auto` axis takes, in UI units (Q4).
    pub(crate) natural_size: Vec2,
    /// Multiplied into the sampled colour; opaque white draws the sprite as
    /// authored.
    pub(crate) tint: [u8; 4],
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
    /// Set for an `Image`.
    pub(crate) image: Option<ImageContent>,
    /// Set for a `Toggle` or a `Slider`.
    pub(crate) control: Option<Control>,
    /// Meshes: whole triangles of finite vertices, in box fractions (M57).
    pub(crate) mesh: Vec<UiVertex>,
    /// Whether the children paint only inside this node's box (Q6).
    pub(crate) clip_children: bool,
    /// Scroll panels: their offset and the last layout's maximum (M57).
    pub(crate) scroll: Option<ScrollState>,
    /// The shadow painted under the box (M58).
    pub(crate) shadow: Option<UiShadow>,
    pub(crate) layout_style: LayoutStyle,
    pub(crate) visibility: Visibility,
    pub(crate) role: Role,
    pub(crate) accessible_name: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) interactive: bool,
    pub(crate) focusable: bool,
    pub(crate) focus_scope: bool,
    pub(crate) hit_testable: bool,
    /// Roots: whether this root, while shown, blocks the roots below it.
    pub(crate) modal: bool,
    /// Whether a primary press dragging past the threshold moves the root
    /// (M57).
    pub(crate) drag_handle: bool,
    /// The cursor shown over this widget and the children that set none.
    pub(crate) cursor: Option<CursorIcon>,
    /// Directional focus's explicit neighbours, by `NavDirection`; `None`
    /// until one is set.
    pub(crate) focus_neighbors: Option<Box<[Option<WidgetId>; 4]>>,
    /// Focus scopes and roots: whether directional focus wraps inside.
    pub(crate) focus_wrap: bool,
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
    /// Labels: the committed content box, the box less Taffy's border and
    /// padding, where the text commits and paints (M56).
    pub(crate) content: Option<Rect>,
    /// Paint only (M56): multiplied into the alpha of everything the
    /// subtree paints, 0..=1.
    pub(crate) opacity: f32,
    /// Paint only (M56): the subtree's offset in UI units.
    pub(crate) translation: Vec2,
    /// Paint only (M56): the subtree's scale about this node's centre.
    pub(crate) scale: f32,
    /// Roots: the transition's fade, multiplied in as `opacity` is.
    pub(crate) fade: f32,
    /// Roots: whether the root keeps its look and blocks the pointer while
    /// its widgets take no input.
    pub(crate) frozen: bool,
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
        let is_control = kind.is_control();
        Self {
            kind,
            parent,
            root,
            layer,
            children: Vec::new(),
            text: None,
            image: None,
            control: None,
            mesh: Vec::new(),
            clip_children: false,
            scroll: None,
            shadow: None,
            layout_style,
            visibility: Visibility::Visible,
            role: kind.default_role(),
            accessible_name: None,
            enabled: true,
            interactive: is_control,
            focusable: is_control,
            focus_scope: false,
            hit_testable: true,
            modal: false,
            drag_handle: false,
            cursor: None,
            focus_neighbors: None,
            focus_wrap: false,
            overrides: StyleOverrides::default(),
            resolved: None,
            style_dirty: true,
            style_dirty_subtree: true,
            text_id: None,
            text_stale: true,
            rect: None,
            content: None,
            opacity: 1.0,
            translation: Vec2::ZERO,
            scale: 1.0,
            fade: 1.0,
            frozen: false,
            root_seq: 0,
            layout_dirty: true,
        }
    }
}
