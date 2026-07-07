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

use std::collections::HashMap;

use tungsten_core::tween::UniformOverrideBlock;
use tungsten_core::{
    AssetRegistry, Entity, FilterMode, MaterialAssetId, Sprite, SpriteAsset, Transform, Visibility,
    World,
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

/// Default sprite extract.
#[must_use]
pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };

    // Collect visible sprites with resolved assets; stable sort by
    // `(z_order, entity_id)` so same-`z_order` ties are deterministic.
    let mut entries: Vec<(Entity, &Transform, &Sprite, &SpriteAsset)> = world
        .query3::<Transform, Sprite, Visibility>()
        .filter_map(|(e, t, s, v)| {
            if !v.visible {
                return None;
            }
            let asset = assets.get_sprite(&s.asset_id)?;
            Some((e, t, s, asset))
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
    for (idx_in_order, (entity, t, s, asset)) in entries.iter().enumerate() {
        if current_z != Some(s.z_order) {
            per_key.clear();
            current_z = Some(s.z_order);
        }
        let override_block = world.get::<UniformOverrideBlock>(*entity);
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
        let batch_idx = if let Some(&i) = per_key.get(&key) {
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
        out[batch_idx].instances.push(SpriteInstance {
            position: [t.position.x, t.position.y],
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
