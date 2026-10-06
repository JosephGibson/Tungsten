//! Tunneling correctness harness: fires deterministic volleys of fast
//! projectiles at thin targets and reports the miss (tunnel-through) rate per
//! scenario and speed — correctness signal, not timing.
//!
//! Run with:
//! `cargo test -p tungsten-core --test physics_tunneling -- --nocapture`
//!
//! Guarantee (asserted, D-064): **zero misses in every scenario at every
//! speed up to 15,360 px/s (256 px/frame)** — static AABB walls, static
//! circle pillars, near-immovable dynamic walls, and head-on dynamic pairs.
//! Speculative contacts are the primary CCD: the narrow phase admits
//! positive-gap pairs and the solver clamps approach to arrive at touching,
//! independent of the fixed substep count; the slab sweep remains as a
//! statics-only safety net.
//!
//! Pushed bodies (asserted, D-092): a resting body hit by a pusher of up to
//! 1,000 times its mass, at up to 15,360 px/s, never ends beyond a static or
//! near-immovable dynamic gate behind it, and neither does the pusher. That
//! holds whichever of the three was spawned first, and with the pushed body
//! asleep before the impact. A pusher at 60–960 px/s never crosses a thin
//! static wall either. Two limits remain: a 1,000:1 pusher at 120–480 px/s
//! can still crush the body through a 4 px *dynamic* gate, and at 15,360 px/s
//! a pusher spawned last can end past the body it pushed.

use glam::Vec2;
use tungsten_core::{
    BodyKind, Collider, Entity, PhysicsBuffers, PhysicsConfig, Position, RigidBody, Time, Velocity,
    World, physics_step,
};

const DT: f32 = 1.0 / 60.0;
const PROJECTILE_RADIUS: f32 = 4.0;
const WALL_X: f32 = 800.0;
const WALL_HALF_THICKNESS: f32 = 2.0;
const WALL_HALF_HEIGHT: f32 = 400.0;
const SPAWN_X: f32 = 100.0;
const VOLLEY_SIZE: usize = 32;
const STEPS: usize = 120;
const SPEEDS: [f32; 6] = [480.0, 960.0, 1_920.0, 3_840.0, 7_680.0, 15_360.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    /// Static thin AABB wall — speculative contact plus slab-sweep net.
    StaticAabbWall,
    /// Static circle pillar column — speculative circle contacts; the sweep
    /// net promotes circles to bounding squares (D-064).
    StaticCirclePillar,
    /// Near-immovable dynamic thin wall — speculative dynamic-pair contacts.
    DynamicAabbWall,
}

fn tunneling_world() -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT);
    world.insert_resource(time);
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    });
    world
}

fn spawn_projectile(world: &mut World, position: Vec2, velocity: Vec2) -> tungsten_core::Entity {
    let entity = world.spawn();
    world.insert(entity, Position(position));
    world.insert(entity, Velocity(velocity));
    world.insert(entity, RigidBody::dynamic());
    world.insert(entity, Collider::circle(PROJECTILE_RADIUS));
    entity
}

fn spawn_wall(world: &mut World, kind: BodyKind) {
    let entity = world.spawn();
    world.insert(entity, Position(Vec2::new(WALL_X, 0.0)));
    world.insert(
        entity,
        match kind {
            BodyKind::Static => RigidBody::r#static(),
            // Heavy enough to be effectively immovable over the run.
            BodyKind::Dynamic => RigidBody::dynamic().with_mass(1.0e6),
        },
    );
    world.insert(
        entity,
        Collider::aabb(Vec2::new(WALL_HALF_THICKNESS, WALL_HALF_HEIGHT)),
    );
    if kind == BodyKind::Dynamic {
        world.insert(entity, Velocity(Vec2::ZERO));
    }
}

/// Overlapping static circles forming a gap-free vertical wall.
fn spawn_circle_pillar(world: &mut World) {
    const PILLAR_RADIUS: f32 = 8.0;
    const PILLAR_SPACING: f32 = 12.0;
    let mut y = -WALL_HALF_HEIGHT;
    while y <= WALL_HALF_HEIGHT {
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(WALL_X, y)));
        world.insert(entity, RigidBody::r#static());
        world.insert(entity, Collider::circle(PILLAR_RADIUS));
        y += PILLAR_SPACING;
    }
}

