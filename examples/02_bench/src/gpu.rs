//! `gpu`: the render path. A sprite field over `z_layers` interleaved layers
//! mixes unlit textures (alternating nearest and linear filtering), lit orbs
//! and crates with normal and emissive maps, and `bench_heavy` material
//! sprites; a glow layer of soft alpha-blended sprites adds overdraw;
//! `lights` point lights move on seeded paths beside one directional light;
//! a render-only 2048 x 512 tilemap scrolls under a scripted camera; text
//! sections are rewritten every frame; the post chain follows `post`, `aa`
//! and `bloom_mips`. Everything draws through the engine extracts (tilemap,
//! default sprite and text), whose cost this benchmark owns.
//!
//! Row `gpu` is judged on the GPU diagnostic run's `gpu`, `render_span` and
//! per-pass rows plus `extract` and `render_encode`; row `gpu-throughput`
//! (preset `throughput`) on `extract` and `render_encode` alone.
//!
//! The sprite field and glow layer carry `ParallaxLayer` factor 0, so they
//! stay on screen while the camera scrolls, and nothing moves them after
//! startup. `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_POST_AA` and
//! `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` win over `aa` and `bloom_mips` when set,
//! so the smoke matrix can pin them; the runner clears them for captures.

use std::fmt::Write as _;

use glam::{Vec2, Vec3};
use serde_json::{Value as Json, json};
use tungsten::core::{AmbientLight, CameraState, Config, DeltaTime, PostAaMode, Resolution, World};
use tungsten::render::TextSection;
use tungsten::{App, extract_sprites_default, extract_tilemaps};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Row, Value};

mod scene;

pub(crate) static BENCH: Bench = Bench {
    name: "gpu",
    workload_version: 1,
    warmup: 60,
    gpu_timing: true,
    knobs: KNOBS,
    presets: PRESETS,
    rows: ROWS,
    row,
    validate,
    derived,
    engine_config,
    configure,
};

const RESOLUTIONS: &[&str] = &[
    "1280x720",
    "1600x900",
    "1920x1080",
    "2560x1440",
    "3840x2160",
];

const KNOBS: &[Knob] = &[
    Knob::int("sprites", 2_000, 0, 2_000_000).scaled(),
    Knob::int("sprite_px", 24, 4, 128),
    Knob::int("textures", 4, 1, 8)
        .note("Distinct unlit textures, alternating nearest and linear filtering"),
    Knob::int("z_layers", 8, 1, 64).note("Interleaved z layers; drives the batch count"),
    Knob::float("lit", 0.5, 0.0, 1.0).note("Share drawn lit, with normal and emissive maps"),
    Knob::int("lights", 16, 0, 16).note("Moving point lights, plus one directional (G3 for more)"),
    Knob::int("glow", 20_000, 0, 500_000)
        .scaled()
        .note("Soft alpha-blended overdraw sprites"),
    Knob::int("glow_px", 48, 8, 256),
    Knob::float("shader_share", 0.1, 0.0, 1.0)
        .note("Share drawn with the bench_heavy material; lit + shader_share <= 1"),
    Knob::int("shader_iters", 32, 0, 512).note("bench_heavy loop iterations per fragment"),
    Knob::int("tile_layers", 3, 0, 4).note("Layers of the 2048 x 512 render-only tilemap"),
    Knob::int("tile_px", 16, 8, 32).note("8, 16 or 32"),
    Knob::float("scroll", 600.0, 0.0, 4_000.0)
        .note("Camera speed in px/s; ping-pongs across the tilemap"),
    Knob::float("zoom", 1.0, 0.25, 2.0),
    Knob::int("glyphs", 4_000, 0, 50_000)
        .scaled()
        .note("Text glyphs, 40 per section"),
    Knob::float("text_change", 1.0, 0.0, 1.0).note("Share of sections rewritten per frame"),
    Knob::choice("post", "light", &["none", "light", "full"]).note(
        "light: bloom, vignette; full: bloom, god rays, fog, color adjust, tonemap, \
         chromatic aberration, film grain, vignette",
    ),
    Knob::choice("aa", "smaa_high", &["off", "smaa_high", "msaa4"])
        .note("TUNGSTEN_RENDER_MSAA and TUNGSTEN_RENDER_POST_AA win when set"),
    Knob::int("bloom_mips", 6, 1, 8).note("TUNGSTEN_RENDER_BLOOM_MAX_MIPS wins when set"),
    Knob::choice("resolution", "1920x1080", RESOLUTIONS),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("sprites", Value::Int(1_000)),
            ("glow", Value::Int(200)),
            ("lights", Value::Int(4)),
            ("tile_layers", Value::Int(1)),
            ("glyphs", Value::Int(200)),
            ("post", Value::Choice("light")),
            ("aa", Value::Choice("off")),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
    Preset {
        name: "throughput",
        set: &[
            ("sprites", Value::Int(400_000)),
            ("sprite_px", Value::Int(8)),
            ("textures", Value::Int(1)),
            ("z_layers", Value::Int(1)),
            ("lit", Value::Float(0.0)),
            ("lights", Value::Int(0)),
            ("glow", Value::Int(0)),
            ("shader_share", Value::Float(0.0)),
            ("tile_layers", Value::Int(0)),
            ("glyphs", Value::Int(0)),
            ("post", Value::Choice("none")),
            ("aa", Value::Choice("off")),
        ],
    },
    Preset {
        name: "visual",
        set: &[
            ("sprites", Value::Int(2_000)),
            ("z_layers", Value::Int(3)),
            ("lights", Value::Int(4)),
            ("glow", Value::Int(0)),
            ("shader_iters", Value::Int(8)),
            ("tile_layers", Value::Int(1)),
            ("scroll", Value::Float(0.0)),
            ("glyphs", Value::Int(64)),
            ("text_change", Value::Float(0.0)),
            ("post", Value::Choice("light")),
            ("aa", Value::Choice("off")),
            ("resolution", Value::Choice("1280x720")),
        ],
    },
];

