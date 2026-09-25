//! Step-2 containment harness (docs/plans/physics-scale-and-ccd.md): a dense
//! 3k pile inside 80 px-thick static walls must never leak a body. Under the
//! pre-D-063 solver, Gauss-Seidel in-place MTV corrections shoved bodies past
//! the floor centerline within ~400 steps and the MTV ejected them out the
//! far side; the soft solver's clamped impulses + max push speed remove that
//! failure class.
//!
//! Release-only by cost (2,400 steps at 3k bodies); auto-ignored in debug.
//! Run with:
//! `cargo test --release -p tungsten-core --test physics_containment -- --nocapture`

use glam::Vec2;
use tungsten_core::{
    Collider, DeltaTime, Entity, Pcg32, PhysicsConfig, Position, RigidBody, Velocity, World,
    physics_step,
};

const DT: f32 = 1.0 / 60.0;
const BODY_RADIUS: f32 = 6.0;
const SPAWN_SPACING: f32 = 14.0;
const PILE_WIDTH: f32 = 1_920.0;
const FLOOR_Y: f32 = 1_080.0;
const GRAVITY_Y: f32 = 900.0;
/// Thin walls by design: thick enough to be a real level boundary, thin
/// enough that a body pushed past the centerline would visibly escape.
const WALL_HALF: f32 = 40.0;
const BODY_COUNT: usize = 3_000;
const STEPS: usize = 2_400;

/// Fully closed box (floor, roof, side walls) spanning `top_y..FLOOR_Y`,
/// 80 px thick; mirrors the bench geometry with thin walls.
fn spawn_static_box(world: &mut World, width: f32, top_y: f32) {
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
    let rows = count.div_ceil(cols);
    let start_y = FLOOR_Y - SPAWN_SPACING - rows as f32 * SPAWN_SPACING;

    let mut bodies = Vec::with_capacity(count);
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

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn dense_pile_never_escapes_thin_walls() {
    let mut world = World::new();
    world.insert_resource(DeltaTime { dt: DT });
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY_Y),
        ..PhysicsConfig::default()
    });

    let mut rng = Pcg32::seeded(0x7C0F_FEE5);
    let rows = BODY_COUNT.div_ceil(((PILE_WIDTH - 28.0) / SPAWN_SPACING) as usize);
    let top_y = FLOOR_Y - SPAWN_SPACING * (rows as f32 + 2.0) - 500.0;
    spawn_static_box(&mut world, PILE_WIDTH, top_y);
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
