//! `particles`: particle emit, tick and count refresh, plus animation
//! playback. `emitters` emitters draw on `configs` generated
//! `ParticleConfig`s spanning every emission kind (continuous, burst,
//! pulse), every velocity model (cone, radial, vector), gravity and drag,
//! scale, color and alpha curves of 2-6 points, both blend modes and
//! lifetimes of 0.3-2.5 s. Each config emits about 150 live particles per
//! emitter at `rate` 1, so the default's 280 emitters target about 42,000.
//! The emitters move on Lissajous paths and `ParticleBudget` is raised to
//! `budget`.
//! `animated` sprites play `clips` generated 8-frame clips; `animate_sprites`
//! walks `query2_mut::<AnimationState, Sprite>`, calls `advance` and
//! restarts finished one-shots.
//!
//! The particle stage is three named `post_update` systems since M38
//! (`particle_count_refresh`, `particle_emit`, `particle_tick`, `D-133`),
//! timed like any other; the row owns those three plus the `animate_sprites`
//! row, and `unattributed` holds only the event flush.
//! Particles spawn through the `CommandBuffer` and everything draws through
//! the default extract: `flush` belongs to `churn`, the extract to `gpu`,
//! and both are reported only.
//!
//! The engine latches a burst after it fires (`continuous_accum` 1), whatever
//! `once` says, so `rearm_bursts` clears the latch every burst period.

use std::f32::consts::TAU;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec2;
use serde_json::{Value as Json, json};
use tungsten::core::assets::AnimationFrame;
use tungsten::core::{
    AnimationData, AnimationRegistry, AnimationState, AssetRegistry, BlendMode, CameraState, Curve,
    EmissionKind, Entity, FilterMode, InitialVelocity, Particle, ParticleBudget, ParticleConfig,
    ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, ParticleRender, Pcg32, Range,
    Sprite, Time, Transform, Visibility, World, splitmix64,
};
use tungsten::render::Renderer;
use tungsten::{App, extract_sprites_default};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::r#gen::{self, Cell};
use crate::knobs::{self, Bench, BenchConfig, Guard, Knob, Preset, Row, Value};
use crate::view::VIEWPORT;

