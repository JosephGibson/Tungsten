//! `churn`: structural change. A steady population is replaced FIFO, each
//! spawn inserts `components` components one archetype move at a time, and a
//! share of the survivors gains a status component that it loses the next
//! frame.
//! `deferred` records everything through the `CommandBuffer` (D-039), which
//! `flush` applies; `immediate` makes the same calls on `World` inside the
//! churn systems, so the two modes differ only in the command-buffer
//! overhead. Judged on `flush` and the churn system rows; RSS growth is
//! reported only.
//!
//! Targets follow spawn serials. With population P and S replacements per
//! frame, frame f despawns serials `[f*S, (f+1)*S)` and spawns
//! `[P + f*S, P + (f+1)*S)`, so every entity lives P / S frames (about
//! `1 / turnover`) and the population never changes. The startup population
//! holds serials `0..P`, pre-aged so churn is steady from the first frame.
//! Survivors whose serial is `f` modulo `groups` gain a status at frame f and
//! lose it at f + 1; gains skip entities that expire at f + 1, so losses
//! equal the previous frame's gains. A deferred spawn has no `Entity` until
//! flush, so both modes find their targets with one scan over `Life`.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::core::{CameraState, CommandBuffer, Entity, PendingEntity, Position, World};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Guard, Knob, Preset, Row, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "churn",
    workload_version: 1,
    warmup: 60,
    gpu_timing: false,
    knobs: KNOBS,
    presets: PRESETS,
    rows: ROWS,
    row,
    validate: knobs::no_cross_checks,
    derived,
    engine_config: knobs::keep_engine_config,
    configure,
};

const KNOBS: &[Knob] = &[
    Knob::int("population", 125_000, 1_000, 2_000_000).scaled(),
    Knob::float("turnover", 0.05, 0.0, 0.5).note("Share replaced per frame, FIFO"),
    Knob::float("toggles", 0.05, 0.0, 0.5)
        .note("Share gaining or losing a status per frame; a status lasts one frame"),
    Knob::int("components", 6, 2, 12).note("Components per spawn, one archetype move each"),
    Knob::int("statuses", 3, 1, 8).note("Distinct status components"),
    Knob::choice("mode", "deferred", &["deferred", "immediate"]).note(
        "deferred: CommandBuffer, applied in flush; immediate: World calls in the churn systems",
    ),
    Knob::int("view", 0, 0, 16_384).note("Rendered sample"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("population", Value::Int(1_000))],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

const P50: &[&str] = &["p50"];

const ROWS: &[Row] = &[Row {
    name: "churn",
    preset: "default",
    owned: &[
        ("stage.flush", &["p50", "p95"]),
        ("system.churn_scan", P50),
        ("system.churn_despawn", P50),
        ("system.churn_toggle", P50),
        ("system.churn_spawn", P50),
    ],
    guards: &[
        Guard::CounterConst {
            counter: "population",
        },
        Guard::CounterEq {
            counter: "spawned",
            other: "despawned",
        },
    ],
    counters: &[
        "population",
        "spawned",
        "despawned",
        "inserted",
        "removed",
        "commands",
    ],
    bottleneck: "stage.flush",
    key_knobs: &["population"],
    note: "",
}];

const STATUS_VALUE: f32 = 1.0;
const SAMPLE_COLORS: [[u8; 4]; 2] = [[120, 186, 240, 255], [242, 196, 92, 255]];

fn row(_cfg: &BenchConfig) -> &'static str {
    "churn"
}

/// Replacements per frame: `population * turnover`, rounded.
fn per_frame(cfg: &BenchConfig) -> u64 {
    (cfg.int("population") as f64 * cfg.float("turnover")).round() as u64
}

/// Status groups: survivors whose serial is the frame index modulo this gain
/// a status, so gains plus next-frame losses make about `toggles` of the
/// population per frame. Only entities that also survive the next frame are
/// eligible. 0 disables toggles.
fn toggle_groups(cfg: &BenchConfig) -> u64 {
    let population = cfg.int("population") as u64;
    let eligible = population.saturating_sub(2 * per_frame(cfg));
    let events = population as f64 * cfg.float("toggles");
    if eligible == 0 || events < 1.0 {
        return 0;
    }
    ((2.0 * eligible as f64 / events).round() as u64).max(2)
}

