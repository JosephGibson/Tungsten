//! Route reach (redo 7.12, §2.7 of its plan): whether a room's route can
//! be walked by the prefab its `by` names, on the cells that prefab's own
//! mover meets, for `tungsten check`'s level rules.
//!
//! A `platformer_controller` prefab is judged on what its casts meet: the
//! cells its filter admits (a cell's layers meet the filter's mask and
//! none of its `without`; one-way cells as ground from above, passed from
//! below), not its body's contacts. It stands on a cell when its body's box
//! is clear there and ground is under it: a full or one-way cell below, or
//! the cell's own slope face when that face is a floor by the controller's
//! rule (its `slope_limit`, else the kit's normal y of −0.7) and not past
//! its `slide_limit`. A slope that is no floor is no ground. From a stand
//! cell it walks to a neighbour, onto a slope cell a row up or down, onto a
//! full cell a row up within its `step_height`; and it falls and jumps to
//! any stand cell its arc lands it on, the arc simulated at 60 Hz from its
//! `jump_speed` (and, with `air_jumps`, the speed that rises as high as the
//! jumps together), its gravity (its own, else `PhysicsConfig`'s) and
//! fractions of its `run_speed`, the box clear of the solid region all the
//! way. Coyote time, the jump buffer and steering in the air are left out,
//! so the check errs short.
//!
//! A `top_down_mover` prefab, whose body the physics step moves, is judged
//! by its body's contacts on the cells its box fits, by 4-neighbour steps.
//!
//! Each leg of a route is a breadth-first search from the stand cell under
//! its first point to the one under its next. A point with no stand cell
//! within 2 cells under it, a leg out of reach and a `by` prefab with
//! neither mover are findings.

use std::collections::{BTreeSet, VecDeque};

use glam::Vec2;
use serde::Deserialize;
use serde_json::Value;
use tungsten_core::World;
use tungsten_core::assets::room::{
    CellKind, PrefabBody, RoomFinding, Terrain, prefab_body, slope_normal, slope_top,
};
use tungsten_core::assets::{LayerPhysics, PlaceShape, PrefabRegistry, RoomRegistry, Slope};
use tungsten_core::physics::{PhysicsConfig, PhysicsLayers};

use super::slide::FLOOR_NORMAL_Y;

/// How far under a route's point its stand cell may be, in cells.
const BELOW: i64 = 2;

/// The longest arc simulated, in steps of 1/60 s.
const ARC_STEPS: usize = 360;

/// Each route of room `room` with a `by` judged for reach, as findings: a
/// `by` naming no prefab or one with neither mover, a stop with nothing to
/// stand on, a leg out of reach. Empty for a room the `RoomRegistry` lacks.
#[must_use]
pub fn route_findings(world: &World, room: &str) -> Vec<RoomFinding> {
    let mut findings = Vec::new();
    let Some(rooms) = world.get_resource::<RoomRegistry>() else {
        return findings;
    };
    let (Some(document), Some(compiled)) = (rooms.room(room), rooms.compiled(room)) else {
        return findings;
    };
    let terrain = Terrain::new(&compiled.map);
    for (at, place) in document.places.iter().enumerate() {
        let Some(by) = &place.by else {
            continue;
        };
        let field = format!("places[{at}]");
        let finding =
            |field: String, message: String| RoomFinding::new(room, &place.id, field, message);
        let mover = match Mover::of(world, by) {
            Ok(mover) => mover,
            Err(message) => {
                findings.push(finding(format!("{field}.by"), message));
                continue;
            }
        };
        let Some(PlaceShape::Route(stops)) = compiled.place(&place.id).map(|p| &p.shape) else {
            continue;
        };
        let mut cells = Vec::with_capacity(stops.len());
        for (stop, &point) in stops.iter().enumerate() {
            let cell = mover.cell_under(&terrain, point);
            if cell.is_none() {
                findings.push(finding(
                    format!("{field}.route[{stop}]"),
                    mover.nothing_under(),
                ));
            }
            cells.push(cell);
        }
        for leg in 1..cells.len() {
            let (Some(from), Some(to)) = (cells[leg - 1], cells[leg]) else {
                continue;
            };
            if !mover.reaches(&terrain, from, to) {
                findings.push(finding(
                    format!("{field}.route[{leg}]"),
                    format!("out of reach of route[{}] for `{by}`", leg - 1),
                ));
            }
        }
    }
    findings
}

