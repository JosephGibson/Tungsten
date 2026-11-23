//! The step's forked stages against its serial step, in lockstep (`D-298`):
//! every participant count must step to the serial step's bodies, sleep
//! states and events, bit for bit.

use super::*;
use crate::Pcg32;
use crate::assets::tilemap::TerrainProps;
use crate::assets::{LayerKind, TilemapData, TilemapInstance, TilemapLayer, TilemapRegistry};
use crate::ecs::{Entity, EventQueue, World};
use crate::physics::broadphase::{ProxyId, QueryMarks, SpatialGrid};
use crate::physics::step::{PhysicsBuffers, apply_radial_impulse, physics_step, wake};
use crate::physics::{
    Aabb, Collider, CollisionEvent, PhysicsConfig, PhysicsWorkers, Position, RigidBody,
    SensorEntered, SensorExited, SpatialFilter, Velocity,
};
use crate::time::Time;
use glam::Vec2;

/// The participant counts every lockstep runs beside the serial step.
const COUNTS: [usize; 5] = [2, 3, 4, 6, 12];

fn seed_world(gravity: f32) -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(1.0 / 60.0);
    world.insert_resource(time);
    world.insert_resource(EventQueue::<CollisionEvent>::new());
    world.insert_resource(EventQueue::<SensorEntered>::new());
    world.insert_resource(EventQueue::<SensorExited>::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, gravity),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(PhysicsBuffers::default());
    world
}

/// What a lockstep compares after a step: each body's position and velocity
/// bits and its sleep state, then the step's events as they print.
fn state(world: &World, bodies: &[Entity]) -> (Vec<[u32; 5]>, Vec<String>) {
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    let rows = bodies
        .iter()
        .map(|&entity| {
            let position = world.get::<Position>(entity).map_or(Vec2::NAN, |p| p.0);
            let velocity = world.get::<Velocity>(entity).map_or(Vec2::NAN, |v| v.0);
            [
                position.x.to_bits(),
                position.y.to_bits(),
                velocity.x.to_bits(),
                velocity.y.to_bits(),
                u32::from(buffers.is_sleeping(entity)),
            ]
        })
        .collect();
    let mut events = Vec::new();
    let collisions = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    events.extend(collisions.iter_current().map(|event| format!("{event:?}")));
    let entered = world.get_resource::<EventQueue<SensorEntered>>().unwrap();
    events.extend(entered.iter_current().map(|event| format!("{event:?}")));
    let exited = world.get_resource::<EventQueue<SensorExited>>().unwrap();
    events.extend(exited.iter_current().map(|event| format!("{event:?}")));
    (rows, events)
}

fn flush(world: &mut World) {
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .flush();
    world
        .get_resource_mut::<EventQueue<SensorEntered>>()
        .unwrap()
        .flush();
    world
        .get_resource_mut::<EventQueue<SensorExited>>()
        .unwrap()
        .flush();
}

/// Steps one serial world and one forced world a count of [`COUNTS`] side by
/// side, each built by `build` and driven by `drive` before every step, and
/// compares them after every step. Returns the forks each forced world ran,
/// and the arrival rounds among them.
fn lockstep(
    name: &str,
    steps: usize,
    build: impl Fn(&mut World) -> Vec<Entity>,
    drive: impl Fn(&mut World, usize, &mut Vec<Entity>),
) -> Vec<(usize, usize)> {
    let mut worlds: Vec<(usize, World, Vec<Entity>)> = std::iter::once(1)
        .chain(COUNTS)
        .map(|count| {
            let mut world = seed_world(900.0);
            if count > 1 {
                world.insert_resource(PhysicsWorkers::forced(count));
            }
            let bodies = build(&mut world);
            (count, world, bodies)
        })
        .collect();
    for step in 0..steps {
        for (_, world, bodies) in &mut worlds {
            drive(world, step, bodies);
            physics_step(world);
        }
        let (rows, events) = state(&worlds[0].1, &worlds[0].2);
        for (count, world, bodies) in &worlds[1..] {
            let (got_rows, got_events) = state(world, bodies);
            assert_eq!(
                got_rows.len(),
                rows.len(),
                "{name}: step {step}, {count} participants"
            );
            for (index, (got, want)) in got_rows.iter().zip(&rows).enumerate() {
                assert_eq!(
                    got, want,
                    "{name}: body {index} at step {step}, {count} participants"
                );
            }
            assert_eq!(
                got_events, events,
                "{name}: events at step {step}, {count} participants"
            );
        }
        for (_, world, _) in &mut worlds {
            flush(world);
        }
    }
    worlds[1..]
        .iter()
        .map(|(_, world, _)| {
            let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
            buffers
                .extra
                .fork
                .pool()
                .map_or((0, 0), |pool| (pool.forks, pool.arrival_forks))
        })
        .collect()
}

