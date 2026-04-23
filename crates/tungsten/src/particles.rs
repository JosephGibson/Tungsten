//! Particle lifecycle: count refresh -> emit -> tick -> command flush -> event flush.
//!
//! Spawn/despawn visibility waits for command flush; burst/drain events flush after it.

use std::sync::Arc;

use glam::Vec2;

use tungsten_core::{
    BlendMode, CommandBuffer, Curve, EmissionKind, Entity, EventQueue, InitialVelocity, Particle,
    ParticleActive, ParticleBudget, ParticleConfig, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Range, Sprite, Transform, Visibility, World, WorldRngSeed,
};

/// Discrete emission event; `count` is post-budget clipping.
#[derive(Debug, Clone, Copy)]
pub struct ParticleBurstEmitted {
    pub emitter: Entity,
    pub count: u32,
}

/// One-shot event after emitter drains and live particles age out.
#[derive(Debug, Clone, Copy)]
pub struct ParticleSystemDrained {
    pub emitter: Entity,
}

/// Rebuild active counts before emission budget clipping.
pub fn particle_count_refresh_system(world: &mut World) {
    let emitter_entities = world.query2_entities::<ParticleEmitter, ParticleEmitterState>();
    for e in &emitter_entities {
        if let Some(state) = world.get_mut::<ParticleEmitterState>(*e) {
            state.active_count = 0;
        }
    }

    let particle_entities = world.query_entities::<Particle>();
    let mut total: u32 = 0;
    for p_ent in &particle_entities {
        let emitter_owner = world.get::<Particle>(*p_ent).and_then(|p| p.emitter);
        total = total.saturating_add(1);
        if let Some(owner) = emitter_owner {
            if let Some(state) = world.get_mut::<ParticleEmitterState>(owner) {
                state.active_count = state.active_count.saturating_add(1);
            }
        }
    }

    if let Some(active) = world.get_resource_mut::<ParticleActive>() {
        active.count = total;
    }
}

