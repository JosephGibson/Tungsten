//! The free spot (S-2): where a restored body overlapping a solid goes, the
//! nearest place it overlaps none.
//!
//! It reads the maps and the bodies, not the query index, which a fresh
//! restore's bodies are not in until a step runs.

use std::cmp::Ordering;

use glam::Vec2;

use crate::assets::{LayerKind, TileIndex, TilemapInstance, TilemapRegistry};
use crate::ecs::{Entity, World};
use crate::physics::{
    Aabb, BodyKind, Collider, PhysicsConfig, Position, RigidBody, Shape, aabb_vs_aabb,
    aabb_vs_circle, circle_vs_circle,
};

/// A cell where the body overlaps no map: 16 px.
const DEFAULT_CELL: f32 = 16.0;
/// How far the search reaches, in cells.
const REACH_CELLS: i32 = 8;
/// The rings a cell holds: a quarter cell apart.
const RINGS_PER_CELL: i32 = 4;

/// A solid the body must not overlap.
enum Solid {
    Box(Aabb),
    Circle(Vec2, f32),
}

/// For a body whose collider overlaps a solid past
/// [`PhysicsConfig::linear_slop`], the nearest position where it overlaps
/// none, searched outward in rings a quarter cell apart out to eight cells;
/// within a ring the nearest wins, ties going up, then right, down, left.
/// A cell is the tile size of the map the body overlaps, 16 px where it
/// overlaps none. Solids are the full collision cells of the layers its
/// mask meets, read from the maps and their instances, and the static
/// bodies those layers meet; one-way and slope cells are not.
///
/// Returns the body's own position when it overlaps nothing, and `None`
/// for an entity without `Position`, `Collider` and `RigidBody`, or when
/// no free spot lies in reach.
#[must_use]
pub fn free_spot(world: &World, entity: Entity) -> Option<Vec2> {
    let position = world.get::<Position>(entity)?.0;
    let collider = *world.get::<Collider>(entity)?;
    let body = world.get::<RigidBody>(entity)?;
    let slop = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default()
        .linear_slop;
    let cell = cell_size(world, aabb_at(&collider, position));
    let reach = cell * REACH_CELLS as f32;
    let bounds = aabb_at(&collider, position);
    let area = Aabb::new(bounds.center, bounds.half_extents + reach);
    let solids = solids(world, entity, body, &area);
    let overlaps = |at: Vec2| {
        solids
            .iter()
            .any(|solid| penetration(&collider, at, solid).is_some_and(|depth| depth > slop))
    };
    if !overlaps(position) {
        return Some(position);
    }
    let step = cell / RINGS_PER_CELL as f32;
    for ring in 1..=REACH_CELLS * RINGS_PER_CELL {
        let mut points = ring_points(ring);
        points.sort_by(|&a, &b| ring_order(a, b, step));
        for (i, j) in points {
            let at = position + Vec2::new(i as f32 * step.x, j as f32 * step.y);
            if !overlaps(at) {
                return Some(at);
            }
        }
    }
    None
}

/// The collider's bounding box with the body at `position`.
fn aabb_at(collider: &Collider, position: Vec2) -> Aabb {
    Aabb::new(position + collider.offset, collider.shape.half_extents())
}

/// The tile size of the first map whose instance `bounds` overlaps.
fn cell_size(world: &World, bounds: Aabb) -> Vec2 {
    let Some(maps) = world.get_resource::<TilemapRegistry>() else {
        return Vec2::splat(DEFAULT_CELL);
    };
    for instance in world.query::<&TilemapInstance>() {
        let Some(data) = maps.get(&instance.id) else {
            continue;
        };
        let size = data.pixel_size();
        let map = Aabb::from_min_max(instance.origin, instance.origin + size);
        if overlap(&map, &bounds) {
            return Vec2::new(data.tile_width as f32, data.tile_height as f32);
        }
    }
    Vec2::splat(DEFAULT_CELL)
}

fn overlap(a: &Aabb, b: &Aabb) -> bool {
    let gap = (a.center - b.center).abs() - (a.half_extents + b.half_extents);
    gap.x < 0.0 && gap.y < 0.0
}

