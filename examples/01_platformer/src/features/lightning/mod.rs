//! Lightning, slot 4: a bolt from the sky onto the aim point, or onto the
//! nearest conductive thing within reach of it, the rod; it stops at the
//! first solid on its way down, so under a ceiling it strikes the ceiling.
//! Its shock hurts the targets where it lands, the caster spared, and it
//! lights nothing. Then conduction, one chain a strike, decided a jump at a
//! time through the saved cascade queue: from the rod, or from the struck
//! conductive thing nearest the strike, to the nearest conductive thing not
//! yet in the chain, each jump weaker, at most six, and none past a frozen
//! receiver. Conductive things are iron, its scraps and the targets tagged
//! `conductive`.

pub mod components;
pub mod draw;
pub mod systems;

use glam::{Vec2, Vec3};
use tungsten::core::{
    BodyKind, Collider, Entity, EventQueue, Light, Position, RigidBody, Schedule, ShakeEvent,
    Shape, Stage, Transform, World, system,
};
use tungsten::kit::{Health, Phase, Tags, ensure_phases};
use tungsten::physics::{SpatialFilter, cast_ray};
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::audio::{self, components::Sound};
use crate::features::combat::{FROZEN, Hit, Kind, has_status};
use crate::features::elements::cascade::{self, Cascades, Entry, Jump};
use crate::features::elements::components::Frozen;
use crate::features::level::{WORLD_MAX, WORLD_MIN};
use crate::features::run::Run;
use crate::features::toys::components::{IronBlock, IronScrap};
use crate::game::GAMEPLAY;
use crate::shared::SaveId;
use components::{Arc, Bolt, StrikeLight};

/// A strike's shock and its reach from where it lands.
pub const STRIKE_SHOCK: f32 = 40.0;
pub const STRIKE_REACH: f32 = 40.0;
/// A jump's reach, the share of its shock each jump keeps, and the most
/// jumps a strike makes.
pub const JUMP_REACH: f32 = 256.0;
pub const JUMP_KEEP: f32 = 0.7;
pub const MAX_JUMPS: u32 = 6;
/// A bolt segment's size, and the sideways step each leaves at its
/// bottom, as the art draws them.
pub const SEGMENT: Vec2 = Vec2::new(24.0, 64.0);
pub const EXITS: [f32; 8] = [-8.0, -4.0, 0.0, 4.0, 8.0, -6.0, 6.0, 2.0];
/// How long a bolt and an arc show, the strike's light, its reach, its
/// colour and the seconds it fades over, and the strike's shake.
pub const BOLT_SECS: f32 = 0.15;
pub const STRIKE_LIGHT: f32 = 3.0;
pub const LIGHT_REACH: f32 = 320.0;
pub const LIGHT_COLOR: Vec3 = Vec3::new(0.82, 0.72, 1.0);
pub const LIGHT_SECS: f32 = 0.2;
pub const STRIKE_SHAKE: f32 = 0.5;

/// Lightning as a plugin: the presentation's ages in the present phase
/// and the bolts, arcs and veil drawn after the sprites.
pub struct LightningPlugin;

impl Plugin for LightningPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::bolts);
        ensure_phases(schedule);
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("lightning_age", systems::age)),
        );
    }
}

/// A body's or a target's centre.
fn centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// How far `at` lies from a body's surface: 0 inside it.
fn surface_distance(world: &World, at: Vec2, entity: Entity) -> Option<f32> {
    let centre = centre(world, entity)?;
    let offset = at - centre;
    Some(match world.get::<Collider>(entity)?.shape {
        Shape::Aabb { half_extents } => (offset.abs() - half_extents).max(Vec2::ZERO).length(),
        Shape::Circle { radius } => (offset.length() - radius).max(0.0),
    })
}

/// Whether `entity` conducts: iron, its scraps, or a live target tagged
/// `conductive`.
#[must_use]
pub fn conductive(world: &World, entity: Entity) -> bool {
    conducts(
        world.has::<IronBlock>(entity) || world.has::<IronScrap>(entity),
        world.get::<Health>(entity),
        world.get::<Tags>(entity),
    )
}

/// Whether a thing conducts: iron, or a live thing tagged `conductive`.
fn conducts(iron: bool, health: Option<&Health>, tags: Option<&Tags>) -> bool {
    iron || (health.is_some_and(|health| !health.is_dead())
        && tags.is_some_and(|tags| tags.has("conductive")))
}

