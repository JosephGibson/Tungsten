//! `ecs`: steady-state query iteration. Fourteen systems, each a different
//! query shape, run over up to 16 components per entity, and zero-sized tags
//! fragment the archetypes. Nothing structural happens after startup, and
//! only the `view` sample carries a `ViewSprite`. Judged on `update` and the
//! per-system rows.
//!
//! `Pos` is the engine's `Position`, so the lean view draws the sample. No
//! engine system reads `Position` unless a benchmark registers one.

use std::f32::consts::TAU;
use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::core::{CameraState, Entity, Pcg32, Position, World};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Guard, Knob, Preset, Row, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

mod systems;

pub(crate) static BENCH: Bench = Bench {
    name: "ecs",
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
    Knob::int("entities", 250_000, 1_000, 4_000_000).scaled(),
    Knob::int("fragmentation", 8, 1, 256).note("Tag-component combinations"),
    Knob::choice("optional", "on", &["on", "off"])
        .note("on: Acc 80%, Regen 40%, Stats 50%, Bag 25%; off: every entity carries all four"),
    Knob::float("followers", 0.1, 0.0, 0.5).note("Share doing a random-access leader lookup"),
    Knob::choice("systems", "all", &["all", "dense", "sparse"])
        .note("dense: whole-population systems only; sparse: optional-component systems only"),
    Knob::int("view", 4_096, 0, 65_536).note("Rendered sample, fixed across scale"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("entities", Value::Int(1_000))],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

const P50: &[&str] = &["p50"];

const ROWS: &[Row] = &[Row {
    name: "ecs",
    preset: "default",
    owned: &[
        ("stage.update", &["p50", "p95"]),
        ("system.brain", P50),
        ("system.cooldowns", P50),
        ("system.buffs", P50),
        ("system.regen", P50),
        ("system.stats_decay", P50),
        ("system.accelerate", P50),
        ("system.follow", P50),
        ("system.integrate", P50),
        ("system.bounds_wrap", P50),
        ("system.heading", P50),
        ("system.tint", P50),
        ("system.age_phase", P50),
        ("system.team_bags", P50),
        ("system.faction_histogram", P50),
    ],
    guards: &[
        Guard::CounterMax {
            counter: "structural",
            max: 0,
        },
        Guard::CounterConst {
            counter: "entities",
        },
    ],
    counters: &["entities", "archetypes", "lookups", "structural", "digest"],
    bottleneck: "update",
    key_knobs: &["entities"],
    note: "",
}];

/// Which `systems` selection runs a system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Matches the whole population.
    Dense,
    /// Matches optional components only.
    Sparse,
}

/// A named system and the selection that runs it.
type System = (&'static str, Reach, fn(&mut World));

/// The 14 systems in run order.
const SYSTEMS: &[System] = &[
    ("brain", Reach::Dense, systems::brain),
    ("cooldowns", Reach::Dense, systems::cooldowns),
    ("buffs", Reach::Sparse, systems::buffs),
    ("regen", Reach::Sparse, systems::regen),
    ("stats_decay", Reach::Sparse, systems::stats_decay),
    ("accelerate", Reach::Sparse, systems::accelerate),
    ("follow", Reach::Sparse, systems::follow),
    ("integrate", Reach::Dense, systems::integrate),
    ("bounds_wrap", Reach::Dense, systems::bounds_wrap),
    ("heading", Reach::Dense, systems::heading),
    ("tint", Reach::Dense, systems::tint),
    ("age_phase", Reach::Dense, systems::age_phase),
    ("team_bags", Reach::Sparse, systems::team_bags),
    (
        "faction_histogram",
        Reach::Dense,
        systems::faction_histogram,
    ),
];

/// `(Acc, Regen, Stats, Bag)` shares under `optional=on`.
const OPTIONAL_SHARES: [f32; 4] = [0.8, 0.4, 0.5, 0.25];
const TEAMS: usize = 4;
const FACTIONS: usize = 8;
const HEALTH_BUCKETS: usize = 4;

const SPEED: (f32, f32) = (20.0, 120.0);
const ACCEL: (f32, f32) = (5.0, 30.0);
const MAX_HEALTH: f32 = 100.0;
const REGEN: (f32, f32) = (1.0, 6.0);
const COOLDOWN_PERIODS: [f32; 4] = [0.5, 1.3, 2.9, 4.7];

const SAMPLE_HALF: f32 = 3.0;
const TEAM_COLORS: [[u8; 4]; TEAMS] = [
    [242, 196, 92, 255],
    [236, 128, 96, 255],
    [120, 186, 240, 255],
    [168, 228, 140, 255],
];

/// `Brain::state` at spawn; `systems` defines the other states.
const WANDER: u8 = 0;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn row(_cfg: &BenchConfig) -> &'static str {
    "ecs"
}

