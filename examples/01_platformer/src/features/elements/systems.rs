//! The element rules' systems.

use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    Collider, CollisionEvent, Entity, EventQueue, Particle, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Position, PrevPosition, Shape, Time, Transform,
    Velocity, With, World,
};
use tungsten::kit::{DamageRequest, Hit};
use tungsten::physics::wake;

use super::components::{BallBurn, Chill, FireEmitter, Frozen, IcePulse};
use super::{
    BASE_SMASH, BURN_SECS, CRUSH_HIT, CRUSH_SPEED, DOUSE_BURSTS_PER_STEP, EMITTER_DWELL,
    FIRE_CENTRE, FIRE_EMITTER_CAP, FIRE_HALF, IMPACT_BURSTS_PER_STEP, IMPACT_COOLDOWN,
    IMPACT_SPEED, SHATTER_SPEED, SMASH_CAP, SMASH_KEEP, SPEED_PER_BALL, SPEED_PER_HIT, THAW_RATE,
    freeze_amount,
};
use crate::features::audio::{self, components::Sound};
use crate::features::level::components::Hazard;
use crate::features::level::systems::drawn_time;
use crate::features::player::components::Player;
use crate::features::toys::components::{
    Ball, BowlingBall, IronBlock, IronScrap, PreviousVelocity, ScrapTumble, SmallBall,
};
use crate::features::toys::systems::holes;
use crate::features::toys::{
    self, BALL_RADIUS, HOLE_RADIUS, IRON_HALF, SCRAP_COUNT, SCRAP_HALF, SMALL_BALL_RADIUS,
};
use crate::game::GAMEPLAY;
use crate::shared::{BURST_CAP, SaveId, despawn_tree, spawn_burst};

/// Two touching small balls' centres, with the solver's resting slack.
const CONTACT_DISTANCE: f32 = 2.0 * SMALL_BALL_RADIUS + 0.5;
/// How far above a burning ball another's centre may sit and still rest on
/// it: a bowling ball on a small one, with slack.
const COVER_REACH: f32 = BALL_RADIUS + SMALL_BALL_RADIUS + 1.0;
/// A new fire grows over these seconds from this part of its flame, and a
/// burn fades over its last seconds to char.
const KINDLE_SECS: f32 = 0.2;
const KINDLE_FROM: f32 = 0.4;
const DYING_SECS: f32 = 2.0;
const CHAR: Vec3 = Vec3::new(55.0, 48.0, 45.0);
// Tints multiply the marbles' grey glass in linear light, so these low
// green and blue channels come out as a dark red to orange coal bed.
const EMBER_DIM: Vec3 = Vec3::new(120.0, 8.0, 3.0);
const EMBER_HOT: Vec3 = Vec3::new(255.0, 64.0, 10.0);
/// Fracture throws the pieces out at 800 to 1,200 px/s, away from a
/// surface blast, with a little lift so a block on the floor still breaks
/// visibly.
const SCRAP_EJECT_SPEED: f32 = 1000.0;
const SCRAP_BLAST_BIAS: f32 = 600.0;
const SCRAP_EJECT_LIFT: f32 = 180.0;
/// Speculative contacts stop iron at a body's surface, not inside it.
const CONTACT_MARGIN: f32 = 3.0;

/// A body's centre: its `Position` plus its collider's offset.
fn centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// A body's centre before this step.
fn prev_centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<PrevPosition>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// Lights `ball` when it is a small ball that never burned.
pub fn ignite(world: &mut World, ball: Entity) {
    if world.has::<SmallBall>(ball) && !world.has::<BallBurn>(ball) {
        world.insert(
            ball,
            BallBurn {
                remaining: BURN_SECS,
                exposed: true,
            },
        );
    }
}

/// `fire_tick`, in the tick phase: each burn runs down by the step, kept at
/// 0 once spent.
pub fn tick_fire(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    for burn in world.query_mut::<&mut BallBurn>() {
        burn.remaining = (burn.remaining - dt).max(0.0);
    }
}

