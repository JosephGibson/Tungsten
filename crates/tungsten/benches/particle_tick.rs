//! Particle integrator bench: 5k live particles, full curve/tint work.

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glam::Vec2;

use tungsten::particles::particle_tick_system;
use tungsten_core::assets::{
    BlendMode, Curve, EmissionKind, InitialVelocity, ParticleConfig, Range,
};
use tungsten_core::{CommandBuffer, DeltaTime, Particle, Sprite, Transform, Visibility, World};

fn make_config() -> Arc<ParticleConfig> {
    Arc::new(ParticleConfig {
        sprite: "spark".into(),
        max_alive: 8_192,
        seed: Some(1),
        blend: BlendMode::Premultiplied,
        emission: EmissionKind::Continuous { rate_hz: 0.0 },
        lifetime: Range { min: 1.0, max: 2.0 },
        initial_velocity: InitialVelocity::Radial {
            speed: Range {
                min: 10.0,
                max: 40.0,
            },
        },
        gravity: [0.0, -30.0],
        drag_per_sec: 0.8,
        angular_velocity: Range {
            min: -2.0,
            max: 2.0,
        },
        start_scale: Range { min: 0.5, max: 1.2 },
        scale_over_life: Some(Curve {
            points: vec![(0.0, 0.2), (0.3, 1.0), (1.0, 0.0)],
        }),
        color_over_life: Some(Curve {
            points: vec![(0.0, [1.0, 0.8, 0.4, 1.0]), (1.0, [0.4, 0.2, 0.9, 1.0])],
        }),
        alpha_over_life: Some(Curve {
            points: vec![(0.0, 0.0), (0.15, 1.0), (1.0, 0.0)],
        }),
        tint: [1.0, 1.0, 1.0, 1.0],
    })
}

fn build_world(n: usize, cfg: &Arc<ParticleConfig>) -> (World, Vec<tungsten_core::Entity>) {
    let mut world = World::new();
    world.insert_resource(DeltaTime::new());
    world.insert_resource(CommandBuffer::new());

    let mut entities = Vec::with_capacity(n);
    for i in 0..n {
        let e = world.spawn();
        let phase = (i as f32) * 0.017;
        let lifetime = 1.0 + (i % 128) as f32 * (1.0 / 128.0);
        world.insert(
            e,
            Particle {
                config: cfg.clone(),
                emitter: None,
                age: 0.0,
                lifetime,
                velocity: Vec2::new(phase.sin() * 30.0, phase.cos() * 30.0),
                angular_velocity: (i % 13) as f32 * 0.1 - 0.65,
                start_scale: 1.0,
                base_rgba: [1.0, 1.0, 1.0, 1.0],
            },
        );
        world.insert(
            e,
            Transform::from_position(Vec2::new((i % 100) as f32, (i / 100) as f32)),
        );
        world.insert(e, Sprite::new("spark"));
        world.insert(e, Visibility::default());
        entities.push(e);
    }
    (world, entities)
}

fn bench_particle_tick_5k(c: &mut Criterion) {
    const N: usize = 5_000;
    let cfg = make_config();
    let (mut world, entities) = build_world(N, &cfg);
    if let Some(d) = world.get_resource_mut::<DeltaTime>() {
        d.dt = 1.0 / 60.0;
    }

    c.bench_function("particle_tick_5k", |b| {
        b.iter(|| {
            // Keep population constant across Criterion iterations.
            for &e in &entities {
                if let Some(p) = world.get_mut::<Particle>(e) {
                    p.age = 0.0;
                }
            }
            particle_tick_system(&mut world);
            if let Some(buf) = world.remove_resource::<CommandBuffer>() {
                world.flush(buf);
                world.insert_resource(CommandBuffer::new());
            }
            black_box(entities.len());
        });
    });
}

criterion_group!(benches, bench_particle_tick_5k);
criterion_main!(benches);
