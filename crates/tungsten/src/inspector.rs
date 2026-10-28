//! Entity inspector; pick uses sprite AABB, then collider AABB.
//!
//! Smallest footprint wins; selection clears after despawn to avoid stale IDs.
//! From M57 a movable window on the core UI (`debug_views`): the entity, then
//! section, row and value columns in a scrolling body. With the UI's routing
//! installed the pick reads the routed input, so a press the UI took never
//! picks, at the press's own position. From M58 two columns: a header row
//! per component, then its fields indented, each value in its kind's hue.

use glam::Vec2;

use tungsten_core::components::{Sprite, Tag, Transform, Visibility};
use tungsten_core::input::{ActionMap, InputState, RawInput, RoutedInput};
use tungsten_core::physics::{Collider, Position, Shape as ColliderShape, Velocity};
use tungsten_core::text::TextAlign;
use tungsten_core::ui::WidgetId;
use tungsten_core::{AssetRegistry, CameraState, Entity, InspectRegistry, Inspectable, World};

use crate::debug_hud::HudCorner;
use crate::debug_views::{
    AMBER, BORDER, CYAN, Cell, Column, GREEN, Header, PANEL, Row, Shape, Tone, VALUE, VIOLET,
    ViewFrame, ViewSpec, ViewWindow, Width, look_of, toggle_key, write_table,
};

/// The inspector's columns (M58): the field, then its value.
const COLUMNS: &[Column] = &[
    Column::new(TextAlign::Start, Width::Capped(40), Tone::Label),
    Column::new(TextAlign::Start, Width::Fill, Tone::Value),
];

/// A value's hue by its kind, read from its text (M58): `true` and `false`
/// violet; a number, or a bracketed or parenthesised list of numbers, cyan;
/// an entity, `Entity(n)`, amber; anything else, a string, green.
pub(crate) fn value_hue(value: &str) -> [u8; 4] {
    let number = |text: &str| text.trim().parse::<f64>().is_ok();
    let list = |open: char, close: char| {
        value
            .strip_prefix(open)
            .and_then(|rest| rest.strip_suffix(close))
            .is_some_and(|inner| !inner.trim().is_empty() && inner.split(',').all(number))
    };
    let entity = value
        .strip_prefix("Entity(")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()));
    if value == "true" || value == "false" {
        VIOLET
    } else if number(value) || list('(', ')') || list('[', ']') {
        CYAN
    } else if entity {
        AMBER
    } else {
        GREEN
    }
}

/// Whether the schedule's `PreUpdate` holds `ui_route`, which
/// `App::resolve_schedule` records (M57): the pick then reads the routed
/// input.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct UiRouteInstalled(pub(crate) bool);

pub use tungsten_core::inspect::InspectFn;

pub struct InspectorState {
    pub enabled: bool,
    pub selected: Option<Entity>,
    /// The corner the window docks at when no place is saved for it: the
    /// bottom left (M58).
    pub corner: HudCorner,
    /// Space between the window and its corner, in UI units.
    pub padding_px: f32,
    /// The UI font family the window's text takes: the engine font's,
    /// [`ENGINE_FONT_ID`](crate::ENGINE_FONT_ID), by default (M57).
    pub font_id: String,
    /// Text size in UI units; 15 (M58), titles two more.
    pub font_size: f32,
    /// Line height in UI units; 20 (M58).
    pub line_height: f32,
    /// The values' and the title's colour; names take the palette's grey
    /// (M58).
    pub color: [u8; 4],
    /// The window's fill (M57).
    pub background: [u8; 4],
    /// The window's border colour; the text outline's before M57; brighter
    /// while the pointer is over the window (M58).
    pub outline_color: [u8; 4],
    /// The window's border width in physical pixels (M58); `0.0` draws
    /// none.
    pub outline_px: f32,
    /// Row refresh interval on the frame interval; a new selection
    /// refreshes at once.
    pub refresh_interval_ms: f32,
    shown_selected: Option<Entity>,
    time_since_refresh_ms: f32,
    /// Where the cursor ended the previous frame the pick ran, physical
    /// pixels: a press with no move before it in its frame picks there.
    last_cursor: Option<(f32, f32)>,
    pub(crate) window: Option<ViewWindow>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            enabled: false,
            selected: None,
            // Below the HUD; the systems overlay and the UI inspector take
            // the right side (M58).
            corner: HudCorner::BottomLeft,
            padding_px: 8.0,
            font_id: crate::ENGINE_FONT_ID.to_string(),
            font_size: 15.0,
            line_height: 20.0,
            color: VALUE,
            background: PANEL,
            outline_color: BORDER,
            outline_px: 1.0,
            refresh_interval_ms: 500.0,
            shown_selected: None,
            time_since_refresh_ms: f32::INFINITY,
            last_cursor: None,
            window: None,
        }
    }
}

