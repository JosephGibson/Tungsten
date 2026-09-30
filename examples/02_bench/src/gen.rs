//! Procedural textures registered under `bench_` IDs, so the suite ships no
//! image files. The `*_rgba` generators are pure; `register*` upload and
//! register their output.

use std::path::PathBuf;

use tungsten::core::assets::UvRect;
use tungsten::core::{AssetRegistry, FilterMode, World};
use tungsten::render::Renderer;

/// Solid white square for walls, floors and other boxes.
pub(crate) const RECT: &str = "bench_rect";
/// White disc with an anti-aliased rim for balls and pegs.
pub(crate) const DISC: &str = "bench_disc";

const RECT_PX: u32 = 4;
const DISC_PX: u32 = 32;

/// Startup hook: upload and register the lean view's textures.
pub(crate) fn register_view_textures(world: &mut World, renderer: &mut Renderer) {
    let rect = vec![255; (RECT_PX * RECT_PX * 4) as usize];
    register(world, renderer, RECT, RECT_PX, FilterMode::Nearest, &rect);
    register(
        world,
        renderer,
        DISC,
        DISC_PX,
        FilterMode::Linear,
        &disc_rgba(DISC_PX),
    );
}

/// Upload one texture and register it as sprite `id`.
pub(crate) fn register(
    world: &mut World,
    renderer: &mut Renderer,
    id: &str,
    size: u32,
    filter: FilterMode,
    rgba: &[u8],
) {
    let handle = renderer.allocate_texture_handle();
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .register_sprite(
            id.to_string(),
            filter,
            size,
            size,
            PathBuf::from(format!("__generated__/{id}.png")),
            handle,
            UvRect::FULL,
            None,
            None,
            None,
        );
    renderer.upload_texture(handle, rgba, size, size, filter);
}

/// White disc, lightly shaded toward the rim, with a one-pixel alpha ramp.
fn disc_rgba(size: u32) -> Vec<u8> {
    let center = size as f32 * 0.5;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let dx = px as f32 + 0.5 - center;
            let dy = py as f32 + 0.5 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = (center - dist).clamp(0.0, 1.0);
            let shade = 1.0 - 0.25 * (dist / center).min(1.0);
            let value = (255.0 * shade).round() as u8;
            rgba.extend_from_slice(&[value, value, value, (255.0 * alpha).round() as u8]);
        }
    }
    rgba
}

/// One square atlas cell: its sprite ID and RGBA pixels.
pub(crate) struct Cell {
    pub(crate) id: String,
    pub(crate) rgba: Vec<u8>,
}

/// One lit atlas cell: its sprite ID and albedo, normal and emissive pixels.
pub(crate) struct LitCell {
    pub(crate) id: String,
    pub(crate) textures: [Vec<u8>; 3],
}

/// A packed page: RGBA, width, height and each cell's UV rect.
type Page = (Vec<u8>, u32, u32, Vec<UvRect>);

/// Packs square `size` px cells row-major into one page. UVs are inset by
/// half a texel, as the manifest loader does.
fn pack(size: u32, cells: &[&[u8]]) -> Page {
    let count = cells.len().max(1) as u32;
    let columns = (count as f32).sqrt().ceil() as u32;
    let rows = count.div_ceil(columns);
    let (width, height) = (columns * size, rows * size);
    let mut page = vec![0; (width * height * 4) as usize];
    let (pw, ph) = (width as f32, height as f32);
    let mut uvs = Vec::with_capacity(cells.len());
    for (index, rgba) in cells.iter().enumerate() {
        let (x0, y0) = (
            (index as u32 % columns) * size,
            (index as u32 / columns) * size,
        );
        for row in 0..size {
            let src = (row * size * 4) as usize;
            let dst = (((y0 + row) * width + x0) * 4) as usize;
            page[dst..dst + (size * 4) as usize]
                .copy_from_slice(&rgba[src..src + (size * 4) as usize]);
        }
        let (x0, y0) = (x0 as f32, y0 as f32);
        uvs.push(UvRect {
            min: [(x0 + 0.5) / pw, (y0 + 0.5) / ph],
            max: [(x0 + size as f32 - 0.5) / pw, (y0 + size as f32 - 0.5) / ph],
        });
    }
    (page, width, height, uvs)
}

