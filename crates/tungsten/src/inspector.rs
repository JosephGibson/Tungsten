//! Text-only entity inspector (M21, `F3`). Users register component types
//! that implement [`Inspectable`]; while the overlay is enabled, the entity
//! whose sprite or collider footprint contains the cursor is picked on
//! the `engine_inspector_pick` edge (default: middle mouse button /
//! "Mouse 3") and the compose helper renders every registered component's
//! labelled rows. The selection sticks until another pick edge fires or
//! the entity despawns, so the operator can read the dump without the
//! cursor moving the selection out from under them.
//!
//! Registration uses an opaque `InspectFn` closure captured at
//! `register::<T>(label)` time. The closure reads `world.get::<T>(entity)`
//! and falls back to an empty row list when the component is absent, so
//! rows for missing components simply don't appear.
//!
//! Picking is axis-aligned-bounding-box (AABB) containment evaluated
//! against two footprint sources per entity, in this order:
//!
//! 1. `Sprite + Transform + Visibility`, sized from the `AssetRegistry`
//!    sprite entry. Footprint uses top-left [`Transform::position`] +
//!    `asset.size * scale` so it matches the sprite shader.
//! 2. `Collider + Position` (physics bodies). Footprint is the collider's
//!    bounding box centered at `Position + offset`. Circles use
//!    `(radius, radius)` as half-extents. This is what catches entities
//!    whose visuals come from custom extract paths (marker components,
//!    animation-frame lookups) rather than the default `Sprite` draw path.
//!
//! When multiple footprints contain the cursor the smallest AABB wins —
//! approximates "topmost visual" without a full z-order walk. A click
//! that hits nothing clears the selection. Rotation is ignored in the hit
//! test (rotated sprites are approximated by their bounding AABB).
//! Selection is cleared on the next frame after the referenced entity is
//! despawned so the overlay never dereferences a stale id.

use glam::Vec2;

use tungsten_core::components::{Sprite, Tag, Transform, Visibility};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::physics::{Collider, Position, Shape, Velocity};
use tungsten_core::{AssetRegistry, CameraState, Entity, Inspectable, World};
use tungsten_render::TextSection;

use crate::app::WindowSize;
use crate::debug_hud::{anchor_text_block, HudCorner};

pub type InspectFn = Box<dyn Fn(&World, Entity) -> Vec<(&'static str, String)>>;

pub struct InspectorState {
    pub enabled: bool,
    pub selected: Option<Entity>,
    registered: Vec<(&'static str, InspectFn)>,
    pub corner: HudCorner,
    pub padding_px: f32,
    pub font_id: String,
    pub font_size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub outline_color: [u8; 4],
    pub outline_px: f32,
    /// Minimum wall-clock interval between rebuilds of the displayed text,
    /// in milliseconds. Defaults to 500 ms (2 Hz) so fast-changing values
    /// dwell long enough for the eye to read them; the cached sections
    /// are re-emitted in between.
    pub refresh_interval_ms: f32,
    cached_sections: Vec<TextSection>,
    cached_viewport: (u32, u32),
    cached_selected: Option<Entity>,
    time_since_refresh_ms: f32,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            enabled: false,
            selected: None,
            registered: Vec::new(),
            // BottomLeft so the inspector does not stomp on the systems
            // overlay (BottomRight) or the debug HUD (TopRight) when all
            // three are visible at once.
            corner: HudCorner::BottomLeft,
            padding_px: 12.0,
            font_id: "mono".to_string(),
            font_size: 28.0,
            line_height: 32.0,
            color: [240, 240, 240, 240],
            outline_color: [0, 0, 0, 220],
            outline_px: 1.5,
            refresh_interval_ms: 500.0,
            cached_sections: Vec::new(),
            cached_viewport: (0, 0),
            cached_selected: None,
            time_since_refresh_ms: f32::INFINITY,
        }
    }
}