/// A platformer controller's numbers as its data gives them; the rest of
/// its fields are no matter to reach.
#[derive(Deserialize)]
struct ControllerData {
    run_speed: f32,
    jump_speed: f32,
    #[serde(default)]
    gravity: Option<f32>,
    #[serde(default)]
    air_jumps: u32,
    #[serde(default)]
    step_height: f32,
    #[serde(default)]
    slope_limit: Option<f32>,
    #[serde(default)]
    slide_limit: Option<f32>,
    #[serde(default)]
    mask: Option<Vec<Value>>,
    #[serde(default)]
    without: Vec<Value>,
}

/// A side-view mover: its box, its numbers and its filter.
struct Platformer {
    size: Vec2,
    run_speed: f32,
    jump_speed: f32,
    gravity: f32,
    air_jumps: u32,
    step_height: f32,
    /// A face whose normal's y is at most this is a floor.
    floor_y: f32,
    /// A floor face whose normal's y is above this slides.
    slide_y: Option<f32>,
    mask: u32,
    without: u32,
}

/// The mover a route's `by` prefab is judged as.
enum Mover {
    Platformer(Platformer),
    TopDown(PrefabBody),
}

/// A stand cell: its column (the body's leftmost) and row (its feet's).
type Cell = (i64, i64);

impl Mover {
    fn of(world: &World, prefab: &str) -> Result<Self, String> {
        let data = world
            .get_resource::<PrefabRegistry>()
            .and_then(|prefabs| prefabs.get(prefab))
            .ok_or_else(|| format!("no prefab `{prefab}`"))?;
        let names = world.get_resource::<PhysicsLayers>();
        let body = prefab_body(&data.components, names)
            .map_err(|message| format!("prefab `{prefab}`'s rigid_body: {message}"))?;
        let need_body = || format!("prefab `{prefab}` has no rigid_body to size its box");
        if let Some(value) = data
            .components
            .get(super::platformer::PLATFORMER_CONTROLLER)
        {
            let controller: ControllerData = serde_json::from_value(value.clone())
                .map_err(|e| format!("prefab `{prefab}`'s platformer_controller: {e}"))?;
            let body = body.ok_or_else(need_body)?;
            let bits = |list: &[Value]| -> Result<u32, String> {
                list.iter().try_fold(0, |bits, entry| {
                    let bit = match entry {
                        Value::Number(n) => n.as_u64().filter(|&n| n < 32).map(|n| 1 << n),
                        Value::String(name) => names.and_then(|names| names.bit(name)),
                        _ => None,
                    };
                    bit.map(|bit| bits | bit)
                        .ok_or_else(|| format!("prefab `{prefab}`'s filter names layer {entry}"))
                })
            };
            let gravity = controller.gravity.unwrap_or_else(|| {
                world
                    .get_resource::<PhysicsConfig>()
                    .map_or(0.0, |physics| physics.gravity.y)
            });
            return Ok(Self::Platformer(Platformer {
                size: body.half_size() * 2.0,
                run_speed: controller.run_speed,
                jump_speed: controller.jump_speed,
                gravity,
                air_jumps: controller.air_jumps,
                step_height: controller.step_height,
                floor_y: controller
                    .slope_limit
                    .map_or(FLOOR_NORMAL_Y, |degrees| -degrees.to_radians().cos()),
                slide_y: controller
                    .slide_limit
                    .map(|degrees| -degrees.to_radians().cos()),
                mask: match &controller.mask {
                    Some(list) => bits(list)?,
                    None => u32::MAX,
                },
                without: bits(&controller.without)?,
            }));
        }
        if data
            .components
            .contains_key(super::top_down::TOP_DOWN_MOVER)
        {
            return Ok(Self::TopDown(body.ok_or_else(need_body)?));
        }
        Err(format!(
            "prefab `{prefab}` has no platformer_controller or top_down_mover"
        ))
    }

