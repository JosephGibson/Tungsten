//! What the props draw over the sprites: a burning crate's warm glow, its
//! char by thirds of its burn, which stays once it is put out, and its
//! flames, three, four past a third and five past two thirds; a burning
//! plank's small glow and one flame; and a frozen crate's or keg's ice
//! cube, fitted to its box.

use glam::Vec2;
use tungsten::core::{AssetRegistry, Transform, World};
use tungsten::render::SpriteBatch;

use super::components::{Crate, Fuel, Keg, Plank};
use super::{BURN_SECS, PLANK_CENTRE};
use crate::features::elements::components::Frozen;
use crate::features::elements::draw::FLAME_FRAMES;
use crate::features::level::draw::{instance, material, push_quad, view};
use crate::features::level::systems::{HALO, HALO_MATERIAL, drawn_time};

/// A prop's frame: its art's side.
const FRAME: f32 = 64.0;
/// A burning crate's glow: its side as a share of the frame's, and its
/// colour, as a burning target's.
const GLOW_SIDE: f32 = 2.2;
const GLOW_COLOR: [u8; 4] = [255, 140, 60, 200];
/// Its flames' frames a second, where each sits on its frame from the
/// top-left in shares of the side, the first three always, and their side
/// as a share of the frame's, growing over the burn's first half.
const FLAME_FPS: f32 = 12.0;
const FLAME_SPOTS: [Vec2; 5] = [
    Vec2::new(0.02, -0.12),
    Vec2::new(0.34, -0.18),
    Vec2::new(0.64, -0.1),
    Vec2::new(0.5, 0.22),
    Vec2::new(0.12, 0.3),
];
const FLAME_SIDE: f32 = 0.5;
const FLAME_GROWTH: f32 = 0.12;
/// A burning plank's glow's side and its one flame's, px, the flame's foot
/// a third of its side under the plank's middle.
const PLANK_GLOW: f32 = 72.0;
const PLANK_FLAME: f32 = 24.0;
/// The props' cubes.
const CRATE_ICE: &str = "ex10_status_ice_crate";
const KEG_ICE: &str = "ex10_status_ice_keg";

/// The burning crates' glows, their chars, their flames, then the frozen
/// props' cubes, each in view.
#[must_use]
pub fn effects(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let shown = |origin: Vec2, reach: f32| {
        (origin + Vec2::splat(FRAME + reach)).cmpge(min).all()
            && (origin - Vec2::splat(reach)).cmple(max).all()
    };
    let crates: Vec<(Vec2, Fuel)> = world
        .query::<(&Crate, &Fuel, &Transform)>()
        .filter(|(_, _, at)| shown(at.position, FRAME))
        .map(|(_, fuel, at)| (at.position, *fuel))
        .collect();
    let planks: Vec<Vec2> = world
        .query::<(&Plank, &Fuel, &Transform)>()
        .filter(|(_, fuel, at)| fuel.lit && shown(at.position, PLANK_GLOW))
        .map(|(_, _, at)| at.position + PLANK_CENTRE)
        .collect();
    let mut batches = Vec::new();
    if let Some(halo) = assets.get_sprite(HALO) {
        let glow = material(world, HALO_MATERIAL);
        let side = FRAME * GLOW_SIDE;
        for &(origin, _) in crates.iter().filter(|(_, fuel)| fuel.lit) {
            let centre = origin + Vec2::splat(FRAME * 0.5);
            let quad = instance(
                halo,
                centre - Vec2::splat(side * 0.5),
                Vec2::splat(side),
                GLOW_COLOR,
            );
            push_quad(&mut batches, halo, quad, glow);
        }
        for &centre in &planks {
            let corner = centre - Vec2::splat(PLANK_GLOW * 0.5);
            let quad = instance(halo, corner, Vec2::splat(PLANK_GLOW), GLOW_COLOR);
            push_quad(&mut batches, halo, quad, glow);
        }
    }
    for &(origin, fuel) in &crates {
        let burnt = 1.0 - fuel.left / BURN_SECS;
        if burnt <= 0.0 {
            continue;
        }
        let stage = ((burnt * 3.0) as usize).min(2);
        if let Some(asset) = assets.get_sprite(&format!("ex10_pine_crate_char_{stage}")) {
            let quad = instance(asset, origin, Vec2::splat(FRAME), [255; 4]);
            push_quad(&mut batches, asset, quad, None);
        }
    }
    let time = drawn_time(world);
    for &(origin, fuel) in crates.iter().filter(|(_, fuel)| fuel.lit) {
        let burnt = (1.0 - fuel.left / BURN_SECS).clamp(0.0, 1.0);
        let count = 3 + usize::from(burnt > 1.0 / 3.0) + usize::from(burnt > 2.0 / 3.0);
        let side = FRAME * (FLAME_SIDE + FLAME_GROWTH * (burnt * 2.0).min(1.0));
        for (i, spot) in FLAME_SPOTS.into_iter().take(count).enumerate() {
            let frame = ((time * FLAME_FPS) as usize + 3 * i) % FLAME_FRAMES.len();
            if let Some(asset) = assets.get_sprite(FLAME_FRAMES[frame]) {
                let quad = instance(asset, origin + spot * FRAME, Vec2::splat(side), [255; 4]);
                push_quad(&mut batches, asset, quad, None);
            }
        }
    }
    for (i, &centre) in planks.iter().enumerate() {
        let frame = ((time * FLAME_FPS) as usize + 3 * i) % FLAME_FRAMES.len();
        if let Some(asset) = assets.get_sprite(FLAME_FRAMES[frame]) {
            let corner = centre - Vec2::new(PLANK_FLAME * 0.5, PLANK_FLAME * (2.0 / 3.0));
            let quad = instance(asset, corner, Vec2::splat(PLANK_FLAME), [255; 4]);
            push_quad(&mut batches, asset, quad, None);
        }
    }
    let cubes = world
        .query::<(&Crate, &Frozen, &Transform)>()
        .map(|(_, _, at)| (CRATE_ICE, at.position))
        .chain(
            world
                .query::<(&Keg, &Frozen, &Transform)>()
                .map(|(_, _, at)| (KEG_ICE, at.position)),
        )
        .filter(|&(_, origin)| shown(origin, 0.0));
    for (name, origin) in cubes {
        if let Some(asset) = assets.get_sprite(name) {
            let quad = instance(asset, origin, Vec2::splat(FRAME), [255; 4]);
            push_quad(&mut batches, asset, quad, None);
        }
    }
    batches
}
