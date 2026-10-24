//! The cave troll: a lanky, long-armed walker on the kit's platformer
//! controller, on a 128 px frame. It walks its ground and roars on sight.
//! Within [`SWIPE_RANGE`] of the player it draws its arm back for
//! [`SWIPE_TELL`] and rakes the box in front of it; within [`SLAM_RANGE`]
//! it raises its fists for [`SLAM_TELL`] with a grunt, a dust ring growing
//! to the slam's reach about its fist's point, and slams the ground,
//! hurting every target within [`SLAM_REACH`] of its fist, shaking the
//! camera, kicking the bodies by their mass and shattering frozen things.
//! Its brain keeps the attack its tell chose. Farther off,
//! with an iron block, a bowling ball, a crate (burning ones too), or a
//! frozen enemy or dummy within [`PICKUP_REACH`], it lifts it over its head
//! and throws it at the player on a ballistic arc; [`Throws`] credits it
//! with the body's impacts. A crate that falls in while held leaves its
//! hands empty.
//!
//! A held object keeps its entity and save ID: its body turns a sensor,
//! which [`holds`] puts at the troll's hands before each step at the
//! troll's velocity, and the throw or the drop gives its body back. A troll
//! that dies, freezes, is stunned or goes drops what it holds.
//!
//! Its prefab holds the rest: regeneration that burning holds, no
//! knockback, and its own rungs on the cold and shock ladders, a short
//! freeze and a stun past four times the others' needs; the chill's grade
//! slows a chilled troll more as its bucket fills, as it does every
//! freezable creature (`combat::systems::chill_grades`). The black hole
//! slows it ([`crate::features::toys`]) and its bar shows over its head
//! ([`super::draw`]).

use glam::Vec2;
use tungsten::core::{
    BodyKind, Collider, Entity, EventQueue, PhysicsConfig, Position, PrevPosition, RigidBody,
    ShakeEvent, Shape, Time, Velocity, World,
};
use tungsten::kit::{Health, PlatformerController, Stats};
use tungsten::physics::wake;

use super::components::{Brain, Enemy, EnemyKind, EnemyState, Held, Strike, Throw, Throws};
use super::systems::{elapsed, face, go, pace, player, rest, sees, stride};
use super::{ARRIVED, centre};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{FROZEN, Hit, Kind, STUNNED, has_status};
use crate::features::dummy::components::Dummy;
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::components::Frozen;
use crate::features::player::PLAYER_HALF;
use crate::features::props::components::Crate;
use crate::features::toys::components::{BowlingBall, IronBlock};
use crate::features::toys::systems::falloff;
use crate::shared::{SaveId, body_centre, spawn_burst};

/// Its patrol's and its search's share of its run speed.
pub const PATROL_PACE: f32 = 0.6;
pub const SEARCH_PACE: f32 = 0.8;
/// How near the player, across and up or down, its chase raises its fist.
pub const SLAM_RANGE: f32 = 140.0;
pub const SLAM_RISE: f32 = 120.0;
/// The slam's tell, its reach about the fist's point and its push on what
/// it hurts; its shake by the courier's distance, whole within the first
/// and none past the second.
pub const SLAM_TELL: f32 = 0.8;
pub const SLAM_REACH: f32 = 160.0;
pub const SLAM_PUSH: f32 = 420.0;
pub const SLAM_SHAKE: f32 = 0.5;
pub const SHAKE_NEAR: f32 = 120.0;
pub const SHAKE_FAR: f32 = 640.0;
/// How near the player, across and up or down, its chase swipes instead;
/// the swipe's tell, its box's half size and its centre from the body's
/// facing right, its physical damage at a factor of 1 and its push, out and
/// up.
pub const SWIPE_RANGE: f32 = 90.0;
pub const SWIPE_RISE: f32 = 80.0;
pub const SWIPE_TELL: f32 = 0.3;
pub const SWIPE_HALF: Vec2 = Vec2::new(40.0, 36.0);
pub const SWIPE_AT: Vec2 = Vec2::new(52.0, -10.0);
pub const SWIPE_DAMAGE: f32 = 18.0;
pub const SWIPE_PUSH: Vec2 = Vec2::new(560.0, -200.0);
/// The kick the slam and the swipe give a body, px/s: this over its mass,
/// at most [`KICK_MOST`], away and [`KICK_RISE`] of it up.
pub const KICK: f32 = 2400.0;
pub const KICK_MOST: f32 = 650.0;
pub const KICK_RISE: f32 = 0.6;
/// Its recovery after an attack, its own.
pub const RECOVER: f32 = 0.35;
/// The seconds from a slam's landing before it may slam again; it chases
/// and swipes meanwhile.
pub const SLAM_COOLDOWN: f32 = 2.0;
/// Its walk, its prefab's `move_speed` stat and run speed: restated so a
/// save can move an older troll's base to it.
pub const MOVE_SPEED: f32 = 91.0;
/// Where its fist meets the ground, from its body's centre facing right.
pub const FIST: Vec2 = Vec2::new(56.0, 50.0);
/// How far from its centre it reaches for something to throw, the throw's
/// tell, and the seconds a throw credits it.
pub const PICKUP_REACH: f32 = 200.0;
pub const THROW_TELL: f32 = 0.6;
pub const THROW_SECS: f32 = 2.0;
/// A throw's flight: its seconds a pixel to the player, at least and at
/// most.
pub const THROW_PACE: f32 = 1.0 / 650.0;
pub const THROW_LEAST: f32 = 0.45;
pub const THROW_MOST: f32 = 1.2;
/// How long a thrown enemy's controller keeps its input locked.
pub const THROWN_LOCK: f32 = 1.0;
/// The seconds its attack's clip runs before the recovery.
pub const ATTACK_SECS: f32 = 0.4;
/// Where it holds what it lifts, from its sprite's top-left: the object's
/// bottom at its hands, over its head.
pub const HANDS: Vec2 = Vec2::new(74.0, 2.0);

