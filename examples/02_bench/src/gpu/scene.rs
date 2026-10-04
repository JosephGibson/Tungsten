//! `gpu` scene building: textures, the sprite field, glow, lights, the
//! tilemap, text content and the post chain.

use std::f32::consts::TAU;
use std::fmt::Write as _;

use glam::{Vec2, Vec3};
use tungsten::WindowSize;
use tungsten::core::post::{
    BloomParams, ColorAdjustParams, FilmGrainParams, FogParams, GodRaysParams, PostPass, PostStack,
    TonemapParams, VignetteParams,
};
use tungsten::core::{
    AssetRegistry, CameraState, EMPTY_TILE, FilterMode, LayerKind, Light, MaterialAssetId,
    MaterialRegistry, ParallaxLayer, Pcg32, Sprite, SpriteAssetId, TilemapData, TilemapInstance,
    TilemapLayer, TilemapRegistry, Transform, Visibility, World, splitmix64,
};
use tungsten::extract_sprites_default;
use tungsten::render::{Renderer, TextSection};

use super::{GpuState, Kind, MAP_TILES, Params};
use crate::r#gen::{self, Cell};

/// Tilemap registry ID.
const TILEMAP: &str = "bench_tiles";
const HEAVY: &str = "bench_heavy";
const ORB: &str = "bench_gpu_orb";
const CRATE: &str = "bench_gpu_crate";
const GLOW: &str = "bench_glow";
/// Unlit, lit and heavy sprite textures are this size; `sprite_px` scales them.
const TEXTURE_PX: u32 = 32;
const GLOW_TEXTURE_PX: u32 = 64;
const TILE_VARIANTS: u32 = 4;
/// Share of cells each tile layer fills, bottom layer first.
const TILE_DENSITY: [f32; 4] = [1.0, 0.35, 0.2, 0.1];
const FONT: &str = "mono";

fn unlit_id(texture: u8) -> String {
    format!("bench_gpu_tex{texture}")
}

fn tile_id(variant: u32) -> String {
    format!("bench_tile_{variant}")
}

/// Startup hook: textures, the `bench_heavy` loop count, then the sprite
/// field and glow layer, which need the manifest's material ID.
pub(super) fn startup(world: &mut World, renderer: &mut Renderer, params: &Params) {
    for texture in 0..params.textures {
        let filter = if texture.is_multiple_of(2) {
            FilterMode::Nearest
        } else {
            FilterMode::Linear
        };
        let rgba = r#gen::pattern_rgba(texture, TEXTURE_PX);
        r#gen::register(
            world,
            renderer,
            &unlit_id(texture as u8),
            TEXTURE_PX,
            filter,
            &rgba,
        );
    }
    for (id, filter, [albedo, normal, emissive]) in [
        (ORB, FilterMode::Linear, r#gen::orb_textures(1, TEXTURE_PX)),
        (
            CRATE,
            FilterMode::Nearest,
            r#gen::crate_textures(4, TEXTURE_PX),
        ),
    ] {
        r#gen::register_lit(
            world, renderer, id, TEXTURE_PX, filter, &albedo, &normal, &emissive,
        );
    }
    let glow = r#gen::soft_dot_rgba(GLOW_TEXTURE_PX);
    r#gen::register(
        world,
        renderer,
        GLOW,
        GLOW_TEXTURE_PX,
        FilterMode::Linear,
        &glow,
    );
    if params.tile_layers > 0 {
        let cells: Vec<Cell> = (0..TILE_VARIANTS)
            .map(|variant| Cell {
                id: tile_id(variant),
                rgba: r#gen::tile_rgba(variant, params.tile_px),
            })
            .collect();
        r#gen::register_atlas(world, renderer, FilterMode::Nearest, params.tile_px, &cells);
        // After manifest composition, which replaces the registry.
        world
            .get_resource_mut::<TilemapRegistry>()
            .expect("TilemapRegistry resource missing")
            .insert(TILEMAP.to_string(), tilemap(params, MAP_TILES));
        let entity = world.spawn();
        world.insert(entity, TilemapInstance::new(TILEMAP, Vec2::ZERO));
    }
    let heavy = heavy_material(world, renderer, params.shader_iters);
    spawn_field(world, params, heavy);

    let (sprites, glow, _) = params.batches();
    let produced = extract_sprites_default(world).len() as u32;
    if produced == sprites + glow {
        log::info!("gpu: the sprite extract makes {produced} batches, as expected");
    } else {
        log::warn!(
            "gpu: the sprite extract makes {produced} batches; bench-config expects {}",
            sprites + glow
        );
    }
}

