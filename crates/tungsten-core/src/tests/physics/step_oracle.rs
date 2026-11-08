//! A seeded random-scene oracle for the physics step (core hardening of
//! 2026-11-07). Each scene holds a closed arena of static walls, a few static
//! boxes, a small tile map of solid, one-way and slope cells, dynamic boxes
//! and circles of 2 to 64 px with random masses, gravity scales (0.04 to 4),
//! restitution and friction at speeds up to 18,000 px/s, and kinematic
//! platforms, under gravity 900 or 3,600 at steps of 1/60 or 1/30 s; between
//! steps it despawns, teleports, wakes, reshapes and drives bodies. Every
//! step is checked against four invariants:
//!
//! 1. **Pairs.** After each pair refresh, every pair with a moving side whose
//!    pair boxes overlap (the step's own boxes, `D-080`), and every one whose
//!    boxes grown by a substep of travel and `2·linear_slop` overlap
//!    (`D-081`'s allowance, which the narrow phase's admission needs), is
//!    among the candidate pairs, once; the extras the list keeps are counted.
//!    A freshly set allowance is the proxy's own: `|v|·t_left`, gravity's
//!    term times the larger of 1 and its own scale, and `4·linear_slop`.
//! 2. **No static passed through.** No dynamic body's centre enters a static
//!    box or the solid cells of the map and leaves by the far side, along
//!    its substep moves (straight lines), within a step or across steps.
//! 3. **A changed contact wakes its sleeper.** A contact that holds a sleeper
//!    at inverse mass 0 and changes the awake side's normal velocity by more
//!    than the sleep threshold in a substep (the hold of gravity a new
//!    contact takes left out), a contact that begins with a sleeper at a
//!    closing speed above the threshold, and the last contact that held a
//!    sleeper up (by [`HELD_DEPTH`] or more, against gravity) ending, each
//!    leave the sleeper awake after the step.
//! 4. **A seed gives one hash.** A scene run twice, in two fresh worlds (one
//!    with the probe, one without), gives one state hash every step.
//!
//! [`KNOWN`] names the findings the oracle trips: open rows of
//! `docs/known-issues.md` and new ones it found. Each check names the
//! findings that explain a failure (`classify_end` for the ended contacts),
//! and a failure is skipped only when one of them is listed. Each entry has
//! a witness, a small scene that trips it: a fix removes its entry, the
//! witness check then stops asking for it, and the default seeds prove that
//! nothing it explained still fails.
//!
//! The ignored tests are a finder's tools: the long mode over many seeds
//! (`ORACLE_SEEDS=a..b`, run in release), a trace of one seed or witness
//! (`ORACLE_SEED`, `ORACLE_WITNESS`, `ORACLE_BODY`, `ORACLE_STEPS`), the
//! reducer that prints each failure class's smallest scene as a test body
//! for `step.rs`, and new-1's crush sweep.

use super::pairs::{PAIR_MARGIN_SLOPS, gravity_allowance};
use super::*;
use crate::assets::{
    LayerKind, Slope, SlopeCorner, SlopeRise, TileProps, TilemapData, TilemapInstance,
    TilemapRegistry,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

// ---------------------------------------------------------------------------
// The generator.

/// splitmix64: the oracle's own generator, so no crate or core type decides
/// its scenes.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x6A09_E667_F3BC_C908)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1_u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    fn log_range(&mut self, lo: f32, hi: f32) -> f32 {
        (lo.ln() + (hi.ln() - lo.ln()) * self.unit()).exp()
    }

    fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn direction(&mut self) -> Vec2 {
        let angle = self.range(0.0, std::f32::consts::TAU);
        Vec2::new(angle.cos(), angle.sin())
    }
}

/// The arena's interior half extents, about the origin; y grows down.
const ARENA: Vec2 = Vec2::new(400.0, 240.0);
/// The kinematic platforms' band, which no static box or map enters.
const BAND: (f32, f32) = (-150.0, -90.0);
/// Static boxes and the map sit below this.
const LOW: f32 = -40.0;
/// A platform turns this far from the side walls, more than any body spans,
/// so it never presses a body into a wall.
const PLATFORM_CLEARANCE: f32 = 140.0;
const CELL: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cell {
    Empty,
    Solid,
    OneWay,
    /// A slope kind, 1 to 12, as the step tests' `slope_kind` numbers them.
    Slope(u8),
}

#[derive(Debug, Clone)]
struct MapSpec {
    origin: Vec2,
    cols: u32,
    rows: u32,
    cells: Vec<Cell>,
}

impl MapSpec {
    fn cell(&self, col: i32, row: i32) -> Cell {
        if col < 0 || row < 0 || col >= self.cols as i32 || row >= self.rows as i32 {
            return Cell::Empty;
        }
        self.cells[(row as u32 * self.cols + col as u32) as usize]
    }

    fn size(&self) -> Vec2 {
        Vec2::new(self.cols as f32, self.rows as f32) * CELL
    }

    fn bounds(&self) -> Aabb {
        Aabb::from_min_max(self.origin, self.origin + self.size())
    }

    fn cell_box(&self, col: i32, row: i32) -> Aabb {
        let min = self.origin + Vec2::new(col as f32, row as f32) * CELL;
        Aabb::from_min_max(min, min + Vec2::splat(CELL))
    }
}

#[derive(Debug, Clone, Copy)]
struct StaticSpec {
    center: Vec2,
    half: Vec2,
    /// A static's `Velocity` reaches its proxy (`ch-static-velocity`); a
    /// few scenes give one box one.
    velocity: Vec2,
    /// An arena wall, which no op touches.
    wall: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Dynamic,
    Kinematic,
}

#[derive(Debug, Clone, Copy)]
struct BodySpec {
    kind: Kind,
    shape: Shape,
    center: Vec2,
    velocity: Vec2,
    mass: f32,
    gravity_scale: f32,
    restitution: f32,
    friction: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Static(usize),
    Body(usize),
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Despawn,
    /// `teleport` to the point.
    Teleport(Vec2),
    /// `wake`.
    Wake,
    /// A collider write: the new shape, no offset.
    Reshape(Shape),
    /// A kinematic platform's `Velocity` write.
    Drive(Vec2),
}

#[derive(Debug, Clone, Copy)]
struct OpSpec {
    /// Applied before this step.
    step: u32,
    target: Target,
    op: Op,
}

#[derive(Debug, Clone)]
struct Scene {
    seed: u64,
    gravity: f32,
    dt: f32,
    steps: u32,
    statics: Vec<StaticSpec>,
    map: Option<MapSpec>,
    bodies: Vec<BodySpec>,
    ops: Vec<OpSpec>,
}

fn shape_box(shape: Shape, center: Vec2) -> Aabb {
    Aabb::new(center, shape.half_extents())
}

fn grown(aabb: Aabb, by: f32) -> Aabb {
    Aabb::new(aabb.center, aabb.half_extents + Vec2::splat(by))
}

fn random_shape(rng: &mut Rng) -> Shape {
    if rng.chance(0.5) {
        Shape::Aabb {
            half_extents: Vec2::new(rng.range(1.0, 32.0), rng.range(1.0, 32.0)),
        }
    } else {
        Shape::Circle {
            radius: rng.range(1.0, 32.0),
        }
    }
}

/// A point where a body of `half` fits clear of the statics, the map and
/// the platforms' band, inside the arena.
fn free_spot(
    rng: &mut Rng,
    half: Vec2,
    statics: &[StaticSpec],
    map: Option<&MapSpec>,
    taken: &[Aabb],
) -> Option<Vec2> {
    for _ in 0..60 {
        let lo = -ARENA + half + Vec2::splat(1.0);
        let hi = ARENA - half - Vec2::splat(1.0);
        let at = Vec2::new(rng.range(lo.x, hi.x), rng.range(lo.y, hi.y));
        let aabb = Aabb::new(at, half + Vec2::splat(1.0));
        let band = Aabb::from_min_max(
            Vec2::new(-ARENA.x, BAND.0 - 2.0),
            Vec2::new(ARENA.x, BAND.1 + 2.0),
        );
        if aabb.overlaps(&band)
            || statics
                .iter()
                .any(|s| aabb.overlaps(&Aabb::new(s.center, s.half)))
            || map.is_some_and(|m| aabb.overlaps(&m.bounds()))
            || taken.iter().any(|t| aabb.overlaps(t))
        {
            continue;
        }
        return Some(at);
    }
    None
}

