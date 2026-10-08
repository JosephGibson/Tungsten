//! R places an iron brick at the cursor: a heavy box that shoves the balls
//! aside, barely yields to the player and crushes what it drives into.
use std::collections::{HashMap, HashSet};

use glam::Vec2;
use tungsten::Bundle;
use tungsten::core::{
    ActionMap, CameraState, Entity, EventQueue, InputState, Time, Transform, World,
};
use tungsten::physics::{
    Collider, CollisionEvent, Position, PrevPosition, RigidBody, RigidBodyBundle, Velocity,
};

use crate::gameplay::{Health, PreviousVelocity, damage_player};
use crate::state::{
    BALL_RADIUS, EffectSounds, PLAYER_HALF, Player, SMALL_BALL_BURSTS_PER_FRAME, SMALL_BALL_SCALE,
    SmallBall,
};
use crate::systems::{cursor_to_world, play_effect_sound, spawn_transient_effect};

/// Four large-ball diameters on a side.
pub(crate) const BRICK_HALF: Vec2 = Vec2::splat(4.0 * BALL_RADIUS);
/// Two hundred balls' worth: collisions and spell forces barely move it.
pub(crate) const BRICK_MASS: f32 = 200.0;
pub(crate) const BRICK_CAP: usize = 24;
/// Sliding friction while the brick rests on something, in pixels/second².
/// Resists sliding when the player or a pile of balls pushes it.
pub(crate) const BRICK_FRICTION: f32 = 1500.0;
/// Ice keeps its slippery feel independently of unfrozen iron's friction.
pub(crate) const FROZEN_FRICTION: f32 = 120.0;
pub(crate) const SCRAP_HALF: Vec2 = Vec2::splat(BRICK_HALF.x / 4.0);
pub(crate) const SCRAP_COUNT: usize = 16;
/// Sixteen scraps preserve a full block's mass, each with one sixteenth of its friction.
pub(crate) const SCRAP_MASS: f32 = BRICK_MASS / 16.0;
pub(crate) const SCRAP_FRICTION: f32 = BRICK_FRICTION / 16.0;
/// Fracture launches chunks outward at 800–1,200 pixels/second before the blast push.
const SCRAP_EJECT_SPEED: f32 = 1000.0;
/// A surface explosion also throws the broken block away from the contact face.
const SCRAP_BLAST_BIAS: f32 = 600.0;
/// A little lift keeps the fracture visible when the block rests on the floor.
const SCRAP_EJECT_LIFT: f32 = 180.0;
/// A fifth of the pull on balls: nearby holes can drag supported iron.
pub(crate) const BRICK_BLACK_HOLE_PULL: f32 = 0.2;
/// Loose fragments respond like balls, so holes readily sweep the light debris up.
pub(crate) const SCRAP_BLACK_HOLE_PULL: f32 = 1.0;
/// The brick's own speed into a body, pixels/second, from which it hurts
/// the player and smashes small balls; a resting or creeping brick does neither.
pub(crate) const BRICK_CRUSH_SPEED: f32 = 360.0;
/// Pre-resolution head-on speed and relative closing speed needed to break frozen iron.
pub(crate) const BRICK_SHATTER_SPEED: f32 = 600.0;
/// Each further step of this much speed costs the player one more heart.
pub(crate) const BRICK_SPEED_PER_HEART: f32 = 600.0;
/// A whole handful of marbles gives way at the crushing threshold.
const BRICK_BASE_SMASH: usize = 4;
/// Each further step of this much head-on speed smashes one more ball per step.
pub(crate) const BRICK_SPEED_PER_BALL: f32 = 60.0;
const BRICK_SMASH_CAP: usize = 64;
/// Share of its speed a brick keeps through a layer of balls it smashed:
/// glass gives way, so a fast brick plows on through layer after layer.
pub(crate) const BRICK_SMASH_KEEP: f32 = 0.9;
/// Several contacts/physics steps share one crunch instead of flooding voices.
const CRUSH_SFX_INTERVAL: f32 = 0.1;
/// Speculative contacts stop the brick at a body's surface, not inside it.
const CONTACT_MARGIN: f32 = 3.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct IronBrick;

/// A small physical piece of shattered iron with its own broken-metal sprite.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IronScrap {
    pub(crate) variant: usize,
}

/// A brief visual tumble on the interpolated scene clock; colliders remain boxes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScrapTumble {
    pub(crate) born_at: f32,
    pub(crate) initial_angle: f32,
    pub(crate) total_turn: f32,
}

