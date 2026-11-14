//! The cave troll: a lanky, long-armed walker on the kit's platformer
//! controller, on a 128 px frame. It walks its ground and roars on sight.
//! Within [`SWIPE_RANGE`] of the player it draws its arm back for its clip
//! ([`SWIPE_TELL_CLIP`]) and rakes the box in front of it; within
//! [`SLAM_RANGE`] it raises its fists for its clip ([`SLAM_TELL_CLIP`]) with
//! a grunt, a dust ring growing
//! to the slam's reach about its fist's point, and slams the ground,
//! hurting every target within [`SLAM_REACH`] of its fist, shaking the
//! camera, kicking the bodies by their mass and shattering frozen things.
//! Its brain keeps the attack its tell chose. Farther off,
//! with an iron block, a bowling ball, a crate (burning ones too), a keg
//! whose fuse is unlit, or a frozen enemy or dummy within [`PICKUP_REACH`],
//! it lifts it over its head and throws it at the player on a ballistic
//! arc; [`Throws`] credits it with the body's impacts. A crate that falls
//! in while held leaves its hands empty. Its way clear ([`clear_way`]):
//! chasing, searching or going home it lifts what lies ahead and kicks
//! what it may not lift; it carries what it lifts until it throws it in a
//! chase or sets it down back on patrol.
//!
//! A held object keeps its entity and save ID: the kit's carry turns its
//! body a sensor and puts it at the troll's hands each step, centre over
//! centre at the troll's velocity ([`pick_up`]), and the throw or the drop
//! gives its body back. A troll that dies, freezes, is stunned or goes drops
//! what it holds, and a held dummy or foe slips out as it thaws.
//!
//! Its prefab holds the rest: regeneration that burning holds, no
//! knockback, and its own builds of the cold and the shock, a short
//! freeze and a stun past four times the others' needs; the chill's grade
//! slows a chilled troll more as its freeze's build fills, as it does every
//! freezable creature (`combat::systems::chill_grades`). The black hole
//! slows it ([`crate::features::toys`]) and its bar shows over its head
//! ([`super::draw`]).

use glam::Vec2;
use tungsten::core::{
    BodyKind, Collider, Entity, PhysicsConfig, Position, PrevPosition, RigidBody, Shape, Velocity,
    World,
};
use tungsten::kit::{
    Health, Hold, PlatformerController, Stats, TagFilter, carried_by, carrier_of, carry,
    lob_velocity,
};
use tungsten::physics::wake;

use super::components::{Brain, Enemy, EnemyKind, EnemyState, Holding, Strike, Throw, Throws};
use super::systems::{elapsed, face, go, pace, player, rest, rested, sees, start_rest, stride};
use super::{ARRIVED, WALK_SHOVE_MOST, centre, heading_of, hop};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{FROZEN, Hit, Kind, STUNNED, has_status};
use crate::features::dummy::components::Dummy;
use crate::features::elements::cascade;
use crate::features::props::{
    self,
    components::{Crate, Keg},
};
use crate::features::toys::components::{BowlingBall, IronBlock};
use crate::shared::{Saved, body_centre, spawn_burst};

/// Its patrol's and its search's share of its run speed.
pub const PATROL_PACE: f32 = 0.6;
pub const SEARCH_PACE: f32 = 0.8;
/// How near the player, across and up or down, its chase raises its fist.
pub const SLAM_RANGE: f32 = 140.0;
pub const SLAM_RISE: f32 = 120.0;
/// The slam's tell, its clip's length, its reach about the fist's point and
/// its push on what it hurts.
pub const SLAM_TELL_CLIP: &str = "ex10_troll_windup";
pub const SLAM_REACH: f32 = 160.0;
pub const SLAM_PUSH: f32 = 420.0;
/// How near the player, across and up or down, its chase swipes instead;
/// the swipe's tell, its box's half size and its centre from the body's
/// facing right, its physical damage at a factor of 1 and its push, out and
/// up.
pub const SWIPE_RANGE: f32 = 90.0;
pub const SWIPE_RISE: f32 = 80.0;
pub const SWIPE_TELL_CLIP: &str = "ex10_troll_swipe_windup";
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
/// Its walk, its prefab's `move_speed` stat and run speed: restated so a
/// save can move an older troll's base to it.
pub const MOVE_SPEED: f32 = 91.0;
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

