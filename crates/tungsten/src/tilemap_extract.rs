//! Tilemap-to-sprite extraction; caller owns batch ordering.

use std::collections::HashMap;

use tungsten_core::assets::{LayerKind, TilemapRegistry};
use tungsten_core::{AssetRegistry, CameraState, TilemapInstance, World};
use tungsten_render::{SpriteBatch, SpriteInstance};

use crate::WindowSize;

/// Extract visible render layers as sprite batches; collision layers skipped.
pub fn extract_tilemaps(world: &World) -> Vec<SpriteBatch> {
    let tilemaps = match world.get_resource::<TilemapRegistry>() {
        Some(r) => r,
        None => return vec![],
    };
    let assets = match world.get_resource::<AssetRegistry>() {
        Some(r) => r,
        None => return vec![],
    };
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

    let (view_min, view_max) = camera.visible_world_aabb(window.width as f32, window.height as f32);

    let mut out: Vec<SpriteBatch> = Vec::new();

    for (_entity, instance) in world.query::<TilemapInstance>() {
        let data = match tilemaps.get(&instance.id) {
            Some(d) => d,
            None => {
                log::warn!(
                    "extract_tilemaps: no tilemap registered for '{}'",
                    instance.id
                );
                continue;
            }
        };

        let tw = data.tile_width as f32;
        let th = data.tile_height as f32;

        // World AABB -> tilemap-local grid range.
        let local_min_x = view_min.x - instance.origin.x;
        let local_min_y = view_min.y - instance.origin.y;
        let local_max_x = view_max.x - instance.origin.x;
        let local_max_y = view_max.y - instance.origin.y;

        let col_start = (local_min_x / tw).floor().max(0.0) as u32;
        let row_start = (local_min_y / th).floor().max(0.0) as u32;
        let col_end = ((local_max_x / tw).ceil().max(0.0) as u32).min(data.width);
        let row_end = ((local_max_y / th).ceil().max(0.0) as u32).min(data.height);

        if col_start >= col_end || row_start >= row_end {
            continue;
        }

        for layer in &data.layers {
            if layer.kind != LayerKind::Render {
                continue;
            }

            // Per-layer batches preserve layer draw order.
            let mut per_texture: HashMap<u32, SpriteBatch> = HashMap::new();

            for row in row_start..row_end {
                for col in col_start..col_end {
                    let idx = (row as usize) * (data.width as usize) + (col as usize);
                    let tile = layer.tiles[idx];
                    if tile < 0 {
                        continue;
                    }
                    let sprite_id = match data.tileset.get(tile as usize) {
                        Some(s) => s,
                        None => continue,
                    };
                    let asset = match assets.get_sprite(sprite_id) {
                        Some(a) => a,
                        None => continue,
                    };

                    let world_x = instance.origin.x + (col as f32) * tw;
                    let world_y = instance.origin.y + (row as f32) * th;

                    let uv_size = [
                        asset.uv.max[0] - asset.uv.min[0],
                        asset.uv.max[1] - asset.uv.min[1],
                    ];
                    per_texture
                        .entry(asset.atlas.0)
                        .or_insert_with(|| SpriteBatch {
                            texture: asset.atlas,
                            filter: asset.filter,
                            instances: Vec::new(),
                        })
                        .instances
                        .push(SpriteInstance {
                            position: [world_x, world_y],
                            size: [tw, th],
                            rotation: 0.0,
                            color: [255; 4],
                            uv_min: asset.uv.min,
                            uv_size,
                        });
                }
            }

            out.extend(per_texture.into_values());
        }
    }

    out
}
