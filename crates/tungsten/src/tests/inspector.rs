use super::*;
use std::path::PathBuf;
use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
use tungsten_core::input::{KeyCode, MouseButton};

fn make_world() -> World {
    let mut world = World::new();
    world.insert_resource(InspectorState::new_with_defaults());
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
    world
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
        );
    let e = world.spawn();
    world.insert(e, Transform::from_position(pos));
    world.insert(e, Sprite::new(id));
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
fn default_registers_canonical_components() {
    let state = InspectorState::new_with_defaults();
    assert_eq!(state.registered_len(), 6);
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

    assert!(world
        .get_resource::<InspectorState>()
        .unwrap()
        .selected
        .is_none());
}

#[test]
fn pick_noop_when_disabled() {
    let mut world = make_world();
    let _e = spawn_sprite(&mut world, "e", Vec2::ZERO, 16);
    press_mouse3_at(&mut world, 0.0, 0.0);

    inspector_pick_system(&mut world);

    assert!(world
        .get_resource::<InspectorState>()
        .unwrap()
        .selected
        .is_none());
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

    assert!(world
        .get_resource::<InspectorState>()
        .unwrap()
        .selected
        .is_none());
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

    assert!(world
        .get_resource::<InspectorState>()
        .unwrap()
        .selected
        .is_none());
}

#[test]
fn compose_renders_hint_message_when_enabled() {
    let mut state = InspectorState::new_with_defaults();
    state.enabled = true;
    let world = World::new();
    let sections = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    assert!(!sections.is_empty());
    assert!(sections
        .iter()
        .any(|s| s.content.contains("mouse 3 to pick")));
}

#[test]
fn compose_renders_registered_rows_for_selected_entity() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Tag::new("hero"));
    world.insert(e, Transform::from_position(Vec2::new(3.0, 5.0)));

    let mut state = InspectorState::new_with_defaults();
    state.enabled = true;
    state.selected = Some(e);

    let sections = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    let joined: String = sections
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    // Compact layout: no bracketed headers or blank separators.
    assert!(joined.contains("Tag"));
    assert!(joined.contains("hero"));
    assert!(joined.contains("Transform"));
    assert!(joined.contains("pos"));
    assert!(!joined.contains("[ Tag ]"));
    assert!(!joined.contains("────"));
}

#[test]
fn compose_returns_empty_when_disabled() {
    let mut state = InspectorState::new_with_defaults();
    let world = World::new();
    let sections = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    assert!(sections.is_empty());
}

#[test]
fn compose_throttles_rebuild_between_intervals() {
    let mut world = World::new();
    let e = world.spawn();
    world.insert(e, Tag::new("hero"));

    let mut state = InspectorState::new_with_defaults();
    state.enabled = true;
    state.selected = Some(e);
    state.refresh_interval_ms = 100.0;

    let first = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    let first_content = first.last().unwrap().content.clone();

    // Cache wins inside 100 ms interval.
    world.get_mut::<Tag>(e).unwrap().name = "villain".into();
    let second = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    assert_eq!(second.last().unwrap().content, first_content);

    let third = compose_inspector_text_section(&mut state, &world, (800, 600), 200.0);
    assert!(third.last().unwrap().content.contains("villain"));
}

#[test]
fn compose_rebuilds_immediately_when_selection_changes() {
    // Regression: selection edge bypasses 500 ms throttle.
    let mut world = World::new();
    let hero = world.spawn();
    world.insert(hero, Tag::new("hero"));
    let villain = world.spawn();
    world.insert(villain, Tag::new("villain"));

    let mut state = InspectorState::new_with_defaults();
    state.enabled = true;
    state.selected = Some(hero);
    state.refresh_interval_ms = 500.0;

    let first = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    assert!(first.last().unwrap().content.contains("hero"));

    state.selected = Some(villain);
    let second = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
    assert!(second.last().unwrap().content.contains("villain"));
    assert!(!second.last().unwrap().content.contains("hero"));
}
