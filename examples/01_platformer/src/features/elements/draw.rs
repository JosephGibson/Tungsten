//! What the element rules draw over the sprites: each exposed burning
//! ball's flame tongue over a soft glow, each frozen ball's ice shell, and
//! the cold pulses' rings.

use glam::Vec2;
use tungsten::core::{AssetRegistry, Collider, Entity, Sprite, Transform, World};
use tungsten::kit::Tags;
use tungsten::render::SpriteBatch;

use super::FLAME_SIZE;
use super::components::{Flame, IcePulse, PulseGlow};
use super::systems::{burn_left, flame_look};
use crate::features::combat::FROZEN;
use crate::features::level::draw::{instance, material, push_quad, view};
use crate::features::level::systems::{CORE, CORE_MATERIAL, drawn_time};
use crate::shared::Saved;

/// A burning ball's flame frames: their art is drawn in 2 × 2 pixel
/// blocks, so a [`FLAME_SIZE`] quad samples it exactly.
pub const FLAME_FRAMES: [&str; 8] = [
    "ex10_fire_0",
    "ex10_fire_1",
    "ex10_fire_2",
    "ex10_fire_3",
    "ex10_fire_4",
    "ex10_fire_5",
    "ex10_fire_6",
    "ex10_fire_7",
];
/// A cold pulse's ring, and a frozen ball's shell over its art.
pub const ICE_RING: &str = "ex10_ice_ring";
pub const ICE_SHELL: &str = "ex10_ice_shell";

/// The flames, then the ice shells, then the cold pulses.
#[must_use]
pub fn effects(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let mut batches = flames(world, assets);
    batches.extend(shells(world, assets));
    batches.extend(pulses(world, assets));
    batches
}

/// A shell of ice over each frozen ball in view, over its 64 px art at the
/// ball's scale.
fn shells(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let Some(asset) = assets.get_sprite(ICE_SHELL) else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let mut batches = Vec::new();
    for (tags, _, at) in
        world.query::<(&Tags, &crate::features::toys::components::Ball, &Transform)>()
    {
        // In view first: a burning pile's balls carry tags too, and those
        // off screen are not asked theirs.
        let size = Vec2::splat(64.0) * at.scale;
        if (at.position + size).cmplt(min).any() || at.position.cmpgt(max).any() {
            continue;
        }
        if !tags.has(FROZEN) {
            continue;
        }
        push_quad(
            &mut batches,
            asset,
            instance(asset, at.position, size, [255; 4]),
            None,
        );
    }
    batches
}

/// A tongue on each exposed burning ball in view, over its glow: every
/// glow first, so a burning crest costs two batches, not two per ball.
fn flames(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let (min, max) = view(world);
    let time = drawn_time(world);
    let reach = Vec2::splat(FLAME_SIZE);
    let lit: Vec<_> = world
        .query::<(Entity, &Flame, &Saved, &Transform, &Collider)>()
        .filter_map(|(ball, flame, id, at, collider)| {
            // A buried ball, or one out of view, draws none: its burn's
            // seconds are read off its statuses only past both.
            let centre = at.position + collider.offset;
            if !flame.exposed
                || (centre + reach).cmplt(min).any()
                || (centre - reach).cmpgt(max).any()
            {
                return None;
            }
            let left = burn_left(world, ball)?;
            let look = flame_look(id.id(), left, flame.exposed, time)?;
            Some((centre, look))
        })
        .collect();
    let mut batches = Vec::new();
    if let Some(asset) = assets.get_sprite(CORE) {
        let glow = material(world, CORE_MATERIAL);
        for &(centre, look) in &lit {
            let size = Vec2::splat(look.glow_size);
            let quad = instance(
                asset,
                centre - size * 0.5 - Vec2::new(0.0, 4.0),
                size,
                look.glow,
            );
            push_quad(&mut batches, asset, quad, glow);
        }
    }
    let frames = FLAME_FRAMES.map(|id| assets.get_sprite(id));
    for (centre, look) in lit {
        let Some(asset) = frames[look.frame] else {
            continue;
        };
        // The tongue's base sits just below the ball's centre, wrapping its top.
        let origin = centre + Vec2::new(-0.5 * FLAME_SIZE, 6.0 - FLAME_SIZE);
        let mut quad = instance(asset, origin, Vec2::splat(FLAME_SIZE), look.color);
        if look.mirror {
            quad.uv_min[0] += quad.uv_size[0];
            quad.uv_size[0] = -quad.uv_size[0];
        }
        push_quad(&mut batches, asset, quad, None);
    }
    batches
}

/// Each cold pulse in view, from its tweened looks: its fading glow, then
/// its turning ring, which a freeze grows a little and a shatter flings
/// out, each its sprite's size times its scale at its top-left.
fn pulses(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let (min, max) = view(world);
    let mut batches = Vec::new();
    for (glow, PulseGlow(ring)) in world.query::<(Entity, &PulseGlow)>() {
        let Some(pulse) = world.get::<IcePulse>(*ring) else {
            continue;
        };
        let reach = Vec2::splat(pulse.size * 1.3);
        if (pulse.at + reach).cmplt(min).any() || (pulse.at - reach).cmpgt(max).any() {
            continue;
        }
        for look in [glow, *ring] {
            drawn(&mut batches, world, assets, look);
        }
    }
    batches
}

/// A pulse's look as its tween left it: its sprite's size times its scale
/// at its top-left, turned, tinted and in its material.
fn drawn(batches: &mut Vec<SpriteBatch>, world: &World, assets: &AssetRegistry, entity: Entity) {
    let (Some(sprite), Some(at)) = (world.get::<Sprite>(entity), world.get::<Transform>(entity))
    else {
        return;
    };
    let Some(art) = assets.sprite(sprite.asset_id) else {
        return;
    };
    let size = Vec2::new(art.width as f32, art.height as f32) * at.scale;
    let mut quad = instance(art, at.position, size, sprite.color);
    quad.rotation = at.rotation;
    push_quad(batches, art, quad, sprite.material_id);
}
