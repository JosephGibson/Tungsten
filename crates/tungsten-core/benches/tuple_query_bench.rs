//! The arity-named queries against the tuple queries (`D-130`) on the same
//! fragmented world, one pair per shape the `ecs` benchmark runs. Each pair
//! does the same work per row, so the difference is the iteration.
//!
//! `single_mut`, `single_read` and `entity_mut` measured the single-component
//! `query_mut::<T>()` and `query::<T>()` on the W15 spike's tree, which now
//! take query data; their `arity` side is that function's loop rebuilt on
//! the slice forms, the entities zipped with the column, which compiles to
//! the index loop the spike measured.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use glam::Vec2;
use tungsten_core::{Entity, OptionalColumn, With, Without, World};

#[derive(Clone, Copy)]
struct Pos(Vec2);
#[derive(Clone, Copy)]
struct Vel(Vec2);
#[derive(Clone, Copy)]
struct Acc(Vec2);
#[derive(Clone, Copy)]
struct Health(f32);
#[derive(Clone, Copy)]
struct Regen(f32);
#[derive(Clone, Copy)]
struct Stats([f32; 8]);
#[derive(Clone, Copy)]
struct Cooldowns([f32; 4]);
#[derive(Clone, Copy)]
struct Brain {
    timer: f32,
    seed: u32,
}
#[derive(Clone, Copy)]
struct Team(u8);
#[derive(Clone, Copy)]
struct Bag([u16; 16]);
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Follow(Entity);
#[derive(Clone, Copy)]
struct Tag<const N: u8>;

const ENTITIES: usize = 100_000;
const DT: f32 = 1.0 / 60.0;
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

