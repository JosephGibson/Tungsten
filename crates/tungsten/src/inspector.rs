//! Text-only entity inspector; pick uses sprite AABB, then collider AABB.
//!
//! Smallest footprint wins; selection clears after despawn to avoid stale IDs.

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
    /// Text rebuild interval; cached sections re-emitted between ticks.
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
            // Avoid HUD TopRight and systems overlay BottomRight.
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register canonical inspectable components.
    #[must_use]
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
                    .map(tungsten_core::Inspectable::inspect_rows)
                    .unwrap_or_default()
            }),
        ));
    }

    #[must_use]
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
        .is_some_and(|s| s.enabled);
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

    // Pick edge sticks selection until next pick or despawn.
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

/// Pick smallest sprite/collider AABB containing `world_cursor`.
fn pick_entity_under_cursor(world: &World, world_cursor: Vec2) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;

    if let Some(registry) = world.get_resource::<AssetRegistry>() {
        for (entity, transform, sprite) in world.query2::<Transform, Sprite>() {
            let visible = world.get::<Visibility>(entity).is_some_and(|v| v.visible);
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

    // Collider fallback catches custom-extract visuals.
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
    if best.is_none_or(|(_, a)| area < a) {
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
    // Cache key: refresh window, viewport, selected entity.
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
    state.cached_sections.clone_from(&sections);
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
            // Size columns from visible component rows only.
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
                            "{section:<section_w$} {row_label:<label_w$} {value}"
                        ));
                    }
                }
            }
        }
    }
    lines
}

#[cfg(test)]
#[path = "tests/inspector.rs"]
mod tests;
