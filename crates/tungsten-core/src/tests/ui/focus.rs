use glam::Vec2;

use super::*;
use crate::ui::theme::{StyleOverrides, TextOverrides, Theme};
use crate::ui::{Anchor, FixedAdvanceMeasure, LayoutStyle, Sizing, UiLayer, Visibility};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);

fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default()
        .with_size(Sizing::Px(w), Sizing::Px(h))
        .anchored(Anchor::TopStart, Vec2::new(x, y))
}

fn settle(tree: &mut UiTree) -> FixedAdvanceMeasure {
    let mut store = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut store);
    tree.clear_paint_dirty();
    store
}

/// A root with three buttons, the middle one inside a panel.
fn menu() -> (UiTree, WidgetId, [WidgetId; 3], WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let a = tree.button(root, "a");
    let panel = tree.panel(root, LayoutStyle::default());
    let b = tree.button(panel, "b");
    let c = tree.button(root, "c");
    settle(&mut tree);
    (tree, root, [a, b, c], panel)
}

#[test]
fn only_a_focusable_enabled_shown_widget_takes_focus() {
    let (mut tree, root, [a, _b, _c], panel) = menu();
    assert!(!tree.focus(root), "a panel is not focusable");
    assert!(!tree.focus(panel));
    assert_eq!(tree.focused(), None);
    assert!(tree.focus(a));
    assert_eq!(tree.focused(), Some(a));
    assert!(
        tree.focus(a),
        "focusing the focused widget is true and marks nothing"
    );
    assert!(tree.is_paint_dirty());

    tree.set_focusable(panel, true);
    assert!(tree.focus(panel), "set_focusable lets a panel take focus");
    assert!(tree.can_focus(panel));
    tree.set_focusable(panel, false);
    assert_eq!(
        tree.focused(),
        None,
        "a widget made unfocusable loses focus"
    );

    tree.blur();
    assert_eq!(tree.focused(), None);
}

#[test]
fn focus_next_walks_tree_order_and_wraps_and_prev_mirrors_it() {
    let (mut tree, _root, [a, b, c], _panel) = menu();
    assert_eq!(
        tree.focus_next(),
        Some(a),
        "from nothing: the first focusable"
    );
    assert_eq!(
        tree.focus_next(),
        Some(b),
        "depth first: the button inside the panel"
    );
    assert_eq!(tree.focus_next(), Some(c));
    assert_eq!(tree.focus_next(), Some(a), "wraps");
    assert_eq!(tree.focus_prev(), Some(c), "wraps backwards");
    assert_eq!(tree.focus_prev(), Some(b));
    assert_eq!(tree.focus_prev(), Some(a));
    assert_eq!(tree.focused(), Some(a));
    tree.blur();
    assert_eq!(
        tree.focus_prev(),
        Some(c),
        "from nothing backwards: the last"
    );

    let mut empty = UiTree::new();
    let root = empty.add_root(UiLayer::Game, LayoutStyle::default());
    empty.panel(root, LayoutStyle::default());
    assert_eq!(empty.focus_next(), None);
    assert_eq!(empty.focus_prev(), None);
}

#[test]
fn skipped_widgets_are_disabled_hidden_collapsed_or_under_a_hidden_panel() {
    let (mut tree, root, [a, b, c], panel) = menu();
    let fourth = tree.button(root, "d");
    let fifth = tree.button(root, "e");
    settle(&mut tree);
    tree.set_enabled(a, false);
    tree.set_visibility(panel, Visibility::Hidden);
    tree.set_visibility(c, Visibility::Hidden);
    tree.set_visibility(fourth, Visibility::Collapsed);
    settle(&mut tree);
    assert!(!tree.can_focus(a));
    assert!(!tree.can_focus(b), "under a hidden panel");
    assert!(!tree.can_focus(c));
    assert!(!tree.can_focus(fourth));
    assert!(!tree.focus(b));
    assert_eq!(tree.focus_next(), Some(fifth));
    assert_eq!(
        tree.focus_next(),
        Some(fifth),
        "the only one left wraps onto itself"
    );
    assert_eq!(tree.focus_prev(), Some(fifth));

    tree.set_visibility(panel, Visibility::Visible);
    tree.set_enabled(a, true);
    assert_eq!(tree.focus_next(), Some(a));
    assert_eq!(tree.focus_next(), Some(b));
    assert_eq!(
        tree.focus_next(),
        Some(fifth),
        "c hidden and fourth collapsed are still skipped"
    );
}