fn cell(at: Vec2, size: f32) -> (i32, i32) {
    ((at.x / size).floor() as i32, (at.y / size).floor() as i32)
}

/// `fire_spread`, in the overlap phase after the physics step: a burning
/// ball lights the small balls its step's contacts name and those resting
/// against it, which sleep and send no contacts. The burning set is the
/// step's, so fire spreads one hop a step.
pub fn spread_fire(world: &mut World) {
    let sources: HashSet<Entity> = world
        .query::<(Entity, &BallBurn)>()
        .filter(|(_, burn)| burn.remaining > 0.0)
        .map(|(entity, _)| entity)
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
    for (entity, _, at, collider) in world.query::<(Entity, &SmallBall, &Position, &Collider)>() {
        if world.has::<BallBurn>(entity) {
            continue;
        }
        let p = at.0 + collider.offset;
        cells
            .entry(cell(p, CONTACT_DISTANCE))
            .or_default()
            .push((entity, p));
    }
    for source in &sources {
        let Some(p) = centre(world, *source) else {
            continue;
        };
        let (x, y) = cell(p, CONTACT_DISTANCE);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &(entity, q) in cells.get(&(x + dx, y + dy)).into_iter().flatten() {
                    if q.distance_squared(p) <= CONTACT_DISTANCE * CONTACT_DISTANCE {
                        targets.insert(entity);
                    }
                }
            }
        }
    }
    let mut targets: Vec<(Option<SaveId>, Entity)> = targets
        .into_iter()
        .map(|entity| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    targets.sort_by_key(|(id, _)| *id);
    for (_, entity) in targets {
        ignite(world, entity);
    }
}

/// `fire_exposure`, in the overlap phase: which burning balls have nothing
/// resting on them. A ball is covered by one right above it, or by one
/// above on each side, as inside a pile; one on a slope with a neighbour
/// uphill stays exposed.
pub fn flame_exposure(world: &mut World) {
    let burning: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &BallBurn)>()
        .filter(|(_, burn)| burn.remaining > 0.0)
        .filter_map(|(entity, _)| Some((entity, centre(world, entity)?)))
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
    let mut cells: HashMap<(i32, i32), Vec<Vec2>> = HashMap::new();
    for (_, at, collider) in world.query::<(&Ball, &Position, &Collider)>() {
        let p = at.0 + collider.offset;
        if p.cmpge(min).all() && p.cmple(max).all() {
            cells.entry(cell(p, COVER_REACH)).or_default().push(p);
        }
    }
    for (entity, p) in burning {
        let (x, y) = cell(p, COVER_REACH);
        let (mut left, mut right, mut over) = (false, false, false);
        for q in (y - 1..=y)
            .flat_map(|cy| (x - 1..=x + 1).map(move |cx| (cx, cy)))
            .filter_map(|key| cells.get(&key))
            .flatten()
        {
            let rise = p.y - q.y;
            let dx = q.x - p.x;
            if !(0.5 * SMALL_BALL_RADIUS..=COVER_REACH).contains(&rise)
                || dx.abs() > 1.3 * SMALL_BALL_RADIUS
            {
                continue;
            }
            over |= dx.abs() < 0.5 * SMALL_BALL_RADIUS;
            left |= dx < 0.0;
            right |= dx > 0.0;
        }
        if let Some(burn) = world.get_mut::<BallBurn>(entity) {
            burn.exposed = !(over || (left && right));
        }
    }
}

/// Four values in `[0, 1]` hashed from a ball's save ID: its phase, rate,
/// mirror and tint.
fn variation(id: SaveId) -> [f32; 4] {
    let id = id.0 as u32 ^ (id.0 >> 32) as u32;
    let hash = toys::components::hash_unit;
    [
        hash(id),
        hash(id ^ 0x68e3_1da4),
        hash(id.wrapping_mul(0x9e37_79b9)),
        hash(id ^ 0x2c1b_3c6d),
    ]
}

