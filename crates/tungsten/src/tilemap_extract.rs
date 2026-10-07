//! Tilemap-to-sprite extraction; caller owns batch ordering.

use tungsten_core::assets::{LayerKind, TextureHandle, TilemapRegistry};
use tungsten_core::{
    AssetRegistry, CameraState, Entity, FilterMode, SpriteAsset, TilemapInstance, World,
};
use tungsten_render::{SpriteBatch, SpriteInstance};

use crate::WindowSize;
use crate::sprite_extract::ExtractScratch;

/// What a tile instance needs from its sprite asset.
#[derive(Clone, Copy)]
pub(crate) struct TileSprite {
    atlas: TextureHandle,
    filter: FilterMode,
    uv_min: [f32; 2],
    uv_size: [f32; 2],
}

#[derive(Clone, Copy, Default)]
enum TileSlot {
    #[default]
    Unresolved,
    Missing,
    Found(TileSprite),
}

/// One tilemap's tileset during an extract call: an entry resolves against
/// the asset registry the first time a visible tile uses it, and never again.
#[derive(Default)]
pub(crate) struct TilesetCache {
    slots: Vec<TileSlot>,
}

impl TilesetCache {
    /// Starts a map whose tileset has `len` entries.
    fn begin(&mut self, len: usize) {
        self.slots.clear();
        self.slots.resize(len, TileSlot::Unresolved);
    }

    /// The sprite of tileset entry `tile`; `resolve` runs on its first use.
    /// `None` for an entry outside the tileset or without a registered sprite.
    #[inline]
    fn get<'a>(
        &mut self,
        tile: usize,
        resolve: impl FnOnce() -> Option<&'a SpriteAsset>,
    ) -> Option<TileSprite> {
        let slot = self.slots.get_mut(tile)?;
        if let TileSlot::Unresolved = slot {
            *slot = match resolve() {
                Some(asset) => TileSlot::Found(TileSprite {
                    atlas: asset.atlas,
                    filter: asset.filter,
                    uv_min: asset.uv.min,
                    uv_size: [
                        asset.uv.max[0] - asset.uv.min[0],
                        asset.uv.max[1] - asset.uv.min[1],
                    ],
                }),
                None => TileSlot::Missing,
            };
        }
        match *slot {
            TileSlot::Found(sprite) => Some(sprite),
            TileSlot::Unresolved | TileSlot::Missing => None,
        }
    }
}

/// Extract visible render layers as sprite batches; collision layers skipped.
///
/// A layer yields one batch per atlas page, in the order its visible tiles
/// (row by row) first use each page.
#[must_use]
pub fn extract_tilemaps(world: &World) -> Vec<SpriteBatch> {
    let Some(tilemaps) = world.get_resource::<TilemapRegistry>() else {
        return vec![];
    };
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return vec![];
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

    ExtractScratch::with(world, |buffers| {
        let tileset = &mut buffers.tileset;
        let pool = &mut buffers.pool;
        let batch_lens = &mut buffers.tile_batch_lens;
        let mut out: Vec<SpriteBatch> = Vec::new();

        for (_entity, instance) in world.query::<(Entity, &TilemapInstance)>() {
            let Some(data) = tilemaps.get(&instance.id) else {
                log::warn!(
                    "extract_tilemaps: no tilemap registered for '{}'",
                    instance.id
                );
                continue;
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

            tileset.begin(data.tileset.len());

            for layer in &data.layers {
                if layer.kind != LayerKind::Render {
                    continue;
                }

                // Per-layer batches preserve layer draw order: this layer's
                // batches are `out[layer_start..]`, one per atlas page.
                let layer_start = out.len();
                let mut last: Option<(TextureHandle, usize)> = None;

                for row in row_start..row_end {
                    for col in col_start..col_end {
                        let idx = (row as usize) * (data.width as usize) + (col as usize);
                        let tile = layer.tiles[idx];
                        if tile < 0 {
                            continue;
                        }
                        let tile = tile as usize;
                        let Some(sprite) = tileset.get(tile, || {
                            data.tileset
                                .get(tile)
                                .and_then(|sprite_id| assets.get_sprite(sprite_id))
                        }) else {
                            continue;
                        };

                        let batch = match last {
                            Some((atlas, batch)) if atlas == sprite.atlas => batch,
                            _ => {
                                let opened = out[layer_start..]
                                    .iter()
                                    .position(|batch| batch.texture == sprite.atlas);
                                let batch = if let Some(offset) = opened {
                                    layer_start + offset
                                } else {
                                    let len =
                                        batch_lens.get(out.len()).map_or(0, |&len| len as usize);
                                    let mut batch = SpriteBatch::new(sprite.atlas, sprite.filter);
                                    batch.instances = pool.take(len);
                                    out.push(batch);
                                    out.len() - 1
                                };
                                last = Some((sprite.atlas, batch));
                                batch
                            }
                        };

                        let world_x = instance.origin.x + (col as f32) * tw;
                        let world_y = instance.origin.y + (row as f32) * th;
                        out[batch].instances.push(SpriteInstance {
                            position: [world_x, world_y],
                            size: [tw, th],
                            rotation: 0.0,
                            color: [255; 4],
                            uv_min: sprite.uv_min,
                            uv_size: sprite.uv_size,
                            z_norm: 0.0,
                            _pad: 0.0,
                        });
                    }
                }
            }
        }

        batch_lens.clear();
        batch_lens.extend(out.iter().map(|batch| batch.instances.len() as u32));
        out
    })
}

#[cfg(test)]
#[path = "tests/tilemap_extract.rs"]
mod tests;
