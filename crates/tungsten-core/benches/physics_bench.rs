//! Physics benches: micro (integration, broadphase) plus full `physics_step`
//! scenarios at multiple body counts.
//!
//! Full-step scenarios are deterministic (fixed `Pcg32` seeds, fixed dt) and
//! steady-state: each world is pre-settled or energy-conserving so per-iteration
//! work stays representative while `b.iter` mutates the world in place.
//! No `EventQueue<CollisionEvent>` resource is inserted, so collision events
//! are dropped inside the step (send cost is excluded, buffers stay bounded).

use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use glam::Vec2;
use tungsten_core::{
    physics_step, Aabb, Collider, DeltaTime, Pcg32, PhysicsConfig, Position, RigidBody,
    SpatialGrid, Velocity, World,
};

const DT: f32 = 1.0 / 60.0;
const BODY_RADIUS: f32 = 6.0;
const SPAWN_SPACING: f32 = 14.0;
const PILE_WIDTH: f32 = 1_920.0;
const FLOOR_Y: f32 = 1_080.0;
const GRAVITY_Y: f32 = 900.0;
/// Settle window: tight spawn grid reaches steady contact state well within this.
const SETTLE_STEPS: usize = 120;

fn base_world(gravity: Vec2) -> World {
    let mut world = World::new();
    world.insert_resource(DeltaTime { dt: DT });
    world.insert_resource(PhysicsConfig {
        gravity,
        ..PhysicsConfig::default()
    });
    world
}