/// How strongly a fire burns, 0 to 1: it grows over its kindling and fades
/// over its last seconds.
fn strength(burn: BallBurn) -> f32 {
    let age = BURN_SECS - burn.remaining;
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

/// A burning or spent ball's tint at level time `time`: embers pulsing
/// between deep red and orange at its own rate, darkening to char over its
/// last seconds.
#[must_use]
pub fn ember_tint(id: SaveId, burn: BallBurn, time: f32) -> [u8; 4] {
    if burn.remaining <= 0.0 {
        return rgba(CHAR, 1.0);
    }
    let [phase, rate, _, _] = variation(id);
    let pulse = 0.5 + 0.5 * (time * (2.0 + 3.0 * rate) + phase * TAU).sin();
    let hot = EMBER_DIM.lerp(EMBER_HOT, pulse);
    rgba(CHAR.lerp(hot, (burn.remaining / DYING_SECS).min(1.0)), 1.0)
}

/// How an exposed burning ball's flame draws: its frame, a mirror, the
/// tongue's tint and the glow's size and tint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlameLook {
    pub frame: usize,
    pub mirror: bool,
    pub color: [u8; 4],
    pub glow_size: f32,
    pub glow: [u8; 4],
}

/// The flame of the ball `id` at level time `time`; `None` for a buried or
/// spent one.
#[must_use]
pub fn flame_look(id: SaveId, burn: BallBurn, time: f32) -> Option<FlameLook> {
    if burn.remaining <= 0.0 || !burn.exposed {
        return None;
    }
    let [phase, rate, mirror, tint] = variation(id);
    let fps = 10.0 + 8.0 * rate;
    let frames = super::draw::FLAME_FRAMES.len();
    let frame = (time * fps + phase * frames as f32) as usize % frames;
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

/// `fire_particles`, in the present phase: a bounded pool of emitters over
/// the burning surface. Each pair, a flame plume and sparks, stays on one
/// exposed ball for [`EMITTER_DWELL`]; the pairs hop in turn, so the pool
/// drifts along the crest. A pile with no exposed ball feeds it from all.
pub fn fire_particles(world: &mut World) {
    let burning: Vec<(Vec2, bool)> = world
        .query::<(Entity, &BallBurn, &Transform, &Collider)>()
        .filter(|(_, burn, ..)| burn.remaining > 0.0)
        .map(|(_, burn, at, collider)| (at.position + collider.offset, burn.exposed))
        .collect();
    let sources: Vec<Vec2> = if burning.iter().any(|&(_, exposed)| exposed) {
        burning.iter().filter(|b| b.1).map(|b| b.0).collect()
    } else {
        burning.iter().map(|b| b.0).collect()
    };
    let pairs = sources.len().min(FIRE_EMITTER_CAP / 2);
    let count = pairs * 2;
    let mut emitters: Vec<Entity> = world
        .query_filtered::<Entity, With<FireEmitter>>()
        .collect();
    for entity in emitters.drain(count.min(emitters.len())..) {
        world.despawn(entity);
    }
    if count == 0 {
        return;
    }
    let registry = world.resource::<ParticleConfigRegistry>();
    let (Some(plume), Some(sparks)) = (
        registry.id_for_name("ex10_ball_burn"),
        registry.id_for_name("ex10_ball_burn_embers"),
    ) else {
        return;
    };
    while emitters.len() < count {
        let config = if emitters.len().is_multiple_of(2) {
            plume
        } else {
            sparks
        };
        let seed = 0xb0_0000 + emitters.len() as u64;
        emitters.push(world.spawn_with((
            FireEmitter,
            ParticleEmitter::with_seed(config, seed),
            ParticleEmitterState::default(),
            Transform::default(),
            SceneEntity { state_id: GAMEPLAY },
        )));
    }
    let time = drawn_time(world);
    for (i, entity) in emitters.into_iter().enumerate() {
        let slot = i / 2;
        let hop = (time / EMITTER_DWELL + slot as f32 / pairs as f32).floor() as usize;
        let source = sources[(slot * sources.len() / pairs + hop * 7919) % sources.len()];
        if let Some(at) = world.get_mut::<Transform>(entity) {
            at.position = source - Vec2::new(0.0, 6.0);
        }
    }
}

/// The first fraction of a step at which a box of `half` about a body
/// sweeping `start..end` relative to a hazard meets the hazard's centre,
/// in relative motion: the slab test.
fn swept_box(start: Vec2, end: Vec2, half: Vec2) -> Option<f32> {
    let (mut enter, mut exit) = (0.0_f32, 1.0_f32);
    for axis in 0..2 {
        let d = end[axis] - start[axis];
        if d.abs() < 1e-5 {
            if start[axis].abs() > half[axis] {
                return None;
            }
        } else {
            let a = (-half[axis] - start[axis]) / d;
            let b = (half[axis] - start[axis]) / d;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}

/// A fire hazard's touch-box centre now and before this step.
fn fire_centres(world: &World, fire: Entity) -> Option<(Vec2, Vec2)> {
    let end = world
        .get::<Position>(fire)
        .map(|p| p.0)
        .or_else(|| world.get::<Transform>(fire).map(|t| t.position))?;
    let start = world.get::<PrevPosition>(fire).map_or(end, |p| p.0);
    Some((start + FIRE_CENTRE, end + FIRE_CENTRE))
}

/// `fire_contacts`, in the overlap phase: a fireball hazard that meets a
/// ball over the step, either moving fast, lights a small ball and bursts
/// a bowling ball where they met, one burst per ball however many fires
/// overlap. The player's touch is the kit's contact damage.
pub fn fire_contacts(world: &mut World) {
    let fires: Vec<(Vec2, Vec2)> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(_, hazard)| hazard.fire)
        .filter_map(|(entity, _)| fire_centres(world, entity))
        .collect();
    if fires.is_empty() {
        return;
    }
    let mut touched: Vec<(Option<SaveId>, Entity, Vec2)> = world
        .query::<(Entity, &Ball, &Position, &Collider)>()
        .filter_map(|(ball, _, at, collider)| {
            let radius = match collider.shape {
                Shape::Circle { radius } => radius,
                Shape::Aabb { half_extents } => half_extents.x,
            };
            let end = at.0 + collider.offset;
            let start = prev_centre(world, ball).unwrap_or(end);
            let hit = fires
                .iter()
                .filter_map(|&(fire_start, fire_end)| {
                    swept_box(
                        start - fire_start,
                        end - fire_end,
                        FIRE_HALF + Vec2::splat(radius),
                    )
                })
                .min_by(f32::total_cmp)?;
            Some((
                world.get::<SaveId>(ball).copied(),
                ball,
                start.lerp(end, hit),
            ))
        })
        .collect();
    touched.sort_by_key(|(id, ..)| *id);
    for (_, ball, at) in touched {
        if world.has::<SmallBall>(ball) {
            ignite(world, ball);
            continue;
        }
        if world.has::<BowlingBall>(ball) {
            despawn_tree(world, ball);
            spawn_burst(world, "ex10_ball_explosion", at);
            toys::ball_ring(world, at);
        }
    }
}

/// `ball_impacts`, in the overlap phase: a small ball that met something
/// faster than [`IMPACT_SPEED`] along the contact, its speed from before
/// the solver stopped it, bursts its sparks, then waits out its cooldown;
/// at most [`IMPACT_BURSTS_PER_STEP`] a step, the rest dropped.
pub fn ball_impacts(world: &mut World) {
    let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() else {
        return;
    };
    let mut hits: Vec<Entity> = Vec::with_capacity(IMPACT_BURSTS_PER_STEP);
    'events: for event in events.iter_current() {
        let va = world
            .get::<PreviousVelocity>(event.a)
            .map_or(Vec2::ZERO, |v| v.0);
        let vb = event
            .b
            .and_then(|b| world.get::<PreviousVelocity>(b))
            .map_or(Vec2::ZERO, |v| v.0);
        if -(va - vb).dot(event.normal) <= IMPACT_SPEED {
            continue;
        }
        for entity in [Some(event.a), event.b].into_iter().flatten() {
            let ready = world
                .get::<SmallBall>(entity)
                .is_some_and(|ball| ball.impact_cooldown <= 0.0);
            if ready && !hits.contains(&entity) {
                hits.push(entity);
                if hits.len() == IMPACT_BURSTS_PER_STEP {
                    break 'events;
                }
            }
        }
    }
    for ball in hits {
        if let Some(state) = world.get_mut::<SmallBall>(ball) {
            state.impact_cooldown = IMPACT_COOLDOWN;
        }
        if let Some(at) = centre(world, ball) {
            spawn_burst(world, "ex10_small_ball_impact", at);
        }
    }
}

/// `impact_sparks`, in the present phase: newborn impact sparks take the
/// hue of their flight's direction, a ring of rainbow.
pub fn impact_sparks(world: &mut World) {
    let Some(config) = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name("ex10_small_ball_impact")
    else {
        return;
    };
    let newborn: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &Particle)>()
        .filter_map(|(entity, particle)| {
            let emitter = world.get::<ParticleEmitter>(particle.emitter?)?;
            (particle.age == 0.0 && emitter.config == config).then_some((entity, particle.velocity))
        })
        .collect();
    for (entity, velocity) in newborn {
        let hue = (velocity.y.atan2(velocity.x) / TAU).rem_euclid(1.0);
        let [r, g, b, _] = toys::systems::rainbow_rgba(hue).map(|c| f32::from(c) / 255.0);
        if let Some(particle) = world.get_mut::<Particle>(entity) {
            particle.base_rgba = [r, g, b, 1.0];
        }
    }
}

