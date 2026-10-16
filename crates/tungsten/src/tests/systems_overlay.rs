use glam::Vec2;
use tungsten_core::input::KeyCode;
use tungsten_core::text::TextAlign;
use tungsten_core::ui::{FixedAdvanceMeasure, UiTree};

use super::*;
use crate::app::WindowSize;
use crate::debug_views::{GREEN, RED, STRONG, YELLOW};

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
    world.insert_resource(FixedAdvanceMeasure::default());
    world.insert_resource(SystemTimingOverlay {
        enabled: true,
        refresh_interval_ms: 0.0,
        ..Default::default()
    });
    world
}

/// The views' frame, then the layout through the world's one text store.
fn frame(world: &mut World) {
    crate::debug_views::update(world);
    let mut store = world.remove_resource::<FixedAdvanceMeasure>().unwrap();
    world
        .resource_mut::<UiTree>()
        .layout(Vec2::new(1280.0, 720.0), &mut store);
    world.insert_resource(store);
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
    assert_eq!(overlay.corner, HudCorner::TopRight);
    assert_eq!(overlay.font_id, crate::ENGINE_FONT_ID);
    assert_eq!((overlay.font_size, overlay.line_height), (15.0, 20.0));
    assert_eq!(
        (overlay.outline_color, overlay.outline_px),
        (crate::debug_views::BORDER, 1.0)
    );
    assert_eq!(overlay.background, crate::debug_views::PANEL);
    assert_eq!(overlay.padding_px, 8.0);
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
    assert_eq!(column_texts(&world, 0), ["system", "other", "alpha"]);
}

