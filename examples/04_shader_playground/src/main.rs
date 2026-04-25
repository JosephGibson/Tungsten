//! Example 04: shader playground (M26).
//!
//! Renders a flock of bouncing sprites (varied speed, rotation, scale, and
//! tint) with the 17 stock post-stack effects visible one at a time. Keys
//! (see workspace `input.json`):
//!
//!   - `N` / `]`  → next effect
//!   - `B` / `[`  → previous effect
//!   - `C` / `Backspace` → clear stack (M25-byte-identical output)
//!
//! Env `TUNGSTEN_POST_STACK_FIXTURE={all|retro_arcade|dreamy|glitch_boss|empty}`
//! preloads a fixed stack and **disables** the cycle — the fixtures are for
//! the smoke matrix, not interactive inspection.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use glam::Vec2;
use tungsten::core::{
    ActionMap, BlendMode, CommandBuffer, Config, Curve, DeltaTime, EmissionKind, InitialVelocity,
    InputState, ParticleConfig, Pcg32, Range, Sprite, Transform, Visibility, World,
};
use tungsten::particles::spawn_particle_via;
use tungsten::{render::TextSection, request_post_aa, App, PostAaState};
use tungsten_core::config::PostAaMode;
use tungsten_core::post::{
    ColorAdjustParams, CrtParams, DissolveParams, DitherParams, FadeParams, FilmGrainParams,
    FogParams, GodRaysParams, LutParams, PixelOutlineParams, PostPass, PostStack, ToneMonoParams,
    TonemapParams, VignetteParams, WipeRadialParams,
};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/04_shader_playground/assets/manifest.json";
const QUAD_ID: &str = "ex04_quad";
/// Source quad is 16x16 (see `assets/quad.png`); logical size = `QUAD_TEXELS * scale`.
const QUAD_TEXELS: f32 = 16.0;
const WINDOW_W: f32 = 1280.0;
const WINDOW_H: f32 = 720.0;
/// Reflection bounds run past the window edges so the flock roams a roomier
/// arena while the window config stays untouched.
const ARENA_SCALE: f32 = 1.75;
const ARENA_W: f32 = WINDOW_W * ARENA_SCALE;
const ARENA_H: f32 = WINDOW_H * ARENA_SCALE;
/// Wall/pair burst counts — tuned for visible pop without blowing the cap.
const WALL_BURST_COUNT: u32 = 14;
const PAIR_BURST_COUNT: u32 = 22;

/// Per-entity bounce state. Lives alongside `Transform` so each sprite moves,
/// spins, and reflects off window edges independently.
#[derive(Debug, Clone, Copy)]
struct Bouncer {
    velocity: Vec2,
    angular_velocity: f32,
    /// Logical side length used as the reflection bounding box.
    size: f32,
}

/// Tracks the cycle cursor so N/B step deterministically. `None` = empty.
/// `Some(i)` points at the roster index the stack currently holds.
#[derive(Debug, Default, Clone, Copy)]
struct CycleCursor {
    index: Option<usize>,
    /// When set, a fixture preloaded the stack; cycle input is disabled.
    fixture_lock: bool,
}

/// Shared burst templates; `Arc` so per-spawn clones stay cheap.
#[derive(Clone)]
struct SparkRecipes {
    wall: Arc<ParticleConfig>,
    pair: Arc<ParticleConfig>,
}

