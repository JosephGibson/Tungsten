use std::cell::Cell;
use std::rc::Rc;

use glam::Vec2;
use tungsten_core::ActionMap;
use tungsten_core::input::{InputState, KeyCode};
use tungsten_core::ui::{FixedAdvanceMeasure, UiTree};

use super::*;
use crate::app::WindowSize;
use crate::debug_views::{GRAPH_WIDTH, GREEN, ORANGE, RED, YELLOW};

/// A world with what the views' frame reads, the HUD on.
fn views_world() -> World {
    let mut world = World::new();
    world.insert_resource(UiTree::new());
    world.insert_resource(WindowSize {
        width: 1280,
        height: 720,
    });
    world.insert_resource(FrameTimings::new());
    world.insert_resource(FixedAdvanceMeasure::default());
    let mut hud = DebugHud::new();
    hud.enabled = true;
    world.insert_resource(hud);
    world
}

/// The views' frame after a frame `interval` ms apart from the one before
/// that took `total` ms, then the layout, through the world's one text
/// store, as the app's text engine keeps its nodes.
fn frame(world: &mut World, interval: Option<f32>, total: f32) {
    let ft = world.resource_mut::<FrameTimings>();
    ft.interval_ms = interval;
    ft.total_ms = total;
    crate::debug_views::update(world);
    let mut store = world.remove_resource::<FixedAdvanceMeasure>().unwrap();
    world
        .resource_mut::<UiTree>()
        .layout(Vec2::new(1280.0, 720.0), &mut store);
    world.insert_resource(store);
}

fn text(tree: &UiTree, id: WidgetId) -> String {
    tree.text(id)
        .map(|text| text.spans.iter().map(|span| span.text.as_str()).collect())
        .unwrap_or_default()
}

/// The HUD's rows: (label, value).
fn rows(world: &World) -> Vec<(String, String)> {
    let window = world.resource::<DebugHud>().window.as_ref().expect("open");
    let tree = world.resource::<UiTree>();
    let labels = tree.children(window.columns[0]);
    let values = tree.children(window.columns[1]);
    labels
        .iter()
        .zip(values)
        .map(|(label, value)| (text(tree, *label), text(tree, *value)))
        .collect()
}

#[test]
fn new_hud_keeps_its_defaults() {
    let hud = DebugHud::new();
    assert!(!hud.enabled);
    assert_eq!(hud.corner, HudCorner::TopLeft);
    assert_eq!(hud.font_id, crate::ENGINE_FONT_ID);
    assert_eq!((hud.font_size, hud.line_height), (15.0, 20.0));
    assert_eq!(hud.color, crate::debug_views::VALUE);
    assert_eq!(hud.background, crate::debug_views::PANEL);
    assert_eq!(
        (hud.outline_color, hud.outline_px),
        (crate::debug_views::BORDER, 1.0)
    );
    assert_eq!(hud.padding_px, 8.0);
    assert_eq!(hud.refresh_interval_ms, 500.0);
    assert_eq!(hud.ewma_alpha, 0.1);
    assert_eq!(hud.rendered_height_px(), 0.0);
    assert_eq!(hud.window(), None);
}

#[test]
fn toggle_flips_enabled() {
    let mut hud = DebugHud::new();
    assert!(!hud.enabled);
    hud.toggle();
    assert!(hud.enabled);
    hud.toggle();
    assert!(!hud.enabled);
}

#[test]
fn action_map_hud_toggle_uses_engine_action() {
    let mut world = World::new();
    let mut input = InputState::new();
    input.key_down(KeyCode::F4);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(DebugHud::new());

    hud_toggle_system(&mut world);

    assert!(world.get_resource::<DebugHud>().unwrap().enabled);
}

