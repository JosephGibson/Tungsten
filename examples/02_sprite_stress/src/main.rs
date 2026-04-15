//! Example 02 — Sprite Stress (M12 canonical scene 2)
//!
//! Spawns SPRITE_COUNT sprites (default 2000; override via STRESS_COUNT env var)
//! and moves them in a sine wave each frame. No physics, audio, or hot reload.
//!
//! Fixed capture rules (M12 baseline):
//!   Build mode:   release  (`cargo run -p example-02-sprite-stress --release`)
//!   Backend:      WGPU_BACKEND=vulkan  (Linux)
//!   Resolution:   1920 × 1080  (set in code)
//!   Frame window: 300 frames after 60-frame warm-up
//!   VSync:        disabled (`config.window.vsync = false`)
//!
//! Telemetry output: printed to stdout every 60 frames.
//! Baseline capture: pipe to `tee perf-runs/<timestamp>/sprite-stress.txt`

use glam::Vec2;
use tungsten::core::{Camera2D, Config, World};
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::{App, FrameTimings};
use tungsten_core::assets::{FilterMode, TextureHandle};

const DEFAULT_SPRITE_COUNT: usize = 2_000;
const COLS: usize = 50;
const SPRITE_SIZE: f32 = 16.0;
const WARMUP_FRAMES: u32 = 60;
const LOG_INTERVAL: u32 = 60;
const PLACEHOLDER_HANDLE: TextureHandle = TextureHandle(0);

struct SpriteEntry {
    base_x: f32,
    base_y: f32,
    phase: f32,
    y_offset: f32,
}

struct SceneState {
    sprites: Vec<SpriteEntry>,
    frame_count: u32,
    total_frame_ms: f64,
    stat_frames: u32,
}

fn tick_sprites(world: &mut World) {
    let state = match world.get_resource_mut::<SceneState>() {
        Some(s) => s,
        None => return,
    };

    state.frame_count += 1;
    let fc = state.frame_count as f32;

    for sprite in &mut state.sprites {
        sprite.y_offset = (fc * 0.02 + sprite.phase).sin() * 4.0;
    }
}

fn log_telemetry(world: &mut World) {
    let frame_count = match world.get_resource::<SceneState>() {
        Some(state) => state.frame_count,
        None => return,
    };

    if frame_count % LOG_INTERVAL == 0 {
        if let Some(ft) = world.get_resource::<FrameTimings>() {
            println!(
                "[frame {:>5}] total={:.2}ms update={:.2}ms extract={:.2}ms render={:.2}ms",
                frame_count, ft.total_ms, ft.update_ms, ft.extract_ms, ft.render_ms
            );
        }
    }

    let total_ms = world
        .get_resource::<FrameTimings>()
        .map(|ft| ft.total_ms as f64);
    if frame_count > WARMUP_FRAMES {
        if let (Some(total_ms), Some(state)) = (total_ms, world.get_resource_mut::<SceneState>()) {
            state.total_frame_ms += total_ms;
            state.stat_frames += 1;
        }
    }
}

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let state = match world.get_resource::<SceneState>() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let instances: Vec<SpriteInstance> = state
        .sprites
        .iter()
        .map(|s| SpriteInstance {
            position: [s.base_x, s.base_y + s.y_offset],
            size: [SPRITE_SIZE, SPRITE_SIZE],
        })
        .collect();

    vec![SpriteBatch {
        texture: PLACEHOLDER_HANDLE,
        filter: FilterMode::Nearest,
        instances,
    }]
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let sprite_count = std::env::var("STRESS_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SPRITE_COUNT);

    let mut config = Config::load("tungsten.json")?;
    config.window.title = format!("Sprite Stress ({sprite_count} sprites)");
    config.window.width = 1920;
    config.window.height = 1080;
    config.window.vsync = false;

    let mut app = App::new(config);

    {
        let world = app.world_mut();

        let sprites: Vec<SpriteEntry> = (0..sprite_count)
            .map(|i| SpriteEntry {
                base_x: (i % COLS) as f32 * SPRITE_SIZE,
                base_y: (i / COLS) as f32 * SPRITE_SIZE,
                phase: i as f32 * 0.1,
                y_offset: 0.0,
            })
            .collect();

        world.insert_resource(SceneState {
            sprites,
            frame_count: 0,
            total_frame_ms: 0.0,
            stat_frames: 0,
        });

        if let Some(cam) = world.get_resource_mut::<Camera2D>() {
            cam.zoom = 1.0;
            cam.position = Vec2::ZERO;
        }
    }

    app.on_startup(|_world, renderer| {
        let rgba = vec![255u8; 16 * 16 * 4];
        renderer.upload_texture(PLACEHOLDER_HANDLE, &rgba, 16, 16);
    });

    app.add_system_named("tick_sprites", tick_sprites);
    app.add_system_named("log_telemetry", log_telemetry);
    app.set_extract_sprites(extract_sprites);

    app.run()
}
