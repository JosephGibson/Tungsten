use glam::Vec2;
use tungsten_core::Config;
use tungsten_core::text::TextNodeStore;
use tungsten_core::ui::{LayoutStyle, Rect, Sizing, UiLayer, UiTree, WidgetId, WidgetKind};

use super::UiHarness;
use crate::App;
use crate::app::WindowSize;

fn app() -> App {
    App::new(Config::default()).expect("App::new failed")
}

fn viewport(harness: &UiHarness) -> Vec2 {
    let size = harness.world().get_resource::<WindowSize>().unwrap();
    Vec2::new(size.width as f32, size.height as f32)
}

/// A root panel with one padded button, built through the harness.
fn menu(harness: &mut UiHarness) -> (WidgetId, WidgetId) {
    let tree = harness.tree_mut();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::column().with_padding(10.0));
    let button = tree.button(root, "Start");
    tree.set_layout_style(button, LayoutStyle::column().with_padding(4.0));
    (root, button)
}

#[test]
fn new_inserts_a_tree_when_absent_and_keeps_one_inserted_first() {
    let harness = UiHarness::new(app());
    assert!(harness.tree().roots().is_empty());
    assert!(harness.world().has_resource::<UiTree>());

    let mut app = app();
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    app.world_mut().insert_resource(tree);
    let harness = UiHarness::new(app);
    assert_eq!(harness.tree().roots(), &[root]);
    assert_eq!(harness.tree().layer(root), Some(UiLayer::Debug));
}

#[test]
fn a_frame_lays_the_tree_out_at_the_window_size() {
    let mut harness = UiHarness::new(app());
    let (root, button) = menu(&mut harness);
    assert_eq!(
        harness.rect(root),
        None,
        "nothing is laid out before the first frame"
    );
    assert_eq!(harness.step(1), 1);
    let viewport = viewport(&harness);
    assert!(viewport.x > 0.0 && viewport.y > 0.0);
    let root_rect = harness.rect(root).unwrap();
    assert_eq!(root_rect.min, Vec2::ZERO);
    assert_eq!(
        root_rect.width(),
        viewport.x,
        "a root's Auto width is the window's"
    );
    // "Start" is five glyphs of 8 px, 20 px tall, inside the button's padding and the root's.
    let button_rect = harness.rect(button).unwrap();
    assert_eq!(
        button_rect,
        Rect::from_pos_size(Vec2::new(10.0, 10.0), Vec2::new(viewport.x - 20.0, 28.0))
    );
    let label = harness.tree().children(button)[0];
    assert_eq!(
        harness.rect(label).unwrap(),
        Rect::from_pos_size(Vec2::new(14.0, 14.0), Vec2::new(viewport.x - 28.0, 20.0))
    );
    let node = harness.tree().text_node(label).unwrap();
    assert_eq!(harness.text().committed(node).unwrap().line_count, 1);
    assert_eq!(harness.text().counts.commits, 1);
}

#[test]
fn hit_and_focus_reach_the_tree() {
    let mut harness = UiHarness::new(app());
    let (root, button) = menu(&mut harness);
    harness.step(1);
    let label = harness.tree().children(button)[0];
    assert_eq!(
        harness.hit(20.0, 20.0),
        Some(label),
        "on the text: the button's label"
    );
    assert_eq!(
        harness.hit(12.0, 12.0),
        Some(button),
        "in the button's padding"
    );
    assert_eq!(harness.hit(5.0, 5.0), Some(root), "in the root's padding");
    assert_eq!(harness.hit(-1.0, 5.0), None);

    assert_eq!(harness.tree().focused(), None);
    assert_eq!(harness.focus_next(), Some(button));
    assert_eq!(harness.tree().focused(), Some(button));
    assert_eq!(
        harness.focus_prev(),
        Some(button),
        "the only focusable wraps onto itself"
    );
    harness.tree_mut().blur();
    assert!(harness.focus(button));
    assert!(!harness.focus(root));
    assert_eq!(harness.tree().kind(button), Some(WidgetKind::Button));
}

#[test]
fn dump_matches_the_layout_literal() {
    let mut harness = UiHarness::new(app());
    menu(&mut harness);
    harness.step(1);
    let w = viewport(&harness).x;
    assert_eq!(
        harness.dump(),
        format!(
            "Panel None Visible (0.0, 0.0) {w:.1}×48.0\n\
             \x20 Button Button Visible (10.0, 10.0) {:.1}×28.0\n\
             \x20   Label Label Visible (14.0, 14.0) {:.1}×20.0\n",
            w - 20.0,
            w - 28.0
        )
    );
}

/// What a system saw of a label's rect, frame by frame.
#[derive(Default)]
struct SeenRects(Vec<Option<Rect>>);

#[test]
fn a_system_sees_the_previous_frames_rects() {
    let mut app = app();
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let label = tree.label(root, "hud");
    app.world_mut().insert_resource(tree);
    app.world_mut().insert_resource(SeenRects::default());
    app.add_system(move |world| {
        let rect = world.get_resource::<UiTree>().unwrap().rect(label);
        world.get_resource_mut::<SeenRects>().unwrap().0.push(rect);
    });
    let mut harness = UiHarness::new(app);
    harness.step(2);
    let seen = &harness.world().get_resource::<SeenRects>().unwrap().0;
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], None, "the first frame runs before any layout");
    assert_eq!(
        seen[1],
        harness.rect(label),
        "the second frame reads the first frame's layout"
    );
    assert_eq!(seen[1].unwrap().height(), 20.0);
}

/// A label's width as a system saw it each frame, after setting its text.
#[derive(Default)]
struct SeenWidths(Vec<Option<f32>>);

fn counting_app() -> (App, WidgetId) {
    let mut app = app();
    let mut tree = UiTree::new();
    let root = tree.add_root(
        UiLayer::Game,
        LayoutStyle::row().with_size(Sizing::Auto, Sizing::Auto),
    );
    let label = tree.label(root, "");
    app.world_mut().insert_resource(tree);
    app.world_mut().insert_resource(SeenWidths::default());
    app.world_mut().insert_resource(0_u32);
    app.add_system(move |world| {
        let frame = {
            let frame = world.get_resource_mut::<u32>().unwrap();
            *frame += 1;
            *frame
        };
        let tree = world.get_resource_mut::<UiTree>().unwrap();
        let width = tree.rect(label).map(|rect| rect.width());
        tree.set_text(label, "x".repeat(frame as usize));
        world
            .get_resource_mut::<SeenWidths>()
            .unwrap()
            .0
            .push(width);
    });
    (app, label)
}

#[test]
fn a_batch_of_frames_records_what_single_steps_record() {
    let (app, label) = counting_app();
    let mut batch = UiHarness::new(app);
    assert_eq!(batch.step(2), 2);

    let (app, _) = counting_app();
    let mut single = UiHarness::new(app);
    assert_eq!(single.step(1), 1);
    assert_eq!(single.step(1), 1);

    let batch_seen = &batch.world().get_resource::<SeenWidths>().unwrap().0;
    let single_seen = &single.world().get_resource::<SeenWidths>().unwrap().0;
    assert_eq!(batch_seen, single_seen);
    assert_eq!(
        batch_seen,
        &vec![None, Some(8.0)],
        "frame 2 saw one glyph from frame 1's text"
    );
    assert_eq!(
        batch.rect(label).unwrap().width(),
        16.0,
        "two glyphs after frame 2"
    );
    assert_eq!(
        batch.layout(),
        tungsten_core::ui::LayoutStats::default(),
        "nothing changed since the last layout"
    );
}
