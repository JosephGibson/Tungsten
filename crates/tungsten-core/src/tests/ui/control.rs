use glam::Vec2;

use super::*;
use crate::ecs::EventQueue;
use crate::ui::{Align, FixedAdvanceMeasure, Role, Sizing, UiEvent, UiLayer, UiValue, WidgetKind};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

/// A root holding a toggle (off) and a slider (0 to 10 by 1, at 5), laid
/// out and clean.
fn controls() -> (UiTree, WidgetId, WidgetId, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::row());
    let toggle = tree.toggle(root, false);
    let slider = tree.slider(root, SliderRange::new(0.0, 10.0).with_step(1.0), 5.0);
    settle(&mut tree);
    (tree, root, toggle, slider)
}

fn settle(tree: &mut UiTree) {
    let mut text = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut text);
    tree.clear_paint_dirty();
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());
}

/// `min + k·step` as the range computes it: in `f64`, stored as `f32`.
fn allowed(min: f32, step: f32, k: u32) -> f32 {
    (f64::from(min) + f64::from(k) * f64::from(step)) as f32
}

#[test]
#[should_panic(expected = "bounds must be finite")]
fn a_range_with_a_bound_not_finite_panics() {
    let _ = SliderRange::new(0.0, f32::INFINITY);
}

#[test]
#[should_panic(expected = "is above max")]
fn a_range_with_min_above_max_panics() {
    let _ = SliderRange::new(2.0, 1.0);
}

#[test]
#[should_panic(expected = "finite and not negative")]
fn a_negative_step_panics() {
    let _ = SliderRange::new(0.0, 1.0).with_step(-0.5);
}

#[test]
#[should_panic(expected = "finite and not negative")]
fn a_step_not_finite_panics() {
    let _ = SliderRange::new(0.0, 1.0).with_step(f32::NAN);
}

#[test]
fn a_range_reports_its_bounds_and_step() {
    let range = SliderRange::new(-1.0, 3.0);
    assert_eq!((range.min(), range.max(), range.step()), (-1.0, 3.0, 0.0));
    let stepped = range.with_step(0.5);
    assert_eq!(stepped.step(), 0.5);
    assert_eq!((stepped.min(), stepped.max()), (-1.0, 3.0));
}

#[test]
fn snapping_clamps_then_takes_the_nearest_allowed_value_lower_on_a_tie() {
    // 0 to 1 by 0.3: 0, 0.3, 0.6, 0.9 and 1 itself.
    let range = SliderRange::new(0.0, 1.0).with_step(0.3);
    assert_eq!(range.snap(0.97), 1.0, "max stays reachable");
    assert_eq!(range.snap(0.8), allowed(0.0, 0.3, 3));
    assert_eq!(range.snap(0.94), allowed(0.0, 0.3, 3));
    assert_eq!(range.snap(0.16), allowed(0.0, 0.3, 1));
    assert_eq!(range.snap(0.15), 0.0, "a tie takes the lower");
    assert_eq!(range.snap(-4.0), 0.0);
    assert_eq!(range.snap(4.0), 1.0);

    // Continuous: clamped only.
    let continuous = SliderRange::new(-2.0, 2.0);
    assert_eq!(continuous.snap(0.123), 0.123);
    assert_eq!(continuous.snap(-9.0), -2.0);
    assert_eq!(continuous.snap(9.0), 2.0);

    // A step longer than the span: the two bounds.
    let coarse = SliderRange::new(0.0, 1.0).with_step(5.0);
    assert_eq!(coarse.snap(0.4), 0.0);
    assert_eq!(coarse.snap(0.6), 1.0);
}

#[test]
fn a_singleton_range_holds_its_one_value() {
    let range = SliderRange::new(3.0, 3.0);
    assert_eq!(range.snap(-100.0), 3.0);
    assert_eq!(range.snap(100.0), 3.0);
    assert_eq!(range.with_step(1.0).snap(7.0), 3.0);
}

#[test]
fn the_widest_range_snaps_to_finite_values() {
    let range = SliderRange::new(-f32::MAX, f32::MAX);
    for value in [-f32::MAX, -1.0, 0.0, 1.0e30, f32::MAX] {
        let snapped = range.snap(value);
        assert!(snapped.is_finite(), "{value} → {snapped}");
        assert_eq!(snapped, value);
    }
    let stepped = range.with_step(f32::MAX);
    // -MAX, 0 and MAX: the span is two steps.
    for (value, expected) in [
        (-f32::MAX, -f32::MAX),
        (-f32::MAX * 0.75, -f32::MAX),
        (-f32::MAX / 2.0, -f32::MAX),
        (-f32::MAX / 4.0, 0.0),
        (f32::MAX / 4.0, 0.0),
        (f32::MAX * 0.75, f32::MAX),
    ] {
        let snapped = stepped.snap(value);
        assert!(snapped.is_finite(), "{value} → {snapped}");
        assert_eq!(snapped, expected, "{value}");
    }
    let fine = range.with_step(1.0e-30);
    assert!(fine.snap(12.5).is_finite());
}