/// Packs `cells`, each `size` px square, into one page, uploads it and
/// registers every cell as a sprite on that page, so they share a batch key
/// (`D-048`).
pub(crate) fn register_atlas(
    world: &mut World,
    renderer: &mut Renderer,
    filter: FilterMode,
    size: u32,
    cells: &[Cell],
) {
    let pixels: Vec<&[u8]> = cells.iter().map(|cell| cell.rgba.as_slice()).collect();
    let (page, width, height, uvs) = pack(size, &pixels);
    let handle = renderer.allocate_texture_handle();
    renderer.upload_texture(handle, &page, width, height, filter);
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    for (cell, uv) in cells.iter().zip(uvs) {
        registry.register_sprite(
            cell.id.clone(),
            filter,
            size,
            size,
            PathBuf::from(format!("__generated__/{}.png", cell.id)),
            handle,
            uv,
            None,
            None,
            None,
        );
    }
}

/// `register_atlas` for lit cells: the albedo, normal and emissive pages
/// share one handle and packing (`D-061`), so the cells form one lit batch key.
pub(crate) fn register_lit_atlas(
    world: &mut World,
    renderer: &mut Renderer,
    filter: FilterMode,
    size: u32,
    cells: &[LitCell],
) {
    let [albedo, normal, emissive] = std::array::from_fn(|layer| {
        let pixels: Vec<&[u8]> = cells
            .iter()
            .map(|cell| cell.textures[layer].as_slice())
            .collect();
        pack(size, &pixels)
    });
    let (width, height, uvs) = (albedo.1, albedo.2, albedo.3);
    let handle = renderer.allocate_texture_handle();
    renderer.upload_texture(handle, &albedo.0, width, height, filter);
    renderer.upload_lit_texture(
        handle,
        &albedo.0,
        &normal.0,
        &emissive.0,
        width,
        height,
        filter,
    );
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    for (cell, uv) in cells.iter().zip(uvs) {
        registry.register_sprite(
            cell.id.clone(),
            filter,
            size,
            size,
            PathBuf::from(format!("__generated__/{}.png", cell.id)),
            handle,
            uv,
            Some(PathBuf::from(format!("__generated__/{}_n.png", cell.id))),
            Some(PathBuf::from(format!("__generated__/{}_e.png", cell.id))),
            Some(handle),
        );
    }
}

/// Upload an albedo, normal and emissive set under one handle and register
/// it as lit sprite `id` (`D-061`).
#[allow(clippy::too_many_arguments)] // Mirrors `Renderer::upload_lit_texture`.
pub(crate) fn register_lit(
    world: &mut World,
    renderer: &mut Renderer,
    id: &str,
    size: u32,
    filter: FilterMode,
    albedo: &[u8],
    normal: &[u8],
    emissive: &[u8],
) {
    let handle = renderer.allocate_texture_handle();
    renderer.upload_texture(handle, albedo, size, size, filter);
    renderer.upload_lit_texture(handle, albedo, normal, emissive, size, size, filter);
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .register_sprite(
            id.to_string(),
            filter,
            size,
            size,
            PathBuf::from(format!("__generated__/{id}.png")),
            handle,
            UvRect::FULL,
            Some(PathBuf::from(format!("__generated__/{id}_n.png"))),
            Some(PathBuf::from(format!("__generated__/{id}_e.png"))),
            Some(handle),
        );
}

/// Eight saturated hues for patterns, tiles and animation clips.
const PALETTE: [[f32; 3]; 8] = [
    [0.95, 0.36, 0.30],
    [0.98, 0.72, 0.25],
    [0.55, 0.85, 0.30],
    [0.25, 0.80, 0.70],
    [0.30, 0.55, 0.95],
    [0.60, 0.40, 0.95],
    [0.92, 0.40, 0.75],
    [0.85, 0.85, 0.85],
];