impl ScrapTumble {
    pub(crate) fn rotation(self, time: f32) -> f32 {
        let age = (time - self.born_at).max(0.0);
        self.initial_angle + self.total_turn * (1.0 - (-4.0 * age).exp())
    }
}

/// Thermal contact checks the object's surface, rather than its centre.
/// Whole bricks become a 4×4 grid of scraps; refrozen scraps become chips.
pub(crate) fn thermal_shatter(world: &mut World, at: Vec2, radius: f32) {
    let mut targets: Vec<_> = world
        .query::<(Entity, &crate::ice::Frozen, &Position, &Collider)>()
        .filter(|(e, _, _, _)| world.has::<IronBrick>(*e) || world.has::<IronScrap>(*e))
        .filter_map(|(e, _, p, collider)| {
            let offset = at - (p.0 + collider.offset);
            let distance = match collider.shape {
                tungsten::physics::Shape::Aabb { half_extents } => {
                    (offset.abs() - half_extents).max(Vec2::ZERO).length()
                }
                tungsten::physics::Shape::Circle { radius } => (offset.length() - radius).max(0.0),
            };
            (distance <= radius).then_some(e)
        })
        .collect();
    targets.sort_by_key(|e| e.id());
    for entity in targets {
        shatter_iron(world, entity, Some(at));
    }
}

/// Thermal blasts and hard iron impacts use the same debris and ice-chip burst.
fn shatter_iron(world: &mut World, entity: Entity, blast_origin: Option<Vec2>) {
    let Some(center) = world.get::<Position>(entity).map(|p| p.0) else {
        return;
    };
    let whole = world.has::<IronBrick>(entity);
    let velocity = world.get::<Velocity>(entity).map_or(Vec2::ZERO, |v| v.0);
    let blast_direction = blast_origin.map_or(Vec2::ZERO, |at| (center - at).normalize_or_zero());
    let born_at = world
        .get_resource::<crate::gameplay::SceneTime>()
        .map_or(0.0, |t| t.0);
    world.despawn(entity);
    if whole {
        for index in 0..SCRAP_COUNT {
            let col = index % 4;
            let row = index / 4;
            let offset = Vec2::new(col as f32 - 1.5, row as f32 - 1.5) * (SCRAP_HALF * 2.0);
            let position = center + offset;
            // Permute the sixteen launch speeds without consuming gameplay RNG.
            let variation = (index * 7 % SCRAP_COUNT) as f32 / (SCRAP_COUNT - 1) as f32;
            let kick = offset.normalize() * SCRAP_EJECT_SPEED * (0.8 + 0.4 * variation)
                + blast_direction * SCRAP_BLAST_BIAS
                - Vec2::Y * SCRAP_EJECT_LIFT;
            world.spawn_with(
                RigidBodyBundle::dynamic(Position(position), Collider::aabb(SCRAP_HALF))
                    .with_body(
                        RigidBody::dynamic()
                            .with_mass(SCRAP_MASS)
                            .with_restitution(0.2),
                    )
                    .with_velocity(Velocity(velocity + kick))
                    .with((
                        IronScrap {
                            variant: (index + row) % 4,
                        },
                        ScrapTumble {
                            born_at,
                            initial_angle: variation * std::f32::consts::TAU,
                            total_turn: (4.0 + 3.0 * variation)
                                * if index % 2 == 0 { 1.0 } else { -1.0 },
                        },
                        PrevPosition(position),
                        Transform::from_position(position),
                    )),
            );
        }
    }
    spawn_transient_effect(world, "ex10_ice_shatter", center);
    crate::ice::ice_pulse(world, center, if whole { 128.0 } else { 32.0 }, true);
}

/// One brick per fresh press, at the cursor, while fewer than `BRICK_CAP` exist.
pub(crate) fn place_brick_system(world: &mut World) {
    if crate::death::player_dead(world) {
        return;
    }
    let pressed = world
        .get_resource::<InputState>()
        .zip(world.get_resource::<ActionMap>())
        .is_some_and(|(input, actions)| actions.just_pressed(input, "place_brick"));
    if !pressed || world.query::<(Entity, &IronBrick)>().count() >= BRICK_CAP {
        return;
    }
    let Some(at) = world
        .get_resource::<InputState>()
        .and_then(InputState::cursor_position)
        .zip(world.get_resource::<CameraState>())
        .and_then(|((x, y), camera)| cursor_to_world(Vec2::new(x, y), camera))
    else {
        return;
    };
    spawn_brick(world, at);
}

