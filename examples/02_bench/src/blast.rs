//! `blast`: a dense pile of circles asleep in a closed bin of 16 px tiles,
//! woken by radial impulses. It owns `physics_step` over a pile's wake and
//! its blasts, with no game system beside it: example 01's full pit (12,000
//! small balls 1,536 px across, its gravity, restitution and broadphase
//! cell) under the engine's own `apply_radial_impulse`.
//!
//! The pile starts as a hexagonal lattice at rest and settles asleep inside
//! the warm-up. From [`FIRST`] frames into the window a blast lands every
//! `period` frames at a seeded column, on the highest body there, so the
//! window holds the pile asleep, its island's wake at the first blast and
//! the blasts on an awake pile after it. The art layer draws through the
//! engine's tilemap extract, the bodies through the lean view.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::core::physics::apply_radial_impulse;
use tungsten::core::{
    CameraState, Collider, CollisionEvent, EventQueue, FilterMode, LayerKind, Pcg32,
    PhysicsBuffers, PhysicsConfig, Position, RigidBody, RigidBodyBundle, SpatialFilter, TileIndex,
    TilemapData, TilemapInstance, TilemapRegistry, World, splitmix64,
};
use tungsten::{App, extract_tilemaps};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen::{self, Cell, TextureSink};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "blast",
    workload_version: 1,
    warmup: WARMUP,
    gpu_timing: false,
    knobs: KNOBS,
    presets: PRESETS,
    row,
    validate,
    derived,
    engine_config: knobs::keep_engine_config,
    configure,
};

/// Frames the lattice takes to settle and sleep, with room to spare: at
/// `default` every body sleeps by frame 400.
const WARMUP: u32 = 480;
/// Window frames before the first blast, so a capture holds the pile asleep.
pub(crate) const FIRST: u32 = 20;

