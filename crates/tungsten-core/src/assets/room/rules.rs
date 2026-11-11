//! A room's level rules (redo 7.12, §2.7 of its plan): what `tungsten
//! check` and `tungsten level check` hold every listed room to, on its
//! compiled map with the World's prefabs, sprites and physics layer names.
//!
//! The solid region of a body is the collision cells its contacts would
//! meet, the runtime's rule both ways: the body's layers meet the layer's
//! mask and the layer's layers meet the body's mask. A full cell is its box
//! and a slope cell its solid part; a one-way cell is not solid to these
//! rules, but counts as ground for a `ground` prop and for reach.
//!
//! - **Spawns clear:** an object whose prefab has a dynamic or kinematic
//!   `rigid_body` (no sensor) does not overlap its solid region at its
//!   spawn, its collider at `position + offset`.
//! - **Props supported** (`D-214` generalized): a prefab's `support` judges
//!   its sprite's drawn box, placed by its anchor as the extract draws it,
//!   against its body's solid region, or every collision layer's for a
//!   prefab without a body. `ground`: the region within 2 px under at least
//!   60 % of the bottom edge's pixel columns, the box sunk no more than
//!   6 px into it; `under`: within 2 px over 60 % of the top edge's;
//!   `wall`: within 3 px of the left or right edge over 60 % of its rows.
//! - **Budgets:** at most `budget.lights` objects (16 by default, `D-061`'s
//!   cap) whose prefab holds one of the light components the caller names,
//!   and at most `budget.objects` objects (1,024 by default).
//!
//! Routes are the kit's (`tungsten_kit::motion::reach`), on the same
//! [`Terrain`] and [`RoomFinding`].

use std::fmt;

use glam::Vec2;
use serde::{Deserialize, Serialize};

use super::RoomRegistry;
use crate::assets::registry::AssetRegistry;
use crate::assets::tilemap::{LayerKind, LayerPhysics, Slope, SlopeCorner, SlopeRise, TilemapData};
use crate::assets::{PrefabData, PrefabRegistry};
use crate::components::variant_by_save_id;
use crate::ecs::World;
use crate::physics::{BodyKind, Collider, PhysicsLayers, Shape};
use crate::save::SaveId;

/// How a room judges a prop's sprite supported: a prefab's `support`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Support {
    /// Standing on the ground.
    Ground,
    /// Hung under a ceiling.
    Under,
    /// Against a wall, on its left or its right.
    Wall,
}

/// A rule a room breaks: the room, the entry, the field's path and what is
/// wrong, written `cave: crate_1: objects[0]: floats: …`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RoomFinding {
    /// The room's ID.
    pub room: String,
    /// The entry's ID.
    pub id: String,
    /// The field's path, as `objects[0]`.
    pub field: String,
    /// What is wrong.
    pub message: String,
}

impl RoomFinding {
    /// A finding of room `room` on entry `id` at `field`.
    #[must_use]
    pub fn new(
        room: impl Into<String>,
        id: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            room: room.into(),
            id: id.into(),
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for RoomFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {}: {}",
            self.room, self.id, self.field, self.message
        )
    }
}

/// A prefab's body as the rules read its `rigid_body`: its kind, collider
/// and layer bits.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PrefabBody {
    /// Its kind.
    pub kind: BodyKind,
    /// Its collider, offset from the entity's position.
    pub collider: Collider,
    /// The layers it is in.
    pub layers: u32,
    /// The layers it meets.
    pub mask: u32,
}

impl PrefabBody {
    /// Whether its contacts meet a collision layer of `physics`: its layers
    /// meet the layer's mask and the layer's layers meet its mask.
    #[must_use]
    pub fn meets(&self, physics: LayerPhysics) -> bool {
        self.layers & physics.mask != 0 && physics.layers & self.mask != 0
    }

    /// Its collider's half size: a box's half extents, a circle's radius on
    /// both axes.
    #[must_use]
    pub fn half_size(&self) -> Vec2 {
        self.collider.shape.half_extents()
    }
}

