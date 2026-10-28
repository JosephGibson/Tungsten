//! The tuple queries (`D-130`) against the slice forms on a fragmented
//! world. Each pair does the same work per row, so the difference is the
//! iteration.
//!
//! `single_mut`, `single_read` and `entity_mut` measured the single-component
//! `query_mut::<T>()` and `query::<T>()` on the W15 spike's tree, which now
//! take query data; their `arity` side is that function's loop rebuilt on
//! the slice forms, the entities zipped with the column, which compiles to
//! the index loop the spike measured. The pairs against the arity-named
//! queries left with them (redo 7.1); the `ecs` benchmark runs those shapes.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use glam::Vec2;
use tungsten_core::{Entity, World};

#[derive(Clone, Copy)]
struct Pos(Vec2);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Vel(Vec2);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Acc(Vec2);
#[derive(Clone, Copy)]
struct Health(f32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Regen(f32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Stats([f32; 8]);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Cooldowns([f32; 4]);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Brain {
    timer: f32,
    seed: u32,
}
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Team(u8);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Bag([u16; 16]);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Follow(Entity);
#[derive(Clone, Copy)]
struct Tag<const N: u8>;

const ENTITIES: usize = 100_000;
const VIEWPORT: Vec2 = Vec2::new(1920.0, 1080.0);

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
}

/// The `ecs` benchmark's population shape: 8 tag combinations, `Acc` on
/// 80%, `Regen` on 40%, `Stats` on 50%, `Bag` on 25%, `Follow` on 10%.
fn world() -> World {
    let mut world = World::new();
    let mut rng = Lcg(7);
    let mut entities = Vec::with_capacity(ENTITIES);
    let mut followers = Vec::new();
    for index in 0..ENTITIES {
        let entity = world.spawn();
        let combination = rng.next() % 8;
        if combination & 1 != 0 {
            world.insert(entity, Tag::<0>);
        }
        if combination & 2 != 0 {
            world.insert(entity, Tag::<1>);
        }
        if combination & 4 != 0 {
            world.insert(entity, Tag::<2>);
        }
        world.insert(entity, Team((rng.next() % 4) as u8));
        world.insert(entity, Health(40.0 + 60.0 * rng.unit()));
        world.insert(
            entity,
            Pos(Vec2::new(rng.unit() * VIEWPORT.x, rng.unit() * VIEWPORT.y)),
        );
        world.insert(
            entity,
            Vel(Vec2::new(rng.unit() - 0.5, rng.unit() - 0.5) * 100.0),
        );
        world.insert(
            entity,
            Brain {
                timer: rng.unit(),
                seed: rng.next() | 1,
            },
        );
        world.insert(entity, Cooldowns([rng.unit(); 4]));
        if rng.unit() < 0.8 {
            world.insert(
                entity,
                Acc(Vec2::new(rng.unit() - 0.5, rng.unit() - 0.5) * 10.0),
            );
        }
        if rng.unit() < 0.4 {
            world.insert(entity, Regen(1.0 + 5.0 * rng.unit()));
        }
        if rng.unit() < 0.5 {
            world.insert(entity, Stats([1.0; 8]));
        }
        if rng.unit() < 0.25 {
            world.insert(
                entity,
                Bag(std::array::from_fn(|slot| ((index + slot) % 17) as u16)),
            );
        }
        if rng.unit() < 0.1 {
            followers.push(index);
        }
        entities.push(entity);
    }
    for &index in &followers {
        let leader = (index + 1) % ENTITIES;
        world.insert(entities[index], Follow(entities[leader]));
    }
    world
}

fn pair<F, G>(
    c: &mut Criterion,
    name: &str,
    rows: usize,
    world: &mut World,
    mut arity: F,
    mut tuple: G,
) where
    F: FnMut(&mut World),
    G: FnMut(&mut World),
{
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Elements(rows as u64));
    group.bench_function(BenchmarkId::new("arity", rows), |b| b.iter(|| arity(world)));
    group.bench_function(BenchmarkId::new("tuple", rows), |b| b.iter(|| tuple(world)));
    group.finish();
}

fn shapes(c: &mut Criterion) {
    let mut world = world();
    let rows = world.query::<&Pos>().count();

    // bounds_wrap: one mutable column, a branchy body.
    let wrap = |point: &mut Vec2| {
        if point.x < 0.0 {
            point.x += VIEWPORT.x;
        } else if point.x >= VIEWPORT.x {
            point.x -= VIEWPORT.x;
        }
        if point.y < 0.0 {
            point.y += VIEWPORT.y;
        } else if point.y >= VIEWPORT.y {
            point.y -= VIEWPORT.y;
        }
    };
    {
        pair(
            c,
            "single_mut",
            rows,
            &mut world,
            |world| {
                for (_, (entities, column)) in world.query_mut_slices::<(Entity, &mut Pos)>() {
                    for (_, position) in entities.iter().zip(column.iter_mut()) {
                        wrap(&mut position.0);
                    }
                }
            },
            |world| {
                for position in world.query_mut::<&mut Pos>() {
                    wrap(&mut position.0);
                }
            },
        );
    }

    // digest: one shared column, a reduction.
    {
        pair(
            c,
            "single_read",
            rows,
            &mut world,
            |world| {
                let mut hash = 0_u64;
                for (_, (entities, column)) in world.query_slices::<(Entity, &Pos)>() {
                    for (_, position) in entities.iter().zip(column.iter()) {
                        hash = (hash ^ u64::from(position.0.x.to_bits()))
                            .wrapping_mul(0x100_0000_01b3);
                    }
                }
                black_box(hash);
            },
            |world| {
                let mut hash = 0_u64;
                for position in world.query::<&Pos>() {
                    hash = (hash ^ u64::from(position.0.x.to_bits())).wrapping_mul(0x100_0000_01b3);
                }
                black_box(hash);
            },
        );
    }

    // churn's scan: the entity beside a mutable column.
    {
        pair(
            c,
            "entity_mut",
            rows,
            &mut world,
            |world| {
                let mut picked = 0_u32;
                for (_, (entities, column)) in world.query_mut_slices::<(Entity, &mut Health)>() {
                    for (&entity, health) in entities.iter().zip(column.iter_mut()) {
                        if health.0 < 41.0 {
                            picked = picked.wrapping_add(entity.id());
                        }
                    }
                }
                black_box(picked);
            },
            |world| {
                let mut picked = 0_u32;
                for (entity, health) in world.query_mut::<(Entity, &mut Health)>() {
                    if health.0 < 41.0 {
                        picked = picked.wrapping_add(entity.id());
                    }
                }
                black_box(picked);
            },
        );
    }
}

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(4))
}

criterion_group! {
    name = benches;
    config = config();
    targets = shapes
}
criterion_main!(benches);
