//! The layout pass (`D-124`, `D-125`): dirty roots only, styles resolved
//! first, text nodes created, set and committed through the
//! [`TextNodeStore`], viewport-space rects, and the font-epoch check that
//! relayouts every root with text when the fonts changed. The debug layers'
//! roots lay out in the viewport divided by the debug scale, in units of
//! their own, and skip the text scale (M58).

use std::fmt::Write as _;

use glam::Vec2;

use crate::text::{TextNodeId, TextNodeStore};

use super::geometry::Rect;
use super::id::WidgetId;
use super::node::{UiLayer, Visibility, WidgetKind};
use super::tree::{LayoutStats, UiTree};

pub(crate) mod taffy;

impl UiTree {
    /// Resolves styles and lays out the dirty roots in a `viewport`-sized
    /// box (logical UI units, `D-127`), creating, setting and committing
    /// text nodes through `text`; a `Debug` or `DebugPopup` root lays out in
    /// `viewport` divided by [`Self::debug_scale`], in its own units (M58).
    /// Text nodes of removed labels are freed first. A font epoch that moved
    /// since the last pass relayouts every root with text, since their
    /// commits are stale (`D-117`); a viewport that differs from the last
    /// pass's relayouts every root, so a static UI follows a resize. A root
    /// with no change costs nothing.
    pub fn layout(&mut self, viewport: Vec2, text: &mut dyn TextNodeStore) -> LayoutStats {
        for node in self.pending_text_release.drain(..) {
            text.remove_node(node);
        }
        if self.viewport != Some(viewport) {
            let roots = self.roots.clone();
            for root in roots {
                if let Some(node) = self.node_mut(root) {
                    node.layout_dirty = true;
                }
            }
            self.viewport = Some(viewport);
            self.paint_dirty = true;
        }
        let epoch = text.font_epoch();
        if self.font_epoch != Some(epoch) {
            if self.font_epoch.is_some() {
                let roots = self.roots.clone();
                for root in roots {
                    if self.subtree_has_text_node(root)
                        && let Some(node) = self.node_mut(root)
                    {
                        node.layout_dirty = true;
                    }
                }
            }
            self.font_epoch = Some(epoch);
        }
        self.resolve_styles();
        // Every debug root is dirty after a change of the debug scale, so
        // the clean ones' rects are already in this one.
        self.laid_debug_scale = Some(self.debug_scale());

        let mut stats = LayoutStats::default();
        let roots = self.roots.clone();
        for root in roots {
            let Some(layer) = self
                .node(root)
                .filter(|node| node.layout_dirty)
                .map(|node| node.layer)
            else {
                continue;
            };
            let text_scale = self.layer_text_scale(layer);
            self.prepare_text(root, text_scale, text);
            let root_viewport = viewport / self.layer_scale(layer);
            let result = taffy::lay_out_root(self, root, root_viewport, text_scale, text);
            stats.roots_laid_out += 1;
            stats.nodes_laid_out += result.nodes;
            stats.measure_calls += result.measure_calls;
            stats.commits += result.commits;
            if let Some(node) = self.node_mut(root) {
                node.layout_dirty = false;
            }
        }
        if stats.roots_laid_out > 0 {
            // Boxes moved, so the paint list must follow, whichever order a
            // caller runs its layout and paint in.
            self.paint_dirty = true;
        }
        stats
    }

    /// Whether any label under `id` holds a text node.
    fn subtree_has_text_node(&self, id: WidgetId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        node.text_id.is_some()
            || node
                .children
                .iter()
                .any(|child| self.subtree_has_text_node(*child))
    }

    /// Gives every label in `id`'s non-collapsed subtree a text node and sets
    /// the stale ones: new labels, changed text, changed resolved text
    /// styles and a changed text scale, which multiplies the size and line
    /// height set here (M56); `scale` is the root's layer's
    /// ([`Self::layer_text_scale`]).
    fn prepare_text(&mut self, id: WidgetId, scale: f32, store: &mut dyn TextNodeStore) {
        let Some(node) = self.node(id) else {
            return;
        };
        if node.visibility == Visibility::Collapsed {
            return;
        }
        let children = node.children.clone();
        if node.kind == WidgetKind::Label {
            let text_node = node.text_id.unwrap_or_else(|| store.create_node());
            if (node.text_stale || node.text_id.is_none())
                && let (Some(content), Some(resolved)) = (&node.text, &node.resolved)
            {
                if scale == 1.0 {
                    store.set_text(text_node, &content.text, &resolved.text);
                } else {
                    let mut style = resolved.text.clone();
                    style.size *= scale;
                    style.line_height *= scale;
                    store.set_text(text_node, &content.text, &style);
                }
            }
            if let Some(node) = self.node_mut(id) {
                node.text_id = Some(text_node);
                node.text_stale = false;
            }
        }
        for child in children {
            self.prepare_text(child, scale, store);
        }
    }

    /// The committed box in viewport space: `None` before the first layout,
    /// when collapsed, or under a collapsed ancestor. A debug root's boxes
    /// are in its layer's units (M58); [`Self::visual_rect`] gives them in
    /// the tree's.
    #[must_use]
    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.node(id)?.rect
    }

    /// The viewport the last layout gave `layer`'s roots, in their units
    /// (M58): the viewport on the game layers, and on the debug layers the
    /// viewport divided by the debug scale it used. `None` before the first
    /// layout.
    #[must_use]
    pub fn layer_viewport(&self, layer: UiLayer) -> Option<Vec2> {
        self.viewport
            .map(|viewport| viewport / self.layer_scale(layer))
    }

    /// The text node a label shapes with; `None` before its first layout, for
    /// a widget that is not a label, or for an invalid ID. M2's paint list
    /// draws it.
    #[must_use]
    pub fn text_node(&self, id: WidgetId) -> Option<TextNodeId> {
        self.node(id)?.text_id
    }

    /// One line per node, depth-indented: kind, role, visibility and the rect
    /// as `(x, y) w×h` to one decimal, or `-` for none. The snapshot-test
    /// format.
    #[must_use]
    pub fn dump_layout(&self) -> String {
        let mut out = String::new();
        for root in &self.roots {
            self.dump_node(*root, 0, &mut out);
        }
        out
    }

    fn dump_node(&self, id: WidgetId, depth: usize, out: &mut String) {
        let Some(node) = self.node(id) else {
            return;
        };
        for _ in 0..depth {
            out.push_str("  ");
        }
        let _ = write!(
            out,
            "{:?} {:?} {:?} ",
            node.kind, node.role, node.visibility
        );
        match node.rect {
            Some(rect) => {
                let _ = writeln!(
                    out,
                    "({:.1}, {:.1}) {:.1}×{:.1}",
                    rect.min.x,
                    rect.min.y,
                    rect.width(),
                    rect.height()
                );
            }
            None => out.push_str("-\n"),
        }
        for child in &node.children {
            self.dump_node(*child, depth + 1, out);
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/layout.rs"]
mod tests;
