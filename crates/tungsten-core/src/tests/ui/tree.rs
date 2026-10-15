use glam::Vec2;

use super::*;
use crate::assets::SpriteAssetId;
use crate::text::StyledText;
use crate::ui::{
    CursorIcon, FixedAdvanceMeasure, LayoutStyle, NavDirection, Role, Theme, UiLayer, Visibility,
    WidgetKind,
};

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

#[test]
fn an_image_is_a_leaf_with_a_sprite_a_natural_size_and_a_tint() {
    let (mut tree, root) = clean_tree();
    let icon = SpriteAssetId::new(3);
    let image = tree.image(root, icon, Vec2::new(32.0, 16.0), LayoutStyle::default());
    assert_eq!(tree.kind(image), Some(WidgetKind::Image));
    assert_eq!(tree.role(image), Some(Role::Image));
    assert_eq!(tree.image_sprite(image), Some(icon));
    assert!(!tree.focusable(image));
    assert!(!tree.interactive(image));
    assert!(tree.hit_testable(image));
    assert!(!tree.clips_children(image));
    assert_eq!(tree.text(image), None);
    assert!(!tree.set_text(image, "x"));
    assert!(tree.is_layout_dirty());
    settle(&mut tree);

    // The same sprite and size mark nothing; a new size marks layout; a new
    // sprite alone marks paint.
    tree.set_image(image, icon, Vec2::new(32.0, 16.0));
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());
    tree.set_image(image, icon, Vec2::new(64.0, 16.0));
    assert!(tree.is_layout_dirty());
    settle(&mut tree);
    let other = SpriteAssetId::new(4);
    tree.set_image(image, other, Vec2::new(64.0, 16.0));
    assert_eq!(tree.image_sprite(image), Some(other));
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());
    tree.clear_paint_dirty();

    // A tint and the clip flag mark paint only; equal values mark nothing.
    tree.set_image_tint(image, [255, 255, 255, 255]);
    assert!(!tree.is_paint_dirty());
    tree.set_image_tint(image, [255, 0, 0, 255]);
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());
    tree.clear_paint_dirty();
    tree.set_clip_children(root, true);
    assert!(tree.clips_children(root));
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());
    tree.clear_paint_dirty();
    tree.set_clip_children(root, true);
    assert!(!tree.is_paint_dirty());

    // Panels and labels hold no image.
    assert_eq!(tree.image_sprite(root), None);
    tree.set_image(root, icon, Vec2::ZERO);
    tree.set_image_tint(root, [0, 0, 0, 0]);
    assert!(!tree.is_paint_dirty());
    assert!(!tree.clips_children(WidgetId::new(99, 0)));
    assert_eq!(
        tree.dump_layout(),
        "Panel None Visible (0.0, 0.0) 800.0×16.0\n  Image Image Visible (0.0, 0.0) 800.0×16.0\n"
    );
}

#[test]
#[should_panic(expected = "Image holds no children")]
fn a_child_of_an_image_panics() {
    let (mut tree, root) = clean_tree();
    let image = tree.image(
        root,
        SpriteAssetId::new(0),
        Vec2::ONE,
        LayoutStyle::default(),
    );
    tree.label(image, "x");
}

