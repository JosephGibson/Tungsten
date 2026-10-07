//! `physics`: pachinko owns narrow phase, contact build, solve, collision
//! events and sleep bookkeeping; `sparse` owns proxy gather, broadphase build
//! and pair query. Both are judged on the `physics_step` system row.
//!
//! Pachinko keeps every ball awake without disabling island sleeping (D-065:
//! an island sleeps after 0.5 s below 20 px/s): bin floors teleport a ball
//! back to the spawn band on its first contact, bumper pegs kick touching
//! balls, and an anti-stall timer (30 px/s, stricter than the engine's
//! threshold, on the same post-step velocity) kicks a slow ball at 0.25 s and
//! respawns it at 0.4 s. The runner rejects any capture with a sleeper.

use std::f32::consts::PI;
use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::core::{
    CameraState, Collider, CollisionEvent, Entity, EventQueue, Pcg32, PhysicsConfig, Position,
    RigidBody, Shape, Time, Velocity, World,
};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Guard, Knob, Preset, Row, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "physics",
    workload_version: 1,
    warmup: 120,
    gpu_timing: false,
    knobs: KNOBS,
    presets: PRESETS,
    rows: ROWS,
    row,
    validate,
    derived,
    engine_config: knobs::keep_engine_config,
    configure,
};

const KNOBS: &[Knob] = &[
    Knob::choice("mode", "pachinko", &["pachinko", "sparse"]),
    Knob::int("balls", 8_000, 200, 400_000)
        .scaled()
        .note("Board width follows"),
    Knob::float("fill", 0.36, 0.05, 0.40).note("Ball area / board area"),
    Knob::choice("radius_mix", "mixed", &["uniform", "mixed", "wide"]).note(
        "uniform: 4.5 px; mixed: 3/4.5/6/9 px at 35/30/25/10%; wide: 2/4/8/16 px at 40/30/20/10%",
    ),
    Knob::float("peg_radius", 5.0, 2.0, 12.0).note("px"),
    Knob::float("bumpers", 0.2, 0.0, 1.0).note("Share of pegs that kick"),
    Knob::float("kick", 260.0, 0.0, 1_000.0).note("Bumper exit speed, px/s"),
    Knob::float("gravity", 900.0, 100.0, 3_000.0).note("px/s^2"),
    Knob::float("restitution", 0.3, 0.0, 1.0).note("Balls"),
    Knob::int("bodies", 8_000, 1_000, 2_000_000)
        .scaled()
        .note("Sparse only; arena area follows"),
    Knob::float("radius", 2.0, 1.0, 8.0).note("Sparse only; px"),
    Knob::float("speed_min", 600.0, 50.0, 8_000.0).note("Sparse only; px/s"),
    Knob::float("speed_max", 1_400.0, 50.0, 8_000.0).note("Sparse only; px/s"),
    Knob::float("density", 0.01, 0.001, 0.05).note("Sparse only; body area / arena area"),
    Knob::int("substeps", 4, 1, 8).note("PhysicsConfig::substeps"),
    Knob::int("iterations", 1, 1, 8).note("PhysicsConfig::solver_iterations"),
    Knob::int("cell", 32, 8, 256).note("PhysicsConfig::broadphase_cell_size, px"),
    Knob::choice("sleep", "on", &["on", "off"]).note("The guard requires zero sleepers either way"),
    Knob::choice("render", "culled", &["culled", "none"]).note("none draws no bodies"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("balls", Value::Int(1_000))],
    },
    Preset {
        name: "default",
        set: &[],
    },
    Preset {
        name: "sparse-min",
        set: &[
            ("mode", Value::Choice("sparse")),
            ("bodies", Value::Int(2_000)),
        ],
    },
    Preset {
        name: "sparse",
        set: &[("mode", Value::Choice("sparse"))],
    },
];

const NO_SLEEPERS: Guard = Guard::PhysicsMax {
    field: "sleeping",
    max: 0,
};

const ROWS: &[Row] = &[
    Row {
        name: "physics",
        preset: "default",
        owned: &[
            ("system.physics_step", &["p50", "p95"]),
            ("stage.update", &["p95"]),
        ],
        guards: &[
            NO_SLEEPERS,
            Guard::CounterMin {
                counter: "teleports",
                min: 1,
            },
        ],
        counters: &[
            "balls",
            "teleports",
            "bumper_kicks",
            "stall_kicks",
            "respawns",
            "events",
            "visible",
        ],
        bottleneck: "system.physics_step",
        key_knobs: &["balls"],
        note: "",
    },
    Row {
        name: "physics-sparse",
        preset: "sparse",
        owned: &[("system.physics_step", &["p50", "p95"])],
        guards: &[NO_SLEEPERS],
        counters: &["bodies", "events", "visible"],
        bottleneck: "system.physics_step",
        key_knobs: &["bodies"],
        note: "",
    },
];

