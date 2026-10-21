//! What the element rules draw over the sprites: each exposed burning
//! ball's flame tongue over a soft glow, and the cold pulses' rings.

use glam::Vec2;
use tungsten::core::{AssetRegistry, Collider, Transform, World};
use tungsten::render::SpriteBatch;

use super::FLAME_SIZE;
use super::components::{BallBurn, IcePulse};
use super::systems::flame_look;
use crate::features::level::draw::{instance, material, push_quad, view};
use crate::features::level::systems::{CORE, CORE_MATERIAL, drawn_time};
use crate::shared::SaveId;

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
/// A cold pulse's ring.
pub const ICE_RING: &str = "ex10_ice_ring";

/// The flames, then the cold pulses.
#[must_use]
pub fn effects(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let mut batches = flames(world, assets);
    batches.extend(pulses(world, assets));
    batches
}

/// A tongue on each exposed burning ball in view, over its glow: every
/// glow first, so a burning crest costs two batches, not two per ball.
fn flames(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let (min, max) = view(world);
    let time = drawn_time(world);
    let reach = Vec2::splat(FLAME_SIZE);
    let lit: Vec<_> = world
        .query::<(&BallBurn, &SaveId, &Transform, &Collider)>()
        .filter_map(|(burn, id, at, collider)| {
            let look = flame_look(*id, *burn, time)?;
            let centre = at.position + collider.offset;
            ((centre + reach).cmpge(min).all() && (centre - reach).cmple(max).all())
                .then_some((centre, look))
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

/// Each cold pulse in view: a fading glow and its turning ring, which a
/// freeze grows a little and a shatter flings out.
fn pulses(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let (min, max) = view(world);
    let glow = material(world, CORE_MATERIAL);
    let mut batches = Vec::new();
    for pulse in world.query::<&IcePulse>() {
        let progress = (pulse.age / pulse.duration()).clamp(0.0, 1.0);
        let reach = Vec2::splat(pulse.size * 1.3);
        if (pulse.at + reach).cmplt(min).any() || (pulse.at - reach).cmpgt(max).any() {
            continue;
        }
        let left = 1.0 - progress;
        if let Some(asset) = assets.get_sprite(CORE) {
            let size = Vec2::splat(pulse.size * (1.5 + progress * 0.8));
            let color = [160, 230, 255, (150.0 * left * left) as u8];
            push_quad(
                &mut batches,
                asset,
                instance(asset, pulse.at - size * 0.5, size, color),
                glow,
            );
        }
        if let Some(asset) = assets.get_sprite(ICE_RING) {
            let growth = if pulse.shatter {
                1.0 + progress * 1.5
            } else {
                1.3 + progress * 0.5
            };
            let size = Vec2::splat(pulse.size * growth);
            let color = [190, 245, 255, (210.0 * left * left) as u8];
            let mut ring = instance(asset, pulse.at - size * 0.5, size, color);
            ring.rotation = pulse.age * if pulse.shatter { -1.5 } else { 0.5 };
            push_quad(&mut batches, asset, ring, None);
        }
    }
    batches
}
