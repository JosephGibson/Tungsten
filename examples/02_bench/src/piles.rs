//! `piles`: bodies under friction on 16 px tiles, in two rows by its `mode`.
//! It owns `physics_step`'s friction, rolling and slope-tile costs (phase
//! 7's piles, `docs/plans/phase-7.md` §2.6).
//!
//! `friction` (row `friction`): boxes and spinning circles under friction
//! and rolling resistance fill a bin of tiles, stirred by kinematic paddles
//! sweeping its floor, one to each 24 cells of it. `slopes` (row `slopes`):
//! the same bodies, spread over a field of slope-tile diamonds, every slope
//! kind among them, flow to the floor, and each that first touches it
//! returns to a band at the top, a stream poured through the field. Sleep
//! is off by default, so every body stays in the solve. The tiles' material is built in
//! code; the art layer draws through the engine's tilemap extract, the
//! bodies and the paddle through the lean view.

use std::collections::{HashSet, VecDeque};
use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::core::{
    CameraState, Collider, CollisionEvent, Entity, EventQueue, FilterMode, LayerKind, Pcg32,
    PhysicsBuffers, PhysicsConfig, PhysicsMaterial, PhysicsMaterials, Position, RigidBody,
    RigidBodyBundle, Slope, Spin, TileIndex, TileProps, TilemapData, TilemapInstance,
    TilemapRegistry, Velocity, World, splitmix64,
};
use tungsten::{App, extract_tilemaps};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen::{self, Cell, TextureSink};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "piles",
    workload_version: 1,
    warmup: 120,
    gpu_timing: false,
    knobs: KNOBS,
    presets: PRESETS,
    row,
    validate,
    derived,
    engine_config: knobs::keep_engine_config,
    configure,
};

const KNOBS: &[Knob] = &[
    Knob::choice("mode", "friction", &["friction", "slopes"]),
    Knob::int("width", 96, 16, 512).note("Cells across, 16 px each"),
    Knob::int("height", 40, 16, 256).note("Cells down"),
    Knob::int("bodies", 3_000, 0, 50_000).scaled(),
    Knob::float("friction", 0.5, 0.0, 2.0).note("Bodies' and tiles' friction"),
    Knob::float("rolling", 0.02, 0.0, 0.5).note("Circles' and tiles' rolling resistance"),
    Knob::float("paddle_speed", 240.0, 0.0, 2_000.0).note("Friction only; px/s"),
    Knob::choice("sleep", "off", &["off", "on"]).note("on lets resting bodies sleep"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "default",
        set: &[],
    },
    Preset {
        name: "friction-min",
        set: &[
            ("width", Value::Int(24)),
            ("height", Value::Int(24)),
            ("bodies", Value::Int(400)),
        ],
    },
    Preset {
        name: "friction",
        set: &[("mode", Value::Choice("friction"))],
    },
    Preset {
        name: "slopes-min",
        set: &[
            ("mode", Value::Choice("slopes")),
            ("width", Value::Int(32)),
            ("height", Value::Int(32)),
            ("bodies", Value::Int(300)),
            ("friction", Value::Float(0.3)),
        ],
    },
    Preset {
        name: "slopes",
        set: &[
            ("mode", Value::Choice("slopes")),
            ("height", Value::Int(48)),
            ("bodies", Value::Int(2_000)),
            ("friction", Value::Float(0.3)),
        ],
    },
];

/// The map's registry ID and its material.
pub(crate) const PILES: &str = "bench_piles";
const STONE: &str = "bench_piles_stone";
const TILE_PX: u32 = 16;
const CELL: f32 = TILE_PX as f32;
/// The art layer, drawn, then the collision layer, each cell holding the
/// same tile: 0 a full one, a slope kind's code from 1 to 12.
const ART: usize = 0;
const SOLID: usize = 1;
const FULL: TileIndex = 0;

