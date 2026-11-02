use glam::Vec2;
use std::path::PathBuf;
use tungsten_core::Config;
use tungsten_core::text::TextNodeStore;

use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
use tungsten_core::ui::{
    LayoutStyle, Rect, Sizing, UiDebug, UiKey, UiLayer, UiTree, Visibility, WidgetId, WidgetKind,
};
use tungsten_core::{AssetRegistry, World};

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

// ---- M46: the paint list through the frame ----

fn frame_ui(harness: &UiHarness) -> &tungsten_core::ui::PaintList {
    &harness.harness().draw().expect("a frame ran").ui
}

#[test]
fn an_app_with_no_roots_paints_an_empty_list_and_the_layout_stage_costs_nothing() {
    let mut harness = UiHarness::new(app());
    assert_eq!(harness.step(1), 1);
    let ui = frame_ui(&harness);
    assert!(ui.is_empty());
    assert_eq!(ui.clips.len(), 1, "the viewport clip alone");
    assert_eq!(harness.text().counts.commits, 0);
    let counts = harness
        .world()
        .get_resource::<crate::RenderCounts>()
        .unwrap();
    assert_eq!(counts.ui_commands, 0);
    assert_eq!(
        counts.ui_layout_us, 0,
        "the harness has no renderer, so the app's layout stage does not run"
    );
    assert_eq!(harness.dump_paint(), "clip 0 (0, 0) 1280×720\n");
}

#[test]
fn a_root_built_in_a_startup_system_is_laid_out_and_painted_the_next_frame() {
    let mut app = app();
    app.add_system_to(
        tungsten_core::Stage::Startup,
        tungsten_core::system("build_ui", |world: &mut World| {
            let tree = world.get_resource_mut::<UiTree>().unwrap();
            let root = tree.add_root(UiLayer::Game, LayoutStyle::column().with_padding(10.0));
            tree.label(root, "hello");
        }),
    );
    let mut harness = UiHarness::new(app);
    assert_eq!(harness.step(1), 1);
    assert!(
        frame_ui(&harness).is_empty(),
        "the first frame painted before the harness laid out"
    );
    assert_eq!(harness.step(1), 1);
    let ui = frame_ui(&harness);
    assert_eq!(ui.commands.len(), 2, "the root's rect and the label's text");
    assert_eq!(ui.text_batches().len(), 1);
    assert_eq!(ui.scale, 1.0);
    assert!(!harness.tree().is_paint_dirty());
}

#[test]
fn a_label_change_marks_paint_and_repaints_with_the_new_box() {
    let mut harness = UiHarness::new(app());
    // A row, so the button's box follows its text's width.
    let root = harness
        .tree_mut()
        .add_root(UiLayer::Game, LayoutStyle::row());
    let button = harness.tree_mut().button(root, "Start");
    assert_eq!(harness.step(2), 2);
    let before = frame_ui(&harness).clone();
    assert!(!harness.tree().is_paint_dirty());

    assert!(harness.tree_mut().set_text(button, "Resume the game"));
    assert!(harness.tree().is_paint_dirty());
    assert_eq!(harness.step(2), 2);
    let after = frame_ui(&harness).clone();
    assert_ne!(before, after, "the button's text run moved with its box");
    assert_eq!(after.commands.len(), before.commands.len());
    assert!(
        after.commands[1].rect().width() > before.commands[1].rect().width(),
        "the button widened"
    );
}