const P50: &[&str] = &["p50"];
const P50_P95: &[&str] = &["p50", "p95"];
const COUNTERS: &[&str] = &[
    "sprites",
    "glow",
    "visible_tiles",
    "glyphs_changed",
    "lights",
];

const ROWS: &[Row] = &[
    Row {
        name: "gpu",
        preset: "default",
        owned: &[
            ("stage.gpu", P50_P95),
            ("gpu.render_span", P50_P95),
            ("gpu.post0_bloom_threshold", P50),
            ("gpu.post0_bloom_composite", P50),
            ("gpu.post1_vignette", P50),
            ("gpu.smaa_edges", P50),
            ("gpu.smaa_blend_weights", P50),
            ("gpu.smaa_neighborhood", P50),
            ("gpu.text", P50),
            ("stage.extract", P50_P95),
            ("stage.render_encode", P50_P95),
        ],
        guards: &[],
        counters: COUNTERS,
        bottleneck: "present",
        key_knobs: &["sprites"],
        note: "`gpu` (the scene pass), `render_span` and the pass rows come from the GPU \
               diagnostic run; without timestamp queries they are n/a and the capture stays valid",
    },
    Row {
        name: "gpu-throughput",
        preset: "throughput",
        owned: &[("stage.extract", P50_P95), ("stage.render_encode", P50_P95)],
        guards: &[],
        counters: COUNTERS,
        bottleneck: "stage.extract",
        key_knobs: &["sprites"],
        note: "",
    },
];

/// Characters per text section; the last section holds the remainder.
const SECTION_CHARS: u32 = 40;
/// Tilemap size in tiles.
const MAP_TILES: (u32, u32) = (2_048, 512);

/// One sprite's look; each kind is its own batch key within a z layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Unlit texture index; even indices filter nearest, odd linear.
    Unlit(u8),
    /// Lit disc with a sphere normal map (linear).
    Orb,
    /// Lit square with a bevelled normal map (nearest).
    Crate,
    /// `bench_heavy` material over unlit texture 0.
    Heavy,
}

impl Kind {
    /// Bit in a z layer's batch-key mask.
    fn bit(self) -> u16 {
        match self {
            Self::Unlit(texture) => 1 << texture,
            Self::Orb => 1 << 8,
            Self::Crate => 1 << 9,
            Self::Heavy => 1 << 10,
        }
    }
}

/// Resolved knob values in the types the scene uses.
#[derive(Debug, Clone, Copy)]
struct Params {
    sprites: u32,
    sprite_px: f32,
    textures: u32,
    z_layers: u32,
    lit: f64,
    lights: u32,
    glow: u32,
    glow_px: f32,
    shader_share: f64,
    shader_iters: i32,
    tile_layers: u32,
    tile_px: u32,
    scroll: f32,
    zoom: f32,
    glyphs: u32,
    text_change: f64,
    post: &'static str,
    aa: &'static str,
    bloom_mips: u32,
    resolution: (u32, u32),
    seed: u64,
}

impl Params {
    fn new(cfg: &BenchConfig) -> Self {
        let int = |name| cfg.int(name) as u32;
        Self {
            sprites: int("sprites"),
            sprite_px: cfg.int("sprite_px") as f32,
            textures: int("textures"),
            z_layers: int("z_layers"),
            lit: cfg.float("lit"),
            lights: int("lights"),
            glow: int("glow"),
            glow_px: cfg.int("glow_px") as f32,
            shader_share: cfg.float("shader_share"),
            shader_iters: cfg.int("shader_iters") as i32,
            tile_layers: int("tile_layers"),
            tile_px: int("tile_px"),
            scroll: cfg.float("scroll") as f32,
            zoom: cfg.float("zoom") as f32,
            glyphs: int("glyphs"),
            text_change: cfg.float("text_change"),
            post: cfg.choice("post"),
            aa: cfg.choice("aa"),
            bloom_mips: int("bloom_mips"),
            resolution: parse_resolution(cfg.choice("resolution")),
            seed: cfg.seed(),
        }
    }

