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
        z_order: 0,
        clip: None,
        stretch: 0.0,
        stop: None,
        attractor: None,
        start_color: None,
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

/// A burst config whose particles outlive the test, so every burst stays
/// countable.
fn long_burst_cfg(once: bool) -> ParticleConfig {
    let mut cfg = base_cfg(EmissionKind::Burst { count: 5, once }, 64);
    cfg.lifetime = Range {
        min: 10.0,
        max: 10.0,
    };
    cfg
}

#[test]
fn burst_not_once_fires_again_after_rearm() {
    let mut world = world_with_resources();
    let id = register_config(&mut world, long_burst_cfg(false), "rearmable");
    let emitter = spawn_emitter(&mut world, id);

    let mut bursts = Vec::new();
    let mut drained_events = 0;
    for round in 0..3 {
        if round > 0 {
            world
                .get_mut::<ParticleEmitterState>(emitter)
                .unwrap()
                .rearm_burst();
        }
        // It fires on the first tick after a re-arm, and only then.
        for _ in 0..3 {
            tick(&mut world, 1.0 / 60.0);
            let queue = world
                .get_resource::<EventQueue<ParticleBurstEmitted>>()
                .unwrap();
            bursts.extend(queue.iter().map(|e| e.count));
            drained_events += world
                .get_resource::<EventQueue<ParticleSystemDrained>>()
                .unwrap()
                .iter()
                .count();
        }
    }

    assert_eq!(bursts, vec![5, 5, 5], "one burst per re-arm");
    assert_eq!(count_particles(&mut world), 15);
    let state = world.get::<ParticleEmitterState>(emitter).unwrap();
    assert!(!state.drained, "a once: false burst never drains");
    assert_eq!(drained_events, 0, "no ParticleSystemDrained");
}

#[test]
fn burst_once_ignores_rearm() {
    let mut world = world_with_resources();
    let id = register_config(&mut world, long_burst_cfg(true), "spent");
    let emitter = spawn_emitter(&mut world, id);

    tick(&mut world, 1.0 / 60.0);
    world
        .get_mut::<ParticleEmitterState>(emitter)
        .unwrap()
        .rearm_burst();
    for _ in 0..3 {
        tick(&mut world, 1.0 / 60.0);
    }

    assert_eq!(count_particles(&mut world), 5, "a spent burst stays spent");
    let state = world.get::<ParticleEmitterState>(emitter).unwrap();
    assert!(state.drained);
    assert!(state.burst_fired);
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
fn pulse_with_zero_total_emits_nothing_and_drains() {
    let mut world = world_with_resources();
    let cfg = base_cfg(
        EmissionKind::Pulse {
            count_per_pulse: 4,
            interval_sec: 0.1,
            total_pulses: Some(0),
        },
        64,
    );
    let id = register_config(&mut world, cfg, "no_pulses");
    let emitter = spawn_emitter(&mut world, id);

    // Drained on its first tick, before any interval has passed.
    tick(&mut world, 0.05);
    let state = world.get::<ParticleEmitterState>(emitter).unwrap();
    assert!(state.drained, "zero pulses drain on the first tick");
    let drained = world
        .get_resource::<EventQueue<ParticleSystemDrained>>()
        .unwrap()
        .iter()
        .count();
    assert_eq!(drained, 1, "ParticleSystemDrained comes once");

    // Past several intervals, still nothing.
    for _ in 0..10 {
        tick(&mut world, 0.05);
    }
    assert_eq!(count_particles(&mut world), 0, "no pulse fires");
    let state = world.get::<ParticleEmitterState>(emitter).unwrap();
    assert_eq!(state.pulses_fired, 0);
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

// Redo 7.9: depth, clips, stretch, stops, attractors, start colours, the
// emitter at run time.

/// Each thread's allocated bytes, for the allocation test; the binary's
/// allocator otherwise forwards to the system's.
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static BYTES: Cell<u64> = const { Cell::new(0) };
    }

    pub struct Counting;

    fn record(bytes: usize) {
        let _ = BYTES.try_with(|total| total.set(total.get() + bytes as u64));
    }

    // SAFETY: every call forwards to `System` unchanged; counting touches a
    // const-initialized thread-local cell, which does not allocate.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record(layout.size());
            // SAFETY: the caller's contract is `System.alloc`'s.
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            record(layout.size());
            // SAFETY: the caller's contract is `System.alloc_zeroed`'s.
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: the caller's contract is `System.dealloc`'s.
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            record(new_size);
            // SAFETY: the caller's contract is `System.realloc`'s.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    /// The calling thread's allocated bytes so far.
    pub fn bytes() -> u64 {
        BYTES.with(Cell::get)
    }
}

