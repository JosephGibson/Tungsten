//! Containment harness (D-063): a dense
//! 3k pile inside 80 px-thick static walls must never leak a body. Under the
//! pre-D-063 solver, Gauss-Seidel in-place MTV corrections shoved bodies past
//! the floor centerline within ~400 steps and the MTV ejected them out the
//! far side; the soft solver's clamped impulses + max push speed remove that
//! failure class.
//!
//! Slow frames (`D-094`): an awake 1,500-ball pile at gravity 3,600 keeps its
//! bodies, its height and the size of its pair list through frames at the
//! 0.1 s cap of `D-088`, because one step advances at most
//! `PhysicsConfig::max_step_dt`. A guard pins the same pile's end state at the
//! 1/60 s step, where the bound must change nothing.
//!
//! Kinematic crowd probe (`D-132`), before a mover builds on kinematic
//! contacts: `crowd_presses_on_a_kinematic_body_from_every_side` (500 circles
//! driven at a kinematic body, still and then walking, 24 cases),
//! `kinematic_body_walks_into_a_sleeping_crowd` (16 cases) and
//! `kinematic_body_pins_a_body_against_a_wall` (8 cases). Each case runs
//! twice, asserts that the kinematic body keeps its own path bit for bit,
//! that every value stays finite and that both runs end alike, and in the
//! first two that no crowd centre enters the body; it prints its deepest
//! penetration, contacts and step time.
//!
//! Release-only by cost (2,400 steps at 3k bodies); auto-ignored in debug.
//! Run with:
//! `cargo test --release -p tungsten-core --test physics_containment -- --nocapture`

mod common;

use std::time::{Duration, Instant};

use common::{BODY_RADIUS, FLOOR_Y, PILE_WIDTH, SPAWN_SPACING, spawn_pile, spawn_static_box};
use glam::Vec2;
use tungsten_core::{
    Collider, Entity, LayerKind, Pcg32, PhysicsBuffers, PhysicsConfig, Position, RigidBody,
    TilemapData, TilemapInstance, TilemapLayer, TilemapRegistry, Time, Velocity, World,
    physics_step,
};

const DT: f32 = 1.0 / 60.0;
const GRAVITY_Y: f32 = 900.0;
/// Thin walls by design: thick enough to be a real level boundary, thin
/// enough that a body pushed past the centerline would visibly escape.
const WALL_HALF: f32 = 40.0;
const BODY_COUNT: usize = 3_000;
const STEPS: usize = 2_400;

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn dense_pile_never_escapes_thin_walls() {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT);
    world.insert_resource(time);
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY_Y),
        ..PhysicsConfig::default()
    });

    let mut rng = Pcg32::seeded(0x7C0F_FEE5);
    let rows = BODY_COUNT.div_ceil(((PILE_WIDTH - 28.0) / SPAWN_SPACING) as usize);
    let top_y = FLOOR_Y - SPAWN_SPACING * (rows as f32 + 2.0) - 500.0;
    spawn_static_box(&mut world, PILE_WIDTH, top_y, WALL_HALF);
    let bodies = spawn_pile(&mut world, BODY_COUNT, &mut rng);

    for _ in 0..STEPS {
        physics_step(&mut world);
    }

    // Escape = center well outside the interior; transient soft-solver
    // penetration is a few px at most, so 20 px is unambiguous.
    const MARGIN: f32 = 20.0;
    let mut escaped = 0usize;
    for &body in &bodies {
        let p = world.get::<Position>(body).unwrap().0 + Vec2::splat(BODY_RADIUS);
        if p.x < -MARGIN
            || p.x > PILE_WIDTH + MARGIN
            || p.y > FLOOR_Y + MARGIN
            || p.y < top_y - MARGIN
        {
            escaped += 1;
        }
    }
    println!("containment: {escaped}/{BODY_COUNT} escaped over {STEPS} steps");
    assert_eq!(
        escaped, 0,
        "{escaped} bodies escaped the 80 px walls over {STEPS} steps"
    );
}