#[test]
fn a_static_tree_repaints_on_a_debug_flip_a_sprite_epoch_move_and_a_resize_only() {
    let mut harness = UiHarness::new(app());
    menu(&mut harness);
    assert_eq!(harness.step(2), 2);
    let painted = frame_ui(&harness).clone();
    assert!(!painted.is_empty());

    // Nothing changed: the same list comes back, untouched.
    assert_eq!(harness.step(1), 1);
    assert_eq!(frame_ui(&harness), &painted);
    assert!(!harness.tree().is_paint_dirty());

    // A debug flag flip repaints: the bounds overlay appends outlines.
    harness
        .world_mut()
        .get_resource_mut::<UiDebug>()
        .unwrap()
        .show_bounds = true;
    assert_eq!(harness.step(1), 1);
    let outlined = frame_ui(&harness).clone();
    assert_eq!(
        outlined.commands.len(),
        painted.commands.len() + 3,
        "one outline per painted node: root, button, label"
    );
    harness
        .world_mut()
        .get_resource_mut::<UiDebug>()
        .unwrap()
        .show_bounds = false;
    assert_eq!(harness.step(1), 1);
    assert_eq!(frame_ui(&harness), &painted);

    // A sprite epoch move repaints too, to the same list for a tree without
    // images.
    harness
        .world_mut()
        .get_resource_mut::<AssetRegistry>()
        .unwrap()
        .register_sprite(
            "icon".to_owned(),
            FilterMode::Nearest,
            4,
            4,
            PathBuf::from("icon.png"),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
    assert_eq!(harness.step(1), 1);
    assert_eq!(frame_ui(&harness), &painted);

    // A resize relayouts the tree and repaints it with the new viewport.
    harness.world_mut().insert_resource(WindowSize {
        width: 640,
        height: 480,
    });
    assert_eq!(harness.step(2), 2);
    let resized = frame_ui(&harness);
    assert_eq!(resized.clips[0].width, 640);
    assert_ne!(resized, &painted);
}

#[test]
fn hide_game_layers_leaves_the_debug_layer_and_paint_dumps_now() {
    let mut harness = UiHarness::new(app());
    menu(&mut harness);
    let debug_root = harness
        .tree_mut()
        .add_root(UiLayer::Debug, LayoutStyle::default());
    harness.tree_mut().label(debug_root, "fps");
    assert_eq!(harness.step(2), 2);
    let all = frame_ui(&harness).clone();
    assert_eq!(all.text_batches().len(), 2);

    harness
        .world_mut()
        .get_resource_mut::<UiDebug>()
        .unwrap()
        .hide_game_layers = true;
    assert_eq!(harness.step(1), 1);
    let debug_only = frame_ui(&harness).clone();
    assert_eq!(debug_only.commands.len(), 2, "the debug root and its label");
    assert_eq!(debug_only.text_batches().len(), 1);

    // `paint` paints now, with the world's flags, and clears the flag.
    let now = harness.paint();
    assert_eq!(now, debug_only);
    assert!(!harness.tree().is_paint_dirty());
    assert!(harness.dump_paint().contains("-- batch 0\n"));
}

#[test]
fn frame_draw_holds_a_prewarms_runs_in_one_frame_only() {
    let mut harness = UiHarness::new(app());
    let (root, _button) = menu(&mut harness);
    harness
        .tree_mut()
        .set_visibility(root, tungsten_core::ui::Visibility::Hidden);
    harness.tree_mut().prewarm(root);
    harness.step(1);
    assert!(
        harness.harness().draw().unwrap().ui_prewarm.is_none(),
        "not laid out yet: the request waits"
    );
    harness.layout();
    harness.step(1);
    let draw = harness.harness().draw().unwrap();
    let prewarm = draw.ui_prewarm.as_ref().expect("the runs, this frame");
    let label = harness.tree().children(harness.tree().children(root)[0])[0];
    let node = harness.tree().text_node(label).unwrap();
    assert_eq!(prewarm.commands.len(), 1);
    assert!(matches!(
        prewarm.commands[0],
        tungsten_core::ui::PaintCommand::Text { node: n, .. } if n == node
    ));
    assert!(draw.ui.is_empty(), "hidden: the frame draws none of it");
    harness.step(1);
    assert!(harness.harness().draw().unwrap().ui_prewarm.is_none());
}

#[test]
fn ui_harness_lays_out_at_the_window_relative_factor() {
    use tungsten_core::ScaleFactor;
    let mut harness = UiHarness::new(app());
    let (root, button) = menu(&mut harness);
    {
        let world = harness.world_mut();
        let scale = world.resource_mut::<ScaleFactor>();
        scale.set_reference_height(Some(720.0));
        scale.set_min_viewport(Some(Vec2::new(731.0, 411.0)));
        // A test inserts the window's size, as the template's fit test does.
        *world.resource_mut::<WindowSize>() = WindowSize {
            width: 3840,
            height: 2160,
        };
    }
    harness.step(1);
    assert_eq!(
        harness.world().resource::<ScaleFactor>().get(),
        3.0,
        "the frame read the inserted size"
    );
    assert_eq!(harness.rect(root).unwrap().width(), 1280.0);
    // The pointer maps through the same factor.
    let centre = {
        let rect = harness.rect(button).unwrap();
        (rect.min + rect.max) * 0.5 * 3.0
    };
    assert!(harness.hit(centre.x, centre.y).is_some());
    // A size changed between frames reaches the next layout without a frame.
    *harness.world_mut().resource_mut::<WindowSize>() = WindowSize {
        width: 1280,
        height: 720,
    };
    harness
        .world_mut()
        .resource_mut::<ScaleFactor>()
        .set_ui_scale(3.0);
    harness.layout();
    let width = harness.rect(root).unwrap().width();
    assert!((width - 731.0).abs() < 1e-3, "the floor holds: {width}");
}

/// A root of `width` × `height` units at the top-left.
fn sized_root(harness: &mut UiHarness, width: f32, height: f32) -> WidgetId {
    harness.tree_mut().add_root(
        UiLayer::Game,
        LayoutStyle::column().with_size(Sizing::Px(width), Sizing::Px(height)),
    )
}

/// A modal root on the game layer: a screen.
fn modal_root(harness: &mut UiHarness) -> WidgetId {
    let tree = harness.tree_mut();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::column());
    tree.set_modal(root, true);
    root
}