    /// World-space size of the view.
    fn view(&self) -> Vec2 {
        Vec2::new(self.resolution.0 as f32, self.resolution.1 as f32) / self.zoom
    }

    /// Sprite `index`'s kind. Weyl sequences interleave the shares evenly
    /// within every z layer, so the layout needs no random draw.
    fn kind(&self, index: u32) -> Kind {
        const GOLDEN: f64 = 0.618_033_988_749_894_9;
        const PLASTIC: f64 = 0.754_877_666_246_692_7;
        let weyl = |alpha: f64| ((f64::from(index) + 1.0) * alpha).fract();
        let in_layer = index / self.z_layers;
        if weyl(GOLDEN) < self.lit {
            if in_layer.is_multiple_of(2) {
                Kind::Orb
            } else {
                Kind::Crate
            }
        } else if self.lit < 1.0 && weyl(PLASTIC) < self.shader_share / (1.0 - self.lit) {
            Kind::Heavy
        } else {
            Kind::Unlit((in_layer % self.textures) as u8)
        }
    }

    fn sections(&self) -> u32 {
        self.glyphs.div_ceil(SECTION_CHARS)
    }

    /// Sections rewritten every frame; the first ones change.
    fn changed_sections(&self) -> u32 {
        (f64::from(self.sections()) * self.text_change).round() as u32
    }

    fn section_chars(&self, section: u32) -> u32 {
        (self.glyphs - section * SECTION_CHARS).min(SECTION_CHARS)
    }

    /// Batches the extracts should produce: one per z layer and kind present,
    /// one for the glow layer, and one per tile layer (the tiles share an
    /// atlas page).
    fn batches(&self) -> (u32, u32, u32) {
        let mut masks = vec![0u16; self.z_layers as usize];
        for index in 0..self.sprites {
            masks[(index % self.z_layers) as usize] |= self.kind(index).bit();
        }
        let sprites = masks.iter().map(|mask| mask.count_ones()).sum();
        (sprites, u32::from(self.glow > 0), self.tile_layers)
    }
}

fn parse_resolution(text: &str) -> (u32, u32) {
    let (width, height) = text.split_once('x').expect("resolution choices are WxH");
    (
        width.parse().expect("resolution width"),
        height.parse().expect("resolution height"),
    )
}

fn row(cfg: &BenchConfig) -> &'static str {
    if cfg.preset == "throughput" {
        "gpu-throughput"
    } else {
        "gpu"
    }
}

fn validate(cfg: &BenchConfig) -> anyhow::Result<()> {
    let tile_px = cfg.int("tile_px");
    if ![8, 16, 32].contains(&tile_px) {
        anyhow::bail!("knob 'tile_px' must be 8, 16 or 32, got {tile_px}");
    }
    let (lit, share) = (cfg.float("lit"), cfg.float("shader_share"));
    if lit + share > 1.0 + 1e-9 {
        anyhow::bail!("knobs 'lit' ({lit}) and 'shader_share' ({share}) must sum to at most 1");
    }
    Ok(())
}

fn derived(cfg: &BenchConfig) -> Json {
    let params = Params::new(cfg);
    let (sprites, glow, tiles) = params.batches();
    let mut kinds = [0u32; 3];
    for index in 0..params.sprites {
        kinds[match params.kind(index) {
            Kind::Unlit(_) => 0,
            Kind::Orb | Kind::Crate => 1,
            Kind::Heavy => 2,
        }] += 1;
    }
    let view = params.view();
    json!({
        "view": [view.x, view.y],
        "tilemap_px": [MAP_TILES.0 * params.tile_px, MAP_TILES.1 * params.tile_px],
        "sprites_unlit": kinds[0],
        "sprites_lit": kinds[1],
        "sprites_heavy": kinds[2],
        "sections": params.sections(),
        "sections_changed": params.changed_sections(),
        "batches": {"sprites": sprites, "glow": glow, "tiles": tiles, "total": sprites + glow + tiles},
    })
}

