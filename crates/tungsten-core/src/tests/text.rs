use super::*;

#[test]
fn default_layout_is_the_layout_sections_always_had() {
    let layout = TextLayout::default();
    assert_eq!(layout.align, TextAlign::Start);
    assert_eq!(layout.wrap, TextWrap::WordOrGlyph);
    assert_eq!(layout.overflow, TextOverflow::Clip);
    assert_eq!(layout.hinting, TextHinting::Disabled);
    assert_eq!(layout.letter_spacing, 0.0);
    assert!(layout.features.is_empty());
}

#[test]
fn tabular_figures_sets_tnum() {
    let features = FontFeatures::tabular_figures();
    assert_eq!(
        features.as_slice(),
        &[FontFeature {
            tag: *b"tnum",
            value: 1
        }]
    );
}

#[test]
fn setting_a_feature_again_replaces_it() {
    let features = FontFeatures::tabular_figures()
        .with(*b"liga", 0)
        .with(*b"tnum", 0);
    assert_eq!(
        features.as_slice(),
        &[
            FontFeature {
                tag: *b"tnum",
                value: 0
            },
            FontFeature {
                tag: *b"liga",
                value: 0
            },
        ]
    );
}

#[test]
fn plain_text_is_one_span_without_overrides() {
    let text = StyledText::from("a");
    assert_eq!(text.spans, vec![TextSpan::new("a")]);
    let span = &text.spans[0];
    assert_eq!(span.text, "a");
    assert_eq!(span.weight, None);
    assert_eq!(span.italic, None);
    assert_eq!(span.color, None);
}

#[test]
fn node_ids_differ_by_generation() {
    let first = TextNodeId::new(3, 0);
    let reused = TextNodeId::new(3, 1);
    assert_ne!(first, reused);
    assert_eq!(first.index(), reused.index());
    assert_eq!(reused.generation(), 1);
}

#[test]
fn font_epoch_counts_up() {
    let epoch = FontEpoch::default();
    assert_eq!(epoch.get(), 0);
    assert!(epoch.next() > epoch);
    assert_eq!(epoch.next(), FontEpoch::new(1));
}

#[test]
fn a_node_store_measures_through_its_supertrait() {
    let mut store = crate::ui::FixedAdvanceMeasure::new(0.5);
    let store: &mut dyn TextNodeStore = &mut store;
    let node = store.create_node();
    store.set_text(
        node,
        &StyledText::from("abcd"),
        &TextStyle::new("sans", 10.0, 12.0),
    );
    let measure: &mut dyn TextMeasure = store;
    let metrics = measure.measure(node, None, MeasureWidth::MaxContent);
    assert_eq!(metrics.size, Vec2::new(20.0, 12.0));
    assert_eq!(metrics.line_count, 1);
}