pub(crate) fn spawn_brick(world: &mut World, at: Vec2) -> Entity {
    world.spawn_with(
        RigidBodyBundle::dynamic(Position(at), Collider::aabb(BRICK_HALF))
            .with_body(
                RigidBody::dynamic()
                    .with_mass(BRICK_MASS)
                    .with_restitution(0.05),
            )
            .with((IronBrick, PrevPosition(at), Transform::from_position(at))),
    )
}

/// The solver has no friction, so a supported brick loses horizontal speed
/// here. Runs after the step on its support contacts.
pub(crate) fn brick_friction(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() else {
        return;
    };
    // `normal` is the way `a` was pushed: up for a body resting on `b`.
    let supported: HashSet<Entity> = events
        .iter_current()
        .filter_map(|event| {
            if event.normal.y < -0.5 {
                Some(event.a)
            } else if event.normal.y > 0.5 {
                event.b
            } else {
                None
            }
        })
        .filter(|e| world.has::<IronBrick>(*e) || world.has::<IronScrap>(*e))
        .collect();
    for brick in supported {
        let friction = if world.has::<crate::ice::Frozen>(brick) {
            FROZEN_FRICTION
        } else {
            BRICK_FRICTION
        };
        let friction = if world.has::<IronScrap>(brick) {
            friction * (SCRAP_FRICTION / BRICK_FRICTION)
        } else {
            friction
        };
        if let Some(velocity) = world.get_mut::<Velocity>(brick) {
            let speed = (velocity.0.x.abs() - friction * dt).max(0.0);
            velocity.0.x = speed.copysign(velocity.0.x);
        }
    }
}

/// How fast a brick at `center` moving at `velocity` drives into a body at
/// `target` with half extents `half`: its speed along the face they meet
/// on, when the two touch and it is at least `BRICK_CRUSH_SPEED`.
fn impact(center: Vec2, velocity: Vec2, target: Vec2, half: Vec2) -> Option<f32> {
    let offset = target - center;
    let gap = offset.abs() - (BRICK_HALF + half);
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
    (speed >= BRICK_CRUSH_SPEED).then_some(speed)
}

/// How many steps of `per` the speed is past `BRICK_CRUSH_SPEED`, plus one.
fn severity(speed: f32, per: f32) -> usize {
    1 + ((speed - BRICK_CRUSH_SPEED) / per) as usize
}

