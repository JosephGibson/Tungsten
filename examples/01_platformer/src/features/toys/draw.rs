//! What the toys draw over the sprites: the shock rings, each black hole's
//! vortex with its infalling streaks, and the sprays' wisps. Quads no
//! entity holds, which the frame's extract builds from the toys' state.

use glam::Vec2;
use tungsten::core::{AssetRegistry, Transform, World};
use tungsten::render::SpriteBatch;

use super::components::{BlackHole, Element, Ring, ShockRing, Spray};
use super::{HOLE_DIAMETER, RING_SECS};
use crate::features::level::draw::{instance, material, push_quad, view};
use crate::features::level::systems::{CORE, CORE_MATERIAL, HALO, HALO_MATERIAL, drawn_time};

/// The rings', the streaks' and the vortex's art.
const SHOCK_RING: &str = "ex10_shock_ring";
const STREAK: &str = "ex10_infall_streak";

/// The rings, the vortices, then the sprays.
#[must_use]
pub fn effects(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let mut batches = rings(world, assets);
    batches.extend(vortices(world, assets));
    batches.extend(sprays(world, assets));
    batches
}

/// Each shock ring, growing and fading: a fireball's eases out with a hot
/// front and a flash, a push's is cool.
fn rings(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let mut batches = Vec::new();
    let Some(asset) = assets.get_sprite(SHOCK_RING) else {
        return batches;
    };
    let flash = material(world, CORE_MATERIAL);
    for (ring, at) in world.query::<(&ShockRing, &Transform)>() {
        let at = at.position;
        let progress = (ring.age / RING_SECS).clamp(0.0, 1.0);
        let fireball = ring.kind == Ring::Fireball;
        let spread = if fireball {
            1.0 - (1.0 - progress).powi(3)
        } else {
            progress
        };
        let size = 32.0 + spread * (ring.size - 32.0);
        let fade = ((1.0 - progress) * 220.0) as u8;
        let color = match ring.kind {
            Ring::Push => [170, 150, 255, fade],
            _ => [255, 210, 140, fade],
        };
        push_quad(
            &mut batches,
            asset,
            instance(
                asset,
                at - Vec2::splat(size / 2.0),
                Vec2::splat(size),
                color,
            ),
            None,
        );
        if fireball {
            let front = size * 0.72;
            let color = [255, 95, 28, ((1.0 - progress).powi(2) * 240.0) as u8];
            push_quad(
                &mut batches,
                asset,
                instance(
                    asset,
                    at - Vec2::splat(front * 0.5),
                    Vec2::splat(front),
                    color,
                ),
                None,
            );
            if let Some(core) = assets.get_sprite(CORE) {
                let left = (1.0 - ring.age / 0.18).max(0.0);
                let size = 100.0 + 180.0 * (1.0 - left);
                let color = [255, 245, 200, (left * 255.0) as u8];
                push_quad(
                    &mut batches,
                    core,
                    instance(core, at - Vec2::splat(size * 0.5), Vec2::splat(size), color),
                    flash,
                );
            }
        }
    }
    batches
}

