//! `integrated`: a composed side-scroller frame, the one benchmark judged on
//! total frame time and its tail. A seeded level (`integrated/level.rs`)
//! `level_tiles` wide and 96 tiles high has a tile collision layer, its
//! rendered terrain and two decoration layers, over four parallax layers.
//! `actors` lit, animated walkers patrol it: they turn at walls and ledges,
//! climb one-tile steps, hop now and then and squash on landing. Every tenth
//! is a caster that fires arcing projectiles at `fire_rate`. A hit despawns
//! the projectile, bursts sparks from a fresh emitter, flashes a struck
//! walker through the root manifest's `damage_flash` material, knocks and
//! wakes a struck crate, and adds camera trauma near the view. Crates sit in
//! piles and may sleep; lit props stand still; pickups bob through tweens;
//! each torch carries a point light and a fire emitter, and the light
//! extract keeps the 16 nearest the view. A HUD and `tags` name tags change
//! every frame, and the scripted camera (`CameraMode::Scripted`) ping-pongs
//! along the level at `camera_speed`.
//!
//! Row `integrated` owns `stage.total` p50/p95/p99 plus its jitter (p99 −
//! p50), and every stage is reported only. Everything draws through the
//! engine extracts; the tilemap batches go between the parallax batches and
//! the rest. `TUNGSTEN_RENDER_POST_AA` and `TUNGSTEN_RENDER_BLOOM_MAX_MIPS`
//! win over `aa` and `bloom_mips` when set; the runner clears them.

use std::fmt::Write as _;

use glam::{Vec2, Vec3};
use serde_json::{Value as Json, json};
use tungsten::core::post::{
    BloomParams, ColorAdjustParams, FilmGrainParams, FogParams, GodRaysParams, PostPass, PostStack,
    TonemapParams, VignetteParams,
};
use tungsten::core::{
    AmbientLight, CameraBounds, CameraController, CameraMode, CameraState, Config, Light,
    LightKind, Particle, ParticleBudget, PhysicsConfig, PostAaMode, Transform, World,
};
use tungsten::render::{SpriteBatch, TextSection};
use tungsten::{
    App, camera_update_system, extract_sprites_default, extract_tilemaps, shake_tick_system,
    squash_stretch_tick_system, squash_stretch_trigger_system,
};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::knobs::{self, Bench, BenchConfig, Guard, Knob, Preset, Row, Value};
use crate::view::VIEWPORT;

mod assets;
mod level;
mod runtime;
mod scene;
mod systems;

pub(crate) static BENCH: Bench = Bench {
    name: "integrated",
    workload_version: 1,
    warmup: 120,
    gpu_timing: true,
    knobs: KNOBS,
    presets: PRESETS,
    rows: ROWS,
    row,
    validate: knobs::no_cross_checks,
    derived,
    engine_config,
    configure,
};

