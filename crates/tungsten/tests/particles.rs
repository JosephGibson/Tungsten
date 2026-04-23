//! Particle integration: refresh -> emit -> tick -> CommandBuffer flush.

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec2;

use tungsten::particles::{
    particle_count_refresh_system, particle_emit_system, particle_tick_system,
    ParticleBurstEmitted, ParticleSystemDrained,
};
use tungsten_core::assets::{
    BlendMode, EmissionKind, InitialVelocity, ParticleConfig, ParticleConfigRegistry, Range,
};
use tungsten_core::{
    CommandBuffer, DeltaTime, Entity, EventQueue, Particle, ParticleActive, ParticleBudget,
    ParticleEmitter, ParticleEmitterState, Transform, World, WorldRngSeed,
};

fn world_with_resources() -> World {
    let mut w = World::new();
    w.insert_resource(DeltaTime::new());
    w.insert_resource(CommandBuffer::new());
    w.insert_resource(ParticleBudget::default());
    w.insert_resource(ParticleActive::default());
    w.insert_resource(WorldRngSeed::default());
    w.insert_resource(EventQueue::<ParticleBurstEmitted>::new());
    w.insert_resource(EventQueue::<ParticleSystemDrained>::new());
    w
}

fn base_cfg(emission: EmissionKind, max_alive: u32) -> ParticleConfig {
    ParticleConfig {
        sprite: "spark".into(),
        max_alive,
        seed: Some(42),
        blend: BlendMode::Alpha,
        emission,
        lifetime: Range { min: 0.5, max: 0.5 },
        initial_velocity: InitialVelocity::Radial {
            speed: Range::single(10.0),
        },
        gravity: [0.0, 0.0],
        drag_per_sec: 0.0,
        angular_velocity: Range::single(0.0),
        start_scale: Range::single(1.0),
        scale_over_life: None,
        color_over_life: None,
        alpha_over_life: None,
        tint: [1.0, 1.0, 1.0, 1.0],
    }
}

fn register_config(
    world: &mut World,
    cfg: ParticleConfig,
    name: &str,
) -> tungsten_core::AssetId<ParticleConfig> {
    let mut reg = ParticleConfigRegistry::new();
    let id = reg.register(
        name.into(),
        PathBuf::from(format!("/tmp/{name}.json")),
        Arc::new(cfg),
    );
    world.insert_resource(reg);
    id
}

fn spawn_emitter(world: &mut World, config: tungsten_core::AssetId<ParticleConfig>) -> Entity {
    let e = world.spawn();
    world.insert(e, ParticleEmitter::new(config));
    world.insert(e, ParticleEmitterState::default());
    world.insert(e, Transform::from_position(Vec2::ZERO));
    e
}

fn tick(world: &mut World, dt: f32) {
    if let Some(d) = world.get_resource_mut::<DeltaTime>() {
        d.dt = dt;
    }
    particle_count_refresh_system(world);
    particle_emit_system(world);
    particle_tick_system(world);
    let buf = world
        .remove_resource::<CommandBuffer>()
        .expect("CommandBuffer missing");
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());
    if let Some(q) = world.get_resource_mut::<EventQueue<ParticleBurstEmitted>>() {
        q.flush();
    }
    if let Some(q) = world.get_resource_mut::<EventQueue<ParticleSystemDrained>>() {
        q.flush();
    }
}

fn count_particles(world: &mut World) -> usize {
    world.query_entities::<Particle>().len()
}

#[test]
fn burst_once_emits_exactly_count_then_drains() {
    let mut world = world_with_resources();
    let id = register_config(
        &mut world,
        base_cfg(
            EmissionKind::Burst {
                count: 8,
                once: true,
            },
            64,
        ),
        "burst",
    );
    let emitter = spawn_emitter(&mut world, id);

    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 8);

    // Event visible after flush in previous window.
    let bursts_prev: Vec<_> = world
        .get_resource::<EventQueue<ParticleBurstEmitted>>()
        .unwrap()
        .iter()
        .map(|e| e.count)
        .collect();
    assert_eq!(bursts_prev, vec![8]);

    for _ in 0..4 {
        tick(&mut world, 1.0 / 60.0);
    }
    let state = world.get::<ParticleEmitterState>(emitter).unwrap();
    assert!(state.drained);
}

