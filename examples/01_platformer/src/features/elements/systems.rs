//! The element rules' systems.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    BodyKind, Collider, CollisionEvent, Entity, EventQueue, Particle, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Position, PrevPosition, RigidBody, Shape, Sprite, Time,
    Transform, Velocity, With, World,
};
use tungsten::kit::{Children, ContactDamage, Health, Hit, Tags};
use tungsten::physics::wake;

use super::cascade::{self, CascadeKind};
use super::components::{BallBurn, Chill, FireEmitter, Frozen, IcePulse};
use super::{
    BASE_SMASH, BODY_HIT_SPEED, BURN_SECS, CRUSH_HIT, CRUSH_SPEED, DOUSE_BURSTS_PER_STEP,
    DOUSE_SECS, EMITTER_DWELL, FIRE_CENTRE, FIRE_EMITTER_CAP, FIRE_HALF, FROZEN_SECS,
    IMPACT_BURSTS_PER_STEP, IMPACT_COOLDOWN, IMPACT_SPEED, LANDING_SHARE, SHATTER_MASS,
    SHATTER_SPEED, SMASH_CAP, SMASH_KEEP, SPEED_PER_BALL, SPEED_PER_HIT, THAW_RATE, blows_keg,
    body_hit, freeze_amount,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{self, FLESH, FROZEN, Kind, has_status};
use crate::features::enemies::components::{Enemy, Throws};
use crate::features::enemies::{beetle, goblin};
use crate::features::level::components::{Doused, Glow, Hazard};
use crate::features::level::systems::drawn_time;
use crate::features::player::components::Player;
use crate::features::props::{
    self, BURNING_IMPACT,
    components::{Fuel, Keg, Rock},
};
use crate::features::run::Run;
use crate::features::toys::components::{
    Ball, BowlingBall, IronBlock, IronScrap, PreviousVelocity, ScrapTumble, SmallBall,
};
use crate::features::toys::systems::holes;
use crate::features::toys::{
    self, BALL_RADIUS, HOLE_RADIUS, IRON_HALF, IRON_LAND_SPEED, IRON_PUSH_MOST, IRON_PUSH_SHARE,
    SCRAP_COUNT, SCRAP_HALF, SMALL_BALL_RADIUS,
};
use crate::game::GAMEPLAY;
use crate::shared::{BURST_CAP, SaveId, body_centre, despawn_tree, spawn_burst, spawn_key_burst};

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

/// Lights `ball` when it is a small ball that never burned and is not
/// frozen.
pub fn ignite(world: &mut World, ball: Entity) {
    if world.has::<SmallBall>(ball) && !world.has::<BallBurn>(ball) && !world.has::<Frozen>(ball) {
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
    // Only the unburnt balls within reach of the burning ones are bucketed:
    // a ball farther from every source than a touch lights nothing (P5).
    let lit: Vec<Vec2> = sources
        .iter()
        .filter_map(|&source| body_centre(world, source))
        .collect();
    let bounds = lit
        .iter()
        .map(|&p| (p, p))
        .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)));
    let mut cells: HashMap<(i32, i32), Vec<(Entity, Vec2)>> = HashMap::new();
    if let Some((min, max)) = bounds {
        let (min, max) = (
            min - Vec2::splat(CONTACT_DISTANCE),
            max + Vec2::splat(CONTACT_DISTANCE),
        );
        for (entity, _, at, collider, burn) in
            world.query::<(Entity, &SmallBall, &Position, &Collider, Option<&BallBurn>)>()
        {
            let p = at.0 + collider.offset;
            if burn.is_some() || p.cmplt(min).any() || p.cmpgt(max).any() {
                continue;
            }
            cells
                .entry(cell(p, CONTACT_DISTANCE))
                .or_default()
                .push((entity, p));
        }
    }
    for &p in &lit {
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
        .filter_map(|(entity, _)| Some((entity, body_centre(world, entity)?)))
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
/// the burning surface. The burning crates take theirs first, a pair each
/// at its box's top; each other pair, a flame plume and sparks, stays on
/// one exposed ball for [`EMITTER_DWELL`], and those pairs hop in turn, so
/// the pool drifts along the crest. A pile with no exposed ball feeds it
/// from all.
pub fn fire_particles(world: &mut World) {
    let mut crates: Vec<(Option<SaveId>, Vec2)> = world
        .query::<(&Fuel, &Transform, &Collider, Option<&SaveId>)>()
        .filter(|(fuel, ..)| fuel.lit)
        .map(|(_, at, collider, id)| {
            let rise = match collider.shape {
                Shape::Aabb { half_extents } => half_extents.y,
                Shape::Circle { radius } => radius,
            };
            // A pair sits 6 px over its source: here, the box's top.
            let top = at.position + collider.offset - Vec2::new(0.0, rise - 6.0);
            (id.copied(), top)
        })
        .collect();
    crates.sort_by_key(|(id, _)| *id);
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
    let fixed = crates.len().min(FIRE_EMITTER_CAP / 2);
    let pairs = sources.len().min(FIRE_EMITTER_CAP / 2 - fixed);
    let count = (fixed + pairs) * 2;
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
        let source = if slot < fixed {
            crates[slot].1
        } else {
            let slot = slot - fixed;
            let hop = (time / EMITTER_DWELL + slot as f32 / pairs as f32).floor() as usize;
            sources[(slot * sources.len() / pairs + hop * 7919) % sources.len()]
        };
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

/// `fire_contacts`, in the overlap phase: a burning fireball hazard that
/// meets a ball over the step, either moving fast, lights a small ball and
/// bursts a bowling ball where they met, one burst per ball however many
/// fires overlap; a frozen ball it meets explodes instead. One that meets a
/// crate or a keg lights it, a fire nobody lit, or explodes it when
/// frozen. The player's touch is the kit's contact damage.
pub fn fire_contacts(world: &mut World) {
    let fires: Vec<(Vec2, Vec2)> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(entity, hazard)| hazard.fire && !world.has::<Doused>(*entity))
        .filter_map(|(entity, _)| fire_centres(world, entity))
        .collect();
    if fires.is_empty() {
        return;
    }
    // The fires' swept boxes joined, grown by their half size and a pixel:
    // a ball whose own swept box misses it meets no fire this step.
    let (low, high) = fires.iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), &(start, end)| (low.min(start.min(end)), high.max(start.max(end))),
    );
    let (low, high) = (low - FIRE_HALF - Vec2::ONE, high + FIRE_HALF + Vec2::ONE);
    let mut touched: Vec<(Option<SaveId>, Entity, Vec2)> = world
        .query::<(Entity, &Ball, &Position, Option<&PrevPosition>, &Collider)>()
        .filter_map(|(ball, _, at, prev, collider)| {
            let radius = match collider.shape {
                Shape::Circle { radius } => radius,
                Shape::Aabb { half_extents } => half_extents.x,
            };
            let end = at.0 + collider.offset;
            let start = prev.map_or(end, |prev| prev.0 + collider.offset);
            let reach = Vec2::splat(radius);
            if (start.max(end) + reach).cmplt(low).any()
                || (start.min(end) - reach).cmpgt(high).any()
            {
                return None;
            }
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
    // The props, each a box, after the balls.
    let props: Vec<(Option<SaveId>, Entity, Vec2)> = props::all(world)
        .into_iter()
        .filter_map(|prop| {
            let at = world.get::<Position>(prop)?;
            let prev = world.get::<PrevPosition>(prop);
            let collider = world.get::<Collider>(prop)?;
            let half = match collider.shape {
                Shape::Aabb { half_extents } => half_extents,
                Shape::Circle { radius } => Vec2::splat(radius),
            };
            let end = at.0 + collider.offset;
            let start = prev.map_or(end, |prev| prev.0 + collider.offset);
            let hit = fires
                .iter()
                .filter_map(|&(fire_start, fire_end)| {
                    swept_box(start - fire_start, end - fire_end, FIRE_HALF + half)
                })
                .min_by(f32::total_cmp)?;
            Some((
                world.get::<SaveId>(prop).copied(),
                prop,
                start.lerp(end, hit),
            ))
        })
        .collect();
    let mut props = props;
    props.sort_by_key(|(id, ..)| *id);
    touched.sort_by_key(|(id, ..)| *id);
    let player = cascade::player_credit(world);
    for (_, prop, at) in props {
        if world.has::<Frozen>(prop) {
            cascade::thermal_shock(world, prop, at, at);
        } else {
            props::light(world, prop, player);
        }
    }
    for (_, ball, at) in touched {
        if world.has::<Frozen>(ball) {
            cascade::thermal_shock(world, ball, at, at);
            continue;
        }
        if world.has::<SmallBall>(ball) {
            ignite(world, ball);
            continue;
        }
        if world.has::<BowlingBall>(ball) {
            despawn_tree(world, ball);
            spawn_burst(world, "ex10_ball_explosion", at);
            toys::ball_ring(world, at);
            audio::play(world, Sound::Blasted, at);
        }
    }
}

/// A bowling ball's closing speed from which it lands with a thud.
const BALL_THUD_SPEED: f32 = 700.0;

/// `ball_impacts`, in the overlap phase: a small ball that met something
/// faster than [`IMPACT_SPEED`] along the contact, its speed from before
/// the solver stopped it, bursts its sparks, then waits out its cooldown;
/// at most [`IMPACT_BURSTS_PER_STEP`] a step, the rest dropped. A bowling
/// ball closing at [`BALL_THUD_SPEED`] or more raises dust where it met and
/// thuds, one a step.
pub fn ball_impacts(world: &mut World) {
    let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() else {
        return;
    };
    let mut hits: Vec<Entity> = Vec::with_capacity(IMPACT_BURSTS_PER_STEP);
    let mut thud: Option<(Option<SaveId>, Vec2)> = None;
    'events: for event in events.iter_current() {
        let va = world
            .get::<PreviousVelocity>(event.a)
            .map_or(Vec2::ZERO, |v| v.0);
        let vb = event
            .b
            .and_then(|b| world.get::<PreviousVelocity>(b))
            .map_or(Vec2::ZERO, |v| v.0);
        let closing = -(va - vb).dot(event.normal);
        if closing >= BALL_THUD_SPEED {
            for (this, pushed) in [(Some(event.a), event.normal), (event.b, -event.normal)] {
                let Some(this) = this.filter(|&this| world.has::<BowlingBall>(this)) else {
                    continue;
                };
                let id = world.get::<SaveId>(this).copied();
                if thud.is_none_or(|(first, _)| id < first)
                    && let Some(centre) = body_centre(world, this)
                {
                    thud = Some((id, centre - pushed * BALL_RADIUS));
                }
            }
        }
        if closing <= IMPACT_SPEED {
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
        if let Some(at) = body_centre(world, ball) {
            spawn_burst(world, "ex10_small_ball_impact", at);
        }
    }
    if let Some((_, at)) = thud {
        spawn_burst(world, "ex10_landing_dust", at);
        audio::play(world, Sound::BallThudded, at);
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
            // The newborn first, then their emitter (P6).
            if particle.age != 0.0 {
                return None;
            }
            let emitter = world.get::<ParticleEmitter>(particle.emitter?)?;
            (emitter.config == config).then_some((entity, particle.velocity))
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
/// hurts the player with physical damage unless a troll threw it, kills a
/// live beetle with a whole hit of its health, and shatters frozen iron on
/// a hard head-on impact. It
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
                if !world.has::<SmallBall>(target)
                    && !world.has::<Player>(target)
                    && !frozen
                    && !beetle::is_beetle(world, target)
                {
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
        // The impact first, then whether an earlier block smashed it (P6).
        let mut struck: Vec<(Entity, f32, Option<SaveId>)> = world
            .query::<(Entity, &SmallBall, &Position, &Collider)>()
            .filter_map(|(entity, _, p, c)| {
                let speed = speed_into(entity, p.0 + c.offset, small_half)?;
                (!smashed.contains(&entity)).then_some(())?;
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
        let beetles: Vec<(Entity, f32, Vec2)> = world
            .query::<(Entity, &Health, &Position, &Collider)>()
            .filter(|(entity, health, ..)| !health.is_dead() && beetle::is_beetle(world, *entity))
            .filter_map(|(entity, health, p, c)| {
                let half = match c.shape {
                    Shape::Aabb { half_extents } => half_extents,
                    Shape::Circle { radius } => Vec2::splat(radius),
                };
                let target = p.0 + c.offset;
                speed_into(entity, target, half)?;
                Some((entity, health.current(), target))
            })
            .collect();
        for (crushed, left, target) in beetles {
            let push = (target - at).normalize_or_zero() * 300.0;
            let hit = combat::Hit::new(crushed, [(Kind::Physical, left)])
                .at(target)
                .push(push)
                .whole()
                .request(world)
                .apply(world);
            hurt |= matches!(hit, Hit::Hurt | Hit::Killed);
            keep = true;
        }
        if keep && let Some(current) = world.get_mut::<Velocity>(block) {
            current.0 = velocity * SMASH_KEEP;
        }
        // A thrown block hurts the player by its impact alone, once.
        let thrown = world
            .get_resource::<Throws>()
            .is_some_and(|throws| throws.of(block).is_some());
        let players: Vec<(Entity, f32, Vec2)> = world
            .query::<(Entity, &Player, &Position, &Collider)>()
            .filter(|_| !thrown)
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
            let hit = combat::Hit::new(player, [(Kind::Physical, CRUSH_HIT * hits)])
                .at(at)
                .push(push)
                .request(world)
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
        if let Some(at) = body_centre(world, entity) {
            cascade::queue(world, CascadeKind::Shatter, Some(entity), at, at, 0.0);
        }
    }
    let positions: Vec<Vec2> = smashed
        .iter()
        .filter_map(|&e| body_centre(world, e))
        .collect();
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

/// Whether `entity` is one of 01's bodies with element states: iron, a
/// scrap, a ball, a crate or a keg.
fn is_body(world: &World, entity: Entity) -> bool {
    is_iron(world, entity) || world.has::<Ball>(entity) || props::is_prop(world, entity)
}

/// Whether a body burns: a small ball on fire, a crate on fire or a keg's
/// lit fuse, which take no frost.
fn burns(world: &World, entity: Entity) -> bool {
    world
        .get::<BallBurn>(entity)
        .is_some_and(|burn| burn.remaining > 0.0)
        || props::burning(world, entity)
}

/// Chills a body, iron, a ball, a crate or a keg, by `amount` this step,
/// frozen at a full coating with its frost burst at `contact` and its cue;
/// a burning one takes no frost.
pub fn chill(world: &mut World, iron: Entity, amount: f32, contact: Vec2) {
    if !is_body(world, iron) || world.has::<Frozen>(iron) || burns(world, iron) {
        return;
    }
    let total = freeze_amount(world, iron) + amount;
    if total >= 1.0 - 1e-5 {
        freeze_iron(world, iron);
        let at = body_centre(world, iron).unwrap_or(contact);
        let size = if world.has::<IronBlock>(iron) {
            128.0
        } else if props::is_prop(world, iron) {
            64.0
        } else if world.has::<SmallBall>(iron) {
            16.0
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

/// Freezes a body through: its chill gives way to [`Frozen`] for
/// [`FROZEN_SECS`] and it wakes. A burning body does not freeze.
pub fn freeze_iron(world: &mut World, iron: Entity) {
    if is_body(world, iron) && !world.has::<Frozen>(iron) && !burns(world, iron) {
        world.insert(iron, Frozen { left: FROZEN_SECS });
        world.remove_component::<Chill>(iron);
        wake(world, iron);
    }
}

/// `frozen_tick`, in the tick phase: each frozen body's clock runs down by
/// the step, and at 0 it thaws.
pub fn tick_frozen(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let mut thawed = Vec::new();
    for (entity, frozen) in world.query_mut::<(Entity, &mut Frozen)>() {
        frozen.left -= dt;
        if frozen.left <= 1e-5 {
            thawed.push(entity);
        }
    }
    for entity in thawed {
        world.remove_component::<Frozen>(entity);
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

/// A blast's heat at `at`: each frozen body whose surface lies within
/// `radius` of it queues thermal shock, its chain begun at `at`.
pub fn thermal_shatter(world: &mut World, at: Vec2, radius: f32) {
    let mut targets: Vec<(Option<SaveId>, Entity, Vec2)> = world
        .query::<(Entity, &Frozen, &Position, &Collider)>()
        .filter(|(entity, ..)| is_body(world, *entity))
        .filter_map(|(entity, _, p, collider)| {
            let body = p.0 + collider.offset;
            let offset = at - body;
            let distance = match collider.shape {
                Shape::Aabb { half_extents } => {
                    (offset.abs() - half_extents).max(Vec2::ZERO).length()
                }
                Shape::Circle { radius } => (offset.length() - radius).max(0.0),
            };
            (distance <= radius).then(|| (world.get::<SaveId>(entity).copied(), entity, body))
        })
        .collect();
    targets.sort_by_key(|(id, ..)| *id);
    for (_, entity, body) in targets {
        cascade::thermal_shock(world, entity, body, at);
    }
}

/// A body's velocity as the step began, which an impact reads.
fn before_step(world: &World, entity: Entity) -> Vec2 {
    world
        .get::<PreviousVelocity>(entity)
        .map(|v| v.0)
        .or_else(|| world.get::<Velocity>(entity).map(|v| v.0))
        .unwrap_or(Vec2::ZERO)
}

/// Whether `entity` is a dynamic body.
fn moves(world: &World, entity: Entity) -> bool {
    world
        .get::<RigidBody>(entity)
        .is_some_and(|body| body.kind == BodyKind::Dynamic)
}

/// A dynamic body's mass.
fn mass(world: &World, entity: Entity) -> f32 {
    world
        .get::<RigidBody>(entity)
        .filter(|body| body.inv_mass > 0.0)
        .map_or(f32::INFINITY, |body| 1.0 / body.inv_mass)
}

/// A thrown body's mass: a dynamic body's own, an enemy's its kind's, whose
/// kinematic body has none.
fn thrown_mass(world: &World, entity: Entity) -> f32 {
    match world.get::<Enemy>(entity) {
        Some(enemy) if !moves(world, entity) => enemy.kind.info().mass,
        _ => mass(world, entity),
    }
}

/// The live entity of save ID `id`.
fn live_by_id(world: &World, id: u64) -> Option<Entity> {
    world
        .query::<(Entity, &SaveId, &Health)>()
        .find(|(_, own, health)| own.0 == id && !health.is_dead())
        .map(|(entity, ..)| entity)
}

/// The gap between two bodies' surfaces, boxes or circles: below 0 where
/// they overlap.
fn gap(a: Vec2, a_shape: Shape, b: Vec2, b_shape: Shape) -> f32 {
    let half = |shape: Shape| match shape {
        Shape::Aabb { half_extents } => half_extents,
        Shape::Circle { radius } => Vec2::splat(radius),
    };
    match (a_shape, b_shape) {
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => a.distance(b) - ra - rb,
        (Shape::Circle { radius }, other) | (other, Shape::Circle { radius }) => {
            let (circle, boxed) = if matches!(a_shape, Shape::Circle { .. }) {
                (a, b)
            } else {
                (b, a)
            };
            ((circle - boxed).abs() - half(other))
                .max(Vec2::ZERO)
                .length()
                - radius
        }
        _ => ((a - b).abs() - (half(a_shape) + half(b_shape))).max_element(),
    }
}

/// The moving bodies that end the step touching a live target, each with
/// its closing speed along the line between their centres from their
/// speeds before the step: the solver stops a body at a target's face in
/// the step it arrives, before any contact reports it. A body is iron, a
/// scrap or a ball, or any body a troll threw; the player is a target of
/// thrown bodies alone, and a thrown body never hits its thrower.
fn target_hits(world: &World, player: Option<Entity>) -> Vec<(Entity, Entity, f32)> {
    let targets: Vec<(Entity, Vec2, Shape, Option<u64>)> = world
        .query::<(Entity, &Health, &Position, &Collider)>()
        .filter(|(_, health, ..)| !health.is_dead())
        .map(|(entity, _, at, collider)| {
            let id = world.get::<SaveId>(entity).map(|id| id.0);
            (entity, at.0 + collider.offset, collider.shape, id)
        })
        .collect();
    if targets.is_empty() {
        return Vec::new();
    }
    let throws = world.get_resource::<Throws>();
    let mut hits = Vec::new();
    for (body, at, collider, previous, velocity) in world.query::<(
        Entity,
        &Position,
        &Collider,
        Option<&PreviousVelocity>,
        Option<&Velocity>,
    )>() {
        // `before_step` from the row: the cheap speed check comes first.
        let before = previous
            .map(|v| v.0)
            .or_else(|| velocity.map(|v| v.0))
            .unwrap_or(Vec2::ZERO);
        if before.length_squared() < BODY_HIT_SPEED * BODY_HIT_SPEED {
            continue;
        }
        let throw = throws.and_then(|throws| throws.of(body));
        if throw.is_none() && !is_body(world, body) {
            continue;
        }
        let centre = at.0 + collider.offset;
        for &(target, target_at, shape, id) in &targets {
            let thrower = throw.is_some_and(|throw| id == Some(throw.thrower));
            let spared = Some(target) == player && throw.is_none();
            if target == body || thrower || spared {
                continue;
            }
            if gap(centre, collider.shape, target_at, shape) > CONTACT_MARGIN {
                continue;
            }
            let towards = (target_at - centre).normalize_or_zero();
            let closing = (before - before_step(world, target)).dot(towards);
            if closing >= BODY_HIT_SPEED {
                hits.push((body, target, closing));
            }
        }
    }
    hits
}

/// A moving body's hit on a target: the body, the target and the closing
/// speed.
type Hurt = (Entity, Entity, f32);

/// The targets an iron blow killed this step, which the enemies' and the
/// dummy's ends burst in gore in place of a poof: the step's alone, written
/// fresh by `body_impacts`, presentation, never saved.
#[derive(Debug, Clone, Default)]
pub struct IronKills(pub Vec<Entity>);

/// Whether an iron blow killed `target` this step.
#[must_use]
pub fn iron_killed(world: &World, target: Entity) -> bool {
    world
        .get_resource::<IronKills>()
        .is_some_and(|kills| kills.0.contains(&target))
}

/// `body_impacts`, in the overlap phase after the physics step: a frozen
/// body meeting something that does not move, or a body of at least
/// [`SHATTER_MASS`], at a closing speed of [`SHATTER_SPEED`] or more, from
/// the step's contacts and the bodies' speeds before it, queues its
/// shatter; a moving body that ends the step touching a live target
/// ([`target_hits`]), closing at [`BODY_HIT_SPEED`] or more, hurts it by
/// [`body_hit`] of its mass and that speed, nothing under its floor
/// (`BODY_HIT_FLOOR`, read before a thrower's factor: no part, no fire, no
/// push, and a throw flies on), credited to the player, or for a thrown
/// body by its thrower's damage factor, credited to its thrower while it
/// lives, the throw ending at its first such hit, a goblin's thrown rock landing
/// `goblin::ROCK_DAMAGE` with `goblin::ROCK_PUSH` along its flight in place
/// of the speed's; and a frozen target landing as hard
/// takes [`LANDING_SHARE`] of the speed past [`SHATTER_SPEED`]. Each pair
/// counts once a step, at its hardest. The throws count down. Iron's hits
/// (a block's or a scrap's, thrown or not) push their target along the
/// closing line by [`IRON_PUSH_SHARE`] of the closing speed, at most
/// [`IRON_PUSH_MOST`], and apply at once, as the crush's do, so each knows
/// whether it killed: a `flesh` target it killed is marked ([`IronKills`]).
/// A block meeting a still body or a tile at [`IRON_LAND_SPEED`] or more
/// lands hard, once a step (`toys::systems::iron_landed`). A keg that is
/// not frozen meeting something hard enough for its mass ([`blows_keg`])
/// goes off, credited to its own throw's thrower (a troll's), else the
/// thrower of what met it, else the player; a burning
/// crate's hit on a target adds [`BURNING_IMPACT`] fire; and a goblin's rock
/// meeting anything at [`SHATTER_SPEED`] or more clacks, at most
/// [`IMPACT_BURSTS_PER_STEP`] a step.
pub fn body_impacts(world: &mut World) {
    world.insert_resource(IronKills::default());
    let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() else {
        return;
    };
    let player = world.get_resource::<Run>().map(|run| run.player);
    let mut shatters: Vec<Entity> = Vec::new();
    let mut hits: BTreeMap<(Option<SaveId>, Option<SaveId>), Hurt> = BTreeMap::new();
    let mut landings: BTreeMap<Option<SaveId>, (Entity, f32)> = BTreeMap::new();
    let mut hard: BTreeMap<Option<SaveId>, Vec2> = BTreeMap::new();
    let mut blown: BTreeMap<Option<SaveId>, (Entity, Option<u64>)> = BTreeMap::new();
    let mut clacks: BTreeMap<Option<SaveId>, Vec2> = BTreeMap::new();
    let throws = world.get_resource::<Throws>();
    for event in events.iter_current() {
        let va = before_step(world, event.a);
        let vb = event.b.map_or(Vec2::ZERO, |b| before_step(world, b));
        let closing = -(va - vb).dot(event.normal);
        // Both a shatter and a landing need this speed.
        if closing < SHATTER_SPEED {
            continue;
        }
        for (this, other, pushed) in [
            (Some(event.a), event.b, event.normal),
            (event.b, Some(event.a), -event.normal),
        ] {
            let Some(this) = this else { continue };
            let still = other.is_none_or(|other| !moves(world, other));
            if still && closing >= IRON_LAND_SPEED && world.has::<IronBlock>(this) {
                let id = world.get::<SaveId>(this).copied();
                if let (false, Some(centre)) = (hard.contains_key(&id), body_centre(world, this)) {
                    hard.insert(id, centre - pushed * IRON_HALF.y);
                }
            }
            let heavy = other
                .is_some_and(|other| moves(world, other) && mass(world, other) >= SHATTER_MASS);
            if closing >= SHATTER_SPEED
                && world.has::<Frozen>(this)
                && is_body(world, this)
                && (still || heavy)
                && !shatters.contains(&this)
            {
                shatters.push(this);
            }
            if world.has::<Rock>(this)
                && let Some(centre) = body_centre(world, this)
            {
                clacks
                    .entry(world.get::<SaveId>(this).copied())
                    .or_insert(centre);
            }
            // What met it weighs in full unless it is a lighter dynamic body.
            let weight = other
                .filter(|&other| moves(world, other))
                .map_or(f32::INFINITY, |other| mass(world, other));
            if world.has::<Keg>(this) && !world.has::<Frozen>(this) && blows_keg(weight, closing) {
                let thrower = |body: Option<Entity>| {
                    body.and_then(|body| throws.and_then(|throws| throws.of(body)))
                        .map(|throw| throw.thrower)
                };
                let credit = thrower(Some(this))
                    .or_else(|| thrower(other))
                    .or_else(|| cascade::player_credit(world));
                let id = world.get::<SaveId>(this).copied();
                blown.entry(id).or_insert((this, credit));
            }
            let target = Some(this) != player
                && world
                    .get::<Health>(this)
                    .is_some_and(|health| !health.is_dead());
            if !target {
                continue;
            }
            let id = world.get::<SaveId>(this).copied();
            if still && closing >= SHATTER_SPEED && has_status(world, this, FROZEN) {
                let best = landings.entry(id).or_insert((this, closing));
                best.1 = best.1.max(closing);
            }
        }
    }
    for (body, target, closing) in target_hits(world, player) {
        let key = (
            world.get::<SaveId>(target).copied(),
            world.get::<SaveId>(body).copied(),
        );
        let best = hits.entry(key).or_insert((body, target, closing));
        best.2 = best.2.max(closing);
    }
    let mut shatters: Vec<(Option<SaveId>, Entity)> = shatters
        .into_iter()
        .map(|entity| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    shatters.sort_by_key(|(id, _)| *id);
    for (_, entity) in shatters {
        if let Some(at) = body_centre(world, entity) {
            cascade::queue(world, CascadeKind::Shatter, Some(entity), at, at, 0.0);
        }
    }
    for (keg, credit) in blown.into_values() {
        props::set_off(world, keg, None, credit);
    }
    let mut landed: Vec<Entity> = Vec::new();
    let mut kills: Vec<Entity> = Vec::new();
    for (body, target, closing) in hits.into_values() {
        let throw = world
            .get_resource::<Throws>()
            .and_then(|throws| throws.of(body))
            .copied();
        let heavy = match throw {
            Some(_) => thrown_mass(world, body),
            None => mass(world, body),
        };
        // A goblin's thrown rock lands its own blow, pushed along its flight.
        let rock = throw.is_some() && world.has::<Rock>(body);
        let amount = if rock {
            goblin::ROCK_DAMAGE
        } else {
            body_hit(heavy, closing)
        };
        // Under the floor it lands nothing, and a throw flies on.
        if amount <= 0.0 {
            continue;
        }
        let at = body_centre(world, target).unwrap_or_default();
        let fire = world
            .get::<Fuel>(body)
            .is_some_and(|fuel| fuel.lit)
            .then_some((Kind::Fire, BURNING_IMPACT));
        let hit = match throw {
            Some(throw) => {
                landed.push(body);
                let parts = [(Kind::Physical, amount * throw.factor)]
                    .into_iter()
                    .chain(fire);
                let mut hit = combat::Hit::new(target, parts);
                if rock {
                    let along = before_step(world, body).normalize_or_zero();
                    hit = hit.push(along * goblin::ROCK_PUSH);
                }
                match live_by_id(world, throw.thrower) {
                    Some(thrower) => hit.from(thrower),
                    None => hit,
                }
            }
            None => {
                let parts = [(Kind::Physical, amount)].into_iter().chain(fire);
                let hit = combat::Hit::new(target, parts);
                match player {
                    Some(player) => hit.from(player),
                    None => hit,
                }
            }
        };
        if !is_iron(world, body) {
            hit.at(at).queue(world);
            continue;
        }
        let from = body_centre(world, body).unwrap_or(at);
        let push =
            (at - from).normalize_or_zero() * (IRON_PUSH_SHARE * closing).min(IRON_PUSH_MOST);
        let outcome = hit.at(at).push(push).request(world).apply(world);
        let flesh = world
            .get::<Tags>(target)
            .is_some_and(|tags| tags.has(FLESH));
        if matches!(outcome, Hit::Killed) && flesh {
            kills.push(target);
        }
    }
    world.insert_resource(IronKills(kills));
    for at in hard.into_values() {
        toys::systems::iron_landed(world, at);
    }
    // A rock meeting anything that hard clacks, a few a step.
    for at in clacks.into_values().take(IMPACT_BURSTS_PER_STEP) {
        audio::play(world, Sound::RockHit, at);
    }
    let dt = world.resource::<Time>().delta();
    let gone: Vec<Entity> = world
        .get_resource::<Throws>()
        .map(|throws| {
            throws
                .0
                .iter()
                .map(|throw| throw.body)
                .filter(|&body| !world.is_alive(body))
                .collect()
        })
        .unwrap_or_default();
    if let Some(throws) = world.get_resource_mut::<Throws>() {
        throws.0.retain_mut(|throw| {
            throw.left -= dt;
            throw.left > 0.0 && !landed.contains(&throw.body) && !gone.contains(&throw.body)
        });
    }
    for (target, closing) in landings.into_values() {
        let amount = (closing - SHATTER_SPEED) * LANDING_SHARE;
        if amount > 0.0 {
            combat::Hit::new(target, [(Kind::Physical, amount)]).queue(world);
        }
    }
}

/// Puts the fire hazard `hazard` out for [`DOUSE_SECS`]: its touch held
/// aside, its glow and trail hidden, with a steam burst; one already out
/// starts its time again.
pub fn douse_hazard(world: &mut World, hazard: Entity) {
    if let Some(doused) = world.get_mut::<Doused>(hazard) {
        doused.left = DOUSE_SECS;
        return;
    }
    if !world.get::<Hazard>(hazard).is_some_and(|h| h.fire) {
        return;
    }
    let touch = world.remove_component::<ContactDamage>(hazard);
    world.insert(
        hazard,
        Doused {
            left: DOUSE_SECS,
            touch,
        },
    );
    if let Some((_, at)) = fire_centres(world, hazard) {
        spawn_burst(world, "ex10_steam_burst", at);
        audio::play(world, Sound::Steamed, at);
    }
}

/// `douse_tick`, in the tick phase: each doused hazard's time runs down by
/// the step, and at 0 it relights, its touch back and its trail emitting.
pub fn tick_doused(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let mut relit = Vec::new();
    for (entity, doused) in world.query_mut::<(Entity, &mut Doused)>() {
        doused.left -= dt;
        if doused.left <= 1e-5 {
            relit.push(entity);
        }
    }
    for hazard in relit {
        if let Some(touch) = world
            .remove_component::<Doused>(hazard)
            .and_then(|doused| doused.touch)
        {
            world.insert(hazard, touch);
        }
        let children: Vec<Entity> = world
            .get::<Children>(hazard)
            .map(|children| children.iter().collect())
            .unwrap_or_default();
        for child in children {
            if let Some(state) = world.get_mut::<ParticleEmitterState>(child) {
                state.drained = false;
            }
        }
    }
}

/// `doused_look`, before the frame's particles: a doused hazard's glow is
/// hidden, its trail stops and its fireball dims; a burning one shows.
pub fn doused_look(world: &mut World) {
    let hazards: Vec<(Entity, bool, Vec<Entity>)> = world
        .query::<(Entity, &Hazard, Option<&Children>)>()
        .filter(|(_, hazard, _)| hazard.fire)
        .map(|(entity, _, children)| {
            let children = children.map(|c| c.iter().collect()).unwrap_or_default();
            (entity, world.has::<Doused>(entity), children)
        })
        .collect();
    for (hazard, doused, children) in hazards {
        if let Some(glow) = world.get_mut::<Glow>(hazard) {
            glow.hidden = doused;
        }
        if let Some(sprite) = world.get_mut::<Sprite>(hazard) {
            sprite.color = if doused {
                [110, 110, 122, 255]
            } else {
                [255; 4]
            };
        }
        if doused {
            for child in children {
                if let Some(state) = world.get_mut::<ParticleEmitterState>(child) {
                    state.drained = true;
                }
            }
        }
    }
}

/// Breaks frozen iron: a block into sixteen scraps thrown out from its
/// grid, away from a blast, a scrap into chips, each with the ice's burst
/// and cue.
pub fn shatter_iron(world: &mut World, iron: Entity, blast: Option<Vec2>) {
    let Some(at) = body_centre(world, iron) else {
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
    spawn_key_burst(world, "ex10_ice_shatter", at);
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
/// reach at once; the balls stay spent, as after a burnout. They put out
/// every crate and keg in their reach too, each kept from lighting while it
/// stays there, its fuel kept. A few of what burned, spread over the set,
/// puff steam.
pub fn douse(world: &mut World) {
    let holes = holes(world);
    if holes.is_empty() {
        return;
    }
    let inside = |p: Vec2| {
        holes
            .iter()
            .any(|h| h.distance_squared(p) < HOLE_RADIUS * HOLE_RADIUS)
    };
    let mut doused: Vec<(Option<SaveId>, Entity, Vec2)> = world
        .query::<(Entity, &BallBurn, &Position, &Collider)>()
        .filter(|(_, burn, ..)| burn.remaining > 0.0)
        .map(|(entity, _, p, c)| (entity, p.0 + c.offset))
        .filter(|(_, p)| inside(*p))
        .map(|(entity, p)| (world.get::<SaveId>(entity).copied(), entity, p))
        .collect();
    let mut wet: Vec<(Option<SaveId>, Entity, Vec2)> = props::all(world)
        .into_iter()
        .filter_map(|entity| Some((entity, body_centre(world, entity)?)))
        .filter(|(_, p)| inside(*p))
        .map(|(entity, p)| (world.get::<SaveId>(entity).copied(), entity, p))
        .collect();
    wet.sort_by_key(|(id, ..)| *id);
    for (id, prop, p) in wet {
        if props::put_out(world, prop) {
            doused.push((id, prop, p));
        }
    }
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