/// The scene a seed names: its low bits pick gravity and step.
fn generate(seed: u64) -> Scene {
    let mut rng = Rng::new(seed);
    let gravity = if seed & 1 == 0 { 900.0 } else { 3600.0 };
    let dt = if seed & 2 == 0 {
        1.0 / 60.0
    } else {
        1.0 / 30.0
    };
    let steps = if seed & 2 == 0 { 240 } else { 150 };

    let mut statics = Vec::new();
    for wall in 0..4 {
        let thick = rng.range(4.0, 20.0);
        let (center, half) = match wall {
            0 => (
                Vec2::new(0.0, ARENA.y + thick),
                Vec2::new(ARENA.x + 40.0, thick),
            ),
            1 => (
                Vec2::new(0.0, -ARENA.y - thick),
                Vec2::new(ARENA.x + 40.0, thick),
            ),
            2 => (
                Vec2::new(-ARENA.x - thick, 0.0),
                Vec2::new(thick, ARENA.y + 40.0),
            ),
            _ => (
                Vec2::new(ARENA.x + thick, 0.0),
                Vec2::new(thick, ARENA.y + 40.0),
            ),
        };
        statics.push(StaticSpec {
            center,
            half,
            velocity: Vec2::ZERO,
            wall: true,
        });
    }

    let map = rng.chance(0.9).then(|| {
        let cols = 6 + rng.below(9) as u32;
        let rows = 3 + rng.below(4) as u32;
        let size = Vec2::new(cols as f32, rows as f32) * CELL;
        let origin = Vec2::new(
            rng.range(-ARENA.x + 16.0, ARENA.x - size.x - 16.0),
            rng.range(LOW + 40.0, ARENA.y - size.y - 4.0),
        );
        let cells = (0..cols * rows)
            .map(|_| {
                let roll = rng.unit();
                if roll < 0.40 {
                    Cell::Empty
                } else if roll < 0.70 {
                    Cell::Solid
                } else if roll < 0.82 {
                    Cell::OneWay
                } else {
                    Cell::Slope(1 + rng.below(12) as u8)
                }
            })
            .collect();
        MapSpec {
            origin,
            cols,
            rows,
            cells,
        }
    });

    let boxes = 2 + rng.below(3);
    let moving_static = rng.chance(1.0 / 6.0);
    for index in 0..boxes {
        let half = Vec2::new(rng.range(4.0, 64.0), rng.range(2.0, 24.0));
        for _ in 0..20 {
            let center = Vec2::new(
                rng.range(-ARENA.x + half.x, ARENA.x - half.x),
                rng.range(LOW + half.y, ARENA.y - half.y),
            );
            let aabb = Aabb::new(center, half);
            if map
                .as_ref()
                .is_some_and(|m| aabb.overlaps(&grown(m.bounds(), 8.0)))
            {
                continue;
            }
            let velocity = if moving_static && index == 0 {
                rng.direction() * rng.range(20.0, 200.0)
            } else {
                Vec2::ZERO
            };
            statics.push(StaticSpec {
                center,
                half,
                velocity,
                wall: false,
            });
            break;
        }
    }

    let mut bodies = Vec::new();
    let mut taken = Vec::new();
    // Platforms first: their boxes keep the bodies off them at the start.
    let platforms = rng.below(3);
    let mut schedules = Vec::new();
    for _ in 0..platforms {
        let half = Vec2::new(rng.range(16.0, 48.0), rng.range(4.0, 8.0));
        let span = ARENA.x - PLATFORM_CLEARANCE - half.x;
        let center = Vec2::new(
            rng.range(-span, span),
            rng.range(BAND.0 + half.y, BAND.1 - half.y),
        );
        // A schedule of drives that keeps the platform in its span: each
        // segment still, left or right for 20 to 60 steps.
        let mut x = center.x;
        let mut at = 0;
        let mut drives = Vec::new();
        while at < steps {
            let length = 20 + rng.below(41) as u32;
            let speed = rng.range(20.0, 150.0);
            let mut vx = match rng.below(3) {
                0 => 0.0,
                1 => speed,
                _ => -speed,
            };
            let travel = vx * dt * length as f32;
            if (x + travel).abs() > span {
                vx = -vx;
            }
            if (x + vx * dt * length as f32).abs() > span {
                vx = 0.0;
            }
            x += vx * dt * length as f32;
            drives.push((at, Vec2::new(vx, 0.0)));
            at += length;
        }
        taken.push(Aabb::new(center, half + Vec2::splat(2.0)));
        schedules.push((bodies.len(), drives.clone()));
        bodies.push(BodySpec {
            kind: Kind::Kinematic,
            shape: Shape::Aabb { half_extents: half },
            center,
            velocity: drives[0].1,
            mass: 1.0,
            gravity_scale: 1.0,
            restitution: 0.0,
            friction: if rng.chance(0.5) {
                rng.range(0.0, 1.0)
            } else {
                0.0
            },
        });
    }
    let dynamics = 8 + rng.below(11);
    for _ in 0..dynamics {
        let shape = random_shape(&mut rng);
        let half = shape.half_extents();
        let Some(center) = free_spot(&mut rng, half, &statics, map.as_ref(), &taken) else {
            continue;
        };
        taken.push(Aabb::new(center, half + Vec2::splat(1.0)));
        let roll = rng.unit();
        let speed = if roll < 0.5 {
            rng.range(0.0, 200.0)
        } else if roll < 0.8 {
            rng.range(200.0, 2000.0)
        } else {
            rng.range(2000.0, 18000.0)
        };
        bodies.push(BodySpec {
            kind: Kind::Dynamic,
            shape,
            center,
            velocity: rng.direction() * speed,
            mass: rng.log_range(0.1, 100.0),
            gravity_scale: if rng.chance(0.4) {
                1.0
            } else {
                rng.log_range(0.04, 4.0)
            },
            restitution: if rng.chance(0.3) {
                0.0
            } else {
                rng.range(0.0, 0.9)
            },
            friction: if rng.chance(0.5) {
                rng.range(0.0, 1.0)
            } else {
                0.0
            },
        });
    }

    // Structures that fall asleep and are then disturbed: a stack on the
    // floor that a striker hits from the side, then an op on its bottom
    // member; and a rider on a still platform, then an op on the platform.
    let mut ops = Vec::new();
    if rng.chance(0.5) {
        let size = rng.range(5.0, 16.0);
        let x = rng.range(-ARENA.x + 60.0, ARENA.x - 60.0);
        let mut floor = ARENA.y;
        let mut members = Vec::new();
        for _ in 0..2 + rng.below(3) {
            let shape = if rng.chance(0.5) {
                Shape::Aabb {
                    half_extents: Vec2::new(size, size * rng.range(0.6, 1.0)),
                }
            } else {
                Shape::Circle { radius: size }
            };
            let half = shape.half_extents();
            let center = Vec2::new(x + rng.range(-1.0, 1.0), floor - half.y - 0.25);
            let aabb = Aabb::new(center, half);
            if statics
                .iter()
                .any(|s| aabb.overlaps(&Aabb::new(s.center, s.half)))
                || map.as_ref().is_some_and(|m| aabb.overlaps(&m.bounds()))
                || taken.iter().any(|t| aabb.overlaps(t))
            {
                break;
            }
            taken.push(grown(aabb, 1.0));
            floor = center.y - half.y - 0.25;
            members.push(bodies.len());
            bodies.push(BodySpec {
                kind: Kind::Dynamic,
                shape,
                center,
                velocity: Vec2::ZERO,
                mass: rng.log_range(0.5, 5.0),
                gravity_scale: 1.0,
                restitution: 0.0,
                friction: if rng.chance(0.5) {
                    rng.range(0.0, 0.5)
                } else {
                    0.0
                },
            });
        }
        if members.len() >= 2 {
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let radius = rng.range(4.0, 12.0);
            let center = Vec2::new(x + side * rng.range(220.0, 340.0), ARENA.y - radius - 0.25);
            let lane = Aabb::from_min_max(
                Vec2::new(center.x.min(x), ARENA.y - 2.0 * radius - 1.0),
                Vec2::new(center.x.max(x), ARENA.y),
            );
            let speed = rng.range(150.0, 450.0);
            if center.x.abs() < ARENA.x - radius
                && !statics
                    .iter()
                    .any(|s| !s.wall && lane.overlaps(&Aabb::new(s.center, s.half)))
                && !map.as_ref().is_some_and(|m| lane.overlaps(&m.bounds()))
            {
                bodies.push(BodySpec {
                    kind: Kind::Dynamic,
                    shape: Shape::Circle { radius },
                    center,
                    velocity: Vec2::new(-side * speed, 0.0),
                    mass: rng.log_range(0.5, 20.0),
                    gravity_scale: 1.0,
                    restitution: 0.0,
                    friction: 0.0,
                });
            }
            let arrival = ((center.x - x).abs() / speed / dt) as u32;
            let step = (arrival + 5 + rng.below(50) as u32).min(steps - 2);
            let target = Target::Body(members[0]);
            let op = match rng.below(4) {
                0 => Op::Despawn,
                1 => Op::Wake,
                2 => Op::Reshape(Shape::Circle { radius: size * 0.5 }),
                _ => match free_spot(&mut rng, Vec2::splat(33.0), &statics, map.as_ref(), &[]) {
                    Some(at) => Op::Teleport(at),
                    None => Op::Despawn,
                },
            };
            ops.push(OpSpec { step, target, op });
        }
    }
    if !schedules.is_empty() && rng.chance(0.6) {
        let host = rng.below(schedules.len());
        let platform = schedules[host].0;
        schedules[host].1.truncate(1);
        schedules[host].1[0].1 = Vec2::ZERO;
        bodies[platform].velocity = Vec2::ZERO;
        let deck = Aabb::new(
            bodies[platform].center,
            bodies[platform].shape.half_extents(),
        );
        let shape = random_shape(&mut rng);
        let half = shape.half_extents().min(Vec2::splat(16.0));
        let shape = match shape {
            Shape::Aabb { .. } => Shape::Aabb { half_extents: half },
            Shape::Circle { .. } => Shape::Circle { radius: half.x },
        };
        let half = shape.half_extents();
        let center = Vec2::new(
            deck.center.x + rng.range(-0.5, 0.5) * deck.half_extents.x,
            deck.min().y - half.y - 0.25,
        );
        bodies.push(BodySpec {
            kind: Kind::Dynamic,
            shape,
            center,
            velocity: Vec2::ZERO,
            mass: rng.log_range(0.5, 5.0),
            gravity_scale: 1.0,
            restitution: 0.0,
            friction: if rng.chance(0.5) {
                rng.range(0.0, 0.5)
            } else {
                0.0
            },
        });
        let step = (rng.range(0.9, 2.0) / dt) as u32;
        let op = if rng.chance(0.6) {
            Op::Despawn
        } else {
            Op::Reshape(Shape::Aabb {
                half_extents: Vec2::new(deck.half_extents.x, deck.half_extents.y * 0.25),
            })
        };
        ops.push(OpSpec {
            step: step.min(steps - 2),
            target: Target::Body(platform),
            op,
        });
    }

    for (body, drives) in schedules {
        for &(at, velocity) in drives.iter().skip(1) {
            ops.push(OpSpec {
                step: at,
                target: Target::Body(body),
                op: Op::Drive(velocity),
            });
        }
    }
    let dynamic: Vec<usize> = (0..bodies.len())
        .filter(|&i| bodies[i].kind == Kind::Dynamic)
        .collect();
    let kinematic: Vec<usize> = (0..bodies.len())
        .filter(|&i| bodies[i].kind == Kind::Kinematic)
        .collect();
    let inner: Vec<usize> = (0..statics.len()).filter(|&i| !statics[i].wall).collect();
    for step in 1..steps {
        // More ops later, when more bodies sleep.
        let rate = if step * 2 < steps { 0.05 } else { 0.12 };
        if dynamic.is_empty() || !rng.chance(rate) {
            continue;
        }
        let pick_target = |rng: &mut Rng| {
            let roll = rng.unit();
            if roll < 0.15 && !inner.is_empty() {
                Target::Static(inner[rng.below(inner.len())])
            } else if roll < 0.3 && !kinematic.is_empty() {
                Target::Body(kinematic[rng.below(kinematic.len())])
            } else {
                Target::Body(dynamic[rng.below(dynamic.len())])
            }
        };
        let roll = rng.unit();
        let (target, op) = if roll < 0.25 {
            (pick_target(&mut rng), Op::Despawn)
        } else if roll < 0.5 {
            let target = dynamic[rng.below(dynamic.len())];
            let Some(at) = free_spot(&mut rng, Vec2::splat(33.0), &statics, map.as_ref(), &[])
            else {
                continue;
            };
            (Target::Body(target), Op::Teleport(at))
        } else if roll < 0.7 {
            (Target::Body(dynamic[rng.below(dynamic.len())]), Op::Wake)
        } else {
            let target = pick_target(&mut rng);
            let old = match target {
                Target::Static(i) => Shape::Aabb {
                    half_extents: statics[i].half,
                },
                Target::Body(i) => bodies[i].shape,
            };
            let factor = rng.range(0.5, 1.5);
            let limit = if matches!(target, Target::Static(_)) {
                64.0
            } else {
                32.0
            };
            let shape = match old {
                Shape::Aabb { half_extents } => Shape::Aabb {
                    half_extents: (half_extents * factor).clamp(Vec2::ONE, Vec2::splat(limit)),
                },
                Shape::Circle { radius } => Shape::Circle {
                    radius: (radius * factor).clamp(1.0, limit),
                },
            };
            (target, Op::Reshape(shape))
        };
        ops.push(OpSpec { step, target, op });
    }
    ops.sort_by_key(|op| op.step);
    Scene {
        seed,
        gravity,
        dt,
        steps,
        statics,
        map,
        bodies,
        ops,
    }
}

// ---------------------------------------------------------------------------
// The probe: hooks the step calls under `cfg(test)`.

/// What the hooks in `substep` record for the oracle; the runner drains it
/// after each step.
#[derive(Debug, Default)]
pub(super) struct Probe {
    misses: Vec<PairMiss>,
    lifted: Vec<Lift>,
    /// Pairs listed, pairs the allowance needs, listed pairs it does not
    /// need, and refreshes checked, summed over the step.
    listed: u64,
    needed: u64,
    extra: u64,
    checked: u64,
    builds_seen: usize,
    repairs_seen: usize,
    /// Each substep's end centre of every dynamic entity proxy.
    paths: Vec<Vec<(Entity, Vec2)>>,
    /// The last substep's normal impulses by pair key; a contact absent from
    /// it carried none.
    impulses: HashMap<u128, f32>,
    hits: Vec<Hit>,
    /// The lighter side of each touching contact between two awake dynamic
    /// bodies whose masses differ ten times or more, with the ratio.
    heavy: Vec<(Entity, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum PairRule {
    /// The list holds a pair twice.
    Duplicate,
    /// The step's own pair boxes overlap and the pair is not listed.
    Box,
    /// The boxes grown by a substep of travel and `2·linear_slop` overlap
    /// and the pair is not listed.
    Allowance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProxySide {
    Body(Entity),
    /// A tile proxy, by its centre.
    Tile(Vec2),
}

#[derive(Debug, Clone, Copy)]
struct PairMiss {
    rule: PairRule,
    a: ProxySide,
    b: ProxySide,
    /// The two world boxes' gap, below 0 when they overlap.
    gap: f32,
}

/// A moving proxy whose fresh allowance exceeds its own.
#[derive(Debug, Clone, Copy)]
struct Lift {
    entity: Entity,
    radius: f32,
    own: f32,
}

/// A contact that held a sleeper at inverse mass 0 and changed the awake
/// side's normal velocity by `change` in a substep.
#[derive(Debug, Clone, Copy)]
struct Hit {
    sleeper: Entity,
    other: Entity,
    change: f32,
}

fn side(proxy: &Proxy) -> ProxySide {
    proxy
        .entity
        .map_or(ProxySide::Tile(proxy.center), ProxySide::Body)
}

/// The larger per-axis gap of two boxes, below 0 when they overlap.
fn box_gap(a: &Aabb, b: &Aabb) -> f32 {
    ((a.center - b.center).abs() - (a.half_extents + b.half_extents)).max_element()
}

/// Invariant 1, after a substep's pair refresh: the brute-force pair sets
/// against the list, and each allowance set by this refresh against the
/// proxy's own.
pub(super) fn after_refresh(
    config: &PhysicsConfig,
    sub_dt: f32,
    time_left: f32,
    buffers: &mut PhysicsBuffers,
) {
    let Some(mut probe) = buffers.oracle.take() else {
        return;
    };
    let proxies = &buffers.proxies;
    let mut listed = HashSet::with_capacity(buffers.pairs.len());
    for &(a, b) in &buffers.pairs {
        if !listed.insert((a.min(b), a.max(b))) {
            probe.misses.push(PairMiss {
                rule: PairRule::Duplicate,
                a: side(&proxies[a as usize]),
                b: side(&proxies[b as usize]),
                gap: 0.0,
            });
        }
    }
    let slack = 2.0 * config.linear_slop;
    let reach: Vec<Aabb> = proxies
        .iter()
        .map(|proxy| grown(proxy.world_aabb(), proxy.velocity.length() * sub_dt + slack))
        .collect();
    let moves: Vec<bool> = proxies.iter().map(Proxy::moves).collect();
    let (mut needed, mut kept) = (0_u64, 0_u64);
    for i in 0..proxies.len() {
        for j in i + 1..proxies.len() {
            if !(moves[i] || moves[j]) || !buffers.layers.meet(i, j) {
                continue;
            }
            let by_box = buffers.pair_aabbs[i].overlaps(&buffers.pair_aabbs[j]);
            let by_allowance = reach[i].overlaps(&reach[j]);
            let in_list = listed.contains(&(i as u32, j as u32));
            needed += u64::from(by_allowance);
            kept += u64::from(in_list && by_allowance);
            if in_list {
                continue;
            }
            let rule = if by_box {
                PairRule::Box
            } else if by_allowance {
                PairRule::Allowance
            } else {
                continue;
            };
            probe.misses.push(PairMiss {
                rule,
                a: side(&proxies[i]),
                b: side(&proxies[j]),
                gap: box_gap(&proxies[i].world_aabb(), &proxies[j].world_aabb()),
            });
        }
    }
    probe.listed += listed.len() as u64;
    probe.needed += needed;
    probe.extra += listed.len() as u64 - kept;
    probe.checked += 1;

    // The allowances this refresh set: every one at a build, the members'
    // at a repair.
    let built = buffers.pair_builds != probe.builds_seen;
    let repaired = buffers.pair_repairs != probe.repairs_seen;
    probe.builds_seen = buffers.pair_builds;
    probe.repairs_seen = buffers.pair_repairs;
    let fresh: Vec<usize> = if built {
        (0..buffers.bodies).collect()
    } else if repaired {
        buffers.tripped.iter().map(|&i| i as usize).collect()
    } else {
        Vec::new()
    };
    let world_gravity = gravity_allowance(config, sub_dt, time_left);
    for index in fresh {
        let proxy = &proxies[index];
        if !proxy.moves() {
            continue;
        }
        let scale = buffers
            .extra
            .gravity_scales
            .get(index)
            .copied()
            .unwrap_or(1.0);
        let gravity = if proxy.kinematic {
            0.0
        } else {
            world_gravity * scale.abs().max(1.0)
        };
        let own = proxy.velocity.length() * time_left
            + gravity
            + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop;
        let radius = buffers.pair_budgets[index].radius;
        if radius > own * (1.0 + 1e-5) + 1e-4 {
            probe.lifted.push(Lift {
                entity: proxy.entity.expect("an entity proxy"),
                radius,
                own,
            });
        }
    }
    buffers.oracle = Some(probe);
}

/// Invariants 2 and 3, after each substep: the dynamic bodies' centres,
/// which the substep moved along straight lines, and each contact that held
/// a sleeper at inverse mass 0, with the normal velocity it gave the awake
/// side beyond the previous substep's.
pub(super) fn after_substep(config: &PhysicsConfig, sub_dt: f32, buffers: &mut PhysicsBuffers) {
    let Some(mut probe) = buffers.oracle.take() else {
        return;
    };
    let proxies = &buffers.proxies;
    probe.paths.push(
        proxies[..buffers.bodies]
            .iter()
            .filter(|proxy| proxy.is_dynamic)
            .filter_map(|proxy| proxy.entity.map(|entity| (entity, proxy.center)))
            .collect(),
    );
    let mut impulses = HashMap::with_capacity(buffers.contacts.len());
    for contact in &buffers.contacts {
        let before = probe.impulses.get(&contact.key).copied().unwrap_or(0.0);
        impulses.insert(contact.key, contact.impulse);
        let (a, b) = (&proxies[contact.a as usize], &proxies[contact.b as usize]);
        let (Some(awake), Some(sleeper)) = (a.entity, b.entity) else {
            continue;
        };
        if a.is_dynamic
            && b.is_dynamic
            && contact.inv_a > 0.0
            && contact.inv_b > 0.0
            && (contact.penetration > 0.0 || contact.impulse > 0.0)
        {
            let ratio = contact.inv_a / contact.inv_b;
            if ratio >= CRUSH_RATIO {
                probe.heavy.push((awake, ratio));
            } else if ratio * CRUSH_RATIO <= 1.0 {
                probe.heavy.push((sleeper, 1.0 / ratio));
            }
        }
        // A dynamic side at inverse mass 0 slept through the contact's build.
        if !(a.is_dynamic && contact.inv_a > 0.0 && b.is_dynamic && contact.inv_b == 0.0) {
            continue;
        }
        let scale = buffers
            .extra
            .gravity_scales
            .get(contact.a as usize)
            .copied()
            .unwrap_or(1.0);
        // A new contact first takes the substep of gravity pressing the
        // awake side into the sleeper; a held one carries it already.
        let hold = if before == 0.0 {
            (-(config.gravity * scale).dot(contact.normal)).max(0.0) * sub_dt
        } else {
            0.0
        };
        let change = (contact.impulse - before) * contact.inv_a - hold;
        if change > config.sleep_threshold {
            probe.hits.push(Hit {
                sleeper,
                other: awake,
                change,
            });
        }
    }
    probe.impulses = impulses;
    // The frame's islands so far: a body pressed through a chain of contacts
    // by one much heavier.
    let parent = &buffers.island_parent;
    let root = |mut i: usize| {
        while parent[i] as usize != i {
            i = parent[i] as usize;
        }
        i
    };
    let mut heaviest: HashMap<usize, f32> = HashMap::new();
    let members: Vec<(usize, Entity, f32)> = proxies[..buffers.bodies]
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_dynamic && !p.sleeping && p.inv_mass > 0.0)
        .filter_map(|(i, p)| p.entity.map(|e| (root(i), e, p.inv_mass)))
        .collect();
    for &(r, _, inv) in &members {
        let most = heaviest.entry(r).or_insert(0.0);
        *most = most.max(1.0 / inv);
    }
    for &(r, entity, inv) in &members {
        let ratio = heaviest[&r] * inv;
        if ratio >= CRUSH_RATIO {
            probe.heavy.push((entity, ratio));
        }
    }
    buffers.oracle = Some(probe);
}

/// A box narrower than half a cell (`oracle-thin-box`, new-2): once it sinks
/// into a solid cell, the cell's side is its smaller overlap.
fn thin_box(world: &World, entity: Entity) -> bool {
    placed(world, entity).is_some_and(
        |(shape, _)| matches!(shape, Shape::Aabb { half_extents } if half_extents.x < CELL / 2.0),
    )
}

/// How much heavier a body pressing another, directly or through its
/// island, must be for a pass through a static to count as new-1's crush.
const CRUSH_RATIO: f32 = 4.0;

/// A body's sleep entry: whether it sleeps and its island tag, which a
/// contact wake leaves as it was.
fn sleep_entry(world: &World, entity: Entity) -> Option<(bool, u64)> {
    let buffers = world.get_resource::<PhysicsBuffers>()?;
    buffers
        .sleep
        .get(entity_key(entity))
        .map(|entry| (entry.sleeping, entry.island))
}

fn asleep(world: &World, entity: Entity) -> bool {
    world.is_alive(entity) && sleep_entry(world, entity).is_some_and(|(sleeping, _)| sleeping)
}

// ---------------------------------------------------------------------------
// Geometry the checks read: crossings and contacts.

/// A box face a centre crossed; y grows down, so `Top` is the min-y face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Left,
    Right,
    Top,
    Bottom,
}

impl Face {
    fn opposite(self) -> Self {
        match self {
            Face::Left => Face::Right,
            Face::Right => Face::Left,
            Face::Top => Face::Bottom,
            Face::Bottom => Face::Top,
        }
    }
}

/// How a centre came to be inside a solid: through a face, or it started
/// there (spawned, teleported or reshaped into it).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Entry {
    /// Through a face, from the start of the segment that entered, at a step.
    Through(Face, Vec2, u32),
    Unknown,
}