const KNOBS: &[Knob] = &[
    Knob::int("width", 98, 18, 512).note("Cells across, 16 px each, the walls' two included"),
    Knob::int("height", 144, 24, 512).note("Cells down, the ceiling's and the floor's included"),
    Knob::int("bodies", 12_000, 100, 50_000)
        .scaled()
        .note("Circles; 12,000 is example 01's full pit"),
    Knob::float("radius", 7.5, 2.0, 16.0),
    Knob::int("period", 20, 1, 600).note("Frames between blasts"),
    Knob::float("reach", 288.0, 16.0, 1_024.0).note("A blast's radius, px"),
    Knob::float("impulse", 2_500.0, 0.0, 10_000.0)
        .note("A blast's impulse at its centre, falling linearly to none at its reach"),
    Knob::float("gravity", 3_600.0, 100.0, 6_000.0),
    Knob::float("restitution", 0.85, 0.0, 1.0),
    Knob::float("cell", 64.0, 8.0, 256.0).note("`PhysicsConfig::broadphase_cell_size`"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("width", Value::Int(34)),
            ("height", Value::Int(48)),
            ("bodies", Value::Int(600)),
            ("reach", Value::Float(96.0)),
            ("impulse", Value::Float(1_200.0)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The map's registry ID and its one tile's sprite.
pub(crate) const BLAST: &str = "bench_blast";
const SOLID_SPRITE: &str = "bench_blast_solid";
const TILE_PX: u32 = 16;
const CELL: f32 = TILE_PX as f32;
const FULL: TileIndex = 0;
/// The lattice's gap between neighbours, and its seeded jitter, px.
const GAP: f32 = 0.5;
const JITTER: f32 = 0.2;
const LAYOUT_SALT: u64 = 0xB1A5_7000_0000_0001;
const BODY_COLORS: [[u8; 4]; 3] = [
    [242, 196, 92, 255],
    [120, 186, 240, 255],
    [168, 228, 140, 255],
];

fn row(_cfg: &BenchConfig) -> &'static str {
    "blast"
}

/// The bin's inside: its left and top, and its size, in px.
fn interior(cfg: &BenchConfig) -> (Vec2, Vec2) {
    let (width, height) = (cfg.int("width") as f32, cfg.int("height") as f32);
    (
        Vec2::splat(CELL),
        Vec2::new((width - 2.0) * CELL, (height - 2.0) * CELL),
    )
}

/// The lattice's columns in a full row and its rows for `cfg`'s bodies.
fn lattice(cfg: &BenchConfig) -> (usize, usize) {
    let pitch = 2.0 * cfg.float("radius") as f32 + GAP;
    let (_, size) = interior(cfg);
    // An odd row starts half a pitch in, so every row holds one less.
    let columns = (((size.x - GAP) / pitch).floor() as usize)
        .saturating_sub(1)
        .max(1);
    (columns, (cfg.int("bodies") as usize).div_ceil(columns))
}

fn validate(cfg: &BenchConfig) -> anyhow::Result<()> {
    let pitch = 2.0 * cfg.float("radius") as f32 + GAP;
    let (_, size) = interior(cfg);
    let (_, rows) = lattice(cfg);
    let tall = rows as f32 * pitch * 0.866_025_4 + pitch;
    anyhow::ensure!(
        tall <= size.y * 0.75,
        "blast: {} bodies stack {tall:.0} px in a bin {:.0} px high; it needs a quarter clear above them",
        cfg.int("bodies"),
        size.y
    );
    anyhow::ensure!(
        cfg.float("reach") as f32 <= size.x,
        "blast: a blast's reach is wider than the bin"
    );
    Ok(())
}

fn derived(cfg: &BenchConfig) -> Json {
    let (columns, rows) = lattice(cfg);
    let size = map_size(cfg);
    json!({
        "world": [size.x, size.y],
        "lattice": [columns, rows],
        "first_blast": WARMUP + FIRST,
        "zoom": (f64::from(zoom(size)) * 1e4).round() / 1e4,
    })
}

fn map_size(cfg: &BenchConfig) -> Vec2 {
    Vec2::new(cfg.int("width") as f32, cfg.int("height") as f32) * CELL
}

/// The zoom that fits a map of `size` px in the viewport.
fn zoom(size: Vec2) -> f32 {
    (VIEWPORT.x / size.x).min(VIEWPORT.y / size.y)
}

/// A body of the pile.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Body;

/// The row's state between frames.
struct BlastState {
    frame: u32,
    rng: Pcg32,
    period: u32,
    reach: f32,
    impulse: f32,
    radius: f32,
    /// The columns a blast's centre may take, px.
    span: (f32, f32),
    size: Vec2,
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let cells = (cfg.int("width") as u32, cfg.int("height") as u32);
    {
        let world = app.world_mut();
        if let Some(physics) = world.get_resource_mut::<PhysicsConfig>() {
            physics.gravity = Vec2::new(0.0, cfg.float("gravity") as f32);
            physics.broadphase_cell_size = cfg.float("cell") as f32;
        }
        let bodies = spawn_bodies(world, cfg);
        let size = map_size(cfg);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            let zoom = zoom(size);
            *camera = CameraState {
                position: size * 0.5 - VIEWPORT / zoom * 0.5,
                zoom,
                rotation: 0.0,
                ..CameraState::new()
            };
        }
        let (origin, inside) = interior(cfg);
        let reach = cfg.float("reach") as f32;
        let margin = (reach * 0.5).min(inside.x * 0.25);
        world.insert_resource(BlastState {
            frame: 0,
            rng: Pcg32::seeded(splitmix64(cfg.seed() ^ LAYOUT_SALT)),
            period: cfg.int("period") as u32,
            reach,
            impulse: cfg.float("impulse") as f32,
            radius: cfg.float("radius") as f32,
            span: (origin.x + margin, origin.x + inside.x - margin),
            size,
        });
        world.insert_resource(BenchCounters::new(BlastCounts {
            bodies,
            ..BlastCounts::default()
        }));
    }
    app.on_startup(move |world, renderer| {
        r#gen::register_view_textures(world, renderer);
        place_map(world, renderer, cells);
    });
    app.register_event::<CollisionEvent>();
    app.add_system_named("bench_counters", bench_counters_system::<BlastCounts>);
    app.add_system_named("blast_cast", blast_cast);
    app.add_system_named("physics_step", tungsten::physics::physics_step);
    app.add_system_named("blast_after_step", blast_after_step);
    app.set_extract_sprites(|world| {
        let mut batches = extract_tilemaps(world);
        batches.extend(view::extract_view(world));
        batches
    });
}

/// The startup hook's World part at `cfg`, for the counter guard, which
/// runs no renderer: the lean view's textures, then the map's.
#[cfg(test)]
pub(crate) fn startup(world: &mut World, textures: &mut dyn TextureSink, cfg: &BenchConfig) {
    r#gen::register_view_textures(world, textures);
    place_map(
        world,
        textures,
        (cfg.int("width") as u32, cfg.int("height") as u32),
    );
}

/// The bin: a wall down each side, a floor and a ceiling, in an art layer
/// and a collision layer, registered after manifest composition, which
/// replaces the registry, and its instance at the origin.
fn place_map(world: &mut World, textures: &mut dyn TextureSink, (width, height): (u32, u32)) {
    let cells = [Cell {
        id: SOLID_SPRITE.to_string(),
        rgba: r#gen::pile_tile_rgba(None, TILE_PX),
    }];
    r#gen::register_atlas(world, textures, FilterMode::Nearest, TILE_PX, &cells);
    let mut map = TilemapData::new(TILE_PX, TILE_PX, width, height);
    let tile = map.add_tile(SOLID_SPRITE.to_string());
    debug_assert_eq!(tile, FULL);
    let art = map.add_layer("art", LayerKind::Render);
    let solid = map.add_layer("solid", LayerKind::Collision);
    for row in 0..height {
        for col in 0..width {
            if row == 0 || row == height - 1 || col == 0 || col == width - 1 {
                map.set_tile(art, col, row, FULL);
                map.set_tile(solid, col, row, FULL);
            }
        }
    }
    world
        .get_resource_mut::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing")
        .insert(BLAST.to_string(), map);
    let entity = world.spawn();
    world.insert(entity, TilemapInstance::new(BLAST, Vec2::ZERO));
}

/// `cfg`'s bodies at rest on a hexagonal lattice from the bin's floor up,
/// each jittered by its seed.
fn spawn_bodies(world: &mut World, cfg: &BenchConfig) -> u32 {
    let count = cfg.int("bodies") as usize;
    let radius = cfg.float("radius") as f32;
    let restitution = cfg.float("restitution") as f32;
    let pitch = 2.0 * radius + GAP;
    let (origin, inside) = interior(cfg);
    let (columns, _) = lattice(cfg);
    let floor = origin.y + inside.y;
    let mut rng = Pcg32::seeded(cfg.seed());
    for index in 0..count {
        let (col, row) = (index % columns, index / columns);
        let shift = if row % 2 == 1 { 0.5 * pitch } else { 0.0 };
        let at = Vec2::new(
            origin.x + GAP + radius + shift + col as f32 * pitch,
            floor - GAP - radius - row as f32 * pitch * 0.866_025_4,
        ) + Vec2::new(
            rng.next_range(-JITTER, JITTER),
            rng.next_range(-JITTER, JITTER),
        );
        let collider = Collider::circle(radius);
        let body = RigidBody::dynamic().with_restitution(restitution);
        let entity =
            world.spawn_with(RigidBodyBundle::dynamic(Position(at), collider).with_body(body));
        world.insert(entity, Body);
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

/// Before the step: on a blast's frame, a radial impulse centred on the
/// highest body within a body's width of a seeded column, as a projectile
/// meeting the pile there would land it.
fn blast_cast(world: &mut World) {
    let Some(state) = world.get_resource_mut::<BlastState>() else {
        return;
    };
    // The frame's index from 0, so the first blast is window frame `FIRST`.
    let at = state.frame;
    state.frame += 1;
    if at < WARMUP + FIRST || !(at - WARMUP - FIRST).is_multiple_of(state.period) {
        return;
    }
    let x = state.rng.next_range(state.span.0, state.span.1);
    let (reach, impulse, radius) = (state.reach, state.impulse, state.radius);
    let top = world
        .query::<(&Position, &Body)>()
        .filter(|(position, _)| (position.0.x - x).abs() <= 2.0 * radius)
        .map(|(position, _)| position.0.y)
        .fold(f32::INFINITY, f32::min);
    if !top.is_finite() {
        return;
    }
    let pushed = apply_radial_impulse(
        world,
        Vec2::new(x, top),
        reach,
        impulse,
        &SpatialFilter::new(),
    );
    if let Some(counters) = world.get_resource_mut::<BenchCounters<BlastCounts>>() {
        counters.counts.blasts = 1;
        counters.counts.pushed = pushed as u32;
    }
}

/// After the step: the frame's awake, sleeping and escaped bodies and the
/// collision events the step sent.
fn blast_after_step(world: &mut World) {
    let Some(size) = world.get_resource::<BlastState>().map(|state| state.size) else {
        return;
    };
    let escaped = world
        .query::<(&Position, &Body)>()
        .filter(|(position, _)| {
            let at = position.0;
            at.x < 0.0 || at.x > size.x || at.y < 0.0 || at.y > size.y
        })
        .count() as u32;
    let (awake, sleeping) = world
        .get_resource::<PhysicsBuffers>()
        .map_or((0, 0), |buffers| {
            let sleeping = buffers.sleeping_count();
            (
                buffers.dynamic_count().saturating_sub(sleeping) as u32,
                sleeping as u32,
            )
        });
    let events = world
        .get_resource::<EventQueue<CollisionEvent>>()
        .map_or(0, |queue| queue.iter_current().count() as u32);
    if let Some(counters) = world.get_resource_mut::<BenchCounters<BlastCounts>>() {
        let counts = &mut counters.counts;
        counts.awake = awake;
        counts.sleeping = sleeping;
        counts.escaped = escaped;
        counts.events = events;
    }
}

/// `bench:` counters: `blasts` is 1 on a blast's frame, `pushed` the bodies
/// its impulse reached, `events` the step's collision events and `visible`
/// the bodies the culled view drew.
#[derive(Debug, Default)]
pub(crate) struct BlastCounts {
    pub(crate) bodies: u32,
    pub(crate) awake: u32,
    pub(crate) sleeping: u32,
    pub(crate) escaped: u32,
    pub(crate) blasts: u32,
    pub(crate) pushed: u32,
    pub(crate) events: u32,
}

impl FrameCounters for BlastCounts {
    fn write(&self, world: &World, line: &mut String) {
        let bounds = view::view_bounds(world);
        let visible = world
            .query::<(&Position, &ViewSprite, &Body)>()
            .filter(|(position, sprite, _)| view::in_view(bounds, position.0, sprite.half))
            .count();
        let _ = write!(
            line,
            " bodies={} awake={} sleeping={} escaped={} blasts={} pushed={} events={} visible={visible}",
            self.bodies,
            self.awake,
            self.sleeping,
            self.escaped,
            self.blasts,
            self.pushed,
            self.events
        );
    }

    fn reset(&mut self) {
        self.awake = 0;
        self.sleeping = 0;
        self.escaped = 0;
        self.blasts = 0;
        self.pushed = 0;
        self.events = 0;
    }
}
