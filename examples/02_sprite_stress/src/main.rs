//! Example 02 — Sprite Stress
//!
//! Two scene modes live under the same package:
//!   - `baseline` (default): the original M12 sine-wave sprite scene
//!   - `ecs-high-load`: a 20k-entity ECS + render + camera stress scene
//!
//! Env vars:
//!   - `STRESS_SCENE=baseline|ecs-high-load`
//!   - `STRESS_COUNT=<n>` overrides the scene-specific default count
//!
//! Fixed capture rules (M12 baseline):
//!   Build mode:   release  (`cargo run -p example-02-sprite-stress --release`)
//!   Backend:      WGPU_BACKEND=vulkan  (Linux)
//!   Resolution:   1920 × 1080  (set through `config.display.resolution`)
//!   Frame window: 300 frames after 60-frame warm-up
//!   Present path: checked-in default auto no-vsync (`tungsten.json` keeps
//!                 `display.present_mode = "auto"` and this example keeps
//!                 `display.vsync = false`)
//!
//! Telemetry output: printed to stdout every 60 frames.
//! Baseline capture: pipe to `tee perf-runs/<timestamp>/sprite-stress.txt`

use std::path::PathBuf;

use glam::Vec2;
use tungsten::core::{
    sync_position_to_transform, Aabb, AssetRegistry, CameraBounds, CameraController, CameraMode,
    CameraState, Config, DeltaTime, FilterMode, PhysicsConfig, Position, ResolvedManifest,
    RigidBody, Sprite, Transform, Velocity, Visibility, World,
};
use tungsten::render::{GpuFrameTimings, SpriteBatch, SpriteInstance, TextSection};
use tungsten::{asset_loader, camera_update_system, App, FrameTimings, WindowSize};
use tungsten_core::assets::TextureHandle;
use tungsten_core::physics::SpatialGrid;

const MANIFEST_ROOT: &str = "assets/manifest.json";
const DEFAULT_SPRITE_COUNT: usize = 2_000;
const DEFAULT_HIGH_LOAD_COUNT: usize = 20_000;
const COLS: usize = 50;
const BASELINE_SPRITE_SIZE: f32 = 16.0;
const BASELINE_PLACEHOLDER_HANDLE: TextureHandle = TextureHandle(0);
const LOG_INTERVAL: u32 = 60;

const HIGH_LOAD_WORLD_WIDTH: f32 = 3_200.0;
const HIGH_LOAD_WORLD_HEIGHT: f32 = 1_800.0;
const HIGH_LOAD_PADDING: f32 = 24.0;
const HIGH_LOAD_CAMERA_ZOOM: f32 = 0.75;
const HIGH_LOAD_DEAD_ZONE: Vec2 = Vec2::new(320.0, 180.0);
const HIGH_LOAD_CAMERA_SMOOTHING: f32 = 0.2;
const HIGH_LOAD_SPRITE_SIZE_PX: u32 = 12;
const HIGH_LOAD_SPRITE_SIZE: f32 = HIGH_LOAD_SPRITE_SIZE_PX as f32;
const HIGH_LOAD_HALF_SIZE: f32 = HIGH_LOAD_SPRITE_SIZE * 0.5;
const HIGH_LOAD_GRID_CELL_SIZE: f32 = 32.0;
const HIGH_LOAD_NEIGHBOR_RADIUS: f32 = 24.0;
const HIGH_LOAD_MIN_SPEED: f32 = 75.0;
const HIGH_LOAD_MAX_SPEED: f32 = 180.0;
const HIGH_LOAD_FLOW_STRENGTH: f32 = 125.0;
const HIGH_LOAD_REPULSION_STRENGTH: f32 = 165.0;
const HIGH_LOAD_TANGENT_STRENGTH: f32 = 42.0;
const HIGH_LOAD_SPRITE_ID: &str = "ex02_high_load_agent";
const HIGH_LOAD_SPRITE_PATH: &str = "__generated__/ex02_high_load_agent.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StressScene {
    Baseline,
    EcsHighLoad,
}

