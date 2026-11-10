//! What the level editor draws in the world, so a click's effect shows
//! before it: each live room's bounds and ID, the current room's cell grid,
//! a marker at every object's spawn point (an object a generator made in
//! its own hue), its places, a box about the object under the cursor and
//! about the pick with their IDs, a drag's preview, a stroke's cells, and
//! the tool's ghost at the cursor's cell. Lines go through `DebugDraw` and
//! IDs through `WorldLabels`.
//!
//! It draws what the view holds: a grid line and an object's marker
//! outside it are skipped, so a room of thousands of objects costs what is
//! on screen.

use glam::Vec2;
use tungsten_core::assets::{CompiledPlace, PlaceShape, RoomRegistry};
use tungsten_core::text::TextOutline;
use tungsten_core::{CameraState, DebugDraw, World, WorldLabel, WorldLabels};

use super::{LevelEditor, Live, Tool, object_box, prefab_size};
use crate::app::WindowSize;

/// The overlay's colours, linear RGBA.
const BOUNDS: [f32; 4] = [0.55, 0.55, 0.62, 0.9];
/// Faint: the blend is in linear light, where a low alpha reads heavy.
const GRID: [f32; 4] = [1.0, 1.0, 1.0, 0.035];
const AUTHORED: [f32; 4] = [0.3, 0.8, 0.95, 1.0];
const GENERATED: [f32; 4] = [0.72, 0.6, 1.0, 1.0];
const PLACE: [f32; 4] = [0.35, 0.85, 0.45, 0.9];
const HOVER: [f32; 4] = [1.0, 1.0, 1.0, 0.55];
const SELECTED: [f32; 4] = [0.95, 0.85, 0.3, 1.0];
const PREVIEW: [f32; 4] = [0.96, 0.6, 0.25, 1.0];
const ERASING: [f32; 4] = [0.95, 0.3, 0.3, 1.0];

/// The labels' colours, sRGB: a room's ID, the hovered object's and the
/// pick's, and the outline under each.
const ROOM_LABEL: [u8; 4] = [200, 200, 210, 255];
const HOVER_LABEL: [u8; 4] = [235, 235, 235, 255];
const PICK_LABEL: [u8; 4] = [250, 235, 150, 255];
const LABEL_OUTLINE: [u8; 4] = [0, 0, 0, 220];
/// The labels' size, in UI units.
const LABEL_SIZE: f32 = 12.0;

/// The smallest cell, in screen pixels, the grid is drawn at: below it the
/// lines would fill the room.
pub(super) const GRID_MIN_CELL: f32 = 6.0;
/// How far outside the view, in screen pixels, an object's spawn point may
/// lie and still be drawn: its box reaches in from there.
const CULL_MARGIN: f32 = 256.0;

/// One overlay mark, gathered before the `DebugDraw` is borrowed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Mark {
    Aabb(Vec2, Vec2, [f32; 4]),
    Circle(Vec2, f32, [f32; 4]),
    Line(Vec2, Vec2, [f32; 4]),
}

/// A label: its text, the world point its bottom-left sits on, its colour.
type Label = (String, Vec2, [u8; 4]);

/// The frame's overlay: its marks and its labels, in draw order.
#[derive(Debug, Default)]
pub(super) struct Overlay {
    pub(super) marks: Vec<Mark>,
    pub(super) labels: Vec<Label>,
}

/// Queues the overlay on the World's `DebugDraw` and `WorldLabels`.
pub(super) fn draw(editor: &LevelEditor, world: &mut World, rooms: &[Live], camera: CameraState) {
    let overlay = gather(editor, world, rooms, camera);
    if let Some(debug) = world.get_resource_mut::<DebugDraw>() {
        for mark in overlay.marks {
            match mark {
                Mark::Aabb(min, max, color) => debug.draw_aabb(min, max, color, 1.0),
                Mark::Circle(centre, radius, color) => {
                    debug.draw_circle(centre, radius, color, 1.0);
                }
                Mark::Line(a, b, color) => debug.draw_line(a, b, color, 1.0),
            }
        }
    }
    if let Some(labels) = world.get_resource_mut::<WorldLabels>() {
        for (text, anchor, color) in overlay.labels {
            labels.push(
                WorldLabel::new(text, anchor, editor.font_id.clone(), LABEL_SIZE)
                    .with_pivot(Vec2::new(0.0, 1.0))
                    .with_offset(Vec2::new(0.0, -2.0))
                    .with_color(color)
                    .with_outline(TextOutline::new(LABEL_OUTLINE, 1.0)),
            );
        }
    }
}