fn sample_size(cfg: &BenchConfig) -> usize {
    cfg.int("view").min(cfg.int("entities")) as usize
}

fn derived(cfg: &BenchConfig) -> Json {
    json!({"world": [VIEWPORT.x, VIEWPORT.y], "view": sample_size(cfg), "zoom": 1.0})
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    {
        let world = app.world_mut();
        let spawned = spawn_population(world, cfg);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(FollowScratch(Vec::with_capacity(spawned.followers)));
        world.insert_resource(Reductions::default());
        let live = world.entity_count();
        world.insert_resource(BenchCounters::new(EcsCounts {
            archetypes: spawned.archetypes,
            live,
            ..EcsCounts::default()
        }));
    }
    app.on_startup(r#gen::register_view_textures);
    app.add_system_named("bench_counters", bench_counters_system::<EcsCounts>);
    let selection = cfg.choice("systems");
    for &(name, reach, system) in SYSTEMS {
        let selected = match selection {
            "dense" => reach == Reach::Dense,
            "sparse" => reach == Reach::Sparse,
            _ => true,
        };
        if selected {
            app.add_system_named(name, system);
        }
    }
    app.add_system_named("ecs_audit", systems::ecs_audit);
    if sample_size(cfg) > 0 {
        app.set_extract_sprites(view::extract_view);
    } else {
        app.set_extract_sprites(|_| Vec::new());
    }
}

#[derive(Debug, Clone, Copy)]
struct Vel(Vec2);
#[derive(Debug, Clone, Copy)]
struct Acc(Vec2);
#[derive(Debug, Clone, Copy)]
struct Heading(f32);
#[derive(Debug, Clone, Copy)]
struct Health(f32);
#[derive(Debug, Clone, Copy)]
struct Regen(f32);
#[derive(Debug, Clone, Copy)]
struct Cooldowns([f32; 4]);
/// `state` is `WANDER`, `DASH`, `REST` or `FLEE`; `seed` drives a per-entity xorshift.
#[derive(Debug, Clone, Copy)]
struct Brain {
    state: u8,
    timer: f32,
    seed: u32,
}
#[derive(Debug, Clone, Copy)]
struct Team(u8);
#[derive(Debug, Clone, Copy)]
struct Faction(u8);
#[derive(Debug, Clone, Copy)]
struct Stats([f32; 8]);
#[derive(Debug, Clone, Copy)]
struct Tint([u8; 4]);
#[derive(Debug, Clone, Copy)]
struct Phase(f32);
#[derive(Debug, Clone, Copy)]
struct Age(f32);
#[derive(Debug, Clone, Copy)]
struct Follow(Entity);
#[derive(Debug, Clone, Copy)]
struct Bag([u16; 16]);
/// Zero-sized archetype fragmenter: bit `N` of an entity's tag combination.
#[derive(Debug, Clone, Copy)]
struct Tag<const N: u8>;

/// Follower steering, reused across frames.
struct FollowScratch(Vec<Vec2>);

/// The two read-only reductions' latest results; the digest folds them in.
#[derive(Debug, Default)]
struct Reductions {
    team_bags: [u64; TEAMS],
    factions: [[u32; HEALTH_BUCKETS]; FACTIONS],
}

struct Spawned {
    archetypes: u32,
    followers: usize,
}

/// Spawns the population, then gives each follower a random leader. Each
/// entity's archetype is its tag combination plus which optional components,
/// `Follow` and `ViewSprite` it carries.
fn spawn_population(world: &mut World, cfg: &BenchConfig) -> Spawned {
    let count = cfg.int("entities") as usize;
    let combinations = cfg.int("fragmentation") as u32;
    let optional = cfg.choice("optional") == "on";
    let follower_share = cfg.float("followers") as f32;
    let sample = sample_size(cfg);
    let mut rng = Pcg32::seeded(cfg.seed());
    let mut entities = Vec::with_capacity(count);
    let mut followers = Vec::new();
    let mut signatures = vec![false; 1 << 14];
    for index in 0..count {
        let combination = rng.next_u32() % combinations;
        let [acc, regen, stats, bag] =
            OPTIONAL_SHARES.map(|share| !optional || rng.next_f32_unit() < share);
        let follower = rng.next_f32_unit() < follower_share;
        let sampled = index < sample;

        let entity = world.spawn();
        insert_tags(world, entity, combination);
        let team = (rng.next_u32() % TEAMS as u32) as u8;
        world.insert(entity, Team(team));
        world.insert(entity, Faction((rng.next_u32() % FACTIONS as u32) as u8));
        world.insert(entity, Tint([255; 4]));
        world.insert(entity, Heading(0.0));
        world.insert(entity, Health(rng.next_range(40.0, MAX_HEALTH)));
        world.insert(entity, Phase(rng.next_range(0.0, TAU)));
        world.insert(entity, Age(0.0));
        let position = Vec2::new(
            rng.next_range(0.0, VIEWPORT.x),
            rng.next_range(0.0, VIEWPORT.y),
        );
        world.insert(entity, Position(position));
        let speed = rng.next_range(SPEED.0, SPEED.1);
        world.insert(entity, Vel(rng.next_unit_vec2() * speed));
        world.insert(
            entity,
            Brain {
                state: WANDER,
                timer: rng.next_range(0.0, 1.0),
                seed: rng.next_u32() | 1,
            },
        );
        let cooldowns = COOLDOWN_PERIODS.map(|period| rng.next_range(0.0, period));
        world.insert(entity, Cooldowns(cooldowns));
        if acc {
            let accel = rng.next_range(ACCEL.0, ACCEL.1);
            world.insert(entity, Acc(rng.next_unit_vec2() * accel));
        }
        if regen {
            world.insert(entity, Regen(rng.next_range(REGEN.0, REGEN.1)));
        }
        if stats {
            world.insert(entity, Stats([1.0; 8]));
        }
        if bag {
            world.insert(
                entity,
                Bag(std::array::from_fn(|slot| ((index + slot) % 17) as u16)),
            );
        }
        if sampled {
            world.insert(
                entity,
                ViewSprite {
                    half: Vec2::splat(SAMPLE_HALF),
                    color: TEAM_COLORS[team as usize],
                    texture: ViewTexture::Disc,
                },
            );
        }
        let signature = combination
            | u32::from(acc) << 8
            | u32::from(regen) << 9
            | u32::from(stats) << 10
            | u32::from(bag) << 11
            | u32::from(follower) << 12
            | u32::from(sampled) << 13;
        signatures[signature as usize] = true;
        entities.push(entity);
        if follower {
            followers.push(index);
        }
    }
    for &index in &followers {
        let mut leader = rng.next_u32() as usize % count;
        if leader == index {
            leader = (leader + 1) % count;
        }
        world.insert(entities[index], Follow(entities[leader]));
    }
    Spawned {
        archetypes: signatures.iter().filter(|&&used| used).count() as u32,
        followers: followers.len(),
    }
}

fn insert_tags(world: &mut World, entity: Entity, combination: u32) {
    macro_rules! tags {
        ($($bit:literal)*) => {
            $(
                if combination & (1 << $bit) != 0 {
                    world.insert(entity, Tag::<$bit>);
                }
            )*
        };
    }
    tags!(0 1 2 3 4 5 6 7);
}

/// FNV-1a over every position's bits in query order, plus the reductions.
fn digest(world: &World) -> u64 {
    let mut hash = FNV_OFFSET;
    let mut mix = |word: u64| hash = (hash ^ word).wrapping_mul(FNV_PRIME);
    for position in world.query::<&Position>() {
        mix(u64::from(position.0.x.to_bits()) | u64::from(position.0.y.to_bits()) << 32);
    }
    if let Some(reductions) = world.get_resource::<Reductions>() {
        reductions.team_bags.into_iter().for_each(&mut mix);
        for bucket in reductions.factions.into_iter().flatten() {
            mix(u64::from(bucket));
        }
    }
    hash
}

/// `bench:` counters. `digest` is a hex token, so the runner parses it as
/// n/a: it stays out of the numeric stats and drift, and still feeds the
/// determinism digest through the line text.
#[derive(Debug, Default)]
struct EcsCounts {
    archetypes: u32,
    /// Live entities at the last audit.
    live: u32,
    lookups: u32,
    structural: u32,
}

impl FrameCounters for EcsCounts {
    fn write(&self, world: &World, line: &mut String) {
        let _ = write!(
            line,
            " entities={} archetypes={} lookups={} structural={} digest={:#018x}",
            world.entity_count(),
            self.archetypes,
            self.lookups,
            self.structural,
            digest(world)
        );
    }

    fn reset(&mut self) {
        self.lookups = 0;
        self.structural = 0;
    }
}
