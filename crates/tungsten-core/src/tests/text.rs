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

#[test]
fn world_label_builders_set_their_fields() {
    let label = WorldLabel::new("12", Vec2::new(3.0, 4.0), "sans", 14.0);
    assert_eq!(label.text, "12");
    assert_eq!(label.anchor, Vec2::new(3.0, 4.0));
    assert_eq!(label.offset, Vec2::ZERO);
    assert_eq!(label.pivot, Vec2::new(0.5, 1.0));
    assert_eq!(label.font_id, "sans");
    assert_eq!(label.font_size, 14.0);
    assert_eq!(label.line_height, 0.0);
    assert_eq!(label.color, [255, 255, 255, 255]);
    assert_eq!(label.scale, 1.0);

    let label = label
        .with_offset(Vec2::new(-1.0, -2.0))
        .with_pivot(Vec2::ZERO)
        .with_line_height(18.0)
        .with_color([255, 200, 0, 255])
        .with_scale(1.5);
    assert_eq!(label.offset, Vec2::new(-1.0, -2.0));
    assert_eq!(label.pivot, Vec2::ZERO);
    assert_eq!(label.line_height, 18.0);
    assert_eq!(label.color, [255, 200, 0, 255]);
    assert_eq!(label.scale, 1.5);

    let mut labels = WorldLabels::default();
    assert_eq!((labels.cap(), labels.cull_margin()), (512, 64.0));
    labels.set_cap(2);
    labels.set_cull_margin(-1.0);
    labels.set_cull_margin(f32::NAN);
    assert_eq!((labels.cap(), labels.cull_margin()), (2, 64.0));
    labels.push(label.clone());
    labels.push(WorldLabel::new("b", Vec2::ZERO, "mono", 11.0));
    assert_eq!(labels.len(), 2);
    let drained: Vec<_> = labels.drain().collect();
    assert_eq!(drained[0], label);
    assert_eq!(drained[1].text, "b");
    assert!(labels.is_empty());
}