/// How fast iron at `centre` moving at `velocity` drives into a body at
/// `target` of half size `half`: its speed along the face they meet on,
/// when they touch and it is at least [`CRUSH_SPEED`].
fn impact(centre: Vec2, velocity: Vec2, target: Vec2, half: Vec2) -> Option<f32> {
    let offset = target - centre;
    let gap = offset.abs() - (IRON_HALF + half);
    if gap.max_element() > CONTACT_MARGIN {
        return None;
    }
    // They meet on the axis with the larger gap, the smaller overlap.
    let normal = if gap.x > gap.y {
        Vec2::new(offset.x.signum(), 0.0)
    } else {
        Vec2::new(0.0, offset.y.signum())
    };
    let speed = velocity.dot(normal);
    (speed >= CRUSH_SPEED).then_some(speed)
}

/// How many steps of `per` the speed is past [`CRUSH_SPEED`], plus one.
fn severity(speed: f32, per: f32) -> usize {
    1 + ((speed - CRUSH_SPEED) / per) as usize
}

fn is_iron(world: &World, entity: Entity) -> bool {
    world.has::<IronBlock>(entity) || world.has::<IronScrap>(entity)
}

/// `iron_crush`, in the overlap phase after the physics step: a moving
/// block crushes the small balls it drives into, more the faster it goes,
/// hurts the player, and shatters frozen iron on a hard head-on impact. It
/// reads its velocity from before the step, since the solver has stopped
/// it on what it met; one that smashed something keeps [`SMASH_KEEP`] of
/// it, so it plows on into the next layer.
pub fn crush(world: &mut World) {
    let mut blocks: Vec<(SaveId, Entity, Vec2, Vec2)> = world
        .query::<(
            Entity,
            &IronBlock,
            &Position,
            &Collider,
            &PreviousVelocity,
            &SaveId,
        )>()
        .filter(|(.., velocity, _)| velocity.0.length() >= CRUSH_SPEED)
        .map(|(entity, _, at, collider, velocity, id)| {
            (*id, entity, at.0 + collider.offset, velocity.0)
        })
        .collect();
    if blocks.is_empty() {
        return;
    }
    blocks.sort_by_key(|(id, ..)| *id);
    // The solver may already have pushed a body off the block's face: keep
    // those contacts, either way round.
    let mut contacts = HashMap::<(Entity, Entity), f32>::new();
    if let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() {
        for event in events.iter_current() {
            let Some(b) = event.b else { continue };
            for (block, target, normal) in [(event.a, b, -event.normal), (b, event.a, event.normal)]
            {
                if !world.has::<IronBlock>(block) {
                    continue;
                }
                let frozen = world.has::<Frozen>(target) && is_iron(world, target);
                if !world.has::<SmallBall>(target) && !world.has::<Player>(target) && !frozen {
                    continue;
                }
                let velocity = world
                    .get::<PreviousVelocity>(block)
                    .map_or(Vec2::ZERO, |v| v.0);
                let mut speed = velocity.dot(normal);
                if frozen {
                    let other = world
                        .get::<PreviousVelocity>(target)
                        .map_or(Vec2::ZERO, |v| v.0);
                    speed = speed.min((velocity - other).dot(normal));
                }
                if speed >= CRUSH_SPEED {
                    let best = contacts.entry((block, target)).or_insert(speed);
                    *best = best.max(speed);
                }
            }
        }
    }
    let small_half = Vec2::splat(SMALL_BALL_RADIUS);
    let mut smashed: Vec<Entity> = Vec::new();
    let mut shattered: HashSet<Entity> = HashSet::new();
    let mut hurt = false;
    for &(_, block, at, velocity) in &blocks {
        let speed_into = |entity: Entity, target: Vec2, half: Vec2| {
            contacts
                .get(&(block, entity))
                .copied()
                .or_else(|| impact(at, velocity, target, half))
        };
        let mut struck: Vec<(Entity, f32, Option<SaveId>)> = world
            .query::<(Entity, &SmallBall, &Position, &Collider)>()
            .filter(|(entity, ..)| !smashed.contains(entity))
            .filter_map(|(entity, _, p, c)| {
                let speed = speed_into(entity, p.0 + c.offset, small_half)?;
                Some((entity, speed, world.get::<SaveId>(entity).copied()))
            })
            .collect();
        // Most head-on first; the save ID breaks ties, so a pile breaks the
        // same way each run.
        struck.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.2.cmp(&b.2)));
        if let Some(&(_, speed, _)) = struck.first() {
            let count = BASE_SMASH + severity(speed, SPEED_PER_BALL) - 1;
            struck.truncate(count.min(SMASH_CAP));
        }
        let mut keep = !struck.is_empty();
        smashed.extend(struck.into_iter().map(|(entity, ..)| entity));
        let frozen_hits: Vec<Entity> = world
            .query::<(Entity, &Frozen, &Position, &Collider)>()
            .filter(|(entity, ..)| *entity != block && is_iron(world, *entity))
            .filter_map(|(entity, _, p, c)| {
                let half = if world.has::<IronBlock>(entity) {
                    IRON_HALF
                } else {
                    SCRAP_HALF
                };
                let target = p.0 + c.offset;
                let speed = contacts.get(&(block, entity)).copied().or_else(|| {
                    let other = world
                        .get::<PreviousVelocity>(entity)
                        .map_or(Vec2::ZERO, |v| v.0);
                    Some(impact(at, velocity, target, half)?.min(impact(
                        at,
                        velocity - other,
                        target,
                        half,
                    )?))
                })?;
                (speed >= SHATTER_SPEED).then_some(entity)
            })
            .collect();
        keep |= !frozen_hits.is_empty();
        shattered.extend(frozen_hits);
        if keep && let Some(current) = world.get_mut::<Velocity>(block) {
            current.0 = velocity * SMASH_KEEP;
        }
        let players: Vec<(Entity, f32, Vec2)> = world
            .query::<(Entity, &Player, &Position, &Collider)>()
            .filter_map(|(entity, _, p, c)| {
                let half = match c.shape {
                    Shape::Aabb { half_extents } => half_extents,
                    Shape::Circle { radius } => Vec2::splat(radius),
                };
                let target = p.0 + c.offset;
                Some((entity, speed_into(entity, target, half)?, target))
            })
            .collect();
        for (player, speed, target) in players {
            let hits = severity(speed, SPEED_PER_HIT) as f32;
            let push = (target - at).normalize_or_zero() * speed.min(600.0);
            let hit = DamageRequest::new(player, CRUSH_HIT * hits)
                .with_position(at)
                .with_knockback(push)
                .apply(world);
            hurt |= matches!(hit, Hit::Hurt | Hit::Killed);
        }
    }
    let mut shattered: Vec<(Option<SaveId>, Entity)> = shattered
        .into_iter()
        .map(|entity| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    shattered.sort_by_key(|(id, _)| *id);
    for &(_, entity) in &shattered {
        shatter_iron(world, entity, None);
    }
    let positions: Vec<Vec2> = smashed.iter().filter_map(|&e| centre(world, e)).collect();
    if hurt || !smashed.is_empty() || !shattered.is_empty() {
        let at = blocks[0].2;
        audio::play(world, Sound::Crushed, at);
    }
    for ball in smashed {
        despawn_tree(world, ball);
    }
    if positions.is_empty() {
        return;
    }
    // A few bursts spread over the set, as the black hole's steam puffs.
    let stride = positions.len().div_ceil(IMPACT_BURSTS_PER_STEP);
    for &at in positions.iter().step_by(stride) {
        spawn_burst(world, "ex10_ball_smash", at);
    }
}

