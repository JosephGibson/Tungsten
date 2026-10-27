//! Lava's crust: the ice spray hardens a basin into stone for a while. A
//! cold ray that stops on a basin's surface chills it; [`CRUST_COLD`]
//! seconds of chill set the whole basin, its touch and its frost child's
//! held aside, its surface solid for every body, with a hiss and steam
//! along it. It melts after [`CRUST_SECS`], its cracks glowing over the
//! last [`CRACK_SECS`]: its touches come back, so a player still on it is
//! thrown out as by any touch, and what stood on it falls in.
//!
//! Its look (01 (k)) reads only the basin's chill and its crust's seconds
//! left, which a save keeps: one front comes down through the whole basin
//! as the chill builds and the crust sets, the crust's frames dull, hold
//! and crack, and a second front comes down as it melts, with a sound and a
//! burst at each change.
//!
//! An uncrusted basin takes the bodies that fall into its burn ([`take`]):
//! a keg or a goblin's bomb goes off, a frozen prop is left to the heat's
//! thermal shock, and every other ball, block, scrap, crate, rock or stone
//! goes in steam and embers, with one sizzle a basin a step.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, BodyKind, Collider, Entity, RigidBody, Shape, Sprite, Time, Transform,
    Visibility, World,
};
use tungsten::kit::{Children, ContactDamage, Health};
use tungsten::physics::{SpatialFilter, overlap};

use super::components::{Glow, Hazard};
use super::{LAVA_LAYER, TILE};
use crate::features::audio::{self, components::Sound};
use crate::features::elements::cascade;
use crate::features::elements::components::Frozen;
use crate::features::props::{
    self,
    components::{Bomb, Keg},
};
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, body_centre, despawn_tree, spawn_burst};

/// Seconds of cold spray that set a basin's crust; unsprayed, its chill
/// drains as fast.
pub const CRUST_COLD: f32 = 0.5;
/// Seconds a crust holds, and how many of its last its cracks glow, in
/// three stages.
pub const CRUST_SECS: f32 = 6.0;
pub const CRACK_SECS: f32 = 1.5;
/// The crust's sprites' depth: over the lava's props, under the decks and
/// the actors.
const CRUST_Z: i32 = -9;

/// A lava basin's chill and its crust, while set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lava {
    /// Seconds of cold spray on it, draining when unsprayed.
    pub cold: f32,
    /// A cold ray stopped on it this step.
    pub sprayed: bool,
    pub crust: Option<Crust>,
}

/// A set crust: its seconds left and the touches it holds aside, the
/// basin's and its frost child's.
#[derive(Debug, Clone, PartialEq)]
pub struct Crust {
    pub left: f32,
    pub touch: Option<ContactDamage>,
    pub frost: Option<ContactDamage>,
}

/// A basin's look: the freeze's front coming down, in px from the surface;
/// the crust's frames (`0..9`: the set, its dulling, the hold, two a crack
/// stage); or the melt's front coming down.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Freezing(f32),
    Crust(usize),
    Melting(f32),
}

/// The set's seconds, over which the front comes down the last of the
/// depth; the crust's dulling after them; the melt's seconds after it goes.
pub const SET_SECS: f32 = 0.3;
pub const DULL_SECS: f32 = 0.15;
pub const MELT_SECS: f32 = 0.4;
/// The share of the depth the front comes down while the chill builds.
const COLD_SHARE: f32 = 0.6;
/// The px between a row's partial frames.
const FRONT_STEP: f32 = 8.0;

/// A basin's look's sprites (entity, row, variant), its rows, its phase,
/// the melt's seconds left, and the freeze bursts' throttle: the third of
/// the depth its front last burst at and the seconds since a build's first.
/// Presentation: not saved, drawn again after a load.
#[derive(Debug, Clone)]
pub struct CrustLook {
    sprites: Vec<(Entity, usize, usize)>,
    rows: usize,
    phase: Option<Phase>,
    melt: f32,
    third: i32,
    since: f32,
}

