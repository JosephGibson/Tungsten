//! The mine's elevator: the map's fifth moving platform, a cage resting in
//! its shaft's collar that carries the player to the mine's upper level,
//! the only way up there. A step onto it, its rider's
//! [`ground`](PlatformerController::ground) turning to it, sends it to the
//! other stop after [`BOARD_SECS`], on a fresh stop-mode path follower; at
//! the top it goes back down [`RETURN_SECS`] after its rider leaves.
//!
//! It moves only while every controller standing on it reports it as its
//! ground and stands wholly over it with [`MARGIN`] to spare: the carry
//! moves a rider uncast after its own move, so a rider past an edge would
//! be lifted into the upper level's deck, and a loaded rider's still first
//! step finds no ground. Such a trip holds until its riders are safe again.
//!
//! Core leaves a kinematic body's pairs unsolved, so it keeps the player out
//! of its deck itself, each step before anything else: a player inside the
//! deck by more than [`ON_TOP`] is set out above it, or below it when its
//! centre is under the deck's, the other side when a tile or a wall fills
//! that one; and a falling deck whose box, swept down by its eased path's
//! fastest step, meets a player under it turns back to the top, from where
//! it comes down again [`RETURN_SECS`] later, so a player who stays under
//! it keeps it up.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Collider, Entity, Position, PrevPosition, Shape, Sprite, Time, Transform,
    Velocity, Visibility, World,
};
use tungsten::kit::{PathFollower, PlatformerController};
use tungsten::physics::{SpatialFilter, overlap};

use crate::features::audio::{self, components::Sound};
use crate::features::enemies::WALL_LAYERS;
use crate::features::run::Run;
use crate::game::GAMEPLAY;

/// Seconds a rider stands on it before it goes, and seconds it waits at the
/// top after its rider left before it goes back down.
pub const BOARD_SECS: f32 = 0.3;
pub const RETURN_SECS: f32 = 4.0;
/// How far inside its edges every rider's body must stand: one step's run
/// at the player's speed and some.
pub const MARGIN: f32 = 12.0;
/// How near its top a body's feet stand on it, and how deep inside it a
/// player may be before it is set out.
pub const ON_TOP: f32 = 2.0;
/// The map's marker where the cable leaves the gantry's sheave.
pub const SHEAVE_MARKER: &str = "lift_sheave";
/// The cable's sprite, a rope down its middle, and its depth: behind the
/// cage.
const CABLE: &str = "ex10_lift_cable";
const CABLE_Z: i32 = -5;
/// Where the cable takes the cage: its crossbar's ring, from its top-left.
const RING: Vec2 = Vec2::new(96.0, 4.0);

/// The `elevator` scene component: how far it rises and how fast, in world
/// pixels and pixels a second.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ElevatorDef {
    pub rise: f32,
    pub speed: f32,
}

/// Where the elevator stands or goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stop {
    Bottom,
    Top,
    Rising,
    Falling,
}

/// The elevator's state, which a save keeps: its stop, a rider's boarding
/// seconds left (none while nobody boards), the seconds it has stood at the
/// top unridden, whether the player stood on it last step and whether a
/// trip holds for an unsafe rider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elevator {
    pub stop: Stop,
    pub board: Option<f32>,
    pub idle: f32,
    pub ridden: bool,
    pub held: bool,
    /// Its point at rest and at the top, and its speed.
    pub bottom: Vec2,
    pub top: Vec2,
    pub speed: f32,
}

/// The cable's sprite, which its look keeps: not saved.
#[derive(Debug, Clone, Copy)]
pub struct CableLook(Entity);

/// At a run's start: each elevator at rest at the bottom.
pub fn start(world: &mut World) {
    let lifts: Vec<(Entity, ElevatorDef, Vec2)> = world
        .query::<(Entity, &ElevatorDef, &Position)>()
        .map(|(entity, def, at)| (entity, *def, at.0))
        .collect();
    for (entity, def, bottom) in lifts {
        world.insert(
            entity,
            Elevator {
                stop: Stop::Bottom,
                board: None,
                idle: 0.0,
                ridden: false,
                held: false,
                bottom,
                top: bottom - Vec2::new(0.0, def.rise),
                speed: def.speed,
            },
        );
    }
}

/// A body's box: its min and max corners.
fn bounds(at: Vec2, collider: &Collider) -> Option<(Vec2, Vec2)> {
    let Shape::Aabb { half_extents } = collider.shape else {
        return None;
    };
    let centre = at + collider.offset;
    Some((centre - half_extents, centre + half_extents))
}

