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
fn the_paint_clip_decides_whether_an_overflowing_child_is_hit() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let parent = tree.panel(root, at(0.0, 0.0, 100.0, 100.0));
    let child = tree.panel(parent, at(50.0, 50.0, 100.0, 100.0));
    layout(&mut tree);
    assert_eq!(tree.rect(child).unwrap().max, Vec2::new(150.0, 150.0));
    assert_eq!(
        tree.hit_test(Vec2::new(75.0, 75.0)),
        Some(child),
        "inside both"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(125.0, 125.0)),
        Some(child),
        "outside a parent that does not clip, where the child paints"
    );

    tree.set_clip_children(parent, true);
    assert_eq!(tree.hit_test(Vec2::new(75.0, 75.0)), Some(child));
    assert_eq!(
        tree.hit_test(Vec2::new(125.0, 125.0)),
        Some(root),
        "outside the clipping parent"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(100.0, 100.0)),
        Some(child),
        "the clip's edges included"
    );
    assert_eq!(tree.hit_test(Vec2::new(100.5, 75.0)), Some(root));

    // A clipping grandparent bounds through a parent that does not clip.
    let outer = tree.panel(root, at(200.0, 0.0, 100.0, 100.0));
    tree.set_clip_children(outer, true);
    let middle = tree.panel(outer, at(0.0, 0.0, 20.0, 20.0));
    let inner = tree.panel(middle, at(50.0, 50.0, 100.0, 100.0));
    layout(&mut tree);
    assert_eq!(
        tree.hit_test(Vec2::new(275.0, 75.0)),
        Some(inner),
        "outside its parent, inside the grandparent's clip"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(325.0, 75.0)),
        Some(root),
        "outside the grandparent's clip"
    );

    // The viewport clips the roots, and a clip with no area hides its
    // subtree from the pointer as from paint.
    let edge = tree.panel(root, at(350.0, 250.0, 100.0, 100.0));
    let empty = tree.panel(root, at(0.0, 200.0, 0.0, 0.0));
    tree.set_clip_children(empty, true);
    let unseen = tree.panel(empty, at(0.0, 0.0, 50.0, 50.0));
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(380.0, 280.0)), Some(edge));
    assert_eq!(
        tree.hit_test(Vec2::new(420.0, 320.0)),
        None,
        "outside the viewport"
    );
    assert_eq!(tree.rect(unseen).unwrap().width(), 50.0);
    assert_eq!(tree.hit_test(Vec2::new(25.0, 225.0)), Some(root));
    tree.set_clip_children(empty, false);
    assert_eq!(tree.hit_test(Vec2::new(25.0, 225.0)), Some(unseen));
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

#[test]
fn a_shown_modal_root_blocks_the_roots_below_it_and_not_those_above() {
    let mut tree = UiTree::new();
    let game = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let under = tree.panel(game, at(0.0, 0.0, 100.0, 40.0));
    let popup = tree.add_root(UiLayer::GamePopup, at(150.0, 100.0, 100.0, 100.0));
    let debug = tree.add_root(UiLayer::Debug, at(300.0, 0.0, 100.0, 100.0));
    tree.set_modal(popup, true);
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(200.0, 150.0)), Some(popup));
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 20.0)),
        None,
        "a widget below the modal"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(10.0, 290.0)),
        None,
        "the root below it"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(350.0, 50.0)),
        Some(debug),
        "a Debug root above it still hits"
    );

    tree.set_visibility(popup, Visibility::Hidden);
    layout(&mut tree);
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 20.0)),
        Some(under),
        "a hidden modal blocks nothing"
    );
    assert_eq!(tree.hit_test(Vec2::new(200.0, 150.0)), Some(game));

    tree.set_visibility(popup, Visibility::Visible);
    tree.set_modal(popup, false);
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(200.0, 150.0)), Some(popup));
    assert_eq!(tree.hit_test(Vec2::new(50.0, 20.0)), Some(under));
}

#[test]
fn a_root_takes_hits_once_it_is_visible_and_laid_out() {
    let mut tree = UiTree::new();
    let game = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    layout(&mut tree);
    let late = tree.add_root(UiLayer::Game, at(0.0, 0.0, 100.0, 100.0));
    tree.set_modal(late, true);
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 50.0)),
        Some(game),
        "an unlaid root is not hit"
    );
    assert_eq!(
        tree.hit_test(Vec2::new(200.0, 200.0)),
        Some(game),
        "an unlaid modal root blocks nothing"
    );
    layout(&mut tree);
    assert_eq!(tree.hit_test(Vec2::new(50.0, 50.0)), Some(late));
    assert_eq!(
        tree.hit_test(Vec2::new(200.0, 200.0)),
        None,
        "laid out, it blocks"
    );

    tree.set_visibility(late, Visibility::Collapsed);
    layout(&mut tree);
    assert_eq!(
        tree.hit_test(Vec2::new(50.0, 50.0)),
        Some(game),
        "collapsed"
    );
}

#[test]
fn hidden_game_layers_take_no_hits() {
    let mut tree = UiTree::new();
    let game = tree.add_root(UiLayer::Game, boxed(400.0, 300.0));
    let popup = tree.add_root(UiLayer::GamePopup, at(0.0, 0.0, 100.0, 100.0));
    let debug = tree.add_root(UiLayer::Debug, at(300.0, 0.0, 100.0, 100.0));
    tree.set_modal(popup, true);
    layout(&mut tree);
    assert_eq!(
        tree.hit_test_layers(Vec2::new(50.0, 50.0), false),
        Some(popup)
    );
    assert_eq!(tree.hit_test_layers(Vec2::new(50.0, 50.0), true), None);
    assert_eq!(tree.hit_test_layers(Vec2::new(200.0, 200.0), true), None);
    assert_eq!(
        tree.hit_test_layers(Vec2::new(350.0, 50.0), true),
        Some(debug)
    );
    assert!(!tree.root_routes(game, true));
    assert!(tree.root_routes(game, false));
}