fn derived(cfg: &BenchConfig) -> Json {
    let population = cfg.int("population") as u64;
    let per_frame = per_frame(cfg);
    let lifetime = (per_frame > 0).then(|| population as f64 / per_frame as f64);
    let groups = toggle_groups(cfg);
    let gains = population
        .saturating_sub(2 * per_frame)
        .checked_div(groups)
        .unwrap_or(0);
    let sample = Sample::new(population, cfg.int("view") as u64);
    json!({
        "per_frame": per_frame,
        "lifetime_frames": lifetime,
        "toggle_groups": groups,
        "gains_per_frame": gains,
        "view": sample.count,
    })
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let population = cfg.int("population") as u64;
    let state = Churn {
        population,
        per_frame: per_frame(cfg),
        groups: toggle_groups(cfg),
        parts: (cfg.int("components") - 1) as u8,
        statuses: cfg.int("statuses") as u8,
        immediate: cfg.choice("mode") == "immediate",
        sample: Sample::new(population, cfg.int("view") as u64),
        frame: 0,
        despawns: Vec::new(),
        toggled: Vec::new(),
    };
    {
        let world = app.world_mut();
        for serial in 0..population {
            spawn_one(world, &state, serial);
        }
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(BenchCounters::new(ChurnCounts::default()));
    }
    let drawn = state.sample.count > 0;
    app.world_mut().insert_resource(state);
    app.on_startup(r#gen::register_view_textures);
    app.add_system_named("bench_counters", bench_counters_system::<ChurnCounts>);
    app.add_system_named("churn_scan", churn_scan);
    app.add_system_named("churn_despawn", churn_despawn);
    app.add_system_named("churn_toggle", churn_toggle);
    app.add_system_named("churn_spawn", churn_spawn);
    if drawn {
        app.set_extract_sprites(view::extract_view);
    } else {
        app.set_extract_sprites(|_| Vec::new());
    }
}

/// Spawn serial, its toggle group (`serial % groups`), and one bit per status
/// component the entity holds.
#[derive(Debug, Clone, Copy)]
struct Life {
    serial: u64,
    group: u64,
    status: u8,
}

/// Spawn payload; each `K` is a distinct component type.
#[allow(dead_code)] // Moved by every archetype change, never read.
#[derive(Debug, Clone, Copy)]
struct Part<const K: u8>([f32; 4]);

/// Toggled status; each `K` is a distinct component type.
#[allow(dead_code)] // Moved by every archetype change, never read.
#[derive(Debug, Clone, Copy)]
struct Status<const K: u8>(f32);

/// The rendered sample: every `stride`-th FIFO slot, up to `count`, on a
/// grid over the viewport. A replacement reuses its slot's cell.
#[derive(Debug, Clone, Copy)]
struct Sample {
    stride: u64,
    count: u64,
    columns: u64,
    cell: f32,
}

impl Sample {
    fn new(population: u64, view: u64) -> Self {
        let count = view.min(population);
        if count == 0 {
            return Self {
                stride: 1,
                count: 0,
                columns: 1,
                cell: 0.0,
            };
        }
        let columns = ((count as f32 * VIEWPORT.x / VIEWPORT.y).sqrt().ceil() as u64).max(1);
        let rows = count.div_ceil(columns);
        Self {
            stride: population / count,
            count,
            columns,
            cell: (VIEWPORT.x / columns as f32).min(VIEWPORT.y / rows as f32),
        }
    }

    fn position(&self, slot: u64) -> Option<Vec2> {
        if !slot.is_multiple_of(self.stride) || slot / self.stride >= self.count {
            return None;
        }
        let index = slot / self.stride;
        Some(Vec2::new(
            ((index % self.columns) as f32 + 0.5) * self.cell,
            ((index / self.columns) as f32 + 0.5) * self.cell,
        ))
    }
}

/// Runtime state; `despawns` and `toggled` are this frame's scan results.
struct Churn {
    population: u64,
    per_frame: u64,
    /// See [`toggle_groups`].
    groups: u64,
    /// Components per spawn besides `Life`.
    parts: u8,
    statuses: u8,
    immediate: bool,
    sample: Sample,
    frame: u64,
    despawns: Vec<Entity>,
    /// `(entity, status, gains)`.
    toggled: Vec<(Entity, u8, bool)>,
}

/// The structural operations, applied now (`World`) or recorded for flush
/// (`CommandBuffer`).
trait Structural {
    type Spawned: Copy;
    fn spawn(&mut self) -> Self::Spawned;
    fn insert_new<T: 'static>(&mut self, entity: Self::Spawned, component: T);
    fn insert<T: 'static>(&mut self, entity: Entity, component: T);
    fn remove<T: 'static>(&mut self, entity: Entity);
    fn despawn(&mut self, entity: Entity);
}

impl Structural for World {
    type Spawned = Entity;

    fn spawn(&mut self) -> Entity {
        World::spawn(self)
    }

    fn insert_new<T: 'static>(&mut self, entity: Entity, component: T) {
        World::insert(self, entity, component);
    }

    fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        World::insert(self, entity, component);
    }

    fn remove<T: 'static>(&mut self, entity: Entity) {
        World::remove_component::<T>(self, entity);
    }

    fn despawn(&mut self, entity: Entity) {
        World::despawn(self, entity);
    }
}

