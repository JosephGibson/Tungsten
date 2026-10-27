//! The goblin sapper (01 (g)): a small, quick walker on the kit's
//! platformer controller, on the beetle's pattern, and a skittish one. On
//! patrol it walks its ground within its leash, every few seconds pausing
//! to look back or skipping in place; it turns at a wall, a ledge or a
//! powder keg, hops a crate, a rock or a step no higher than
//! [`HOP_CLEAR`], and steps down a drop no deeper than [`STEP_DOWN`]. It
//! cackles on sight. It keeps its distance: closer than
//! [`KEEP_OFF`] across it backs away, hopping what it can, farther than
//! [`CLOSE_IN`] it closes, and between them it rests facing the player
//! while its [`COOLDOWN`] runs; then, the player within [`THROW_RISE`] up
//! or down, it approaches. One `KitRng` draw picks a dash-in under
//! [`DASH_CHANCE`]: a crouch for [`DASH_TELL`], then a sprint at
//! [`DASH_SPEED`] for at most [`DASH_MOST`] that ends in its stab once
//! within the stab's reach. Else it winds up a throw for [`TELL_SECS`]: the
//! tell's draw picks a bomb under [`BOMB_CHANCE`], else a rock, which its
//! brain keeps as its strike; its attack throws that from its hand at the
//! courier's centre on a ballistic arc, then it recovers for [`RECOVER`].
//! A courier within [`STAB_REACH`] across and [`STAB_RISE`] up or down
//! gets its knife at once: a [`STAB_TELL`] tell, a stab of [`STAB_DAMAGE`]
//! in a box ahead of it, and a leap back. Below [`FLEE_SHARE`] of its
//! health it runs from a courier within [`FLEE_REACH`] across rather than
//! throw; burning, it panics, running from the player, flailing, until the
//! fire is out ([`panics`]).
//!
//! A rock is a stone body (`props`, a `PropKind::Rock`) on the world's
//! gravity, credited to the goblin through `Throws`: its first blow on a
//! live target lands [`ROCK_DAMAGE`] physical at the goblin's damage factor
//! at the throw, pushed [`ROCK_PUSH`] along its flight, from the goblin
//! while it lives; then it bounces, rolls and stays as any body does. A
//! bomb is a lit prop on the world's gravity that rolls and blows after its
//! fuse, credited to the goblin.

use glam::Vec2;
use tungsten::core::{Collider, Entity, PhysicsConfig, Position, Time, Velocity, World};
use tungsten::kit::{Health, KitRng, PlatformerController, StateMachine, Stats, Tags};

use super::components::{Brain, Enemy, EnemyKind, EnemyState, Strike, Throw, Throws};
use super::hop::{self, Hops, grounded};
use super::systems::{elapsed, face, go, player, rest, sees};
use super::{ARRIVED, centre, troll};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{BURNING, Hit, Kind, has_status};
use crate::features::props;
use crate::shared::{SaveId, spawn_burst};

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
/// Its hops by those, and no leap: a deep ledge stops it.
pub const HOPS: Hops = Hops {
    rise: HOP_RISE,
    clear: HOP_CLEAR,
    step_down: STEP_DOWN,
    leap: None,
};
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
/// The seconds after a throw's release or a dash-in's end before it may
/// approach again (V26), and the share of its approaches that are a
/// dash-in: one draw at each.
pub const COOLDOWN: f32 = 2.5;
pub const DASH_CHANCE: f32 = 1.0 / 3.0;
/// Its dash-in: its crouch, seconds; its sprint's speed, px/s, and the most
/// seconds it runs, its input locked; the scuff of dust at its heels.
pub const DASH_TELL: f32 = 0.3;
pub const DASH_SPEED: f32 = 560.0;
pub const DASH_MOST: f32 = 0.6;
pub const DASH_DUST: &str = "ex10_dash_dust";
/// Its panic's startled skip, px/s up.
pub const PANIC_HOP: f32 = 300.0;

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

/// Walks `goblin` along `heading` at `pace` by its [`HOPS`]
/// ([`hop::scurry`]). Whether it moves.
fn scurry(world: &mut World, goblin: Entity, heading: f32, pace: f32) -> bool {
    hop::scurry(world, goblin, heading, pace, &HOPS).moves()
}