pub(crate) static BENCH: Bench = Bench {
    name: "particles",
    workload_version: 1,
    warmup: 180,
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
    Knob::int("emitters", 280, 0, 20_000).scaled(),
    Knob::int("configs", 12, 1, 64).note("Generated particle configs, assigned round-robin"),
    Knob::float("rate", 1.0, 0.1, 10.0)
        .note("Emission multiplier; each emitter targets about 150 live particles at 1"),
    Knob::int("budget", 100_000, 1_000, 5_000_000)
        .scaled()
        .note("ParticleBudget::global_cap"),
    Knob::int("animated", 8_000, 0, 1_000_000).scaled(),
    Knob::int("clips", 8, 1, 32).note("8-frame clips; odd clips are one-shots the bench restarts"),
    Knob::int("anim_fps", 12, 1, 60),
    Knob::choice("motion", "on", &["on", "off"]).note("Emitter movement"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[("emitters", Value::Int(8)), ("animated", Value::Int(500))],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

const P50_P95: &[&str] = &["p50", "p95"];

const ROWS: &[Row] = &[Row {
    name: "particles",
    preset: "default",
    owned: &[
        ("system.particle_count_refresh", P50_P95),
        ("system.particle_emit", P50_P95),
        ("system.particle_tick", P50_P95),
        ("system.animate_sprites", P50_P95),
    ],
    guards: &[Guard::CounterBand {
        counter: "live",
        tolerance: 0.1,
    }],
    counters: &["live", "emitters", "animated", "frame_changes"],
    bottleneck: "system.particle_tick",
    key_knobs: &["emitters", "animated"],
    note: "The particle stage is three named `post_update` systems since M38 (`D-133`): the row \
           owns `particle_count_refresh`, `particle_emit`, `particle_tick` and `animate_sprites`; \
           `unattributed` holds only the event flush",
}];

/// Live particles each emitter targets at `rate` 1.
const LIVE_PER_EMITTER: f64 = 150.0;
const FRAMES_PER_CLIP: u32 = 8;
const PARTICLE_PX: u32 = 16;
const FRAME_PX: u32 = 16;
/// Animated sprites draw at 12 px.
const ANIMATED_SCALE: f32 = 0.75;
const PARTICLE_SPRITES: [&str; 2] = ["bench_particle_dot", "bench_particle_spark"];

fn row(_cfg: &BenchConfig) -> &'static str {
    "particles"
}

/// Design target: every emitter holds about `150 * rate` particles, within
/// the global budget.
fn live_model(cfg: &BenchConfig) -> u64 {
    let wanted = cfg.int("emitters") as f64 * LIVE_PER_EMITTER * cfg.float("rate");
    (wanted.round() as u64).min(cfg.int("budget") as u64)
}

fn frame_ms(cfg: &BenchConfig) -> u32 {
    (1000.0 / cfg.int("anim_fps") as f64).round() as u32
}

fn derived(cfg: &BenchConfig) -> Json {
    json!({
        "live_model": live_model(cfg),
        "frame_ms": frame_ms(cfg),
        "view": [VIEWPORT.x, VIEWPORT.y],
    })
}

fn clip_id(clip: u32) -> String {
    format!("bench_clip_{clip}")
}

fn frame_id(clip: u32, frame: u32) -> String {
    format!("bench_anim_{clip}_{frame}")
}

/// Curve with `points` evenly spaced points from `value(0)` to `value(1)`.
fn curve<V: Copy>(points: u32, value: impl Fn(f32) -> V) -> Curve<V> {
    Curve {
        points: (0..points)
            .map(|index| {
                let t = index as f32 / (points - 1) as f32;
                (t, value(t))
            })
            .collect(),
    }
}

/// Config `index`, scaled so one emitter holds about `target` particles.
/// Returns the config and, for bursts, the re-arm period in seconds.
fn particle_config(index: u32, target: f64) -> (ParticleConfig, Option<f32>) {
    let min_life = 0.3 + 0.2 * (index % 6) as f32;
    let max_life = (min_life + 0.3 + 0.3 * (index % 4) as f32).min(2.5);
    let mean_life = f64::from(min_life + max_life) * 0.5;
    let per_second = target / mean_life;
    let (emission, burst_period) = match index % 3 {
        0 => (
            EmissionKind::Continuous {
                rate_hz: per_second as f32,
            },
            None,
        ),
        1 => {
            let period = 0.4 + 0.1 * (index % 4) as f32;
            let count = (per_second * f64::from(period)).round().max(1.0) as u32;
            (EmissionKind::Burst { count, once: false }, Some(period))
        }
        _ => {
            let interval = 0.2 + 0.05 * (index % 4) as f32;
            let count_per_pulse = (per_second * f64::from(interval)).round().max(1.0) as u32;
            (
                EmissionKind::Pulse {
                    count_per_pulse,
                    interval_sec: interval,
                    total_pulses: None,
                },
                None,
            )
        }
    };
    let speed = Range {
        min: 40.0 + 10.0 * (index % 5) as f32,
        max: 120.0 + 20.0 * (index % 5) as f32,
    };
    let initial_velocity = match (index / 3) % 3 {
        0 => InitialVelocity::Cone {
            direction: [0.0, -1.0],
            spread_deg: 30.0 + 15.0 * (index % 4) as f32,
            speed,
        },
        1 => InitialVelocity::Radial { speed },
        _ => InitialVelocity::Vector {
            direction: [1.0, -0.4],
            speed,
        },
    };
    let hue = index as f32 * 0.37;
    let config = ParticleConfig {
        sprite: PARTICLE_SPRITES[(index % 2) as usize].to_string(),
        render: ParticleRender::Quad,
        max_alive: (target * 4.0).ceil() as u32,
        seed: None,
        blend: if index.is_multiple_of(2) {
            BlendMode::Alpha
        } else {
            BlendMode::Premultiplied
        },
        emission,
        lifetime: Range {
            min: min_life,
            max: max_life,
        },
        initial_velocity,
        gravity: [0.0, [0.0, 120.0, 300.0, -80.0][(index % 4) as usize]],
        drag_per_sec: [0.0, 0.5, 1.5][((index / 2) % 3) as usize],
        angular_velocity: Range {
            min: -2.0,
            max: 2.0,
        },
        start_scale: Range { min: 0.4, max: 0.9 },
        scale_over_life: Some(curve(2 + index % 5, |t| 1.0 + 0.6 * (t * 3.1).sin())),
        color_over_life: Some(curve(2 + (index + 1) % 5, |t| {
            let phase = hue + t * 2.0;
            [
                0.6 + 0.4 * phase.sin(),
                0.6 + 0.4 * (phase + 2.1).sin(),
                0.6 + 0.4 * (phase + 4.2).sin(),
                1.0,
            ]
        })),
        alpha_over_life: Some(curve(2 + (index + 2) % 5, |t| (1.0 - t) * (0.6 + 0.4 * t))),
        tint: [1.0, 1.0, 1.0, 1.0],
    };
    if let Err(err) = config.validate() {
        panic!("generated particle config {index} is invalid: {err}");
    }
    (config, burst_period)
}

/// An emitter's Lissajous path around its grid cell.
#[derive(Debug, Clone, Copy)]
struct EmitterPath {
    center: Vec2,
    amplitude: Vec2,
    rate: Vec2,
    phase: Vec2,
}

/// Burst emitters: `rearm_bursts` clears the engine's latch every `period`.
#[derive(Debug, Clone, Copy)]
struct BurstRearm {
    period: f32,
    timer: f32,
}

/// Runtime state.
#[derive(Debug)]
struct ParticlesState {
    elapsed: f32,
    motion: bool,
    emitters: u32,
    animated: u32,
}

/// Cell centers of an `count`-cell grid over the viewport, row-major, and
/// the cell size.
fn grid(count: u32) -> (Vec<Vec2>, f32) {
    if count == 0 {
        return (Vec::new(), 0.0);
    }
    let columns = ((count as f32 * VIEWPORT.x / VIEWPORT.y).sqrt().ceil() as u32).max(1);
    let rows = count.div_ceil(columns);
    let cell = (VIEWPORT.x / columns as f32).min(VIEWPORT.y / rows as f32);
    let centers = (0..count)
        .map(|index| {
            Vec2::new(
                ((index % columns) as f32 + 0.5) * cell,
                ((index / columns) as f32 + 0.5) * cell,
            )
        })
        .collect();
    (centers, cell)
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let emitters = cfg.int("emitters") as u32;
    let configs = cfg.int("configs") as u32;
    let animated = cfg.int("animated") as u32;
    let clips = cfg.int("clips") as u32;
    let target = LIVE_PER_EMITTER * cfg.float("rate");
    let mut rng = Pcg32::seeded(cfg.seed());
    {
        let world = app.world_mut();
        world.insert_resource(ParticleBudget {
            global_cap: cfg.int("budget") as u32,
        });
        let registry = world
            .get_resource_mut::<ParticleConfigRegistry>()
            .expect("ParticleConfigRegistry resource missing");
        let configs: Vec<_> = (0..configs)
            .map(|index| {
                let (config, burst) = particle_config(index, target);
                let name = format!("bench_particles_{index}");
                let path = PathBuf::from(format!("__generated__/{name}.json"));
                (registry.register(name, path, Arc::new(config)), burst)
            })
            .collect();

        let (centers, cell) = grid(emitters);
        for (index, center) in centers.into_iter().enumerate() {
            let (config, burst) = configs[index % configs.len()];
            let entity = world.spawn();
            world.insert(entity, Transform::from_position(center));
            let seed = splitmix64(cfg.seed() ^ (index as u64 + 1));
            world.insert(entity, ParticleEmitter::with_seed(config, seed));
            // Stagger pulses and bursts so emitters don't fire in lockstep.
            let offset = rng.next_f32_unit();
            world.insert(
                entity,
                ParticleEmitterState {
                    pulse_timer: offset * 0.2,
                    ..ParticleEmitterState::default()
                },
            );
            if let Some(period) = burst {
                world.insert(
                    entity,
                    BurstRearm {
                        period,
                        timer: offset * period,
                    },
                );
            }
            let reach = (cell * 0.5).min(160.0);
            world.insert(
                entity,
                EmitterPath {
                    center,
                    amplitude: Vec2::new(rng.next_range(0.3, 1.0), rng.next_range(0.3, 1.0))
                        * reach,
                    rate: Vec2::new(rng.next_range(0.5, 1.5), rng.next_range(0.5, 1.5)),
                    phase: Vec2::new(rng.next_range(0.0, TAU), rng.next_range(0.0, TAU)),
                },
            );
        }

        let mut animations = AnimationRegistry::new();
        let duration_ms = frame_ms(cfg);
        let sprites = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        let frame_ids: Vec<Vec<_>> = (0..clips)
            .map(|clip| {
                (0..FRAMES_PER_CLIP)
                    .map(|frame| sprites.intern_sprite(&frame_id(clip, frame)))
                    .collect()
            })
            .collect();
        for clip in 0..clips {
            animations.insert(
                clip_id(clip),
                AnimationData {
                    looping: clip.is_multiple_of(2),
                    frames: frame_ids[clip as usize]
                        .iter()
                        .map(|&sprite| AnimationFrame {
                            sprite,
                            duration_ms,
                        })
                        .collect(),
                },
            );
        }
        world.insert_resource(animations);
        let (centers, _) = grid(animated);
        let half = FRAME_PX as f32 * ANIMATED_SCALE * 0.5;
        for (index, center) in centers.into_iter().enumerate() {
            let clip = index as u32 % clips;
            let frame = rng.next_u32() % FRAMES_PER_CLIP;
            let mut state = AnimationState::new(clip_id(clip));
            state.frame_index = frame as usize;
            state.accumulated_ms = rng.next_f32_unit() * duration_ms as f32;
            let mut sprite = Sprite::new(frame_ids[clip as usize][frame as usize]);
            sprite.z_order = -1;
            let entity = world.spawn();
            world.insert(
                entity,
                Transform {
                    position: center - Vec2::splat(half),
                    rotation: 0.0,
                    scale: Vec2::splat(ANIMATED_SCALE),
                },
            );
            world.insert(entity, sprite);
            world.insert(entity, Visibility::default());
            world.insert(entity, state);
        }

        if let Some(camera) = world.get_resource_mut::<CameraState>() {
            *camera = CameraState::new();
        }
        world.insert_resource(ParticlesState {
            elapsed: 0.0,
            motion: cfg.choice("motion") == "on",
            emitters,
            animated,
        });
        world.insert_resource(BenchCounters::new(ParticleCounts::default()));
    }
    app.on_startup(move |world, renderer| register_textures(world, renderer, clips));
    app.add_system_named("bench_counters", bench_counters_system::<ParticleCounts>);
    app.add_system_named("move_emitters", move_emitters);
    app.add_system_named("rearm_bursts", rearm_bursts);
    app.add_system_named("animate_sprites", animate_sprites);
    app.set_extract_sprites(extract_sprites_default);
}

/// Startup hook: particle sprites and animation frames, one atlas page each.
fn register_textures(world: &mut World, renderer: &mut Renderer, clips: u32) {
    let particles = [
        Cell {
            id: PARTICLE_SPRITES[0].to_string(),
            rgba: r#gen::soft_dot_rgba(PARTICLE_PX),
        },
        Cell {
            id: PARTICLE_SPRITES[1].to_string(),
            rgba: r#gen::spark_rgba(PARTICLE_PX),
        },
    ];
    r#gen::register_atlas(world, renderer, FilterMode::Linear, PARTICLE_PX, &particles);
    let frames: Vec<Cell> = (0..clips)
        .flat_map(|clip| {
            (0..FRAMES_PER_CLIP).map(move |frame| Cell {
                id: frame_id(clip, frame),
                rgba: r#gen::anim_frame_rgba(clip, frame, FRAMES_PER_CLIP, FRAME_PX),
            })
        })
        .collect();
    r#gen::register_atlas(world, renderer, FilterMode::Linear, FRAME_PX, &frames);
}

fn dt_seconds(world: &World) -> f32 {
    world.get_resource::<Time>().map_or(0.0, Time::delta)
}

/// Moves each emitter along its Lissajous path (`motion=on`).
fn move_emitters(world: &mut World) {
    let dt = dt_seconds(world);
    let Some(state) = world.get_resource_mut::<ParticlesState>() else {
        return;
    };
    state.elapsed += dt;
    if !state.motion {
        return;
    }
    let elapsed = state.elapsed;
    for (_, transform, path) in world.query_mut::<(Entity, &mut Transform, &mut EmitterPath)>() {
        let angle = path.rate * elapsed + path.phase;
        transform.position = path.center + path.amplitude * Vec2::new(angle.x.sin(), angle.y.cos());
    }
}

/// Clears each burst emitter's fired latch once per period.
fn rearm_bursts(world: &mut World) {
    let dt = dt_seconds(world);
    for (_, emitter, rearm) in
        world.query_mut::<(Entity, &mut ParticleEmitterState, &mut BurstRearm)>()
    {
        rearm.timer += dt;
        if rearm.timer >= rearm.period {
            rearm.timer -= rearm.period;
            emitter.continuous_accum = 0.0;
        }
    }
}

/// Advances every animated sprite; a finished one-shot restarts at frame 0.
fn animate_sprites(world: &mut World) {
    let dt_ms = dt_seconds(world) * 1000.0;
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    let mut changes = 0;
    for (_, state, sprite) in world.query_mut::<(Entity, &mut AnimationState, &mut Sprite)>() {
        if state.finished {
            state.frame_index = 0;
            state.accumulated_ms = 0.0;
            state.finished = false;
            if let Some(first) = state.current_sprite(&registry) {
                sprite.asset_id = first;
                changes += 1;
            }
        } else if let Some(next) = state.advance(dt_ms, &registry) {
            sprite.asset_id = next;
            changes += 1;
        }
    }
    world.insert_resource(registry);
    if let Some(counters) = world.get_resource_mut::<BenchCounters<ParticleCounts>>() {
        counters.counts.frame_changes += changes;
    }
}

/// `bench:` counters; `live` counts particle entities after the frame's flush.
#[derive(Debug, Default)]
struct ParticleCounts {
    frame_changes: u32,
}

impl FrameCounters for ParticleCounts {
    fn write(&self, world: &World, line: &mut String) {
        let Some(state) = world.get_resource::<ParticlesState>() else {
            return;
        };
        let _ = write!(
            line,
            " live={} emitters={} animated={} frame_changes={}",
            world.query::<(Entity, &Particle)>().count(),
            state.emitters,
            state.animated,
            self.frame_changes
        );
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}