#[test]
fn the_rows_list_fps_then_the_built_in_rows_then_a_providers_in_columns() {
    let mut world = views_world();
    world.insert_resource(RenderCounts {
        entities: 42,
        sprite_instances: 128,
        ui_commands: 17,
        ui_layout_us: 42,
        ui_roots_laid_out: 1,
        ui_nodes_laid_out: 9,
        text_batches: 2,
        text_batches_prepared: 1,
        text_full_frames: 3,
        text_atlas_full: 0,
        world_labels: 5,
        world_labels_culled: 1,
        world_labels_dropped: 2,
        text_label_glyphs: 37,
        text_layouts: 12,
        ..RenderCounts::default()
    });
    world.insert_resource(HudActiveState("gameplay".into()));
    world.resource_mut::<FrameTimings>().system_timings = vec![("sys_a".into(), 1.0)];
    world.resource_mut::<DebugHud>().add_row(|_| {
        vec![HudRow {
            label: "extra",
            value: "value".into(),
        }]
    });
    frame(&mut world, None, 0.0);
    let rows = rows(&world);
    let labels: Vec<&str> = rows.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        ["fps", "draw", "cpu", "ui", "text", "labels", "extra"],
        "M58 splits the ui row in three"
    );
    assert_eq!(rows[1].1, "42 ents  128 spr");
    assert_eq!(rows[3].1, "17 cmds layout 42us 1r/9n");
    assert_eq!(rows[4].1, "2b 1prep full 3 atlas_full 0 lay 12");
    assert_eq!(rows[5].1, "5/1c/2d lg 37");
    assert_eq!(rows[6].1, "value");
    let all = rows
        .iter()
        .map(|(label, value)| format!("{label} {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    for absent in ["player", "state", "gameplay", "sys_a"] {
        assert!(!all.contains(absent), "{absent} in\n{all}");
    }
}

#[test]
fn the_rows_refresh_at_the_first_interval_then_every_500_ms_of_interval() {
    let mut world = views_world();
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    world.resource_mut::<DebugHud>().add_row(move |_| {
        counter.set(counter.get() + 1);
        vec![HudRow {
            label: "tick",
            value: counter.get().to_string(),
        }]
    });
    frame(&mut world, None, 50.0);
    assert_eq!(rows(&world).last().unwrap().1, "1", "the first frame shown");
    assert_eq!(rows(&world)[0].1, MISSING, "no interval yet");
    frame(&mut world, Some(100.0), 900.0);
    assert_eq!(calls.get(), 2, "the first interval refreshes at once");
    assert_ne!(rows(&world)[0].1, MISSING);
    for _ in 0..4 {
        // A slow frame's work is not its interval: 400 ms of interval.
        frame(&mut world, Some(100.0), 900.0);
    }
    assert_eq!(calls.get(), 2);
    frame(&mut world, Some(100.0), 1.0);
    assert_eq!(calls.get(), 3, "at 500 ms of interval");
    assert_eq!(rows(&world).last().unwrap().1, "3");
}

#[test]
fn fps_comes_from_the_interval_and_the_ms_from_the_frame_total() {
    let mut world = views_world();
    for _ in 0..200 {
        frame(&mut world, Some(20.0), 5.0);
    }
    let hud = world.resource::<DebugHud>();
    assert!(
        (hud.interval_ewma - 20.0).abs() < 1e-3,
        "{}",
        hud.interval_ewma
    );
    assert!((hud.total_ewma - 5.0).abs() < 1e-3, "{}", hud.total_ewma);
    assert_eq!(rows(&world)[0], ("fps".into(), " 50.0   5.00ms".into()));
}

#[test]
fn the_graph_redraws_every_frame_six_vertices_an_interval_up_to_240() {
    let mut world = views_world();
    for _ in 0..10 {
        frame(&mut world, Some(10.0), 1.0);
    }
    frame(&mut world, None, 1.0);
    let graph = world
        .resource::<DebugHud>()
        .window
        .as_ref()
        .unwrap()
        .graph
        .unwrap();
    assert_eq!(
        world.resource::<UiTree>().mesh_vertices(graph).len(),
        60 + 12,
        "a frame with no interval adds no bar; two guide lines"
    );
    for _ in 0..300 {
        frame(&mut world, Some(20.0), 1.0);
    }
    let vertices = world.resource::<UiTree>().mesh_vertices(graph);
    assert_eq!(vertices.len(), 240 * 6 + 12);
    assert!(
        vertices[..240 * 6]
            .iter()
            .all(|vertex| vertex.color == crate::debug_views::YELLOW),
        "20 ms bars are over 16.7 and up to 25: yellow"
    );
}