/// A shape's half extents.
fn half(shape: Shape) -> Vec2 {
    match shape {
        Shape::Aabb { half_extents } => half_extents,
        Shape::Circle { radius } => Vec2::splat(radius),
    }
}

/// What `troll` holds.
#[must_use]
pub fn held(world: &World, troll: Entity) -> Option<Entity> {
    world
        .query::<(Entity, &Held)>()
        .find(|(_, hold)| hold.by == troll)
        .map(|(entity, _)| entity)
}

/// Whether a troll may lift `entity`: an iron block, a bowling ball, a
/// crate, burning or not, or a live frozen enemy or dummy, not a troll,
/// held or in flight; never a keg.
fn liftable(world: &World, entity: Entity) -> bool {
    let target = world.has::<Dummy>(entity)
        || world
            .get::<Enemy>(entity)
            .is_some_and(|enemy| enemy.kind != EnemyKind::Troll);
    let frozen_target = target
        && has_status(world, entity, FROZEN)
        && world
            .get::<Health>(entity)
            .is_some_and(|health| !health.is_dead());
    let thing = world.has::<IronBlock>(entity)
        || world.has::<BowlingBall>(entity)
        || world.has::<Crate>(entity);
    (thing || frozen_target)
        && !world.has::<Held>(entity)
        && world
            .get_resource::<Throws>()
            .is_none_or(|throws| throws.of(entity).is_none())
}

/// The nearest thing `troll` may lift within [`PICKUP_REACH`] of its
/// centre, the lower save ID on a tie.
#[must_use]
pub fn nearest_liftable(world: &World, troll: Entity) -> Option<Entity> {
    let at = centre(world, troll)?;
    // The reach first, then what may be lifted (P6).
    world
        .query::<(Entity, &SaveId, &Position, &Collider)>()
        .filter_map(|(entity, id, position, collider)| {
            let distance = (position.0 + collider.offset).distance(at);
            (distance <= PICKUP_REACH && entity != troll).then_some((distance, id.0, entity))
        })
        .filter(|&(.., entity)| liftable(world, entity))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(.., entity)| entity)
}

/// `troll` lifts `object` over its head: its body a sensor until the throw
/// or the drop. Whether it did: a troll holds one thing at a time.
pub fn pick_up(world: &mut World, troll: Entity, object: Entity) -> bool {
    if held(world, troll).is_some() || world.has::<Held>(object) {
        return false;
    }
    let Some(body) = world.get::<RigidBody>(object).copied() else {
        return false;
    };
    if let Some(throws) = world.get_resource_mut::<Throws>() {
        throws.0.retain(|throw| throw.body != object);
    }
    world.insert(object, Held { by: troll, body });
    if let Some(own) = world.get_mut::<RigidBody>(object) {
        *own = RigidBody::sensor();
    }
    place(world, troll, object);
    if let Some(at) = world.get::<Position>(object).map(|at| at.0)
        && let Some(prev) = world.get_mut::<PrevPosition>(object)
    {
        prev.0 = at;
    }
    true
}

