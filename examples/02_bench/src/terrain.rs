//! `terrain`: falling bodies on a tile map edited every frame. It owns the
//! runtime terrain's share of `physics_step` (redo 7.8): the registry's
//! writes and journal, the tile cache's chunk re-traces and proxy rebuild,
//! the pair grid's tile restage and the sensors' re-test of the changed
//! cells.
//!
//! A seeded cave of 16 px cells under a band of sky fills a collision layer
//! and a styled art layer, whose 16 generated variants and brush the
//! auto-tiler picks by open sides. Each frame refills the cells it broke 60
//! frames before, then breaks `edits` surface cells, each a seeded one of
//! the floors (solid cells under air) of a column on a seeded walk, in both
//! layers, so from the 60th frame on the solid count holds. Circles fall on
//! it, through the floors broken under them and out of the map, and one
//! below the map respawns in the sky, so the row never settles; `sensors`
//! sensors span the map's height. The art layer draws through the engine's tilemap extract,
//! the bodies through the lean view.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::core::{
    CameraState, Collider, CollisionEvent, EMPTY_TILE, Entity, FilterMode, LayerKind, Pcg32,
    PhysicsBuffers, PhysicsConfig, Position, RigidBody, SensorEntered, TileIndex, TileProps,
    TilemapData, TilemapInstance, TilemapRegistry, Velocity, World, splitmix64,
};
use tungsten::{App, extract_tilemaps};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen::{self, Cell, TextureSink};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "terrain",
    workload_version: 1,
    warmup: 120,
    gpu_timing: false,
    knobs: KNOBS,
    presets: PRESETS,
    row,
    validate: knobs::no_cross_checks,
    derived,
    engine_config: knobs::keep_engine_config,
    configure,
};

