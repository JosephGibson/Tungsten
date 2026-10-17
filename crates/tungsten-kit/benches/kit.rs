//! The kit's per-entity per-step systems at 1,000 and 10,000 entities, one
//! fixed step per iteration: `lifetime_tick`, `health_tick`, `action_input`,
//! the bodiless `top_down_mover`, `TagFilter::matches`, `stats_apply`,
//! `cooldown_tick` (no jitter) and `cues` (one `damaged` occurrence per
//! entity per step, with an entity cue and one rule). The measured run with
//! its bound is W13i's; `cargo test -p tungsten-kit --bench kit` runs each
//! row once.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::Vec2;
use tungsten_core::physics::Position;
use tungsten_core::{
    ActionMap, Binding, CommandBuffer, EventQueue, InputState, KeyCode, Plugin, PluginSet,
    Schedule, Stage, StagedLoad, Time, World,
};
use tungsten_kit::{
    ActionInput, ActionInputPlugin, Cooldown, CooldownPlugin, Counters, CountersPlugin, CueAction,
    Cues, CuesPlugin, Damaged, Health, HealthPlugin, Intent, Lifetime, LifetimePlugin,
    PlatformerController, Stats, StatsPlugin, TagFilter, Tags, TagsPlugin, TopDownMover,
    TopDownMoverPlugin,
};

const SIZES: [usize; 2] = [1_000, 10_000];
const STEP: f32 = 1.0 / 60.0;

/// A World with the clock and a buffer, and `plugin`'s schedule, resolved.
fn rig(plugin: &impl Plugin) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let mut schedule = Schedule::new();
    plugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn fixed_step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(STEP);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn lifetime_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("lifetime_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&LifetimePlugin);
        for _ in 0..n {
            world.spawn_with((Lifetime::new(1e6),));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn health_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("health_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&HealthPlugin);
        for _ in 0..n {
            world.spawn_with((Health::new(10.0).with_invulnerability(1e6),));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn action_input(c: &mut Criterion) {
    let mut group = c.benchmark_group("action_input");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&ActionInputPlugin);
        let mut actions = ActionMap::new();
        for (action, code) in [
            ("move_left", KeyCode::ArrowLeft),
            ("move_right", KeyCode::ArrowRight),
            ("move_up", KeyCode::ArrowUp),
            ("move_down", KeyCode::ArrowDown),
            ("jump", KeyCode::Space),
        ] {
            actions.replace_bindings(action, vec![Binding::Key { code }]);
        }
        world.insert_resource(actions);
        let mut input = InputState::new();
        input.key_down(KeyCode::ArrowRight);
        input.key_down(KeyCode::Space);
        world.insert_resource(input);
        for _ in 0..n {
            world.spawn_with((
                ActionInput::new("move_left", "move_right")
                    .with_vertical("move_up", "move_down")
                    .with_slot("jump", "jump"),
                Intent::default(),
            ));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn top_down_mover_bodiless(c: &mut Criterion) {
    let mut group = c.benchmark_group("top_down_mover_bodiless");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&TopDownMoverPlugin);
        for i in 0..n {
            let mut mover = TopDownMover::new(120.0);
            mover.set_direction(Vec2::new(1.0, (i % 3) as f32 - 1.0));
            world.spawn_with((mover, Position::new(i as f32, 0.0)));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn tag_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("tag_filter");
    for n in SIZES {
        let tags: Vec<Tags> = (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    Tags::new(["team.enemy", "flying"])
                } else {
                    Tags::new(["team.player"])
                }
            })
            .collect();
        let filter = TagFilter::new().all(["team.enemy"]).none(["boss"]);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let hits = tags
                    .iter()
                    .filter(|tags| filter.matches(Some(black_box(tags))))
                    .count();
                black_box(hits)
            });
        });
    }
    group.finish();
}

fn stats_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_apply");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&StatsPlugin::new());
        for i in 0..n {
            world.spawn_with((
                TopDownMover::new(100.0),
                PlatformerController::new(200.0, 600.0),
                Health::new(5.0),
                Stats::new()
                    .with(Stats::MOVE_SPEED, 80.0 + (i % 7) as f32)
                    .with(Stats::MAX_HEALTH, 8.0)
                    .with("armor", 2.0),
            ));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn cooldown_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("cooldown_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&CooldownPlugin);
        for i in 0..n {
            world.spawn_with((Cooldown::new(0.5 + (i % 5) as f32 * 0.1),));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                fixed_step(black_box(&mut world), &mut schedule);
                world.flush_events();
            });
        });
    }
    group.finish();
}

/// The `cues` row's shared rule, staged from a file as a manifest's load
/// stages it.
fn stage_rule(world: &mut World) {
    let dir = std::env::temp_dir().join(format!("tungsten_kit_bench_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rules.json");
    std::fs::write(
        &path,
        r#"[{ "on": "damaged", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "hits", "add": 1, "target": "game" } }] }]"#,
    )
    .unwrap();
    let mut load = StagedLoad::new();
    load.add_data(world, "cue_rules", "bench", &path).unwrap();
    load.commit(world).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

fn cues(c: &mut Criterion) {
    let mut group = c.benchmark_group("cues");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&CuesPlugin);
        PluginSet::new()
            .with(TagsPlugin::new().with_tags(["team.enemy"]))
            .with(HealthPlugin)
            .with(CountersPlugin)
            .build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        stage_rule(&mut world);
        let entities: Vec<_> = (0..n)
            .map(|_| {
                world.spawn_with((
                    Health::new(1e9),
                    Tags::new(["team.enemy"]),
                    Cues::new().on("damaged", [CueAction::counter_add("hits", 1)]),
                ))
            })
            .collect();
        let hit_all = |world: &mut World| {
            let queue = world.resource_mut::<EventQueue<Damaged>>();
            for &entity in &entities {
                queue.send(Damaged::new(entity, 1.0, false));
            }
        };
        // One warm step: every entity's own cue and the rule ran.
        hit_all(&mut world);
        fixed_step(&mut world, &mut schedule);
        world.flush_events();
        assert_eq!(world.resource::<Counters>().get("hits"), n as i64);
        assert_eq!(world.get::<Counters>(entities[0]).unwrap().get("hits"), 1);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let queue = world.resource_mut::<EventQueue<Damaged>>();
                for &entity in &entities {
                    queue.send(Damaged::new(entity, 1.0, false));
                }
                fixed_step(black_box(&mut world), &mut schedule);
                world.flush_events();
            });
        });
    }
    group.finish();
}

criterion_group!(
    kit,
    lifetime_tick,
    health_tick,
    action_input,
    top_down_mover_bodiless,
    tag_filter,
    stats_apply,
    cooldown_tick,
    cues
);
criterion_main!(kit);