/// Dedicated RNG for burst jitter so bouncer motion stays deterministic.
struct SparkRng(Pcg32);

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Shader Playground — M26".to_string();

    // M27: optional `TUNGSTEN_POST_AA_FIXTURE` env override before `App::new`
    // so the renderer wakes up with the requested SMAA preset already active.
    if let Ok(fixture) = std::env::var("TUNGSTEN_POST_AA_FIXTURE") {
        match PostAaMode::from_str(&fixture) {
            Ok(mode) => config.render.post_aa = mode,
            Err(_) => {
                anyhow::bail!(
                    "invalid TUNGSTEN_POST_AA_FIXTURE='{fixture}': expected one of off, smaa_low, smaa_medium, smaa_high, smaa_ultra"
                );
            }
        }
    }

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);

    {
        let world = app.world_mut();
        world.insert_resource(CycleCursor::default());
        world.insert_resource(SparkRecipes {
            wall: wall_spark_config(),
            pair: pair_spark_config(),
        });
        world.insert_resource(SparkRng(Pcg32::seeded(0xEF04_5A9C_A2D1_7B03)));
    }

    app.on_startup(|world, _renderer| {
        spawn_bouncers(world);

        let fixture = std::env::var("TUNGSTEN_POST_STACK_FIXTURE").unwrap_or_default();
        if !fixture.is_empty() && fixture != "empty" {
            if let Some(stack) = world.get_resource_mut::<PostStack>() {
                match fixture.as_str() {
                    "all" => push_every_effect(stack),
                    "retro_arcade" => push_retro_arcade(stack),
                    "dreamy" => push_dreamy(stack),
                    "glitch_boss" => push_glitch_boss(stack),
                    _ => {}
                }
            }
            if let Some(cursor) = world.get_resource_mut::<CycleCursor>() {
                cursor.fixture_lock = true;
            }
        }
    });

    app.add_system_named("playground_bounce", bounce_system);
    app.add_system_named("playground_collisions", pair_collision_system);
    app.add_system_named("playground_cycle_input", cycle_input_system);
    app.add_system_named("playground_post_aa_input", post_aa_input_system);
    app.set_extract_text(playground_text);

    app.run()
}

const POST_AA_CYCLE: &[PostAaMode] = &[
    PostAaMode::Off,
    PostAaMode::SmaaLow,
    PostAaMode::SmaaMedium,
    PostAaMode::SmaaHigh,
    PostAaMode::SmaaUltra,
];

fn next_post_aa(current: PostAaMode) -> PostAaMode {
    let idx = POST_AA_CYCLE
        .iter()
        .position(|&m| m == current)
        .unwrap_or(0);
    POST_AA_CYCLE[(idx + 1) % POST_AA_CYCLE.len()]
}

fn post_aa_input_system(world: &mut World) {
    let (cycle, off, low, medium, high, ultra) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        (
            actions.just_pressed(input, "playground_cycle_post_aa"),
            actions.just_pressed(input, "playground_post_aa_off"),
            actions.just_pressed(input, "playground_post_aa_low"),
            actions.just_pressed(input, "playground_post_aa_medium"),
            actions.just_pressed(input, "playground_post_aa_high"),
            actions.just_pressed(input, "playground_post_aa_ultra"),
        )
    };
    let current = world
        .get_resource::<PostAaState>()
        .map_or(PostAaMode::Off, |s| s.mode);

    let target = if cycle {
        Some(next_post_aa(current))
    } else if off {
        Some(PostAaMode::Off)
    } else if low {
        Some(PostAaMode::SmaaLow)
    } else if medium {
        Some(PostAaMode::SmaaMedium)
    } else if high {
        Some(PostAaMode::SmaaHigh)
    } else if ultra {
        Some(PostAaMode::SmaaUltra)
    } else {
        None
    };

    if let Some(mode) = target {
        if mode != current {
            request_post_aa(world, mode);
        }
    }
}

#[derive(Clone, Copy)]
struct BouncerSpec {
    position: Vec2,
    velocity: Vec2,
    rotation: f32,
    scale_mul: f32,
    angular_velocity: f32,
    color: [u8; 4],
}

fn spawn_bouncers(world: &mut World) {
    let specs: &[BouncerSpec] = &[
        BouncerSpec {
            position: Vec2::new(200.0, 160.0),
            velocity: Vec2::new(220.0, 160.0),
            rotation: 0.0,
            scale_mul: 6.0,
            angular_velocity: 1.6,
            color: [255, 255, 255, 255],
        },
        BouncerSpec {
            position: Vec2::new(480.0, 300.0),
            velocity: Vec2::new(-320.0, 140.0),
            rotation: 0.7,
            scale_mul: 4.0,
            angular_velocity: -2.4,
            color: [255, 110, 110, 255],
        },
        BouncerSpec {
            position: Vec2::new(780.0, 220.0),
            velocity: Vec2::new(260.0, -260.0),
            rotation: 1.2,
            scale_mul: 8.0,
            angular_velocity: 0.9,
            color: [110, 255, 150, 255],
        },
        BouncerSpec {
            position: Vec2::new(960.0, 520.0),
            velocity: Vec2::new(-180.0, -200.0),
            rotation: 2.1,
            scale_mul: 3.0,
            angular_velocity: -1.2,
            color: [120, 180, 255, 255],
        },
        BouncerSpec {
            position: Vec2::new(540.0, 600.0),
            velocity: Vec2::new(380.0, 280.0),
            rotation: 0.3,
            scale_mul: 5.5,
            angular_velocity: 2.6,
            color: [240, 210, 110, 255],
        },
        BouncerSpec {
            position: Vec2::new(1080.0, 140.0),
            velocity: Vec2::new(-140.0, 340.0),
            rotation: 1.7,
            scale_mul: 7.0,
            angular_velocity: -0.6,
            color: [255, 140, 220, 255],
        },
    ];
    for &BouncerSpec {
        position,
        velocity,
        rotation,
        scale_mul,
        angular_velocity,
        color,
    } in specs
    {
        let entity = world.spawn();
        world.insert(
            entity,
            Transform {
                position,
                rotation,
                scale: Vec2::splat(scale_mul),
            },
        );
        let mut sprite = Sprite::new(QUAD_ID);
        sprite.color = color;
        world.insert(entity, sprite);
        world.insert(entity, Visibility::default());
        world.insert(
            entity,
            Bouncer {
                velocity,
                angular_velocity,
                size: QUAD_TEXELS * scale_mul,
            },
        );
    }
}