impl InspectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-registers the engine's canonical inspectable components so fresh
    /// `App` instances produce useful inspector output without extra wiring.
    pub fn new_with_defaults() -> Self {
        let mut state = Self::default();
        state.register::<Tag>("Tag");
        state.register::<Transform>("Transform");
        state.register::<Visibility>("Visibility");
        state.register::<Sprite>("Sprite");
        state.register::<Position>("Position");
        state.register::<Velocity>("Velocity");
        state
    }

    pub fn register<T: 'static + Inspectable>(&mut self, label: &'static str) {
        self.registered.push((
            label,
            Box::new(|world: &World, entity: Entity| {
                world
                    .get::<T>(entity)
                    .map(|c| c.inspect_rows())
                    .unwrap_or_default()
            }),
        ));
    }

    pub fn registered_len(&self) -> usize {
        self.registered.len()
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

pub(crate) fn inspector_toggle_system(world: &mut World) {
    let pressed = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        actions.just_pressed(input, "engine_toggle_inspector")
    };
    if pressed {
        if let Some(state) = world.get_resource_mut::<InspectorState>() {
            state.toggle();
        }
    }
}

pub(crate) fn inspector_pick_system(world: &mut World) {
    let enabled = world
        .get_resource::<InspectorState>()
        .map(|s| s.enabled)
        .unwrap_or(false);
    if !enabled {
        return;
    }

    if let Some(selected) = world
        .get_resource::<InspectorState>()
        .and_then(|s| s.selected)
    {
        if !world.is_alive(selected) {
            if let Some(state) = world.get_resource_mut::<InspectorState>() {
                state.selected = None;
            }
        }
    }

    // Click-to-inspect on `engine_inspector_pick` (default: Mouse 3 /
    // middle button). Selection sticks until the next pick edge or until
    // the target entity despawns, so the operator can read the dump
    // without the cursor dragging the selection around.
    let (cursor, camera, viewport) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        if !actions.just_pressed(input, "engine_inspector_pick") {
            return;
        }
        let Some(cursor) = input.cursor_position() else {
            return;
        };
        let Some(camera) = world.get_resource::<CameraState>().copied() else {
            return;
        };
        let Some(viewport) = world.get_resource::<WindowSize>().copied() else {
            return;
        };
        (cursor, camera, viewport)
    };

    let world_cursor = screen_to_world(cursor, &camera, viewport);
    let picked = pick_entity_under_cursor(world, world_cursor);
    if let Some(state) = world.get_resource_mut::<InspectorState>() {
        state.selected = picked;
    }
}

/// AABB hit-test: return the entity whose footprint (sprite or collider)
/// contains `world_cursor`, preferring the smallest-area hit when
/// multiple overlap. See the module docstring for the footprint rules.
fn pick_entity_under_cursor(world: &World, world_cursor: Vec2) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;

    // Source 1: default-sprite footprint (Transform + Sprite + Visibility).
    if let Some(registry) = world.get_resource::<AssetRegistry>() {
        for (entity, transform, sprite) in world.query2::<Transform, Sprite>() {
            let visible = world
                .get::<Visibility>(entity)
                .map(|v| v.visible)
                .unwrap_or(false);
            if !visible {
                continue;
            }
            let Some(asset) = registry.get_sprite(&sprite.asset_id) else {
                continue;
            };
            let size = Vec2::new(
                asset.width as f32 * transform.scale.x.abs(),
                asset.height as f32 * transform.scale.y.abs(),
            );
            try_hit_aabb(&mut best, entity, world_cursor, transform.position, size);
        }
    }

    // Source 2: physics collider footprint. Catches the player, balls,
    // black holes, and any other gameplay entity whose visual is drawn by
    // a custom extract path rather than a default `Sprite` component.
    for (entity, collider, position) in world.query2::<Collider, Position>() {
        let centre = position.0 + collider.offset;
        let half = match collider.shape {
            Shape::Aabb { half_extents } => Vec2::new(half_extents.x.abs(), half_extents.y.abs()),
            Shape::Circle { radius } => Vec2::splat(radius.abs()),
        };
        try_hit_aabb(&mut best, entity, world_cursor, centre - half, half * 2.0);
    }

    best.map(|(e, _)| e)
}

fn try_hit_aabb(
    best: &mut Option<(Entity, f32)>,
    entity: Entity,
    cursor: Vec2,
    min: Vec2,
    size: Vec2,
) {
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let max = min + size;
    if cursor.x < min.x || cursor.x > max.x || cursor.y < min.y || cursor.y > max.y {
        return;
    }
    let area = size.x * size.y;
    if best.map(|(_, a)| area < a).unwrap_or(true) {
        *best = Some((entity, area));
    }
}