#[test]
fn max_stays_reachable_when_the_step_index_loses_precision() {
    // 2^53 + 1 steps: `f64` cannot index the last one (API critique C.1).
    let min = -(2.0_f32.powi(53));
    let range = SliderRange::new(min, 1.0).with_step(1.0);
    assert_eq!(range.snap(1.0), 1.0);
    assert_eq!(range.snap(0.6), 1.0);
    assert_eq!(range.snap(0.4), 0.0);
    assert_eq!(range.snap(-0.6), -1.0);
    assert_eq!(range.snap(min), min);
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let slider = tree.slider(root, range, 1.0);
    assert_eq!(tree.slider_value(slider), Some(1.0));
    assert!(tree.set_slider_value(slider, 0.0));
    assert!(tree.set_slider_value(slider, 1.0));
    assert_eq!(tree.slider_value(slider), Some(1.0));
}

#[test]
fn builders_make_focusable_interactive_leaves_with_their_roles() {
    let (tree, _root, toggle, slider) = controls();
    for (id, kind, role) in [
        (toggle, WidgetKind::Toggle, Role::CheckBox),
        (slider, WidgetKind::Slider, Role::Slider),
    ] {
        assert_eq!(tree.kind(id), Some(kind));
        assert_eq!(tree.role(id), Some(role));
        assert!(tree.focusable(id));
        assert!(tree.interactive(id));
        assert!(kind.is_leaf());
        assert_eq!(tree.text(id), None, "a control holds no text");
        assert_eq!(tree.accessible_name(id), None);
    }
    assert_eq!(tree.toggled(toggle), Some(false));
    assert_eq!(tree.slider_value(slider), Some(5.0));
    assert_eq!(
        tree.slider_range(slider),
        Some(SliderRange::new(0.0, 10.0).with_step(1.0))
    );
    assert_eq!(tree.slider_value(toggle), None);
    assert_eq!(tree.slider_range(toggle), None);
    assert_eq!(tree.toggled(slider), None);
}

#[test]
fn a_control_takes_its_caption_as_its_accessible_name() {
    let (mut tree, _root, toggle, _slider) = controls();
    assert!(!tree.set_text(toggle, "ignored"), "a toggle holds no text");
    assert!(!tree.is_layout_dirty());
    tree.set_accessible_name(toggle, Some("Damage numbers".to_owned()));
    assert_eq!(tree.accessible_name(toggle), Some("Damage numbers"));
}

#[test]
#[should_panic(expected = "Toggle holds no children")]
fn a_toggle_holds_no_children() {
    let (mut tree, _root, toggle, _slider) = controls();
    tree.label(toggle, "caption");
}

#[test]
#[should_panic(expected = "Slider holds no children")]
fn a_slider_holds_no_children() {
    let (mut tree, _root, _toggle, slider) = controls();
    tree.panel(slider, LayoutStyle::default());
}

#[test]
fn a_slider_built_out_of_range_or_off_its_step_is_clamped_and_snapped() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let range = SliderRange::new(0.0, 10.0).with_step(2.0);
    for (value, expected) in [(-5.0, 0.0), (99.0, 10.0), (4.9, 4.0), (5.1, 6.0)] {
        let slider = tree.slider(root, range, value);
        assert_eq!(tree.slider_value(slider), Some(expected), "{value}");
    }
}

#[test]
fn a_slider_built_with_a_value_not_finite_starts_at_min() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let range = SliderRange::new(-3.0, 7.0).with_step(0.5);
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let slider = tree.slider(root, range, value);
        assert_eq!(tree.slider_value(slider), Some(-3.0), "{value}");
    }
}

#[test]
fn set_toggled_reports_a_change_marks_paint_only_and_sends_nothing() {
    let (mut tree, _root, toggle, slider) = controls();
    assert!(!tree.set_toggled(toggle, false), "equal: no change");
    assert!(!tree.is_paint_dirty());

    assert!(tree.set_toggled(toggle, true));
    assert_eq!(tree.toggled(toggle), Some(true));
    assert!(tree.is_paint_dirty());
    assert!(
        !tree.is_layout_dirty(),
        "a state paints, it does not measure"
    );

    tree.clear_paint_dirty();
    assert!(!tree.set_toggled(slider, true), "a slider is not a toggle");
    assert!(!tree.is_paint_dirty());
    // The tree holds no event queue: only the router sends `ValueChanged`
    // (step 3's route tests pin that a setter's change routes nothing).
}