/// Writes `iterations` into `bench_heavy`'s authored `i32s[0]` and rebuilds
/// its pipeline, so every heavy sprite shares one UBO and needs no
/// per-entity override block.
fn heavy_material(world: &mut World, renderer: &mut Renderer, iterations: i32) -> MaterialAssetId {
    let registry = world
        .get_resource_mut::<MaterialRegistry>()
        .expect("MaterialRegistry resource missing");
    let id = registry
        .get(HEAVY)
        .expect("examples/02_bench/assets/manifest.json provides bench_heavy");
    let mut defaults = registry.defaults_for_id(id).expect("bench_heavy defaults");
    defaults.i32s[0] = iterations;
    let path = registry
        .path_for_id(id)
        .expect("bench_heavy path")
        .to_path_buf();
    let shader = registry
        .shader_name_for_id(id)
        .expect("bench_heavy shader")
        .to_string();
    registry.allocate(HEAVY, path, shader, defaults);
    if let Err(err) = renderer.reload_material(id, defaults) {
        panic!("rebuilding bench_heavy with {iterations} iterations failed: {err}");
    }
    id
}

/// The world registry's ID for sprite `name`.
fn sprite_id(world: &mut World, name: &str) -> SpriteAssetId {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprite(name)
}

/// Spawns the sprite field (z layers `0..z_layers`) and the glow layer
/// above it, both screen-locked through `ParallaxLayer` factor 0.
fn spawn_field(world: &mut World, params: &Params, heavy: MaterialAssetId) {
    let mut rng = Pcg32::seeded(params.seed);
    let view = params.view();
    let place = |rng: &mut Pcg32, size: f32| {
        Vec2::new(
            rng.next_f32_unit() * (view.x - size).max(0.0),
            rng.next_f32_unit() * (view.y - size).max(0.0),
        )
    };
    let sprite_scale = Vec2::splat(params.sprite_px / TEXTURE_PX as f32);
    for index in 0..params.sprites {
        let position = place(&mut rng, params.sprite_px);
        let (asset, color, material) = match params.kind(index) {
            Kind::Unlit(texture) => {
                let mut tint = [255; 4];
                for channel in &mut tint[..3] {
                    *channel = 160 + (rng.next_u32() % 96) as u8;
                }
                (unlit_id(texture), tint, None)
            }
            Kind::Orb => (ORB.to_string(), [255; 4], None),
            Kind::Crate => (CRATE.to_string(), [255; 4], None),
            Kind::Heavy => (unlit_id(0), [255; 4], Some(heavy)),
        };
        let mut sprite = Sprite::new(sprite_id(world, &asset));
        sprite.color = color;
        sprite.z_order = (index % params.z_layers) as i32;
        sprite.material_id = material;
        spawn_screen_locked(world, position, sprite_scale, sprite);
    }
    let glow_scale = Vec2::splat(params.glow_px / GLOW_TEXTURE_PX as f32);
    let glow = sprite_id(world, GLOW);
    for _ in 0..params.glow {
        let position = place(&mut rng, params.glow_px);
        let mut sprite = Sprite::new(glow);
        let green = 150 + (rng.next_u32() % 90) as u8;
        sprite.color = [255, green, 110, 56];
        sprite.z_order = params.z_layers as i32;
        spawn_screen_locked(world, position, glow_scale, sprite);
    }
}

