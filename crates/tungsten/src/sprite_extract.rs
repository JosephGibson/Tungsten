//! Default sprite extract: `Transform + Sprite + Visibility` -> [`SpriteBatch`].
//!
//! D-042: explicit `Visibility` required. Order: stable `(z_order, entity.id)`,
//! batched within z-runs. `z_norm` is derived from the same painter ordering
//! so `DepthSortMode::GpuDepth` reproduces the CPU-visible order.
//!
//! M26: batch key extends with `(material_id, uniform_overrides_hash)` so
//! per-entity material animations never alias through one UBO upload. Same-
//! material same-override batches still collapse; the M25 default bytes are
//! byte-identical when no sprite carries `material_id`.
//!
//! M30 (`D-073`): a `ParallaxLayer` entity's instance position is remapped
//! against `CameraState.position` at extract time, so one view-projection still
//! draws every layer. The remap touches positions only — `BatchKey` and `z_norm`
//! are unchanged, and a sprite without `ParallaxLayer` extracts byte-identically
//! to pre-M30.
//!
//! Per-sprite cost: optional `UniformOverrideBlock` / `ParallaxLayer` columns
//! resolve once per archetype (`query3_opt2`), and runs of one asset id or one
//! batch key skip their map lookups via a last-seen memo. Output bytes are
//! unchanged.

use std::collections::HashMap;

use glam::Vec2;
use tungsten_core::tween::UniformOverrideBlock;
use tungsten_core::{
    AssetRegistry, CameraState, Entity, FilterMode, MaterialAssetId, ParallaxLayer, Sprite,
    SpriteAsset, Transform, Visibility, World, parallax_world_position,
};
use tungsten_render::{SpriteBatch, SpriteInstance};

/// Hash of a `UniformOverrideBlock`'s 256-byte payload, used as a batch-split
/// key so per-entity overrides cannot alias through one UBO upload.
fn override_key(block: &UniformOverrideBlock) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for byte in block.to_bytes() {
        byte.hash(&mut hasher);
    }
    hasher.finish()
}

type BatchKey = (u32, FilterMode, Option<MaterialAssetId>, Option<u64>, bool);

type ExtractEntry<'w> = (
    Entity,
    &'w Transform,
    &'w Sprite,
    &'w SpriteAsset,
    Option<&'w UniformOverrideBlock>,
    Option<&'w ParallaxLayer>,
);

/// Default sprite extract.
#[must_use]
pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let camera_position = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);

    // Collect visible sprites with resolved assets; stable sort by
    // `(z_order, entity_id)` so same-`z_order` ties are deterministic.
    let mut last_asset: Option<(&str, &SpriteAsset)> = None;
    let mut entries: Vec<ExtractEntry<'_>> = world
        .query3_opt2::<Transform, Sprite, Visibility, UniformOverrideBlock, ParallaxLayer>()
        .filter_map(|(e, t, s, v, override_block, parallax)| {
            if !v.visible {
                return None;
            }
            let asset = match last_asset {
                Some((id, asset)) if id == s.asset_id => asset,
                _ => {
                    let asset = assets.get_sprite(&s.asset_id)?;
                    last_asset = Some((s.asset_id.as_str(), asset));
                    asset
                }
            };
            Some((e, t, s, asset, override_block, parallax))
        })
        .collect();
    entries.sort_by(|a, b| {
        a.2.z_order
            .cmp(&b.2.z_order)
            .then_with(|| a.0.id().cmp(&b.0.id()))
    });

    let total = entries.len();

    // Batch by effective material state inside each z-run.
    let mut out: Vec<SpriteBatch> = Vec::new();
    let mut current_z: Option<i32> = None;
    let mut per_key: HashMap<BatchKey, usize> = HashMap::new();
    let mut last_batch: Option<(BatchKey, usize)> = None;
    for (idx_in_order, &(_entity, t, s, asset, override_block, parallax)) in
        entries.iter().enumerate()
    {
        if current_z != Some(s.z_order) {
            per_key.clear();
            last_batch = None;
            current_z = Some(s.z_order);
        }
        let override_hash = override_block.map(override_key);
        // M29: lit wins over material when both are present. The collision
        // is intentionally a non-goal in M29 (see plan); warn so debug logs
        // surface the conflict rather than silently dropping the material.
        let lit = asset.lit_atlas.is_some();
        let effective_material = if lit { None } else { s.material_id };
        if lit && s.material_id.is_some() {
            log::warn!(
                "lit sprite '{}' carries material_id {:?}; lit wins (material UBO not bound)",
                s.asset_id,
                s.material_id
            );
        }
        let key: BatchKey = (
            asset.atlas.0,
            asset.filter,
            effective_material,
            override_hash,
            lit,
        );
        let batch_idx = match last_batch {
            Some((last_key, i)) if last_key == key => i,
            _ => {
                let i = if let Some(&i) = per_key.get(&key) {
                    i
                } else {
                    let i = out.len();
                    per_key.insert(key, i);
                    let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
                    batch.material_id = effective_material;
                    batch.uniform_overrides = if lit { None } else { override_block.copied() };
                    batch.lit = lit;
                    out.push(batch);
                    i
                };
                last_batch = Some((key, i));
                i
            }
        };
        let width_world = asset.width as f32 * t.scale.x;
        let height_world = asset.height as f32 * t.scale.y;
        let uv_size = [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ];
        // Map painter order so the depth test reproduces "later-drawn wins".
        // With `depth_compare = LessEqual` and a 1.0 clear, a fragment passes
        // when its z ≤ the current depth. Keeping the first-drawn (most
        // distant) at a larger z and the last-drawn (closest) at 0 lets every
        // subsequent overlap pass the test, so the final visible pixel matches
        // the CpuStable painter output.
        let z_norm = if total > 0 {
            (total as f32 - 1.0 - idx_in_order as f32) / total as f32
        } else {
            0.0
        };
        let position = match parallax {
            Some(layer) => {
                parallax_world_position(t.position, layer.scroll_factor, camera_position)
            }
            None => t.position,
        };
        out[batch_idx].instances.push(SpriteInstance {
            position: [position.x, position.y],
            size: [width_world, height_world],
            rotation: t.rotation,
            color: s.color,
            uv_min: asset.uv.min,
            uv_size,
            z_norm,
            _pad: 0.0,
        });
    }
    out
}

#[cfg(test)]
#[path = "tests/sprite_extract.rs"]
mod tests;
