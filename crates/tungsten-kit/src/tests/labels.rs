use super::*;
use serde_json::json;

/// A World with a clock, the live labels and the frame's labels.
fn world() -> World {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(LiveLabels::default());
    world.insert_resource(WorldLabels::default());
    world
}

/// One frame of `dt`: the tick, then the frame's labels drained.
fn frame(world: &mut World, dt: f32) -> Vec<WorldLabel> {
    world.resource_mut::<Time>().advance_frame(dt);
    label_tick(world);
    world.resource_mut::<WorldLabels>().drain().collect()
}

#[test]
fn an_amount_shows_whole_or_to_one_decimal() {
    assert_eq!(amount_text(3.0), "3");
    assert_eq!(amount_text(1.5), "1.5");
    assert_eq!(amount_text(12.34), "12.3");
    assert_eq!(amount_text(0.0), "0");
    assert_eq!(amount_text(-2.0), "-2");
}

#[test]
fn a_linear_channel_encodes_to_srgb() {
    assert_eq!(srgb_byte(0.0), 0);
    assert_eq!(srgb_byte(1.0), 255);
    assert_eq!(srgb_byte(0.5), 188);
    assert_eq!(srgb_byte(0.2), 124);
    assert_eq!((srgb_byte(-1.0), srgb_byte(2.0)), (0, 255), "clamped");
}

#[test]
fn a_label_pops_over_its_first_fifth_rises_and_fades_over_its_second_half() {
    let style = LabelStyle::new("sans").with_rise(40.0).with_pop(2.0);
    let at = |t| shape_at(&style, t);
    assert_eq!(at(0.0), (Vec2::ZERO, 2.0, 1.0));
    let (offset, scale, alpha) = at(0.1);
    assert!((scale - 1.5).abs() < 1e-6 && alpha == 1.0 && (offset.y + 4.0).abs() < 1e-5);
    assert_eq!(at(0.2).1, 1.0);
    assert_eq!(at(0.5).2, 1.0);
    assert!((at(0.75).2 - 0.5).abs() < 1e-6);
    assert_eq!(at(1.0), (Vec2::new(0.0, -40.0), 1.0, 0.0));
}

#[test]
fn a_label_draws_each_frame_ages_on_the_game_clock_and_holds_while_paused() {
    let mut world = world();
    let style = LabelStyle::new("sans")
        .with_seconds(0.5)
        .with_color([1.0, 0.5, 0.0, 1.0])
        .with_size(20.0);
    float_label(&mut world, &style, Vec2::new(10.0, 20.0), 3.0);
    let first = frame(&mut world, 0.1);
    assert_eq!(first.len(), 1);
    let label = &first[0];
    assert_eq!(
        (label.text.as_str(), label.font_id.as_str(), label.font_size),
        ("3", "sans", 20.0)
    );
    assert_eq!(
        (label.anchor, label.offset, label.scale),
        (Vec2::new(10.0, 20.0), Vec2::ZERO, 1.5)
    );
    assert_eq!(label.color, [255, 188, 0, 255]);

    let second = frame(&mut world, 0.1);
    assert!(
        (second[0].offset.y + 6.4).abs() < 1e-4,
        "a fifth of the way up"
    );

    world.resource_mut::<Time>().pause();
    for _ in 0..10 {
        let held = frame(&mut world, 0.1);
        assert!((held[0].offset.y + 6.4).abs() < 1e-4, "paused, still drawn");
    }
    world.resource_mut::<Time>().resume();
    let third = frame(&mut world, 0.2);
    assert!(third[0].color[3] < 255, "fading past half its seconds");
    assert!(frame(&mut world, 0.2).is_empty(), "gone after its seconds");
    assert_eq!(world.resource::<LiveLabels>().len(), 0);
}

#[test]
fn past_the_cap_the_oldest_label_goes() {
    let mut world = world();
    let style = LabelStyle::new("sans");
    for i in 0..=MAX_LABELS {
        float_label(&mut world, &style, Vec2::ZERO, i as f32);
    }
    let drawn = frame(&mut world, 0.0);
    assert_eq!(drawn.len(), MAX_LABELS);
    assert_eq!(drawn[0].text, "1", "label 0 dropped");
}

#[test]
fn without_frame_labels_they_still_age() {
    let mut world = world();
    world.remove_resource::<WorldLabels>();
    float_label(
        &mut world,
        &LabelStyle::new("sans").with_seconds(0.1),
        Vec2::ZERO,
        1.0,
    );
    world.resource_mut::<Time>().advance_frame(0.0);
    label_tick(&mut world);
    world.resource_mut::<Time>().advance_frame(0.2);
    label_tick(&mut world);
    assert_eq!(world.resource::<LiveLabels>().len(), 0);
}

#[test]
fn an_empty_label_takes_no_style_and_a_partial_one_needs_a_font() {
    let empty: LabelDef = serde_json::from_value(json!({})).unwrap();
    assert!(empty.0.is_none());
    assert_eq!(serde_json::to_value(LabelDef(None)).unwrap(), json!({}));
    let named: LabelDef = serde_json::from_value(json!({ "font": "sans", "size": 20 })).unwrap();
    assert_eq!(named.0, Some(LabelStyle::new("sans").with_size(20.0)));
    let written = serde_json::to_value(named).unwrap();
    assert_eq!(written["font"], "sans");
    assert_eq!(written["rise"], 32.0, "every field, as a style writes it");
    let partial = serde_json::from_value::<LabelDef>(json!({ "size": 20 }))
        .err()
        .unwrap()
        .to_string();
    assert!(partial.contains("names its font"), "{partial}");
    for bad in [
        json!({ "font": "" }),
        json!({ "font": "sans", "size": 0 }),
        json!({ "color": [1, 1, 1, 1] }),
        json!({ "font": "sans", "glow": 1 }),
        json!({ "font": "sans", "rise": "up" }),
    ] {
        assert!(
            serde_json::from_value::<LabelDef>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}