/// Chills `iron` by `amount` this step, frozen at a full coating with its
/// frost burst at `contact` and its cue.
pub fn chill(world: &mut World, iron: Entity, amount: f32, contact: Vec2) {
    if !is_iron(world, iron) || world.has::<Frozen>(iron) {
        return;
    }
    let total = freeze_amount(world, iron) + amount;
    if total >= 1.0 - 1e-5 {
        freeze_iron(world, iron);
        let at = centre(world, iron).unwrap_or(contact);
        let size = if world.has::<IronBlock>(iron) {
            128.0
        } else {
            32.0
        };
        // Crystals on the face the cold met, where the iron cannot hide them.
        spawn_burst(world, "ex10_ice_frost", contact);
        ice_pulse(world, at, size, false);
    } else {
        world.insert(
            iron,
            Chill {
                amount: total,
                held: true,
            },
        );
    }
}

/// Freezes iron through: its chill gives way to [`Frozen`], and it wakes.
pub fn freeze_iron(world: &mut World, iron: Entity) {
    if is_iron(world, iron) && !world.has::<Frozen>(iron) {
        world.insert(iron, Frozen);
        world.remove_component::<Chill>(iron);
        wake(world, iron);
    }
}

/// `iron_thaw`, in the react phase: iron the cold did not reach this step
/// thaws by the step, its chill gone at 0.
pub fn thaw(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let mut thawed = Vec::new();
    for (entity, chill) in world.query_mut::<(Entity, &mut Chill)>() {
        if !chill.held {
            chill.amount = (chill.amount - THAW_RATE * dt).max(0.0);
        }
        chill.held = false;
        if chill.amount <= 0.0 {
            thawed.push(entity);
        }
    }
    for entity in thawed {
        world.remove_component::<Chill>(entity);
    }
}

