//! Keyboard focus (`D-125`): one focused widget, tab order in depth-first
//! tree order inside the nearest focus scope, wrapping, and the rules that
//! keep `focused()` from naming a widget the traversal would skip. M3 drives
//! these from the `ui_*` actions; M1 exposes the calls.

use super::id::WidgetId;
use super::node::Visibility;
use super::tree::UiTree;

impl UiTree {
    /// Whether `id` can hold focus now: focusable, enabled, visible, and
    /// under no hidden or collapsed ancestor.
    #[must_use]
    pub fn can_focus(&self, id: WidgetId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        node.focusable && node.enabled && self.is_shown(id)
    }

    /// Whether `id` and every ancestor are visible.
    pub(crate) fn is_shown(&self, id: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(id) = current {
            let Some(node) = self.node(id) else {
                return false;
            };
            if node.visibility != Visibility::Visible {
                return false;
            }
            current = node.parent;
        }
        true
    }

    /// Whether `ancestor` is `id` or one of its ancestors.
    pub(crate) fn is_within(&self, id: WidgetId, ancestor: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.node(id).and_then(|node| node.parent);
        }
        false
    }

    /// The scope `id`'s traversal stays in: itself or the nearest ancestor
    /// flagged a focus scope, else its root.
    fn scope_of(&self, id: WidgetId) -> Option<WidgetId> {
        let mut current = Some(id);
        while let Some(id) = current {
            let node = self.node(id)?;
            if node.focus_scope || node.parent.is_none() {
                return Some(id);
            }
            current = node.parent;
        }
        None
    }

    /// Focusable widgets under `id` in depth-first order, skipping hidden and
    /// collapsed subtrees.
    fn collect_focusables(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
        let Some(node) = self.node(id) else {
            return;
        };
        if node.visibility != Visibility::Visible {
            return;
        }
        if node.focusable && node.enabled {
            out.push(id);
        }
        for child in &node.children {
            self.collect_focusables(*child, out);
        }
    }

    /// The tab order of the current scope: the focused widget's scope, or
    /// every root in order when nothing is focused.
    fn tab_order(&self) -> Vec<WidgetId> {
        let mut order = Vec::new();
        match self.focused.and_then(|focused| self.scope_of(focused)) {
            Some(scope) => self.collect_focusables(scope, &mut order),
            None => {
                for root in &self.roots {
                    self.collect_focusables(*root, &mut order);
                }
            }
        }
        order
    }

    /// Focuses `id` when [`Self::can_focus`]; returns whether it did. Moving
    /// into another scope records the focus left behind, which removing that
    /// scope restores. The widgets that gained and lost focus resolve their
    /// styles at the next layout, so a `focused` variant with text overrides
    /// relayouts then.
    pub fn focus(&mut self, id: WidgetId) -> bool {
        if !self.can_focus(id) {
            return false;
        }
        if self.focused == Some(id) {
            return true;
        }
        let previous = self.focused;
        let new_scope = self.scope_of(id);
        let old_scope = previous.and_then(|previous| self.scope_of(previous));
        if let Some(scope) = new_scope
            && new_scope != old_scope
        {
            self.focus_return.push((scope, previous));
        }
        self.set_focused(Some(id));
        true
    }

    /// Clears focus.
    pub fn blur(&mut self) {
        self.set_focused(None);
    }

    /// The widget holding keyboard focus.
    #[must_use]
    pub fn focused(&self) -> Option<WidgetId> {
        self.focused
    }

    /// Moves focus to the next focusable widget in tree order inside the
    /// current scope, wrapping at the end; from nothing, the first focusable
    /// of the first root. `None` when nothing can take focus.
    pub fn focus_next(&mut self) -> Option<WidgetId> {
        self.focus_step(1)
    }

    /// The mirror of [`Self::focus_next`].
    pub fn focus_prev(&mut self) -> Option<WidgetId> {
        self.focus_step(-1)
    }

    fn focus_step(&mut self, step: isize) -> Option<WidgetId> {
        let order = self.tab_order();
        if order.is_empty() {
            return None;
        }
        let len = order.len() as isize;
        let next = match self
            .focused
            .and_then(|focused| order.iter().position(|id| *id == focused))
        {
            Some(at) => order[((at as isize + step).rem_euclid(len)) as usize],
            None if step > 0 => order[0],
            None => order[order.len() - 1],
        };
        self.focus(next).then_some(next)
    }

    /// Writes the focused widget and marks both widgets for resolution.
    pub(crate) fn set_focused(&mut self, next: Option<WidgetId>) {
        if self.focused == next {
            return;
        }
        if let Some(previous) = self.focused {
            self.mark_style_dirty(previous);
        }
        self.focused = next;
        if let Some(next) = next {
            self.mark_style_dirty(next);
        }
        self.mark_paint_dirty();
    }

    /// Clears focus when it sits on `id` or under it; for a widget that was
    /// hidden, collapsed, disabled or made unfocusable.
    pub(crate) fn drop_focus_under(&mut self, id: WidgetId) {
        if let Some(focused) = self.focused
            && self.is_within(focused, id)
        {
            self.set_focused(None);
        }
    }

    /// After `removed` left the tree: forgets their scope entries, and when
    /// focus was lost with a scope, restores the focus recorded when that
    /// scope was entered, if it is still valid.
    pub(crate) fn focus_after_removal(&mut self, removed: &[WidgetId]) {
        let mut restore = None;
        let mut kept = Vec::with_capacity(self.focus_return.len());
        for (scope, previous) in self.focus_return.drain(..) {
            if removed.contains(&scope) {
                restore = previous;
            } else {
                kept.push((scope, previous));
            }
        }
        self.focus_return = kept;
        if self.focused.is_none()
            && let Some(previous) = restore
            && self.can_focus(previous)
        {
            self.set_focused(Some(previous));
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/focus.rs"]
mod tests;