    fn nothing_under(&self) -> String {
        match self {
            Self::Platformer(_) => "nothing to stand on within 2 cells under it".to_owned(),
            Self::TopDown(_) => "its mover's box does not fit there".to_owned(),
        }
    }

    /// The stand cell a route's `point` starts or ends a leg at.
    fn cell_under(&self, terrain: &Terrain, point: Vec2) -> Option<Cell> {
        let cell = terrain.cell_size();
        match self {
            Self::Platformer(mover) => {
                let col = mover.column_at(terrain, point.x);
                let row = (point.y / cell.y).floor() as i64;
                (row..=row + BELOW)
                    .find_map(|row| mover.stand(terrain, (col, row)).map(|_| (col, row)))
            }
            Self::TopDown(body) => {
                let at = (
                    (point.x / cell.x).floor() as i64,
                    (point.y / cell.y).floor() as i64,
                );
                fits(terrain, body, at).then_some(at)
            }
        }
    }

    /// Whether `to` is reached from `from`.
    fn reaches(&self, terrain: &Terrain, from: Cell, to: Cell) -> bool {
        let mut seen: BTreeSet<Cell> = BTreeSet::from([from]);
        let mut queue = VecDeque::from([from]);
        while let Some(at) = queue.pop_front() {
            if at == to {
                return true;
            }
            let next = match self {
                Self::Platformer(mover) => mover.moves(terrain, at),
                Self::TopDown(body) => [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .into_iter()
                    .map(|(dc, dr)| (at.0 + dc, at.1 + dr))
                    .filter(|&cell| fits(terrain, body, cell))
                    .collect(),
            };
            for cell in next {
                if seen.insert(cell) {
                    queue.push_back(cell);
                }
            }
        }
        false
    }
}

/// Whether a top-down body's box fits at `cell`'s centre, by its contacts.
fn fits(terrain: &Terrain, body: &PrefabBody, (col, row): Cell) -> bool {
    let [cols, rows] = terrain.size();
    if col < 0 || row < 0 || col >= i64::from(cols) || row >= i64::from(rows) {
        return false;
    }
    let cell = terrain.cell_size();
    let center = (Vec2::new(col as f32, row as f32) + 0.5) * cell;
    let mut collider = body.collider;
    collider.offset = Vec2::ZERO;
    terrain
        .collider_hit(&collider, center, &|physics| body.meets(physics))
        .is_none()
}

impl Platformer {
    /// Whether its casts meet a collision layer of `physics`.
    fn admits(&self, physics: LayerPhysics) -> bool {
        physics.layers & self.mask != 0 && physics.layers & self.without == 0
    }

    /// The columns its box spans.
    fn span(&self, terrain: &Terrain) -> i64 {
        ((self.size.x / terrain.cell_size().x) - 1e-3)
            .ceil()
            .max(1.0) as i64
    }

    /// The leftmost column of its span with its centre at `x`.
    fn column_at(&self, terrain: &Terrain, x: f32) -> i64 {
        let span = self.span(terrain) as f32;
        (x / terrain.cell_size().x - span / 2.0 + 0.5).floor() as i64
    }

    /// Whether a slope face is ground: a floor by its rule, not sliding.
    fn floor(&self, slope: Slope) -> bool {
        let normal = slope_normal(slope);
        normal.y <= self.floor_y && self.slide_y.is_none_or(|slide| normal.y <= slide)
    }

