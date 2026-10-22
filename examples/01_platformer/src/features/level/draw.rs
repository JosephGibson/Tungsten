//! What the level draws besides its sprites and tiles: the sky with the
//! moon and the parallax strips behind the map, each area's far strip and
//! the black night sky crossfaded by where the view stands, which a
//! replaced sprite base draws before the engine's default tilemaps and
//! sprites, and the cursor, drawn after them.

use glam::Vec2;
use tungsten::core::{
    AssetRegistry, CameraState, MaterialAssetId, MaterialRegistry, SpriteAsset, World,
    parallax_world_position,
};
use tungsten::kit::cursor_to_world;
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::{WindowSize, extract_sprites_default, extract_tilemaps};

use super::TILE;
use super::components::SceneTime;
use super::systems::{CORE, CORE_MATERIAL, HALO, HALO_MATERIAL, area_shares, drawn_time};
use crate::game::GAMEPLAY;
use crate::shell;

/// The sky, which fills the view, then the strips by depth: each its
/// sprite, its top in world pixels, its scroll factor and its scale.
const BACKDROP: [(&str, f32, f32, Vec2); 5] = [
    ("ex10_sky", 0.0, 0.05, Vec2::splat(2.0)),
    ("ex10_clouds_far", 6.0 * TILE, 0.12, Vec2::new(1.5, 0.8)),
    ("ex10_clouds_near", 9.0 * TILE, 0.22, Vec2::new(1.5, 0.8)),
    ("ex10_distant_ridges", 10.0 * TILE, 0.35, Vec2::splat(2.0)),
    ("ex10_near_woodland", 18.0 * TILE, 0.6, Vec2::splat(2.0)),
];
/// An outdoor area's far strip, between the ridges and the woodland: its
/// top in world pixels, its scroll factor and its scale.
const FAR_STRIP: (f32, f32, Vec2) = (14.0 * TILE, 0.45, Vec2::splat(2.0));
/// The mist bands over the ravine's far side, drawn with its strip: each
/// band's sprite, its top and its scroll factor.
const MIST_STRIP: &str = "ex10_far_cliffs";
const MISTS: [(&str, f32, f32); 3] = [
    ("ex10_mist_0", 17.0 * TILE, 0.5),
    ("ex10_mist_1", 20.0 * TILE, 0.55),
    ("ex10_mist_2", 23.0 * TILE, 0.6),
];
/// The black night sky, over the midnight one where an area's `night` is
/// set, and how bright the clouds draw under it at full night.
pub const SKY_BLACK: &str = "ex10_sky_black";
const NIGHT_CLOUDS: f32 = 0.6;
/// The moon, its halo and its glow, drawn over the sky at a fixed place in
/// the view: its centre as fractions of the view, its diameter a fraction
/// of the view's height, and each disc's size and colour.
pub const MOON: &str = "ex10_moon";
const MOON_AT: Vec2 = Vec2::new(0.81, 0.12);
const MOON_DIAMETER: f32 = 0.135;
const MOON_DISCS: [(&str, f32, [u8; 4]); 3] = [
    (HALO, 2.6, [230, 215, 160, 150]),
    (CORE, 1.35, [255, 237, 183, 100]),
    (MOON, 1.0, [255; 4]),
];
/// Clouds drift at this many world pixels per second times their scroll.
const CLOUD_DRIFT: f32 = 28.0;
/// How far past the view the strips reach, which absorbs the shake.
const OVERHANG: f32 = 2.0 * TILE;
/// The cursor's sprite, drawn this many pixels across on screen.
pub const CURSOR: &str = "ex10_cursor";
const CURSOR_PIXELS: f32 = 32.0;

/// The sprite channel's base: the backdrop while a run is live, then the
/// engine's default, the tilemaps at the far plane and the `Sprite`
/// entities.
#[must_use]
pub fn base(world: &World) -> Vec<SpriteBatch> {
    let mut batches = backdrop(world);
    let mut tiles = extract_tilemaps(world);
    for tile in tiles.iter_mut().flat_map(|batch| &mut batch.instances) {
        tile.z_norm = 1.0;
    }
    batches.extend(tiles);
    batches.extend(extract_sprites_default(world));
    batches
}