const KNOBS: &[Knob] = &[
    Knob::int("width", 512, 32, 4_096).note("Cells across, 16 px each"),
    Knob::int("height", 192, 32, 1_024).note("Cells down, the sky's band included"),
    Knob::int("bodies", 1_200, 0, 20_000).scaled(),
    Knob::int("edits", 32, 1, 256)
        .scaled()
        .note("Surface cells broken a frame, in both layers; as many refilled"),
    Knob::int("sensors", 4, 0, 16).note("Each a column the map's height"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("width", Value::Int(64)),
            ("height", Value::Int(32)),
            ("bodies", Value::Int(60)),
            ("edits", Value::Int(4)),
            ("sensors", Value::Int(1)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The map's registry ID.
pub(crate) const TERRAIN: &str = "bench_terrain";
const TILE_PX: u32 = 16;
/// The art layer, drawn, then the collision layer.
const ART: usize = 0;
const SOLID: usize = 1;
/// Tileset entries: the collision layer's plain tile, the style's brush,
/// then the variant of each mask from 0 to 15.
const PLAIN: TileIndex = 0;
const BRUSH: TileIndex = 1;
const STYLE: &str = "rock";

/// Share of the cave's cells solid before smoothing, and the passes.
const FILL: f32 = 0.47;
const SMOOTHING: u32 = 4;
const CAVE_SALT: u64 = 0x7E44_A1A5_0000_0001;
/// Frames a broken cell stays broken.
const REFILL_AFTER: usize = 60;
/// The walk's largest step, in columns.
const WALK_STEP: u32 = 7;

const GRAVITY: f32 = 900.0;
const RESTITUTION: f32 = 0.2;
const RADII: [f32; 3] = [3.0, 4.5, 6.0];
/// How far below the map a body falls before it respawns.
const RESPAWN_BELOW: f32 = 64.0;
const SPAWN_SPEED_X: f32 = 40.0;
const SENSOR_HALF_WIDTH: f32 = 8.0;
const BODY_COLORS: [[u8; 4]; 3] = [
    [242, 196, 92, 255],
    [120, 186, 240, 255],
    [168, 228, 140, 255],
];

fn row(_cfg: &BenchConfig) -> &'static str {
    "terrain"
}

fn round4(value: f64) -> f64 {
    (value * 1e4).round() / 1e4
}

fn derived(cfg: &BenchConfig) -> Json {
    let cave = Cave::new(cfg);
    let size = cave.size();
    json!({
        "world": [size.x, size.y],
        "sky": cave.sky,
        "drains": cave.drains,
        "solid": cave.solid_count(),
        "chunks": cave.width.div_ceil(16) * cave.height.div_ceil(16),
        "zoom": round4(f64::from(zoom(size))),
        "traced_max": 8 * cfg.int("edits"),
    })
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let cave = Cave::new(cfg);
    {
        let world = app.world_mut();
        if let Some(physics) = world.get_resource_mut::<PhysicsConfig>() {
            physics.gravity = Vec2::new(0.0, GRAVITY);
        }
        let mut state = TerrainState::new(cfg, &cave);
        let bodies = spawn_bodies(world, &mut state, cfg.int("bodies") as usize);
        spawn_sensors(world, &cave, cfg.int("sensors") as u32);
        let size = cave.size();
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            let zoom = zoom(size);
            *camera = CameraState {
                position: size * 0.5 - VIEWPORT / zoom * 0.5,
                zoom,
                rotation: 0.0,
            };
        }
        world.insert_resource(state);
        world.insert_resource(BenchCounters::new(TerrainCounts {
            bodies,
            ..TerrainCounts::default()
        }));
    }
    app.on_startup(move |world, renderer| {
        r#gen::register_view_textures(world, renderer);
        place_map(world, renderer, &cave);
    });
    // The hand-wired step sends these; without `PhysicsPlugin` the row
    // registers the queues itself.
    app.register_event::<CollisionEvent>();
    app.register_event::<SensorEntered>();
    app.add_system_named("bench_counters", bench_counters_system::<TerrainCounts>);
    app.add_system_named("terrain_edits", terrain_edits);
    app.add_system_named("physics_step", tungsten::physics::physics_step);
    app.add_system_named("terrain_respawn", terrain_respawn);
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
    place_map(world, textures, &Cave::new(cfg));
}

/// The art tiles on one page, the map in the registry, after manifest
/// composition, which replaces it, and its instance at the origin.
fn place_map(world: &mut World, textures: &mut dyn TextureSink, cave: &Cave) {
    let cells: Vec<Cell> = std::iter::once(None)
        .chain((0..16).map(Some))
        .map(|mask| Cell {
            id: sprite_id(mask),
            rgba: r#gen::terrain_rgba(mask, TILE_PX),
        })
        .collect();
    r#gen::register_atlas(world, textures, FilterMode::Nearest, TILE_PX, &cells);
    world
        .get_resource_mut::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing")
        .insert(TERRAIN.to_string(), cave.map());
    let entity = world.spawn();
    world.insert(entity, TilemapInstance::new(TERRAIN, Vec2::ZERO));
}

/// The art sprite of the variant for open sides `mask`, or of the brush.
fn sprite_id(mask: Option<u8>) -> String {
    mask.map_or_else(
        || "bench_terrain_brush".to_string(),
        |mask| format!("bench_terrain_{mask}"),
    )
}

/// The cave's cells, row-major, `true` solid: a band of sky, then rock
/// smoothed from seeded noise, cut by drains to the bottom.
struct Cave {
    width: u32,
    height: u32,
    /// Rows of sky at the top.
    sky: u32,
    /// Two-column shafts from the sky through the bottom row.
    drains: u32,
    solid: Vec<bool>,
}

impl Cave {
    fn new(cfg: &BenchConfig) -> Self {
        let (width, height) = (cfg.int("width") as u32, cfg.int("height") as u32);
        let sky = (height / 8).max(4);
        let mut rng = Pcg32::seeded(splitmix64(cfg.seed() ^ CAVE_SALT));
        let mut solid: Vec<bool> = (0..width * height)
            .map(|at| at / width >= sky && rng.next_f32_unit() < FILL)
            .collect();
        for _ in 0..SMOOTHING {
            solid = smooth(&solid, width, height, sky);
        }
        let drains = (width / 64).max(1);
        for _ in 0..drains {
            let col = rng.next_u32() % (width - 1);
            for row in sky..height {
                for col in [col, col + 1] {
                    solid[(row * width + col) as usize] = false;
                }
            }
        }
        Self {
            width,
            height,
            sky,
            drains,
            solid,
        }
    }

    fn size(&self) -> Vec2 {
        Vec2::new(
            (self.width * TILE_PX) as f32,
            (self.height * TILE_PX) as f32,
        )
    }

    fn solid_count(&self) -> usize {
        self.solid.iter().filter(|&&solid| solid).count()
    }

    /// The map: each solid cell the brush in the art layer and the plain
    /// tile in the collision layer.
    fn map(&self) -> TilemapData {
        let mut map = TilemapData::new(TILE_PX, TILE_PX, self.width, self.height);
        let plain = map.add_tile("bench_terrain_solid");
        let brush = map.add_tile(sprite_id(None));
        debug_assert_eq!((plain, brush), (PLAIN, BRUSH));
        let mut props = TileProps::default();
        props.style = Some(STYLE.into());
        map.set_tile_props(brush, props.clone());
        for mask in 0..16 {
            let tile = map.add_tile(sprite_id(Some(mask)));
            let mut variant = props.clone();
            variant.mask = Some(mask);
            map.set_tile_props(tile, variant);
        }
        let art = map.add_layer("rock", LayerKind::Render);
        let solid = map.add_layer("solid", LayerKind::Collision);
        debug_assert_eq!((art, solid), (ART, SOLID));
        for (at, _) in self.solid.iter().enumerate().filter(|(_, solid)| **solid) {
            let (col, row) = (at as u32 % self.width, at as u32 / self.width);
            map.set_tile(art, col, row, brush);
            map.set_tile(solid, col, row, plain);
        }
        map
    }
}

/// One smoothing pass: a cell turns solid with 5 of its 8 neighbours solid
/// and stays solid with 4; the sides and the bottom count as rock, the sky
/// as air.
fn smooth(solid: &[bool], width: u32, height: u32, sky: u32) -> Vec<bool> {
    let (w, h) = (i64::from(width), i64::from(height));
    (0..width * height)
        .map(|at| {
            let (col, row) = (i64::from(at % width), i64::from(at / width));
            if row < i64::from(sky) {
                return false;
            }
            let mut around = 0;
            for dr in -1..=1 {
                for dc in -1..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let (c, r) = (col + dc, row + dr);
                    let rock = c < 0 || c >= w || r >= h || (r >= 0 && solid[(r * w + c) as usize]);
                    around += u32::from(rock);
                }
            }
            around >= if solid[at as usize] { 4 } else { 5 }
        })
        .collect()
}