/// The body `components` give: their `rigid_body` read with `layers`'
/// names; `Ok(None)` without one.
///
/// # Errors
///
/// The `rigid_body` value's error.
pub fn prefab_body(
    components: &std::collections::BTreeMap<String, serde_json::Value>,
    layers: Option<&PhysicsLayers>,
) -> Result<Option<PrefabBody>, String> {
    let Some(value) = components.get("rigid_body") else {
        return Ok(None);
    };
    let (body, collider) = crate::physics::PhysicsPlugin::read_rigid_body(value, layers)?;
    Ok(Some(PrefabBody {
        kind: body.kind,
        collider,
        layers: body.layers,
        mask: body.mask,
    }))
}

/// What a cell holds for a body or a filter: nothing, a full box, a one-way
/// top or a slope's solid part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellKind {
    /// Nothing it meets.
    Empty,
    /// A full cell.
    Solid,
    /// A one-way cell: a top face from above alone.
    OneWay,
    /// A slope cell.
    Slope(Slope),
}

/// A compiled room's collision cells, auto-tiled, as the rules and reach
/// read them, in the room's own pixels.
#[derive(Debug, Clone)]
pub struct Terrain {
    map: TilemapData,
}

impl Terrain {
    /// The terrain of `map`, a compiled room's map: its brushes resolved as
    /// the `TilemapRegistry` resolves them.
    #[must_use]
    pub fn new(map: &TilemapData) -> Self {
        let mut map = map.clone();
        map.auto_tile();
        Self { map }
    }

    /// A cell's size in pixels.
    #[must_use]
    pub fn cell_size(&self) -> Vec2 {
        Vec2::new(self.map.tile_width as f32, self.map.tile_height as f32)
    }