#[global_allocator]
static ALLOC: counting::Counting = counting::Counting;

fn burst(count: u32) -> EmissionKind {
    EmissionKind::Burst { count, once: true }
}

/// The live particles' (velocity, transform, sprite), by entity order.
fn particles(world: &World) -> Vec<(Particle, Transform, Sprite)> {
    let mut all: Vec<_> = world
        .query::<(Entity, &Particle, &Transform, &Sprite)>()
        .map(|(e, p, t, s)| (e.id(), (p.clone(), *t, *s)))
        .collect();
    all.sort_by_key(|(id, _)| *id);
    all.into_iter().map(|(_, parts)| parts).collect()
}

#[test]
fn a_newborn_draws_at_its_configs_z_order_first() {
    let mut world = world_with_resources();
    let mut cfg = base_cfg(burst(3), 10);
    cfg.z_order = 7;
    let id = register_config(&mut world, cfg, "deep");
    spawn_emitter(&mut world, id);
    tick(&mut world, 1.0 / 60.0);
    let born = particles(&world);
    assert_eq!(born.len(), 3);
    assert!(
        born.iter().all(|(p, _, s)| s.z_order == 7 && p.age == 0.0),
        "at its depth before its first tick"
    );
}

/// A world whose animation registry holds `spin`: sprites 1 then 2, 100 ms
/// each, looping, and `pop`: sprite 3, then 4, one-shot.
fn with_clips(world: &mut World) {
    use tungsten_core::SpriteAssetId;
    use tungsten_core::assets::{AnimationData, AnimationFrame, AnimationRegistry};
    let frames = |ids: &[u32]| {
        ids.iter()
            .map(|&i| AnimationFrame {
                sprite: SpriteAssetId::new(i),
                duration_ms: 100,
                sound: None,
            })
            .collect()
    };
    let mut clips = AnimationRegistry::new();
    clips.insert(
        "spin".into(),
        AnimationData {
            looping: true,
            frames: frames(&[1, 2]),
        },
    );
    clips.insert(
        "pop".into(),
        AnimationData {
            looping: false,
            frames: frames(&[3, 4]),
        },
    );
    world.insert_resource(clips);
}

#[test]
fn a_clip_steps_with_age() {
    let mut world = world_with_resources();
    with_clips(&mut world);
    let mut cfg = base_cfg(burst(1), 10);
    cfg.clip = Some("spin".into());
    cfg.lifetime = Range::single(1.0);
    let id = register_config(&mut world, cfg, "spinner");
    spawn_emitter(&mut world, id);
    tick(&mut world, 0.0);
    let shown = |world: &World| particles(world)[0].2.asset_id.index();
    assert_eq!(shown(&world), 1, "the clip's first frame from its spawn");
    for (dt, expected) in [(0.05, 1), (0.1, 2), (0.1, 1), (0.1, 2)] {
        tick(&mut world, dt);
        assert_eq!(
            shown(&world),
            expected,
            "at age {}",
            particles(&world)[0].0.age
        );
    }
}