impl Structural for CommandBuffer {
    type Spawned = PendingEntity;

    fn spawn(&mut self) -> PendingEntity {
        CommandBuffer::spawn(self)
    }

    fn insert_new<T: 'static>(&mut self, entity: PendingEntity, component: T) {
        self.insert_pending(entity, component);
    }

    fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        CommandBuffer::insert(self, entity, component);
    }

    fn remove<T: 'static>(&mut self, entity: Entity) {
        self.remove_component::<T>(entity);
    }

    fn despawn(&mut self, entity: Entity) {
        CommandBuffer::despawn(self, entity);
    }
}

fn command_buffer(world: &mut World) -> &mut CommandBuffer {
    world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing")
}

/// Spawns serial `serial` with `Life`, its parts and, in the sample, a
/// `Position` and `ViewSprite`. Returns the components inserted.
fn spawn_one<S: Structural>(ops: &mut S, state: &Churn, serial: u64) -> u32 {
    let entity = ops.spawn();
    let group = serial.checked_rem(state.groups).unwrap_or(0);
    ops.insert_new(
        entity,
        Life {
            serial,
            group,
            status: 0,
        },
    );
    let parts = state.parts;
    let value = serial as f32;
    macro_rules! parts {
        ($($k:literal)*) => {
            $(
                if parts > $k {
                    ops.insert_new(entity, Part::<$k>([value; 4]));
                }
            )*
        };
    }
    parts!(0 1 2 3 4 5 6 7 8 9 10);
    let mut inserted = 1 + u32::from(parts);
    if let Some(position) = state.sample.position(serial % state.population) {
        ops.insert_new(entity, Position(position));
        ops.insert_new(
            entity,
            ViewSprite {
                half: Vec2::splat(state.sample.cell * 0.35),
                color: SAMPLE_COLORS[(serial / state.population % 2) as usize],
                texture: ViewTexture::Rect,
            },
        );
        inserted += 2;
    }
    inserted
}

fn toggle<S: Structural>(ops: &mut S, entity: Entity, status: u8, gains: bool) {
    macro_rules! statuses {
        ($($k:literal)*) => {
            match status {
                $(
                    $k => if gains {
                        ops.insert(entity, Status::<$k>(STATUS_VALUE));
                    } else {
                        ops.remove::<Status<$k>>(entity);
                    },
                )*
                _ => unreachable!("status {status} out of range"),
            }
        };
    }
    statuses!(0 1 2 3 4 5 6 7);
}

