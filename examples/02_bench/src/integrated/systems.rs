//! `integrated` gameplay systems, in frame order: AI, projectile
//! bookkeeping, the collision-event scan after the physics step, hit
//! reactions, flash and spark cleanup, animation, body sync, the scripted
//! camera, torch flicker and the HUD with its name tags.

use glam::Vec2;
use tungsten::ParticleSystemDrained;
use tungsten::core::tween::UniformOverrideBlock;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, CameraState, Collider, CollisionEvent,
    CommandBuffer, DeltaTime, Entity, EventQueue, Light, Particle, ParticleEmitter,
    ParticleEmitterState, Position, RigidBody, ShakeEvent, Sprite, SquashEvent, SquashTrigger,
    Transform, TweenComplete, Velocity, Visibility, World, splitmix64,
};
use tungsten::render::TextSection;

use super::IntegratedCounts;
use super::assets::{BOLT, PARTICLE_PX};
use super::runtime::{
    Actor, Body, FLASH_TAG, JUMP_EVERY, Kind, Projectile, Runtime, Spark, Torch, WALKER_HALF,
    swap_clip, top_left,
};
use super::scene::{BOLT_SCALE, BOLT_TINT, BOLT_Z, SPARK_ORIGIN};
use crate::counters::BenchCounters;
use crate::view::VIEWPORT;

const JUMP_SPEED: f32 = 330.0;
/// A random hop needs ground this far ahead, so tiers keep their walkers.
const JUMP_REACH: f32 = 56.0;
const TURN_COOLDOWN: f32 = 0.25;
/// Frames in the air before touching down counts as a landing.
const LAND_AIR_FRAMES: u16 = 6;
const BOLT_RADIUS: f32 = 3.0;
const BOLT_SPEED: f32 = 700.0;
const BOLT_MASS: f32 = 0.25;
/// Launch angle above the horizontal, radians.
const SHOT_ANGLE: (f32, f32) = (0.14, 0.5);
/// A projectile that never hits anything expires after this many seconds.
const BOLT_LIFETIME: f32 = 3.0;
const CRATE_KNOCK: f32 = 200.0;
const HIT_TRAUMA: f32 = 0.08;
/// Hits this close outside the view still shake the camera.
const SHAKE_REACH: f32 = 64.0;
const FONT: &str = "mono";
const TAG_COLOR: [u8; 4] = [236, 240, 255, 230];
const HUD_COLOR: [u8; 4] = [255, 244, 214, 255];

fn delta(world: &World) -> f32 {
    world
        .get_resource::<DeltaTime>()
        .map_or(0.0, DeltaTime::seconds)
}

fn count(world: &mut World, update: impl FnOnce(&mut IntegratedCounts)) {
    if let Some(counters) = world.get_resource_mut::<BenchCounters<IntegratedCounts>>() {
        update(&mut counters.counts);
    }
}

fn in_rect(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point.cmpge(min).all() && point.cmple(max).all()
}