#[test]
fn a_scope_keeps_traversal_inside_it_and_its_removal_restores_focus() {
    let (mut tree, _root, [a, b, _c], _panel) = menu();
    assert!(tree.focus(b));
    let dialog = tree.add_root(UiLayer::GamePopup, LayoutStyle::default());
    tree.set_focus_scope(dialog, true);
    let ok = tree.button(dialog, "ok");
    let cancel = tree.button(dialog, "cancel");
    settle(&mut tree);

    assert!(tree.focus(ok));
    assert_eq!(tree.focus_next(), Some(cancel));
    assert_eq!(tree.focus_next(), Some(ok), "wraps inside the dialog");
    assert_eq!(tree.focus_prev(), Some(cancel));

    tree.remove(dialog);
    assert_eq!(tree.focused(), Some(b), "the focus left behind comes back");
    assert_eq!(
        tree.focus_next(),
        Some(a).map(|_| tree.children(tree.roots()[0])[2]),
        "back in the menu"
    );

    // The earlier focus is restored only while it is still valid.
    assert!(tree.focus(b));
    let dialog = tree.add_root(UiLayer::GamePopup, LayoutStyle::default());
    tree.set_focus_scope(dialog, true);
    let ok = tree.button(dialog, "ok");
    settle(&mut tree);
    assert!(tree.focus(ok));
    tree.set_enabled(b, false);
    tree.remove(dialog);
    assert_eq!(tree.focused(), None);

    // A scope that is not a root works the same.
    let (mut tree, root, [a, _b, _c], _panel) = menu();
    assert!(tree.focus(a));
    let group = tree.panel(root, LayoutStyle::default());
    tree.set_focus_scope(group, true);
    let x = tree.button(group, "x");
    let y = tree.button(group, "y");
    settle(&mut tree);
    assert!(tree.focus(y));
    assert_eq!(tree.focus_next(), Some(x));
    assert_eq!(tree.focus_next(), Some(y));
    tree.remove(group);
    assert_eq!(tree.focused(), Some(a));
}

#[test]
fn focus_clears_when_its_widget_or_an_ancestor_leaves_the_traversal() {
    let (mut tree, _root, [a, b, _c], panel) = menu();
    assert!(tree.focus(a));
    tree.remove(a);
    assert_eq!(tree.focused(), None, "removed");

    assert!(tree.focus(b));
    tree.set_visibility(b, Visibility::Hidden);
    assert_eq!(tree.focused(), None, "hidden");
    tree.set_visibility(b, Visibility::Visible);

    assert!(tree.focus(b));
    tree.set_visibility(b, Visibility::Collapsed);
    assert_eq!(tree.focused(), None, "collapsed");
    tree.set_visibility(b, Visibility::Visible);

    assert!(tree.focus(b));
    tree.set_enabled(b, false);
    assert_eq!(tree.focused(), None, "disabled");
    tree.set_enabled(b, true);

    assert!(tree.focus(b));
    tree.set_visibility(panel, Visibility::Hidden);
    assert_eq!(tree.focused(), None, "a hidden ancestor");
    tree.set_visibility(panel, Visibility::Visible);

    assert!(tree.focus(b));
    tree.set_visibility(panel, Visibility::Collapsed);
    assert_eq!(tree.focused(), None, "a collapsed ancestor");
    tree.set_visibility(panel, Visibility::Visible);

    assert!(tree.focus(b));
    tree.remove(panel);
    assert_eq!(tree.focused(), None, "a removed ancestor");
    assert!(!tree.contains(b));
}