/// Each slope kind's name and polygon in its cell's local units, y down, by
/// code less 1, as a `slope` tile's step reads it.
const KINDS: [(&str, &[(f32, f32)]); 12] = [
    ("45 br", &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0)]),
    ("45 bl", &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)]),
    ("45 tr", &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]),
    ("45 tl", &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
    ("2:1 low br", &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.5)]),
    ("2:1 low bl", &[(0.0, 0.5), (0.0, 1.0), (1.0, 1.0)]),
    ("2:1 low tr", &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5)]),
    ("2:1 low tl", &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.5)]),
    (
        "2:1 high br",
        &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.5)],
    ),
    (
        "2:1 high bl",
        &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
    ),
    (
        "2:1 high tr",
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.5)],
    ),
    (
        "2:1 high tl",
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5), (0.0, 1.0)],
    ),
];

/// A diamond of slope cells: its top row's codes, left to right, over its
/// bottom row's: a 45° one two cells wide, a 2:1 one four.
const DIAMOND_45: [[TileIndex; 2]; 2] = [[1, 2], [3, 4]];
const DIAMOND_2_1: [[TileIndex; 4]; 2] = [[5, 9, 10, 6], [7, 11, 12, 8]];
/// Columns from one diamond's left to the next, and rows between a field's
/// diamond rows.
const DIAMOND_PITCH: u32 = 7;
const DIAMOND_ROWS: u32 = 4;

const GRAVITY: f32 = 900.0;
const RESTITUTION: f32 = 0.1;
/// Bodies' sizes, a circle's radius or a box's half width, cycled.
const SIZES: [f32; 3] = [3.0, 4.0, 5.0];
const SPAWN_SPEED_X: f32 = 40.0;
/// A paddle's half extents; the floor's cells each paddle sweeps; and the
/// cells a paddle turns back short of its section's ends, so it pins no
/// body against a wall: a kinematic body's push has no bound, and a body
/// crushed into a corner cell leaves through the map's edge.
const PADDLE_HALF: Vec2 = Vec2::new(8.0, 40.0);
const PADDLE_SECTION: u32 = 24;
const PADDLE_CLEARANCE: f32 = 2.0 * CELL;
/// A circle turning faster than this, in rad/s, spins.
const SPINNING: f32 = 0.5;
/// Frames the `recycled` counter sums.
const RECYCLE_WINDOW: usize = 60;
const LAYOUT_SALT: u64 = 0x9115_E5A1_0000_0001;
const BODY_COLORS: [[u8; 4]; 3] = [
    [242, 196, 92, 255],
    [120, 186, 240, 255],
    [168, 228, 140, 255],
];
const PADDLE_COLOR: [u8; 4] = [236, 128, 96, 255];

fn row(cfg: &BenchConfig) -> &'static str {
    if is_slopes(cfg) { "slopes" } else { "friction" }
}

fn is_slopes(cfg: &BenchConfig) -> bool {
    cfg.choice("mode") == "slopes"
}

fn validate(cfg: &BenchConfig) -> anyhow::Result<()> {
    let field = Field::new(cfg);
    if is_slopes(cfg) {
        let (narrow, wide) = field.diamonds;
        anyhow::ensure!(
            narrow > 0 && wide > 0,
            "slopes: a {} × {} cell field holds {narrow} 45° and {wide} 2:1 diamonds; \
             it needs one of each, every slope kind",
            field.width,
            field.height
        );
    } else {
        let narrowest = paddle_sweeps(&field)
            .iter()
            .map(|(low, high)| high - low)
            .fold(f32::INFINITY, f32::min);
        anyhow::ensure!(
            narrowest >= 4.0 * PADDLE_HALF.x,
            "friction: the bin is too narrow for its paddle's sweep"
        );
    }
    Ok(())
}

fn round4(value: f64) -> f64 {
    (value * 1e4).round() / 1e4
}