#[test]
fn rendered_height_is_the_windows_height_and_padding_in_physical_pixels() {
    let mut world = views_world();
    world.insert_resource(tungsten_core::ScaleFactor::default());
    // The dock takes the window's content height at the second frame and
    // the HUD reads it at the third.
    for _ in 0..3 {
        frame(&mut world, None, 1.0);
    }
    let root = world.resource::<DebugHud>().window().unwrap();
    let height = world.resource::<UiTree>().rect(root).unwrap().height();
    assert!(height > 0.0);
    assert_eq!(
        world.resource::<DebugHud>().rendered_height_px(),
        height + 8.0
    );
    world.resource_mut::<DebugHud>().enabled = false;
    frame(&mut world, None, 1.0);
    assert_eq!(world.resource::<DebugHud>().rendered_height_px(), 0.0);
    assert_eq!(world.resource::<DebugHud>().window(), None);
    assert!(!world.resource::<UiTree>().contains(root), "closed");
}

#[test]
fn a_long_custom_row_holds_the_hud_to_its_width_cap_on_one_line() {
    // §2.7's probe, the HUD's half: 45% of 1280 is 576, and the label
    // column takes at most 40% of that.
    let mut world = views_world();
    let label: &'static str = Box::leak("L".repeat(200).into_boxed_str());
    world.resource_mut::<DebugHud>().add_row(move |_| {
        vec![HudRow {
            label,
            value: "V".repeat(200),
        }]
    });
    frame(&mut world, None, 1.0);
    frame(&mut world, None, 1.0);
    let window = world.resource::<DebugHud>().window.as_ref().unwrap();
    let tree = world.resource::<UiTree>();
    let root = tree.rect(window.root).unwrap();
    assert!((root.width() - 576.0).abs() < 1e-3, "{root:?}");
    let labels = tree.rect(window.columns[0]).unwrap();
    assert!((labels.width() - 230.4).abs() < 1e-3, "{labels:?}");
    let values = tree.rect(window.columns[1]).unwrap();
    assert!(
        (labels.width() + values.width() - 576.0).abs() < 1e-3,
        "the value column takes the rest: {values:?}"
    );
    for column in &window.columns {
        let cell = *tree.children(*column).last().unwrap();
        assert_eq!(tree.rect(cell).unwrap().height(), 20.0, "one line");
        assert_eq!(text(tree, cell).chars().count(), 200, "the text kept whole");
    }
}

// ---- M58 step 4: the HUD ----

/// The HUD's window.
fn window(world: &World) -> &ViewWindow {
    world.resource::<DebugHud>().window.as_ref().unwrap()
}

#[test]
fn the_graph_is_240_units_wide_in_a_narrow_and_a_wide_window_and_stays_in_view() {
    let mut world = views_world();
    for _ in 0..3 {
        frame(&mut world, Some(10.0), 1.0);
    }
    let narrow = world
        .resource::<UiTree>()
        .rect(window(&world).root)
        .unwrap();
    let graph = window(&world).graph.unwrap();
    assert_eq!(
        world.resource::<UiTree>().rect(graph).unwrap().width(),
        GRAPH_WIDTH
    );

    let long: &'static str = Box::leak("L".repeat(120).into_boxed_str());
    world.resource_mut::<DebugHud>().add_row(move |_| {
        (0..60)
            .map(|_| HudRow {
                label: long,
                value: "V".repeat(200),
            })
            .collect()
    });
    world.resource_mut::<DebugHud>().refresh_interval_ms = 0.0;
    for _ in 0..3 {
        frame(&mut world, Some(10.0), 1.0);
    }
    let tree = world.resource::<UiTree>();
    let wide = tree.rect(window(&world).root).unwrap();
    assert!(wide.width() > narrow.width(), "{narrow:?} {wide:?}");
    assert_eq!(tree.rect(graph).unwrap().width(), GRAPH_WIDTH);
    assert_eq!(tree.rect(graph).unwrap().height(), 4.0 * 20.0);

    let body = window(&world).body;
    world
        .resource_mut::<UiTree>()
        .set_scroll(body, f32::INFINITY);
    frame(&mut world, Some(10.0), 1.0);
    let tree = world.resource::<UiTree>();
    assert!(tree.scroll(body).unwrap() > 0.0, "scrolled to its end");
    let shown = tree.visual_rect(graph).unwrap();
    let root = tree.rect(window(&world).root).unwrap();
    assert!(
        shown.min.y >= root.min.y && shown.max.y <= root.max.y,
        "in the fixed header: {shown:?} in {root:?}"
    );
}

