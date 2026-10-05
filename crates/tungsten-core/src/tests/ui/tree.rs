use glam::Vec2;

use super::*;
use crate::text::StyledText;
use crate::ui::{FixedAdvanceMeasure, LayoutStyle, Role, UiLayer, Visibility, WidgetKind};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

/// A tree with one `Game` root, laid out and painted, so every flag is clean.
fn clean_tree() -> (UiTree, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    settle(&mut tree);
    (tree, root)
}

fn settle(tree: &mut UiTree) {
    let mut text = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut text);
    tree.clear_paint_dirty();
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());
}

#[test]
fn a_removed_id_is_never_valid_again_and_its_slot_gets_a_new_generation() {
    let (mut tree, root) = clean_tree();
    let panel = tree.panel(root, LayoutStyle::default());
    assert!(tree.contains(panel));
    tree.remove(panel);
    assert!(!tree.contains(panel));
    assert_eq!(tree.kind(panel), None);
    assert_eq!(tree.parent(panel), None);
    assert!(tree.children(panel).is_empty());
    assert!(!tree.enabled(panel));

    // Setters on the dead ID do nothing and the tree stays as it was.
    settle(&mut tree);
    tree.set_visibility(panel, Visibility::Hidden);
    assert!(!tree.set_text(panel, "x"));
    tree.remove(panel);
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());

    let reused = tree.panel(root, LayoutStyle::default());
    assert_eq!(reused.index(), panel.index());
    assert_eq!(reused.generation(), panel.generation() + 1);
    assert_ne!(reused, panel);
    assert!(tree.contains(reused));
    assert!(!tree.contains(panel));
}

#[test]
fn removing_a_subtree_removes_its_descendants() {
    let (mut tree, root) = clean_tree();
    let keep = tree.panel(root, LayoutStyle::default());
    let gone = tree.panel(root, LayoutStyle::default());
    let child = tree.label(gone, "a");
    let button = tree.button(gone, "b");
    let button_label = tree.children(button)[0];
    tree.remove(gone);
    for id in [gone, child, button, button_label] {
        assert!(!tree.contains(id), "{id:?}");
    }
    assert!(tree.contains(keep));
    assert_eq!(tree.children(root), &[keep]);
    assert_eq!(
        tree.dump_layout(),
        "Panel None Visible (0.0, 0.0) 800.0×0.0\n  Panel None Visible -\n"
    );
}

#[test]
fn removing_a_root_drops_it_from_the_roots() {
    let mut tree = UiTree::new();
    let first = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let second = tree.add_root(UiLayer::Game, LayoutStyle::default());
    settle(&mut tree);
    tree.remove(first);
    assert_eq!(tree.roots(), &[second]);
    assert!(!tree.contains(first));
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());
}

#[test]
fn sibling_order_is_insertion_order() {
    let (mut tree, root) = clean_tree();
    let a = tree.label(root, "a");
    let b = tree.panel(root, LayoutStyle::default());
    let c = tree.button(root, "c");
    assert_eq!(tree.children(root), &[a, b, c]);
    tree.remove(b);
    assert_eq!(tree.children(root), &[a, c]);
    let d = tree.label(root, "d");
    assert_eq!(tree.children(root), &[a, c, d]);
    for id in [a, c, d] {
        assert_eq!(tree.parent(id), Some(root));
        assert_eq!(tree.root_of(id), Some(root));
        assert_eq!(tree.layer(id), Some(UiLayer::Game));
    }
}

#[test]
fn roots_list_by_layer_then_insertion() {
    let mut tree = UiTree::new();
    let debug = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    let game_a = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let popup = tree.add_root(UiLayer::GamePopup, LayoutStyle::default());
    let game_b = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let debug_popup = tree.add_root(UiLayer::DebugPopup, LayoutStyle::default());
    assert_eq!(tree.roots(), &[game_a, game_b, popup, debug, debug_popup]);
    assert_eq!(tree.layer(popup), Some(UiLayer::GamePopup));
    assert_eq!(tree.parent(popup), None);
    assert_eq!(tree.root_of(popup), Some(popup));
    tree.remove(game_a);
    let game_c = tree.add_root(UiLayer::Game, LayoutStyle::default());
    assert_eq!(tree.roots(), &[game_b, game_c, popup, debug, debug_popup]);
}