fn bounce_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(1.0 / 60.0, DeltaTime::seconds);

    // Outward normals for each axis the entity crossed this tick. Collected
    // first so the burst spawn pass runs after every world.get_mut release.
    let mut contacts: Vec<(Vec2, Vec2)> = Vec::new();

    for entity in world.query2_entities::<Transform, Bouncer>() {
        let (mut velocity, angular_velocity, size) = {
            let b = world.get::<Bouncer>(entity).copied().unwrap();
            (b.velocity, b.angular_velocity, b.size)
        };

        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position += velocity * dt;
            t.rotation += angular_velocity * dt;

            let half = size * 0.5;
            if t.position.x - half < 0.0 {
                t.position.x = half;
                velocity.x = velocity.x.abs();
                contacts.push((Vec2::new(0.0, t.position.y), Vec2::new(1.0, 0.0)));
            } else if t.position.x + half > ARENA_W {
                t.position.x = ARENA_W - half;
                velocity.x = -velocity.x.abs();
                contacts.push((Vec2::new(ARENA_W, t.position.y), Vec2::new(-1.0, 0.0)));
            }
            if t.position.y - half < 0.0 {
                t.position.y = half;
                velocity.y = velocity.y.abs();
                contacts.push((Vec2::new(t.position.x, 0.0), Vec2::new(0.0, 1.0)));
            } else if t.position.y + half > ARENA_H {
                t.position.y = ARENA_H - half;
                velocity.y = -velocity.y.abs();
                contacts.push((Vec2::new(t.position.x, ARENA_H), Vec2::new(0.0, -1.0)));
            }
        }

        if let Some(b) = world.get_mut::<Bouncer>(entity) {
            b.velocity = velocity;
        }
    }

    if contacts.is_empty() {
        return;
    }
    let Some(recipe) = world.get_resource::<SparkRecipes>().map(|r| r.wall.clone()) else {
        return;
    };
    with_spawn_ctx(world, |buf, rng| {
        for (point, normal) in contacts {
            emit_cone_burst(
                buf,
                rng,
                &recipe,
                point,
                normal,
                WALL_BURST_COUNT,
                (180.0, 420.0),
                90.0,
                (0.25, 0.55),
                (0.3, 0.55),
            );
        }
    });
}

