use glam::Vec2;

use super::*;
use crate::text::{TextMeasure, TextNodeStore, TextWrap};
use crate::ui::theme::{StyleOverrides, TextOverrides, Theme};
use crate::ui::{
    Align, Anchor, FixedAdvanceMeasure, LayoutStyle, Sizing, UiLayer, UiTree, Visibility, WidgetId,
};

/// 8 px per glyph at the theme's size 16; lines are 20 tall.
const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
const FOX: &str = "The quick brown fox jumps over the lazy dog";
const EPS: f32 = 1e-3;

fn store() -> FixedAdvanceMeasure {
    FixedAdvanceMeasure::new(0.5)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= EPS
}

fn fill() -> LayoutStyle {
    LayoutStyle {
        width: Sizing::Fill,
        ..LayoutStyle::default()
    }
}

/// Every laid-out label holds a commit under the current epoch whose lines fit
/// its box: measurement, the committed width and the solved height agree.
fn assert_commits_fit(tree: &UiTree, store: &mut FixedAdvanceMeasure) {
    fn walk(tree: &UiTree, store: &mut FixedAdvanceMeasure, id: WidgetId) {
        if tree.kind(id) == Some(crate::ui::WidgetKind::Label)
            && let Some(rect) = tree.rect(id)
        {
            let node = tree
                .text_node(id)
                .expect("a laid-out label has a text node");
            let committed = store
                .committed(node)
                .expect("a laid-out label is committed");
            assert!(
                committed.size.y <= rect.height() + EPS,
                "{id:?}: {} px of lines in a {} px box",
                committed.size.y,
                rect.height()
            );
            let at_width = store.measure(
                node,
                Some(rect.width()),
                crate::text::MeasureWidth::MaxContent,
            );
            assert_eq!(
                committed.line_count, at_width.line_count,
                "{id:?}: the commit wraps as the solve did"
            );
        }
        for child in tree.children(id) {
            walk(tree, store, *child);
        }
    }
    for root in tree.roots() {
        walk(tree, store, *root);
    }
}

/// Case A: a 300-wide column (padding 10, gap 10) holding a row (gap 10) of a
/// 15-character `Auto` label and the fox sized `Fill`.
fn case_a() -> (UiTree, WidgetId, WidgetId, WidgetId, WidgetId) {
    let mut tree = UiTree::new();
    let column = tree.add_root(
        UiLayer::Game,
        LayoutStyle::column()
            .with_size(Sizing::Px(300.0), Sizing::Auto)
            .with_padding(10.0)
            .with_gap(10.0),
    );
    let row = tree.panel(column, LayoutStyle::row().with_gap(10.0));
    let short = tree.label(row, "Hello, world!!!");
    let long = tree.label(row, FOX);
    tree.set_layout_style(long, fill());
    (tree, column, row, short, long)
}

#[test]
fn case_a_a_wrapped_fill_label_beside_a_content_sized_one() {
    let (mut tree, _column, _row, short, long) = case_a();
    let mut store = store();
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 1);
    assert_eq!(stats.nodes_laid_out, 4);
    assert_eq!(stats.commits, 2);
    assert!(stats.measure_calls > 0);
    assert_eq!(
        tree.dump_layout(),
        "Panel None Visible (0.0, 0.0) 300.0×80.0\n\
         \x20 Panel None Visible (10.0, 10.0) 280.0×60.0\n\
         \x20   Label Label Visible (10.0, 10.0) 120.0×60.0\n\
         \x20   Label Label Visible (140.0, 10.0) 150.0×60.0\n"
    );
    let long_node = tree.text_node(long).unwrap();
    assert_eq!(store.committed(long_node).unwrap().line_count, 3);
    assert_eq!(
        store
            .committed(tree.text_node(short).unwrap())
            .unwrap()
            .line_count,
        1
    );
    assert_commits_fit(&tree, &mut store);
}

