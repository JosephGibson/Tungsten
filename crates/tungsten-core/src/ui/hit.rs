//! Hit testing (`D-125`, through the paint clip stack from M3, `D-149`): paint
//! order reversed, so layers high to low, roots and siblings latest first,
//! children before their parent; a widget counts inside its own box and
//! every clipping ancestor's, as its paint is clipped. A shown modal root
//! blocks the roots below it. Disabled widgets are hit and block what is
//! behind them; hidden and collapsed subtrees are skipped. From M56 a
//! widget's box is where it paints, under its and its ancestors'
//! translation and scale; opacity and a root's freeze change nothing here.
//! From M57 a scroll panel's children are hit where its offset moves them.
//! From M58 a debug root is hit in its own units: the point divided by the
//! debug scale of the last layout. Shadows are not hit.

use glam::Vec2;

use super::geometry::{Rect, UiTransform};
use super::id::WidgetId;
use super::node::{UiLayer, Visibility};
use super::paint::node_transform;
use super::scroll::children_transform;
use super::tree::{MIN_SCALE, UiTree};

impl UiTree {
    /// The deepest visible, hit-testable widget under `point` (viewport
    /// space), in paint order reversed: the highest layer, the latest root
    /// and sibling, and children before their parent first. A widget counts
    /// inside its [`visual_rect`](Self::visual_rect), edges included, when
    /// that point is also inside every
    /// [`clip_children`](Self::set_clip_children) ancestor's and the
    /// viewport, as its paint is clipped; an ancestor that does not clip
    /// does not bound it, and a subtree whose composed scale is below 1/64
    /// is skipped. Only visible, laid-out
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
        let viewport = self.viewport?;
        for &root in self.roots.iter().rev() {
            if !self.root_routes(root, hide_game_layers) {
                continue;
            }
            // In the root's units: a debug root's span the debug scale (M58).
            let layer = self
                .node(root)
                .map_or(1.0, |node| self.layer_scale(node.layer));
            let clip = Rect::from_pos_size(Vec2::ZERO, viewport / layer);
            if let Some(hit) = self.hit_node(root, point / layer, clip, UiTransform::IDENTITY) {
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

    /// The hit in `id`'s subtree, inside `clip`: the clip `id` paints in,
    /// under its ancestors' composed map `parent`, all in its root's units.
    fn hit_node(
        &self,
        id: WidgetId,
        point: Vec2,
        clip: Rect,
        parent: UiTransform,
    ) -> Option<WidgetId> {
        if !clip.contains(point) {
            return None;
        }
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible {
            return None;
        }
        let layout = node.rect?;
        let unit = self.unit_scale(node.layer);
        let transform = parent.compose(node_transform(node, layout, unit));
        if transform.scale < MIN_SCALE {
            return None;
        }
        let rect = transform.rect(layout);
        let child_clip = if node.clip_children {
            intersect(clip, rect)
        } else {
            Some(clip)
        };
        let children = transform.compose(children_transform(node, unit));
        if let Some(child_clip) = child_clip
            && let Some(hit) = node
                .children
                .iter()
                .rev()
                .find_map(|child| self.hit_node(*child, point, child_clip, children))
        {
            return Some(hit);
        }
        (node.hit_testable && rect.contains(point)).then_some(id)
    }

    /// The scale translations round at in a root on `layer`, in physical
    /// pixels per unit of that root: the last paint's scale, else 1, times
    /// the layer's scale (M58).
    fn unit_scale(&self, layer: UiLayer) -> f32 {
        self.paint_scale.unwrap_or(1.0) * self.layer_scale(layer)
    }

    /// Where `id` paints and is hit, in viewport space (M56): its committed
    /// box under its own and its ancestors' translation and scale, and its
    /// scroll panel ancestors' offsets (M57), with translations rounded at
    /// the last paint's scale; under a debug root, mapped from the root's
    /// units into the tree's (M58). Equal to [`Self::rect`] on a game layer
    /// when nothing moves it; `None` when that is.
    #[must_use]
    pub fn visual_rect(&self, id: WidgetId) -> Option<Rect> {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(at) = current {
            let node = self.node(at)?;
            chain.push((node, node.rect?));
            current = node.parent;
        }
        let (own, ancestors) = chain.split_first()?;
        let layer = self.layer_scale(own.0.layer);
        let unit = self.unit_scale(own.0.layer);
        let parent = ancestors
            .iter()
            .rev()
            .fold(UiTransform::IDENTITY, |parent, (node, rect)| {
                parent
                    .compose(node_transform(node, *rect, unit))
                    .compose(children_transform(node, unit))
            });
        let transform = parent.compose(node_transform(own.0, own.1, unit));
        let rect = transform.rect(own.1);
        Some(Rect::new(rect.min * layer, rect.max * layer))
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
