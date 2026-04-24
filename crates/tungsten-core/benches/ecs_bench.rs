/// ECS benches: archetypal storage vs naive `HashMap` baseline.
///
/// D-036 result set.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use tungsten_core::{CommandBuffer, EventQueue, World};

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Position {
    x: f32,
    y: f32,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Velocity {
    dx: f32,
    dy: f32,
}

#[allow(dead_code)]
#[derive(Clone)]
struct Name(String);

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Health(f32);

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Mass(f32);

const N: usize = 10_000;

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

fn bench_query2_10k_5archetypes_pv(c: &mut Criterion) {
    use glam::Vec2;
    use tungsten_core::{Collider, Position, RigidBody, Shape, Velocity, World};

    const CHUNK: usize = 2_000;

    let mut world = World::new();

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 0.0)));
        world.insert(e, Velocity(Vec2::new(1.0, 0.0)));
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 100.0)));
        world.insert(e, Velocity(Vec2::splat(0.5)));
        world.insert(e, RigidBody::dynamic());
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 200.0)));
        world.insert(e, Velocity(Vec2::ONE));
        world.insert(
            e,
            Collider {
                shape: Shape::Aabb {
                    half_extents: Vec2::splat(8.0),
                },
                offset: Vec2::ZERO,
            },
        );
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 300.0)));
        world.insert(e, Velocity(Vec2::new(0.25, 0.75)));
        world.insert(e, RigidBody::r#static());
        world.insert(
            e,
            Collider {
                shape: Shape::Circle { radius: 8.0 },
                offset: Vec2::ZERO,
            },
        );
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(i as f32, 400.0)));
        world.insert(e, Velocity(Vec2::new(0.75, 0.25)));
        world.insert(e, Health(100.0));
        world.insert(e, Mass(1.0));
    }

    c.bench_function("query2_10k_5archetypes_pv", |b| {
        b.iter(|| {
            let sum: Vec2 = world
                .query2::<Position, Velocity>()
                .fold(Vec2::ZERO, |acc, (_, p, v)| acc + p.0 + v.0);
            black_box(sum);
        });
    });
}

fn bench_spawn_despawn_1k(c: &mut Criterion) {
    use glam::Vec2;
    use tungsten_core::{Position, World};

    c.bench_function("spawn_despawn_1k", |b| {
        b.iter(|| {
            let mut world = World::new();
            let entities: Vec<_> = (0..1_000u32)
                .map(|i| {
                    let e = world.spawn();
                    world.insert(e, Position(Vec2::new(i as f32, 0.0)));
                    e
                })
                .collect();
            for e in &entities {
                world.despawn(*e);
            }
            black_box(world);
        });
    });
}

fn bench_command_buffer_flush_1k(c: &mut Criterion) {
    c.bench_function("command_buffer_flush_1k_spawns", |b| {
        b.iter(|| {
            let mut world = World::new();
            let mut buf = CommandBuffer::new();
            for i in 0..1_000u32 {
                let pending = buf.spawn();
                buf.insert_pending(
                    pending,
                    Position {
                        x: i as f32,
                        y: 0.0,
                    },
                );
                buf.insert_pending(pending, Velocity { dx: 1.0, dy: 0.0 });
            }
            world.flush(buf);
            black_box(&world);
        });
    });
}

// D-036 baseline: HashMap<TypeId, HashMap<u32, Box<dyn Any>>>.
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

    /// Old query2 shape: query plus per-entity `HashMap` lookup.
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

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev00(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev01(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev02(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev03(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev04(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev05(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev06(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev07(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev08(u32);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Ev09(u32);

fn bench_event_queue_flush_10_types(c: &mut Criterion) {
    c.bench_function("event_queue_flush_10_types", |b| {
        b.iter(|| {
            let mut q0: EventQueue<Ev00> = EventQueue::new();
            let mut q1: EventQueue<Ev01> = EventQueue::new();
            let mut q2: EventQueue<Ev02> = EventQueue::new();
            let mut q3: EventQueue<Ev03> = EventQueue::new();
            let mut q4: EventQueue<Ev04> = EventQueue::new();
            let mut q5: EventQueue<Ev05> = EventQueue::new();
            let mut q6: EventQueue<Ev06> = EventQueue::new();
            let mut q7: EventQueue<Ev07> = EventQueue::new();
            let mut q8: EventQueue<Ev08> = EventQueue::new();
            let mut q9: EventQueue<Ev09> = EventQueue::new();

            for i in 0..100u32 {
                q0.send(Ev00(i));
                q1.send(Ev01(i));
                q2.send(Ev02(i));
                q3.send(Ev03(i));
                q4.send(Ev04(i));
                q5.send(Ev05(i));
                q6.send(Ev06(i));
                q7.send(Ev07(i));
                q8.send(Ev08(i));
                q9.send(Ev09(i));
            }

            black_box(&mut q0).flush();
            black_box(&mut q1).flush();
            black_box(&mut q2).flush();
            black_box(&mut q3).flush();
            black_box(&mut q4).flush();
            black_box(&mut q5).flush();
            black_box(&mut q6).flush();
            black_box(&mut q7).flush();
            black_box(&mut q8).flush();
            black_box(&mut q9).flush();
        });
    });
}

// D-042 gate: query3 sprite path across matching and excluded archetypes.
fn bench_sprite_components_query3_2k(c: &mut Criterion) {
    use glam::Vec2;
    use tungsten_core::{Sprite, Tag, Transform, Visibility, World};

    const CHUNK: usize = 2_000 / 2;

    let mut world = World::new();

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::new(i as f32, 0.0)));
        world.insert(e, Sprite::new("a"));
        world.insert(e, Visibility::default());
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::new(i as f32, 1.0)));
        world.insert(e, Sprite::new("b"));
        world.insert(e, Visibility::default());
        world.insert(e, Tag::new("b"));
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::new(i as f32, 2.0)));
        world.insert(e, Sprite::new("c"));
    }

    for i in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::new(i as f32, 3.0)));
        world.insert(e, Visibility::default());
    }

    for _ in 0..CHUNK {
        let e = world.spawn();
        world.insert(e, Sprite::new("e"));
        world.insert(e, Visibility::default());
    }

    c.bench_function("sprite_components_query3_2k", |b| {
        b.iter(|| {
            let sum: i64 = world
                .query3::<Transform, Sprite, Visibility>()
                .map(|(_, _, s, _)| i64::from(s.z_order))
                .sum();
            black_box(sum);
        });
    });
}

criterion_group!(
    benches,
    bench_spawn_insert,
    bench_query_single,
    bench_query2_homogeneous,
    bench_query2_fragmented,
    bench_query2_10k_5archetypes_pv,
    bench_spawn_despawn_1k,
    bench_command_buffer_flush_1k,
    bench_naive_query_single,
    bench_naive_query2_via_entities,
    bench_event_queue_flush_10_types,
    bench_sprite_components_query3_2k,
);
criterion_main!(benches);
