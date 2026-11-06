//! `spread`: chemistry's spread over a field (redo 7.10b). Bodiless receivers
//! tagged `flammable` on a square grid 24 px apart, every one lit at the
//! start with `burning` (max 4, decay 0.5), which sends `heat` 1 within
//! 50 px every 0.25 s; `heat` on `flammable` builds `burning` by 1. So the
//! field stays saturated: each receiver emits every 0.25 s, the queue holds
//! one `heat` entry per receiver and drains its cap of 256 a run. Judged on
//! `chemistry`, `chemistry_tick` and `effect_tick`. The kit's tags, health,
//! effects and chemistry plugins run in the app's own schedule, with their
//! phases; no `KitPlugins::set()`.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::core::{
    CameraState, DataRef, DataRegistry, Entity, PluginSet, Position, Stage, World, system,
};
use tungsten::kit::{
    Chemistry, ChemistryPlugin, Effect, EffectRequest, Effects, EffectsPlugin, HealthPlugin,
    Interaction, Tags, TagsPlugin,
};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "spread",
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
    Knob::int("entities", 12_000, 1, 100_000)
        .scaled()
        .note("Bodiless receivers on a square grid 24 px apart; 12,000 is the redo's spread row"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("entities", Value::Int(200))],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The receivers' tag, the stimulus and the status.
const FLAMMABLE: &str = "flammable";
const HEAT: &str = "heat";
pub(crate) const BURNING: &str = "burning";
/// The grid's spacing and a dot's half size, in px.
const SPACING: f32 = 24.0;
const HALF: f32 = 2.0;

fn row(_cfg: &BenchConfig) -> &'static str {
    "spread"
}

/// The grid's columns for `entities` receivers: the square's side.
fn columns(entities: i64) -> i64 {
    (entities as f64).sqrt().ceil().max(1.0) as i64
}

fn derived(cfg: &BenchConfig) -> Json {
    let entities = cfg.int("entities");
    let columns = columns(entities);
    json!({
        "world": [VIEWPORT.x, VIEWPORT.y],
        "columns": columns,
        "rows": (entities + columns - 1) / columns,
    })
}

/// The `bench:` counters, read from the frame's final state: the
/// receivers, those burning, and chemistry's queue and counts.
#[derive(Debug, Default)]
pub(crate) struct SpreadCounts;

impl FrameCounters for SpreadCounts {
    fn write(&self, world: &World, line: &mut String) {
        let entities = world.query::<(Entity, &Tags)>().count();
        let burning = world
            .query::<&Effects>()
            .filter(|effects| effects.has(BURNING))
            .count();
        let (pending, dropped, cut, ran) =
            world
                .get_resource::<Chemistry>()
                .map_or((0, 0, 0, 0), |chemistry| {
                    (
                        chemistry.pending() as u64,
                        chemistry.dropped(),
                        chemistry.cut(),
                        chemistry.ran(),
                    )
                });
        let _ = write!(
            line,
            " entities={entities} burning={burning} pending={pending} dropped={dropped} cut={cut} ran={ran}"
        );
    }

    fn reset(&mut self) {}
}

/// The status and the rule, in code: `burning` of max 4 and decay 0.5,
/// sending `heat` 1 within 50 px every 0.25 s, and `heat` on `flammable`
/// building it by 1.
fn insert_data(world: &mut World) {
    let rule: Interaction = serde_json::from_value(json!({
        "stimulus": HEAT, "on": { "all": [FLAMMABLE] },
        "outputs": [{ "build": { "status": BURNING, "amount": 1 } }]
    }))
    .expect("the spread rule");
    let data = world.resource_mut::<DataRegistry>();
    data.insert(
        BURNING,
        Effect::new()
            .with_intensity(4.0, 0.5)
            .with_emit(HEAT, 1.0, 50.0, 0.25),
    );
    data.insert("catches", rule);
}

/// The receivers on their grid, each lit.
fn spawn_field(world: &mut World, cfg: &BenchConfig) {
    let entities = cfg.int("entities");
    let columns = columns(entities);
    let origin = Vec2::splat(SPACING);
    for index in 0..entities {
        let at = origin
            + Vec2::new(
                (index % columns) as f32 * SPACING,
                (index / columns) as f32 * SPACING,
            );
        let receiver = world.spawn_with((
            Position(at),
            Tags::new([FLAMMABLE]),
            ViewSprite {
                half: Vec2::splat(HALF),
                color: [240, 128, 64, 255],
                texture: ViewTexture::Disc,
            },
        ));
        EffectRequest::new(receiver, DataRef::by_id(BURNING)).apply(world);
    }
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    app.add_plugins(
        PluginSet::new()
            .with(TagsPlugin::new().with_tags([FLAMMABLE, HEAT]))
            .with(HealthPlugin)
            .with(EffectsPlugin)
            .with(ChemistryPlugin::new()),
    );
    {
        let world = app.world_mut();
        insert_data(world);
        spawn_field(world, cfg);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(BenchCounters::new(SpreadCounts));
    }
    app.on_startup(|world, renderer| r#gen::register_view_textures(world, renderer));
    app.add_system_to(
        Stage::PreUpdate,
        system("bench_counters", bench_counters_system::<SpreadCounts>),
    );
    app.set_extract_sprites(view::extract_view);
}