impl StressScene {
    fn parse(raw: Option<&str>) -> anyhow::Result<Self> {
        match raw.unwrap_or("baseline") {
            "baseline" => Ok(Self::Baseline),
            "ecs-high-load" => Ok(Self::EcsHighLoad),
            other => Err(anyhow::anyhow!(
                "Unknown STRESS_SCENE '{other}'. Expected 'baseline' or 'ecs-high-load'"
            )),
        }
    }

    fn default_count(self) -> usize {
        match self {
            Self::Baseline => DEFAULT_SPRITE_COUNT,
            Self::EcsHighLoad => DEFAULT_HIGH_LOAD_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExampleOptions {
    scene: StressScene,
    count: usize,
}

impl ExampleOptions {
    fn from_env() -> anyhow::Result<Self> {
        let raw_scene = std::env::var("STRESS_SCENE").ok();
        let scene = StressScene::parse(raw_scene.as_deref())?;
        let raw_count = std::env::var("STRESS_COUNT").ok();
        let count = resolve_count(scene, raw_count.as_deref());
        Ok(Self { scene, count })
    }
}

fn resolve_count(scene: StressScene, raw_count: Option<&str>) -> usize {
    raw_count
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or_else(|| scene.default_count())
}

#[derive(Debug, Default, Clone, Copy)]
struct TelemetryState {
    frame_count: u32,
    metadata_logged: bool,
}

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
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StressAgent {
    phase: f32,
    tint_seed: f32,
}

#[derive(Debug, Clone, Copy)]
struct HighLoadSceneState {
    leader: tungsten::core::Entity,
    entity_count: usize,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let options = ExampleOptions::from_env()?;

    let mut config = Config::load("tungsten.json")?;
    config.window.title = match options.scene {
        StressScene::Baseline => format!("Sprite Stress ({} sprites)", options.count),
        StressScene::EcsHighLoad => {
            format!("Sprite Stress ECS High Load ({} entities)", options.count)
        }
    };
    config.display.resolution = Some(tungsten::core::Resolution {
        width: 1920,
        height: 1080,
    });
    config.display.vsync = Some(false);

    let mut app = App::new(config);

    match options.scene {
        StressScene::Baseline => configure_baseline_scene(&mut app, options.count),
        StressScene::EcsHighLoad => configure_high_load_scene(&mut app, options.count),
    }

    app.run()
}

fn configure_baseline_scene(app: &mut App, sprite_count: usize) {
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
        renderer.upload_texture(BASELINE_PLACEHOLDER_HANDLE, &rgba, 16, 16);
    });

    app.add_system_named("tick_sprites", tick_baseline_sprites);
    app.add_system_named("log_telemetry", log_telemetry);
    app.set_extract_sprites(extract_baseline_sprites);
}

fn seed_baseline_world(world: &mut World, sprite_count: usize) {
    let sprites: Vec<BaselineSpriteEntry> = (0..sprite_count)
        .map(|i| BaselineSpriteEntry {
            base_x: (i % COLS) as f32 * BASELINE_SPRITE_SIZE,
            base_y: (i / COLS) as f32 * BASELINE_SPRITE_SIZE,
            phase: i as f32 * 0.1,
            y_offset: 0.0,
        })
        .collect();

    world.insert_resource(BaselineSceneState { sprites });
}

fn tick_baseline_sprites(world: &mut World) {
    let frame = {
        let Some(telemetry) = world.get_resource_mut::<TelemetryState>() else {
            return;
        };
        telemetry.frame_count = telemetry.frame_count.saturating_add(1);
        telemetry.frame_count as f32
    };

    let state = match world.get_resource_mut::<BaselineSceneState>() {
        Some(state) => state,
        None => return,
    };

    for sprite in &mut state.sprites {
        sprite.y_offset = (frame * 0.02 + sprite.phase).sin() * 4.0;
    }
}

fn extract_baseline_sprites(world: &World) -> Vec<SpriteBatch> {
    let state = match world.get_resource::<BaselineSceneState>() {
        Some(state) => state,
        None => return Vec::new(),
    };

    let instances: Vec<SpriteInstance> = state
        .sprites
        .iter()
        .map(|sprite| SpriteInstance {
            position: [sprite.base_x, sprite.base_y + sprite.y_offset],
            size: [BASELINE_SPRITE_SIZE, BASELINE_SPRITE_SIZE],
            rotation: 0.0,
            color: [255; 4],
        })
        .collect();

    vec![SpriteBatch {
        texture: BASELINE_PLACEHOLDER_HANDLE,
        filter: FilterMode::Nearest,
        instances,
    }]
}

fn configure_high_load_scene(app: &mut App, entity_count: usize) {
    {
        let world = app.world_mut();
        world.insert_resource(TelemetryState::default());
        if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
            cfg.gravity = Vec2::ZERO;
            cfg.broadphase_cell_size = HIGH_LOAD_GRID_CELL_SIZE;
        }
        seed_high_load_world(world, entity_count);
    }

