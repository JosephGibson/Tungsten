//! Combat's systems: the states the step's resolve begins with, the
//! reactions to the parts that landed and to `shattered`, `frozen` and
//! `shocked`, the statuses' tints on their targets' sprites and their
//! particles.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    Collider, Entity, EventCursor, EventQueue, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Position, Sprite, Transform, World,
};
use tungsten::kit::{DamagedPart, EffectApplied, Health, Tags};

use super::components::{Reaction, StatusEmitter, StatusLook};
use super::{BURNING, CHILLED, FROZEN, Kind, SHOCKED};
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::{STEAM_REACH, TARGET_BLAST};
use crate::features::level::systems::drawn_time;
use crate::game::GAMEPLAY;

/// Which targets were frozen and which burning when the step's resolve
/// began: a World resource `combat_states` writes last before it, which
/// the step's reactions read, so an effect the step's own parts apply
/// never reacts to an earlier part (Q24).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveStates {
    pub frozen: Vec<Entity>,
    pub burning: Vec<Entity>,
}

/// `combat_states`, in the resolve phase before the kit's damage resolve:
/// records the targets carrying frozen and burning.
pub fn states(world: &mut World) {
    let mut states = ResolveStates::default();
    for (entity, _, tags) in world.query::<(Entity, &Health, &Tags)>() {
        if tags.has(FROZEN) {
            states.frozen.push(entity);
        }
        if tags.has(BURNING) {
            states.burning.push(entity);
        }
    }
    world.insert_resource(states);
}

/// A target's body centre.
fn centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// `combat_reactions`, in the react phase: a fire part landed on a target
/// that was frozen queues thermal shock's explosion at it, and a cold part
/// on one that was burning a steam burst, dead or alive; `shattered`
/// applied queues its shatter, and `frozen` and `shocked` applied send
/// their reactions for the hints. Each queued entry runs from the next
/// step.
pub fn reactions(
    world: &mut World,
    parts: &mut EventCursor<DamagedPart>,
    applied: &mut EventCursor<EffectApplied>,
) {
    let states = world
        .get_resource::<ResolveStates>()
        .cloned()
        .unwrap_or_default();
    let mut queued: Vec<(CascadeKind, Entity, f32)> = Vec::new();
    if let Some(events) = world.get_resource::<EventQueue<DamagedPart>>() {
        for part in parts.read(events) {
            if part.periodic {
                continue;
            }
            let fire = part.kinds.has(world, Kind::Fire.tag());
            let cold = part.kinds.has(world, Kind::Cold.tag());
            if fire && states.frozen.contains(&part.entity) {
                queued.push((CascadeKind::Explosion, part.entity, TARGET_BLAST));
            }
            if cold && states.burning.contains(&part.entity) {
                queued.push((CascadeKind::Steam, part.entity, STEAM_REACH));
            }
        }
    }
    let mut seen = Vec::new();
    if let Some(events) = world.get_resource::<EventQueue<EffectApplied>>() {
        for effect in applied.read(events) {
            match effect.id.as_deref() {
                Some("shattered") => queued.push((CascadeKind::Shatter, effect.entity, 0.0)),
                Some("frozen") => seen.push(Reaction::Freeze),
                Some("shocked") => seen.push(Reaction::Shock),
                _ => {}
            }
        }
    }
    for reaction in seen {
        super::react(world, reaction);
    }
    for (kind, target, strength) in queued {
        let Some(at) = centre(world, target) else {
            continue;
        };
        cascade::queue(world, kind, Some(target), at, at, strength);
    }
}

/// The tints a status gives its target's sprite, which multiply its
/// colours: frozen's cold, chilled's frost, and shocked's violet, which
/// flickers.
pub const COLD_TINT: [u8; 4] = [158, 219, 255, 255];
pub const FROST_TINT: [u8; 4] = [204, 237, 255, 255];
pub const SHOCK_TINT: [u8; 4] = [219, 199, 255, 255];
/// How fast shocked's tint and crackle flicker, frames a second.
pub const FLICKER_HZ: f32 = 12.0;

/// Shocked's frame at level time `time`, 0 or 1.
#[must_use]
pub fn flicker(time: f32) -> usize {
    (time * FLICKER_HZ) as usize % 2
}

/// The tint `tags` give a target at level time `time`: frozen over
/// shocked over chilled, else none.
#[must_use]
pub fn tint(tags: &Tags, time: f32) -> [u8; 4] {
    if tags.has(FROZEN) {
        COLD_TINT
    } else if tags.has(SHOCKED) && flicker(time) == 0 {
        SHOCK_TINT
    } else if tags.has(CHILLED) {
        FROST_TINT
    } else {
        [255; 4]
    }
}

/// `status_tints`, in the present phase: each target with health takes its
/// statuses' tint.
pub fn tints(world: &mut World) {
    let time = drawn_time(world);
    for (_, tags, sprite) in world.query_mut::<(&Health, &Tags, &mut Sprite)>() {
        sprite.color = tint(tags, time);
    }
}

/// `status_particles`, before the frame's particles: a live target that
/// carries chilled, bleeding or shocked has that status's emitter at its
/// body's centre; one that lost it, died or left loses the emitter.
pub fn particles(world: &mut World) {
    let mut wanted: Vec<(Entity, StatusLook, Vec2)> = Vec::new();
    for (entity, health, tags, at) in world.query::<(Entity, &Health, &Tags, &Transform)>() {
        if health.is_dead() {
            continue;
        }
        let offset = world
            .get::<Collider>(entity)
            .map_or(Vec2::splat(32.0), |collider| collider.offset);
        for look in StatusLook::ALL {
            if tags.has(look.tag()) {
                wanted.push((entity, look, at.position + offset));
            }
        }
    }
    let live: Vec<(Entity, StatusEmitter)> = world
        .query::<(Entity, &StatusEmitter)>()
        .map(|(entity, emitter)| (entity, *emitter))
        .collect();
    for &(entity, emitter) in &live {
        let kept = wanted
            .iter()
            .any(|&(owner, look, _)| owner == emitter.owner && look == emitter.look);
        if !kept {
            world.despawn(entity);
        }
    }
    for (owner, look, centre) in wanted {
        let existing = live
            .iter()
            .find(|(_, emitter)| emitter.owner == owner && emitter.look == look)
            .map(|&(entity, _)| entity);
        let emitter = match existing {
            Some(entity) => entity,
            None => {
                let Some(config) = world
                    .get_resource::<ParticleConfigRegistry>()
                    .and_then(|registry| registry.id_for_name(look.config()))
                else {
                    continue;
                };
                let seed = 0x5a_0000 + u64::from(owner.id()) * 4 + look as u64;
                world.spawn_with((
                    StatusEmitter { owner, look },
                    ParticleEmitter::with_seed(config, seed),
                    ParticleEmitterState::default(),
                    Transform::from_position(centre),
                    SceneEntity { state_id: GAMEPLAY },
                ))
            }
        };
        if let Some(transform) = world.get_mut::<Transform>(emitter) {
            transform.position = centre;
        }
    }
}