/// The platformer's gravity, four times `GRAVITY_Y`: contact sag per unit of
/// load scales with it.
const SLOW_GRAVITY_Y: f32 = 3_600.0;
/// The frame dt cap of `D-088`.
const SLOW_DT: f32 = 0.1;
const SLOW_FLOOR_TOP: f32 = 2_000.0;
const SLOW_RADIUS: f32 = 10.0;
const SLOW_WIDTH: f32 = 1_600.0;
const SLOW_BODY_COUNT: usize = 1_500;

struct SlowPile {
    world: World,
    balls: Vec<Entity>,
}

fn spawn_slab(world: &mut World, center: Vec2, half_extents: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Position(center));
    world.insert(entity, RigidBody::r#static());
    world.insert(entity, Collider::aabb(half_extents));
}

fn step_frames(world: &mut World, dt: f32, frames: usize) {
    world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
    for _ in 0..frames {
        physics_step(world);
    }
}

/// 1,500 balls of radius 10 in an open box 1,600 px wide, settled for 240
/// steps at 1/60 s. Rows are offset by half a pitch so the pile packs.
fn settled_slow_pile() -> SlowPile {
    const PER_ROW: usize = 75;
    const PITCH: f32 = 21.0;
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT);
    world.insert_resource(time);
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, SLOW_GRAVITY_Y),
        broadphase_cell_size: 64.0,
        ..PhysicsConfig::default()
    });
    spawn_slab(
        &mut world,
        Vec2::new(SLOW_WIDTH * 0.5, SLOW_FLOOR_TOP + 128.0),
        Vec2::new(SLOW_WIDTH * 0.5 + 256.0, 128.0),
    );
    for wall_x in [-128.0, SLOW_WIDTH + 128.0] {
        spawn_slab(
            &mut world,
            Vec2::new(wall_x, SLOW_FLOOR_TOP - 1_000.0),
            Vec2::new(128.0, 1_256.0),
        );
    }
    let balls = (0..SLOW_BODY_COUNT)
        .map(|index| {
            let (col, row) = (index % PER_ROW, index / PER_ROW);
            let offset = if row % 2 == 0 { 0.0 } else { PITCH * 0.5 };
            let x = PITCH + col as f32 * PITCH + offset;
            let y = SLOW_FLOOR_TOP - SLOW_RADIUS - 1.0 - row as f32 * PITCH;
            let entity = world.spawn();
            world.insert(entity, Position(Vec2::new(x, y)));
            world.insert(entity, Velocity(Vec2::ZERO));
            world.insert(entity, Collider::circle(SLOW_RADIUS));
            world.insert(entity, RigidBody::dynamic());
            entity
        })
        .collect();
    let mut pile = SlowPile { world, balls };
    step_frames(&mut pile.world, DT, 240);
    pile
}

/// Sleeping off keeps the pile awake; a sleeping pile is frozen and safe at
/// any dt (`D-088`).
fn keep_awake(pile: &mut SlowPile) {
    pile.world
        .get_resource_mut::<PhysicsConfig>()
        .unwrap()
        .sleep_threshold = 0.0;
}

fn below_floor(pile: &SlowPile) -> usize {
    pile.balls
        .iter()
        .filter(|&&entity| pile.world.get::<Position>(entity).unwrap().0.y > SLOW_FLOOR_TOP)
        .count()
}

/// Height of the pile: the floor minus the 2nd percentile of ball y.
fn pile_height(pile: &SlowPile) -> f32 {
    let mut ys: Vec<f32> = pile
        .balls
        .iter()
        .map(|&entity| pile.world.get::<Position>(entity).unwrap().0.y)
        .collect();
    ys.sort_by(f32::total_cmp);
    SLOW_FLOOR_TOP - ys[ys.len() / 50]
}

fn pair_count(pile: &SlowPile) -> usize {
    pile.world
        .get_resource::<PhysicsBuffers>()
        .unwrap()
        .pair_count()
}

