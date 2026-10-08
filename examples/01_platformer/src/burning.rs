//! Example-local fire propagation; no physics changes or per-ball lights.
//!
//! A burning pile reads as glowing coals under a flickering crest. Every
//! burning ball takes a pulsing ember tint; only balls with nothing resting
//! on them (`BallBurn::exposed`) draw a flame tongue and a glow and feed the
//! particle pool, since flames drawn inside a pile only stack into one
//! opaque sheet. Each ball takes its flame frame, rate, mirror, tint and
//! pulse from a hash of its id, so neighbours never flicker in step.
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use tungsten::core::{
    Entity, EventQueue, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Time,
    Transform, With, World,
};
use tungsten::physics::{CollisionEvent, Position};

use crate::state::{BALL_RADIUS, Ball, SMALL_BALL_SCALE, SmallBall, hash_unit};

pub(crate) const BALL_BURN_SECONDS: f32 = 10.0;
/// 64 sampled surface balls, each with a flame plume and a spark emitter.
pub(crate) const BALL_FIRE_EMITTER_CAP: usize = 128;
/// Seconds a pool emitter pair stays on one ball; the pairs hop in turn.
pub(crate) const EMITTER_DWELL: f32 = 0.5;
// Allow the solver's small resting separation without igniting distant balls.
const CONTACT_DISTANCE: f32 = 2.0 * BALL_RADIUS * SMALL_BALL_SCALE + 0.5;
const SMALL_RADIUS: f32 = BALL_RADIUS * SMALL_BALL_SCALE;
/// How far above a burning ball another ball's centre may sit and still
/// rest on it: a large ball on a small one, plus slack.
const COVER_REACH: f32 = BALL_RADIUS + SMALL_RADIUS + 1.0;
/// Seconds a new fire takes to grow from `KINDLE_FROM` to its full flame.
const KINDLE_SECS: f32 = 0.2;
const KINDLE_FROM: f32 = 0.4;
/// The last seconds of a burn, over which the flame fades and the ember
/// tint darkens to char.
const DYING_SECS: f32 = 2.0;
const CHAR: Vec3 = Vec3::new(55.0, 48.0, 45.0);
// Tints multiply the marbles' grey glass in linear light, so these low green
// and blue channels come out as a dark red to orange coal bed.
const EMBER_DIM: Vec3 = Vec3::new(120.0, 8.0, 3.0);
const EMBER_HOT: Vec3 = Vec3::new(255.0, 64.0, 10.0);
/// Burning-ball flame frames. The art is drawn in 2×2 pixel blocks, so a
/// `FLAME_SIZE` quad samples it exactly.
pub(crate) const FLAME_SPRITE_IDS: [&str; 8] = [
    "ex10_fire_0",
    "ex10_fire_1",
    "ex10_fire_2",
    "ex10_fire_3",
    "ex10_fire_4",
    "ex10_fire_5",
    "ex10_fire_6",
    "ex10_fire_7",
];
pub(crate) const FLAME_SIZE: f32 = 32.0;

/// Retained at zero after burnout: a spent ball cannot ignite again.
#[derive(Clone, Copy)]
pub(crate) struct BallBurn {
    pub(crate) remaining: f32,
    /// Nothing rests on this ball: it burns on the pile's surface
    /// (`flame_exposure`). A newly lit ball counts as exposed.
    pub(crate) exposed: bool,
}

pub(crate) struct BallFireEmitter;