#[test]
fn a_focus_change_relayouts_only_when_the_focused_variant_changes_text() {
    let themed = |focused: StyleOverrides| {
        let mut tree = UiTree::new();
        let mut theme = Theme::default();
        theme.kinds.button.focused = focused;
        tree.set_theme(theme);
        let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
        let button = tree.button(root, "Start");
        let store = settle(&mut tree);
        (tree, button, store)
    };

    let (mut tree, button, mut store) = themed(StyleOverrides {
        text: TextOverrides {
            size: Some(24.0),
            line_height: Some(30.0),
            ..TextOverrides::default()
        },
        ..StyleOverrides::default()
    });
    assert!(tree.focus(button));
    assert!(!tree.is_layout_dirty(), "layout dirt waits for resolution");
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 1);
    assert_eq!(tree.rect(tree.children(button)[0]).unwrap().height(), 30.0);
    tree.blur();
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 1, "losing focus relayouts too");
    assert_eq!(tree.rect(tree.children(button)[0]).unwrap().height(), 20.0);

    let (mut tree, button, mut store) = themed(StyleOverrides {
        background: Some([9, 9, 9, 255]),
        ..StyleOverrides::default()
    });
    assert!(tree.focus(button));
    assert!(tree.is_paint_dirty());
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 0);
    assert_eq!(
        tree.resolved_style(button).unwrap().background,
        [9, 9, 9, 255]
    );
}

/// A full-viewport root holding a grid, 50 by 30 each: `t1 t2 t3` with
/// centres on y 15, `b1 b2 b3` on y 115, at x 25, 125 and 225, and `mid`
/// centred at (175, 75), nearer `b2` along the axis than `t2` but off to
/// the side.
fn grid() -> (UiTree, WidgetId, [WidgetId; 7]) {
    let mut tree = UiTree::new();
    let root = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default().with_size(Sizing::Px(400.0), Sizing::Px(300.0)),
    );
    let mut place = |x: f32, y: f32| {
        let button = tree.button(root, "x");
        tree.set_layout_style(button, at(x, y, 50.0, 30.0));
        button
    };
    let ids = [
        place(0.0, 0.0),
        place(100.0, 0.0),
        place(200.0, 0.0),
        place(0.0, 100.0),
        place(100.0, 100.0),
        place(200.0, 100.0),
        place(150.0, 60.0),
    ];
    settle(&mut tree);
    (tree, root, ids)
}

#[test]
fn focus_direction_takes_the_explicit_neighbour_then_the_scored_nearest() {
    let (mut tree, _root, [t1, t2, t3, b1, b2, b3, mid]) = grid();
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        None,
        "nothing focused"
    );
    assert!(tree.focus(t1));
    assert_eq!(tree.focus_direction(NavDirection::Right), Some(t2));
    assert_eq!(tree.focus_direction(NavDirection::Down), Some(b2));
    assert_eq!(
        tree.focus_direction(NavDirection::Up),
        Some(t2),
        "t2 level with b2 beats mid, nearer along the axis but off to the side"
    );
    assert_eq!(tree.focus_direction(NavDirection::Right), Some(t3));
    assert_eq!(tree.focus_direction(NavDirection::Down), Some(b3));
    assert_eq!(tree.focus_direction(NavDirection::Left), Some(b2));
    assert_eq!(tree.focus_direction(NavDirection::Left), Some(b1));
    assert_eq!(
        tree.focus_direction(NavDirection::Left),
        None,
        "nothing lies that way"
    );
    assert_eq!(tree.focused(), Some(b1), "and focus stays");

    tree.set_focus_neighbor(b1, NavDirection::Right, Some(mid));
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        Some(mid),
        "the explicit neighbour first"
    );
    assert!(tree.focus(b1));
    tree.set_focus_neighbor(b1, NavDirection::Right, None);
    assert_eq!(tree.focus_direction(NavDirection::Right), Some(b2));

    assert!(tree.focus(b1));
    tree.set_focus_neighbor(b1, NavDirection::Right, Some(b3));
    tree.set_enabled(b3, false);
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        Some(b2),
        "a neighbour that cannot take focus falls back to the search"
    );
}