/// Emit particles via command buffer; discrete emissions enqueue burst events.
pub fn particle_emit_system(world: &mut World) {
    let dt = world
        .get_resource::<tungsten_core::DeltaTime>()
        .map(|d| d.dt)
        .unwrap_or(0.0);

    let budget = world
        .get_resource::<ParticleBudget>()
        .copied()
        .unwrap_or_default();
    let mut global_active = world
        .get_resource::<ParticleActive>()
        .map(|a| a.count)
        .unwrap_or(0);

    let emitter_entities =
        world.query3_entities::<ParticleEmitter, ParticleEmitterState, Transform>();

    for emitter_ent in emitter_entities {
        let (config_id, seed_override) = match world.get::<ParticleEmitter>(emitter_ent) {
            Some(em) => (em.config, em.seed_override),
            None => continue,
        };

        // Config snapshot resolved once, on first tick.
        let needs_snapshot = world
            .get::<ParticleEmitterState>(emitter_ent)
            .map(|s| !s.first_tick_done)
            .unwrap_or(false);

        if needs_snapshot {
            let snapshot = world
                .get_resource::<ParticleConfigRegistry>()
                .and_then(|r| r.get(config_id).cloned());
            let Some(snapshot) = snapshot else {
                continue;
            };

            let seed = if let Some(s) = seed_override {
                s
            } else if let Some(s) = snapshot.seed {
                s
            } else if let Some(ws) = world.get_resource_mut::<WorldRngSeed>() {
                ws.derive_seed()
            } else {
                0
            };
            if let Some(state) = world.get_mut::<ParticleEmitterState>(emitter_ent) {
                state.config_snapshot = Some(snapshot);
                state.rng = tungsten_core::Pcg32::seeded(seed);
                state.first_tick_done = true;
            }
        }

        let snapshot = match world
            .get::<ParticleEmitterState>(emitter_ent)
            .and_then(|s| s.config_snapshot.clone())
        {
            Some(s) => s,
            None => continue,
        };

        let origin = match world.get::<Transform>(emitter_ent) {
            Some(t) => t.position,
            None => continue,
        };

        let (to_emit, is_discrete) = plan_emission(world, emitter_ent, dt, &snapshot);

        if to_emit == 0 {
            maybe_emit_drained(world, emitter_ent);
            continue;
        }

        let (active_count, max_alive) = {
            let state = world.get::<ParticleEmitterState>(emitter_ent);
            (
                state.map(|s| s.active_count).unwrap_or(0),
                snapshot.max_alive,
            )
        };
        let per_emitter_headroom = max_alive.saturating_sub(active_count);
        let global_headroom = budget.global_cap.saturating_sub(global_active);
        let n_eff = to_emit.min(per_emitter_headroom).min(global_headroom);

        if n_eff == 0 {
            maybe_emit_drained(world, emitter_ent);
            continue;
        }

        // Batch spawn without holding a `&mut World` resource borrow.
        let mut buf = match world.remove_resource::<CommandBuffer>() {
            Some(b) => b,
            None => continue,
        };

        for _ in 0..n_eff {
            let (particle, transform, sprite) =
                build_particle(world, emitter_ent, &snapshot, origin);
            let pending = buf.spawn();
            buf.insert_pending(pending, particle);
            buf.insert_pending(pending, transform);
            buf.insert_pending(pending, sprite);
            buf.insert_pending(pending, Visibility { visible: true });
        }
        world.insert_resource(buf);

        // Pre-flush bookkeeping; next count refresh reconciles archetype state.
        if let Some(state) = world.get_mut::<ParticleEmitterState>(emitter_ent) {
            state.active_count = state.active_count.saturating_add(n_eff);
        }
        global_active = global_active.saturating_add(n_eff);
        if let Some(active) = world.get_resource_mut::<ParticleActive>() {
            active.count = global_active;
        }

        if is_discrete {
            if let Some(q) = world.get_resource_mut::<EventQueue<ParticleBurstEmitted>>() {
                q.send(ParticleBurstEmitted {
                    emitter: emitter_ent,
                    count: n_eff,
                });
            }
        }

        maybe_emit_drained(world, emitter_ent);
    }
}

/// Age/integrate particles; despawns deferred to command flush.
pub fn particle_tick_system(world: &mut World) {
    let dt = world
        .get_resource::<tungsten_core::DeltaTime>()
        .map(|d| d.dt)
        .unwrap_or(0.0);
    if dt <= 0.0 {
        return;
    }

    let mut buf = match world.remove_resource::<CommandBuffer>() {
        Some(b) => b,
        None => return,
    };

    let particle_entities = world.query3_entities::<Particle, Transform, Sprite>();
    for entity in particle_entities {
        // Snapshot before writeback to keep borrows disjoint.
        let (config, age_new, lifetime) = {
            let p = match world.get::<Particle>(entity) {
                Some(p) => p,
                None => continue,
            };
            (p.config.clone(), p.age + dt, p.lifetime)
        };

        if age_new >= lifetime {
            buf.despawn(entity);
            continue;
        }

        let u = (age_new / lifetime).clamp(0.0, 1.0);

        let drag_factor = (-config.drag_per_sec * dt).exp();
        let gravity = Vec2::new(config.gravity[0], config.gravity[1]);
        let (new_vel, new_ang_vel, start_scale, base_rgba) = {
            let p = world.get::<Particle>(entity).unwrap();
            (
                (p.velocity + gravity * dt) * drag_factor,
                p.angular_velocity,
                p.start_scale,
                p.base_rgba,
            )
        };

        if let Some(p) = world.get_mut::<Particle>(entity) {
            p.age = age_new;
            p.velocity = new_vel;
        }

        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position += new_vel * dt;
            t.rotation += new_ang_vel * dt;
            let scale = start_scale * sample_or_one(config.scale_over_life.as_ref(), u);
            t.scale = Vec2::splat(scale);
        }

        let mut rgba = match config.color_over_life.as_ref() {
            Some(c) => c.sample(u),
            None => [1.0, 1.0, 1.0, 1.0],
        };
        if let Some(alpha_curve) = config.alpha_over_life.as_ref() {
            rgba[3] *= alpha_curve.sample(u);
        }
        if matches!(config.blend, BlendMode::Premultiplied) {
            rgba[0] *= rgba[3];
            rgba[1] *= rgba[3];
            rgba[2] *= rgba[3];
        }
        let final_rgba = [
            (rgba[0] * base_rgba[0]).clamp(0.0, 1.0),
            (rgba[1] * base_rgba[1]).clamp(0.0, 1.0),
            (rgba[2] * base_rgba[2]).clamp(0.0, 1.0),
            (rgba[3] * base_rgba[3]).clamp(0.0, 1.0),
        ];
        if let Some(s) = world.get_mut::<Sprite>(entity) {
            s.color = [
                (final_rgba[0] * 255.0) as u8,
                (final_rgba[1] * 255.0) as u8,
                (final_rgba[2] * 255.0) as u8,
                (final_rgba[3] * 255.0) as u8,
            ];
        }
    }

    world.insert_resource(buf);
}

