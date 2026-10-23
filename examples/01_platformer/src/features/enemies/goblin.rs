//! The goblin sapper (01 (g)): a small, quick walker on the kit's
//! platformer controller, on the beetle's pattern, and a skittish one. On
//! patrol it walks its ground within its leash, every few seconds pausing
//! to look back or skipping in place; it turns at a wall, a ledge or a
//! powder keg, hops a crate, a rock or a step no higher than
//! [`HOP_CLEAR`], and steps down a drop no deeper than [`STEP_DOWN`]. It
//! cackles on sight. It keeps its distance: closer than
//! [`KEEP_OFF`] across it backs away, hopping what it can, farther than
//! [`CLOSE_IN`] it closes, and between them, seeing the player within
//! [`THROW_RISE`] up or down, it winds up a throw for [`TELL_SECS`]. The
//! tell's one `KitRng` draw picks a bomb under [`BOMB_CHANCE`], else a
//! rock, which its brain keeps as its strike; its attack throws that from
//! its hand at the courier's centre on a ballistic arc, then it recovers
//! for [`RECOVER`]. A courier within [`STAB_REACH`] across and
//! [`STAB_RISE`] up or down gets its knife instead: a [`STAB_TELL`] tell, a
//! stab of [`STAB_DAMAGE`] in a box ahead of it, and a leap back. Below
//! [`FLEE_SHARE`] of its health it runs from a courier within
//! [`FLEE_REACH`] across rather than throw.
//!
//! A rock is a stone body (`props`, a `PropKind::Rock`) on the world's
//! gravity, credited to the goblin through `Throws`: its first blow on a
//! live target lands [`ROCK_DAMAGE`] physical at the goblin's damage factor
//! at the throw, pushed [`ROCK_PUSH`] along its flight, from the goblin
//! while it lives; then it bounces, rolls and stays as any body does. A
//! bomb is a lit prop on the world's gravity that rolls and blows after its
//! fuse, credited to the goblin.

use glam::Vec2;
use tungsten::core::{
    BodyKind, Collider, Entity, PhysicsConfig, Position, RigidBody, Shape, Time, Velocity, World,
};
use tungsten::kit::{Health, KitRng, PlatformerController, Stats, Tags};
use tungsten::physics::{SpatialFilter, cast_ray, overlap};

use super::components::{Brain, Enemy, EnemyKind, EnemyState, Strike, Throw, Throws};
use super::systems::{blocked, elapsed, face, go, player, rest, sees, solid, stride};
use super::{ARRIVED, centre, troll};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{Hit, Kind};
use crate::features::props::{
    self,
    components::{Bomb, Keg},
};
use crate::shared::SaveId;

