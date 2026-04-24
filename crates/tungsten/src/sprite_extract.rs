//! Default sprite extract: `Transform + Sprite + Visibility` -> [`SpriteBatch`].
//!
//! D-042: explicit `Visibility` required. Order: stable `(z_order, entity.id)`,
//! batched within z-runs. `z_norm` is derived from the same painter ordering
//! so `DepthSortMode::GpuDepth` reproduces the CPU-visible order.

use std::collections::HashMap;

use tungsten_core::{
    AssetRegistry, Entity, FilterMode, Sprite, SpriteAsset, Transform, Visibility, World,
};
use tungsten_render::{SpriteBatch, SpriteInstance};

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

    // Batch by `(atlas, filter)` within each z-run only.
    let mut out: Vec<SpriteBatch> = Vec::new();
    let mut current_z: Option<i32> = None;
    let mut per_key: HashMap<(u32, FilterMode), usize> = HashMap::new();
    for (idx_in_order, (_e, t, s, asset)) in entries.iter().enumerate() {
        if current_z != Some(s.z_order) {
            per_key.clear();
            current_z = Some(s.z_order);
        }
        let key = (asset.atlas.0, asset.filter);
        let batch_idx = if let Some(&i) = per_key.get(&key) {
            i
        } else {
            let i = out.len();
            per_key.insert(key, i);
            out.push(SpriteBatch {
                texture: asset.atlas,
                filter: asset.filter,
                instances: Vec::new(),
            });
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