/// At a run's start: each basin's chill and crust, none.
pub fn start(world: &mut World) {
    let basins: Vec<Entity> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(_, hazard)| hazard.lava)
        .map(|(entity, _)| entity)
        .collect();
    for basin in basins {
        world.insert(basin, Lava::default());
    }
}

/// A cold ray stopped on `basin`'s surface this step: its chill runs; a set
/// crust holds as it is.
pub fn chill(world: &mut World, basin: Entity) {
    if let Some(lava) = world.get_mut::<Lava>(basin) {
        lava.sprayed = true;
    }
}

/// `lava_tick`, in the tick phase: each basin's chill runs while sprayed
/// and drains else, and sets its crust at [`CRUST_COLD`]; each crust runs
/// down and melts at 0.
pub fn tick(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let mut set = Vec::new();
    let mut melted = Vec::new();
    for (basin, lava) in world.query_mut::<(Entity, &mut Lava)>() {
        let sprayed = std::mem::take(&mut lava.sprayed);
        if let Some(crust) = &mut lava.crust {
            crust.left -= dt;
            if crust.left <= 1e-5 {
                melted.push(basin);
            }
        } else if sprayed {
            lava.cold += dt;
            if lava.cold >= CRUST_COLD - 1e-4 {
                set.push(basin);
            }
        } else {
            lava.cold = (lava.cold - dt).max(0.0);
        }
    }
    for basin in set {
        harden(world, basin, CRUST_SECS);
        if let Some((left, right, skin)) = surface(world, basin) {
            let mut x = left + TILE * 0.5;
            while x < right {
                spawn_burst(world, "ex10_steam_burst", Vec2::new(x, skin));
                x += TILE * 1.5;
            }
            audio::play(world, Sound::LavaSet, Vec2::new((left + right) * 0.5, skin));
        }
    }
    for basin in melted {
        melt(world, basin);
    }
}

/// Sets `basin`'s crust for `left` seconds, silently: its touches held,
/// its surface in the tiles' layer meeting every body, its glow out. A
/// load's crust, and the spray's.
pub fn harden(world: &mut World, basin: Entity, left: f32) {
    let touch = world.remove_component::<ContactDamage>(basin);
    let frost = frost_child(world, basin).and_then(|child| world.remove_component(child));
    if let Some(body) = world.get_mut::<RigidBody>(basin) {
        body.layers = 1;
        body.mask = RigidBody::ALL_LAYERS;
    }
    if let Some(glow) = world.get_mut::<Glow>(basin) {
        glow.hidden = true;
    }
    if let Some(lava) = world.get_mut::<Lava>(basin) {
        lava.cold = 0.0;
        lava.crust = Some(Crust { left, touch, frost });
    }
}

/// Melts `basin`'s crust: its touches back, its surface the player's alone
/// again, its glow lit.
fn melt(world: &mut World, basin: Entity) {
    let Some(crust) = world
        .get_mut::<Lava>(basin)
        .and_then(|lava| lava.crust.take())
    else {
        return;
    };
    if let Some(touch) = crust.touch {
        world.insert(basin, touch);
    }
    if let (Some(frost), Some(child)) = (crust.frost, frost_child(world, basin)) {
        world.insert(child, frost);
    }
    if let Some(body) = world.get_mut::<RigidBody>(basin) {
        body.layers = LAVA_LAYER;
        body.mask = 0;
    }
    if let Some(glow) = world.get_mut::<Glow>(basin) {
        glow.hidden = false;
    }
}