// Pachinko layout, top to bottom: ceiling, spawn band, peg field, bin zone.
const SPAWN_BAND: f32 = 128.0;
const PEG_ROWS: u32 = 32;
const BIN_ZONE: f32 = 160.0;
const MIN_BINS: u32 = 4;
const WALL: f32 = 80.0;
const FLOOR: f32 = 32.0;
const DIVIDER_W: f32 = 4.0;
const DIVIDER_H: f32 = 96.0;

// `(radius, share)` per class, smallest first.
const UNIFORM: &[(f32, f32)] = &[(4.5, 1.0)];
const MIXED: &[(f32, f32)] = &[(3.0, 0.35), (4.5, 0.30), (6.0, 0.25), (9.0, 0.10)];
const WIDE: &[(f32, f32)] = &[(2.0, 0.40), (4.0, 0.30), (8.0, 0.20), (16.0, 0.10)];

const PLACE_ATTEMPTS: u32 = 4_096;
const PLACE_GAP: f32 = 0.5;
const PLACE_SALT: u64 = 0x5EED_BA11_0000_0001;

const STALL_SPEED: f32 = 30.0;
const STALL_KICK_AFTER: f32 = 0.25;
const STALL_RESPAWN_AFTER: f32 = 0.4;
const STALL_KICK_SPEED: f32 = 200.0;
// Respawned balls fall at 60-120 px/s, clear of the stall threshold.
const SPAWN_SPEED_X: f32 = 40.0;
const SPAWN_SPEED_Y: (f32, f32) = (60.0, 120.0);

const SPARSE_WALL: f32 = 64.0;
const SPARSE_ZOOM: f32 = 0.5;

const WALL_COLOR: [u8; 4] = [86, 90, 112, 255];
const FLOOR_COLOR: [u8; 4] = [58, 132, 214, 255];
const PEG_COLOR: [u8; 4] = [150, 152, 168, 255];
const BUMPER_COLOR: [u8; 4] = [255, 112, 64, 255];
const BALL_COLORS: [[u8; 4]; 4] = [
    [242, 196, 92, 255],
    [236, 128, 96, 255],
    [120, 186, 240, 255],
    [168, 228, 140, 255],
];
const SPARSE_COLOR: [u8; 4] = [196, 220, 255, 255];

fn row(cfg: &BenchConfig) -> &'static str {
    if is_sparse(cfg) {
        "physics-sparse"
    } else {
        "physics"
    }
}

fn is_sparse(cfg: &BenchConfig) -> bool {
    cfg.choice("mode") == "sparse"
}

fn validate(cfg: &BenchConfig) -> anyhow::Result<()> {
    let (low, high) = (cfg.float("speed_min"), cfg.float("speed_max"));
    anyhow::ensure!(
        low <= high,
        "knob 'speed_min' ({low}) must not exceed 'speed_max' ({high})"
    );
    Ok(())
}

fn round4(value: f64) -> f64 {
    (value * 1e4).round() / 1e4
}