#[test]
fn focus_direction_skips_disabled_and_hidden_widgets_and_stays_in_its_scope() {
    let (mut tree, root, [t1, t2, t3, _b1, b2, _b3, mid]) = grid();
    tree.set_visibility(t2, Visibility::Hidden);
    assert!(tree.focus(t1));
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        Some(t3),
        "past the hidden t2"
    );

    tree.set_visibility(t2, Visibility::Visible);
    tree.set_enabled(t2, false);
    tree.set_enabled(mid, false);
    assert!(tree.focus(b2));
    assert_eq!(
        tree.focus_direction(NavDirection::Up),
        Some(t1),
        "t1 and t3 tie with t2 and mid disabled: the first in tab order"
    );

    // Another root level with t3 is not a candidate.
    let side = tree.add_root(UiLayer::Game, at(300.0, 0.0, 100.0, 30.0));
    let beyond = tree.button(side, "beyond");
    // A scope inside the root keeps its own widgets to themselves.
    let group = tree.panel(root, at(0.0, 200.0, 400.0, 100.0));
    tree.set_focus_scope(group, true);
    let left = tree.button(group, "l");
    tree.set_layout_style(left, at(0.0, 0.0, 50.0, 30.0));
    let right = tree.button(group, "r");
    tree.set_layout_style(right, at(200.0, 0.0, 50.0, 30.0));
    settle(&mut tree);
    assert!(tree.focus(t3));
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        None,
        "another root's widget"
    );
    assert!(tree.focus(left));
    assert_eq!(tree.focus_direction(NavDirection::Right), Some(right));
    assert_eq!(
        tree.focus_direction(NavDirection::Up),
        None,
        "the root's widgets lie outside the group's scope"
    );
    assert!(tree.can_focus(beyond));
}

#[test]
fn focus_direction_wraps_only_inside_a_scope_flagged_to() {
    let (mut tree, root, [t1, t2, t3, b1, _b2, _b3, mid]) = grid();
    assert!(tree.focus(t3));
    assert_eq!(tree.focus_direction(NavDirection::Right), None);
    tree.set_focus_wrap(mid, true);
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        None,
        "the flag is read on the scope, here the root"
    );

    tree.set_focus_wrap(root, true);
    assert!(tree.focus_wrap(root));
    assert_eq!(
        tree.focus_direction(NavDirection::Right),
        Some(t1),
        "from the scope's left edge, level with t3"
    );
    assert_eq!(tree.focus_direction(NavDirection::Left), Some(t3));
    assert!(tree.focus(t1));
    assert_eq!(
        tree.focus_direction(NavDirection::Up),
        Some(b1),
        "from the scope's bottom edge"
    );
    assert_eq!(tree.focus_direction(NavDirection::Down), Some(t1));
    assert_eq!(tree.focus_direction(NavDirection::Right), Some(t2));

    tree.set_focus_wrap(root, false);
    assert!(tree.focus(t3));
    assert_eq!(tree.focus_direction(NavDirection::Right), None);
}