/// Patrol AI, before the physics step. A walker blocked by a one-tile step
/// hops it; otherwise a block (last frame's contacts) or a ledge ahead
/// turns it. Grounded walkers hop at random where the landing is supported,
/// every walker walks, and casters fire through the `CommandBuffer`.
pub(super) fn actor_ai(world: &mut World) {
    let dt = delta(world);
    let Some(mut runtime) = world.remove_resource::<Runtime>() else {
        return;
    };
    let rate = runtime.params.fire_rate;
    let period = if rate > 0.0 {
        rate.recip()
    } else {
        f32::INFINITY
    };
    let mut shots: Vec<(Vec2, Vec2)> = Vec::new();
    let mut turns: u32 = 0;
    let rt = &mut runtime;
    for (_, actor, velocity, position) in world.query3_mut::<Actor, Velocity, Position>() {
        let slot = actor.slot as usize;
        let a = &mut rt.actors;
        let feet = position.0 + Vec2::new(0.0, WALKER_HALF.y);
        let ahead = feet + Vec2::new(a.dir[slot] * (WALKER_HALF.x + 3.0), 0.0);
        let grounded = a.grounded[slot];
        a.turn_cooldown[slot] = (a.turn_cooldown[slot] - dt).max(0.0);
        let step = grounded
            && rt.level.solid_at(ahead - Vec2::Y * 8.0)
            && !rt.level.solid_at(ahead - Vec2::Y * 24.0);
        let mut hop = a.blocked[slot] && step;
        if !hop && a.turn_cooldown[slot] == 0.0 {
            let ledge = grounded
                && !rt.level.solid_at(ahead + Vec2::Y * 4.0)
                && !rt.level.solid_at(ahead + Vec2::Y * 20.0);
            if a.blocked[slot] || ledge {
                a.dir[slot] = -a.dir[slot];
                a.turn_cooldown[slot] = TURN_COOLDOWN;
                turns += 1;
            }
        }
        a.jump_timer[slot] -= dt;
        if grounded && a.jump_timer[slot] <= 0.0 {
            a.jump_timer[slot] = rt.rng.next_range(JUMP_EVERY.0, JUMP_EVERY.1);
            hop |= rt
                .level
                .solid_at(feet + Vec2::new(a.dir[slot] * JUMP_REACH, 4.0));
        }
        velocity.0.x = a.dir[slot] * a.speed[slot];
        if hop && grounded {
            velocity.0.y = -JUMP_SPEED;
        }
        a.fire_timer[slot] -= dt;
        if a.fire_timer[slot] <= 0.0 {
            a.fire_timer[slot] += period;
            let angle = rt.rng.next_range(SHOT_ANGLE.0, SHOT_ANGLE.1);
            let dir = a.dir[slot];
            let muzzle = position.0 + Vec2::new(dir * (WALKER_HALF.x + BOLT_RADIUS + 2.0), -6.0);
            shots.push((
                muzzle,
                Vec2::new(dir * angle.cos(), -angle.sin()) * BOLT_SPEED,
            ));
        }
    }
    runtime.totals.shots += shots.len() as u64;
    runtime.totals.turns += u64::from(turns);
    world.insert_resource(runtime);
    let bolt = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprite(BOLT);
    if let Some(buf) = world.get_resource_mut::<CommandBuffer>() {
        for &(center, velocity) in &shots {
            let body = Body {
                half: Vec2::splat(BOLT_RADIUS),
                size: Vec2::splat(PARTICLE_PX as f32),
            };
            let scale = Vec2::splat(BOLT_SCALE);
            let mut sprite = Sprite::new(bolt);
            sprite.color = BOLT_TINT;
            sprite.z_order = BOLT_Z;
            let pending = buf.spawn();
            buf.insert_pending(pending, Position(center));
            buf.insert_pending(pending, Velocity(velocity));
            buf.insert_pending(pending, RigidBody::dynamic().with_mass(BOLT_MASS));
            buf.insert_pending(pending, Collider::circle(BOLT_RADIUS));
            buf.insert_pending(pending, Projectile::default());
            buf.insert_pending(pending, body);
            buf.insert_pending(
                pending,
                Transform {
                    position: top_left(center, body, scale),
                    rotation: 0.0,
                    scale,
                },
            );
            buf.insert_pending(pending, sprite);
            buf.insert_pending(pending, Visibility::default());
        }
    }
    let shot = shots.len() as u32;
    count(world, |counts| {
        counts.shots += shot;
        counts.turns += turns;
    });
}