fn derived(cfg: &BenchConfig) -> Json {
    let field = Field::new(cfg);
    let size = field.size();
    json!({
        "world": [size.x, size.y],
        "band": field.band,
        "paddles": if is_slopes(cfg) { 0 } else { paddle_sweeps(&field).len() },
        "diamonds": [field.diamonds.0, field.diamonds.1],
        "slope_cells": field.cells.iter().filter(|&&tile| tile > FULL).count(),
        "zoom": round4(f64::from(zoom(size))),
    })
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let field = Field::new(cfg);
    {
        let world = app.world_mut();
        if let Some(physics) = world.get_resource_mut::<PhysicsConfig>() {
            physics.gravity = Vec2::new(0.0, GRAVITY);
            if cfg.choice("sleep") == "off" {
                physics.sleep_threshold = 0.0;
            }
        }
        world.insert_resource(PhysicsMaterials::new([(
            STONE.to_owned(),
            PhysicsMaterial {
                friction: cfg.float("friction") as f32,
                rolling: cfg.float("rolling") as f32,
                ..PhysicsMaterial::default()
            },
        )]));
        let mut state = PileState::new(cfg, &field);
        let bodies = spawn_bodies(world, &mut state, cfg);
        if !state.slopes {
            spawn_paddles(world, &field, cfg.float("paddle_speed") as f32);
        }
        let size = field.size();
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            let zoom = zoom(size);
            *camera = CameraState {
                position: size * 0.5 - VIEWPORT / zoom * 0.5,
                zoom,
                rotation: 0.0,
            };
        }
        world.insert_resource(state);
        world.insert_resource(BenchCounters::new(PileCounts {
            bodies,
            ..PileCounts::default()
        }));
    }
    app.on_startup(move |world, renderer| {
        r#gen::register_view_textures(world, renderer);
        place_map(world, renderer, &field);
    });
    app.register_event::<CollisionEvent>();
    app.add_system_named("bench_counters", bench_counters_system::<PileCounts>);
    app.add_system_named("piles_paddle", piles_paddle);
    app.add_system_named("physics_step", tungsten::physics::physics_step);
    app.add_system_named("piles_after_step", piles_after_step);
    app.set_extract_sprites(|world| {
        let mut batches = extract_tilemaps(world);
        batches.extend(view::extract_view(world));
        batches
    });
}

/// The zoom that fits a map of `size` px in the viewport.
fn zoom(size: Vec2) -> f32 {
    (VIEWPORT.x / size.x).min(VIEWPORT.y / size.y)
}

/// The startup hook's World part at `cfg`, for the counter guards, which
/// run no renderer: the lean view's textures, then the map's.
#[cfg(test)]
pub(crate) fn startup(world: &mut World, textures: &mut dyn TextureSink, cfg: &BenchConfig) {
    r#gen::register_view_textures(world, textures);
    place_map(world, textures, &Field::new(cfg));
}

/// The tiles' art on one page, the map in the registry, after manifest
/// composition, which replaces it, and its instance at the origin.
fn place_map(world: &mut World, textures: &mut dyn TextureSink, field: &Field) {
    let cells: Vec<Cell> = (0..=KINDS.len())
        .map(|tile| Cell {
            id: sprite_id(tile as TileIndex),
            rgba: r#gen::pile_tile_rgba((tile > 0).then(|| KINDS[tile - 1].1), TILE_PX),
        })
        .collect();
    r#gen::register_atlas(world, textures, FilterMode::Nearest, TILE_PX, &cells);
    world
        .get_resource_mut::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing")
        .insert(PILES.to_string(), field.map());
    let entity = world.spawn();
    world.insert(entity, TilemapInstance::new(PILES, Vec2::ZERO));
}

/// The art sprite of tile `tile`: the full cell's or a slope kind's.
fn sprite_id(tile: TileIndex) -> String {
    if tile == FULL {
        "bench_piles_solid".to_string()
    } else {
        format!("bench_piles_slope_{tile}")
    }
}

/// The map's cells, row-major, each empty (−1) or a tile: walls down both
/// sides, a floor along the bottom and, in `slopes`, rows of diamonds below
/// the spawn band, staggered, a 45° one and a 2:1 one by turns.
#[derive(Clone)]
struct Field {
    width: u32,
    height: u32,
    /// Rows of the spawn band at the top, clear of diamonds.
    band: u32,
    cells: Vec<TileIndex>,
    /// The 45° and the 2:1 diamonds.
    diamonds: (u32, u32),
}