/// `lava_take`, in the overlap phase after `props_heat` and before the
/// cascades' run: each basin with its touch, no crust, takes the dynamic
/// bodies with no health its burn box overlaps, in save-ID order. A keg or
/// a bomb not frozen goes off, credited to its lighter, else the player; a
/// frozen prop is left to the heat's thermal shock; every other body goes,
/// in steam and embers at its centre. A basin that took anything sizzles
/// once, at its surface's middle.
pub fn take(world: &mut World) {
    let basins: Vec<(Entity, Vec2, Vec2)> = world
        .query::<(Entity, &Lava, &Transform, &ContactDamage)>()
        .filter(|(_, lava, ..)| lava.crust.is_none())
        .filter_map(|(basin, _, at, touch)| match touch.shape()?.shape() {
            Shape::Aabb { half_extents } => {
                Some((basin, at.position + touch.offset(), half_extents))
            }
            Shape::Circle { .. } => None,
        })
        .collect();
    let mut met = Vec::new();
    for (basin, centre, half) in basins {
        let shape = Shape::Aabb { half_extents: half };
        overlap(world, shape, centre, &SpatialFilter::new(), &mut met);
        let mut bodies: Vec<(Option<SaveId>, Entity)> = met
            .iter()
            .flatten()
            .copied()
            .filter(|&body| {
                world
                    .get::<RigidBody>(body)
                    .is_some_and(|rigid| rigid.kind == BodyKind::Dynamic)
                    && !world.has::<Health>(body)
            })
            .map(|body| (world.get::<SaveId>(body).copied(), body))
            .collect();
        bodies.sort_unstable_by_key(|&(id, body)| (id, body.id()));
        bodies.dedup();
        let mut took = false;
        for (_, body) in bodies {
            let frozen = world.has::<Frozen>(body);
            let lighter = world
                .get::<Keg>(body)
                .map(|keg| keg.lighter)
                .or_else(|| world.get::<Bomb>(body).map(|bomb| bomb.lighter));
            if let Some(lighter) = lighter {
                if !frozen {
                    let credit = lighter.or_else(|| cascade::player_credit(world));
                    took |= props::set_off(world, body, None, credit);
                }
                continue;
            }
            if frozen && props::is_prop(world, body) {
                continue;
            }
            let Some(at) = body_centre(world, body) else {
                continue;
            };
            despawn_tree(world, body);
            spawn_burst(world, "ex10_steam_burst", at);
            spawn_burst(world, "ex10_blast_embers", at);
            took = true;
        }
        if let (true, Some((left, right, skin))) = (took, surface(world, basin)) {
            audio::play(
                world,
                Sound::LavaTook,
                Vec2::new((left + right) * 0.5, skin),
            );
        }
    }
}

/// `basin`'s frost child: the child touching what is frozen.
fn frost_child(world: &World, basin: Entity) -> Option<Entity> {
    world.get::<Children>(basin)?.iter().next()
}

/// `basin`'s surface: its left and right edges and its skin, in world
/// pixels.
fn surface(world: &World, basin: Entity) -> Option<(f32, f32, f32)> {
    let at = world.get::<Transform>(basin)?.position;
    let collider = world.get::<Collider>(basin)?;
    let Shape::Aabb { half_extents } = collider.shape else {
        return None;
    };
    let middle = at + collider.offset;
    Some((
        middle.x - half_extents.x,
        middle.x + half_extents.x,
        middle.y - half_extents.y,
    ))
}

/// The rows of lava under `basin`'s surface row, by its touch's box (the
/// crust's held one while set): from just over the skin to the bed.
fn rows(world: &World, basin: Entity) -> usize {
    let held = world
        .get::<Lava>(basin)
        .and_then(|lava| lava.crust.as_ref())
        .and_then(|crust| crust.touch.clone());
    let touch = held.or_else(|| world.get::<ContactDamage>(basin).cloned());
    touch
        .as_ref()
        .and_then(ContactDamage::shape)
        .map_or(1, |shape| {
            let height = match shape.shape() {
                Shape::Aabb { half_extents } => 2.0 * half_extents.y,
                Shape::Circle { radius } => 2.0 * radius,
            };
            ((height / TILE).round() as usize).max(1)
        })
}

/// The crust's frame at `left` seconds once its front is down: its dulling
/// over [`DULL_SECS`], the hold, then two for each of the three crack
/// stages over its last [`CRACK_SECS`].
fn frame(left: f32) -> usize {
    let since = CRUST_SECS - left;
    if since < SET_SECS + DULL_SECS {
        1
    } else if left > CRACK_SECS {
        2
    } else {
        3 + (((CRACK_SECS - left) / (CRACK_SECS / 6.0)) as usize).min(5)
    }
}