#[test]
fn the_theme_asset_id_follows_set_theme_asset_and_clears_on_set_theme() {
    let (mut tree, _root) = clean_tree();
    assert_eq!(tree.theme_id(), None);
    let epoch = tree.theme_epoch();
    let mut dark = Theme::default();
    dark.tokens.colors.text = [1, 2, 3, 255];
    tree.set_theme_asset("dark", dark.clone());
    assert_eq!(tree.theme_id(), Some("dark"));
    assert_eq!(tree.theme(), &dark);
    assert_eq!(tree.theme_epoch(), epoch.next());
    assert!(tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    settle(&mut tree);
    assert_eq!(tree.theme_id(), Some("dark"), "a layout keeps it");
    tree.set_theme(Theme::default());
    assert_eq!(tree.theme_id(), None);
    assert_eq!(tree.theme_epoch(), epoch.next().next());
}

#[test]
fn hover_and_press_clear_on_removal_hiding_and_disabling() {
    let (mut tree, root) = clean_tree();
    assert!(tree.focus_visible(), "focus shows on a new tree");
    assert_eq!((tree.hovered(), tree.pressed()), (None, None));
    let panel = tree.panel(root, LayoutStyle::default());
    let button = tree.button(panel, "ok");
    let other = tree.button(root, "other");
    settle(&mut tree);

    tree.set_hovered(Some(button));
    tree.set_pressed(Some(button));
    tree.set_visibility(panel, Visibility::Hidden);
    assert_eq!(
        (tree.hovered(), tree.pressed()),
        (None, None),
        "under a hidden panel"
    );
    tree.set_visibility(panel, Visibility::Visible);

    tree.set_hovered(Some(button));
    tree.set_pressed(Some(button));
    tree.set_visibility(button, Visibility::Collapsed);
    assert_eq!((tree.hovered(), tree.pressed()), (None, None), "collapsed");
    tree.set_visibility(button, Visibility::Visible);

    tree.set_hovered(Some(button));
    tree.set_pressed(Some(other));
    tree.set_visibility(panel, Visibility::Hidden);
    assert_eq!(tree.pressed(), Some(other), "a press elsewhere stays");
    tree.set_visibility(panel, Visibility::Visible);
    tree.set_hovered(Some(button));
    tree.set_enabled(button, false);
    assert_eq!(tree.hovered(), None, "disabled");
    assert_eq!(tree.pressed(), Some(other));
    tree.set_enabled(button, true);

    tree.set_hovered(Some(button));
    tree.remove(panel);
    assert_eq!(tree.hovered(), None, "removed with its parent");
    assert_eq!(tree.pressed(), Some(other));
    tree.remove(other);
    assert_eq!(tree.pressed(), None, "removed");
}

#[test]
fn modal_cursor_focus_links_and_wrap_are_node_data_that_mark_nothing() {
    let (mut tree, root) = clean_tree();
    let panel = tree.panel(root, LayoutStyle::default());
    let button = tree.button(panel, "ok");
    settle(&mut tree);

    assert!(!tree.modal(root));
    tree.set_modal(root, true);
    assert!(tree.modal(root));
    tree.set_modal(panel, true);
    assert!(!tree.modal(panel), "only a root is modal");

    assert_eq!(tree.cursor(button), None);
    tree.set_cursor(button, Some(CursorIcon::Pointer));
    assert_eq!(tree.cursor(button), Some(CursorIcon::Pointer));
    assert_eq!(
        tree.cursor(tree.children(button)[0]),
        None,
        "the set one only"
    );
    tree.set_cursor(button, None);
    assert_eq!(tree.cursor(button), None);
    assert_eq!(CursorIcon::default(), CursorIcon::Default);

    assert_eq!(tree.focus_neighbor(button, NavDirection::Up), None);
    tree.set_focus_neighbor(button, NavDirection::Up, Some(panel));
    assert_eq!(tree.focus_neighbor(button, NavDirection::Up), Some(panel));
    assert_eq!(tree.focus_neighbor(button, NavDirection::Down), None);
    tree.set_focus_neighbor(button, NavDirection::Up, None);
    assert_eq!(tree.focus_neighbor(button, NavDirection::Up), None);

    assert!(!tree.focus_wrap(panel));
    tree.set_focus_wrap(panel, true);
    assert!(tree.focus_wrap(panel));

    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty(), "none of them marks anything");

    tree.set_cursor(button, Some(CursorIcon::Text));
    tree.set_focus_neighbor(button, NavDirection::Left, Some(root));
    tree.remove(panel);
    assert!(!tree.focus_wrap(panel), "an invalid ID reads as unset");
    assert_eq!(tree.cursor(button), None);
    assert_eq!(tree.focus_neighbor(button, NavDirection::Left), None);
    tree.set_cursor(button, Some(CursorIcon::Move));
    tree.set_modal(button, true);
    assert!(!tree.modal(button));
}

#[test]
fn widget_dirty_names_the_style_text_and_layout_a_widget_waits_on() {
    let (mut tree, root) = clean_tree();
    let label = tree.label(root, "x");
    let panel = tree.panel(root, LayoutStyle::default());
    let all = WidgetDirty {
        style: true,
        text: true,
        layout: true,
    };
    assert_eq!(
        tree.widget_dirty(label),
        Some(all),
        "a new label waits on all"
    );
    assert_eq!(
        tree.widget_dirty(panel),
        Some(WidgetDirty { text: false, ..all }),
        "only a label has a text node"
    );
    settle(&mut tree);
    assert_eq!(tree.widget_dirty(label), Some(WidgetDirty::default()));

    tree.set_text(label, "changed");
    assert_eq!(
        tree.widget_dirty(label),
        Some(WidgetDirty {
            style: false,
            ..all
        })
    );
    settle(&mut tree);
    tree.set_overrides(
        panel,
        crate::ui::StyleOverrides {
            background: Some([1, 2, 3, 4]),
            ..Default::default()
        },
    );
    assert_eq!(
        tree.widget_dirty(panel),
        Some(WidgetDirty {
            style: true,
            ..WidgetDirty::default()
        }),
        "a style change alone lays nothing out"
    );
    tree.remove(label);
    assert_eq!(tree.widget_dirty(label), None);
}
