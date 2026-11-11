//! `generate`: room generation's churn (redo 7.12, P7-16). It owns the path
//! a generated run takes through the engine: `World::generate_rooms`
//! running a generator registered in code over a template room, its rooms
//! compiled and registered with their generation, each spawned at its
//! origin with its objects named; and, the frame after, every room's
//! instance and objects despawned and the rooms removed with their maps and
//! areas (`World::remove_room`) before the next seed's run.
//!
//! The template is built in code (`World::insert_room`): a 24 × 14 hall of
//! 16 px cells, a floor two rows deep and a wall at its left, in an art
//! layer and a collision layer, holding one circle from a prefab registered
//! in code. The generator adds `rooms` copies of it side by side, SF22's
//! chain, each with `objects` circles at cells its seed's stream picks.
//! They are dynamic bodies the step never moves: the physics plugin builds
//! into a schedule the app never runs, as `rooms`'s does. Each frame
//! despawns the last run, removes its rooms, generates the next seed's and
//! spawns them, so a frame pays a run's whole churn; runs alternate two
//! prefixes, so a removal that left a run's rooms or areas behind would
//! show. The art layer draws through the engine's tilemap extract and the
//! bodies through the lean view.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::core::physics::PhysicsPlugin;
use tungsten::core::{
    CameraState, Entity, FilterMode, Generate, Plugin, PrefabData, PrefabRegistry, Room,
    RoomGenerator, RoomRegistry, Schedule, Tileset, World, splitmix64,
};
use tungsten::{App, extract_tilemaps};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen::{self, Cell, TextureSink};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "generate",
    workload_version: 1,
    warmup: 60,
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
    Knob::int("rooms", 8, 1, 64).note("Rooms a generation adds side by side, SF22's chain"),
    Knob::int("objects", 24, 1, 1_000)
        .scaled()
        .note("Circles each room holds, at cells its seed picks"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("rooms", Value::Int(2)), ("objects", Value::Int(4))],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The template room's and its tileset's ID.
pub(crate) const TEMPLATE: &str = "bench_hall";
/// The generator's ID.
pub(crate) const GENERATOR: &str = "bench_chain";
/// The prefixes runs alternate, odd frames' first.
pub(crate) const PREFIXES: [&str; 2] = ["run_a", "run_b"];
const TILE_PX: u32 = 16;
const COLS: u32 = 24;
const ROWS: u32 = 14;
/// Rows of rock along the bottom.
const FLOOR: u32 = 2;
/// The circles' prefab.
const BALL: &str = "bench_hall_ball";
const RADIUS: f32 = 4.0;
const SEED_SALT: u64 = 0x6E4E_7A7E_0000_0712;
const ROCK: (&str, [u8; 4]) = ("bench_hall_rock", [112, 104, 96, 255]);
const BODY: ViewSprite = ViewSprite {
    half: Vec2::splat(RADIUS),
    color: [232, 176, 96, 255],
    texture: ViewTexture::Disc,
};

fn row(_cfg: &BenchConfig) -> &'static str {
    "generate"
}

fn derived(cfg: &BenchConfig) -> Json {
    let rooms = cfg.int("rooms") as u32;
    json!({
        "world": [rooms * COLS * TILE_PX, ROWS * TILE_PX],
        "bodies": i64::from(rooms) * cfg.int("objects"),
        "zoom": (f64::from(zoom(rooms)) * 1e4).round() / 1e4,
    })
}

/// The `bench:` counters: the rooms the frame generated and spawned, the
/// entities their spawns made, the rooms it removed, and the bodies in view.
#[derive(Debug, Default)]
pub(crate) struct GenerateCounts {
    pub(crate) rooms: u64,
    pub(crate) spawned: u64,
    pub(crate) removed: u64,
}

