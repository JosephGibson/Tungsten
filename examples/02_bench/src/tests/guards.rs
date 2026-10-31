//! Counter guards (redo 7.3): each ported row at a small configuration of its
//! own, stepped headless through its warm-up and a 60-frame window, its
//! counts pinned in `guards/<row>.json`. Each guard asserts its workload
//! first: the row the configuration names, the populations it spawned and,
//! every frame, as many extracted sprite instances as drawable entities, so
//! a pin is never read or written over a workload the guard has not seen.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::WindowSize;
use tungsten::core::assets::TextureHandle;
use tungsten::core::{
    AnimationState, AssetRegistry, CameraState, Config, Entity, FilterMode, Light, ParallaxLayer,
    ParticleEmitter, Position, Sprite, TilemapInstance, Transform, Visibility, World,
    parallax_world_position,
};
use tungsten::testing::{CounterGuard, Harness};

use crate::churn::Life;
use crate::counters::BenchCounters;
use crate::r#gen::{self, TextureSink};
use crate::knobs::{self, BenchConfig};
use crate::physics::Ball;
use crate::piles::{self, PileCounts};
use crate::terrain::{self, TerrainCounts};
use crate::view::{self, ViewSprite};
use crate::{BENCHES, bench_app, gpu, particles};

/// Frames each guard counts after its row's warm-up.
const WINDOW: u32 = 60;

/// Handles the harness allocates before a startup: more than any row
/// registers.
const HANDLES: u32 = 16;

/// The harness's texture sink: handles the harness allocated, in order, and
/// no upload, since no renderer runs.
struct HeadlessTextures(std::vec::IntoIter<TextureHandle>);

impl TextureSink for HeadlessTextures {
    fn allocate_texture_handle(&mut self) -> TextureHandle {
        self.0.next().expect("the guard allocates enough handles")
    }

    fn upload_texture(&mut self, _: TextureHandle, _: &[u8], _: u32, _: u32, _: FilterMode) {}

    fn upload_lit_texture(
        &mut self,
        _: TextureHandle,
        _: &[u8],
        _: &[u8],
        _: &[u8],
        _: u32,
        _: u32,
        _: FilterMode,
    ) {
    }
}

fn manifest_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// The app `main` builds at `cfg`, its manifest's headless sections loaded
/// for a row that sets one, and its startup hook's World part run on the
/// harness's handles, in the window's order: manifests, startup, frames.
fn harness(cfg: &BenchConfig) -> Harness {
    let config = Config::load(manifest_path("../../tungsten.json")).expect("tungsten.json");
    let mut harness = Harness::new(bench_app(config, cfg).expect("the bench app builds"));
    let handles: Vec<TextureHandle> = (0..HANDLES)
        .map(|_| harness.allocate_texture_handle())
        .collect();
    let mut textures = HeadlessTextures(handles.into_iter());
    match cfg.bench.name {
        "physics" | "ecs" | "churn" => {
            r#gen::register_view_textures(harness.world_mut(), &mut textures);
        }
        "particles" => {
            let clips = cfg.int("clips") as u32;
            particles::register_textures(harness.world_mut(), &mut textures, clips);
        }
        "gpu" => {
            harness
                .load_manifest(manifest_path("assets/manifest.json"))
                .expect("the bench manifest loads headless");
            gpu::startup(harness.world_mut(), &mut textures, cfg);
        }
        "terrain" => terrain::startup(harness.world_mut(), &mut textures, cfg),
        "piles" => piles::startup(harness.world_mut(), &mut textures, cfg),
        other => panic!("no guard startup for bench {other}"),
    }
    harness
}

fn count<T: 'static>(world: &World) -> u64 {
    world.query::<(Entity, &T)>().count() as u64
}

/// The lean view's drawable quads: each `ViewSprite` inside the camera's
/// view, by the view extract's own rule, whether its texture registered or
/// not.
fn view_drawable(world: &World) -> u64 {
    let bounds = view::view_bounds(world);
    world
        .query::<(&Position, &ViewSprite)>()
        .filter(|(position, sprite)| view::in_view(bounds, position.0, sprite.half))
        .count() as u64
}

/// The default extract's drawable sprites: each visible `Sprite` with a
/// `Transform` that the extract's cull keeps (`sprite_extract.rs`'s
/// `outside_view`, at `WindowSize` through the camera). A sprite without its
/// asset counts, so a missing registration shows as a shortfall; a sprite
/// drawn with a material is never culled.
fn sprites_drawable(world: &World) -> u64 {
    let assets = world.resource::<AssetRegistry>();
    let camera = world.resource::<CameraState>();
    let window = world.resource::<WindowSize>();
    let (min, max) = camera.visible_world_aabb(window.width as f32, window.height as f32);
    let rows = world.query::<(&Transform, &Sprite, &Visibility, Option<&ParallaxLayer>)>();
    rows.filter(|&(transform, sprite, visibility, parallax)| {
        if !visibility.visible {
            return false;
        }
        let Some(asset) = assets.sprite(sprite.asset_id) else {
            return true;
        };
        if sprite.material_id.is_some() && asset.lit_atlas.is_none() {
            return true;
        }
        let position = parallax.map_or(transform.position, |layer| {
            parallax_world_position(transform.position, layer.scroll_factor, camera.position)
        });
        let size = Vec2::new(
            asset.width as f32 * transform.scale.x,
            asset.height as f32 * transform.scale.y,
        );
        let (lo, hi) = if transform.rotation == 0.0 {
            (position.min(position + size), position.max(position + size))
        } else {
            let centre = position + size * 0.5;
            let half = Vec2::splat(size.abs().element_sum() * 0.5);
            (centre - half, centre + half)
        };
        !(hi.cmplt(min) | lo.cmpgt(max)).any()
    })
    .count() as u64
}

