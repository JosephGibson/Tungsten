use super::*;
use std::path::PathBuf;

use crate::app::WindowSize;
use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
use tungsten_core::input::{KeyCode, MouseButton};
use tungsten_core::ui::UiTree;

fn make_world() -> World {
    let mut world = World::new();
    world.insert_resource(InspectorState::new());
    world.insert_resource(default_inspect_registry());
    world.insert_resource(CameraState::new());
    world.insert_resource(WindowSize {
        width: 800,
        height: 600,
    });
    world.insert_resource(AssetRegistry::new());
    world
}

/// Register square sprite and spawn at top-left position.
fn spawn_sprite(world: &mut World, id: &str, pos: Vec2, size: u32) -> Entity {
    let asset_id = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry missing")
        .register_sprite(
            id.to_string(),
            FilterMode::Nearest,
            size,
            size,
            PathBuf::from(format!("test/{id}.png")),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
    let e = world.spawn();
    world.insert(e, Transform::from_position(pos));
    world.insert(e, Sprite::new(asset_id));
    world.insert(e, Visibility::default());
    e
}

fn press_mouse3_at(world: &mut World, x: f32, y: f32) {
    let mut input = InputState::new();
    input.update_cursor_position(x, y);
    input.mouse_down(MouseButton::Middle);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
}

#[test]
#[allow(deprecated)]
fn default_registers_canonical_components() {
    let state = InspectorState::new_with_defaults();
    assert_eq!(state.registered_len(), 6);
}

#[test]
fn default_inspect_registry_registers_canonical_components() {
    assert_eq!(default_inspect_registry().len(), 6);
}

#[test]
fn toggle_on_f3_action_flips_enabled() {
    let mut world = make_world();
    let mut input = InputState::new();
    input.key_down(KeyCode::F3);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());

    inspector_toggle_system(&mut world);

    assert!(world.get_resource::<InspectorState>().unwrap().enabled);
}

#[test]
fn pick_selects_sprite_under_cursor_on_mouse3_edge() {
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

    let target = spawn_sprite(&mut world, "target", Vec2::new(100.0, 100.0), 16);
    let _decoy = spawn_sprite(&mut world, "decoy", Vec2::new(500.0, 500.0), 16);

    press_mouse3_at(&mut world, 108.0, 108.0);
    inspector_pick_system(&mut world);

    let selected = world.get_resource::<InspectorState>().unwrap().selected;
    assert_eq!(selected, Some(target));
}

#[test]
fn pick_clears_selection_when_cursor_is_in_empty_space() {
    // Regression: empty-space click clears instead of nearest-picking.
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

    let player = spawn_sprite(&mut world, "player", Vec2::new(100.0, 100.0), 16);
    world.get_resource_mut::<InspectorState>().unwrap().selected = Some(player);

    press_mouse3_at(&mut world, 600.0, 400.0);
    inspector_pick_system(&mut world);

    assert_eq!(
        world.get_resource::<InspectorState>().unwrap().selected,
        None
    );
}

#[test]
fn pick_prefers_smaller_aabb_when_sprites_overlap() {
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

    let _bg = spawn_sprite(&mut world, "bg", Vec2::new(0.0, 0.0), 200);
    let fg = spawn_sprite(&mut world, "fg", Vec2::new(90.0, 90.0), 20);

    press_mouse3_at(&mut world, 100.0, 100.0);
    inspector_pick_system(&mut world);

    assert_eq!(
        world.get_resource::<InspectorState>().unwrap().selected,
        Some(fg)
    );
}

#[test]
fn pick_hits_physics_body_without_sprite_component() {
    // Regression: custom-extracted player still pickable via physics footprint.
    use tungsten_core::physics::{Collider, Position};

    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

    let body = world.spawn();
    world.insert(body, Position(Vec2::new(200.0, 200.0)));
    world.insert(body, Collider::aabb(Vec2::splat(16.0)));

    let _decoy = spawn_sprite(&mut world, "decoy", Vec2::new(800.0, 800.0), 16);

    press_mouse3_at(&mut world, 205.0, 205.0);
    inspector_pick_system(&mut world);

    assert_eq!(
        world.get_resource::<InspectorState>().unwrap().selected,
        Some(body)
    );
}

#[test]
fn pick_hits_circle_collider_bounding_box() {
    use tungsten_core::physics::{Collider, Position};

    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(100.0, 100.0)));
    world.insert(ball, Collider::circle(8.0));

    press_mouse3_at(&mut world, 102.0, 98.0);
    inspector_pick_system(&mut world);

    assert_eq!(
        world.get_resource::<InspectorState>().unwrap().selected,
        Some(ball)
    );
}

