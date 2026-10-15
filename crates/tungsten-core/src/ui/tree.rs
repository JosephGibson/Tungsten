//! The `UiTree` resource (`D-125`): slots behind generational IDs, roots on
//! layers, building, queries, the behaviour flags, the interaction state and
//! the dirty flags. Every mutation is a method call that marks only what it
//! changes: text, visibility, an image's size and layout style mark layout
//! (and so paint); enabled, a tint and the clip flag mark paint; hover,
//! press and focus visibility mark the widgets' styles; roles, names and the
//! behaviour flags mark nothing.

use std::collections::HashSet;

use glam::Vec2;

use crate::assets::SpriteAssetId;
use crate::text::{FontEpoch, StyledText, TextNodeId, TextStyle};

use super::id::WidgetId;
use super::node::{
    CursorIcon, ImageContent, Node, Role, TextContent, UiLayer, Visibility, WidgetKind,
};
use super::style::LayoutStyle;
use super::theme::{self, ResolvedStyle, StyleOverrides, Theme, ThemeEpoch, WidgetState};

struct Slot {
    generation: u32,
    node: Option<Node>,
}

/// Counts from one style resolution pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StyleStats {
    /// Nodes resolved: the style-dirty ones and those under a parent whose
    /// text changed.
    pub(crate) resolved: u32,
    /// Resolved nodes whose text style changed in any property.
    pub(crate) text_changed: u32,
    /// Resolved nodes whose text style changed in a property that measures:
    /// anything but the colour.
    pub(crate) measurement_changed: u32,
}

/// Counts from one [`UiTree::layout`] pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutStats {
    /// Roots laid out: the dirty ones.
    pub roots_laid_out: u32,
    /// Nodes under those roots that were laid out.
    pub nodes_laid_out: u32,
    /// Calls the solver made into the text store's `measure`.
    pub measure_calls: u32,
    /// Text nodes committed.
    pub commits: u32,
}

/// The UI: one `World` resource holding every root, on layers, with the
/// theme, focus and the solver state. Every mutation is a method call that
/// marks what it changes layout- or paint-dirty; nothing draws (`D-125`).
#[derive(Default)]
pub struct UiTree {
    slots: Vec<Slot>,
    free: Vec<u32>,
    /// Roots by layer, then insertion order.
    pub(crate) roots: Vec<WidgetId>,
    next_root_seq: u64,
    pub(crate) paint_dirty: bool,
    pub(crate) theme: Theme,
    pub(crate) theme_epoch: ThemeEpoch,
    /// The `ui_themes` ID the theme came from, for a reload to re-apply
    /// (Q9); `None` for the built-in theme or one set by hand.
    pub(crate) theme_id: Option<String>,
    /// The viewport of the last layout pass; `None` before the first.
    pub(crate) viewport: Option<Vec2>,
    /// Sprites an image named that the registry had not registered, each
    /// warned about once.
    pub(crate) warned_sprites: HashSet<SpriteAssetId>,
    /// The widget holding keyboard focus.
    pub(crate) focused: Option<WidgetId>,
    /// Text nodes of removed labels, freed at the next layout.
    pub(crate) pending_text_release: Vec<TextNodeId>,
    /// The store's font epoch at the last layout; `None` before the first.
    pub(crate) font_epoch: Option<FontEpoch>,
    /// Per focus scope entered, the focus left behind, restored when the
    /// scope is removed, hidden or collapsed.
    pub(crate) focus_return: Vec<(WidgetId, Option<WidgetId>)>,
    /// The widget under the pointer that the router hovers.
    pub(crate) hovered: Option<WidgetId>,
    /// The widget the router shows pressed.
    pub(crate) pressed: Option<WidgetId>,
    /// Whether focus is hidden, after pointer use; visible on a new tree.
    pub(crate) focus_hidden: bool,
}

