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
//!   Present path: auto no-vsync (`config.render.present_mode = "auto"`
//!                 and `config.window.vsync = false`)
//!
//! Telemetry output: printed to stdout every 60 frames.
//! Baseline capture: pipe to `tee perf-runs/<timestamp>/sprite-stress.txt`

use glam::Vec2;
use tungsten::core::{Camera2D, Config, World};
use tungsten::render::{GpuFrameTimings, SpriteBatch, SpriteInstance};
use tungsten::{App, FrameTimings};
use tungsten_core::assets::{FilterMode, TextureHandle};

const DEFAULT_SPRITE_COUNT: usize = 2_000;
const COLS: usize = 50;
const SPRITE_SIZE: f32 = 16.0;
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
    metadata_logged: bool,
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
    let (frame_count, metadata_logged) = match world.get_resource::<SceneState>() {
        Some(state) => (state.frame_count, state.metadata_logged),
        None => return,
    };
    let sample_frame = frame_count.saturating_sub(1);

    if !metadata_logged {
        let metadata_line = world.get_resource::<GpuFrameTimings>().map(|gpu| {
            format!(
                "[gpu] backend={} adapter={} present_mode={} max_frame_latency={}",
                gpu.backend.as_deref().unwrap_or("unknown"),
                gpu.adapter_name.as_deref().unwrap_or("unknown"),
                gpu.present_mode.as_deref().unwrap_or("unknown"),
                gpu.max_frame_latency.unwrap_or(0)
            )
        });
        if let Some(line) = metadata_line {
            println!("{line}");
            if let Some(state) = world.get_resource_mut::<SceneState>() {
                state.metadata_logged = true;
            }
        }
    }

    if sample_frame > 0 && sample_frame % LOG_INTERVAL == 0 {
        if let Some(ft) = world.get_resource::<FrameTimings>() {
            let gpu_ms = world
                .get_resource::<GpuFrameTimings>()
                .and_then(|gpu| gpu.frame_gpu_ms)
                .map(|ms| format!("{ms:.2}ms"))
                .unwrap_or_else(|| "n/a".to_string());
            println!(
                "[frame {:>5}] total={:.2}ms update={:.2}ms extract={:.2}ms render={:.2}ms acquire={:.2}ms encode={:.2}ms submit={:.2}ms gpu={}",
                sample_frame,
                ft.total_ms,
                ft.update_ms,
                ft.extract_ms,
                ft.render_ms,
                ft.render_acquire_ms,
                ft.render_encode_ms,
                ft.render_submit_present_ms,
                gpu_ms
            );
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
            metadata_logged: false,
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
