//! The systems' map (`D-311`) in example 01: a burning pit with iron sliding
//! on it steps alike serially and with `SystemWorkers` forcing every map of
//! its fire and friction systems to fork, at 2, 3, 4, 6 and 12 participants.

use example_01_platformer::features::elements::{self, components::Flame};
use example_01_platformer::features::run::pit;
use example_01_platformer::features::toys::components::{IronBlock, SmallBall};
use glam::Vec2;
use tungsten::core::{Entity, Position, SystemWorkers, Velocity, World};
use tungsten::testing::UiHarness;

use super::run;

const COUNTS: [usize; 5] = [2, 3, 4, 6, 12];
const BALLS: usize = 2_000;
const SETTLE: u32 = 120;
const STEPS: u32 = 90;

/// Every body's position and velocity bits and each ball's flame: whether it
/// is exposed, by entity, in entity order.
fn state(world: &World) -> Vec<(u32, [u32; 4], Option<bool>)> {
    let mut rows: Vec<(u32, [u32; 4], Option<bool>)> = world
        .query::<(Entity, &Position, &Velocity, Option<&Flame>)>()
        .map(|(entity, at, velocity, flame)| {
            (
                entity.id(),
                [
                    at.0.x.to_bits(),
                    at.0.y.to_bits(),
                    velocity.0.x.to_bits(),
                    velocity.0.y.to_bits(),
                ],
                flame.map(|flame| flame.exposed),
            )
        })
        .collect();
    rows.sort_unstable_by_key(|row| row.0);
    rows
}

/// A run with the pit filled and settled, every fifth small ball lit and two
/// iron blocks dropped clear of the pit's shelves onto the pile, sliding at
/// 300 px/s, with `workers` in its World.
fn burning_pit(workers: Option<SystemWorkers>) -> (UiHarness, Vec<Entity>) {
    let (mut harness, _) = run();
    if let Some(workers) = workers {
        harness.world_mut().insert_resource(workers);
    }
    assert_eq!(pit::fill(harness.world_mut(), BALLS), BALLS);
    harness.step(SETTLE);
    let world = harness.world_mut();
    let mut balls: Vec<Entity> = world
        .query::<(Entity, &SmallBall)>()
        .map(|(entity, _)| entity)
        .collect();
    balls.sort_unstable_by_key(|entity| entity.id());
    for &ball in balls.iter().step_by(5) {
        elements::ignite(world, ball);
    }
    // The map places iron of its own, far from the pit: the cast blocks are
    // the ones the casts add.
    let placed = irons(world);
    pit::cast_at(world, "iron_block", Vec2::new(3300.0, 3200.0)).expect("an iron block");
    pit::cast_at(world, "iron_block", Vec2::new(4400.0, 3200.0)).expect("an iron block");
    let cast: Vec<Entity> = irons(world)
        .into_iter()
        .filter(|iron| !placed.contains(iron))
        .collect();
    assert_eq!(cast.len(), 2, "two blocks cast");
    for (&iron, speed) in cast.iter().zip([300.0, -300.0]) {
        world.get_mut::<Velocity>(iron).expect("iron moves").0.x = speed;
    }
    (harness, cast)
}

/// The iron blocks, in entity order.
fn irons(world: &World) -> Vec<Entity> {
    let mut irons: Vec<Entity> = world
        .query::<(Entity, &IronBlock)>()
        .map(|(entity, _)| entity)
        .collect();
    irons.sort_unstable_by_key(|entity| entity.id());
    irons
}

#[test]
fn a_burning_pit_with_iron_steps_alike_with_its_maps_forked() {
    let (mut serial, cast) = burning_pit(None);
    let mut forced: Vec<(usize, UiHarness)> = COUNTS
        .into_iter()
        .map(|count| (count, burning_pit(Some(SystemWorkers::forced(count, 16))).0))
        .collect();
    let mut peak_burning = 0;
    // Whether friction slowed a sliding block, as its fork must show.
    let mut slowed = false;
    let speeds = |world: &World| -> Vec<f32> {
        cast.iter()
            .map(|&iron| world.get::<Velocity>(iron).map_or(0.0, |v| v.0.x.abs()))
            .collect()
    };
    for step in 0..STEPS {
        let before = speeds(serial.world());
        serial.step(1);
        let after = speeds(serial.world());
        slowed |= before
            .iter()
            .zip(&after)
            .any(|(&was, &now)| was > 0.0 && now < was);
        let want = state(serial.world());
        let (burning, _) = pit::fire(serial.world());
        peak_burning = peak_burning.max(burning);
        for (count, harness) in &mut forced {
            harness.step(1);
            let got = state(harness.world());
            assert_eq!(got.len(), want.len(), "step {step}, {count} participants");
            for (got, want) in got.iter().zip(&want) {
                assert_eq!(
                    got, want,
                    "step {step}, {count} participants: entity {}",
                    want.0
                );
            }
        }
    }
    // The fire reached enough balls for every map to fork many chunks and
    // spread past the balls lit at the start, the iron slid and slowed, and
    // the forced worlds' workers started.
    assert!(peak_burning > 200, "{peak_burning} balls burned at once");
    let (burning, spent) = pit::fire(serial.world());
    assert!(
        burning + spent > BALLS / 5,
        "the fire spread: {burning} + {spent}"
    );
    assert!(
        cast.iter().all(|&iron| serial.world().is_alive(iron)),
        "both cast blocks stand"
    );
    assert!(slowed, "friction slowed no sliding block");
    for (count, harness) in &forced {
        let workers = harness.world().get_resource::<SystemWorkers>().unwrap();
        assert!(workers.started(), "{count} participants never forked");
    }
}