const KNOBS: &[Knob] = &[
    Knob::int("level_tiles", 1_536, 256, 16_384).note("Level width in 16 px tiles; 96 tiles high"),
    Knob::int("actors", 2_500, 0, 100_000)
        .scaled()
        .note("Walkers; every tenth is a caster"),
    Knob::int("crates", 2_000, 0, 100_000)
        .scaled()
        .note("Dynamic crates in piles of 10; they may sleep"),
    Knob::int("props", 6_000, 0, 200_000)
        .scaled()
        .note("Static lit and unlit decoration"),
    Knob::int("torches", 300, 0, 10_000)
        .scaled()
        .note("A point light and a fire emitter each; the extract keeps 16 lights"),
    Knob::int("pickups", 1_000, 0, 50_000)
        .scaled()
        .note("Bobbing through position tweens"),
    Knob::float("fire_rate", 0.3, 0.0, 5.0).note("Shots per second per caster"),
    Knob::int("tags", 64, 0, 512)
        .note("Name tags on the walkers nearest the view center, rewritten every frame"),
    Knob::choice("tile_collision", "on", &["on", "off"])
        .note("off: merged static boxes replace the per-tile proxies"),
    Knob::choice("post", "game", &["none", "game", "full"]).note(
        "game: bloom, tonemap, vignette; full adds god rays, fog, color adjust, \
         chromatic aberration and film grain",
    ),
    Knob::choice("aa", "smaa_high", &["off", "smaa_high"])
        .note("TUNGSTEN_RENDER_POST_AA wins when set"),
    Knob::int("bloom_mips", 6, 1, 8).note("TUNGSTEN_RENDER_BLOOM_MAX_MIPS wins when set"),
    Knob::float("camera_speed", 360.0, 0.0, 2_000.0)
        .note("Scripted camera speed in px/s; ping-pongs along the level"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("level_tiles", Value::Int(256)),
            ("actors", Value::Int(50)),
            ("crates", Value::Int(50)),
            ("props", Value::Int(100)),
            ("torches", Value::Int(8)),
            ("pickups", Value::Int(20)),
            ("tags", Value::Int(4)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

const COUNTERS: &[&str] = &[
    "actors",
    "projectiles",
    "hits",
    "particles",
    "lights",
    "camera_x",
    "view_out",
    "flashing",
    "landings",
    "shots",
    "turns",
    "events",
];

const ROWS: &[Row] = &[Row {
    name: "integrated",
    preset: "default",
    owned: &[("stage.total", &["p50", "p95", "p99", "jitter"])],
    guards: &[Guard::CounterMax {
        counter: "view_out",
        max: 0,
    }],
    counters: COUNTERS,
    bottleneck: "system.physics_step",
    key_knobs: &["actors", "crates", "props", "torches", "pickups"],
    note: "Judged on total frame time; `jitter` is p99 - p50 of `total`, judged with p99's \
           threshold. No stage is owned: the declared bottleneck, \
           physics_step, is the stage calibration found limiting (walkers and crates against the \
           tile proxies), ahead of the default extract. Tiles draw at z_norm 0, so the row needs \
           the default cpu_stable depth sort (under gpu_depth they cover every sprite). Particle \
           and tween time is timed under the engine's `post_update` systems (M38). Changing \
           HUD and name-tag text grows RSS through the text buffer cache (360-frame TTL)",
}];

/// Every n-th walker is a caster.
const CASTER_EVERY: u32 = 10;
/// Crates per pile: rows of 4, 3, 2 and 1.
const PILE: u32 = 10;
/// Live fire particles one torch can hold.
const FIRE_MAX_ALIVE: u32 = 32;
/// Particle budget headroom for spark bursts.
const SPARK_BUDGET: u32 = 4_096;
const GRAVITY: f32 = 900.0;
/// Room the scripted camera keeps from the level's edges; more than the
/// largest shake offset, so the view never leaves the level.
const CAMERA_MARGIN: f32 = 16.0;
const SHAKE_MAX: Vec2 = Vec2::new(6.0, 4.0);
const SHAKE_DECAY: f32 = 1.5;
const SHAKE_HZ: f32 = 18.0;
const AMBIENT: Vec3 = Vec3::new(0.34, 0.34, 0.4);

/// Resolved knob values in the types the scene uses.
#[derive(Debug, Clone, Copy)]
struct Params {
    level_tiles: u32,
    actors: u32,
    crates: u32,
    props: u32,
    torches: u32,
    pickups: u32,
    fire_rate: f32,
    tags: u32,
    tile_collision: bool,
    post: &'static str,
    aa: &'static str,
    bloom_mips: u32,
    camera_speed: f32,
    seed: u64,
}

impl Params {
    fn new(cfg: &BenchConfig) -> Self {
        let int = |name| cfg.int(name) as u32;
        Self {
            level_tiles: int("level_tiles"),
            actors: int("actors"),
            crates: int("crates"),
            props: int("props"),
            torches: int("torches"),
            pickups: int("pickups"),
            fire_rate: cfg.float("fire_rate") as f32,
            tags: int("tags"),
            tile_collision: cfg.choice("tile_collision") == "on",
            post: cfg.choice("post"),
            aa: cfg.choice("aa"),
            bloom_mips: int("bloom_mips"),
            camera_speed: cfg.float("camera_speed") as f32,
            seed: cfg.seed(),
        }
    }

    fn casters(&self) -> u32 {
        self.actors.div_ceil(CASTER_EVERY)
    }

    /// Enough for every torch's fire plus the spark bursts, so the budget
    /// never clips the scaled workload.
    fn particle_budget(&self) -> u32 {
        self.torches * FIRE_MAX_ALIVE + SPARK_BUDGET
    }
}

/// The scripted camera's path: its top-left ping-pongs over `x0..=x1` at
/// height `y`, `CAMERA_MARGIN` inside the level.
#[derive(Debug, Clone, Copy)]
struct CameraPath {
    x0: f32,
    x1: f32,
    y: f32,
}

impl CameraPath {
    fn new(level: Vec2) -> Self {
        let x0 = CAMERA_MARGIN;
        Self {
            x0,
            x1: (level.x - VIEWPORT.x - CAMERA_MARGIN).max(x0),
            y: (level.y - VIEWPORT.y - CAMERA_MARGIN).max(0.0),
        }
    }

    /// The top-left's x after `travel` px along the path.
    fn at(&self, travel: f32) -> f32 {
        let span = self.x1 - self.x0;
        if span <= 0.0 {
            return self.x0;
        }
        let phase = travel % (2.0 * span);
        self.x0
            + if phase > span {
                2.0 * span - phase
            } else {
                phase
            }
    }
}

fn row(_cfg: &BenchConfig) -> &'static str {
    "integrated"
}

fn derived(cfg: &BenchConfig) -> Json {
    let params = Params::new(cfg);
    let level = level::Level::generate(params.level_tiles, params.seed);
    let size = level.size();
    let camera = CameraPath::new(size);
    let (tiles, boxes) = if params.tile_collision {
        (level.solid_tiles(), 0)
    } else {
        (0, level.boxes().len())
    };
    json!({
        "world": [size.x, size.y],
        "view": [VIEWPORT.x, VIEWPORT.y],
        "tile_proxies": tiles,
        "static_boxes": boxes,
        "spans": level.spans.len(),
        "walkable_px": level.walkable().round(),
        "casters": params.casters(),
        "crate_piles": params.crates.div_ceil(PILE),
        "particle_budget": params.particle_budget(),
        "camera": {"x": [camera.x0, camera.x1], "y": camera.y},
    })
}

fn engine_config(config: &mut Config, cfg: &BenchConfig) {
    let params = Params::new(cfg);
    let post_aa = if params.aa == "smaa_high" {
        PostAaMode::SmaaHigh
    } else {
        PostAaMode::Off
    };
    let render = &mut config.render;
    knobs::unless_env(
        "integrated",
        &mut render.post_aa,
        "TUNGSTEN_RENDER_POST_AA",
        "aa",
        post_aa,
    );
    knobs::unless_env(
        "integrated",
        &mut render.bloom_max_mips,
        "TUNGSTEN_RENDER_BLOOM_MAX_MIPS",
        "bloom_mips",
        params.bloom_mips,
    );
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let params = Params::new(cfg);
    let level = level::Level::generate(params.level_tiles, params.seed);
    let camera = CameraPath::new(level.size());
    {
        let world = app.world_mut();
        if let Some(physics) = world.get_resource_mut::<PhysicsConfig>() {
            physics.gravity = Vec2::new(0.0, GRAVITY);
        }
        world.insert_resource(ParticleBudget {
            global_cap: params.particle_budget(),
        });
        world.insert_resource(AmbientLight(AMBIENT));
        world.insert_resource(post_stack(params.post));
        if let Some(controller) = world.get_resource_mut::<CameraController>() {
            controller.mode = CameraMode::Scripted;
            controller.bounds = Some(CameraBounds {
                min: Vec2::ZERO,
                max: level.size(),
            });
            controller.shake_max_offset = SHAKE_MAX;
            controller.shake_decay = SHAKE_DECAY;
            controller.shake_frequency_hz = SHAKE_HZ;
        }
        if let Some(state) = world.get_resource_mut::<CameraState>() {
            *state = CameraState {
                position: Vec2::new(camera.x0, camera.y),
                zoom: 1.0,
                rotation: 0.0,
            };
        }
        world.insert_resource(BenchCounters::new(IntegratedCounts::default()));
    }
    // The root manifest provides the `damage_flash` material and the fonts.
    app.set_manifest_roots(vec!["assets/manifest.json".into()]);
    app.on_startup(move |world, renderer| scene::startup(world, renderer, params, level, camera));
    // Hand-wired engine systems send these; without `PhysicsPlugin` and
    // `GameFeelPlugin` the row registers the queues itself.
    app.register_event::<tungsten::core::CollisionEvent>();
    app.register_event::<tungsten::core::ShakeEvent>();
    app.register_event::<tungsten::core::SquashEvent>();
    app.add_system_named("bench_counters", bench_counters_system::<IntegratedCounts>);
    app.add_system_named("actor_ai", systems::actor_ai);
    app.add_system_named("projectiles", systems::projectiles);
    app.add_system_named("physics_step", tungsten::physics::physics_step);
    app.add_system_named("collision_events", systems::collision_events);
    app.add_system_named("projectile_hits", systems::projectile_hits);
    app.add_system_named("flash_restore", systems::flash_restore);
    app.add_system_named("spark_cleanup", systems::spark_cleanup);
    app.add_system_named("animate_actors", systems::animate_actors);
    // Game feel (`D-073`): senders, then the squash pair, then shake before
    // the camera update.
    app.add_system_named("squash_stretch_trigger", squash_stretch_trigger_system);
    app.add_system_named("squash_stretch_tick", squash_stretch_tick_system);
    app.add_system_named("sync_bodies", systems::sync_bodies);
    app.add_system_named("camera_script", systems::camera_script);
    app.add_system_named("shake_tick", shake_tick_system);
    app.add_system_named("camera_update", camera_update_system);
    app.add_system_named("torch_flicker", systems::torch_flicker);
    app.add_system_named("name_tags", systems::name_tags);
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);
}

fn post_stack(post: &str) -> PostStack {
    // A higher threshold than `gpu`'s: this frame is dense with bright
    // sprites, and a 0.6 threshold veils all of it. The passes are the same.
    let bloom = PostPass::Bloom(BloomParams {
        threshold: 0.8,
        knee: 0.3,
        intensity: 0.5,
        radius: 1.0,
    });
    let tonemap = PostPass::Tonemap(TonemapParams::default());
    let vignette = PostPass::Vignette(VignetteParams::default());
    PostStack(match post {
        "none" => Vec::new(),
        "game" => vec![bloom, tonemap, vignette],
        _ => vec![
            bloom,
            PostPass::GodRays(GodRaysParams::default()),
            PostPass::Fog(FogParams::default()),
            PostPass::ColorAdjust(ColorAdjustParams {
                hue: 0.0,
                saturation: 1.1,
                contrast: 1.05,
            }),
            tonemap,
            PostPass::ChromaticAberration(2.0),
            PostPass::FilmGrain(FilmGrainParams::default()),
            vignette,
        ],
    })
}

/// Parallax batches first, then the tilemap, then every other sprite. The
/// parallax strips share one atlas page and the lowest z orders, so the
/// default extract emits their batches first.
fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let mut sprites = extract_sprites_default(world);
    let page = world
        .get_resource::<runtime::Runtime>()
        .and_then(|runtime| runtime.parallax_page);
    let split = page.map_or(0, |page| {
        sprites
            .iter()
            .take_while(|batch| batch.texture == page)
            .count()
    });
    let front = sprites.split_off(split);
    sprites.extend(extract_tilemaps(world));
    sprites.extend(front);
    sprites
}

