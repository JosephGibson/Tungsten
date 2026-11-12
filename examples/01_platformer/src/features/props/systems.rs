//! The props' systems: the round props' roll, the heat between the props,
//! the burning balls and the lava, and the looks. Their fires', fuses' and
//! douses' clocks are their statuses' (7.13a).

use glam::Vec2;
use tungsten::core::{
    AssetRegistry, Collider, Entity, Position, Shape, Sprite, Time, Transform, Velocity, World,
};
use tungsten::kit::{ContactDamage, Effects, Stats, Tags, stimulate};
use tungsten::physics::{SpatialFilter, overlap};

use super::components::{Bomb, Crate, Keg, Plank, Rock, StoneSize};
use super::{
    BOMB_CENTRE, BOMB_RADIUS, BURN_SECS, FUEL, FUSE_SECS, LIT, PLANK_BURN_SECS, PROP_BURN,
    ROCK_RADIUS, SPREAD_AFTER, SPREAD_REACH, TOUCH_REACH, alight, burning, fuel_left,
};
use crate::features::combat::systems::{EMBER_TINTS, flicker};
use crate::features::combat::{BURNING, DOUSED, FROZEN, has_status};
use crate::features::elements::cascade;
use crate::features::elements::systems::ignite;
use crate::features::elements::{COAT, KINDLE, is_burning, is_frozen};
use crate::features::level::lava::Lava;
use crate::features::level::systems::drawn_time;
use crate::features::run::Run;
use crate::features::toys::components::{ScrapTumble, SmallBall};
use crate::features::toys::systems::frost_tint;
use crate::shared::SaveId;

/// A crate's boards at the end of its burn, which its tint nears as it
/// burns.
pub const CHAR_TINT: [u8; 4] = [112, 86, 72, 255];
/// A lit keg's pulse over its fuse's last half: the tint it nears and how
/// fast it beats, a second.
pub const FUSE_PULSE: [u8; 4] = [255, 185, 165, 255];
pub const PULSE_HZ: f32 = 6.0;
/// Where a bomb's fuse burns, from its 16 px art's top-left as drawn
/// upright, its frames' `fuse` point, which its look's sparks and smoke
/// follow; and how fast its spark's two frames turn, a second.
pub const BOMB_TIP: Vec2 = Vec2::new(10.5, 0.5);
pub const BOMB_SPARK_HZ: f32 = 12.0;

/// `props_roll`, in the tick phase: each bomb and rock turns by the way it
/// travels over its radius, as a ball rolling without slipping does, in
/// the air too, where the turn its throw gave it carries on. Its sprite
/// turns about its art's middle, its body's centre.
pub fn roll(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    for (_, velocity, transform) in world.query_mut::<(&Bomb, &Velocity, &mut Transform)>() {
        transform.rotation =
            (transform.rotation + velocity.0.x * dt / BOMB_RADIUS) % std::f32::consts::TAU;
    }
    for (_, velocity, transform) in world.query_mut::<(&Rock, &Velocity, &mut Transform)>() {
        transform.rotation =
            (transform.rotation + velocity.0.x * dt / ROCK_RADIUS) % std::f32::consts::TAU;
    }
}

/// Where a bomb's fuse burns now: its tip turned with the bomb about the
/// art's middle, from the sprite's top-left at `corner`.
#[must_use]
pub fn bomb_tip(corner: Vec2, rotation: f32) -> Vec2 {
    corner + BOMB_CENTRE + Vec2::from_angle(rotation).rotate(BOMB_TIP - BOMB_CENTRE)
}

/// A prop as the heat reads it: its save ID, entity, body centre and half
/// size.
#[derive(Debug, Clone, Copy)]
struct Body {
    id: Option<SaveId>,
    entity: Entity,
    centre: Vec2,
    half: Vec2,
}