/// A shape's half extents.
fn half(shape: Shape) -> Vec2 {
    shape.half_extents()
}

/// What `troll` holds: the body the kit's carry holds for it.
#[must_use]
pub fn held(world: &World, troll: Entity) -> Option<Entity> {
    carried_by(world, troll)
}

/// Whether a troll may lift `entity`: an iron block, a bowling ball, a
/// crate, burning or not, a keg whose fuse is unlit (a lit one it kicks),
/// or a live frozen enemy or dummy, not a troll, held or in flight.
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
    let unlit = world.has::<Keg>(entity) && !props::burning(world, entity);
    let thing = world.has::<IronBlock>(entity)
        || world.has::<BowlingBall>(entity)
        || world.has::<Crate>(entity)
        || unlit;
    (thing || frozen_target)
        && carrier_of(world, entity).is_none()
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
        .query::<(Entity, &Saved, &Position, &Collider)>()
        .filter_map(|(entity, id, position, collider)| {
            let distance = (position.0 + collider.offset).distance(at);
            (distance <= PICKUP_REACH && entity != troll).then_some((distance, id.id().0, entity))
        })
        .filter(|&(.., entity)| liftable(world, entity))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(.., entity)| entity)
}

/// `troll` lifts `object` over its head on the kit's carry ([`hold`]): its
/// body a sensor until the throw or the drop, the troll's [`Holding`]
/// naming it. Whether it did: a troll holds one thing at a time.
pub fn pick_up(world: &mut World, troll: Entity, object: Entity) -> bool {
    if held(world, troll).is_some() || carrier_of(world, object).is_some() {
        return false;
    }
    let Some(hold) = hold(world, object).filter(|_| world.has::<RigidBody>(object)) else {
        return false;
    };
    let thrown = world.get_resource_mut::<Throws>().is_some_and(|throws| {
        let before = throws.0.len();
        throws.0.retain(|throw| throw.body != object);
        throws.0.len() < before
    });
    if thrown {
        crate::features::elements::systems::end_throw(world, object);
    }
    if !carry(world, troll, object, hold) {
        return false;
    }
    world.insert(troll, Holding(object));
    true
}

/// How a troll holds `object`: the object's bottom at its body's `hands`
/// point over its head, set centre over centre so it holds facing either
/// way, let go when the troll freezes or is stunned, and a held dummy or
/// foe let go once it thaws. `None` for an object without a collider.
fn hold(world: &World, object: Entity) -> Option<Hold> {
    let body = super::body(world, EnemyKind::Troll);
    let hands = body.point("hands", 1.0)?;
    let below = half(world.get::<Collider>(object)?.shape).y;
    let hold = Hold::new(hands - body.centre - Vec2::new(0.0, below))
        .centred()
        .with_drop(TagFilter::new().any([FROZEN, STUNNED]));
    let target = world.has::<Dummy>(object) || world.has::<Enemy>(object);
    Some(if target {
        hold.with_held_drop(TagFilter::new().none([FROZEN]))
    } else {
        hold
    })
}