/// A centre that entered a solid through one face and left by the far one:
/// the faces, and where the segment that entered began.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Crossing {
    enter: Face,
    exit: Face,
    from: Vec2,
    /// The step whose segment entered.
    entered: u32,
}

fn strictly_inside(p: Vec2, aabb: &Aabb) -> bool {
    let (min, max) = (aabb.min(), aabb.max());
    p.x > min.x && p.x < max.x && p.y > min.y && p.y < max.y
}

/// Where the segment `p0`→`p1` meets the open box: its entry face, `None`
/// when `p0` is not outside, and its exit face, `None` when `p1` is not
/// outside; `None` when it misses the interior.
fn slab(p0: Vec2, p1: Vec2, aabb: &Aabb) -> Option<(Option<Face>, Option<Face>)> {
    let (min, max) = (aabb.min(), aabb.max());
    let d = p1 - p0;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    let (mut enter, mut exit) = (None, None);
    for axis in 0..2 {
        let (p, d, lo, hi) = if axis == 0 {
            (p0.x, d.x, min.x, max.x)
        } else {
            (p0.y, d.y, min.y, max.y)
        };
        let (low_face, high_face) = if axis == 0 {
            (Face::Left, Face::Right)
        } else {
            (Face::Top, Face::Bottom)
        };
        if d == 0.0 {
            if p <= lo || p >= hi {
                return None;
            }
            continue;
        }
        let (ta, tb) = ((lo - p) / d, (hi - p) / d);
        let (near, near_face, far, far_face) = if d > 0.0 {
            (ta, low_face, tb, high_face)
        } else {
            (tb, high_face, ta, low_face)
        };
        if near > t0 {
            t0 = near;
            enter = Some(near_face);
        }
        if far < t1 {
            t1 = far;
            exit = Some(far_face);
        }
    }
    (t0 < t1).then_some((enter, exit))
}

/// Track a centre's segment against one box; returns a crossing.
fn box_crossing(
    state: &mut Option<Entry>,
    p0: Vec2,
    p1: Vec2,
    aabb: &Aabb,
    step: u32,
) -> Option<Crossing> {
    if strictly_inside(p0, aabb) {
        state.get_or_insert(Entry::Unknown);
    } else {
        *state = None;
    }
    let (enter, exit) = slab(p0, p1, aabb)?;
    match (enter, exit) {
        (Some(enter), Some(exit)) => {
            *state = None;
            (exit == enter.opposite()).then_some(Crossing {
                enter,
                exit,
                from: p0,
                entered: step,
            })
        }
        (Some(enter), None) => {
            *state = Some(Entry::Through(enter, p0, step));
            None
        }
        (None, Some(exit)) => match state.take() {
            Some(Entry::Through(enter, from, entered)) if exit == enter.opposite() => {
                Some(Crossing {
                    enter,
                    exit,
                    from,
                    entered,
                })
            }
            _ => None,
        },
        (None, None) => None,
    }
}

/// Track a centre's segment against the map's solid cells as one region,
/// walking the cells it crosses; returns a crossing.
fn map_crossing(
    state: &mut Option<Entry>,
    p0: Vec2,
    p1: Vec2,
    map: &MapSpec,
    step: u32,
) -> Option<Crossing> {
    let solid = |col: i32, row: i32| map.cell(col, row) == Cell::Solid;
    let cell_of = |p: Vec2| {
        let u = (p - map.origin) / CELL;
        (u.x.floor() as i32, u.y.floor() as i32)
    };
    let (c0, r0) = cell_of(p0);
    if solid(c0, r0) && strictly_inside(p0, &map.cell_box(c0, r0)) {
        state.get_or_insert(Entry::Unknown);
    } else {
        *state = None;
    }
    // Clip to the map grown by a cell, outside which nothing is solid.
    let bounds = grown(map.bounds(), CELL);
    let delta = p1 - p0;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for axis in 0..2 {
        let (from, along, lo, hi) = if axis == 0 {
            (p0.x, delta.x, bounds.min().x, bounds.max().x)
        } else {
            (p0.y, delta.y, bounds.min().y, bounds.max().y)
        };
        if along == 0.0 {
            if from < lo || from > hi {
                return None;
            }
            continue;
        }
        let (ta, tb) = ((lo - from) / along, (hi - from) / along);
        t0 = t0.max(ta.min(tb));
        t1 = t1.min(ta.max(tb));
    }
    if t0 >= t1 {
        return None;
    }
    let start = (p0 + delta * t0 - map.origin) / CELL;
    let end = (p0 + delta * t1 - map.origin) / CELL;
    let span = end - start;
    let mut cell = (start.x.floor() as i32, start.y.floor() as i32);
    let step_x = if span.x > 0.0 { 1 } else { -1 };
    let step_y = if span.y > 0.0 { 1 } else { -1 };
    let next_t = |at: f32, cell: i32, step: i32, along: f32| {
        if along == 0.0 {
            f32::INFINITY
        } else {
            let edge = (if step > 0 { cell + 1 } else { cell }) as f32;
            (edge - at) / along
        }
    };
    let mut tx = next_t(start.x, cell.0, step_x, span.x);
    let mut ty = next_t(start.y, cell.1, step_y, span.y);
    let dtx = if span.x == 0.0 {
        f32::INFINITY
    } else {
        1.0 / span.x.abs()
    };
    let dty = if span.y == 0.0 {
        f32::INFINITY
    } else {
        1.0 / span.y.abs()
    };
    let mut inside = solid(cell.0, cell.1);
    let mut found = None;
    loop {
        let (next_at, along_x) = if tx <= ty { (tx, true) } else { (ty, false) };
        if next_at > 1.0 {
            break;
        }
        let (next, leave, enter) = if along_x {
            tx += dtx;
            if step_x > 0 {
                ((cell.0 + 1, cell.1), Face::Right, Face::Left)
            } else {
                ((cell.0 - 1, cell.1), Face::Left, Face::Right)
            }
        } else {
            ty += dty;
            if step_y > 0 {
                ((cell.0, cell.1 + 1), Face::Bottom, Face::Top)
            } else {
                ((cell.0, cell.1 - 1), Face::Top, Face::Bottom)
            }
        };
        let next_inside = solid(next.0, next.1);
        if inside && !next_inside {
            if let Some(Entry::Through(entered, from, at)) = state.take()
                && leave == entered.opposite()
                && found.is_none()
            {
                found = Some(Crossing {
                    enter: entered,
                    exit: leave,
                    from,
                    entered: at,
                });
            }
        } else if !inside && next_inside {
            *state = Some(Entry::Through(enter, p0, step));
        }
        inside = next_inside;
        cell = next;
    }
    found
}

/// The depth of `s` at `cs` into `o` at `co` (below 0 apart: minus the
/// gap) and the unit normal pushing `s` out of `o`.
fn shape_contact(s: Shape, cs: Vec2, o: Shape, co: Vec2) -> (f32, Vec2) {
    match (s, o) {
        (Shape::Aabb { half_extents: hs }, Shape::Aabb { half_extents: ho }) => {
            let delta = cs - co;
            let overlap = hs + ho - delta.abs();
            if overlap.x > 0.0 && overlap.y > 0.0 {
                if overlap.x < overlap.y {
                    (overlap.x, Vec2::new(delta.x.signum(), 0.0))
                } else {
                    (
                        overlap.y,
                        Vec2::new(
                            0.0,
                            if delta.y == 0.0 {
                                -1.0
                            } else {
                                delta.y.signum()
                            },
                        ),
                    )
                }
            } else {
                let gap = (-overlap).max(Vec2::ZERO);
                (-gap.length(), Vec2::new(0.0, -1.0))
            }
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            circle_box(cs, radius, co, half_extents)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            let (depth, normal) = circle_box(co, radius, cs, half_extents);
            (depth, -normal)
        }
        (Shape::Circle { radius: rs }, Shape::Circle { radius: ro }) => {
            let delta = cs - co;
            let distance = delta.length();
            (rs + ro - distance, delta.normalize_or(Vec2::NEG_Y))
        }
    }
}

/// A circle against a box: the depth and the normal pushing the circle out.
fn circle_box(center: Vec2, radius: f32, box_center: Vec2, half: Vec2) -> (f32, Vec2) {
    let local = center - box_center;
    let closest = local.clamp(-half, half);
    if closest == local {
        // The centre is inside: out through the nearest face.
        let to_face = half - local.abs();
        if to_face.x < to_face.y {
            (radius + to_face.x, Vec2::new(local.x.signum(), 0.0))
        } else {
            (
                radius + to_face.y,
                Vec2::new(
                    0.0,
                    if local.y == 0.0 {
                        -1.0
                    } else {
                        local.y.signum()
                    },
                ),
            )
        }
    } else {
        let delta = local - closest;
        let distance = delta.length();
        (radius - distance, delta / distance)
    }
}