fn spawn_screen_locked(world: &mut World, position: Vec2, scale: Vec2, sprite: Sprite) {
    let entity = world.spawn();
    world.insert(
        entity,
        Transform {
            position,
            rotation: 0.0,
            scale,
        },
    );
    world.insert(entity, sprite);
    world.insert(entity, Visibility::default());
    world.insert(entity, ParallaxLayer::new(Vec2::ZERO));
}

/// A point light's Lissajous path, relative to the camera's top-left.
#[derive(Debug, Clone, Copy)]
pub(super) struct LightPath {
    center: Vec2,
    amplitude: Vec2,
    rate: Vec2,
    phase: Vec2,
}

const LIGHT_COLORS: [Vec3; 4] = [
    Vec3::new(1.0, 0.65, 0.3),
    Vec3::new(0.35, 0.6, 1.0),
    Vec3::new(0.5, 1.0, 0.45),
    Vec3::new(1.0, 0.4, 0.8),
];

/// `lights` moving point lights plus one directional light.
pub(super) fn spawn_lights(world: &mut World, params: &Params) {
    let mut rng = Pcg32::seeded(splitmix64(params.seed ^ 0x11_6417));
    let view = params.view();
    for index in 0..params.lights {
        let path = LightPath {
            center: Vec2::new(rng.next_f32_unit(), rng.next_f32_unit()) * view,
            amplitude: Vec2::new(rng.next_range(0.1, 0.35), rng.next_range(0.1, 0.35)) * view,
            rate: Vec2::new(rng.next_range(0.2, 0.9), rng.next_range(0.2, 0.9)),
            phase: Vec2::new(rng.next_range(0.0, TAU), rng.next_range(0.0, TAU)),
        };
        let entity = world.spawn();
        world.insert(entity, Transform::from_position(path.center));
        world.insert(
            entity,
            Light::point(LIGHT_COLORS[index as usize % 4], view.x * 0.3),
        );
        world.insert(entity, path);
    }
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, Light::directional(Vec3::splat(0.35), -0.8));
}

/// System: moves the point lights along their paths, relative to the camera.
pub(super) fn move_lights(world: &mut World) {
    let Some(elapsed) = world.get_resource::<GpuState>().map(|state| state.elapsed) else {
        return;
    };
    let origin = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);
    for (_, transform, path) in world.query2_mut::<Transform, LightPath>() {
        let angle = path.rate * elapsed + path.phase;
        transform.position =
            origin + path.center + path.amplitude * Vec2::new(angle.x.sin(), angle.y.cos());
    }
}

/// The render-only tilemap: layer 0 fills every cell with opaque ground or
/// stone; upper layers scatter translucent grass and moss.
fn tilemap(params: &Params, (width, height): (u32, u32)) -> TilemapData {
    let cells = (width * height) as usize;
    let layers = (0..params.tile_layers)
        .map(|layer| {
            let mut rng = Pcg32::seeded(splitmix64(params.seed ^ (0x7117 + u64::from(layer))));
            let density = TILE_DENSITY[layer as usize];
            let tiles = (0..cells)
                .map(|_| {
                    let roll = rng.next_u32();
                    let variant = (roll & 1) as i32;
                    if layer == 0 {
                        variant
                    } else if (roll >> 8) as f32 / 16_777_216.0 < density {
                        2 + variant
                    } else {
                        EMPTY_TILE
                    }
                })
                .collect();
            TilemapLayer {
                name: format!("layer{layer}"),
                kind: LayerKind::Render,
                tiles,
            }
        })
        .collect();
    TilemapData {
        tile_width: params.tile_px,
        tile_height: params.tile_px,
        width,
        height,
        tileset: (0..TILE_VARIANTS).map(tile_id).collect(),
        layers,
    }
}