/// An awake pile keeps its bodies and its height through 10 s of 0.1 s
/// frames. Unbounded, the softened contacts drop 91 of the 1,500 through the
/// floor.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn awake_pile_keeps_its_bodies_and_height_through_slow_frames() {
    let mut pile = settled_slow_pile();
    let before = pile_height(&pile);
    assert_eq!(below_floor(&pile), 0);
    keep_awake(&mut pile);
    step_frames(&mut pile.world, SLOW_DT, 100);
    let after = pile_height(&pile);
    println!(
        "slow frames: height {before} -> {after}, {} below the floor",
        below_floor(&pile)
    );
    assert_eq!(below_floor(&pile), 0, "bodies went through the floor");
    assert!(
        after > 0.85 * before,
        "the pile crunched down: height {before} -> {after}"
    );
}

/// A slow frame must not multiply the frame's work: the pair list of the
/// awake pile at 0.1 s frames stays within 3 times the 1/60 s list.
/// Unbounded, it grows from 4,392 to 59,185 pairs.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn slow_frames_do_not_multiply_the_pair_list() {
    let mut pile = settled_slow_pile();
    keep_awake(&mut pile);
    step_frames(&mut pile.world, DT, 30);
    let pairs_fast = pair_count(&pile);
    step_frames(&mut pile.world, SLOW_DT, 30);
    let pairs_slow = pair_count(&pile);
    println!("slow frames: pair list {pairs_fast} -> {pairs_slow}");
    assert!(
        pairs_slow <= 3 * pairs_fast,
        "pair list grew from {pairs_fast} to {pairs_slow} on slow frames"
    );
}

