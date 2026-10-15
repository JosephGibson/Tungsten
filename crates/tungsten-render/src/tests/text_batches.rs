use std::collections::HashMap;
use std::path::Path;

use glam::Vec2;
use tungsten_core::assets::ResolvedFontFamily;
use tungsten_core::text::{StyledText, TextStyle};

use super::*;

fn font_bytes(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fonts")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The packaged sans face as the `sans` family, no GPU.
fn engine() -> TextEngine {
    let mut engine = TextEngine::new(FontSource::Packaged);
    engine.load_font("sans", font_bytes("Inter/static/Inter-Regular.ttf"));
    let families: HashMap<String, ResolvedFontFamily> = [(
        "sans".to_string(),
        ResolvedFontFamily::new(vec!["sans".to_string()]),
    )]
    .into_iter()
    .collect();
    engine.set_font_families(&families, &["sans".to_string()]);
    engine
}

fn style() -> TextStyle {
    TextStyle::new("sans", 16.0, 20.0)
}

/// A committed node holding `text`.
fn committed_node(engine: &mut TextEngine, text: &str) -> TextNodeId {
    let node = engine.create_node();
    engine.set_text(node, &StyledText::from(text), &style());
    engine.commit_layout(node, Vec2::new(200.0, 20.0));
    node
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_pos_size(Vec2::new(x, y), Vec2::new(width, height))
}

fn clip(x: i32, y: i32, width: i32, height: i32) -> ClipRect {
    ClipRect {
        x,
        y,
        width,
        height,
    }
}

fn text(node: TextNodeId, rect: Rect, clip: u32) -> PaintCommand {
    PaintCommand::Text {
        node,
        rect,
        color: [255, 255, 255, 255],
        scale: 1.0,
        clip,
    }
}

fn panel(rect: Rect, clip: u32) -> PaintCommand {
    PaintCommand::Rect {
        rect,
        fill: [0, 0, 0, 255],
        border: [0, 0, 0, 0],
        border_width: 0.0,
        corner_radius: 0.0,
        clip,
    }
}

fn epoch(value: u64) -> FontEpoch {
    FontEpoch::new(value)
}

fn signatures(list: &PaintList, engine: &TextEngine) -> Vec<BatchSignature> {
    list.text_batches()
        .into_iter()
        .map(|range| batch_signature(list, range, engine, 800, 600))
        .collect()
}

// ---- the trim rule ----

#[test]
fn the_kth_changed_frame_is_full_and_resets_the_count() {
    let mut policy = TrimPolicy::default();
    let mut kinds = Vec::new();
    for _ in 0..(FULL_FRAME_INTERVAL * 2) {
        kinds.push(policy.decide(epoch(1), 2, true));
    }
    let full_at: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| matches!(kind, FrameKind::Full { .. }))
        .map(|(i, _)| i + 1)
        .collect();
    assert_eq!(
        full_at,
        [
            FULL_FRAME_INTERVAL as usize,
            2 * FULL_FRAME_INTERVAL as usize
        ],
        "every Kth changed frame"
    );
    assert_eq!(
        kinds[0],
        FrameKind::Partial,
        "the first changed frame is partial"
    );
    assert_eq!(
        kinds[FULL_FRAME_INTERVAL as usize - 1],
        FrameKind::Full { forced: false }
    );
    assert_eq!(policy.full_frames_total(), 2);
    assert_eq!(policy.atlas_full_total(), 0);
}

#[test]
fn an_unchanged_frame_prepares_nothing_and_advances_no_counter() {
    let mut policy = TrimPolicy::default();
    for _ in 0..3 {
        assert_eq!(policy.decide(epoch(1), 2, true), FrameKind::Partial);
    }
    for _ in 0..100 {
        assert_eq!(policy.decide(epoch(1), 2, false), FrameKind::Nothing);
    }
    for _ in 0..(FULL_FRAME_INTERVAL - 4) {
        assert_eq!(policy.decide(epoch(1), 2, true), FrameKind::Partial);
    }
    assert_eq!(
        policy.decide(epoch(1), 2, true),
        FrameKind::Full { forced: false },
        "the quiet frames did not count"
    );
}