#[test]
fn find_takes_the_front_most_visible_focusable_match() {
    let mut harness = UiHarness::new(app());
    let back = harness
        .tree_mut()
        .add_root(UiLayer::Game, LayoutStyle::column());
    let front = harness
        .tree_mut()
        .add_root(UiLayer::Game, LayoutStyle::column());
    let tree = harness.tree_mut();
    let behind = tree.button(back, "Play");
    let panel = tree.panel(front, LayoutStyle::default());
    tree.set_accessible_name(panel, Some("Play".to_owned()));
    let disabled = tree.button(front, "Play");
    tree.set_enabled(disabled, false);
    let ahead = tree.button(front, "Play");
    assert_eq!(
        harness.find("Play"),
        Some(ahead),
        "the front root's control that can take focus, past a named panel, a disabled button and labels"
    );
    harness
        .tree_mut()
        .set_visibility(ahead, Visibility::Collapsed);
    assert_eq!(
        harness.find("Play"),
        Some(behind),
        "a collapsed control is passed over"
    );
    harness
        .tree_mut()
        .set_visibility(ahead, Visibility::Visible);
    harness.tree_mut().set_visibility(front, Visibility::Hidden);
    assert_eq!(harness.find("Play"), Some(behind), "and a hidden root");
    assert_eq!(harness.find("Quit"), None);
}

#[test]
fn find_looks_past_a_toast_to_the_screen_below() {
    let mut harness = UiHarness::new(app());
    let covered = modal_root(&mut harness);
    let screen = modal_root(&mut harness);
    let tree = harness.tree_mut();
    tree.button(covered, "Retry");
    let retry = tree.button(screen, "Retry");
    crate::screens::toast(harness.world_mut(), "Retry");
    let tree = harness.tree();
    let toast_root = *tree.roots().last().expect("the toasts' root");
    assert!(
        !tree.modal(toast_root) && toast_root != screen,
        "the toast is front-most"
    );
    assert_eq!(
        harness.front_modal(),
        Some(screen),
        "the toast is no screen"
    );
    assert_eq!(
        harness.find("Retry"),
        Some(retry),
        "the toast's label takes no focus, the covered screen is behind"
    );
}

#[test]
fn named_finds_disabled_widgets_on_the_front_modal() {
    let mut harness = UiHarness::new(app());
    let covered = modal_root(&mut harness);
    let screen = modal_root(&mut harness);
    let hud = harness
        .tree_mut()
        .add_root(UiLayer::Game, LayoutStyle::column());
    let tree = harness.tree_mut();
    let back = tree.button(covered, "Back");
    let apply = tree.button(screen, "Apply");
    tree.set_enabled(apply, false);
    tree.label(screen, "Volume 80%");
    let hud_apply = tree.button(hud, "Apply");
    assert_eq!(
        harness.front_modal(),
        Some(screen),
        "a root that is not modal is no screen"
    );
    assert_eq!(
        harness.named("Apply"),
        Some(apply),
        "disabled, on the front screen"
    );
    assert_eq!(
        harness.find("Apply"),
        Some(hud_apply),
        "find takes focusable controls only"
    );
    assert_eq!(harness.named("Back"), None, "the covered screen's");
    assert!(harness.shows("Volume 80%"));
    assert!(harness.shows("Apply"), "a button's text");
    assert!(!harness.shows("Back"));
    harness
        .tree_mut()
        .set_visibility(screen, Visibility::Hidden);
    assert_eq!(harness.front_modal(), Some(covered));
    assert_eq!(harness.named("Back"), Some(back));
}

#[test]
fn find_key_finds_a_keyed_widget() {
    let mut harness = UiHarness::new(app());
    let back = harness
        .tree_mut()
        .add_root(UiLayer::Game, LayoutStyle::column());
    let front = harness
        .tree_mut()
        .add_root(UiLayer::Game, LayoutStyle::column());
    let tree = harness.tree_mut();
    let behind = tree.button(back, "Volume");
    tree.set_key(behind, "volume");
    tree.button(front, "volume");
    let keyed = tree.button(front, "Loudness");
    tree.set_key(keyed, "volume");
    let volume = UiKey::new("volume");
    assert_eq!(
        harness.find_key(volume),
        Some(keyed),
        "by key, not caption, from the front"
    );
    assert_eq!(harness.find_key(UiKey::new("music")), None);
    harness.tree_mut().set_visibility(front, Visibility::Hidden);
    assert_eq!(harness.find_key(volume), Some(behind));
}