/// Pair collisions as AABB (`Bouncer.size`), resolved by swapping the velocity
/// component along the shallowest overlap axis and separating along the same.
fn pair_collision_system(world: &mut World) {
    let entities = world.query2_entities::<Transform, Bouncer>();
    let mut snapshots: Vec<(usize, Vec2, f32, Vec2)> = entities
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let pos = world.get::<Transform>(*e)?.position;
            let b = world.get::<Bouncer>(*e)?;
            Some((i, pos, b.size, b.velocity))
        })
        .collect();

    let mut contacts: Vec<Vec2> = Vec::new();
    for i in 0..snapshots.len() {
        for j in (i + 1)..snapshots.len() {
            let (_, pa, sa, va) = snapshots[i];
            let (_, pb, sb, vb) = snapshots[j];
            let ha = sa * 0.5;
            let hb = sb * 0.5;
            let delta = pb - pa;
            let overlap_x = (ha + hb) - delta.x.abs();
            let overlap_y = (ha + hb) - delta.y.abs();
            if overlap_x <= 0.0 || overlap_y <= 0.0 {
                continue;
            }

            // Shallowest axis wins — that is the pushout direction.
            if overlap_x < overlap_y {
                let sign = if delta.x >= 0.0 { 1.0 } else { -1.0 };
                let push = overlap_x * 0.5 * sign;
                snapshots[i].1.x -= push;
                snapshots[j].1.x += push;
                // Only swap if they are actually approaching along the normal.
                let rel = vb.x - va.x;
                if rel * sign < 0.0 {
                    let va_new = Vec2::new(vb.x, va.y);
                    let vb_new = Vec2::new(va.x, vb.y);
                    snapshots[i].3 = va_new;
                    snapshots[j].3 = vb_new;
                }
                let contact_x = pa.x + sign * ha;
                let contact_y = 0.5 * (pa.y + pb.y);
                contacts.push(Vec2::new(contact_x, contact_y));
            } else {
                let sign = if delta.y >= 0.0 { 1.0 } else { -1.0 };
                let push = overlap_y * 0.5 * sign;
                snapshots[i].1.y -= push;
                snapshots[j].1.y += push;
                let rel = vb.y - va.y;
                if rel * sign < 0.0 {
                    let va_new = Vec2::new(va.x, vb.y);
                    let vb_new = Vec2::new(vb.x, va.y);
                    snapshots[i].3 = va_new;
                    snapshots[j].3 = vb_new;
                }
                let contact_x = 0.5 * (pa.x + pb.x);
                let contact_y = pa.y + sign * ha;
                contacts.push(Vec2::new(contact_x, contact_y));
            }
        }
    }

    // Write back resolved state.
    for (idx, pos, _, vel) in &snapshots {
        let e = entities[*idx];
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.position = *pos;
        }
        if let Some(b) = world.get_mut::<Bouncer>(e) {
            b.velocity = *vel;
        }
    }

    if contacts.is_empty() {
        return;
    }
    let Some(recipe) = world.get_resource::<SparkRecipes>().map(|r| r.pair.clone()) else {
        return;
    };
    with_spawn_ctx(world, |buf, rng| {
        for point in contacts {
            emit_cone_burst(
                buf,
                rng,
                &recipe,
                point,
                Vec2::ZERO,
                PAIR_BURST_COUNT,
                (160.0, 360.0),
                360.0,
                (0.35, 0.7),
                (0.35, 0.65),
            );
        }
    });
}

/// Temporarily takes the `CommandBuffer` and `SparkRng` out of the world so
/// the closure can mutate both without fighting the ECS borrow checker.
fn with_spawn_ctx(world: &mut World, f: impl FnOnce(&mut CommandBuffer, &mut Pcg32)) {
    let Some(mut buf) = world.remove_resource::<CommandBuffer>() else {
        return;
    };
    let mut rng = world
        .remove_resource::<SparkRng>()
        .unwrap_or(SparkRng(Pcg32::seeded(1)));
    f(&mut buf, &mut rng.0);
    world.insert_resource(rng);
    world.insert_resource(buf);
}

/// Spawn `count` particles fanned around `direction` (pass `Vec2::ZERO` for
/// full-circle radial). `speed`/`life`/`scale` are `(min, max)` ranges.
#[allow(clippy::too_many_arguments)]
fn emit_cone_burst(
    buf: &mut CommandBuffer,
    rng: &mut Pcg32,
    config: &Arc<ParticleConfig>,
    origin: Vec2,
    direction: Vec2,
    count: u32,
    speed: (f32, f32),
    spread_deg: f32,
    life: (f32, f32),
    scale: (f32, f32),
) {
    let use_radial = direction.length_squared() < 1.0e-6;
    let base = if use_radial {
        Vec2::ZERO
    } else {
        direction.normalize_or_zero()
    };
    for _ in 0..count {
        let dir = if use_radial {
            rng.next_unit_vec2()
        } else {
            let spread_rad = spread_deg.to_radians();
            let jitter = rng.next_range(-spread_rad * 0.5, spread_rad * 0.5);
            let (s, c) = jitter.sin_cos();
            Vec2::new(base.x * c - base.y * s, base.x * s + base.y * c)
        };
        let v = dir * rng.next_range(speed.0, speed.1);
        let lifetime = rng.next_range(life.0, life.1);
        let start_scale = rng.next_range(scale.0, scale.1);
        spawn_particle_via(buf, None, config.clone(), origin, v, lifetime, start_scale);
    }
}