/// Whether `entity` is frozen: a frozen body, or a target carrying frozen.
#[must_use]
pub fn frozen(world: &World, entity: Entity) -> bool {
    world.has::<Frozen>(entity) || has_status(world, entity, FROZEN)
}

/// The conductive things with save IDs, their centres, but `spared`.
fn conductors(world: &World, spared: Option<Entity>) -> Vec<(Entity, Vec2, u64)> {
    // From the query's columns, not a lookup each (P6).
    world
        .query::<(
            Entity,
            &SaveId,
            Option<&IronBlock>,
            Option<&IronScrap>,
            Option<&Health>,
            Option<&Tags>,
        )>()
        .filter(|&(entity, _, iron, scrap, health, tags)| {
            Some(entity) != spared && conducts(iron.is_some() || scrap.is_some(), health, tags)
        })
        .filter_map(|(entity, id, ..)| Some((entity, centre(world, entity)?, id.0)))
        .collect()
}

/// The conductive thing nearest `at` within `reach`, centre to centre, not
/// one of `members` by save ID nor `spared`; ties to the lower save ID.
#[must_use]
pub fn nearest_conductor(
    world: &World,
    at: Vec2,
    reach: f32,
    members: &[u64],
    spared: Option<Entity>,
) -> Option<(Entity, Vec2)> {
    conductors(world, spared)
        .into_iter()
        .filter(|(_, centre, id)| centre.distance(at) <= reach && !members.contains(id))
        .min_by(|a, b| {
            a.1.distance(at)
                .total_cmp(&b.1.distance(at))
                .then(a.2.cmp(&b.2))
        })
        .map(|(entity, centre, _)| (entity, centre))
}

/// Where a bolt falling from the top of the level at `point.x` stops: the
/// first solid tile or body its line meets, `caster`'s body aside, or the
/// top of the first static collider it crosses, else the level's foot. The
/// aim point's height does not stop it.
#[must_use]
pub fn bolt_end(world: &World, point: Vec2, caster: Option<Entity>) -> Vec2 {
    let top = Vec2::new(point.x, WORLD_MIN.y);
    let mut stop = WORLD_MAX.y - top.y;
    let filter = caster.map_or_else(SpatialFilter::new, |caster| {
        SpatialFilter::new().excluding(caster)
    });
    if let Some(hit) = cast_ray(world, top, Vec2::Y, stop, &filter) {
        stop = stop.min(hit.distance);
    }
    for (at, collider, body) in world.query::<(&Position, &Collider, &RigidBody)>() {
        if body.kind != BodyKind::Static {
            continue;
        }
        let Shape::Aabb { half_extents } = collider.shape else {
            continue;
        };
        let centre = at.0 + collider.offset;
        let face = centre.y - half_extents.y - top.y;
        if (point.x - centre.x).abs() <= half_extents.x && (0.0..stop).contains(&face) {
            stop = face;
        }
    }
    top + Vec2::Y * stop
}