#[test]
fn hiding_or_collapsing_a_scope_restores_the_focus_it_was_entered_from() {
    let (mut tree, _root, [a, b, _c], _panel) = menu();
    assert!(tree.focus(b));
    let dialog = tree.add_root(UiLayer::GamePopup, LayoutStyle::default());
    let ok = tree.button(dialog, "ok");
    settle(&mut tree);
    assert!(tree.focus(ok));
    tree.set_visibility(dialog, Visibility::Hidden);
    assert_eq!(tree.focused(), Some(b), "the focus left behind comes back");

    // Entered again, the dialog records the focus it was entered from then.
    tree.set_visibility(dialog, Visibility::Visible);
    assert!(tree.focus(a));
    assert!(tree.focus(ok));
    tree.set_visibility(dialog, Visibility::Collapsed);
    assert_eq!(tree.focused(), Some(a));

    // Moving back and forth keeps one record per scope.
    tree.set_visibility(dialog, Visibility::Visible);
    for _ in 0..3 {
        assert!(tree.focus(ok));
        assert!(tree.focus(a));
    }
    assert_eq!(tree.focus_return.len(), 2);

    // Hiding an ancestor of a scope leaves it too; a recorded widget that
    // cannot take focus any more restores nothing.
    let (mut tree, root, [a, _b, _c], _panel) = menu();
    let holder = tree.panel(root, LayoutStyle::default());
    let group = tree.panel(holder, LayoutStyle::default());
    tree.set_focus_scope(group, true);
    let x = tree.button(group, "x");
    settle(&mut tree);
    assert!(tree.focus(a));
    assert!(tree.focus(x));
    tree.set_visibility(holder, Visibility::Hidden);
    assert_eq!(tree.focused(), Some(a));

    tree.set_visibility(holder, Visibility::Visible);
    assert!(tree.focus(x));
    tree.set_enabled(a, false);
    tree.set_visibility(holder, Visibility::Hidden);
    assert_eq!(tree.focused(), None);
}

#[test]
fn disabling_a_container_disables_what_is_inside_it() {
    // `D-125`: traversal skips disabled nodes and their descendants, and
    // disabling the focused node or an ancestor clears it.
    let (mut tree, _root, [a, b, c], panel) = menu();
    assert!(tree.focus(b));
    tree.set_hovered(Some(b));
    tree.set_pressed(Some(b));
    tree.set_enabled(panel, false);
    assert_eq!(tree.focused(), None, "disabling an ancestor clears focus");
    assert_eq!(tree.hovered(), None);
    assert_eq!(tree.pressed(), None);
    assert!(tree.enabled(b), "the widget's own flag is unchanged");
    assert!(!tree.can_focus(b));
    assert!(!tree.focus(b));
    assert_eq!(tree.focus_next(), Some(a));
    assert_eq!(tree.focus_next(), Some(c), "tab skips the disabled panel");
    assert_eq!(tree.focus_next(), Some(a));
    assert_eq!(tree.focus_prev(), Some(c));

    tree.set_enabled(panel, true);
    assert!(tree.can_focus(b));
    assert_eq!(tree.focus_prev(), Some(b));
}

#[test]
fn closing_a_popup_with_a_scope_inside_restores_the_focus_from_before_it() {
    // `D-149`: hiding a scope restores the focus recorded on entering it.
    // With a scope nested in a popup, the inner scope's record or one
    // pointing inside the popup won, and focus was lost.
    let (mut tree, _root, [a, _b, _c], _panel) = menu();
    let popup = tree.add_root(UiLayer::GamePopup, LayoutStyle::default());
    let ok = tree.button(popup, "ok");
    let group = tree.panel(popup, LayoutStyle::default());
    tree.set_focus_scope(group, true);
    let x = tree.button(group, "x");
    settle(&mut tree);
    assert!(tree.focus(a));
    assert!(tree.focus(ok));
    assert!(tree.focus(x));
    tree.set_visibility(popup, Visibility::Hidden);
    assert_eq!(tree.focused(), Some(a), "the newest record that can focus");

    // Coming back out of the inner scope records nothing for the popup.
    tree.set_visibility(popup, Visibility::Visible);
    assert!(tree.focus(ok));
    assert!(tree.focus(x));
    assert!(tree.focus(ok));
    tree.set_visibility(popup, Visibility::Hidden);
    assert_eq!(tree.focused(), Some(a));

    tree.set_visibility(popup, Visibility::Visible);
    assert!(tree.focus(x));
    tree.remove(popup);
    assert_eq!(tree.focused(), Some(a));
}
