//! The `UiTree` resource (`D-125`): slots behind generational IDs, roots on
//! layers, building, queries, the behaviour flags and the dirty flags. Every
//! mutation is a method call that marks only what it changes: text,
//! visibility and layout style mark layout (and so paint); enabled marks
//! paint; roles, names and the behaviour flags mark nothing.

use crate::text::{FontEpoch, StyledText, TextNodeId, TextStyle};

use super::id::WidgetId;
use super::node::{Node, Role, TextContent, UiLayer, Visibility, WidgetKind};
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
    /// The widget holding keyboard focus.
    pub(crate) focused: Option<WidgetId>,
    /// Text nodes of removed labels, freed at the next layout.
    pub(crate) pending_text_release: Vec<TextNodeId>,
    /// The store's font epoch at the last layout; `None` before the first.
    pub(crate) font_epoch: Option<FontEpoch>,
    /// Per focus scope entered, the focus left behind, restored when the
    /// scope is removed.
    pub(crate) focus_return: Vec<(WidgetId, Option<WidgetId>)>,
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

    fn add_child(&mut self, parent: WidgetId, kind: WidgetKind, style: LayoutStyle) -> WidgetId {
        let (root, layer) = self.node(parent).map_or_else(
            || panic!("UiTree: parent {parent:?} is not in the tree"),
            |node| {
                assert!(
                    node.kind != WidgetKind::Label,
                    "UiTree: a label holds no children ({parent:?})"
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

    /// Removes `id` and its subtree. Focus held inside it is released and its
    /// text nodes are freed at the next layout. An invalid ID does nothing.
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
            WidgetKind::Panel => None,
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
    pub fn set_visibility(&mut self, id: WidgetId, visibility: Visibility) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.visibility == visibility {
            return;
        }
        node.visibility = visibility;
        if visibility != Visibility::Visible {
            self.drop_focus_under(id);
        }
        self.mark_layout_dirty(id);
    }

    /// The widget's own visibility, not its ancestors'.
    #[must_use]
    pub fn visibility(&self, id: WidgetId) -> Option<Visibility> {
        self.node(id).map(|node| node.visibility)
    }

    /// Enables or disables `id`. A disabled widget keeps its box and blocks
    /// what is behind it; it holds no focus, so disabling the focused widget
    /// clears focus. Marks paint, and the node's style for the next
    /// resolution, which relayouts its root when the `disabled` variant
    /// changes the text style.
    pub fn set_enabled(&mut self, id: WidgetId, enabled: bool) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.enabled == enabled {
            return;
        }
        node.enabled = enabled;
        if !enabled && self.focused == Some(id) {
            self.set_focused(None);
        }
        self.mark_style_dirty(id);
        self.mark_paint_dirty();
    }

    /// Whether `id` is enabled; `false` for an invalid ID.
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

    /// Marks a custom widget as one M3 emits events for (w01 §14). Marks
    /// nothing now.
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

    // ---- theme and style ----

    /// The theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Replaces the theme: bumps the epoch, marks every node for resolution
    /// and every root for layout.
    pub fn set_theme(&mut self, theme: Theme) {
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

    /// Rises on every [`Self::set_theme`].
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

    /// The state the node's style resolves for: `Disabled` when disabled,
    /// `Focused` while it holds focus, else `Normal`.
    #[must_use]
    pub fn widget_state(&self, id: WidgetId) -> Option<WidgetState> {
        let node = self.node(id)?;
        Some(if !node.enabled {
            WidgetState::Disabled
        } else if self.focused == Some(id) {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        })
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
    /// (`D-117`); a non-text change marks paint. [`Self::layout`] runs this
    /// first.
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
            let state = self.widget_state(id).unwrap_or_default();
            let resolved =
                theme::resolve(&self.theme, node.kind, state, &node.overrides, parent_text);
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
            if changed_text {
                self.mark_layout_dirty(id);
            } else if changed_visual {
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