/// Shatters the frozen iron whose surface lies within `radius` of `at`.
pub fn thermal_shatter(world: &mut World, at: Vec2, radius: f32) {
    let mut targets: Vec<(Option<SaveId>, Entity)> = world
        .query::<(Entity, &Frozen, &Position, &Collider)>()
        .filter(|(entity, ..)| is_iron(world, *entity))
        .filter_map(|(entity, _, p, collider)| {
            let offset = at - (p.0 + collider.offset);
            let distance = match collider.shape {
                Shape::Aabb { half_extents } => {
                    (offset.abs() - half_extents).max(Vec2::ZERO).length()
                }
                Shape::Circle { radius } => (offset.length() - radius).max(0.0),
            };
            (distance <= radius).then(|| (world.get::<SaveId>(entity).copied(), entity))
        })
        .collect();
    targets.sort_by_key(|(id, _)| *id);
    for (_, entity) in targets {
        shatter_iron(world, entity, Some(at));
    }
}

/// Breaks frozen iron: a block into sixteen scraps thrown out from its
/// grid, away from a blast, a scrap into chips, each with the ice's burst
/// and cue.
pub fn shatter_iron(world: &mut World, iron: Entity, blast: Option<Vec2>) {
    let Some(at) = centre(world, iron) else {
        return;
    };
    let whole = world.has::<IronBlock>(iron);
    let velocity = world.get::<Velocity>(iron).map_or(Vec2::ZERO, |v| v.0);
    let away = blast.map_or(Vec2::ZERO, |from| (at - from).normalize_or_zero());
    let born_at = world
        .get_resource::<crate::features::level::components::SceneTime>()
        .map_or(0.0, |t| t.0);
    despawn_tree(world, iron);
    if whole {
        for index in 0..SCRAP_COUNT {
            let (col, row) = (index % 4, index / 4);
            let offset = Vec2::new(col as f32 - 1.5, row as f32 - 1.5) * (SCRAP_HALF * 2.0);
            // Permutes the sixteen launch speeds without the game's stream.
            let variation = (index * 7 % SCRAP_COUNT) as f32 / (SCRAP_COUNT - 1) as f32;
            let kick = offset.normalize() * SCRAP_EJECT_SPEED * (0.8 + 0.4 * variation)
                + away * SCRAP_BLAST_BIAS
                - Vec2::Y * SCRAP_EJECT_LIFT;
            let tumble = ScrapTumble {
                born_at,
                initial_angle: variation * TAU,
                total_turn: (4.0 + 3.0 * variation) * if index % 2 == 0 { 1.0 } else { -1.0 },
            };
            toys::spawn_scrap(world, at + offset, velocity + kick, index + row, tumble);
        }
    }
    spawn_burst(world, "ex10_ice_shatter", at);
    ice_pulse(world, at, if whole { 128.0 } else { 32.0 }, true);
}