/// Its patrol's and its search's share of its run speed.
pub const PATROL_PACE: f32 = 0.6;
pub const SEARCH_PACE: f32 = 0.8;
/// How near the player, across, it backs away, and how far it closes in;
/// between them it throws, the player no more than [`THROW_RISE`] above or
/// below it.
pub const KEEP_OFF: f32 = 140.0;
pub const CLOSE_IN: f32 = 420.0;
pub const THROW_RISE: f32 = 200.0;
/// Its throw's tell, the throw's clip and its recovery, seconds.
pub const TELL_SECS: f32 = 0.45;
pub const THROW_SECS: f32 = 0.25;
pub const RECOVER: f32 = 0.9;
/// The share of its throws that are bombs: one draw at each tell.
pub const BOMB_CHANCE: f32 = 0.2;
/// Where its throwing hand lets go, from its sprite's top-left facing
/// right.
pub const HAND: Vec2 = Vec2::new(50.0, 30.0);
/// A rock's flight's seconds a pixel to the player, at least and at most,
/// on the world's gravity.
pub const ROCK_PACE: f32 = 1.0 / 650.0;
pub const ROCK_LEAST: f32 = 0.35;
pub const ROCK_MOST: f32 = 0.9;
/// A thrown rock's blow at a damage factor of 1, its push along its
/// flight, px/s, and the seconds its throw credits the goblin.
pub const ROCK_DAMAGE: f32 = 8.0;
pub const ROCK_PUSH: f32 = 260.0;
pub const ROCK_CREDIT: f32 = 2.0;
/// Its stab: how near across, and up or down, the courier draws it; its
/// tell and its clip, seconds; its blow at a damage factor of 1 and its
/// push, out and up; and its box, [`STAB_HALF`] about [`STAB_AT`] from its
/// body's centre, mirrored as it faces.
pub const STAB_REACH: f32 = 56.0;
pub const STAB_RISE: f32 = 40.0;
pub const STAB_TELL: f32 = 0.2;
pub const STAB_SECS: f32 = 0.22;
pub const STAB_DAMAGE: f32 = 6.0;
pub const STAB_PUSH: Vec2 = Vec2::new(320.0, -120.0);
pub const STAB_HALF: Vec2 = Vec2::new(20.0, 16.0);
pub const STAB_AT: Vec2 = Vec2::new(24.0, -4.0);
/// Its leap back after a stab, px/s back and up, and the seconds its input
/// stays locked through it.
pub const LEAP_BACK: Vec2 = Vec2::new(260.0, 520.0);
pub const LEAP_LOCK: f32 = 0.3;
/// A hop over a body or a step, px/s up, clearing about 72 px, its run
/// carrying it over once it is clear; the highest it hops, px over its
/// feet; and the deepest drop it steps down, px under them.
pub const HOP_RISE: f32 = 720.0;
pub const HOP_CLEAR: f32 = 64.0;
pub const STEP_DOWN: f32 = 96.0;
/// Its patrol's fidget: every [`FIDGET_EVERY`] seconds and a bit more by
/// its save ID, for [`FIDGET_PAUSE`] it stands looking back, or skips in
/// place at [`SKIP`] px/s up, in turn.
pub const FIDGET_EVERY: f32 = 2.6;
pub const FIDGET_PAUSE: f32 = 0.7;
pub const SKIP: f32 = 420.0;
/// Below this share of its health it runs from a courier within
/// [`FLEE_REACH`] across rather than throw.
pub const FLEE_SHARE: f32 = 0.3;
pub const FLEE_REACH: f32 = 260.0;

/// On patrol it walks its ground, turning at each end, at walls, at kegs
/// and at its leash and hopping what it can; every so often it fidgets,
/// standing to look back for [`FIDGET_PAUSE`] or skipping in place, in
/// turn, timed by its patrol's seconds, which its machine saves.
pub fn patrol(world: &mut World, goblin: Entity) {
    let Some(mine) = world.get::<Enemy>(goblin).copied() else {
        return;
    };
    if let (Some(leash), Some(at)) = (mine.leash, centre(world, goblin)) {
        let out = at.x - mine.home().x;
        if out.abs() >= leash
            && out.signum() == mine.heading
            && let Some(enemy) = world.get_mut::<Enemy>(goblin)
        {
            enemy.heading = -mine.heading;
        }
    }
    let dt = world.resource::<Time>().delta();
    let id = world.get::<SaveId>(goblin).map_or(0, |id| id.0);
    let every = FIDGET_EVERY + 0.4 * (id % 3) as f32;
    let t = elapsed(world, goblin);
    // The fidget's cycle now and a step before, each counted from its start.
    let cycle = |t: f32| (t.max(0.0) / every) as u32;
    let paused = |t: f32| t >= 0.0 && cycle(t) > 0 && t - cycle(t) as f32 * every < FIDGET_PAUSE;
    let (now, before) = (paused(t), paused(t - dt));
    let looking = cycle(t) % 2 == 0;
    if now && !before {
        if looking {
            // It looks back over its shoulder, turned round for the pause.
            if let Some(enemy) = world.get_mut::<Enemy>(goblin) {
                enemy.heading = -enemy.heading;
            }
        } else {
            skip(world, goblin);
        }
    }
    if before && !now && cycle(t - dt) % 2 == 0 {
        // The look's end: it turns back the way it walked.
        if let Some(enemy) = world.get_mut::<Enemy>(goblin) {
            enemy.heading = -enemy.heading;
        }
    }
    if now && looking {
        rest(world, goblin);
        return;
    }
    wander(world, goblin, PATROL_PACE);
}

/// Walks its ground at `pace` along its heading, the other way when that is
/// stopped, hopping what it can; in the air it keeps its run until it
/// lands.
fn wander(world: &mut World, goblin: Entity, pace: f32) {
    let Some(heading) = world.get::<Enemy>(goblin).map(|mine| mine.heading) else {
        return;
    };
    if !grounded(world, goblin) {
        return;
    }
    if !scurry(world, goblin, heading, pace) && !scurry(world, goblin, -heading, pace) {
        rest(world, goblin);
    }
}

