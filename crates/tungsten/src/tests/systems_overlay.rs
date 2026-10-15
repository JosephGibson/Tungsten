use glam::Vec2;
use tungsten_core::input::KeyCode;
use tungsten_core::text::TextAlign;
use tungsten_core::ui::{FixedAdvanceMeasure, UiTree};

use super::*;
use crate::app::WindowSize;

/// A world with what the views' frame reads, the overlay on, and these
/// system timings.
fn world_with_timings(systems: &[(&str, f32)]) -> World {
    let mut world = World::new();
    world.insert_resource(UiTree::new());
    world.insert_resource(WindowSize {
        width: 1280,
        height: 720,
    });
    let mut ft = FrameTimings::new();
    ft.system_timings = systems.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    world.insert_resource(ft);
    world.insert_resource(SystemTimingOverlay {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..Default::default()
    });
    world
}

fn frame(world: &mut World) {
    crate::debug_views::update(world);
    world.resource_mut::<UiTree>().layout(
        Vec2::new(1280.0, 720.0),
        &mut FixedAdvanceMeasure::default(),
    );
}

fn column_texts(world: &World, column: usize) -> Vec<String> {
    let window = world
        .resource::<SystemTimingOverlay>()
        .window
        .as_ref()
        .expect("open");
    let tree = world.resource::<UiTree>();
    tree.children(window.columns[column])
        .iter()
        .map(|label| {
            tree.text(*label)
                .map(|text| text.spans.iter().map(|span| span.text.as_str()).collect())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn the_overlay_keeps_its_defaults() {
    let overlay = SystemTimingOverlay::default();
    assert!(!overlay.enabled);
    assert_eq!(overlay.corner, HudCorner::BottomRight);
    assert_eq!(overlay.font_id, crate::ENGINE_FONT_ID);
    assert_eq!((overlay.font_size, overlay.line_height), (28.0, 32.0));
    assert_eq!(
        (overlay.outline_color, overlay.outline_px),
        ([0, 0, 0, 220], 1.5)
    );
    assert_eq!(overlay.background, [16, 16, 20, 200]);
    assert_eq!(overlay.padding_px, 12.0);
    assert_eq!((overlay.refresh_interval_ms, overlay.alpha), (500.0, 0.1));
    assert_eq!(overlay.window(), None);
}

#[test]
fn nothing_opens_while_disabled() {
    let mut world = world_with_timings(&[("a", 1.0)]);
    world.resource_mut::<SystemTimingOverlay>().enabled = false;
    frame(&mut world);
    assert_eq!(world.resource::<SystemTimingOverlay>().window(), None);
    assert!(world.resource::<UiTree>().roots().is_empty());
}

#[test]
fn the_averages_converge_on_constant_input() {
    let mut world = world_with_timings(&[("a", 2.0), ("b", 4.0)]);
    for _ in 0..200 {
        frame(&mut world);
    }
    let overlay = world.resource::<SystemTimingOverlay>();
    let a = overlay.ewma_for("a").unwrap();
    let b = overlay.ewma_for("b").unwrap();
    assert!((a - 2.0).abs() < 0.01, "a = {a}");
    assert!((b - 4.0).abs() < 0.01, "b = {b}");
}

#[test]
fn a_system_gone_from_the_timings_drops_its_average() {
    let mut world = world_with_timings(&[("alpha", 1.0), ("beta", 1.0)]);
    frame(&mut world);
    assert!(
        world
            .resource::<SystemTimingOverlay>()
            .ewma_for("beta")
            .is_some()
    );
    world.resource_mut::<FrameTimings>().system_timings = vec![("alpha".into(), 1.0)];
    frame(&mut world);
    let overlay = world.resource::<SystemTimingOverlay>();
    assert!(overlay.ewma_for("alpha").is_some());
    assert!(overlay.ewma_for("beta").is_none());
    assert_eq!(column_texts(&world, 0), ["alpha"]);
}

#[test]
fn rows_sort_by_average_with_the_names_right_aligned() {
    let mut world = world_with_timings(&[("fast", 0.5), ("slow", 3.0), ("medium", 1.25)]);
    frame(&mut world);
    assert_eq!(column_texts(&world, 0), ["slow", "medium", "fast"]);
    assert_eq!(
        column_texts(&world, 1),
        ["  3.00ms", "  1.25ms", "  0.50ms"]
    );
    let window = world
        .resource::<SystemTimingOverlay>()
        .window
        .as_ref()
        .unwrap();
    let tree = world.resource::<UiTree>();
    let name = tree.children(window.columns[0])[0];
    assert_eq!(
        tree.resolved_style(name).unwrap().text.layout.align,
        TextAlign::End
    );
    assert!(tree.scroll(window.body).is_some(), "a scrolling body");
}

#[test]
fn toggle_on_f2_action_flips_enabled() {
    let mut world = World::new();
    let mut input = InputState::new();
    input.key_down(KeyCode::F2);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(SystemTimingOverlay::default());

    systems_overlay_toggle_system(&mut world);

    assert!(world.get_resource::<SystemTimingOverlay>().unwrap().enabled);
}
