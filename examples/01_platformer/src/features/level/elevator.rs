//! The mine's elevator: the map's fifth moving platform, a cage resting in
//! its shaft's collar that carries the player to the mine's upper level,
//! the only way up there. It is the kit's two-way lift, its prefab's
//! `path_follower`: a step onto it by the player, the rider its rule's
//! filter takes, sends it to its other end after the rule's `board`
//! seconds; the rider it carried there holds it until it steps off; and at
//! the top it goes back down the rule's `back` seconds after its last rider
//! left. Its `departed` and `arrived` cues play its start and stop sounds.
//! 01's own rules for it stay here, on the kit's pause and turn-back, each
//! step before the follower's.
//!
//! It moves only while every controller standing on it reports it as its
//! ground and stands wholly over it with [`MARGIN`] to spare: the carry
//! stops a rider who overhangs its edge under the upper level's deck, where
//! the lift would rise on into it, and a loaded rider's still first step
//! finds no ground. A lift with such a rider is paused until its riders are
//! safe again.
//!
//! Core leaves a kinematic body's pairs unsolved, so it keeps the player out
//! of its deck itself, each step before anything else: a player inside the
//! deck by more than [`ON_TOP`] is set out above it, or below it when its
//! centre is under the deck's, the other side when a tile or a wall fills
//! that one; and a falling deck whose box, swept down by its eased path's
//! fastest step, meets a player under it turns back the way it came, to
//! the top, from where it comes down again once it has stood there alone,
//! so a player who stays under it keeps it up.

use glam::Vec2;
use serde::Deserialize;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Collider, Entity, Position, PrevPosition, Shape, Sprite, Time, Transform,
    Velocity, Visibility, World,
};
use tungsten::kit::{PathFollower, PlatformerController};
use tungsten::physics::{SpatialFilter, overlap};

use crate::features::enemies::WALL_LAYERS;
use crate::features::run::Run;
use crate::game::GAMEPLAY;

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

/// The `elevator` scene component, a marker with no fields: the deck 01's
/// own rules read ([`tick`]) and hang a cable from ([`look`]). Its motion,
/// its riders and its sounds are its prefab's `path_follower` and `cues`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lift {}

/// The run's lifts' decks, listed once at its start ([`start`]), so no
/// step queries for them.
#[derive(Debug, Clone, Default)]
pub struct Decks(Vec<Entity>);

/// At a run's start: the lifts' decks listed.
pub fn start(world: &mut World) {
    let decks = world
        .query::<(Entity, &Lift)>()
        .map(|(entity, _)| entity)
        .collect();
    world.insert_resource(Decks(decks));
}

/// The `at`th of the run's decks.
fn deck(world: &World, at: usize) -> Option<Entity> {
    world.get_resource::<Decks>()?.0.get(at).copied()
}

/// The cable's sprite, which its look keeps: not saved.
#[derive(Debug, Clone, Copy)]
pub struct CableLook(Entity);

/// A body's box: its min and max corners.
fn bounds(at: Vec2, collider: &Collider) -> Option<(Vec2, Vec2)> {
    let Shape::Aabb { half_extents } = collider.shape else {
        return None;
    };
    let centre = at + collider.offset;
    Some((centre - half_extents, centre + half_extents))
}

/// Whether any controller standing on `deck`, whose box is `(min, max)`,
/// is unsafe to move: past its margin, or not yet finding it as its ground.
fn riders(world: &World, deck: Entity, (min, max): (Vec2, Vec2)) -> bool {
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
        }
    }
    unsafe_
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

/// `elevator_tick`, in the intent phase before the path follower: for each
/// lift's deck, the player set out of it when inside it; a deck on its way
/// down turned back over the player ([`PathFollower::turn_back`]); then the
/// lift paused while a rider on it is unsafe to move
/// ([`PathFollower::set_paused`]). Its boarding, its return and its
/// arrivals are the follower's.
pub fn tick(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let player = world.get_resource::<Run>().map(|run| run.player);
    let mut next = 0;
    while let Some(deck) = deck(world, next) {
        next += 1;
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
            let falling = world
                .get::<PathFollower>(deck)
                .filter(|follower| follower.returning())
                .map(PathFollower::speed);
            if let Some(speed) = falling {
                // The eased path's fastest step, at π/2 of its mean speed.
                let reach = speed * std::f32::consts::FRAC_PI_2 * dt + ON_TOP;
                if under(world, player, deck_box, reach)
                    && let Some(follower) = world.get_mut::<PathFollower>(deck)
                {
                    follower.turn_back();
                }
            }
        }
        let held = riders(world, deck, deck_box);
        if let Some(follower) = world.get_mut::<PathFollower>(deck) {
            follower.set_paused(held);
        }
    }
}

/// `elevator_look`, before the frame's sprites: each elevator's cable from
/// the gantry's sheave down to its cage's ring.
pub fn look(world: &mut World) {
    let Some(sheave) = super::marker(world, SHEAVE_MARKER) else {
        return;
    };
    let mut next = 0;
    while let Some(deck) = deck(world, next) {
        next += 1;
        let Some(at) = world.get::<Transform>(deck).map(|at| at.position) else {
            continue;
        };
        let cable = world.get::<CableLook>(deck).copied();
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
