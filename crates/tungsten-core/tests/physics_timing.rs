//! Debug-mode physics timing probes; stdout is the signal.

use std::time::Instant;

use glam::Vec2;
use tungsten_core::assets::{
    LayerKind, TilemapData, TilemapInstance, TilemapLayer, TilemapRegistry,
};
use tungsten_core::physics::{
    physics_step, Collider, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten_core::{CommandBuffer, DeltaTime, World};

const N_BALLS: usize = 1_000;
const N_FRAMES: usize = 30;
const GRAVITY_Y: f32 = 900.0;
const BALL_RADIUS: f32 = 6.0;
const TILE: f32 = 16.0;
const MAP_COLS: u32 = 84;
const MAP_ROWS: u32 = 32;

fn build_world() -> World {
    let mut world = World::new();
    world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY_Y),
        broadphase_cell_size: 16.0,
        ..PhysicsConfig::default()
    });

    let mut tiles = vec![-1i32; (MAP_COLS * MAP_ROWS) as usize];
    let floor_row = MAP_ROWS - 1;
    for col in 0..MAP_COLS {
        let idx = (floor_row * MAP_COLS + col) as usize;
        tiles[idx] = 0;
    }
    let tilemap = TilemapData {
        tile_width: TILE as u32,
        tile_height: TILE as u32,
        width: MAP_COLS,
        height: MAP_ROWS,
        tileset: vec!["ground".into()],
        layers: vec![TilemapLayer {
            name: "collision".into(),
            kind: LayerKind::Collision,
            tiles,
        }],
    };
    let mut registry = TilemapRegistry::new();
    registry.insert("level".into(), tilemap);
    world.insert_resource(registry);

    let map = world.spawn();
    world.insert(map, TilemapInstance::new("level", Vec2::ZERO));

    let cols = 40usize;
    for i in 0..N_BALLS {
        let col = i % cols;
        let row = i / cols;
        let x = 10.0 + col as f32 * 14.0;
        let y = 60.0 + row as f32 * 14.0;
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(x, y)));
        world.insert(e, Velocity(Vec2::new(0.0, 0.0)));
        world.insert(e, Collider::circle(BALL_RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.85));
    }

    world
}

#[test]
fn physics_step_timing_probe_1000_balls() {
    let mut world = build_world();

    for _ in 0..3 {
        physics_step(&mut world);
    }

    let start = Instant::now();
    for _ in 0..N_FRAMES {
        physics_step(&mut world);
    }
    let elapsed = start.elapsed();
    let per_frame_us = elapsed.as_secs_f64() * 1_000_000.0 / N_FRAMES as f64;

    println!(
        "[physics_only] {N_BALLS} balls × {N_FRAMES} frames = {:.2}ms total, {:.1}µs/frame",
        elapsed.as_secs_f64() * 1000.0,
        per_frame_us
    );
}

#[test]
fn full_frame_timing_probe_1000_balls() {
    use tungsten_core::sync_position_to_transform;
    use tungsten_core::Transform;

    let mut world = build_world();

    let balls: Vec<_> = world.query_entities::<Position>();
    for e in balls {
        world.insert(e, Transform::from_position(Vec2::ZERO));
    }

    for _ in 0..3 {
        physics_step(&mut world);
        sync_position_to_transform(&mut world);
        let buf = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush(buf);
        world.insert_resource(CommandBuffer::new());
    }

    let start = Instant::now();
    for _ in 0..N_FRAMES {
        physics_step(&mut world);
        sync_position_to_transform(&mut world);
        let buf = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush(buf);
        world.insert_resource(CommandBuffer::new());
    }
    let elapsed = start.elapsed();
    let per_frame_us = elapsed.as_secs_f64() * 1_000_000.0 / N_FRAMES as f64;

    println!(
        "[full_frame] {N_BALLS} balls × {N_FRAMES} frames = {:.2}ms total, {:.1}µs/frame",
        elapsed.as_secs_f64() * 1000.0,
        per_frame_us
    );
}
