//! Tilemap-to-sprite extraction; caller owns batch ordering. A render
//! layer's `LayerDraw` (redo 7.9) tints its tiles and, when unlit, draws a
//! lit tileset on the stock pipeline; the default sprite base draws a layer
//! with a `z` among the sprites, while these extracts keep every layer in
//! file order.

use tungsten_core::assets::{LayerKind, TextureHandle, TilemapRegistry};
use tungsten_core::{
    AssetRegistry, CameraState, Entity, FilterMode, SpriteAsset, TilemapInstance, World,
};
use tungsten_render::{SpriteBatch, SpriteInstance};

use crate::sprite_extract::{ExtractScratch, render_viewport};

/// What a tile instance needs from its sprite asset.
#[derive(Clone, Copy)]
pub(crate) struct TileSprite {
    atlas: TextureHandle,
    filter: FilterMode,
    /// Whether the sprite has a lit atlas, which draws it on the lit
    /// pipeline (`D-061`).
    lit: bool,
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
                    lit: asset.lit_atlas.is_some(),
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
/// A layer yields one batch per atlas page and lighting, in the order its
/// visible tiles (row by row) first use each. A tile whose sprite has a lit
/// atlas draws in a lit batch, as a sprite does (`D-061`), unless its layer
/// is unlit; each tile takes its layer's tint as its colour. Every instance
/// has `z_norm` 0.
#[must_use]
pub fn extract_tilemaps(world: &World) -> Vec<SpriteBatch> {
    extract_layers(world, None, None)
}

/// A layer the default sprite base draws among the sprites: its `z` and
/// its batches.
pub(crate) type RaisedLayer = (i32, Vec<SpriteBatch>);

/// [`extract_tilemaps`] for the default sprite base: the layers without a
/// `z`, while each layer with one goes into `raised`, map by map in file
/// order, untouched when no layer has one.
pub(crate) fn extract_tiles_raising(
    world: &World,
    raised: &mut Vec<RaisedLayer>,
) -> Vec<SpriteBatch> {
    extract_layers(world, None, Some(raised))
}

/// The named render layers of every tilemap, map by map, each map's layers in
/// file order; a name no map has draws nothing; collision layers never draw.
/// The batches are [`extract_tilemaps`]'s for those layers, so naming every
/// render layer draws what it draws.
#[must_use]
pub fn extract_tilemap_layers(world: &World, layers: &[&str]) -> Vec<SpriteBatch> {
    extract_layers(world, Some(layers), None)
}

/// The tilemap extract: every render layer, or only those `names` lists;
/// with `raised`, a layer with a `z` goes there instead.
fn extract_layers(
    world: &World,
    names: Option<&[&str]>,
    mut raised: Option<&mut Vec<RaisedLayer>>,
) -> Vec<SpriteBatch> {
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
    // Culls at the size render projects with, as the sprite extract does.
    let (width, height) = render_viewport(world).unwrap_or((1280, 720));

    let (view_min, view_max) = camera.visible_world_aabb(width as f32, height as f32);

    ExtractScratch::with(world, |buffers| {
        let tileset = &mut buffers.tileset;
        let pool = &mut buffers.pool;
        let batch_lens = &mut buffers.tile_batch_lens;
        let unknown = &mut buffers.unknown_tilemaps;
        let mut out: Vec<SpriteBatch> = Vec::new();

        for (_entity, instance) in world.query::<(Entity, &TilemapInstance)>() {
            let Some(data) = tilemaps.get(&instance.id) else {
                if !unknown.contains(&instance.id) {
                    log::warn!(
                        "extract_tilemaps: no tilemap registered for '{}'; it draws nothing",
                        instance.id
                    );
                    unknown.push(instance.id.clone());
                }
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

            for (index, layer) in data.layers.iter().enumerate() {
                if layer.kind != LayerKind::Render
                    || names.is_some_and(|names| !names.contains(&layer.name.as_str()))
                {
                    continue;
                }
                let draw = data.layer_draw(index);
                // A raised layer's batches are a list of their own; the
                // others' go on in `out`, whose capacity hints they keep.
                let (target, hints) = match (raised.as_deref_mut(), draw.z) {
                    (Some(raised), Some(z)) => {
                        raised.push((z, Vec::new()));
                        (&mut raised.last_mut().unwrap().1, None)
                    }
                    _ => (&mut out, Some(&*batch_lens)),
                };

                // Per-layer batches preserve layer draw order: this layer's
                // batches are `target[layer_start..]`, one per atlas page and
                // lighting.
                let layer_start = target.len();
                let mut last: Option<(TextureHandle, bool, usize)> = None;

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
                        let lit = sprite.lit && !draw.unlit;

                        let batch = match last {
                            Some((atlas, last_lit, batch))
                                if atlas == sprite.atlas && last_lit == lit =>
                            {
                                batch
                            }
                            _ => {
                                let opened = target[layer_start..].iter().position(|batch| {
                                    batch.texture == sprite.atlas && batch.lit == lit
                                });
                                let batch = if let Some(offset) = opened {
                                    layer_start + offset
                                } else {
                                    let len = hints
                                        .and_then(|hints| hints.get(target.len()))
                                        .map_or(0, |&len| len as usize);
                                    let mut batch = SpriteBatch::new(sprite.atlas, sprite.filter);
                                    batch.lit = lit;
                                    batch.instances = pool.take(len);
                                    target.push(batch);
                                    target.len() - 1
                                };
                                last = Some((sprite.atlas, lit, batch));
                                batch
                            }
                        };

                        let world_x = instance.origin.x + (col as f32) * tw;
                        let world_y = instance.origin.y + (row as f32) * th;
                        target[batch].instances.push(SpriteInstance {
                            position: [world_x, world_y],
                            size: [tw, th],
                            rotation: 0.0,
                            color: draw.tint,
                            uv_min: sprite.uv_min,
                            uv_size: sprite.uv_size,
                            z_norm: 0.0,
                            _pad: 0.0,
                        });
                    }
                }
                // A raised layer with nothing in view takes no place.
                if draw.z.is_some()
                    && let Some(raised) = raised.as_deref_mut()
                    && raised.last().is_some_and(|(_, batches)| batches.is_empty())
                {
                    raised.pop();
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