// ---------------------------------------------------------------------------
// The runner.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Other {
    Body(usize),
    Static(usize),
    Cell(i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OtherKind {
    Wall,
    Static,
    Kinematic,
    Dynamic,
    Cell,
}

/// A sleeper's contact that holds it up, as a step left it.
#[derive(Debug, Clone, Copy)]
struct Touch {
    sleeper: usize,
    other: Other,
    kind: OtherKind,
    sleeper_tag: u64,
    other_sleeping: bool,
    other_tag: u64,
}

/// What the ops before a step did, by target.
#[derive(Debug, Default)]
struct Changes {
    gone: Vec<Target>,
    teleported: Vec<Target>,
    woken: Vec<Target>,
    reshaped: Vec<Target>,
    driven: Vec<Target>,
}

/// One failed check: its invariant, its kind, where and the finding that
/// explains it.
#[derive(Debug, Clone)]
struct Failure {
    invariant: u8,
    kind: &'static str,
    /// Each finding that explains it; empty for a new failure.
    findings: Vec<&'static str>,
    step: u32,
    detail: String,
}

impl Failure {
    fn key(&self) -> Key {
        let findings = if self.findings.is_empty() {
            "new".to_owned()
        } else {
            self.findings.join(" | ")
        };
        (self.invariant, self.kind, findings)
    }
}

/// A failure class: invariant, kind and the findings that explain it.
type Key = (u8, &'static str, String);

/// Whether a listed finding explains a class.
fn explained(key: &Key) -> bool {
    key.2
        .split(" | ")
        .any(|id| KNOWN.iter().any(|known| known.id == id))
}

/// What the probe counted over a scene.
#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    refreshes: u64,
    listed: u64,
    needed: u64,
    extra: u64,
    lifted: u64,
    segments: u64,
    hits: u64,
    touches: u64,
    sleeper_steps: u64,
    max_speed: f32,
}

#[derive(Debug, Default)]
struct Outcome {
    hashes: Vec<u64>,
    failures: Vec<Failure>,
    counts: Counts,
}

struct Live {
    statics: Vec<Entity>,
    bodies: Vec<Entity>,
}

fn mix(hash: &mut u64, value: u64) {
    *hash = (*hash ^ value).wrapping_mul(0x0000_0100_0000_01B3);
    *hash ^= *hash >> 29;
}

fn slope_of(code: u8) -> Slope {
    let rises = [SlopeRise::Full, SlopeRise::Low, SlopeRise::High];
    let corners = [
        SlopeCorner::BottomRight,
        SlopeCorner::BottomLeft,
        SlopeCorner::TopRight,
        SlopeCorner::TopLeft,
    ];
    let at = usize::from(code - 1);
    Slope {
        rise: rises[at / 4],
        corner: corners[at % 4],
    }
}

fn map_data(map: &MapSpec) -> TilemapData {
    let mut data = TilemapData::new(16, 16, map.cols, map.rows);
    let solid = data.add_tile("solid");
    let ledge = data.add_tile("ledge");
    data.set_tile_props(
        ledge,
        TileProps {
            one_way: true,
            ..TileProps::default()
        },
    );
    let slopes: Vec<_> = (1..=12)
        .map(|code| {
            let tile = data.add_tile(format!("slope{code}"));
            data.set_tile_props(
                tile,
                TileProps {
                    slope: Some(slope_of(code)),
                    ..TileProps::default()
                },
            );
            tile
        })
        .collect();
    let layer = data.add_layer("solid", LayerKind::Collision);
    for row in 0..map.rows {
        for col in 0..map.cols {
            let tile = match map.cell(col as i32, row as i32) {
                Cell::Empty => continue,
                Cell::Solid => solid,
                Cell::OneWay => ledge,
                Cell::Slope(code) => slopes[usize::from(code - 1)],
            };
            data.set_tile(layer, col, row, tile);
        }
    }
    data
}

fn build_world(scene: &Scene, probe: bool) -> (World, Live) {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(scene.dt);
    world.insert_resource(time);
    world.insert_resource(EventQueue::<CollisionEvent>::new());
    world.insert_resource(EventQueue::<ContactBegan>::new());
    world.insert_resource(EventQueue::<ContactEnded>::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, scene.gravity),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    if probe {
        world.insert_resource(PhysicsBuffers {
            oracle: Some(Box::default()),
            ..PhysicsBuffers::default()
        });
    }
    let statics = scene
        .statics
        .iter()
        .map(|spec| {
            let entity = world.spawn();
            world.insert(entity, Position(spec.center));
            world.insert(entity, Collider::aabb(spec.half));
            world.insert(entity, RigidBody::r#static());
            if spec.velocity != Vec2::ZERO {
                world.insert(entity, Velocity(spec.velocity));
            }
            entity
        })
        .collect();
    if let Some(map) = &scene.map {
        world
            .resource_mut::<TilemapRegistry>()
            .insert("oracle".into(), map_data(map));
        let entity = world.spawn();
        world.insert(entity, TilemapInstance::new("oracle", map.origin));
    }
    let bodies = scene
        .bodies
        .iter()
        .map(|spec| {
            let entity = world.spawn();
            world.insert(entity, Position(spec.center));
            world.insert(entity, Velocity(spec.velocity));
            world.insert(
                entity,
                Collider {
                    shape: spec.shape,
                    offset: Vec2::ZERO,
                },
            );
            let body = match spec.kind {
                Kind::Dynamic => RigidBody::dynamic()
                    .with_mass(spec.mass)
                    .with_gravity_scale(spec.gravity_scale)
                    .with_restitution(spec.restitution),
                Kind::Kinematic => RigidBody::kinematic(),
            };
            world.insert(entity, body.with_friction(spec.friction));
            entity
        })
        .collect();
    (world, Live { statics, bodies })
}

fn entity_of(live: &Live, target: Target) -> Entity {
    match target {
        Target::Static(i) => live.statics[i],
        Target::Body(i) => live.bodies[i],
    }
}

fn apply(world: &mut World, live: &Live, op: &OpSpec, changes: &mut Changes) {
    let entity = entity_of(live, op.target);
    if !world.is_alive(entity) {
        return;
    }
    match op.op {
        Op::Despawn => {
            world.despawn(entity);
            changes.gone.push(op.target);
        }
        Op::Teleport(at) => {
            teleport(world, entity, at);
            changes.teleported.push(op.target);
        }
        Op::Wake => {
            wake(world, entity);
            changes.woken.push(op.target);
        }
        Op::Reshape(shape) => {
            if let Some(collider) = world.get_mut::<Collider>(entity) {
                collider.shape = shape;
            }
            changes.reshaped.push(op.target);
        }
        Op::Drive(velocity) => {
            if let Some(v) = world.get_mut::<Velocity>(entity) {
                v.0 = velocity;
            }
            changes.driven.push(op.target);
        }
    }
}

/// A live collider's shape and centre.
fn placed(world: &World, entity: Entity) -> Option<(Shape, Vec2)> {
    if !world.is_alive(entity) {
        return None;
    }
    let collider = world.get::<Collider>(entity)?;
    let position = world.get::<Position>(entity)?;
    Some((collider.shape, collider.center(position.0)))
}

fn other_kind(scene: &Scene, other: Other) -> OtherKind {
    match other {
        Other::Static(i) if scene.statics[i].wall => OtherKind::Wall,
        Other::Static(_) => OtherKind::Static,
        Other::Body(i) if scene.bodies[i].kind == Kind::Kinematic => OtherKind::Kinematic,
        Other::Body(_) => OtherKind::Dynamic,
        Other::Cell(..) => OtherKind::Cell,
    }
}

/// The depth of `sleeper` into `other` as the world holds them now, `None`
/// when `other` is gone.
fn depth_now(
    world: &World,
    scene: &Scene,
    live: &Live,
    sleeper: Entity,
    other: Other,
) -> Option<f32> {
    let (shape, center) = placed(world, sleeper)?;
    match other {
        Other::Body(i) => {
            let (o_shape, o_center) = placed(world, live.bodies[i])?;
            Some(shape_contact(shape, center, o_shape, o_center).0)
        }
        Other::Static(i) => {
            let (o_shape, o_center) = placed(world, live.statics[i])?;
            Some(shape_contact(shape, center, o_shape, o_center).0)
        }
        Other::Cell(col, row) => {
            let aabb = scene.map.as_ref()?.cell_box(col, row);
            Some(
                shape_contact(
                    shape,
                    center,
                    Shape::Aabb {
                        half_extents: aabb.half_extents,
                    },
                    aabb.center,
                )
                .0,
            )
        }
    }
}

/// How deep a sleeper must sit in what holds it up for the contact to
/// count: the step rests bodies 0.25 to 1 px deep, and a shallower overlap
/// at the step's end can be a gap where the last substep built it, or a
/// side's graze.
const HELD_DEPTH: f32 = 0.2;

/// Every contact that holds a sleeper up: it overlaps the other side by
/// [`HELD_DEPTH`] or more, and the other side pushes it against gravity.
/// Slope cells are left out.
fn sleeper_touches(world: &World, scene: &Scene, live: &Live) -> Vec<Touch> {
    let up = -Vec2::new(0.0, scene.gravity).normalize();
    let mut touches = Vec::new();
    for (index, &sleeper) in live.bodies.iter().enumerate() {
        if !asleep(world, sleeper) {
            continue;
        }
        let Some((shape, center)) = placed(world, sleeper) else {
            continue;
        };
        let sleeper_tag = sleep_entry(world, sleeper).map_or(0, |(_, tag)| tag);
        let mut push =
            |other: Other, o_shape: Shape, o_center: Vec2, other_entity: Option<Entity>| {
                let (depth, normal) = shape_contact(shape, center, o_shape, o_center);
                if depth >= HELD_DEPTH && normal.dot(up) >= 0.2 {
                    let (other_sleeping, other_tag) = other_entity
                        .and_then(|e| sleep_entry(world, e))
                        .unwrap_or((false, 0));
                    touches.push(Touch {
                        sleeper: index,
                        other,
                        kind: other_kind(scene, other),
                        sleeper_tag,
                        other_sleeping,
                        other_tag,
                    });
                }
            };
        for (i, &entity) in live.statics.iter().enumerate() {
            if let Some((o_shape, o_center)) = placed(world, entity) {
                push(Other::Static(i), o_shape, o_center, None);
            }
        }
        for (i, &entity) in live.bodies.iter().enumerate() {
            if i == index {
                continue;
            }
            if let Some((o_shape, o_center)) = placed(world, entity) {
                push(Other::Body(i), o_shape, o_center, Some(entity));
            }
        }
        if let Some(map) = &scene.map {
            let aabb = shape_box(shape, center);
            let lo = ((aabb.min() - map.origin) / CELL).floor();
            let hi = ((aabb.max() - map.origin) / CELL).floor();
            for row in lo.y as i32..=hi.y as i32 {
                for col in lo.x as i32..=hi.x as i32 {
                    if matches!(map.cell(col, row), Cell::Solid | Cell::OneWay) {
                        let cell = map.cell_box(col, row);
                        push(
                            Other::Cell(col, row),
                            Shape::Aabb {
                                half_extents: cell.half_extents,
                            },
                            cell.center,
                            None,
                        );
                    }
                }
            }
        }
    }
    touches
}

fn label(target: Target) -> String {
    match target {
        Target::Static(i) => format!("static {i}"),
        Target::Body(i) => format!("body {i}"),
    }
}

fn other_label(other: Other) -> String {
    match other {
        Other::Body(i) => format!("body {i}"),
        Other::Static(i) => format!("static {i}"),
        Other::Cell(col, row) => format!("cell ({col}, {row})"),
    }
}

/// Run `scene`, with the probe and its checks or without, and return each
/// step's state hash and the failures.
fn run_scene(scene: &Scene, probe: bool) -> Outcome {
    let (mut world, live) = build_world(scene, probe);
    let mut outcome = Outcome::default();
    let index_of: HashMap<Entity, Target> = live
        .statics
        .iter()
        .enumerate()
        .map(|(i, &e)| (e, Target::Static(i)))
        .chain(
            live.bodies
                .iter()
                .enumerate()
                .map(|(i, &e)| (e, Target::Body(i))),
        )
        .collect();
    let name = |entity: Entity| {
        index_of
            .get(&entity)
            .map_or_else(|| format!("{entity:?}"), |&t| label(t))
    };
    let threshold = world.resource::<PhysicsConfig>().sleep_threshold;
    let slop = world.resource::<PhysicsConfig>().linear_slop;
    // Crossing states: (body, static) and (body, the map).
    let mut in_box: HashMap<(usize, usize), Option<Entry>> = HashMap::new();
    let mut in_map: HashMap<usize, Option<Entry>> = HashMap::new();
    let mut touches: Vec<Touch> = Vec::new();
    // Sleepers a flagged hit or begin left asleep, with the other side:
    // their contact should have woken them.
    let mut flagged: HashSet<(usize, Other)> = HashSet::new();
    // Each body's last contact with a body `CRUSH_RATIO` times its mass or
    // more.
    let mut last_heavy: HashMap<usize, (u32, f32)> = HashMap::new();
    // Sleepers whose collider was written while they slept: a shape write
    // wakes nothing, so such a sleeper can grow into another or shrink off
    // its support.
    let mut written_asleep: HashSet<usize> = HashSet::new();
    // Bodies that fell asleep held up by nothing (`ch-midair-sleep`).
    let mut slept_unheld: HashSet<usize> = HashSet::new();
    // A sleeper held up by a sleeper of another island tag, with no write
    // that made the contact: adoption missed the pair (`ch-tag-adoption`).
    let mut mismatched: HashSet<(usize, Other)> = HashSet::new();
    // Sleepers an ended contact already left hanging, with that failure's
    // findings: a later contact's end while they stay asleep continues it.
    let mut hung: HashMap<usize, Vec<&'static str>> = HashMap::new();
    // Each body's last step touching a static box that carries a velocity,
    // which the solver reads as a moving surface (`ch-static-velocity`).
    let mut moving_static_touch: HashMap<usize, u32> = HashMap::new();
    let mut ops = scene.ops.iter().peekable();
    for step in 0..scene.steps {
        let mut changes = Changes::default();
        while let Some(op) = ops.next_if(|op| op.step <= step) {
            if let (Target::Body(i), Op::Reshape(_)) = (op.target, op.op)
                && asleep(&world, live.bodies[i])
            {
                written_asleep.insert(i);
            }
            apply(&mut world, &live, op, &mut changes);
        }
        for target in changes.reshaped.iter().chain(&changes.gone) {
            if let Target::Static(s) = *target {
                in_box.retain(|&(_, static_index), _| static_index != s);
            }
        }
        let before: Vec<Option<(Vec2, bool)>> = live
            .bodies
            .iter()
            .map(|&e| placed(&world, e).map(|(_, center)| (center, asleep(&world, e))))
            .collect();

        physics_step(&mut world);

        let collisions = world
            .resource::<EventQueue<CollisionEvent>>()
            .iter_current()
            .count();
        let began: Vec<ContactBegan> = world
            .resource::<EventQueue<ContactBegan>>()
            .iter_current()
            .copied()
            .collect();
        let ended = world
            .resource::<EventQueue<ContactEnded>>()
            .iter_current()
            .count();
        world.resource_mut::<EventQueue<CollisionEvent>>().flush();
        world.resource_mut::<EventQueue<ContactBegan>>().flush();
        world.resource_mut::<EventQueue<ContactEnded>>().flush();

        // Invariant 4's hash: every spec entity's state, then the events.
        let mut hash = 0xCBF2_9CE4_8422_2325_u64 ^ scene.seed;
        for &entity in live.statics.iter().chain(&live.bodies) {
            if !world.is_alive(entity) {
                mix(&mut hash, 0);
                continue;
            }
            mix(&mut hash, 1);
            if let Some(position) = world.get::<Position>(entity) {
                mix(&mut hash, u64::from(position.0.x.to_bits()));
                mix(&mut hash, u64::from(position.0.y.to_bits()));
            }
            if let Some(velocity) = world.get::<Velocity>(entity) {
                mix(&mut hash, u64::from(velocity.0.x.to_bits()));
                mix(&mut hash, u64::from(velocity.0.y.to_bits()));
            }
            mix(&mut hash, u64::from(asleep(&world, entity)));
        }
        mix(&mut hash, collisions as u64);
        mix(&mut hash, began.len() as u64);
        mix(&mut hash, ended as u64);
        outcome.hashes.push(hash);
        if !probe {
            continue;
        }

        let mut found = Vec::new();
        let mut fail =
            |invariant: u8, kind: &'static str, findings: Vec<&'static str>, detail: String| {
                found.push(Failure {
                    invariant,
                    kind,
                    findings,
                    step,
                    detail,
                });
            };
        let recorded = std::mem::take(
            world
                .resource_mut::<PhysicsBuffers>()
                .oracle
                .as_mut()
                .expect("the probe stays in the buffers"),
        );
        let builds_seen = recorded.builds_seen;
        let repairs_seen = recorded.repairs_seen;
        let impulses = recorded.impulses.clone();

        // Invariant 1.
        let counts = &mut outcome.counts;
        counts.refreshes += recorded.checked;
        counts.listed += recorded.listed;
        counts.needed += recorded.needed;
        counts.extra += recorded.extra;
        counts.lifted += recorded.lifted.len() as u64;
        let side_name = |side: ProxySide| match side {
            ProxySide::Body(e) => name(e),
            ProxySide::Tile(at) => format!("tile at {at}"),
        };
        let moving_static = |side: ProxySide| match side {
            ProxySide::Body(e) => index_of.get(&e).is_some_and(
                |t| matches!(t, Target::Static(i) if scene.statics[*i].velocity != Vec2::ZERO),
            ),
            ProxySide::Tile(_) => false,
        };
        let mut seen = HashSet::new();
        for miss in &recorded.misses {
            let a = side_name(miss.a);
            let b = side_name(miss.b);
            if !seen.insert((miss.rule, a.clone(), b.clone())) {
                continue;
            }
            let finding = (miss.rule == PairRule::Allowance
                && (moving_static(miss.a) || moving_static(miss.b)))
            .then_some("ch-static-velocity");
            let kind = match miss.rule {
                PairRule::Duplicate => "pair listed twice",
                PairRule::Box => "pair boxes overlap, pair missed",
                PairRule::Allowance => "allowance reaches, pair missed",
            };
            fail(
                1,
                kind,
                finding.into_iter().collect(),
                format!("{a} and {b}, box gap {:.3}", miss.gap),
            );
        }
        if let Some(lift) = recorded.lifted.first() {
            let lift_scale = scene
                .bodies
                .iter()
                .map(|b| b.gravity_scale.abs())
                .fold(1.0_f32, f32::max);
            let finding = (lift_scale > 1.0).then_some("ch-gravity-scale-pairs");
            fail(
                1,
                "allowance above its own",
                finding.into_iter().collect(),
                format!(
                    "{} proxies, first {}: radius {:.3} against its own {:.3} (largest scale {lift_scale:.3})",
                    recorded.lifted.len(),
                    name(lift.entity),
                    lift.radius,
                    lift.own
                ),
            );
        }

        // Invariant 2: each dynamic body's substep path. A crossing that
        // entered within three steps of a contact with a body at least
        // `CRUSH_RATIO` times heavier, or in an island holding one, is the
        // crush of new-1.
        for &(light, ratio) in &recorded.heavy {
            if let Some(Target::Body(index)) = index_of.get(&light) {
                last_heavy.insert(*index, (step, ratio));
            }
        }
        let crush = |body: usize, entered: u32| -> Vec<&'static str> {
            last_heavy
                .get(&body)
                .filter(|(at, _)| *at + 3 >= entered)
                .map(|_| "oracle-crush")
                .into_iter()
                .collect()
        };
        let mut paths: BTreeMap<usize, Vec<Vec2>> = BTreeMap::new();
        for (index, start) in before.iter().enumerate() {
            if let Some((center, _)) = start
                && scene.bodies[index].kind == Kind::Dynamic
            {
                paths.insert(index, vec![*center]);
            }
        }
        for substep in &recorded.paths {
            for &(entity, center) in substep {
                if let Some(Target::Body(index)) = index_of.get(&entity)
                    && let Some(path) = paths.get_mut(index)
                {
                    path.push(center);
                }
            }
        }
        let static_boxes: Vec<(usize, Aabb)> = live
            .statics
            .iter()
            .enumerate()
            .filter_map(|(i, &e)| {
                placed(&world, e).map(|(shape, center)| (i, shape_box(shape, center)))
            })
            .collect();
        for (&body, path) in &paths {
            if path.iter().any(|p| !p.is_finite()) {
                fail(2, "non-finite centre", Vec::new(), format!("body {body}"));
                continue;
            }
            for segment in path.windows(2) {
                let (p0, p1) = (segment[0], segment[1]);
                counts_add_speed(
                    &mut outcome.counts,
                    (p1 - p0).length() / scene.dt * recorded.paths.len().max(1) as f32,
                );
                outcome.counts.segments += 1;
                for &(s, aabb) in &static_boxes {
                    let state = in_box.entry((body, s)).or_default();
                    if let Some(crossing) = box_crossing(state, p0, p1, &aabb, step) {
                        let mut findings = crush(body, crossing.entered);
                        if scene.statics[s].velocity != Vec2::ZERO
                            || moving_static_touch
                                .get(&body)
                                .is_some_and(|at| at + 3 >= crossing.entered)
                        {
                            findings.push("ch-static-velocity");
                        }
                        fail(
                            2,
                            "passed through a static box",
                            findings,
                            format!(
                                "body {body} through static {s} ({:?} to {:?}), {} to {p1}",
                                crossing.enter, crossing.exit, crossing.from
                            ),
                        );
                    }
                }
                if let Some(map) = &scene.map {
                    let state = in_map.entry(body).or_default();
                    if let Some(crossing) = map_crossing(state, p0, p1, map, step) {
                        let mut findings = crush(body, crossing.entered);
                        if thin_box(&world, live.bodies[body]) {
                            findings.push("oracle-thin-box");
                        }
                        if moving_static_touch
                            .get(&body)
                            .is_some_and(|at| at + 3 >= crossing.entered)
                        {
                            findings.push("ch-static-velocity");
                        }
                        fail(
                            2,
                            "passed through solid cells",
                            findings,
                            format!(
                                "body {body} through the map ({:?} to {:?}), {} to {p1}",
                                crossing.enter, crossing.exit, crossing.from
                            ),
                        );
                    }
                }
            }
        }

        // Invariant 3: hits, begins and ends.
        let mut hit_seen = HashSet::new();
        for hit in &recorded.hits {
            outcome.counts.hits += 1;
            if !asleep(&world, hit.sleeper) {
                continue;
            }
            if let (Some(Target::Body(s)), Some(Target::Body(o))) =
                (index_of.get(&hit.sleeper), index_of.get(&hit.other))
            {
                flagged.insert((*s, Other::Body(*o)));
            }
            if hit_seen.insert(hit.sleeper) {
                fail(
                    3,
                    "a contact's change left its sleeper asleep",
                    vec!["ch-arrival-wake"],
                    format!(
                        "{} on {}: normal velocity change {:.1} px/s",
                        name(hit.other),
                        name(hit.sleeper),
                        hit.change
                    ),
                );
            }
        }
        for begin in &began {
            if begin.speed <= threshold {
                continue;
            }
            for entity in [Some(begin.a), begin.b].into_iter().flatten() {
                let Some(Target::Body(index)) = index_of.get(&entity) else {
                    continue;
                };
                let slept = before[*index].is_some_and(|(_, sleeping)| sleeping);
                if slept && asleep(&world, entity) {
                    let other = if entity == begin.a {
                        begin.b
                    } else {
                        Some(begin.a)
                    };
                    if let Some(Target::Body(o)) = other.and_then(|o| index_of.get(&o)) {
                        flagged.insert((*index, Other::Body(*o)));
                    }
                    fail(
                        3,
                        "a begin above the threshold left its sleeper asleep",
                        vec!["ch-arrival-wake"],
                        format!(
                            "{} and {}: closing speed {:.1} px/s, {} asleep",
                            name(begin.a),
                            begin.b.map_or_else(|| "a tile".into(), name),
                            begin.speed,
                            name(entity)
                        ),
                    );
                }
            }
        }
        // A held contact that ended counts when it was the sleeper's last:
        // a sleeper still held up by something else does not hang.
        let touches_now = sleeper_touches(&world, scene, &live);
        for touch in &touches {
            let sleeper = live.bodies[touch.sleeper];
            if !asleep(&world, sleeper) || touches_now.iter().any(|t| t.sleeper == touch.sleeper) {
                continue;
            }
            if depth_now(&world, scene, &live, sleeper, touch.other)
                .is_some_and(|depth| depth > -slop)
            {
                continue;
            }
            let other_target = match touch.other {
                Other::Body(i) => Some(Target::Body(i)),
                Other::Static(i) => Some(Target::Static(i)),
                Other::Cell(..) => None,
            };
            let did =
                |list: &[Target], target: Option<Target>| target.is_some_and(|t| list.contains(&t));
            let other_entity = other_target.map(|t| entity_of(&live, t));
            let other_tag_now = other_entity
                .and_then(|e| sleep_entry(&world, e))
                .map_or(0, |(_, tag)| tag);
            let facts = EndFacts {
                kind: touch.kind,
                sleeper_reshaped: did(&changes.reshaped, Some(Target::Body(touch.sleeper))),
                other_reshaped: did(&changes.reshaped, other_target),
                other_gone: other_entity.is_some_and(|e| !world.is_alive(e)),
                other_sleeping: touch.other_sleeping,
                same_tag: touch.other_tag == touch.sleeper_tag,
                sleeper_tag: touch.sleeper_tag,
                written_asleep: written_asleep.contains(&touch.sleeper)
                    || matches!(touch.other, Other::Body(i) if written_asleep.contains(&i)),
                slept_unheld: slept_unheld.contains(&touch.sleeper),
                mismatched: mismatched.contains(&(touch.sleeper, touch.other)),
                other_awake_now: other_entity
                    .is_some_and(|e| world.is_alive(e) && !asleep(&world, e)),
                other_tag_now,
                flagged: flagged.contains(&(touch.sleeper, touch.other)),
            };
            let mut what = Vec::new();
            for (list, verb) in [
                (&changes.gone, "despawned"),
                (&changes.teleported, "teleported"),
                (&changes.woken, "woken"),
                (&changes.reshaped, "reshaped"),
                (&changes.driven, "driven"),
            ] {
                for target in list {
                    what.push(format!("{} {verb}", label(*target)));
                }
            }
            let mut findings = classify_end(&facts);
            match hung.get(&touch.sleeper) {
                Some(earlier) if findings.is_empty() => findings.clone_from(earlier),
                Some(_) => {}
                None => {
                    hung.insert(touch.sleeper, findings.clone());
                }
            }
            fail(
                3,
                "a held sleeper's contact ended, the sleeper asleep",
                findings,
                format!(
                    "body {} off {} ({:?}, {}; tags {:#x}/{:#x}); ops: {}",
                    touch.sleeper,
                    other_label(touch.other),
                    touch.kind,
                    if touch.other_sleeping {
                        "asleep"
                    } else {
                        "awake"
                    },
                    touch.sleeper_tag,
                    touch.other_tag,
                    if what.is_empty() {
                        "none".into()
                    } else {
                        what.join(", ")
                    }
                ),
            );
        }
        for (i, spec) in scene.statics.iter().enumerate() {
            if spec.velocity == Vec2::ZERO {
                continue;
            }
            let Some((o_shape, o_center)) = placed(&world, live.statics[i]) else {
                continue;
            };
            for (body, &entity) in live.bodies.iter().enumerate() {
                if let Some((shape, center)) = placed(&world, entity)
                    && shape_contact(shape, center, o_shape, o_center).0 > -1.0
                {
                    moving_static_touch.insert(body, step);
                }
            }
        }
        flagged.retain(|&(sleeper, _)| asleep(&world, live.bodies[sleeper]));
        written_asleep.retain(|&body| asleep(&world, live.bodies[body]));
        for (body, &entity) in live.bodies.iter().enumerate() {
            let slept_now = asleep(&world, entity) && !before[body].is_some_and(|(_, slept)| slept);
            if slept_now && !touches_now.iter().any(|t| t.sleeper == body) {
                slept_unheld.insert(body);
            }
        }
        slept_unheld.retain(|&body| asleep(&world, live.bodies[body]));
        for touch in &touches_now {
            let written = written_asleep.contains(&touch.sleeper)
                || matches!(touch.other, Other::Body(i) if written_asleep.contains(&i));
            if touch.kind == OtherKind::Dynamic
                && touch.other_sleeping
                && touch.other_tag != touch.sleeper_tag
                && !written
            {
                mismatched.insert((touch.sleeper, touch.other));
            }
        }
        mismatched.retain(|&(sleeper, _)| asleep(&world, live.bodies[sleeper]));
        hung.retain(|&sleeper, _| asleep(&world, live.bodies[sleeper]));
        touches = touches_now;
        outcome.counts.touches += touches.len() as u64;
        outcome.counts.sleeper_steps +=
            live.bodies.iter().filter(|&&e| asleep(&world, e)).count() as u64;

        // The probe keeps its counters and carried impulses across steps.
        let probe = world
            .resource_mut::<PhysicsBuffers>()
            .oracle
            .as_mut()
            .expect("the probe stays in the buffers");
        probe.builds_seen = builds_seen;
        probe.repairs_seen = repairs_seen;
        probe.impulses = impulses;
        outcome.failures.extend(found);
    }
    outcome
}

fn counts_add_speed(counts: &mut Counts, speed: f32) {
    counts.max_speed = counts.max_speed.max(speed);
}

/// What an ended contact's sleeper and other side went through.
#[derive(Debug, Clone, Copy)]
// Independent facts the classifier reads, not the states of one machine.
#[allow(clippy::struct_excessive_bools)]
struct EndFacts {
    kind: OtherKind,
    sleeper_reshaped: bool,
    other_reshaped: bool,
    other_gone: bool,
    other_sleeping: bool,
    /// The two sides' island tags were one when the contact held.
    same_tag: bool,
    /// The other side is alive and awake after the step, and its sleep
    /// entry's tag then, which a contact wake leaves as it was.
    other_awake_now: bool,
    other_tag_now: u64,
    /// The sleeper's tag when the contact held.
    sleeper_tag: u64,
    /// A side's collider was written while it slept, which may have made
    /// the contact or ended it.
    written_asleep: bool,
    /// The sleeper fell asleep held up by nothing: in mid-air.
    slept_unheld: bool,
    /// The two slept under different island tags while it held, with no
    /// write that made it.
    mismatched: bool,
    /// A hit or begin of this pair above the threshold left the sleeper
    /// asleep before.
    flagged: bool,
}

// ---------------------------------------------------------------------------
// Known findings and the classifier.

/// A finding the oracle trips: an open row of `docs/known-issues.md`, named
/// by its ID there, and the failures it explains, which the oracle skips.
#[derive(Debug, Clone, Copy)]
struct Known {
    id: &'static str,
    source: &'static str,
    skips: &'static str,
    /// A small hand-built scene that trips it while it is open: a fix
    /// elsewhere leaves it alone, where it can reshuffle a random seed.
    witness: fn() -> Scene,
}

// Witnesses: each scene's floor top is at y 100 and its boxes rest 0.25 px
// into what holds them.

fn witness_scene(
    gravity: f32,
    steps: u32,
    statics: Vec<StaticSpec>,
    bodies: Vec<BodySpec>,
    ops: Vec<OpSpec>,
) -> Scene {
    Scene {
        seed: 0,
        gravity,
        dt: 1.0 / 60.0,
        steps,
        statics,
        map: None,
        bodies,
        ops,
    }
}

fn floor(half_y: f32) -> StaticSpec {
    StaticSpec {
        center: Vec2::new(0.0, 100.0 + half_y),
        half: Vec2::new(600.0, half_y),
        velocity: Vec2::ZERO,
        wall: true,
    }
}

fn block(half: f32, center: Vec2, velocity: Vec2, mass: f32) -> BodySpec {
    BodySpec {
        kind: Kind::Dynamic,
        shape: Shape::Aabb {
            half_extents: Vec2::splat(half),
        },
        center,
        velocity,
        mass,
        gravity_scale: 1.0,
        restitution: 0.0,
        friction: 0.0,
    }
}

fn op(step: u32, body: usize, op: Op) -> OpSpec {
    OpSpec {
        step,
        target: Target::Body(body),
        op,
    }
}

/// A circle of radius 19 at rest on an 8 px floor and a circle 300 times
/// its mass falling onto it at 1,000 px/s, at steps of 1/30 s: the light
/// circle ends below the floor (new-1's sweep: 26 of 864 cases pass).
fn witness_crush() -> Scene {
    crush_scene(
        Shape::Circle { radius: 19.0 },
        Shape::Circle { radius: 30.0 },
        300.0,
        1000.0,
        4.0,
        1.0 / 30.0,
    )
}

/// new-1's rig: a light body (mass 0.25) at rest on a static floor of half
/// height `half_y`, and a body `ratio` times its mass that meets it within a
/// step, falling at `speed`; gravity 3,600, 40 steps of `dt`.
fn crush_scene(
    victim: Shape,
    pusher: Shape,
    ratio: f32,
    speed: f32,
    half_y: f32,
    dt: f32,
) -> Scene {
    let rest = 100.0 - victim.half_extents().y - 0.25;
    let start = rest - victim.half_extents().y - pusher.half_extents().y - 0.5 - speed * dt;
    let mut scene = witness_scene(
        3600.0,
        40,
        vec![floor(half_y)],
        vec![
            BodySpec {
                shape: victim,
                ..block(6.0, Vec2::new(0.0, rest), Vec2::ZERO, 0.25)
            },
            BodySpec {
                shape: pusher,
                ..block(
                    6.0,
                    Vec2::new(0.0, start),
                    Vec2::new(0.0, speed),
                    0.25 * ratio,
                )
            },
        ],
        Vec::new(),
    );
    scene.dt = dt;
    scene
}

/// A box asleep on the floor (its top at y 84.275); a second box set 0.25 px
/// into it at step 45, awake, and a third dropped from 150 px onto that one
/// at step 50: the hit reaches the sleeper through the awake box at inverse
/// mass 0.
fn witness_arrival_wake() -> Scene {
    witness_scene(
        3600.0,
        90,
        vec![floor(10.0)],
        vec![
            block(8.0, Vec2::new(0.0, 91.75), Vec2::ZERO, 1.0),
            block(8.0, Vec2::new(300.0, 91.75), Vec2::ZERO, 1.0),
            block(8.0, Vec2::new(-300.0, 91.75), Vec2::ZERO, 1.0),
        ],
        vec![
            op(45, 1, Op::Teleport(Vec2::new(0.0, 76.53))),
            op(50, 2, Op::Teleport(Vec2::new(0.0, -90.75))),
        ],
    )
}

/// A box asleep on the floor whose collider shrinks to half at step 60.
fn witness_write_wake() -> Scene {
    witness_scene(
        900.0,
        62,
        vec![floor(10.0)],
        vec![block(8.0, Vec2::new(0.0, 91.75), Vec2::ZERO, 1.0)],
        vec![op(
            60,
            0,
            Op::Reshape(Shape::Aabb {
                half_extents: Vec2::splat(4.0),
            }),
        )],
    )
}

/// A two-box stack asleep on the floor; a light ball (mass 0.05) rolls into
/// the bottom box at step 77, whose contact wake takes it alone out of the
/// island and moves it at about 14 px/s, under the threshold, so the top box
/// stays asleep; the bottom box despawns at step 100 and the top box hangs.
fn witness_island_tags() -> Scene {
    let mut ball = block(6.0, Vec2::new(-400.0, 93.75), Vec2::new(300.0, 0.0), 0.05);
    ball.shape = Shape::Circle { radius: 6.0 };
    witness_scene(
        900.0,
        110,
        vec![floor(10.0)],
        vec![
            block(8.0, Vec2::new(0.0, 91.75), Vec2::ZERO, 1.0),
            block(8.0, Vec2::new(0.0, 75.5), Vec2::ZERO, 1.0),
            ball,
        ],
        vec![op(100, 0, Op::Despawn)],
    )
}

/// Two boxes apart on the floor, one with gravity scale 3: the other's
/// allowance takes the lift.
fn witness_gravity_scale_pairs() -> Scene {
    let mut scaled = block(8.0, Vec2::new(100.0, 91.75), Vec2::ZERO, 1.0);
    scaled.gravity_scale = 3.0;
    witness_scene(
        900.0,
        2,
        vec![floor(10.0)],
        vec![
            block(8.0, Vec2::new(-100.0, 91.75), Vec2::ZERO, 1.0),
            scaled,
        ],
        Vec::new(),
    )
}

/// A box of gravity scale 0.04 asleep in mid-air (at y 4.54 after half a
/// second), a box teleported 2 px into it from below at step 40 and
/// despawned at step 41, still touching: the sleeper hangs with nothing
/// under it, where a box that fell away at the threshold's speed woke it.
fn witness_midair_sleep() -> Scene {
    let mut floater = block(8.0, Vec2::ZERO, Vec2::ZERO, 1.0);
    floater.gravity_scale = 0.04;
    witness_scene(
        900.0,
        60,
        vec![floor(10.0)],
        vec![
            floater,
            block(8.0, Vec2::new(300.0, 91.75), Vec2::ZERO, 1.0),
        ],
        vec![
            op(40, 1, Op::Teleport(Vec2::new(0.0, 18.5))),
            op(41, 1, Op::Despawn),
        ],
    )
}

/// Seed 13734's scene up to its step 18, where a thin box (half 4.4 by
/// 19.3) pushed into a solid cell beside a ceiling slope falls through it.
/// A random scene, so a fix elsewhere can move it: reduce another seed then.
fn witness_thin_box() -> Scene {
    let mut scene = generate(13734);
    scene.steps = 19;
    scene.ops.retain(|op| op.step < 19);
    scene
}

/// A box at rest on a 16 px static floor that carries a velocity of
/// (0, 200), through which it sinks.
fn witness_static_velocity() -> Scene {
    let mut ground = floor(8.0);
    ground.velocity = Vec2::new(0.0, 200.0);
    witness_scene(
        900.0,
        120,
        vec![ground],
        vec![block(8.0, Vec2::new(0.0, 91.75), Vec2::ZERO, 1.0)],
        Vec::new(),
    )
}

const KNOWN: &[Known] = &[
    Known {
        id: "oracle-thin-box",
        source: "known-issues' oracle-thin-box row, from new-2",
        skips: "invariant 2: a box narrower than half a cell passes solid cells",
        witness: witness_thin_box,
    },
    Known {
        id: "ch-midair-sleep",
        source: "known-issues' ch-midair-sleep row",
        skips: "invariant 3: a sleeper that fell asleep held up by nothing loses the last contact \
                that held it since, the sleeper asleep after the step",
        witness: witness_midair_sleep,
    },
    Known {
        id: "oracle-crush",
        source: "known-issues' oracle-crush row, from new-1",
        skips: "invariant 2: a body passes a static box or solid cells it entered within three \
                steps of touching an awake dynamic body four or more times its mass, directly or \
                through the step's island",
        witness: witness_crush,
    },
    Known {
        id: "ch-arrival-wake",
        source: "known-issues' ch-arrival-wake row",
        skips: "invariant 3: a contact holding a sleeper at inverse mass 0 that changes the awake \
                side's normal velocity by more than the sleep threshold in a substep, or a begin \
                above the threshold, with the sleeper asleep after the step",
        witness: witness_arrival_wake,
    },
    Known {
        id: "ch-write-wake",
        source: "known-issues' ch-write-wake row",
        skips: "invariant 3: a sleeper's held contact ends because its own collider, or a \
                sleeping support's, was written, the sleeper asleep after the step",
        witness: witness_write_wake,
    },
    Known {
        id: "ch-island-tags",
        source: "known-issues' ch-island-tags row",
        skips: "invariant 3: a sleeper held up by an awake body that still carries the \
                sleeper's island tag (a contact woke it out of the island), the contact ending \
                with the sleeper asleep",
        witness: witness_island_tags,
    },
    Known {
        id: "ch-gravity-scale-pairs",
        source: "known-issues' ch-gravity-scale-pairs row",
        skips: "invariant 1: a moving proxy's fresh allowance above its own while some body's \
                gravity scale is above 1 (a cost: no pair is missed)",
        witness: witness_gravity_scale_pairs,
    },
    Known {
        id: "ch-static-velocity",
        source: "known-issues' ch-static-velocity row",
        skips: "invariants 1 and 2: a pair with a static box that has a velocity missed by the \
                allowance, or a body passing a static box or solid cells it entered within three \
                steps of touching such a box (which carries it as a moving surface)",
        witness: witness_static_velocity,
    },
];

/// The findings that explain an ended contact's sleeper left asleep.
fn classify_end(facts: &EndFacts) -> Vec<&'static str> {
    let mut findings = Vec::new();
    // A collider written, now or while asleep: shape writes wake nothing.
    if facts.sleeper_reshaped || facts.written_asleep {
        findings.push("ch-write-wake");
    }
    match facts.kind {
        // A kinematic support is not in the support record.
        OtherKind::Kinematic => findings.push("ch-kinematic-support"),
        OtherKind::Dynamic => {
            // Asleep in mid-air before anything held it.
            if facts.slept_unheld {
                findings.push("ch-midair-sleep");
            }
            // A hit or begin from it should have woken the sleeper.
            if facts.flagged {
                findings.push("ch-arrival-wake");
            }
            // A sleeping support's shape write wakes neither side.
            if facts.other_sleeping
                && facts.other_reshaped
                && !facts.other_gone
                && !findings.contains(&"ch-write-wake")
            {
                findings.push("ch-write-wake");
            }
            // Asleep under another island's tag with no write that made the
            // contact, now or before: adoption missed the pair.
            if facts.mismatched
                || (facts.other_sleeping && !facts.same_tag && !facts.written_asleep)
            {
                findings.push("ch-tag-adoption");
            }
            // Awake, before the step or after it, with the sleeper's tag: a
            // contact woke it out of the island and left the sleeper.
            let woken_out = (!facts.other_sleeping && facts.same_tag)
                || (facts.other_awake_now && facts.other_tag_now == facts.sleeper_tag);
            if woken_out {
                findings.push("ch-island-tags");
            }
        }
        _ => {}
    }
    findings
}

