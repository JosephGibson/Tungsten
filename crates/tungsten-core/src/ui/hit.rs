//! Hit testing (`D-125`, through the paint clip stack from M3, `D-149`): paint
//! order reversed, so layers high to low, roots and siblings latest first,
//! children before their parent; a widget counts inside its own box and
//! every clipping ancestor's, as its paint is clipped. A shown modal root
//! blocks the roots below it. Disabled widgets are hit and block what is
//! behind them; hidden and collapsed subtrees are skipped.

use glam::Vec2;

use super::geometry::Rect;
use super::id::WidgetId;
use super::node::{UiLayer, Visibility};
use super::tree::UiTree;

impl UiTree {
    /// The deepest visible, hit-testable widget under `point` (viewport
    /// space), in paint order reversed: the highest layer, the latest root
    /// and sibling, and children before their parent first. A widget counts
    /// inside its own committed rect, edges included, when that point is
    /// also inside every [`clip_children`](Self::set_clip_children)
    /// ancestor's rect and the viewport, as its paint is clipped; an
    /// ancestor that does not clip does not bound it. Only visible, laid-out
    /// roots count, and a modal one ([`Self::set_modal`]) blocks every root
    /// below it, so a point it misses hits nothing. Disabled widgets count
    /// and block; hidden, collapsed and non-hit-testable ones do not, and
    /// what is behind them is hit. `None` before the first layout.
    #[must_use]
    pub fn hit_test(&self, point: Vec2) -> Option<WidgetId> {
        self.hit_test_layers(point, false)
    }

    /// [`Self::hit_test`] with the `Game` and `GamePopup` roots left out
    /// while `hide_game_layers` is set, as their paint is.
    pub(crate) fn hit_test_layers(&self, point: Vec2, hide_game_layers: bool) -> Option<WidgetId> {
        let viewport = Rect::from_pos_size(Vec2::ZERO, self.viewport?);
        for &root in self.roots.iter().rev() {
            if !self.root_routes(root, hide_game_layers) {
                continue;
            }
            if let Some(hit) = self.hit_node(root, point, viewport) {
                return Some(hit);
            }
            if self.modal(root) {
                return None;
            }
        }
        None
    }

    /// Whether `root` takes input: visible and laid out, and not on a game
    /// layer while `hide_game_layers` is set.
    pub(crate) fn root_routes(&self, root: WidgetId, hide_game_layers: bool) -> bool {
        self.node(root).is_some_and(|node| {
            node.visibility == Visibility::Visible
                && node.rect.is_some()
                && !(hide_game_layers && matches!(node.layer, UiLayer::Game | UiLayer::GamePopup))
        })
    }

    /// The hit in `id`'s subtree, inside `clip`: the clip `id` paints in.
    fn hit_node(&self, id: WidgetId, point: Vec2, clip: Rect) -> Option<WidgetId> {
        if !clip.contains(point) {
            return None;
        }
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible {
            return None;
        }
        let rect = node.rect?;
        let child_clip = if node.clip_children {
            intersect(clip, rect)
        } else {
            Some(clip)
        };
        if let Some(child_clip) = child_clip
            && let Some(hit) = node
                .children
                .iter()
                .rev()
                .find_map(|child| self.hit_node(*child, point, child_clip))
        {
            return Some(hit);
        }
        (node.hit_testable && rect.contains(point)).then_some(id)
    }
}

/// The part of both rects; `None` when it has no area, as the paint list
/// skips a clip that covers no pixel.
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let rect = Rect::new(a.min.max(b.min), a.max.min(b.max));
    (rect.width() > 0.0 && rect.height() > 0.0).then_some(rect)
}

#[cfg(test)]
#[path = "../tests/ui/hit.rs"]
mod tests;
