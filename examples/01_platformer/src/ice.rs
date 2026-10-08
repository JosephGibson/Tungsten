//! Hold F to channel a cold spray toward the cursor, gradually frosting iron.
//! Frozen iron stays brittle until a thermal blast or hard iron impact breaks it apart.
use glam::Vec2;
use tungsten::core::{
    ActionMap, AudioCommands, AudioHandle, CameraState, Entity, InputState, Particle,
    ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Time, Transform, World,
};
use tungsten::physics::{Collider, Position, Shape};

use crate::brick::{IronBrick, IronScrap};
use crate::state::{
    EffectSequence, EffectSounds, Player, PlayerPresentation, TRANSIENT_EMITTER_CAP,
};
use crate::systems::{cursor_to_world, play_effect_sound, spawn_transient_effect};

pub(crate) const ICE_BEAM_RANGE: f32 = 7.0 * crate::state::TILE;
pub(crate) const FREEZE_TIME: f32 = 1.5;
const THAW_RATE: f32 = 0.15;
const SPRAY_HALF_ANGLE: f32 = 14.0 * std::f32::consts::PI / 180.0;
const BEAM_HALF_WIDTH: f32 = 4.0;

/// Frozen objects have reduced sliding friction and shatter under heat or hard iron impacts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frozen;

/// Partial frost; only a completed coating gains the brittle `Frozen` marker.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Chill {
    pub(crate) amount: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IceBeam {
    owner: Entity,
    pub(crate) start: Vec2,
    pub(crate) end: Vec2,
    pub(crate) age: f32,
    emitters: [Option<Entity>; 3],
    looping_sound: Option<AudioHandle>,
}

/// The engine's authored cones face +X; rotate newborns once into their birth aim.
#[derive(Clone, Copy)]
struct IceSprayEmitter {
    direction: Vec2,
}

struct OrientedIceParticle;

/// A brief completion or fracture cue, independent of the held spray's lifetime.
#[derive(Clone, Copy)]
pub(crate) struct IcePulse {
    pub(crate) at: Vec2,
    pub(crate) size: f32,
    pub(crate) age: f32,
    pub(crate) shatter: bool,
}

impl IcePulse {
    pub(crate) fn duration(self) -> f32 {
        if self.shatter { 0.32 } else { 0.24 }
    }
}

pub(crate) fn ice_pulse(world: &mut World, at: Vec2, size: f32, shatter: bool) {
    if world.query::<(Entity, &IcePulse)>().count() >= TRANSIENT_EMITTER_CAP {
        return;
    }
    // A group of scraps shares one crack/chime; visual cues remain per object.
    let recent = world
        .query::<(Entity, &IcePulse)>()
        .any(|(_, pulse)| pulse.shatter == shatter && pulse.age < 0.1);
    if !recent {
        play_effect_sound(
            world,
            if shatter {
                |s| s.ice_shatter
            } else {
                |s| s.ice_freeze
            },
        );
    }
    world.spawn_with((IcePulse {
        at,
        size,
        age: 0.0,
        shatter,
    },));
}

fn channel_held(world: &World) -> bool {
    !crate::death::player_dead(world)
        && world
            .get_resource::<InputState>()
            .zip(world.get_resource::<ActionMap>())
            .is_some_and(|(input, actions)| actions.is_pressed(input, "cast_ice_beam"))
}

fn aimed_direction(world: &World, player: Entity, center: Vec2) -> Vec2 {
    let facing = if world
        .get::<PlayerPresentation>(player)
        .is_some_and(|p| p.facing_left)
    {
        -Vec2::X
    } else {
        Vec2::X
    };
    let target = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
        .zip(world.get_resource::<CameraState>())
        .and_then(|((x, y), camera)| cursor_to_world(Vec2::new(x, y), camera));
    target.map_or(facing, |at| (at - center).try_normalize().unwrap_or(facing))
}

