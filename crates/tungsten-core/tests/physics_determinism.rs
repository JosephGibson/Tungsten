//! Step-6 determinism gate (docs/plans/physics-scale-and-ccd.md, D-067):
//! `physics_step` must produce bit-identical world state across full runs on
//! identical inputs. The step is serial end to end (D-067 dropped the
//! parallel solver), so this pins the property future work must preserve:
//! gather order, pair ordering, union-find, and the solver iteration order
//! are all deterministic.
//!
//! The scenario is the bench's dense 3k pile measured from spawn, so the hash
//! covers full churn (fall, impact, pile compression), the sleep transition,
//! and the settled tail in one run.
//!
//! Release-only by cost; auto-ignored in debug. Run with:
//! `cargo test --release -p tungsten-core --test physics_determinism -- --nocapture`

use glam::Vec2;
use tungsten_core::{
    physics_step, Collider, DeltaTime, Entity, Pcg32, PhysicsConfig, Position, RigidBody, Velocity,
    World,
};

const DT: f32 = 1.0 / 60.0;
const BODY_RADIUS: f32 = 6.0;
const SPAWN_SPACING: f32 = 14.0;
const PILE_WIDTH: f32 = 1_920.0;
const FLOOR_Y: f32 = 1_080.0;
const GRAVITY_Y: f32 = 900.0;
const BODY_COUNT: usize = 3_000;
const STEPS: usize = 240;

fn spawn_static_box(world: &mut World, width: f32, top_y: f32) {
    const WALL_HALF: f32 = 1_000.0;
    let mid_x = width * 0.5;
    let mid_y = (top_y + FLOOR_Y) * 0.5;
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

/// FNV-1a over every body's position/velocity bits, in spawn order.
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

fn run_pile() -> u64 {
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
    state_hash(&world, &bodies)
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-scale step test; run with --release"
)]
fn physics_step_is_bit_identical_across_runs() {
    let first = run_pile();
    let second = run_pile();
    println!("state hashes — first: {first:#018x}, second: {second:#018x}");
    assert_eq!(
        first, second,
        "physics_step diverged between identical runs"
    );
}
