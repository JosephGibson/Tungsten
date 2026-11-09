//! `rooms`: a room edited live (redo 7.12). It owns the patch path a
//! script's `level` step and a room file's reload share
//! (`World::apply_room_batch`): the batch applied to a copy of the room's
//! document, the room compiled for the batch's check and again for the
//! patch, its objects checked against the prefabs, its cells compared and
//! the changed ones written in place, and the moved objects despawned and
//! spawned again under their names; and, every 120 frames, the room's whole
//! spawn.
//!
//! The room is built in code (`World::insert_room`): a seeded cave of
//! rects, 45° and 2:1 ramps rising right and a one-way bridge line over a
//! grid of 16 px cells, in an art layer and a collision layer, and `bodies`
//! circles from a prefab registered in code. They are dynamic bodies the
//! step never moves: the physics plugin builds into a schedule the app
//! never runs, as `saves`'s does, so their components register and the
//! step stays out of the frame. Each frame's batch moves `moves` objects a
//! cell right and paints `paints` cells of the art layer; the next frame's
//! moves them back and erases the cells, so from the second frame on every
//! frame patches the same work. Every 120 frames the room's instance and
//! objects despawn at once, freeing their names, and the room spawns again.
//! The art layer draws through the engine's tilemap extract and the bodies
//! through the lean view, which a moved body's new spawn takes from the
//! instance by the spawn inheritance.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::core::assets::room::MapPatch;
use tungsten::core::physics::PhysicsPlugin;
use tungsten::core::save::{SaveId, SaveState};
use tungsten::core::{
    CameraState, FilterMode, Pcg32, Plugin, PrefabData, PrefabRegistry, Room, RoomCommand,
    RoomRegistry, Schedule, Tileset, World, splitmix64,
};
use tungsten::{App, extract_tilemaps};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen::{self, Cell, TextureSink};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "rooms",
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
    Knob::int("width", 120, 40, 512).note("Cells across, 16 px each"),
    Knob::int("height", 68, 24, 256).note("Cells down"),
    Knob::int("bodies", 400, 1, 10_000).scaled(),
    Knob::int("moves", 16, 1, 1_000)
        .scaled()
        .note("Objects a frame's batch moves a cell, at most `bodies`"),
    Knob::int("paints", 32, 1, 1_000)
        .scaled()
        .note("Art cells a frame's batch paints, or erases on the next"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("width", Value::Int(40)),
            ("height", Value::Int(24)),
            ("bodies", Value::Int(40)),
            ("moves", Value::Int(4)),
            ("paints", Value::Int(8)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The room's and its tileset's ID.
const ROOM: &str = "bench_room";
const TILE_PX: u32 = 16;
/// The bodies' prefab.
const BALL: &str = "bench_ball";
const RADIUS: f32 = 4.0;
/// Rows of rock along the bottom.
const FLOOR: u32 = 4;
/// Frames between the room's whole spawns.
const RESPAWN_EVERY: u64 = 120;
const ROOM_SALT: u64 = 0x7E44_A1A5_0000_0712;
/// The tiles, in tileset order: the style's brush, its slope tiles, the
/// bridge's plank and the paint.
const TILES: [(&str, [u8; 4]); 6] = [
    ("bench_room_rock", [112, 104, 96, 255]),
    ("bench_room_45", [132, 124, 112, 255]),
    ("bench_room_low", [132, 124, 112, 255]),
    ("bench_room_high", [132, 124, 112, 255]),
    ("bench_room_plank", [168, 120, 72, 255]),
    ("bench_room_moss", [96, 168, 88, 255]),
];
const BODY: ViewSprite = ViewSprite {
    half: Vec2::splat(RADIUS),
    color: [120, 186, 240, 255],
    texture: ViewTexture::Disc,
};

fn row(_cfg: &BenchConfig) -> &'static str {
    "rooms"
}

fn validate(cfg: &BenchConfig) -> anyhow::Result<()> {
    anyhow::ensure!(
        cfg.int("moves") <= cfg.int("bodies"),
        "rooms: `moves` ({}) moves at most every body ({})",
        cfg.int("moves"),
        cfg.int("bodies")
    );
    Ok(())
}

fn derived(cfg: &BenchConfig) -> Json {
    let cave = Cave::new(cfg);
    json!({
        "world": [cave.width * TILE_PX, cave.height * TILE_PX],
        "cells": cave.width * cave.height,
        "shapes": cave.shapes.len(),
        "zoom": (f64::from(zoom(&cave)) * 1e4).round() / 1e4,
    })
}

/// The `bench:` counters: the objects the frame's patch spawned again, the
/// cells it wrote in place, the entities a whole spawn made, and the bodies
/// in view.
#[derive(Debug, Default)]
pub(crate) struct RoomCounts {
    pub(crate) moved: u64,
    pub(crate) painted: u64,
    pub(crate) spawned: u64,
}

impl FrameCounters for RoomCounts {
    fn write(&self, world: &World, line: &mut String) {
        let bounds = view::view_bounds(world);
        let visible = world
            .query::<(&tungsten::core::Position, &ViewSprite)>()
            .filter(|(position, sprite)| view::in_view(bounds, position.0, sprite.half))
            .count();
        let _ = write!(
            line,
            " moved={} painted={} spawned={} visible={visible}",
            self.moved, self.painted, self.spawned
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The run: the frame, the two batches it alternates and the objects'
/// names, which a whole spawn frees.
struct RoomsState {
    frame: u64,
    forth: Arc<Vec<RoomCommand>>,
    back: Arc<Vec<RoomCommand>>,
    names: Vec<SaveId>,
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let cave = Cave::new(cfg);
    {
        let world = app.world_mut();
        PhysicsPlugin.build(&mut Schedule::new(), world);
        world.inherit_from_source::<ViewSprite>();
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            let zoom = zoom(&cave);
            let size = cave.size();
            *camera = CameraState {
                position: size * 0.5 - VIEWPORT / zoom * 0.5,
                zoom,
                rotation: 0.0,
                ..CameraState::new()
            };
        }
        world.insert_resource(cave.state());
        world.insert_resource(BenchCounters::new(RoomCounts::default()));
    }
    app.on_startup(move |world, renderer| {
        r#gen::register_view_textures(world, renderer);
        place_room(world, renderer, &cave);
    });
    app.add_system_named("bench_counters", bench_counters_system::<RoomCounts>);
    app.add_system_named("rooms_edit", rooms_edit);
    app.set_extract_sprites(|world| {
        let mut batches = extract_tilemaps(world);
        batches.extend(view::extract_view(world));
        batches
    });
}

/// The startup hook's World part at `cfg`, for the counter guard, which
/// runs no renderer: the lean view's textures, then the room's.
#[cfg(test)]
pub(crate) fn startup(world: &mut World, textures: &mut dyn TextureSink, cfg: &BenchConfig) {
    r#gen::register_view_textures(world, textures);
    place_room(world, textures, &Cave::new(cfg));
}

/// The zoom that fits the room in the viewport.
fn zoom(cave: &Cave) -> f32 {
    let size = cave.size();
    (VIEWPORT.x / size.x).min(VIEWPORT.y / size.y)
}

/// The tiles on one page, the ball's prefab, the tileset and the room,
/// after manifest composition, which replaces the maps; then its spawn at
/// the origin.
fn place_room(world: &mut World, textures: &mut dyn TextureSink, cave: &Cave) {
    let cells: Vec<Cell> = TILES
        .iter()
        .map(|&(id, color)| Cell {
            id: id.to_owned(),
            rgba: color.repeat((TILE_PX * TILE_PX) as usize),
        })
        .collect();
    r#gen::register_atlas(world, textures, FilterMode::Nearest, TILE_PX, &cells);
    let ball: PrefabData = serde_json::from_value(json!({ "components": { "rigid_body": {
        "kind": "dynamic", "shape": { "kind": "circle", "radius": RADIUS } } } }))
    .expect("the ball's prefab");
    if !world.has_resource::<PrefabRegistry>() {
        world.insert_resource(PrefabRegistry::new());
    }
    world
        .resource_mut::<PrefabRegistry>()
        .insert(BALL.into(), ball);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(ROOM, tileset());
    world.insert_resource(rooms);
    world
        .insert_room(ROOM, cave.room())
        .expect("the bench's room compiles");
    world
        .spawn_room(ROOM, Vec2::ZERO, &(BODY,))
        .expect("the bench's room spawns");
}

/// The frame's batch through the `level` step's path, after the room's
/// whole spawn every 120 frames.
fn rooms_edit(world: &mut World) {
    let Some(state) = world.get_resource_mut::<RoomsState>() else {
        return;
    };
    state.frame += 1;
    let frame = state.frame;
    let batch = if frame % 2 == 1 {
        Arc::clone(&state.forth)
    } else {
        Arc::clone(&state.back)
    };
    let spawned = if frame % RESPAWN_EVERY == 0 {
        spawn_again(world)
    } else {
        0
    };
    let (moved, painted) = match world.apply_room_batch(ROOM, &batch) {
        Ok(patch) => {
            let cells = match patch.map {
                MapPatch::Cells(cells) => cells as u64,
                _ => 0,
            };
            (patch.respawned.len() as u64, cells)
        }
        Err(err) => {
            log::warn!("rooms: {err}");
            (0, 0)
        }
    };
    let counts = &mut world.resource_mut::<BenchCounters<RoomCounts>>().counts;
    counts.moved = moved;
    counts.painted = painted;
    counts.spawned = spawned;
}

/// Despawns the room's instance and objects at once, freeing their names,
/// and spawns it again: the entities the spawn made.
fn spawn_again(world: &mut World) -> u64 {
    let mut dead = Vec::new();
    if let Some(instance) = world
        .get_resource::<RoomRegistry>()
        .and_then(|rooms| rooms.live(ROOM))
    {
        dead.push(instance);
    }
    if let (Some(state), Some(saves)) = (
        world.get_resource::<RoomsState>(),
        world.get_resource::<SaveState>(),
    ) {
        dead.extend(state.names.iter().filter_map(|&id| saves.named_entity(id)));
    }
    for entity in dead {
        world.despawn(entity);
    }
    match world.spawn_room(ROOM, Vec2::ZERO, &(BODY,)) {
        Ok(spawned) => spawned.objects().count() as u64 + 1,
        Err(err) => {
            log::warn!("rooms: {err}");
            0
        }
    }
}

/// The room's tileset: the rock style's brush and slope tiles, the
/// one-way plank and the moss the batches paint.
fn tileset() -> Tileset {
    let [rock, slope_45, low, high, plank, moss] = TILES.map(|(id, _)| id);
    Tileset::from_json(
        &json!({ "version": 1, "tiles": [
            { "sprite": rock, "style": "rock" },
            { "sprite": slope_45, "style": "rock", "slope": "45 br" },
            { "sprite": low, "style": "rock", "slope": "2:1 low br" },
            { "sprite": high, "style": "rock", "slope": "2:1 high br" },
            { "sprite": plank, "one_way": true },
            { "sprite": moss }
        ] })
        .to_string(),
    )
    .expect("the bench's tileset")
}

/// The seeded room: its size, shapes and bodies, the objects a batch moves
/// and the art cells it paints.
struct Cave {
    width: u32,
    height: u32,
    shapes: Vec<Json>,
    bodies: Vec<[f32; 2]>,
    moves: usize,
    paints: Vec<[u32; 2]>,
}

impl Cave {
    fn new(cfg: &BenchConfig) -> Self {
        let (width, height) = (cfg.int("width") as u32, cfg.int("height") as u32);
        let mut rng = Pcg32::seeded(splitmix64(cfg.seed() ^ ROOM_SALT));
        let ground = height - FLOOR;
        let rock = |shape: Json| -> Json {
            let mut shape = shape;
            shape["tile"] = json!("bench_room_rock");
            shape["layers"] = json!(["art", "solid"]);
            shape
        };
        let mut shapes = vec![
            rock(json!({ "id": "floor", "rect": [0, ground, width, FLOOR] })),
            rock(json!({ "id": "wall_left", "rect": [0, 0, 1, ground] })),
            rock(json!({ "id": "wall_right", "rect": [width - 1, 0, 1, ground] })),
        ];
        // A ramp every 20 columns on the floor, 45° and 2:1 in turn, each
        // rising right to its top four rows up.
        for (index, x) in (2..width.saturating_sub(12)).step_by(20).enumerate() {
            let x = x + rng.next_u32() % 4;
            let run = if index % 2 == 0 { 4 } else { 8 };
            shapes.push(rock(json!({
                "id": format!("ramp_{index}"),
                "polygon": [[x, ground], [x + run, ground - 4], [x + run, ground]],
            })));
        }
        // Ledges in the air, a third of the way down and lower.
        for index in 0..width / 12 {
            let len = 4 + rng.next_u32() % 7;
            let x = 2 + rng.next_u32() % (width - len - 3);
            let y = height / 4 + rng.next_u32() % (ground - 6 - height / 4);
            shapes.push(rock(
                json!({ "id": format!("ledge_{index}"), "rect": [x, y, len, 1] }),
            ));
        }
        let (left, right) = (width / 3, 2 * width / 3);
        shapes.push(json!({
            "id": "bridge", "line": [[left, height / 3], [right, height / 3]],
            "tile": "bench_room_plank", "layers": ["art", "solid"],
        }));
        let bodies = (0..cfg.int("bodies"))
            .map(|_| {
                [
                    1.5 + rng.next_f32_unit() * (width - 3) as f32,
                    1.5 + rng.next_f32_unit() * (ground - 3) as f32,
                ]
            })
            .collect();
        let mut picked = BTreeSet::new();
        while picked.len() < cfg.int("paints") as usize {
            let col = 1 + rng.next_u32() % (width - 2);
            let row = 1 + rng.next_u32() % (ground - 1);
            picked.insert([col, row]);
        }
        Self {
            width,
            height,
            shapes,
            bodies,
            moves: cfg.int("moves") as usize,
            paints: picked.into_iter().collect(),
        }
    }

    fn size(&self) -> Vec2 {
        Vec2::new(
            (self.width * TILE_PX) as f32,
            (self.height * TILE_PX) as f32,
        )
    }

    fn room(&self) -> Room {
        let objects: Vec<Json> = self
            .bodies
            .iter()
            .enumerate()
            .map(|(index, at)| json!({ "id": format!("ball_{index}"), "prefab": BALL, "at": at }))
            .collect();
        Room::from_json(
            &json!({
                "version": 1, "cell": [TILE_PX, TILE_PX], "size": [self.width, self.height],
                "tileset": ROOM,
                "layers": [{ "name": "art" }, { "name": "solid", "kind": "collision" }],
                "shapes": self.shapes,
                "objects": objects,
            })
            .to_string(),
        )
        .expect("the bench's room")
    }

    /// The run's two batches, the first `moves` bodies a cell right and the
    /// cells painted, then back and erased, and the bodies' names.
    fn state(&self) -> RoomsState {
        let batch = |by: i32, paint: Json| -> Vec<RoomCommand> {
            let mut commands: Vec<Json> = (0..self.moves)
                .map(|index| json!({ "move": { "id": format!("ball_{index}"), "by": [by, 0] } }))
                .collect();
            commands.push(paint);
            RoomCommand::parse_batch(&Json::Array(commands).to_string()).expect("the bench's batch")
        };
        let cells = json!(self.paints);
        RoomsState {
            frame: 0,
            forth: Arc::new(batch(
                1,
                json!({ "paint": { "layer": "art", "cells": cells, "tile": "bench_room_moss" } }),
            )),
            back: Arc::new(batch(
                -1,
                json!({ "erase": { "layer": "art", "cells": cells } }),
            )),
            names: (0..self.bodies.len())
                .map(|index| SaveId::authored(&format!("{ROOM}.ball_{index}")))
                .collect(),
        }
    }
}