fn plan_emission(
    world: &mut World,
    emitter_ent: Entity,
    dt: f32,
    cfg: &Arc<ParticleConfig>,
) -> (u32, bool) {
    let state = match world.get_mut::<ParticleEmitterState>(emitter_ent) {
        Some(s) => s,
        None => return (0, false),
    };
    state.elapsed += dt;

    if state.drained {
        return (0, false);
    }

    match cfg.emission {
        EmissionKind::Burst { count, once } => {
            // One-shot latch: `continuous_accum` 0 = pending, 1 = fired.
            if state.continuous_accum >= 1.0 {
                return (0, true);
            }
            state.continuous_accum = 1.0;
            if once {
                state.drained = true;
            }
            (count, true)
        }
        EmissionKind::Continuous { rate_hz } => {
            state.continuous_accum += rate_hz * dt;
            let n = state.continuous_accum.floor().max(0.0) as u32;
            state.continuous_accum -= n as f32;
            (n, false)
        }
        EmissionKind::Pulse {
            count_per_pulse,
            interval_sec,
            total_pulses,
        } => {
            state.pulse_timer += dt;
            // At most one pulse per tick; overflow waits.
            if state.pulse_timer < interval_sec {
                return (0, true);
            }
            state.pulse_timer -= interval_sec;
            state.pulses_fired = state.pulses_fired.saturating_add(1);
            if let Some(total) = total_pulses {
                if state.pulses_fired >= total {
                    state.drained = true;
                }
            }
            (count_per_pulse, true)
        }
    }
}

fn maybe_emit_drained(world: &mut World, emitter_ent: Entity) {
    let (drained, active, already) = match world.get::<ParticleEmitterState>(emitter_ent) {
        Some(s) => (s.drained, s.active_count, s.drain_reported),
        None => return,
    };
    if !drained || active != 0 || already {
        return;
    }
    if let Some(state) = world.get_mut::<ParticleEmitterState>(emitter_ent) {
        state.drain_reported = true;
    }
    if let Some(q) = world.get_resource_mut::<EventQueue<ParticleSystemDrained>>() {
        q.send(ParticleSystemDrained {
            emitter: emitter_ent,
        });
    }
}