    /// The room's size in cells.
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        [self.map.width, self.map.height]
    }

    /// What cell `(col, row)` holds over every collision layer `admit`
    /// takes: a full cell over a slope over a one-way cell over nothing;
    /// nothing past the room.
    #[must_use]
    pub fn cell(&self, col: i64, row: i64, admit: &dyn Fn(LayerPhysics) -> bool) -> CellKind {
        let (Ok(col), Ok(row)) = (u32::try_from(col), u32::try_from(row)) else {
            return CellKind::Empty;
        };
        if col >= self.map.width || row >= self.map.height {
            return CellKind::Empty;
        }
        let mut kind = CellKind::Empty;
        for (index, layer) in self.map.layers.iter().enumerate() {
            if layer.kind != LayerKind::Collision || !admit(self.map.layer_physics(index)) {
                continue;
            }
            let tile = self.map.tile_at(index, col, row).unwrap_or(-1);
            if tile < 0 {
                continue;
            }
            let props = self.map.tile_props(tile);
            let this = match (props.slope, props.one_way) {
                (Some(slope), _) => CellKind::Slope(slope),
                (None, true) => CellKind::OneWay,
                (None, false) => CellKind::Solid,
            };
            kind = match (kind, this) {
                (CellKind::Solid, _) | (_, CellKind::Solid) => CellKind::Solid,
                (CellKind::Slope(slope), _) | (_, CellKind::Slope(slope)) => CellKind::Slope(slope),
                (CellKind::OneWay, _) | (_, CellKind::OneWay) => CellKind::OneWay,
                _ => CellKind::Empty,
            };
        }
        kind
    }

    /// Whether `point` lies in the solid region `admit` takes; with
    /// `one_way`, a one-way cell's whole box counts.
    #[must_use]
    pub fn solid_at(
        &self,
        point: Vec2,
        admit: &dyn Fn(LayerPhysics) -> bool,
        one_way: bool,
    ) -> bool {
        let cell = self.cell_size();
        let (col, row) = (
            (point.x / cell.x).floor() as i64,
            (point.y / cell.y).floor() as i64,
        );
        match self.cell(col, row, admit) {
            CellKind::Empty => false,
            CellKind::Solid => true,
            CellKind::OneWay => one_way,
            CellKind::Slope(slope) => {
                let local = point / cell - Vec2::new(col as f32, row as f32);
                inside_polygon(slope_polygon(slope), local)
            }
        }
    }

    /// The first cell the box from `min` to `max` overlaps with area in the
    /// solid region `admit` takes, one-way cells left out; `None` when it is
    /// clear.
    #[must_use]
    pub fn box_hit(
        &self,
        min: Vec2,
        max: Vec2,
        admit: &dyn Fn(LayerPhysics) -> bool,
    ) -> Option<[u32; 2]> {
        const EPS: f32 = 1e-3;
        let cell = self.cell_size();
        let (min, max) = (min + Vec2::splat(EPS), max - Vec2::splat(EPS));
        if min.x >= max.x || min.y >= max.y {
            return None;
        }
        let cols = (min.x / cell.x).floor() as i64..=(max.x / cell.x).floor() as i64;
        let rows = (min.y / cell.y).floor() as i64..=(max.y / cell.y).floor() as i64;
        for row in rows {
            for col in cols.clone() {
                let hit = match self.cell(col, row, admit) {
                    CellKind::Empty | CellKind::OneWay => false,
                    CellKind::Solid => true,
                    CellKind::Slope(slope) => {
                        let origin = Vec2::new(col as f32, row as f32);
                        box_meets_polygon(
                            slope_polygon(slope),
                            min / cell - origin,
                            max / cell - origin,
                        )
                    }
                };
                if hit {
                    return Some([col as u32, row as u32]);
                }
            }
        }
        None
    }

    /// The first cell a collider at `position` overlaps in the solid region
    /// `admit` takes: a box's, or a circle's, through its bounding box and
    /// then its own shape.
    #[must_use]
    pub fn collider_hit(
        &self,
        collider: &Collider,
        position: Vec2,
        admit: &dyn Fn(LayerPhysics) -> bool,
    ) -> Option<[u32; 2]> {
        let center = position + collider.offset;
        let half = collider.shape.half_extents();
        match collider.shape {
            Shape::Circle { radius } => {
                let cell = self.cell_size();
                let (min, max) = (center - half, center + half);
                for row in (min.y / cell.y).floor() as i64..=(max.y / cell.y).floor() as i64 {
                    for col in (min.x / cell.x).floor() as i64..=(max.x / cell.x).floor() as i64 {
                        let origin = Vec2::new(col as f32, row as f32) * cell;
                        let polygon: &[(f32, f32)] = match self.cell(col, row, admit) {
                            CellKind::Empty | CellKind::OneWay => continue,
                            CellKind::Solid => &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
                            CellKind::Slope(slope) => slope_polygon(slope),
                        };
                        let points: Vec<Vec2> = polygon
                            .iter()
                            .map(|&(u, v)| origin + Vec2::new(u, v) * cell)
                            .collect();
                        if circle_meets_polygon(&points, center, radius - 1e-3) {
                            return Some([col as u32, row as u32]);
                        }
                    }
                }
                None
            }
            _ => self.box_hit(center - half, center + half, admit),
        }
    }
}

/// A slope's face normal, pointing out of its solid part, y down.
#[must_use]
pub fn slope_normal(slope: Slope) -> Vec2 {
    let points = slope_polygon(slope);
    // The face is the one edge along no axis; its normal points away from
    // the polygon's centroid.
    let n = points.len();
    let centroid = points
        .iter()
        .fold(Vec2::ZERO, |sum, &(u, v)| sum + Vec2::new(u, v))
        / n as f32;
    for at in 0..n {
        let (a, b) = (Vec2::from(points[at]), Vec2::from(points[(at + 1) % n]));
        let edge = b - a;
        if edge.x != 0.0 && edge.y != 0.0 {
            let normal = Vec2::new(edge.y, -edge.x).normalize();
            return if normal.dot(a - centroid) >= 0.0 {
                normal
            } else {
                -normal
            };
        }
    }
    Vec2::NEG_Y
}