    /// Its feet's height standing at `cell`, or `None` when it cannot stand
    /// there: its box clear and ground under it.
    fn stand(&self, terrain: &Terrain, (col, row): Cell) -> Option<f32> {
        let [cols, rows] = terrain.size();
        let span = self.span(terrain);
        if col < 0 || row < 0 || col + span > i64::from(cols) || row >= i64::from(rows) {
            return None;
        }
        let cell = terrain.cell_size();
        let admit = |physics| self.admits(physics);
        let mut feet: Option<f32> = None;
        let mut ground = false;
        for c in col..col + span {
            match terrain.cell(c, row, &admit) {
                CellKind::Slope(slope) if self.floor(slope) => {
                    let top = (row as f32 + slope_top(slope)) * cell.y;
                    feet = Some(feet.map_or(top, |feet| feet.min(top)));
                    ground = true;
                }
                CellKind::Solid | CellKind::Slope(_) => return None,
                _ => {}
            }
            if matches!(
                terrain.cell(c, row + 1, &admit),
                CellKind::Solid | CellKind::OneWay
            ) {
                ground = true;
            }
        }
        if !ground {
            return None;
        }
        let feet = feet.unwrap_or((row + 1) as f32 * cell.y);
        let left = col as f32 * cell.x + (span as f32 * cell.x - self.size.x) / 2.0;
        let clear = terrain
            .box_hit(
                Vec2::new(left, feet - self.size.y),
                Vec2::new(left + self.size.x, feet),
                &admit,
            )
            .is_none();
        clear.then_some(feet)
    }

    /// The stand cells it reaches from `at` in one move: a walk or a step,
    /// or the landing of an arc.
    fn moves(&self, terrain: &Terrain, at: Cell) -> Vec<Cell> {
        let mut out = Vec::new();
        let Some(feet) = self.stand(terrain, at) else {
            return out;
        };
        let admit = |physics| self.admits(physics);
        for dc in [-1, 1] {
            for dr in [-1, 0, 1] {
                let to = (at.0 + dc, at.1 + dr);
                let Some(target) = self.stand(terrain, to) else {
                    continue;
                };
                let slope = (at.0.min(to.0)..at.0.max(to.0) + self.span(terrain))
                    .any(|c| matches!(terrain.cell(c, to.1, &admit), CellKind::Slope(_)));
                if slope || (feet - target).abs() <= self.step_height + 0.5 {
                    out.push(to);
                }
            }
        }
        if self.gravity <= 0.0 {
            return out;
        }
        let rise = self.jump_speed * f32::sqrt(1.0 + self.air_jumps as f32);
        let mut launches = vec![self.jump_speed];
        if self.air_jumps > 0 {
            launches.push(rise);
        }
        for fraction in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let vx = self.run_speed * fraction;
            for &up in &launches {
                out.extend(self.arc(terrain, at, feet, vx, -up));
            }
            if fraction != 0.0 {
                out.extend(self.arc(terrain, at, feet, vx, 0.0));
            }
        }
        out
    }

    /// Where an arc from `at` with velocity `(vx, vy)` lands, at 60 Hz: the
    /// stand cell whose feet it reaches falling, or `None` when its box
    /// meets the solid region first or it leaves the room.
    fn arc(&self, terrain: &Terrain, at: Cell, feet: f32, vx: f32, vy: f32) -> Option<Cell> {
        const DT: f32 = 1.0 / 60.0;
        let cell = terrain.cell_size();
        let [_, rows] = terrain.size();
        let span = self.span(terrain) as f32;
        let mut x = (at.0 as f32 + span / 2.0) * cell.x;
        let mut y = feet;
        let mut vy = vy;
        let admit = |physics| self.admits(physics);
        for _ in 0..ARC_STEPS {
            let (from_y, next_x) = (y, x + vx * DT);
            vy += self.gravity * DT;
            let next_y = y + vy * DT;
            if vy > 0.0 {
                // Falling: a stand cell whose feet it passes catches it.
                let col = self.column_at(terrain, next_x);
                let rows_crossed =
                    (from_y / cell.y).floor() as i64 - 1..=(next_y / cell.y).floor() as i64 + 1;
                for row in rows_crossed {
                    if let Some(stand) = self.stand(terrain, (col, row))
                        && stand > from_y - 1e-3
                        && stand <= next_y + 1e-3
                        && (col, row) != at
                    {
                        return Some((col, row));
                    }
                }
            }
            let half = self.size.x / 2.0;
            let hit = terrain.box_hit(
                Vec2::new(next_x - half, next_y - self.size.y),
                Vec2::new(next_x + half, next_y),
                &admit,
            );
            if hit.is_some() || next_y > (rows as f32 + 1.0) * cell.y {
                return None;
            }
            (x, y) = (next_x, next_y);
        }
        None
    }
}

#[cfg(test)]
#[path = "../tests/reach.rs"]
mod tests;