fn build_particle(
    world: &mut World,
    emitter_ent: Entity,
    cfg: &Arc<ParticleConfig>,
    origin: Vec2,
) -> (Particle, Transform, Sprite) {
    let state = world.get_mut::<ParticleEmitterState>(emitter_ent).unwrap();
    let lifetime = sample_range(&mut state.rng, cfg.lifetime).max(1.0e-4);
    let start_scale = sample_range(&mut state.rng, cfg.start_scale).max(0.0);
    let angular_velocity = sample_range(&mut state.rng, cfg.angular_velocity);
    let velocity = sample_initial_velocity(&mut state.rng, &cfg.initial_velocity);

    let mut rgba = match cfg.color_over_life.as_ref() {
        Some(c) => c.sample(0.0),
        None => [1.0, 1.0, 1.0, 1.0],
    };
    if let Some(alpha_curve) = cfg.alpha_over_life.as_ref() {
        rgba[3] *= alpha_curve.sample(0.0);
    }
    if matches!(cfg.blend, BlendMode::Premultiplied) {
        rgba[0] *= rgba[3];
        rgba[1] *= rgba[3];
        rgba[2] *= rgba[3];
    }
    let base_rgba = [cfg.tint[0], cfg.tint[1], cfg.tint[2], cfg.tint[3]];
    let initial_color = [
        (rgba[0] * base_rgba[0] * 255.0).clamp(0.0, 255.0) as u8,
        (rgba[1] * base_rgba[1] * 255.0).clamp(0.0, 255.0) as u8,
        (rgba[2] * base_rgba[2] * 255.0).clamp(0.0, 255.0) as u8,
        (rgba[3] * base_rgba[3] * 255.0).clamp(0.0, 255.0) as u8,
    ];

    let particle = Particle {
        config: cfg.clone(),
        emitter: Some(emitter_ent),
        age: 0.0,
        lifetime,
        velocity,
        angular_velocity,
        start_scale,
        base_rgba,
    };
    let transform = Transform {
        position: origin,
        rotation: 0.0,
        scale: Vec2::splat(start_scale),
    };
    let sprite = Sprite {
        asset_id: cfg.sprite.clone(),
        color: initial_color,
        z_order: 0,
    };
    (particle, transform, sprite)
}

fn sample_range(rng: &mut tungsten_core::Pcg32, r: Range) -> f32 {
    if r.max <= r.min {
        r.min
    } else {
        rng.next_range(r.min, r.max)
    }
}

fn sample_initial_velocity(rng: &mut tungsten_core::Pcg32, iv: &InitialVelocity) -> Vec2 {
    match iv {
        InitialVelocity::Cone {
            direction,
            spread_deg,
            speed,
        } => {
            let base = Vec2::new(direction[0], direction[1]).normalize_or_zero();
            let spread_rad = spread_deg.to_radians();
            let jitter = rng.next_range(-spread_rad * 0.5, spread_rad * 0.5);
            let (sj, cj) = jitter.sin_cos();
            let dir = Vec2::new(base.x * cj - base.y * sj, base.x * sj + base.y * cj);
            dir * sample_range(rng, *speed)
        }
        InitialVelocity::Radial { speed } => rng.next_unit_vec2() * sample_range(rng, *speed),
        InitialVelocity::Vector { direction, speed } => {
            let base = Vec2::new(direction[0], direction[1]).normalize_or_zero();
            base * sample_range(rng, *speed)
        }
    }
}

fn sample_or_one(curve: Option<&Curve<f32>>, t: f32) -> f32 {
    curve.map(|c| c.sample(t)).unwrap_or(1.0)
}

/// Spawn a fully formed particle without the emit system.
pub fn spawn_particle_via(
    cmd: &mut CommandBuffer,
    emitter_ent: Option<Entity>,
    config: Arc<ParticleConfig>,
    position: Vec2,
    velocity: Vec2,
    lifetime: f32,
    start_scale: f32,
) {
    let sprite_id = config.sprite.clone();
    let base = config.tint;
    let particle = Particle {
        config,
        emitter: emitter_ent,
        age: 0.0,
        lifetime,
        velocity,
        angular_velocity: 0.0,
        start_scale,
        base_rgba: base,
    };
    let pending = cmd.spawn();
    cmd.insert_pending(pending, particle);
    cmd.insert_pending(
        pending,
        Transform {
            position,
            rotation: 0.0,
            scale: Vec2::splat(start_scale),
        },
    );
    cmd.insert_pending(
        pending,
        Sprite {
            asset_id: sprite_id,
            color: [255, 255, 255, 255],
            z_order: 0,
        },
    );
    cmd.insert_pending(pending, Visibility { visible: true });
}