#[test]
fn the_graph_draws_240_bars_on_the_ramp_and_two_guide_lines_over_them() {
    let mut out = Vec::new();
    let series = [16.6, 16.8, 24.9, 25.1, 33.2, 33.4];
    crate::debug_views::graph_vertices(&series, 80.0, &mut out);
    assert_eq!(out.len(), 6 * 6 + 12);
    let colors: Vec<[u8; 4]> = out[..36].chunks(6).map(|bar| bar[0].color).collect();
    assert_eq!(colors, [GREEN, YELLOW, YELLOW, ORANGE, ORANGE, RED]);
    // The tallest bar, 33.4, tops the graph; the guides sit at 16.7 and
    // 33.3 against it, one unit of 80 thick, over the bars.
    for (guide, ms) in out[36..].chunks(6).zip([16.7_f32, 33.3]) {
        // Kept inside the graph: the 33.3 line meets its top edge.
        let half = 0.5 / 80.0;
        let centre = f32::midpoint(guide[0].position.y, guide[2].position.y);
        let expected = (1.0 - ms / 33.4).clamp(half, 1.0 - half);
        assert!((centre - expected).abs() < 1e-4, "{ms}: {centre}");
        assert!((guide[2].position.y - guide[0].position.y - 1.0 / 80.0).abs() < 1e-6);
        assert_eq!((guide[0].position.x, guide[1].position.x), (0.0, 1.0));
        assert_eq!(guide[0].color, [166, 166, 176, 128]);
    }
    let full: Vec<f32> = (0..300).map(|i| (i % 40) as f32).collect();
    crate::debug_views::graph_vertices(&full, 80.0, &mut out);
    assert_eq!(out.len(), 240 * 6 + 12, "240 bars at most");
}

#[test]
fn the_stats_are_the_least_mean_most_and_1_percent_low_or_a_dash() {
    use crate::debug_views::graph_stats;
    assert_eq!(graph_stats(&[]), None);
    // 200 intervals: 1% is the slowest two, 30 and 40.
    let mut series = vec![10.0; 198];
    series.extend([30.0, 40.0]);
    let [min, mean, max, low] = graph_stats(&series).unwrap();
    assert_eq!((min, max), (10.0, 40.0));
    assert!((mean - 10.25).abs() < 1e-4, "{mean}");
    assert_eq!(low, 35.0);
    assert_eq!(graph_stats(&[5.0, 7.0]).unwrap()[3], 7.0, "at least one");

    let mut world = views_world();
    frame(&mut world, None, 1.0);
    let text_of =
        |world: &World, i: usize| text(world.resource::<UiTree>(), window(world).stats[i]);
    for i in 0..4 {
        assert_eq!(text_of(&world, i), MISSING, "before the first interval");
    }
    frame(&mut world, Some(20.0), 1.0);
    assert_eq!(text_of(&world, 0), "20.00ms");
    let stat = window(&world).stats[3];
    let style = world
        .resource::<UiTree>()
        .overrides(stat)
        .unwrap()
        .text
        .clone();
    assert_eq!((style.color, style.weight), (Some(YELLOW), Some(600)));
}

#[test]
fn both_averages_start_from_the_first_interval() {
    let mut world = views_world();
    frame(&mut world, None, 3.0);
    assert!(!world.resource::<DebugHud>().seeded);
    frame(&mut world, Some(40.0), 8.0);
    let hud = world.resource::<DebugHud>();
    assert_eq!(
        (hud.interval_ewma, hud.total_ewma),
        (40.0, 8.0),
        "not from 0"
    );
    assert_eq!(rows(&world)[0].1, " 25.0   8.00ms");
}

#[test]
fn the_fps_rows_time_takes_the_ramps_colour() {
    for (total, color) in [(5.0, GREEN), (20.0, YELLOW), (30.0, ORANGE), (40.0, RED)] {
        let mut world = views_world();
        frame(&mut world, None, total);
        frame(&mut world, Some(16.0), total);
        let value = world
            .resource::<UiTree>()
            .children(window(&world).columns[1])[0];
        let spans = world
            .resource::<UiTree>()
            .text(value)
            .unwrap()
            .spans
            .clone();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].color, Some(color), "{total} ms");
        assert_eq!(spans[1].weight, Some(600));
        assert_eq!(spans[0].color, None, "the fps in the value colour");
    }
}