/// The crates, kegs and planks, in save-ID order.
fn bodies(world: &World) -> Vec<Body> {
    let body = |entity: Entity, at: &Position, collider: &Collider, id: Option<&SaveId>| {
        let half = collider.shape.half_extents();
        Body {
            id: id.copied(),
            entity,
            centre: at.0 + collider.offset,
            half,
        }
    };
    let mut bodies: Vec<Body> = world
        .query::<(Entity, &Crate, &Position, &Collider, Option<&SaveId>)>()
        .map(|(entity, _, at, collider, id)| body(entity, at, collider, id))
        .chain(
            world
                .query::<(Entity, &Keg, &Position, &Collider, Option<&SaveId>)>()
                .map(|(entity, _, at, collider, id)| body(entity, at, collider, id)),
        )
        .chain(
            world
                .query::<(Entity, &Plank, &Position, &Collider, Option<&SaveId>)>()
                .map(|(entity, _, at, collider, id)| body(entity, at, collider, id)),
        )
        .collect();
    bodies.sort_by_key(|body| body.id);
    bodies
}

/// The seconds of fire `prop` starts with: a plank's [`PLANK_BURN_SECS`],
/// a crate's [`BURN_SECS`].
fn whole_burn(world: &World, prop: Entity) -> f32 {
    if world.has::<Plank>(prop) {
        PLANK_BURN_SECS
    } else {
        BURN_SECS
    }
}

/// Whether two boxes overlap.
fn overlaps(a: Vec2, a_half: Vec2, b: Vec2, b_half: Vec2) -> bool {
    ((a - b).abs() - (a_half + b_half)).max_element() < 0.0
}

/// The bodies but tiles within `reach` of a box of `half` about `centre`,
/// at the last step's places, in save-ID order.
fn near(world: &World, centre: Vec2, half: Vec2, reach: f32) -> Vec<(Option<SaveId>, Entity)> {
    let mut met = Vec::new();
    let shape = Shape::Aabb {
        half_extents: half + Vec2::splat(reach),
    };
    overlap(
        world,
        shape,
        centre,
        &SpatialFilter::new().with_tiles(false),
        &mut met,
    );
    let mut near: Vec<(Option<SaveId>, Entity)> = met
        .into_iter()
        .flatten()
        .map(|entity| (world.get::<SaveId>(entity).copied(), entity))
        .collect();
    near.sort_by_key(|(id, _)| *id);
    near
}

/// `props_heat`, in the overlap phase after the physics step and before
/// chemistry's run, each prop in save-ID order: one whose box overlaps an
/// uncrusted lava basin's is sent `heat`, crediting the player, which
/// lights it or, frozen, shocks it; one with a burning small ball within
/// [`TOUCH_REACH`] of its box is sent `kindle`, crediting the player as the
/// ball's fire does. Then each burning crate or plank lights the small
/// balls within [`TOUCH_REACH`] of its box, and once it has burned
/// [`SPREAD_AFTER`] of its fire sends `kindle` to the props within
/// [`SPREAD_REACH`], credited to its lighter. Only a prop the stimulus can
/// light or shock is sent it. Proximity runs per prop through the spatial
/// index, never over the balls: resting bodies send no contacts.
pub fn heat(world: &mut World) {
    let props = bodies(world);
    if props.is_empty() {
        return;
    }
    let player = world.get_resource::<Run>().map(|run| run.player);
    let basins: Vec<(Vec2, Vec2)> = world
        .query::<(&Lava, &Transform, &ContactDamage)>()
        .filter(|(lava, ..)| lava.crust.is_none())
        .filter_map(|(_, at, touch)| match touch.shape()?.shape() {
            Shape::Circle { .. } => None,
            other => Some((at.position + touch.offset(), other.half_extents())),
        })
        .collect();
    for prop in &props {
        if !world.is_alive(prop.entity) {
            continue;
        }
        let in_lava = basins
            .iter()
            .any(|&(centre, half)| overlaps(prop.centre, prop.half, centre, half));
        if in_lava {
            if is_frozen(world, prop.entity) || kindles(world, prop.entity) {
                cascade::heat(world, prop.entity, prop.centre, player);
            }
            continue;
        }
        if !kindles(world, prop.entity) {
            continue;
        }
        let caught = near(world, prop.centre, prop.half, TOUCH_REACH)
            .into_iter()
            .any(|(_, entity)| world.has::<SmallBall>(entity) && is_burning(world, entity));
        if caught {
            stimulate(world, prop.entity, KINDLE, 1.0, player);
        }
    }
    for prop in props {
        if !world.is_alive(prop.entity) || !alight(world, prop.entity) {
            continue;
        }
        for (_, ball) in near(world, prop.centre, prop.half, TOUCH_REACH) {
            if world.has::<SmallBall>(ball) {
                ignite(world, ball);
            }
        }
        let left = fuel_left(world, prop.entity);
        if whole_burn(world, prop.entity) - left < SPREAD_AFTER - 1e-4 {
            continue;
        }
        let lighter = world
            .get::<Effects>(prop.entity)
            .and_then(|effects| {
                effects
                    .instances()
                    .find(|instance| instance.effect().id() == Some(PROP_BURN))
            })
            .and_then(|instance| instance.source());
        for (_, other) in near(world, prop.centre, prop.half, SPREAD_REACH) {
            if other != prop.entity && super::is_prop(world, other) && kindles(world, other) {
                stimulate(world, other, KINDLE, 1.0, lighter);
            }
        }
    }
}

