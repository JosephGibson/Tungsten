//! Example-local fire propagation; no physics changes or per-ball lights.
use std::collections::{HashMap, HashSet};

use glam::Vec2;
use tungsten::core::{
    Entity, EventQueue, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Time,
    Transform, With, World,
};
use tungsten::physics::{CollisionEvent, Position};

use crate::state::{BALL_RADIUS, SMALL_BALL_SCALE, SmallBall};

pub(crate) const BALL_BURN_SECONDS: f32 = 10.0;
/// 64 sampled burning balls, each with a flame plume and a spark emitter.
pub(crate) const BALL_FIRE_EMITTER_CAP: usize = 128;
// Allow the solver's small resting separation without igniting distant balls.
const CONTACT_DISTANCE: f32 = 2.0 * BALL_RADIUS * SMALL_BALL_SCALE + 0.5;

/// Retained at zero after burnout: a spent ball cannot ignite again.
#[derive(Clone, Copy)]
pub(crate) struct BallBurn {
    pub(crate) remaining: f32,
}

pub(crate) struct BallFireEmitter;

pub(crate) fn ignite(world: &mut World, entity: Entity) {
    if world.get::<SmallBall>(entity).is_some() && world.get::<BallBurn>(entity).is_none() {
        world.insert(
            entity,
            BallBurn {
                remaining: BALL_BURN_SECONDS,
            },
        );
    }
}

pub(crate) fn tick_ball_fire(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    for entity in world
        .query_filtered::<Entity, With<BallBurn>>()
        .collect::<Vec<_>>()
    {
        let burn = world.get_mut::<BallBurn>(entity).unwrap();
        burn.remaining = (burn.remaining - dt).max(0.0);
    }
}

fn cell(position: Vec2) -> (i32, i32) {
    (
        (position.x / CONTACT_DISTANCE).floor() as i32,
        (position.y / CONTACT_DISTANCE).floor() as i32,
    )
}

/// Use current contacts for moving impacts and local proximity for sleeping
/// piles, which no longer emit physics contacts. Sources are frozen per pass.
pub(crate) fn spread_ball_fire(world: &mut World) {
    let sources: HashSet<_> = world
        .query::<(Entity, &BallBurn)>()
        .filter(|(_, b)| b.remaining > 0.0)
        .map(|(e, _)| e)
        .collect();
    if sources.is_empty() {
        return;
    }
    let mut targets = HashSet::new();
    if let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() {
        for event in events.iter_current() {
            let Some(b) = event.b else { continue };
            if sources.contains(&event.a) {
                targets.insert(b);
            }
            if sources.contains(&b) {
                targets.insert(event.a);
            }
        }
    }
    let mut cells: HashMap<(i32, i32), Vec<(Entity, Vec2)>> = HashMap::new();
    for (e, _) in world.query::<(Entity, &SmallBall)>() {
        if world.get::<BallBurn>(e).is_some() {
            continue;
        }
        if let Some(p) = world.get::<Position>(e) {
            cells.entry(cell(p.0)).or_default().push((e, p.0));
        }
    }
    for source in &sources {
        let Some(position) = world.get::<Position>(*source) else {
            continue;
        };
        let (x, y) = cell(position.0);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(neighbors) = cells.get(&(x + dx, y + dy)) {
                    for &(e, p) in neighbors {
                        if p.distance_squared(position.0) <= CONTACT_DISTANCE * CONTACT_DISTANCE {
                            targets.insert(e);
                        }
                    }
                }
            }
        }
    }
    // Stable insertion order, independent of randomized HashMap iteration.
    let mut targets: Vec<_> = targets.into_iter().collect();
    targets.sort_by_key(|e| e.id());
    for entity in targets {
        ignite(world, entity);
    }
}

/// Rotate a small pool across the burning population. Every ball has a flame
/// sprite; only this bounded pool produces additional drifting particles.
pub(crate) fn ball_fire_particles(world: &mut World) {
    let burning: Vec<_> = world
        .query::<(Entity, &BallBurn)>()
        .filter(|(_, burn)| burn.remaining > 0.0)
        .filter_map(|(e, _)| world.get::<Position>(e).map(|p| p.0))
        .collect();
    let source_cap = BALL_FIRE_EMITTER_CAP / 2;
    let count = burning.len().min(source_cap) * 2;
    let mut emitters = world
        .query_filtered::<Entity, With<BallFireEmitter>>()
        .collect::<Vec<_>>();
    for entity in emitters.drain(count.min(emitters.len())..) {
        world.despawn(entity);
    }
    if count == 0 {
        return;
    }
    let Some(config) = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|r| r.id_for_name("ex10_ball_burn"))
    else {
        return;
    };
    let Some(embers) = world
        .get_resource::<ParticleConfigRegistry>()
        .and_then(|r| r.id_for_name("ex10_ball_burn_embers"))
    else {
        return;
    };
    while emitters.len() < count {
        let e = world.spawn();
        world.insert(e, BallFireEmitter);
        world.insert(
            e,
            ParticleEmitter::with_seed(
                if emitters.len().is_multiple_of(2) {
                    config
                } else {
                    embers
                },
                0xb0_0000 + e.id() as u64,
            ),
        );
        world.insert(e, ParticleEmitterState::default());
        world.insert(e, Transform::default());
        emitters.push(e);
    }
    let time = world
        .get_resource::<crate::gameplay::SceneTime>()
        .map_or(0.0, |t| t.0);
    let offset = (time * 8.0) as usize * source_cap;
    for (i, entity) in emitters.into_iter().enumerate() {
        world.get_mut::<Transform>(entity).unwrap().position =
            burning[(offset + i / 2) % burning.len()] - Vec2::new(0.0, 5.0);
    }
}
