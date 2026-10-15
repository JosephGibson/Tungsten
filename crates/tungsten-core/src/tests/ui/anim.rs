use super::*;
use crate::text::TextNodeStore as _;
use crate::ui::{FixedAdvanceMeasure, LayoutStats, LayoutStyle, UiLayer};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
/// 3/8 s: exact in binary, and it does not divide the 1 s tracks, so the
/// timelines carry a remainder across cycles.
const DT: f32 = 0.375;
const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// A tree with a root and a button: (tree, button).
fn rig() -> (UiTree, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let button = tree.button(root, "go");
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    (tree, button)
}

/// Ticks `dt`, returning the finished events.
fn tick(tree: &mut UiTree, dt: f32) -> Vec<UiEvent> {
    let mut finished = Vec::new();
    tree.tick_animations(dt, &mut finished);
    finished
}

/// The opacities after each of `ticks` ticks of [`DT`].
fn opacities(track: UiTrack, ticks: usize) -> (Vec<f32>, Vec<UiEvent>) {
    let (mut tree, button) = rig();
    assert!(tree.animate(button, track));
    let mut values = vec![tree.opacity(button).unwrap()];
    let mut events = Vec::new();
    for _ in 0..ticks {
        events.extend(tick(&mut tree, DT));
        values.push(tree.opacity(button).unwrap());
    }
    (values, events)
}

fn finished(widget: WidgetId, property: UiProperty, tag: u32) -> UiEvent {
    UiEvent::AnimationFinished {
        widget,
        property,
        tag,
    }
}

#[test]
fn each_property_interpolates_under_two_easings() {
    for easing in [Easing::Linear, Easing::QuadIn] {
        let k = easing.apply(DT);
        let (mut tree, button) = rig();
        tree.animate(button, UiTrack::opacity(0.0, 0.8, 1.0).with_easing(easing));
        tree.animate(
            button,
            UiTrack::translation(Vec2::ZERO, Vec2::new(8.0, -16.0), 1.0).with_easing(easing),
        );
        tree.animate(button, UiTrack::scale(1.0, 2.0, 1.0).with_easing(easing));
        tree.animate(
            button,
            UiTrack::background(BLACK, WHITE, 1.0).with_easing(easing),
        );
        tree.animate(
            button,
            UiTrack::border(WHITE, BLACK, 1.0).with_easing(easing),
        );
        tree.animate(
            button,
            UiTrack::text_color(BLACK, WHITE, 1.0).with_easing(easing),
        );
        assert_eq!(tree.opacity(button), Some(0.0), "the start value at once");
        assert_eq!(tree.scale(button), Some(1.0));
        assert_eq!(tree.overrides(button).unwrap().background, Some(BLACK));
        tick(&mut tree, DT);
        assert_eq!(tree.opacity(button), Some(0.8 * k), "{easing:?}");
        assert_eq!(tree.translation(button), Some(Vec2::new(8.0, -16.0) * k));
        assert_eq!(tree.scale(button), Some(1.0 + k));
        let up = (255.0 * k).round() as u8;
        let down = (255.0 - 255.0 * k).round() as u8;
        let overrides = tree.overrides(button).unwrap();
        assert_eq!(overrides.background, Some([up, up, up, 255]));
        assert_eq!(overrides.border, Some([down, down, down, 255]));
        assert_eq!(overrides.text.color, Some([up, up, up, 255]));
    }
}

#[test]
fn repeats_follow_a_hand_computed_timeline_with_the_remainder_carried() {
    let once = UiTrack::opacity(0.0, 1.0, 1.0).with_tag(7);
    let (values, events) = opacities(once, 4);
    assert_eq!(values, [0.0, 0.375, 0.75, 1.0, 1.0]);
    assert_eq!(events.len(), 1);

    let times = once.with_repeat(TweenRepeat::Times(3));
    let (values, events) = opacities(times, 9);
    assert_eq!(
        values,
        [0.0, 0.375, 0.75, 0.125, 0.5, 0.875, 0.25, 0.625, 1.0, 1.0]
    );
    assert_eq!(events.len(), 1, "after the third cycle, once");

    let looped = once.with_repeat(TweenRepeat::Loop);
    let (values, events) = opacities(looped, 8);
    assert_eq!(
        values,
        [0.0, 0.375, 0.75, 0.125, 0.5, 0.875, 0.25, 0.625, 0.0]
    );
    assert!(events.is_empty(), "a loop never finishes");

    let ping = once.with_repeat(TweenRepeat::PingPong);
    let (values, events) = opacities(ping, 8);
    assert_eq!(
        values,
        [0.0, 0.375, 0.75, 0.875, 0.5, 0.125, 0.25, 0.625, 1.0]
    );
    assert!(events.is_empty(), "a ping-pong never finishes");
}