/// Guard: at the pinned 1/60 s step the bound changes nothing. FNV-1a over
/// the settled pile's position bits in spawn order; the value is from the
/// tree before `max_step_dt` existed.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn pinned_step_state_is_unchanged() {
    const EXPECTED: u64 = 0xaee2_72e0_1ffc_4e4c;
    let pile = settled_slow_pile();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &entity in &pile.balls {
        let position = pile.world.get::<Position>(entity).unwrap().0;
        for bits in [position.x.to_bits(), position.y.to_bits()] {
            hash = (hash ^ u64::from(bits)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    println!("pinned step: pile state hash {hash:#018x}");
    assert_eq!(
        hash, EXPECTED,
        "the settled pile's state moved at a 1/60 s step: {hash:#018x}"
    );
}

/// Crowd bodies, their radius and their spacing.
const CROWD: usize = 500;
const CROWD_RADIUS: f32 = 10.0;
const CROWD_PITCH: f32 = 21.0;
/// The kinematic body's radius, or its box's half extent.
const BODY_SIZE: f32 = 12.0;
const DRIVE_SPEED: f32 = 90.0;
const WALK_SPEED: f32 = 150.0;
/// Frame dts; the step bound holds the last two to 1/30 s (`D-094`).
const PROBE_DTS: [f32; 4] = [1.0 / 60.0, 1.0 / 30.0, 0.05, 0.1];

#[derive(Debug, Clone, Copy)]
enum BodyShape {
    Circle,
    Box,
}

impl BodyShape {
    fn collider(self) -> Collider {
        match self {
            BodyShape::Circle => Collider::circle(BODY_SIZE),
            BodyShape::Box => Collider::aabb(Vec2::splat(BODY_SIZE)),
        }
    }

    /// Signed distance from `point` to the surface of the shape at
    /// `center`; negative inside.
    fn distance(self, center: Vec2, point: Vec2) -> f32 {
        match self {
            BodyShape::Circle => point.distance(center) - BODY_SIZE,
            BodyShape::Box => {
                let outside = (point - center).abs() - Vec2::splat(BODY_SIZE);
                if outside.x > 0.0 || outside.y > 0.0 {
                    outside.max(Vec2::ZERO).length()
                } else {
                    outside.max_element()
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SpawnOrder {
    /// The body, then the rings inside out.
    BodyFirst,
    /// The rings outside in, then the body.
    BodyLast,
    /// The crowd in a seeded shuffle, the body in the middle.
    Shuffled,
}

/// One case's measures: the end state's hash and what the case prints.
struct Probe {
    hash: u64,
    deepest: f32,
    crowd_p99: f32,
    contacts: usize,
    sleepers: usize,
    step_time: Duration,
    steps: usize,
    woke: usize,
    crossed: bool,
}

impl Probe {
    fn line(&self) -> String {
        let micros = self.step_time.as_secs_f64() * 1.0e6 / self.steps as f64;
        format!(
            "deepest {:.2} px, crowd p99 {:.2} px, contacts {}, sleepers {}, {micros:.0} us/step",
            self.deepest.max(0.0),
            self.crowd_p99,
            self.contacts,
            self.sleepers
        )
    }
}

/// The determinism test's FNV-1a over each body's position and velocity bits.
fn state_hash(world: &World, bodies: &[Entity]) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    let mut mix = |value: u32| {
        hash ^= u64::from(value);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    };
    for &entity in bodies {
        let position = world.get::<Position>(entity).map_or(Vec2::ZERO, |p| p.0);
        let velocity = world.get::<Velocity>(entity).map_or(Vec2::ZERO, |v| v.0);
        mix(position.x.to_bits());
        mix(position.y.to_bits());
        mix(velocity.x.to_bits());
        mix(velocity.y.to_bits());
    }
    hash
}

fn probe_world() -> World {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(PhysicsConfig::default());
    world.insert_resource(TilemapRegistry::new());
    world
}

/// A body with `Position`, `Velocity`, `Collider` and `RigidBody`, so the
/// kinematic body and the crowd share one archetype and spawn order is
/// proxy order.
fn spawn_probe_body(world: &mut World, body: RigidBody, collider: Collider, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Position(at));
    world.insert(entity, Velocity(Vec2::ZERO));
    world.insert(entity, collider);
    world.insert(entity, body);
    entity
}

fn spawn_crowd_body(world: &mut World, at: Vec2) -> Entity {
    spawn_probe_body(
        world,
        RigidBody::dynamic(),
        Collider::circle(CROWD_RADIUS),
        at,
    )
}

/// One step of `dt` with the kinematic body at `velocity`, timed.
fn timed_step(world: &mut World, body: Entity, velocity: Vec2, dt: f32, time: &mut Duration) {
    world.get_resource_mut::<Time>().unwrap().advance_frame(dt);
    world.get_mut::<Velocity>(body).unwrap().0 = velocity;
    let start = Instant::now();
    physics_step(world);
    *time += start.elapsed();
}

/// The kinematic body's position bits after every step of `script`, a
/// list of (dt, velocity), starting at `start` in a world that holds nothing
/// else.
fn kinematic_path(shape: BodyShape, start: Vec2, script: &[(f32, Vec2)]) -> Vec<[u32; 2]> {
    let mut world = probe_world();
    let body = spawn_probe_body(&mut world, RigidBody::kinematic(), shape.collider(), start);
    let mut time = Duration::ZERO;
    script
        .iter()
        .map(|&(dt, velocity)| {
            timed_step(&mut world, body, velocity, dt, &mut time);
            bits(&world, body)
        })
        .collect()
}

fn bits(world: &World, entity: Entity) -> [u32; 2] {
    world
        .get::<Position>(entity)
        .unwrap()
        .0
        .to_array()
        .map(f32::to_bits)
}

/// The kinematic body under test and its path alone.
struct Mover<'a> {
    shape: BodyShape,
    body: Entity,
    path: &'a [[u32; 2]],
}

/// After a step: the body on its own path, every crowd value finite, and,
/// when `inside_fails`, no crowd centre inside the body. Returns the
/// deepest crowd penetration into the body.
fn check_step(
    world: &World,
    case: &str,
    step: usize,
    mover: &Mover<'_>,
    crowd: &[Entity],
    inside_fails: bool,
) -> f32 {
    let Mover { shape, body, path } = *mover;
    assert_eq!(
        bits(world, body),
        path[step],
        "{case}: the body left its path at step {step}"
    );
    let center = world.get::<Position>(body).unwrap().0;
    let mut deepest = f32::NEG_INFINITY;
    for &entity in crowd {
        let position = world.get::<Position>(entity).unwrap().0;
        let velocity = world.get::<Velocity>(entity).unwrap().0;
        assert!(
            position.is_finite() && velocity.is_finite(),
            "{case}: a non-finite body at step {step}: {position:?} {velocity:?}"
        );
        let distance = shape.distance(center, position);
        assert!(
            !inside_fails || distance >= 0.0,
            "{case}: a crowd centre {distance} px inside the body at step {step}"
        );
        deepest = deepest.max(CROWD_RADIUS - distance);
    }
    deepest
}

/// The 99th percentile of crowd–crowd penetration over overlapping pairs.
fn crowd_p99(world: &World, crowd: &[Entity]) -> f32 {
    let positions: Vec<Vec2> = crowd
        .iter()
        .map(|&entity| world.get::<Position>(entity).unwrap().0)
        .collect();
    let mut depths = Vec::new();
    for (i, a) in positions.iter().enumerate() {
        for b in &positions[i + 1..] {
            let depth = 2.0 * CROWD_RADIUS - a.distance(*b);
            if depth > 0.0 {
                depths.push(depth);
            }
        }
    }
    depths.sort_by(f32::total_cmp);
    depths.get(depths.len() * 99 / 100).copied().unwrap_or(0.0)
}

fn finish(world: &World, crowd: &[Entity], hashed: &[Entity], probe: &mut Probe) {
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    probe.contacts = buffers.contact_count();
    probe.sleepers = buffers.sleeping_count();
    probe.crowd_p99 = crowd_p99(world, crowd);
    probe.hash = state_hash(world, hashed);
}

fn empty_probe(steps: usize) -> Probe {
    Probe {
        hash: 0,
        deepest: f32::NEG_INFINITY,
        crowd_p99: 0.0,
        contacts: 0,
        sleepers: 0,
        step_time: Duration::ZERO,
        steps,
        woke: 0,
        crossed: false,
    }
}

/// 500 points on rings 21 px apart around the origin, inside out, about
/// 21 px apart along each ring; the first ring clears the box's corner.
fn ring_points() -> Vec<Vec2> {
    let mut points = Vec::with_capacity(CROWD);
    let mut radius = BODY_SIZE * std::f32::consts::SQRT_2 + CROWD_RADIUS + 2.0;
    while points.len() < CROWD {
        let count = (std::f32::consts::TAU * radius / CROWD_PITCH) as usize;
        for i in 0..count.min(CROWD - points.len()) {
            let angle = std::f32::consts::TAU * i as f32 / count as f32;
            points.push(Vec2::from_angle(angle) * radius);
        }
        radius += CROWD_PITCH;
    }
    points
}

/// 300 steps with the body still, then 120 walking along +x.
fn press_script(dt: f32) -> Vec<(f32, Vec2)> {
    (0..420)
        .map(|step| {
            let walk = if step < 300 {
                Vec2::ZERO
            } else {
                Vec2::new(WALK_SPEED, 0.0)
            };
            (dt, walk)
        })
        .collect()
}

fn press_case(shape: BodyShape, order: SpawnOrder, dt: f32, path: &[[u32; 2]]) -> Probe {
    let case = format!("press {shape:?} {order:?} dt {dt}");
    let mut world = probe_world();
    let points = ring_points();
    let spawn_body = |world: &mut World| {
        spawn_probe_body(world, RigidBody::kinematic(), shape.collider(), Vec2::ZERO)
    };
    let (body, crowd) = match order {
        SpawnOrder::BodyFirst => {
            let body = spawn_body(&mut world);
            let crowd = points
                .iter()
                .map(|&p| spawn_crowd_body(&mut world, p))
                .collect();
            (body, crowd)
        }
        SpawnOrder::BodyLast => {
            let crowd: Vec<Entity> = points
                .iter()
                .rev()
                .map(|&p| spawn_crowd_body(&mut world, p))
                .collect();
            (spawn_body(&mut world), crowd)
        }
        SpawnOrder::Shuffled => {
            let mut shuffled = points.clone();
            let mut rng = Pcg32::seeded(0x0049_C0DE);
            for i in (1..shuffled.len()).rev() {
                let j = rng.next_u32() as usize % (i + 1);
                shuffled.swap(i, j);
            }
            let (front, back) = shuffled.split_at(CROWD / 2);
            let mut crowd: Vec<Entity> = front
                .iter()
                .map(|&p| spawn_crowd_body(&mut world, p))
                .collect();
            let body = spawn_body(&mut world);
            crowd.extend(back.iter().map(|&p| spawn_crowd_body(&mut world, p)));
            (body, crowd)
        }
    };
    let mut probe = empty_probe(path.len());
    for (step, &(dt, walk)) in press_script(dt).iter().enumerate() {
        // Every crowd body heads for the body's centre at the drive speed.
        let target = world.get::<Position>(body).unwrap().0;
        for &entity in &crowd {
            let from = world.get::<Position>(entity).unwrap().0;
            world.get_mut::<Velocity>(entity).unwrap().0 =
                (target - from).normalize_or_zero() * DRIVE_SPEED;
        }
        timed_step(&mut world, body, walk, dt, &mut probe.step_time);
        let mover = Mover { shape, body, path };
        let deepest = check_step(&world, &case, step, &mover, &crowd, true);
        probe.deepest = probe.deepest.max(deepest);
    }
    finish(&world, &crowd, &crowd, &mut probe);
    probe
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn crowd_presses_on_a_kinematic_body_from_every_side() {
    for shape in [BodyShape::Circle, BodyShape::Box] {
        for dt in PROBE_DTS {
            let path = kinematic_path(shape, Vec2::ZERO, &press_script(dt));
            for order in [
                SpawnOrder::BodyFirst,
                SpawnOrder::BodyLast,
                SpawnOrder::Shuffled,
            ] {
                let first = press_case(shape, order, dt, &path);
                let second = press_case(shape, order, dt, &path);
                assert_eq!(
                    first.hash, second.hash,
                    "press {shape:?} {order:?} dt {dt}: two runs apart"
                );
                println!(
                    "crowd press {shape:?} {order:?} dt {dt:.4}: {}",
                    first.line()
                );
            }
        }
    }
}

/// 500 points on a 20 by 25 grid 21 px apart, its first column 2 px to the
/// right of the body.
fn grid_points() -> Vec<Vec2> {
    (0..CROWD)
        .map(|index| {
            let (col, row) = ((index % 20) as f32, (index / 20) as f32);
            Vec2::new(
                BODY_SIZE + 2.0 + CROWD_RADIUS + col * CROWD_PITCH,
                (row - 12.0) * CROWD_PITCH,
            )
        })
        .collect()
}

/// Steps at 1/60 s until all 500 sleep, at most 120; their count.
fn settle_crowd(world: &mut World, body: Entity, time: &mut Duration) -> usize {
    for step in 1..=120 {
        timed_step(world, body, Vec2::ZERO, 1.0 / 60.0, time);
        if world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .sleeping_count()
            == CROWD
        {
            return step;
        }
    }
    panic!("the idle crowd never slept");
}

fn walk_case(shape: BodyShape, speed: f32, dt: f32) -> Probe {
    let case = format!("walk {shape:?} {speed} px/s dt {dt}");
    let mut world = probe_world();
    let body = spawn_probe_body(
        &mut world,
        RigidBody::kinematic(),
        shape.collider(),
        Vec2::ZERO,
    );
    let crowd: Vec<Entity> = grid_points()
        .into_iter()
        .map(|p| spawn_crowd_body(&mut world, p))
        .collect();
    let mut settle_time = Duration::ZERO;
    let settled = settle_crowd(&mut world, body, &mut settle_time);
    let mut script = vec![(1.0 / 60.0, Vec2::ZERO); settled];
    script.extend(std::iter::repeat_n((dt, Vec2::new(speed, 0.0)), 240));
    let path = kinematic_path(shape, Vec2::ZERO, &script);
    let mover = Mover {
        shape,
        body,
        path: &path,
    };
    let mut probe = empty_probe(240);
    let mut woke = vec![false; CROWD];
    for (step, &(dt, walk)) in script.iter().enumerate().skip(settled) {
        timed_step(&mut world, body, walk, dt, &mut probe.step_time);
        let deepest = check_step(&world, &case, step, &mover, &crowd, true);
        probe.deepest = probe.deepest.max(deepest);
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        for (woken, &entity) in woke.iter_mut().zip(&crowd) {
            *woken |= !buffers.is_sleeping(entity);
        }
    }
    probe.woke = woke.iter().filter(|&&woken| woken).count();
    finish(&world, &crowd, &crowd, &mut probe);
    probe
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn kinematic_body_walks_into_a_sleeping_crowd() {
    for shape in [BodyShape::Circle, BodyShape::Box] {
        for speed in [WALK_SPEED, 10.0] {
            for dt in PROBE_DTS {
                let first = walk_case(shape, speed, dt);
                let second = walk_case(shape, speed, dt);
                assert_eq!(
                    first.hash, second.hash,
                    "walk {shape:?} {speed} px/s dt {dt}: two runs apart"
                );
                println!(
                    "crowd walk {shape:?} {speed} px/s dt {dt:.4}: {}, woke {}",
                    first.line(),
                    first.woke
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Wall {
    StaticBox,
    Tiles,
}

/// The wall's face, where the pinned body rests.
const WALL_FACE: f32 = 92.0;
/// Where the pinning box starts: 20 px short of the pinned body.
const PIN_START: Vec2 = Vec2::new(40.0, 0.0);

fn pin_case(wall: Wall, dt: f32, path: &[[u32; 2]]) -> Probe {
    let case = format!("pin {wall:?} dt {dt}");
    let mut world = probe_world();
    match wall {
        Wall::StaticBox => {
            let entity = world.spawn();
            world.insert(entity, Position(Vec2::new(WALL_FACE + 8.0, 0.0)));
            world.insert(entity, Collider::aabb(Vec2::new(8.0, 64.0)));
            world.insert(entity, RigidBody::r#static());
        }
        Wall::Tiles => {
            let mut map_data = TilemapData::new(16, 16, 1, 8);
            map_data.tileset = vec!["solid".into()];
            map_data.layers = vec![TilemapLayer {
                name: "solid".into(),
                kind: LayerKind::Collision,
                tiles: vec![0; 8],
            }];
            world
                .get_resource_mut::<TilemapRegistry>()
                .unwrap()
                .insert("wall".into(), map_data);
            let map = world.spawn();
            world.insert(
                map,
                TilemapInstance::new("wall", Vec2::new(WALL_FACE, -64.0)),
            );
        }
    }
    let pinned = spawn_crowd_body(&mut world, Vec2::new(WALL_FACE - CROWD_RADIUS, 0.0));
    let body = spawn_probe_body(
        &mut world,
        RigidBody::kinematic(),
        BodyShape::Box.collider(),
        PIN_START,
    );
    let mover = Mover {
        shape: BodyShape::Box,
        body,
        path,
    };
    let mut probe = empty_probe(path.len());
    let crowd = [pinned];
    for (step, &(dt, walk)) in pin_script(dt).iter().enumerate() {
        timed_step(&mut world, body, walk, dt, &mut probe.step_time);
        let deepest = check_step(&world, &case, step, &mover, &crowd, false);
        probe.deepest = probe.deepest.max(deepest);
        probe.crossed |= world.get::<Position>(pinned).unwrap().0.x > WALL_FACE;
    }
    finish(&world, &crowd, &crowd, &mut probe);
    probe
}

/// 120 steps of the box walking along +x.
fn pin_script(dt: f32) -> Vec<(f32, Vec2)> {
    vec![(dt, Vec2::new(WALK_SPEED, 0.0)); 120]
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn kinematic_body_pins_a_body_against_a_wall() {
    for wall in [Wall::StaticBox, Wall::Tiles] {
        for dt in PROBE_DTS {
            let path = kinematic_path(BodyShape::Box, PIN_START, &pin_script(dt));
            let first = pin_case(wall, dt, &path);
            let second = pin_case(wall, dt, &path);
            assert_eq!(
                first.hash, second.hash,
                "pin {wall:?} dt {dt}: two runs apart"
            );
            println!(
                "crowd pin {wall:?} dt {dt:.4}: {}, crossed the wall {}",
                first.line(),
                first.crossed
            );
        }
    }
}
