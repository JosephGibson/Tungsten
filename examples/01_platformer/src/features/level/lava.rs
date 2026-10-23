//! Lava's crust: the ice spray hardens a basin into stone for a while. A
//! cold ray that stops on a basin's surface chills it; [`CRUST_COLD`]
//! seconds of chill set the whole basin, its touch and its frost child's
//! held aside, its surface solid for every body, with a hiss and steam
//! along it. It melts after [`CRUST_SECS`], its cracks glowing over the
//! last [`CRACK_SECS`]: its touches come back, so a player still on it is
//! thrown out as by any touch, and what stood on it falls in.

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, Collider, Entity, RigidBody, Shape, Sprite, Time, Transform, Visibility, World,
};
use tungsten::kit::{Children, ContactDamage};

use super::components::{Glow, Hazard};
use super::{LAVA_LAYER, TILE};
use crate::features::audio::{self, components::Sound};
use crate::game::GAMEPLAY;
use crate::shared::spawn_burst;

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

impl Crust {
    /// Its look's stage: 0 set, then 1 to 3 over its last [`CRACK_SECS`].
    #[must_use]
    pub fn stage(&self) -> usize {
        if self.left > CRACK_SECS {
            return 0;
        }
        (1 + ((CRACK_SECS - self.left) / (CRACK_SECS / 3.0)) as usize).min(3)
    }
}

/// A set crust's sprites, one a lava cell, which its look keeps: not saved,
/// drawn again after a load.
#[derive(Debug, Clone, Default)]
pub struct CrustLook {
    sprites: Vec<(Entity, bool, usize)>,
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

/// The rows of lava under `basin`'s surface row, by its held touch's box:
/// from just over the skin to the bed.
fn rows(crust: &Crust) -> usize {
    crust
        .touch
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

/// A set crust as its look reads it: its stage and its rows of lava.
type Stage = Option<(usize, usize)>;

/// `crust_look`, before the frame's sprites: a set crust's sprites over
/// its lava, spawned when it sets and after a load, at its stage; gone when
/// it melts.
pub fn look(world: &mut World) {
    let basins: Vec<(Entity, Stage, bool)> = world
        .query::<(Entity, &Lava, Option<&CrustLook>)>()
        .map(|(basin, lava, shown)| {
            let crust = lava
                .crust
                .as_ref()
                .map(|crust| (crust.stage(), rows(crust)));
            (basin, crust, shown.is_some())
        })
        .collect();
    for (basin, crust, shown) in basins {
        match crust {
            Some((stage, depth)) => {
                if !shown {
                    spawn_look(world, basin, depth);
                }
                restage(world, basin, stage);
            }
            None if shown => {
                if let Some(look) = world.remove_component::<CrustLook>(basin) {
                    for (sprite, ..) in look.sprites {
                        world.despawn(sprite);
                    }
                }
            }
            None => {}
        }
    }
}

/// Spawns `basin`'s crust sprites: its surface row's tops and `depth` - 1
/// rows of cooled rock under them, each column one of two variants.
fn spawn_look(world: &mut World, basin: Entity, depth: usize) {
    let Some((left, right, skin)) = surface(world, basin) else {
        return;
    };
    let row_top = skin - 6.0;
    let mut sprites = Vec::new();
    let columns = ((right - left) / TILE).round() as usize;
    for column in 0..columns {
        for row in 0..depth {
            let at = Vec2::new(left + column as f32 * TILE, row_top + row as f32 * TILE);
            let variant = (column + row) % 2;
            let top = row == 0;
            let Some(asset) = world
                .get_resource_mut::<AssetRegistry>()
                .map(|assets| assets.intern_sprite(&crust_sprite(top, 0, variant)))
            else {
                return;
            };
            let mut sprite = Sprite::new(asset);
            sprite.z_order = CRUST_Z;
            let entity = world.spawn_with((
                Transform::from_position(at),
                sprite,
                Visibility { visible: true },
                SceneEntity { state_id: GAMEPLAY },
            ));
            sprites.push((entity, top, variant));
        }
    }
    world.insert(basin, CrustLook { sprites });
}

/// Shows `basin`'s crust sprites at `stage`.
fn restage(world: &mut World, basin: Entity, stage: usize) {
    let Some(sprites) = world
        .get::<CrustLook>(basin)
        .map(|look| look.sprites.clone())
    else {
        return;
    };
    for (entity, top, variant) in sprites {
        let Some(asset) = world
            .get_resource_mut::<AssetRegistry>()
            .map(|assets| assets.intern_sprite(&crust_sprite(top, stage, variant)))
        else {
            return;
        };
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.asset_id = asset;
        }
    }
}

/// A crust sprite's manifest ID.
fn crust_sprite(top: bool, stage: usize, variant: usize) -> String {
    let part = if top { "top" } else { "deep" };
    format!("ex10_lava_crust_{part}_{stage}_{variant}")
}
