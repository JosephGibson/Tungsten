//! The kit's per-entity per-step systems at 1,000 and 10,000 entities, one
//! fixed step per iteration: `lifetime_tick`, `health_tick`, `action_input`,
//! the bodiless `top_down_mover` and `TagFilter::matches`. The measured run
//! with its bound is W13i's; `cargo bench -p tungsten-kit --bench kit --
//! --test` runs each row once.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::Vec2;
use tungsten_core::physics::Position;
use tungsten_core::{
    ActionMap, Binding, CommandBuffer, InputState, KeyCode, Plugin, Schedule, Stage, Time, World,
};
use tungsten_kit::{
    ActionInput, ActionInputPlugin, Health, HealthPlugin, Intent, Lifetime, LifetimePlugin,
    TagFilter, Tags, TopDownMover, TopDownMoverPlugin,
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

criterion_group!(
    kit,
    lifetime_tick,
    health_tick,
    action_input,
    top_down_mover_bodiless,
    tag_filter
);
criterion_main!(kit);
