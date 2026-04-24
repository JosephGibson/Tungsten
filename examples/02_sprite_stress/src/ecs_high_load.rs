//! Full-system perf case: 50k ECS agents, steering, broadphase, camera, tints.
//!
//! Synthetic sprite bypasses manifest so perf capture stays self-contained.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::core::{
    sync_position_to_transform, Aabb, AssetRegistry, CameraBounds, CameraController, CameraMode,
    CameraState, DeltaTime, FilterMode, PhysicsConfig, Position, ResolvedManifest, RigidBody,
    Sprite, Transform, Velocity, Visibility, World,
};
use tungsten::render::{Renderer, SpriteBatch, SpriteInstance, TextSection};
use tungsten::{asset_loader, camera_update_system, App, FrameTimings, WindowSize};
use tungsten_core::physics::SpatialGrid;

use crate::shared::{log_telemetry, rgb_wheel_color, telemetry_frame, TelemetryState};

pub(crate) const DEFAULT_HIGH_LOAD_COUNT: usize = 50_000;

const MANIFEST_ROOT: &str = "assets/manifest.json";

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

#[derive(Debug, Clone, Copy, PartialEq)]
struct StressAgent {
    phase: f32,
    tint_seed: f32,
}

#[derive(Debug, Clone, Copy)]
struct HighLoadSceneState {
    #[cfg_attr(not(test), allow(dead_code))]
    leader: tungsten::core::Entity,
    entity_count: usize,
    elapsed: f32,
}

#[derive(Debug)]
struct HighLoadSteeringScratch {
    grid: SpatialGrid,
}

impl Default for HighLoadSteeringScratch {
    fn default() -> Self {
        Self {
            grid: SpatialGrid::new(HIGH_LOAD_GRID_CELL_SIZE),
        }
    }
}

pub(crate) fn configure_high_load_scene(app: &mut App, entity_count: usize) {
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
    app.add_system_named("tint_agents_system", tint_agents_system);
    app.add_system_named("high_load_camera_base_system", high_load_camera_base_system);
    app.add_system_named("camera_update_system", camera_update_system);
    app.add_system_named("log_telemetry", log_telemetry);
    app.set_extract_sprites(extract_high_load_sprites);
    app.set_extract_text(extract_high_load_text);
}