/// What the overlay shows this frame.
pub(super) fn gather(
    editor: &LevelEditor,
    world: &World,
    rooms: &[Live],
    camera: CameraState,
) -> Overlay {
    let mut overlay = Overlay::default();
    let Some(registry) = world.get_resource::<RoomRegistry>() else {
        return overlay;
    };
    let zoom = camera.zoom.max(f32::EPSILON);
    let window = world
        .get_resource::<WindowSize>()
        .map_or(Vec2::new(1280.0, 720.0), |size| {
            Vec2::new(size.width as f32, size.height as f32)
        });
    // The view in world pixels; the editor's camera is unrotated.
    let view = (camera.position, camera.position + window / zoom);
    let reach = CULL_MARGIN / zoom;
    let in_view = |at: Vec2| {
        at.x >= view.0.x - reach
            && at.y >= view.0.y - reach
            && at.x <= view.1.x + reach
            && at.y <= view.1.y + reach
    };
    let radius = 3.0 / zoom;
    let current = editor.current.as_deref();
    let picked =
        |room: &str, id: &str| current == Some(room) && editor.selection.as_deref() == Some(id);
    let hovered = |room: &str, id: &str| {
        (editor.hover.as_ref()).is_some_and(|(over, object)| over == room && object == id)
    };
    let Overlay { marks, labels } = &mut overlay;
    for live in rooms {
        let Some(document) = registry.room(&live.id) else {
            continue;
        };
        if editor.grid && current == Some(live.id.as_str()) {
            grid(marks, live, view, zoom);
        }
        marks.push(Mark::Aabb(live.origin, live.origin + live.size, BOUNDS));
        labels.push((live.id.clone(), live.origin, ROOM_LABEL));
        for object in &document.objects {
            let marker = live.origin + Vec2::from(object.at) * live.cell;
            if !in_view(marker) {
                continue;
            }
            let hue = if object.generator.is_some() {
                GENERATED
            } else {
                AUTHORED
            };
            marks.push(Mark::Aabb(
                marker - Vec2::splat(radius),
                marker + Vec2::splat(radius),
                hue,
            ));
            if picked(&live.id, &object.id) {
                let (min, size) = object_box(world, &live.id, &object.id, marker, camera);
                marks.push(Mark::Aabb(min, min + size, SELECTED));
                labels.push((object.id.clone(), min, PICK_LABEL));
                if let Some(drag) = &editor.drag {
                    let by = Vec2::from(drag.by) * live.cell;
                    marks.push(Mark::Aabb(min + by, min + size + by, PREVIEW));
                }
            } else if hovered(&live.id, &object.id) {
                let (min, size) = object_box(world, &live.id, &object.id, marker, camera);
                marks.push(Mark::Aabb(min, min + size, HOVER));
                labels.push((object.id.clone(), min, HOVER_LABEL));
            }
        }
        if let Some(stroke) = (editor.stroke.as_ref()).filter(|stroke| stroke.room == live.id) {
            let hue = if stroke.erase { ERASING } else { PREVIEW };
            let cell =
                |col: u32, row: u32| live.origin + Vec2::new(col as f32, row as f32) * live.cell;
            if stroke.rect {
                let [col, row, width, height] = stroke.bounds();
                marks.push(Mark::Aabb(
                    cell(col, row),
                    cell(col + width, row + height),
                    hue,
                ));
            } else {
                for &[col, row] in &stroke.cells {
                    marks.push(Mark::Aabb(cell(col, row), cell(col + 1, row + 1), hue));
                }
            }
        }
        if let Some(compiled) = registry.compiled(&live.id) {
            for place in compiled.places.values() {
                place_marks(marks, live.origin, place, radius);
            }
        }
        ghost(marks, editor, world, live);
    }
    overlay
}

/// The current room's cell lines inside the view, while a cell is at least
/// [`GRID_MIN_CELL`] screen pixels.
fn grid(marks: &mut Vec<Mark>, live: &Live, view: (Vec2, Vec2), zoom: f32) {
    if live.cell.min_element() * zoom < GRID_MIN_CELL {
        return;
    }
    let min = live.origin.max(view.0);
    let max = (live.origin + live.size).min(view.1);
    if min.x >= max.x || min.y >= max.y {
        return;
    }
    let first = ((min - live.origin) / live.cell).ceil();
    let last = ((max - live.origin) / live.cell).floor();
    // The room's own edges are its bounds' lines.
    let columns = (live.size.x / live.cell.x) as i64;
    let rows = (live.size.y / live.cell.y) as i64;
    for col in (first.x as i64).max(1)..=(last.x as i64).min(columns - 1) {
        let x = live.origin.x + col as f32 * live.cell.x;
        marks.push(Mark::Line(Vec2::new(x, min.y), Vec2::new(x, max.y), GRID));
    }
    for row in (first.y as i64).max(1)..=(last.y as i64).min(rows - 1) {
        let y = live.origin.y + row as f32 * live.cell.y;
        marks.push(Mark::Line(Vec2::new(min.x, y), Vec2::new(max.x, y), GRID));
    }
}

/// What the tool will do at the cursor's cell in `live`: Place's prefab as
/// its sprite's box from the cell's top-left, Paint's and Erase's cell
/// before a stroke starts, and the eyedropper's cell.
fn ghost(marks: &mut Vec<Mark>, editor: &LevelEditor, world: &World, live: &Live) {
    let Some((room, [col, row])) = &editor.cursor else {
        return;
    };
    if *room != live.id || editor.drag.is_some() || editor.stroke.is_some() {
        return;
    }
    let min = live.origin + Vec2::new(*col as f32, *row as f32) * live.cell;
    let (size, hue) = match editor.tool {
        _ if editor.picking => (live.cell, HOVER),
        Tool::Place => {
            let size = (editor.prefab_or_first(world))
                .and_then(|prefab| prefab_size(world, &prefab))
                .unwrap_or(live.cell);
            (size, PREVIEW)
        }
        Tool::Paint => (live.cell, PREVIEW),
        Tool::Erase => (live.cell, ERASING),
        Tool::Select | Tool::Pick => return,
    };
    marks.push(Mark::Aabb(min, min + size, hue));
}

/// A place's marks: a point's circle, an area's box, a route's lines.
fn place_marks(marks: &mut Vec<Mark>, origin: Vec2, place: &CompiledPlace, radius: f32) {
    match &place.shape {
        PlaceShape::Point(point) => marks.push(Mark::Circle(origin + *point, radius * 2.0, PLACE)),
        PlaceShape::Rect { min, size } => {
            marks.push(Mark::Aabb(origin + *min, origin + *min + *size, PLACE));
        }
        PlaceShape::Route(points) => {
            for pair in points.windows(2) {
                marks.push(Mark::Line(origin + pair[0], origin + pair[1], PLACE));
            }
        }
        _ => {}
    }
}