/// Wall-plane x beyond which a projectile has tunneled through the target.
fn beyond_wall_x(kind: TargetKind) -> f32 {
    let target_half = match kind {
        TargetKind::StaticAabbWall | TargetKind::DynamicAabbWall => WALL_HALF_THICKNESS,
        TargetKind::StaticCirclePillar => 8.0,
    };
    WALL_X + target_half + PROJECTILE_RADIUS + 1.0
}

/// Fire one deterministic volley at `speed`; return (misses, total).
fn run_wall_volley(kind: TargetKind, speed: f32) -> (usize, usize) {
    let mut world = tunneling_world();
    match kind {
        TargetKind::StaticAabbWall => spawn_wall(&mut world, BodyKind::Static),
        TargetKind::DynamicAabbWall => spawn_wall(&mut world, BodyKind::Dynamic),
        TargetKind::StaticCirclePillar => spawn_circle_pillar(&mut world),
    }

    let mut projectiles = Vec::with_capacity(VOLLEY_SIZE);
    for index in 0..VOLLEY_SIZE {
        // Deterministic y spread across the wall height, off-center.
        let t = (index as f32 + 0.5) / VOLLEY_SIZE as f32;
        let y = (t - 0.5) * (WALL_HALF_HEIGHT * 1.6);
        let entity = spawn_projectile(&mut world, Vec2::new(SPAWN_X, y), Vec2::new(speed, 0.0));
        projectiles.push(entity);
    }

    for _ in 0..STEPS {
        physics_step(&mut world);
    }

    let threshold = beyond_wall_x(kind);
    let misses = projectiles
        .iter()
        .filter(|&&entity| {
            world
                .get::<Position>(entity)
                .is_some_and(|pos| pos.0.x > threshold)
        })
        .count();
    (misses, projectiles.len())
}

/// Head-on dynamic pairs; a miss is a pair whose members swapped sides.
fn run_head_on_volley(closing_speed: f32) -> (usize, usize) {
    const PAIR_GAP_Y: f32 = 24.0;
    let mut world = tunneling_world();
    let half_speed = closing_speed * 0.5;

    let mut pairs = Vec::with_capacity(VOLLEY_SIZE);
    for index in 0..VOLLEY_SIZE {
        let y = index as f32 * PAIR_GAP_Y;
        let left = spawn_projectile(
            &mut world,
            Vec2::new(SPAWN_X, y),
            Vec2::new(half_speed, 0.0),
        );
        let right = spawn_projectile(
            &mut world,
            Vec2::new(WALL_X, y),
            Vec2::new(-half_speed, 0.0),
        );
        pairs.push((left, right));
    }

    for _ in 0..STEPS {
        physics_step(&mut world);
    }

    let misses = pairs
        .iter()
        .filter(|&&(left, right)| {
            let lx = world.get::<Position>(left).map(|p| p.0.x);
            let rx = world.get::<Position>(right).map(|p| p.0.x);
            matches!((lx, rx), (Some(lx), Some(rx)) if lx > rx + 1.0)
        })
        .count();
    (misses, pairs.len())
}

struct ReportRow {
    scenario: &'static str,
    speed: f32,
    misses: usize,
    total: usize,
}

impl ReportRow {
    fn miss_rate(&self) -> f32 {
        self.misses as f32 / self.total as f32
    }
}

fn print_report(rows: &[ReportRow]) {
    println!();
    println!(
        "{:<26} {:>12} {:>10} {:>10} {:>8}",
        "scenario", "speed(px/s)", "px/frame", "miss/total", "miss%"
    );
    for row in rows {
        println!(
            "{:<26} {:>12.0} {:>10.1} {:>7}/{:<3} {:>7.1}",
            row.scenario,
            row.speed,
            row.speed * DT,
            row.misses,
            row.total,
            row.miss_rate() * 100.0
        );
    }
    println!();
}