impl Field {
    fn new(cfg: &BenchConfig) -> Self {
        let (width, height) = (cfg.int("width") as u32, cfg.int("height") as u32);
        let band = (height / 4).max(4);
        let mut cells = vec![-1; (width * height) as usize];
        let mut set = |col: u32, row: u32, tile: TileIndex| {
            cells[(row * width + col) as usize] = tile;
        };
        for row in 0..height {
            set(0, row, FULL);
            set(width - 1, row, FULL);
        }
        for col in 0..width {
            set(col, height - 1, FULL);
        }
        let mut diamonds = (0, 0);
        if is_slopes(cfg) {
            let mut row = band + 2;
            let mut line = 0;
            // Two clear cells at each wall, so no channel narrower than a
            // diamond's own gaps jams.
            while row + 1 + 3 < height {
                let mut col = 3 + (line % 2) * 3;
                let mut index = line;
                loop {
                    let wide = index % 2 == 1;
                    let span = if wide { 4 } else { 2 };
                    if col + span + 3 > width {
                        break;
                    }
                    for dr in 0..2 {
                        for dc in 0..span {
                            let tile = if wide {
                                DIAMOND_2_1[dr as usize][dc as usize]
                            } else {
                                DIAMOND_45[dr as usize][dc as usize]
                            };
                            set(col + dc, row + dr, tile);
                        }
                    }
                    if wide {
                        diamonds.1 += 1;
                    } else {
                        diamonds.0 += 1;
                    }
                    col += DIAMOND_PITCH;
                    index += 1;
                }
                row += DIAMOND_ROWS;
                line += 1;
            }
        }
        Self {
            width,
            height,
            band,
            cells,
            diamonds,
        }
    }

    fn size(&self) -> Vec2 {
        Vec2::new(self.width as f32 * CELL, self.height as f32 * CELL)
    }

    /// The bin's inside, between the walls and above the floor, in px.
    fn interior(&self) -> Vec2 {
        Vec2::new(
            (self.width - 2) as f32 * CELL,
            (self.height - 1) as f32 * CELL,
        )
    }

    /// The tile at `(col, row)`; −1 for an empty cell or one off the map.
    fn tile(&self, col: i64, row: i64) -> TileIndex {
        let inside =
            (0..i64::from(self.width)).contains(&col) && (0..i64::from(self.height)).contains(&row);
        if inside {
            self.cells[(row as u32 * self.width + col as u32) as usize]
        } else {
            -1
        }
    }

    /// The map: each cell's tile in both layers, the slope tiles' kinds and
    /// every tile's material set.
    fn map(&self) -> TilemapData {
        let mut map = TilemapData::new(TILE_PX, TILE_PX, self.width, self.height);
        for tile in 0..=KINDS.len() {
            let entry = map.add_tile(sprite_id(tile as TileIndex));
            debug_assert_eq!(entry, tile as TileIndex);
            let mut props = TileProps::default();
            props.material = Some(STONE.into());
            if tile > 0 {
                props.slope = Some(Slope::parse(KINDS[tile - 1].0).expect("a slope kind"));
            }
            map.set_tile_props(entry, props);
        }
        let art = map.add_layer("art", LayerKind::Render);
        let solid = map.add_layer("solid", LayerKind::Collision);
        debug_assert_eq!((art, solid), (ART, SOLID));
        for (at, &tile) in self.cells.iter().enumerate() {
            if tile >= 0 {
                let (col, row) = (at as u32 % self.width, at as u32 / self.width);
                map.set_tile(art, col, row, tile);
                map.set_tile(solid, col, row, tile);
            }
        }
        map
    }
}