fn derived(cfg: &BenchConfig) -> Json {
    if is_sparse(cfg) {
        let arena = arena_size(cfg);
        return json!({"world": [arena.x, arena.y], "zoom": SPARSE_ZOOM});
    }
    let board = Board::new(cfg);
    let pegs = board.pegs().len();
    json!({
        "world": [board.width, board.height],
        "pitch": [board.pitch_x, board.pitch_y],
        "bins": board.bins,
        "pegs": pegs,
        "bumpers": bumper_count(pegs, cfg.float("bumpers")),
        "fill": round4(board.fill(cfg.int("balls") as usize)),
        "zoom": round4(f64::from(board.zoom())),
    })
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let sparse = is_sparse(cfg);
    let render = cfg.choice("render") == "culled";
    {
        let world = app.world_mut();
        if let Some(physics) = world.get_resource_mut::<PhysicsConfig>() {
            physics.substeps = cfg.int("substeps") as u32;
            physics.solver_iterations = cfg.int("iterations") as u32;
            physics.broadphase_cell_size = cfg.int("cell") as f32;
            physics.gravity = if sparse {
                Vec2::ZERO
            } else {
                Vec2::new(0.0, cfg.float("gravity") as f32)
            };
            if cfg.choice("sleep") == "off" {
                physics.sleep_threshold = 0.0;
            }
        }
        let (bodies, camera) = if sparse {
            let state = spawn_sparse(world, cfg);
            let camera = centered(state.arena * 0.5, SPARSE_ZOOM);
            let bodies = state.bodies;
            world.insert_resource(state);
            (bodies, camera)
        } else {
            let state = spawn_pachinko(world, cfg);
            let board = &state.board;
            let camera = centered(Vec2::new(board.width, board.height) * 0.5, board.zoom());
            let bodies = state.balls;
            world.insert_resource(state);
            (bodies, camera)
        };
        if let Some(state) = world.get_resource_mut::<CameraState>() {
            *state = camera;
        }
        world.insert_resource(BenchCounters::new(PhysicsCounts {
            sparse,
            render,
            bodies,
            ..PhysicsCounts::default()
        }));
    }
    app.on_startup(r#gen::register_view_textures);
    // The hand-wired step sends these; without `PhysicsPlugin` the row
    // registers the queue itself.
    app.register_event::<tungsten::core::CollisionEvent>();
    app.add_system_named("bench_counters", bench_counters_system::<PhysicsCounts>);
    app.add_system_named("physics_step", tungsten::physics::physics_step);
    if sparse {
        app.add_system_named("renormalize_speeds", renormalize_speeds);
    } else {
        app.add_system_named("stall_guard", stall_guard);
        app.add_system_named("pachinko_events", pachinko_events);
    }
    if render {
        app.set_extract_sprites(view::extract_view);
    } else {
        app.set_extract_sprites(|_| Vec::new());
    }
}

/// Camera whose view of `VIEWPORT` at `zoom` is centered on `center`.
fn centered(center: Vec2, zoom: f32) -> CameraState {
    CameraState {
        position: center - VIEWPORT / zoom * 0.5,
        zoom,
        rotation: 0.0,
    }
}

/// Dynamic body marker for both modes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ball {
    pub(crate) radius: f32,
}

/// Anti-stall timer: seconds below `STALL_SPEED`, and whether this streak
/// already got its kick.
#[derive(Debug, Clone, Copy, Default)]
struct Stall {
    slow: f32,
    kicked: bool,
}

fn spawn_static(
    world: &mut World,
    center: Vec2,
    collider: Collider,
    restitution: f32,
    color: [u8; 4],
) -> Entity {
    let (half, texture) = match collider.shape {
        Shape::Circle { radius } => (Vec2::splat(radius), ViewTexture::Disc),
        Shape::Aabb { half_extents } => (half_extents, ViewTexture::Rect),
    };
    let entity = world.spawn();
    world.insert(entity, Position(center));
    world.insert(entity, RigidBody::r#static().with_restitution(restitution));
    world.insert(entity, collider);
    world.insert(
        entity,
        ViewSprite {
            half,
            color,
            texture,
        },
    );
    entity
}

fn spawn_ball(
    world: &mut World,
    center: Vec2,
    velocity: Vec2,
    radius: f32,
    restitution: f32,
    color: [u8; 4],
) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Position(center));
    world.insert(entity, Velocity(velocity));
    // Mass follows area, normalized to the 4.5 px ball.
    world.insert(
        entity,
        RigidBody::dynamic()
            .with_mass(radius * radius / (4.5 * 4.5))
            .with_restitution(restitution),
    );
    world.insert(entity, Collider::circle(radius));
    world.insert(entity, Ball { radius });
    world.insert(
        entity,
        ViewSprite {
            half: Vec2::splat(radius),
            color,
            texture: ViewTexture::Disc,
        },
    );
    entity
}

/// Pachinko board geometry. y grows downward; the interior spans
/// `[0, width] x [0, height]`, with the ceiling above and bin floors below.
#[derive(Debug, Clone)]
pub(crate) struct Board {
    mix: &'static [(f32, f32)],
    pub(crate) d_max: f32,
    pub(crate) peg_radius: f32,
    pub(crate) pitch_x: f32,
    pub(crate) pitch_y: f32,
    pub(crate) bins: u32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    mean_area: f32,
}

/// One initial ball: center, radius and its class index in the mix.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlacedBall {
    pub(crate) center: Vec2,
    pub(crate) radius: f32,
    class: usize,
}