/// Registers projectiles the last flush spawned, and expires old ones.
pub(super) fn projectiles(world: &mut World) {
    let dt = delta(world);
    let Some(mut runtime) = world.remove_resource::<Runtime>() else {
        return;
    };
    let mut expired = Vec::new();
    for (entity, projectile) in world.query_mut::<Projectile>() {
        if !projectile.registered {
            projectile.registered = true;
            runtime.set_kind(entity, Kind::Projectile);
        }
        projectile.age += dt;
        if projectile.age >= BOLT_LIFETIME {
            runtime.set_kind(entity, Kind::None);
            expired.push(entity);
        }
    }
    world.insert_resource(runtime);
    if let Some(buf) = world.get_resource_mut::<CommandBuffer>() {
        for entity in expired {
            buf.despawn(entity);
        }
    }
}

/// Scans this frame's collision events once, both sides of each contact:
/// flags grounded and blocked walkers and queues each projectile's first
/// contact as a hit. Walkers that touch down after `LAND_AIR_FRAMES` in the
/// air get a landing squash. `iter_current` skips the previous frame's
/// window, which `iter` would replay.
pub(super) fn collision_events(world: &mut World) {
    let Some(mut runtime) = world.remove_resource::<Runtime>() else {
        return;
    };
    runtime.actors.grounded.fill(false);
    runtime.actors.blocked.fill(false);
    runtime.hits.clear();
    let mut events = 0;
    if let Some(queue) = world.get_resource::<EventQueue<CollisionEvent>>() {
        for (order, event) in queue.iter_current().enumerate() {
            events += 1;
            runtime.touch(event.a, event.b, event.normal, order);
            if let Some(b) = event.b {
                runtime.touch(b, Some(event.a), -event.normal, order);
            }
        }
    }
    runtime
        .hits
        .sort_unstable_by_key(|hit| (hit.projectile.id(), hit.order));
    runtime.hits.dedup_by_key(|hit| hit.projectile.id());
    let mut landings = Vec::new();
    let actors = &mut runtime.actors;
    for slot in 0..actors.len() {
        if actors.grounded[slot] {
            if actors.air_frames[slot] >= LAND_AIR_FRAMES {
                landings.push(actors.entity[slot]);
            }
            actors.air_frames[slot] = 0;
        } else {
            actors.air_frames[slot] = actors.air_frames[slot].saturating_add(1);
        }
    }
    runtime.totals.landings += landings.len() as u64;
    world.insert_resource(runtime);
    if let Some(queue) = world.get_resource_mut::<EventQueue<SquashEvent>>() {
        for &entity in &landings {
            queue.send(SquashEvent {
                entity,
                trigger: SquashTrigger::OnLand,
            });
        }
    }
    let landed = landings.len() as u32;
    count(world, |counts| {
        counts.events += events;
        counts.landings += landed;
    });
}

/// Reacts to this frame's hits through the `CommandBuffer`: despawns the
/// projectile and spawns a fresh spark emitter at the impact. A burst fires
/// once whatever `once` says (the engine latches it), so every hit gets its
/// own emitter, which `spark_cleanup` removes when drained. A struck walker
/// flashes, a struck crate is knocked and woken, and hits near the view add
/// camera trauma.
pub(super) fn projectile_hits(world: &mut World) {
    let Some(mut runtime) = world.remove_resource::<Runtime>() else {
        return;
    };
    let hits = std::mem::take(&mut runtime.hits);
    let Some(mut buf) = world.remove_resource::<CommandBuffer>() else {
        world.insert_resource(runtime);
        return;
    };
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let (view_min, view_max) = camera.visible_world_aabb(VIEWPORT.x, VIEWPORT.y);
    let reach = Vec2::splat(SHAKE_REACH);
    let mut shakes = 0;
    for hit in &hits {
        let at = world
            .get::<Position>(hit.projectile)
            .map_or(Vec2::ZERO, |p| p.0);
        runtime.set_kind(hit.projectile, Kind::None);
        buf.despawn(hit.projectile);
        if let Some(config) = runtime.spark {
            runtime.spark_seed = splitmix64(runtime.spark_seed);
            let pending = buf.spawn();
            buf.insert_pending(pending, Transform::from_position(at - SPARK_ORIGIN));
            buf.insert_pending(
                pending,
                ParticleEmitter::with_seed(config, runtime.spark_seed),
            );
            buf.insert_pending(pending, ParticleEmitterState::default());
            buf.insert_pending(pending, Spark);
        }
        match hit.other.map_or(Kind::None, |other| runtime.kind(other)) {
            Kind::Actor(slot) => runtime.strike(world, &mut buf, slot as usize),
            Kind::Crate => {
                if let Some(other) = hit.other {
                    if let Some(velocity) = world.get_mut::<Velocity>(other) {
                        velocity.0 -= hit.normal * CRATE_KNOCK;
                    }
                    tungsten::physics::wake(world, other);
                }
            }
            Kind::Projectile | Kind::None => {}
        }
        if in_rect(at, view_min - reach, view_max + reach) {
            shakes += 1;
        }
    }
    world.insert_resource(buf);
    let hit_count = hits.len() as u32;
    runtime.totals.hits += u64::from(hit_count);
    runtime.hits = hits;
    world.insert_resource(runtime);
    if let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>() {
        for _ in 0..shakes {
            queue.send(ShakeEvent {
                trauma_add: HIT_TRAUMA,
            });
        }
    }
    count(world, |counts| counts.hits += hit_count);
}