#[test]
fn continuous_rate_matches_expected_count() {
    let mut world = world_with_resources();
    let mut cfg = base_cfg(EmissionKind::Continuous { rate_hz: 100.0 }, 500);
    cfg.lifetime = Range {
        min: 10.0,
        max: 10.0,
    };
    let id = register_config(&mut world, cfg, "cont");
    let _ = spawn_emitter(&mut world, id);

    // 1 second at 100 Hz; allow accumulator rounding.
    for _ in 0..60 {
        tick(&mut world, 1.0 / 60.0);
    }
    let n = count_particles(&mut world);
    assert!((99..=101).contains(&n), "expected ~100, got {n}");
}

#[test]
fn pulse_emits_fixed_pulses_then_drains() {
    let mut world = world_with_resources();
    let cfg = base_cfg(
        EmissionKind::Pulse {
            count_per_pulse: 4,
            interval_sec: 0.1,
            total_pulses: Some(3),
        },
        64,
    );
    let id = register_config(&mut world, cfg, "pulse");
    let emitter = spawn_emitter(&mut world, id);

    // Extra ticks let drain fire after active_count reaches zero.
    for _ in 0..40 {
        tick(&mut world, 0.05);
    }

    let state = world.get::<ParticleEmitterState>(emitter).unwrap();
    assert_eq!(state.pulses_fired, 3, "exactly 3 pulses");
    assert!(state.drained, "pulse emitter drained after total_pulses");
}

#[test]
fn per_emitter_max_alive_clips_emissions() {
    let mut world = world_with_resources();
    let cfg = base_cfg(
        EmissionKind::Burst {
            count: 1000,
            once: true,
        },
        16,
    );
    let id = register_config(&mut world, cfg, "clip");
    let _ = spawn_emitter(&mut world, id);

    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 16, "clipped to max_alive");
}

#[test]
fn global_budget_cap_clips_across_emitters() {
    let mut world = world_with_resources();
    if let Some(b) = world.get_resource_mut::<ParticleBudget>() {
        b.global_cap = 10;
    }
    let cfg = base_cfg(
        EmissionKind::Burst {
            count: 100,
            once: true,
        },
        1000,
    );
    let id = register_config(&mut world, cfg, "global");
    let _ = spawn_emitter(&mut world, id);
    let _ = spawn_emitter(&mut world, id);

    tick(&mut world, 1.0 / 60.0);
    assert!(count_particles(&mut world) <= 10);
}

#[test]
fn hot_reload_snapshot_preserves_live_particles() {
    let mut world = world_with_resources();
    let initial = base_cfg(
        EmissionKind::Burst {
            count: 4,
            once: true,
        },
        16,
    );
    let id = register_config(&mut world, initial, "snap");
    let _ = spawn_emitter(&mut world, id);
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 4);

    let entities = world.query_entities::<Particle>();
    let original_arcs: Vec<_> = entities
        .iter()
        .map(|e| Arc::as_ptr(&world.get::<Particle>(*e).unwrap().config))
        .collect();

    let new_cfg = Arc::new(base_cfg(
        EmissionKind::Burst {
            count: 4,
            once: true,
        },
        16,
    ));
    world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .replace(id, new_cfg.clone());

    // Live particles keep original config Arc across hot reload.
    let entities = world.query_entities::<Particle>();
    for (e, original) in entities.iter().zip(original_arcs.iter()) {
        let current = Arc::as_ptr(&world.get::<Particle>(*e).unwrap().config);
        assert_eq!(current, *original, "live particle Arc must not swap");
    }
}