#[test]
fn case_b_hidden_keeps_its_box_and_collapsed_frees_it() {
    let mut tree = UiTree::new();
    let column = tree.add_root(
        UiLayer::Game,
        LayoutStyle::column().with_size(Sizing::Px(150.0), Sizing::Auto),
    );
    let top = tree.panel(
        column,
        LayoutStyle::default().with_size(Sizing::Auto, Sizing::Px(50.0)),
    );
    let label = tree.label(column, FOX);
    let bottom = tree.panel(
        column,
        LayoutStyle::default().with_size(Sizing::Auto, Sizing::Px(50.0)),
    );
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    let visible = tree.dump_layout();
    assert_eq!(
        visible,
        "Panel None Visible (0.0, 0.0) 150.0×160.0\n\
         \x20 Panel None Visible (0.0, 0.0) 150.0×50.0\n\
         \x20 Label Label Visible (0.0, 50.0) 150.0×60.0\n\
         \x20 Panel None Visible (0.0, 110.0) 150.0×50.0\n"
    );
    assert_commits_fit(&tree, &mut store);

    tree.set_visibility(label, Visibility::Hidden);
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.commits, 1, "the hidden label commits too");
    assert_eq!(
        tree.dump_layout(),
        visible.replace("Label Label Visible", "Label Label Hidden"),
        "hidden keeps its box"
    );
    assert_eq!(tree.rect(bottom).unwrap().min.y, 110.0);
    assert_commits_fit(&tree, &mut store);

    tree.set_visibility(label, Visibility::Collapsed);
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.commits, 0, "a collapsed label is not committed");
    assert_eq!(stats.nodes_laid_out, 3);
    assert_eq!(
        tree.dump_layout(),
        "Panel None Visible (0.0, 0.0) 150.0×100.0\n\
         \x20 Panel None Visible (0.0, 0.0) 150.0×50.0\n\
         \x20 Label Label Collapsed -\n\
         \x20 Panel None Visible (0.0, 50.0) 150.0×50.0\n"
    );
    assert_eq!(tree.rect(label), None);
    assert_eq!(tree.rect(top).unwrap().height(), 50.0);
    assert_commits_fit(&tree, &mut store);

    tree.set_visibility(label, Visibility::Visible);
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.dump_layout(), visible);
}

#[test]
fn case_c_min_and_max_widths_clamp_a_fill_child() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let row1 = tree.panel(
        root,
        LayoutStyle::row().with_size(Sizing::Px(300.0), Sizing::Px(50.0)),
    );
    let capped = tree.panel(
        row1,
        LayoutStyle {
            max_width: Some(100.0),
            ..fill()
        },
    );
    let row2 = tree.panel(
        root,
        LayoutStyle::row().with_size(Sizing::Px(150.0), Sizing::Px(50.0)),
    );
    let floored = tree.panel(
        row2,
        LayoutStyle {
            min_width: Some(200.0),
            ..fill()
        },
    );
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.rect(capped).unwrap().size(), Vec2::new(100.0, 50.0));
    assert_eq!(tree.rect(capped).unwrap().min, Vec2::ZERO);
    let floored = tree.rect(floored).unwrap();
    assert_eq!(floored.size(), Vec2::new(200.0, 50.0));
    assert!(
        floored.max.x > tree.rect(row2).unwrap().max.x,
        "the child overflows its row"
    );
    assert_eq!(
        tree.rect(root).unwrap().size(),
        Vec2::new(400.0, 100.0),
        "a root's Auto width stretches to the viewport"
    );
}

#[test]
fn case_d_anchors_place_against_the_viewport() {
    let mut tree = UiTree::new();
    let corner = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default()
            .with_size(Sizing::Px(100.0), Sizing::Px(40.0))
            .anchored(Anchor::BottomEnd, Vec2::splat(10.0)),
    );
    let centre = tree.add_root(
        UiLayer::GamePopup,
        LayoutStyle::default()
            .with_size(Sizing::Px(100.0), Sizing::Px(40.0))
            .anchored(Anchor::Center, Vec2::new(0.0, 10.0)),
    );
    let top_centre = tree.add_root(
        UiLayer::Debug,
        LayoutStyle::default()
            .with_size(Sizing::Px(50.0), Sizing::Px(20.0))
            .anchored(Anchor::TopCenter, Vec2::new(5.0, 7.0)),
    );
    let mut store = store();
    tree.layout(Vec2::new(300.0, 200.0), &mut store);
    assert_eq!(
        tree.rect(corner).unwrap(),
        Rect::from_pos_size(Vec2::new(190.0, 150.0), Vec2::new(100.0, 40.0))
    );
    assert_eq!(
        tree.rect(centre).unwrap(),
        Rect::from_pos_size(Vec2::new(100.0, 90.0), Vec2::new(100.0, 40.0))
    );
    assert_eq!(
        tree.rect(top_centre).unwrap(),
        Rect::from_pos_size(Vec2::new(130.0, 7.0), Vec2::new(50.0, 20.0))
    );

    // An anchored child inside a padded panel positions against the panel's box.
    let mut tree = UiTree::new();
    let panel = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default().with_size(Sizing::Px(200.0), Sizing::Px(100.0)),
    );
    let badge = tree.panel(
        panel,
        LayoutStyle::default()
            .with_size(Sizing::Px(20.0), Sizing::Px(10.0))
            .anchored(Anchor::TopEnd, Vec2::new(4.0, 6.0)),
    );
    let flow = tree.panel(
        panel,
        LayoutStyle::default().with_size(Sizing::Auto, Sizing::Px(30.0)),
    );
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.rect(badge).unwrap().min, Vec2::new(176.0, 6.0));
    assert_eq!(
        tree.rect(flow).unwrap(),
        Rect::from_pos_size(Vec2::ZERO, Vec2::new(200.0, 30.0)),
        "the anchored child takes no flow space"
    );
}

