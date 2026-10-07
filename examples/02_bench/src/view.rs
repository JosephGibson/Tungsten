//! Lean view: a culled, unsorted extract over `Position` plus [`ViewSprite`].
//!
//! The engine's default extract resolves string IDs and sorts every sprite
//! each frame; benchmarks that don't own render cost draw through this view
//! instead, so their render cost stays flat as they scale.

use glam::Vec2;
use tungsten::WindowSize;
use tungsten::core::{AssetRegistry, CameraState, Entity, Position, World};
use tungsten::render::{SpriteBatch, SpriteInstance};

use crate::r#gen;

/// Canonical capture viewport; `main` requests it and benchmarks fit their
/// cameras to it.
pub(crate) const VIEWPORT: Vec2 = Vec2::new(1920.0, 1080.0);

/// Texture a [`ViewSprite`] draws with; each one becomes a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewTexture {
    Rect,
    Disc,
}

const TEXTURES: [(ViewTexture, &str); 2] = [
    (ViewTexture::Rect, r#gen::RECT),
    (ViewTexture::Disc, r#gen::DISC),
];

/// A quad centered on the entity's `Position`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ViewSprite {
    pub(crate) half: Vec2,
    pub(crate) color: [u8; 4],
    pub(crate) texture: ViewTexture,
}

/// World-space `(min, max)` the camera shows this frame.
pub(crate) fn view_bounds(world: &World) -> (Vec2, Vec2) {
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let window = world.get_resource::<WindowSize>().map_or(VIEWPORT, |size| {
        Vec2::new(size.width as f32, size.height as f32)
    });
    camera.visible_world_aabb(window.x, window.y)
}

/// Whether a quad centered at `center` overlaps `bounds`.
pub(crate) fn in_view(bounds: (Vec2, Vec2), center: Vec2, half: Vec2) -> bool {
    let (min, max) = bounds;
    center.x + half.x >= min.x
        && center.x - half.x <= max.x
        && center.y + half.y >= min.y
        && center.y - half.y <= max.y
}

/// `set_extract_sprites` hook: one batch per texture, no sort, off-view quads skipped.
pub(crate) fn extract_view(world: &World) -> Vec<SpriteBatch> {
    let Some(registry) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let mut batches: Vec<(ViewTexture, SpriteBatch)> = TEXTURES
        .iter()
        .filter_map(|&(texture, id)| {
            let asset = registry.get_sprite(id)?;
            Some((texture, SpriteBatch::new(asset.atlas, asset.filter)))
        })
        .collect();
    let bounds = view_bounds(world);
    for (_entity, position, sprite) in world.query::<(Entity, &Position, &ViewSprite)>() {
        if !in_view(bounds, position.0, sprite.half) {
            continue;
        }
        if let Some((_, batch)) = batches
            .iter_mut()
            .find(|(texture, _)| *texture == sprite.texture)
        {
            batch.instances.push(SpriteInstance::whole(
                (position.0 - sprite.half).to_array(),
                (sprite.half * 2.0).to_array(),
                0.0,
                sprite.color,
            ));
        }
    }
    batches
        .into_iter()
        .map(|(_, batch)| batch)
        .filter(|batch| !batch.instances.is_empty())
        .collect()
}