/// Puts `object` at `troll`'s hands at `troll`'s velocity.
fn place(world: &mut World, troll: Entity, object: Entity) {
    let (Some(at), Some(collider)) = (
        world.get::<Position>(troll).map(|at| at.0),
        world.get::<Collider>(object).copied(),
    ) else {
        return;
    };
    let centre = at + HANDS - Vec2::new(0.0, half(collider.shape).y);
    let velocity = world.get::<Velocity>(troll).map_or(Vec2::ZERO, |v| v.0);
    if let Some(position) = world.get_mut::<Position>(object) {
        position.0 = centre - collider.offset;
    }
    if let Some(own) = world.get_mut::<Velocity>(object) {
        own.0 = velocity;
    }
}

/// Lets `object` go at `velocity` with its own body back: a dynamic body
/// takes the velocity, a frozen enemy's controller a launch, its input
/// locked for [`THROWN_LOCK`] unless it is a drop.
fn release(world: &mut World, object: Entity, velocity: Vec2) {
    let Some(hold) = world.get::<Held>(object).copied() else {
        return;
    };
    world.remove_component::<Held>(object);
    if let Some(body) = world.get_mut::<RigidBody>(object) {
        *body = hold.body;
    }
    if let Some(own) = world.get_mut::<Velocity>(object) {
        own.0 = velocity;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(object) {
        controller.launch(velocity);
        if velocity != Vec2::ZERO {
            controller.lock_input(THROWN_LOCK);
        }
    }
    if hold.body.kind == BodyKind::Dynamic {
        wake(world, object);
    }
}

/// Drops `object` where it is, at rest.
pub fn drop_held(world: &mut World, object: Entity) {
    release(world, object, Vec2::ZERO);
}

/// `troll`'s damage factor: its `damage` stat over its base.
pub(crate) fn factor(world: &World, troll: Entity) -> f32 {
    world.get::<Stats>(troll).map_or(1.0, |stats| {
        match (stats.get(Stats::DAMAGE), stats.base(Stats::DAMAGE)) {
            (Some(damage), Some(base)) if base > 0.0 => damage / base,
            _ => 1.0,
        }
    })
}

/// The point a thrown `object`'s centre flies to: the player's centre, its
/// bottom no lower than the player's feet.
fn aim(world: &World, object: Entity, target: Vec2) -> Vec2 {
    let below = world
        .get::<Collider>(object)
        .map_or(0.0, |collider| half(collider.shape).y);
    let feet = target.y + PLAYER_HALF.y;
    Vec2::new(target.x, target.y.min(feet - below))
}

/// The velocity that carries a body from `from` to `to` in `time` under
/// `gravity`.
#[must_use]
pub fn ballistic(from: Vec2, to: Vec2, time: f32, gravity: Vec2) -> Vec2 {
    (to - from) / time - 0.5 * gravity * time
}

/// A throw's seconds in the air over `distance`.
#[must_use]
pub fn flight(distance: f32) -> f32 {
    (distance * THROW_PACE).clamp(THROW_LEAST, THROW_MOST)
}

/// `troll` throws what it holds at the player on a ballistic arc under the
/// world's gravity, or drops it with no player; [`Throws`] credits it with
/// the body's impacts for [`THROW_SECS`] at its damage factor now. The body
/// thrown.
pub fn throw(world: &mut World, troll: Entity) -> Option<Entity> {
    let object = held(world, troll)?;
    let from = body_centre(world, object)?;
    let gravity = world
        .get_resource::<PhysicsConfig>()
        .map_or(Vec2::ZERO, |config| config.gravity);
    let Some((_, target)) = player(world) else {
        drop_held(world, object);
        return Some(object);
    };
    let to = aim(world, object, target);
    let velocity = ballistic(from, to, flight(from.distance(to)), gravity);
    release(world, object, velocity);
    let throw = Throw {
        body: object,
        thrower: world.get::<SaveId>(troll).map_or(0, |id| id.0),
        factor: factor(world, troll),
        left: THROW_SECS,
    };
    if let Some(throws) = world.get_resource_mut::<Throws>() {
        throws.0.push(throw);
    }
    audio::play(world, Sound::TrollThrow, from);
    Some(object)
}

/// The point its fist meets the ground, as it faces.
#[must_use]
pub fn fist(world: &World, troll: Entity) -> Option<Vec2> {
    let at = centre(world, troll)?;
    let heading = world.get::<Enemy>(troll)?.heading;
    Some(at + Vec2::new(FIST.x * heading, FIST.y))
}

/// What a blow of the troll's does to each saved thing in `near`, in
/// save-ID order, each with its body's centre: a live target takes
/// `damage` physical from `troll` with `push(at)`; a frozen thing shatters;
/// any other dynamic body is kicked away from `from` by its mass,
/// [`KICK`] over it at most [`KICK_MOST`], and [`KICK_RISE`] of that up.
fn blow(
    world: &mut World,
    troll: Entity,
    near: &[(u64, Entity, Vec2)],
    damage: f32,
    from: Vec2,
    push: impl Fn(Vec2) -> Vec2,
) {
    for &(_, entity, at) in near {
        let live = world
            .get::<Health>(entity)
            .is_some_and(|health| !health.is_dead());
        if live {
            Hit::new(entity, [(Kind::Physical, damage)])
                .at(at)
                .push(push(at))
                .from(troll)
                .queue(world);
        }
        let frozen = world.has::<Frozen>(entity) || (live && has_status(world, entity, FROZEN));
        if frozen {
            cascade::queue(world, CascadeKind::Shatter, Some(entity), at, at, 0.0);
            continue;
        }
        let Some(body) = world.get::<RigidBody>(entity).copied() else {
            continue;
        };
        if live || body.kind != BodyKind::Dynamic {
            continue;
        }
        let kick = (KICK * body.inv_mass).min(KICK_MOST);
        let away = (at - from).normalize_or(Vec2::NEG_Y);
        if let Some(velocity) = world.get_mut::<Velocity>(entity) {
            velocity.0 += away * kick + Vec2::new(0.0, -KICK_RISE * kick);
        }
        wake(world, entity);
    }
}

/// The slam: every live target but the troll within [`SLAM_REACH`] of its
/// fist takes its `damage` as physical, pushed away; every other body in
/// reach is kicked away by its mass; every frozen thing in reach shatters;
/// the camera shakes by the courier's distance; its [`SLAM_COOLDOWN`]
/// starts.
pub fn slam(world: &mut World, troll: Entity) {
    let Some(point) = fist(world, troll) else {
        return;
    };
    if let Some(brain) = world.get_mut::<Brain>(troll) {
        brain.slam_cooldown = SLAM_COOLDOWN;
    }
    let damage = world
        .get::<Stats>(troll)
        .and_then(|stats| stats.get(Stats::DAMAGE))
        .unwrap_or(40.0);
    let mut near: Vec<(u64, Entity, Vec2)> = world
        .query::<(Entity, &SaveId, &Position, &Collider)>()
        .filter_map(|(entity, id, position, collider)| {
            let at = position.0 + collider.offset;
            (at.distance(point) <= SLAM_REACH && entity != troll).then_some((id.0, entity, at))
        })
        .collect();
    near.sort_unstable_by_key(|&(id, ..)| id);
    let push = |at: Vec2| (at - point).normalize_or(Vec2::NEG_Y) * SLAM_PUSH;
    blow(world, troll, &near, damage, point, push);
    if let Some((_, courier)) = player(world) {
        let trauma = SLAM_SHAKE * falloff(courier.distance(point), SHAKE_NEAR, SHAKE_FAR);
        if trauma > 0.0
            && let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>()
        {
            queue.send(ShakeEvent { trauma_add: trauma });
        }
    }
    spawn_burst(world, "ex10_blast_dust", point);
    audio::play(world, Sound::TrollSlam, point);
}

/// Whether a body of `shape` about `centre` meets the box of `half` about
/// `at`.
pub(crate) fn meets(centre: Vec2, shape: Shape, at: Vec2, half: Vec2) -> bool {
    ((centre - at).abs() - (half + self::half(shape))).max_element() < 0.0
}

/// The swipe's box: [`SWIPE_HALF`] about [`SWIPE_AT`] from its body's
/// centre, mirrored as it faces.
#[must_use]
pub fn swipe_box(world: &World, troll: Entity) -> Option<Vec2> {
    let at = centre(world, troll)?;
    let heading = world.get::<Enemy>(troll)?.heading;
    Some(at + Vec2::new(SWIPE_AT.x * heading, SWIPE_AT.y))
}

/// The swipe: every live target but the troll whose body meets the box in
/// front of it takes [`SWIPE_DAMAGE`] physical at its damage factor, pushed
/// out and up, dropped on one that stands firm; every frozen thing there
/// shatters; every other body there is kicked away from the troll by its
/// mass. Its sound.
pub fn swipe(world: &mut World, troll: Entity) {
    let (Some(box_at), Some(from)) = (swipe_box(world, troll), centre(world, troll)) else {
        return;
    };
    let heading = world.get::<Enemy>(troll).map_or(1.0, |mine| mine.heading);
    let mut near: Vec<(u64, Entity, Vec2)> = world
        .query::<(Entity, &SaveId, &Position, &Collider)>()
        .filter_map(|(entity, id, position, collider)| {
            let at = position.0 + collider.offset;
            (entity != troll && meets(at, collider.shape, box_at, SWIPE_HALF))
                .then_some((id.0, entity, at))
        })
        .collect();
    near.sort_unstable_by_key(|&(id, ..)| id);
    let damage = SWIPE_DAMAGE * factor(world, troll);
    let push = |_: Vec2| Vec2::new(SWIPE_PUSH.x * heading, SWIPE_PUSH.y);
    blow(world, troll, &near, damage, from, push);
    audio::play(world, Sound::TrollSwiped, box_at);
}

/// `troll_holds`, in the move phase after the movers: each held object
/// goes to its troll's hands at its troll's velocity, since a sensor moves
/// by its velocity in the step; one whose troll is gone, dead, frozen or
/// stunned, or a held enemy or dummy that thawed, drops where it is, its
/// body back at rest.
pub fn holds(world: &mut World) {
    let mut held: Vec<(Option<SaveId>, Entity, Entity)> = world
        .query::<(Entity, &Held)>()
        .map(|(entity, hold)| (world.get::<SaveId>(entity).copied(), entity, hold.by))
        .collect();
    held.sort_unstable_by_key(|&(id, ..)| id);
    for (_, object, troll) in held {
        let able = world.is_alive(troll)
            && world
                .get::<Health>(troll)
                .is_some_and(|health| !health.is_dead())
            && !has_status(world, troll, FROZEN)
            && !has_status(world, troll, STUNNED);
        let target = world.has::<Dummy>(object) || world.has::<Enemy>(object);
        let thawed = target && !has_status(world, object, FROZEN);
        if able && !thawed {
            place(world, troll, object);
        } else {
            drop_held(world, object);
        }
    }
}

/// On patrol it walks its ground, turning at each end and at walls.
pub fn patrol(world: &mut World, troll: Entity) {
    pace(world, troll, PATROL_PACE);
}

/// Its alert: it roars.
pub fn roar(world: &mut World, troll: Entity) {
    if let Some(at) = centre(world, troll) {
        audio::play(world, Sound::TrollRoar, at);
    }
}

/// Ends the chase's step in the tell of `strike`, facing `x`, its brain
/// keeping the strike.
fn tell(world: &mut World, troll: Entity, x: f32, strike: Strike) {
    face(world, troll, x);
    if let Some(brain) = world.get_mut::<Brain>(troll) {
        brain.strike = Some(strike);
    }
    go(world, troll, EnemyState::WindUp);
    rest(world, troll);
}

/// The chase: holding something, it throws; within [`SWIPE_RANGE`] it
/// swipes and within [`SLAM_RANGE`] it slams, once its [`SLAM_COOLDOWN`]
/// has run; else with something to lift within [`PICKUP_REACH`] it lifts
/// it and throws; else it walks at the player. It searches once it loses
/// sight.
pub fn chase(world: &mut World, troll: Entity) {
    if !sees(world, troll) {
        go(world, troll, EnemyState::Search);
        return;
    }
    let (Some((_, target)), Some(at)) = (player(world), centre(world, troll)) else {
        return;
    };
    if held(world, troll).is_some() {
        return tell(world, troll, target.x, Strike::Throw);
    }
    let grounded = world
        .get::<PlatformerController>(troll)
        .is_some_and(PlatformerController::grounded);
    let (across, rise) = ((target.x - at.x).abs(), (target.y - at.y).abs());
    if grounded && across <= SWIPE_RANGE && rise <= SWIPE_RISE {
        return tell(world, troll, target.x, Strike::Swipe);
    }
    let rested = world
        .get::<Brain>(troll)
        .is_none_or(|brain| brain.slam_cooldown <= 0.0);
    if grounded && rested && across <= SLAM_RANGE && rise <= SLAM_RISE {
        return tell(world, troll, target.x, Strike::Slam);
    }
    if grounded
        && let Some(object) = nearest_liftable(world, troll)
        && pick_up(world, troll, object)
    {
        return tell(world, troll, target.x, Strike::Throw);
    }
    stride(world, troll, (target.x - at.x).signum(), 1.0);
}

/// The strike `troll`'s tell chose: its brain's, else, for an older save's
/// brain, the throw with something over its head and the slam without.
#[must_use]
pub fn strike(world: &World, troll: Entity) -> Strike {
    match world.get::<Brain>(troll).and_then(|brain| brain.strike) {
        Some(strike) => strike,
        None if held(world, troll).is_some() => Strike::Throw,
        None => Strike::Slam,
    }
}

/// Its tell's start: the slam's grunt.
pub fn enter_windup(world: &mut World, troll: Entity) {
    if strike(world, troll) == Strike::Slam
        && let Some(at) = centre(world, troll)
    {
        audio::play(world, Sound::TrollGrunted, at);
    }
}

/// Its tell, [`THROW_TELL`] for a throw, [`SWIPE_TELL`] for a swipe and
/// [`SLAM_TELL`] for a slam; then the attack.
pub fn windup(world: &mut World, troll: Entity) {
    rest(world, troll);
    let tell = match strike(world, troll) {
        Strike::Throw => THROW_TELL,
        Strike::Swipe => SWIPE_TELL,
        _ => SLAM_TELL,
    };
    if elapsed(world, troll) >= tell - 1e-4 {
        go(world, troll, EnemyState::Attack);
    }
}

/// The attack: its swipe, or the throw of what it holds, else its slam,
/// which its brain keeps as the strike it made.
pub fn enter_attack(world: &mut World, troll: Entity) {
    if strike(world, troll) == Strike::Swipe {
        swipe(world, troll);
    } else if throw(world, troll).is_none() {
        if let Some(brain) = world.get_mut::<Brain>(troll) {
            brain.strike = Some(Strike::Slam);
        }
        slam(world, troll);
    }
}

/// The attack's clip runs [`ATTACK_SECS`], then the recovery, its strike
/// spent.
pub fn attack(world: &mut World, troll: Entity) {
    rest(world, troll);
    if elapsed(world, troll) >= ATTACK_SECS - 1e-4 {
        if let Some(brain) = world.get_mut::<Brain>(troll) {
            brain.strike = None;
        }
        go(world, troll, EnemyState::Recover);
    }
}

/// Walks toward `point`'s column: its search's or its home's. Whether it is
/// there, or can go no nearer.
pub fn seek(world: &mut World, troll: Entity, point: Option<Vec2>) -> bool {
    let (Some(point), Some(at)) = (point, centre(world, troll)) else {
        rest(world, troll);
        return true;
    };
    let across = point.x - at.x;
    if across.abs() <= ARRIVED {
        rest(world, troll);
        return true;
    }
    !stride(world, troll, across.signum(), SEARCH_PACE)
}

/// Its clip for `state`: over its head what it holds; the throw, the swipe
/// or the slam through its attack, by the strike its tell chose, the throw
/// too after a lift (`shown`, its last clip); the swipe's or the slam's
/// wind-up through its tell; standing while it recovers, roars or stands
/// still; else its walk.
#[must_use]
pub fn clip(world: &World, troll: Entity, state: EnemyState, shown: &str) -> &'static str {
    let lifting = shown.contains("troll_pickup") || shown.contains("troll_throw");
    let strike = world.get::<Brain>(troll).and_then(|brain| brain.strike);
    match state {
        EnemyState::Attack => match strike {
            Some(Strike::Swipe) => "troll_swipe",
            Some(Strike::Throw) => "troll_throw",
            Some(_) => "troll_slam",
            None if lifting => "troll_throw",
            None => "troll_slam",
        },
        _ if held(world, troll).is_some() => "troll_pickup",
        EnemyState::WindUp if strike == Some(Strike::Swipe) => "troll_swipe_windup",
        EnemyState::WindUp => "troll_windup",
        EnemyState::Alert | EnemyState::Recover => "troll_idle",
        _ => {
            let moving = world
                .get::<Velocity>(troll)
                .is_some_and(|velocity| velocity.0.x.abs() > 1.0);
            if moving { "troll_walk" } else { "troll_idle" }
        }
    }
}

/// `troll_cooldowns`, in the intent phase before the enemies' states: each
/// troll's slam cooldown runs down by the step.
pub fn cooldowns(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    for (enemy, brain) in world.query_mut::<(&Enemy, &mut Brain)>() {
        if enemy.kind == EnemyKind::Troll && brain.slam_cooldown > 0.0 {
            brain.slam_cooldown = (brain.slam_cooldown - dt).max(0.0);
        }
    }
}