/// Miss-rate matrix across target kinds and speeds. Asserts zero misses for
/// **all four scenarios at all speeds** (D-064).
#[test]
fn tunneling_miss_rate_report() {
    let mut rows = Vec::new();

    for &speed in &SPEEDS {
        let (misses, total) = run_wall_volley(TargetKind::StaticAabbWall, speed);
        rows.push(ReportRow {
            scenario: "static_thin_wall_aabb",
            speed,
            misses,
            total,
        });
    }
    for &speed in &SPEEDS {
        let (misses, total) = run_wall_volley(TargetKind::StaticCirclePillar, speed);
        rows.push(ReportRow {
            scenario: "static_circle_pillar",
            speed,
            misses,
            total,
        });
    }
    for &speed in &SPEEDS {
        let (misses, total) = run_wall_volley(TargetKind::DynamicAabbWall, speed);
        rows.push(ReportRow {
            scenario: "dynamic_thin_wall",
            speed,
            misses,
            total,
        });
    }
    for &speed in &SPEEDS {
        let (misses, total) = run_head_on_volley(speed);
        rows.push(ReportRow {
            scenario: "head_on_dynamic_pair",
            speed,
            misses,
            total,
        });
    }

    print_report(&rows);

    for row in &rows {
        assert_eq!(
            row.misses, 0,
            "{} tunneled at {} px/s — speculative-contact CCD regressed",
            row.scenario, row.speed
        );
    }
}

/// The discrete narrow phase alone must stop moderate-speed bodies: at
/// 480 px/s the per-substep travel (2 px at 4 fixed substeps) never exceeds
/// the projectile radius, so contacts resolve without any speculative gap.
#[test]
fn moderate_speed_never_tunnels() {
    let (misses, total) = run_wall_volley(TargetKind::StaticAabbWall, 480.0);
    assert_eq!(misses, 0);
    assert_eq!(total, VOLLEY_SIZE);
}

const STALL_FLOOR_TOP: f32 = 480.0;

/// Bodies resting on a static floor under gravity, with sleeping off so the
/// settled pile stays awake and the step bound off (`D-094`) so the step
/// takes the whole dt it is handed.
fn stall_world(bodies: &[(Vec2, Collider)]) -> (World, Vec<tungsten_core::Entity>) {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT);
    world.insert_resource(time);
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, 900.0),
        sleep_threshold: 0.0,
        max_step_dt: 0.0,
        ..PhysicsConfig::default()
    });
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, STALL_FLOOR_TOP + 20.0)));
    world.insert(floor, RigidBody::r#static());
    world.insert(floor, Collider::aabb(Vec2::new(4_000.0, 20.0)));
    let entities = bodies
        .iter()
        .map(|&(position, collider)| {
            let entity = world.spawn();
            world.insert(entity, Position(position));
            world.insert(entity, Velocity(Vec2::ZERO));
            world.insert(entity, RigidBody::dynamic());
            world.insert(entity, collider);
            entity
        })
        .collect();
    (world, entities)
}

/// The frame dt cap (`D-088`): the app hands the simulation at most 0.1 s per
/// frame (`MAX_DT_SECS` in `tungsten::app`). One step of that length on a
/// settled, awake pile loses no body and leaves it under 50 px/s. Longer
/// steps do not hold: 0.2 s leaves over 110 px/s, and 2 s drops bodies
/// through the floor.
///
/// Since `D-094` a default `physics_step` advances at most 1/30 s of that
/// dt, which would pass here without taking a 0.1 s step. The scene runs
/// with `max_step_dt: 0.0` so it keeps pinning what a real 0.1 s step does
/// to these two piles.
#[test]
fn one_capped_stall_step_keeps_a_settled_pile() {
    const STALL_DT: f32 = 0.1;
    let stack: Vec<(Vec2, Collider)> = (0..5)
        .map(|i| {
            (
                Vec2::new(0.0, STALL_FLOOR_TOP - 16.0 - 32.0 * i as f32),
                Collider::aabb(Vec2::splat(16.0)),
            )
        })
        .collect();
    let pile: Vec<(Vec2, Collider)> = (0..30)
        .map(|i| {
            let (col, row) = ((i % 6) as f32, (i / 6) as f32);
            (
                Vec2::new(col * 13.0 - 30.0, STALL_FLOOR_TOP - 7.0 - row * 13.0),
                Collider::circle(6.0),
            )
        })
        .collect();

    for (name, bodies) in [("stack", stack), ("pile", pile)] {
        let (mut world, entities) = stall_world(&bodies);
        for _ in 0..240 {
            physics_step(&mut world);
        }
        world
            .get_resource_mut::<Time>()
            .unwrap()
            .advance_frame(STALL_DT);
        physics_step(&mut world);
        world.get_resource_mut::<Time>().unwrap().advance_frame(DT);

        let mut max_speed: f32 = 0.0;
        for _ in 0..120 {
            physics_step(&mut world);
            for &entity in &entities {
                max_speed = max_speed.max(world.get::<Velocity>(entity).unwrap().0.length());
            }
        }
        assert!(
            max_speed < 50.0,
            "{name}: {max_speed} px/s after one {STALL_DT} s step"
        );
        for &entity in &entities {
            let y = world.get::<Position>(entity).unwrap().0.y;
            assert!(
                (STALL_FLOOR_TOP - 400.0..=STALL_FLOOR_TOP).contains(&y),
                "{name}: a body ended at y {y} after one {STALL_DT} s step"
            );
        }
    }
}

