//! The layout pass (`D-124`, `D-125`): dirty roots only, styles resolved
//! first, text nodes created, set and committed through the
//! [`TextNodeStore`], viewport-space rects, and the font-epoch check that
//! relayouts every root with text when the fonts changed.

use std::fmt::Write as _;

use glam::Vec2;

use crate::text::{TextNodeId, TextNodeStore};

use super::geometry::Rect;
use super::id::WidgetId;
use super::node::{Visibility, WidgetKind};
use super::tree::{LayoutStats, UiTree};

pub(crate) mod taffy;

impl UiTree {
    /// Resolves styles and lays out the dirty roots in a `viewport`-sized
    /// box (window pixels until M0b), creating, setting and committing text
    /// nodes through `text`. Text nodes of removed labels are freed first. A
    /// font epoch that moved since the last pass relayouts every root with
    /// text, since their commits are stale (`D-117`). A root with no change
    /// costs nothing.
    pub fn layout(&mut self, viewport: Vec2, text: &mut dyn TextNodeStore) -> LayoutStats {
        for node in self.pending_text_release.drain(..) {
            text.remove_node(node);
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

        let mut stats = LayoutStats::default();
        let roots = self.roots.clone();
        for root in roots {
            if !self.node(root).is_some_and(|node| node.layout_dirty) {
                continue;
            }
            self.prepare_text(root, text);
            let result = taffy::lay_out_root(self, root, viewport, text);
            stats.roots_laid_out += 1;
            stats.nodes_laid_out += result.nodes;
            stats.measure_calls += result.measure_calls;
            stats.commits += result.commits;
            if let Some(node) = self.node_mut(root) {
                node.layout_dirty = false;
            }
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
    /// the stale ones: new labels, changed text and changed resolved text
    /// styles.
    fn prepare_text(&mut self, id: WidgetId, store: &mut dyn TextNodeStore) {
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
                store.set_text(text_node, &content.text, &resolved.text);
            }
            if let Some(node) = self.node_mut(id) {
                node.text_id = Some(text_node);
                node.text_stale = false;
            }
        }
        for child in children {
            self.prepare_text(child, store);
        }
    }

    /// The committed box in viewport space: `None` before the first layout,
    /// when collapsed, or under a collapsed ancestor.
    #[must_use]
    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.node(id)?.rect
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