fn extract_text(world: &World) -> Vec<TextSection> {
    world
        .get_resource::<runtime::Runtime>()
        .map(|runtime| {
            let mut sections = runtime.hud.clone();
            sections.extend(runtime.tags.iter().cloned());
            sections
        })
        .unwrap_or_default()
}

/// Squared distance from `point` to the rectangle `min..max`.
fn distance_sq(point: Vec2, min: Vec2, max: Vec2) -> f32 {
    (point - point.clamp(min, max)).length_squared()
}

/// `bench:` counters. `lights` counts the point lights that reach the view,
/// before the extract keeps 16; `view_out` is how far the final view leaves
/// the level, in whole px (the guard).
#[derive(Debug, Default)]
struct IntegratedCounts {
    hits: u32,
    landings: u32,
    shots: u32,
    turns: u32,
    events: u32,
}

impl FrameCounters for IntegratedCounts {
    fn write(&self, world: &World, line: &mut String) {
        let Some(runtime) = world.get_resource::<runtime::Runtime>() else {
            return;
        };
        let camera = world
            .get_resource::<CameraState>()
            .copied()
            .unwrap_or_default();
        let (view_min, view_max) = camera.visible_world_aabb(VIEWPORT.x, VIEWPORT.y);
        let level = runtime.level.size();
        let out = [
            -view_min.x,
            -view_min.y,
            view_max.x - level.x,
            view_max.y - level.y,
        ]
        .into_iter()
        .fold(0.0f32, f32::max)
        .ceil();
        let lights = world
            .query2::<Transform, Light>()
            .filter(|(_, transform, light)| match light.kind {
                LightKind::Point { radius, .. } => {
                    distance_sq(transform.position, view_min, view_max) < radius * radius
                }
                LightKind::Directional { .. } => false,
            })
            .count();
        let _ = write!(
            line,
            " actors={} projectiles={} hits={} particles={} lights={lights} camera_x={} \
             view_out={out} flashing={} landings={} shots={} turns={} events={}",
            runtime.actors.len(),
            world.query::<runtime::Projectile>().count(),
            self.hits,
            world.query::<Particle>().count(),
            camera.position.x.round(),
            runtime.actors.flashing(),
            self.landings,
            self.shots,
            self.turns,
            self.events,
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}