#[test]
fn atlas_full_and_a_reload_force_the_next_frame_full() {
    let mut policy = TrimPolicy::default();
    assert_eq!(policy.decide(epoch(1), 2, true), FrameKind::Partial);
    policy.atlas_full();
    assert_eq!(policy.atlas_full_total(), 1);
    assert_eq!(
        policy.decide(epoch(1), 2, false),
        FrameKind::Full { forced: true },
        "forced even with nothing changed: the failed batch must prepare"
    );
    assert_eq!(policy.decide(epoch(1), 2, true), FrameKind::Partial);
    policy.force_full();
    assert_eq!(
        policy.decide(epoch(1), 2, true),
        FrameKind::Full { forced: true }
    );
    assert_eq!(
        policy.decide(epoch(1), 2, true),
        FrameKind::Partial,
        "the force is spent"
    );
    assert_eq!(policy.full_frames_total(), 2);
}

#[test]
fn a_moved_font_epoch_is_a_full_frame_but_the_first_epoch_is_not() {
    let mut policy = TrimPolicy::default();
    assert_eq!(policy.decide(epoch(3), 2, true), FrameKind::Partial);
    assert_eq!(
        policy.decide(epoch(4), 2, false),
        FrameKind::Full { forced: false }
    );
    assert_eq!(policy.decide(epoch(4), 2, true), FrameKind::Partial);
}

#[test]
fn with_no_batches_every_changed_frame_is_full() {
    let mut policy = TrimPolicy::default();
    for _ in 0..5 {
        assert_eq!(
            policy.decide(epoch(1), 0, true),
            FrameKind::Full { forced: false }
        );
        assert_eq!(policy.decide(epoch(1), 0, false), FrameKind::Nothing);
    }
    assert_eq!(policy.full_frames_total(), 5);
}

#[test]
fn labels_count_as_a_batch() {
    let labels = [WorldLabelRun {
        content: "12".to_string(),
        ..WorldLabelRun::default()
    }];
    assert_eq!(trim_batches(0, &[]), 0);
    assert_eq!(trim_batches(0, &labels), 1);
    assert_eq!(trim_batches(2, &labels), 3);

    // A frame of labels and no UI batch defers its trim to the Kth changed
    // frame, as a frame of UI does; without labels every changed frame is
    // full.
    let mut policy = TrimPolicy::default();
    let kinds: Vec<FrameKind> = (0..FULL_FRAME_INTERVAL)
        .map(|_| policy.decide(epoch(1), trim_batches(0, &labels), true))
        .collect();
    let last = FULL_FRAME_INTERVAL as usize - 1;
    assert!(kinds[..last].iter().all(|kind| *kind == FrameKind::Partial));
    assert_eq!(kinds[last], FrameKind::Full { forced: false });
    assert_eq!(
        policy.decide(epoch(1), trim_batches(0, &[]), true),
        FrameKind::Full { forced: false }
    );
}

// ---- signatures and the book ----

#[test]
fn a_changed_label_re_prepares_its_batch_alone() {
    let mut engine = engine();
    let left = committed_node(&mut engine, "left");
    let right = committed_node(&mut engine, "right");
    let list = PaintList {
        scale: 1.0,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![
            text(left, rect(0.0, 0.0, 100.0, 20.0), 0),
            panel(rect(50.0, 0.0, 100.0, 100.0), 0),
            text(right, rect(60.0, 10.0, 80.0, 20.0), 0),
        ],
    };
    let mut book = BatchBook::default();
    let first = signatures(&list, &engine);
    assert_eq!(first.len(), 2);
    assert_eq!(book.changed(&first), [true, true], "new batches prepare");
    book.mark_prepared(0, first[0].clone());
    book.mark_prepared(1, first[1].clone());
    assert_eq!(book.changed(&first), [false, false]);
    assert!(book.drawable(0) && book.drawable(1));

    engine.set_text(right, &StyledText::from("right!"), &style());
    engine.commit_layout(right, Vec2::new(200.0, 20.0));
    let second = signatures(&list, &engine);
    assert_eq!(second[0], first[0]);
    assert_ne!(second[1], first[1]);
    assert_eq!(book.changed(&second), [false, true]);

    // Equal text keeps the signature; a recommit does not.
    engine.set_text(right, &StyledText::from("right!"), &style());
    assert_eq!(signatures(&list, &engine)[1], second[1]);
    engine.commit_layout(right, Vec2::new(200.0, 20.0));
    assert_ne!(signatures(&list, &engine)[1], second[1]);
}