/// Whether `kindle` would light `prop`: not frozen, alight or doused.
fn kindles(world: &World, prop: Entity) -> bool {
    !is_frozen(world, prop) && !burning(world, prop) && !has_status(world, prop, DOUSED)
}

/// Shows a rock's art of `variant`.
pub fn show_rock(world: &mut World, rock: Entity, variant: usize) {
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(&format!("ex10_goblin_rock_{}", variant % 2));
    if let Some(sprite) = world.get_mut::<Sprite>(rock) {
        sprite.asset_id = id;
    }
}

/// Shows a stone's art of its `size` and `variant`.
pub fn show_stone(world: &mut World, stone: Entity, size: StoneSize, variant: usize) {
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(&format!("ex10_stone_{}_{}", size.name(), variant % 2));
    if let Some(sprite) = world.get_mut::<Sprite>(stone) {
        sprite.asset_id = id;
    }
}

/// Shows a plank's art of `variant`, its row.
pub fn show_plank(world: &mut World, plank: Entity, variant: usize) {
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(&format!("ex10_plank_{}", variant % super::PLANK_ROWS.len()));
    if let Some(sprite) = world.get_mut::<Sprite>(plank) {
        sprite.asset_id = id;
    }
}

/// Shows a crate's art of `variant`.
pub fn show_variant(world: &mut World, prop: Entity, variant: usize) {
    let id = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(&format!("ex10_pine_crate_{}", variant % 2));
    if let Some(sprite) = world.get_mut::<Sprite>(prop) {
        sprite.asset_id = id;
    }
}

/// `a` toward `b` by `share`, 0 to 1.
fn mix(a: [u8; 4], b: [u8; 4], share: f32) -> [u8; 4] {
    let share = share.clamp(0.0, 1.0);
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * share).round() as u8
    })
}

/// How frozen a prop is, 0 to 1, by its tags and effects: 1 frozen, else
/// its coat.
fn frost(tags: Option<&Tags>, effects: Option<&Effects>) -> f32 {
    if tags.is_some_and(|tags| tags.has(FROZEN)) {
        1.0
    } else {
        effects
            .and_then(|effects| effects.intensity(COAT))
            .unwrap_or(0.0)
    }
}

/// The frame of a fuse with `left` seconds: its quarter burnt.
#[must_use]
pub fn fuse_frame(left: f32) -> usize {
    (((FUSE_SECS - left) / (FUSE_SECS / 4.0)).max(0.0) as usize).min(3)
}