/// Cool-blue streak recipe for wall impacts. Only `spawn_particle_via`-read
/// fields matter (sprite, tint, drag, gravity, curves, blend) — emission and
/// initial_velocity are never consulted, so their values are placeholders.
fn wall_spark_config() -> Arc<ParticleConfig> {
    Arc::new(ParticleConfig {
        sprite: QUAD_ID.into(),
        max_alive: 1,
        seed: None,
        blend: BlendMode::Alpha,
        emission: EmissionKind::Burst {
            count: 1,
            once: true,
        },
        lifetime: Range {
            min: 0.25,
            max: 0.55,
        },
        initial_velocity: InitialVelocity::Radial {
            speed: Range::single(1.0),
        },
        gravity: [0.0, 520.0],
        drag_per_sec: 2.4,
        angular_velocity: Range::single(0.0),
        start_scale: Range {
            min: 0.3,
            max: 0.55,
        },
        scale_over_life: Some(Curve {
            points: vec![(0.0, 1.0), (1.0, 0.0)],
        }),
        color_over_life: Some(Curve {
            points: vec![
                (0.0, [1.0, 1.0, 1.0, 1.0]),
                (0.35, [0.7, 0.9, 1.0, 1.0]),
                (1.0, [0.2, 0.4, 0.95, 1.0]),
            ],
        }),
        alpha_over_life: Some(Curve {
            points: vec![(0.0, 0.0), (0.12, 1.0), (1.0, 0.0)],
        }),
        tint: [1.0, 1.0, 1.0, 1.0],
    })
}

/// Hot-orange radial recipe for entity-entity hits — distinct silhouette and
/// palette from the wall sparks so the two are legible side-by-side.
fn pair_spark_config() -> Arc<ParticleConfig> {
    Arc::new(ParticleConfig {
        sprite: QUAD_ID.into(),
        max_alive: 1,
        seed: None,
        blend: BlendMode::Alpha,
        emission: EmissionKind::Burst {
            count: 1,
            once: true,
        },
        lifetime: Range {
            min: 0.35,
            max: 0.7,
        },
        initial_velocity: InitialVelocity::Radial {
            speed: Range::single(1.0),
        },
        gravity: [0.0, 260.0],
        drag_per_sec: 1.8,
        angular_velocity: Range {
            min: -6.0,
            max: 6.0,
        },
        start_scale: Range {
            min: 0.35,
            max: 0.65,
        },
        scale_over_life: Some(Curve {
            points: vec![(0.0, 0.6), (0.2, 1.0), (1.0, 0.0)],
        }),
        color_over_life: Some(Curve {
            points: vec![
                (0.0, [1.0, 1.0, 0.85, 1.0]),
                (0.4, [1.0, 0.55, 0.15, 1.0]),
                (1.0, [0.6, 0.1, 0.0, 1.0]),
            ],
        }),
        alpha_over_life: Some(Curve {
            points: vec![(0.0, 0.0), (0.1, 1.0), (1.0, 0.0)],
        }),
        tint: [1.0, 1.0, 1.0, 1.0],
    })
}

fn cycle_input_system(world: &mut World) {
    let locked = world
        .get_resource::<CycleCursor>()
        .is_some_and(|c| c.fixture_lock);
    if locked {
        return;
    }

    let (next, prev, clear) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        (
            actions.just_pressed(input, "post_next"),
            actions.just_pressed(input, "post_prev"),
            actions.just_pressed(input, "post_clear"),
        )
    };

    if !(next || prev || clear) {
        return;
    }

    let current = world
        .get_resource::<CycleCursor>()
        .and_then(|c| c.index)
        .unwrap_or(0);
    let len = EFFECT_ROSTER.len();
    let new_index: Option<usize> = if clear {
        None
    } else if next {
        Some((current + 1) % len)
    } else {
        Some((current + len - 1) % len)
    };

    if let Some(cursor) = world.get_resource_mut::<CycleCursor>() {
        cursor.index = new_index;
    }
    if let Some(stack) = world.get_resource_mut::<PostStack>() {
        stack.clear();
        if let Some(i) = new_index {
            stack.push(EFFECT_ROSTER[i]());
        }
    }
}