fn spawn_body(
    world: &mut World,
    at: Vec2,
    velocity: Vec2,
    body: RigidBody,
    collider: Collider,
) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Position(at));
    world.insert(entity, Velocity(velocity));
    world.insert(entity, body);
    world.insert(entity, collider);
    entity
}

/// A pit of solid tiles, `fill` rows deep under its floor, 40 cells wide.
fn spawn_pit(world: &mut World, fill: u32) {
    let (width, height) = (40u32, 20 + fill);
    let mut tiles = vec![-1i32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            if x == 0 || x == width - 1 || y >= 19 {
                tiles[(y * width + x) as usize] = 0;
            }
        }
    }
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "pit".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width,
            height,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "solid".into(),
                kind: LayerKind::Collision,
                tiles,
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("pit", Vec2::ZERO));
}

fn spawn_mixed(world: &mut World, rng: &mut Pcg32, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let at = Vec2::new(
                30.0 + (i % 28) as f32 * 20.0 + rng.next_range(-3.0, 3.0),
                250.0 - (i / 28) as f32 * 18.0 + rng.next_range(-3.0, 3.0),
            );
            let velocity = rng.next_unit_vec2() * rng.next_range(0.0, 120.0);
            let body = RigidBody::dynamic().with_restitution(rng.next_range(0.0, 0.6));
            let collider = if i % 4 == 0 {
                Collider::aabb(Vec2::splat(5.0 + (i % 3) as f32))
            } else {
                Collider::circle(4.0 + (i % 5) as f32)
            };
            spawn_body(world, at, velocity, body, collider)
        })
        .collect()
}

#[test]
fn a_pile_in_a_tile_pit_steps_alike_at_every_count_split_merged_and_as_built() {
    for (name, fill, split) in [
        ("as built, tiles past the movers", 12, None),
        ("as built, movers past the tiles", 0, None),
        ("forced split", 0, Some(true)),
        ("forced merged", 12, Some(false)),
    ] {
        let forks = lockstep(
            name,
            90,
            |world| {
                world
                    .get_resource_mut::<PhysicsBuffers>()
                    .unwrap()
                    .grid
                    .force_split = split;
                spawn_pit(world, fill);
                spawn_mixed(world, &mut Pcg32::seeded(0xD298), 260)
            },
            |world, step, _| {
                if step == 60 {
                    apply_radial_impulse(
                        world,
                        Vec2::new(320.0, 280.0),
                        200.0,
                        9_000.0,
                        &SpatialFilter::default(),
                    );
                }
            },
        );
        // The blast knocks bodies into neighbours no contact admitted, so
        // the arrival pass lists them and its rounds fork too.
        assert!(
            forks
                .iter()
                .all(|&(forks, rounds)| forks > rounds && rounds > 0),
            "{name}: {forks:?}"
        );
    }
}

