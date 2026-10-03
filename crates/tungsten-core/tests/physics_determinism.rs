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

mod common;

use common::{FLOOR_Y, PILE_WIDTH, SPAWN_SPACING, spawn_pile, spawn_static_box};
use glam::Vec2;
use tungsten_core::{
    DeltaTime, Entity, Pcg32, PhysicsConfig, Position, Velocity, World, physics_step,
};

const DT: f32 = 1.0 / 60.0;
const GRAVITY_Y: f32 = 900.0;
/// Half the thickness of the bench's static walls.
const WALL_HALF: f32 = 1_000.0;
const BODY_COUNT: usize = 3_000;
const STEPS: usize = 240;

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
    spawn_static_box(&mut world, PILE_WIDTH, top_y, WALL_HALF);
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
    // From `just physics-release` (the perf runner's flags, generic x86-64)
    // on the 0.40 tree at `afbc330`. A bug fix that moves it updates this
    // value and records the old and new hash in its `CHANGELOG.md` line.
    const EXPECTED: u64 = 0x088e_c07a_73c1_b168;
    let first = run_pile();
    let second = run_pile();
    println!("state hashes — first: {first:#018x}, second: {second:#018x}");
    assert_eq!(
        first, second,
        "physics_step diverged between identical runs"
    );
    assert_eq!(
        first, EXPECTED,
        "the pile's state hash moved: {first:#018x}"
    );
}