#[test]
fn a_clip_rectangle_that_changes_under_an_unchanged_command_re_prepares() {
    let mut engine = engine();
    let node = committed_node(&mut engine, "clipped");
    let mut list = PaintList {
        scale: 1.0,
        clips: vec![clip(0, 0, 800, 600), clip(0, 0, 50, 50)],
        commands: vec![text(node, rect(0.0, 0.0, 100.0, 20.0), 1)],
    };
    let before = signatures(&list, &engine);
    list.clips[1] = clip(0, 0, 60, 50);
    let after = signatures(&list, &engine);
    assert_ne!(
        before, after,
        "the signature carries the rectangle, not the index"
    );
    let mut book = BatchBook::default();
    book.changed(&before);
    book.mark_prepared(0, before[0].clone());
    assert_eq!(book.changed(&after), [true]);
}

#[test]
fn position_colour_scale_and_viewport_are_in_the_signature() {
    let mut engine = engine();
    let node = committed_node(&mut engine, "x");
    let base = PaintList {
        scale: 1.0,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![text(node, rect(0.0, 0.0, 100.0, 20.0), 0)],
    };
    let reference = batch_signature(&base, 0..1, &engine, 800, 600);
    let mut moved = base.clone();
    moved.commands[0] = text(node, rect(1.0, 0.0, 100.0, 20.0), 0);
    assert_ne!(batch_signature(&moved, 0..1, &engine, 800, 600), reference);
    let mut recoloured = base.clone();
    recoloured.commands[0] = PaintCommand::Text {
        node,
        rect: rect(0.0, 0.0, 100.0, 20.0),
        color: [1, 2, 3, 255],
        scale: 1.0,
        clip: 0,
    };
    assert_ne!(
        batch_signature(&recoloured, 0..1, &engine, 800, 600),
        reference
    );
    let mut scaled = base.clone();
    scaled.scale = 2.0;
    assert_ne!(batch_signature(&scaled, 0..1, &engine, 800, 600), reference);
    let mut text_scaled = base.clone();
    text_scaled.commands[0] = PaintCommand::Text {
        node,
        rect: rect(0.0, 0.0, 100.0, 20.0),
        color: [255, 255, 255, 255],
        scale: 0.5,
        clip: 0,
    };
    assert_ne!(
        batch_signature(&text_scaled, 0..1, &engine, 800, 600),
        reference,
        "the command's text scale is in the signature (M56)"
    );
    assert_ne!(batch_signature(&base, 0..1, &engine, 640, 480), reference);
    assert_eq!(batch_signature(&base, 0..1, &engine, 800, 600), reference);
}

#[test]
fn a_batch_absent_for_a_frame_re_prepares_when_it_returns() {
    let mut engine = engine();
    let a = committed_node(&mut engine, "a");
    let b = committed_node(&mut engine, "b");
    let two = PaintList {
        scale: 1.0,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![
            text(a, rect(0.0, 0.0, 100.0, 20.0), 0),
            panel(rect(0.0, 0.0, 300.0, 300.0), 0),
            text(b, rect(10.0, 10.0, 100.0, 20.0), 0),
        ],
    };
    let one = PaintList {
        scale: 1.0,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![text(a, rect(0.0, 0.0, 100.0, 20.0), 0)],
    };
    let mut book = BatchBook::default();
    let both = signatures(&two, &engine);
    book.changed(&both);
    book.mark_prepared(0, both[0].clone());
    book.mark_prepared(1, both[1].clone());

    let only = signatures(&one, &engine);
    assert_eq!(book.changed(&only), [false]);
    assert!(book.drawable(0));
    assert!(!book.drawable(1), "the absent batch lost its signature");

    assert_eq!(
        book.changed(&both),
        [false, true],
        "the returning batch prepares again with the same signature"
    );
}

#[test]
fn a_failed_batch_is_not_drawn_and_prepares_again() {
    let mut engine = engine();
    let node = committed_node(&mut engine, "x");
    let list = PaintList {
        scale: 1.0,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![text(node, rect(0.0, 0.0, 100.0, 20.0), 0)],
    };
    let mut book = BatchBook::default();
    let signature = signatures(&list, &engine);
    book.changed(&signature);
    book.mark_failed(0);
    assert!(!book.drawable(0));
    assert_eq!(book.changed(&signature), [true]);
    book.mark_prepared(0, signature[0].clone());
    assert!(book.drawable(0));
    assert!(!book.drawable(7), "an index never prepared");
}