impl Board {
    pub(crate) fn new(cfg: &BenchConfig) -> Self {
        let mix = match cfg.choice("radius_mix") {
            "uniform" => UNIFORM,
            "wide" => WIDE,
            _ => MIXED,
        };
        let d_max = 2.0 * mix.iter().map(|&(radius, _)| radius).fold(0.0, f32::max);
        let peg_radius = cfg.float("peg_radius") as f32;
        // Pitch 3 d_max + 2 r_p, whole 16 px, at least 64: the in-row gap is
        // 3 d_max and the diagonal gap over 2.5 d_max, so no ball wedges.
        let pitch_x = (((3.0 * d_max + 2.0 * peg_radius) / 16.0).ceil() * 16.0).max(64.0);
        let pitch_y = 0.75 * pitch_x;
        let height = SPAWN_BAND + PEG_ROWS as f32 * pitch_y + BIN_ZONE;
        let mean_area: f32 = mix
            .iter()
            .map(|&(radius, share)| share * PI * radius * radius)
            .sum();
        let bin_width = 2.0 * pitch_x;
        let area = cfg.int("balls") as f64 * f64::from(mean_area) / cfg.float("fill");
        let bins = ((area / f64::from(height) / f64::from(bin_width)).ceil() as u32).max(MIN_BINS);
        Self {
            mix,
            d_max,
            peg_radius,
            pitch_x,
            pitch_y,
            bins,
            width: bins as f32 * bin_width,
            height,
            mean_area,
        }
    }

    fn bin_width(&self) -> f32 {
        2.0 * self.pitch_x
    }

    fn divider_top(&self) -> f32 {
        self.height - DIVIDER_H
    }

    /// Even rows start half a pitch from the wall, odd rows a full pitch.
    fn row_layout(&self, row: u32) -> (f32, u32) {
        let columns = 2 * self.bins;
        if row.is_multiple_of(2) {
            (0.5, columns)
        } else {
            (1.0, columns - 1)
        }
    }

    fn row_y(&self, row: u32) -> f32 {
        SPAWN_BAND + self.pitch_y * (row as f32 + 0.5)
    }

    pub(crate) fn pegs(&self) -> Vec<Vec2> {
        let mut pegs = Vec::new();
        for row in 0..PEG_ROWS {
            let (first, count) = self.row_layout(row);
            let y = self.row_y(row);
            pegs.extend(
                (0..count).map(|column| Vec2::new(self.pitch_x * (column as f32 + first), y)),
            );
        }
        pegs
    }

    /// Fit-height zoom: the default board shows whole, and wider boards
    /// overflow the centered view, so the culled view's cost plateaus.
    pub(crate) fn zoom(&self) -> f32 {
        VIEWPORT.y / self.height
    }

    fn fill(&self, balls: usize) -> f64 {
        balls as f64 * f64::from(self.mean_area) / (f64::from(self.width) * f64::from(self.height))
    }

    /// `(radius, class)` for `count` balls, largest first. Class counts round
    /// cumulatively, so they sum to `count`.
    fn radii(&self, count: usize) -> Vec<(f32, usize)> {
        let mut classes = Vec::with_capacity(self.mix.len());
        let mut cumulative = 0.0_f64;
        let mut assigned = 0;
        for (class, &(radius, share)) in self.mix.iter().enumerate() {
            cumulative += f64::from(share);
            let upto = if class + 1 == self.mix.len() {
                count
            } else {
                ((count as f64 * cumulative).round() as usize).min(count)
            };
            classes.push((radius, class, upto - assigned));
            assigned = upto;
        }
        classes
            .iter()
            .rev()
            .flat_map(|&(radius, class, n)| std::iter::repeat_n((radius, class), n))
            .collect()
    }

    /// Whether a ball fits at `center` without touching the interior bounds,
    /// a peg, a divider or its cap.
    fn clear_of_statics(&self, center: Vec2, radius: f32) -> bool {
        if center.x - radius < 0.0
            || center.x + radius > self.width
            || center.y - radius < 0.0
            || center.y + radius > self.height
        {
            return false;
        }
        // The reach stays under one pitch, so only the nearest rows and
        // columns can hold a touching peg.
        let reach = radius + self.peg_radius + PLACE_GAP;
        let nearest_row = ((center.y - SPAWN_BAND) / self.pitch_y - 0.5).round() as i64;
        for row in nearest_row - 1..=nearest_row + 1 {
            if !(0..i64::from(PEG_ROWS)).contains(&row) {
                continue;
            }
            let (first, count) = self.row_layout(row as u32);
            let nearest_column = (center.x / self.pitch_x - first).round() as i64;
            for column in nearest_column - 1..=nearest_column + 1 {
                if !(0..i64::from(count)).contains(&column) {
                    continue;
                }
                let peg = Vec2::new(
                    self.pitch_x * (column as f32 + first),
                    self.row_y(row as u32),
                );
                if peg.distance_squared(center) < reach * reach {
                    return false;
                }
            }
        }
        let divider = (center.x / self.bin_width()).round();
        if divider >= 1.0 && divider < self.bins as f32 {
            let near_x = (center.x - divider * self.bin_width()).abs()
                < radius + DIVIDER_W * 0.5 + PLACE_GAP;
            let near_y = center.y + radius + PLACE_GAP > self.divider_top() - DIVIDER_W * 0.5;
            if near_x && near_y {
                return false;
            }
        }
        true
    }