/// Row `row`'s sprite under `phase`, none where the melt shows: the set
/// frame above a front, a partial at it ([`FRONT_STEP`] px apart) and
/// nothing below it as it freezes; nothing above the melt's front, a
/// partial at it and the last crack frame below it as it melts.
fn row_sprite(phase: Phase, row: usize, variant: usize) -> Option<String> {
    let part = if row == 0 { "top" } else { "deep" };
    let local = |depth: f32| depth - row as f32 * TILE;
    match phase {
        Phase::Freezing(depth) => {
            let at = local(depth);
            if at >= TILE {
                Some(format!("ex10_lava_crust_{part}_0_{variant}"))
            } else if at >= 0.0 {
                let partial = (at / FRONT_STEP) as usize;
                Some(format!("ex10_lava_freeze_{part}_{partial}_{variant}"))
            } else {
                None
            }
        }
        Phase::Crust(n) => Some(format!("ex10_lava_crust_{part}_{n}_{variant}")),
        Phase::Melting(depth) => {
            let at = local(depth);
            if at < 0.0 {
                Some(format!("ex10_lava_crust_{part}_8_{variant}"))
            } else if at < TILE {
                let partial = (at / FRONT_STEP) as usize;
                Some(format!("ex10_lava_melt_{part}_{partial}_{variant}"))
            } else {
                None
            }
        }
    }
}

/// `crust_look`, before the frame's sprites: each basin's look by its
/// chill, its crust's seconds left and the melt's. One front comes down
/// [`COLD_SHARE`] of the depth as the chill builds and the rest over the
/// crust's first [`SET_SECS`]; the crust's frames follow; a second front
/// comes down over the [`MELT_SECS`] after the crust goes. A sound and a
/// burst mark each change ([`changed`]).
pub fn look(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    let basins: Vec<(Entity, f32, Option<f32>, bool)> = world
        .query::<(Entity, &Lava, Option<&CrustLook>)>()
        .map(|(basin, lava, shown)| {
            let left = lava.crust.as_ref().map(|crust| crust.left);
            (basin, lava.cold, left, shown.is_some())
        })
        .collect();
    for (basin, cold, crust, shown) in basins {
        if !shown && crust.is_none() && cold <= 0.0 {
            continue;
        }
        if !shown {
            spawn_look(world, basin);
        }
        let Some(mut look) = world.get::<CrustLook>(basin).cloned() else {
            continue;
        };
        let depth = look.rows as f32 * TILE;
        let before = look.phase;
        look.since += dt;
        let phase = match crust {
            Some(left) => {
                let since = CRUST_SECS - left;
                if since < SET_SECS {
                    let share = COLD_SHARE + (1.0 - COLD_SHARE) * since / SET_SECS;
                    Some(Phase::Freezing(depth * share))
                } else {
                    Some(Phase::Crust(frame(left)))
                }
            }
            None => match before {
                Some(Phase::Crust(_)) => {
                    look.melt = MELT_SECS;
                    Some(Phase::Melting(0.0))
                }
                Some(Phase::Melting(_)) => {
                    look.melt -= dt;
                    (look.melt > 0.0).then(|| Phase::Melting(depth * (1.0 - look.melt / MELT_SECS)))
                }
                _ if cold > 0.0 => {
                    let share = COLD_SHARE * (cold / CRUST_COLD).min(1.0);
                    Some(Phase::Freezing(depth * share))
                }
                _ => None,
            },
        };
        if phase.is_none() && cold <= 0.0 {
            look.third = -1;
        }
        changed(world, basin, &mut look, before, phase, depth);
        look.phase = phase;
        for &(entity, row, variant) in &look.sprites {
            let name = phase.and_then(|phase| row_sprite(phase, row, variant));
            if let Some(visibility) = world.get_mut::<Visibility>(entity) {
                visibility.visible = name.is_some();
            }
            if let Some(name) = name
                && let Some(asset) = world
                    .get_resource_mut::<AssetRegistry>()
                    .map(|assets| assets.intern_sprite(&name))
                && let Some(sprite) = world.get_mut::<Sprite>(entity)
            {
                sprite.asset_id = asset;
            }
        }
        world.insert(basin, look);
    }
}