#[test]
fn a_stretched_particle_lies_along_its_velocity() {
    use tungsten_core::AssetRegistry;
    use tungsten_core::assets::{FilterMode, TextureHandle, UvRect};
    let mut world = world_with_resources();
    let mut sprites = AssetRegistry::new();
    sprites.register_sprite(
        "spark".into(),
        FilterMode::Nearest,
        10,
        4,
        PathBuf::from("spark.png"),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
    world.insert_resource(sprites);
    let mut cfg = base_cfg(burst(1), 10);
    cfg.stretch = 0.1;
    cfg.angular_velocity = Range::single(3.0);
    cfg.initial_velocity = InitialVelocity::Vector {
        direction: [1.0, 1.0],
        speed: Range::single(100.0),
    };
    let id = register_config(&mut world, cfg, "streak");
    spawn_emitter(&mut world, id);
    tick(&mut world, 0.0);
    tick(&mut world, 1.0 / 60.0);
    let (_, t, _) = particles(&world)[0];
    assert!(
        (t.rotation - std::f32::consts::FRAC_PI_4).abs() < 1e-5,
        "along (1, 1), its spin ignored: {}",
        t.rotation
    );
    // 100 px/s × 0.1 s = 10 px more on a 10 px sprite: twice as long.
    assert!(
        (t.scale.x - 2.0).abs() < 1e-4 && (t.scale.y - 1.0).abs() < 1e-6,
        "{:?}",
        t.scale
    );
}

#[test]
fn a_stop_ends_a_particle_at_the_wall_it_flies_at() {
    use tungsten_core::assets::TilemapRegistry;
    use tungsten_core::assets::particle::{LayerRef, ParticleStop};
    use tungsten_core::physics::{
        Collider, PhysicsConfig, Position, RigidBodyBundle, physics_step,
    };
    let mut world = world_with_resources();
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    world.insert_resource(PhysicsConfig::default());
    world.insert_resource(TilemapRegistry::new());
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(100.0, 0.0)),
        Collider::aabb(Vec2::new(10.0, 50.0)),
    ));
    physics_step(&mut world);
    let mut cfg = base_cfg(burst(1), 10);
    cfg.lifetime = Range::single(2.0);
    cfg.initial_velocity = InitialVelocity::Vector {
        direction: [1.0, 0.0],
        speed: Range::single(100.0),
    };
    cfg.stop = Some(ParticleStop {
        mask: vec![LayerRef::Number(0)],
    });
    let id = register_config(&mut world, cfg.clone(), "stopped");
    spawn_emitter(&mut world, id);
    tick(&mut world, 0.0);
    let lifetime = particles(&world)[0].0.lifetime;
    assert!(
        (lifetime - 0.9).abs() < 1e-4,
        "90 px at 100 px/s: {lifetime}"
    );
    // A mask the wall is not on stops nothing.
    let mut world2 = world_with_resources();
    world2.insert_resource(PhysicsConfig::default());
    world2.insert_resource(TilemapRegistry::new());
    world2.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(100.0, 0.0)),
        Collider::aabb(Vec2::new(10.0, 50.0)),
    ));
    world2.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    physics_step(&mut world2);
    cfg.stop = Some(ParticleStop {
        mask: vec![LayerRef::Number(5)],
    });
    let id = register_config(&mut world2, cfg, "unstopped");
    spawn_emitter(&mut world2, id);
    tick(&mut world2, 0.0);
    assert_eq!(particles(&world2)[0].0.lifetime, 2.0);
}

#[test]
fn an_attractor_pulls_and_swirls_toward_its_emitter() {
    use tungsten_core::assets::particle::ParticleAttractor;
    let mut world = world_with_resources();
    let mut cfg = base_cfg(burst(1), 10);
    cfg.lifetime = Range::single(5.0);
    cfg.initial_velocity = InitialVelocity::Vector {
        direction: [1.0, 0.0],
        speed: Range::single(100.0),
    };
    cfg.attractor = Some(ParticleAttractor {
        pull: 600.0,
        swirl: 300.0,
    });
    let id = register_config(&mut world, cfg, "orbit");
    spawn_emitter(&mut world, id);
    tick(&mut world, 0.0);
    for _ in 0..10 {
        tick(&mut world, 1.0 / 60.0);
    }
    let (p, t, _) = particles(&world).remove(0);
    assert!(t.position.x > 0.0, "it flew out first");
    assert!(p.velocity.x < 100.0, "the pull slows it: {}", p.velocity.x);
    // The swirl turns it across the pull: toward is -x, so its cross is -y.
    assert!(p.velocity.y < 0.0, "the swirl: {}", p.velocity.y);
}