/// A cold pulse at `at`, unless [`BURST_CAP`] show, with its sound: a group
/// of shattering or freezing iron shares one through the cue's interval.
pub fn ice_pulse(world: &mut World, at: Vec2, size: f32, shatter: bool) {
    if world.query::<&IcePulse>().count() >= BURST_CAP {
        return;
    }
    audio::play(
        world,
        if shatter {
            Sound::Shattered
        } else {
            Sound::Froze
        },
        at,
    );
    world.spawn_with((
        IcePulse {
            at,
            size,
            age: 0.0,
            shatter,
        },
        SceneEntity { state_id: GAMEPLAY },
    ));
}

/// `ice_pulses`, in the present phase: each pulse ages by the frame and
/// goes after its duration.
pub fn age_pulses(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let mut done = Vec::new();
    for (entity, pulse) in world.query_mut::<(Entity, &mut IcePulse)>() {
        pulse.age += dt;
        if pulse.age >= pulse.duration() {
            done.push(entity);
        }
    }
    for entity in done {
        world.despawn(entity);
    }
}

/// `hole_douse`: the open holes put out every burning small ball in their
/// reach at once; the balls stay spent, as after a burnout, and a few of
/// them, spread over the set, puff steam.
pub fn douse(world: &mut World) {
    let holes = holes(world);
    if holes.is_empty() {
        return;
    }
    let mut doused: Vec<(Option<SaveId>, Entity, Vec2)> = world
        .query::<(Entity, &BallBurn, &Position, &Collider)>()
        .filter(|(_, burn, ..)| burn.remaining > 0.0)
        .map(|(entity, _, p, c)| (entity, p.0 + c.offset))
        .filter(|(_, p)| {
            holes
                .iter()
                .any(|h| h.distance_squared(*p) < HOLE_RADIUS * HOLE_RADIUS)
        })
        .map(|(entity, p)| (world.get::<SaveId>(entity).copied(), entity, p))
        .collect();
    if doused.is_empty() {
        return;
    }
    doused.sort_by_key(|(id, ..)| *id);
    for &(_, ball, _) in &doused {
        if let Some(burn) = world.get_mut::<BallBurn>(ball) {
            burn.remaining = 0.0;
        }
    }
    let stride = doused.len().div_ceil(DOUSE_BURSTS_PER_STEP);
    for &(.., at) in doused.iter().step_by(stride) {
        spawn_burst(world, "ex10_extinguish", at);
    }
    audio::play(world, Sound::Doused, doused[0].2);
}