impl FrameCounters for GenerateCounts {
    fn write(&self, world: &World, line: &mut String) {
        let bounds = view::view_bounds(world);
        let visible = world
            .query::<(&tungsten::core::Position, &ViewSprite)>()
            .filter(|(position, sprite)| view::in_view(bounds, position.0, sprite.half))
            .count();
        let _ = write!(
            line,
            " rooms={} spawned={} removed={} visible={visible}",
            self.rooms, self.spawned, self.removed
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The run: the frame, the seeds' base, and the live run's rooms and the
/// entities their spawns made.
pub(crate) struct GenerateState {
    frame: u64,
    seed: u64,
    pub(crate) live: Vec<String>,
    spawned: Vec<Entity>,
}

/// `rooms` copies of the template side by side, each with `objects`
/// circles at cells from the generation's stream.
struct Chain {
    rooms: usize,
    objects: usize,
}

impl RoomGenerator for Chain {
    fn version(&self) -> u32 {
        1
    }

    fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
        let template = generation.template(TEMPLATE)?;
        let width = (COLS * TILE_PX) as f32;
        for at in 0..self.rooms {
            let mut room = template.clone();
            let ball = room.objects.pop().ok_or("the template holds its circle")?;
            for index in 0..self.objects {
                let mut object = ball.clone();
                object.id = format!("ball_{index}");
                let rng = generation.rng();
                object.at = [
                    1.5 + rng.next_f32_unit() * (COLS - 3) as f32,
                    1.5 + rng.next_f32_unit() * (ROWS - FLOOR - 3) as f32,
                ];
                room.objects.push(object);
            }
            generation.add(&format!("r{at}"), room, [at as f32 * width, 0.0])?;
        }
        Ok(())
    }
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let rooms = cfg.int("rooms") as u32;
    app.register_generator(
        GENERATOR,
        Chain {
            rooms: rooms as usize,
            objects: cfg.int("objects") as usize,
        },
    );
    {
        let world = app.world_mut();
        PhysicsPlugin.build(&mut Schedule::new(), world);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            let zoom = zoom(rooms);
            let size = Vec2::new((rooms * COLS * TILE_PX) as f32, (ROWS * TILE_PX) as f32);
            *camera = CameraState {
                position: size * 0.5 - VIEWPORT / zoom * 0.5,
                zoom,
                rotation: 0.0,
                ..CameraState::new()
            };
        }
        world.insert_resource(GenerateState {
            frame: 0,
            seed: splitmix64(cfg.seed() ^ SEED_SALT),
            live: Vec::new(),
            spawned: Vec::new(),
        });
        world.insert_resource(BenchCounters::new(GenerateCounts::default()));
    }
    app.on_startup(|world, renderer| {
        r#gen::register_view_textures(world, renderer);
        place_template(world, renderer);
    });
    app.add_system_named("bench_counters", bench_counters_system::<GenerateCounts>);
    app.add_system_named("generate_churn", generate_churn);
    app.set_extract_sprites(|world| {
        let mut batches = extract_tilemaps(world);
        batches.extend(view::extract_view(world));
        batches
    });
}

/// The startup hook's World part, for the counter guard, which runs no
/// renderer: the lean view's textures, then the template's.
#[cfg(test)]
pub(crate) fn startup(world: &mut World, textures: &mut dyn TextureSink) {
    r#gen::register_view_textures(world, textures);
    place_template(world, textures);
}

/// The zoom that fits `rooms` side by side in the viewport.
fn zoom(rooms: u32) -> f32 {
    let size = Vec2::new((rooms * COLS * TILE_PX) as f32, (ROWS * TILE_PX) as f32);
    (VIEWPORT.x / size.x).min(VIEWPORT.y / size.y)
}

/// The rock tile on one page, the circle's prefab, the tileset and the
/// template, after manifest composition, which replaces the maps.
fn place_template(world: &mut World, textures: &mut dyn TextureSink) {
    let (rock, color) = ROCK;
    let cells = [Cell {
        id: rock.to_owned(),
        rgba: color.repeat((TILE_PX * TILE_PX) as usize),
    }];
    r#gen::register_atlas(world, textures, FilterMode::Nearest, TILE_PX, &cells);
    let ball: PrefabData = serde_json::from_value(json!({ "components": { "rigid_body": {
        "kind": "dynamic", "shape": { "kind": "circle", "radius": RADIUS } } } }))
    .expect("the circle's prefab");
    if !world.has_resource::<PrefabRegistry>() {
        world.insert_resource(PrefabRegistry::new());
    }
    world
        .resource_mut::<PrefabRegistry>()
        .insert(BALL.into(), ball);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(
        TEMPLATE,
        Tileset::from_json(&json!({ "version": 1, "tiles": [{ "sprite": rock }] }).to_string())
            .expect("the bench's tileset"),
    );
    world.insert_resource(rooms);
    world
        .insert_room(TEMPLATE, template())
        .expect("the bench's template compiles");
}

/// The hall a generation copies: its floor, its wall and one circle.
fn template() -> Room {
    let (rock, _) = ROCK;
    let ground = ROWS - FLOOR;
    Room::from_json(
        &json!({
            "version": 1, "cell": [TILE_PX, TILE_PX], "size": [COLS, ROWS], "tileset": TEMPLATE,
            "layers": [{ "name": "art" }, { "name": "solid", "kind": "collision" }],
            "shapes": [
                { "id": "floor", "rect": [0, ground, COLS, FLOOR], "tile": rock, "layers": ["art", "solid"] },
                { "id": "wall", "rect": [0, 0, 1, ground], "tile": rock, "layers": ["art", "solid"] }
            ],
            "objects": [{ "id": "ball", "prefab": BALL, "at": [2, 2] }],
        })
        .to_string(),
    )
    .expect("the bench's template")
}

/// The frame's churn: the last run despawned and its rooms removed, then
/// the next seed's run generated under the other prefix and spawned.
fn generate_churn(world: &mut World) {
    let Some(state) = world.get_resource_mut::<GenerateState>() else {
        return;
    };
    state.frame += 1;
    let frame = state.frame;
    let seed = state.seed.wrapping_add(frame);
    let mut spawned = std::mem::take(&mut state.spawned);
    let mut live = std::mem::take(&mut state.live);
    for entity in spawned.drain(..) {
        world.despawn(entity);
    }
    let mut removed = 0;
    for room in live.drain(..) {
        match world.remove_room(&room) {
            Ok(_) => removed += 1,
            Err(err) => log::warn!("generate: {err}"),
        }
    }
    let prefix = PREFIXES[usize::from(frame % 2 == 0)];
    match world.generate_rooms(GENERATOR, seed, prefix) {
        Ok(generated) => {
            for (id, origin) in generated.rooms() {
                match world.spawn_room(id, origin, &(BODY,)) {
                    Ok(room) => {
                        spawned.push(room.instance());
                        spawned.extend(room.objects().map(|(_, entity)| entity));
                        live.push(id.to_owned());
                    }
                    Err(err) => log::warn!("generate: {err}"),
                }
            }
        }
        Err(err) => log::warn!("generate: {err}"),
    }
    let counts = &mut world.resource_mut::<BenchCounters<GenerateCounts>>().counts;
    counts.rooms = live.len() as u64;
    counts.spawned = spawned.len() as u64;
    counts.removed = removed;
    let state = world.resource_mut::<GenerateState>();
    state.live = live;
    state.spawned = spawned;
}