// ---------------------------------------------------------------------------
// Running seeds and reporting.

#[derive(Debug, Default)]
struct Report {
    /// By failure key: the count and the first few examples, by seed.
    classes: BTreeMap<Key, (u64, Vec<(u64, Failure)>)>,
    /// By (gravity, step): refreshes, pairs listed, needed and extra, lifts.
    regimes: BTreeMap<(u32, u32), Counts>,
    seeds: u64,
    steps: u64,
}

impl Report {
    fn add(&mut self, scene: &Scene, outcome: &Outcome) {
        self.seeds += 1;
        self.steps += u64::from(scene.steps);
        for failure in &outcome.failures {
            let entry = self.classes.entry(failure.key()).or_default();
            entry.0 += 1;
            if entry.1.len() < 4 && entry.1.iter().all(|(seed, _)| *seed != scene.seed) {
                entry.1.push((scene.seed, failure.clone()));
            }
        }
        let regime = self
            .regimes
            .entry((scene.gravity as u32, (1.0 / scene.dt).round() as u32))
            .or_default();
        let c = &outcome.counts;
        regime.refreshes += c.refreshes;
        regime.listed += c.listed;
        regime.needed += c.needed;
        regime.extra += c.extra;
        regime.lifted += c.lifted;
        regime.segments += c.segments;
        regime.hits += c.hits;
        regime.touches += c.touches;
        regime.sleeper_steps += c.sleeper_steps;
        regime.max_speed = regime.max_speed.max(c.max_speed);
    }