fn gpu_drawable(world: &World) -> u64 {
    sprites_drawable(world) + u64::from(gpu::visible_tiles(world))
}

/// Steps the warm-up, then counts the window a frame at a time, asserting
/// each frame's extracted instances against `drawable`. The checks run
/// between counted frames, so their allocations stay out of the reading.
fn measure(harness: &mut Harness, warmup: u32, drawable: fn(&World) -> u64) -> CounterGuard {
    measure_checking(harness, warmup, drawable, |_, _| {})
}

/// [`measure`], calling `check` with the World and the window frame's index
/// after each frame.
fn measure_checking(
    harness: &mut Harness,
    warmup: u32,
    drawable: fn(&World) -> u64,
    mut check: impl FnMut(&World, u32),
) -> CounterGuard {
    let mut total = CounterGuard::measure(harness, warmup, 0);
    for frame in 0..WINDOW {
        let one = CounterGuard::measure(harness, 0, 1);
        let expected = drawable(harness.world());
        assert_eq!(
            one.max_sprite_instances, expected,
            "window frame {frame}: the extract drew {} instances of {expected} drawable",
            one.max_sprite_instances
        );
        check(harness.world(), frame);
        total.allocations += one.allocations;
        total.allocated_bytes += one.allocated_bytes;
        total.archetype_moves += one.archetype_moves;
        total.spawned += one.spawned;
        total.despawned += one.despawned;
        total.events_sent += one.events_sent;
        total.max_sprite_batches = total.max_sprite_batches.max(one.max_sprite_batches);
        total.max_sprite_instances = total.max_sprite_instances.max(one.max_sprite_instances);
    }
    total
}

/// One guard: `row` at `bench`'s `preset` and `scale`, its populations
/// checked by `populations` before any frame.
fn guard(
    row: &str,
    (bench, preset, scale): (&str, &str, Option<&str>),
    populations: fn(&World, &BenchConfig),
    drawable: fn(&World) -> u64,
) {
    let bench = knobs::select(BENCHES, Some(bench)).expect("a bench");
    let cfg = knobs::resolve(bench, Some(preset), scale, None).expect("a configuration");
    assert_eq!(cfg.row(), row, "the configuration measures another row");
    let mut harness = harness(&cfg);
    populations(harness.world(), &cfg);
    let reading = measure(&mut harness, cfg.bench.warmup, drawable);
    assert!(
        reading.max_sprite_instances > 0 || cfg.bench.name == "churn",
        "{row}: the guard drew nothing"
    );
    reading.check(manifest_path(&format!("src/tests/guards/{row}.json")));
}

fn knob(cfg: &BenchConfig, name: &str) -> u64 {
    cfg.int(name) as u64
}

#[test]
fn guards_physics() {
    guard(
        "physics",
        ("physics", "min", None),
        |world, cfg| assert_eq!(count::<Ball>(world), knob(cfg, "balls"), "balls"),
        view_drawable,
    );
}

#[test]
fn guards_physics_sparse() {
    guard(
        "physics-sparse",
        ("physics", "sparse-min", None),
        |world, cfg| assert_eq!(count::<Ball>(world), knob(cfg, "bodies"), "bodies"),
        view_drawable,
    );
}

#[test]
fn guards_ecs() {
    guard(
        "ecs",
        ("ecs", "min", None),
        |world, cfg| {
            let entities = knob(cfg, "entities");
            assert_eq!(u64::from(world.entity_count()), entities, "entities");
            assert!(count::<ViewSprite>(world) > 0, "sampled view sprites");
        },
        view_drawable,
    );
}

#[test]
fn guards_churn() {
    guard(
        "churn",
        ("churn", "min", None),
        |world, cfg| assert_eq!(count::<Life>(world), knob(cfg, "population"), "population"),
        view_drawable,
    );
}

#[test]
fn guards_particles() {
    guard(
        "particles",
        ("particles", "min", None),
        |world, cfg| {
            assert_eq!(
                count::<ParticleEmitter>(world),
                knob(cfg, "emitters"),
                "emitters"
            );
            assert_eq!(
                count::<AnimationState>(world),
                knob(cfg, "animated"),
                "animated"
            );
        },
        sprites_drawable,
    );
}

