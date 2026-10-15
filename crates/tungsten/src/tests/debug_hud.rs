use std::cell::Cell;
use std::rc::Rc;

use glam::Vec2;
use tungsten_core::ActionMap;
use tungsten_core::input::{InputState, KeyCode};
use tungsten_core::ui::{FixedAdvanceMeasure, UiTree};

use super::*;
use crate::app::WindowSize;

/// A world with what the views' frame reads, the HUD on.
fn views_world() -> World {
    let mut world = World::new();
    world.insert_resource(UiTree::new());
    world.insert_resource(WindowSize {
        width: 1280,
        height: 720,
    });
    world.insert_resource(FrameTimings::new());
    let mut hud = DebugHud::new();
    hud.enabled = true;
    world.insert_resource(hud);
    world
}

/// The views' frame after a frame `interval` ms apart from the one before
/// that took `total` ms, then the layout.
fn frame(world: &mut World, interval: Option<f32>, total: f32) {
    let ft = world.resource_mut::<FrameTimings>();
    ft.interval_ms = interval;
    ft.total_ms = total;
    crate::debug_views::update(world);
    world.resource_mut::<UiTree>().layout(
        Vec2::new(1280.0, 720.0),
        &mut FixedAdvanceMeasure::default(),
    );
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
    assert_eq!(hud.corner, HudCorner::TopRight);
    assert_eq!(hud.font_id, crate::ENGINE_FONT_ID);
    assert_eq!((hud.font_size, hud.line_height), (26.0, 30.0));
    assert_eq!(hud.color, [240, 240, 240, 240]);
    assert_eq!(hud.background, [16, 16, 20, 200]);
    assert_eq!((hud.outline_color, hud.outline_px), ([0, 0, 0, 220], 1.0));
    assert_eq!(hud.padding_px, 10.0);
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
    assert_eq!(labels, ["fps", "draw", "cpu", "ui", "extra"]);
    assert_eq!(rows[1].1, "42 ents  128 spr");
    assert_eq!(
        rows[3].1,
        "17 cmds layout 42us 1r/9n text 2b 1prep full 3 atlas_full 0 labels 5/1c/2d lg 37 lay 12"
    );
    assert_eq!(rows[4].1, "value");
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
fn the_rows_refresh_every_500_ms_of_frame_interval() {
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
    for _ in 0..4 {
        // A slow frame's work is not its interval: 400 ms of interval.
        frame(&mut world, Some(100.0), 900.0);
    }
    assert_eq!(calls.get(), 1);
    frame(&mut world, Some(100.0), 1.0);
    assert_eq!(calls.get(), 2, "at 500 ms of interval");
    assert_eq!(rows(&world).last().unwrap().1, "2");
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
        60,
        "a frame with no interval adds no bar"
    );
    for _ in 0..300 {
        frame(&mut world, Some(20.0), 1.0);
    }
    let vertices = world.resource::<UiTree>().mesh_vertices(graph);
    assert_eq!(vertices.len(), 240 * 6);
    assert!(
        vertices
            .iter()
            .all(|vertex| vertex.color == crate::debug_views::GRAPH_SLOW),
        "20 ms bars are over 16.7"
    );
}

#[test]
fn rendered_height_is_the_windows_height_and_padding_in_physical_pixels() {
    let mut world = views_world();
    world.insert_resource(tungsten_core::ScaleFactor::default());
    frame(&mut world, None, 1.0);
    frame(&mut world, None, 1.0);
    let root = world.resource::<DebugHud>().window().unwrap();
    let height = world.resource::<UiTree>().rect(root).unwrap().height();
    assert!(height > 0.0);
    assert_eq!(
        world.resource::<DebugHud>().rendered_height_px(),
        height + 10.0
    );
    world.resource_mut::<DebugHud>().enabled = false;
    frame(&mut world, None, 1.0);
    assert_eq!(world.resource::<DebugHud>().rendered_height_px(), 0.0);
    assert_eq!(world.resource::<DebugHud>().window(), None);
    assert!(!world.resource::<UiTree>().contains(root), "closed");
}