/// A moving brick damages the player, crushes marbles and shatters frozen iron
/// on a sufficiently fast head-on impact.
/// It reads the velocity from before the step: by now the solver has
/// stopped it on whatever it hit. A brick that smashed balls gets back
/// `BRICK_SMASH_KEEP` of that velocity, so it carries on into the next layer.
pub(crate) fn brick_impacts(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if let Some(sounds) = world.get_resource_mut::<EffectSounds>() {
        sounds.crush_cooldown = (sounds.crush_cooldown - dt).max(0.0);
    }
    let bricks: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &IronBrick, &Position)>()
        .filter_map(|(e, _, p)| {
            let velocity = world.get::<PreviousVelocity>(e)?.0;
            (velocity.length() >= BRICK_CRUSH_SPEED).then_some((e, p.0, velocity))
        })
        .collect();
    if bricks.is_empty() {
        return;
    }
    // The solver may already have pushed a ball away from the brick's face.
    // Preserve those contacts, including either ordering of the pair.
    let mut contacts = HashMap::<(Entity, Entity), f32>::new();
    if let Some(events) = world.get_resource::<EventQueue<CollisionEvent>>() {
        for event in events.iter_current() {
            let Some(b) = event.b else { continue };
            for (brick, target, normal) in [(event.a, b, -event.normal), (b, event.a, event.normal)]
            {
                if world.get::<IronBrick>(brick).is_none() {
                    continue;
                }
                let frozen_iron = world.has::<crate::ice::Frozen>(target)
                    && (world.has::<IronBrick>(target) || world.has::<IronScrap>(target));
                if !world.has::<SmallBall>(target) && !world.has::<Player>(target) && !frozen_iron {
                    continue;
                }
                let velocity = world
                    .get::<PreviousVelocity>(brick)
                    .map_or(Vec2::ZERO, |v| v.0);
                let speed = velocity.dot(normal);
                let speed = if frozen_iron {
                    let target_velocity = world
                        .get::<PreviousVelocity>(target)
                        .map_or(Vec2::ZERO, |v| v.0);
                    speed.min((velocity - target_velocity).dot(normal))
                } else {
                    speed
                };
                if speed >= BRICK_CRUSH_SPEED {
                    contacts
                        .entry((brick, target))
                        .and_modify(|s| *s = s.max(speed))
                        .or_insert(speed);
                }
            }
        }
    }
    let small_half = Vec2::splat(BALL_RADIUS * SMALL_BALL_SCALE);
    let mut smashed = Vec::new();
    let mut shattered = HashSet::new();
    let mut crushed_player = false;
    for &(brick, center, velocity) in &bricks {
        let hit_speed = |e, p, half| {
            contacts
                .get(&(brick, e))
                .copied()
                .or_else(|| impact(center, velocity, p, half))
        };
        let mut struck: Vec<(Entity, f32)> = world
            .query::<(Entity, &SmallBall, &Position)>()
            .filter_map(|(e, _, p)| Some((e, hit_speed(e, p.0, small_half)?)))
            .filter(|(e, _)| !smashed.contains(e))
            .collect();
        // Most head-on first; the id breaks ties so a pile smashes the same way each run.
        struck.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.id().cmp(&b.0.id())));
        if let Some(&(_, speed)) = struck.first() {
            let count = BRICK_BASE_SMASH + severity(speed, BRICK_SPEED_PER_BALL) - 1;
            struck.truncate(count.min(BRICK_SMASH_CAP));
        }
        if !struck.is_empty()
            && let Some(current) = world.get_mut::<Velocity>(brick)
        {
            current.0 = velocity * BRICK_SMASH_KEEP;
        }
        smashed.extend(struck.into_iter().map(|(e, _)| e));
        let frozen_hits: Vec<_> = world
            .query::<(Entity, &crate::ice::Frozen, &Position)>()
            .filter(|(e, _, _)| {
                *e != brick && (world.has::<IronBrick>(*e) || world.has::<IronScrap>(*e))
            })
            .filter_map(|(e, _, p)| {
                let half = if world.has::<IronBrick>(e) {
                    BRICK_HALF
                } else {
                    SCRAP_HALF
                };
                let speed = contacts.get(&(brick, e)).copied().or_else(|| {
                    let relative =
                        velocity - world.get::<PreviousVelocity>(e).map_or(Vec2::ZERO, |v| v.0);
                    Some(
                        impact(center, velocity, p.0, half)?
                            .min(impact(center, relative, p.0, half)?),
                    )
                })?;
                (speed >= BRICK_SHATTER_SPEED).then_some(e)
            })
            .collect();
        if !frozen_hits.is_empty() {
            if let Some(current) = world.get_mut::<Velocity>(brick) {
                current.0 = velocity * BRICK_SMASH_KEEP;
            }
            shattered.extend(frozen_hits);
        }
        let players: Vec<(Entity, f32)> = world
            .query::<(Entity, &Player, &Position)>()
            .filter_map(|(e, _, p)| Some((e, hit_speed(e, p.0, PLAYER_HALF)?)))
            .collect();
        for (player, speed) in players {
            let hearts = severity(speed, BRICK_SPEED_PER_HEART).min(u8::MAX as usize) as u8;
            crushed_player |= !crate::death::player_dead(world)
                && world
                    .get::<Health>(player)
                    .is_none_or(|h| h.immunity <= 0.0);
            damage_player(world, player, center, hearts);
        }
    }
    let mut shattered: Vec<_> = shattered.into_iter().collect();
    shattered.sort_by_key(|e| e.id());
    for &entity in &shattered {
        shatter_iron(world, entity, None);
    }
    if (crushed_player || !smashed.is_empty() || !shattered.is_empty())
        && world
            .get_resource::<EffectSounds>()
            .is_some_and(|s| s.crush_cooldown <= 0.0)
    {
        play_effect_sound(world, |s| s.crush);
        world
            .get_resource_mut::<EffectSounds>()
            .unwrap()
            .crush_cooldown = CRUSH_SFX_INTERVAL;
    }
    if smashed.is_empty() {
        return;
    }
    let positions: Vec<Vec2> = smashed
        .iter()
        .filter_map(|e| world.get::<Position>(*e).map(|p| p.0))
        .collect();
    for ball in smashed {
        world.despawn(ball);
    }
    // A few bursts spread over the set, as the black hole's steam puffs.
    let stride = positions.len().div_ceil(SMALL_BALL_BURSTS_PER_FRAME);
    for &position in positions.iter().step_by(stride) {
        spawn_transient_effect(world, "ex10_ball_smash", position);
    }
}
