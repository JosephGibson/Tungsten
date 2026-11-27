//! `story`: the story systems over a field of NPCs (8.1's first plan).
//! `walkers` interactors walk seeded closed paths through a square grid of
//! `npcs` interactables 48 px apart, close enough that a walker always has
//! one in reach and its focus moves every few frames; each interactable's
//! `when` reads a flag and a counter. Every `period` frames each walker
//! presses its `interact` slot, staggered, and the `interacted` cue barks
//! with `say`, counts a visit and adds to the game's `talks`. `holders`
//! quest holders each hold `quests` quests of sixteen stages, whose
//! objectives are `talks` thresholds, so stages complete through the run
//! and each pays its holder a counter. Judged on `interact`,
//! `interact_prompt`, `say_tick` and `quest_tick`. The kit's counters, cues,
//! interact, say and quests plugins run in the app's own schedule with their
//! phases; no `KitPlugins::set()`. The walkers' presses are the bench's own
//! `Intent` writes in `Phase::Intent`, before `interact`.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Map, Value as Json, json};
use tungsten::App;
use tungsten::core::{
    CameraState, DataRegistry, Entity, Pcg32, PluginSet, Position, Stage, World, splitmix64, system,
};
use tungsten::kit::{
    Barks, Condition, Counters, CountersPlugin, Cues, CuesPlugin, INTERACT, Intent, InteractPlugin,
    Interactable, Interactor, Phase, Quest, Quests, QuestsPlugin, SayPlugin,
};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "story",
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
    Knob::int("walkers", 8, 1, 64)
        .scaled()
        .note("Interactors on seeded closed paths through the field"),
    Knob::int("npcs", 400, 1, 20_000)
        .scaled()
        .note("Interactables on a square grid 48 px apart, each with a `when`"),
    Knob::int("holders", 64, 1, 4_096)
        .scaled()
        .note("Entities holding `quests` quests each"),
    Knob::int("quests", 8, 1, 32).note("Quests a holder holds, sixteen stages each"),
    Knob::int("period", 30, 1, 600).note("Frames between a walker's presses"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("walkers", Value::Int(2)),
            ("npcs", Value::Int(40)),
            ("holders", Value::Int(8)),
            ("quests", Value::Int(4)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The walkers' intent slot.
const SLOT: &str = "interact";
/// The game's counter each press adds to, which the objectives read.
pub(crate) const TALKS: &str = "talks";
/// The holder's counter each completed stage's reward adds to.
pub(crate) const EARNED: &str = "earned";
/// Stages a quest holds.
const STAGES: i64 = 16;
/// The grid's spacing in px: a point inside it is at most 34 px from an
/// interactable, inside the interactor's default reach of 48.
const SPACING: f32 = 48.0;
/// The walker system's name, ahead of `interact` in `Phase::Intent`.
const WALK: &str = "story_walk";
/// Seeds the walkers' paths.
const PATH_SALT: u64 = 0x5703_0000_0000_0001;

fn row(_cfg: &BenchConfig) -> &'static str {
    "story"
}

/// The grid's columns for `npcs` interactables: the square's side.
fn columns(npcs: i64) -> i64 {
    (npcs as f64).sqrt().ceil().max(1.0) as i64
}

/// The walkers' area, `(min, max)`: the grid's full rows, so every point in
/// it has an interactable in reach.
fn walk_area(npcs: i64) -> (Vec2, Vec2) {
    let columns = columns(npcs);
    let full_rows = (npcs / columns).max(1);
    let min = Vec2::splat(SPACING);
    let max = min + Vec2::new((columns - 1) as f32, (full_rows - 1) as f32) * SPACING;
    (min, max)
}

fn derived(cfg: &BenchConfig) -> Json {
    let npcs = cfg.int("npcs");
    let columns = columns(npcs);
    let (min, max) = walk_area(npcs);
    json!({
        "world": [VIEWPORT.x, VIEWPORT.y],
        "columns": columns,
        "rows": (npcs + columns - 1) / columns,
        "walk": [min.x, min.y, max.x, max.y],
        "stages": STAGES,
    })
}

/// The `bench:` counters, read from the frame's final state: the
/// populations, the walkers with a focus, the live barks, the game's talks
/// and the stages completed so far.
#[derive(Debug, Default)]
pub(crate) struct StoryCounts;

impl FrameCounters for StoryCounts {
    fn write(&self, world: &World, line: &mut String) {
        let walkers = world.query::<&Interactor>().count();
        let focused = focused(world);
        let npcs = world.query::<&Interactable>().count();
        let holders = world.query::<&Quests>().count();
        let barks = world.get_resource::<Barks>().map_or(0, Barks::len);
        let talks = world
            .get_resource::<Counters>()
            .map_or(0, |counters| counters.get(TALKS));
        let stages = stages(world);
        let _ = write!(
            line,
            " walkers={walkers} focused={focused} npcs={npcs} holders={holders} barks={barks} talks={talks} stages={stages}"
        );
    }

    fn reset(&mut self) {}
}

/// The walkers with a focus.
pub(crate) fn focused(world: &World) -> usize {
    world
        .query::<&Interactor>()
        .filter(|interactor| interactor.focus().is_some())
        .count()
}

/// The stages completed so far: every holder's `earned`.
pub(crate) fn stages(world: &World) -> i64 {
    world
        .query::<(Entity, &Quests)>()
        .map(|(holder, _)| {
            world
                .get::<Counters>(holder)
                .map_or(0, |counters| counters.get(EARNED))
        })
        .sum()
}

/// Quest `q` of `quests`: sixteen stages chained by `next`, stage `i`
/// complete once the game's `talks` reaches `quests * i + q + 1`, so each
/// talk completes one quest's stage and the quests advance in turn; each
/// stage pays its holder one `earned`.
fn quest(q: i64, quests: i64) -> Quest {
    let mut stages = Map::new();
    for i in 0..STAGES {
        let mut stage = json!({
            "journal": format!("Talk to the town, part {i}."),
            "objectives": [{
                "text": "Talks",
                "when": { "counter": { "name": TALKS, "min": quests * i + q + 1, "target": "game" } }
            }],
            "rewards": [{ "counter": { "name": EARNED, "add": 1 } }]
        });
        if i + 1 < STAGES {
            stage["next"] = json!(format!("s{}", i + 1));
        }
        stages.insert(format!("s{i}"), stage);
    }
    serde_json::from_value(
        json!({ "title": format!("Rounds {q}"), "start": "s0", "stages": stages }),
    )
    .expect("the story quest")
}

fn insert_data(world: &mut World, quests: i64) {
    let data = world.resource_mut::<DataRegistry>();
    for q in 0..quests {
        data.insert(format!("q{q}"), quest(q, quests));
    }
}

/// The interactables on their grid, each offered while it is not hushed
/// and its visits stay in bounds, each barking, counting its visit and
/// adding to the game's talks when used.
fn spawn_npcs(world: &mut World, npcs: i64) {
    let columns = columns(npcs);
    let when: Condition = serde_json::from_value(json!({ "all": [
        { "not": { "flag": "hushed" } },
        { "counter": { "name": "visits", "max": 1_000_000 } }
    ] }))
    .expect("the npcs' when");
    let cues: Cues = serde_json::from_value(json!({ "interacted": [
        { "say": "Evening." },
        { "counter": { "name": "visits", "add": 1 } },
        { "counter": { "name": TALKS, "add": 1, "target": "game" } }
    ] }))
    .expect("the npcs' cues");
    let origin = Vec2::splat(SPACING);
    for index in 0..npcs {
        let at = origin
            + Vec2::new(
                (index % columns) as f32 * SPACING,
                (index / columns) as f32 * SPACING,
            );
        world.spawn_with((
            Position(at),
            Interactable::new("Talk").with_when(when.clone()),
            cues.clone(),
            ViewSprite {
                half: Vec2::splat(6.0),
                color: [96, 160, 224, 255],
                texture: ViewTexture::Rect,
            },
        ));
    }
}

/// One walker: its entity, its path's phase and angular speed on each
/// axis, and the frame offset of its presses.
struct Walker {
    entity: Entity,
    phase: Vec2,
    omega: Vec2,
    offset: u64,
    pressed: bool,
}

/// The walkers' system state: each walker, the area's centre and half
/// size, the press period and the step count.
struct Walk {
    walkers: Vec<Walker>,
    centre: Vec2,
    half: Vec2,
    period: u64,
    step: u64,
}

impl Walk {
    /// Where a walker of `phase` and `omega` stands at the current step.
    fn at(&self, phase: Vec2, omega: Vec2) -> Vec2 {
        let t = self.step as f32 / 60.0;
        let angle = omega * t + phase;
        self.centre + self.half * Vec2::new(angle.x.sin(), angle.y.sin())
    }

    /// Moves each walker along its path and presses its slot on its frames,
    /// writing its intent only when the press changes.
    fn run(&mut self, world: &mut World) {
        for i in 0..self.walkers.len() {
            let at = self.at(self.walkers[i].phase, self.walkers[i].omega);
            let walker = &mut self.walkers[i];
            if let Some(position) = world.get_mut::<Position>(walker.entity) {
                position.0 = at;
            }
            let press = (self.step + walker.offset).is_multiple_of(self.period);
            if press != walker.pressed {
                walker.pressed = press;
                if let Some(intent) = world.get_mut::<Intent>(walker.entity) {
                    intent.set_slot(SLOT, press, press);
                }
            }
        }
        self.step += 1;
    }
}

/// The walkers, their seeded paths over the walk area and staggered
/// presses.
fn spawn_walkers(world: &mut World, cfg: &BenchConfig) -> Walk {
    let walkers = cfg.int("walkers");
    let period = cfg.int("period") as u64;
    let (min, max) = walk_area(cfg.int("npcs"));
    let mut rng = Pcg32::seeded(splitmix64(cfg.seed() ^ PATH_SALT));
    let mut walk = Walk {
        walkers: Vec::with_capacity(walkers as usize),
        centre: (min + max) * 0.5,
        half: (max - min) * 0.5,
        period,
        step: 0,
    };
    for w in 0..walkers {
        let phase = Vec2::new(
            rng.next_range(0.0, std::f32::consts::TAU),
            rng.next_range(0.0, std::f32::consts::TAU),
        );
        let omega = Vec2::new(rng.next_range(0.6, 1.2), rng.next_range(0.6, 1.2));
        let entity = world.spawn_with((
            Position(walk.at(phase, omega)),
            Interactor::new(),
            Intent::default(),
            ViewSprite {
                half: Vec2::splat(5.0),
                color: [240, 208, 96, 255],
                texture: ViewTexture::Disc,
            },
        ));
        walk.walkers.push(Walker {
            entity,
            phase,
            omega,
            offset: (w as u64 * period) / walkers as u64,
            pressed: false,
        });
    }
    walk
}

/// The holders, each holding every quest at a stage of its own, the first
/// four in turn, so their completions start apart.
fn spawn_holders(world: &mut World, holders: i64, quests: i64) {
    for h in 0..holders {
        let held: Map<String, Json> = (0..quests)
            .map(|q| (format!("q{q}"), json!({ "stage": format!("s{}", h % 4) })))
            .collect();
        let quests: Quests =
            serde_json::from_value(Json::Object(held)).expect("the holder's quests");
        world.spawn_with((quests,));
    }
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    app.add_plugins(
        PluginSet::new()
            .with(CountersPlugin)
            .with(CuesPlugin)
            .with(InteractPlugin)
            .with(SayPlugin)
            .with(QuestsPlugin),
    );
    let mut walk = {
        let world = app.world_mut();
        insert_data(world, cfg.int("quests"));
        spawn_npcs(world, cfg.int("npcs"));
        spawn_holders(world, cfg.int("holders"), cfg.int("quests"));
        let walk = spawn_walkers(world, cfg);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(BenchCounters::new(StoryCounts));
        walk
    };
    app.on_startup(|world, renderer| r#gen::register_view_textures(world, renderer));
    app.add_system_to(
        Stage::PreUpdate,
        system("bench_counters", bench_counters_system::<StoryCounts>),
    );
    app.add_system_to(
        Phase::Intent.stage(),
        Phase::Intent.join(system(WALK, move |world: &mut World| walk.run(world)).before(INTERACT)),
    );
    app.set_extract_sprites(view::extract_view);
}