/// Who stands on `deck`, whose box is `(min, max)`: whether the player
/// stands wholly on it, and whether any controller on it is unsafe to move,
/// past its margin or not yet finding it as its ground.
fn riders(world: &World, deck: Entity, (min, max): (Vec2, Vec2)) -> (bool, bool) {
    let player = world.get_resource::<Run>().map(|run| run.player);
    let mut player_on = false;
    let mut unsafe_ = false;
    for (entity, controller, at, collider) in
        world.query::<(Entity, &PlatformerController, &Position, &Collider)>()
    {
        if entity == deck {
            continue;
        }
        let Some((low, high)) = bounds(at.0, collider) else {
            continue;
        };
        let grounded = controller.ground() == Some(deck);
        let on_top = (high.y - min.y).abs() <= ON_TOP && low.x < max.x && high.x > min.x;
        if !grounded && !on_top {
            continue;
        }
        let wholly = low.x >= min.x + MARGIN && high.x <= max.x - MARGIN;
        if !grounded || !wholly {
            unsafe_ = true;
        } else if Some(entity) == player {
            player_on = true;
        }
    }
    (player_on, unsafe_)
}

/// Whether a tile or a wall fills `collider`'s box at `at`, a pixel in from
/// each side: the tiles alone, then the walls' layers, two queries, since a
/// mask filters the tiles too.
fn filled(world: &World, at: Vec2, collider: &Collider) -> bool {
    let Shape::Aabb { half_extents } = collider.shape else {
        return false;
    };
    let shape = Shape::Aabb {
        half_extents: (half_extents - Vec2::ONE).max(Vec2::ZERO),
    };
    let centre = at + collider.offset;
    let mut met = Vec::new();
    [
        SpatialFilter::new().with_bodies(false),
        SpatialFilter::new().with_mask(WALL_LAYERS),
    ]
    .iter()
    .any(|filter| {
        overlap(world, shape, centre, filter, &mut met);
        !met.is_empty()
    })
}

/// Sets `player` out of the deck whose box is `(min, max)` when its body is
/// inside it by more than [`ON_TOP`]: above when its centre is above the
/// deck's, else below, the other side when a tile or a wall fills the
/// first; its point and its last point moved alike, its velocity stopped.
fn set_out(world: &mut World, player: Entity, (min, max): (Vec2, Vec2)) {
    let (Some(at), Some(collider)) = (
        world.get::<Position>(player).map(|at| at.0),
        world.get::<Collider>(player).copied(),
    ) else {
        return;
    };
    let Some((low, high)) = bounds(at, &collider) else {
        return;
    };
    let across = high.x.min(max.x) - low.x.max(min.x);
    let deep = high.y.min(max.y) - low.y.max(min.y);
    if across <= 0.0 || deep <= ON_TOP {
        return;
    }
    let up = Vec2::new(0.0, min.y - high.y);
    let down = Vec2::new(0.0, max.y - low.y);
    let (first, other) = if low.y + high.y < min.y + max.y {
        (up, down)
    } else {
        (down, up)
    };
    let to = if filled(world, at + first, &collider) {
        at + other
    } else {
        at + first
    };
    if let Some(position) = world.get_mut::<Position>(player) {
        position.0 = to;
    }
    if let Some(previous) = world.get_mut::<PrevPosition>(player) {
        previous.0 = to;
    }
    if let Some(velocity) = world.get_mut::<Velocity>(player) {
        velocity.0 = Vec2::ZERO;
    }
}

/// Whether `player`'s body is under the deck whose box is `(min, max)`,
/// within `reach` of its underside: past its top by more than [`ON_TOP`],
/// so not a rider on it.
fn under(world: &World, player: Entity, (min, max): (Vec2, Vec2), reach: f32) -> bool {
    let (Some(at), Some(collider)) = (world.get::<Position>(player), world.get::<Collider>(player))
    else {
        return false;
    };
    bounds(at.0, collider).is_some_and(|(low, high)| {
        low.x < max.x && high.x > min.x && low.y < max.y + reach && high.y > min.y + ON_TOP
    })
}

/// Sends `deck` from where it stands toward `to`, a fresh follower.
fn go(world: &mut World, deck: Entity, to: Vec2, speed: f32) {
    let Some(from) = world.get::<Position>(deck).map(|at| at.0) else {
        return;
    };
    if from.distance(to) < 0.01 {
        return;
    }
    let follower =
        PathFollower::new([from, to], speed).with_easing(tungsten::core::Easing::SineInOut);
    world.insert(deck, follower);
}

/// Stops `deck`: no follower, no velocity.
fn halt(world: &mut World, deck: Entity) {
    world.remove_component::<PathFollower>(deck);
    if let Some(velocity) = world.get_mut::<Velocity>(deck) {
        velocity.0 = Vec2::ZERO;
    }
}

