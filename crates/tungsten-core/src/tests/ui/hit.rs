use glam::Vec2;

use super::*;
use crate::ui::{Anchor, FixedAdvanceMeasure, LayoutStyle, Sizing, UiLayer, Visibility};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);

fn boxed(w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default().with_size(Sizing::Px(w), Sizing::Px(h))
}

fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    boxed(w, h).anchored(Anchor::TopStart, Vec2::new(x, y))
}

fn layout(tree: &mut UiTree) {
    let mut store = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut store);
}

#[test]
fn nothing_is_hit_before_the_first_layout_or_outside_every_root() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, at(10.0, 10.0, 100.0, 100.0));
    assert_eq!(tree.hit_test(Vec2::new(50.0, 50.0)), None);
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(50.0, 50.0)), Some(root));
    assert_eq!(tree.hit_test(Vec2::new(5.0, 5.0)), None);
    assert_eq!(tree.hit_test(Vec2::new(111.0, 50.0)), None);
    assert_eq!(
        tree.hit_test(Vec2::new(110.0, 110.0)),
        Some(root),
        "edges included"
    );
}

#[test]
fn the_later_sibling_wins_where_two_overlap() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let under = tree.panel(root, at(0.0, 0.0, 200.0, 200.0));
    let over = tree.panel(root, at(100.0, 100.0, 200.0, 200.0));
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(150.0, 150.0)), Some(over));
    assert_eq!(tree.hit_test(Vec2::new(50.0, 50.0)), Some(under));
    assert_eq!(tree.hit_test(Vec2::new(250.0, 250.0)), Some(over));
    assert_eq!(tree.hit_test(Vec2::new(350.0, 50.0)), Some(root));
}

#[test]
fn a_child_outside_its_parents_box_is_not_hit() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let parent = tree.panel(root, at(0.0, 0.0, 100.0, 100.0));
    let child = tree.panel(parent, at(50.0, 50.0, 100.0, 100.0));
    layout(&mut tree);
    assert_eq!(
        tree.hit_test(Vec2::new(75.0, 75.0)),
        Some(child),
        "inside both"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(125.0, 125.0)),
        Some(root),
        "inside the child, outside the parent"
    );
    assert_eq!(tree.rect(child).unwrap().max, Vec2::new(150.0, 150.0));
}

#[test]
fn hidden_collapsed_and_non_hit_testable_nodes_let_what_is_behind_through() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let behind = tree.panel(root, at(0.0, 0.0, 300.0, 300.0));
    let hidden = tree.panel(root, at(0.0, 0.0, 100.0, 100.0));
    let hidden_child = tree.button(hidden, "x");
    let collapsed = tree.panel(root, at(100.0, 0.0, 100.0, 100.0));
    let transparent = tree.label(root, "hud");
    tree.set_layout_style(transparent, at(200.0, 0.0, 100.0, 100.0));
    tree.set_hit_testable(transparent, false);
    layout(&mut tree);
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 10.0)),
        Some(tree.children(hidden_child)[0]),
        "the button's label, the deepest hit"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 50.0)),
        Some(hidden),
        "below the button"
    );
    assert_eq!(tree.hit_test(Vec2::new(150.0, 50.0)), Some(collapsed));
    assert_eq!(
        tree.hit_test(Vec2::new(250.0, 50.0)),
        Some(behind),
        "the label is transparent"
    );

    tree.set_visibility(hidden, Visibility::Hidden);
    tree.set_visibility(collapsed, Visibility::Collapsed);
    layout(&mut tree);
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 10.0)),
        Some(behind),
        "hidden, children included"
    );
    assert_eq!(tree.hit_test(Vec2::new(50.0, 50.0)), Some(behind));
    assert_eq!(
        tree.hit_test(Vec2::new(150.0, 50.0)),
        Some(behind),
        "collapsed"
    );

    // A non-hit-testable panel still routes to its hit-testable children.
    let frame = tree.panel(root, at(300.0, 0.0, 100.0, 100.0));
    tree.set_hit_testable(frame, false);
    let inner = tree.panel(frame, at(10.0, 10.0, 20.0, 20.0));
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(320.0, 20.0)), Some(inner));
    assert_eq!(
        tree.hit_test(Vec2::new(390.0, 90.0)),
        Some(root),
        "the frame itself is transparent"
    );
}

#[test]
fn a_disabled_button_is_hit_and_blocks() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let behind = tree.panel(root, at(0.0, 0.0, 200.0, 200.0));
    let button = tree.button(root, "ok");
    tree.set_layout_style(button, at(50.0, 50.0, 100.0, 40.0));
    tree.set_enabled(button, false);
    layout(&mut tree);
    let label = tree.children(button)[0];
    assert_eq!(
        tree.hit_test(Vec2::new(60.0, 60.0)),
        Some(label),
        "the label is the deepest hit"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(140.0, 85.0)),
        Some(button),
        "past the label's text box"
    );
    assert_eq!(tree.hit_test(Vec2::new(10.0, 10.0)), Some(behind));
    assert!(!tree.enabled(button));
}

#[test]
fn a_debug_root_beats_a_game_root_and_a_later_root_beats_an_earlier_one() {
    let mut tree = UiTree::new();
    let debug = tree.add_root(UiLayer::Debug, at(0.0, 0.0, 100.0, 100.0));
    let game_a = tree.add_root(UiLayer::Game, at(0.0, 0.0, 200.0, 200.0));
    let game_b = tree.add_root(UiLayer::Game, at(50.0, 50.0, 200.0, 200.0));
    let popup = tree.add_root(UiLayer::GamePopup, at(150.0, 150.0, 50.0, 50.0));
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(10.0, 10.0)), Some(debug));
    assert_eq!(
        tree.hit_test(Vec2::new(75.0, 75.0)),
        Some(debug),
        "the debug layer is above both game roots"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(120.0, 120.0)),
        Some(game_b),
        "the later game root is in front"
    );
    assert_eq!(tree.hit_test(Vec2::new(10.0, 150.0)), Some(game_a));
    assert_eq!(tree.hit_test(Vec2::new(175.0, 175.0)), Some(popup));
    assert_eq!(tree.hit_test(Vec2::new(300.0, 300.0)), None);
}