/// Non-empty render-layer tiles inside the camera's view, culled as
/// `extract_tilemaps` culls them.
pub(super) fn visible_tiles(world: &World) -> u32 {
    let (Some(maps), Some(camera), Some(window)) = (
        world.get_resource::<TilemapRegistry>(),
        world.get_resource::<CameraState>(),
        world.get_resource::<WindowSize>(),
    ) else {
        return 0;
    };
    let (view_min, view_max) = camera.visible_world_aabb(window.width as f32, window.height as f32);
    let mut count = 0;
    for (_, instance) in world.query::<TilemapInstance>() {
        let Some(map) = maps.get(&instance.id) else {
            continue;
        };
        let (tw, th) = (map.tile_width as f32, map.tile_height as f32);
        let local_min = view_min - instance.origin;
        let local_max = view_max - instance.origin;
        let col_start = (local_min.x / tw).floor().max(0.0) as u32;
        let row_start = (local_min.y / th).floor().max(0.0) as u32;
        let col_end = ((local_max.x / tw).ceil().max(0.0) as u32).min(map.width);
        let row_end = ((local_max.y / th).ceil().max(0.0) as u32).min(map.height);
        if col_start >= col_end || row_start >= row_end {
            continue;
        }
        for layer in map
            .layers
            .iter()
            .filter(|layer| layer.kind == LayerKind::Render)
        {
            for row in row_start..row_end {
                let start = (row * map.width) as usize;
                count += layer.tiles[start + col_start as usize..start + col_end as usize]
                    .iter()
                    .filter(|&&tile| tile >= 0)
                    .count() as u32;
            }
        }
    }
    count
}

/// Section `section` as of `frame`: its index, the frame and hash digits,
/// cut to the section's character count. Sections fill 360 px columns of
/// 20 px rows; once the columns fill the width, the next page shifts down
/// 7 px and overlaps.
pub(super) fn text_section(params: &Params, section: u32, frame: u32) -> TextSection {
    const MARGIN: u32 = 16;
    const COLUMN_PX: u32 = 360;
    const ROW_PX: u32 = 20;
    let (width, height) = params.resolution;
    let rows = (height.saturating_sub(2 * MARGIN) / ROW_PX).max(1);
    let columns = (width.saturating_sub(2 * MARGIN) / COLUMN_PX).max(1);
    let column = section / rows;
    let page = column / columns;
    let x = MARGIN + (column % columns) * COLUMN_PX;
    let y = MARGIN + (section % rows) * ROW_PX + page * 7 % ROW_PX;

    let chars = params.section_chars(section) as usize;
    let mut content = format!("{section:04} f{frame:06}");
    let mut hash = splitmix64((u64::from(section) << 32) | u64::from(frame));
    while content.len() < chars {
        let _ = write!(content, " {:08x}", hash as u32);
        hash = splitmix64(hash);
    }
    content.truncate(chars);
    TextSection {
        content,
        font_id: FONT.to_string(),
        font_size: 14.0,
        line_height: 18.0,
        color: [230, 236, 255, 255],
        position: [x as f32, y as f32],
        bounds: None,
        ..Default::default()
    }
}

pub(super) fn post_stack(post: &str) -> PostStack {
    let bloom = PostPass::Bloom(BloomParams {
        threshold: 0.6,
        knee: 0.3,
        intensity: 0.8,
        radius: 1.0,
    });
    let vignette = PostPass::Vignette(VignetteParams::default());
    PostStack(match post {
        "none" => Vec::new(),
        "light" => vec![bloom, vignette],
        _ => vec![
            bloom,
            PostPass::GodRays(GodRaysParams::default()),
            PostPass::Fog(FogParams::default()),
            PostPass::ColorAdjust(ColorAdjustParams {
                hue: 0.0,
                saturation: 1.1,
                contrast: 1.05,
            }),
            PostPass::Tonemap(TonemapParams::default()),
            PostPass::ChromaticAberration(2.0),
            PostPass::FilmGrain(FilmGrainParams::default()),
            vignette,
        ],
    })
}
