//! Render-hot-path baseline: synthetic texture, grid sprites, no ECS traversal.

use glam::Vec2;
use tungsten::core::{CameraState, DeltaTime, FilterMode, World};
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::{App, WindowSize};
use tungsten_core::assets::TextureHandle;

use crate::shared::{log_telemetry, rgb_wheel_color, TelemetryState};

pub(crate) const DEFAULT_SPRITE_COUNT: usize = 2_000;

const BASELINE_SPRITE_SIZE: f32 = 16.0;
const BASELINE_PLACEHOLDER_HANDLE: TextureHandle = TextureHandle(0);

#[derive(Debug)]
struct BaselineSpriteEntry {
    base_x: f32,
    base_y: f32,
    phase: f32,
    y_offset: f32,
}

#[derive(Debug)]
struct BaselineSceneState {
    sprites: Vec<BaselineSpriteEntry>,
    elapsed: f32,
}

pub(crate) fn configure_baseline_scene(app: &mut App, sprite_count: usize) {
    {
        let world = app.world_mut();
        world.insert_resource(TelemetryState::default());
        seed_baseline_world(world, sprite_count);
        if let Some(cam) = world.get_resource_mut::<CameraState>() {
            cam.zoom = 1.0;
            cam.position = Vec2::ZERO;
        }
    }

    app.on_startup(|_world, renderer| {
        let rgba = vec![255u8; 16 * 16 * 4];
        renderer.upload_texture(
            BASELINE_PLACEHOLDER_HANDLE,
            &rgba,
            16,
            16,
            FilterMode::Nearest,
        );
    });

    app.add_system_named("tick_sprites", tick_baseline_sprites);
    app.add_system_named("log_telemetry", log_telemetry);
    app.set_extract_sprites(extract_baseline_sprites);
}

fn seed_baseline_world(world: &mut World, sprite_count: usize) {
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1_920,
            height: 1_080,
        });
    let (cols, step_x, step_y) = baseline_grid_layout(window, sprite_count);

    let sprites: Vec<BaselineSpriteEntry> = (0..sprite_count)
        .map(|i| BaselineSpriteEntry {
            base_x: (i % cols) as f32 * step_x,
            base_y: (i / cols) as f32 * step_y,
            phase: i as f32 * 0.1,
            y_offset: 0.0,
        })
        .collect();

    world.insert_resource(BaselineSceneState {
        sprites,
        elapsed: 0.0,
    });
}

fn baseline_grid_layout(window: WindowSize, sprite_count: usize) -> (usize, f32, f32) {
    if sprite_count == 0 {
        return (1, 0.0, 0.0);
    }
    let width = (window.width as f32).max(BASELINE_SPRITE_SIZE);
    let height = (window.height as f32).max(BASELINE_SPRITE_SIZE);
    let aspect = width / height;
    let cols = ((sprite_count as f32 * aspect).sqrt().ceil() as usize).max(1);
    let rows = sprite_count.div_ceil(cols);
    let step_x = if cols > 1 {
        (width - BASELINE_SPRITE_SIZE) / (cols - 1) as f32
    } else {
        0.0
    };
    let step_y = if rows > 1 {
        (height - BASELINE_SPRITE_SIZE) / (rows - 1) as f32
    } else {
        0.0
    };
    (cols, step_x, step_y)
}

fn tick_baseline_sprites(world: &mut World) {
    let frame = {
        let Some(telemetry) = world.get_resource_mut::<TelemetryState>() else {
            return;
        };
        telemetry.frame_count = telemetry.frame_count.saturating_add(1);
        telemetry.frame_count as f32
    };

    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(1.0 / 60.0, tungsten_core::DeltaTime::seconds);

    let Some(state) = world.get_resource_mut::<BaselineSceneState>() else {
        return;
    };

    state.elapsed += dt;
    for sprite in &mut state.sprites {
        sprite.y_offset = (frame * 0.02 + sprite.phase).sin() * 4.0;
    }
}

fn extract_baseline_sprites(world: &World) -> Vec<SpriteBatch> {
    let Some(state) = world.get_resource::<BaselineSceneState>() else {
        return Vec::new();
    };

    let time = state.elapsed;
    let instances: Vec<SpriteInstance> = state
        .sprites
        .iter()
        .map(|sprite| {
            let pulse = 1.0 + 0.25 * (time * 2.0 + sprite.phase).sin();
            let size = BASELINE_SPRITE_SIZE * pulse;
            SpriteInstance::whole(
                [sprite.base_x, sprite.base_y + sprite.y_offset],
                [size, size],
                time * 0.5 + sprite.phase,
                rgb_wheel_color(time, sprite.phase),
            )
        })
        .collect();

    vec![SpriteBatch {
        texture: BASELINE_PLACEHOLDER_HANDLE,
        filter: FilterMode::Nearest,
        instances,
    }]
}
