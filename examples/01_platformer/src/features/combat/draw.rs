//! What combat draws over the sprites: each target's status overlays in
//! its sprite's size class (the player's in a square on his body), flames
//! in a warm glow while burning, an ice cube while frozen (fitted to the
//! body's hurtbox where its kind has one), flickering to its cracked twin
//! as the freeze runs out, a crackle while shocked and a halo of stars
//! while stunned.

use glam::Vec2;
use tungsten::core::{AssetRegistry, Collider, Entity, Sprite, Transform, World};
use tungsten::kit::{Health, Tags};
use tungsten::render::SpriteBatch;

use super::systems::{THAW_FLICKER_HZ, THAW_WARN, flicker, frozen_left};
use super::{BURNING, FROZEN, SHOCKED, STUNNED};
use crate::features::dummy::components::Dummy;
use crate::features::elements::draw::FLAME_FRAMES;
use crate::features::enemies::components::Enemy;
use crate::features::level::draw::{instance, material, push_quad, view};
use crate::features::level::systems::{HALO, HALO_MATERIAL, drawn_time};
use crate::features::player::components::Player;
use crate::features::player::{BODY_OFFSET, OVERLAY_SIDE};

/// A burning target's flames: frames a second, their side as a share of
/// the target's, and where each sits on the target's box, from its
/// top-left, in shares of its side; and the warm glow under them, its side
/// as a share of the target's and its colour.
const FLAME_FPS: f32 = 12.0;
const FLAME_SIDE: f32 = 0.55;
const FLAME_SPOTS: [Vec2; 3] = [
    Vec2::new(0.16, 0.12),
    Vec2::new(0.46, 0.28),
    Vec2::new(0.28, 0.44),
];
const BURN_GLOW: f32 = 2.2;
const BURN_GLOW_COLOR: [u8; 4] = [255, 140, 60, 200];

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
/// hurtbox on a frame of its sprite's size, else its size class's; over
/// its freeze's last [`THAW_WARN`] seconds, its cracked twin every other
/// [`THAW_FLICKER_HZ`] tick.
fn ice(world: &World, entity: Entity, class: &str, time: f32) -> String {
    let cube = if world.has::<Dummy>(entity) {
        "ex10_status_ice_dummy".to_string()
    } else if let Some(enemy) = world.get::<Enemy>(entity) {
        enemy.kind.info().ice.to_string()
    } else {
        format!("ex10_status_ice_{class}")
    };
    let thawing = frozen_left(world, entity).is_some_and(|left| left < THAW_WARN);
    if thawing && (time * THAW_FLICKER_HZ) as usize % 2 == 1 {
        format!("{cube}_cracked")
    } else {
        cube
    }
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
        // The player's square sits on his body; another target's is its
        // sprite's.
        let (side, origin) = if world.has::<Player>(entity) {
            let body = world
                .get::<Collider>(entity)
                .map_or(BODY_OFFSET, |c| c.offset);
            let centre = at.position + body;
            (OVERLAY_SIDE, centre - Vec2::splat(OVERLAY_SIDE * 0.5))
        } else {
            let width = assets
                .sprite(sprite.asset_id)
                .map_or(64.0, |asset| asset.width as f32);
            (width * at.scale.x.abs(), at.position)
        };
        let size = Vec2::splat(side);
        if (origin + size).cmplt(min).any() || origin.cmpgt(max).any() {
            continue;
        }
        let class = class(side);
        if tags.has(BURNING) {
            if let Some(halo) = assets.get_sprite(HALO) {
                let glow = side * BURN_GLOW;
                let centre = origin + size * 0.5;
                push_quad(
                    &mut batches,
                    halo,
                    instance(
                        halo,
                        centre - Vec2::splat(glow * 0.5),
                        Vec2::splat(glow),
                        BURN_GLOW_COLOR,
                    ),
                    material(world, HALO_MATERIAL),
                );
            }
            let flame = Vec2::splat((side * FLAME_SIDE).max(12.0));
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
                &ice(world, entity, class, time),
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