/// Ends each finished flash: back to the lit clip, and the material and
/// override block go. The tween stage sends `TweenComplete` after the
/// systems, so it arrives in the previous window, which `iter` reads once.
pub(super) fn flash_restore(world: &mut World) {
    let done: Vec<Entity> = world
        .get_resource::<EventQueue<TweenComplete>>()
        .map(|queue| {
            queue
                .iter()
                .filter(|event| event.tag.as_deref() == Some(FLASH_TAG))
                .map(|event| event.entity)
                .collect()
        })
        .unwrap_or_default();
    if done.is_empty() {
        return;
    }
    let Some(mut runtime) = world.remove_resource::<Runtime>() else {
        return;
    };
    let mut restored = Vec::new();
    for entity in done {
        let Kind::Actor(slot) = runtime.kind(entity) else {
            continue;
        };
        let slot = slot as usize;
        if !runtime.actors.flashing[slot] {
            continue;
        }
        runtime.actors.flashing[slot] = false;
        swap_clip(world, entity, runtime.actors.variant[slot], None);
        restored.push(entity);
    }
    world.insert_resource(runtime);
    if let Some(buf) = world.get_resource_mut::<CommandBuffer>() {
        for entity in restored {
            buf.remove_component::<UniformOverrideBlock>(entity);
        }
    }
}

/// Despawns spark emitters whose burst has aged out. `ParticleSystemDrained`
/// is sent in the particle stage, so it arrives in the previous window.
pub(super) fn spark_cleanup(world: &mut World) {
    let drained: Vec<Entity> = world
        .get_resource::<EventQueue<ParticleSystemDrained>>()
        .map(|queue| queue.iter().map(|event| event.emitter).collect())
        .unwrap_or_default();
    let sparks: Vec<Entity> = drained
        .into_iter()
        .filter(|&emitter| world.has::<Spark>(emitter))
        .collect();
    if let Some(buf) = world.get_resource_mut::<CommandBuffer>() {
        for emitter in sparks {
            buf.despawn(emitter);
        }
    }
}

/// Advances every walker's clip; each frame change clones a sprite ID.
pub(super) fn animate_actors(world: &mut World) {
    let dt_ms = delta(world) * 1000.0;
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    for (_, state, sprite) in world.query2_mut::<AnimationState, Sprite>() {
        if let Some(next) = state.advance(dt_ms, &registry) {
            sprite.asset_id = next;
        }
    }
    world.insert_resource(registry);
}

/// Physics to visuals: stands each body's sprite on its collider's bottom
/// edge at the sprite's current scale, which a landing squash may have set.
pub(super) fn sync_bodies(world: &mut World) {
    for (_, transform, position, body) in world.query3_mut::<Transform, Position, Body>() {
        transform.position = top_left(position.0, *body, transform.scale);
    }
}

