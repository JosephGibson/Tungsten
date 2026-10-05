use super::*;
use crate::text::TextLayout;

/// Size 10 at advance 0.5: 5 px per glyph, 12 px lines.
const ADVANCE: f32 = 5.0;
const LINE_HEIGHT: f32 = 12.0;

fn style(wrap: TextWrap) -> TextStyle {
    TextStyle::new("sans", 10.0, LINE_HEIGHT).with_layout(TextLayout::default().with_wrap(wrap))
}

fn node_with(store: &mut FixedAdvanceMeasure, text: &str, style: &TextStyle) -> TextNodeId {
    let node = store.create_node();
    store.set_text(node, &StyledText::from(text), style);
    node
}

fn width(store: &mut FixedAdvanceMeasure, node: TextNodeId, available: MeasureWidth) -> f32 {
    store.measure(node, None, available).size.x
}

#[test]
fn min_content_follows_the_wrap_mode() {
    let mut store = FixedAdvanceMeasure::default();
    let text = "ab cde f";
    let none = node_with(&mut store, text, &style(TextWrap::None));
    let glyph = node_with(&mut store, text, &style(TextWrap::Glyph));
    let word = node_with(&mut store, text, &style(TextWrap::Word));
    let either = node_with(&mut store, text, &style(TextWrap::WordOrGlyph));
    assert_eq!(
        width(&mut store, none, MeasureWidth::MinContent),
        8.0 * ADVANCE
    );
    assert_eq!(width(&mut store, glyph, MeasureWidth::MinContent), ADVANCE);
    assert_eq!(
        width(&mut store, word, MeasureWidth::MinContent),
        3.0 * ADVANCE
    );
    assert_eq!(
        width(&mut store, either, MeasureWidth::MinContent),
        3.0 * ADVANCE
    );
    let glyph_lines = store.measure(glyph, None, MeasureWidth::MinContent);
    assert_eq!(glyph_lines.line_count, 8);
    assert_eq!(glyph_lines.size.y, 8.0 * LINE_HEIGHT);
}

#[test]
fn max_content_is_the_widest_unwrapped_line() {
    let mut store = FixedAdvanceMeasure::default();
    for wrap in [
        TextWrap::None,
        TextWrap::Glyph,
        TextWrap::Word,
        TextWrap::WordOrGlyph,
    ] {
        let node = node_with(&mut store, "ab cde f\nxyz", &style(wrap));
        let metrics = store.measure(node, None, MeasureWidth::MaxContent);
        assert_eq!(
            metrics.size,
            Vec2::new(8.0 * ADVANCE, 2.0 * LINE_HEIGHT),
            "{wrap:?}"
        );
        assert_eq!(metrics.line_count, 2, "{wrap:?}");
        assert_eq!(metrics.first_baseline, 0.8 * LINE_HEIGHT, "{wrap:?}");
    }
}

#[test]
fn a_definite_width_wraps_per_mode_and_a_wide_word_differs() {
    let mut store = FixedAdvanceMeasure::default();
    let box_width = 4.0 * ADVANCE;
    let word = node_with(&mut store, "abcdef gh", &style(TextWrap::Word));
    let either = node_with(&mut store, "abcdef gh", &style(TextWrap::WordOrGlyph));
    let glyph = node_with(&mut store, "abcdef gh", &style(TextWrap::Glyph));
    let none = node_with(&mut store, "abcdef gh", &style(TextWrap::None));

    // `Word`: the six-glyph word overflows on its own line.
    let metrics = store.measure(word, None, MeasureWidth::Definite(box_width));
    assert_eq!(metrics.size, Vec2::new(6.0 * ADVANCE, 2.0 * LINE_HEIGHT));
    assert_eq!(metrics.line_count, 2);
    // `WordOrGlyph`: it breaks inside: "abcd", "ef", "gh".
    let metrics = store.measure(either, None, MeasureWidth::Definite(box_width));
    assert_eq!(metrics.size, Vec2::new(4.0 * ADVANCE, 3.0 * LINE_HEIGHT));
    assert_eq!(metrics.line_count, 3);
    // `Glyph`: "abcd", "ef g", "h".
    let metrics = store.measure(glyph, None, MeasureWidth::Definite(box_width));
    assert_eq!(metrics.size, Vec2::new(4.0 * ADVANCE, 3.0 * LINE_HEIGHT));
    assert_eq!(metrics.line_count, 3);
    // `None`: one line past the box.
    let metrics = store.measure(none, None, MeasureWidth::Definite(box_width));
    assert_eq!(metrics.size, Vec2::new(9.0 * ADVANCE, LINE_HEIGHT));
    assert_eq!(metrics.line_count, 1);
}

