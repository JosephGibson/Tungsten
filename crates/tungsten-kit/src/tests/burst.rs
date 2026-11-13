use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec2;
use serde_json::json;
use tungsten_core::{
    Entity, Particle, ParticleConfig, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Plugin, Schedule, Stage, Transform, World,
};

use super::*;

/// A World with the burst plugin and the particle configs `sparks` and
/// `gore`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    let config: ParticleConfig = serde_json::from_value(json!({
        "sprite": "spark",
        "max_alive": 8,
        "emission": { "kind": "continuous", "rate_hz": 30.0 },
        "lifetime": { "min": 0.2, "max": 0.2 },
        "initial_velocity": { "kind": "radial", "speed": { "min": 40.0, "max": 60.0 } }
    }))
    .unwrap();
    let config = Arc::new(config);
    let mut configs = ParticleConfigRegistry::new();
    for name in ["sparks", "gore"] {
        configs.register(
            name.into(),
            PathBuf::from(format!("{name}.json")),
            Arc::clone(&config),
        );
    }
    world.insert_resource(configs);
    let mut schedule = Schedule::new();
    BurstPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// Marks `burst`'s emitter as the frame's particle tick leaves a drained
/// one.
fn drain(world: &mut World, burst: Entity) {
    let state = world.get_mut::<ParticleEmitterState>(burst).unwrap();
    state.first_tick_done = true;
    state.drained = true;
    state.drain_reported = true;
    state.active_count = 0;
}

fn cleanup(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

#[test]
fn a_burst_spawns_at_once_and_goes_with_its_particles() {
    let (mut world, mut schedule) = rig();
    let at = Vec2::new(30.0, -12.0);
    let first = burst(&mut world, "sparks", at, false).expect("a burst");
    // At once, so the frame's emit pass sees it: its point, its config and
    // its seed, the World's first burst.
    assert_eq!(world.get::<Transform>(first).unwrap().position, at);
    let emitter = world.get::<ParticleEmitter>(first).unwrap();
    let sparks = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name("sparks");
    assert_eq!(Some(emitter.config), sparks);
    let second = burst(&mut world, "sparks", at, false).unwrap();
    let orders = [first, second].map(|burst| world.get::<ParticleBurst>(burst).unwrap().order);
    assert_eq!(orders, [1, 2]);
    assert_eq!(world.resource::<BurstBudget>().spawned(), 2);
    assert!(
        burst(&mut world, "smoke", at, true).is_none(),
        "an unknown config"
    );
    // Drained, it stays while a particle it emitted lives, and goes with it.
    drain(&mut world, first);
    let config = Arc::clone(
        world
            .resource::<ParticleConfigRegistry>()
            .get(sparks.unwrap())
            .unwrap(),
    );
    let mote = world.spawn_with((Particle {
        config,
        emitter: Some(first),
        age: 0.0,
        lifetime: 0.2,
        velocity: Vec2::ZERO,
        angular_velocity: 0.0,
        start_scale: 1.0,
        base_rgba: [1.0; 4],
    },));
    cleanup(&mut world, &mut schedule);
    assert!(world.is_alive(first), "a particle of its own lives");
    world.despawn(mote);
    cleanup(&mut world, &mut schedule);
    assert!(!world.is_alive(first), "gone with its last particle");
    assert!(world.is_alive(second), "one still emitting stays");
}

#[test]
fn bursts_keep_their_budget() {
    let (mut world, _) = rig();
    world.insert_resource(BurstBudget::new().with_cap(2));
    let cosmetic = [
        burst(&mut world, "sparks", Vec2::ZERO, false).unwrap(),
        burst(&mut world, "sparks", Vec2::ZERO, false).unwrap(),
    ];
    // Cosmetic past the cap: dropped, uncounted.
    assert!(burst(&mut world, "sparks", Vec2::ZERO, false).is_none());
    assert_eq!(world.resource::<BurstBudget>().spawned(), 2);
    // A key burst drains the oldest cosmetic one's slot, its particles left
    // to fade.
    let gore = burst(&mut world, "gore", Vec2::ZERO, true).expect("a key burst");
    let oldest = world.get::<ParticleBurst>(cosmetic[0]).unwrap();
    assert!(!oldest.counted && !oldest.key);
    assert!(
        world
            .get::<ParticleEmitterState>(cosmetic[0])
            .unwrap()
            .drained
    );
    assert!(world.get::<ParticleBurst>(cosmetic[1]).unwrap().counted);
    assert!(world.get::<ParticleBurst>(gore).unwrap().key);
    // Then the next: the other cosmetic one's slot.
    let more = burst(&mut world, "gore", Vec2::ZERO, true).unwrap();
    assert!(!world.get::<ParticleBurst>(cosmetic[1]).unwrap().counted);
    // Every counted slot a key burst's: a key one is dropped too.
    assert!(burst(&mut world, "gore", Vec2::ZERO, true).is_none());
    let counted = world
        .query::<&ParticleBurst>()
        .filter(|burst| burst.counted)
        .count();
    assert_eq!(counted, 2);
    assert!(world.get::<ParticleBurst>(more).unwrap().counted);
    // A World without a budget takes the default.
    let mut bare = World::new();
    bare.insert_resource(ParticleConfigRegistry::new());
    assert!(burst(&mut bare, "sparks", Vec2::ZERO, false).is_none());
    assert_eq!(
        bare.resource::<BurstBudget>().cap(),
        BurstBudget::DEFAULT_CAP
    );
}