/// Fixed-step exposure avoids changing freeze speed with the draw frame rate.
pub(crate) fn cast_ice_beam_system(world: &mut World) {
    ice_beam_cleanup(world);
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let mut exposed = Vec::new();
    let player = world
        .query::<(Entity, &Player, &Position)>()
        .next()
        .map(|(e, _, p)| (e, p.0));
    if channel_held(world)
        && let Some((player, center)) = player
    {
        let aim = aimed_direction(world, player, center);
        if world.query::<(Entity, &IceBeam)>().next().is_none() {
            start_channel(world, player, center, aim);
        }
        // Seven occluded rays cover the same 28-degree cone as the particles.
        // Count each surface once, regardless of how many rays touch it.
        for ray in -3..=3 {
            let direction = Vec2::from_angle(ray as f32 / 3.0 * SPRAY_HALF_ANGLE).rotate(aim);
            if let (at, Some(hit)) = trace_beam(world, center, direction, ICE_BEAM_RANGE)
                && (world.has::<IronBrick>(hit) || world.has::<IronScrap>(hit))
                && !world.has::<Frozen>(hit)
                && !exposed.iter().any(|&(e, _)| e == hit)
            {
                exposed.push((hit, at));
            }
        }
    }
    if dt <= 0.0 {
        return;
    }
    let cooling: Vec<_> = world
        .query_mut::<(Entity, &mut Chill)>()
        .filter_map(|(e, chill)| {
            if !exposed.iter().any(|&(hit, _)| hit == e) {
                chill.amount = (chill.amount - THAW_RATE * dt).max(0.0);
            }
            (chill.amount == 0.0).then_some(e)
        })
        .collect();
    for e in cooling {
        world.remove_component::<Chill>(e);
    }
    for (hit, contact) in exposed {
        let amount = freeze_amount(world, hit) + dt / FREEZE_TIME;
        if amount >= 1.0 - 1e-5 {
            freeze_iron(world, hit);
            let at = world.get::<Position>(hit).map_or(Vec2::ZERO, |p| p.0);
            let size = if world.has::<IronBrick>(hit) {
                128.0
            } else {
                32.0
            };
            // Put crystals on the contacted face where the iron cannot hide them.
            spawn_transient_effect(world, "ex10_ice_frost", contact);
            ice_pulse(world, at, size, false);
        } else {
            world.insert(hit, Chill { amount });
        }
    }
}

fn start_channel(world: &mut World, owner: Entity, center: Vec2, aim: Vec2) {
    let (end, _) = trace_beam(world, center, aim, ICE_BEAM_RANGE);
    let start = center + aim * 24.0_f32.min(center.distance(end));
    let emitters = ["ex10_ice_beam", "ex10_ice_mist", "ex10_ice_contact"].map(|name| {
        let config = world
            .get_resource::<ParticleConfigRegistry>()?
            .id_for_name(name)?;
        let seed = if let Some(sequence) = world.get_resource_mut::<EffectSequence>() {
            sequence.0 = sequence.0.wrapping_add(1);
            0x1ce0_0000 + sequence.0
        } else {
            0x1ce0_0000 + owner.id() as u64
        };
        Some(world.spawn_with((
            ParticleEmitter::with_seed(config, seed),
            ParticleEmitterState::default(),
            Transform::from_position(start),
        )))
    });
    for emitter in emitters[..2].iter().flatten() {
        world.insert(*emitter, IceSprayEmitter { direction: aim });
    }
    let sound = world.get_resource::<EffectSounds>().map(|s| s.ice_loop);
    play_effect_sound(world, |s| s.ice_cast);
    let looping_sound = if let Some((handle, volume)) = sound
        && let Some(commands) = world.get_resource_mut::<AudioCommands>()
    {
        commands.play_with(handle, volume, true);
        Some(handle)
    } else {
        None
    };
    world.spawn_with((IceBeam {
        owner,
        start,
        end,
        age: 0.0,
        emitters,
        looping_sound,
    },));
}

pub(crate) fn freeze_amount(world: &World, entity: Entity) -> f32 {
    if world.has::<Frozen>(entity) {
        1.0
    } else {
        world.get::<Chill>(entity).map_or(0.0, |c| c.amount)
    }
}