/// The sound and the bursts a look's change makes, a burst to each two
/// columns: as the freeze starts, `LavaChilled`, glints and frost mist, and
/// more mist as its front reaches each further third of the depth, a new
/// build after a full drain at most once a second; at each crack stage,
/// `LavaCracked` and sparks; as the melt starts, `LavaMelted` and spatter.
/// The set's steam and `LavaSet` are the tick's.
fn changed(
    world: &mut World,
    basin: Entity,
    look: &mut CrustLook,
    before: Option<Phase>,
    phase: Option<Phase>,
    depth: f32,
) {
    let Some((left, right, skin)) = surface(world, basin) else {
        return;
    };
    let middle = Vec2::new((left + right) * 0.5, skin);
    let pairs = (((right - left) / TILE).round() as usize).div_ceil(2);
    if let Some(Phase::Freezing(front)) = phase {
        let third = ((3.0 * front / depth) as i32).min(2);
        if third > look.third && (look.third >= 0 || look.since >= 1.0) {
            for k in 0..pairs {
                let x = (left + (2 * k + 1) as f32 * TILE).min(right - 8.0);
                spawn_burst(world, "ex10_lava_mist", Vec2::new(x, skin - 4.0));
            }
            if look.third < 0 {
                spawn_burst(world, "ex10_lava_frost", middle - Vec2::new(0.0, 2.0));
                audio::play(world, Sound::LavaChilled, middle);
                look.since = 0.0;
            }
            look.third = third;
        }
    }
    if let Some(Phase::Crust(n @ (3 | 5 | 7))) = phase
        && before != Some(Phase::Crust(n))
    {
        for k in 0..pairs {
            let x = left + ((2 * k) as f32 + 0.5 + 0.5 * ((n + k) % 2) as f32) * TILE;
            spawn_burst(
                world,
                "ex10_lava_sparks",
                Vec2::new(x.min(right - 8.0), skin + 4.0),
            );
        }
        audio::play(world, Sound::LavaCracked, middle);
    }
    if phase == Some(Phase::Melting(0.0)) {
        for k in 0..pairs {
            let x = (left + (2 * k + 1) as f32 * TILE).min(right - 8.0);
            spawn_burst(world, "ex10_lava_spatter", Vec2::new(x, skin));
        }
        audio::play(world, Sound::LavaMelted, middle);
    }
}

/// Spawns `basin`'s look's sprites, hidden: its surface row's and each row
/// under it to the bed, each column one of two variants.
fn spawn_look(world: &mut World, basin: Entity) {
    let Some((left, right, skin)) = surface(world, basin) else {
        return;
    };
    let rows = rows(world, basin);
    let row_top = skin - 6.0;
    let mut sprites = Vec::new();
    let columns = ((right - left) / TILE).round() as usize;
    for column in 0..columns {
        for row in 0..rows {
            let at = Vec2::new(left + column as f32 * TILE, row_top + row as f32 * TILE);
            let variant = (column + row) % 2;
            let part = if row == 0 { "top" } else { "deep" };
            let Some(asset) = world
                .get_resource_mut::<AssetRegistry>()
                .map(|assets| assets.intern_sprite(&format!("ex10_lava_crust_{part}_0_{variant}")))
            else {
                return;
            };
            let mut sprite = Sprite::new(asset);
            sprite.z_order = CRUST_Z;
            let entity = world.spawn_with((
                Transform::from_position(at),
                sprite,
                Visibility { visible: false },
                SceneEntity { state_id: GAMEPLAY },
            ));
            sprites.push((entity, row, variant));
        }
    }
    world.insert(
        basin,
        CrustLook {
            sprites,
            rows,
            phase: None,
            melt: 0.0,
            third: -1,
            since: 1.0,
        },
    );
}
