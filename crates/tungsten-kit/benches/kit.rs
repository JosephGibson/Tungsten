//! The kit's per-entity systems at 1,000 and 10,000 entities, 33 rows, one
//! fixed step or one frame's present phase per iteration:
//!
//! - fixed step: `lifetime_tick`, `health_tick`, `action_input`, the
//!   bodiless `top_down_mover`, `TagFilter::matches`, `stats_apply`,
//!   `cooldown_tick` (no jitter), `cues` (one `damaged` occurrence per
//!   entity per step, with an entity cue and one rule), and M64's
//!   `linear_motion`, `oscillator`, `follow` and `steering` (each after one
//!   tagged target, through the nearest search), `orbit` (its spawn point),
//!   `knockback` (pushes running), `speed_limit` (bodiless),
//!   `speed_limit_settle` (the plugin's step on dynamic bodies, no solver),
//!   `wrap` and `stay_in_view` (a region), `aim` (one tagged target);
//! - M65's, each at one density, its area growing with its count, so no
//!   grid row is judged on a denser crowd: `hit_grid` (moving hurtboxes, no
//!   hitbox), `hitbox_tick` (each hitbox over one hurtbox it has already
//!   hit, the steady state), `contact_damage` (each over one target, hit
//!   again each interval), `projectile_tick` (bodiless projectiles with
//!   hitboxes in open space beside one static body, so the wall queries
//!   run), `pickup_tick` (no collector reached), `pickup_magnet` (each
//!   pickup in a collector's radius, one collector per ten pickups, so a
//!   scan over every collector would fail the bound), `damage_resolve` (one
//!   request a step per target, through its resistance and its shield) and
//!   `shield_tick` (recharging);
//! - M66's: `damage_resolve_typed` (one request a step per target of two
//!   typed parts, half of one converted, with penetration, through two
//!   resistances, one under a modifier, and its shield) and `cues_typed`
//!   (one `damaged_part` occurrence per entity per step with its kinds,
//!   with an entity cue and one rule's kind filter);
//! - present phase: `oscillator_present` (rotation and scale), `facing`,
//!   `character_animator` (a clip set, unchanged) and `depth_sort`.
//!
//! The bound, from M64's first measured run: each row's median per entity
//! at 10,000 is at most twice its median per entity at 1,000, so no row
//! grows faster than its entities. `cargo test -p tungsten-kit --bench kit`
//! runs each row once.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::Vec2;
use tungsten_core::assets::AnimationFrame;
use tungsten_core::physics::{Collider, Position, PrevPosition, RigidBodyBundle, Velocity};
use tungsten_core::{
    ActionMap, AnimationData, AnimationRegistry, Binding, Bundle, CommandBuffer, EventQueue,
    InputState, KeyCode, Plugin, PluginSet, Schedule, Sprite, SpriteAssetId, Stage, StagedLoad,
    Time, Transform, World,
};
use tungsten_kit::{
    ActionInput, ActionInputPlugin, Aim, AimPlugin, CharacterAnimator, CharacterAnimatorPlugin,
    Clips, ContactDamage, ContactDamagePlugin, Conversion, Cooldown, CooldownPlugin, Counters,
    CountersPlugin, CueAction, Cues, CuesPlugin, DamagePart, DamageRequest, Damaged, DamagedPart,
    DepthSort, DepthSortPlugin, Facing, FacingPlugin, Follow, FollowPlugin, Health, HealthPlugin,
    HitPlugin, HitShape, Hitbox, Hurtbox, Intent, Knockback, KnockbackPlugin, Lifetime,
    LifetimePlugin, LinearMotion, LinearMotionPlugin, Modifier, Orbit, OrbitPlugin, Oscillator,
    OscillatorPlugin, Penetration, Pickup, PickupPlugin, PlatformerController, Projectile,
    ProjectilePlugin, Resist, Shield, SpeedLimit, SpeedLimitPlugin, Stats, StatsPlugin, StayInView,
    StayInViewPlugin, Steering, SteeringPlugin, TagFilter, Tags, TagsPlugin, Target, TopDownMover,
    TopDownMoverPlugin, Wrap, WrapPlugin, knock_back,
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

/// One frame's present phase: `PostUpdate`, where the kit's present
/// systems run.
fn present_step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(STEP);
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

/// A bodiless entity at `at`: a `Position` and a `PrevPosition`.
fn placed(at: Vec2) -> (Position, PrevPosition) {
    (Position(at), PrevPosition(at))
}

/// A row of `n` entities each spawned by `spawn(world, i)` into `plugin`'s
/// rig, `setup` run once after, then `step` per iteration.
fn row(
    c: &mut Criterion,
    name: &str,
    plugin: &impl Plugin,
    setup: fn(&mut World),
    spawn: fn(&mut World, usize),
    step: fn(&mut World, &mut Schedule),
) {
    let mut group = c.benchmark_group(name);
    for n in SIZES {
        let (mut world, mut schedule) = rig(plugin);
        setup(&mut world);
        for i in 0..n {
            spawn(&mut world, i);
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn nothing(_: &mut World) {}

/// The one tagged target the seeking rows look for.
fn one_target(world: &mut World) {
    world.spawn_with((
        Tags::new(["team.player"]),
        Position(Vec2::new(5000.0, 5000.0)),
    ));
}

fn the_player() -> Target {
    Target::nearest(TagFilter::new().all(["team.player"]))
}

fn linear_motion(c: &mut Criterion) {
    row(
        c,
        "linear_motion",
        &LinearMotionPlugin,
        nothing,
        |world, i| {
            let velocity = Vec2::new(30.0, (i % 5) as f32);
            world.spawn_with((LinearMotion::new(velocity),).with(placed(Vec2::ZERO)));
        },
        fixed_step,
    );
}

fn oscillator(c: &mut Criterion) {
    row(
        c,
        "oscillator",
        &OscillatorPlugin,
        nothing,
        |world, i| {
            let bob = Oscillator::position(Vec2::new(0.0, 4.0), 1.0 + (i % 3) as f32 * 0.1);
            world.spawn_with((bob,).with(placed(Vec2::new(i as f32, 0.0))));
        },
        fixed_step,
    );
}

fn oscillator_present(c: &mut Criterion) {
    row(
        c,
        "oscillator_present",
        &OscillatorPlugin,
        nothing,
        |world, i| {
            let wobble = Oscillator::rotation(0.3, 0.8).with_scale(0.1, 0.5 + (i % 3) as f32 * 0.1);
            world.spawn_with((wobble, Transform::from_position(Vec2::new(i as f32, 0.0))));
        },
        present_step,
    );
}

fn follow(c: &mut Criterion) {
    row(
        c,
        "follow",
        &FollowPlugin,
        one_target,
        |world, i| {
            let pet = Follow::new(the_player()).with_lag(0.2);
            world.spawn_with((pet,).with(placed(Vec2::new(i as f32, 0.0))));
        },
        fixed_step,
    );
}

fn orbit(c: &mut Criterion) {
    row(
        c,
        "orbit",
        &OrbitPlugin,
        nothing,
        |world, i| {
            let blade = Orbit::new(40.0, 3.0);
            world.spawn_with((blade,).with(placed(Vec2::new(i as f32, 0.0))));
        },
        fixed_step,
    );
}

fn steering(c: &mut Criterion) {
    row(
        c,
        "steering",
        &SteeringPlugin,
        one_target,
        |world, i| {
            let seeker = Steering::seek(the_player(), 90.0).with_acceleration(600.0);
            world.spawn_with((seeker,).with(placed(Vec2::new(i as f32, 0.0))));
        },
        fixed_step,
    );
}

fn knockback(c: &mut Criterion) {
    row(
        c,
        "knockback",
        &KnockbackPlugin,
        nothing,
        |world, i| {
            let rover =
                world.spawn_with((Knockback::new(1e6),).with(placed(Vec2::new(i as f32, 0.0))));
            assert!(knock_back(world, rover, Vec2::new(-200.0, 50.0)));
        },
        fixed_step,
    );
}

fn speed_limit(c: &mut Criterion) {
    row(
        c,
        "speed_limit",
        &SpeedLimitPlugin,
        nothing,
        |world, i| {
            let cap = SpeedLimit::speed(100.0).with_y(60.0);
            world.spawn_with(
                (cap, Velocity(Vec2::new(150.0, (i % 7) as f32 * 20.0))).with(placed(Vec2::ZERO)),
            );
        },
        fixed_step,
    );
}

fn speed_limit_settle(c: &mut Criterion) {
    row(
        c,
        "speed_limit_settle",
        &SpeedLimitPlugin,
        nothing,
        |world, i| {
            let body =
                RigidBodyBundle::dynamic(Position::new(i as f32 * 4.0, 0.0), Collider::circle(1.0));
            let entity = world.spawn_with(body.with((SpeedLimit::y(300.0),)));
            world.get_mut::<Velocity>(entity).unwrap().0 = Vec2::new(0.0, 400.0);
        },
        fixed_step,
    );
}

fn wrap(c: &mut Criterion) {
    row(
        c,
        "wrap",
        &WrapPlugin,
        nothing,
        |world, i| {
            let arena =
                Wrap::region(Vec2::ZERO, Vec2::new(20_000.0, 720.0)).with_size(Vec2::splat(16.0));
            world.spawn_with((arena,).with(placed(Vec2::new(i as f32, 100.0))));
        },
        fixed_step,
    );
}

fn stay_in_view(c: &mut Criterion) {
    row(
        c,
        "stay_in_view",
        &StayInViewPlugin,
        nothing,
        |world, i| {
            let room = StayInView::region(Vec2::ZERO, Vec2::new(20_000.0, 720.0))
                .with_size(Vec2::splat(16.0));
            world.spawn_with((room,).with(placed(Vec2::new(i as f32, 100.0))));
        },
        fixed_step,
    );
}

fn aim(c: &mut Criterion) {
    row(
        c,
        "aim",
        &AimPlugin,
        one_target,
        |world, i| {
            let sentry = Aim::at(the_player()).with_offset(Vec2::splat(8.0));
            world.spawn_with((sentry, Position(Vec2::new(i as f32, 0.0))));
        },
        fixed_step,
    );
}

fn facing(c: &mut Criterion) {
    row(
        c,
        "facing",
        &FacingPlugin,
        nothing,
        |world, i| {
            let heading = Facing::new().four_way().turning(0.0);
            let velocity = Velocity(Vec2::new((i % 5) as f32 - 2.0, 3.0));
            world.spawn_with((heading, velocity, Transform::default()));
        },
        present_step,
    );
}

/// The animator row's clips: `idle` and a four-way `walk`.
fn clips(world: &mut World) {
    let mut clips = AnimationRegistry::new();
    for (first, id) in ["idle", "walk_left", "walk_right", "walk_up", "walk_down"]
        .into_iter()
        .enumerate()
    {
        clips.insert(
            id.to_string(),
            AnimationData {
                looping: true,
                frames: (0..2)
                    .map(|frame| AnimationFrame {
                        sprite: SpriteAssetId::new((first * 2 + frame) as u32),
                        duration_ms: 100,
                    })
                    .collect(),
            },
        );
    }
    world.insert_resource(clips);
}

fn character_animator(c: &mut Criterion) {
    // The animator alone: the facing it reads holds still, and no tick
    // advances the clips.
    row(
        c,
        "character_animator",
        &CharacterAnimatorPlugin,
        clips,
        |world, i| {
            let walker = CharacterAnimator::new("idle").with_move(Clips::four(
                "walk_left",
                "walk_right",
                "walk_up",
                "walk_down",
            ));
            let velocity = Velocity(Vec2::new(40.0 * ((i % 3) as f32 - 1.0), 30.0));
            world.spawn_with((
                walker,
                Facing::new().four_way(),
                velocity,
                Sprite::new(SpriteAssetId::new(0)),
            ));
        },
        present_step,
    );
}

fn depth_sort(c: &mut Criterion) {
    row(
        c,
        "depth_sort",
        &DepthSortPlugin,
        nothing,
        |world, i| {
            let at = Transform::from_position(Vec2::new(0.0, (i % 700) as f32));
            world.spawn_with((
                DepthSort::new().with_offset(16.0),
                at,
                Sprite::new(SpriteAssetId::new(0)),
            ));
        },
        present_step,
    );
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

/// Stages `json` as entry `bench` of `kind` from a file, as a manifest's
/// load stages it.
fn stage(world: &mut World, kind: &str, json: &str) {
    let dir = std::env::temp_dir().join(format!("tungsten_kit_bench_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{kind}.json"));
    std::fs::write(&path, json).unwrap();
    let mut load = StagedLoad::new();
    load.add_data(world, kind, "bench", &path).unwrap();
    load.commit(world).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The `cues` row's shared rule.
fn stage_rule(world: &mut World) {
    stage(
        world,
        "cue_rules",
        r#"[{ "on": "damaged", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "hits", "add": 1, "target": "game" } }] }]"#,
    );
}

/// M66's rows' damage types, `bench.a` and `bench.b`.
fn stage_types(world: &mut World) {
    stage(world, "damage_types", r#"{ "bench.a": {}, "bench.b": {} }"#);
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

fn cues_typed(c: &mut Criterion) {
    let mut group = c.benchmark_group("cues_typed");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&CuesPlugin);
        PluginSet::new()
            .with(TagsPlugin::new().with_tags(["bench.a", "bench.b"]))
            .with(HealthPlugin)
            .with(CountersPlugin)
            .build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        stage_types(&mut world);
        stage(
            &mut world,
            "cue_rules",
            r#"[{ "on": "damaged_part", "kind": { "any": ["bench"] }, "do": [{ "counter": { "name": "hits", "add": 1, "target": "game" } }] }]"#,
        );
        // One warm apply gives a typed part's kinds.
        let probe = world.spawn_with((Health::new(1e9),));
        DamageRequest::from_parts(
            probe,
            [DamagePart::new(1.0).with_tags(Tags::new(["bench.a"]))],
        )
        .apply(&mut world);
        let kinds = world
            .resource::<EventQueue<DamagedPart>>()
            .iter()
            .next()
            .unwrap()
            .kinds;
        world.flush_events();
        world.flush_events();
        let entities: Vec<_> = (0..n)
            .map(|_| {
                world.spawn_with((
                    Health::new(1e9),
                    Cues::new().on("damaged_part", [CueAction::counter_add("hits", 1)]),
                ))
            })
            .collect();
        let part = |entity| DamagedPart::new(entity, 1.0).with_kinds(kinds);
        // One warm step: every entity's own cue and the rule ran.
        let queue = world.resource_mut::<EventQueue<DamagedPart>>();
        for &entity in &entities {
            queue.send(part(entity));
        }
        fixed_step(&mut world, &mut schedule);
        world.flush_events();
        assert_eq!(world.resource::<Counters>().get("hits"), n as i64);
        assert_eq!(world.get::<Counters>(entities[0]).unwrap().get("hits"), 1);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let queue = world.resource_mut::<EventQueue<DamagedPart>>();
                for &entity in &entities {
                    queue.send(part(entity));
                }
                fixed_step(black_box(&mut world), &mut schedule);
                world.flush_events();
            });
        });
    }
    group.finish();
}

/// Where the `i`th of `n` entities stands: a square of `n` cells 32 px
/// apart, so the density is the same at every count.
fn spread(i: usize, n: usize) -> Vec2 {
    let side = (n as f32).sqrt().ceil() as usize;
    Vec2::new((i % side) as f32, (i / side) as f32) * 32.0
}

/// A fixed step, then the event flush, so a row's sent events stay one
/// step's.
fn fixed_step_and_flush(world: &mut World, schedule: &mut Schedule) {
    fixed_step(world, schedule);
    world.flush_events();
}

fn hit_grid(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_grid");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&HitPlugin);
        for i in 0..n {
            // A step's motion: no snapshot runs here, so the sweep stays.
            let at = spread(i, n);
            world.spawn_with((
                Hurtbox::new(HitShape::circle(6.0)),
                Position(at),
                PrevPosition(at - Vec2::new(4.0, 2.0)),
            ));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn hitbox_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("hitbox_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&HitPlugin);
        for i in 0..n {
            let at = spread(i, n);
            world.spawn_with((Hurtbox::new(HitShape::circle(6.0)), Position(at)));
            world.spawn_with((Hitbox::new(HitShape::circle(4.0), 1.0), Position(at)));
        }
        // One warm step: each hitbox hits its hurtbox once; after it, the
        // steady state of a live activation.
        fixed_step_and_flush(&mut world, &mut schedule);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step_and_flush(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn contact_damage(c: &mut Criterion) {
    let mut group = c.benchmark_group("contact_damage");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&ContactDamagePlugin);
        for i in 0..n {
            let at = spread(i, n);
            world.spawn_with((Hurtbox::new(HitShape::circle(4.0)), Position(at)));
            let aura = ContactDamage::new(1.0).with_shape(HitShape::circle(8.0), Vec2::ZERO);
            world.spawn_with((aura, Position(at)));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step_and_flush(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn projectile_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("projectile_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&ProjectilePlugin);
        // One static body beside them, indexed by one physics step, so the
        // wall queries run against an index.
        world.spawn_with(RigidBodyBundle::r#static(
            Position::new(-500.0, -500.0),
            Collider::aabb(Vec2::splat(50.0)),
        ));
        tungsten_core::physics::physics_step(&mut world);
        for i in 0..n {
            let at = spread(i, n);
            world.spawn_with((
                Hitbox::new(HitShape::circle(2.0), 1.0),
                Projectile::new(),
                Position(at),
                PrevPosition(at - Vec2::new(5.0, 0.0)),
            ));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step_and_flush(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn pickup_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("pickup_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&PickupPlugin);
        for i in 0..n {
            let at = spread(i, n);
            world.spawn_with((
                Pickup::new(HitShape::circle(4.0)),
                Position(at),
                PrevPosition(at - Vec2::new(2.0, 0.0)),
            ));
            // A collector in every tenth cell, 16 px off: none reached.
            if i % 10 == 0 {
                world.spawn_with((
                    Hurtbox::new(HitShape::circle(4.0)),
                    Position(at + Vec2::splat(16.0)),
                ));
            }
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step_and_flush(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn pickup_magnet(c: &mut Criterion) {
    let mut group = c.benchmark_group("pickup_magnet");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&PickupPlugin);
        for i in 0..n {
            let at = spread(i, n);
            // So slow it barely moves over a sitting: the same work each step.
            world.spawn_with((
                Pickup::new(HitShape::circle(1.0)).with_magnet(1e-3),
                Position(at),
                PrevPosition(at),
            ));
            // One collector per ten pickups, its reach over its neighbours.
            if i % 10 == 0 {
                world.spawn_with((
                    Hurtbox::new(HitShape::circle(1.0)),
                    Position(at + Vec2::splat(16.0)),
                    Stats::new().with(Stats::PICKUP_RADIUS, 120.0),
                ));
            }
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step_and_flush(black_box(&mut world), &mut schedule));
        });
    }
    group.finish();
}

fn damage_resolve(c: &mut Criterion) {
    let mut group = c.benchmark_group("damage_resolve");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&HealthPlugin);
        let fire = Tags::new(["damage.fire"]);
        let targets: Vec<_> = (0..n)
            .map(|_| {
                world.spawn_with((
                    Health::new(1e12),
                    Resist::new().with_factor("damage.fire", 0.5),
                    Shield::new(1e9).with_recharge(1e9, 0.0),
                ))
            })
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                for &target in targets.iter().rev() {
                    DamageRequest::new(target, 2.0)
                        .with_tags(fire.clone())
                        .queue(&mut world);
                }
                fixed_step_and_flush(black_box(&mut world), &mut schedule);
            });
        });
    }
    group.finish();
}

fn damage_resolve_typed(c: &mut Criterion) {
    let mut group = c.benchmark_group("damage_resolve_typed");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&HealthPlugin);
        TagsPlugin::new()
            .with_tags(["bench.a", "bench.b"])
            .build(&mut schedule, &mut world);
        schedule.resolve().unwrap();
        stage_types(&mut world);
        let targets: Vec<_> = (0..n)
            .map(|_| {
                let mut resist = Resist::new()
                    .with_factor("bench.a", 0.5)
                    .with_factor("bench.b", 0.8);
                resist.add_modifier("bench.b", "bench", Modifier::Mul(0.5));
                world.spawn_with((
                    Health::new(1e12),
                    resist,
                    Shield::new(1e9).with_recharge(1e9, 0.0),
                ))
            })
            .collect();
        let (a, b) = (Tags::new(["bench.a"]), Tags::new(["bench.b"]));
        let conversion = Conversion::new().with_rule("bench.a", "bench.b", 0.5);
        let penetration = Penetration::new().with_points("bench.b", 0.25);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bencher, _| {
            bencher.iter(|| {
                for &target in targets.iter().rev() {
                    DamageRequest::from_parts(
                        target,
                        [
                            DamagePart::new(2.0).with_tags(a.clone()),
                            DamagePart::new(1.0).with_tags(b.clone()),
                        ],
                    )
                    .with_conversion(conversion.clone())
                    .with_penetration(penetration.clone())
                    .queue(&mut world);
                }
                fixed_step_and_flush(black_box(&mut world), &mut schedule);
            });
        });
    }
    group.finish();
}

fn shield_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("shield_tick");
    for n in SIZES {
        let (mut world, mut schedule) = rig(&HealthPlugin);
        for _ in 0..n {
            // Drained far below its max: it recharges for a whole sitting.
            let shield: Shield = serde_json::from_value(serde_json::json!({
                "current": 0.0, "max": 1e9, "recharge": 1.0, "delay": 0.0
            }))
            .unwrap();
            world.spawn_with((shield,));
        }
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| fixed_step(black_box(&mut world), &mut schedule));
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
    cues,
    cues_typed,
    linear_motion,
    oscillator,
    oscillator_present,
    follow,
    orbit,
    steering,
    knockback,
    speed_limit,
    speed_limit_settle,
    wrap,
    stay_in_view,
    aim,
    facing,
    character_animator,
    depth_sort,
    hit_grid,
    hitbox_tick,
    contact_damage,
    projectile_tick,
    pickup_tick,
    pickup_magnet,
    damage_resolve,
    damage_resolve_typed,
    shield_tick
);
criterion_main!(kit);
