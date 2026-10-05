use glam::Vec2;

use super::*;
use crate::ui::theme::{StyleOverrides, TextOverrides, Theme};
use crate::ui::{FixedAdvanceMeasure, LayoutStyle, UiLayer, Visibility};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);

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