#[test]
fn set_slider_value_clamps_snaps_and_reports_a_change() {
    let (mut tree, _root, toggle, slider) = controls();
    assert!(!tree.set_slider_value(slider, 5.0), "equal: no change");
    assert!(
        !tree.set_slider_value(slider, 5.4),
        "snaps to the same value"
    );
    assert!(!tree.is_paint_dirty());

    assert!(tree.set_slider_value(slider, 6.6));
    assert_eq!(tree.slider_value(slider), Some(7.0));
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());

    assert!(tree.set_slider_value(slider, 40.0));
    assert_eq!(tree.slider_value(slider), Some(10.0));
    assert!(tree.set_slider_value(slider, -40.0));
    assert_eq!(tree.slider_value(slider), Some(0.0));

    tree.clear_paint_dirty();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(!tree.set_slider_value(slider, value), "{value}");
    }
    assert_eq!(tree.slider_value(slider), Some(0.0));
    assert!(
        !tree.set_slider_value(toggle, 1.0),
        "a toggle is not a slider"
    );
    assert!(!tree.is_paint_dirty());
}

#[test]
fn set_slider_range_snaps_the_value_into_the_new_range() {
    let (mut tree, _root, _toggle, slider) = controls();
    tree.set_slider_range(slider, SliderRange::new(0.0, 10.0).with_step(1.0));
    assert!(!tree.is_paint_dirty(), "an equal range marks nothing");

    tree.set_slider_range(slider, SliderRange::new(0.0, 3.0).with_step(1.5));
    assert_eq!(tree.slider_value(slider), Some(3.0));
    assert_eq!(
        tree.slider_range(slider),
        Some(SliderRange::new(0.0, 3.0).with_step(1.5))
    );
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());

    tree.clear_paint_dirty();
    tree.set_slider_range(slider, SliderRange::new(0.0, 3.0));
    assert_eq!(tree.slider_value(slider), Some(3.0), "the value holds");
    assert!(tree.is_paint_dirty(), "the step changed");
}

#[test]
fn an_auto_control_takes_its_size_from_the_line_height() {
    let (tree, _root, toggle, slider) = controls();
    let line = tree.theme().tokens.text.line_height;
    assert_eq!(line, 20.0);
    assert_eq!(
        tree.rect(toggle).expect("laid out").size(),
        Vec2::new(20.0, 20.0)
    );
    assert_eq!(
        tree.rect(slider).expect("laid out").size(),
        Vec2::new(200.0, 20.0)
    );
}

#[test]
fn a_px_axis_wins_over_the_natural_size() {
    let (mut tree, root, toggle, slider) = controls();
    tree.set_layout_style(
        root,
        LayoutStyle {
            align_items: Align::Start,
            ..LayoutStyle::row()
        },
    );
    tree.set_layout_style(
        slider,
        LayoutStyle::default().with_size(Sizing::Px(120.0), Sizing::Auto),
    );
    tree.set_layout_style(
        toggle,
        LayoutStyle::default().with_size(Sizing::Auto, Sizing::Px(32.0)),
    );
    let mut text = FixedAdvanceMeasure::default();
    let stats = tree.layout(VIEWPORT, &mut text);
    assert_eq!(stats.measure_calls, 0, "a control measures no text");
    assert_eq!(
        tree.rect(slider).expect("laid out").size(),
        Vec2::new(120.0, 20.0)
    );
    assert_eq!(
        tree.rect(toggle).expect("laid out").size(),
        Vec2::new(20.0, 32.0)
    );
}

#[test]
fn value_changed_returns_the_last_value_of_the_current_window() {
    let (_tree, _root, toggle, slider) = controls();
    let mut queue = EventQueue::<UiEvent>::new();
    queue.send(UiEvent::ValueChanged {
        widget: slider,
        value: UiValue::Number(2.0),
    });
    queue.flush();
    assert_eq!(
        queue.value_changed(slider),
        None,
        "the previous frame's change is not this routing's"
    );
    queue.send(UiEvent::ValueChanged {
        widget: slider,
        value: UiValue::Number(3.0),
    });
    queue.send(UiEvent::ValueChanged {
        widget: toggle,
        value: UiValue::Bool(true),
    });
    queue.send(UiEvent::ValueChanged {
        widget: slider,
        value: UiValue::Number(4.0),
    });
    assert_eq!(queue.value_changed(slider), Some(UiValue::Number(4.0)));
    assert_eq!(queue.value_changed(toggle), Some(UiValue::Bool(true)));
    let event = queue.iter_current().next().copied().expect("sent");
    assert_eq!(event.widget(), slider);

    assert_eq!(UiValue::Bool(true).as_bool(), Some(true));
    assert_eq!(UiValue::Bool(true).as_number(), None);
    assert_eq!(UiValue::Number(0.5).as_number(), Some(0.5));
    assert_eq!(UiValue::Number(0.5).as_bool(), None);
}
