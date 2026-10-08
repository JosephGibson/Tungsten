//! R places an iron brick at the cursor: a heavy box that shoves the balls
//! aside, barely yields to the player and crushes what it drives into.
use std::collections::HashSet;

use glam::Vec2;
use tungsten::Bundle;
use tungsten::core::{
    ActionMap, CameraState, Entity, EventQueue, InputState, Time, Transform, World,
};
use tungsten::physics::{
    Collider, CollisionEvent, Position, PrevPosition, RigidBody, RigidBodyBundle, Velocity,
};

use crate::gameplay::{PreviousVelocity, damage_player};
use crate::state::{
    BALL_RADIUS, PLAYER_HALF, Player, SMALL_BALL_BURSTS_PER_FRAME, SMALL_BALL_SCALE, SmallBall,
};
use crate::systems::{cursor_to_world, spawn_transient_effect};

/// Four large-ball diameters on a side.
pub(crate) const BRICK_HALF: Vec2 = Vec2::splat(4.0 * BALL_RADIUS);
/// About 250 small balls' worth, a little over four times the former 60-ball
/// weight, so a blast that sends a ball far barely shifts it.
pub(crate) const BRICK_MASS: f32 = 250.0;
pub(crate) const BRICK_CAP: usize = 24;
/// Sliding friction while the brick rests on something, in pixels/second².
/// The player shoves it along slowly, a small fraction of walking speed;
/// balls barely move it.
pub(crate) const BRICK_FRICTION: f32 = 1000.0;
/// The brick's own speed into a body, pixels/second, from which it hurts
/// the player and smashes small balls; a resting or creeping brick does neither.
pub(crate) const BRICK_CRUSH_SPEED: f32 = 360.0;
/// Each further step of this much speed costs the player one more heart.
pub(crate) const BRICK_SPEED_PER_HEART: f32 = 600.0;
/// Each further step of this much speed smashes one more ball per step.
pub(crate) const BRICK_SPEED_PER_BALL: f32 = 120.0;
/// Share of its speed a brick keeps through a layer of balls it smashed:
/// glass gives way, so a fast brick plows on through layer after layer.
pub(crate) const BRICK_SMASH_KEEP: f32 = 0.75;
/// Speculative contacts stop the brick at a body's surface, not inside it.
const CONTACT_MARGIN: f32 = 3.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct IronBrick;

/// One brick per fresh press, at the cursor, while fewer than `BRICK_CAP` exist.
pub(crate) fn place_brick_system(world: &mut World) {
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
        .filter(|e| world.get::<IronBrick>(*e).is_some())
        .collect();
    for brick in supported {
        if let Some(velocity) = world.get_mut::<Velocity>(brick) {
            let speed = (velocity.0.x.abs() - BRICK_FRICTION * dt).max(0.0);
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

/// A brick moving at crushing speed damages the player it drives into and
/// smashes the small balls ahead of it, more of both the faster it goes.
/// It reads the velocity from before the step: by now the solver has
/// stopped it on whatever it hit. A brick that smashed balls gets back
/// `BRICK_SMASH_KEEP` of that velocity, so it carries on into the next layer.
pub(crate) fn brick_impacts(world: &mut World) {
    let bricks: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &IronBrick, &Position)>()
        .filter_map(|(e, _, p)| {
            let velocity = world.get::<PreviousVelocity>(e)?.0;
            (velocity.length() >= BRICK_CRUSH_SPEED).then_some((e, p.0, velocity))
        })
        .collect();
    let small_half = Vec2::splat(BALL_RADIUS * SMALL_BALL_SCALE);
    let mut smashed = Vec::new();
    for &(brick, center, velocity) in &bricks {
        let mut struck: Vec<(Entity, f32)> = world
            .query::<(Entity, &SmallBall, &Position)>()
            .filter_map(|(e, _, p)| Some((e, impact(center, velocity, p.0, small_half)?)))
            .filter(|(e, _)| !smashed.contains(e))
            .collect();
        // Most head-on first; the id breaks ties so a pile smashes the same way each run.
        struck.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.id().cmp(&b.0.id())));
        struck.truncate(severity(velocity.length(), BRICK_SPEED_PER_BALL));
        if !struck.is_empty()
            && let Some(current) = world.get_mut::<Velocity>(brick)
        {
            current.0 = velocity * BRICK_SMASH_KEEP;
        }
        smashed.extend(struck.into_iter().map(|(e, _)| e));
        let players: Vec<(Entity, f32)> = world
            .query::<(Entity, &Player, &Position)>()
            .filter_map(|(e, _, p)| Some((e, impact(center, velocity, p.0, PLAYER_HALF)?)))
            .collect();
        for (player, speed) in players {
            let hearts = severity(speed, BRICK_SPEED_PER_HEART).min(u8::MAX as usize) as u8;
            damage_player(world, player, center, hearts);
        }
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
