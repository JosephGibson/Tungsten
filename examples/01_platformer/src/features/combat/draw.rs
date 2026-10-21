//! What combat draws over the sprites: each target's status overlays in
//! its sprite's size class, flames while burning, an ice cube while frozen
//! (fitted to the body's hurtbox where its kind has one), a crackle while
//! shocked and a halo of stars while stunned.

use glam::Vec2;
use tungsten::core::{AssetRegistry, Entity, Sprite, Transform, World};
use tungsten::kit::{Health, Tags};
use tungsten::render::SpriteBatch;

use super::systems::flicker;
use super::{BURNING, FROZEN, SHOCKED, STUNNED};
use crate::features::dummy::components::Dummy;
use crate::features::elements::draw::FLAME_FRAMES;
use crate::features::enemies::components::Enemy;
use crate::features::level::draw::{instance, push_quad, view};
use crate::features::level::systems::drawn_time;

/// A burning target's flames: frames a second, and where each sits on the
/// target's box, from its top-left, in shares of its side.
const FLAME_FPS: f32 = 12.0;
const FLAME_SPOTS: [Vec2; 2] = [Vec2::new(0.22, 0.18), Vec2::new(0.58, 0.36)];

/// The overlays' size class for a sprite `side` pixels across: small 32,
/// medium 64, large 128.
#[must_use]
pub fn class(side: f32) -> &'static str {
    if side <= 48.0 {
        "small"
    } else if side <= 96.0 {
        "medium"
    } else {
        "large"
    }
}

/// The ice cube a frozen `entity` shows: its kind's own, fitted to its
/// hurtbox on a frame of its sprite's size, else its size class's.
fn ice(world: &World, entity: Entity, class: &str) -> String {
    if world.has::<Dummy>(entity) {
        return "ex10_status_ice_dummy".to_string();
    }
    if let Some(enemy) = world.get::<Enemy>(entity) {
        return enemy.kind.info().ice.to_string();
    }
    format!("ex10_status_ice_{class}")
}

/// Pushes the sprite `name` over the box at `origin` of `size`, when it is
/// loaded.
fn quad(
    batches: &mut Vec<SpriteBatch>,
    assets: &AssetRegistry,
    name: &str,
    origin: Vec2,
    size: Vec2,
) {
    if let Some(asset) = assets.get_sprite(name) {
        push_quad(
            batches,
            asset,
            instance(asset, origin, size, [255; 4]),
            None,
        );
    }
}

/// Every live target's overlays in view.
#[must_use]
pub fn overlays(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let time = drawn_time(world);
    let mut batches = Vec::new();
    for (entity, health, tags, sprite, at) in
        world.query::<(Entity, &Health, &Tags, &Sprite, &Transform)>()
    {
        if health.is_dead() {
            continue;
        }
        let width = assets
            .sprite(sprite.asset_id)
            .map_or(64.0, |asset| asset.width as f32);
        let side = width * at.scale.x.abs();
        let size = Vec2::splat(side);
        let origin = at.position;
        if (origin + size).cmplt(min).any() || origin.cmpgt(max).any() {
            continue;
        }
        let class = class(side);
        if tags.has(BURNING) {
            let flame = Vec2::splat((side * 0.4).max(12.0));
            for (i, spot) in FLAME_SPOTS.into_iter().enumerate() {
                let frame = ((time * FLAME_FPS) as usize + 3 * i) % FLAME_FRAMES.len();
                quad(
                    &mut batches,
                    assets,
                    FLAME_FRAMES[frame],
                    origin + spot * side,
                    flame,
                );
            }
        }
        if tags.has(FROZEN) {
            quad(
                &mut batches,
                assets,
                &ice(world, entity, class),
                origin,
                size,
            );
        }
        if tags.has(SHOCKED) {
            let name = format!("ex10_status_crackle_{class}_{}", flicker(time));
            quad(&mut batches, assets, &name, origin, size);
        }
        if tags.has(STUNNED) {
            quad(
                &mut batches,
                assets,
                &format!("ex10_status_halo_{class}"),
                origin,
                size,
            );
        }
    }
    batches
}