/// The solids in `area` that `body` meets: full collision cells of the
/// layers it meets and static bodies, `entity` itself left out.
fn solids(world: &World, entity: Entity, body: &RigidBody, area: &Aabb) -> Vec<Solid> {
    let meets = |layers: u32, mask: u32| body.layers & mask != 0 && layers & body.mask != 0;
    let mut solids = Vec::new();
    if let Some(maps) = world.get_resource::<TilemapRegistry>() {
        for instance in world.query::<&TilemapInstance>() {
            let Some(data) = maps.get(&instance.id) else {
                continue;
            };
            let tile = Vec2::new(data.tile_width as f32, data.tile_height as f32);
            let min = ((area.center - area.half_extents - instance.origin) / tile).floor();
            let max = ((area.center + area.half_extents - instance.origin) / tile).ceil();
            let cols = min.x.max(0.0) as u32..(max.x.max(0.0) as u32).min(data.width);
            let rows = min.y.max(0.0) as u32..(max.y.max(0.0) as u32).min(data.height);
            for (index, layer) in data.layers.iter().enumerate() {
                let physics = data.layer_physics(index);
                if layer.kind != LayerKind::Collision || !meets(physics.layers, physics.mask) {
                    continue;
                }
                for row in rows.clone() {
                    for col in cols.clone() {
                        let Some(found) = data.tile_at(index, col, row) else {
                            continue;
                        };
                        if !full_cell(data, found) {
                            continue;
                        }
                        let center =
                            instance.origin + Vec2::new(col as f32 + 0.5, row as f32 + 0.5) * tile;
                        solids.push(Solid::Box(Aabb::new(center, tile * 0.5)));
                    }
                }
            }
        }
    }
    for (other, position, collider, rigid) in
        world.query::<(Entity, &Position, &Collider, &RigidBody)>()
    {
        if other == entity || rigid.kind != BodyKind::Static || !meets(rigid.layers, rigid.mask) {
            continue;
        }
        let center = position.0 + collider.offset;
        solids.push(match collider.shape {
            Shape::Aabb { half_extents } => Solid::Box(Aabb::new(center, half_extents)),
            Shape::Circle { radius } => Solid::Circle(center, radius),
        });
    }
    solids
}

/// A cell that is a full box: a tile, neither one-way nor a slope.
fn full_cell(data: &crate::assets::TilemapData, tile: TileIndex) -> bool {
    if tile < 0 {
        return false;
    }
    let props = data.tile_props(tile);
    !props.one_way && props.slope.is_none()
}

/// How deep the collider at `at` overlaps `solid`; `None` apart.
fn penetration(collider: &Collider, at: Vec2, solid: &Solid) -> Option<f32> {
    let center = at + collider.offset;
    let contact = match (collider.shape, solid) {
        (Shape::Aabb { half_extents }, Solid::Box(cell)) => {
            aabb_vs_aabb(&Aabb::new(center, half_extents), cell)
        }
        (Shape::Circle { radius }, Solid::Box(cell)) => aabb_vs_circle(cell, center, radius),
        (Shape::Aabb { half_extents }, Solid::Circle(other, other_radius)) => {
            aabb_vs_circle(&Aabb::new(center, half_extents), *other, *other_radius)
        }
        (Shape::Circle { radius }, Solid::Circle(other, other_radius)) => {
            circle_vs_circle(center, radius, *other, *other_radius)
        }
    };
    contact.map(|contact| contact.penetration)
}

/// The steps of ring `ring`: each `(i, j)` whose larger magnitude is
/// `ring`.
fn ring_points(ring: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::with_capacity(8 * ring as usize);
    for i in -ring..=ring {
        for j in -ring..=ring {
            if i.abs().max(j.abs()) == ring {
                points.push((i, j));
            }
        }
    }
    points
}

/// The nearer first; at one distance, clockwise from up: up, then right,
/// down, left (screen y grows down).
fn ring_order(a: (i32, i32), b: (i32, i32), step: Vec2) -> Ordering {
    let distance = |(i, j): (i32, i32)| {
        let (x, y) = (
            f64::from(i) * f64::from(step.x),
            f64::from(j) * f64::from(step.y),
        );
        x * x + y * y
    };
    let clockwise = |(i, j): (i32, i32)| {
        let angle = f64::from(i).atan2(-f64::from(j));
        if angle < 0.0 {
            angle + std::f64::consts::TAU
        } else {
            angle
        }
    };
    distance(a)
        .total_cmp(&distance(b))
        .then_with(|| clockwise(a).total_cmp(&clockwise(b)))
}

#[cfg(test)]
#[path = "../tests/physics/free_spot.rs"]
mod tests;