/// A skip in place: a little hop straight up.
fn skip(world: &mut World, goblin: Entity) {
    if grounded(world, goblin)
        && let Some(controller) = world.get_mut::<PlatformerController>(goblin)
    {
        controller.launch(Vec2::new(0.0, -SKIP));
    }
}

/// Whether `goblin` stands on something.
fn grounded(world: &World, goblin: Entity) -> bool {
    world
        .get::<PlatformerController>(goblin)
        .is_some_and(PlatformerController::grounded)
}

/// What is ahead of a walking goblin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ahead {
    Clear,
    Hop,
    Step,
    Stop,
}

/// What lies ahead of `goblin` along `heading`: a body at its height that
/// is no keg or bomb, or a wall no higher than [`HOP_CLEAR`], with room
/// over it and ground beyond, to hop; a keg, a bomb, a higher wall or a
/// ledge with no ground within [`STEP_DOWN`] under its feet, to stop at;
/// a short drop to step down; else a clear way.
fn ahead(world: &World, goblin: Entity, heading: f32) -> Ahead {
    let Some(at) = centre(world, goblin) else {
        return Ahead::Stop;
    };
    let half = EnemyKind::Goblin.info().half;
    // From inside its own edge, so a body it already presses against counts.
    let probe = at + Vec2::new(heading * (half.x + 4.0), 0.0);
    let mut met = Vec::new();
    let shape = Shape::Aabb {
        half_extents: Vec2::new(8.0, half.y - 4.0),
    };
    let filter = SpatialFilter::new().with_tiles(false).excluding(goblin);
    overlap(world, shape, probe, &filter, &mut met);
    let body = met.into_iter().flatten().find(|&entity| {
        world
            .get::<RigidBody>(entity)
            .is_some_and(|body| body.kind == BodyKind::Dynamic)
    });
    if let Some(body) = body {
        let hop =
            !world.has::<Keg>(body) && !world.has::<Bomb>(body) && room(world, goblin, heading);
        return if hop { Ahead::Hop } else { Ahead::Stop };
    }
    if !blocked(world, goblin, heading) {
        return Ahead::Clear;
    }
    let wall = solid(world, at, Vec2::new(heading, 0.0), half.x + 8.0, goblin);
    if wall {
        return if room(world, goblin, heading) {
            Ahead::Hop
        } else {
            Ahead::Stop
        };
    }
    // A ledge: a short drop it steps down, a deep one it stops at.
    let toes = Vec2::new(at.x + heading * (half.x + 6.0), at.y);
    let filter = SpatialFilter::new().excluding(goblin);
    match cast_ray(world, toes, Vec2::Y, half.y + STEP_DOWN, &filter) {
        Some(_) => Ahead::Step,
        None => Ahead::Stop,
    }
}

/// Whether a hop along `heading` has room: nothing over its head for the
/// hop's height, nothing ahead [`HOP_CLEAR`] over its feet, and ground
/// beyond, no more than a tile down.
fn room(world: &World, goblin: Entity, heading: f32) -> bool {
    let Some(at) = centre(world, goblin) else {
        return false;
    };
    let half = EnemyKind::Goblin.info().half;
    let filter = SpatialFilter::new().excluding(goblin);
    let rise = HOP_RISE * HOP_RISE / 7200.0;
    let head = cast_ray(world, at, Vec2::NEG_Y, half.y + rise, &filter).is_none();
    let over = Vec2::new(at.x, at.y + half.y - HOP_CLEAR - 6.0);
    let clear = cast_ray(world, over, Vec2::new(heading, 0.0), half.x + 64.0, &filter).is_none();
    let beyond = Vec2::new(at.x + heading * (half.x + 72.0), over.y);
    let ground = cast_ray(
        world,
        beyond,
        Vec2::Y,
        HOP_CLEAR + half.y * 2.0 + 64.0,
        &filter,
    )
    .is_some();
    head && clear && ground
}