fn with_counts(world: &mut World, update: impl FnOnce(&mut ChurnCounts)) {
    if let Some(counters) = world.get_resource_mut::<BenchCounters<ChurnCounts>>() {
        update(&mut counters.counts);
    }
}

/// Finds this frame's targets: the S oldest serials expire, last frame's
/// status holders lose their status, and this frame's group gains one. The
/// `Life` bits change now; the components follow in the toggle system or at
/// flush.
fn churn_scan(world: &mut World) {
    let Some(mut state) = world.remove_resource::<Churn>() else {
        return;
    };
    state.despawns.clear();
    state.toggled.clear();
    let expire_before = (state.frame + 1) * state.per_frame;
    let gain_from = expire_before + state.per_frame;
    let group = state.frame % state.groups.max(1);
    let statuses = u64::from(state.statuses);
    for (entity, life) in world.query_mut::<Life>() {
        if life.serial < expire_before {
            state.despawns.push(entity);
        } else if life.status != 0 {
            state
                .toggled
                .push((entity, life.status.trailing_zeros() as u8, false));
            life.status = 0;
        } else if state.groups > 0 && life.group == group && life.serial >= gain_from {
            let status = (life.serial / state.groups % statuses) as u8;
            state.toggled.push((entity, status, true));
            life.status = 1 << status;
        }
    }
    world.insert_resource(state);
}

fn churn_despawn(world: &mut World) {
    let Some(state) = world.remove_resource::<Churn>() else {
        return;
    };
    if state.immediate {
        for &entity in &state.despawns {
            Structural::despawn(world, entity);
        }
    } else {
        let buffer = command_buffer(world);
        for &entity in &state.despawns {
            Structural::despawn(buffer, entity);
        }
    }
    let despawned = state.despawns.len() as u32;
    world.insert_resource(state);
    with_counts(world, |counts| counts.despawned += despawned);
}

fn churn_toggle(world: &mut World) {
    let Some(state) = world.remove_resource::<Churn>() else {
        return;
    };
    if state.immediate {
        for &(entity, status, gains) in &state.toggled {
            toggle(world, entity, status, gains);
        }
    } else {
        let buffer = command_buffer(world);
        for &(entity, status, gains) in &state.toggled {
            toggle(buffer, entity, status, gains);
        }
    }
    let gained = state.toggled.iter().filter(|toggle| toggle.2).count() as u32;
    let lost = state.toggled.len() as u32 - gained;
    world.insert_resource(state);
    with_counts(world, |counts| {
        counts.inserted += gained;
        counts.removed += lost;
    });
}

/// Last churn system: spawns this frame's serials and, in deferred mode,
/// reads the frame's command count.
fn churn_spawn(world: &mut World) {
    let Some(mut state) = world.remove_resource::<Churn>() else {
        return;
    };
    let first = state.population + state.frame * state.per_frame;
    let mut inserted = 0;
    let mut commands = 0;
    if state.immediate {
        for serial in first..first + state.per_frame {
            inserted += spawn_one(world, &state, serial);
        }
    } else {
        let buffer = command_buffer(world);
        for serial in first..first + state.per_frame {
            inserted += spawn_one(buffer, &state, serial);
        }
        commands = buffer.len() as u32;
    }
    let spawned = state.per_frame as u32;
    state.frame += 1;
    world.insert_resource(state);
    with_counts(world, |counts| {
        counts.spawned += spawned;
        counts.inserted += inserted;
        counts.commands += commands;
    });
}

/// `bench:` counters; `population` is the live entity count after flush.
#[derive(Debug, Default)]
struct ChurnCounts {
    spawned: u32,
    despawned: u32,
    inserted: u32,
    removed: u32,
    commands: u32,
}

impl FrameCounters for ChurnCounts {
    fn write(&self, world: &World, line: &mut String) {
        let _ = write!(
            line,
            " population={} spawned={} despawned={} inserted={} removed={} commands={}",
            world.entity_count(),
            self.spawned,
            self.despawned,
            self.inserted,
            self.removed,
            self.commands
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}
