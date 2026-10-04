//! `integrated` assets, registered in the startup hook because manifest
//! composition replaces the tilemap, animation and particle registries
//! before it runs: generated textures, the level tilemap, walk clips and the
//! fire and spark particle configs.

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec2;
use tungsten::core::assets::AnimationFrame;
use tungsten::core::{
    AnimationData, AnimationRegistry, AssetId, AssetRegistry, BlendMode, Curve, EmissionKind,
    FilterMode, InitialVelocity, LayerKind, ParticleConfig, ParticleConfigRegistry, ParticleRender,
    Range, TilemapData, TilemapInstance, TilemapLayer, TilemapRegistry, World,
};
use tungsten::render::Renderer;

use super::FIRE_MAX_ALIVE;
use super::level::{Level, ROWS, TILE};
use crate::r#gen::{self, Cell, LitCell};

/// Texture size of walkers, props, crates, pickups and torch poles.
pub(super) const SPRITE_PX: u32 = 24;
pub(super) const PARTICLE_PX: u32 = 16;
pub(super) const RIDGE_PX: u32 = 128;
pub(super) const RIDGE_LAYERS: usize = 4;
pub(super) const VARIANTS: u8 = 4;
const WALKER_HUES: [u32; 4] = [0, 2, 4, 5];
pub(super) const WALK_FRAMES: u32 = 6;
pub(super) const WALK_FRAME_MS: u32 = 90;
const TILEMAP: &str = "bench_int_level";
pub(super) const PROP_LIT: [&str; 3] = ["bench_int_orb", "bench_int_box", "bench_int_lamp"];
pub(super) const PROP_UNLIT: [&str; 3] =
    ["bench_int_plant0", "bench_int_plant1", "bench_int_plant2"];
pub(super) const CRATE: &str = "bench_int_crate";
pub(super) const GEM: &str = "bench_int_gem";
pub(super) const POLE: &str = "bench_int_pole";
const FIRE: &str = "bench_int_fire";
const SPARK: &str = "bench_int_spark";
pub(super) const BOLT: &str = "bench_int_bolt";

pub(super) fn walk_clip(variant: u8, flat: bool) -> String {
    let suffix = if flat { "_flat" } else { "" };
    format!("bench_int_walk{variant}{suffix}")
}

pub(super) fn walk_frame(variant: u8, frame: usize, flat: bool) -> String {
    let suffix = if flat { "_flat" } else { "" };
    format!("bench_int_walk{variant}_{frame}{suffix}")
}

fn tile_id(variant: u32) -> String {
    format!("bench_int_tile{variant}")
}

pub(super) fn ridge_id(layer: usize) -> String {
    format!("bench_int_ridge{layer}")
}