    app.on_startup(|world, renderer| {
        let manifest = ResolvedManifest::load(MANIFEST_ROOT).expect("Failed to load root manifest");
        asset_loader::load_fonts(&manifest, world, renderer).expect("Failed to load root fonts");
        register_high_load_sprite(world, renderer);
    });

    app.add_system_named("steer_agents_system", steer_agents_system);
    app.add_system_named("physics_step", tungsten::physics::physics_step);
    app.add_system_named("confine_agents_system", confine_agents_system);
    app.add_system_named("sync_position_to_transform", sync_position_to_transform);
    app.add_system_named("orient_agents_system", orient_agents_system);
    app.add_system_named("high_load_camera_base_system", high_load_camera_base_system);
    app.add_system_named("camera_update_system", camera_update_system);
    app.add_system_named("log_telemetry", log_telemetry);
    app.set_extract_sprites(extract_high_load_sprites);
    app.set_extract_text(extract_high_load_text);
}

fn seed_high_load_world(world: &mut World, entity_count: usize) {
    if entity_count == 0 {
        return;
    }

    let cols = high_load_cols(entity_count);
    let rows = (entity_count + cols - 1) / cols;
    let usable_width = (HIGH_LOAD_WORLD_WIDTH - HIGH_LOAD_PADDING * 2.0 - HIGH_LOAD_SPRITE_SIZE)
        .max(HIGH_LOAD_SPRITE_SIZE);
    let usable_height = (HIGH_LOAD_WORLD_HEIGHT - HIGH_LOAD_PADDING * 2.0 - HIGH_LOAD_SPRITE_SIZE)
        .max(HIGH_LOAD_SPRITE_SIZE);
    let step_x = if cols > 1 {
        usable_width / (cols - 1) as f32
    } else {
        0.0
    };
    let step_y = if rows > 1 {
        usable_height / (rows - 1) as f32
    } else {
        0.0
    };

    let mut leader = None;
    for index in 0..entity_count {
        let col = index % cols;
        let row = index / cols;
        let jitter_x = (hash_unit(index as u32, 0xA123_BC45) - 0.5) * step_x * 0.35;
        let jitter_y = (hash_unit(index as u32, 0xC001_D00D) - 0.5) * step_y * 0.35;
        let x = (HIGH_LOAD_PADDING + col as f32 * step_x + jitter_x)
            .clamp(0.0, HIGH_LOAD_WORLD_WIDTH - HIGH_LOAD_SPRITE_SIZE);
        let y = (HIGH_LOAD_PADDING + row as f32 * step_y + jitter_y)
            .clamp(0.0, HIGH_LOAD_WORLD_HEIGHT - HIGH_LOAD_SPRITE_SIZE);
        let phase = hash_unit(index as u32, 0x1357_2468) * std::f32::consts::TAU;
        let tint_seed = hash_unit(index as u32, 0x2468_1357);
        let direction =
            Vec2::new((phase * 1.3).cos(), (phase * 0.7 + 1.1).sin()).normalize_or_zero();
        let speed = HIGH_LOAD_MIN_SPEED + tint_seed * (HIGH_LOAD_MAX_SPEED - HIGH_LOAD_MIN_SPEED);
        let position = Vec2::new(x, y);
        let velocity = if direction.length_squared() > 0.0 {
            direction * speed
        } else {
            Vec2::X * HIGH_LOAD_MIN_SPEED
        };

        let entity = world.spawn();
        leader.get_or_insert(entity);
        world.insert(entity, StressAgent { phase, tint_seed });
        world.insert(entity, Position(position));
        world.insert(entity, Velocity(velocity));
        world.insert(entity, RigidBody::dynamic());
        world.insert(entity, Transform::from_position(position));
        world.insert(
            entity,
            Sprite {
                asset_id: HIGH_LOAD_SPRITE_ID.into(),
                color: stress_agent_color(tint_seed),
                z_order: 0,
            },
        );
        world.insert(entity, Visibility::default());
    }

    if let Some(leader) = leader {
        configure_high_load_camera(world, leader);
        world.insert_resource(HighLoadSceneState {
            leader,
            entity_count,
        });
    }
}