/// The highest point of a slope's solid part in its cell, as a share of
/// the cell from its top: 0 for a `45` or a `2:1 high` floor or any
/// ceiling, 0.5 for a `2:1 low` floor.
#[must_use]
pub fn slope_top(slope: Slope) -> f32 {
    slope_polygon(slope)
        .iter()
        .map(|&(_, v)| v)
        .fold(f32::INFINITY, f32::min)
}

/// A slope's solid part in a unit cell, y down: `physics::step::slope`'s
/// polygons, by rise and corner.
fn slope_polygon(slope: Slope) -> &'static [(f32, f32)] {
    use SlopeCorner::{BottomLeft, BottomRight, TopLeft, TopRight};
    use SlopeRise::{Full, High, Low};
    match (slope.rise, slope.corner) {
        (Full, BottomRight) => &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0)],
        (Full, BottomLeft) => &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
        (Full, TopRight) => &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
        (Full, TopLeft) => &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
        (Low, BottomRight) => &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
        (Low, BottomLeft) => &[(0.0, 0.5), (0.0, 1.0), (1.0, 1.0)],
        (Low, TopRight) => &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5)],
        (Low, TopLeft) => &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.5)],
        (High, BottomRight) => &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.5)],
        (High, BottomLeft) => &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
        (High, TopRight) => &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.5)],
        (High, TopLeft) => &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5), (0.0, 1.0)],
    }
}

/// Whether `point` lies inside the convex `polygon`, its edges included.
fn inside_polygon(polygon: &[(f32, f32)], point: Vec2) -> bool {
    let n = polygon.len();
    let mut sign = 0.0f32;
    for at in 0..n {
        let (a, b) = (Vec2::from(polygon[at]), Vec2::from(polygon[(at + 1) % n]));
        let cross = (b - a).perp_dot(point - a);
        if cross.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    true
}

/// Whether the box from `min` to `max` overlaps the convex `polygon` with
/// area, by the separating axes: x, y and each edge's normal.
fn box_meets_polygon(polygon: &[(f32, f32)], min: Vec2, max: Vec2) -> bool {
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    let points: Vec<Vec2> = polygon.iter().map(|&p| Vec2::from(p)).collect();
    let mut axes = vec![Vec2::X, Vec2::Y];
    for at in 0..points.len() {
        let edge = points[(at + 1) % points.len()] - points[at];
        axes.push(Vec2::new(-edge.y, edge.x));
    }
    axes.iter().all(|&axis| {
        let span = |set: &[Vec2]| {
            set.iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                    let d = p.dot(axis);
                    (lo.min(d), hi.max(d))
                })
        };
        let (a, b) = (span(&corners), span(&points));
        a.0 < b.1 - 1e-6 && b.0 < a.1 - 1e-6
    })
}