/// A falling body.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Body {
    radius: f32,
}

/// A sensor spanning the map's height.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sensor;

/// The row's state between frames.
struct TerrainState {
    width: u32,
    height: u32,
    sky: u32,
    edits: u32,
    /// The walk's column.
    walk: u32,
    rng: Pcg32,
    /// The cells broken, by frame modulo [`REFILL_AFTER`].
    broken: Vec<Vec<(u32, u32)>>,
    frame: usize,
    /// `PhysicsBuffers::terrain_traces` at the last frame's end.
    traces: u64,
}

impl TerrainState {
    fn new(cfg: &BenchConfig, cave: &Cave) -> Self {
        Self {
            width: cave.width,
            height: cave.height,
            sky: cave.sky,
            edits: cfg.int("edits") as u32,
            walk: 0,
            rng: Pcg32::seeded(cfg.seed()),
            broken: vec![Vec::new(); REFILL_AFTER],
            frame: 0,
            traces: 0,
        }
    }

    /// A point in the sky for a body of `radius`.
    fn sky_point(&mut self, radius: f32) -> Vec2 {
        let size = Vec2::new((self.width * TILE_PX) as f32, (self.sky * TILE_PX) as f32);
        Vec2::new(
            self.rng.next_range(radius, size.x - radius),
            self.rng.next_range(radius, size.y * 0.5),
        )
    }