#[test]
fn backward_plays_from_the_end_to_the_start() {
    let track = UiTrack::opacity(0.0, 1.0, 1.0).with_direction(TweenDirection::Backward);
    let (values, events) = opacities(track, 3);
    assert_eq!(values, [1.0, 0.625, 0.25, 0.0]);
    assert_eq!(events.len(), 1);
}

#[test]
fn once_and_times_send_one_tagged_event_read_once_and_loops_send_none() {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.register_event::<UiEvent>();
    let (tree, button) = rig();
    world.insert_resource(tree);
    let frame = |world: &mut World| {
        world.flush_events();
        world.resource_mut::<Time>().advance_frame(DT);
        ui_animate_system(world);
    };
    let tree = world.resource_mut::<UiTree>();
    tree.animate(button, UiTrack::opacity(0.0, 1.0, 0.5).with_tag(3));
    tree.animate(
        button,
        UiTrack::scale(1.0, 2.0, 0.25)
            .with_repeat(TweenRepeat::Times(2))
            .with_tag(4),
    );
    tree.animate(
        button,
        UiTrack::translation(Vec2::ZERO, Vec2::ONE, 0.25).with_repeat(TweenRepeat::Loop),
    );
    tree.animate(
        button,
        UiTrack::background(BLACK, WHITE, 0.25).with_repeat(TweenRepeat::PingPong),
    );
    frame(&mut world);
    let queue = world.resource::<EventQueue<UiEvent>>();
    assert!(!queue.animation_finished(button, 3));
    assert!(!queue.animation_finished(button, 4));
    frame(&mut world);
    let queue = world.resource::<EventQueue<UiEvent>>();
    let events: Vec<UiEvent> = queue.iter_current().copied().collect();
    assert_eq!(
        events,
        [
            finished(button, UiProperty::Opacity, 3),
            finished(button, UiProperty::Scale, 4),
        ]
    );
    assert!(queue.animation_finished(button, 3));
    assert!(queue.animation_finished(button, 4));
    assert!(!queue.animation_finished(button, 0));
    frame(&mut world);
    let queue = world.resource::<EventQueue<UiEvent>>();
    assert!(
        !queue.animation_finished(button, 3),
        "read once: the current window"
    );
    assert_eq!(queue.iter_current().count(), 0);
    for _ in 0..8 {
        frame(&mut world);
    }
    assert_eq!(
        world
            .resource::<EventQueue<UiEvent>>()
            .iter_current()
            .count(),
        0
    );
    assert!(world.resource::<UiTree>().is_animating(button));
}

#[test]
fn a_replaced_track_is_silent_and_stopping_keeps_the_value() {
    let (mut tree, button) = rig();
    tree.animate(button, UiTrack::opacity(0.0, 1.0, 1.0));
    tick(&mut tree, DT);
    tree.animate(button, UiTrack::opacity(1.0, 0.5, 1.0));
    assert_eq!(tree.opacity(button), Some(1.0));
    let mut events = Vec::new();
    for _ in 0..3 {
        events.extend(tick(&mut tree, DT));
    }
    assert_eq!(
        events,
        [finished(button, UiProperty::Opacity, 0)],
        "only the replacement finishes"
    );
    assert_eq!(tree.opacity(button), Some(0.5));

    tree.animate(button, UiTrack::scale(1.0, 2.0, 1.0));
    tick(&mut tree, DT);
    tree.stop_animation(button, UiProperty::Scale);
    assert!(!tree.is_animating(button));
    assert!(tick(&mut tree, DT).is_empty());
    assert_eq!(tree.scale(button), Some(1.375), "where it stopped");
}