/// Every live particle's lifetime under `start_color`, the rest of `base_cfg`
/// alike, with a lifetime range so each draw shows.
fn lifetimes_with(
    start_color: Option<tungsten_core::assets::particle::StartColor>,
) -> Vec<(f32, [f32; 4])> {
    let mut world = world_with_resources();
    let mut cfg = base_cfg(burst(4), 10);
    cfg.lifetime = Range { min: 0.5, max: 1.5 };
    cfg.start_color = start_color;
    let id = register_config(&mut world, cfg, "coloured");
    spawn_emitter(&mut world, id);
    tick(&mut world, 0.0);
    particles(&world)
        .iter()
        .map(|(p, _, _)| (p.lifetime, p.base_rgba))
        .collect()
}

#[test]
fn a_start_color_range_draws_once_and_a_direction_curve_none() {
    use tungsten_core::assets::particle::StartColor;
    let plain = lifetimes_with(None);
    let ranged = lifetimes_with(Some(StartColor::Range([
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
    ])));
    let curved = lifetimes_with(Some(StartColor::ByDirection(Curve {
        points: vec![(0.0, [0.0, 1.0, 0.0, 1.0]), (1.0, [0.0, 1.0, 0.0, 1.0])],
    })));
    assert_eq!(
        plain[0].0, ranged[0].0,
        "the first particle's draws come first"
    );
    assert_ne!(
        plain[1].0, ranged[1].0,
        "one more draw a particle shifts the stream"
    );
    assert!(
        ranged
            .iter()
            .all(|(_, c)| c[1] == 0.0 && (c[0] + c[2] - 1.0).abs() < 1e-5),
        "between red and blue"
    );
    let lifetimes = |list: &[(f32, [f32; 4])]| list.iter().map(|(l, _)| *l).collect::<Vec<_>>();
    assert_eq!(
        lifetimes(&plain),
        lifetimes(&curved),
        "a direction curve draws nothing"
    );
    assert!(curved.iter().all(|(_, c)| *c == [0.0, 1.0, 0.0, 1.0]));
}

#[test]
fn an_unset_start_color_draws_nothing_new() {
    // The stream base_cfg's emitter draws, by hand: a lifetime, then a unit
    // vector, per particle; the start scale and spin are single values.
    let mut rng = tungsten_core::Pcg32::seeded(42);
    let expected: Vec<f32> = (0..4)
        .map(|_| {
            let lifetime = rng.next_range(0.5, 1.5);
            let _ = rng.next_unit_vec2();
            lifetime
        })
        .collect();
    let got: Vec<f32> = lifetimes_with(None).iter().map(|(l, _)| *l).collect();
    assert_eq!(got, expected, "an emitter's lifetimes as before");
}

#[test]
fn rate_scale_and_paused_steer_a_live_emitter() {
    let mut world = world_with_resources();
    let mut cfg = base_cfg(EmissionKind::Continuous { rate_hz: 60.0 }, 1_000);
    cfg.lifetime = Range::single(10.0);
    let id = register_config(&mut world, cfg, "fountain");
    let emitter = spawn_emitter(&mut world, id);
    world
        .get_mut::<ParticleEmitter>(emitter)
        .unwrap()
        .rate_scale = 0.5;
    for _ in 0..60 {
        tick(&mut world, 1.0 / 60.0);
    }
    assert_eq!(count_particles(&mut world), 30, "half the rate");
    world.get_mut::<ParticleEmitter>(emitter).unwrap().paused = true;
    for _ in 0..60 {
        tick(&mut world, 1.0 / 60.0);
    }
    assert_eq!(count_particles(&mut world), 30, "nothing while paused");
    // A burst not yet fired waits out the pause, then fires.
    let mut held = base_cfg(burst(5), 10);
    held.lifetime = Range::single(10.0);
    let mut reg = world.remove_resource::<ParticleConfigRegistry>().unwrap();
    let burst_id = reg.register(
        "held".into(),
        PathBuf::from("/tmp/held.json"),
        Arc::new(held),
    );
    world.insert_resource(reg);
    let waiting = spawn_emitter(&mut world, burst_id);
    world.get_mut::<ParticleEmitter>(waiting).unwrap().paused = true;
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 30);
    world.get_mut::<ParticleEmitter>(waiting).unwrap().paused = false;
    tick(&mut world, 1.0 / 60.0);
    assert_eq!(count_particles(&mut world), 35);
}