#[test]
fn equal_text_is_not_a_change() {
    let (mut tree, root) = clean_tree();
    let label = tree.label(root, "score");
    settle(&mut tree);
    assert!(!tree.set_text(label, "score"));
    assert!(!tree.set_text(label, StyledText::from("score")));
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());

    assert!(tree.set_text(label, "score: 1"));
    assert!(tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    assert_eq!(tree.text(label), Some(&StyledText::from("score: 1")));

    // A panel holds no text.
    settle(&mut tree);
    assert!(!tree.set_text(root, "x"));
    assert_eq!(tree.text(root), None);
    assert!(!tree.is_layout_dirty());
}

#[test]
fn visibility_marks_layout_and_enabled_marks_paint_only() {
    let (mut tree, root) = clean_tree();
    let button = tree.button(root, "ok");
    settle(&mut tree);

    tree.set_visibility(button, Visibility::Hidden);
    assert_eq!(tree.visibility(button), Some(Visibility::Hidden));
    assert!(tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    settle(&mut tree);
    tree.set_visibility(button, Visibility::Hidden);
    assert!(!tree.is_layout_dirty(), "an equal visibility marks nothing");

    tree.set_enabled(button, false);
    assert!(!tree.enabled(button));
    assert!(tree.is_paint_dirty());
    assert!(
        !tree.is_layout_dirty(),
        "enabled marks paint, not layout, until styles resolve"
    );
    tree.clear_paint_dirty();
    tree.set_enabled(button, false);
    assert!(!tree.is_paint_dirty(), "an equal enabled marks nothing");

    // Roles, names and the behaviour flags mark nothing.
    tree.set_role(button, Role::Dialog);
    tree.set_accessible_name(button, Some("Confirm".to_owned()));
    tree.set_interactive(button, false);
    tree.set_focusable(button, false);
    tree.set_focus_scope(button, true);
    tree.set_hit_testable(button, false);
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());
    assert_eq!(tree.role(button), Some(Role::Dialog));
    assert!(!tree.interactive(button));
    assert!(!tree.focusable(button));
    assert!(tree.focus_scope(button));
    assert!(!tree.hit_testable(button));

    // A layout style change marks layout; an equal one marks nothing.
    tree.set_layout_style(button, LayoutStyle::row());
    assert!(tree.is_layout_dirty());
    settle(&mut tree);
    tree.set_layout_style(button, LayoutStyle::row());
    assert!(!tree.is_layout_dirty());
    assert_eq!(tree.layout_style(button), Some(&LayoutStyle::row()));
}

#[test]
fn a_dirty_root_is_the_one_that_changed() {
    let mut tree = UiTree::new();
    let a = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let b = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    let label = tree.label(b, "fps");
    settle(&mut tree);
    assert!(tree.set_text(label, "fps 60"));
    assert!(tree.is_layout_dirty());
    assert_eq!(tree.root_of(label), Some(b));
    // Adding under `a` marks `a` too; removing from `b` marks `b`.
    tree.panel(a, LayoutStyle::default());
    settle(&mut tree);
    tree.remove(label);
    assert!(tree.is_layout_dirty());
}