pub(super) fn register_textures(world: &mut World, renderer: &mut Renderer) {
    let mut lit = Vec::new();
    let mut flat = Vec::new();
    for variant in 0..VARIANTS {
        for frame in 0..WALK_FRAMES {
            let hue = WALKER_HUES[variant as usize];
            let textures = r#gen::walker_textures(hue, frame, WALK_FRAMES, SPRITE_PX);
            flat.push(Cell {
                id: walk_frame(variant, frame as usize, true),
                rgba: textures[0].clone(),
            });
            lit.push(LitCell {
                id: walk_frame(variant, frame as usize, false),
                textures,
            });
        }
    }
    r#gen::register_lit_atlas(world, renderer, FilterMode::Linear, SPRITE_PX, &lit);
    r#gen::register_atlas(world, renderer, FilterMode::Linear, SPRITE_PX, &flat);

    let props = [
        (PROP_LIT[0], r#gen::orb_textures(1, SPRITE_PX)),
        (PROP_LIT[1], r#gen::crate_textures(4, SPRITE_PX)),
        (PROP_LIT[2], r#gen::orb_textures(6, SPRITE_PX)),
        (CRATE, r#gen::crate_textures(1, SPRITE_PX)),
    ]
    .map(|(id, textures)| LitCell {
        id: id.to_string(),
        textures,
    });
    r#gen::register_lit_atlas(world, renderer, FilterMode::Nearest, SPRITE_PX, &props);

    let unlit = [
        (PROP_UNLIT[0], 2),
        (PROP_UNLIT[1], 3),
        (PROP_UNLIT[2], 7),
        (GEM, 4),
        (POLE, 5),
    ]
    .map(|(id, kind)| Cell {
        id: id.to_string(),
        rgba: r#gen::pattern_rgba(kind, SPRITE_PX),
    });
    r#gen::register_atlas(world, renderer, FilterMode::Linear, SPRITE_PX, &unlit);

    let particles = [
        (FIRE, r#gen::soft_dot_rgba(PARTICLE_PX)),
        (SPARK, r#gen::spark_rgba(PARTICLE_PX)),
        (BOLT, r#gen::soft_dot_rgba(PARTICLE_PX)),
    ]
    .map(|(id, rgba)| Cell {
        id: id.to_string(),
        rgba,
    });
    r#gen::register_atlas(world, renderer, FilterMode::Linear, PARTICLE_PX, &particles);

    let tiles: Vec<Cell> = (0..4)
        .map(|variant| Cell {
            id: tile_id(variant),
            rgba: r#gen::tile_rgba(variant, TILE as u32),
        })
        .collect();
    r#gen::register_atlas(world, renderer, FilterMode::Nearest, TILE as u32, &tiles);

    let ridges: Vec<Cell> = (0..RIDGE_LAYERS)
        .map(|layer| Cell {
            id: ridge_id(layer),
            rgba: r#gen::ridge_rgba(layer as u32, RIDGE_PX),
        })
        .collect();
    r#gen::register_atlas(world, renderer, FilterMode::Linear, RIDGE_PX, &ridges);
}

/// The level tilemap: the collision layer (`tile_collision=on`), which the
/// engine never draws, then the rendered terrain and two decoration layers.
pub(super) fn register_tilemap(world: &mut World, level: &Level, collision: bool) {
    let mut layers = Vec::new();
    if collision {
        layers.push(TilemapLayer {
            name: "solid".to_string(),
            kind: LayerKind::Collision,
            tiles: level.collision_tiles(),
        });
    }
    for (name, tiles) in [
        ("terrain", &level.fill),
        ("grass", &level.grass),
        ("moss", &level.moss),
    ] {
        layers.push(TilemapLayer {
            name: name.to_string(),
            kind: LayerKind::Render,
            tiles: tiles.clone(),
        });
    }
    world
        .get_resource_mut::<TilemapRegistry>()
        .expect("TilemapRegistry resource missing")
        .insert(
            TILEMAP.to_string(),
            TilemapData {
                tile_width: TILE as u32,
                tile_height: TILE as u32,
                width: level.width,
                height: ROWS,
                tileset: (0..4).map(tile_id).collect(),
                layers,
            },
        );
    let entity = world.spawn();
    world.insert(entity, TilemapInstance::new(TILEMAP, Vec2::ZERO));
}

/// Each walker variant's lit walk clip and its unlit copy for the flash.
pub(super) fn register_clips(world: &mut World) {
    let mut registry = world
        .remove_resource::<AnimationRegistry>()
        .unwrap_or_default();
    let sprites = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    for variant in 0..VARIANTS {
        for flat in [false, true] {
            let frames = (0..WALK_FRAMES as usize)
                .map(|frame| AnimationFrame {
                    sprite: sprites.intern_sprite(&walk_frame(variant, frame, flat)),
                    duration_ms: WALK_FRAME_MS,
                })
                .collect();
            registry.insert(
                walk_clip(variant, flat),
                AnimationData {
                    looping: true,
                    frames,
                },
            );
        }
    }
    world.insert_resource(registry);
}

fn curve<V: Copy>(points: &[(f32, V)]) -> Curve<V> {
    Curve {
        points: points.to_vec(),
    }
}

/// Torch fire (continuous, rising) and hit sparks (one radial burst).
pub(super) fn register_particles(
    world: &mut World,
) -> (AssetId<ParticleConfig>, AssetId<ParticleConfig>) {
    let fire = ParticleConfig {
        sprite: FIRE.to_string(),
        render: ParticleRender::Quad,
        max_alive: FIRE_MAX_ALIVE,
        seed: None,
        blend: BlendMode::Premultiplied,
        emission: EmissionKind::Continuous { rate_hz: 30.0 },
        lifetime: Range { min: 0.4, max: 0.9 },
        initial_velocity: InitialVelocity::Cone {
            direction: [0.0, -1.0],
            spread_deg: 35.0,
            speed: Range {
                min: 25.0,
                max: 60.0,
            },
        },
        gravity: [0.0, -50.0],
        drag_per_sec: 1.0,
        angular_velocity: Range {
            min: -1.5,
            max: 1.5,
        },
        start_scale: Range {
            min: 0.45,
            max: 0.8,
        },
        scale_over_life: Some(curve(&[(0.0, 1.0), (1.0, 0.3)])),
        color_over_life: Some(curve(&[
            (0.0, [1.0, 0.9, 0.5, 1.0]),
            (0.5, [1.0, 0.5, 0.15, 1.0]),
            (1.0, [0.6, 0.15, 0.05, 1.0]),
        ])),
        alpha_over_life: Some(curve(&[(0.0, 0.9), (1.0, 0.0)])),
        tint: [1.0; 4],
    };
    let spark = ParticleConfig {
        sprite: SPARK.to_string(),
        render: ParticleRender::Quad,
        max_alive: 12,
        seed: None,
        blend: BlendMode::Alpha,
        emission: EmissionKind::Burst {
            count: 12,
            once: true,
        },
        lifetime: Range {
            min: 0.2,
            max: 0.45,
        },
        initial_velocity: InitialVelocity::Radial {
            speed: Range {
                min: 90.0,
                max: 240.0,
            },
        },
        gravity: [0.0, 500.0],
        drag_per_sec: 1.5,
        angular_velocity: Range {
            min: -6.0,
            max: 6.0,
        },
        start_scale: Range {
            min: 0.35,
            max: 0.7,
        },
        scale_over_life: Some(curve(&[(0.0, 1.0), (1.0, 0.5)])),
        color_over_life: Some(curve(&[
            (0.0, [1.0, 1.0, 0.85, 1.0]),
            (1.0, [1.0, 0.6, 0.2, 1.0]),
        ])),
        alpha_over_life: Some(curve(&[(0.0, 1.0), (1.0, 0.0)])),
        tint: [1.0; 4],
    };
    let registry = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .expect("ParticleConfigRegistry resource missing");
    let [fire, spark] =
        [("bench_int_fire", fire), ("bench_int_spark", spark)].map(|(name, config)| {
            if let Err(err) = config.validate() {
                panic!("generated particle config {name} is invalid: {err}");
            }
            let path = PathBuf::from(format!("__generated__/{name}.json"));
            registry.register(name.to_string(), path, Arc::new(config))
        });
    (fire, spark)
}
