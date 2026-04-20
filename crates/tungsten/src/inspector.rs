//! Text-only entity inspector (M21, `F3`). Users register component types
//! that implement [`Inspectable`]; while the overlay is enabled, the nearest
//! entity to the mouse cursor in world space is re-picked every frame
//! (hover-to-inspect) and the compose helper renders every registered
//! component's labelled rows.
//!
//! Registration uses an opaque `InspectFn` closure captured at
//! `register::<T>(label)` time. The closure reads `world.get::<T>(entity)`
//! and falls back to an empty row list when the component is absent, so
//! rows for missing components simply don't appear.
//!
//! Picking uses world-space squared distance between the cursor (mapped
//! through [`CameraState::position`], `zoom`, and `rotation`) and every
//! entity's [`Transform::position`]. Entities without `Transform` are not
//! selectable. Selection is cleared on the next frame after the referenced
//! entity is despawned so the overlay never dereferences a stale id.

use glam::Vec2;

use tungsten_core::components::{Sprite, Tag, Transform, Visibility};
use tungsten_core::input::{ActionMap, InputState};
use tungsten_core::physics::{Position, Velocity};
use tungsten_core::{CameraState, Entity, Inspectable, World};
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
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            enabled: false,
            selected: None,
            registered: Vec::new(),
            // BottomLeft so the inspector does not stomp on the systems
            // overlay (TopLeft) or the debug HUD (TopRight) when all three
            // are visible at once.
            corner: HudCorner::BottomLeft,
            padding_px: 12.0,
            font_id: "mono".to_string(),
            font_size: 22.0,
            line_height: 26.0,
            color: [240, 240, 240, 240],
            outline_color: [0, 0, 0, 220],
            outline_px: 1.5,
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

    // Hover-to-inspect: every frame, when the cursor is inside the window,
    // re-pick the nearest `Transform` entity. No button edge required. The
    // previous LMB-edge gate is documented in D-047 / plan notes.
    let (cursor, camera, viewport) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
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

    let mut best: Option<(Entity, f32)> = None;
    for (entity, transform) in world.query::<Transform>() {
        let dist_sq = (transform.position - world_cursor).length_squared();
        if best.map(|(_, b)| dist_sq < b).unwrap_or(true) {
            best = Some((entity, dist_sq));
        }
    }

    let picked = best.map(|(e, _)| e);
    if let Some(state) = world.get_resource_mut::<InspectorState>() {
        state.selected = picked;
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
    state: &InspectorState,
    world: &World,
    viewport: (u32, u32),
) -> Vec<TextSection> {
    if !state.enabled {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    match state.selected {
        None => {
            lines.push("inspector: hover over an entity".to_string());
        }
        Some(entity) if !world.is_alive(entity) => {
            lines.push("inspector: hover over an entity".to_string());
        }
        Some(entity) => {
            lines.push(format!("inspector: {entity}"));
            for (label, inspect) in &state.registered {
                let rows = inspect(world, entity);
                if rows.is_empty() {
                    continue;
                }
                lines.push(format!("[{label}]"));
                for (row_label, value) in rows {
                    lines.push(format!("  {row_label:>10}  {value}"));
                }
            }
        }
    }

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

    if state.outline_px > 0.0 {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten_core::input::KeyCode;

    fn make_world() -> World {
        let mut world = World::new();
        world.insert_resource(InspectorState::new_with_defaults());
        world.insert_resource(CameraState::new());
        world.insert_resource(WindowSize {
            width: 800,
            height: 600,
        });
        world
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
    fn pick_selects_nearest_transform_entity_on_hover() {
        let mut world = make_world();
        world.get_resource_mut::<InspectorState>().unwrap().enabled = true;

        let far = world.spawn();
        world.insert(far, Transform::from_position(Vec2::new(500.0, 500.0)));

        let near = world.spawn();
        world.insert(near, Transform::from_position(Vec2::new(110.0, 110.0)));

        // Hover-only: the cursor is present but no mouse button is down.
        let mut input = InputState::new();
        input.update_cursor_position(100.0, 100.0);
        world.insert_resource(input);

        inspector_pick_system(&mut world);

        let selected = world.get_resource::<InspectorState>().unwrap().selected;
        assert_eq!(selected, Some(near));
    }

    #[test]
    fn pick_noop_when_disabled() {
        let mut world = make_world();
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::ZERO));

        let mut input = InputState::new();
        input.update_cursor_position(0.0, 0.0);
        world.insert_resource(input);

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
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::ZERO));
        // InputState with no cursor_position set must not cause a pick.
        world.insert_resource(InputState::new());

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

        let doomed = world.spawn();
        world.insert(doomed, Transform::from_position(Vec2::ZERO));
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
    fn compose_renders_no_selection_message_when_enabled() {
        let mut state = InspectorState::new_with_defaults();
        state.enabled = true;
        let world = World::new();
        let sections = compose_inspector_text_section(&state, &world, (800, 600));
        assert!(!sections.is_empty());
        assert!(sections
            .iter()
            .any(|s| s.content.contains("hover over an entity")));
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

        let sections = compose_inspector_text_section(&state, &world, (800, 600));
        let joined: String = sections
            .iter()
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("[Tag]"));
        assert!(joined.contains("hero"));
        assert!(joined.contains("[Transform]"));
        assert!(joined.contains("pos"));
    }

    #[test]
    fn compose_returns_empty_when_disabled() {
        let state = InspectorState::new_with_defaults();
        let world = World::new();
        let sections = compose_inspector_text_section(&state, &world, (800, 600));
        assert!(sections.is_empty());
    }
}
