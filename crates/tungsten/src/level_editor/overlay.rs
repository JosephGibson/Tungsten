//! What the level editor draws in the world, through `DebugDraw`: each
//! live room's bounds, a marker at every object's spawn point (an object a
//! generator made in its own hue), its places, the pick's box, a drag's
//! preview and a stroke's cells.

use glam::Vec2;
use tungsten_core::assets::{CompiledPlace, PlaceShape, RoomRegistry};
use tungsten_core::{CameraState, DebugDraw, World};

use super::{LevelEditor, Live, object_box};

/// The overlay's colours, linear RGBA.
const BOUNDS: [f32; 4] = [0.55, 0.55, 0.62, 0.9];
const AUTHORED: [f32; 4] = [0.3, 0.8, 0.95, 1.0];
const GENERATED: [f32; 4] = [0.72, 0.6, 1.0, 1.0];
const PLACE: [f32; 4] = [0.35, 0.85, 0.45, 0.9];
const SELECTED: [f32; 4] = [0.95, 0.85, 0.3, 1.0];
const PREVIEW: [f32; 4] = [0.96, 0.6, 0.25, 1.0];

/// One overlay mark, gathered before the `DebugDraw` is borrowed.
enum Mark {
    Aabb(Vec2, Vec2, [f32; 4]),
    Circle(Vec2, f32, [f32; 4]),
    Line(Vec2, Vec2, [f32; 4]),
}

/// The overlay: each live room's bounds, its objects' markers and its
/// places; the pick's box and a drag's preview.
pub(super) fn draw(editor: &LevelEditor, world: &mut World, rooms: &[Live], camera: CameraState) {
    let mut draw = Vec::new();
    let picked = |room: &str, id: &str| {
        editor.current.as_deref() == Some(room) && editor.selection.as_deref() == Some(id)
    };
    let radius = 3.0 / camera.zoom.max(f32::EPSILON);
    if let Some(registry) = world.get_resource::<RoomRegistry>() {
        for live in rooms {
            let Some(document) = registry.room(&live.id) else {
                continue;
            };
            draw.push(Mark::Aabb(live.origin, live.origin + live.size, BOUNDS));
            for object in &document.objects {
                let marker = live.origin + Vec2::from(object.at) * live.cell;
                let hue = if object.generator.is_some() {
                    GENERATED
                } else {
                    AUTHORED
                };
                draw.push(Mark::Circle(marker, radius, hue));
                if picked(&live.id, &object.id) {
                    let (min, size) = object_box(world, &live.id, &object.id, marker, camera);
                    draw.push(Mark::Aabb(min, min + size, SELECTED));
                    if let Some(drag) = &editor.drag {
                        let by = Vec2::from(drag.by) * live.cell;
                        draw.push(Mark::Aabb(min + by, min + size + by, PREVIEW));
                    }
                }
            }
            if let Some(stroke) = editor
                .stroke
                .as_ref()
                .filter(|stroke| stroke.room == live.id)
            {
                for &[col, row] in &stroke.cells {
                    let min = live.origin + Vec2::new(col as f32, row as f32) * live.cell;
                    draw.push(Mark::Aabb(min, min + live.cell, PREVIEW));
                }
            }
            if let Some(compiled) = registry.compiled(&live.id) {
                for place in compiled.places.values() {
                    place_marks(&mut draw, live.origin, place, radius);
                }
            }
        }
    }
    if let Some(debug) = world.get_resource_mut::<DebugDraw>() {
        for mark in draw {
            match mark {
                Mark::Aabb(min, max, color) => debug.draw_aabb(min, max, color, 1.0),
                Mark::Circle(centre, radius, color) => {
                    debug.draw_circle(centre, radius, color, 1.0);
                }
                Mark::Line(a, b, color) => debug.draw_line(a, b, color, 1.0),
            }
        }
    }
}

/// A place's marks: a point's circle, an area's box, a route's lines.
fn place_marks(draw: &mut Vec<Mark>, origin: Vec2, place: &CompiledPlace, radius: f32) {
    match &place.shape {
        PlaceShape::Point(point) => draw.push(Mark::Circle(origin + *point, radius * 2.0, PLACE)),
        PlaceShape::Rect { min, size } => {
            draw.push(Mark::Aabb(origin + *min, origin + *min + *size, PLACE));
        }
        PlaceShape::Route(points) => {
            for pair in points.windows(2) {
                draw.push(Mark::Line(origin + pair[0], origin + pair[1], PLACE));
            }
        }
        _ => {}
    }
}
