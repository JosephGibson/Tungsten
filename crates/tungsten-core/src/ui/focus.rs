//! Keyboard focus (`D-125`): one focused widget, tab order in depth-first
//! tree order inside the nearest focus scope, wrapping, directional focus
//! (`D-149`), and the rules that keep `focused()` from naming a widget the
//! traversal would skip. M3's router drives these from the `ui_*` actions.

use glam::Vec2;

use super::geometry::Rect;
use super::id::WidgetId;
use super::node::Visibility;
use super::tree::UiTree;

/// A direction for [`UiTree::focus_direction`], the `ui_up`, `ui_down`,
/// `ui_left` and `ui_right` actions' (`D-149`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavDirection {
    /// Towards the top of the viewport.
    Up,
    /// Towards the bottom.
    Down,
    /// Towards the left.
    Left,
    /// Towards the right.
    Right,
}

impl NavDirection {
    /// The slot in a node's explicit neighbours.
    const fn index(self) -> usize {
        match self {
            Self::Up => 0,
            Self::Down => 1,
            Self::Left => 2,
            Self::Right => 3,
        }
    }

    /// From `from` to `to`: the distance this way, negative behind, and the
    /// offset across it.
    fn axes(self, from: Vec2, to: Vec2) -> (f32, f32) {
        let d = to - from;
        match self {
            Self::Up => (-d.y, d.x.abs()),
            Self::Down => (d.y, d.x.abs()),
            Self::Left => (-d.x, d.y.abs()),
            Self::Right => (d.x, d.y.abs()),
        }
    }

    /// Where a wrapped search starts: on `scope`'s edge behind this
    /// direction, level with `from`.
    fn wrap_origin(self, scope: Rect, from: Vec2) -> Vec2 {
        match self {
            Self::Up => Vec2::new(from.x, scope.max.y),
            Self::Down => Vec2::new(from.x, scope.min.y),
            Self::Left => Vec2::new(scope.max.x, from.y),
            Self::Right => Vec2::new(scope.min.x, from.y),
        }
    }
}

fn centre(rect: Rect) -> Vec2 {
    (rect.min + rect.max) * 0.5
}

/// The candidate whose centre lies `dir` of `origin` with the lowest score,
/// the distance that way plus twice the offset across it (`D-149`); the
/// first in tab order on a tie. `inclusive` lets a centre level with
/// `origin` count.
fn nearest(
    dir: NavDirection,
    origin: Vec2,
    candidates: &[(WidgetId, Vec2)],
    inclusive: bool,
) -> Option<WidgetId> {
    let mut best: Option<(WidgetId, f32)> = None;
    for &(id, centre) in candidates {
        let (along, across) = dir.axes(origin, centre);
        if along < 0.0 || (along == 0.0 && !inclusive) {
            continue;
        }
        let score = along + 2.0 * across;
        if best.is_none_or(|(_, best)| score < best) {
            best = Some((id, score));
        }
    }
    best.map(|(id, _)| id)
}

impl UiTree {
    /// Whether `id` can hold focus now: focusable, enabled, visible, and
    /// under no disabled, hidden or collapsed ancestor.
    #[must_use]
    pub fn can_focus(&self, id: WidgetId) -> bool {
        let Some(node) = self.node(id) else {
            return false;
        };
        node.focusable && self.is_enabled_in_tree(id) && self.is_shown(id)
    }

