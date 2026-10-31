//! `saves`: the cost of a world save (redo 7.9). One area of named objects
//! with health, stats and tags, and a named spawner's runtime spawns with
//! bodies, all drifting each frame so a save holds changes; each frame
//! `save_snapshot`, first in `pre_update` where the command buffer is
//! empty, captures the World (`capture_world`) and serializes it. Judged on
//! `save_snapshot` and `update`. The kit and physics plugins build into a
//! schedule the app never runs: their saved components register, their
//! systems stay out of the frame.

use std::fmt::Write as _;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten::core::save::{self, Saved, capture_world};
use tungsten::core::{
    CameraState, CommandBuffer, Pcg32, Plugin, Position, PrefabRegistry, Schedule, Stage, Time,
    World, system,
};
use tungsten::kit::{Health, HealthPlugin, Stats, StatsPlugin, Tags, TagsPlugin};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen;
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};
use crate::view::{self, VIEWPORT, ViewSprite, ViewTexture};

pub(crate) static BENCH: Bench = Bench {
    name: "saves",
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
    Knob::int("objects", 100, 1, 10_000)
        .scaled()
        .note("Named objects in the area"),
    Knob::int("spawns", 12_000, 0, 100_000)
        .scaled()
        .note("The named spawner's runtime spawns; 12,000 is example 01's full pit"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("objects", Value::Int(20)), ("spawns", Value::Int(200))],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// The area the bench names its objects in.
const AREA: &str = "bench";
/// The spawns' prefab, empty: each takes its body through `with`.
const ORB: &str = "orb";
/// `save_snapshot`'s name, which its owned metrics read.
pub(crate) const SAVE_SNAPSHOT: &str = "save_snapshot";
const SPEED: (f32, f32) = (10.0, 60.0);
const HALF: f32 = 3.0;

fn row(_cfg: &BenchConfig) -> &'static str {
    "saves"
}

fn derived(cfg: &BenchConfig) -> Json {
    json!({
        "world": [VIEWPORT.x, VIEWPORT.y],
        "records": cfg.int("objects") + 1 + cfg.int("spawns"),
    })
}

/// The `bench:` counters: the records the frame's save held and its bytes.
#[derive(Debug, Default)]
pub(crate) struct SaveCounts {
    pub(crate) saved: u64,
    pub(crate) bytes: u64,
    pub(crate) visible: u64,
}

impl FrameCounters for SaveCounts {
    fn write(&self, _world: &World, line: &mut String) {
        let _ = write!(
            line,
            " saved={} bytes={} visible={}",
            self.saved, self.bytes, self.visible
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The objects, the spawner and its spawns: one named the run's way, the
/// spawns from it through the World's command buffer.
fn spawn_world(world: &mut World, cfg: &BenchConfig) {
    let mut rng = Pcg32::seeded(cfg.seed());
    let draw = |rng: &mut Pcg32| {
        let at = Vec2::new(
            rng.next_range(0.0, VIEWPORT.x),
            rng.next_range(0.0, VIEWPORT.y),
        );
        let angle = rng.next_range(0.0, std::f32::consts::TAU);
        let speed = rng.next_range(SPEED.0, SPEED.1);
        (at, Vec2::from_angle(angle) * speed)
    };
    let sprite = |color| ViewSprite {
        half: Vec2::splat(HALF),
        color,
        texture: ViewTexture::Disc,
    };
    for index in 0..cfg.int("objects") {
        let (at, velocity) = draw(&mut rng);
        let object = world.spawn_with((
            Position(at),
            Velocity(velocity),
            Health::new(10.0),
            Stats::new().with("speed", velocity.length()),
            Tags::new(["bench.object"]),
            sprite([242, 196, 92, 255]),
        ));
        save::name(world, object, Some(AREA), &format!("object_{index}"))
            .expect("the bench's names are free");
    }
    let spawner = world.spawn_with((Position(VIEWPORT * 0.5),));
    save::name(world, spawner, Some(AREA), "spawner").expect("the spawner's name is free");
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        ORB.into(),
        serde_json::from_value(json!({})).expect("an empty prefab"),
    );
    world.insert_resource(prefabs);
    let mut commands = CommandBuffer::new();
    for _ in 0..cfg.int("spawns") {
        let (at, velocity) = draw(&mut rng);
        let mut body = RigidBodyBundle::kinematic(Position(at), Collider::circle(HALF));
        body.velocity = Some(Velocity(velocity));
        let orb = world
            .prepare_prefab(ORB, at)
            .expect("the orb prefab prepares")
            .with(body)
            .with((sprite([120, 186, 240, 255]),));
        commands.spawn_prefab_from(spawner, orb);
    }
    world.flush(commands);
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    {
        let world = app.world_mut();
        let mut registrations = Schedule::new();
        for plugin in [
            &PhysicsPlugin as &dyn Plugin,
            &HealthPlugin,
            &StatsPlugin::default(),
            &TagsPlugin::new(),
        ] {
            plugin.build(&mut registrations, world);
        }
        spawn_world(world, cfg);
        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(BenchCounters::new(SaveCounts::default()));
    }
    app.on_startup(|world, renderer| r#gen::register_view_textures(world, renderer));
    app.add_system_to(
        Stage::PreUpdate,
        system("bench_counters", bench_counters_system::<SaveCounts>),
    );
    app.add_system_to(
        Stage::PreUpdate,
        system(SAVE_SNAPSHOT, save_snapshot).after("bench_counters"),
    );
    app.add_system_named("saves_drift", drift);
    app.set_extract_sprites(view::extract_view);
}

/// Captures the World and serializes it: the records saved and the bytes.
fn save_snapshot(world: &mut World) {
    let bytes = match capture_world(world) {
        Ok(saved) => serde_json::to_vec(&saved).map_or(0, |json| json.len()),
        Err(err) => {
            log::warn!("save_snapshot: {err}");
            0
        }
    };
    let saved = world.query::<&Saved>().count() as u64;
    let visible = {
        let bounds = view::view_bounds(world);
        world
            .query::<(&Position, &ViewSprite)>()
            .filter(|(position, sprite)| view::in_view(bounds, position.0, sprite.half))
            .count() as u64
    };
    let counts = &mut world.resource_mut::<BenchCounters<SaveCounts>>().counts;
    counts.saved = saved;
    counts.bytes = bytes as u64;
    counts.visible = visible;
}

/// Moves everything by its velocity, bouncing off the viewport's edges, so
/// each save holds changed positions.
fn drift(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    for (position, velocity) in world.query_mut::<(&mut Position, &mut Velocity)>() {
        position.0 += velocity.0 * dt;
        for axis in 0..2 {
            if position.0[axis] < 0.0 || position.0[axis] > VIEWPORT[axis] {
                velocity.0[axis] = -velocity.0[axis];
                position.0[axis] = position.0[axis].clamp(0.0, VIEWPORT[axis]);
            }
        }
    }
}
