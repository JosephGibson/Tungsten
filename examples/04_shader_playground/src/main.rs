//! Example 04: shader playground (M26).
//!
//! Renders a bouncing sprite with the 17 stock post-stack effects visible
//! one at a time. Keys (see workspace `input.json`):
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
const SPRITE_SIZE: f32 = 96.0;

struct PlaygroundState {
    position: Vec2,
    velocity: Vec2,
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
        world.insert_resource(PlaygroundState {
            position: Vec2::new(640.0 - SPRITE_SIZE * 0.5, 360.0 - SPRITE_SIZE * 0.5),
            velocity: Vec2::new(220.0, 160.0),
        });
        world.insert_resource(CycleCursor::default());
    }

    app.on_startup(|world, _renderer| {
        let entity = world.spawn();
        world.insert(
            entity,
            Transform {
                position: Vec2::new(640.0, 360.0),
                rotation: 0.0,
                scale: Vec2::ONE,
            },
        );
        world.insert(entity, Sprite::new(QUAD_ID));
        world.insert(entity, Visibility::default());

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

fn bounce_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(1.0 / 60.0, DeltaTime::seconds);
    let position = {
        let Some(state) = world.get_resource_mut::<PlaygroundState>() else {
            return;
        };
        state.position += state.velocity * dt;
        let (w, h) = (1280.0, 720.0);
        if state.position.x < 0.0 || state.position.x + SPRITE_SIZE > w {
            state.velocity.x *= -1.0;
            state.position.x = state.position.x.clamp(0.0, w - SPRITE_SIZE);
        }
        if state.position.y < 0.0 || state.position.y + SPRITE_SIZE > h {
            state.velocity.y *= -1.0;
            state.position.y = state.position.y.clamp(0.0, h - SPRITE_SIZE);
        }
        state.position
    };

    let entity_opt = world.query::<Sprite>().next().map(|(e, _)| e);
    if let Some(entity) = entity_opt {
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = position;
            t.scale = Vec2::splat(SPRITE_SIZE / 16.0);
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