fn configure_high_load_camera(world: &mut World, leader: tungsten::core::Entity) {
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.mode = CameraMode::Follow(leader);
        controller.dead_zone_size = HIGH_LOAD_DEAD_ZONE;
        controller.smoothing_factor = HIGH_LOAD_CAMERA_SMOOTHING;
        controller.bounds = Some(CameraBounds {
            min: Vec2::ZERO,
            max: Vec2::new(HIGH_LOAD_WORLD_WIDTH, HIGH_LOAD_WORLD_HEIGHT),
        });
        controller.zoom_multiplier = 1.0;
        controller.shake_amplitude = Vec2::ZERO;
        controller.shake_frequency_hz = 0.0;
        controller.shake_phase = 0.0;
    }
}

fn register_high_load_sprite(world: &mut World, renderer: &mut tungsten::render::Renderer) {
    let rgba = build_high_load_sprite_rgba();
    let handle = {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        registry.register_sprite(
            HIGH_LOAD_SPRITE_ID.to_string(),
            FilterMode::Nearest,
            HIGH_LOAD_SPRITE_SIZE_PX,
            HIGH_LOAD_SPRITE_SIZE_PX,
            PathBuf::from(HIGH_LOAD_SPRITE_PATH),
        )
    };
    renderer.upload_texture(
        handle,
        &rgba,
        HIGH_LOAD_SPRITE_SIZE_PX,
        HIGH_LOAD_SPRITE_SIZE_PX,
    );
}

fn build_high_load_sprite_rgba() -> Vec<u8> {
    let mut rgba =
        Vec::with_capacity((HIGH_LOAD_SPRITE_SIZE_PX * HIGH_LOAD_SPRITE_SIZE_PX * 4) as usize);
    let center = HIGH_LOAD_SPRITE_SIZE * 0.5 - 0.5;
    let radius = center;

    for y in 0..HIGH_LOAD_SPRITE_SIZE_PX {
        for x in 0..HIGH_LOAD_SPRITE_SIZE_PX {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let edge = ((radius - dist) / radius).clamp(0.0, 1.0);
            let brightness = (180.0 + edge * 75.0).round() as u8;
            let alpha = if dist <= radius { 255 } else { 0 };
            rgba.extend_from_slice(&[brightness, brightness, 255, alpha]);
        }
    }

    rgba
}