    /// Whether `id` and every ancestor are enabled: a disabled widget
    /// disables what is inside it (`D-125`).
    pub(crate) fn is_enabled_in_tree(&self, id: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(id) = current {
            let Some(node) = self.node(id) else {
                return false;
            };
            if !node.enabled {
                return false;
            }
            current = node.parent;
        }
        true
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
    pub(crate) fn scope_of(&self, id: WidgetId) -> Option<WidgetId> {
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

    /// Focusable widgets under `id` in depth-first order, skipping disabled,
    /// hidden and collapsed subtrees. Callers start at a root or at the
    /// focused widget's scope, whose ancestors are enabled.
    fn collect_focusables(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
        let Some(node) = self.node(id) else {
            return;
        };
        if node.visibility != Visibility::Visible || !node.enabled {
            return;
        }
        if node.focusable {
            out.push(id);
        }
        for child in &node.children {
            self.collect_focusables(*child, out);
        }
    }

    /// The first widget under `id`, a root, in tab order, that can take
    /// focus; disabled, hidden and collapsed subtrees are skipped.
    pub(crate) fn first_focusable(&self, id: WidgetId) -> Option<WidgetId> {
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible || !node.enabled {
            return None;
        }
        if node.focusable {
            return Some(id);
        }
        node.children
            .iter()
            .find_map(|child| self.first_focusable(*child))
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
    /// into another scope from outside it records the focus left behind,
    /// replacing that scope's earlier record, and removing, hiding or
    /// collapsing the scope restores it; coming back from a scope nested
    /// inside records nothing. The widgets that gained and lost focus
    /// resolve their styles at the next layout, so a `focused` variant with
    /// text overrides relayouts then.
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
            && !previous.is_some_and(|previous| self.is_within(previous, scope))
        {
            self.focus_return.retain(|(entered, _)| *entered != scope);
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

    /// Moves focus `dir` inside the current scope (`D-149`): to the focused
    /// widget's explicit neighbour that way when it can take focus;
    /// otherwise to the nearest laid-out widget that can, in the focused
    /// widget's scope, whose centre lies that way, scored by the distance
    /// that way plus twice the offset across it, the first in tab order on a
    /// tie. When none lies that way and the scope wraps
    /// ([`Self::set_focus_wrap`]), the search runs again from the scope's
    /// opposite edge. Returns the widget focused; `None`, focus unchanged,
    /// when nothing is focused or nothing lies that way.
    pub fn focus_direction(&mut self, dir: NavDirection) -> Option<WidgetId> {
        let focused = self.focused?;
        if let Some(neighbor) = self.focus_neighbor(focused, dir)
            && self.can_focus(neighbor)
        {
            return self.focus(neighbor).then_some(neighbor);
        }
        let from = centre(self.rect(focused)?);
        let scope = self.scope_of(focused)?;
        let mut order = Vec::new();
        self.collect_focusables(scope, &mut order);
        let candidates: Vec<(WidgetId, Vec2)> = order
            .into_iter()
            .filter(|id| *id != focused)
            .filter_map(|id| Some((id, centre(self.rect(id)?))))
            .collect();
        let target = nearest(dir, from, &candidates, false).or_else(|| {
            if !self.focus_wrap(scope) {
                return None;
            }
            let origin = dir.wrap_origin(self.rect(scope)?, from);
            nearest(dir, origin, &candidates, true)
        })?;
        self.focus(target).then_some(target)
    }

    /// Sets `id`'s explicit neighbour `dir`, which
    /// [`Self::focus_direction`] takes before searching, or clears it with
    /// `None`. The neighbour may sit in another scope. Marks nothing.
    pub fn set_focus_neighbor(&mut self, id: WidgetId, dir: NavDirection, to: Option<WidgetId>) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        if node.focus_neighbors.is_none() && to.is_none() {
            return;
        }
        node.focus_neighbors.get_or_insert_default()[dir.index()] = to;
    }

    /// `id`'s explicit neighbour `dir`; `None` when none is set or for an
    /// invalid ID.
    #[must_use]
    pub fn focus_neighbor(&self, id: WidgetId, dir: NavDirection) -> Option<WidgetId> {
        self.node(id)?.focus_neighbors.as_ref()?[dir.index()]
    }

    /// Lets directional focus wrap inside `scope`: a search that finds
    /// nothing that way runs again from the scope's opposite edge. Read on
    /// the scope the focused widget's traversal stays in, a root or a widget
    /// flagged with [`Self::set_focus_scope`]. Off by default. Marks nothing.
    pub fn set_focus_wrap(&mut self, scope: WidgetId, on: bool) {
        if let Some(node) = self.node_mut(scope) {
            node.focus_wrap = on;
        }
    }

    /// Whether directional focus wraps inside `id`; `false` for an invalid
    /// ID.
    #[must_use]
    pub fn focus_wrap(&self, id: WidgetId) -> bool {
        self.node(id).is_some_and(|node| node.focus_wrap)
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

    /// For `id` hidden or collapsed: clears focus when it sits on `id` or
    /// under it, then leaves the scopes under `id` as removing them would.
    pub(crate) fn hide_focus_under(&mut self, id: WidgetId) {
        if let Some(focused) = self.focused
            && self.is_within(focused, id)
        {
            self.set_focused(None);
        }
        self.leave_scopes(|tree, scope| tree.is_within(scope, id));
    }

    /// After `removed` left the tree: leaves their scopes.
    pub(crate) fn focus_after_removal(&mut self, removed: &[WidgetId]) {
        self.leave_scopes(|_, scope| removed.contains(&scope));
    }

    /// Forgets the entries of the scopes `gone` names, and when focus was
    /// lost with them, restores the newest of their recorded focuses that
    /// can still take focus: a scope nested in a closed one recorded a
    /// widget inside it.
    fn leave_scopes(&mut self, gone: impl Fn(&Self, WidgetId) -> bool) {
        let entries = std::mem::take(&mut self.focus_return);
        let (left, kept): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|&(scope, _)| gone(self, scope));
        self.focus_return = kept;
        if self.focused.is_none()
            && let Some(previous) = left
                .iter()
                .rev()
                .filter_map(|&(_, previous)| previous)
                .find(|&previous| self.can_focus(previous))
        {
            self.set_focused(Some(previous));
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/focus.rs"]
mod tests;