/// Adds `quad` of `asset` to `batches`, unlit, drawn through `material`
/// when there is one: onto the last batch when it draws the same way, so
/// a run of quads costs one batch.
pub fn push_quad(
    batches: &mut Vec<SpriteBatch>,
    asset: &SpriteAsset,
    mut quad: SpriteInstance,
    material: Option<MaterialAssetId>,
) {
    quad.z_norm = 0.0;
    let joins = batches.last().is_some_and(|batch| {
        batch.texture == asset.atlas
            && batch.filter == asset.filter
            && batch.material_id == material
            && !batch.lit
    });
    if !joins {
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        batch.material_id = material;
        batches.push(batch);
    }
    if let Some(batch) = batches.last_mut() {
        batch.instances.push(quad);
    }
}

/// The material `name` names in the manifest, when it loaded.
#[must_use]
pub fn material(world: &World, name: &str) -> Option<MaterialAssetId> {
    world.get_resource::<MaterialRegistry>()?.get(name)
}

/// The world box the window shows.
#[must_use]
pub fn view(world: &World) -> (Vec2, Vec2) {
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    camera.visible_world_aabb(window.width as f32, window.height as f32)
}

/// A quad of `asset` from `position`, its top-left, `size` across, tinted
/// `color`, at the far plane.
#[must_use]
pub fn instance(asset: &SpriteAsset, position: Vec2, size: Vec2, color: [u8; 4]) -> SpriteInstance {
    SpriteInstance {
        position: position.to_array(),
        size: size.to_array(),
        rotation: 0.0,
        color,
        uv_min: asset.uv.min,
        uv_size: [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ],
        // At the far plane with the tiles, under every sprite.
        z_norm: 1.0,
        _pad: 0.0,
    }
}