/// Hops along `heading`: up at [`HOP_RISE`], running at its full speed, so
/// its run carries it over once it is clear of what it hops.
fn hop(world: &mut World, goblin: Entity, heading: f32) -> bool {
    if !grounded(world, goblin) {
        return false;
    }
    if let Some(mine) = world.get_mut::<Enemy>(goblin) {
        mine.heading = heading;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(goblin) {
        controller.launch(Vec2::new(0.0, -HOP_RISE));
        controller.set_run(heading);
    }
    true
}

/// Walks `goblin` along `heading` at `pace` of its run speed, as a walker's
/// stride does, hopping a body or a low step ahead and stepping down a
/// short drop; stopped by a keg, a bomb, a high wall or a deep ledge, it
/// stands. Whether it moves.
pub fn scurry(world: &mut World, goblin: Entity, heading: f32, pace: f32) -> bool {
    match ahead(world, goblin, heading) {
        Ahead::Clear => stride(world, goblin, heading, pace),
        Ahead::Hop => hop(world, goblin, heading),
        Ahead::Step => {
            // Off the short drop, which the walk's ledge check refuses.
            if let Some(mine) = world.get_mut::<Enemy>(goblin) {
                mine.heading = heading;
            }
            if let Some(controller) = world.get_mut::<PlatformerController>(goblin) {
                controller.set_run(heading * pace);
            }
            true
        }
        Ahead::Stop => {
            rest(world, goblin);
            false
        }
    }
}

/// Its alert: it cackles.
pub fn cackle(world: &mut World, goblin: Entity) {
    if let Some(at) = centre(world, goblin) {
        audio::play(world, Sound::GoblinCackled, at);
    }
}

/// Whether `goblin` is down to [`FLEE_SHARE`] of its health or less.
fn hurt(world: &World, goblin: Entity) -> bool {
    world
        .get::<Health>(goblin)
        .is_some_and(|health| health.current() <= FLEE_SHARE * health.max())
}

/// The chase: a courier within [`STAB_REACH`] across and [`STAB_RISE`] up
/// or down gets its stab's tell; badly hurt, it runs from one within
/// [`FLEE_REACH`]; closer than [`KEEP_OFF`] across it backs away, farther
/// than [`CLOSE_IN`] it closes, each hopping what it can; else, the player
/// within [`THROW_RISE`] up or down, it faces the player and winds up a
/// throw. It searches once it loses sight.
pub fn chase(world: &mut World, goblin: Entity) {
    if !sees(world, goblin) {
        go(world, goblin, EnemyState::Search);
        return;
    }
    let (Some((_, target)), Some(at)) = (player(world), centre(world, goblin)) else {
        return;
    };
    if !grounded(world, goblin) {
        return;
    }
    let across = target.x - at.x;
    let rise = target.y - at.y;
    if across.abs() <= STAB_REACH && rise.abs() <= STAB_RISE {
        face(world, goblin, target.x);
        if let Some(brain) = world.get_mut::<Brain>(goblin) {
            brain.strike = Some(Strike::Stab);
        }
        go(world, goblin, EnemyState::WindUp);
        return;
    }
    if hurt(world, goblin)
        && across.abs() < FLEE_REACH
        && scurry(world, goblin, -across.signum(), 1.0)
    {
        return;
    }
    if across.abs() < KEEP_OFF && scurry(world, goblin, -across.signum(), 1.0) {
        return;
    }
    if across.abs() > CLOSE_IN {
        scurry(world, goblin, across.signum(), 1.0);
        return;
    }
    rest(world, goblin);
    if rise.abs() <= THROW_RISE {
        face(world, goblin, target.x);
        go(world, goblin, EnemyState::WindUp);
    }
}

/// The strike its brain keeps.
fn strike(world: &World, goblin: Entity) -> Option<Strike> {
    world.get::<Brain>(goblin).and_then(|brain| brain.strike)
}

/// Its tell: it faces the player; a stab its chase chose stays, else one
/// draw of the kit's stream picks its strike, a bomb under
/// [`BOMB_CHANCE`], else a rock.
pub fn enter_windup(world: &mut World, goblin: Entity) {
    if let Some((_, target)) = player(world) {
        face(world, goblin, target.x);
    }
    if strike(world, goblin) == Some(Strike::Stab) {
        return;
    }
    let bomb = world
        .get_resource_mut::<KitRng>()
        .is_some_and(|rng| rng.unit() < BOMB_CHANCE);
    if let Some(brain) = world.get_mut::<Brain>(goblin) {
        brain.strike = Some(if bomb { Strike::Bomb } else { Strike::Rock });
    }
}

/// The tell lasts [`STAB_TELL`] for a stab, else [`TELL_SECS`], then the
/// attack.
pub fn windup(world: &mut World, goblin: Entity) {
    rest(world, goblin);
    let tell = if strike(world, goblin) == Some(Strike::Stab) {
        STAB_TELL
    } else {
        TELL_SECS
    };
    if elapsed(world, goblin) >= tell - 1e-4 {
        go(world, goblin, EnemyState::Attack);
    }
}

/// The attack: its stab, or its throw.
pub fn enter_attack(world: &mut World, goblin: Entity) {
    if strike(world, goblin) == Some(Strike::Stab) {
        stab(world, goblin);
    } else {
        throw(world, goblin);
    }
}

/// The stab's clip runs [`STAB_SECS`], then it spends its strike and leaps
/// back into its recovery; the throw's runs [`THROW_SECS`], then the
/// recovery.
pub fn attack(world: &mut World, goblin: Entity) {
    rest(world, goblin);
    let stabbing = strike(world, goblin) == Some(Strike::Stab);
    let clip = if stabbing { STAB_SECS } else { THROW_SECS };
    if elapsed(world, goblin) < clip - 1e-4 {
        return;
    }
    if stabbing {
        if let Some(brain) = world.get_mut::<Brain>(goblin) {
            brain.strike = None;
        }
        leap_back(world, goblin);
    }
    go(world, goblin, EnemyState::Recover);
}

/// A leap back from where it faces: up and away at [`LEAP_BACK`], its input
/// locked for [`LEAP_LOCK`].
fn leap_back(world: &mut World, goblin: Entity) {
    let heading = world.get::<Enemy>(goblin).map_or(1.0, |mine| mine.heading);
    if let Some(controller) = world.get_mut::<PlatformerController>(goblin) {
        controller.launch(Vec2::new(-heading * LEAP_BACK.x, -LEAP_BACK.y));
        controller.lock_input(LEAP_LOCK);
    }
}

/// Where its throwing hand lets go: [`HAND`], mirrored as it faces.
#[must_use]
pub fn hand(world: &World, goblin: Entity) -> Option<Vec2> {
    let at = centre(world, goblin)?;
    let heading = world.get::<Enemy>(goblin)?.heading;
    let info = EnemyKind::Goblin.info();
    Some(at + Vec2::new((HAND.x - info.centre.x) * heading, HAND.y - info.centre.y))
}

/// The stab's box: [`STAB_HALF`] about [`STAB_AT`] from its body's centre,
/// mirrored as it faces.
#[must_use]
pub fn stab_box(world: &World, goblin: Entity) -> Option<Vec2> {
    let at = centre(world, goblin)?;
    let heading = world.get::<Enemy>(goblin)?.heading;
    Some(at + Vec2::new(STAB_AT.x * heading, STAB_AT.y))
}

/// The stab: every live target but an enemy whose body meets the box ahead
/// of it takes [`STAB_DAMAGE`] physical at its damage factor, pushed out
/// and up by [`STAB_PUSH`]. Its sound.
pub fn stab(world: &mut World, goblin: Entity) {
    let Some(box_at) = stab_box(world, goblin) else {
        return;
    };
    let heading = world.get::<Enemy>(goblin).map_or(1.0, |mine| mine.heading);
    let mut near: Vec<(u64, Entity, Vec2)> = world
        .query::<(Entity, &SaveId, &Position, &Collider, &Health)>()
        .filter(|&(entity, .., health)| entity != goblin && !health.is_dead())
        .filter(|&(entity, ..)| {
            !world
                .get::<Tags>(entity)
                .is_some_and(|tags| tags.has("team.enemy"))
        })
        .filter_map(|(entity, id, position, collider, _)| {
            let at = position.0 + collider.offset;
            troll::meets(at, collider.shape, box_at, STAB_HALF).then_some((id.0, entity, at))
        })
        .collect();
    near.sort_unstable_by_key(|&(id, ..)| id);
    let damage = STAB_DAMAGE * factor(world, goblin);
    let push = Vec2::new(STAB_PUSH.x * heading, STAB_PUSH.y);
    for (_, target, at) in near {
        Hit::new(target, [(Kind::Physical, damage)])
            .at(at)
            .push(push)
            .from(goblin)
            .queue(world);
    }
    audio::play(world, Sound::GoblinStabbed, box_at);
}

/// `goblin`'s damage factor: its `damage` stat over its base.
fn factor(world: &World, goblin: Entity) -> f32 {
    world.get::<Stats>(goblin).map_or(1.0, |stats| {
        match (stats.get(Stats::DAMAGE), stats.base(Stats::DAMAGE)) {
            (Some(damage), Some(base)) if base > 0.0 => damage / base,
            _ => 1.0,
        }
    })
}

/// Its throw from its hand at the courier's centre on the world's gravity,
/// with its sound: the strike its tell chose, which it spends. A rock, a
/// stone body, flies a flight of its distance at [`ROCK_PACE`], held to
/// [`ROCK_LEAST`]–[`ROCK_MOST`], its blow credited to the goblin at its
/// factor now for [`ROCK_CREDIT`]; a bomb flies the troll's throw's flight,
/// lit, credited to the goblin. What it threw.
pub fn throw(world: &mut World, goblin: Entity) -> Option<Entity> {
    let strike = world
        .get_mut::<Brain>(goblin)
        .and_then(|brain| brain.strike.take())
        .unwrap_or(Strike::Rock);
    let (_, target) = player(world)?;
    let from = hand(world, goblin)?;
    let thrower = world.get::<SaveId>(goblin).map(|id| id.0);
    let gravity = world
        .get_resource::<PhysicsConfig>()
        .map_or(Vec2::ZERO, |config| config.gravity);
    let thrown = if strike == Strike::Bomb {
        let velocity =
            troll::ballistic(from, target, troll::flight(from.distance(target)), gravity);
        let bomb = props::spawn_bomb(world, from, thrower);
        if let Some(own) = world.get_mut::<Velocity>(bomb) {
            own.0 = velocity;
        }
        bomb
    } else {
        let time = (from.distance(target) * ROCK_PACE).clamp(ROCK_LEAST, ROCK_MOST);
        let velocity = troll::ballistic(from, target, time, gravity);
        let rock = props::lob_rock(world, from, velocity);
        let throw = Throw {
            body: rock,
            thrower: thrower.unwrap_or_default(),
            factor: factor(world, goblin),
            left: ROCK_CREDIT,
        };
        if let Some(throws) = world.get_resource_mut::<Throws>() {
            throws.0.push(throw);
        }
        rock
    };
    audio::play(world, Sound::RockThrown, from);
    Some(thrown)
}

/// Walks toward `point`'s column, hopping what it can: its search's or its
/// home's. Whether it is there, or can go no nearer.
pub fn seek(world: &mut World, goblin: Entity, point: Option<Vec2>) -> bool {
    let (Some(point), Some(at)) = (point, centre(world, goblin)) else {
        rest(world, goblin);
        return true;
    };
    let across = point.x - at.x;
    if across.abs() <= ARRIVED {
        rest(world, goblin);
        return true;
    }
    if !grounded(world, goblin) {
        return false;
    }
    !scurry(world, goblin, across.signum(), SEARCH_PACE)
}

/// Its clip for `state`: its stab's or its throw's tell, a rock or a lit
/// bomb, by the strike its brain keeps; the stab or the throw through its
/// attack; a hop while it is in the air; standing while alert or
/// recovering; else walking or standing as it moves.
#[must_use]
pub fn clip(world: &World, goblin: Entity, state: EnemyState) -> &'static str {
    match state {
        EnemyState::WindUp => match strike(world, goblin) {
            Some(Strike::Bomb) => "goblin_windup_bomb",
            Some(Strike::Stab) => "goblin_stab_windup",
            _ => "goblin_windup",
        },
        EnemyState::Attack if strike(world, goblin) == Some(Strike::Stab) => "goblin_stab",
        EnemyState::Attack => "goblin_throw",
        _ if !grounded(world, goblin) => "goblin_hop",
        EnemyState::Alert | EnemyState::Recover => "goblin_idle",
        _ => {
            let moving = world
                .get::<Velocity>(goblin)
                .is_some_and(|velocity| velocity.0.x.abs() > 1.0);
            if moving { "goblin_walk" } else { "goblin_idle" }
        }
    }
}