/// Whether `point` lies strictly inside the cell at `(col, row)`'s slope
/// polygon; false for an empty or full cell.
fn in_slope(field: &Field, point: Vec2) -> bool {
    let (col, row) = ((point.x / CELL).floor(), (point.y / CELL).floor());
    let tile = field.tile(col as i64, row as i64);
    if tile <= FULL {
        return false;
    }
    let local = (point - Vec2::new(col, row) * CELL) / CELL;
    let points = KINDS[(tile - 1) as usize].1;
    let turns: Vec<f32> = (0..points.len())
        .map(|i| {
            let (p, q) = (points[i], points[(i + 1) % points.len()]);
            let (p, q) = (Vec2::new(p.0, p.1), Vec2::new(q.0, q.1));
            (q - p).perp_dot(local - p)
        })
        .collect();
    turns.iter().all(|&turn| turn > 0.0) || turns.iter().all(|&turn| turn < 0.0)
}

/// A body of the pile: a circle that rolls or a box.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Body {
    size: f32,
}

/// A kinematic paddle sweeping its section of the bin's floor, its centre
/// between `low` and `high`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Paddle {
    speed: f32,
    low: f32,
    high: f32,
}

/// The row's state between frames.
struct PileState {
    slopes: bool,
    field: Field,
    rng: Pcg32,
    /// Bodies recycled in each of the last [`RECYCLE_WINDOW`] frames.
    recycled: VecDeque<u32>,
}

impl PileState {
    fn new(cfg: &BenchConfig, field: &Field) -> Self {
        Self {
            slopes: is_slopes(cfg),
            field: field.clone(),
            rng: Pcg32::seeded(splitmix64(cfg.seed() ^ LAYOUT_SALT)),
            recycled: VecDeque::with_capacity(RECYCLE_WINDOW + 1),
        }
    }

    /// The region bodies spawn in: the spawn band under `slopes`, the upper
    /// three fifths of the bin otherwise.
    fn spawn_region(&self) -> (Vec2, Vec2) {
        let inside = self.field.interior();
        let top = if self.slopes {
            self.field.band as f32 * CELL
        } else {
            inside.y * 0.6
        };
        (Vec2::new(CELL, 0.0), Vec2::new(CELL + inside.x, top))
    }

    /// A point in the spawn band's upper half for a body of `size`.
    fn band_point(&mut self, size: f32) -> Vec2 {
        let (min, max) = self.spawn_region();
        Vec2::new(
            self.rng.next_range(min.x + size, max.x - size),
            self.rng
                .next_range(min.y + size, (max.y * 0.5).max(min.y + 2.0 * size)),
        )
    }
}

/// The bodies' starting points: under `slopes`, the centres of the field's
/// empty cells clear of its walls and diamonds, in a seeded order, jittered
/// within each cell, a cell taking another body once every one has one, so
/// the stream is in flight from the start; otherwise a jittered grid over
/// the spawn region.
fn start_points(state: &mut PileState, count: usize) -> Vec<Vec2> {
    if state.slopes {
        let field = &state.field;
        let mut open: Vec<(u32, u32)> = (0..field.height - 1)
            .flat_map(|row| (1..field.width - 1).map(move |col| (col, row)))
            .filter(|&(col, row)| {
                (-1..=1).all(|dr| {
                    (-1..=1).all(|dc| field.tile(i64::from(col) + dc, i64::from(row) + dr) < 0)
                })
            })
            .collect();
        for at in (1..open.len()).rev() {
            open.swap(at, state.rng.next_u32() as usize % (at + 1));
        }
        return (0..count)
            .map(|index| {
                let (col, row) = open[index % open.len().max(1)];
                let jitter = Vec2::new(
                    state.rng.next_range(-2.0, 2.0),
                    state.rng.next_range(-2.0, 2.0),
                );
                (Vec2::new(col as f32, row as f32) + 0.5) * CELL + jitter
            })
            .collect();
    }
    let (min, max) = state.spawn_region();
    let area = max - min;
    let columns = ((count as f32 * area.x / area.y).sqrt().ceil() as usize).max(1);
    let rows = count.div_ceil(columns).max(1);
    let cell = Vec2::new(area.x / columns as f32, area.y / rows as f32);
    (0..count)
        .map(|index| {
            let size = SIZES[index % SIZES.len()];
            let origin = min + Vec2::new((index % columns) as f32, (index / columns) as f32) * cell;
            origin
                + Vec2::new(
                    state.rng.next_range(size, (cell.x - size).max(size)),
                    state.rng.next_range(size, (cell.y - size).max(size)),
                )
        })
        .collect()
}