pub(crate) fn ignite(world: &mut World, entity: Entity) {
    if world.get::<SmallBall>(entity).is_some() && world.get::<BallBurn>(entity).is_none() {
        world.insert(
            entity,
            BallBurn {
                remaining: BALL_BURN_SECONDS,
                exposed: true,
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

/// Marks which burning balls have nothing resting on them. A ball is
/// covered by a ball directly above it, or by one above on each side, as in
/// a pile's interior; a ball on a slope, with one neighbour above on the
/// uphill side, stays exposed.
pub(crate) fn flame_exposure(world: &mut World) {
    let burning: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &BallBurn, &Position)>()
        .filter(|(_, burn, _)| burn.remaining > 0.0)
        .map(|(e, _, p)| (e, p.0))
        .collect();
    let Some((min, max)) = burning
        .iter()
        .map(|&(_, p)| (p, p))
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
    else {
        return;
    };
    let (min, max) = (
        min - Vec2::splat(COVER_REACH),
        max + Vec2::splat(COVER_REACH),
    );
    let cell = |p: Vec2| {
        (
            (p.x / COVER_REACH).floor() as i32,
            (p.y / COVER_REACH).floor() as i32,
        )
    };
    let mut cells: HashMap<(i32, i32), Vec<Vec2>> = HashMap::new();
    for (_, _, p) in world.query::<(Entity, &Ball, &Position)>() {
        if p.0.cmpge(min).all() && p.0.cmple(max).all() {
            cells.entry(cell(p.0)).or_default().push(p.0);
        }
    }
    for (entity, p) in burning {
        let (x, y) = cell(p);
        let (mut left, mut right, mut over) = (false, false, false);
        for q in (y - 1..=y)
            .flat_map(|cy| (x - 1..=x + 1).map(move |cx| (cx, cy)))
            .filter_map(|key| cells.get(&key))
            .flatten()
        {
            let rise = p.y - q.y;
            let dx = q.x - p.x;
            if !(0.5 * SMALL_RADIUS..=COVER_REACH).contains(&rise) || dx.abs() > 1.3 * SMALL_RADIUS
            {
                continue;
            }
            over |= dx.abs() < 0.5 * SMALL_RADIUS;
            left |= dx < 0.0;
            right |= dx > 0.0;
        }
        world.get_mut::<BallBurn>(entity).unwrap().exposed = !(over || (left && right));
    }
}

/// Four values in [0, 1] hashed from `entity`'s id: a ball's own phase,
/// rate, mirror and tint.
fn variation(entity: Entity) -> [f32; 4] {
    let id = entity.id();
    [
        hash_unit(id),
        hash_unit(id ^ 0x68e3_1da4),
        hash_unit(id.wrapping_mul(0x9e37_79b9)),
        hash_unit(id ^ 0x2c1b_3c6d),
    ]
}

/// How strongly a fire burns, in [0, 1]: it grows over `KINDLE_SECS` and
/// fades over the last `DYING_SECS`.
fn strength(burn: BallBurn) -> f32 {
    let age = BALL_BURN_SECONDS - burn.remaining;
    let kindle = KINDLE_FROM + (1.0 - KINDLE_FROM) * (age / KINDLE_SECS).min(1.0);
    kindle * (burn.remaining / DYING_SECS).min(1.0)
}

fn rgba(color: Vec3, alpha: f32) -> [u8; 4] {
    let c = color.clamp(Vec3::ZERO, Vec3::splat(255.0));
    [
        c.x as u8,
        c.y as u8,
        c.z as u8,
        (alpha.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

/// A burning or spent ball's tint at scene time `time`: embers pulsing
/// between deep red and orange at the ball's own rate, darkening to char
/// over the last `DYING_SECS`.
pub(crate) fn ember_tint(entity: Entity, burn: BallBurn, time: f32) -> [u8; 4] {
    if burn.remaining <= 0.0 {
        return rgba(CHAR, 1.0);
    }
    let [phase, rate, _, _] = variation(entity);
    let pulse = 0.5 + 0.5 * (time * (2.0 + 3.0 * rate) + phase * TAU).sin();
    let hot = EMBER_DIM.lerp(EMBER_HOT, pulse);
    rgba(CHAR.lerp(hot, (burn.remaining / DYING_SECS).min(1.0)), 1.0)
}

/// How one exposed burning ball's flame draws: the frame, a mirror, the
/// tongue's tint and the glow's size and tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlameLook {
    pub(crate) frame: usize,
    pub(crate) mirror: bool,
    pub(crate) color: [u8; 4],
    pub(crate) glow_size: f32,
    pub(crate) glow: [u8; 4],
}

/// The flame of `entity` at scene time `time`; `None` for a buried or spent ball.
pub(crate) fn flame_look(entity: Entity, burn: BallBurn, time: f32) -> Option<FlameLook> {
    if burn.remaining <= 0.0 || !burn.exposed {
        return None;
    }
    let [phase, rate, mirror, tint] = variation(entity);
    let fps = 10.0 + 8.0 * rate;
    let frame =
        (time * fps + phase * FLAME_SPRITE_IDS.len() as f32) as usize % FLAME_SPRITE_IDS.len();
    let strength = strength(burn);
    Some(FlameLook {
        frame,
        mirror: mirror < 0.5,
        color: rgba(
            Vec3::new(255.0, 222.0 + 33.0 * tint, 186.0 + 69.0 * tint),
            strength,
        ),
        glow_size: 28.0 + 12.0 * tint,
        glow: rgba(Vec3::new(255.0, 128.0 + 40.0 * tint, 52.0), 0.3 * strength),
    })
}

/// A bounded pool of emitters over the burning surface. Each pair (a flame
/// plume and sparks) stays on one exposed ball for `EMITTER_DWELL`; the
/// pairs hop in turn, a fraction of a dwell apart, so the pool drifts along
/// the crest rather than jumping all at once.
pub(crate) fn ball_fire_particles(world: &mut World) {
    let burning: Vec<(Vec2, bool)> = world
        .query::<(Entity, &BallBurn)>()
        .filter(|(_, burn)| burn.remaining > 0.0)
        .filter_map(|(e, burn)| Some((crate::extract::drawn_position(world, e)?, burn.exposed)))
        .collect();
    // A burning pile always has a surface; buried balls feed it only if not.
    let sources: Vec<Vec2> = if burning.iter().any(|&(_, exposed)| exposed) {
        burning.iter().filter(|b| b.1).map(|b| b.0).collect()
    } else {
        burning.iter().map(|b| b.0).collect()
    };
    let pairs = sources.len().min(BALL_FIRE_EMITTER_CAP / 2);
    let count = pairs * 2;
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
    let time = crate::extract::drawn_scene_time(world);
    for (i, entity) in emitters.into_iter().enumerate() {
        let slot = i / 2;
        let hop = (time / EMITTER_DWELL + slot as f32 / pairs as f32).floor() as usize;
        let source = sources[(slot * sources.len() / pairs + hop * 7919) % sources.len()];
        world.get_mut::<Transform>(entity).unwrap().position = source - Vec2::new(0.0, 6.0);
    }
}