/// Fully closed box (floor, roof, side walls) spanning `top_y..FLOOR_Y`.
///
/// Walls are massively thick: Gauss-Seidel in-place corrections under pile
/// pressure can shove a body past a thin wall's centerline, after which the
/// MTV ejects it out the far side and it free-falls forever, pinning the
/// global substep count at the cap. Thick walls keep the bench steady-state.
fn spawn_static_box(world: &mut World, width: f32, top_y: f32) {
    const WALL_HALF: f32 = 1_000.0;
    let mid_x = width * 0.5;
    let mid_y = (top_y + FLOOR_Y) * 0.5;
    let half_h = (FLOOR_Y - top_y) * 0.5;
    let walls = [
        // (center, half extents)
        (
            Vec2::new(mid_x, FLOOR_Y + WALL_HALF),
            Vec2::new(mid_x + WALL_HALF * 2.0, WALL_HALF),
        ),
        (
            Vec2::new(mid_x, top_y - WALL_HALF),
            Vec2::new(mid_x + WALL_HALF * 2.0, WALL_HALF),
        ),
        (
            Vec2::new(-WALL_HALF, mid_y),
            Vec2::new(WALL_HALF, half_h + WALL_HALF * 2.0),
        ),
        (
            Vec2::new(width + WALL_HALF, mid_y),
            Vec2::new(WALL_HALF, half_h + WALL_HALF * 2.0),
        ),
    ];
    for (center, half_extents) in walls {
        let entity = world.spawn();
        world.insert(entity, Position(center - half_extents));
        world.insert(entity, RigidBody::r#static());
        world.insert(
            entity,
            Collider::aabb(half_extents).with_offset(half_extents),
        );
    }
}

fn spawn_circle(world: &mut World, position: Vec2, velocity: Vec2, radius: f32, restitution: f32) {
    let entity = world.spawn();
    world.insert(entity, Position(position));
    world.insert(entity, Velocity(velocity));
    world.insert(entity, RigidBody::dynamic().with_restitution(restitution));
    world.insert(
        entity,
        Collider::circle(radius).with_offset(Vec2::splat(radius)),
    );
}

/// Tight grid of circles just above the floor, mirroring the in-app
/// physics-stress scene: settles into a dense persistent-contact pile.
fn spawn_pile(world: &mut World, count: usize, rng: &mut Pcg32) -> f32 {
    let usable_width = PILE_WIDTH - SPAWN_SPACING * 2.0;
    let cols = ((usable_width / SPAWN_SPACING).floor() as usize).max(1);
    let rows = count.div_ceil(cols);
    let start_y = FLOOR_Y - SPAWN_SPACING - rows as f32 * SPAWN_SPACING;

    for index in 0..count {
        let col = index % cols;
        let row = index / cols;
        let jitter_x = rng.next_range(-0.2, 0.2) * SPAWN_SPACING;
        let jitter_y = rng.next_range(-0.2, 0.2) * SPAWN_SPACING;
        let x = (SPAWN_SPACING + col as f32 * SPAWN_SPACING + jitter_x)
            .clamp(0.0, PILE_WIDTH - BODY_RADIUS * 2.0);
        let y = start_y + row as f32 * SPAWN_SPACING + jitter_y;
        spawn_circle(world, Vec2::new(x, y), Vec2::ZERO, BODY_RADIUS, 0.1);
    }
    start_y
}

fn settle(world: &mut World, steps: usize) {
    for _ in 0..steps {
        physics_step(world);
    }
}

/// Dense settled pile under gravity: persistent contacts, ~1 substep/step.
fn build_dense_pile(count: usize) -> World {
    let mut world = base_world(Vec2::new(0.0, GRAVITY_Y));
    let mut rng = Pcg32::seeded(0x7C0F_FEE5);
    let rows = count.div_ceil(((PILE_WIDTH - 28.0) / SPAWN_SPACING) as usize);
    let top_y = FLOOR_Y - SPAWN_SPACING * (rows as f32 + 2.0) - 500.0;
    spawn_static_box(&mut world, PILE_WIDTH, top_y);
    spawn_pile(&mut world, count, &mut rng);
    settle(&mut world, SETTLE_STEPS);
    world
}

/// Fast elastic bullets bouncing in a closed box, no gravity: every body
/// exceeds the substep travel threshold, so this drives the max-substep +
/// speculative-sweep path continuously. Box area scales with count (~8% fill).
fn build_projectile_stream(count: usize) -> World {
    const PROJECTILE_RADIUS: f32 = 4.0;
    const PROJECTILE_SPEED: f32 = 2_400.0;

    let area_per_body = std::f32::consts::PI * PROJECTILE_RADIUS * PROJECTILE_RADIUS;
    let side = (count as f32 * area_per_body / 0.08).sqrt().max(512.0);

    let mut world = base_world(Vec2::ZERO);
    spawn_static_box(&mut world, side, FLOOR_Y - side);

    let mut rng = Pcg32::seeded(0xB1A5_7B17);
    let cols = (count as f32).sqrt().ceil() as usize;
    let spacing = side / (cols as f32 + 1.0);
    for index in 0..count {
        let col = index % cols;
        let row = index / cols;
        let x = spacing + col as f32 * spacing;
        let y = FLOOR_Y - side + spacing + row as f32 * spacing;
        let dir = rng.next_unit_vec2();
        spawn_circle(
            &mut world,
            Vec2::new(x, y),
            dir * PROJECTILE_SPEED,
            PROJECTILE_RADIUS,
            1.0,
        );
    }
    world
}

/// Settled pile around a grid of static AABB pillars: mixed static/dynamic
/// narrow phase plus per-substep re-insertion of many static proxies.
fn build_mixed(count: usize) -> World {
    const PILLAR_HALF: f32 = 12.0;
    const PILLAR_STRIDE: f32 = 96.0;

    let mut world = base_world(Vec2::new(0.0, GRAVITY_Y));
    let mut rng = Pcg32::seeded(0x5EED_CAFE);
    let rows = count.div_ceil(((PILE_WIDTH - 28.0) / SPAWN_SPACING) as usize);
    let top_y = FLOOR_Y - SPAWN_SPACING * (rows as f32 + 2.0) - 500.0;
    spawn_static_box(&mut world, PILE_WIDTH, top_y);

    // Pillar band tall enough that the settled pile buries several rows.
    let band_top = FLOOR_Y - 1_600.0;
    let mut y = FLOOR_Y - PILLAR_STRIDE;
    while y > band_top {
        let mut x = PILLAR_STRIDE;
        while x < PILE_WIDTH - PILLAR_STRIDE * 0.5 {
            let entity = world.spawn();
            world.insert(entity, Position(Vec2::new(x, y)));
            world.insert(entity, RigidBody::r#static());
            world.insert(entity, Collider::aabb(Vec2::splat(PILLAR_HALF)));
            x += PILLAR_STRIDE;
        }
        y -= PILLAR_STRIDE;
    }

    spawn_pile(&mut world, count, &mut rng);
    settle(&mut world, SETTLE_STEPS);
    world
}

/// Settled pile plus one fast elastic bullet: a single fast body forces the
/// global substep count to the cap, multiplying whole-world work.
fn build_pile_plus_bullet(count: usize) -> World {
    let mut world = build_dense_pile(count);
    let rows = count.div_ceil(((PILE_WIDTH - 28.0) / SPAWN_SPACING) as usize);
    let above_pile = FLOOR_Y - SPAWN_SPACING * (rows as f32 + 2.0) - 200.0;
    spawn_circle(
        &mut world,
        Vec2::new(PILE_WIDTH * 0.5, above_pile),
        Vec2::new(2_500.0, 0.0),
        4.0,
        1.0,
    );
    world
}

fn bench_physics_step_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("physics_step");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(12));
    group.warm_up_time(Duration::from_secs(2));

    let scenarios: &[(&str, fn(usize) -> World, &[usize])] = &[
        ("dense_pile", build_dense_pile, &[3_000, 10_000, 25_000]),
        (
            "projectile_stream",
            build_projectile_stream,
            &[3_000, 10_000, 25_000],
        ),
        ("mixed_static_dynamic", build_mixed, &[10_000, 25_000]),
        (
            "pile_plus_bullet",
            build_pile_plus_bullet,
            &[10_000, 25_000],
        ),
    ];

    for (name, build, counts) in scenarios {
        for &count in counts.iter() {
            let mut world = build(count);
            group.bench_with_input(BenchmarkId::new(*name, count), &count, |b, _| {
                b.iter(|| {
                    physics_step(black_box(&mut world));
                });
            });
        }
    }

    group.finish();
}

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
    bench_physics_step_scenarios,
);
criterion_main!(benches);