/// `elevator_tick`, in the intent phase before the path follower: the
/// player set out of a deck it is inside, a falling deck turned back over
/// the player, then boards, returns, holds and arrivals.
pub fn tick(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let player = world.get_resource::<Run>().map(|run| run.player);
    let lifts: Vec<(Entity, Elevator)> = world
        .query::<(Entity, &Elevator)>()
        .map(|(entity, lift)| (entity, *lift))
        .collect();
    for (deck, mut lift) in lifts {
        let (Some(at), Some(collider)) = (
            world.get::<Position>(deck).map(|at| at.0),
            world.get::<Collider>(deck).copied(),
        ) else {
            continue;
        };
        let Some(deck_box) = bounds(at, &collider) else {
            continue;
        };
        if let Some(player) = player {
            set_out(world, player, deck_box);
            // The eased path's fastest step, at π/2 of its mean speed.
            let reach = lift.speed * std::f32::consts::FRAC_PI_2 * dt + ON_TOP;
            if lift.stop == Stop::Falling && under(world, player, deck_box, reach) {
                halt(world, deck);
                go(world, deck, lift.top, lift.speed);
                lift.stop = Stop::Rising;
                lift.held = false;
                lift.board = None;
                lift.idle = 0.0;
                audio::play(world, Sound::LiftStarted, at);
            }
        }
        let (player_on, unsafe_) = riders(world, deck, deck_box);
        match lift.stop {
            Stop::Bottom | Stop::Top => {
                let at_top = lift.stop == Stop::Top;
                if player_on && !lift.ridden {
                    lift.board = Some(BOARD_SECS);
                }
                if !player_on {
                    lift.board = None;
                    if at_top {
                        if lift.ridden {
                            lift.idle = 0.0;
                        }
                        lift.idle += dt;
                    }
                }
                let boarded = lift.board.is_some_and(|left| left - dt <= 1e-5);
                lift.board = lift.board.map(|left| left - dt);
                let returns = at_top && !player_on && lift.idle >= RETURN_SECS - 1e-4;
                if (boarded || returns) && !unsafe_ {
                    let (stop, to) = if at_top {
                        (Stop::Falling, lift.bottom)
                    } else {
                        (Stop::Rising, lift.top)
                    };
                    go(world, deck, to, lift.speed);
                    lift.stop = stop;
                    lift.board = None;
                    lift.idle = 0.0;
                    audio::play(world, Sound::LiftStarted, at);
                }
            }
            Stop::Rising | Stop::Falling => {
                let to = if lift.stop == Stop::Rising {
                    lift.top
                } else {
                    lift.bottom
                };
                if unsafe_ {
                    if !lift.held {
                        halt(world, deck);
                        lift.held = true;
                    }
                } else if lift.held {
                    go(world, deck, to, lift.speed);
                    lift.held = false;
                }
                let arrived = world
                    .get::<PathFollower>(deck)
                    .map_or(at.distance(to) < 0.5, PathFollower::is_finished);
                if arrived && !lift.held {
                    halt(world, deck);
                    lift.stop = if lift.stop == Stop::Rising {
                        Stop::Top
                    } else {
                        Stop::Bottom
                    };
                    lift.idle = 0.0;
                    audio::play(world, Sound::LiftStopped, at);
                }
            }
        }
        lift.ridden = player_on;
        if let Some(own) = world.get_mut::<Elevator>(deck) {
            *own = lift;
        }
    }
}

/// `elevator_look`, before the frame's sprites: each elevator's cable from
/// the gantry's sheave down to its cage's ring.
pub fn look(world: &mut World) {
    let Some(sheave) = super::marker(world, SHEAVE_MARKER) else {
        return;
    };
    let lifts: Vec<(Entity, Vec2, Option<CableLook>)> = world
        .query::<(Entity, &Elevator, &Transform, Option<&CableLook>)>()
        .map(|(entity, _, transform, cable)| (entity, transform.position, cable.copied()))
        .collect();
    for (deck, at, cable) in lifts {
        let ring = at + RING;
        let length = (ring.y - sheave.y).max(1.0);
        let cable = match cable {
            Some(CableLook(entity)) => entity,
            None => {
                let Some(asset) = world
                    .get_resource_mut::<AssetRegistry>()
                    .map(|assets| assets.intern_sprite(CABLE))
                else {
                    return;
                };
                let mut sprite = Sprite::new(asset);
                sprite.z_order = CABLE_Z;
                let entity = world.spawn_with((
                    Transform::default(),
                    sprite,
                    Visibility { visible: true },
                    SceneEntity { state_id: GAMEPLAY },
                ));
                world.insert(deck, CableLook(entity));
                entity
            }
        };
        if let Some(transform) = world.get_mut::<Transform>(cable) {
            transform.position = Vec2::new(ring.x - 32.0, sheave.y);
            transform.scale = Vec2::new(1.0, length / 64.0);
        }
    }
}