pub(crate) fn freeze_iron(world: &mut World, entity: Entity) {
    if !world.has::<Frozen>(entity)
        && (world.has::<IronBrick>(entity) || world.has::<IronScrap>(entity))
    {
        world.insert(entity, Frozen);
        world.remove_component::<Chill>(entity);
        tungsten::physics::wake(world, entity);
    }
}

pub(crate) fn ice_beam_cleanup(world: &mut World) {
    let expired: Vec<_> = world
        .query::<(Entity, &IceBeam)>()
        .filter(|(_, beam)| !channel_held(world) || !world.has::<Player>(beam.owner))
        .map(|(e, beam)| (e, *beam))
        .collect();
    if !expired.is_empty() {
        orient_newborn_particles(world);
    }
    for (e, beam) in expired {
        for emitter in beam.emitters.into_iter().flatten() {
            world.despawn(emitter);
        }
        if let Some(handle) = beam.looping_sound
            && let Some(commands) = world.get_resource_mut::<AudioCommands>()
        {
            commands.stop(handle);
        }
        if world.has::<Player>(beam.owner) && !crate::death::player_dead(world) {
            play_effect_sound(world, |s| s.ice_end);
        }
        world.despawn(e);
    }
}

/// Follow the interpolated caster and current cursor on every drawn frame.
pub(crate) fn ice_beam_presentation(world: &mut World) {
    ice_beam_cleanup(world);
    // Birth aim belongs to the previous emission, before today's aim is written.
    orient_newborn_particles(world);
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let expired: Vec<_> = world
        .query_mut::<(Entity, &mut IcePulse)>()
        .filter_map(|(e, pulse)| {
            pulse.age += dt;
            (pulse.age >= pulse.duration()).then_some(e)
        })
        .collect();
    for e in expired {
        world.despawn(e);
    }
    // Start the visual/audio channel even on a high-refresh frame with no fixed step.
    let caster = world.query::<(Entity, &Player)>().next().map(|(e, _)| e);
    if dt > 0.0
        && channel_held(world)
        && world.query::<(Entity, &IceBeam)>().next().is_none()
        && let Some(owner) = caster
        && let Some(center) = crate::extract::drawn_position(world, owner)
    {
        let aim = aimed_direction(world, owner, center);
        start_channel(world, owner, center, aim);
    }
    let beams: Vec<_> = world
        .query::<(Entity, &IceBeam)>()
        .map(|(e, b)| (e, *b))
        .collect();
    for (e, mut beam) in beams {
        let Some(center) = crate::extract::drawn_position(world, beam.owner) else {
            continue;
        };
        let aim = aimed_direction(world, beam.owner, center);
        let (end, _) = trace_beam(world, center, aim, ICE_BEAM_RANGE);
        let contact = spray_contact(world, center, aim);
        beam.start = center + aim * 24.0_f32.min(center.distance(end));
        beam.end = end;
        beam.age += dt;
        for (index, emitter) in beam.emitters.into_iter().enumerate() {
            let Some(emitter) = emitter else { continue };
            let at = if index == 2 {
                contact.unwrap_or(end)
            } else {
                beam.start
            };
            if let Some(transform) = world.get_mut::<Transform>(emitter) {
                transform.position = at;
            }
            if let Some(spray) = world.get_mut::<IceSprayEmitter>(emitter) {
                spray.direction = aim;
            }
            if index == 2
                && let Some(state) = world.get_mut::<ParticleEmitterState>(emitter)
            {
                state.drained = contact.is_none();
            }
        }
        world.insert(e, beam);
    }
}