    /// The walk's next column holding a floor, a solid cell under air, and
    /// a seeded one of its floors.
    fn surface(&mut self, map: &TilemapData) -> Option<(u32, u32)> {
        let solid = |col: u32, row: u32| map.tile_at(SOLID, col, row).is_some_and(|tile| tile >= 0);
        for _ in 0..self.width {
            self.walk = (self.walk + 1 + self.rng.next_u32() % WALK_STEP) % self.width;
            let col = self.walk;
            let floor = |row: &u32| solid(col, *row) && (*row == 0 || !solid(col, row - 1));
            let floors = (0..self.height).filter(floor).count() as u32;
            if floors > 0 {
                let pick = self.rng.next_u32() % floors;
                let row = (0..self.height).filter(floor).nth(pick as usize);
                return row.map(|row| (col, row));
            }
        }
        None
    }
}

/// `count` bodies on a jittered grid over the sky, falling.
fn spawn_bodies(world: &mut World, state: &mut TerrainState, count: usize) -> u32 {
    let sky = Vec2::new((state.width * TILE_PX) as f32, (state.sky * TILE_PX) as f32);
    let columns = ((count as f32 * sky.x / sky.y).sqrt().ceil() as usize).max(1);
    let rows = count.div_ceil(columns).max(1);
    let cell = Vec2::new(sky.x / columns as f32, sky.y / rows as f32);
    for index in 0..count {
        let radius = RADII[index % RADII.len()];
        let origin = Vec2::new(
            (index % columns) as f32 * cell.x,
            (index / columns) as f32 * cell.y,
        );
        let offset = Vec2::new(
            state.rng.next_range(radius, (cell.x - radius).max(radius)),
            state.rng.next_range(radius, (cell.y - radius).max(radius)),
        );
        let velocity = Vec2::new(
            state.rng.next_range(-SPAWN_SPEED_X, SPAWN_SPEED_X),
            state.rng.next_range(0.0, 60.0),
        );
        let entity = world.spawn();
        world.insert(entity, Position(origin + offset));
        world.insert(entity, Velocity(velocity));
        // Mass follows area, normalized to the 4.5 px body.
        world.insert(
            entity,
            RigidBody::dynamic()
                .with_mass(radius * radius / (4.5 * 4.5))
                .with_restitution(RESTITUTION),
        );
        world.insert(entity, Collider::circle(radius));
        world.insert(entity, Body { radius });
        world.insert(
            entity,
            ViewSprite {
                half: Vec2::splat(radius),
                color: BODY_COLORS[index % BODY_COLORS.len()],
                texture: ViewTexture::Disc,
            },
        );
    }
    count as u32
}

/// `count` sensors, each a column the map's height, spread evenly across.
fn spawn_sensors(world: &mut World, cave: &Cave, count: u32) {
    let size = cave.size();
    for index in 0..count {
        let x = size.x * (index as f32 + 0.5) / count as f32;
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(x, size.y * 0.5)));
        world.insert(entity, RigidBody::sensor());
        world.insert(
            entity,
            Collider::aabb(Vec2::new(SENSOR_HALF_WIDTH, size.y * 0.5)),
        );
        world.insert(entity, Sensor);
    }
}

fn with_counts(world: &mut World, update: impl FnOnce(&mut TerrainCounts)) {
    if let Some(counters) = world.get_resource_mut::<BenchCounters<TerrainCounts>>() {
        update(&mut counters.counts);
    }
}

