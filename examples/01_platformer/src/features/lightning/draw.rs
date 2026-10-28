//! What the lightning draws over the sprites: on a strike's first frame a
//! veil over the view; each strike's bolt, its segments stacked from the
//! view's top down to where it landed; and each jump's arc, a zigzag of
//! thin quads, each turned along its stretch, no raster turned.

use glam::Vec2;
use tungsten::core::{AssetRegistry, SpriteAsset, World};
use tungsten::render::{SpriteBatch, SpriteInstance};

use super::components::{Arc, Bolt};
use super::{ARC_JITTER, ARC_SECS, EXITS, SEGMENT};
use crate::features::level::draw::{instance, push_quad, view};

/// The bolt's segments, by piece.
pub const BOLT_PIECES: [&str; 8] = [
    "ex10_bolt_0",
    "ex10_bolt_1",
    "ex10_bolt_2",
    "ex10_bolt_3",
    "ex10_bolt_4",
    "ex10_bolt_5",
    "ex10_bolt_6",
    "ex10_bolt_7",
];
/// The art the veil and the arcs take a solid texel from: the tracer's
/// white-hot head.
pub const TRACER: &str = "ex10_tracer";
/// The veil's near-white violet, and an arc's violet glow and white core.
pub const VEIL: [u8; 4] = [236, 228, 255, 80];
pub const GLOW: [u8; 4] = [184, 140, 255, 110];
pub const CORE: [u8; 4] = [255, 255, 255, 255];
/// An arc's core and glow widths, px, and how far its zigzag's corners
/// swing aside at most.
const CORE_WIDTH: f32 = 2.0;
const GLOW_WIDTH: f32 = 6.0;
const KINK: f32 = 10.0;

/// The veil, then the bolts, then the arcs.
#[must_use]
pub fn bolts(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let (min, max) = view(world);
    let mut batches = Vec::new();
    let solid = assets.get_sprite(TRACER);
    if let Some(solid) = solid
        && world.query::<&Bolt>().any(|bolt| bolt.frames == 1)
    {
        push_quad(
            &mut batches,
            solid,
            filled(solid, min, max - min, VEIL),
            None,
        );
    }
    for bolt in world.query::<&Bolt>() {
        stack(&mut batches, assets, bolt, min.y);
    }
    if let Some(solid) = solid {
        for arc in world.query::<&Arc>() {
            zigzag(&mut batches, solid, arc);
        }
    }
    batches
}

/// A bolt's segments from the bottom up: the last leaves where it landed
/// and each above it where the one under it enters, at the whole scale
/// that lets the stack reach the view's top.
fn stack(batches: &mut Vec<SpriteBatch>, assets: &AssetRegistry, bolt: &Bolt, top: f32) {
    let reach = bolt.landing.y - top;
    if reach <= 0.0 || bolt.pieces.is_empty() {
        return;
    }
    let scale = (reach / (bolt.pieces.len() as f32 * SEGMENT.y))
        .ceil()
        .max(1.0);
    let size = SEGMENT * scale;
    let mut bottom = bolt.landing;
    for &piece in bolt.pieces.iter().rev() {
        let piece = piece % BOLT_PIECES.len();
        // Each piece enters at its top centre and leaves its exit aside.
        let entry = Vec2::new(bottom.x - EXITS[piece] * scale, bottom.y - size.y);
        if let Some(asset) = assets.get_sprite(BOLT_PIECES[piece]) {
            let corner = Vec2::new(entry.x - size.x * 0.5, entry.y);
            push_quad(
                batches,
                asset,
                instance(asset, corner, size, [255; 4]),
                None,
            );
        }
        bottom = entry;
    }
}

/// An arc's zigzag, its glow then its core, its corners on a new path every
/// [`ARC_JITTER`] and fading over its last third.
fn zigzag(batches: &mut Vec<SpriteBatch>, solid: &SpriteAsset, arc: &Arc) {
    let span = arc.to - arc.from;
    let length = span.length();
    if length < 1.0 {
        return;
    }
    let aside = (span / length).perp();
    let corners = ((length / 32.0) as usize).clamp(2, 10);
    let seed = arc.seed.wrapping_add((arc.age / ARC_JITTER) as u64);
    let mut points = vec![arc.from];
    for k in 1..corners {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let swing = side * KINK * (0.5 + 0.5 * unit(seed, k));
        points.push(arc.from + span * (k as f32 / corners as f32) + aside * swing);
    }
    points.push(arc.to);
    let left = ((ARC_SECS - arc.age) / (ARC_SECS / 3.0)).clamp(0.0, 1.0);
    for (width, color) in [(GLOW_WIDTH, GLOW), (CORE_WIDTH, CORE)] {
        let mut color = color;
        color[3] = (f32::from(color[3]) * left).round() as u8;
        for pair in points.windows(2) {
            // A stretch's quad, a width longer so the corners close.
            let stretch = pair[1] - pair[0];
            let size = Vec2::new(stretch.length() + width, width);
            let corner = (pair[0] + pair[1]) * 0.5 - size * 0.5;
            let mut quad = filled(solid, corner, size, color);
            quad.rotation = stretch.y.atan2(stretch.x);
            push_quad(batches, solid, quad, None);
        }
    }
}

/// A quad of one colour: every pixel samples the tracer's solid texel.
fn filled(asset: &SpriteAsset, corner: Vec2, size: Vec2, color: [u8; 4]) -> SpriteInstance {
    let mut quad = instance(asset, corner, size, color);
    let span = [
        asset.uv.max[0] - asset.uv.min[0],
        asset.uv.max[1] - asset.uv.min[1],
    ];
    // The head's texel (29, 1) of the tracer's 32 × 3.
    quad.uv_min = [
        asset.uv.min[0] + span[0] * (29.5 / 32.0),
        asset.uv.min[1] + span[1] * 0.5,
    ];
    quad.uv_size = [0.0, 0.0];
    quad
}

/// A number in `[0, 1)` from `seed` and `k`.
fn unit(seed: u64, k: usize) -> f32 {
    let mut x =
        seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (k as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 31;
    (x % 1000) as f32 / 1000.0
}
