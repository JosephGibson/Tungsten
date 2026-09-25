//! Diagnostic probe (ignored by default): characterizes the settled
//! dense-pile world from the physics bench — jitter floor, sleep engagement,
//! containment, wall time. Mirrors `build_dense_pile` from
//! benches/physics_bench.rs.
//! Run with: `cargo test --release -p tungsten-core --test substep_probe -- --ignored --nocapture`
//! Used to attribute bench deltas (state divergence vs real cost) in
//! docs/plans/physics-scale-and-ccd.md. Substeps are fixed via
//! `PhysicsConfig::substeps` since D-064 (no velocity-derived heuristic to
//! replicate). Since D-065 the default run reports the awake-body count so
//! sleep engagement is observable; a second sleep-disabled run re-records the
//! raw jitter floor the sleep threshold must sit above.

use glam::Vec2;
use tungsten_core::{
    Collider, DeltaTime, Entity, Pcg32, PhysicsBuffers, PhysicsConfig, Position, RigidBody,
    Velocity, World, physics_step,
};

const DT: f32 = 1.0 / 60.0;
const BODY_RADIUS: f32 = 6.0;
const SPAWN_SPACING: f32 = 14.0;
const PILE_WIDTH: f32 = 1_920.0;
const FLOOR_Y: f32 = 1_080.0;
const GRAVITY_Y: f32 = 900.0;
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

fn spawn_static_box(world: &mut World, width: f32, top_y: f32) {
    const WALL_HALF: f32 = 1_000.0;
    let mid_x = width * 0.5;
    let mid_y = f32::midpoint(top_y, FLOOR_Y);
    let half_h = (FLOOR_Y - top_y) * 0.5;
    let walls = [
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

fn spawn_pile(world: &mut World, count: usize, rng: &mut Pcg32) -> Vec<Entity> {
    let usable_width = PILE_WIDTH - SPAWN_SPACING * 2.0;
    let cols = ((usable_width / SPAWN_SPACING).floor() as usize).max(1);
    let mut bodies = Vec::with_capacity(count);
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
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(x, y)));
        world.insert(entity, Velocity(Vec2::ZERO));
        world.insert(entity, RigidBody::dynamic().with_restitution(0.1));
        world.insert(
            entity,
            Collider::circle(BODY_RADIUS).with_offset(Vec2::splat(BODY_RADIUS)),
        );
        bodies.push(entity);
    }
    bodies
}

fn probe_pile(count: usize, sleep_threshold: Option<f32>, steps: usize, label: &str) {
    let mut world = base_world(Vec2::new(0.0, GRAVITY_Y));
    if let Some(threshold) = sleep_threshold
        && let Some(cfg) = world.get_resource_mut::<PhysicsConfig>()
    {
        cfg.sleep_threshold = threshold;
    }
    let mut rng = Pcg32::seeded(0x7C0F_FEE5);
    let rows = count.div_ceil(((PILE_WIDTH - 28.0) / SPAWN_SPACING) as usize);
    let top_y = FLOOR_Y - SPAWN_SPACING * (rows as f32 + 2.0) - 500.0;
    spawn_static_box(&mut world, PILE_WIDTH, top_y);
    let bodies = spawn_pile(&mut world, count, &mut rng);
    for _ in 0..SETTLE_STEPS {
        physics_step(&mut world);
    }

    // Fixed-substep model (D-064): every step runs the configured count.
    let substeps = world
        .get_resource::<PhysicsConfig>()
        .map_or(0, |c| c.substeps);
    let threshold = world
        .get_resource::<PhysicsConfig>()
        .map_or(0.0, |c| c.sleep_threshold);
    let mut speed_sum_all = 0.0f64;
    let mut max_speed = 0.0f32;
    let mut escaped = 0usize;
    let tail_start = steps - 50;
    println!("== {label} ==");
    let t0 = std::time::Instant::now();
    let mut tail_time = std::time::Duration::ZERO;
    for step in 0..steps {
        let mut speed_sum = 0.0f64;
        let mut step_max = 0.0f32;
        let mut above = 0usize;
        for &e in &bodies {
            let v = world.get::<Velocity>(e).map_or(Vec2::ZERO, |v| v.0);
            let speed = v.length();
            speed_sum += f64::from(speed);
            step_max = step_max.max(speed);
            if speed > threshold {
                above += 1;
            }
        }
        max_speed = max_speed.max(step_max);
        speed_sum_all += speed_sum / bodies.len() as f64;
        let step_t0 = std::time::Instant::now();
        physics_step(&mut world);
        if step >= tail_start {
            tail_time += step_t0.elapsed();
        }
        if step % 20 == 0 || step == steps - 1 {
            let sleeping = world
                .get_resource::<PhysicsBuffers>()
                .map_or(0, PhysicsBuffers::sleeping_count);
            println!(
                "  step {:>3}: awake {:>6} / {count}, avg {:>6.1} px/s, max {:>7.1}, >thr {above}",
                step,
                count - sleeping,
                speed_sum / bodies.len() as f64,
                step_max
            );
        }
    }
    let elapsed = t0.elapsed();
    println!(
        "  steady-state physics_step (last 50 steps): {:.2} ms",
        tail_time.as_secs_f64() * 1e3 / 50.0
    );

    for &e in &bodies {
        let p = world.get::<Position>(e).map_or(Vec2::ZERO, |p| p.0);
        if p.y > FLOOR_Y + 50.0 || p.x < -50.0 || p.x > PILE_WIDTH + 50.0 {
            escaped += 1;
        }
    }

    println!("  fixed substeps per step: {substeps}");
    println!(
        "  avg body speed over {steps} steps: {:.1} px/s (max single-body {max_speed:.1})",
        speed_sum_all / steps as f64
    );
    println!("  escaped bodies: {escaped}");
    println!(
        "  avg step wall time (incl probe overhead): {:.2} ms",
        elapsed.as_secs_f64() * 1e3 / steps as f64
    );
}

#[test]
#[ignore = "diagnostic probe; run explicitly with --ignored"]
fn probe_dense_pile_10k() {
    // Default config: sleep engagement visible via the awake count (D-065).
    probe_pile(10_000, None, 300, "default config (sleeping enabled)");
    // Sleep disabled: raw jitter floor the sleep threshold must sit above.
    probe_pile(10_000, Some(0.0), 300, "sleep disabled (raw jitter floor)");
}

#[test]
#[ignore = "diagnostic probe; run explicitly with --ignored"]
fn probe_dense_pile_25k() {
    // Step 5 (D-066) done-when: the slept 25k steady state must run <= 2 ms.
    // 25k settles far past the 10k window, so give the pile a longer run and
    // read the last-50-step tail once fully asleep.
    probe_pile(25_000, None, 900, "25k default config (sleeping enabled)");
}