    /// Initial balls by seeded dart throwing, largest first, clear of the
    /// statics and of each other. A ball that finds no spot within
    /// `PLACE_ATTEMPTS` keeps its last candidate, and the solver's push-out
    /// separates it; the defaults never need that.
    pub(crate) fn place_balls(&self, count: usize, seed: u64) -> Vec<PlacedBall> {
        let mut rng = Pcg32::seeded(seed ^ PLACE_SALT);
        let mut grid = BallGrid::new(self.width, self.height, self.d_max, count);
        let mut placed = Vec::with_capacity(count);
        for (radius, class) in self.radii(count) {
            let mut center = Vec2::ZERO;
            for _ in 0..PLACE_ATTEMPTS {
                center = Vec2::new(
                    rng.next_range(radius, self.width - radius),
                    rng.next_range(radius, self.height - radius),
                );
                if self.clear_of_statics(center, radius) && grid.clear(&placed, center, radius) {
                    break;
                }
            }
            grid.insert(placed.len(), center);
            placed.push(PlacedBall {
                center,
                radius,
                class,
            });
        }
        placed
    }
}

/// Uniform grid of placed balls, one intrusive list per cell. The cell is the
/// largest diameter, so touching balls sit in neighboring cells.
struct BallGrid {
    cell: f32,
    columns: usize,
    rows: usize,
    heads: Vec<u32>,
    next: Vec<u32>,
}

impl BallGrid {
    const EMPTY: u32 = u32::MAX;

    fn new(width: f32, height: f32, cell: f32, capacity: usize) -> Self {
        let columns = (width / cell).ceil() as usize + 1;
        let rows = (height / cell).ceil() as usize + 1;
        Self {
            cell,
            columns,
            rows,
            heads: vec![Self::EMPTY; columns * rows],
            next: Vec::with_capacity(capacity),
        }
    }

    fn cell_of(&self, center: Vec2) -> (usize, usize) {
        let column = ((center.x / self.cell).max(0.0) as usize).min(self.columns - 1);
        let row = ((center.y / self.cell).max(0.0) as usize).min(self.rows - 1);
        (column, row)
    }

    fn insert(&mut self, index: usize, center: Vec2) {
        let (column, row) = self.cell_of(center);
        let slot = row * self.columns + column;
        self.next.push(self.heads[slot]);
        self.heads[slot] = index as u32;
    }

    fn clear(&self, placed: &[PlacedBall], center: Vec2, radius: f32) -> bool {
        let (column, row) = self.cell_of(center);
        for near_row in row.saturating_sub(1)..=(row + 1).min(self.rows - 1) {
            for near_column in column.saturating_sub(1)..=(column + 1).min(self.columns - 1) {
                let mut index = self.heads[near_row * self.columns + near_column];
                while index != Self::EMPTY {
                    let other = &placed[index as usize];
                    let reach = radius + other.radius + PLACE_GAP;
                    if other.center.distance_squared(center) < reach * reach {
                        return false;
                    }
                    index = self.next[index as usize];
                }
            }
        }
        true
    }
}

fn bumper_count(pegs: usize, share: f64) -> usize {
    ((pegs as f64 * share).round() as usize).min(pegs)
}

/// A fixed, seeded subset of the pegs (partial Fisher-Yates).
fn pick_bumpers(pegs: usize, share: f64, rng: &mut Pcg32) -> Vec<bool> {
    let count = bumper_count(pegs, share);
    let mut order: Vec<usize> = (0..pegs).collect();
    for index in 0..count {
        let pick = index + rng.next_u32() as usize % (pegs - index);
        order.swap(index, pick);
    }
    let mut bumper = vec![false; pegs];
    for &index in &order[..count] {
        bumper[index] = true;
    }
    bumper
}

/// What a static body does to a ball that touches it. Statics spawn before
/// any ball and never despawn, so the table is indexed by entity slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Plain,
    Bumper,
    Floor,
}

/// Pachinko runtime state; systems take it out of the world while they run.
struct Pachinko {
    board: Board,
    balls: u32,
    kick: f32,
    rng: Pcg32,
    roles: Vec<Role>,
    teleports: Vec<Entity>,
    kicks: Vec<(Entity, Vec2)>,
    respawns: Vec<Entity>,
}

impl Pachinko {
    fn role(&self, entity: Entity) -> Role {
        self.roles
            .get(entity.id() as usize)
            .copied()
            .unwrap_or(Role::Plain)
    }