#[test]
fn a_word_that_fits_exactly_stays_on_one_line() {
    let mut store = FixedAdvanceMeasure::default();
    let node = node_with(&mut store, "ab", &style(TextWrap::WordOrGlyph));
    assert_eq!(
        store
            .measure(node, None, MeasureWidth::Definite(2.0 * ADVANCE))
            .line_count,
        1
    );
    assert_eq!(
        store
            .measure(node, None, MeasureWidth::Definite(2.0 * ADVANCE - 0.1))
            .line_count,
        2
    );
}

#[test]
fn known_width_wins_over_available() {
    let mut store = FixedAdvanceMeasure::default();
    let node = node_with(&mut store, "ab cd", &style(TextWrap::WordOrGlyph));
    let metrics = store.measure(node, Some(2.0 * ADVANCE), MeasureWidth::MaxContent);
    assert_eq!(metrics.line_count, 2);
    assert_eq!(metrics.size.x, 2.0 * ADVANCE);
}

#[test]
fn letter_spacing_widens_every_glyph() {
    let mut store = FixedAdvanceMeasure::default();
    let spaced = style(TextWrap::None).with_layout(TextLayout::default().with_letter_spacing(0.1));
    let node = node_with(&mut store, "abcd", &spaced);
    assert_eq!(
        width(&mut store, node, MeasureWidth::MaxContent),
        4.0 * 10.0 * 0.6
    );
}

#[test]
fn spans_concatenate() {
    let mut store = FixedAdvanceMeasure::default();
    let node = store.create_node();
    let text = StyledText {
        spans: vec![
            crate::text::TextSpan::new("ab").with_weight(700),
            crate::text::TextSpan::new("cd"),
        ],
    };
    store.set_text(node, &text, &style(TextWrap::None));
    assert_eq!(
        width(&mut store, node, MeasureWidth::MaxContent),
        4.0 * ADVANCE
    );
}

#[test]
fn empty_text_measures_one_line_height() {
    let mut store = FixedAdvanceMeasure::default();
    let node = node_with(&mut store, "", &style(TextWrap::WordOrGlyph));
    for available in [
        MeasureWidth::MinContent,
        MeasureWidth::MaxContent,
        MeasureWidth::Definite(100.0),
    ] {
        let metrics = store.measure(node, None, available);
        assert_eq!(metrics.size, Vec2::new(0.0, LINE_HEIGHT), "{available:?}");
        assert_eq!(metrics.line_count, 1, "{available:?}");
    }
    assert_eq!(
        store
            .commit_layout(node, Vec2::new(100.0, LINE_HEIGHT))
            .line_count,
        1
    );
}

#[test]
fn a_node_without_text_measures_default() {
    let mut store = FixedAdvanceMeasure::default();
    let node = store.create_node();
    assert_eq!(
        store.measure(node, None, MeasureWidth::MaxContent),
        TextMetrics::default()
    );
    assert_eq!(
        store.commit_layout(node, Vec2::new(10.0, 10.0)),
        TextMetrics::default()
    );
    assert_eq!(store.committed(node), None);
}

