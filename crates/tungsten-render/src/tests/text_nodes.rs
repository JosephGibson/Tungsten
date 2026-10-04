use std::collections::HashMap;
use std::path::Path;

use glyphon::fontdb;
use tungsten_core::assets::ResolvedFontFamily;
use tungsten_core::text::{EllipsisAt, TextLayout, TextOverflow, TextSpan};

use super::super::{FontSource, TextSection, text_areas};
use super::*;

const LINE_HEIGHT: f32 = 18.0;
const LONG_LINE: &str = "a long line of words that runs far wider than its box";

fn font_bytes(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fonts")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The shared manifest's faces, families and chain, with packaged fonts only.
fn engine() -> TextEngine {
    let mut engine = TextEngine::new(FontSource::Packaged);
    for (id, path) in [
        ("mono", "JetBrainsMono/static/JetBrainsMono-Regular.ttf"),
        ("sans", "Inter/static/Inter-Regular.ttf"),
        ("sans_bold", "Inter/static/Inter-Bold.ttf"),
    ] {
        engine.load_font(id, font_bytes(path));
    }
    let families: HashMap<String, ResolvedFontFamily> = [
        ("sans", vec!["sans".to_string(), "sans_bold".to_string()]),
        ("mono", vec!["mono".to_string()]),
    ]
    .into_iter()
    .map(|(id, faces)| (id.to_string(), ResolvedFontFamily::new(faces)))
    .collect();
    engine.set_font_families(&families, &["sans".to_string(), "mono".to_string()]);
    engine
}

fn mono() -> TextStyle {
    TextStyle::new("mono", 14.0, LINE_HEIGHT)
}

fn node_with(engine: &mut TextEngine, text: &str, style: &TextStyle) -> TextNodeId {
    let node = engine.create_node();
    engine.set_text(node, &StyledText::from(text), style);
    node
}

fn max_width(engine: &mut TextEngine, text: &str, style: &TextStyle) -> f32 {
    let node = node_with(engine, text, style);
    engine.measure(node, None, MeasureWidth::MaxContent).size.x
}

/// Glyph, face, x, y and advance of every glyph in a node's buffer.
fn node_glyphs(engine: &TextEngine, node: TextNodeId) -> Vec<(u16, fontdb::ID, f32, f32, f32)> {
    let current = engine.nodes.slot(node).unwrap().node.as_ref().unwrap();
    current
        .buffer
        .layout_runs()
        .flat_map(|run| {
            run.glyphs
                .iter()
                .map(|g| (g.glyph_id, g.font_id, g.x, run.line_y, g.w))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn ellipsis_glyph(engine: &mut TextEngine) -> u16 {
    let node = node_with(engine, "\u{2026}", &mono());
    engine.commit_layout(node, Vec2::new(100.0, 100.0));
    node_glyphs(engine, node)[0].0
}

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "{what}: {actual} != {expected}"
    );
}

#[test]
fn min_content_follows_the_wrap_mode() {
    let mut engine = engine();
    let bbbb = max_width(&mut engine, "bbbb", &mono());
    let glyph = max_width(&mut engine, "b", &mono());
    let whole = max_width(&mut engine, "aa bbbb c", &mono());
    for (wrap, expected) in [
        (TextWrap::Word, bbbb),
        (TextWrap::WordOrGlyph, bbbb),
        (TextWrap::Glyph, glyph),
        (TextWrap::None, whole),
    ] {
        let style = mono().with_layout(TextLayout::default().with_wrap(wrap));
        let node = node_with(&mut engine, "aa bbbb c", &style);
        let min = engine.measure(node, None, MeasureWidth::MinContent);
        assert_close(min.size.x, expected, &format!("{wrap:?}"));
    }
}

#[test]
fn max_content_is_the_unwrapped_width_in_every_mode() {
    let mut engine = engine();
    let whole = max_width(&mut engine, "aa bbbb c", &mono());
    for wrap in [
        TextWrap::Word,
        TextWrap::WordOrGlyph,
        TextWrap::Glyph,
        TextWrap::None,
    ] {
        let style = mono().with_layout(TextLayout::default().with_wrap(wrap));
        let node = node_with(&mut engine, "aa bbbb c", &style);
        let max = engine.measure(node, None, MeasureWidth::MaxContent);
        assert_close(max.size.x, whole, &format!("{wrap:?}"));
        assert_eq!(max.line_count, 1, "{wrap:?}");
    }
}

#[test]
fn a_definite_width_wraps_and_counts_lines() {
    let mut engine = engine();
    let node = node_with(&mut engine, LONG_LINE, &mono());
    let max = engine.measure(node, None, MeasureWidth::MaxContent);
    let width = max.size.x / 3.0;
    let wrapped = engine.measure(node, None, MeasureWidth::Definite(width));
    assert!(wrapped.line_count >= 3, "{} lines", wrapped.line_count);
    assert!(wrapped.size.x <= width);
    assert_close(
        wrapped.size.y,
        LINE_HEIGHT * wrapped.line_count as f32,
        "height",
    );
}

#[test]
fn known_width_wins_over_available() {
    let mut engine = engine();
    let node = node_with(&mut engine, LONG_LINE, &mono());
    let known = engine.measure(node, Some(120.0), MeasureWidth::MaxContent);
    let definite = engine.measure(node, None, MeasureWidth::Definite(120.0));
    assert_eq!(known, definite);
    assert!(known.line_count > 1);
}

#[test]
fn empty_text_measures_one_line_height() {
    let mut engine = engine();
    let node = node_with(&mut engine, "", &mono());
    for width in [
        MeasureWidth::MinContent,
        MeasureWidth::MaxContent,
        MeasureWidth::Definite(50.0),
    ] {
        let metrics = engine.measure(node, None, width);
        assert_eq!(metrics.size.x, 0.0, "{width:?}");
        assert_close(metrics.size.y, LINE_HEIGHT, &format!("{width:?}"));
        assert_eq!(metrics.line_count, 1, "{width:?}");
    }
}

#[test]
fn repeated_measures_and_equal_text_do_no_work() {
    let mut engine = engine();
    let node = node_with(&mut engine, LONG_LINE, &mono());
    let first = engine.measure(node, None, MeasureWidth::Definite(100.0));
    let (shapes, layouts) = (engine.nodes.shapes, engine.nodes.layouts);
    assert_eq!(
        engine.measure(node, None, MeasureWidth::Definite(100.0)),
        first
    );
    assert_eq!(engine.nodes.layouts, layouts, "same width");

    engine.set_text(node, &StyledText::from(LONG_LINE), &mono());
    assert_eq!(
        engine.measure(node, None, MeasureWidth::Definite(100.0)),
        first
    );
    assert_eq!(engine.nodes.shapes, shapes, "equal set_text");
    assert_eq!(engine.nodes.layouts, layouts, "equal set_text");

    // Min- and max-content are cached too, and probing never reshapes.
    engine.measure(node, None, MeasureWidth::MinContent);
    engine.measure(node, None, MeasureWidth::MaxContent);
    let layouts = engine.nodes.layouts;
    engine.measure(node, None, MeasureWidth::MinContent);
    engine.measure(node, None, MeasureWidth::MaxContent);
    assert_eq!(engine.nodes.layouts, layouts, "cached probes");
    assert_eq!(engine.nodes.shapes, shapes, "probes");
}

#[test]
fn font_reload_reshapes_at_the_next_measure() {
    let mut engine = engine();
    let node = node_with(&mut engine, LONG_LINE, &mono());
    engine.measure(node, None, MeasureWidth::MaxContent);
    let shapes = engine.nodes.shapes;
    let before = engine.font_epoch();

    engine.reload_font(
        "mono",
        font_bytes("JetBrainsMono/static/JetBrainsMono-Regular.ttf"),
    );
    let after = engine.font_epoch();
    assert!(after > before);
    let metrics = engine.measure(node, None, MeasureWidth::MaxContent);
    assert_eq!(engine.nodes.shapes, shapes + 1);
    assert!(metrics.size.x > 0.0);
    let current = engine.nodes.slot(node).unwrap().node.as_ref().unwrap();
    assert_eq!(current.shaped, Some(after));
}

#[test]
fn commit_after_probes_leaves_the_committed_width() {
    let mut engine = engine();
    let node = node_with(&mut engine, LONG_LINE, &mono());
    engine.measure(node, None, MeasureWidth::MinContent);
    engine.measure(node, None, MeasureWidth::MaxContent);
    let committed = engine.commit_layout(node, Vec2::new(150.0, 200.0));
    let current = engine.nodes.slot(node).unwrap().node.as_ref().unwrap();
    assert_eq!(current.buffer.size(), (Some(150.0), Some(200.0)));
    assert_eq!(engine.committed(node), Some(committed));
    assert!(committed.size.x <= 150.0);

    // A cached probe leaves the buffer alone; a new width moves it off the
    // committed layout.
    engine.measure(node, None, MeasureWidth::MaxContent);
    assert_eq!(engine.committed(node), Some(committed));
    engine.measure(node, None, MeasureWidth::Definite(90.0));
    assert_eq!(engine.committed(node), None);
}

#[test]
fn a_removed_node_measures_zero_and_its_slot_gets_a_new_generation() {
    let mut engine = engine();
    let removed = node_with(&mut engine, "gone", &mono());
    assert!(
        engine
            .measure(removed, None, MeasureWidth::MaxContent)
            .size
            .x
            > 0.0
    );
    let spare = engine.spare.len();
    engine.remove_node(removed);
    assert_eq!(engine.spare.len(), spare + 1, "the buffer is spare");

    assert_eq!(
        engine.measure(removed, None, MeasureWidth::MaxContent),
        TextMetrics::default()
    );
    assert_eq!(
        engine.commit_layout(removed, Vec2::new(10.0, 10.0)),
        TextMetrics::default()
    );
    let reused = engine.create_node();
    assert_eq!(reused.index(), removed.index());
    assert_ne!(reused, removed);
    // A node without text measures zero too.
    assert_eq!(
        engine.measure(reused, None, MeasureWidth::MaxContent),
        TextMetrics::default()
    );
}

#[test]
fn a_node_lays_out_like_a_section() {
    let mut engine = engine();
    let text = "0042 f000150 deadbeef 0badc0de wraps here";
    let node = node_with(&mut engine, text, &mono());
    engine.commit_layout(node, Vec2::new(200.0, 400.0));
    let from_node = node_glyphs(&engine, node);

    let section = TextSection {
        content: text.to_string(),
        font_id: "mono".to_string(),
        font_size: 14.0,
        line_height: LINE_HEIGHT,
        bounds: Some([200.0, 400.0]),
        ..Default::default()
    };
    assert!(engine.update(&[section], 1920, 1080));
    let from_section: Vec<_> = text_areas(&engine.prepared, &engine.buffers)
        .flat_map(|area| {
            area.buffer
                .layout_runs()
                .flat_map(|run| {
                    run.glyphs
                        .iter()
                        .map(|g| (g.glyph_id, g.font_id, g.x, run.line_y, g.w))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(from_node.len() > 10);
    assert_eq!(from_node, from_section);
}

#[test]
fn a_bold_span_shapes_from_the_bold_face() {
    let mut engine = engine();
    let regular = engine.font_attrs["sans"].face_ids[0];
    let bold = engine.font_attrs["sans_bold"].face_ids[0];
    let node = engine.create_node();
    let text = StyledText {
        spans: vec![
            TextSpan::new("light "),
            TextSpan::new("bold").with_weight(700),
        ],
    };
    engine.set_text(node, &text, &TextStyle::new("sans", 16.0, 20.0));
    engine.commit_layout(node, Vec2::new(400.0, 40.0));
    let faces: Vec<fontdb::ID> = node_glyphs(&engine, node)
        .into_iter()
        .map(|g| g.1)
        .collect();
    assert_eq!(faces.len(), "light bold".len());
    assert!(faces[..6].iter().all(|&face| face == regular), "{faces:?}");
    assert!(faces[6..].iter().all(|&face| face == bold), "{faces:?}");
}

#[test]
fn an_ellipsis_without_a_line_limit_measures_every_line_and_commits_to_the_box() {
    let mut engine = engine();
    let ellipsis = ellipsis_glyph(&mut engine);
    let style = mono().with_layout(TextLayout::default().with_overflow(TextOverflow::Ellipsis {
        at: EllipsisAt::End,
        lines: None,
    }));
    let node = node_with(&mut engine, LONG_LINE, &style);
    let measured = engine.measure(node, None, MeasureWidth::Definite(120.0));
    assert!(measured.line_count > 1, "{} lines", measured.line_count);

    let committed = engine.commit_layout(node, Vec2::new(120.0, LINE_HEIGHT));
    assert_eq!(committed.line_count, 1);
    assert_eq!(
        node_glyphs(&engine, node).last().map(|g| g.0),
        Some(ellipsis)
    );
}