fn playground_text(world: &World) -> Vec<TextSection> {
    let cursor = world
        .get_resource::<CycleCursor>()
        .copied()
        .unwrap_or_default();
    let stack_len = world.get_resource::<PostStack>().map_or(0, PostStack::len);
    let active_name = if cursor.fixture_lock {
        format!("fixture-locked ({stack_len} effect[s])")
    } else {
        match cursor.index {
            Some(i) => format!("{} ({}/{})", effect_label(i), i + 1, EFFECT_ROSTER.len()),
            None => "none".to_string(),
        }
    };
    let hint = if cursor.fixture_lock {
        "cycle disabled — TUNGSTEN_POST_STACK_FIXTURE is set".to_string()
    } else {
        "N/] next   B/[ prev   C/Backspace clear".to_string()
    };
    let post_aa_label = world
        .get_resource::<PostAaState>()
        .map_or("off", |s| s.mode.as_str());
    vec![TextSection {
        content: format!(
            "Shader Playground · active: {active_name}\npost_aa: {post_aa_label}   (Tab cycle  0/5/6/7/8 set)\n{hint}"
        ),
        font_id: "mono".into(),
        font_size: 20.0,
        line_height: 24.0,
        color: [220, 230, 255, 240],
        position: [16.0, 14.0],
        bounds: None,
    }]
}

fn effect_label(i: usize) -> &'static str {
    [
        "tonemap",
        "vignette",
        "lut",
        "chromatic_aberration",
        "color_adjust",
        "tone_mono",
        "crt",
        "film_grain",
        "dither",
        "pixel_outline",
        "fade",
        "wipe_radial",
        "dissolve",
        "glitch",
        "pixelate",
        "fog",
        "god_rays",
    ][i]
}

/// 17-entry roster of constructors the cycle walks through. Order matches the
/// stock-effect roster in the M26 plan so `N` moves top-to-bottom of the table.
///
/// Transition-style effects (`Fade`, `WipeRadial`, `Dissolve`) default to
/// `progress = 0` on the engine side (the natural starting point for a
/// game-driven animation). The playground constructors pick mid-transition
/// values instead so the user can actually see the effect on a still frame.
const EFFECT_ROSTER: &[fn() -> PostPass] = &[
    || PostPass::Tonemap(TonemapParams::default()),
    || PostPass::Vignette(VignetteParams::default()),
    || {
        PostPass::Lut(LutParams {
            mix: 0.75,
            ..LutParams::default()
        })
    },
    || PostPass::ChromaticAberration(2.5),
    || PostPass::ColorAdjust(ColorAdjustParams::default()),
    || PostPass::ToneMono(ToneMonoParams::default()),
    || PostPass::Crt(CrtParams::default()),
    || PostPass::FilmGrain(FilmGrainParams::default()),
    || PostPass::Dither(DitherParams::default()),
    || PostPass::PixelOutline(PixelOutlineParams::default()),
    || {
        PostPass::Fade(FadeParams {
            progress: 0.4,
            ..FadeParams::default()
        })
    },
    || {
        PostPass::WipeRadial(WipeRadialParams {
            progress: 0.6,
            softness: 0.08,
            ..WipeRadialParams::default()
        })
    },
    || {
        PostPass::Dissolve(DissolveParams {
            progress: 0.5,
            noise_scale: 24.0,
            ..DissolveParams::default()
        })
    },
    || PostPass::Glitch(tungsten_core::post::GlitchParams::default()),
    || PostPass::Pixelate(4.0),
    || PostPass::Fog(FogParams::default()),
    || PostPass::GodRays(GodRaysParams::default()),
];

fn push_every_effect(stack: &mut PostStack) {
    for ctor in EFFECT_ROSTER {
        stack.push(ctor());
    }
}

fn push_retro_arcade(stack: &mut PostStack) {
    stack.push(PostPass::Crt(CrtParams::default()));
    stack.push(PostPass::FilmGrain(FilmGrainParams::default()));
    stack.push(PostPass::ColorAdjust(ColorAdjustParams::default()));
}

fn push_dreamy(stack: &mut PostStack) {
    stack.push(PostPass::Fade(FadeParams::default()));
    stack.push(PostPass::Vignette(VignetteParams::default()));
    stack.push(PostPass::ToneMono(ToneMonoParams::default()));
}

fn push_glitch_boss(stack: &mut PostStack) {
    stack.push(PostPass::Glitch(
        tungsten_core::post::GlitchParams::default(),
    ));
    stack.push(PostPass::ChromaticAberration(2.0));
    stack.push(PostPass::Dither(DitherParams::default()));
}