fn screen_to_world(cursor: (f32, f32), camera: &CameraState, _viewport: WindowSize) -> Vec2 {
    let zoom = camera.zoom.max(f32::EPSILON);
    let screen = Vec2::new(cursor.0, cursor.1);
    if camera.rotation == 0.0 {
        return camera.position + screen / zoom;
    }
    let (sin, cos) = (-camera.rotation).sin_cos();
    let scaled = screen / zoom;
    let rotated = Vec2::new(
        scaled.x * cos - scaled.y * sin,
        scaled.x * sin + scaled.y * cos,
    );
    camera.position + rotated
}

pub(crate) fn compose_inspector_text_section(
    state: &mut InspectorState,
    world: &World,
    viewport: (u32, u32),
    frame_ms: f32,
) -> Vec<TextSection> {
    if !state.enabled {
        state.cached_sections.clear();
        state.time_since_refresh_ms = f32::INFINITY;
        return Vec::new();
    }

    state.time_since_refresh_ms += frame_ms;
    // Cache reuse requires every invariant to match: the refresh window
    // must still be open, the viewport must match (layout depends on it),
    // and the selection must match (selection edges should appear
    // immediately — otherwise the operator clicks Mouse 3 and waits up to
    // `refresh_interval_ms` to see their pick).
    if state.time_since_refresh_ms < state.refresh_interval_ms
        && !state.cached_sections.is_empty()
        && state.cached_viewport == viewport
        && state.cached_selected == state.selected
    {
        return state.cached_sections.clone();
    }
    state.time_since_refresh_ms = 0.0;
    state.cached_viewport = viewport;
    state.cached_selected = state.selected;

    let lines = render_inspector_lines(state, world);
    let content = lines.join("\n");
    let (x, y) = anchor_text_block(
        state.corner,
        &lines,
        state.font_size,
        state.line_height,
        state.padding_px,
        viewport,
    );
    let main = TextSection {
        content,
        font_id: state.font_id.clone(),
        font_size: state.font_size,
        line_height: state.line_height,
        color: state.color,
        position: [x, y],
        bounds: None,
    };

    let sections = if state.outline_px > 0.0 {
        let ox = state.outline_px;
        let mut out = Vec::with_capacity(5);
        for (dx, dy) in [(-ox, 0.0), (ox, 0.0), (0.0, -ox), (0.0, ox)] {
            out.push(TextSection {
                content: main.content.clone(),
                font_id: main.font_id.clone(),
                font_size: main.font_size,
                line_height: main.line_height,
                color: state.outline_color,
                position: [x + dx, y + dy],
                bounds: None,
            });
        }
        out.push(main);
        out
    } else {
        vec![main]
    };
    state.cached_sections = sections.clone();
    sections
}

