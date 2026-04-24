//! Physics micro-benches: position integration and broadphase rebuild.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glam::Vec2;
use tungsten_core::{Aabb, Position, RigidBody, SpatialGrid, Velocity, World};

fn bench_position_integration_50k(c: &mut Criterion) {
    const N: usize = 50_000;

    let mut world = World::new();
    for i in 0..N {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 0.0)));
        world.insert(e, Velocity(Vec2::new(1.0, 0.5)));
        world.insert(e, RigidBody::dynamic());
    }
    let dt = 1.0_f32 / 60.0;

    c.bench_function("position_integration_50k", |b| {
        b.iter(|| {
            let entities = world.query2_entities::<Position, Velocity>();
            for entity in &entities {
                if let (Some(vel), Some(pos)) = (
                    world.get::<Velocity>(*entity).map(|v| v.0),
                    world.get_mut::<Position>(*entity),
                ) {
                    pos.0 += vel * black_box(dt);
                }
            }
        });
    });
}

fn bench_broadphase_rebuild_5k(c: &mut Criterion) {
    const N: usize = 5_000;
    let cell_size = 32.0_f32;
    let half_extent = Vec2::splat(8.0);

    let positions: Vec<Vec2> = (0..N)
        .map(|i| Vec2::new((i % 100) as f32 * 16.0, (i / 100) as f32 * 16.0))
        .collect();

    c.bench_function("broadphase_rebuild_5k_dynamic", |b| {
        b.iter(|| {
            let mut grid = SpatialGrid::new(cell_size);
            for (id, &center) in positions.iter().enumerate() {
                let aabb = Aabb::new(center, half_extent);
                grid.insert(id as u32, &aabb);
            }
            let query_aabb = Aabb::new(Vec2::new(800.0, 400.0), Vec2::splat(100.0));
            let mut out = Vec::new();
            grid.query(&query_aabb, None, &mut out);
            black_box(out.len());
        });
    });
}

criterion_group!(
    benches,
    bench_position_integration_50k,
    bench_broadphase_rebuild_5k,
);
criterion_main!(benches);
