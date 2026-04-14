/// M12 ECS benchmarks — archetypal storage vs. naive HashMap baseline.
///
/// Measures:
///   - `spawn_insert` — 10k entity spawn + 3-component insert
///   - `query_single` — iterate 10k entities via `query::<Position>()`
///   - `query2_homogeneous` — iterate 10k entities (one archetype) via
///                            `query2::<Position, Velocity>()`
///   - `query2_fragmented` — iterate 10k entities spread across 5 archetypes
///                           (different component supersets) via `query2`
///   - `naive_query_single` — same workload via a minimal HashMap simulation
///                            (stand-in for the pre-M12 storage)
///
/// Results are documented in DECISIONS.md D-036.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use tungsten_core::ecs::World;

// ---------------------------------------------------------------------------
// Component types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[derive(Clone)]
struct Name(String);

#[derive(Clone, Copy)]
struct Health(f32);

#[derive(Clone, Copy)]
struct Mass(f32);

const N: usize = 10_000;

// ---------------------------------------------------------------------------
// Benchmark: spawn + insert (construction cost)
// ---------------------------------------------------------------------------

fn bench_spawn_insert(c: &mut Criterion) {
    c.bench_function("spawn_insert_3_components_10k", |b| {
        b.iter(|| {
            let mut world = World::new();
            for i in 0..N {
                let e = world.spawn();
                world.insert(
                    e,
                    Position {
                        x: i as f32,
                        y: 0.0,
                    },
                );
                world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
                world.insert(e, Name(format!("entity_{i}")));
            }
            black_box(world);
        });
    });
}

// ---------------------------------------------------------------------------
// Benchmark: query<Position> — single-type iteration, 10k entities
// ---------------------------------------------------------------------------

fn bench_query_single(c: &mut Criterion) {
    let mut world = World::new();
    for i in 0..N {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e, Health(100.0));
    }

    c.bench_function("query_single_10k", |b| {
        b.iter(|| {
            let sum: f32 = world.query::<Position>().map(|(_, p)| p.x).sum();
            black_box(sum);
        });
    });
}

// ---------------------------------------------------------------------------
// Benchmark: query2<Position, Velocity> — homogeneous (one archetype)
// ---------------------------------------------------------------------------

fn bench_query2_homogeneous(c: &mut Criterion) {
    let mut world = World::new();
    for i in 0..N {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
    }

    c.bench_function("query2_homogeneous_10k", |b| {
        b.iter(|| {
            let sum: f32 = world
                .query2::<Position, Velocity>()
                .map(|(_, p, v)| p.x + v.dx)
                .sum();
            black_box(sum);
        });
    });
}

// ---------------------------------------------------------------------------
// Benchmark: query2<Position, Velocity> — fragmented (5 archetypes)
//
// 2k entities per archetype:
//   arch 1: {Position, Velocity}
//   arch 2: {Position, Velocity, Health}
//   arch 3: {Position, Velocity, Mass}
//   arch 4: {Position, Velocity, Health, Mass}
//   arch 5: {Position, Velocity, Name}
// ---------------------------------------------------------------------------

fn bench_query2_fragmented(c: &mut Criterion) {
    let mut world = World::new();
    let chunk = N / 5;

    for i in 0..chunk {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
    }
    for i in 0..chunk {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e, Health(100.0));
    }
    for i in 0..chunk {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e, Mass(1.0));
    }
    for i in 0..chunk {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e, Health(100.0));
        world.insert(e, Mass(1.0));
    }
    for i in 0..chunk {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e, Name(format!("e{i}")));
    }

    c.bench_function("query2_fragmented_5arch_10k", |b| {
        b.iter(|| {
            let sum: f32 = world
                .query2::<Position, Velocity>()
                .map(|(_, p, v)| p.x + v.dx)
                .sum();
            black_box(sum);
        });
    });
}

// ---------------------------------------------------------------------------
// Naive baseline: minimal HashMap simulation of the pre-M12 storage.
//
// This is an inline reproduction of the pre-M12 ComponentStore approach:
//   HashMap<TypeId, HashMap<u32, Box<dyn Any>>>
// Used to establish a comparison point for D-036's benchmark results.
// ---------------------------------------------------------------------------

struct NaiveWorld {
    next_id: u32,
    stores: HashMap<TypeId, HashMap<u32, Box<dyn Any>>>,
}

impl NaiveWorld {
    fn new() -> Self {
        Self {
            next_id: 0,
            stores: HashMap::new(),
        }
    }

    fn spawn(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn insert<T: 'static>(&mut self, id: u32, val: T) {
        self.stores
            .entry(TypeId::of::<T>())
            .or_default()
            .insert(id, Box::new(val));
    }

    fn query<T: 'static>(&self) -> impl Iterator<Item = (u32, &T)> {
        self.stores
            .get(&TypeId::of::<T>())
            .into_iter()
            .flat_map(|store| {
                store
                    .iter()
                    .filter_map(|(&id, v)| v.downcast_ref::<T>().map(|c| (id, c)))
            })
    }

    /// Simulate query2 via query + per-entity HashMap lookup (old mutation pattern).
    fn query_entities<T: 'static>(&self) -> Vec<u32> {
        self.stores
            .get(&TypeId::of::<T>())
            .map(|s| s.keys().copied().collect())
            .unwrap_or_default()
    }

    fn get<T: 'static>(&self, id: u32) -> Option<&T> {
        self.stores
            .get(&TypeId::of::<T>())?
            .get(&id)?
            .downcast_ref::<T>()
    }
}

fn bench_naive_query_single(c: &mut Criterion) {
    let mut world = NaiveWorld::new();
    for i in 0..N {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e, Health(100.0));
    }

    c.bench_function("naive_query_single_10k", |b| {
        b.iter(|| {
            let sum: f32 = world.query::<Position>().map(|(_, p)| p.x).sum();
            black_box(sum);
        });
    });
}

fn bench_naive_query2_via_entities(c: &mut Criterion) {
    let mut world = NaiveWorld::new();
    for i in 0..N {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: i as f32,
                y: 0.0,
            },
        );
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
    }

    c.bench_function("naive_query2_via_entities_10k", |b| {
        b.iter(|| {
            let entities = world.query_entities::<Position>();
            let sum: f32 = entities
                .iter()
                .filter_map(|&id| {
                    let p = world.get::<Position>(id)?;
                    let v = world.get::<Velocity>(id)?;
                    Some(p.x + v.dx)
                })
                .sum();
            black_box(sum);
        });
    });
}

// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_spawn_insert,
    bench_query_single,
    bench_query2_homogeneous,
    bench_query2_fragmented,
    bench_naive_query_single,
    bench_naive_query2_via_entities,
);
criterion_main!(benches);