#[test]
fn text_bounds_are_the_box_intersected_with_the_clip() {
    let bounds = text_bounds(rect(10.5, 20.25, 100.0, 20.0), clip(0, 0, 800, 600));
    assert_eq!(
        (bounds.left, bounds.top, bounds.right, bounds.bottom),
        (10, 20, 111, 41)
    );
    let clipped = text_bounds(rect(10.0, 20.0, 100.0, 20.0), clip(0, 0, 50, 30));
    assert_eq!(
        (clipped.left, clipped.top, clipped.right, clipped.bottom),
        (10, 20, 50, 30)
    );
    let outside = text_bounds(rect(100.0, 100.0, 10.0, 10.0), clip(0, 0, 50, 50));
    assert_eq!(
        outside.right, outside.left,
        "an empty intersection collapses, never inverts"
    );
    assert_eq!(outside.bottom, outside.top);
}

#[test]
fn only_committed_nodes_under_the_current_epoch_give_areas() {
    let mut engine = engine();
    let committed = committed_node(&mut engine, "committed");
    let bare = engine.create_node();
    engine.set_text(bare, &StyledText::from("set, not committed"), &style());
    let empty = engine.create_node();
    assert!(engine.node_area(committed).is_some());
    assert!(engine.node_area(bare).is_none());
    assert!(engine.node_area(empty).is_none());
    assert_eq!(engine.node_revision(committed), Some(1), "one commit");
    assert_eq!(engine.node_revision(bare), Some(0));
    assert_eq!(engine.node_revision(empty), None);

    let list = PaintList {
        scale: 2.0,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![
            text(committed, rect(10.0, 10.0, 100.0, 20.0), 0),
            text(bare, rect(10.0, 40.0, 100.0, 20.0), 0),
        ],
    };
    let (_, nodes) = engine.batch_parts();
    let mut areas = Vec::new();
    batch_areas(&list, 0..2, &nodes, &mut areas);
    assert_eq!(areas.len(), 1, "the uncommitted node draws nothing");
    assert_eq!((areas[0].left, areas[0].top), (10.0, 10.0));
    assert_eq!(
        areas[0].scale, 2.0,
        "rasterized at physical size, laid out logically"
    );
    assert_eq!((areas[0].bounds.left, areas[0].bounds.right), (10, 110));

    engine.reload_font("sans", font_bytes("Inter/static/Inter-Regular.ttf"));
    assert!(
        engine.node_area(committed).is_none(),
        "stale under the moved epoch"
    );
    engine.commit_layout(committed, Vec2::new(200.0, 20.0));
    assert!(engine.node_area(committed).is_some());
    assert_eq!(engine.node_revision(committed), Some(2));
}

#[test]
fn a_commands_text_scale_reaches_its_area_on_top_of_the_paint_scale() {
    let mut engine = engine();
    let node = committed_node(&mut engine, "x");
    let list = PaintList {
        scale: 1.5,
        clips: vec![clip(0, 0, 800, 600)],
        commands: vec![
            text(node, rect(0.0, 0.0, 100.0, 20.0), 0),
            PaintCommand::Text {
                node,
                rect: rect(0.0, 30.0, 50.0, 10.0),
                color: [255, 255, 255, 255],
                scale: 0.5,
                clip: 0,
            },
        ],
    };
    let (_, nodes) = engine.batch_parts();
    let mut areas = Vec::new();
    batch_areas(&list, 0..2, &nodes, &mut areas);
    let scales: Vec<f32> = areas.iter().map(|area| area.scale).collect();
    assert_eq!(scales, [1.5, 0.75]);
    assert_eq!((areas[1].left, areas[1].top), (0.0, 30.0));
}

#[test]
fn the_prewarms_areas_are_the_lists_runs_at_their_boxes_and_scales() {
    let mut engine = engine();
    let a = committed_node(&mut engine, "ab");
    let b = committed_node(&mut engine, "cd");
    let list = PaintList {
        scale: 2.0,
        clips: vec![clip(0, 0, 1600, 1200)],
        commands: vec![
            text(a, rect(10.0, 20.0, 100.0, 20.0), 0),
            PaintCommand::Text {
                node: b,
                rect: rect(200.0, 300.0, 50.0, 10.0),
                color: [255, 255, 255, 255],
                scale: 1.5,
                clip: 0,
            },
        ],
    };
    let (_, nodes) = engine.batch_parts();
    let mut areas = Vec::new();
    prewarm_areas(&list, &nodes, &mut areas);
    let placed: Vec<(f32, f32, f32)> = areas
        .iter()
        .map(|area| (area.left, area.top, area.scale))
        .collect();
    assert_eq!(placed, [(10.0, 20.0, 2.0), (200.0, 300.0, 3.0)]);
    assert_eq!(area_glyphs(&areas), 4);
}
