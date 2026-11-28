//! `items`: the items over swapping holders and a field of ground items
//! (8.1's second plan). `holders` inventories of 24 cells and 4 worn
//! slots, each slot's spare in the bag, swap one worn item for its spare
//! every `period` frames, staggered and in turn over the slots, through
//! `move_item` in the bench's own system in `Phase::Intent`: one equipment
//! status ended and one applied, its stat lines moving between them.
//! `ground` labelled ground items lie on a square grid; `collectors` walk
//! seeded closed paths over it, and each unit a collector's bag took is
//! dropped back by `drop_item` at the next frame's start, in `PreUpdate`
//! before the fixed steps, so the ground and the bags keep every unit and
//! each frame labels as many ground items however many steps it runs (a
//! collected one lies, labelled, until the frame's command flush). Judged
//! on `pickup_tick`,
//! `ground_item_labels`, `effect_tick` and `stats_apply`. The kit's tags,
//! stats, counters, cues, effects, abilities, hit, pickup and items plugins
//! run in the app's own schedule with their phases; no `KitPlugins::set()`.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::core::{
    CameraState, DataRegistry, Entity, Pcg32, PluginSet, Position, PrefabData, PrefabRegistry,
    Stage, World, WorldLabels, splitmix64, system,
};
use tungsten::kit::{
    AbilitiesPlugin, CountersPlugin, CuesPlugin, Effects, EffectsPlugin, GROUND_ITEM_LABELS,
    GroundItem, HitPlugin, HitShape, Hurtbox, Inventory, ItemBase, ItemPlace, ItemsPlugin, Phase,
    PickupPlugin, Stats, StatsPlugin, Tags, TagsPlugin, drop_item, give, move_item,
};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "items",
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
    Knob::int("holders", 64, 1, 4_096)
        .scaled()
        .note("Inventories of 24 cells and 4 worn slots, each slot's spare in the bag"),
    Knob::int("ground", 400, 1, 20_000)
        .scaled()
        .note("Labelled ground items on a square grid 24 px apart"),
    Knob::int("collectors", 8, 1, 64)
        .scaled()
        .note("Collectors on seeded closed paths over the ground"),
    Knob::int("period", 10, 1, 600).note("Frames between a holder's swaps"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("holders", Value::Int(2)),
            ("ground", Value::Int(20)),
            ("collectors", Value::Int(1)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The worn slots, each its own kind.
const SLOTS: [&str; 4] = ["hood", "charm", "ring", "belt"];
/// The game stats the worn items' lines raise.
const STATS: [&str; 2] = ["warmth", "luck"];
/// The tag a pickup's filter asks of its collector.
const COLLECTOR_TAG: &str = "team.player";
/// The ground items' base and its `ground` prefab.
const FLASK: &str = "flask";
const DROP: &str = "flask_drop";
/// A bag's cells.
const CELLS: u32 = 24;
/// The grid's spacing in px.
const SPACING: f32 = 24.0;
/// The bench's systems.
const SWAP: &str = "items_swap";
const WALK: &str = "items_walk";
const RETURN: &str = "items_return";
const TALLY: &str = "items_tally";
/// Seeds the collectors' paths.
const PATH_SALT: u64 = 0x17e3_0000_0000_0001;

fn row(_cfg: &BenchConfig) -> &'static str {
    "items"
}

/// The grid's columns for `ground` items: the square's side.
fn columns(ground: i64) -> i64 {
    (ground as f64).sqrt().ceil().max(1.0) as i64
}

/// The collectors' area, `(min, max)`: the grid's box.
fn walk_area(ground: i64) -> (Vec2, Vec2) {
    let columns = columns(ground);
    let rows = (ground + columns - 1) / columns;
    let min = Vec2::splat(SPACING);
    let max = min + Vec2::new((columns - 1) as f32, (rows - 1).max(0) as f32) * SPACING;
    (min, max)
}

fn derived(cfg: &BenchConfig) -> Json {
    let ground = cfg.int("ground");
    let columns = columns(ground);
    let (min, max) = walk_area(ground);
    json!({
        "world": [VIEWPORT.x, VIEWPORT.y],
        "columns": columns,
        "rows": (ground + columns - 1) / columns,
        "walk": [min.x, min.y, max.x, max.y],
        "slots": SLOTS.len(),
        "cells": CELLS,
    })
}

/// What the bench's systems counted: swaps and units taken so far, and
/// the labels the last frame pushed, read before the extract drains them.
#[derive(Debug, Default)]
pub(crate) struct ItemsTally {
    swaps: u64,
    taken: u64,
    labels: usize,
}

/// The `bench:` counters, read from the frame's final state: the
/// populations, the items worn, the swaps, the ground items and their
/// labels, the units the collectors' bags hold and the units taken.
#[derive(Debug, Default)]
pub(crate) struct ItemsCounts;

impl FrameCounters for ItemsCounts {
    fn write(&self, world: &World, line: &mut String) {
        let holders = world
            .query::<&Inventory>()
            .filter(|bag| bag.slots().next().is_some())
            .count();
        let worn = worn(world);
        let ground = world.query::<&GroundItem>().count();
        let collectors = world.query::<(&Inventory, &Hurtbox)>().count();
        let (swaps, taken, labels) = (swaps(world), taken(world), labels(world));
        let held = held(world);
        let _ = write!(
            line,
            " holders={holders} worn={worn} swaps={swaps} ground={ground} labels={labels} collectors={collectors} held={held} taken={taken}"
        );
    }

    fn reset(&mut self) {}
}

/// The flask units the collectors' bags hold.
pub(crate) fn held(world: &World) -> u32 {
    world
        .query::<(&Inventory, &Hurtbox)>()
        .map(|(bag, _)| bag.count_of(FLASK))
        .sum()
}

/// The items worn across every bag.
pub(crate) fn worn(world: &World) -> usize {
    world
        .query::<&Inventory>()
        .map(|bag| {
            bag.slots()
                .filter(|(_, slot)| slot.item().is_some())
                .count()
        })
        .sum()
}

/// The swaps so far.
pub(crate) fn swaps(world: &World) -> u64 {
    world
        .get_resource::<ItemsTally>()
        .map_or(0, |tally| tally.swaps)
}

/// The units the collectors took and dropped back so far.
pub(crate) fn taken(world: &World) -> u64 {
    world
        .get_resource::<ItemsTally>()
        .map_or(0, |tally| tally.taken)
}

/// The labels the last frame pushed.
pub(crate) fn labels(world: &World) -> usize {
    world
        .get_resource::<ItemsTally>()
        .map_or(0, |tally| tally.labels)
}

/// Slot `kind`'s two bases, `<kind>_0` and `<kind>_1`, each raising one of
/// the game stats, and the flask the ground holds.
fn insert_data(world: &mut World) {
    let data = world.resource_mut::<DataRegistry>();
    for (s, kind) in SLOTS.iter().enumerate() {
        for variant in 0..2 {
            let stat = STATS[(s + variant) % STATS.len()];
            let base: ItemBase = serde_json::from_value(json!({
                "name": format!("{kind} {variant}"),
                "icon": kind,
                "slot": kind,
                "implicit": [{ "stat": stat, "modifier": { "add": 2 + variant } }]
            }))
            .expect("a worn base");
            data.insert(format!("{kind}_{variant}"), base);
        }
    }
    let flask: ItemBase = serde_json::from_value(
        json!({ "name": "Oil flask", "icon": FLASK, "stack": 5, "ground": DROP }),
    )
    .expect("the flask");
    data.insert(FLASK, flask);
    let drop: PrefabData = serde_json::from_value(json!({ "components": {
        "pickup": { "shape": { "circle": 6 }, "filter": { "all": [COLLECTOR_TAG] } },
        "ground_item": { "item": FLASK }
    } }))
    .expect("the flask's ground prefab");
    if !world.has_resource::<PrefabRegistry>() {
        world.insert_resource(PrefabRegistry::new());
    }
    world
        .resource_mut::<PrefabRegistry>()
        .insert(DROP.to_owned(), drop);
}

/// The holders: each wears `<kind>_1` in every slot, `<kind>_0` its spare
/// in cell `s`; their swaps staggered over the period.
fn spawn_holders(world: &mut World, holders: i64, period: u64) -> Swap {
    let mut swap = Swap {
        holders: Vec::with_capacity(holders as usize),
        period,
        step: 0,
    };
    for h in 0..holders {
        let mut bag = Inventory::new(CELLS);
        for kind in SLOTS {
            bag = bag.with_slot(kind, kind);
        }
        let stats = STATS
            .iter()
            .fold(Stats::new(), |stats, stat| stats.with(stat, 10.0));
        let holder = world.spawn_with((bag, stats, Effects::new()));
        for kind in SLOTS {
            give(world, holder, &format!("{kind}_0"), 1);
        }
        for kind in SLOTS {
            let worn = format!("{kind}_1");
            give(world, holder, &worn, 1);
            let cell = world
                .get::<Inventory>(holder)
                .and_then(|bag| {
                    bag.cells()
                        .position(|item| item.is_some_and(|item| item.base() == worn))
                })
                .expect("the item given");
            move_item(world, holder, ItemPlace::Cell(cell), ItemPlace::worn(kind))
                .expect("the holder wears its slot");
        }
        swap.holders.push(Holder {
            entity: holder,
            offset: (h as u64 * period) / holders as u64,
            turn: 0,
        });
    }
    swap
}

/// One holder: its entity, the frame offset of its swaps and its next
/// slot.
struct Holder {
    entity: Entity,
    offset: u64,
    turn: usize,
}

/// The holders' swaps: each holder, the period and the step count.
struct Swap {
    holders: Vec<Holder>,
    period: u64,
    step: u64,
}

impl Swap {
    /// Swaps each holder due this step: its turn's slot with its spare in
    /// cell `turn`.
    fn run(&mut self, world: &mut World) {
        let mut swapped = 0;
        for holder in &mut self.holders {
            if !(self.step + holder.offset).is_multiple_of(self.period) {
                continue;
            }
            let slot = SLOTS[holder.turn];
            let moved = move_item(
                world,
                holder.entity,
                ItemPlace::Cell(holder.turn),
                ItemPlace::worn(slot),
            );
            if moved.is_ok() {
                swapped += 1;
            }
            holder.turn = (holder.turn + 1) % SLOTS.len();
        }
        self.step += 1;
        if swapped > 0 {
            tally_mut(world).swaps += swapped;
        }
    }
}

fn tally_mut(world: &mut World) -> &mut ItemsTally {
    if !world.has_resource::<ItemsTally>() {
        world.insert_resource(ItemsTally::default());
    }
    world.resource_mut::<ItemsTally>()
}

/// The ground items on their grid, each labelled and drawn.
fn spawn_ground(world: &mut World, ground: i64) {
    let columns = columns(ground);
    let origin = Vec2::splat(SPACING);
    for index in 0..ground {
        let at = origin
            + Vec2::new(
                (index % columns) as f32 * SPACING,
                (index / columns) as f32 * SPACING,
            );
        let entity = world
            .spawn_prefab(DROP, at)
            .expect("the flask's ground prefab spawns");
        world.insert(
            entity,
            ViewSprite {
                half: Vec2::splat(4.0),
                color: [224, 176, 64, 255],
                texture: ViewTexture::Rect,
            },
        );
    }
}

/// One collector: its entity and its path's phase and angular speed on
/// each axis.
struct Walker {
    entity: Entity,
    phase: Vec2,
    omega: Vec2,
}

/// The collectors' paths: each collector, the area's centre and half size
/// and the step count.
struct Walk {
    walkers: Vec<Walker>,
    centre: Vec2,
    half: Vec2,
    step: u64,
}

impl Walk {
    /// Where a collector of `phase` and `omega` stands at the current step.
    fn at(&self, phase: Vec2, omega: Vec2) -> Vec2 {
        let t = self.step as f32 / 60.0;
        let angle = omega * t + phase;
        self.centre + self.half * Vec2::new(angle.x.sin(), angle.y.sin())
    }

    fn run(&mut self, world: &mut World) {
        for walker in &self.walkers {
            let at = self.at(walker.phase, walker.omega);
            if let Some(position) = world.get_mut::<Position>(walker.entity) {
                position.0 = at;
            }
        }
        self.step += 1;
    }
}

/// The collectors on their seeded paths, each with a bag of four cells,
/// the pickup's tag and a hurtbox to collect with.
fn spawn_collectors(world: &mut World, cfg: &BenchConfig) -> Walk {
    let collectors = cfg.int("collectors");
    let (min, max) = walk_area(cfg.int("ground"));
    let mut rng = Pcg32::seeded(splitmix64(cfg.seed() ^ PATH_SALT));
    let mut walk = Walk {
        walkers: Vec::with_capacity(collectors as usize),
        centre: (min + max) * 0.5,
        half: (max - min) * 0.5,
        step: 0,
    };
    for _ in 0..collectors {
        let phase = Vec2::new(
            rng.next_range(0.0, std::f32::consts::TAU),
            rng.next_range(0.0, std::f32::consts::TAU),
        );
        let omega = Vec2::new(rng.next_range(0.6, 1.2), rng.next_range(0.6, 1.2));
        let entity = world.spawn_with((
            Position(walk.at(phase, omega)),
            Inventory::new(4),
            Tags::new([COLLECTOR_TAG]),
            Hurtbox::new(HitShape::circle(8.0)),
            ViewSprite {
                half: Vec2::splat(5.0),
                color: [96, 224, 160, 255],
                texture: ViewTexture::Disc,
            },
        ));
        walk.walkers.push(Walker {
            entity,
            phase,
            omega,
        });
    }
    walk
}

/// Drops back, a unit at a time, everything each collector's bag took the
/// frame before, ahead of this frame's pickups.
fn return_taken(world: &mut World, collectors: &[Entity]) {
    let mut taken = 0;
    for &collector in collectors {
        for cell in 0..4 {
            while world
                .get::<Inventory>(collector)
                .and_then(|bag| bag.cell(cell))
                .is_some()
            {
                if drop_item(world, collector, cell, 1).is_none() {
                    break;
                }
                taken += 1;
            }
        }
    }
    if taken > 0 {
        tally_mut(world).taken += taken;
    }
}

/// Keeps the labels the frame pushed, before the extract drains them.
fn tally_labels(world: &mut World) {
    let labels = world
        .get_resource::<WorldLabels>()
        .map_or(0, WorldLabels::len);
    tally_mut(world).labels = labels;
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let stats = STATS.iter().fold(StatsPlugin::new(), |plugin, stat| {
        plugin.with_stat(stat, 0.0, f32::INFINITY)
    });
    app.add_plugins(
        PluginSet::new()
            .with(TagsPlugin::new().with_tags([COLLECTOR_TAG]))
            .with(stats)
            .with(CountersPlugin)
            .with(CuesPlugin)
            .with(EffectsPlugin)
            .with(AbilitiesPlugin)
            .with(HitPlugin)
            .with(PickupPlugin)
            .with(ItemsPlugin),
    );
    let period = cfg.int("period") as u64;
    let (mut swap, mut walk, collectors) = {
        let world = app.world_mut();
        insert_data(world);
        world.insert_resource(ItemsTally::default());
        let swap = spawn_holders(world, cfg.int("holders"), period);
        spawn_ground(world, cfg.int("ground"));
        let walk = spawn_collectors(world, cfg);
        let collectors: Vec<Entity> = walk.walkers.iter().map(|walker| walker.entity).collect();
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(BenchCounters::new(ItemsCounts));
        (swap, walk, collectors)
    };
    app.on_startup(|world, renderer| r#gen::register_view_textures(world, renderer));
    app.add_system_to(
        Stage::PreUpdate,
        system("bench_counters", bench_counters_system::<ItemsCounts>),
    );
    app.add_system_to(
        Phase::Intent.stage(),
        Phase::Intent.join(system(SWAP, move |world: &mut World| swap.run(world))),
    );
    app.add_system_to(
        Phase::Intent.stage(),
        Phase::Intent.join(system(WALK, move |world: &mut World| walk.run(world))),
    );
    app.add_system_to(
        Stage::PreUpdate,
        system(RETURN, move |world: &mut World| {
            return_taken(world, &collectors);
        })
        .after("bench_counters"),
    );
    app.add_system_to(
        Phase::Present.stage(),
        Phase::Present.join(system(TALLY, tally_labels).after(GROUND_ITEM_LABELS)),
    );
    app.set_extract_sprites(view::extract_view);
}