fn steer_agents_system(world: &mut World) {
    if let Some(telemetry) = world.get_resource_mut::<TelemetryState>() {
        telemetry.frame_count = telemetry.frame_count.saturating_add(1);
    }

    let entities = world.query3_entities::<StressAgent, Position, Velocity>();
    if entities.is_empty() {
        return;
    }

    let dt = world
        .get_resource::<DeltaTime>()
        .map(|delta| delta.seconds())
        .filter(|dt| *dt > 0.0)
        .unwrap_or(1.0 / 60.0);
    let frame = telemetry_frame(world) as f32;

    let mut positions = Vec::with_capacity(entities.len());
    let mut velocities = Vec::with_capacity(entities.len());
    let mut agents = Vec::with_capacity(entities.len());
    let mut grid = SpatialGrid::new(HIGH_LOAD_GRID_CELL_SIZE);

    for (index, entity) in entities.iter().enumerate() {
        let position = world
            .get::<Position>(*entity)
            .copied()
            .map(|p| p.0)
            .unwrap_or(Vec2::ZERO);
        let velocity = world
            .get::<Velocity>(*entity)
            .copied()
            .map(|v| v.0)
            .unwrap_or(Vec2::ZERO);
        let agent = *world.get::<StressAgent>(*entity).unwrap();
        let center = position + Vec2::splat(HIGH_LOAD_HALF_SIZE);

        positions.push(center);
        velocities.push(velocity);
        agents.push(agent);
        grid.insert(
            index as u32,
            &Aabb::new(center, Vec2::splat(HIGH_LOAD_HALF_SIZE)),
        );
    }

    let world_center = Vec2::new(HIGH_LOAD_WORLD_WIDTH * 0.5, HIGH_LOAD_WORLD_HEIGHT * 0.5);
    let mut candidates = Vec::new();

    for (index, entity) in entities.iter().enumerate() {
        let position = positions[index];
        let velocity = velocities[index];
        let agent = agents[index];

        let mut repulsion = Vec2::ZERO;
        let neighborhood = Aabb::new(
            position,
            Vec2::splat(HIGH_LOAD_NEIGHBOR_RADIUS + HIGH_LOAD_HALF_SIZE),
        );
        grid.query(&neighborhood, Some(index as u32), &mut candidates);

        for &candidate in &candidates {
            let other = positions[candidate as usize];
            let delta = position - other;
            let dist_sq = delta.length_squared();
            if dist_sq <= 0.0001 || dist_sq >= HIGH_LOAD_NEIGHBOR_RADIUS.powi(2) {
                continue;
            }

            let distance = dist_sq.sqrt();
            repulsion += delta / distance
                * ((HIGH_LOAD_NEIGHBOR_RADIUS - distance) / HIGH_LOAD_NEIGHBOR_RADIUS);
        }

        let to_center = world_center - position;
        let tangent = Vec2::new(-to_center.y, to_center.x).normalize_or_zero();
        let flow = Vec2::new(
            (position.y * 0.004 + frame * 0.015 + agent.phase).sin(),
            (position.x * 0.003 - frame * 0.013 + agent.phase * 1.4).cos(),
        );
        let drift = Vec2::new(
            (agent.tint_seed * std::f32::consts::TAU).cos(),
            (agent.tint_seed * std::f32::consts::TAU).sin(),
        ) * 8.0;
        let steering = flow * HIGH_LOAD_FLOW_STRENGTH
            + repulsion * HIGH_LOAD_REPULSION_STRENGTH
            + tangent * HIGH_LOAD_TANGENT_STRENGTH
            + drift;

        let mut next_velocity = velocity + steering * dt;
        let speed = next_velocity.length();
        if speed <= f32::EPSILON {
            next_velocity = Vec2::new(agent.phase.cos(), agent.phase.sin()) * HIGH_LOAD_MIN_SPEED;
        } else if speed < HIGH_LOAD_MIN_SPEED {
            next_velocity = next_velocity / speed * HIGH_LOAD_MIN_SPEED;
        } else if speed > HIGH_LOAD_MAX_SPEED {
            next_velocity = next_velocity / speed * HIGH_LOAD_MAX_SPEED;
        }

        if let Some(vel) = world.get_mut::<Velocity>(*entity) {
            vel.0 = next_velocity;
        }
    }
}