/// The sky and its strips, with the moon over the sky, while the level's
/// clock runs: a run is live. The areas the view stands in set the night
/// sky's share over the midnight one, the clouds' darkening under it, and
/// each far strip's share, drawn after the ridges, with the ravine's mist.
#[must_use]
pub fn backdrop(world: &World) -> Vec<SpriteBatch> {
    let (Some(assets), true) = (
        world.get_resource::<AssetRegistry>(),
        world.has_resource::<SceneTime>(),
    ) else {
        return Vec::new();
    };
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let (min, max) = view(world);
    let overhang = Vec2::splat(OVERHANG);
    let time = drawn_time(world);
    let shares = area_shares(world, (min.x + max.x) * 0.5);
    let night: f32 = shares
        .iter()
        .filter(|(area, _)| area.night)
        .map(|(_, share)| share)
        .sum();
    let alpha = |share: f32| (share.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut batches = Vec::new();
    for (id, top, scroll, scale) in BACKDROP {
        let Some(asset) = assets.get_sprite(id) else {
            continue;
        };
        if id == "ex10_sky" {
            // The sky fills the view rather than stretching a strip of it.
            let size = max - min + overhang * 2.0;
            let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
            batch
                .instances
                .push(instance(asset, min - overhang, size, [255; 4]));
            batches.push(batch);
            if let (Some(black), true) = (assets.get_sprite(SKY_BLACK), night > 0.0) {
                let mut batch = SpriteBatch::new(black.atlas, black.filter);
                let color = [255, 255, 255, alpha(night)];
                batch
                    .instances
                    .push(instance(black, min - overhang, size, color));
                batches.push(batch);
            }
            moon(world, assets, (min, max), &mut batches);
            continue;
        }
        let shade = (255.0 * (1.0 - (1.0 - NIGHT_CLOUDS) * night.clamp(0.0, 1.0))).round() as u8;
        let color = if id.starts_with("ex10_clouds") {
            [shade, shade, shade, 255]
        } else {
            [255; 4]
        };
        let view = (camera.position, min, max, time);
        batches.push(strip(asset, id, (top, scroll, scale), color, view));
        if id != "ex10_distant_ridges" {
            continue;
        }
        for (area, share) in &shares {
            let Some(far) = area.backdrop.as_deref() else {
                continue;
            };
            let color = [255, 255, 255, alpha(*share)];
            if let Some(asset) = assets.get_sprite(far) {
                batches.push(strip(asset, far, FAR_STRIP, color, view));
            }
            if far != MIST_STRIP {
                continue;
            }
            for (mist, top, scroll) in MISTS {
                if let Some(asset) = assets.get_sprite(mist) {
                    batches.push(strip(asset, mist, (top, scroll, FAR_STRIP.2), color, view));
                }
            }
        }
    }
    batches
}

/// A strip of `asset` across the view at `top`, scrolling by `scroll`,
/// `scale` times its size, tinted `color`: a cloud bank drifting, a ridge
/// or far strip with its last row of pixels run on to the view's foot.
fn strip(
    asset: &SpriteAsset,
    id: &str,
    (top, scroll, scale): (f32, f32, Vec2),
    color: [u8; 4],
    (camera, min, max, time): (Vec2, Vec2, Vec2, f32),
) -> SpriteBatch {
    let overhang = Vec2::splat(OVERHANG);
    let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
    let size = Vec2::new(asset.width as f32, asset.height as f32) * scale;
    let cloud = id.starts_with("ex10_clouds");
    // Mist bands are translucent all through: no foot runs on below them.
    let footed = !cloud && !id.starts_with("ex10_mist");
    let drift = if cloud {
        time * scroll * CLOUD_DRIFT
    } else {
        0.0
    };
    let at = parallax_world_position(Vec2::new(drift, top), Vec2::splat(scroll), camera);
    let first = ((min.x - overhang.x - at.x) / size.x).floor() as i32;
    let last = ((max.x + overhang.x - at.x) / size.x).ceil() as i32;
    for col in first..last {
        let position = at + Vec2::new(col as f32 * size.x, 0.0);
        batch.instances.push(instance(asset, position, size, color));
        // The strip's last row of pixels runs on to the view's foot.
        let bottom = position.y + size.y;
        if footed && bottom < max.y + overhang.y {
            let foot = Vec2::new(size.x, max.y + overhang.y - bottom);
            let mut fill = instance(asset, Vec2::new(position.x, bottom), foot, color);
            fill.uv_min[1] =
                asset.uv.max[1] - (asset.uv.max[1] - asset.uv.min[1]) / asset.height as f32 * 0.5;
            fill.uv_size[1] = 0.0;
            batch.instances.push(fill);
        }
    }
    batch
}

/// The moon's discs, round whatever the view's aspect, each glow through
/// its soft material when the manifest has it.
fn moon(
    world: &World,
    assets: &AssetRegistry,
    (min, max): (Vec2, Vec2),
    out: &mut Vec<SpriteBatch>,
) {
    let view = max - min;
    let centre = min + view * MOON_AT;
    let diameter = view.y * MOON_DIAMETER;
    let materials = world.get_resource::<MaterialRegistry>();
    let material = |id: &str| -> Option<MaterialAssetId> {
        let name = match id {
            HALO => HALO_MATERIAL,
            CORE => CORE_MATERIAL,
            _ => return None,
        };
        materials.and_then(|registry| registry.get(name))
    };
    for (id, size, color) in MOON_DISCS {
        let Some(asset) = assets.get_sprite(id) else {
            continue;
        };
        let size = diameter * size;
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        batch.material_id = material(id);
        batch.instances.push(instance(
            asset,
            centre - Vec2::splat(size * 0.5),
            Vec2::splat(size),
            color,
        ));
        out.push(batch);
    }
}

/// The cursor at its world point, a constant size on screen, while
/// gameplay is on top: the shell's screens show the system's pointer.
#[must_use]
pub fn cursor(world: &World) -> Vec<SpriteBatch> {
    if shell::top(world) != Some(GAMEPLAY) {
        return Vec::new();
    }
    let (Some(asset), Some(point), Some(camera)) = (
        world
            .get_resource::<AssetRegistry>()
            .and_then(|assets| assets.get_sprite(CURSOR)),
        cursor_to_world(world),
        world.get_resource::<CameraState>(),
    ) else {
        return Vec::new();
    };
    let size = Vec2::splat(CURSOR_PIXELS / camera.zoom.max(f32::EPSILON));
    let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
    let mut sprite = instance(asset, point - size * 0.5, size, [255; 4]);
    sprite.z_norm = 0.0;
    batch.instances.push(sprite);
    vec![batch]
}
