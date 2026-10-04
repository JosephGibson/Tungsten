//! Scene files (`D-046`): load with path context, spawn through the
//! `CommandBuffer`.

use std::path::Path;

use glam::Vec2;
use tungsten_core::assets::SceneData;
use tungsten_core::{
    AssetRegistry, CommandBuffer, Sprite, SpriteAssetId, Tag, Transform, Visibility, World,
};

use crate::state::{SceneEntity, StateId};

/// Load scene file with path context.
pub fn load_scene(path: &Path) -> anyhow::Result<SceneData> {
    SceneData::load(path)
        .map_err(|source| anyhow::anyhow!("Failed to load scene '{}': {source}", path.display()))
}

/// Spawn scene entities through `CommandBuffer`; D-046 leaves sprite names unresolved:
/// each is interned, so a sprite a later manifest reload registers draws from then on.
/// D-055 one `Tween` per entity — additional tween entries log `ERROR` and are dropped.
pub fn spawn_scene(world: &mut World, data: &SceneData, state_id: StateId) {
    let mut registry = world.get_resource_mut::<AssetRegistry>();
    let sprite_ids: Vec<Option<SpriteAssetId>> = data
        .entities
        .iter()
        .map(|entry| {
            let sprite = entry.sprite.as_ref()?;
            let registry = registry
                .as_deref_mut()
                .expect("AssetRegistry resource missing");
            Some(registry.intern_sprite(&sprite.asset_id))
        })
        .collect();
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");
    for (entry, sprite_id) in data.entities.iter().zip(sprite_ids) {
        let pending = buf.spawn();
        buf.insert_pending(
            pending,
            Transform {
                position: Vec2::from(entry.transform.position),
                rotation: entry.transform.rotation,
                scale: Vec2::from(entry.transform.scale),
            },
        );
        if let (Some(sprite), Some(asset_id)) = (&entry.sprite, sprite_id) {
            buf.insert_pending(
                pending,
                Sprite {
                    asset_id,
                    color: sprite.color,
                    z_order: sprite.z_order,
                    material_id: None,
                },
            );
        }
        buf.insert_pending(
            pending,
            Visibility {
                visible: entry.visible,
            },
        );
        if let Some(name) = &entry.tag {
            buf.insert_pending(pending, Tag::new(name.clone()));
        }
        buf.insert_pending(pending, SceneEntity { state_id });
        if let Some(tween) = entry.tweens.first() {
            if entry.tweens.len() > 1 {
                log::error!(
                    "Scene entry carries {} tweens; D-055 allows one -- keeping the first",
                    entry.tweens.len()
                );
            }
            buf.insert_pending(pending, tween.into_tween());
        }
    }
}