#[test]
fn a_button_is_a_button_with_one_label_child() {
    let (mut tree, root) = clean_tree();
    let button = tree.button(root, "Start");
    assert_eq!(tree.kind(button), Some(WidgetKind::Button));
    assert_eq!(tree.role(button), Some(Role::Button));
    assert!(tree.focusable(button));
    assert!(tree.interactive(button));
    assert!(tree.enabled(button));
    assert!(tree.hit_testable(button));
    assert!(!tree.focus_scope(button));
    let children = tree.children(button);
    assert_eq!(children.len(), 1);
    let label = children[0];
    assert_eq!(tree.kind(label), Some(WidgetKind::Label));
    assert_eq!(tree.role(label), Some(Role::Label));
    assert!(!tree.focusable(label));
    assert!(!tree.interactive(label));
    assert_eq!(tree.text(label), Some(&StyledText::from("Start")));
    assert_eq!(tree.text(button), Some(&StyledText::from("Start")));

    // Setting the button's text sets its label's.
    settle(&mut tree);
    assert!(tree.set_text(button, "Resume"));
    assert_eq!(tree.text(label), Some(&StyledText::from("Resume")));
    assert!(tree.is_layout_dirty());
    assert!(!tree.set_text(button, "Resume"));

    // Panels and labels start without the button's behaviour.
    let panel = tree.panel(root, LayoutStyle::default());
    assert_eq!(tree.role(panel), Some(Role::None));
    assert!(!tree.focusable(panel));
    assert!(!tree.interactive(panel));
    assert!(tree.hit_testable(panel));
    assert_eq!(
        tree.dump_layout(),
        "Panel None Visible (0.0, 0.0) 800.0×20.0\n  Button Button Visible (0.0, 0.0) 800.0×20.0\n    Label Label Visible (0.0, 0.0) 800.0×20.0\n  Panel None Visible -\n"
    );
}

#[test]
fn the_accessible_name_is_the_text_until_one_is_set() {
    let (mut tree, root) = clean_tree();
    let button = tree.button(root, "Start");
    let label = tree.label(root, "Lives: 3");
    let panel = tree.panel(root, LayoutStyle::default());
    assert_eq!(tree.accessible_name(button), Some("Start"));
    assert_eq!(tree.accessible_name(label), Some("Lives: 3"));
    assert_eq!(tree.accessible_name(panel), None);

    tree.set_accessible_name(button, Some("Start the game".to_owned()));
    assert_eq!(tree.accessible_name(button), Some("Start the game"));
    tree.set_text(button, "Go");
    assert_eq!(tree.accessible_name(button), Some("Start the game"));
    tree.set_accessible_name(button, None);
    assert_eq!(tree.accessible_name(button), Some("Go"));

    tree.set_accessible_name(panel, Some("Status".to_owned()));
    assert_eq!(tree.accessible_name(panel), Some("Status"));

    // Spans flatten.
    let styled = StyledText {
        spans: vec![
            crate::text::TextSpan::new("Lives: "),
            crate::text::TextSpan::new("3").with_weight(700),
        ],
    };
    tree.set_text(label, styled);
    assert_eq!(tree.accessible_name(label), Some("Lives: 3"));
}

#[test]
fn a_new_tree_is_empty_and_clean_until_a_root_arrives() {
    let mut tree = UiTree::new();
    assert!(tree.roots().is_empty());
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());
    assert_eq!(tree.dump_layout(), "");
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    assert!(tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    assert_eq!(tree.kind(root), Some(WidgetKind::Panel));
    assert_eq!(tree.visibility(root), Some(Visibility::Visible));
    let mut text = FixedAdvanceMeasure::default();
    assert_eq!(
        tree.layout(VIEWPORT, &mut text),
        LayoutStats {
            roots_laid_out: 1,
            nodes_laid_out: 1,
            measure_calls: 0,
            commits: 0,
        }
    );
    assert!(!tree.is_layout_dirty());
}

#[test]
#[should_panic(expected = "not in the tree")]
fn a_child_of_a_dead_parent_panics() {
    let (mut tree, root) = clean_tree();
    let panel = tree.panel(root, LayoutStyle::default());
    tree.remove(panel);
    tree.panel(panel, LayoutStyle::default());
}