#[test]
fn min_content_per_wrap_mode_in_a_row_narrower_than_the_text() {
    let mut store = store();
    for (wrap, expected_width) in [
        (TextWrap::Word, 40.0),        // the widest word, "quick"
        (TextWrap::WordOrGlyph, 40.0), // as the engine computes it
        (TextWrap::Glyph, 30.0),       // one glyph is the floor, so the label fills the row
        (TextWrap::None, 344.0),       // the unwrapped line
    ] {
        let mut tree = UiTree::new();
        let root = tree.add_root(
            UiLayer::Game,
            LayoutStyle::row().with_size(Sizing::Px(30.0), Sizing::Auto),
        );
        let label = tree.label(root, FOX);
        tree.set_layout_style(label, fill());
        tree.set_overrides(
            label,
            StyleOverrides {
                text: TextOverrides {
                    wrap: Some(wrap),
                    ..TextOverrides::default()
                },
                ..StyleOverrides::default()
            },
        );
        tree.layout(VIEWPORT, &mut store);
        assert!(
            near(tree.rect(label).unwrap().width(), expected_width),
            "{wrap:?}: {} wide",
            tree.rect(label).unwrap().width()
        );
        assert_commits_fit(&tree, &mut store);
    }
}

#[test]
fn a_second_layout_with_nothing_changed_does_nothing() {
    let (mut tree, _column, _row, _short, _long) = case_a();
    let mut store = store();
    let first = tree.layout(VIEWPORT, &mut store);
    assert_eq!(first.roots_laid_out, 1);
    let before = store.counts;
    let second = tree.layout(VIEWPORT, &mut store);
    assert_eq!(second, LayoutStats::default());
    assert_eq!(
        store.counts, before,
        "no measure, set or commit reached the store"
    );
    assert!(!tree.is_layout_dirty());
}

#[test]
fn a_change_under_one_root_lays_out_that_root_alone() {
    let mut tree = UiTree::new();
    let game = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let game_label = tree.label(game, "score");
    let debug = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    tree.label(debug, "fps");
    let mut store = store();
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 2);
    assert_eq!(stats.commits, 2);

    assert!(tree.set_text(game_label, "score: 10"));
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 1);
    assert_eq!(stats.nodes_laid_out, 2);
    assert_eq!(stats.commits, 1, "the text nodes of the laid-out root");
    assert_eq!(
        store
            .committed(tree.text_node(game_label).unwrap())
            .unwrap()
            .size
            .x,
        9.0 * 8.0
    );
    assert_commits_fit(&tree, &mut store);
}

#[test]
fn an_empty_label_keeps_one_line_height() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let empty = tree.label(root, "");
    let below = tree.label(root, "x");
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.rect(empty).unwrap().size(), Vec2::new(400.0, 20.0));
    assert_eq!(tree.rect(below).unwrap().min.y, 20.0);
    assert_commits_fit(&tree, &mut store);
}