fn render_inspector_lines(state: &InspectorState, world: &World) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let selected = match state.selected {
        Some(e) if world.is_alive(e) => Some(e),
        _ => None,
    };

    match selected {
        None => {
            lines.push("mouse 3 to pick".to_string());
        }
        Some(entity) => {
            // Pre-scan once to size the section and row-label columns to
            // whichever components actually returned rows. Gives tight,
            // inline "Section Label Value" rows without per-section
            // headers or blank separator lines.
            let mut visible: Vec<(&str, Vec<(&'static str, String)>)> = Vec::new();
            for (section, inspect) in &state.registered {
                let rows = inspect(world, entity);
                if rows.is_empty() {
                    continue;
                }
                visible.push((section, rows));
            }
            lines.push(format!("entity {entity}"));
            if visible.is_empty() {
                lines.push("(no registered components)".to_string());
            } else {
                let section_w = visible
                    .iter()
                    .map(|(name, _)| name.chars().count())
                    .max()
                    .unwrap_or(0);
                let label_w = visible
                    .iter()
                    .flat_map(|(_, rows)| rows.iter().map(|(l, _)| l.chars().count()))
                    .max()
                    .unwrap_or(0);
                for (section, rows) in &visible {
                    for (row_label, value) in rows {
                        lines.push(format!(
                            "{section:<sw$} {row_label:<lw$} {value}",
                            sw = section_w,
                            lw = label_w
                        ));
                    }
                }
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
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

    /// Registers an `id`×`id` sized sprite asset and spawns a sprite entity
    /// at `pos` (treated as the top-left corner, matching the shader).
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

        // Sprite at (100, 100) with 16×16 footprint → covers (100..116, 100..116).
        let target = spawn_sprite(&mut world, "target", Vec2::new(100.0, 100.0), 16);
        // Decoy sprite well away from the cursor.
        let _decoy = spawn_sprite(&mut world, "decoy", Vec2::new(500.0, 500.0), 16);

        // Cursor inside the target sprite's AABB.
        press_mouse3_at(&mut world, 108.0, 108.0);
        inspector_pick_system(&mut world);

        let selected = world.get_resource::<InspectorState>().unwrap().selected;
        assert_eq!(selected, Some(target));
    }

    #[test]
    fn pick_clears_selection_when_cursor_is_in_empty_space() {
        // Regression for the example-01 bug: the previous nearest-distance
        // picker always picked *some* entity (typically the camera-followed
        // player), so clicking into empty sky still surfaced that entity.
        // The AABB-based picker must return no selection when the cursor is
        // outside every sprite's footprint.
        let mut world = make_world();
        world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

        let player = spawn_sprite(&mut world, "player", Vec2::new(100.0, 100.0), 16);
        // Seed a prior selection so we can observe the clear.
        world.get_resource_mut::<InspectorState>().unwrap().selected = Some(player);

        // Cursor is far from every sprite's AABB.
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

        // Big background sprite and a small foreground sprite, overlapping.
        let _bg = spawn_sprite(&mut world, "bg", Vec2::new(0.0, 0.0), 200);
        let fg = spawn_sprite(&mut world, "fg", Vec2::new(90.0, 90.0), 20);

        // Cursor lies inside both AABBs; smallest wins.
        press_mouse3_at(&mut world, 100.0, 100.0);
        inspector_pick_system(&mut world);

        assert_eq!(
            world.get_resource::<InspectorState>().unwrap().selected,
            Some(fg)
        );
    }

    #[test]
    fn pick_hits_physics_body_without_sprite_component() {
        // Regression for example 01: the platformer renders the player
        // through a custom extract path keyed off `CurrentSprite`, not the
        // default `Sprite` component. Picking must still find it via its
        // physics `Collider + Position` footprint.
        use tungsten_core::physics::{Collider, Position};

        let mut world = make_world();
        world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

        let body = world.spawn();
        // Player-like: centered at (200, 200), ±16 half-extents → covers
        // (184..216, 184..216). No Sprite component on the entity.
        world.insert(body, Position(Vec2::new(200.0, 200.0)));
        world.insert(body, Collider::aabb(Vec2::splat(16.0)));

        // A far-away distractor to pin down the selection.
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
        // Centered at (100, 100) with radius 8 → bounding AABB (92..108, 92..108).
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

        // Cursor sits over the sprite but no button is pressed.
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

        // No cursor position, even with Mouse 3 pressed, must not pick.
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
        // Compact layout: section name inline as the row prefix, no
        // bracketed header and no blank separator lines.
        assert!(joined.contains("Tag"));
        assert!(joined.contains("hero"));
        assert!(joined.contains("Transform"));
        assert!(joined.contains("pos"));
        // No leftover header characters from the old layout.
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

        // Change something observable between frames; cache still wins because
        // only 16 ms has elapsed of the 100 ms interval.
        world.get_mut::<Tag>(e).unwrap().name = "villain".into();
        let second = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
        assert_eq!(second.last().unwrap().content, first_content);

        // Advancing past the interval must rebuild.
        let third = compose_inspector_text_section(&mut state, &world, (800, 600), 200.0);
        assert!(third.last().unwrap().content.contains("villain"));
    }

    #[test]
    fn compose_rebuilds_immediately_when_selection_changes() {
        // Regression: at 500 ms throttle, clicking Mouse 3 on a new entity
        // used to leave the inspector showing the previous selection (or
        // the hint) for up to half a second. A selection edge must bypass
        // the throttle so picks feel responsive.
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

        // Well under the 500 ms throttle; a selection change must still
        // rebuild the cached sections.
        state.selected = Some(villain);
        let second = compose_inspector_text_section(&mut state, &world, (800, 600), 16.0);
        assert!(second.last().unwrap().content.contains("villain"));
        assert!(!second.last().unwrap().content.contains("hero"));
    }
}
