//! Scene files (`D-046`): load with path context, spawn through the
//! `CommandBuffer`.

use std::path::Path;

use glam::Vec2;
use tungsten_core::assets::SceneData;
use tungsten_core::{CommandBuffer, Sprite, Tag, Transform, Visibility, World};

use crate::state::{SceneEntity, StateId};

/// Load scene file with path context.
pub fn load_scene(path: &Path) -> anyhow::Result<SceneData> {
    SceneData::load(path)
        .map_err(|source| anyhow::anyhow!("Failed to load scene '{}': {source}", path.display()))
}

/// Spawn scene entities through `CommandBuffer`; D-046 leaves sprite IDs unresolved.
/// D-055 one `Tween` per entity — additional tween entries log `ERROR` and are dropped.
pub fn spawn_scene(world: &mut World, data: &SceneData, state_id: StateId) {
    let buf = world
        .get_resource_mut::<CommandBuffer>()
        .expect("CommandBuffer resource missing");
    for entry in &data.entities {
        let pending = buf.spawn();
        buf.insert_pending(
            pending,
            Transform {
                position: Vec2::from(entry.transform.position),
                rotation: entry.transform.rotation,
                scale: Vec2::from(entry.transform.scale),
            },
        );
        if let Some(sprite) = &entry.sprite {
            buf.insert_pending(
                pending,
                Sprite {
                    asset_id: sprite.asset_id.clone(),
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