/// `gpu`'s populations: the sprite field and glow, the point lights beside
/// the directional one, and the tilemap when it has layers.
fn gpu_populations(world: &World, cfg: &BenchConfig) {
    let sprites = knob(cfg, "sprites") + knob(cfg, "glow");
    assert_eq!(count::<Sprite>(world), sprites, "field and glow sprites");
    assert_eq!(count::<Light>(world), knob(cfg, "lights") + 1, "lights");
    let maps = u64::from(knob(cfg, "tile_layers") > 0);
    assert_eq!(count::<TilemapInstance>(world), maps, "tilemaps");
}

#[test]
fn guards_gpu() {
    guard("gpu", ("gpu", "min", None), gpu_populations, gpu_drawable);
}

#[test]
fn guards_gpu_throughput() {
    guard(
        "gpu-throughput",
        ("gpu", "throughput", Some("0.01")),
        gpu_populations,
        gpu_drawable,
    );
}

/// `terrain`'s drawable instances: the bodies in view and the art layer's
/// visible tiles.
/// The lean view's drawable quads and the visible tiles of the map's art
/// layer.
fn tiled_drawable(world: &World) -> u64 {
    view_drawable(world) + u64::from(gpu::visible_tiles(world))
}

/// The last frame's edits, refills and chunks traced.
fn terrain_edits(world: &World) -> (u64, u64, u64) {
    let counts = &world.resource::<BenchCounters<TerrainCounts>>().counts;
    (
        u64::from(counts.edits),
        u64::from(counts.refills),
        u64::from(counts.traced),
    )
}

#[test]
fn guards_terrain() {
    let bench = knobs::select(BENCHES, Some("terrain")).expect("a bench");
    let cfg = knobs::resolve(bench, Some("min"), None, None).expect("a configuration");
    assert_eq!(
        cfg.row(),
        "terrain",
        "the configuration measures another row"
    );
    let mut harness = harness(&cfg);
    let world = harness.world();
    assert_eq!(
        count::<terrain::Body>(world),
        knob(&cfg, "bodies"),
        "bodies"
    );
    assert_eq!(
        count::<terrain::Sensor>(world),
        knob(&cfg, "sensors"),
        "sensors"
    );
    assert_eq!(count::<TilemapInstance>(world), 1, "the map");
    // Past the first refills, the warm-up's last frame and the window's
    // break and refill `edits` cells each and trace at most the 8 × `edits`
    // chunks around them.
    CounterGuard::measure(&mut harness, cfg.bench.warmup, 0);
    let edits = knob(&cfg, "edits");
    let check = |(broken, refilled, traced): (u64, u64, u64), when: &str| {
        assert_eq!((broken, refilled), (edits, edits), "{when}: edits");
        assert!(
            traced > 0 && traced <= 8 * edits,
            "{when}: {traced} chunks traced"
        );
    };
    check(terrain_edits(harness.world()), "warm-up");
    let reading = measure(&mut harness, 0, tiled_drawable);
    check(terrain_edits(harness.world()), "window");
    assert!(
        reading.max_sprite_instances > 0,
        "terrain: the guard drew nothing"
    );
    reading.check(manifest_path("src/tests/guards/terrain.json"));
}

/// One `piles` guard: `row` at `preset`, its bodies, paddles and map counted
/// first, and `frame` checking each window frame's counters before the pin.
fn guard_piles(row: &str, preset: &str, paddles: u64, frame: fn(&PileCounts, u32)) {
    let bench = knobs::select(BENCHES, Some("piles")).expect("a bench");
    let cfg = knobs::resolve(bench, Some(preset), None, None).expect("a configuration");
    assert_eq!(cfg.row(), row, "the configuration measures another row");
    let mut harness = harness(&cfg);
    let world = harness.world();
    assert_eq!(count::<piles::Body>(world), knob(&cfg, "bodies"), "bodies");
    assert_eq!(count::<piles::Paddle>(world), paddles, "the paddle");
    assert_eq!(count::<TilemapInstance>(world), 1, "the map");
    let reading = measure_checking(
        &mut harness,
        cfg.bench.warmup,
        tiled_drawable,
        |world, at| {
            frame(&world.resource::<BenchCounters<PileCounts>>().counts, at);
        },
    );
    assert!(
        reading.max_sprite_instances > 0,
        "{row}: the guard drew nothing"
    );
    reading.check(manifest_path(&format!("src/tests/guards/{row}.json")));
}

#[test]
fn guards_friction() {
    // Stirred by the paddle, nothing sleeps and nothing leaves the bin.
    guard_piles("friction", "friction-min", 1, |counts, at| {
        assert_eq!(
            (counts.sleeping, counts.escaped),
            (0, 0),
            "window frame {at}: sleeping and escaped"
        );
        assert!(counts.spinning > 0, "window frame {at}: no circle spins");
    });
}

#[test]
fn guards_slopes() {
    // The stream returns at least 60 bodies a second to the band, and no
    // body's centre is inside a slope cell's polygon.
    guard_piles("slopes", "slopes-min", 0, |counts, at| {
        assert_eq!(counts.inside, 0, "window frame {at}: inside a slope");
        assert!(
            counts.recycled >= 60,
            "window frame {at}: {} returned over the last 60 frames",
            counts.recycled
        );
    });
}