#[test]
fn rows_sort_by_average_under_the_column_headers() {
    let mut world = world_with_timings(&[("fast", 0.5), ("slow", 3.0), ("medium", 1.25)]);
    frame(&mut world);
    assert_eq!(
        column_texts(&world, 0),
        ["system", "other", "slow", "medium", "fast"]
    );
    assert_eq!(
        column_texts(&world, 1),
        ["avg ms", "4.75", "3.00", "1.25", "0.50"]
    );
    let window = world
        .resource::<SystemTimingOverlay>()
        .window
        .as_ref()
        .unwrap();
    let tree = world.resource::<UiTree>();
    let time = tree.children(window.columns[1])[2];
    assert_eq!(
        tree.resolved_style(time).unwrap().text.layout.align,
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

// ---- M58 step 5: stages, numbers and bars ----

fn overlay_window(world: &World) -> &ViewWindow {
    world
        .resource::<SystemTimingOverlay>()
        .window
        .as_ref()
        .expect("open")
}

/// Column `c`'s resolved text colours and weights.
fn column_inks(world: &World, c: usize) -> Vec<([u8; 4], u16)> {
    let tree = world.resource::<UiTree>();
    tree.children(overlay_window(world).columns[c])
        .iter()
        .map(|label| {
            let text = &tree.resolved_style(*label).unwrap().text;
            (text.color, text.weight)
        })
        .collect()
}

#[test]
fn the_title_carries_the_total_and_the_headers_name_the_columns() {
    let mut world = world_with_timings(&[("a", 1.25), ("b", 2.5)]);
    frame(&mut world);
    let tree = world.resource::<UiTree>();
    let caption = overlay_window(&world).caption;
    let title: String = tree.text(caption).unwrap().spans[0].text.clone();
    assert_eq!(title, "Systems \u{b7} 3.75 ms");
    let headers: Vec<String> = (0..4).map(|c| column_texts(&world, c)[0].clone()).collect();
    assert_eq!(headers, ["system", "avg ms", "max ms", "% frame"]);
    for c in 0..4 {
        assert_eq!(column_inks(&world, c)[0], (LABEL, STRONG), "column {c}");
    }
}

#[test]
fn systems_group_by_stage_slowest_first_each_stage_in_its_hue_with_its_total() {
    let mut world = world_with_timings(&[("a", 1.0), ("b", 3.0), ("c", 2.0), ("d", 0.5)]);
    let stages = [
        ("a", Stage::Update),
        ("b", Stage::Update),
        ("c", Stage::PreUpdate),
    ]
    .into_iter()
    .map(|(name, stage)| (name.to_string(), stage))
    .collect();
    world.insert_resource(SystemStages(stages));
    frame(&mut world);
    assert_eq!(
        column_texts(&world, 0),
        [
            "system",
            "pre_update",
            "c",
            "update",
            "b",
            "a",
            "other",
            "d"
        ]
    );
    assert_eq!(
        column_texts(&world, 1),
        [
            "avg ms", "2.00", "2.00", "4.00", "3.00", "1.00", "0.50", "0.50"
        ]
    );
    let inks = column_inks(&world, 0);
    assert_eq!(inks[1], (BLUE, STRONG));
    assert_eq!(inks[3], (CYAN, STRONG));
    assert_eq!(inks[6], (LABEL, STRONG));
    let tree = world.resource::<UiTree>();
    let names = tree.children(overlay_window(&world).columns[0]);
    let start = |i: usize| tree.layout_style(names[i]).unwrap().padding.start;
    assert_eq!((start(1), start(2)), (6.0, 18.0), "systems indent");
}

#[test]
fn the_app_records_each_systems_stage_from_its_schedule() {
    let mut app = crate::App::new(tungsten_core::Config::default()).unwrap();
    app.add_system(|_| {});
    let mut harness = crate::testing::UiHarness::new(app);
    harness
        .world_mut()
        .resource_mut::<SystemTimingOverlay>()
        .enabled = true;
    harness.step(3);
    let stages = harness.world().resource::<SystemStages>();
    assert_eq!(stages.0.get("system_0"), Some(&Stage::Update));
    assert_eq!(
        stages.0.get(crate::plugins::UI_ROUTE),
        Some(&Stage::PreUpdate)
    );
    let names = column_texts(harness.world(), 0);
    let order: Vec<&str> = [
        "startup",
        "pre_update",
        "fixed_update",
        "update",
        "post_update",
        "other",
    ]
    .into_iter()
    .filter(|stage| names.iter().any(|name| name == stage))
    .collect();
    let shown: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| order.contains(name))
        .collect();
    assert_eq!(shown, order, "in frame order");
    assert!(shown.contains(&"pre_update") && shown.contains(&"update"));
    let inks = column_inks(harness.world(), 0);
    let at = |stage: &str| names.iter().position(|name| name == stage).unwrap();
    assert_eq!(inks[at("pre_update")].0, BLUE);
    assert_eq!(inks[at("update")].0, CYAN);
    if names.iter().any(|name| name == "post_update") {
        assert_eq!(inks[at("post_update")].0, MAGENTA);
    }
}

#[test]
fn a_spike_shows_as_the_max_for_four_refreshes() {
    let mut world = world_with_timings(&[("a", 1.0)]);
    frame(&mut world);
    world.resource_mut::<FrameTimings>().system_timings = vec![("a".into(), 9.0)];
    frame(&mut world);
    world.resource_mut::<FrameTimings>().system_timings = vec![("a".into(), 1.0)];
    let max = |world: &World| column_texts(world, 2)[2].clone();
    assert_eq!(max(&world), "9.00", "the spike's refresh");
    for refresh in 2..=4 {
        frame(&mut world);
        assert_eq!(max(&world), "9.00", "refresh {refresh} after it");
    }
    frame(&mut world);
    assert_eq!(max(&world), "1.00", "gone at the fifth");
    assert_eq!(column_inks(&world, 2)[2].0, GREEN);
}

#[test]
fn the_share_divides_by_the_frames_average_and_the_bar_shows_it_on_the_ramp() {
    let mut world = world_with_timings(&[("a", 2.0), ("b", 4.0)]);
    world.resource_mut::<FrameTimings>().total_ms = 10.0;
    frame(&mut world);
    assert_eq!(column_texts(&world, 3)[2..], ["40.0%", "20.0%"]);
    assert_eq!(column_inks(&world, 1)[2].0, RED, "4 ms is over 3.33");
    assert_eq!(column_inks(&world, 1)[3].0, YELLOW, "2 ms is up to 2.5");
    let tree = world.resource::<UiTree>();
    let bars = tree.children(overlay_window(&world).columns[4]);
    let fill = |row: usize| {
        let track = tree.children(bars[row])[0];
        let fill = tree.children(track)[0];
        (
            tree.rect(fill).unwrap().width(),
            tree.resolved_style(fill).unwrap().background,
            tree.visibility(track),
        )
    };
    assert!((fill(2).0 - 48.0 * 0.4).abs() < 1e-3, "{:?}", fill(2));
    assert_eq!(
        (fill(2).1, fill(2).2),
        (RED, Some(tungsten_core::ui::Visibility::Visible))
    );
    assert!((fill(3).0 - 48.0 * 0.2).abs() < 1e-3, "{:?}", fill(3));
    assert_eq!(fill(3).1, YELLOW);
    assert_eq!(
        fill(0).2,
        Some(tungsten_core::ui::Visibility::Hidden),
        "a header row has no bar"
    );
    assert_eq!(tree.rect(bars[2]).unwrap().height(), 20.0, "one line");
}