#[test]
#[should_panic(expected = "wide: ")]
fn assert_fits_fails_a_box_outside_its_parent() {
    let mut harness = UiHarness::new(app());
    let root = sized_root(&mut harness, 200.0, 100.0);
    let panel = harness.tree_mut().panel(
        root,
        LayoutStyle::default().with_size(Sizing::Px(150.0), Sizing::Px(40.0)),
    );
    harness.assert_fits("narrow");
    harness.tree_mut().set_layout_style(
        panel,
        LayoutStyle::default().with_size(Sizing::Px(300.0), Sizing::Px(40.0)),
    );
    harness.assert_fits("wide");
}

/// Ten buttons in a list 80 units tall, a scroll panel or a plain one.
fn long_list(harness: &mut UiHarness, scroll: bool) -> WidgetId {
    let root = sized_root(harness, 200.0, 100.0);
    let tree = harness.tree_mut();
    let style = LayoutStyle::column().with_size(Sizing::Fill, Sizing::Px(80.0));
    let list = if scroll {
        tree.scroll_panel(root, style)
    } else {
        tree.panel(root, style)
    };
    for row in 0..10 {
        tree.button(list, format!("Row {row}"));
    }
    list
}

#[test]
fn assert_fits_lets_a_scroll_panel_scroll() {
    let mut harness = UiHarness::new(app());
    let list = long_list(&mut harness, true);
    harness.assert_fits("a scrolled list");
    assert!(
        harness.tree().scroll_max(list).is_some_and(|max| max > 0.0),
        "its rows overflow it"
    );
    let plain = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut harness = UiHarness::new(app());
        long_list(&mut harness, false);
        harness.assert_fits("a plain list");
    }));
    assert!(plain.is_err(), "the same rows in a plain panel do not fit");
}

#[test]
#[should_panic(expected = "narrow: \"A label that wraps\" on one line")]
fn assert_fits_one_line_fails_a_wrapped_label() {
    let mut harness = UiHarness::new(app());
    let root = sized_root(&mut harness, 120.0, 100.0);
    let tree = harness.tree_mut();
    let short = tree.label(root, "Short");
    // 18 glyphs of 8 units: 144 units, wrapped at 120.
    let long = tree.label(root, "A label that wraps");
    harness.assert_fits_one_line("narrow", &[short]);
    harness.assert_fits_one_line("narrow", &[short, long]);
}

#[test]
fn tree_json_names_key_text_rect_focus_and_shown() {
    let mut harness = UiHarness::new(app());
    let (root, button) = menu(&mut harness);
    let tree = harness.tree_mut();
    tree.set_key(button, "start");
    let hidden = tree.label(root, "Secret");
    tree.set_visibility(hidden, Visibility::Hidden);
    tree.focus(button);
    harness.step(1);
    let json: serde_json::Value =
        serde_json::from_str(&harness.tree_json()).expect("the dump is JSON");
    let nodes = json["nodes"].as_array().expect("its nodes");
    assert_eq!(
        nodes.len(),
        4,
        "the root, the button, its label and the hidden label"
    );
    let id_of = |id: WidgetId| format!("{}:{}", id.index(), id.generation());
    let node = |id: WidgetId| {
        nodes
            .iter()
            .find(|node| node["id"] == id_of(id))
            .unwrap_or_else(|| panic!("{id:?} in the dump"))
    };
    let start = node(button);
    assert_eq!(start["kind"], "Button");
    assert_eq!(start["key"], "start");
    assert_eq!(start["name"], "Start");
    assert_eq!(start["text"], "Start");
    assert_eq!(start["root"], id_of(root));
    assert_eq!(start["depth"], 1);
    assert_eq!(start["focused"], true);
    assert_eq!(start["shown"], true);
    let rect = harness.rect(button).expect("laid out");
    assert_eq!(
        start["rect"],
        serde_json::json!([rect.min.x, rect.min.y, rect.width(), rect.height()])
    );
    let secret = node(hidden);
    assert_eq!(secret["shown"], false);
    assert_eq!(secret["focused"], false);
    assert_eq!(secret["text"], "Secret");
    let top = node(root);
    assert_eq!(top["depth"], 0);
    assert!(
        top["key"].is_null() && top["text"].is_null(),
        "a panel holds neither"
    );
}