/// Before the step: refills the cells broken [`REFILL_AFTER`] frames ago,
/// the brush in the art layer, then breaks `edits` surface cells in both.
fn terrain_edits(world: &mut World) {
    let Some(mut state) = world.remove_resource::<TerrainState>() else {
        return;
    };
    let slot = state.frame % REFILL_AFTER;
    let mut cells = std::mem::take(&mut state.broken[slot]);
    let refills = cells.len() as u32;
    if let Some(registry) = world.get_resource_mut::<TilemapRegistry>() {
        for &(col, row) in &cells {
            registry.set_tile(TERRAIN, SOLID, col, row, PLAIN);
            registry.set_tile(TERRAIN, ART, col, row, BRUSH);
        }
        cells.clear();
        for _ in 0..state.edits {
            let Some((col, row)) = registry.get(TERRAIN).and_then(|map| state.surface(map)) else {
                break;
            };
            registry.set_tile(TERRAIN, SOLID, col, row, EMPTY_TILE);
            registry.set_tile(TERRAIN, ART, col, row, EMPTY_TILE);
            cells.push((col, row));
        }
    }
    let edits = cells.len() as u32;
    state.broken[slot] = cells;
    state.frame += 1;
    world.insert_resource(state);
    with_counts(world, |counts| {
        counts.edits = edits;
        counts.refills = refills;
    });
}

/// After the step: a body below the map respawns in the sky; the step's
/// traces and awake bodies go to the counters.
fn terrain_respawn(world: &mut World) {
    let Some(mut state) = world.remove_resource::<TerrainState>() else {
        return;
    };
    let below = (state.height * TILE_PX) as f32 + RESPAWN_BELOW;
    let mut respawns = 0;
    for (_entity, position, velocity, body) in
        world.query_mut::<(Entity, &mut Position, &mut Velocity, &Body)>()
    {
        if position.0.y > below {
            position.0 = state.sky_point(body.radius);
            velocity.0 = Vec2::new(state.rng.next_range(-SPAWN_SPEED_X, SPAWN_SPEED_X), 0.0);
            respawns += 1;
        }
    }
    let (traces, awake) =
        world
            .get_resource::<PhysicsBuffers>()
            .map_or((state.traces, 0), |buffers| {
                let awake = buffers
                    .dynamic_count()
                    .saturating_sub(buffers.sleeping_count());
                (buffers.terrain_traces(), awake as u32)
            });
    let traced = (traces - state.traces) as u32;
    state.traces = traces;
    world.insert_resource(state);
    with_counts(world, |counts| {
        counts.traced = traced;
        counts.awake = awake;
        counts.respawns = respawns;
    });
}

/// `bench:` counters; `visible` counts the bodies the culled view drew.
#[derive(Debug, Default)]
pub(crate) struct TerrainCounts {
    pub(crate) bodies: u32,
    pub(crate) edits: u32,
    pub(crate) refills: u32,
    pub(crate) traced: u32,
    pub(crate) awake: u32,
    pub(crate) respawns: u32,
}

impl FrameCounters for TerrainCounts {
    fn write(&self, world: &World, line: &mut String) {
        let bounds = view::view_bounds(world);
        let visible = world
            .query::<(Entity, &Position, &Body)>()
            .filter(|(_, position, body)| {
                view::in_view(bounds, position.0, Vec2::splat(body.radius))
            })
            .count();
        let _ = write!(
            line,
            " bodies={} edits={} refills={} traced={} awake={} respawns={} visible={visible}",
            self.bodies, self.edits, self.refills, self.traced, self.awake, self.respawns
        );
    }

    fn reset(&mut self) {
        self.edits = 0;
        self.refills = 0;
        self.traced = 0;
        self.awake = 0;
        self.respawns = 0;
    }
}