    fn set_role(&mut self, entity: Entity, role: Role) {
        let slot = entity.id() as usize;
        if self.roles.len() <= slot {
            self.roles.resize(slot + 1, Role::Plain);
        }
        self.roles[slot] = role;
    }

    /// A spawn-band position and a small downward velocity.
    fn spawn_point(&mut self, radius: f32) -> (Vec2, Vec2) {
        let width = self.board.width;
        let center = Vec2::new(
            self.rng.next_range(radius, width - radius),
            self.rng.next_range(radius, SPAWN_BAND - radius),
        );
        let velocity = Vec2::new(
            self.rng.next_range(-SPAWN_SPEED_X, SPAWN_SPEED_X),
            self.rng.next_range(SPAWN_SPEED_Y.0, SPAWN_SPEED_Y.1),
        );
        (center, velocity)
    }

    /// Move a ball to the spawn band. The next gather picks up the direct
    /// `Position`/`Velocity` write (D-066).
    fn respawn(&mut self, world: &mut World, entity: Entity) {
        let Some(radius) = world.get::<Ball>(entity).map(|ball| ball.radius) else {
            return;
        };
        let (center, velocity) = self.spawn_point(radius);
        if let Some(position) = world.get_mut::<Position>(entity) {
            position.0 = center;
        }
        if let Some(current) = world.get_mut::<Velocity>(entity) {
            current.0 = velocity;
        }
        if let Some(stall) = world.get_mut::<Stall>(entity) {
            *stall = Stall::default();
        }
    }
}

fn spawn_pachinko(world: &mut World, cfg: &BenchConfig) -> Pachinko {
    let board = Board::new(cfg);
    let mut rng = Pcg32::seeded(cfg.seed());
    let pegs = board.pegs();
    let bumpers = pick_bumpers(pegs.len(), cfg.float("bumpers"), &mut rng);
    let balls = board.place_balls(cfg.int("balls") as usize, cfg.seed());
    let mut state = Pachinko {
        balls: balls.len() as u32,
        kick: cfg.float("kick") as f32,
        rng,
        roles: Vec::new(),
        teleports: Vec::new(),
        kicks: Vec::new(),
        respawns: Vec::new(),
        board,
    };
    let board = state.board.clone();
    let (width, height) = (board.width, board.height);

    let side_half = Vec2::new(WALL * 0.5, (height + FLOOR + WALL) * 0.5);
    let side_y = (height + FLOOR - WALL) * 0.5;
    for center in [
        Vec2::new(-WALL * 0.5, side_y),
        Vec2::new(width + WALL * 0.5, side_y),
    ] {
        spawn_static(world, center, Collider::aabb(side_half), 0.0, WALL_COLOR);
    }
    spawn_static(
        world,
        Vec2::new(width * 0.5, -WALL * 0.5),
        Collider::aabb(Vec2::new(width * 0.5 + WALL, WALL * 0.5)),
        0.0,
        WALL_COLOR,
    );
    for (peg, bumper) in pegs.iter().zip(bumpers) {
        let color = if bumper { BUMPER_COLOR } else { PEG_COLOR };
        let entity = spawn_static(world, *peg, Collider::circle(board.peg_radius), 0.0, color);
        if bumper {
            state.set_role(entity, Role::Bumper);
        }
    }
    // Dividers with round caps, so nothing rests on a flat top.
    let bin_width = board.bin_width();
    for bin in 1..board.bins {
        let x = bin as f32 * bin_width;
        spawn_static(
            world,
            Vec2::new(x, height - DIVIDER_H * 0.5),
            Collider::aabb(Vec2::new(DIVIDER_W * 0.5, DIVIDER_H * 0.5)),
            0.0,
            WALL_COLOR,
        );
        spawn_static(
            world,
            Vec2::new(x, board.divider_top()),
            Collider::circle(DIVIDER_W * 0.5),
            0.0,
            WALL_COLOR,
        );
    }
    for bin in 0..board.bins {
        let entity = spawn_static(
            world,
            Vec2::new((bin as f32 + 0.5) * bin_width, height + FLOOR * 0.5),
            Collider::aabb(Vec2::new(bin_width * 0.5, FLOOR * 0.5)),
            0.0,
            FLOOR_COLOR,
        );
        state.set_role(entity, Role::Floor);
    }

    let restitution = cfg.float("restitution") as f32;
    for ball in &balls {
        let velocity = Vec2::new(
            state.rng.next_range(-SPAWN_SPEED_X, SPAWN_SPEED_X),
            state.rng.next_range(SPAWN_SPEED_Y.0, SPAWN_SPEED_Y.1),
        );
        let color = BALL_COLORS[ball.class % BALL_COLORS.len()];
        let entity = spawn_ball(
            world,
            ball.center,
            velocity,
            ball.radius,
            restitution,
            color,
        );
        world.insert(entity, Stall::default());
    }
    state
}

