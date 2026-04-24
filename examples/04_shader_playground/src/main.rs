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

use glam::Vec2;
use tungsten::core::{
    ActionMap, Config, DeltaTime, InputState, Sprite, Transform, Visibility, World,
};
use tungsten::{render::TextSection, App};
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

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Shader Playground — M26".to_string();

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);

    {
        let world = app.world_mut();
        world.insert_resource(CycleCursor::default());
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
    app.add_system_named("playground_cycle_input", cycle_input_system);
    app.set_extract_text(playground_text);

    app.run()
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
        BouncerSpec { position: Vec2::new(200.0, 160.0), velocity: Vec2::new(220.0, 160.0), rotation: 0.0, scale_mul: 6.0, angular_velocity: 1.6, color: [255, 255, 255, 255] },
        BouncerSpec { position: Vec2::new(480.0, 300.0), velocity: Vec2::new(-320.0, 140.0), rotation: 0.7, scale_mul: 4.0, angular_velocity: -2.4, color: [255, 110, 110, 255] },
        BouncerSpec { position: Vec2::new(780.0, 220.0), velocity: Vec2::new(260.0, -260.0), rotation: 1.2, scale_mul: 8.0, angular_velocity: 0.9, color: [110, 255, 150, 255] },
        BouncerSpec { position: Vec2::new(960.0, 520.0), velocity: Vec2::new(-180.0, -200.0), rotation: 2.1, scale_mul: 3.0, angular_velocity: -1.2, color: [120, 180, 255, 255] },
        BouncerSpec { position: Vec2::new(540.0, 600.0), velocity: Vec2::new(380.0, 280.0), rotation: 0.3, scale_mul: 5.5, angular_velocity: 2.6, color: [240, 210, 110, 255] },
        BouncerSpec { position: Vec2::new(1080.0, 140.0), velocity: Vec2::new(-140.0, 340.0), rotation: 1.7, scale_mul: 7.0, angular_velocity: -0.6, color: [255, 140, 220, 255] },
    ];
    for &BouncerSpec { position, velocity, rotation, scale_mul, angular_velocity, color } in specs {
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
            } else if t.position.x + half > WINDOW_W {
                t.position.x = WINDOW_W - half;
                velocity.x = -velocity.x.abs();
            }
            if t.position.y - half < 0.0 {
                t.position.y = half;
                velocity.y = velocity.y.abs();
            } else if t.position.y + half > WINDOW_H {
                t.position.y = WINDOW_H - half;
                velocity.y = -velocity.y.abs();
            }
        }

        if let Some(b) = world.get_mut::<Bouncer>(entity) {
            b.velocity = velocity;
        }
    }
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
    let cursor = world.get_resource::<CycleCursor>().copied().unwrap_or_default();
    let stack_len = world
        .get_resource::<PostStack>()
        .map(PostStack::len)
        .unwrap_or(0);
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
    vec![TextSection {
        content: format!("Shader Playground · active: {active_name}\n{hint}"),
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
    || PostPass::Lut(LutParams {
        mix: 0.75,
        ..LutParams::default()
    }),
    || PostPass::ChromaticAberration(2.5),
    || PostPass::ColorAdjust(ColorAdjustParams::default()),
    || PostPass::ToneMono(ToneMonoParams::default()),
    || PostPass::Crt(CrtParams::default()),
    || PostPass::FilmGrain(FilmGrainParams::default()),
    || PostPass::Dither(DitherParams::default()),
    || PostPass::PixelOutline(PixelOutlineParams::default()),
    || PostPass::Fade(FadeParams {
        progress: 0.4,
        ..FadeParams::default()
    }),
    || PostPass::WipeRadial(WipeRadialParams {
        progress: 0.6,
        softness: 0.08,
        ..WipeRadialParams::default()
    }),
    || PostPass::Dissolve(DissolveParams {
        progress: 0.5,
        noise_scale: 24.0,
        ..DissolveParams::default()
    }),
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