const COLUMN_FLOOR_TOP: f32 = 2_000.0;
const COLUMN_RADIUS: f32 = 7.5;

/// The step bound (`D-094`): one call advances at most
/// `PhysicsConfig::max_step_dt`, so frames at the 0.1 s cap of `D-088` do not
/// soften the contacts. Twenty balls stacked on a floor at gravity 3,600
/// (the platformer's, four times this file's other scenes) keep their order
/// through 8 s of such frames. Unbounded, a 0.1 s step runs ball-ball
/// contacts at 10 Hz instead of 30 and the lowest ball ends under the floor
/// top.
#[test]
fn stacked_column_survives_slow_frames() {
    const SLOW_DT: f32 = 0.1;
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT);
    world.insert_resource(time);
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, 3_600.0),
        broadphase_cell_size: 64.0,
        sleep_threshold: 0.0,
        ..PhysicsConfig::default()
    });
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, COLUMN_FLOOR_TOP + 200.0)));
    world.insert(floor, RigidBody::r#static());
    world.insert(floor, Collider::aabb(Vec2::new(400.0, 200.0)));
    let balls: Vec<Entity> = (0..20)
        .map(|i| {
            let y = COLUMN_FLOOR_TOP - COLUMN_RADIUS - 2.0 * COLUMN_RADIUS * i as f32;
            let entity = world.spawn();
            world.insert(entity, Position(Vec2::new(0.0, y)));
            world.insert(entity, Velocity(Vec2::ZERO));
            world.insert(entity, Collider::circle(COLUMN_RADIUS));
            world.insert(entity, RigidBody::dynamic());
            entity
        })
        .collect();

    for _ in 0..240 {
        physics_step(&mut world);
    }
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(SLOW_DT);
    for _ in 0..80 {
        physics_step(&mut world);
    }

    let ys: Vec<f32> = balls
        .iter()
        .map(|&entity| world.get::<Position>(entity).unwrap().0.y)
        .collect();
    for (i, y) in ys.iter().enumerate() {
        assert!(
            *y < COLUMN_FLOOR_TOP,
            "ball {i} centre is below the floor top: y={y}"
        );
    }
    for (i, pair) in ys.windows(2).enumerate() {
        let gap = pair[0] - pair[1];
        assert!(
            gap > COLUMN_RADIUS,
            "balls {i} and {} overlap by more than half a diameter: gap={gap}",
            i + 1
        );
    }
}

const GATE_X: f32 = 800.0;
const GATE_HALF_HEIGHT: f32 = 400.0;
const PUSH_RADIUS: f32 = 8.0;
const PUSH_MASSES: [f32; 4] = [1.0, 10.0, 100.0, 1_000.0];
const PUSH_GAPS: [f32; 4] = [0.5, 3.0, 12.0, 60.0];

/// When the pusher is spawned. The solver gives the last word to the contact
/// that comes later in the pair list, and the list follows spawn order, so a
/// scene that holds in one order can fail in another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PusherSpawn {
    /// Before the pushed body.
    First,
    /// After the pushed body and the gate.
    Last,
    /// After the pushed body and the gate have fallen asleep.
    LastOntoSleeper,
}

