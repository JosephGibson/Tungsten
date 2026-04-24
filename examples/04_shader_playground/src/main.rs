//! Example 04: shader playground (M26).
//!
//! Renders a bouncing sprite with the 17 stock post-stack effects toggleable
//! on demand. Keeps the M26 byte-identity guarantee in the default config —
//! with no fixture and no toggles, the post stack stays empty.
//!
//! Env:
//!   - `TUNGSTEN_POST_STACK_FIXTURE=all`   → push all 17 stock effects
//!   - `TUNGSTEN_POST_STACK_FIXTURE=empty` → leave the stack empty (default)
//!
//! Scaffolded minimally in M26; preset cycle / per-effect HUD row toggles
//! stay as follow-up work once an action-map binding scheme is agreed.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::core::{Config, DeltaTime, Sprite, Transform, Visibility, World};
use tungsten::{render::TextSection, App};
use tungsten_core::post::{
    ColorAdjustParams, CrtParams, DissolveParams, DitherParams, FadeParams, FilmGrainParams,
    FogParams, GodRaysParams, LutParams, PostPass, PostStack, ToneMonoParams, TonemapParams,
    VignetteParams, WipeRadialParams,
};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/04_shader_playground/assets/manifest.json";
const QUAD_ID: &str = "ex04_quad";
const SPRITE_SIZE: f32 = 96.0;

struct PlaygroundState {
    position: Vec2,
    velocity: Vec2,
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
    }

    app.on_startup(|world, _renderer| {
        // Bouncing sprite.
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

        // Fixture-driven post stack for the smoke test matrix.
        if let Some(stack) = world.get_resource_mut::<PostStack>() {
            match std::env::var("TUNGSTEN_POST_STACK_FIXTURE")
                .unwrap_or_default()
                .as_str()
            {
                "all" => push_every_effect(stack),
                "retro_arcade" => push_retro_arcade(stack),
                "dreamy" => push_dreamy(stack),
                "glitch_boss" => push_glitch_boss(stack),
                _ => {}
            }
        }
    });

    app.add_system_named("playground_bounce", bounce_system);
    app.set_extract_text(playground_text);

    app.run()
}

fn bounce_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(1.0 / 60.0, DeltaTime::seconds);
    let (position, velocity) = {
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
        (state.position, state.velocity)
    };

    let entity_opt = world.query::<Sprite>().next().map(|(e, _)| e);
    if let Some(entity) = entity_opt {
        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = position;
            t.scale = Vec2::splat(SPRITE_SIZE / 16.0);
        }
    }
    let _ = velocity;
}

fn playground_text(world: &World) -> Vec<TextSection> {
    let stack_len = world
        .get_resource::<PostStack>()
        .map(PostStack::len)
        .unwrap_or(0);
    vec![TextSection {
        content: format!(
            "Shader Playground · post stack: {stack_len} effect(s)\nFixtures: TUNGSTEN_POST_STACK_FIXTURE=all|retro_arcade|dreamy|glitch_boss|empty"
        ),
        font_id: "mono".into(),
        font_size: 18.0,
        line_height: 22.0,
        color: [220, 230, 255, 240],
        position: [16.0, 14.0],
        bounds: None,
    }]
}

fn push_every_effect(stack: &mut PostStack) {
    stack.push(PostPass::Tonemap(TonemapParams::default()));
    stack.push(PostPass::Vignette(VignetteParams::default()));
    stack.push(PostPass::Lut(LutParams::default()));
    stack.push(PostPass::ChromaticAberration(1.0));
    stack.push(PostPass::ColorAdjust(ColorAdjustParams::default()));
    stack.push(PostPass::ToneMono(ToneMonoParams::default()));
    stack.push(PostPass::Crt(CrtParams::default()));
    stack.push(PostPass::FilmGrain(FilmGrainParams::default()));
    stack.push(PostPass::Dither(DitherParams::default()));
    stack.push(PostPass::PixelOutline(
        tungsten_core::post::PixelOutlineParams::default(),
    ));
    stack.push(PostPass::Fade(FadeParams::default()));
    stack.push(PostPass::WipeRadial(WipeRadialParams::default()));
    stack.push(PostPass::Dissolve(DissolveParams::default()));
    stack.push(PostPass::Glitch(
        tungsten_core::post::GlitchParams::default(),
    ));
    stack.push(PostPass::Pixelate(2.0));
    stack.push(PostPass::Fog(FogParams::default()));
    stack.push(PostPass::GodRays(GodRaysParams::default()));
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
