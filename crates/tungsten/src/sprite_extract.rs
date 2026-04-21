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
mod tests {
    use super::*;
    use glam::Vec2;
    use std::path::PathBuf;
    use tungsten_core::assets::{TextureHandle, UvRect};
    use tungsten_core::{AssetRegistry, Sprite, Transform, Visibility, World};

    fn register_sprite(world: &mut World, id: &str, filter: FilterMode) {
        let registry = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        registry.register_sprite(
            id.to_string(),
            filter,
            16,
            16,
            PathBuf::from(format!("test/{id}.png")),
            TextureHandle(0),
            UvRect::FULL,
        );
    }

    fn world_with_registry() -> World {
        let mut world = World::new();
        world.insert_resource(AssetRegistry::new());
        world
    }

    #[test]
    fn missing_visibility_emits_nothing() {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);

        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, Sprite::new("quad"));
        // intentionally no Visibility

        let batches = extract_sprites_default(&world);
        let total: usize = batches.iter().map(|b| b.instances.len()).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn invisible_entity_emits_nothing() {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);

        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, Sprite::new("quad"));
        world.insert(e, Visibility { visible: false });

        let batches = extract_sprites_default(&world);
        let total: usize = batches.iter().map(|b| b.instances.len()).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn missing_asset_id_emits_nothing() {
        let mut world = world_with_registry();
        // No sprite registered for "ghost".

        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, Sprite::new("ghost"));
        world.insert(e, Visibility::default());

        let batches = extract_sprites_default(&world);
        let total: usize = batches.iter().map(|b| b.instances.len()).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn transform_scale_applies_to_instance_size() {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);

        let e = world.spawn();
        world.insert(
            e,
            Transform {
                position: Vec2::new(5.0, 7.0),
                rotation: 0.5,
                scale: Vec2::new(2.0, 3.0),
            },
        );
        world.insert(e, Sprite::new("quad"));
        world.insert(e, Visibility::default());

        let batches = extract_sprites_default(&world);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].instances.len(), 1);
        let inst = &batches[0].instances[0];
        assert_eq!(inst.position, [5.0, 7.0]);
        assert_eq!(inst.size, [32.0, 48.0]);
        assert_eq!(inst.rotation, 0.5);
        assert_eq!(inst.color, [255; 4]);
        assert_eq!(inst.uv_min, [0.0, 0.0]);
        assert_eq!(inst.uv_size, [1.0, 1.0]);
    }

    #[test]
    fn sprite_color_reaches_instance() {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);

        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(
            e,
            Sprite {
                asset_id: "quad".into(),
                color: [10, 20, 30, 255],
                z_order: 0,
            },
        );
        world.insert(e, Visibility::default());

        let batches = extract_sprites_default(&world);
        assert_eq!(batches[0].instances[0].color, [10, 20, 30, 255]);
    }

    #[test]
    fn z_order_groups_do_not_merge_across_a_lower_z_entry() {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);

        // Spawn three entities all sharing the same (texture, filter) key
        // but at different z_orders. Because the per-key batch map resets
        // between z-order runs, we get 3 batches, not 1.
        for z in [-1, 0, 1] {
            let e = world.spawn();
            world.insert(e, Transform::default());
            world.insert(
                e,
                Sprite {
                    asset_id: "quad".into(),
                    color: [255; 4],
                    z_order: z,
                },
            );
            world.insert(e, Visibility::default());
        }

        let batches = extract_sprites_default(&world);
        assert_eq!(batches.len(), 3);
        assert!(batches.iter().all(|b| b.instances.len() == 1));
    }

    #[test]
    fn same_z_same_texture_collapses_to_one_batch() {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);

        for _ in 0..4 {
            let e = world.spawn();
            world.insert(e, Transform::default());
            world.insert(e, Sprite::new("quad"));
            world.insert(e, Visibility::default());
        }

        let batches = extract_sprites_default(&world);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].instances.len(), 4);
    }

    #[test]
    fn missing_asset_registry_returns_empty() {
        let world = World::new();
        let batches = extract_sprites_default(&world);
        assert!(batches.is_empty());
    }
}