/// A crate's or a plank's tint with `left` of its fire's fuel, alight or
/// not: the frost's when frozen or chilled, else its fire's flicker
/// (burning's two ember tints) toward char by its burnt share of `whole` to
/// the 1.2, one put out its char alone.
fn wood_tint(left: f32, lit: bool, whole: f32, frost: f32, time: f32) -> [u8; 4] {
    if frost > 0.0 {
        return frost_tint(frost);
    }
    let burnt = (1.0 - left / whole).clamp(0.0, 1.0);
    let base = if lit {
        EMBER_TINTS[flicker(time)]
    } else {
        [255; 4]
    };
    mix(base, CHAR_TINT, burnt.powf(1.2))
}

/// `props_look`, in the present phase: a frozen or chilled prop takes the
/// frost's tint; a burning crate its fire's flicker (burning's two ember
/// tints) toward char by its burnt share to the 1.2, one put out its char
/// alone, and a plank so over [`PLANK_BURN_SECS`], turned by its tumble; a
/// keg its lit fuse's frame by the quarter burnt, pulsing red over the
/// fuse's last half; a bomb its spark's two frames in turn.
pub fn look(world: &mut World) {
    let time = drawn_time(world);
    let fire = |stats: &Stats, tags: Option<&Tags>| {
        let left = stats.base(FUEL).unwrap_or(0.0);
        (left, tags.is_some_and(|tags| tags.has(BURNING)))
    };
    for (_, stats, sprite, tags, effects) in
        world.query_mut::<(&Crate, &Stats, &mut Sprite, Option<&Tags>, Option<&Effects>)>()
    {
        let (left, lit) = fire(stats, tags);
        sprite.color = wood_tint(left, lit, BURN_SECS, frost(tags, effects), time);
    }
    for (_, stats, sprite, transform, tumble, tags, effects) in world.query_mut::<(
        &Plank,
        &Stats,
        &mut Sprite,
        &mut Transform,
        Option<&ScrapTumble>,
        Option<&Tags>,
        Option<&Effects>,
    )>() {
        let (left, lit) = fire(stats, tags);
        sprite.color = wood_tint(left, lit, PLANK_BURN_SECS, frost(tags, effects), time);
        if let Some(tumble) = tumble {
            transform.rotation = tumble.rotation(time);
        }
    }
    let kegs: Vec<(Entity, Option<f32>, f32)> = world
        .query::<(Entity, &Keg, Option<&Tags>, Option<&Effects>)>()
        .map(|(entity, _, tags, effects)| {
            let fuse = effects
                .and_then(|effects| {
                    effects
                        .instances()
                        .find(|instance| instance.effect().id() == Some(LIT))
                })
                .and_then(|instance| instance.left());
            (entity, fuse, frost(tags, effects))
        })
        .collect();
    for (keg, fuse, frost) in kegs {
        let name = match fuse {
            Some(left) => format!("ex10_powder_keg_lit_{}", fuse_frame(left)),
            None => "ex10_powder_keg".to_string(),
        };
        let asset = world.resource_mut::<AssetRegistry>().intern_sprite(&name);
        let color = if frost > 0.0 {
            frost_tint(frost)
        } else {
            let burnt = fuse.map_or(0.0, |left| (FUSE_SECS - left) / FUSE_SECS);
            let depth = ((burnt - 0.5) * 2.0).clamp(0.0, 1.0);
            let beat = 0.5 + 0.5 * (time * PULSE_HZ * std::f32::consts::TAU).sin();
            mix([255; 4], FUSE_PULSE, depth * beat)
        };
        if let Some(sprite) = world.get_mut::<Sprite>(keg) {
            sprite.asset_id = asset;
            sprite.color = color;
        }
    }
    let bombs: Vec<Entity> = world
        .query::<(Entity, &Bomb)>()
        .map(|(entity, _)| entity)
        .collect();
    if bombs.is_empty() {
        return;
    }
    let spark = (time * BOMB_SPARK_HZ) as usize % 2;
    let asset = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(&format!("ex10_goblin_bomb_{spark}"));
    for bomb in bombs {
        if let Some(sprite) = world.get_mut::<Sprite>(bomb) {
            sprite.asset_id = asset;
        }
    }
}