/// Writes the scripted path's base position; the camera update adds shake
/// and clamps to the level bounds.
pub(super) fn camera_script(world: &mut World) {
    let dt = delta(world);
    let Some(runtime) = world.get_resource_mut::<Runtime>() else {
        return;
    };
    runtime.elapsed += dt;
    let position = Vec2::new(
        runtime
            .camera
            .at(runtime.params.camera_speed * runtime.elapsed),
        runtime.camera.y,
    );
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.position = position;
        camera.zoom = 1.0;
    }
}

pub(super) fn torch_flicker(world: &mut World) {
    let Some(elapsed) = world
        .get_resource::<Runtime>()
        .map(|runtime| runtime.elapsed)
    else {
        return;
    };
    for (_, light, torch) in world.query2_mut::<Light, Torch>() {
        light.intensity = torch.base * (0.85 + 0.15 * (elapsed * torch.rate + torch.phase).sin());
    }
}

fn section(content: String, size: f32, color: [u8; 4], at: Vec2) -> TextSection {
    TextSection {
        content,
        font_id: FONT.to_string(),
        font_size: size,
        line_height: size + 4.0,
        color,
        position: [at.x, at.y],
        bounds: None,
        ..Default::default()
    }
}

/// Rewrites the HUD and the name tags: the `tags` walkers nearest the view
/// center, each labelled with its name, hit points and x position, so every
/// moving walker's tag changes every frame.
pub(super) fn name_tags(world: &mut World) {
    let Some(mut runtime) = world.remove_resource::<Runtime>() else {
        return;
    };
    runtime.frame += 1;
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let (view_min, view_max) = camera.visible_world_aabb(VIEWPORT.x, VIEWPORT.y);
    let center = (view_min + view_max) * 0.5;
    let mut near: Vec<(f32, u32, Vec2)> = world
        .query2::<Actor, Position>()
        .filter(|(_, _, position)| in_rect(position.0, view_min, view_max))
        .map(|(_, actor, position)| {
            (
                (position.0 - center).length_squared(),
                actor.slot,
                position.0,
            )
        })
        .collect();
    let tags = runtime.params.tags as usize;
    let nearest =
        |a: &(f32, u32, Vec2), b: &(f32, u32, Vec2)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
    if near.len() > tags {
        near.select_nth_unstable_by(tags, nearest);
        near.truncate(tags);
    }
    near.sort_unstable_by_key(|&(_, slot, _)| slot);
    runtime.tags.clear();
    for (_, slot, position) in near {
        let s = slot as usize;
        let name = if runtime.actors.caster[s] { 'C' } else { 'W' };
        let content = format!(
            "{name}{slot:04} {}hp x{}",
            runtime.actors.hp[s], position.x as i32
        );
        let at =
            (position - camera.position) * camera.zoom + Vec2::new(-24.0, -WALKER_HALF.y - 28.0);
        runtime.tags.push(section(content, 11.0, TAG_COLOR, at));
    }
    let totals = &runtime.totals;
    let lines = [
        format!(
            "integrated  frame {:06}  camera x {:5}",
            runtime.frame, camera.position.x as i32
        ),
        format!(
            "actors {}  flashing {}  projectiles {}  particles {}",
            runtime.actors.len(),
            runtime.actors.flashing(),
            world.query::<Projectile>().count(),
            world.query::<Particle>().count()
        ),
        format!(
            "shots {}  hits {}  landings {}  turns {}",
            totals.shots, totals.hits, totals.landings, totals.turns
        ),
    ];
    runtime.hud = lines
        .into_iter()
        .enumerate()
        .map(|(index, content)| {
            section(
                content,
                14.0,
                HUD_COLOR,
                Vec2::new(16.0, 16.0 + 20.0 * index as f32),
            )
        })
        .collect();
    world.insert_resource(runtime);
}
