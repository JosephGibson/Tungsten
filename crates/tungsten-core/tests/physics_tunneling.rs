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

use glam::Vec2;
use tungsten_core::{
    BodyKind, Collider, DeltaTime, PhysicsConfig, Position, RigidBody, Velocity, World,
    physics_step,
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
    world.insert_resource(DeltaTime { dt: DT });
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
/// **all four scenarios at all speeds** (D-064, step 3 of
/// docs/plans/physics-scale-and-ccd.md).
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