/// Each black hole: its halo, disk and two vortices turning, the streaks
/// falling in, then its photon ring and core, fading in its last quarter
/// second.
fn vortices(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    const D: f32 = HOLE_DIAMETER;
    let time = drawn_time(world);
    let mut batches = Vec::new();
    let materials = |id: &str| match id {
        HALO => material(world, HALO_MATERIAL),
        CORE => material(world, CORE_MATERIAL),
        _ => None,
    };
    let under: [(&str, f32, f32, [u8; 4]); 4] = [
        (HALO, D * 2.6, 0.0, [110, 80, 255, 150]),
        (
            "ex10_accretion_disk",
            D * 1.6,
            time * 0.9,
            [255, 125, 150, 150],
        ),
        ("ex10_vortex", D * 1.8, -time * 2.8, [95, 125, 255, 200]),
        ("ex10_vortex", D * 1.2, time * 4.5, [190, 170, 255, 225]),
    ];
    let over: [(&str, f32, f32, [u8; 4]); 2] = [
        (
            "ex10_photon_ring",
            D * 0.57,
            time * 6.0,
            [255, 235, 215, 255],
        ),
        ("ex10_vortex_core", D * 0.47, -time * 1.3, [255; 4]),
    ];
    for (hole, at) in world.query::<(&BlackHole, &Transform)>() {
        let at = at.position;
        let fade = (hole.left * 4.0).min(1.0);
        let layer =
            |batches: &mut Vec<SpriteBatch>,
             (id, diameter, rotation, color): (&str, f32, f32, [u8; 4])| {
                let Some(asset) = assets.get_sprite(id) else {
                    return;
                };
                let size = diameter * (1.0 + 0.045 * (time * 7.0).sin());
                let mut quad = instance(
                    asset,
                    at - Vec2::splat(size / 2.0),
                    Vec2::splat(size),
                    color,
                );
                quad.rotation = rotation;
                quad.color[3] = (f32::from(quad.color[3]) * fade) as u8;
                push_quad(batches, asset, quad, materials(id));
            };
        for spec in under {
            layer(&mut batches, spec);
        }
        if let Some(asset) = assets.get_sprite(STREAK) {
            for i in 0..48 {
                let progress = (i as f32 / 48.0 + time * 0.45).fract();
                let radius = (1.0 - progress) * D * 1.05;
                let angle = i as f32 * 2.399_963_2 + time * 3.5 + progress * 5.0;
                let (sin, cos) = angle.sin_cos();
                // Along the spiral: orbiting at 5.75 rad/s while it falls in.
                let heading = Vec2::new(-sin, cos) * radius * 5.75 - Vec2::new(cos, sin) * D * 0.47;
                let size = Vec2::splat(22.0 + progress * 18.0);
                let warm = progress * progress;
                let color = [
                    (150.0 + 105.0 * warm) as u8,
                    (200.0 - 10.0 * warm) as u8,
                    (255.0 - 145.0 * warm) as u8,
                    (230.0 * fade * (std::f32::consts::PI * progress).sin()) as u8,
                ];
                let mut quad = instance(
                    asset,
                    at + Vec2::new(cos, sin) * radius - size / 2.0,
                    size,
                    color,
                );
                quad.rotation = heading.y.atan2(heading.x);
                push_quad(&mut batches, asset, quad, None);
            }
        }
        for spec in over {
            layer(&mut batches, spec);
        }
    }
    batches
}

/// Each spray's soft widening wisps from its muzzle to where it stops,
/// cold blue or fire orange, reaching out over its first frames.
fn sprays(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let mut batches = Vec::new();
    let Some(asset) = assets.get_sprite(CORE) else {
        return batches;
    };
    let glow = material(world, CORE_MATERIAL);
    let (min, max) = view(world);
    for spray in world.query::<&Spray>() {
        let travel = spray.end - spray.start;
        let length = travel.length().min(spray.age * 800.0 + 24.0);
        if length <= 0.01
            || spray.start.cmplt(min - Vec2::splat(600.0)).any()
            || spray.start.cmpgt(max + Vec2::splat(600.0)).any()
        {
            continue;
        }
        let direction = travel.normalize();
        let across = Vec2::new(-direction.y, direction.x);
        let tint = match spray.element {
            Element::Cold => [100, 200, 255],
            Element::Fire => [255, 140, 60],
        };
        for i in 0..6 {
            let progress = (i as f32 + 0.5) / 6.0;
            let distance = length * progress;
            let flutter = (spray.age * 18.0 - i as f32 * 1.7).sin();
            let centre = spray.start + direction * distance + across * flutter * progress * 9.0;
            let size = Vec2::new((length / 6.0 * 1.8).max(32.0), 18.0 + distance * 0.45);
            let alpha = (100.0 - progress * 40.0 + flutter * 10.0) as u8;
            let mut quad = instance(
                asset,
                centre - size * 0.5,
                size,
                [tint[0], tint[1], tint[2], alpha],
            );
            quad.rotation = travel.y.atan2(travel.x);
            push_quad(&mut batches, asset, quad, glow);
        }
    }
    batches
}