fn delta_seconds(world: &World) -> f32 {
    world.get_resource::<Time>().map_or(0.0, Time::delta)
}

fn with_counts(world: &mut World, update: impl FnOnce(&mut PhysicsCounts)) {
    if let Some(counters) = world.get_resource_mut::<BenchCounters<PhysicsCounts>>() {
        update(&mut counters.counts);
    }
}

/// Runs right after `physics_step`, on the same post-step velocities the
/// engine's sleep timers read, so a ball's streak here is never shorter.
fn stall_guard(world: &mut World) {
    let dt = delta_seconds(world);
    let Some(mut state) = world.remove_resource::<Pachinko>() else {
        return;
    };
    let mut kicks = 0;
    state.respawns.clear();
    for (entity, velocity, stall) in world.query_mut::<(Entity, &mut Velocity, &mut Stall)>() {
        if velocity.0.length_squared() >= STALL_SPEED * STALL_SPEED {
            *stall = Stall::default();
            continue;
        }
        stall.slow += dt;
        if stall.slow >= STALL_RESPAWN_AFTER {
            state.respawns.push(entity);
        } else if stall.slow >= STALL_KICK_AFTER && !stall.kicked {
            stall.kicked = true;
            let side = if state.rng.next_u32() & 1 == 0 {
                -1.0
            } else {
                1.0
            };
            let angle = state.rng.next_range(PI / 6.0, PI / 3.0);
            velocity.0 = STALL_KICK_SPEED * Vec2::new(side * angle.sin(), -angle.cos());
            kicks += 1;
        }
    }
    let respawns = std::mem::take(&mut state.respawns);
    for &entity in &respawns {
        state.respawn(world, entity);
    }
    let respawned = respawns.len() as u32;
    state.respawns = respawns;
    world.insert_resource(state);
    with_counts(world, |counts| {
        counts.stall_kicks += kicks;
        counts.respawns += respawned;
    });
}

/// Consumes this frame's collision events: a bin floor teleports the ball to
/// the spawn band, a bumper raises its speed along the contact normal to at
/// least `kick`. `iter_current` skips the previous frame's window, which
/// `iter` would replay.
fn pachinko_events(world: &mut World) {
    let Some(mut state) = world.remove_resource::<Pachinko>() else {
        return;
    };
    let mut events = 0;
    state.teleports.clear();
    state.kicks.clear();
    if let Some(queue) = world.get_resource::<EventQueue<CollisionEvent>>() {
        for event in queue.iter_current() {
            events += 1;
            match event.b.map_or(Role::Plain, |other| state.role(other)) {
                Role::Floor => state.teleports.push(event.a),
                Role::Bumper => state.kicks.push((event.a, event.normal)),
                Role::Plain => {}
            }
        }
    }
    // One teleport per ball, in entity order, so RNG draws stay deterministic.
    state.teleports.sort_unstable_by_key(|entity| entity.id());
    state.teleports.dedup();
    let mut bumper_kicks = 0;
    for &(entity, normal) in &state.kicks {
        if state
            .teleports
            .binary_search_by_key(&entity.id(), |teleported| teleported.id())
            .is_ok()
        {
            continue;
        }
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            let along = velocity.0.dot(normal);
            if along < state.kick {
                velocity.0 += (state.kick - along) * normal;
                bumper_kicks += 1;
            }
        }
    }
    let teleports = std::mem::take(&mut state.teleports);
    for &entity in &teleports {
        state.respawn(world, entity);
    }
    let teleported = teleports.len() as u32;
    state.teleports = teleports;
    world.insert_resource(state);
    with_counts(world, |counts| {
        counts.teleports += teleported;
        counts.bumper_kicks += bumper_kicks;
        counts.events += events;
    });
}

/// Sparse arena: `bodies * pi r^2 / density`, 16:9, in multiples of 32 px.
fn arena_size(cfg: &BenchConfig) -> Vec2 {
    let radius = cfg.float("radius");
    let area =
        cfg.int("bodies") as f64 * std::f64::consts::PI * radius * radius / cfg.float("density");
    let snap = |side: f64| ((side / 32.0).round().max(1.0) * 32.0) as f32;
    Vec2::new(
        snap((area * 16.0 / 9.0).sqrt()),
        snap((area * 9.0 / 16.0).sqrt()),
    )
}

/// Sparse runtime state.
struct Sparse {
    arena: Vec2,
    bodies: u32,
    speed_min: f32,
    speed_max: f32,
    rng: Pcg32,
}