#[test]
fn bullets_sleepers_spawns_and_despawns_step_alike_at_every_count() {
    for seed in 0..4u64 {
        let forks = lockstep(
            &format!("seed {seed}"),
            70,
            |world| {
                let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
                config.substeps = [1, 2, 4, 8][seed as usize % 4];
                config.broadphase_cell_size = [8.0, 16.0, 32.0, 64.0][seed as usize % 4];
                let floor = world.spawn();
                world.insert(floor, Position(Vec2::new(-400.0, 300.0)));
                world.insert(floor, RigidBody::r#static());
                world.insert(
                    floor,
                    Collider::aabb(Vec2::new(900.0, 20.0)).with_offset(Vec2::new(900.0, 20.0)),
                );
                let mut rng = Pcg32::seeded(0x00B0_11E7 + seed);
                let mut bodies = spawn_mixed(world, &mut rng, 120);
                // A bullet fast enough to query past the grid's oversize bound.
                bodies.push(spawn_body(
                    world,
                    Vec2::new(-300.0, 240.0),
                    Vec2::new(rng.next_range(2_400.0, 15_000.0), 0.0),
                    RigidBody::dynamic().with_restitution(0.8),
                    Collider::circle(4.0),
                ));
                bodies
            },
            |world, step, bodies| match step {
                // The pile sleeps in places by now; wake one, shove one,
                // take one away and bring new ones in.
                40 => wake(world, bodies[3]),
                46 => world.get_mut::<Velocity>(bodies[17]).unwrap().0 = Vec2::new(0.0, -260.0),
                50 => {
                    let gone = bodies.remove(5);
                    world.despawn(gone);
                }
                54 => {
                    let mut rng = Pcg32::seeded(0x5EED + seed);
                    for i in 0..12 {
                        bodies.push(spawn_body(
                            world,
                            Vec2::new(60.0 + i as f32 * 30.0, 40.0),
                            rng.next_unit_vec2() * 300.0,
                            RigidBody::dynamic(),
                            Collider::circle(6.0),
                        ));
                    }
                }
                _ => {}
            },
        );
        assert!(
            forks
                .iter()
                .all(|&(forks, rounds)| forks > rounds && rounds > 0),
            "seed {seed}: {forks:?}"
        );
    }
}

#[test]
fn layers_sensors_and_a_kinematic_mover_step_alike_at_every_count() {
    let forks = lockstep(
        "layers",
        80,
        |world| {
            spawn_pit(world, 0);
            let mut rng = Pcg32::seeded(0x0001_A7E5);
            let mut bodies = Vec::new();
            for i in 0..160 {
                // Two layers that meet themselves and the tiles' layer 1,
                // never each other.
                let (layers, mask) = if i % 2 == 0 {
                    (0b010, 0b011)
                } else {
                    (0b100, 0b101)
                };
                bodies.push(spawn_body(
                    world,
                    Vec2::new(
                        40.0 + (i % 20) as f32 * 27.0 + rng.next_range(-2.0, 2.0),
                        260.0 - (i / 20) as f32 * 16.0,
                    ),
                    rng.next_unit_vec2() * 60.0,
                    RigidBody::dynamic().with_layers(layers).with_mask(mask),
                    Collider::circle(6.0),
                ));
            }
            bodies.push(spawn_body(
                world,
                Vec2::new(200.0, 200.0),
                Vec2::ZERO,
                RigidBody::sensor(),
                Collider::aabb(Vec2::new(120.0, 60.0)),
            ));
            bodies.push(spawn_body(
                world,
                Vec2::new(60.0, 285.0),
                Vec2::new(180.0, 0.0),
                RigidBody::kinematic().with_layers(0b110).with_mask(0b110),
                Collider::aabb(Vec2::new(30.0, 8.0)),
            ));
            bodies
        },
        |_, _, _| {},
    );
    assert!(forks.iter().all(|&(forks, _)| forks > 0), "{forks:?}");
}

#[test]
fn a_world_without_the_resource_or_with_one_participant_starts_no_thread() {
    for workers in [
        None,
        Some(PhysicsWorkers::SERIAL),
        Some(PhysicsWorkers::forced(1)),
    ] {
        let mut world = seed_world(900.0);
        if let Some(workers) = workers {
            world.insert_resource(workers);
        }
        spawn_pit(&mut world, 0);
        spawn_mixed(&mut world, &mut Pcg32::seeded(7), 60);
        for _ in 0..20 {
            physics_step(&mut world);
        }
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(buffers.extra.fork.pool().is_none(), "{workers:?}");
    }
}

#[test]
fn a_small_world_under_the_thresholds_never_forks() {
    let mut world = seed_world(900.0);
    world.insert_resource(PhysicsWorkers::new(4));
    spawn_pit(&mut world, 0);
    spawn_mixed(&mut world, &mut Pcg32::seeded(7), 200);
    for _ in 0..30 {
        physics_step(&mut world);
    }
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(
        buffers.extra.fork.pool().is_none(),
        "200 bodies are under every stage's threshold"
    );
}

#[test]
fn a_changed_participant_count_remakes_the_pool() {
    let mut world = seed_world(900.0);
    world.insert_resource(PhysicsWorkers::forced(2));
    spawn_pit(&mut world, 0);
    spawn_mixed(&mut world, &mut Pcg32::seeded(7), 60);
    physics_step(&mut world);
    let scratches = |world: &World| {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        buffers.extra.fork.pool().map(|pool| pool.scratches.len())
    };
    assert_eq!(scratches(&world), Some(2));
    world.insert_resource(PhysicsWorkers::forced(5));
    physics_step(&mut world);
    assert_eq!(scratches(&world), Some(5));
}

#[test]
fn a_share_that_panics_fails_the_fork_instead_of_hanging_it() {
    let mut workers = Workers::new(3);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        workers.fork(ForkData {
            stage: Stage::Fail { participant: 2 },
            ..ForkData::default()
        })
    }));
    let message = caught
        .unwrap_err()
        .downcast_ref::<&str>()
        .map(ToString::to_string)
        .unwrap_or_default();
    assert_eq!(message, "a physics worker's share of the step panicked");
    // Dropping the pool joins its threads, the one that panicked included.
    drop(workers);
}