/// Whether a circle meets a convex polygon: its centre inside it, or an
/// edge nearer than `radius`.
fn circle_meets_polygon(points: &[Vec2], center: Vec2, radius: f32) -> bool {
    let unit: Vec<(f32, f32)> = points.iter().map(|p| (p.x, p.y)).collect();
    if inside_polygon(&unit, center) {
        return true;
    }
    (0..points.len()).any(|at| {
        let (a, b) = (points[at], points[(at + 1) % points.len()]);
        let edge = b - a;
        let t = ((center - a).dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
        (a + edge * t).distance(center) < radius
    })
}

/// How a prop's sprite sits: its drawn box in the room's pixels, as the
/// extract draws it for the object named `name`: the sprite its save
/// identity picks from the pool (the sprite and its `variants`), its size
/// and anchor times the prefab's `scale`.
fn drawn_box(
    world: &World,
    prefab: &PrefabData,
    position: Vec2,
    name: &str,
) -> Option<(Vec2, Vec2)> {
    let sprite = prefab.sprite.as_ref()?;
    let pick = variant_by_save_id(SaveId::authored(name), 1 + sprite.variants.len());
    let shown = match pick {
        0 => &sprite.asset_id,
        n => &sprite.variants[n - 1],
    };
    let asset = world.get_resource::<AssetRegistry>()?.get_sprite(shown)?;
    let scale = Vec2::from(prefab.scale);
    let size = Vec2::new(asset.width as f32, asset.height as f32) * scale;
    let anchor = asset.anchor.map_or(Vec2::ZERO, Vec2::from) * scale;
    let min = position - anchor;
    Some((min, min + size))
}

/// The share of `samples` that pass, as `k of n`, and whether it reaches
/// 60 %.
fn most(passed: usize, total: usize) -> bool {
    total > 0 && passed * 5 >= total * 3
}

impl World {
    /// Room `id`'s rule findings, in object order: each spawn inside its
    /// solid region and each prop its `support` finds unsupported, then each
    /// link to a place no registered room holds, then each budget the room
    /// passes; `lights` names the components that light an object (the
    /// umbrella's check names `light`). Empty for a room the
    /// [`RoomRegistry`] lacks.
    #[must_use]
    pub fn room_findings(&self, id: &str, lights: &[&str]) -> Vec<RoomFinding> {
        let mut findings = Vec::new();
        let Some(rooms) = self.get_resource::<RoomRegistry>() else {
            return findings;
        };
        let (Some(room), Some(compiled)) = (rooms.room(id), rooms.compiled(id)) else {
            return findings;
        };
        let terrain = Terrain::new(&compiled.map);
        let names = self.get_resource::<PhysicsLayers>();
        let prefabs = self.get_resource::<PrefabRegistry>();
        let cell = terrain.cell_size();
        let mut lit: Vec<usize> = Vec::new();
        for (at, object) in room.objects.iter().enumerate() {
            let field = format!("objects[{at}]");
            let finding = |message: String| RoomFinding::new(id, &object.id, &field, message);
            let Some(prefab) = prefabs.and_then(|prefabs| prefabs.get(&object.prefab)) else {
                continue;
            };
            let Ok(components) =
                crate::assets::prefab::apply_properties(&prefab.components, &object.set)
            else {
                continue;
            };
            if lights.iter().any(|light| components.contains_key(*light)) {
                lit.push(at);
            }
            let position = Vec2::from(object.at) * cell;
            let body = match prefab_body(&components, names) {
                Ok(body) => body,
                Err(message) => {
                    findings.push(finding(format!("rigid_body: {message}")));
                    continue;
                }
            };
            let solid_body = body.filter(|body| body.kind != BodyKind::Sensor);
            if let Some(body) = solid_body
                && matches!(body.kind, BodyKind::Dynamic | BodyKind::Kinematic)
                && let Some([col, row]) =
                    terrain.collider_hit(&body.collider, position, &|physics| body.meets(physics))
            {
                findings.push(finding(format!(
                    "spawns inside the terrain: its collider overlaps cell ({col}, {row})"
                )));
            }
            if let Some(support) = prefab.support {
                let admit_body;
                let admit: &dyn Fn(LayerPhysics) -> bool = match solid_body {
                    Some(body) => {
                        admit_body = move |physics| body.meets(physics);
                        &admit_body
                    }
                    None => &|_| true,
                };
                match drawn_box(self, prefab, position, &format!("{id}.{}", object.id)) {
                    None => findings.push(finding(
                        "support: the prefab draws no registered sprite to judge".to_owned(),
                    )),
                    Some((min, max)) => {
                        if let Some(message) = supported(&terrain, support, min, max, admit) {
                            findings.push(finding(message));
                        }
                    }
                }
            }
        }
        for (at, place) in room.places.iter().enumerate() {
            let Some(to) = &place.to else {
                continue;
            };
            let found = to
                .split_once('.')
                .is_some_and(|(other, target)| rooms.place(other, target).is_some());
            if !found {
                findings.push(RoomFinding::new(
                    id,
                    &place.id,
                    format!("places[{at}].to"),
                    format!("links to `{to}`, which no room holds"),
                ));
            }
        }
        if let Some(&past) = lit.get(compiled.lights as usize) {
            findings.push(RoomFinding::new(
                id,
                &room.objects[past].id,
                format!("objects[{past}]"),
                format!(
                    "budget: {} objects hold a light, past budget.lights {}",
                    lit.len(),
                    compiled.lights
                ),
            ));
        }
        if room.objects.len() as u64 > u64::from(compiled.objects) {
            let past = &room.objects[compiled.objects as usize];
            findings.push(RoomFinding::new(
                id,
                &past.id,
                format!("objects[{}]", compiled.objects),
                format!(
                    "budget: {} objects, past budget.objects {}",
                    room.objects.len(),
                    compiled.objects
                ),
            ));
        }
        findings
    }
}

/// Why a prop of `support` with the drawn box `min`–`max` is unsupported,
/// or `None` when it is supported.
fn supported(
    terrain: &Terrain,
    support: Support,
    min: Vec2,
    max: Vec2,
    admit: &dyn Fn(LayerPhysics) -> bool,
) -> Option<String> {
    let columns: Vec<f32> = (min.x.floor() as i64..max.x.ceil() as i64)
        .map(|x| x as f32 + 0.5)
        .filter(|x| *x > min.x && *x < max.x)
        .collect();
    let rows: Vec<f32> = (min.y.floor() as i64..max.y.ceil() as i64)
        .map(|y| y as f32 + 0.5)
        .filter(|y| *y > min.y && *y < max.y)
        .collect();
    // Whether the band from `from` to `to` along a column or a row holds the
    // region, sampled every quarter pixel and at its far end.
    let band = |fixed: f32, from: f32, to: f32, vertical: bool, one_way: bool| {
        let steps = ((to - from) * 4.0).ceil().max(1.0) as usize;
        (0..=steps).any(|k| {
            let p = (from + k as f32 * 0.25).min(to - 1e-3).max(from + 1e-3);
            let point = if vertical {
                Vec2::new(fixed, p)
            } else {
                Vec2::new(p, fixed)
            };
            terrain.solid_at(point, admit, one_way)
        })
    };
    match support {
        Support::Ground => {
            let grounded = columns
                .iter()
                .filter(|&&x| band(x, max.y - 6.0, max.y + 2.0, true, true))
                .count();
            if !most(grounded, columns.len()) {
                return Some(format!(
                    "floats: {grounded} of {} bottom columns have ground within 2 px",
                    columns.len()
                ));
            }
            let sunk = columns
                .iter()
                .filter(|&&x| terrain.solid_at(Vec2::new(x, max.y - 6.5), admit, true))
                .count();
            most(sunk, columns.len()).then(|| {
                format!(
                    "sunk: {sunk} of {} bottom columns are more than 6 px into the ground",
                    columns.len()
                )
            })
        }
        Support::Under => {
            let hung = columns
                .iter()
                .filter(|&&x| band(x, min.y - 2.0, min.y + 6.0, true, false))
                .count();
            (!most(hung, columns.len())).then(|| {
                format!(
                    "hangs free: {hung} of {} top columns have a ceiling within 2 px",
                    columns.len()
                )
            })
        }
        Support::Wall => {
            let side = |x: f32| {
                rows.iter()
                    .filter(|&&y| band(y, x - 3.0, x + 3.0, false, false))
                    .count()
            };
            let touching = side(min.x).max(side(max.x));
            (!most(touching, rows.len())).then(|| {
                format!(
                    "stands free: {touching} of {} rows touch a wall within 3 px",
                    rows.len()
                )
            })
        }
    }
}

#[cfg(test)]
#[path = "../../tests/assets/room_rules.rs"]
mod tests;