#[test]
fn an_initially_hidden_label_reserves_its_twins_box() {
    let mut store = store();
    let build = |hidden: bool| {
        let mut tree = UiTree::new();
        let column = tree.add_root(
            UiLayer::Game,
            LayoutStyle::column().with_size(Sizing::Px(150.0), Sizing::Auto),
        );
        let label = tree.label(column, FOX);
        if hidden {
            tree.set_visibility(label, Visibility::Hidden);
        }
        let after = tree.label(column, "end");
        (tree, label, after)
    };
    let (mut visible, v_label, v_after) = build(false);
    let (mut hidden, h_label, h_after) = build(true);
    visible.layout(VIEWPORT, &mut store);
    hidden.layout(VIEWPORT, &mut store);
    assert_eq!(hidden.rect(h_label), visible.rect(v_label));
    assert_eq!(hidden.rect(h_after), visible.rect(v_after));
    assert_eq!(hidden.rect(h_after).unwrap().min.y, 60.0);

    // Still after a text change while hidden, and after a viewport change.
    for tree in [&mut visible, &mut hidden] {
        let label = tree.children(tree.roots()[0])[0];
        assert!(tree.set_text(label, "The quick brown fox"));
    }
    visible.layout(VIEWPORT, &mut store);
    hidden.layout(VIEWPORT, &mut store);
    assert_eq!(hidden.rect(h_label), visible.rect(v_label));
    assert_eq!(hidden.rect(h_after).unwrap().min.y, 40.0, "two lines now");

    let narrow = Vec2::new(100.0, 300.0);
    for tree in [&mut visible, &mut hidden] {
        let root = tree.roots()[0];
        tree.set_layout_style(
            root,
            LayoutStyle::column().with_size(Sizing::Px(80.0), Sizing::Auto),
        );
        tree.layout(narrow, &mut store);
    }
    assert_eq!(hidden.rect(h_label), visible.rect(v_label));
    assert_eq!(
        hidden.rect(h_after).unwrap().min.y,
        40.0,
        "two lines at 80 px: \"The quick\" / \"brown fox\""
    );
    assert_commits_fit(&visible, &mut store);
    assert_commits_fit(&hidden, &mut store);
}

#[test]
fn a_font_epoch_bump_relayouts_every_root_with_text() {
    let mut tree = UiTree::new();
    let game = tree.add_root(UiLayer::Game, LayoutStyle::default());
    tree.label(game, "a");
    let popup = tree.add_root(UiLayer::GamePopup, LayoutStyle::default());
    tree.button(popup, "b");
    let bare = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    tree.panel(bare, LayoutStyle::default());
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.layout(VIEWPORT, &mut store), LayoutStats::default());

    store.bump_font_epoch();
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(
        stats.roots_laid_out, 2,
        "the roots with text; the bare one stays put"
    );
    assert_eq!(stats.commits, 2);
    assert_commits_fit(&tree, &mut store);
    assert_eq!(tree.layout(VIEWPORT, &mut store), LayoutStats::default());
}

fn themed_button(
    disabled_text: TextOverrides,
    background: Option<[u8; 4]>,
) -> (UiTree, WidgetId, WidgetId, FixedAdvanceMeasure) {
    let mut tree = UiTree::new();
    let mut theme = Theme::default();
    theme.kinds.button.disabled = StyleOverrides {
        background,
        text: disabled_text,
        ..StyleOverrides::default()
    };
    tree.set_theme(theme);
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let button = tree.button(root, "Start");
    let label = tree.children(button)[0];
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.layout(VIEWPORT, &mut store), LayoutStats::default());
    (tree, button, label, store)
}

#[test]
fn a_state_change_relayouts_only_when_the_resolved_text_changed() {
    // A larger disabled text size: the root relayouts and the label grows.
    let (mut tree, button, label, mut store) = themed_button(
        TextOverrides {
            size: Some(32.0),
            line_height: Some(40.0),
            ..TextOverrides::default()
        },
        None,
    );
    assert_eq!(tree.rect(label).unwrap().height(), 20.0);
    tree.set_enabled(button, false);
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 1);
    assert_eq!(stats.commits, 1);
    assert_eq!(tree.rect(label).unwrap().height(), 40.0);
    assert_commits_fit(&tree, &mut store);

    // The background only: nothing relayouts.
    let (mut tree, button, label, mut store) =
        themed_button(TextOverrides::default(), Some([5, 5, 5, 255]));
    tree.set_enabled(button, false);
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats, LayoutStats::default());
    assert!(store.committed(tree.text_node(label).unwrap()).is_some());

    // The text colour: the root relayouts and the label keeps a valid commit.
    let (mut tree, button, label, mut store) = themed_button(
        TextOverrides {
            color: Some([6, 6, 6, 255]),
            ..TextOverrides::default()
        },
        None,
    );
    let before = store.counts;
    tree.set_enabled(button, false);
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(stats.roots_laid_out, 1);
    assert_eq!(stats.commits, 1);
    assert_eq!(
        store.counts.set_text_calls,
        before.set_text_calls + 1,
        "the node is re-set with its new colour"
    );
    let node = tree.text_node(label).unwrap();
    assert!(
        store.committed(node).is_some(),
        "the commit survives the colour change"
    );
    assert_commits_fit(&tree, &mut store);
}