fn spray_contact(world: &World, center: Vec2, aim: Vec2) -> Option<Vec2> {
    let mut closest_iron: Option<Vec2> = None;
    let mut closest_surface: Option<Vec2> = None;
    for ray in -3..=3 {
        let direction = Vec2::from_angle(ray as f32 / 3.0 * SPRAY_HALF_ANGLE).rotate(aim);
        let (at, hit) = trace_beam(world, center, direction, ICE_BEAM_RANGE);
        if center.distance(at) >= ICE_BEAM_RANGE - 0.01 {
            continue;
        }
        if closest_surface.is_none_or(|p| center.distance_squared(at) < center.distance_squared(p))
        {
            closest_surface = Some(at);
        }
        if hit.is_some_and(|e| world.has::<IronBrick>(e) || world.has::<IronScrap>(e))
            && closest_iron.is_none_or(|p| center.distance_squared(at) < center.distance_squared(p))
        {
            closest_iron = Some(at);
        }
    }
    closest_iron.or(closest_surface)
}

fn orient_newborn_particles(world: &mut World) {
    let newborns: Vec<_> = world
        .query::<(Entity, &Particle, &Transform)>()
        .filter_map(|(e, particle, transform)| {
            if world.has::<OrientedIceParticle>(e) || particle.age > 0.0 {
                return None;
            }
            let aim = world.get::<IceSprayEmitter>(particle.emitter?)?.direction;
            let velocity = aim.rotate(particle.velocity);
            let speed = velocity.length();
            let direction = velocity.try_normalize()?;
            let (end, _) = trace_beam(world, transform.position, direction, ICE_BEAM_RANGE - 24.0);
            // Expire before a blocking surface; gravity and drag only shorten the travel.
            let lifetime = (transform.position.distance(end) - BEAM_HALF_WIDTH).max(0.0) / speed;
            Some((e, velocity, lifetime))
        })
        .collect();
    for (e, velocity, lifetime) in newborns {
        if let Some(particle) = world.get_mut::<Particle>(e) {
            particle.velocity = velocity;
            particle.lifetime = particle.lifetime.min(lifetime);
        }
        world.insert(e, OrientedIceParticle);
    }
}

/// Exact collider intersections; four-pixel samples stop the beam on tiles.
fn trace_beam(world: &World, start: Vec2, aim: Vec2, range: f32) -> (Vec2, Option<Entity>) {
    let mut distance = range;
    let mut hit = None;
    for (e, collider, position) in world.query::<(Entity, &Collider, &Position)>() {
        if world.has::<Player>(e) {
            continue;
        }
        if let Some(t) = ray_distance(start, aim, position.0 + collider.offset, collider.shape)
            && (t < distance || (t == distance && hit.is_none_or(|old: Entity| e.id() < old.id())))
        {
            distance = t;
            hit = Some(e);
        }
    }
    let normal = Vec2::new(-aim.y, aim.x) * BEAM_HALF_WIDTH;
    let steps = (distance / 4.0).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let t = distance * i as f32 / steps as f32;
        let at = start + aim * t;
        if [at, at + normal, at - normal]
            .into_iter()
            .any(|p| crate::fireball::solid_tile(world, p))
        {
            return (at, None);
        }
    }
    (start + aim * distance, hit)
}

fn ray_distance(start: Vec2, aim: Vec2, center: Vec2, shape: Shape) -> Option<f32> {
    let offset = start - center;
    match shape {
        Shape::Circle { radius } => {
            let radius = radius + BEAM_HALF_WIDTH;
            let c = offset.length_squared() - radius * radius;
            if c <= 0.0 {
                return Some(0.0);
            }
            let b = offset.dot(aim);
            let discriminant = b * b - c;
            if discriminant < 0.0 {
                return None;
            }
            let t = -b - discriminant.sqrt();
            (t >= 0.0).then_some(t)
        }
        Shape::Aabb { half_extents } => {
            let half = half_extents + Vec2::splat(BEAM_HALF_WIDTH);
            let mut near: f32 = 0.0;
            let mut far: f32 = ICE_BEAM_RANGE;
            for axis in 0..2 {
                if aim[axis].abs() < 1e-6 {
                    if offset[axis].abs() > half[axis] {
                        return None;
                    }
                } else {
                    let a = (-half[axis] - offset[axis]) / aim[axis];
                    let b = (half[axis] - offset[axis]) / aim[axis];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                }
            }
            (near <= far).then_some(near)
        }
    }
}