/// Strikes from `caster` at the aim point `aim`: the bolt falls at the
/// aim's x from the sky until it meets a tile or a body; its shock hurts
/// the targets within [`STRIKE_REACH`] of where it lands, `caster` spared;
/// and one chain of conduction starts, its first jump due the step after.
/// Where it landed.
pub fn strike(world: &mut World, caster: Entity, aim: Vec2) -> Vec2 {
    let landing = bolt_end(world, aim, Some(caster));
    let mut struck: Vec<(Option<SaveId>, Entity)> = world
        .query::<(Entity, &Collider)>()
        .filter(|&(entity, _)| entity != caster)
        .filter(|&(entity, _)| {
            world.get::<Health>(entity).is_some_and(|h| !h.is_dead()) || conductive(world, entity)
        })
        .filter(|&(entity, _)| {
            surface_distance(world, landing, entity).is_some_and(|gap| gap <= STRIKE_REACH)
        })
        .map(|(entity, _)| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    struck.sort_by_key(|(id, _)| *id);
    for &(_, target) in &struck {
        if world.get::<Health>(target).is_some_and(|h| !h.is_dead()) {
            Hit::new(target, [(Kind::Shock, STRIKE_SHOCK)])
                .from(caster)
                .at(landing)
                .queue(world);
        }
    }
    let number = world.get_resource_mut::<Cascades>().map_or(0, |cascades| {
        cascades.strike += 1;
        cascades.strike
    });
    let conductors: Vec<(Entity, u64)> = struck
        .iter()
        .filter(|&&(_, entity)| conductive(world, entity))
        .filter_map(|&(id, entity)| Some((entity, id?.0)))
        .collect();
    // The chain starts at the struck conductor nearest where the bolt
    // landed.
    let start = conductors
        .iter()
        .filter_map(|&(entity, id)| Some((entity, centre(world, entity)?, id)))
        .min_by(|a, b| {
            a.1.distance(landing)
                .total_cmp(&b.1.distance(landing))
                .then(a.2.cmp(&b.2))
        })
        .map(|(entity, ..)| entity);
    if let Some(start) = start.filter(|&start| !frozen(world, start)) {
        let members: Vec<u64> = conductors.iter().map(|&(_, id)| id).collect();
        let from = centre(world, start).unwrap_or(landing);
        if let Some((next, _)) = nearest_conductor(world, from, JUMP_REACH, &members, Some(caster))
        {
            cascade::queue_jump(
                world,
                Jump {
                    receiver: next,
                    from,
                    strength: STRIKE_SHOCK * JUMP_KEEP,
                    strike: number,
                    jumps: 1,
                    chain: members,
                },
                2,
            );
        }
    }
    show(world, landing, number);
    landing
}

/// A strike's look at `landing`: its bolt from the sky with its veil, its
/// light, the shake and the thunder. The strike's number picks the bolt's
/// five to eight pieces.
fn show(world: &mut World, landing: Vec2, number: u64) {
    let count = 5 + (number % 4) as usize;
    let pieces: Vec<usize> = (0..count)
        .map(|k| ((number as usize).wrapping_mul(7) + k * 3) % EXITS.len())
        .collect();
    world.spawn_with((
        Bolt {
            landing,
            pieces,
            age: 0.0,
            frames: 0,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
    let mut light = Light::point(LIGHT_COLOR, LIGHT_REACH);
    light.intensity = STRIKE_LIGHT;
    world.spawn_with((
        Transform::from_position(landing),
        light,
        StrikeLight { age: 0.0 },
        SceneEntity { state_id: GAMEPLAY },
    ));
    if let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>() {
        queue.send(ShakeEvent {
            trauma_add: STRIKE_SHAKE,
        });
    }
    audio::play(world, Sound::Thundered, landing);
}

/// A jump's run, from its cascade entry: its arc from `from` to `at` and a
/// crackle; its strength in shock to its receiver when it lives; then,
/// unless the receiver is frozen on arrival, conducts no more or the chain
/// has made [`MAX_JUMPS`], the next jump queued from it, due the next step.
pub fn conduct(
    world: &mut World,
    receiver: Option<Entity>,
    at: Vec2,
    from: Vec2,
    source: Option<Entity>,
    entry: &Entry,
) {
    let strike = entry.strike.unwrap_or(0);
    world.spawn_with((
        Arc {
            from,
            to: at,
            seed: strike * 8 + u64::from(entry.jumps),
            age: 0.0,
            frames: 0,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
    audio::play(world, Sound::Crackled, at);
    let Some(receiver) = receiver else {
        return;
    };
    if world.get::<Health>(receiver).is_some_and(|h| !h.is_dead()) {
        let mut hit = Hit::new(receiver, [(Kind::Shock, entry.strength)]).at(at);
        if let Some(source) = source {
            hit = hit.from(source);
        }
        hit.queue(world);
    }
    if frozen(world, receiver) || !conductive(world, receiver) || entry.jumps >= MAX_JUMPS {
        return;
    }
    let mut chain = entry.chain.clone();
    if let Some(id) = world.get::<SaveId>(receiver) {
        chain.push(id.0);
    }
    let player = world.get_resource::<Run>().map(|run| run.player);
    if let Some((next, _)) = nearest_conductor(world, at, JUMP_REACH, &chain, player) {
        cascade::queue_jump(
            world,
            Jump {
                receiver: next,
                from: at,
                strength: entry.strength * JUMP_KEEP,
                strike,
                jumps: entry.jumps + 1,
                chain,
            },
            1,
        );
    }
}