fn xorshift(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
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

// The `arity` side calls the functions deprecated since 0.54.0; it goes when
// W4b removes them.
#[allow(deprecated)]
fn shapes(c: &mut Criterion) {
    let mut world = world();
    let rows = world.query_entities::<Pos>().len();

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

    // integrate: a mutable and a shared column.
    {
        pair(
            c,
            "pair_mut_read",
            rows,
            &mut world,
            |world| {
                for (_, position, vel) in world.query2_mut::<Pos, Vel>() {
                    position.0 += vel.0 * DT;
                }
            },
            |world| {
                for (position, vel) in world.query_mut::<(&mut Pos, &Vel)>() {
                    position.0 += vel.0 * DT;
                }
            },
        );
    }

    // brain, trimmed: three mutable columns.
    {
        pair(
            c,
            "triple_mut",
            rows,
            &mut world,
            |world| {
                for (_, brain, vel, health) in world.query3_mut::<Brain, Vel, Health>() {
                    brain.timer -= DT;
                    if brain.timer <= 0.0 {
                        let roll = xorshift(&mut brain.seed);
                        brain.timer = 0.2 + ((roll >> 8) & 63) as f32 / 64.0;
                        vel.0 = -vel.0;
                    }
                    health.0 = (health.0 - DT).clamp(0.0, 100.0);
                }
            },
            |world| {
                for (brain, vel, health) in world.query_mut::<(&mut Brain, &mut Vel, &mut Health)>()
                {
                    brain.timer -= DT;
                    if brain.timer <= 0.0 {
                        let roll = xorshift(&mut brain.seed);
                        brain.timer = 0.2 + ((roll >> 8) & 63) as f32 / 64.0;
                        vel.0 = -vel.0;
                    }
                    health.0 = (health.0 - DT).clamp(0.0, 100.0);
                }
            },
        );
    }

    // team_bags: two shared columns, a reduction.
    let bag_rows = world.query2_entities::<Team, Bag>().len();
    {
        pair(
            c,
            "pair_read",
            bag_rows,
            &mut world,
            |world| {
                let mut totals = [0_u64; 4];
                for (_, team, bag) in world.query2::<Team, Bag>() {
                    totals[team.0 as usize] +=
                        bag.0.iter().map(|&item| u64::from(item)).sum::<u64>();
                }
                black_box(totals);
            },
            |world| {
                let mut totals = [0_u64; 4];
                for (team, bag) in world.query::<(&Team, &Bag)>() {
                    totals[team.0 as usize] +=
                        bag.0.iter().map(|&item| u64::from(item)).sum::<u64>();
                }
                black_box(totals);
            },
        );
    }

    // buffs: shared, mutable and two optional columns.
    {
        pair(
            c,
            "opt2_mut",
            rows,
            &mut world,
            |world| {
                for (_, health, cooldowns, regen, stats) in
                    world.query2_opt2_mut::<Health, Cooldowns, Regen, Stats>()
                {
                    if let Some(regen) = regen {
                        cooldowns.0[1] -= regen.0 * 0.1 * DT;
                    }
                    if let Some(stats) = stats {
                        stats.0[0] = health.0 * 0.01 + stats.0[1] * 0.5;
                    }
                }
            },
            |world| {
                for (health, cooldowns, regen, stats) in
                    world
                        .query_mut::<(&Health, &mut Cooldowns, Option<&Regen>, Option<&mut Stats>)>(
                        )
                {
                    if let Some(regen) = regen {
                        cooldowns.0[1] -= regen.0 * 0.1 * DT;
                    }
                    if let Some(stats) = stats {
                        stats.0[0] = health.0 * 0.01 + stats.0[1] * 0.5;
                    }
                }
            },
        );
    }

    // The default extract's shape: three shared and two optional columns.
    {
        pair(
            c,
            "triple_opt2_read",
            rows,
            &mut world,
            |world| {
                let mut sum = Vec2::ZERO;
                for (_, position, vel, health, acc, regen) in
                    world.query3_opt2::<Pos, Vel, Health, Acc, Regen>()
                {
                    let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                    let regen = regen.map_or(0.0, |regen| regen.0);
                    sum += position.0 + vel.0 * health.0 + acc * regen;
                }
                black_box(sum);
            },
            |world| {
                let mut sum = Vec2::ZERO;
                for (position, vel, health, acc, regen) in
                    world.query::<(&Pos, &Vel, &Health, Option<&Acc>, Option<&Regen>)>()
                {
                    let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                    let regen = regen.map_or(0.0, |regen| regen.0);
                    sum += position.0 + vel.0 * health.0 + acc * regen;
                }
                black_box(sum);
            },
        );
    }

    // The default extract's read as `extract_into` writes it: `for_each`
    // over the entity, three shared and two optional columns, a row skipped
    // on a flag, the entity kept with the row's result.
    {
        pair(
            c,
            "entity_triple_opt2_read",
            rows,
            &mut world,
            |world| {
                let mut sum = Vec2::ZERO;
                let mut picked = 0_u32;
                world
                    .query3_opt2::<Pos, Vel, Health, Acc, Regen>()
                    .for_each(|(entity, position, vel, health, acc, regen)| {
                        if health.0 < 41.0 {
                            return;
                        }
                        let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                        let regen = regen.map_or(0.0, |regen| regen.0);
                        sum += position.0 + vel.0 * health.0 + acc * regen;
                        picked = picked.wrapping_add(entity.id());
                    });
                black_box((sum, picked));
            },
            |world| {
                let mut sum = Vec2::ZERO;
                let mut picked = 0_u32;
                world
                    .query::<(Entity, &Pos, &Vel, &Health, Option<&Acc>, Option<&Regen>)>()
                    .for_each(|(entity, position, vel, health, acc, regen)| {
                        if health.0 < 41.0 {
                            return;
                        }
                        let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                        let regen = regen.map_or(0.0, |regen| regen.0);
                        sum += position.0 + vel.0 * health.0 + acc * regen;
                        picked = picked.wrapping_add(entity.id());
                    });
                black_box((sum, picked));
            },
        );
    }

    // The same read as `extract_into` writes it now: the slice form, each
    // archetype's entities zipped with the three shared columns and the two
    // optional ones zipped on outside through `OptionalColumn`, the arity
    // function's loop.
    {
        pair(
            c,
            "entity_triple_opt2_read_slices",
            rows,
            &mut world,
            |world| {
                let mut sum = Vec2::ZERO;
                let mut picked = 0_u32;
                world
                    .query3_opt2::<Pos, Vel, Health, Acc, Regen>()
                    .for_each(|(entity, position, vel, health, acc, regen)| {
                        if health.0 < 41.0 {
                            return;
                        }
                        let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                        let regen = regen.map_or(0.0, |regen| regen.0);
                        sum += position.0 + vel.0 * health.0 + acc * regen;
                        picked = picked.wrapping_add(entity.id());
                    });
                black_box((sum, picked));
            },
            |world| {
                let mut sum = Vec2::ZERO;
                let mut picked = 0_u32;
                for (_, (entities, positions, vels, healths, accs, regens)) in world
                    .query_slices::<(Entity, &Pos, &Vel, &Health, Option<&Acc>, Option<&Regen>)>()
                {
                    for (((((&entity, position), vel), health), acc), regen) in entities
                        .iter()
                        .zip(positions)
                        .zip(vels)
                        .zip(healths)
                        .zip(OptionalColumn(accs.map(<[_]>::iter)))
                        .zip(OptionalColumn(regens.map(<[_]>::iter)))
                    {
                        if health.0 < 41.0 {
                            continue;
                        }
                        let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                        let regen = regen.map_or(0.0, |regen| regen.0);
                        sum += position.0 + vel.0 * health.0 + acc * regen;
                        picked = picked.wrapping_add(entity.id());
                    }
                }
                black_box((sum, picked));
            },
        );
    }

    // The slice form consumed as the arity function consumes it: an outer
    // `for_each` over the archetypes and an inner `for_each` over the same
    // zip, the row body with `return`. Retry 3's candidate for `extract_into`.
    {
        pair(
            c,
            "entity_triple_opt2_read_foreach",
            rows,
            &mut world,
            |world| {
                let mut sum = Vec2::ZERO;
                let mut picked = 0_u32;
                world
                    .query3_opt2::<Pos, Vel, Health, Acc, Regen>()
                    .for_each(|(entity, position, vel, health, acc, regen)| {
                        if health.0 < 41.0 {
                            return;
                        }
                        let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                        let regen = regen.map_or(0.0, |regen| regen.0);
                        sum += position.0 + vel.0 * health.0 + acc * regen;
                        picked = picked.wrapping_add(entity.id());
                    });
                black_box((sum, picked));
            },
            |world| {
                let mut sum = Vec2::ZERO;
                let mut picked = 0_u32;
                world
                    .query_slices::<(Entity, &Pos, &Vel, &Health, Option<&Acc>, Option<&Regen>)>()
                    .for_each(|(_, (entities, positions, vels, healths, accs, regens))| {
                        entities
                            .iter()
                            .zip(positions)
                            .zip(vels)
                            .zip(healths)
                            .zip(OptionalColumn(accs.map(<[_]>::iter)))
                            .zip(OptionalColumn(regens.map(<[_]>::iter)))
                            .for_each(|(((((&entity, position), vel), health), acc), regen)| {
                                if health.0 < 41.0 {
                                    return;
                                }
                                let acc = acc.map_or(Vec2::ZERO, |acc| acc.0);
                                let regen = regen.map_or(0.0, |regen| regen.0);
                                sum += position.0 + vel.0 * health.0 + acc * regen;
                                picked = picked.wrapping_add(entity.id());
                            });
                    });
                black_box((sum, picked));
            },
        );
    }

    // physics-style exclusion: three mutable columns without a fourth.
    let without_rows = rows - world.query_entities::<Follow>().len();
    {
        pair(
            c,
            "triple_mut_without",
            without_rows,
            &mut world,
            |world| {
                for (_, position, vel, health) in
                    world.query3_mut_without::<Pos, Vel, Health, Follow>()
                {
                    position.0 -= vel.0 * DT;
                    health.0 -= DT;
                }
            },
            |world| {
                for (position, vel, health) in
                    world.query_mut_filtered::<(&mut Pos, &mut Vel, &mut Health), Without<Follow>>()
                {
                    position.0 -= vel.0 * DT;
                    health.0 -= DT;
                }
            },
        );
    }

    // follow's steer pass: a mutable column filtered by a tag it never reads.
    let follow_rows = world.query_entities::<Follow>().len();
    {
        pair(
            c,
            "single_mut_with",
            follow_rows,
            &mut world,
            |world| {
                for (_, _, vel) in world.query2_mut::<Follow, Vel>() {
                    vel.0 *= 0.99;
                }
            },
            |world| {
                for vel in world.query_mut_filtered::<&mut Vel, With<Follow>>() {
                    vel.0 *= 0.99;
                }
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