#[test]
fn a_removed_node_measures_default_and_its_slot_gets_a_new_generation() {
    let mut store = FixedAdvanceMeasure::default();
    let first = node_with(&mut store, "abc", &style(TextWrap::None));
    assert_eq!(store.live_nodes(), 1);
    store.remove_node(first);
    assert_eq!(store.live_nodes(), 0);
    assert_eq!(
        store.measure(first, None, MeasureWidth::MaxContent),
        TextMetrics::default()
    );
    assert_eq!(store.committed(first), None);
    store.set_text(first, &StyledText::from("stale"), &style(TextWrap::None));
    store.remove_node(first);
    assert_eq!(store.live_nodes(), 0);

    let reused = store.create_node();
    assert_eq!(reused.index(), first.index());
    assert_ne!(reused, first);
    assert_eq!(reused.generation(), first.generation() + 1);
    assert_eq!(
        store.measure(reused, None, MeasureWidth::MaxContent),
        TextMetrics::default()
    );
    assert_eq!(store.live_nodes(), 1);
}

#[test]
fn a_commit_holds_until_the_text_style_or_epoch_changes() {
    let mut store = FixedAdvanceMeasure::default();
    let node = node_with(&mut store, "ab cd", &style(TextWrap::WordOrGlyph));
    let metrics = store.commit_layout(node, Vec2::new(2.0 * ADVANCE, 2.0 * LINE_HEIGHT));
    assert_eq!(metrics.line_count, 2);
    assert_eq!(store.committed(node), Some(metrics));

    // Equal text keeps the commit; other text drops it.
    store.set_text(
        node,
        &StyledText::from("ab cd"),
        &style(TextWrap::WordOrGlyph),
    );
    assert_eq!(store.committed(node), Some(metrics));
    store.set_text(
        node,
        &StyledText::from("ab ce"),
        &style(TextWrap::WordOrGlyph),
    );
    assert_eq!(store.committed(node), None);

    // A style change drops it too, a colour change included.
    let metrics = store.commit_layout(node, Vec2::new(2.0 * ADVANCE, 2.0 * LINE_HEIGHT));
    store.set_text(
        node,
        &StyledText::from("ab ce"),
        &style(TextWrap::WordOrGlyph).with_color([1, 2, 3, 4]),
    );
    assert_eq!(store.committed(node), None);
    assert_ne!(metrics, TextMetrics::default());

    // A font epoch bump stales the commit without any text change.
    let metrics = store.commit_layout(node, Vec2::new(2.0 * ADVANCE, 2.0 * LINE_HEIGHT));
    assert_eq!(store.committed(node), Some(metrics));
    let before = store.font_epoch();
    store.bump_font_epoch();
    assert_eq!(store.font_epoch(), before.next());
    assert_eq!(store.committed(node), None);
    assert_eq!(
        store.commit_layout(node, Vec2::new(2.0 * ADVANCE, 2.0 * LINE_HEIGHT)),
        metrics
    );
    assert_eq!(store.committed(node), Some(metrics));
}

#[test]
fn counts_rise_per_call() {
    let mut store = FixedAdvanceMeasure::default();
    assert_eq!(store.counts, MeasureCounts::default());
    let node = store.create_node();
    store.set_text(node, &StyledText::from("ab"), &style(TextWrap::None));
    store.set_text(node, &StyledText::from("ab"), &style(TextWrap::None));
    store.measure(node, None, MeasureWidth::MaxContent);
    store.measure(node, None, MeasureWidth::MinContent);
    store.measure(node, None, MeasureWidth::Definite(10.0));
    store.commit_layout(node, Vec2::new(10.0, LINE_HEIGHT));
    store.remove_node(node);
    store.measure(node, None, MeasureWidth::MaxContent);
    store.set_text(node, &StyledText::from("ab"), &style(TextWrap::None));
    store.commit_layout(node, Vec2::new(10.0, LINE_HEIGHT));
    assert_eq!(
        store.counts,
        MeasureCounts {
            measure_calls: 4,
            set_text_calls: 3,
            commits: 2,
        }
    );
}

#[test]
fn the_double_is_a_measure_through_the_store_trait() {
    let mut store = FixedAdvanceMeasure::new(1.0);
    let node = node_with(&mut store, "abc", &style(TextWrap::None));
    let dyn_store: &mut dyn TextNodeStore = &mut store;
    let measure: &mut dyn TextMeasure = dyn_store;
    assert_eq!(
        measure.measure(node, None, MeasureWidth::MaxContent).size.x,
        30.0
    );
    assert_eq!(store.advance_em(), 1.0);
}