/// The ids a walk visits for `query`, in order.
fn own_walk(grid: &mut SpatialGrid, query: &Aabb) -> Vec<ProxyId> {
    let mut out = Vec::new();
    grid.for_each_run_in(query, 0, |mut run| run.each(None, |id| out.push(id)));
    out
}

fn shared_walk(grid: &SpatialGrid, marks: &mut QueryMarks, query: &Aabb) -> Vec<ProxyId> {
    let mut out = Vec::new();
    grid.for_each_run_in_shared(marks, query, 0, |mut run| run.each(None, |id| out.push(id)));
    out
}

#[test]
fn a_shared_walk_visits_what_the_grids_own_walk_visits_in_its_order() {
    // Dense boxes over a few cells, then the same with one far outlier,
    // which changes the grid's layout; single-cell boxes alone, which need
    // no marks.
    for (name, spread, outlier, half) in [
        ("dense", 300.0, false, 30.0),
        ("with an outlier", 300.0, true, 30.0),
        ("single cells", 300.0, false, 2.0),
        ("sparse", 60_000.0, false, 40.0),
    ] {
        let mut rng = Pcg32::seeded(0x6A1D);
        let mut grid = SpatialGrid::new(32.0);
        for id in 0..400 {
            let center = Vec2::new(rng.next_range(0.0, spread), rng.next_range(0.0, spread));
            let half = Vec2::new(rng.next_range(1.0, half), rng.next_range(1.0, half));
            grid.insert(id, &Aabb::new(center, half));
        }
        if outlier {
            grid.insert(400, &Aabb::new(Vec2::splat(90_000.0), Vec2::splat(8.0)));
        }
        grid.prepare();
        let mut marks = QueryMarks::default();
        for _ in 0..200 {
            let query = Aabb::new(
                Vec2::new(
                    rng.next_range(-50.0, spread + 50.0),
                    rng.next_range(-50.0, spread + 50.0),
                ),
                Vec2::new(rng.next_range(1.0, 120.0), rng.next_range(1.0, 120.0)),
            );
            assert_eq!(
                shared_walk(&grid, &mut marks, &query),
                own_walk(&mut grid, &query),
                "{name}: {query:?}"
            );
        }
    }
}

#[test]
fn the_steps_buffers_keep_their_auto_traits_with_a_pool_inside() {
    fn shared<T: Send + Sync + std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
    shared::<PhysicsBuffers>();
}