/// Integer hash for deterministic texture noise.
fn hash(x: u32, y: u32, salt: u32) -> u32 {
    let mut h = x
        .wrapping_mul(0x8da6_b343)
        .wrapping_add(y.wrapping_mul(0xd816_3841))
        .wrapping_add(salt.wrapping_mul(0xcb1a_b31f));
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

/// Calls `pixel(u, v, px, py)` for every pixel, with `u, v` in [-1, 1] at
/// pixel centers, and collects the RGBA values `pixel` returns in [0, 1].
fn paint(size: u32, mut pixel: impl FnMut(f32, f32, u32, u32) -> [f32; 4]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let half = size as f32 * 0.5;
    for py in 0..size {
        for px in 0..size {
            let u = (px as f32 + 0.5 - half) / half;
            let v = (py as f32 + 0.5 - half) / half;
            for channel in pixel(u, v, px, py) {
                rgba.push((channel.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    rgba
}

/// Coverage of a shape with signed distance `d` (negative inside), in
/// [-1, 1] units, anti-aliased over one pixel.
fn coverage(d: f32, size: u32) -> f32 {
    (0.5 - d * size as f32 * 0.5).clamp(0.0, 1.0)
}

/// Unlit pattern `kind` modulo 8: disc, ring, checks, stripes, diamond,
/// framed square, cross and blob, each in its own hue.
pub(crate) fn pattern_rgba(kind: u32, size: u32) -> Vec<u8> {
    let [r, g, b] = PALETTE[(kind % 8) as usize];
    paint(size, |u, v, px, py| {
        let radius = (u * u + v * v).sqrt();
        let (d, shade) = match kind % 8 {
            0 => (radius - 0.9, 1.0 - 0.3 * radius),
            1 => ((radius - 0.65).abs() - 0.25, 1.0),
            2 => (
                u.abs().max(v.abs()) - 0.95,
                if (px * 4 / size + py * 4 / size).is_multiple_of(2) {
                    1.0
                } else {
                    0.55
                },
            ),
            3 => (
                u.abs().max(v.abs()) - 0.95,
                if ((px + py) * 6 / size).is_multiple_of(2) {
                    1.0
                } else {
                    0.5
                },
            ),
            4 => (u.abs() + v.abs() - 0.95, 1.0 - 0.4 * (u.abs() + v.abs())),
            5 => (
                u.abs().max(v.abs()) - 0.9,
                if u.abs().max(v.abs()) > 0.7 { 0.6 } else { 1.0 },
            ),
            6 => (u.abs().min(v.abs()) - 0.3, 1.0),
            _ => (radius - 0.75 - 0.15 * (3.0 * v.atan2(u)).sin(), 0.9),
        };
        let alpha = coverage(d, size);
        [r * shade, g * shade, b * shade, alpha]
    })
}

/// White dot with a Gaussian alpha falloff reaching 0 at the rim.
pub(crate) fn soft_dot_rgba(size: u32) -> Vec<u8> {
    paint(size, |u, v, _, _| {
        let r2 = u * u + v * v;
        let alpha = ((-3.0 * r2).exp() - (-3.0f32).exp()).max(0.0) / (1.0 - (-3.0f32).exp());
        [1.0, 1.0, 1.0, alpha]
    })
}

/// Four-pointed white spark.
pub(crate) fn spark_rgba(size: u32) -> Vec<u8> {
    paint(size, |u, v, _, _| {
        let arms = (1.0 - u.abs() * 6.0).max(0.0) * (1.0 - v.abs())
            + (1.0 - v.abs() * 6.0).max(0.0) * (1.0 - u.abs());
        let core = (1.0 - (u * u + v * v).sqrt() * 2.5).max(0.0);
        [1.0, 1.0, 1.0, (arms + core).min(1.0)]
    })
}

/// Lit orb: an albedo disc in `hue`, a sphere normal map and an emissive core.
pub(crate) fn orb_textures(hue: u32, size: u32) -> [Vec<u8>; 3] {
    let [r, g, b] = PALETTE[(hue % 8) as usize];
    let albedo = paint(size, |u, v, _, _| {
        let alpha = coverage((u * u + v * v).sqrt() - 0.92, size);
        [r, g, b, alpha]
    });
    let normal = paint(size, |u, v, _, _| {
        let r2 = (u * u + v * v).min(1.0);
        let n = [u, v, (1.0 - r2).sqrt()];
        [n[0] * 0.5 + 0.5, n[1] * 0.5 + 0.5, n[2] * 0.5 + 0.5, 1.0]
    });
    let emissive = paint(size, |u, v, _, _| {
        let glow = (1.0 - (u * u + v * v).sqrt() / 0.3).max(0.0);
        [glow, glow * 0.7, glow * 0.3, 1.0]
    });
    [albedo, normal, emissive]
}

/// Lit crate: an albedo square with planks in `hue`, a bevelled normal map
/// and an emissive stripe.
pub(crate) fn crate_textures(hue: u32, size: u32) -> [Vec<u8>; 3] {
    let [r, g, b] = PALETTE[(hue % 8) as usize];
    let albedo = paint(size, |u, v, _, py| {
        let plank = if (py * 4 / size).is_multiple_of(2) {
            1.0
        } else {
            0.8
        };
        let alpha = coverage(u.abs().max(v.abs()) - 0.96, size);
        [r * plank, g * plank, b * plank, alpha]
    });
    let bevel = 0.25;
    let normal = paint(size, |u, v, _, _| {
        let tilt = |t: f32| ((t.abs() - (1.0 - bevel)).max(0.0) / bevel) * t.signum();
        let n = glam::Vec3::new(tilt(u), tilt(v), 1.0).normalize();
        [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0]
    });
    let emissive = paint(size, |_, v, _, _| {
        let stripe = (1.0 - v.abs() / 0.12).max(0.0);
        [stripe * 0.2, stripe * 0.8, stripe, 1.0]
    });
    [albedo, normal, emissive]
}

/// Tile `variant` modulo 4: 0 and 1 are opaque ground and stone, 2 and 3
/// translucent grass and moss for the upper layers.
pub(crate) fn tile_rgba(variant: u32, size: u32) -> Vec<u8> {
    let variant = variant % 4;
    paint(size, |u, v, px, py| {
        let noise = (hash(px, py, variant) & 0xff) as f32 / 255.0;
        match variant {
            0 => {
                let shade = 0.8 + 0.2 * noise;
                [0.42 * shade, 0.30 * shade, 0.20 * shade, 1.0]
            }
            1 => {
                let mortar = py % (size / 2).max(1) == 0
                    || (px + if py < size / 2 { 0 } else { size / 2 }) % size == 0;
                let shade = if mortar { 0.45 } else { 0.7 + 0.2 * noise };
                [0.55 * shade, 0.57 * shade, 0.62 * shade, 1.0]
            }
            2 => {
                let blade = (px * 3 + py) % 5 == 0 && v > -0.2;
                let alpha = if blade { 0.9 } else { 0.0 };
                [0.35, 0.75 + 0.2 * noise, 0.30, alpha]
            }
            _ => {
                let alpha = 0.35 * (1.0 - (u * u + v * v).sqrt()).max(0.0) + 0.1 * noise;
                [0.30, 0.55, 0.40, alpha]
            }
        }
    })
}

/// Frame `frame` of `frames` for clip `clip`: a disc in the clip's hue with
/// a spoke that turns once over the clip and a filling wedge.
pub(crate) fn anim_frame_rgba(clip: u32, frame: u32, frames: u32, size: u32) -> Vec<u8> {
    let [r, g, b] = PALETTE[(clip % 8) as usize];
    let progress = (frame as f32 + 0.5) / frames.max(1) as f32;
    let spoke = progress * std::f32::consts::TAU + clip as f32 * 0.7;
    paint(size, |u, v, _, _| {
        let radius = (u * u + v * v).sqrt();
        let alpha = coverage(radius - 0.9, size);
        let angle = v.atan2(u).rem_euclid(std::f32::consts::TAU);
        let filled = angle / std::f32::consts::TAU < progress;
        let along = u * spoke.cos() + v * spoke.sin();
        let across = (-u * spoke.sin() + v * spoke.cos()).abs();
        let on_spoke = along > 0.0 && across < 0.12;
        let shade = if on_spoke {
            1.0
        } else if filled {
            0.75
        } else {
            0.4
        };
        [
            r * shade + 0.2 * f32::from(u8::from(on_spoke)),
            g * shade,
            b * shade,
            alpha,
        ]
    })
}

/// Walker frame `frame` of `frames` in `hue`: a round body on two legs that
/// swing through one stride per clip, with glowing eyes. Returns the albedo,
/// an ellipsoid normal map and the emissive eyes.
pub(crate) fn walker_textures(hue: u32, frame: u32, frames: u32, size: u32) -> [Vec<u8>; 3] {
    let [r, g, b] = PALETTE[(hue % 8) as usize];
    let swing = 0.2 * (frame as f32 / frames.max(1) as f32 * std::f32::consts::TAU).sin();
    // Squared ellipse radius of the body: below 1 inside.
    let body = |u: f32, v: f32| (u / 0.6).powi(2) + ((v + 0.2) / 0.62).powi(2);
    let leg = |u: f32, v: f32| {
        (0.3..0.95).contains(&v)
            && ((u + 0.22 - swing).abs() < 0.1 || (u - 0.22 - swing).abs() < 0.1)
    };
    let eye = |u: f32, v: f32| {
        let dy = (v + 0.32).powi(2);
        ((u - 0.2).powi(2) + dy).min((u + 0.2).powi(2) + dy) < 0.012
    };
    let albedo = paint(size, |u, v, _, _| {
        let radius = body(u, v);
        let alpha = coverage((radius.sqrt() - 1.0) * 0.6, size);
        if eye(u, v) {
            [1.0, 1.0, 0.95, 1.0]
        } else if alpha > 0.0 {
            let shade = 1.0 - 0.3 * radius.min(1.0);
            let alpha = if leg(u, v) { 1.0 } else { alpha };
            [r * shade, g * shade, b * shade, alpha]
        } else if leg(u, v) {
            [r * 0.35, g * 0.35, b * 0.35, 1.0]
        } else {
            [0.0; 4]
        }
    });
    let normal = paint(size, |u, v, _, _| {
        let radius = body(u, v);
        if radius < 1.0 {
            let tilt =
                glam::Vec3::new(u / 0.6, (v + 0.2) / 0.62, (1.0 - radius).sqrt()).normalize();
            [
                tilt.x * 0.5 + 0.5,
                tilt.y * 0.5 + 0.5,
                tilt.z * 0.5 + 0.5,
                1.0,
            ]
        } else {
            [0.5, 0.5, 1.0, 1.0]
        }
    });
    let emissive = paint(size, |u, v, _, _| {
        if eye(u, v) {
            [1.0, 0.85, 0.5, 1.0]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    });
    [albedo, normal, emissive]
}

/// Parallax strip `layer` (0 farthest): a ridge silhouette, opaque below its
/// profile and clear above. The profile repeats across the strip, so strips
/// tile seamlessly side by side.
pub(crate) fn ridge_rgba(layer: u32, size: u32) -> Vec<u8> {
    let depth = layer as f32 / 3.0;
    let color = [0.16 + 0.12 * depth, 0.2 + 0.14 * depth, 0.3 + 0.08 * depth];
    let (k1, k2) = (1.0 + layer as f32, 3.0 + 2.0 * layer as f32);
    paint(size, |u, v, _, _| {
        let x = (u + 1.0) * std::f32::consts::PI;
        let height = 0.35 + 0.25 * (k1 * x + layer as f32).sin() + 0.08 * (k2 * x).sin();
        let alpha = coverage(1.0 - 2.0 * height - v, size);
        let shade = 0.85 + 0.15 * v;
        [color[0] * shade, color[1] * shade, color[2] * shade, alpha]
    })
}