/// Lets `object` go from its troll's hands at `velocity`, the kit's release
/// giving its body back (a dynamic body takes the velocity and wakes, a
/// frozen enemy's controller a launch) and the troll's [`Holding`] gone, the
/// thrown controller's input locked for [`THROWN_LOCK`] unless it is a drop.
fn release(world: &mut World, object: Entity, velocity: Vec2) {
    let Some(troll) = carrier_of(world, object) else {
        return;
    };
    tungsten::kit::release(world, troll, velocity);
    if world
        .get::<Holding>(troll)
        .is_some_and(|holding| holding.0 == object)
    {
        world.remove_component::<Holding>(troll);
    }
    if velocity != Vec2::ZERO
        && let Some(controller) = world.get_mut::<PlatformerController>(object)
    {
        controller.lock_input(THROWN_LOCK);
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
    let feet = target.y + crate::features::player::body(world).half.y;
    Vec2::new(target.x, target.y.min(feet - below))
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
    let velocity = lob_velocity(from, to, flight(from.distance(to)), gravity);
    release(world, object, velocity);
    let throw = Throw {
        body: object,
        thrower: crate::shared::save_id(world, troll).map_or(0, |id| id.0),
        factor: factor(world, troll),
        left: THROW_SECS,
    };
    if let Some(throws) = world.get_resource_mut::<Throws>() {
        throws.0.push(throw);
    }
    crate::features::elements::systems::arm_throw(world, &throw);
    audio::play(world, Sound::TrollThrow, from);
    Some(object)
}

/// The point its fist meets the ground: its body's `fist` point as it
/// faces.
#[must_use]
pub fn fist(world: &World, troll: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(troll)?.0;
    let heading = heading_of(world, troll)?;
    Some(at + super::body(world, EnemyKind::Troll).point("fist", heading)?)
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
        // A body by its ice, a target while it lives.
        let body = !world.has::<Health>(entity);
        let frozen = (body || live) && has_status(world, entity, FROZEN);
        if frozen {
            cascade::shatter_at(world, entity);
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
/// its boom's rule shakes the camera by the listener's distance; its rest
/// starts, its prefab's `cooldown` (`systems::start_rest`), through which it
/// chases and swipes but slams not.
pub fn slam(world: &mut World, troll: Entity) {
    let Some(point) = fist(world, troll) else {
        return;
    };
    start_rest(world, troll);
    let damage = world
        .get::<Stats>(troll)
        .and_then(|stats| stats.get(Stats::DAMAGE))
        .unwrap_or(40.0);
    let mut near: Vec<(u64, Entity, Vec2)> = world
        .query::<(Entity, &Saved, &Position, &Collider)>()
        .filter_map(|(entity, id, position, collider)| {
            let at = position.0 + collider.offset;
            (at.distance(point) <= SLAM_REACH && entity != troll).then_some((id.id().0, entity, at))
        })
        .collect();
    near.sort_unstable_by_key(|&(id, ..)| id);
    let push = |at: Vec2| (at - point).normalize_or(Vec2::NEG_Y) * SLAM_PUSH;
    blow(world, troll, &near, damage, point, push);
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
    let heading = heading_of(world, troll)?;
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
    let heading = heading_of(world, troll).unwrap_or(1.0);
    let mut near: Vec<(u64, Entity, Vec2)> = world
        .query::<(Entity, &Saved, &Position, &Collider)>()
        .filter_map(|(entity, id, position, collider)| {
            let at = position.0 + collider.offset;
            (entity != troll && meets(at, collider.shape, box_at, SWIPE_HALF)).then_some((
                id.id().0,
                entity,
                at,
            ))
        })
        .collect();
    near.sort_unstable_by_key(|&(id, ..)| id);
    let damage = SWIPE_DAMAGE * factor(world, troll);
    let push = |_: Vec2| Vec2::new(SWIPE_PUSH.x * heading, SWIPE_PUSH.y);
    blow(world, troll, &near, damage, from, push);
    audio::play(world, Sound::TrollSwiped, box_at);
}

/// `troll_holds`, in the move phase after the kit's follow pass, which puts
/// each held object at its troll's hands at its troll's velocity or drops
/// it where it is, its body back at rest, once its troll is gone, dead,
/// frozen or stunned or a held enemy or dummy thawed ([`hold`]): a troll
/// whose [`Holding`] names a body the kit no longer holds for it loses it,
/// so a save never names a let-go body.
pub fn holds(world: &mut World) {
    let empty: Vec<Entity> = world
        .query::<(Entity, &Holding)>()
        .filter(|&(troll, holding)| carrier_of(world, holding.0) != Some(troll))
        .map(|(troll, _)| troll)
        .collect();
    for troll in empty {
        world.remove_component::<Holding>(troll);
    }
}

/// On patrol it walks its ground, turning at each end and at walls,
/// having set down what it carried home ([`set_down`]).
pub fn patrol(world: &mut World, troll: Entity) {
    if let Some(object) = held(world, troll) {
        set_down(world, troll, object);
    }
    pace(world, troll, PATROL_PACE);
}

/// Sets `object` down out of `troll`'s hands: from their height but behind
/// its back, at rest, so it falls to the ground where its patrol walks
/// away from it, not onto its head, too short a way to set a keg off.
fn set_down(world: &mut World, troll: Entity, object: Entity) {
    let heading = heading_of(world, troll).unwrap_or(1.0);
    if let (Some(at), Some(collider), Some(point)) = (
        centre(world, troll),
        world.get::<Collider>(object).copied(),
        world.get::<Position>(object).map(|position| position.0),
    ) {
        let behind = super::body(world, EnemyKind::Troll).half.x + half(collider.shape).x + 4.0;
        let moved = Vec2::new(at.x - heading * behind - collider.offset.x, point.y);
        if let Some(position) = world.get_mut::<Position>(object) {
            position.0 = moved;
        }
        if let Some(prev) = world.get_mut::<PrevPosition>(object) {
            prev.0 = moved;
        }
    }
    drop_held(world, object);
}

/// Clears its way along `heading` (V25): holding nothing, it lifts what it
/// may lift just ahead of it ([`hop::body_ahead`]); a body ahead it may not
/// lift, too heavy for a walker's shove ([`WALK_SHOVE_MOST`]), it kicks
/// along its heading by its blow's kick, [`KICK`] over its mass at most
/// [`KICK_MOST`] and [`KICK_RISE`] of it up, unless the body already moves
/// away faster, with its swipe's sound. Whether it lifted something.
fn clear_way(world: &mut World, troll: Entity, heading: f32) -> bool {
    let Some(body) = hop::body_ahead(world, troll, heading) else {
        return false;
    };
    if held(world, troll).is_none() && liftable(world, body) && pick_up(world, troll, body) {
        return true;
    }
    let Some(inv_mass) = world
        .get::<RigidBody>(body)
        .filter(|_| carrier_of(world, body).is_none())
        .map(|own| own.inv_mass)
    else {
        return false;
    };
    let kick = (KICK * inv_mass).min(KICK_MOST);
    let Some(velocity) = world.get_mut::<Velocity>(body) else {
        return false;
    };
    if inv_mass * WALK_SHOVE_MOST >= 1.0 || velocity.0.x * heading >= kick {
        return false;
    }
    velocity.0.x = heading * kick;
    velocity.0.y = velocity.0.y.min(-KICK_RISE * kick);
    wake(world, body);
    if let Some(at) = body_centre(world, body) {
        audio::play(world, Sound::TrollSwiped, at);
    }
    false
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
/// swipes and within [`SLAM_RANGE`] it slams, once its rest after the
/// last slam has run; else with something to lift within [`PICKUP_REACH`] it lifts
/// it and throws; else it walks at the player, clearing its way. It
/// searches once it loses sight.
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
    if grounded && rested(world, troll) && across <= SLAM_RANGE && rise <= SLAM_RISE {
        return tell(world, troll, target.x, Strike::Slam);
    }
    if grounded
        && let Some(object) = nearest_liftable(world, troll)
        && pick_up(world, troll, object)
    {
        return tell(world, troll, target.x, Strike::Throw);
    }
    let heading = (target.x - at.x).signum();
    if grounded && clear_way(world, troll, heading) {
        return tell(world, troll, target.x, Strike::Throw);
    }
    stride(world, troll, heading, 1.0);
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

/// Its tell, [`THROW_TELL`] for a throw, else its clip's length,
/// [`SWIPE_TELL_CLIP`] for a swipe and [`SLAM_TELL_CLIP`] for a slam; then
/// the attack.
pub fn windup(world: &mut World, troll: Entity) {
    rest(world, troll);
    let tell = match strike(world, troll) {
        Strike::Throw => THROW_TELL,
        Strike::Swipe => super::clip_secs(world, SWIPE_TELL_CLIP),
        _ => super::clip_secs(world, SLAM_TELL_CLIP),
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

/// Walks toward `point`'s column, clearing its way and carrying on with
/// what it lifts: its search's or its home's. Whether it is there, or can
/// go no nearer.
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
    clear_way(world, troll, across.signum());
    !stride(world, troll, across.signum(), SEARCH_PACE)
}