fn confine_agents_system(world: &mut World) {
    let entities = world.query2_entities::<Position, Velocity>();
    for entity in entities {
        let Some(position) = world.get::<Position>(entity).copied().map(|p| p.0) else {
            continue;
        };
        let Some(velocity) = world.get::<Velocity>(entity).copied().map(|v| v.0) else {
            continue;
        };

        let mut next_position = position;
        let mut next_velocity = velocity;
        let max_x = HIGH_LOAD_WORLD_WIDTH - HIGH_LOAD_SPRITE_SIZE;
        let max_y = HIGH_LOAD_WORLD_HEIGHT - HIGH_LOAD_SPRITE_SIZE;

        if next_position.x <= 0.0 {
            next_position.x = 0.0;
            if next_velocity.x < 0.0 {
                next_velocity.x = next_velocity.x.abs();
            }
        } else if next_position.x >= max_x {
            next_position.x = max_x;
            if next_velocity.x > 0.0 {
                next_velocity.x = -next_velocity.x.abs();
            }
        }

        if next_position.y <= 0.0 {
            next_position.y = 0.0;
            if next_velocity.y < 0.0 {
                next_velocity.y = next_velocity.y.abs();
            }
        } else if next_position.y >= max_y {
            next_position.y = max_y;
            if next_velocity.y > 0.0 {
                next_velocity.y = -next_velocity.y.abs();
            }
        }

        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 = next_position;
        }
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 = next_velocity;
        }
    }
}

fn orient_agents_system(world: &mut World) {
    let entities = world.query2_entities::<Velocity, Transform>();
    for entity in entities {
        let Some(velocity) = world.get::<Velocity>(entity).copied().map(|v| v.0) else {
            continue;
        };
        if velocity.length_squared() <= 0.0001 {
            continue;
        }
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.rotation = velocity.y.atan2(velocity.x);
        }
    }
}

fn high_load_camera_base_system(world: &mut World) {
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.zoom = HIGH_LOAD_CAMERA_ZOOM;
    }
}

fn extract_high_load_sprites(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let Some(asset) = assets.get_sprite(HIGH_LOAD_SPRITE_ID) else {
        return Vec::new();
    };
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1_920,
            height: 1_080,
        });
    let (view_min, view_max) = camera.visible_world_aabb(window.width as f32, window.height as f32);

    let mut instances = Vec::new();
    for (_entity, transform, sprite, visibility) in world.query3::<Transform, Sprite, Visibility>()
    {
        if !visibility.visible || sprite.asset_id != HIGH_LOAD_SPRITE_ID {
            continue;
        }

        let size = Vec2::new(
            asset.width as f32 * transform.scale.x,
            asset.height as f32 * transform.scale.y,
        );
        let sprite_min = transform.position;
        let sprite_max = transform.position + size;
        if sprite_max.x < view_min.x
            || sprite_min.x > view_max.x
            || sprite_max.y < view_min.y
            || sprite_min.y > view_max.y
        {
            continue;
        }

        instances.push(SpriteInstance {
            position: [transform.position.x, transform.position.y],
            size: [size.x, size.y],
            rotation: transform.rotation,
            color: sprite.color,
        });
    }

    if instances.is_empty() {
        return Vec::new();
    }

    vec![SpriteBatch {
        texture: asset.texture,
        filter: asset.filter,
        instances,
    }]
}

fn extract_high_load_text(world: &World) -> Vec<TextSection> {
    let Some(state) = world.get_resource::<HighLoadSceneState>() else {
        return Vec::new();
    };
    let total_ms = world
        .get_resource::<FrameTimings>()
        .map(|timings| timings.total_ms)
        .unwrap_or(0.0);
    let fps = if total_ms > 0.0 {
        (1_000.0 / total_ms).round() as u32
    } else {
        0
    };

    vec![
        TextSection {
            content: "Sprite Stress ECS High Load".into(),
            font_id: "sans_bold".into(),
            font_size: 26.0,
            line_height: 30.0,
            color: [248, 248, 255, 255],
            position: [16.0, 14.0],
            bounds: None,
        },
        TextSection {
            content: format!("FPS: {fps}   Entities: {}", state.entity_count),
            font_id: "mono".into(),
            font_size: 20.0,
            line_height: 24.0,
            color: [210, 230, 255, 240],
            position: [16.0, 46.0],
            bounds: None,
        },
    ]
}

