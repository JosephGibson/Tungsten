//! Default sprite extract: `Transform + Sprite + Visibility` -> [`SpriteBatch`].
//!
//! D-042: explicit `Visibility` required. Order: stable `z_order`, batched within z-runs.

use std::collections::HashMap;

use tungsten_core::{AssetRegistry, FilterMode, Sprite, SpriteAsset, Transform, Visibility, World};
use tungsten_render::{SpriteBatch, SpriteInstance};

/// Default sprite extract.
#[must_use]
pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };

    // Collect visible sprites with resolved assets; stable sort by z.
    let mut entries: Vec<(&Transform, &Sprite, &SpriteAsset)> = world
        .query3::<Transform, Sprite, Visibility>()
        .filter_map(|(_e, t, s, v)| {
            if !v.visible {
                return None;
            }
            let asset = assets.get_sprite(&s.asset_id)?;
            Some((t, s, asset))
        })
        .collect();
    entries.sort_by_key(|(_, s, _)| s.z_order);

    // Batch by `(atlas, filter)` within each z-run only.
    let mut out: Vec<SpriteBatch> = Vec::new();
    let mut current_z: Option<i32> = None;
    let mut per_key: HashMap<(u32, FilterMode), usize> = HashMap::new();
    for (t, s, asset) in entries {
        if current_z != Some(s.z_order) {
            per_key.clear();
            current_z = Some(s.z_order);
        }
        let key = (asset.atlas.0, asset.filter);
        let idx = if let Some(&i) = per_key.get(&key) {
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
        out[idx].instances.push(SpriteInstance {
            position: [t.position.x, t.position.y],
            size: [width_world, height_world],
            rotation: t.rotation,
            color: s.color,
            uv_min: asset.uv.min,
            uv_size,
        });
    }
    out
}

#[cfg(test)]
#[path = "tests/sprite_extract.rs"]
mod tests;