/// `cfg`'s bodies at their starting points, every other one a circle that
/// rolls, each under the bench's friction.
fn spawn_bodies(world: &mut World, state: &mut PileState, cfg: &BenchConfig) -> u32 {
    let count = cfg.int("bodies") as usize;
    let (friction, rolling) = (cfg.float("friction") as f32, cfg.float("rolling") as f32);
    let points = start_points(state, count);
    for (index, &at) in points.iter().enumerate() {
        let size = SIZES[index % SIZES.len()];
        let velocity = Vec2::new(state.rng.next_range(-SPAWN_SPEED_X, SPAWN_SPEED_X), 0.0);
        let round = index % 2 == 0;
        let collider = if round {
            Collider::circle(size)
        } else {
            Collider::aabb(Vec2::new(size, size * 0.8))
        };
        // Mass follows area, normalized to the 4 px body.
        let mut body = RigidBody::dynamic()
            .with_mass(size * size / 16.0)
            .with_restitution(RESTITUTION)
            .with_friction(friction);
        if round {
            body = body.with_rolling(rolling);
        }
        let mut bundle = RigidBodyBundle::dynamic(Position(at), collider).with_body(body);
        if round {
            bundle = bundle.with_spin();
        }
        let entity = world.spawn_with(bundle);
        world.insert(entity, Velocity(velocity));
        world.insert(entity, Body { size });
        world.insert(
            entity,
            ViewSprite {
                half: collider.shape.half_extents(),
                color: BODY_COLORS[index % BODY_COLORS.len()],
                texture: if round {
                    ViewTexture::Disc
                } else {
                    ViewTexture::Rect
                },
            },
        );
    }
    count as u32
}

/// Each paddle's sweep, its centre's least and greatest x: the bin's floor
/// in sections of [`PADDLE_SECTION`] cells, the last taking the rest.
fn paddle_sweeps(field: &Field) -> Vec<(f32, f32)> {
    let cells = field.width - 2;
    let count = (cells / PADDLE_SECTION).max(1);
    let width = (cells / count) as f32 * CELL;
    (0..count)
        .map(|index| {
            let start = CELL + index as f32 * width;
            let end = if index + 1 == count {
                CELL + field.interior().x
            } else {
                start + width
            };
            (
                start + PADDLE_CLEARANCE + PADDLE_HALF.x,
                end - PADDLE_CLEARANCE - PADDLE_HALF.x,
            )
        })
        .collect()
}

/// The paddles, each standing on the bin's floor at its sweep's left end,
/// all moving right, so none closes on another.
fn spawn_paddles(world: &mut World, field: &Field, speed: f32) {
    let floor = field.interior().y;
    for (low, high) in paddle_sweeps(field) {
        let entity = world.spawn_with(RigidBodyBundle::kinematic(
            Position(Vec2::new(low, floor - PADDLE_HALF.y)),
            Collider::aabb(PADDLE_HALF),
        ));
        world.insert(entity, Velocity(Vec2::new(speed, 0.0)));
        world.insert(entity, Paddle { speed, low, high });
        world.insert(
            entity,
            ViewSprite {
                half: PADDLE_HALF,
                color: PADDLE_COLOR,
                texture: ViewTexture::Rect,
            },
        );
    }
}

/// Before the step: each paddle turns back at either end of its sweep.
fn piles_paddle(world: &mut World) {
    for (position, velocity, paddle) in world.query_mut::<(&Position, &mut Velocity, &Paddle)>() {
        if position.0.x >= paddle.high {
            velocity.0.x = -paddle.speed;
        } else if position.0.x <= paddle.low {
            velocity.0.x = paddle.speed;
        }
    }
}