fn log_telemetry(world: &mut World) {
    let (frame_count, metadata_logged) = match world.get_resource::<TelemetryState>() {
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
            if let Some(state) = world.get_resource_mut::<TelemetryState>() {
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

fn telemetry_frame(world: &World) -> u32 {
    world
        .get_resource::<TelemetryState>()
        .map(|state| state.frame_count)
        .unwrap_or(0)
}

fn high_load_cols(count: usize) -> usize {
    let aspect = HIGH_LOAD_WORLD_WIDTH / HIGH_LOAD_WORLD_HEIGHT;
    ((count as f32 * aspect).sqrt().ceil() as usize).max(1)
}

fn stress_agent_color(seed: f32) -> [u8; 4] {
    let r = ((seed * 5.7).sin() * 0.5 + 0.5) * 120.0 + 110.0;
    let g = ((seed * 7.3 + 1.7).sin() * 0.5 + 0.5) * 110.0 + 110.0;
    let b = ((seed * 9.1 + 4.4).sin() * 0.5 + 0.5) * 105.0 + 120.0;
    [r.round() as u8, g.round() as u8, b.round() as u8, 255]
}

fn hash_u32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

fn hash_unit(index: u32, salt: u32) -> f32 {
    hash_u32(index ^ salt) as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
        world.insert_resource(CameraState::new());
        world.insert_resource(CameraController::default());
        world.insert_resource(WindowSize {
            width: 800,
            height: 600,
        });
        world.insert_resource(FrameTimings::new());
        world.insert_resource(AssetRegistry::new());
        world
    }

    fn register_test_high_load_sprite(world: &mut World) {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry missing");
        registry.register_sprite(
            HIGH_LOAD_SPRITE_ID.to_string(),
            FilterMode::Nearest,
            HIGH_LOAD_SPRITE_SIZE_PX,
            HIGH_LOAD_SPRITE_SIZE_PX,
            PathBuf::from(HIGH_LOAD_SPRITE_PATH),
        );
    }

    #[test]
    fn stress_scene_defaults_to_baseline() {
        assert_eq!(StressScene::parse(None).unwrap(), StressScene::Baseline);
    }

    #[test]
    fn stress_scene_parses_high_load_mode() {
        assert_eq!(
            StressScene::parse(Some("ecs-high-load")).unwrap(),
            StressScene::EcsHighLoad
        );
    }

    #[test]
    fn high_load_mode_defaults_to_twenty_thousand_entities() {
        assert_eq!(
            resolve_count(StressScene::EcsHighLoad, None),
            DEFAULT_HIGH_LOAD_COUNT
        );
    }

    #[test]
    fn seed_high_load_world_spawns_requested_entities_and_components() {
        let mut world = test_world();
        seed_high_load_world(&mut world, 32);

        let entities = world.query::<StressAgent>().count();
        assert_eq!(entities, 32);

        let first = world
            .query::<StressAgent>()
            .next()
            .map(|(entity, _)| entity)
            .expect("missing StressAgent entity");
        assert!(world.get::<Position>(first).is_some());
        assert!(world.get::<Velocity>(first).is_some());
        assert!(world.get::<RigidBody>(first).is_some());
        assert!(world.get::<Transform>(first).is_some());
        assert!(world.get::<Sprite>(first).is_some());
        assert!(world.get::<Visibility>(first).is_some());

        let state = world.get_resource::<HighLoadSceneState>().unwrap();
        assert_eq!(state.entity_count, 32);

        let controller = world.get_resource::<CameraController>().unwrap();
        assert!(matches!(controller.mode, CameraMode::Follow(entity) if entity == state.leader));
    }

    #[test]
    fn steer_agents_system_changes_nearby_velocities_deterministically() {
        let mut world_a = test_world();
        world_a.insert_resource(TelemetryState::default());
        let a = world_a.spawn();
        world_a.insert(
            a,
            StressAgent {
                phase: 0.3,
                tint_seed: 0.2,
            },
        );
        world_a.insert(a, Position(Vec2::new(100.0, 100.0)));
        world_a.insert(a, Velocity(Vec2::new(80.0, 0.0)));

        let b = world_a.spawn();
        world_a.insert(
            b,
            StressAgent {
                phase: 1.1,
                tint_seed: 0.7,
            },
        );
        world_a.insert(b, Position(Vec2::new(112.0, 100.0)));
        world_a.insert(b, Velocity(Vec2::new(-80.0, 0.0)));

        let mut world_b = test_world();
        world_b.insert_resource(TelemetryState::default());
        let a2 = world_b.spawn();
        world_b.insert(
            a2,
            StressAgent {
                phase: 0.3,
                tint_seed: 0.2,
            },
        );
        world_b.insert(a2, Position(Vec2::new(100.0, 100.0)));
        world_b.insert(a2, Velocity(Vec2::new(80.0, 0.0)));

        let b2 = world_b.spawn();
        world_b.insert(
            b2,
            StressAgent {
                phase: 1.1,
                tint_seed: 0.7,
            },
        );
        world_b.insert(b2, Position(Vec2::new(112.0, 100.0)));
        world_b.insert(b2, Velocity(Vec2::new(-80.0, 0.0)));

        steer_agents_system(&mut world_a);
        steer_agents_system(&mut world_b);

        let vel_a = world_a.get::<Velocity>(a).unwrap().0;
        let vel_b = world_b.get::<Velocity>(a2).unwrap().0;
        assert_ne!(vel_a, Vec2::new(80.0, 0.0));
        assert_eq!(vel_a, vel_b);
    }

    #[test]
    fn confine_agents_system_clamps_and_reflects() {
        let mut world = test_world();
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(-8.0, HIGH_LOAD_WORLD_HEIGHT)));
        world.insert(entity, Velocity(Vec2::new(-40.0, 55.0)));

        confine_agents_system(&mut world);

        let position = world.get::<Position>(entity).unwrap().0;
        let velocity = world.get::<Velocity>(entity).unwrap().0;
        assert_eq!(position.x, 0.0);
        assert_eq!(position.y, HIGH_LOAD_WORLD_HEIGHT - HIGH_LOAD_SPRITE_SIZE);
        assert!(velocity.x > 0.0);
        assert!(velocity.y < 0.0);
    }

    #[test]
    fn orient_agents_system_writes_rotation_from_velocity() {
        let mut world = test_world();
        let entity = world.spawn();
        world.insert(entity, Velocity(Vec2::new(0.0, 10.0)));
        world.insert(entity, Transform::default());

        orient_agents_system(&mut world);

        let rotation = world.get::<Transform>(entity).unwrap().rotation;
        assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn high_load_extract_culls_offscreen_agents_and_batches_visible_ones() {
        let mut world = test_world();
        register_test_high_load_sprite(&mut world);

        world.insert_resource(CameraState {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
        });
        world.insert_resource(WindowSize {
            width: 100,
            height: 100,
        });

        let visible = world.spawn();
        world.insert(visible, Transform::from_position(Vec2::new(8.0, 8.0)));
        world.insert(visible, Sprite::new(HIGH_LOAD_SPRITE_ID));
        world.insert(visible, Visibility::default());

        let also_visible = world.spawn();
        world.insert(
            also_visible,
            Transform::from_position(Vec2::new(40.0, 40.0)),
        );
        world.insert(
            also_visible,
            Sprite {
                asset_id: HIGH_LOAD_SPRITE_ID.into(),
                color: [12, 34, 56, 255],
                z_order: 0,
            },
        );
        world.insert(also_visible, Visibility::default());

        let hidden = world.spawn();
        world.insert(
            hidden,
            Transform::from_position(Vec2::new(HIGH_LOAD_WORLD_WIDTH, HIGH_LOAD_WORLD_HEIGHT)),
        );
        world.insert(hidden, Sprite::new(HIGH_LOAD_SPRITE_ID));
        world.insert(hidden, Visibility::default());

        let batches = extract_high_load_sprites(&world);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].instances.len(), 2);
    }

    #[test]
    fn high_load_text_hud_shows_fps_and_entity_count() {
        let mut world = test_world();
        let leader = world.spawn();
        world.insert_resource(HighLoadSceneState {
            leader,
            entity_count: 42,
        });
        if let Some(timings) = world.get_resource_mut::<FrameTimings>() {
            timings.total_ms = 20.0;
        }

        let text = extract_high_load_text(&world);
        assert_eq!(text.len(), 2);
        assert!(text[1].content.contains("FPS: 50"));
        assert!(text[1].content.contains("Entities: 42"));
    }
}
