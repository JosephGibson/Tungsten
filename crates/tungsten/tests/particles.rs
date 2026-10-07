//! Particle integration: refresh -> emit -> tick -> `CommandBuffer` flush.

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec2;

use tungsten::particles::{
    ParticleBurstEmitted, ParticleSystemDrained, extract_mesh_particles,
    particle_count_refresh_system, particle_emit_system, particle_tick_system,
    spawn_mesh_particle_via,
};
use tungsten_core::assets::{
    BlendMode, Curve, EmissionKind, InitialVelocity, ParticleConfig, ParticleConfigRegistry,
    ParticleMesh, ParticleMeshAssetId, ParticleMeshRegistry, ParticleRender, Range,
};
use tungsten_core::{
    CommandBuffer, Entity, EventQueue, MeshParticle, Particle, ParticleActive, ParticleBudget,
    ParticleEmitter, ParticleEmitterState, Sprite, Time, Transform, Visibility, With, World,
    WorldRngSeed,
};

fn world_with_resources() -> World {
    let mut w = World::new();
    w.insert_resource(Time::new());
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
        render: ParticleRender::Quad,
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
    if let Some(time) = world.get_resource_mut::<Time>() {
        time.advance_frame(dt);
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
    world.query_filtered::<Entity, With<Particle>>().count()
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

    let entities = world
        .query_filtered::<Entity, With<Particle>>()
        .collect::<Vec<_>>();
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
    let entities = world
        .query_filtered::<Entity, With<Particle>>()
        .collect::<Vec<_>>();
    for (e, original) in entities.iter().zip(original_arcs.iter()) {
        let current = Arc::as_ptr(&world.get::<Particle>(*e).unwrap().config);
        assert_eq!(current, *original, "live particle Arc must not swap");
    }
}

fn mesh_cfg(emission: EmissionKind, max_alive: u32) -> ParticleConfig {
    let mut cfg = base_cfg(emission, max_alive);
    cfg.sprite = String::new();
    cfg.render = ParticleRender::Mesh { mesh: "tri".into() };
    cfg
}

fn triangle() -> ParticleMesh {
    ParticleMesh {
        vertices: vec![[0.0, -7.0], [6.0, 7.0], [-6.0, 7.0]],
        indices: vec![0, 1, 2],
    }
}

fn register_triangle(world: &mut World) -> ParticleMeshAssetId {
    let mut reg = ParticleMeshRegistry::new();
    let id = reg.insert("tri", triangle());
    world.insert_resource(reg);
    id
}

fn spawn_mesh_entity(
    world: &mut World,
    mesh: ParticleMeshAssetId,
    transform: Transform,
    color: [u8; 4],
    visible: bool,
) -> Entity {
    let e = world.spawn();
    world.insert(e, transform);
    world.insert(e, MeshParticle { mesh, color });
    world.insert(e, Visibility { visible });
    e
}

#[test]
fn mesh_config_spawns_mesh_particle_without_sprite() {
    let mut world = world_with_resources();
    let mesh = register_triangle(&mut world);
    let id = register_config(
        &mut world,
        mesh_cfg(
            EmissionKind::Burst {
                count: 8,
                once: true,
            },
            64,
        ),
        "mesh_burst",
    );
    let _ = spawn_emitter(&mut world, id);

    tick(&mut world, 1.0 / 60.0);

    let particles = world
        .query_filtered::<Entity, With<Particle>>()
        .collect::<Vec<_>>();
    assert_eq!(particles.len(), 8);
    for e in particles {
        let drawn = world.get::<MeshParticle>(e).expect("mesh particle");
        assert_eq!(drawn.mesh, mesh);
        assert_eq!(drawn.color, [255; 4]);
        assert!(
            world.get::<Sprite>(e).is_none(),
            "no Sprite on a mesh particle"
        );
        assert!(world.get::<Visibility>(e).is_some_and(|v| v.visible));
        assert!(world.get::<Transform>(e).is_some());
    }
}

#[test]
fn mesh_particles_count_against_budget_and_max_alive() {
    // Per-emitter `max_alive`.
    let mut world = world_with_resources();
    register_triangle(&mut world);
    let id = register_config(
        &mut world,
        mesh_cfg(
            EmissionKind::Burst {
                count: 1000,
                once: true,
            },
            16,
        ),
        "mesh_clip",
    );
    let emitter = spawn_emitter(&mut world, id);
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 16, "clipped to max_alive");

    particle_count_refresh_system(&mut world);
    assert_eq!(world.get_resource::<ParticleActive>().unwrap().count, 16);
    assert_eq!(
        world
            .get::<ParticleEmitterState>(emitter)
            .unwrap()
            .active_count,
        16
    );

    // Global `ParticleBudget`.
    let mut world = world_with_resources();
    register_triangle(&mut world);
    world
        .get_resource_mut::<ParticleBudget>()
        .unwrap()
        .global_cap = 10;
    let id = register_config(
        &mut world,
        mesh_cfg(
            EmissionKind::Burst {
                count: 100,
                once: true,
            },
            1000,
        ),
        "mesh_global",
    );
    let _ = spawn_emitter(&mut world, id);
    let _ = spawn_emitter(&mut world, id);
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 10, "clipped to the global cap");
}