/// A pusher moving at `speed` toward a resting body of mass 1 that sits `gap`
/// short of a gate.
#[derive(Debug, Clone, Copy)]
struct PushCase {
    spawn: PusherSpawn,
    gate: BodyKind,
    gate_half_thickness: f32,
    /// The gate is spawned before the pushed body.
    gate_first: bool,
    pusher_mass: f32,
    speed: f32,
    gap: f32,
}

struct PushScene {
    world: World,
    pusher: Entity,
    pushed: Entity,
    gate: Entity,
}

impl PushScene {
    fn x(&self, entity: Entity) -> f32 {
        self.world.get::<Position>(entity).unwrap().0.x
    }

    /// Neither body may end beyond the gate's centre.
    fn held(&self) -> bool {
        let gate_x = self.x(self.gate);
        self.x(self.pushed) <= gate_x && self.x(self.pusher) <= gate_x
    }
}

fn push_scene(case: PushCase) -> PushScene {
    let mut world = tunneling_world();
    let pushed_x = GATE_X - case.gate_half_thickness - PUSH_RADIUS - case.gap;
    // The pusher starts a little short of the pushed body, so the impact
    // happens inside the run.
    let pusher_x = pushed_x - 2.0 * PUSH_RADIUS - 30.0;

    let spawn_gate = |world: &mut World| {
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(GATE_X, 0.0)));
        world.insert(
            entity,
            Collider::aabb(Vec2::new(case.gate_half_thickness, GATE_HALF_HEIGHT)),
        );
        match case.gate {
            BodyKind::Static => world.insert(entity, RigidBody::r#static()),
            BodyKind::Dynamic => {
                // Heavy enough to be effectively immovable over the run.
                world.insert(entity, RigidBody::dynamic().with_mass(1.0e6));
                world.insert(entity, Velocity(Vec2::ZERO));
            }
        }
        entity
    };
    let spawn_ball = |world: &mut World, x: f32, speed: f32, mass: f32| {
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(x, 0.0)));
        world.insert(entity, Velocity(Vec2::new(speed, 0.0)));
        world.insert(entity, RigidBody::dynamic().with_mass(mass));
        world.insert(entity, Collider::circle(PUSH_RADIUS));
        entity
    };

    let mut gate = None;
    if case.gate_first {
        gate = Some(spawn_gate(&mut world));
    }
    let mut pusher = None;
    if case.spawn == PusherSpawn::First {
        pusher = Some(spawn_ball(
            &mut world,
            pusher_x,
            case.speed,
            case.pusher_mass,
        ));
    }
    let pushed = spawn_ball(&mut world, pushed_x, 0.0, 1.0);
    let gate = gate.unwrap_or_else(|| spawn_gate(&mut world));
    if case.spawn == PusherSpawn::LastOntoSleeper {
        for _ in 0..60 {
            physics_step(&mut world);
        }
        let asleep = world
            .get_resource::<PhysicsBuffers>()
            .is_some_and(|buffers| buffers.is_sleeping(pushed));
        assert!(asleep, "the pushed body must be asleep before the impact");
    }
    let pusher =
        pusher.unwrap_or_else(|| spawn_ball(&mut world, pusher_x, case.speed, case.pusher_mass));

    PushScene {
        world,
        pusher,
        pushed,
        gate,
    }
}

/// Fails with the number of cases that did not hold and the first few of them.
fn assert_all_held(what: &str, total: usize, failed: &[PushCase]) {
    let shown: Vec<String> = failed.iter().take(8).map(|c| format!("{c:?}")).collect();
    assert!(
        failed.is_empty(),
        "{what}: a body ended beyond the gate in {} of {total} cases, first:\n  {}",
        failed.len(),
        shown.join("\n  ")
    );
}