impl InspectorState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// The window's root while the inspector is shown (M57).
    #[must_use]
    pub fn window(&self) -> Option<WidgetId> {
        self.window.as_ref().map(|window| window.root)
    }

    /// The inspector's frame (M57): the selected entity's rows, on its
    /// interval or at once when the selection changed; while it is shown.
    pub(crate) fn update_window(&mut self, world: &World, frame: &mut ViewFrame<'_>) {
        if frame.closes(self.window.as_ref()) {
            self.enabled = false;
        }
        let spec = ViewSpec {
            name: "inspector",
            title: "Inspector",
            key: toggle_key(world, "engine_toggle_inspector"),
            enabled: self.enabled,
            corner: self.corner,
            padding: self.padding_px,
            strip: false,
            look: look_of!(self),
            shape: Shape {
                columns: COLUMNS,
                header: Header::Line,
            },
        };
        if !frame.sync(&mut self.window, &spec) {
            self.time_since_refresh_ms = f32::INFINITY;
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let interval = world
            .get_resource::<crate::telemetry::FrameTimings>()
            .and_then(|ft| ft.interval_ms)
            .filter(|ms| ms.is_finite());
        self.time_since_refresh_ms += interval.unwrap_or(0.0);
        if self.time_since_refresh_ms < self.refresh_interval_ms
            && self.shown_selected == self.selected
        {
            return;
        }
        self.time_since_refresh_ms = 0.0;
        self.shown_selected = self.selected;
        let (header, rows) = inspector_rows(self, world);
        if let Some(line) = window.header {
            frame.tree.set_plain_text(line, &header);
        }
        write_table(frame.tree, window, &rows);
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
    if pressed && let Some(state) = world.get_resource_mut::<InspectorState>() {
        state.toggle();
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
        && !world.is_alive(selected)
        && let Some(state) = world.get_resource_mut::<InspectorState>()
    {
        state.selected = None;
    }

    // Pick edge sticks selection until next pick or despawn. With the UI's
    // routing, the routed view: a press the UI took is not in it.
    let routed = world
        .get_resource::<UiRouteInstalled>()
        .is_some_and(|installed| installed.0);
    let last_cursor = world
        .get_resource::<InspectorState>()
        .and_then(|state| state.last_cursor);
    let (press, cursor) = {
        let input = if routed {
            world
                .get_resource::<RoutedInput>()
                .map(std::ops::Deref::deref)
        } else {
            world.get_resource::<InputState>()
        };
        let Some(input) = input else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        (
            press_position(input, actions, last_cursor),
            input.cursor_position(),
        )
    };
    if let Some(state) = world.get_resource_mut::<InspectorState>() {
        state.last_cursor = cursor;
    }
    let Some(at) = press else {
        return;
    };
    let Some(camera) = world.get_resource::<CameraState>().copied() else {
        return;
    };
    let world_cursor = camera.screen_to_world(Vec2::new(at.0, at.1));
    let picked = pick_entity_under_cursor(world, world_cursor);
    if let Some(state) = world.get_resource_mut::<InspectorState>() {
        state.selected = picked;
    }
}

/// Where this frame's first press of `engine_inspector_pick` in `input`'s
/// stream happened: the last pointer move before it in the stream, else
/// `last_cursor`, where the cursor ended the previous frame. `None` with no
/// press, or no position for it.
fn press_position(
    input: &InputState,
    actions: &ActionMap,
    last_cursor: Option<(f32, f32)>,
) -> Option<(f32, f32)> {
    let bindings = actions.bindings("engine_inspector_pick");
    let mut at = last_cursor;
    for event in input.raw_events() {
        match event.input {
            RawInput::PointerMoved { x, y, .. } => at = Some((x, y)),
            press if crate::app::presses(bindings, press) => return at,
            _ => {}
        }
    }
    None
}

/// `Sprite` rows with the asset's name, which core's `Inspectable` has no registry to read.
fn sprite_rows(world: &World, entity: Entity) -> Vec<(&'static str, String)> {
    let Some(sprite) = world.get::<Sprite>(entity) else {
        return Vec::new();
    };
    let mut rows = sprite.inspect_rows();
    let name = world
        .get_resource::<AssetRegistry>()
        .and_then(|registry| registry.sprite_name(sprite.asset_id));
    if let (Some(name), Some(row)) = (name, rows.iter_mut().find(|(key, _)| *key == "asset")) {
        row.1 = name.to_string();
    }
    rows
}

/// Pick smallest sprite/collider AABB containing `world_cursor`; a collider
/// sits at its body's `Transform` when it has one, else at `Position`.
fn pick_entity_under_cursor(world: &World, world_cursor: Vec2) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;

    if let Some(registry) = world.get_resource::<AssetRegistry>() {
        for (entity, transform, sprite) in world.query::<(Entity, &Transform, &Sprite)>() {
            let visible = world.get::<Visibility>(entity).is_some_and(|v| v.visible);
            if !visible {
                continue;
            }
            let Some(asset) = registry.sprite(sprite.asset_id) else {
                continue;
            };
            // A negative scale spans the quad back from the point, as the
            // sprite shader draws it.
            let span = Vec2::new(asset.width as f32, asset.height as f32) * transform.scale;
            let min = transform.position + span.min(Vec2::ZERO);
            try_hit_aabb(&mut best, entity, world_cursor, min, span.abs());
        }
    }

    // Collider fallback catches custom-extract visuals. A body is tested
    // where the last frame drew it, the `Transform` `physics_sync` wrote:
    // the pick runs before this frame's steps, so its `alpha` would pair
    // with positions the steps have yet to move.
    for (entity, collider, position, drawn) in
        world.query::<(Entity, &Collider, &Position, Option<&Transform>)>()
    {
        let centre = drawn.map_or(position.0, |transform| transform.position) + collider.offset;
        let half = match collider.shape {
            ColliderShape::Aabb { half_extents } => {
                Vec2::new(half_extents.x.abs(), half_extents.y.abs())
            }
            ColliderShape::Circle { radius } => Vec2::splat(radius.abs()),
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

/// The canonical engine components' rows: what `App` registers before any
/// plugin, in the [`InspectRegistry`] resource plugins and games extend.
#[must_use]
pub fn default_inspect_registry() -> InspectRegistry {
    let mut registry = InspectRegistry::new();
    registry.register::<Tag>("Tag");
    registry.register::<Transform>("Transform");
    registry.register::<Visibility>("Visibility");
    registry.register_with("Sprite", sprite_rows);
    registry.register::<Position>("Position");
    registry.register::<Velocity>("Velocity");
    registry
}

/// The header line and the rows: `middle-click an entity to pick` and none
/// with nothing selected (M58); else `entity Entity(n)`, then for each
/// section of the registry with rows, a header row of its name and its
/// fields indented below it, each value in its kind's hue (M58).
fn inspector_rows(state: &InspectorState, world: &World) -> (String, Vec<Row>) {
    let empty = InspectRegistry::new();
    let registry = world.get_resource::<InspectRegistry>().unwrap_or(&empty);
    let Some(entity) = state.selected.filter(|entity| world.is_alive(*entity)) else {
        return ("middle-click an entity to pick".to_string(), Vec::new());
    };
    let mut rows = Vec::new();
    for (section, inspect) in registry.iter() {
        let fields = inspect(world, entity);
        if fields.is_empty() {
            continue;
        }
        rows.push(Row {
            cells: vec![Cell::new(section)],
            header: true,
            indent: false,
        });
        for (field, value) in fields {
            let hue = value_hue(&value);
            rows.push(Row {
                cells: vec![Cell::new(field), Cell::new(value).colored(hue)],
                header: false,
                indent: true,
            });
        }
    }
    if rows.is_empty() {
        rows.push(Row::data(vec![Cell::new("(no registered components)")]));
    }
    (format!("entity {entity}"), rows)
}

#[cfg(test)]
#[path = "tests/inspector.rs"]
mod tests;