fn engine_config(config: &mut Config, cfg: &BenchConfig) {
    let params = Params::new(cfg);
    config.display.resolution = Some(Resolution {
        width: params.resolution.0,
        height: params.resolution.1,
    });
    let render = &mut config.render;
    let msaa = if params.aa == "msaa4" { 4 } else { 1 };
    knobs::unless_env("gpu", &mut render.msaa, "TUNGSTEN_RENDER_MSAA", "aa", msaa);
    let post_aa = if params.aa == "smaa_high" {
        PostAaMode::SmaaHigh
    } else {
        PostAaMode::Off
    };
    knobs::unless_env(
        "gpu",
        &mut render.post_aa,
        "TUNGSTEN_RENDER_POST_AA",
        "aa",
        post_aa,
    );
    knobs::unless_env(
        "gpu",
        &mut render.bloom_max_mips,
        "TUNGSTEN_RENDER_BLOOM_MAX_MIPS",
        "bloom_mips",
        params.bloom_mips,
    );
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let params = Params::new(cfg);
    let view = params.view();
    let map_px = Vec2::new(
        (MAP_TILES.0 * params.tile_px) as f32,
        (MAP_TILES.1 * params.tile_px) as f32,
    );
    let state = GpuState {
        params,
        elapsed: 0.0,
        frame: 0,
        span_x: (map_px.x - view.x).max(0.0),
        camera_y: ((map_px.y - view.y) * 0.5).max(0.0),
    };
    {
        let world = app.world_mut();
        world.insert_resource(AmbientLight(Vec3::splat(0.3)));
        world.insert_resource(scene::post_stack(params.post));
        scene::spawn_lights(world, &params);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState {
                position: Vec2::new(0.0, state.camera_y),
                zoom: params.zoom,
                rotation: 0.0,
            };
        }
        world.insert_resource(state);
        world.insert_resource(BenchCounters::new(GpuCounts::default()));
    }
    app.set_manifest_roots(vec![
        "assets/manifest.json".into(),
        "examples/02_bench/assets/manifest.json".into(),
    ]);
    app.on_startup(move |world, renderer| scene::startup(world, renderer, &params));
    app.add_system_named("bench_counters", bench_counters_system::<GpuCounts>);
    app.add_system_named("gpu_camera", gpu_camera);
    app.add_system_named("gpu_lights", scene::move_lights);
    app.add_system_named("gpu_text", gpu_text);
    app.set_extract_sprites(|world| {
        let mut batches = extract_tilemaps(world);
        batches.extend(extract_sprites_default(world));
        batches
    });
    app.set_extract_text(extract_text);
}

/// Runtime state: the resolved knobs, time and the camera's scroll range.
#[derive(Debug)]
struct GpuState {
    params: Params,
    elapsed: f32,
    frame: u32,
    /// Horizontal camera travel before it turns back.
    span_x: f32,
    camera_y: f32,
}

/// Scripted camera: ping-pongs along the tilemap at `scroll` px/s.
fn gpu_camera(world: &mut World) {
    let dt = world.get_resource::<DeltaTime>().map_or(0.0, |dt| dt.dt);
    let Some(state) = world.get_resource_mut::<GpuState>() else {
        return;
    };
    state.elapsed += dt;
    let travel = state.params.scroll * state.elapsed;
    let x = if state.span_x > 0.0 {
        let phase = travel % (2.0 * state.span_x);
        if phase > state.span_x {
            2.0 * state.span_x - phase
        } else {
            phase
        }
    } else {
        0.0
    };
    let (camera_y, zoom) = (state.camera_y, state.params.zoom);
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.position = Vec2::new(x, camera_y);
        camera.zoom = zoom;
    }
}

/// Advances the text frame and counts the glyphs rewritten this frame.
fn gpu_text(world: &mut World) {
    let Some(state) = world.get_resource_mut::<GpuState>() else {
        return;
    };
    state.frame += 1;
    let params = state.params;
    let changed: u32 = (0..params.changed_sections())
        .map(|section| params.section_chars(section))
        .sum();
    if let Some(counters) = world.get_resource_mut::<BenchCounters<GpuCounts>>() {
        counters.counts.glyphs_changed = changed;
    }
}

/// Text sections in a column grid; changed sections show the current frame,
/// the rest frame 0.
fn extract_text(world: &World) -> Vec<TextSection> {
    let Some(state) = world.get_resource::<GpuState>() else {
        return Vec::new();
    };
    let params = &state.params;
    let changed = params.changed_sections();
    (0..params.sections())
        .map(|section| {
            let frame = if section < changed { state.frame } else { 0 };
            scene::text_section(params, section, frame)
        })
        .collect()
}

/// `bench:` counters; `visible_tiles` repeats the tilemap extract's culling.
#[derive(Debug, Default)]
struct GpuCounts {
    glyphs_changed: u32,
}

impl FrameCounters for GpuCounts {
    fn write(&self, world: &World, line: &mut String) {
        let Some(state) = world.get_resource::<GpuState>() else {
            return;
        };
        let params = &state.params;
        let _ = write!(
            line,
            " sprites={} glow={} visible_tiles={} glyphs_changed={} lights={}",
            params.sprites,
            params.glow,
            scene::visible_tiles(world),
            self.glyphs_changed,
            params.lights
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}