fn seed_high_load_world(world: &mut World, entity_count: usize) {
    world.insert_resource(HighLoadSteeringScratch::default());

    if entity_count == 0 {
        return;
    }

    let cols = high_load_cols(entity_count);
    let rows = entity_count.div_ceil(cols);
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
                material_id: None,
            },
        );
        world.insert(entity, Visibility::default());
    }

    if let Some(leader) = leader {
        configure_high_load_camera(world, leader);
        world.insert_resource(HighLoadSceneState {
            leader,
            entity_count,
            elapsed: 0.0,
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
        controller.shake_amplitude = Vec2::new(3.0, 2.0);
        controller.shake_frequency_hz = 4.0;
        controller.shake_phase = 0.0;
    }
}

fn register_high_load_sprite(world: &mut World, renderer: &mut Renderer) {
    let rgba = build_high_load_sprite_rgba();
    let handle = renderer.allocate_texture_handle();
    {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        registry.register_sprite(
            HIGH_LOAD_SPRITE_ID.to_string(),
            FilterMode::Nearest,
            HIGH_LOAD_SPRITE_SIZE_PX,
            HIGH_LOAD_SPRITE_SIZE_PX,
            PathBuf::from(HIGH_LOAD_SPRITE_PATH),
            handle,
            tungsten::core::assets::UvRect::FULL,
        );
    }
    renderer.upload_texture(
        handle,
        &rgba,
        HIGH_LOAD_SPRITE_SIZE_PX,
        HIGH_LOAD_SPRITE_SIZE_PX,
        FilterMode::Nearest,
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
        .map(tungsten_core::DeltaTime::seconds)
        .filter(|dt| *dt > 0.0)
        .unwrap_or(1.0 / 60.0);
    let frame = telemetry_frame(world) as f32;
    let half_size = Vec2::splat(HIGH_LOAD_HALF_SIZE);

    let mut positions = Vec::with_capacity(entities.len());
    let mut velocities = Vec::with_capacity(entities.len());
    let mut agents = Vec::with_capacity(entities.len());

    for entity in &entities {
        let position = world
            .get::<Position>(*entity)
            .copied()
            .map_or(Vec2::ZERO, |p| p.0);
        let velocity = world
            .get::<Velocity>(*entity)
            .copied()
            .map_or(Vec2::ZERO, |v| v.0);
        let agent = *world.get::<StressAgent>(*entity).unwrap();
        let center = position + Vec2::splat(HIGH_LOAD_HALF_SIZE);

        positions.push(center);
        velocities.push(velocity);
        agents.push(agent);
    }

    let world_center = Vec2::new(HIGH_LOAD_WORLD_WIDTH * 0.5, HIGH_LOAD_WORLD_HEIGHT * 0.5);
    let mut next_velocities = Vec::with_capacity(entities.len());

    {
        let Some(scratch) = world.get_resource_mut::<HighLoadSteeringScratch>() else {
            return;
        };
        scratch.grid.clear();
        for (index, &center) in positions.iter().enumerate() {
            scratch
                .grid
                .insert(index as u32, &Aabb::new(center, half_size));
        }

        let mut candidates = Vec::new();
        for index in 0..entities.len() {
            let position = positions[index];
            let velocity = velocities[index];
            let agent = agents[index];

            let mut repulsion = Vec2::ZERO;
            let neighborhood = Aabb::new(
                position,
                Vec2::splat(HIGH_LOAD_NEIGHBOR_RADIUS + HIGH_LOAD_HALF_SIZE),
            );
            scratch
                .grid
                .query(&neighborhood, Some(index as u32), &mut candidates);

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
                next_velocity =
                    Vec2::new(agent.phase.cos(), agent.phase.sin()) * HIGH_LOAD_MIN_SPEED;
            } else if speed < HIGH_LOAD_MIN_SPEED {
                next_velocity = next_velocity / speed * HIGH_LOAD_MIN_SPEED;
            } else if speed > HIGH_LOAD_MAX_SPEED {
                next_velocity = next_velocity / speed * HIGH_LOAD_MAX_SPEED;
            }

            next_velocities.push(next_velocity);
        }
    }

    for (entity, next_velocity) in entities.iter().zip(next_velocities) {
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

fn tint_agents_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(1.0 / 60.0, tungsten_core::DeltaTime::seconds);
    let elapsed = {
        let Some(state) = world.get_resource_mut::<HighLoadSceneState>() else {
            return;
        };
        state.elapsed += dt;
        state.elapsed
    };
    let entities = world.query2_entities::<StressAgent, Sprite>();
    for entity in entities {
        let Some(tint_seed) = world.get::<StressAgent>(entity).map(|a| a.tint_seed) else {
            continue;
        };
        let color = rgb_wheel_color(elapsed, tint_seed * std::f32::consts::TAU);
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.color = color;
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

        instances.push(SpriteInstance::whole(
            [transform.position.x, transform.position.y],
            [size.x, size.y],
            transform.rotation,
            sprite.color,
        ));
    }

    if instances.is_empty() {
        return Vec::new();
    }

    let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
    batch.instances = instances;
    vec![batch]
}

fn extract_high_load_text(world: &World) -> Vec<TextSection> {
    let Some(state) = world.get_resource::<HighLoadSceneState>() else {
        return Vec::new();
    };
    let total_ms = world
        .get_resource::<FrameTimings>()
        .map_or(0.0, |timings| timings.total_ms);
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
#[path = "tests/ecs_high_load.rs"]
mod tests;
