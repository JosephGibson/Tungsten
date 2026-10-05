//! Hit testing (`D-125`): layers high to low, roots and siblings latest
//! first, the deepest visible hit-testable widget inside every ancestor's
//! box. Disabled widgets are hit and block what is behind them; hidden and
//! collapsed subtrees are skipped.

use glam::Vec2;

use super::id::WidgetId;
use super::node::Visibility;
use super::tree::UiTree;

impl UiTree {
    /// The deepest visible, hit-testable widget under `point` (viewport
    /// space), on the highest layer and the latest root and sibling first. A
    /// widget counts only inside its own committed rect and every ancestor's,
    /// so the ancestor box is the first clip model. Disabled widgets count
    /// and block; hidden, collapsed and non-hit-testable ones do not, and
    /// what is behind them is hit. `None` before the first layout.
    #[must_use]
    pub fn hit_test(&self, point: Vec2) -> Option<WidgetId> {
        self.roots
            .iter()
            .rev()
            .find_map(|root| self.hit_node(*root, point))
    }

    fn hit_node(&self, id: WidgetId, point: Vec2) -> Option<WidgetId> {
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible || !node.rect?.contains(point) {
            return None;
        }
        node.children
            .iter()
            .rev()
            .find_map(|child| self.hit_node(*child, point))
            .or_else(|| node.hit_testable.then_some(id))
    }
}

#[cfg(test)]
#[path = "../tests/ui/hit.rs"]
mod tests;