#[test]
fn removing_a_widget_drops_its_tracks() {
    let (mut tree, button) = rig();
    let label = tree.children(button)[0];
    tree.animate(label, UiTrack::opacity(0.0, 1.0, 1.0));
    tree.animate(button, UiTrack::opacity(0.0, 1.0, 1.0));
    assert!(tree.is_animating(label));
    tree.remove(button);
    assert!(!tree.is_animating(label));
    assert!(tree.tracks.is_empty());
    assert!(tick(&mut tree, 2.0).is_empty());
}

#[test]
fn refused_values_leave_the_property_unchanged() {
    let (mut tree, button) = rig();
    tree.clear_paint_dirty();
    for track in [
        UiTrack::opacity(f32::NAN, 1.0, 1.0),
        UiTrack::opacity(0.0, f32::INFINITY, 1.0),
        UiTrack::translation(Vec2::new(f32::NAN, 0.0), Vec2::ZERO, 1.0),
        UiTrack::translation(Vec2::ZERO, Vec2::splat(f32::NEG_INFINITY), 1.0),
        UiTrack::scale(0.0, 1.0, 1.0),
        UiTrack::scale(1.0, -2.0, 1.0),
        UiTrack::scale(f32::NAN, 1.0, 1.0),
    ] {
        assert!(!tree.animate(button, track), "{track:?}");
    }
    assert!(!tree.is_animating(button));
    assert!(!tree.is_paint_dirty());
    assert_eq!(tree.opacity(button), Some(1.0));
    assert_eq!(tree.translation(button), Some(Vec2::ZERO));
    assert_eq!(tree.scale(button), Some(1.0));
}

#[test]
fn a_duration_that_cannot_run_finishes_at_the_next_tick_on_its_end_value() {
    for secs in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let (mut tree, button) = rig();
        tree.animate(button, UiTrack::opacity(0.25, 0.75, secs).with_tag(9));
        assert_eq!(tree.opacity(button), Some(0.25));
        let events = tick(&mut tree, 0.0);
        assert_eq!(
            events,
            [finished(button, UiProperty::Opacity, 9)],
            "secs {secs}"
        );
        assert_eq!(tree.opacity(button), Some(0.75));
    }
}

#[test]
fn a_zero_dt_moves_nothing() {
    let (mut tree, button) = rig();
    tree.animate(button, UiTrack::opacity(0.0, 1.0, 1.0));
    tick(&mut tree, DT);
    tree.clear_paint_dirty();
    for dt in [0.0, -1.0, f32::NAN] {
        assert!(tick(&mut tree, dt).is_empty());
    }
    assert_eq!(tree.opacity(button), Some(0.375));
    assert!(!tree.is_paint_dirty());
}

#[test]
fn colour_tracks_repaint_without_layout_or_set_text() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let button = tree.button(root, "go");
    let mut store = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut store);
    let label = tree.children(button)[0];
    assert_eq!(store.counts.set_text_calls, 1);
    let before = store.counts;
    tree.clear_paint_dirty();
    tree.animate(button, UiTrack::background(BLACK, WHITE, 1.0));
    tree.animate(button, UiTrack::border(BLACK, WHITE, 1.0));
    tree.animate(button, UiTrack::text_color(BLACK, WHITE, 1.0));
    for _ in 0..2 {
        tick(&mut tree, DT);
        assert!(tree.is_paint_dirty());
        assert_eq!(tree.layout(VIEWPORT, &mut store), LayoutStats::default());
        tree.clear_paint_dirty();
    }
    assert_eq!(store.counts.set_text_calls, before.set_text_calls);
    assert_eq!(
        tree.resolved_style(label).unwrap().text.color,
        tree.resolved_style(button).unwrap().text.color,
        "the label inherits the track's colour"
    );
    assert_eq!(
        tree.resolved_style(button).unwrap().background,
        [191, 191, 191, 255]
    );
    assert!(store.committed(tree.text_node(label).unwrap()).is_some());
}