/// A resting body pushed at a gate stays in front of it, and so does its
/// pusher (D-092): for a static or near-immovable dynamic gate, 4 or 32 px
/// thick, every pusher mass and speed, every gap, and every spawn order.
#[test]
fn pushed_body_never_crosses_a_gate() {
    const PUSH_SPEEDS: [f32; 4] = [960.0, 1_920.0, 7_680.0, 15_360.0];
    let mut total = 0;
    let mut failed = Vec::new();
    for spawn in [
        PusherSpawn::First,
        PusherSpawn::Last,
        PusherSpawn::LastOntoSleeper,
    ] {
        let mut failed_in_order = 0;
        for gate in [BodyKind::Static, BodyKind::Dynamic] {
            for gate_half_thickness in [2.0, 16.0] {
                for gate_first in [false, true] {
                    for pusher_mass in PUSH_MASSES {
                        for speed in PUSH_SPEEDS {
                            for gap in PUSH_GAPS {
                                let case = PushCase {
                                    spawn,
                                    gate,
                                    gate_half_thickness,
                                    gate_first,
                                    pusher_mass,
                                    speed,
                                    gap,
                                };
                                let mut scene = push_scene(case);
                                for _ in 0..60 {
                                    physics_step(&mut scene.world);
                                }
                                total += 1;
                                if !scene.held() {
                                    failed_in_order += 1;
                                    failed.push(case);
                                }
                            }
                        }
                    }
                }
            }
        }
        println!("pusher spawned {spawn:?}: {failed_in_order} of 512 cases not held");
    }
    assert_all_held("pushed body", total, &failed);
}

/// A slow push never takes a body through a thin static wall, whichever of
/// the three was spawned first (D-092). Before the arrival pass, a wall
/// contact solved ahead of the push let a heavy pusher march the body through:
/// that is the order with the wall spawned first and the pusher last.
#[test]
fn slow_push_never_crosses_a_static_wall() {
    const SLOW_SPEEDS: [f32; 5] = [60.0, 120.0, 240.0, 480.0, 960.0];
    let mut total = 0;
    let mut failed = Vec::new();
    for spawn in [PusherSpawn::First, PusherSpawn::Last] {
        for gate_first in [false, true] {
            for pusher_mass in PUSH_MASSES {
                for speed in SLOW_SPEEDS {
                    for gap in PUSH_GAPS {
                        let case = PushCase {
                            spawn,
                            gate: BodyKind::Static,
                            gate_half_thickness: WALL_HALF_THICKNESS,
                            gate_first,
                            pusher_mass,
                            speed,
                            gap,
                        };
                        let mut scene = push_scene(case);
                        // A slow pusher needs longer than a second to arrive.
                        for _ in 0..600 {
                            physics_step(&mut scene.world);
                        }
                        total += 1;
                        if !scene.held() {
                            failed.push(case);
                        }
                    }
                }
            }
        }
    }
    assert_all_held("slow push", total, &failed);
}

/// A train with nothing immovable in it keeps its order and its momentum: a
/// heavy body that hits a light one resting next to a third pushes both along.
/// Guards the arrival pass (D-092) against stopping a pusher that has
/// somewhere to go.
#[test]
fn free_train_keeps_its_order_and_momentum() {
    for (pusher_mass, last_mass) in [(100.0, 10.0), (1_000.0, 100.0), (10.0, 100.0), (1.0, 1.0)] {
        for speed in [960.0_f32, 7_680.0] {
            let mut world = tunneling_world();
            let mut spawn = |x: f32, speed: f32, mass: f32| {
                let entity = world.spawn();
                world.insert(entity, Position(Vec2::new(x, 0.0)));
                world.insert(entity, Velocity(Vec2::new(speed, 0.0)));
                world.insert(entity, RigidBody::dynamic().with_mass(mass));
                world.insert(entity, Collider::circle(PUSH_RADIUS));
                entity
            };
            let train = [
                (spawn(0.0, speed, pusher_mass), pusher_mass),
                (spawn(46.0, 0.0, 1.0), 1.0),
                (spawn(62.5, 0.0, last_mass), last_mass),
            ];
            for _ in 0..30 {
                physics_step(&mut world);
            }

            let xs = train.map(|(entity, _)| world.get::<Position>(entity).unwrap().0.x);
            assert!(
                xs[0] < xs[1] && xs[1] < xs[2],
                "masses {pusher_mass}/1/{last_mass} at {speed} px/s: order lost, x = {xs:?}"
            );
            let momentum: f32 = train
                .iter()
                .map(|&(entity, mass)| mass * world.get::<Velocity>(entity).unwrap().0.x)
                .sum();
            let before = pusher_mass * speed;
            assert!(
                (momentum - before).abs() <= 0.005 * before,
                "masses {pusher_mass}/1/{last_mass} at {speed} px/s: momentum {momentum}, was {before}"
            );
        }
    }
}