/// A skip in place: a little hop straight up.
fn skip(world: &mut World, goblin: Entity) {
    if grounded(world, goblin)
        && let Some(controller) = world.get_mut::<PlatformerController>(goblin)
    {
        controller.launch(Vec2::new(0.0, -SKIP));
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
/// than [`CLOSE_IN`] it closes, each hopping what it can; else it rests
/// facing the player, and with its [`COOLDOWN`] spent and the player within
/// [`THROW_RISE`] up or down it approaches: one draw of the kit's stream, a
/// dash-in's tell under [`DASH_CHANCE`], else a throw's. It searches once
/// it loses sight.
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
    face(world, goblin, target.x);
    let cooling = world
        .get::<Brain>(goblin)
        .is_some_and(|brain| brain.cooldown > 0.0);
    if cooling || rise.abs() > THROW_RISE {
        return;
    }
    let dash = world
        .get_resource_mut::<KitRng>()
        .is_some_and(|rng| rng.unit() < DASH_CHANCE);
    if let Some(brain) = world.get_mut::<Brain>(goblin) {
        brain.strike = dash.then_some(Strike::Dash);
    }
    go(world, goblin, EnemyState::WindUp);
}

/// The strike its brain keeps.
fn strike(world: &World, goblin: Entity) -> Option<Strike> {
    world.get::<Brain>(goblin).and_then(|brain| brain.strike)
}

/// Its tell: it faces the player; a stab its chase chose stays, a dash-in
/// crouches with a snicker, else one draw of the kit's stream picks its
/// throw, a bomb under [`BOMB_CHANCE`], else a rock.
pub fn enter_windup(world: &mut World, goblin: Entity) {
    if let Some((_, target)) = player(world) {
        face(world, goblin, target.x);
    }
    match strike(world, goblin) {
        Some(Strike::Stab) => return,
        Some(Strike::Dash) => {
            if let Some(at) = centre(world, goblin) {
                audio::play(world, Sound::GoblinCrouched, at);
            }
            return;
        }
        _ => {}
    }
    let bomb = world
        .get_resource_mut::<KitRng>()
        .is_some_and(|rng| rng.unit() < BOMB_CHANCE);
    if let Some(brain) = world.get_mut::<Brain>(goblin) {
        brain.strike = Some(if bomb { Strike::Bomb } else { Strike::Rock });
    }
}

/// The tell lasts [`STAB_TELL`] for a stab, [`DASH_TELL`] for a dash-in,
/// else [`TELL_SECS`], then the attack.
pub fn windup(world: &mut World, goblin: Entity) {
    rest(world, goblin);
    let tell = match strike(world, goblin) {
        Some(Strike::Stab) => STAB_TELL,
        Some(Strike::Dash) => DASH_TELL,
        _ => TELL_SECS,
    };
    if elapsed(world, goblin) >= tell - 1e-4 {
        go(world, goblin, EnemyState::Attack);
    }
}

/// The attack: its stab, its dash-in's sprint, or its throw.
pub fn enter_attack(world: &mut World, goblin: Entity) {
    match strike(world, goblin) {
        Some(Strike::Stab) => stab(world, goblin),
        Some(Strike::Dash) => dash(world, goblin),
        _ => {
            throw(world, goblin);
        }
    }
}

/// The dash-in's sprint along its heading at [`DASH_SPEED`], its input
/// locked for [`DASH_MOST`], with its dust at its heels and its sound; a
/// body stops it dead.
fn dash(world: &mut World, goblin: Entity) {
    let heading = world.get::<Enemy>(goblin).map_or(1.0, |mine| mine.heading);
    if let Some(controller) = world.get_mut::<PlatformerController>(goblin) {
        controller.launch(Vec2::new(heading * DASH_SPEED, 0.0));
        controller.lock_input(DASH_MOST);
    }
    if let Some(at) = centre(world, goblin) {
        let heels = at + Vec2::new(0.0, EnemyKind::Goblin.info().half.y);
        spawn_burst(world, DASH_DUST, heels);
        audio::play(world, Sound::GoblinDashed, at);
    }
}

/// Ends its dash-in with `next` as its strike, `None` for its recovery, a
/// stab for its attack entered again; its [`COOLDOWN`] starts.
fn end_dash(world: &mut World, goblin: Entity, next: Option<Strike>) {
    if let Some(brain) = world.get_mut::<Brain>(goblin) {
        brain.strike = next;
        brain.cooldown = COOLDOWN;
    }
    let state = if next.is_some() {
        EnemyState::Attack
    } else {
        EnemyState::Recover
    };
    go(world, goblin, state);
}

/// The dash-in runs until a courier is within [`STAB_REACH`] across and
/// [`STAB_RISE`] up or down, where it stops and its attack starts again as
/// the stab, or for [`DASH_MOST`], then the recovery. The stab's clip runs
/// [`STAB_SECS`], then it spends its strike and leaps back into its
/// recovery; the throw's runs [`THROW_SECS`], then the recovery.
pub fn attack(world: &mut World, goblin: Entity) {
    rest(world, goblin);
    if strike(world, goblin) == Some(Strike::Dash) {
        let reached = match (player(world), centre(world, goblin)) {
            (Some((_, target)), Some(at)) => {
                (target.x - at.x).abs() <= STAB_REACH && (target.y - at.y).abs() <= STAB_RISE
            }
            _ => false,
        };
        if reached {
            // It stops where it reached, for the stab to land there.
            if let Some(controller) = world.get_mut::<PlatformerController>(goblin) {
                controller.launch(Vec2::ZERO);
            }
            end_dash(world, goblin, Some(Strike::Stab));
        } else if elapsed(world, goblin) >= DASH_MOST - 1e-4 {
            end_dash(world, goblin, None);
        }
        return;
    }
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
    if let Some(brain) = world.get_mut::<Brain>(goblin) {
        brain.cooldown = COOLDOWN;
    }
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

/// Its clip for `state`: its panic, in the air too; its stab's, its
/// dash-in's or its throw's tell, a rock or a lit bomb, by the strike its
/// brain keeps; the stab, the dash or the throw through its attack; a hop
/// while it is in the air; standing while alert or recovering; else
/// walking or standing as it moves.
#[must_use]
pub fn clip(world: &World, goblin: Entity, state: EnemyState) -> &'static str {
    match state {
        EnemyState::Panic => "goblin_panic",
        EnemyState::WindUp => match strike(world, goblin) {
            Some(Strike::Bomb) => "goblin_windup_bomb",
            Some(Strike::Stab) => "goblin_stab_windup",
            Some(Strike::Dash) => "goblin_dash_windup",
            _ => "goblin_windup",
        },
        EnemyState::Attack if strike(world, goblin) == Some(Strike::Stab) => "goblin_stab",
        EnemyState::Attack if strike(world, goblin) == Some(Strike::Dash) => "goblin_dash",
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

/// `goblin_panic`, in the intent phase after the hold and before the
/// machine: each goblin whose machine is not held, burning, neither in nor
/// bound for its panic, spends its strike and panics (V26).
pub fn panics(world: &mut World) {
    let burning: Vec<Entity> = world
        .query::<(Entity, &Enemy, &StateMachine<EnemyState>)>()
        .filter(|(_, enemy, machine)| {
            enemy.kind == EnemyKind::Goblin
                && !machine.is_held()
                && machine.state() != EnemyState::Panic
                && machine.pending() != Some(EnemyState::Panic)
        })
        .map(|(entity, ..)| entity)
        .filter(|&entity| has_status(world, entity, BURNING))
        .collect();
    for goblin in burning {
        if let Some(brain) = world.get_mut::<Brain>(goblin) {
            brain.strike = None;
        }
        go(world, goblin, EnemyState::Panic);
    }
}

/// The way from the player across, else its heading.
fn away(world: &World, goblin: Entity) -> f32 {
    let heading = world.get::<Enemy>(goblin).map_or(1.0, |mine| mine.heading);
    match (player(world), centre(world, goblin)) {
        (Some((_, target)), Some(at)) if (at.x - target.x).abs() > 1.0 => {
            (at.x - target.x).signum()
        }
        _ => heading,
    }
}

/// Its panic's start: a yelp and, from the ground, a startled skip away
/// from the player at its run speed and [`PANIC_HOP`] up, which a dash-in's
/// lock does not hold off, the lock then keeping it; in the air it keeps
/// its flight.
pub fn enter_panic(world: &mut World, goblin: Entity) {
    if let Some(at) = centre(world, goblin) {
        audio::play(world, Sound::GoblinYelped, at);
    }
    if !grounded(world, goblin) {
        return;
    }
    let away = away(world, goblin);
    if let Some(mine) = world.get_mut::<Enemy>(goblin) {
        mine.heading = away;
    }
    let run = world
        .get::<Stats>(goblin)
        .and_then(|stats| stats.get(Stats::MOVE_SPEED))
        .unwrap_or(0.0);
    if let Some(controller) = world.get_mut::<PlatformerController>(goblin) {
        controller.launch(Vec2::new(away * run, -PANIC_HOP));
    }
}

/// Its panic: while it burns it runs from the player at its full run,
/// hopping what it can, the other way once that stops, else it stands
/// flailing; in the air it keeps its run. The fire out, it chases on sight,
/// else searches.
pub fn panic(world: &mut World, goblin: Entity) {
    if !has_status(world, goblin, BURNING) {
        let next = if sees(world, goblin) {
            EnemyState::Chase
        } else {
            EnemyState::Search
        };
        go(world, goblin, next);
        return;
    }
    if !grounded(world, goblin) {
        return;
    }
    let away = away(world, goblin);
    if !scurry(world, goblin, away, 1.0) && !scurry(world, goblin, -away, 1.0) {
        rest(world, goblin);
    }
}