#[test]
fn pick_tests_a_body_where_the_last_frame_drew_it() {
    // The world `pre_update` sees at an accumulator rollover: the clock has
    // set the new frame's alpha, the steps have not run, and the body's
    // `Transform` holds the point the last frame's `physics_sync` drew.
    use tungsten_core::Time;
    use tungsten_core::physics::{Collider, Position, PrevPosition};

    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;
    let mut time = Time::new();
    time.advance_frame(time.fixed_step() * 0.1);
    assert!((time.alpha() - 0.1).abs() < 1e-6, "{}", time.alpha());
    world.insert_resource(time);

    let body = world.spawn();
    world.insert(body, PrevPosition(Vec2::new(0.0, 50.0)));
    world.insert(body, Position(Vec2::new(100.0, 50.0)));
    world.insert(body, Transform::from_position(Vec2::new(90.0, 50.0)));
    world.insert(body, Collider::aabb(Vec2::splat(4.0)));

    for (x, picked) in [(90.0, Some(body)), (100.0, None), (10.0, None)] {
        press_mouse3_at(&mut world, x, 50.0);
        inspector_pick_system(&mut world);
        assert_eq!(
            world.get_resource::<InspectorState>().unwrap().selected,
            picked,
            "a click at x {x}"
        );
    }
}

#[test]
fn pick_skips_invisible_sprites() {
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

    let hidden = spawn_sprite(&mut world, "hidden", Vec2::new(100.0, 100.0), 16);
    world.get_mut::<Visibility>(hidden).unwrap().visible = false;

    press_mouse3_at(&mut world, 108.0, 108.0);
    inspector_pick_system(&mut world);

    assert_eq!(
        world.get_resource::<InspectorState>().unwrap().selected,
        None
    );
}

#[test]
fn pick_ignores_hover_without_mouse3_edge() {
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;
    world.insert_resource(ActionMap::default_map());

    let _e = spawn_sprite(&mut world, "e", Vec2::new(100.0, 100.0), 16);

    let mut input = InputState::new();
    input.update_cursor_position(108.0, 108.0);
    world.insert_resource(input);

    inspector_pick_system(&mut world);

    assert!(
        world
            .get_resource::<InspectorState>()
            .unwrap()
            .selected
            .is_none()
    );
}

#[test]
fn pick_noop_when_disabled() {
    let mut world = make_world();
    let _e = spawn_sprite(&mut world, "e", Vec2::ZERO, 16);
    press_mouse3_at(&mut world, 0.0, 0.0);

    inspector_pick_system(&mut world);

    assert!(
        world
            .get_resource::<InspectorState>()
            .unwrap()
            .selected
            .is_none()
    );
}

#[test]
fn pick_skips_when_cursor_is_outside_window() {
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;
    world.insert_resource(ActionMap::default_map());
    let _e = spawn_sprite(&mut world, "e", Vec2::ZERO, 16);

    let mut input = InputState::new();
    input.mouse_down(MouseButton::Middle);
    world.insert_resource(input);

    inspector_pick_system(&mut world);

    assert!(
        world
            .get_resource::<InspectorState>()
            .unwrap()
            .selected
            .is_none()
    );
}

#[test]
fn stale_selection_is_cleared_before_picking() {
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;
    world.insert_resource(ActionMap::default_map());

    let doomed = spawn_sprite(&mut world, "doomed", Vec2::ZERO, 16);
    world.get_resource_mut::<InspectorState>().unwrap().selected = Some(doomed);
    world.despawn(doomed);

    let input = InputState::new();
    world.insert_resource(input);

    inspector_pick_system(&mut world);

    assert!(
        world
            .get_resource::<InspectorState>()
            .unwrap()
            .selected
            .is_none()
    );
}

/// The views' frame after a frame `interval` ms apart from the one before.
fn views_frame(world: &mut World, interval: Option<f32>) {
    if !world.has_resource::<UiTree>() {
        world.insert_resource(UiTree::new());
    }
    let mut timings = crate::telemetry::FrameTimings::new();
    timings.interval_ms = interval;
    world.insert_resource(timings);
    crate::debug_views::update(world);
}

/// The window's header and its rows' cells.
fn window_texts(world: &World) -> (String, Vec<Vec<String>>) {
    let window = world
        .resource::<InspectorState>()
        .window
        .as_ref()
        .expect("open");
    let tree = world.resource::<UiTree>();
    let text = |id: WidgetId| -> String {
        tree.text(id)
            .map(|text| text.spans.iter().map(|span| span.text.as_str()).collect())
            .unwrap_or_default()
    };
    let columns: Vec<&[WidgetId]> = window
        .columns
        .iter()
        .map(|column| tree.children(*column))
        .collect();
    let rows = (0..columns[0].len())
        .map(|i| columns.iter().map(|column| text(column[i])).collect())
        .collect();
    (text(window.header.unwrap()), rows)
}

