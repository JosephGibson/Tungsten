//! Default sprite extract: `Transform + Sprite + Visibility` components
//! -> [`SpriteBatch`] values consumed by the renderer.
//!
//! Installed automatically by [`App`](crate::App) when the user did not call
//! [`App::set_extract_sprites`](crate::App::set_extract_sprites). Entities
//! with `Transform + Sprite` but no `Visibility` are not emitted — an
//! explicit `Visibility` component is required (D-042). There is no implicit
//! fallback.
//!
//! Ordering: entries are sorted by [`Sprite::z_order`] ascending; ties
//! preserve the underlying archetype iteration order because
//! [`Vec::sort_by_key`] is stable. Within a `z_order` group, entries are
//! batched by `(texture, filter)`; the per-group batch map resets when the
//! `z_order` value changes, so lower-z entries never merge into a later
//! higher-z batch of the same texture (painter-order preserved).

use std::collections::HashMap;

use tungsten_core::{AssetRegistry, FilterMode, Sprite, SpriteAsset, Transform, Visibility, World};
use tungsten_render::{SpriteBatch, SpriteInstance};

/// Default sprite-extract entry point. See module docs.
pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };

    // Phase A: collect references, filtered by visibility and asset
    // resolution, sorted stably by z_order ascending.
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

    // Phase B: batch by (atlas, filter) within each z_order run.
    let mut out: Vec<SpriteBatch> = Vec::new();
    let mut current_z: Option<i32> = None;
    let mut per_key: HashMap<(u32, FilterMode), usize> = HashMap::new();
    for (t, s, asset) in entries {
        if current_z != Some(s.z_order) {
            per_key.clear();
            current_z = Some(s.z_order);
        }
        let key = (asset.atlas.0, asset.filter);
        let idx = match per_key.get(&key) {
            Some(&i) => i,
            None => {
                let i = out.len();
                per_key.insert(key, i);
                out.push(SpriteBatch {
                    texture: asset.atlas,
                    filter: asset.filter,
                    instances: Vec::new(),
                });
                i
            }
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
