//! Physics + render stress: N dynamic circle bodies with colliders piling
//! under gravity inside a static box, each rendered through the default
//! sprite extract path (`Transform + Sprite + Visibility`, registry ID
//! resolution, z-sort, batching).
//!
//! Unlike `ecs-high-load` (RigidBody but no Collider — integration only),
//! this scene drives the broadphase grid, narrow phase, and Gauss-Seidel
//! solver with a dense persistent-contact pile at steady state.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::App;
use tungsten::core::{
    AssetRegistry, CameraState, Collider, FilterMode, PhysicsConfig, Position, RigidBody, Sprite,
    Transform, Velocity, Visibility, World, sync_position_to_transform,
};
use tungsten::render::Renderer;

use crate::shared::{TelemetryState, log_telemetry};

pub(crate) const DEFAULT_PHYSICS_STRESS_COUNT: usize = 3_000;

const WORLD_WIDTH: f32 = 1_920.0;
const WORLD_HEIGHT: f32 = 1_080.0;
const GRAVITY_Y: f32 = 900.0;
const BODY_RADIUS: f32 = 6.0;
const BODY_SPRITE_SIZE_PX: u32 = 12;
const BODY_RESTITUTION: f32 = 0.1;
const SPAWN_SPACING: f32 = 14.0;
const WALL_THICKNESS: f32 = 80.0;
const SPRITE_ID: &str = "ex02_physics_stress_body";
const SPRITE_PATH: &str = "__generated__/ex02_physics_stress_body.png";

pub(crate) fn configure_physics_stress_scene(app: &mut App, body_count: usize) {
    {
        let world = app.world_mut();
        world.insert_resource(TelemetryState::default());
        if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
            cfg.gravity = Vec2::new(0.0, GRAVITY_Y);
        }
        if let Some(cam) = world.get_resource_mut::<CameraState>() {
            cam.zoom = 1.0;
            cam.position = Vec2::ZERO;
        }
        seed_physics_world(world, body_count);
    }

    app.on_startup(|world, renderer| {
        register_body_sprite(world, renderer);
    });

    app.add_system_named("physics_step", tungsten::physics::physics_step);
    app.add_system_named("sync_position_to_transform", sync_position_to_transform);
    app.add_system_named("log_telemetry", log_telemetry);
    // No set_extract_sprites: the engine's default extract path is the
    // render-side load under measurement.
}

fn seed_physics_world(world: &mut World, body_count: usize) {
    spawn_static_box(world);

    if body_count == 0 {
        return;
    }

    // Grid spawn just above the floor so the pile settles within warm-up.
    let usable_width = WORLD_WIDTH - SPAWN_SPACING * 2.0;
    let cols = ((usable_width / SPAWN_SPACING).floor() as usize).max(1);
    let rows = body_count.div_ceil(cols);
    let start_y = WORLD_HEIGHT - SPAWN_SPACING - rows as f32 * SPAWN_SPACING;

    for index in 0..body_count {
        let col = index % cols;
        let row = index / cols;
        let jitter_x = (hash_unit(index as u32, 0x9E37_79B9) - 0.5) * SPAWN_SPACING * 0.4;
        let jitter_y = (hash_unit(index as u32, 0x85EB_CA6B) - 0.5) * SPAWN_SPACING * 0.4;
        let x = (SPAWN_SPACING + col as f32 * SPAWN_SPACING + jitter_x)
            .clamp(0.0, WORLD_WIDTH - BODY_RADIUS * 2.0);
        let y = start_y + row as f32 * SPAWN_SPACING + jitter_y;
        let tint_seed = hash_unit(index as u32, 0x2468_1357);

        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(x, y)));
        world.insert(entity, Velocity(Vec2::ZERO));
        world.insert(
            entity,
            RigidBody::dynamic().with_restitution(BODY_RESTITUTION),
        );
        world.insert(
            entity,
            Collider::circle(BODY_RADIUS).with_offset(Vec2::splat(BODY_RADIUS)),
        );
        world.insert(entity, Transform::from_position(Vec2::new(x, y)));
        world.insert(
            entity,
            Sprite {
                asset_id: SPRITE_ID.into(),
                color: body_color(tint_seed),
                z_order: 0,
                material_id: None,
            },
        );
        world.insert(entity, Visibility::default());
    }
}

/// Floor plus side walls as static AABB colliders; open top.
fn spawn_static_box(world: &mut World) {
    let walls = [
        // (top-left position, half extents)
        (
            Vec2::new(-WALL_THICKNESS, WORLD_HEIGHT),
            Vec2::new(WORLD_WIDTH * 0.5 + WALL_THICKNESS, WALL_THICKNESS * 0.5),
        ),
        (
            Vec2::new(-WALL_THICKNESS, -WORLD_HEIGHT),
            Vec2::new(WALL_THICKNESS * 0.5, WORLD_HEIGHT * 1.5),
        ),
        (
            Vec2::new(WORLD_WIDTH, -WORLD_HEIGHT),
            Vec2::new(WALL_THICKNESS * 0.5, WORLD_HEIGHT * 1.5),
        ),
    ];

    for (position, half_extents) in walls {
        let entity = world.spawn();
        world.insert(entity, Position(position));
        world.insert(entity, RigidBody::r#static());
        world.insert(
            entity,
            Collider::aabb(half_extents).with_offset(half_extents),
        );
    }
}

fn register_body_sprite(world: &mut World, renderer: &mut Renderer) {
    let rgba = build_body_sprite_rgba();
    let handle = renderer.allocate_texture_handle();
    {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        registry.register_sprite(
            SPRITE_ID.to_string(),
            FilterMode::Nearest,
            BODY_SPRITE_SIZE_PX,
            BODY_SPRITE_SIZE_PX,
            PathBuf::from(SPRITE_PATH),
            handle,
            tungsten::core::assets::UvRect::FULL,
            None,
            None,
            None,
        );
    }
    renderer.upload_texture(
        handle,
        &rgba,
        BODY_SPRITE_SIZE_PX,
        BODY_SPRITE_SIZE_PX,
        FilterMode::Nearest,
    );
}

fn build_body_sprite_rgba() -> Vec<u8> {
    let size = BODY_SPRITE_SIZE_PX as f32;
    let mut rgba = Vec::with_capacity((BODY_SPRITE_SIZE_PX * BODY_SPRITE_SIZE_PX * 4) as usize);
    let center = size * 0.5 - 0.5;
    let radius = center;

    for y in 0..BODY_SPRITE_SIZE_PX {
        for x in 0..BODY_SPRITE_SIZE_PX {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let edge = ((radius - dist) / radius).clamp(0.0, 1.0);
            let brightness = (170.0 + edge * 85.0).round() as u8;
            let alpha = if dist <= radius { 255 } else { 0 };
            rgba.extend_from_slice(&[255, brightness, brightness.saturating_sub(40), alpha]);
        }
    }

    rgba
}

fn body_color(seed: f32) -> [u8; 4] {
    let r = ((seed * 5.7).sin() * 0.5 + 0.5) * 100.0 + 155.0;
    let g = ((seed * 7.3 + 1.7).sin() * 0.5 + 0.5) * 140.0 + 80.0;
    let b = ((seed * 9.1 + 4.4).sin() * 0.5 + 0.5) * 90.0 + 60.0;
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