    fn text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{} seeds, {} steps", self.seeds, self.steps);
        for ((gravity, rate), c) in &self.regimes {
            let per = |v: u64| v as f64 / c.refreshes.max(1) as f64;
            let _ = writeln!(
                out,
                "gravity {gravity} at 1/{rate} s: {} refreshes, per refresh {:.1} listed, {:.1} needed, \
                 {:.1} extra; {} lifted allowances; {} segments, fastest {:.0} px/s; {} sleeper steps, \
                 {} held contacts, {} sleeper hits",
                c.refreshes,
                per(c.listed),
                per(c.needed),
                per(c.extra),
                c.lifted,
                c.segments,
                c.max_speed,
                c.sleeper_steps,
                c.touches,
                c.hits
            );
        }
        for known in KNOWN {
            let trips: u64 = self
                .classes
                .iter()
                .filter(|((_, _, findings), _)| findings.split(" | ").any(|id| id == known.id))
                .map(|(_, (count, _))| count)
                .sum();
            let _ = writeln!(
                out,
                "KNOWN {} ({}): {trips} failures skipped; {}",
                known.id, known.source, known.skips
            );
        }
        for ((invariant, kind, finding), (count, examples)) in &self.classes {
            let _ = writeln!(out, "invariant {invariant} [{finding}] {kind}: {count}");
            for (seed, failure) in examples {
                let _ = writeln!(
                    out,
                    "    seed {seed} step {}: {}",
                    failure.step, failure.detail
                );
            }
        }
        out
    }

    /// Failures no listed finding explains.
    fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (key, (count, examples)) in &self.classes {
            let (invariant, kind, finding) = key;
            if explained(key) {
                continue;
            }
            let (seed, failure) = &examples[0];
            problems.push(format!(
                "invariant {invariant} ({kind}) failed {count} times, unexplained ({finding}); \
                 first at seed {seed} step {}: {}",
                failure.step, failure.detail
            ));
        }
        problems
    }
}