#[test]
fn mesh_particle_ages_out_and_tick_writes_color() {
    let mut world = world_with_resources();
    register_triangle(&mut world);
    let mut cfg = mesh_cfg(
        EmissionKind::Burst {
            count: 1,
            once: true,
        },
        4,
    );
    cfg.alpha_over_life = Some(Curve {
        points: vec![(0.0, 1.0), (1.0, 0.0)],
    });
    cfg.angular_velocity = Range::single(2.0);
    let id = register_config(&mut world, cfg, "mesh_fade");
    let _ = spawn_emitter(&mut world, id);

    // Spawned at the end of the first tick, so it has not aged yet.
    tick(&mut world, 1.0 / 60.0);
    let e = world
        .query_filtered::<Entity, With<Particle>>()
        .next()
        .expect("an entity matches");
    assert_eq!(world.get::<MeshParticle>(e).unwrap().color, [255; 4]);

    // Half of the 0.5 s lifetime: alpha 0.5, moved and rotated.
    tick(&mut world, 0.25);
    assert_eq!(
        world.get::<MeshParticle>(e).unwrap().color,
        [255, 255, 255, 127]
    );
    let transform = *world.get::<Transform>(e).unwrap();
    assert!((transform.position.length() - 2.5).abs() < 1.0e-4);
    assert!((transform.rotation - 0.5).abs() < 1.0e-6);

    tick(&mut world, 0.3);
    assert_eq!(count_particles(&mut world), 0, "aged out and despawned");
}

#[test]
fn unknown_mesh_name_emits_nothing() {
    // No registry at all.
    let mut world = world_with_resources();
    let cfg = mesh_cfg(EmissionKind::Continuous { rate_hz: 600.0 }, 64);
    let id = register_config(&mut world, cfg.clone(), "mesh_unknown");
    let _ = spawn_emitter(&mut world, id);
    tick(&mut world, 1.0 / 60.0);
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 0);

    // A registry that lacks the name.
    let mut world = world_with_resources();
    let mut reg = ParticleMeshRegistry::new();
    reg.insert("other", triangle());
    world.insert_resource(reg);
    let id = register_config(&mut world, cfg, "mesh_unknown");
    let _ = spawn_emitter(&mut world, id);
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 0);
    assert_eq!(world.get_resource::<ParticleActive>().unwrap().count, 0);
}

#[test]
fn extract_groups_instances_by_mesh() {
    let mut world = World::new();
    let empty = extract_mesh_particles(&world);
    assert!(empty.is_empty());
    assert_eq!(empty.capacity(), 0, "no allocation without mesh particles");

    let a = ParticleMeshAssetId::new(0);
    let b = ParticleMeshAssetId::new(1);
    let at = |x: f32| Transform {
        position: Vec2::new(x, -x),
        rotation: 0.25,
        scale: Vec2::new(2.0, 3.0),
    };
    spawn_mesh_entity(&mut world, a, at(1.0), [1, 2, 3, 4], true);
    spawn_mesh_entity(&mut world, b, at(2.0), [5, 6, 7, 8], true);
    spawn_mesh_entity(&mut world, a, at(3.0), [9, 10, 11, 12], true);

    let batches = extract_mesh_particles(&world);
    assert_eq!(batches.len(), 2, "one batch per mesh");
    assert_eq!(batches[0].mesh, a, "first-seen order");
    assert_eq!(batches[1].mesh, b);

    let xs: Vec<f32> = batches[0].instances.iter().map(|i| i.position[0]).collect();
    assert_eq!(xs, [1.0, 3.0]);
    assert_eq!(batches[1].instances.len(), 1);

    let instance = batches[1].instances[0];
    assert_eq!(instance.position, [2.0, -2.0]);
    assert_eq!(instance.scale, [2.0, 3.0]);
    assert_eq!(instance.rotation, 0.25);
    assert_eq!(instance.color, [5, 6, 7, 8]);
}

#[test]
fn extract_skips_invisible_mesh_particles() {
    let mut world = World::new();
    let mesh = ParticleMeshAssetId::new(3);
    spawn_mesh_entity(
        &mut world,
        mesh,
        Transform::from_position(Vec2::new(1.0, 0.0)),
        [255; 4],
        false,
    );
    assert!(extract_mesh_particles(&world).is_empty());

    // `spawn_mesh_particle_via` spawns a visible mesh particle.
    let cfg = Arc::new(mesh_cfg(EmissionKind::Continuous { rate_hz: 0.0 }, 4));
    let mut cmd = CommandBuffer::new();
    spawn_mesh_particle_via(
        &mut cmd,
        None,
        cfg,
        mesh,
        Vec2::new(4.0, 5.0),
        Vec2::ZERO,
        1.0,
        2.0,
    );
    world.flush(cmd);

    let batches = extract_mesh_particles(&world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].mesh, mesh);
    assert_eq!(batches[0].instances.len(), 1);
    assert_eq!(batches[0].instances[0].position, [4.0, 5.0]);
    assert_eq!(batches[0].instances[0].scale, [2.0, 2.0]);
    assert_eq!(count_particles(&mut world), 1);
}