impl std::fmt::Debug for UiTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiTree")
            .field(
                "nodes",
                &self.slots.iter().filter(|s| s.node.is_some()).count(),
            )
            .field("roots", &self.roots)
            .field("paint_dirty", &self.paint_dirty)
            .finish_non_exhaustive()
    }
}

impl UiTree {
    /// An empty tree with the built-in theme.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ---- slots ----

    pub(crate) fn node(&self, id: WidgetId) -> Option<&Node> {
        self.slots
            .get(id.index() as usize)
            .filter(|slot| slot.generation == id.generation())
            .and_then(|slot| slot.node.as_ref())
    }

    pub(crate) fn node_mut(&mut self, id: WidgetId) -> Option<&mut Node> {
        self.slots
            .get_mut(id.index() as usize)
            .filter(|slot| slot.generation == id.generation())
            .and_then(|slot| slot.node.as_mut())
    }

    fn insert(&mut self, make: impl FnOnce(WidgetId) -> Node) -> WidgetId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            let id = WidgetId::new(index, slot.generation);
            slot.node = Some(make(id));
            return id;
        }
        let index = u32::try_from(self.slots.len()).expect("widget count fits in u32");
        let id = WidgetId::new(index, 0);
        self.slots.push(Slot {
            generation: 0,
            node: Some(make(id)),
        });
        id
    }

    /// Frees `id`'s slot; its generation moves on.
    fn release(&mut self, id: WidgetId) -> Option<Node> {
        let slot = self.slots.get_mut(id.index() as usize)?;
        if slot.generation != id.generation() {
            return None;
        }
        let node = slot.node.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index());
        Some(node)
    }

    // ---- dirt ----

    /// Marks `id`'s root for the next layout pass, and paint with it.
    pub(crate) fn mark_layout_dirty(&mut self, id: WidgetId) {
        let Some(root) = self.node(id).map(|node| node.root) else {
            return;
        };
        if let Some(root) = self.node_mut(root) {
            root.layout_dirty = true;
        }
        self.paint_dirty = true;
    }

    pub(crate) fn mark_paint_dirty(&mut self) {
        self.paint_dirty = true;
    }

    /// Whether any root needs a layout pass.
    #[must_use]
    pub fn is_layout_dirty(&self) -> bool {
        self.roots
            .iter()
            .any(|root| self.node(*root).is_some_and(|node| node.layout_dirty))
    }

    /// Whether anything changed since [`Self::clear_paint_dirty`]. M2's
    /// extract reads it after the layout pass.
    #[must_use]
    pub fn is_paint_dirty(&self) -> bool {
        self.paint_dirty
    }

    /// Marks the tree painted.
    pub fn clear_paint_dirty(&mut self) {
        self.paint_dirty = false;
    }

    // ---- building ----

    /// A new root panel on `layer`. Roots on the same layer keep their
    /// insertion order; a later root is in front.
    pub fn add_root(&mut self, layer: UiLayer, style: LayoutStyle) -> WidgetId {
        let seq = self.next_root_seq;
        self.next_root_seq += 1;
        let id = self.insert(|id| {
            let mut node = Node::new(WidgetKind::Panel, None, id, layer, style);
            node.root_seq = seq;
            node
        });
        let at = self
            .roots
            .iter()
            .position(|root| self.node(*root).is_some_and(|node| node.layer > layer))
            .unwrap_or(self.roots.len());
        self.roots.insert(at, id);
        self.paint_dirty = true;
        id
    }

    pub(crate) fn add_child(
        &mut self,
        parent: WidgetId,
        kind: WidgetKind,
        style: LayoutStyle,
    ) -> WidgetId {
        let (root, layer) = self.node(parent).map_or_else(
            || panic!("UiTree: parent {parent:?} is not in the tree"),
            |node| {
                assert!(
                    !node.kind.is_leaf(),
                    "UiTree: a {:?} holds no children ({parent:?})",
                    node.kind
                );
                (node.root, node.layer)
            },
        );
        let id = self.insert(|_| Node::new(kind, Some(parent), root, layer, style));
        if let Some(parent) = self.node_mut(parent) {
            parent.children.push(id);
        }
        self.mark_layout_dirty(id);
        self.mark_style_dirty(id);
        id
    }

    /// A panel under `parent`. Panics when `parent` is not in the tree.
    pub fn panel(&mut self, parent: WidgetId, style: LayoutStyle) -> WidgetId {
        self.add_child(parent, WidgetKind::Panel, style)
    }

    /// A label under `parent` with `text`. Panics when `parent` is not in
    /// the tree.
    pub fn label(&mut self, parent: WidgetId, text: impl Into<StyledText>) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::Label, LayoutStyle::default());
        if let Some(node) = self.node_mut(id) {
            node.text = Some(TextContent::new(text.into()));
        }
        id
    }

    /// A button under `parent`: a `Button` holding one `Label` with `text`,
    /// focusable and interactive. Its text inherits the button's text style
    /// (w01 §14). Panics when `parent` is not in the tree.
    pub fn button(&mut self, parent: WidgetId, text: impl Into<StyledText>) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::Button, LayoutStyle::default());
        self.label(id, text);
        id
    }

    /// An image under `parent`: a leaf drawing `sprite` stretched into its
    /// box, or nine-sliced when the sprite has insets. An `Auto` axis takes
    /// `natural_size` (Q4); the tint starts opaque white. Panics when
    /// `parent` is not in the tree.
    pub fn image(
        &mut self,
        parent: WidgetId,
        sprite: SpriteAssetId,
        natural_size: Vec2,
        style: LayoutStyle,
    ) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::Image, style);
        if let Some(node) = self.node_mut(id) {
            node.image = Some(ImageContent {
                sprite,
                natural_size,
                tint: [255, 255, 255, 255],
            });
        }
        id
    }

    /// Replaces an image's sprite and natural size: marks layout when the
    /// size changed, paint when only the sprite did, nothing when both are
    /// equal. Does nothing for a widget that is not an image.
    pub fn set_image(&mut self, id: WidgetId, sprite: SpriteAssetId, natural_size: Vec2) {
        let Some(image) = self.node_mut(id).and_then(|node| node.image.as_mut()) else {
            return;
        };
        let size_changed = image.natural_size != natural_size;
        let sprite_changed = image.sprite != sprite;
        image.sprite = sprite;
        image.natural_size = natural_size;
        if size_changed {
            self.mark_layout_dirty(id);
        } else if sprite_changed {
            self.mark_paint_dirty();
        }
    }

    /// An image's sprite; `None` for a widget that is not an image.
    #[must_use]
    pub fn image_sprite(&self, id: WidgetId) -> Option<SpriteAssetId> {
        self.node(id)?.image.map(|image| image.sprite)
    }

    /// Sets the colour an image's pixels are multiplied by. Marks paint; an
    /// equal tint marks nothing.
    pub fn set_image_tint(&mut self, id: WidgetId, tint: [u8; 4]) {
        let Some(image) = self.node_mut(id).and_then(|node| node.image.as_mut()) else {
            return;
        };
        if image.tint == tint {
            return;
        }
        image.tint = tint;
        self.mark_paint_dirty();
    }

    /// Keeps `id`'s children's paint inside its box: the paint list clips
    /// them to the intersection of the box and the enclosing clip, and skips
    /// them when it is empty. Off by default; roots are clipped by the
    /// viewport alone (Q6). Marks paint; an equal flag marks nothing.
    pub fn set_clip_children(&mut self, id: WidgetId, on: bool) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.clip_children == on {
            return;
        }
        node.clip_children = on;
        self.mark_paint_dirty();
    }

    /// Whether `id` clips its children's paint; `false` for an invalid ID.
    #[must_use]
    pub fn clips_children(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.clip_children)
    }

    /// Removes `id` and its subtree. Focus, hover and a press held inside it
    /// are released, focus going back to where it was when a scope removed
    /// with it was entered, and its text nodes are freed at the next layout.
    /// An invalid ID does nothing.
    pub fn remove(&mut self, id: WidgetId) {
        let Some(parent) = self.node(id).map(|node| node.parent) else {
            return;
        };
        if let Some(parent) = parent {
            self.mark_layout_dirty(parent);
            if let Some(parent) = self.node_mut(parent) {
                parent.children.retain(|child| *child != id);
            }
        } else {
            self.roots.retain(|root| *root != id);
            self.paint_dirty = true;
        }
        let mut stack = vec![id];
        let mut removed = Vec::new();
        while let Some(current) = stack.pop() {
            if self.focused == Some(current) {
                self.set_focused(None);
            }
            if self.hovered == Some(current) {
                self.hovered = None;
            }
            if self.pressed == Some(current) {
                self.pressed = None;
            }
            removed.push(current);
            if let Some(node) = self.release(current) {
                if let Some(text_node) = node.text_id {
                    self.pending_text_release.push(text_node);
                }
                stack.extend(node.children);
            }
        }
        self.focus_after_removal(&removed);
    }

    // ---- queries ----

    /// Whether `id` names a widget in the tree.
    #[must_use]
    pub fn contains(&self, id: WidgetId) -> bool {
        self.node(id).is_some()
    }

    /// The parent; `None` for a root or an invalid ID.
    #[must_use]
    pub fn parent(&self, id: WidgetId) -> Option<WidgetId> {
        self.node(id)?.parent
    }

    /// Children in sibling order; empty for an invalid ID.
    #[must_use]
    pub fn children(&self, id: WidgetId) -> &[WidgetId] {
        self.node(id).map_or(&[], |node| node.children.as_slice())
    }

    /// Roots by layer, then insertion order.
    #[must_use]
    pub fn roots(&self) -> &[WidgetId] {
        &self.roots
    }

    /// The widget's kind.
    #[must_use]
    pub fn kind(&self, id: WidgetId) -> Option<WidgetKind> {
        self.node(id).map(|node| node.kind)
    }

    /// The layer of the widget's root.
    #[must_use]
    pub fn layer(&self, id: WidgetId) -> Option<UiLayer> {
        self.node(id).map(|node| node.layer)
    }

    /// The root `id` belongs to; itself for a root.
    #[must_use]
    pub fn root_of(&self, id: WidgetId) -> Option<WidgetId> {
        self.node(id).map(|node| node.root)
    }

    // ---- text ----

    /// The node that holds `id`'s text: a label itself, a button's label.
    fn text_holder(&self, id: WidgetId) -> Option<WidgetId> {
        let node = self.node(id)?;
        match node.kind {
            WidgetKind::Label => Some(id),
            WidgetKind::Button => node.children.first().copied(),
            WidgetKind::Panel | WidgetKind::Image | WidgetKind::Toggle | WidgetKind::Slider => None,
        }
    }

    /// Sets a label's or a button's text. Returns `false`, and marks nothing,
    /// when the text is equal (compare-on-write, w01 §2) or `id` holds no
    /// text.
    pub fn set_text(&mut self, id: WidgetId, text: impl Into<StyledText>) -> bool {
        let Some(target) = self.text_holder(id) else {
            return false;
        };
        let text = text.into();
        let Some(node) = self.node_mut(target) else {
            return false;
        };
        if node
            .text
            .as_ref()
            .is_some_and(|current| current.text == text)
        {
            return false;
        }
        node.text = Some(TextContent::new(text));
        node.text_stale = true;
        self.mark_layout_dirty(target);
        true
    }

    /// A label's text, or a button's label's.
    #[must_use]
    pub fn text(&self, id: WidgetId) -> Option<&StyledText> {
        let target = self.text_holder(id)?;
        self.node(target)?
            .text
            .as_ref()
            .map(|content| &content.text)
    }

    // ---- layout style ----

    /// Replaces the layout style; an equal style marks nothing.
    pub fn set_layout_style(&mut self, id: WidgetId, style: LayoutStyle) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.layout_style == style {
            return;
        }
        node.layout_style = style;
        self.mark_layout_dirty(id);
    }

    /// The layout style.
    #[must_use]
    pub fn layout_style(&self, id: WidgetId) -> Option<&LayoutStyle> {
        self.node(id).map(|node| &node.layout_style)
    }

    // ---- flags ----

    /// Shows, hides (keeping the box) or collapses `id` and its subtree.
    /// Hiding or collapsing releases focus, hover and a press held inside it;
    /// focus goes back to where it was when a scope under `id` was entered,
    /// as [`Self::remove`] does (`D-149`).
    pub fn set_visibility(&mut self, id: WidgetId, visibility: Visibility) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.visibility == visibility {
            return;
        }
        node.visibility = visibility;
        if visibility != Visibility::Visible {
            self.drop_pointer_under(id);
            self.hide_focus_under(id);
        }
        self.mark_layout_dirty(id);
    }

    /// The widget's own visibility, not its ancestors'.
    #[must_use]
    pub fn visibility(&self, id: WidgetId) -> Option<Visibility> {
        self.node(id).map(|node| node.visibility)
    }

    /// Enables or disables `id`. A disabled widget keeps its box and blocks
    /// what is behind it; it and every widget inside it take no focus,
    /// press or click (`D-125`), so disabling releases the focus, hover and
    /// press held anywhere under it. Its own `disabled` variant applies; the
    /// widgets inside keep theirs. Marks paint, and the node's style for the
    /// next resolution, which relayouts its root when the `disabled` variant
    /// changes the text style.
    pub fn set_enabled(&mut self, id: WidgetId, enabled: bool) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.enabled == enabled {
            return;
        }
        node.enabled = enabled;
        if !enabled {
            if self
                .focused
                .is_some_and(|focused| self.is_within(focused, id))
            {
                self.set_focused(None);
            }
            self.drop_pointer_under(id);
        }
        self.mark_style_dirty(id);
        self.mark_paint_dirty();
    }

    /// Whether `id` itself is enabled, whatever its ancestors are; `false`
    /// for an invalid ID.
    #[must_use]
    pub fn enabled(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.enabled)
    }

    /// Sets the accessibility role. Marks nothing.
    pub fn set_role(&mut self, id: WidgetId, role: Role) {
        if let Some(node) = self.node_mut(id) {
            node.role = role;
        }
    }

    /// The accessibility role.
    #[must_use]
    pub fn role(&self, id: WidgetId) -> Option<Role> {
        self.node(id).map(|node| node.role)
    }

    /// Sets the name assistive technology would read, or clears it so a
    /// label's or button's text is the name again. Marks nothing.
    pub fn set_accessible_name(&mut self, id: WidgetId, name: Option<String>) {
        if let Some(node) = self.node_mut(id) {
            node.accessible_name = name;
        }
    }

    /// The accessible name: the one set, else a label's text or a button's
    /// label's text.
    #[must_use]
    pub fn accessible_name(&self, id: WidgetId) -> Option<&str> {
        let node = self.node(id)?;
        if let Some(name) = &node.accessible_name {
            return Some(name);
        }
        let target = self.text_holder(id)?;
        self.node(target)?
            .text
            .as_ref()
            .map(|content| content.flat.as_str())
    }

    /// Marks `id` as a widget the pointer acts on (`D-149`): a hit's target
    /// is its nearest enabled interactive ancestor-or-self, which hovers,
    /// presses and gets the `UiEvent`s. Buttons start interactive. Marks
    /// nothing.
    pub fn set_interactive(&mut self, id: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(id) {
            node.interactive = on;
        }
    }

    /// Whether `id` is interactive; `false` for an invalid ID.
    #[must_use]
    pub fn interactive(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.interactive)
    }

    /// Lets `id` take keyboard focus. Marks nothing, except that the focused
    /// widget made unfocusable loses focus.
    pub fn set_focusable(&mut self, id: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(id) {
            node.focusable = on;
        }
        if !on && self.focused == Some(id) {
            self.set_focused(None);
        }
    }

    /// Whether `id` may take focus; `false` for an invalid ID.
    #[must_use]
    pub fn focusable(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.focusable)
    }

    /// Keeps `focus_next` and `focus_prev` inside `id`'s subtree while focus
    /// is in it. Marks nothing.
    pub fn set_focus_scope(&mut self, id: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(id) {
            node.focus_scope = on;
        }
    }

    /// Whether `id` is a focus scope; `false` for an invalid ID.
    #[must_use]
    pub fn focus_scope(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.focus_scope)
    }

    /// Whether the pointer sees `id`; off for pointer-transparent HUD text
    /// (w01 §6). Marks nothing.
    pub fn set_hit_testable(&mut self, id: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(id) {
            node.hit_testable = on;
        }
    }

    /// Whether the pointer sees `id`; `false` for an invalid ID.
    #[must_use]
    pub fn hit_testable(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.hit_testable)
    }

    /// Makes `root` modal, or not (`D-149`). While a modal root is visible and
    /// laid out it blocks every root below it in hit order
    /// ([`Self::hit_test`]); roots above it, Debug roots among them, still
    /// hit, and the router gives the UI every key press. Does nothing for a
    /// widget that is not a root. Marks nothing.
    pub fn set_modal(&mut self, root: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(root)
            && node.parent.is_none()
        {
            node.modal = on;
        }
    }

    /// Whether `id` is a modal root; `false` for any other widget or an
    /// invalid ID.
    #[must_use]
    pub fn modal(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.modal)
    }

    /// Sets the system cursor the pointer shows over `id` and over the
    /// widgets under it that set none, or clears it with `None` so `id`
    /// shows its nearest ancestor's (`D-149`). Marks nothing.
    pub fn set_cursor(&mut self, id: WidgetId, cursor: Option<CursorIcon>) {
        if let Some(node) = self.node_mut(id) {
            node.cursor = cursor;
        }
    }

    /// The cursor set on `id` itself, not inherited; `None` when none is set
    /// or for an invalid ID.
    #[must_use]
    pub fn cursor(&self, id: WidgetId) -> Option<CursorIcon> {
        self.node(id)?.cursor
    }

    // ---- interaction state ----

    /// The widget the pointer hovers: the target under it, or the captured
    /// widget while the pointer is over it (`D-149`). The router sets it;
    /// removal, hiding and disabling clear it.
    #[must_use]
    pub fn hovered(&self) -> Option<WidgetId> {
        self.hovered
    }

    /// The widget shown pressed: the captured widget while the pointer is
    /// over it (`D-149`). The router sets it; removal, hiding and disabling
    /// clear it.
    #[must_use]
    pub fn pressed(&self) -> Option<WidgetId> {
        self.pressed
    }

    /// Whether focus is visible, so the focused widget layers its `focused`
    /// variant: after keyboard use, not after pointer use (`D-149`). True on
    /// a new tree, so a focus set at build shows.
    #[must_use]
    pub fn focus_visible(&self) -> bool {
        !self.focus_hidden
    }

    /// Moves the hover to `next`, marking both widgets' styles.
    pub(crate) fn set_hovered(&mut self, next: Option<WidgetId>) {
        if self.hovered == next {
            return;
        }
        if let Some(previous) = self.hovered {
            self.mark_style_dirty(previous);
        }
        self.hovered = next;
        if let Some(next) = next {
            self.mark_style_dirty(next);
        }
    }

    /// Moves the press to `next`, marking both widgets' styles.
    pub(crate) fn set_pressed(&mut self, next: Option<WidgetId>) {
        if self.pressed == next {
            return;
        }
        if let Some(previous) = self.pressed {
            self.mark_style_dirty(previous);
        }
        self.pressed = next;
        if let Some(next) = next {
            self.mark_style_dirty(next);
        }
    }

    /// Shows or hides focus, marking the focused widget's style.
    pub(crate) fn set_focus_visible(&mut self, visible: bool) {
        let hidden = !visible;
        if self.focus_hidden == hidden {
            return;
        }
        self.focus_hidden = hidden;
        if let Some(focused) = self.focused {
            self.mark_style_dirty(focused);
        }
    }

    /// Clears the hover and the press when they sit on `id` or under it; for
    /// a widget that was hidden or collapsed.
    fn drop_pointer_under(&mut self, id: WidgetId) {
        if self
            .hovered
            .is_some_and(|hovered| self.is_within(hovered, id))
        {
            self.set_hovered(None);
        }
        if self
            .pressed
            .is_some_and(|pressed| self.is_within(pressed, id))
        {
            self.set_pressed(None);
        }
    }

    // ---- theme and style ----

    /// The theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Replaces the theme: bumps the epoch, marks every node for resolution
    /// and every root for layout. The tree then follows no theme asset
    /// ([`Self::theme_id`] is `None`) until [`Self::set_theme_asset`].
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme_id = None;
        self.apply_theme(theme);
    }

    /// [`Self::set_theme`] with the `ui_themes` ID `theme` was loaded from,
    /// so a reload of that asset re-applies it to this tree (Q9).
    pub fn set_theme_asset(&mut self, id: &str, theme: Theme) {
        self.theme_id = Some(id.to_owned());
        self.apply_theme(theme);
    }

    fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.theme_epoch = self.theme_epoch.next();
        for slot in &mut self.slots {
            if let Some(node) = &mut slot.node {
                node.style_dirty = true;
                node.style_dirty_subtree = true;
                node.layout_dirty = true;
            }
        }
        self.paint_dirty = true;
    }

    /// The `ui_themes` ID the theme came from; `None` for the built-in theme
    /// or one set by hand.
    #[must_use]
    pub fn theme_id(&self) -> Option<&str> {
        self.theme_id.as_deref()
    }

    /// Rises on every [`Self::set_theme`] and [`Self::set_theme_asset`].
    #[must_use]
    pub fn theme_epoch(&self) -> ThemeEpoch {
        self.theme_epoch
    }

    /// Replaces the node's own overrides, applied after the kind's state
    /// variant; equal overrides mark nothing.
    pub fn set_overrides(&mut self, id: WidgetId, overrides: StyleOverrides) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.overrides == overrides {
            return;
        }
        node.overrides = overrides;
        self.mark_style_dirty(id);
        self.mark_paint_dirty();
    }

    /// The node's own overrides.
    #[must_use]
    pub fn overrides(&self, id: WidgetId) -> Option<&StyleOverrides> {
        self.node(id).map(|node| &node.overrides)
    }

    /// The last resolution; `None` before the first [`Self::layout`].
    #[must_use]
    pub fn resolved_style(&self, id: WidgetId) -> Option<&ResolvedStyle> {
        self.node(id)?.resolved.as_ref()
    }

    /// The top state the node's style layers (`D-149`): `Disabled` when
    /// disabled, else `Pressed`, `Hovered`, or `Focused` while it holds
    /// visible focus, else `Normal`.
    #[must_use]
    pub fn widget_state(&self, id: WidgetId) -> Option<WidgetState> {
        let node = self.node(id)?;
        Some(
            self.style_layers(id, node)
                .into_iter()
                .rev()
                .find(|state| *state != WidgetState::Normal)
                .unwrap_or_default(),
        )
    }

    /// The states whose variants `id`'s style applies, in order: `disabled`
    /// alone; otherwise `focused` while focus is on it and visible, then
    /// `hovered`, then `pressed`. `Normal` fills the slots not applied.
    fn style_layers(&self, id: WidgetId, node: &Node) -> [WidgetState; 3] {
        let normal = WidgetState::Normal;
        if !node.enabled {
            return [WidgetState::Disabled, normal, normal];
        }
        let layer = |on: bool, state: WidgetState| if on { state } else { normal };
        [
            layer(
                self.focused == Some(id) && !self.focus_hidden,
                WidgetState::Focused,
            ),
            layer(self.hovered == Some(id), WidgetState::Hovered),
            layer(self.pressed == Some(id), WidgetState::Pressed),
        ]
    }

    /// Marks `id` for the next resolution pass, and its root's subtree.
    pub(crate) fn mark_style_dirty(&mut self, id: WidgetId) {
        let Some(root) = self.node(id).map(|node| node.root) else {
            return;
        };
        if let Some(node) = self.node_mut(id) {
            node.style_dirty = true;
        }
        if let Some(root) = self.node_mut(root) {
            root.style_dirty_subtree = true;
        }
    }

    /// Resolves every style-dirty node, and every node under one whose
    /// resolved text changed, on the roots that hold one. A node whose
    /// resolved text style changed in any property marks its root for
    /// layout, since the text node must be re-set and committed again
    /// (`D-117`); a non-text change marks paint. A toggle or a slider holds
    /// no text node, so its text change marks layout only when it measures
    /// differently (its line height sizes it), and paint when only the
    /// colour moved. [`Self::layout`] runs this first.
    pub(crate) fn resolve_styles(&mut self) -> StyleStats {
        let mut stats = StyleStats::default();
        let base = self.theme.base_text();
        let roots = self.roots.clone();
        for root in roots {
            if !self.node(root).is_some_and(|node| node.style_dirty_subtree) {
                continue;
            }
            self.resolve_subtree(root, &base, false, &mut stats);
            if let Some(node) = self.node_mut(root) {
                node.style_dirty_subtree = false;
            }
        }
        stats
    }

    fn resolve_subtree(
        &mut self,
        id: WidgetId,
        parent_text: &TextStyle,
        parent_text_changed: bool,
        stats: &mut StyleStats,
    ) {
        let Some(node) = self.node(id) else {
            return;
        };
        let children = node.children.clone();
        let mut text_changed = false;
        if node.style_dirty || parent_text_changed {
            let layers = self.style_layers(id, node);
            let holds_text = !matches!(node.kind, WidgetKind::Toggle | WidgetKind::Slider);
            let resolved = theme::resolve_layers(
                &self.theme,
                node.kind,
                &layers,
                &node.overrides,
                parent_text,
            );
            let (changed_text, changed_measure, changed_visual) = match &node.resolved {
                Some(old) => (
                    old.text != resolved.text,
                    old.text_measures_differently(&resolved),
                    old.background != resolved.background
                        || old.border != resolved.border
                        || old.border_width != resolved.border_width
                        || old.corner_radius != resolved.corner_radius,
                ),
                None => (true, true, true),
            };
            stats.resolved += 1;
            stats.text_changed += u32::from(changed_text);
            stats.measurement_changed += u32::from(changed_measure);
            if let Some(node) = self.node_mut(id) {
                node.resolved = Some(resolved);
                node.style_dirty = false;
                node.text_stale |= changed_text;
            }
            if changed_text && (holds_text || changed_measure) {
                self.mark_layout_dirty(id);
            } else if changed_text || changed_visual {
                self.paint_dirty = true;
            }
            text_changed = changed_text;
        }
        let text = self
            .node(id)
            .and_then(|node| node.resolved.as_ref())
            .map_or_else(|| parent_text.clone(), |resolved| resolved.text.clone());
        for child in children {
            self.resolve_subtree(child, &text, text_changed, stats);
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/tree.rs"]
mod tests;