/// Run each seed's scene with the probe and without, comparing their
/// hashes step by step (invariant 4).
fn run_seeds(seeds: impl IntoIterator<Item = u64>) -> Report {
    let mut report = Report::default();
    for seed in seeds {
        let scene = generate(seed);
        let mut outcome = run_scene(&scene, true);
        let again = run_scene(&scene, false);
        if let Some(step) =
            (0..outcome.hashes.len()).find(|&i| outcome.hashes.get(i) != again.hashes.get(i))
        {
            outcome.failures.push(Failure {
                invariant: 4,
                kind: "two runs of one seed hashed apart",
                findings: Vec::new(),
                step: step as u32,
                detail: format!(
                    "{:#x} against {:#x}",
                    outcome.hashes[step],
                    again.hashes.get(step).copied().unwrap_or(0)
                ),
            });
        }
        report.add(&scene, &outcome);
    }
    report
}

/// The default run's seeds, about 25 s in a debug build: their low bits
/// cover both gravities and both steps. Eight tests share them, an eighth
/// each, so the test harness's threads run them side by side.
const DEFAULT_SEEDS: std::ops::Range<u64> = 0..1000;

/// The default seeds' eighth `part` (0 to 7): every failure explained.
fn holds_on_default_eighth(part: u64) {
    let len = DEFAULT_SEEDS.end - DEFAULT_SEEDS.start;
    let start = DEFAULT_SEEDS.start + len * part / 8;
    let end = DEFAULT_SEEDS.start + len * (part + 1) / 8;
    let report = run_seeds(start..end);
    println!("{}", report.text());
    let problems = report.problems();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_1() {
    holds_on_default_eighth(0);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_2() {
    holds_on_default_eighth(1);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_3() {
    holds_on_default_eighth(2);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_4() {
    holds_on_default_eighth(3);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_5() {
    holds_on_default_eighth(4);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_6() {
    holds_on_default_eighth(5);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_7() {
    holds_on_default_eighth(6);
}

#[test]
fn physics_oracle_holds_on_the_default_seeds_8() {
    holds_on_default_eighth(7);
}

#[test]
fn physics_oracle_witnesses_trip_their_findings() {
    let problems = witness_problems();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Each listed finding's witness trips it, and every failure a witness
/// shows is explained: a witness that trips nothing is a fix to prove by
/// removing the entry.
fn witness_problems() -> Vec<String> {
    let mut problems = Vec::new();
    for known in KNOWN {
        let scene = (known.witness)();
        let outcome = run_scene(&scene, true);
        if !outcome
            .failures
            .iter()
            .any(|failure| failure.findings.contains(&known.id))
        {
            problems.push(format!(
                "{} ({}): its witness trips nothing; if it is fixed, remove its entry",
                known.id, known.source
            ));
        }
        for failure in &outcome.failures {
            if !explained(&failure.key()) {
                problems.push(format!(
                    "{}'s witness failed unexplained at step {}: invariant {} ({}): {}",
                    known.id, failure.step, failure.invariant, failure.kind, failure.detail
                ));
            }
        }
    }
    problems
}

/// `ORACLE_SEEDS=a..b` (default 0..400): the long mode, run once in
/// release.
#[test]
#[ignore = "long mode: many seeds, run in release"]
fn physics_oracle_holds_on_many_seeds() {
    let seeds = std::env::var("ORACLE_SEEDS").unwrap_or_else(|_| "0..400".into());
    let (a, b) = seeds.split_once("..").expect("ORACLE_SEEDS is a..b");
    let report = run_seeds(a.parse::<u64>().unwrap()..b.parse::<u64>().unwrap());
    println!("{}", report.text());
    let problems = report.problems();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// `ORACLE_SEED=n ORACLE_BODY=i ORACLE_STEPS=a..b`: one seed's scene, its
/// failures, and body `i`'s state over steps `a..b`, for a finder.
#[test]
#[ignore = "a finder's trace of one seed"]
fn physics_oracle_trace() {
    let seed: u64 = std::env::var("ORACLE_SEED").map_or(0, |s| s.parse().unwrap());
    let scene = match std::env::var("ORACLE_WITNESS") {
        Ok(id) => (KNOWN
            .iter()
            .find(|k| k.id == id)
            .expect("a listed id")
            .witness)(),
        Err(_) => generate(seed),
    };
    println!("{scene:#?}");
    let outcome = run_scene(&scene, true);
    for failure in &outcome.failures {
        if failure.invariant == 1 && failure.kind == "allowance above its own" {
            continue;
        }
        println!(
            "step {} invariant {} [{:?}] {}: {}",
            failure.step, failure.invariant, failure.findings, failure.kind, failure.detail
        );
    }
    let Ok(body) = std::env::var("ORACLE_BODY") else {
        return;
    };
    let body: usize = body.parse().unwrap();
    let steps = std::env::var("ORACLE_STEPS").unwrap_or_else(|_| "0..0".into());
    let (a, b) = steps.split_once("..").unwrap();
    let (a, b): (u32, u32) = (a.parse().unwrap(), b.parse().unwrap());
    let (mut world, live) = build_world(&scene, false);
    let mut ops = scene.ops.iter().peekable();
    for step in 0..b.min(scene.steps) {
        let mut changes = Changes::default();
        while let Some(op) = ops.next_if(|op| op.step <= step) {
            if (a..b).contains(&step) {
                println!("  op before step {step}: {op:?}");
            }
            apply(&mut world, &live, op, &mut changes);
        }
        physics_step(&mut world);
        if (a..b).contains(&step) {
            let e = live.bodies[body];
            println!(
                "  after step {step}: position {:?} velocity {:?} asleep {} entry {:?}",
                world.get::<Position>(e).map(|p| p.0),
                world.get::<Velocity>(e).map(|v| v.0),
                asleep(&world, e),
                sleep_entry(&world, e)
            );
            let Some((_, here)) = placed(&world, e) else {
                continue;
            };
            for (i, &other) in live.bodies.iter().enumerate() {
                if i == body {
                    continue;
                }
                if let Some((shape, at)) = placed(&world, other)
                    && (at - here).length() < 120.0
                {
                    println!(
                        "      body {i} ({:?}, mass {:.3}) at {at} velocity {:?}",
                        shape,
                        scene.bodies[i].mass,
                        world.get::<Velocity>(other).map(|v| v.0)
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reduction, for a finder: the smallest scene that still fails one way.

/// The first step at which `scene` fails as `key` says, invariant 4 by two
/// runs.
fn fails_as(scene: &Scene, key: &Key) -> Option<u32> {
    let outcome = run_scene(scene, true);
    if key.0 == 4 {
        let again = run_scene(scene, false);
        return (0..outcome.hashes.len())
            .find(|&i| outcome.hashes.get(i) != again.hashes.get(i))
            .map(|i| i as u32);
    }
    outcome
        .failures
        .iter()
        .find(|failure| failure.key() == *key)
        .map(|failure| failure.step)
}

/// `scene` without body `index`: its ops go and later bodies' shift.
fn without_body(scene: &Scene, index: usize) -> Scene {
    let mut next = scene.clone();
    next.bodies.remove(index);
    next.ops.retain(|op| op.target != Target::Body(index));
    for op in &mut next.ops {
        if let Target::Body(i) = &mut op.target
            && *i > index
        {
            *i -= 1;
        }
    }
    next
}

fn without_static(scene: &Scene, index: usize) -> Scene {
    let mut next = scene.clone();
    next.statics.remove(index);
    next.ops.retain(|op| op.target != Target::Static(index));
    for op in &mut next.ops {
        if let Target::Static(i) = &mut op.target
            && *i > index
        {
            *i -= 1;
        }
    }
    next
}

/// Greedy one-at-a-time removal of steps, ops, bodies, statics, the map
/// and its cells, and simplification of each body's parameters, while the
/// scene still fails as `key`.
fn reduce(scene: &Scene, key: &Key) -> Scene {
    let mut best = scene.clone();
    let Some(step) = fails_as(&best, key) else {
        return best;
    };
    best.steps = step + 1;
    best.ops.retain(|op| op.step <= step);
    let mut changed = true;
    while changed {
        changed = false;
        let try_take = |candidate: Scene, best: &mut Scene| {
            if let Some(step) = fails_as(&candidate, key) {
                *best = candidate;
                best.steps = step + 1;
                best.ops.retain(|op| op.step <= step);
                true
            } else {
                false
            }
        };
        for index in (0..best.ops.len()).rev() {
            let mut candidate = best.clone();
            candidate.ops.remove(index);
            changed |= try_take(candidate, &mut best);
        }
        for index in (0..best.bodies.len()).rev() {
            changed |= try_take(without_body(&best, index), &mut best);
        }
        for index in (0..best.statics.len()).rev() {
            changed |= try_take(without_static(&best, index), &mut best);
        }
        if best.map.is_some() {
            let mut candidate = best.clone();
            candidate.map = None;
            if try_take(candidate, &mut best) {
                changed = true;
            } else {
                let cells = best.map.as_ref().map_or(0, |m| m.cells.len());
                for index in 0..cells {
                    if best
                        .map
                        .as_ref()
                        .is_some_and(|m| m.cells[index] == Cell::Empty)
                    {
                        continue;
                    }
                    let mut candidate = best.clone();
                    if let Some(map) = &mut candidate.map {
                        map.cells[index] = Cell::Empty;
                    }
                    changed |= try_take(candidate, &mut best);
                }
            }
        }
        for index in 0..best.bodies.len() {
            let simpler: [fn(&mut BodySpec); 4] = [
                |b| b.restitution = 0.0,
                |b| b.friction = 0.0,
                |b| b.gravity_scale = 1.0,
                |b| b.mass = 1.0,
            ];
            for simplify in simpler {
                let mut candidate = best.clone();
                let before = format!("{:?}", candidate.bodies[index]);
                simplify(&mut candidate.bodies[index]);
                if format!("{:?}", candidate.bodies[index]) != before {
                    changed |= try_take(candidate, &mut best);
                }
            }
        }
    }
    best
}

fn vec_source(v: Vec2) -> String {
    format!("Vec2::new({:?}, {:?})", v.x, v.y)
}

fn shape_source(shape: Shape) -> String {
    match shape {
        Shape::Aabb { half_extents } => format!("Collider::aabb({})", vec_source(half_extents)),
        Shape::Circle { radius } => format!("Collider::circle({radius:?})"),
    }
}

/// A reduced scene as a test in the finders' form, for `step.rs`, whose
/// helpers it uses; the assertion is the finder's to write.
fn scene_source(scene: &Scene) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "    let mut world = seed_world();");
    let _ = writeln!(
        out,
        "    world.insert_resource(EventQueue::<ContactBegan>::new());"
    );
    let _ = writeln!(
        out,
        "    world.insert_resource(EventQueue::<ContactEnded>::new());"
    );
    let _ = writeln!(
        out,
        "    world.resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, {:?});",
        scene.gravity
    );
    let _ = writeln!(
        out,
        "    world.resource_mut::<Time>().advance_frame({:?});",
        scene.dt
    );
    for (i, s) in scene.statics.iter().enumerate() {
        let _ = writeln!(
            out,
            "    let static_{i} = spawn_static(&mut world, Collider::aabb({}), {});",
            vec_source(s.half),
            vec_source(s.center)
        );
        if s.velocity != Vec2::ZERO {
            let _ = writeln!(
                out,
                "    world.insert(static_{i}, Velocity({}));",
                vec_source(s.velocity)
            );
        }
    }
    if let Some(map) = &scene.map {
        let _ = writeln!(
            out,
            "    // The map: `#` solid, `=` one-way, `a`-`l` slope kinds 1-12."
        );
        let _ = writeln!(out, "    let rows = [");
        for row in 0..map.rows {
            let line: String = (0..map.cols)
                .map(|col| match map.cell(col as i32, row as i32) {
                    Cell::Empty => '.',
                    Cell::Solid => '#',
                    Cell::OneWay => '=',
                    Cell::Slope(code) => (b'a' + code - 1) as char,
                })
                .collect();
            let _ = writeln!(out, "        \"{line}\",");
        }
        let _ = writeln!(out, "    ];");
        let _ = writeln!(
            out,
            "    spawn_oracle_map(&mut world, &rows, {});",
            vec_source(map.origin)
        );
    }
    for (i, b) in scene.bodies.iter().enumerate() {
        let body = match b.kind {
            Kind::Dynamic => format!(
                "RigidBody::dynamic().with_mass({:?}).with_gravity_scale({:?}).with_restitution({:?}).with_friction({:?})",
                b.mass, b.gravity_scale, b.restitution, b.friction
            ),
            Kind::Kinematic => format!("RigidBody::kinematic().with_friction({:?})", b.friction),
        };
        let _ = writeln!(
            out,
            "    let body_{i} = spawn_body(&mut world, {body}, {}, {}, {});",
            shape_source(b.shape),
            vec_source(b.center),
            vec_source(b.velocity)
        );
    }
    let _ = writeln!(out, "    for step in 0..{} {{", scene.steps);
    for op in &scene.ops {
        let target = match op.target {
            Target::Static(i) => format!("static_{i}"),
            Target::Body(i) => format!("body_{i}"),
        };
        let action = match op.op {
            Op::Despawn => format!("world.despawn({target})"),
            Op::Teleport(at) => format!("teleport(&mut world, {target}, {})", vec_source(at)),
            Op::Wake => format!("wake(&mut world, {target})"),
            Op::Reshape(shape) => format!(
                "world.get_mut::<Collider>({target}).unwrap().shape = {}.shape",
                shape_source(shape)
            ),
            Op::Drive(v) => format!(
                "world.get_mut::<Velocity>({target}).unwrap().0 = {}",
                vec_source(v)
            ),
        };
        let _ = writeln!(out, "        if step == {} {{ {action}; }}", op.step);
    }
    let _ = writeln!(out, "        physics_step(&mut world);");
    let _ = writeln!(out, "        drain_events(&mut world);");
    let _ = writeln!(out, "        // the invariant's assertion");
    let _ = writeln!(out, "    }}");
    out
}

/// `ORACLE_SEEDS=a..b`: every failure class's smallest seed, reduced, with
/// its scene as a test body. Run in release.
#[test]
#[ignore = "a finder's reduction of each failure class"]
fn physics_oracle_reduce() {
    let seeds = std::env::var("ORACLE_SEEDS").unwrap_or_else(|_| "0..400".into());
    let (a, b) = seeds.split_once("..").expect("ORACLE_SEEDS is a..b");
    let skip_known = std::env::var("ORACLE_ALL").is_err();
    let mut smallest: BTreeMap<Key, u64> = BTreeMap::new();
    for seed in a.parse::<u64>().unwrap()..b.parse::<u64>().unwrap() {
        let scene = generate(seed);
        let outcome = run_scene(&scene, true);
        let again = run_scene(&scene, false);
        let mut keys: Vec<Key> = outcome.failures.iter().map(Failure::key).collect();
        if outcome.hashes != again.hashes {
            keys.push((4, "two runs of one seed hashed apart", "new".to_owned()));
        }
        for key in keys {
            if key.1 == "allowance above its own" || (skip_known && key.2 != "new") {
                continue;
            }
            smallest.entry(key).or_insert(seed);
        }
    }
    for (key, seed) in smallest {
        let scene = generate(seed);
        let reduced = reduce(&scene, &key);
        let outcome = run_scene(&reduced, true);
        let failure = outcome.failures.iter().find(|f| f.key() == key);
        println!(
            "== invariant {} [{}] {}: seed {seed}, reduced to {} bodies, {} statics, map {}, {} ops, {} steps",
            key.0,
            key.2,
            key.1,
            reduced.bodies.len(),
            reduced.statics.len(),
            reduced.map.is_some(),
            reduced.ops.len(),
            reduced.steps
        );
        if let Some(failure) = failure {
            println!("   step {}: {}", failure.step, failure.detail);
        }
        println!("{}", scene_source(&reduced));
    }
}

// ---------------------------------------------------------------------------
// The detectors' own checks: each fires on a case built to fail it.

#[test]
fn oracle_box_crossing_sees_a_pass_and_nothing_else() {
    let wall = Aabb::new(Vec2::ZERO, Vec2::new(10.0, 2.0));
    let cross = |path: &[Vec2]| {
        let mut state = None;
        path.windows(2)
            .filter_map(|s| box_crossing(&mut state, s[0], s[1], &wall, 0))
            .map(|c| (c.enter, c.exit))
            .collect::<Vec<_>>()
    };
    let (above, below) = (Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0));
    assert_eq!(cross(&[above, below]), [(Face::Top, Face::Bottom)]);
    assert_eq!(cross(&[below, above]), [(Face::Bottom, Face::Top)]);
    // In over one segment, out over the next.
    assert_eq!(
        cross(&[above, Vec2::new(0.0, 0.5), below]),
        [(Face::Top, Face::Bottom)]
    );
    // In and back out the same face, a corner cut, a graze and a miss.
    assert!(cross(&[above, Vec2::new(0.0, 1.0), above]).is_empty());
    assert!(cross(&[Vec2::new(-12.0, -1.0), Vec2::new(-8.0, -3.0)]).is_empty());
    assert!(cross(&[Vec2::new(-12.0, -2.0), Vec2::new(12.0, -2.0)]).is_empty());
    assert!(cross(&[Vec2::new(-12.0, -6.0), Vec2::new(12.0, -6.0)]).is_empty());
    // A centre that starts inside leaves without a crossing.
    assert!(cross(&[Vec2::new(0.0, 1.0), below]).is_empty());
    // Sideways through.
    assert_eq!(
        cross(&[Vec2::new(-20.0, 0.0), Vec2::new(20.0, 0.0)]),
        [(Face::Left, Face::Right)]
    );
}

#[test]
fn oracle_map_crossing_sees_solid_cells_as_one_region() {
    let map = MapSpec {
        origin: Vec2::new(-24.0, 0.0),
        cols: 3,
        rows: 2,
        // Top row: solid, one-way, slope; bottom row: solid, solid, empty.
        cells: vec![
            Cell::Solid,
            Cell::OneWay,
            Cell::Slope(1),
            Cell::Solid,
            Cell::Solid,
            Cell::Empty,
        ],
    };
    let cross = |path: &[Vec2]| {
        let mut state = None;
        path.windows(2)
            .filter_map(|s| map_crossing(&mut state, s[0], s[1], &map, 0))
            .map(|c| (c.enter, c.exit))
            .collect::<Vec<_>>()
    };
    // Down through the two solid cells of the first column, in one segment
    // and in three.
    let (top, bottom) = (Vec2::new(-16.0, -10.0), Vec2::new(-16.0, 42.0));
    assert_eq!(cross(&[top, bottom]), [(Face::Top, Face::Bottom)]);
    assert_eq!(
        cross(&[top, Vec2::new(-16.0, 8.0), Vec2::new(-16.0, 24.0), bottom]),
        [(Face::Top, Face::Bottom)]
    );
    // Down the one-way column enters solid only in the bottom row.
    assert_eq!(
        cross(&[Vec2::new(0.0, -10.0), Vec2::new(0.0, 42.0)]),
        [(Face::Top, Face::Bottom)]
    );
    // Down the slope column meets nothing solid.
    assert!(cross(&[Vec2::new(16.0, -10.0), Vec2::new(16.0, 42.0)]).is_empty());
    // Along the bottom row, through both solid cells and out into empty.
    assert_eq!(
        cross(&[Vec2::new(-40.0, 24.0), Vec2::new(30.0, 24.0)]),
        [(Face::Left, Face::Right)]
    );
    // Into the solid row and back out of the face it entered.
    assert!(
        cross(&[
            Vec2::new(-16.0, -10.0),
            Vec2::new(-16.0, 4.0),
            Vec2::new(-16.0, -10.0)
        ])
        .is_empty()
    );
}

#[test]
fn oracle_contacts_read_depth_and_the_support_normal() {
    let floor = Shape::Aabb {
        half_extents: Vec2::new(50.0, 5.0),
    };
    let up = Vec2::NEG_Y;
    let (depth, normal) = shape_contact(
        Shape::Aabb {
            half_extents: Vec2::splat(4.0),
        },
        Vec2::new(0.0, -8.7),
        floor,
        Vec2::ZERO,
    );
    assert!(
        (depth - 0.3).abs() < 1e-4 && normal == up,
        "{depth} {normal}"
    );
    let (depth, normal) = shape_contact(
        Shape::Circle { radius: 4.0 },
        Vec2::new(3.0, -8.5),
        floor,
        Vec2::ZERO,
    );
    assert!(
        (depth - 0.5).abs() < 1e-4 && normal == up,
        "{depth} {normal}"
    );
    let (depth, normal) = shape_contact(
        floor,
        Vec2::ZERO,
        Shape::Circle { radius: 4.0 },
        Vec2::new(3.0, -8.5),
    );
    assert!(
        (depth - 0.5).abs() < 1e-4 && normal == -up,
        "{depth} {normal}"
    );
    let (depth, _) = shape_contact(
        Shape::Circle { radius: 4.0 },
        Vec2::new(0.0, -10.0),
        Shape::Circle { radius: 5.0 },
        Vec2::ZERO,
    );
    assert!((depth + 1.0).abs() < 1e-4, "{depth}");
    let (depth, _) = shape_contact(
        Shape::Aabb {
            half_extents: Vec2::splat(4.0),
        },
        Vec2::new(57.0, -13.0),
        floor,
        Vec2::ZERO,
    );
    assert!((depth + 5.0).abs() < 1e-4, "{depth}");
}

#[test]
fn oracle_pair_check_sees_a_pair_the_list_drops() {
    // A ball over a floor, one step; then the list loses its pairs and the
    // check runs on the same buffers.
    let scene = Scene {
        seed: 0,
        gravity: 900.0,
        dt: 1.0 / 60.0,
        steps: 1,
        statics: vec![StaticSpec {
            center: Vec2::new(0.0, 20.0),
            half: Vec2::new(100.0, 10.0),
            velocity: Vec2::ZERO,
            wall: true,
        }],
        map: None,
        bodies: vec![BodySpec {
            kind: Kind::Dynamic,
            shape: Shape::Circle { radius: 5.0 },
            center: Vec2::new(0.0, 4.0),
            velocity: Vec2::new(0.0, 300.0),
            mass: 1.0,
            gravity_scale: 1.0,
            restitution: 0.0,
            friction: 0.0,
        }],
        ops: Vec::new(),
    };
    let outcome = run_scene(&scene, true);
    assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    let (mut world, _) = build_world(&scene, true);
    physics_step(&mut world);
    let config = *world.resource::<PhysicsConfig>();
    let buffers = world.resource_mut::<PhysicsBuffers>();
    assert!(!buffers.pairs.is_empty(), "the ball pairs with the floor");
    buffers.pairs.clear();
    buffers.oracle = Some(Box::default());
    after_refresh(&config, 1.0 / 240.0, 1.0 / 240.0, buffers);
    let probe = buffers.oracle.as_ref().unwrap();
    assert!(
        probe.misses.iter().any(|miss| miss.rule == PairRule::Box),
        "{:?}",
        probe.misses
    );
}

/// new-1's sweep case: whether the light body passed through the floor,
/// and whether the oracle named the crush.
fn crush_case(
    victim: Shape,
    pusher: Shape,
    ratio: f32,
    speed: f32,
    half_y: f32,
    dt: f32,
) -> (bool, bool) {
    let failures = run_scene(&crush_scene(victim, pusher, ratio, speed, half_y, dt), true).failures;
    let crossed = failures.iter().filter(|failure| failure.invariant == 2);
    let named = crossed
        .clone()
        .any(|failure| failure.findings.contains(&"oracle-crush"));
    (crossed.count() > 0, named)
}

#[test]
#[ignore = "a finder's sweep for new-1"]
fn physics_oracle_crush_sweep() {
    let shapes = [
        ("circle 6", Shape::Circle { radius: 6.0 }),
        ("circle 19", Shape::Circle { radius: 19.0 }),
        (
            "box 6",
            Shape::Aabb {
                half_extents: Vec2::splat(6.0),
            },
        ),
    ];
    let pushers = [
        ("circle 30", Shape::Circle { radius: 30.0 }),
        (
            "box 20",
            Shape::Aabb {
                half_extents: Vec2::splat(20.0),
            },
        ),
    ];
    let mut passed = 0;
    let mut cases = 0;
    for (vn, victim) in shapes {
        for (pn, pusher) in pushers {
            for ratio in [4.0, 10.0, 30.0, 100.0, 300.0, 1000.0] {
                for speed in [500.0, 1000.0, 2000.0, 5000.0] {
                    for half_y in [4.0, 8.0, 13.0] {
                        for dt in [1.0 / 60.0, 1.0 / 30.0] {
                            cases += 1;
                            let (crossed, named) =
                                crush_case(victim, pusher, ratio, speed, half_y, dt);
                            if crossed {
                                passed += 1;
                                println!(
                                    "PASSED {vn} under {pn} ratio {ratio} speed {speed} floor {} dt 1/{} named {named}",
                                    2.0 * half_y,
                                    (1.0 / dt).round()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{passed} of {cases} cases passed the floor");
}