#[test]
fn a_direction_override_turns_a_cone() {
    let mut world = world_with_resources();
    let mut cfg = base_cfg(burst(6), 10);
    cfg.initial_velocity = InitialVelocity::Cone {
        direction: [1.0, 0.0],
        spread_deg: 0.0,
        speed: Range::single(50.0),
    };
    let id = register_config(&mut world, cfg, "jet");
    let emitter = spawn_emitter(&mut world, id);
    world.get_mut::<ParticleEmitter>(emitter).unwrap().direction = Some(Vec2::new(0.0, 2.0));
    tick(&mut world, 0.0);
    for (p, _, _) in particles(&world) {
        assert!(
            (p.velocity - Vec2::new(0.0, 50.0)).length() < 1e-4,
            "{:?}",
            p.velocity
        );
    }
}

#[test]
fn a_changed_config_takes_its_snapshot_at_the_next_tick() {
    let mut world = world_with_resources();
    let mut a = base_cfg(EmissionKind::Continuous { rate_hz: 60.0 }, 100);
    a.lifetime = Range { min: 0.5, max: 1.5 };
    let mut b = a.clone();
    b.z_order = 9;
    let mut reg = ParticleConfigRegistry::new();
    let a_id = reg.register("a".into(), PathBuf::from("/tmp/a.json"), Arc::new(a));
    let b_id = reg.register("b".into(), PathBuf::from("/tmp/b.json"), Arc::new(b));
    world.insert_resource(reg);
    let emitter = spawn_emitter(&mut world, a_id);
    tick(&mut world, 1.0 / 60.0);
    world.get_mut::<ParticleEmitter>(emitter).unwrap().config = b_id;
    tick(&mut world, 1.0 / 60.0);
    let born = particles(&world);
    assert_eq!(
        born.iter().map(|(_, _, s)| s.z_order).collect::<Vec<_>>(),
        [0, 9]
    );
    // The stream goes on: the second particle's lifetime is the next draw.
    let mut rng = tungsten_core::Pcg32::seeded(42);
    let first = rng.next_range(0.5, 1.5);
    let _ = rng.next_unit_vec2();
    let second = rng.next_range(0.5, 1.5);
    assert_eq!((born[0].0.lifetime, born[1].0.lifetime), (first, second));
}

#[test]
fn a_frame_without_the_new_fields_allocates_nothing_new() {
    use tungsten_core::assets::particle::ParticleAttractor;
    // A warm continuous emitter's window, plain and with a still attractor
    // (no pull, no swirl): the kept buffer allocates nothing once warm.
    let window = |attractor: Option<ParticleAttractor>| {
        let mut world = world_with_resources();
        let mut cfg = base_cfg(EmissionKind::Continuous { rate_hz: 120.0 }, 500);
        cfg.attractor = attractor;
        let id = register_config(&mut world, cfg, "steady");
        spawn_emitter(&mut world, id);
        for _ in 0..90 {
            tick(&mut world, 1.0 / 60.0);
        }
        let before = counting::bytes();
        for _ in 0..60 {
            tick(&mut world, 1.0 / 60.0);
        }
        counting::bytes() - before
    };
    let plain = window(None);
    let still = window(Some(ParticleAttractor {
        pull: 0.0,
        swirl: 0.0,
    }));
    assert_eq!(plain, still, "plain {plain} B, with an attractor {still} B");
}