#[test]
fn case_g_a_fractional_wrap_boundary() {
    let mut store = store();
    let mut run = |width: f32| {
        let mut tree = UiTree::new();
        let column = tree.add_root(
            UiLayer::Game,
            LayoutStyle::column().with_size(Sizing::Px(width), Sizing::Auto),
        );
        let label = tree.label(column, "ab");
        tree.set_overrides(
            label,
            StyleOverrides {
                text: TextOverrides {
                    size: Some(32.4),
                    line_height: Some(40.0),
                    ..TextOverrides::default()
                },
                ..StyleOverrides::default()
            },
        );
        tree.layout(VIEWPORT, &mut store);
        let rect = tree.rect(label).unwrap();
        let lines = store
            .committed(tree.text_node(label).unwrap())
            .unwrap()
            .line_count;
        assert_commits_fit(&tree, &mut store);
        (rect, lines)
    };
    let (wide, wide_lines) = run(32.4);
    assert_eq!(wide_lines, 1);
    assert!(
        near(wide.width(), 32.4) && near(wide.height(), 40.0),
        "{wide:?}"
    );
    let (narrow, narrow_lines) = run(32.3);
    assert_eq!(narrow_lines, 2);
    assert!(
        near(narrow.width(), 32.3) && near(narrow.height(), 80.0),
        "{narrow:?}"
    );
}

#[test]
fn a_removed_labels_text_node_is_released_at_the_next_layout() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let keep = tree.label(root, "keep");
    let gone = tree.button(root, "gone");
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(store.live_nodes(), 2);
    let gone_node = tree.text_node(tree.children(gone)[0]).unwrap();
    tree.remove(gone);
    assert_eq!(
        store.live_nodes(),
        2,
        "released at the next layout, not before"
    );
    let stats = tree.layout(VIEWPORT, &mut store);
    assert_eq!(store.live_nodes(), 1);
    assert_eq!(store.committed(gone_node), None);
    assert_eq!(stats.commits, 1);
    assert!(store.committed(tree.text_node(keep).unwrap()).is_some());
    assert_commits_fit(&tree, &mut store);
}

#[test]
fn stretch_is_the_default_and_start_keeps_content_height() {
    let mut tree = UiTree::new();
    let row = tree.add_root(
        UiLayer::Game,
        LayoutStyle::row().with_size(Sizing::Px(300.0), Sizing::Px(60.0)),
    );
    let stretched = tree.label(row, "a");
    let topped = tree.label(row, "b");
    tree.set_layout_style(
        topped,
        LayoutStyle {
            align_self: Some(Align::Start),
            ..LayoutStyle::default()
        },
    );
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert_eq!(tree.rect(stretched).unwrap().size(), Vec2::new(8.0, 60.0));
    assert_eq!(tree.rect(topped).unwrap().size(), Vec2::new(8.0, 20.0));
    assert_commits_fit(&tree, &mut store);
}

#[test]
fn a_fractional_percent_width_stays_fractional() {
    let mut tree = UiTree::new();
    let row = tree.add_root(
        UiLayer::Game,
        LayoutStyle::row().with_size(Sizing::Px(300.0), Sizing::Px(50.0)),
    );
    let third = tree.panel(
        row,
        LayoutStyle::default().with_size(Sizing::Percent(33.3), Sizing::Auto),
    );
    let label = tree.label(row, FOX);
    tree.set_layout_style(label, fill());
    let mut store = store();
    tree.layout(VIEWPORT, &mut store);
    assert!(near(tree.rect(third).unwrap().width(), 99.9));
    assert!(near(tree.rect(label).unwrap().width(), 200.1));
    assert_eq!(
        store
            .committed(tree.text_node(label).unwrap())
            .unwrap()
            .line_count,
        2
    );
    assert_commits_fit(&tree, &mut store);
}