fn spawn_sparse(world: &mut World, cfg: &BenchConfig) -> Sparse {
    let arena = arena_size(cfg);
    let radius = cfg.float("radius") as f32;
    let count = cfg.int("bodies") as usize;
    let (speed_min, speed_max) = (cfg.float("speed_min") as f32, cfg.float("speed_max") as f32);
    let mut rng = Pcg32::seeded(cfg.seed());

    let half = SPARSE_WALL * 0.5;
    let walls = [
        (
            Vec2::new(-half, arena.y * 0.5),
            Vec2::new(half, arena.y * 0.5 + SPARSE_WALL),
        ),
        (
            Vec2::new(arena.x + half, arena.y * 0.5),
            Vec2::new(half, arena.y * 0.5 + SPARSE_WALL),
        ),
        (
            Vec2::new(arena.x * 0.5, -half),
            Vec2::new(arena.x * 0.5 + SPARSE_WALL, half),
        ),
        (
            Vec2::new(arena.x * 0.5, arena.y + half),
            Vec2::new(arena.x * 0.5 + SPARSE_WALL, half),
        ),
    ];
    for (center, half_extents) in walls {
        spawn_static(world, center, Collider::aabb(half_extents), 1.0, WALL_COLOR);
    }

    // One body per cell of a jittered grid: no initial overlaps at any
    // density in range.
    let columns = ((count as f32 * arena.x / arena.y).sqrt().ceil() as usize).max(1);
    let rows = count.div_ceil(columns).max(1);
    let cell = Vec2::new(arena.x / columns as f32, arena.y / rows as f32);
    for index in 0..count {
        let origin = Vec2::new(
            (index % columns) as f32 * cell.x,
            (index / columns) as f32 * cell.y,
        );
        let offset = Vec2::new(
            rng.next_range(radius, (cell.x - radius).max(radius)),
            rng.next_range(radius, (cell.y - radius).max(radius)),
        );
        let velocity = rng.next_unit_vec2() * rng.next_range(speed_min, speed_max);
        spawn_ball(world, origin + offset, velocity, radius, 1.0, SPARSE_COLOR);
    }
    Sparse {
        arena,
        bodies: count as u32,
        speed_min,
        speed_max,
        rng,
    }
}

/// The soft solver dissipates energy; renormalizing every body's speed into
/// `[speed_min, speed_max]` keeps the sparse workload constant and far above
/// the sleep threshold.
fn renormalize_speeds(world: &mut World) {
    let Some(mut state) = world.remove_resource::<Sparse>() else {
        return;
    };
    let (low, high) = (state.speed_min, state.speed_max);
    for (_entity, velocity, _ball) in world.query_mut::<(Entity, &mut Velocity, &mut Ball)>() {
        let speed = velocity.0.length();
        if speed < low {
            velocity.0 = if speed > 0.0 {
                velocity.0 * (low / speed)
            } else {
                state.rng.next_unit_vec2() * low
            };
        } else if speed > high {
            velocity.0 *= high / speed;
        }
    }
    world.insert_resource(state);
    let events = world
        .get_resource::<EventQueue<CollisionEvent>>()
        .map_or(0, |queue| queue.iter_current().count() as u32);
    with_counts(world, |counts| counts.events += events);
}

/// `bench:` counters; `visible` counts the bodies the culled view drew.
#[derive(Debug, Default)]
struct PhysicsCounts {
    sparse: bool,
    render: bool,
    bodies: u32,
    teleports: u32,
    bumper_kicks: u32,
    stall_kicks: u32,
    respawns: u32,
    events: u32,
}

impl FrameCounters for PhysicsCounts {
    fn write(&self, world: &World, line: &mut String) {
        let visible = if self.render {
            let bounds = view::view_bounds(world);
            world
                .query::<(Entity, &Position, &Ball)>()
                .filter(|(_, position, ball)| {
                    view::in_view(bounds, position.0, Vec2::splat(ball.radius))
                })
                .count()
        } else {
            0
        };
        let _ = if self.sparse {
            write!(
                line,
                " bodies={} events={} visible={visible}",
                self.bodies, self.events
            )
        } else {
            write!(
                line,
                " balls={} teleports={} bumper_kicks={} stall_kicks={} respawns={} events={} visible={visible}",
                self.bodies,
                self.teleports,
                self.bumper_kicks,
                self.stall_kicks,
                self.respawns,
                self.events
            )
        };
    }

    fn reset(&mut self) {
        self.teleports = 0;
        self.bumper_kicks = 0;
        self.stall_kicks = 0;
        self.respawns = 0;
        self.events = 0;
    }
}

#[cfg(test)]
#[path = "tests/physics.rs"]
mod tests;