/// After the step: under `slopes`, each body that touched the floor returns
/// to the spawn band, at rest; then the frame's awake, sleeping, escaped,
/// inside and spinning counts.
fn piles_after_step(world: &mut World) {
    let Some(mut state) = world.remove_resource::<PileState>() else {
        return;
    };
    let mut recycled = 0;
    if state.slopes {
        let floor = state.field.height - 1;
        let mut seen = HashSet::new();
        let landed: Vec<Entity> = world
            .get_resource::<EventQueue<CollisionEvent>>()
            .map(|queue| {
                queue
                    .iter_current()
                    .filter(|event| event.tile.is_some_and(|tile| tile.row == floor))
                    .map(|event| event.a)
                    .filter(|&body| seen.insert(body))
                    .collect()
            })
            .unwrap_or_default();
        for body in landed {
            let Some(size) = world.get::<Body>(body).map(|body| body.size) else {
                continue;
            };
            let at = state.band_point(size);
            tungsten::physics::teleport(world, body, at);
            if let Some(spin) = world.get_mut::<Spin>(body) {
                spin.angular_velocity = 0.0;
            }
            recycled += 1;
        }
        state.recycled.push_back(recycled);
        if state.recycled.len() > RECYCLE_WINDOW {
            state.recycled.pop_front();
        }
    }
    // Out of the bin: past a wall's or the floor's cells, not pressed into
    // them, as the paddle presses a pile against a wall.
    let size = state.field.size();
    let (mut escaped, mut inside, mut spinning) = (0, 0, 0);
    for (_entity, position, spin, _body) in
        world.query::<(Entity, &Position, Option<&Spin>, &Body)>()
    {
        let at = position.0;
        if at.x < 0.0 || at.x > size.x || at.y > size.y {
            escaped += 1;
        }
        if in_slope(&state.field, at) {
            inside += 1;
        }
        if spin.is_some_and(|spin| spin.angular_velocity.abs() > SPINNING) {
            spinning += 1;
        }
    }
    let (awake, sleeping) = world
        .get_resource::<PhysicsBuffers>()
        .map_or((0, 0), |buffers| {
            let sleeping = buffers.sleeping_count();
            (
                buffers.dynamic_count().saturating_sub(sleeping) as u32,
                sleeping as u32,
            )
        });
    let window = state.recycled.iter().sum();
    world.insert_resource(state);
    if let Some(counters) = world.get_resource_mut::<BenchCounters<PileCounts>>() {
        let counts = &mut counters.counts;
        counts.awake = awake;
        counts.sleeping = sleeping;
        counts.escaped = escaped;
        counts.inside = inside;
        counts.spinning = spinning;
        counts.recycled = window;
    }
}

/// `bench:` counters: `recycled` sums the last 60 frames' returns to the
/// band; `visible` counts the bodies the culled view drew.
#[derive(Debug, Default)]
pub(crate) struct PileCounts {
    pub(crate) bodies: u32,
    pub(crate) awake: u32,
    pub(crate) sleeping: u32,
    pub(crate) escaped: u32,
    pub(crate) spinning: u32,
    pub(crate) recycled: u32,
    pub(crate) inside: u32,
}

impl FrameCounters for PileCounts {
    fn write(&self, world: &World, line: &mut String) {
        let bounds = view::view_bounds(world);
        let visible = world
            .query::<(&Position, &ViewSprite, &Body)>()
            .filter(|(position, sprite, _)| view::in_view(bounds, position.0, sprite.half))
            .count();
        let _ = write!(
            line,
            " bodies={} awake={} sleeping={} escaped={} spinning={} recycled={} inside={} visible={visible}",
            self.bodies,
            self.awake,
            self.sleeping,
            self.escaped,
            self.spinning,
            self.recycled,
            self.inside
        );
    }

    fn reset(&mut self) {
        self.awake = 0;
        self.sleeping = 0;
        self.escaped = 0;
        self.spinning = 0;
        self.recycled = 0;
        self.inside = 0;
    }
}