#[test]
fn the_window_says_mouse_3_to_pick_with_nothing_selected() {
    let mut world = make_world();
    world.resource_mut::<InspectorState>().enabled = true;
    views_frame(&mut world, None);
    let (header, rows) = window_texts(&world);
    assert_eq!(header, "mouse 3 to pick");
    assert!(rows.is_empty());
}

#[test]
#[allow(deprecated)]
fn the_window_lists_the_entity_then_section_row_and_value_columns() {
    let mut world = make_world();
    let e = world.spawn();
    world.insert(e, Tag::new("hero"));
    world.insert(e, Transform::from_position(Vec2::new(3.0, 5.0)));
    let mut state = InspectorState::new();
    state.register::<Tag>("Again");
    state.enabled = true;
    state.selected = Some(e);
    world.insert_resource(state);
    views_frame(&mut world, None);
    let (header, rows) = window_texts(&world);
    assert_eq!(header, format!("entity {e}"));
    assert_eq!(rows[0], ["Tag", "name", "hero"]);
    assert!(
        rows.iter()
            .any(|row| row[0] == "Transform" && row[1] == "pos")
    );
    assert_eq!(
        rows.last().unwrap()[0],
        "Again",
        "the registry's rows first, then the state's own"
    );

    let bare = world.spawn();
    world.resource_mut::<InspectorState>().selected = Some(bare);
    views_frame(&mut world, None);
    let (header, rows) = window_texts(&world);
    assert_eq!(header, format!("entity {bare}"));
    assert_eq!(rows, [["(no registered components)", "", ""]]);
}

#[test]
fn a_new_selection_refreshes_at_once_and_otherwise_the_interval_does() {
    let mut world = make_world();
    let hero = world.spawn();
    world.insert(hero, Tag::new("hero"));
    let villain = world.spawn();
    world.insert(villain, Tag::new("villain"));
    {
        let state = world.resource_mut::<InspectorState>();
        state.enabled = true;
        state.selected = Some(hero);
        state.refresh_interval_ms = 100.0;
    }
    views_frame(&mut world, None);
    assert_eq!(window_texts(&world).1[0][2], "hero");

    world.get_mut::<Tag>(hero).unwrap().name = "renamed".into();
    views_frame(&mut world, Some(60.0));
    assert_eq!(window_texts(&world).1[0][2], "hero", "inside the interval");
    views_frame(&mut world, Some(60.0));
    assert_eq!(window_texts(&world).1[0][2], "renamed", "past it");

    world.resource_mut::<InspectorState>().selected = Some(villain);
    views_frame(&mut world, Some(1.0));
    let (header, rows) = window_texts(&world);
    assert_eq!(header, format!("entity {villain}"), "at once");
    assert_eq!(rows[0][2], "villain");
}

#[test]
fn sprite_rows_print_the_asset_name() {
    let mut world = make_world();
    let e = spawn_sprite(&mut world, "hero", Vec2::ZERO, 16);
    let registry = default_inspect_registry();
    let (_, read) = registry
        .iter()
        .find(|(label, _)| *label == "Sprite")
        .expect("Sprite is registered by default");
    let rows = read(&world, e);
    assert_eq!(rows[0], ("asset", "hero".to_string()));
    assert_eq!(rows.len(), 3);
}

#[test]
fn pick_follows_a_rotated_camera() {
    // The pick turned the cursor by the inverse of the camera's rotation:
    // at position (100, 50), zoom 2 and rotation 0.5 rad, the pixel where
    // (130, 70) draws mapped to (133.04, 35.56).
    let mut world = make_world();
    world.get_resource_mut::<InspectorState>().unwrap().enabled = true;
    {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::new(100.0, 50.0);
        camera.zoom = 2.0;
        camera.rotation = 0.5;
    }
    let target = spawn_sprite(&mut world, "target", Vec2::new(126.0, 66.0), 8);
    // (130, 70) relative to the camera, turned back by the rotation and
    // scaled by the zoom: the pixel it draws at.
    let (sin, cos) = 0.5_f32.sin_cos();
    let local = Vec2::new(30.0, 20.0);
    let screen = Vec2::new(
        local.x * cos + local.y * sin,
        -local.x * sin + local.y * cos,
    ) * 2.0;
    press_mouse3_at(&mut world, screen.x, screen.y);
    inspector_pick_system(&mut world);
    let selected = world.get_resource::<InspectorState>().unwrap().selected;
    assert_eq!(selected, Some(target));
}
