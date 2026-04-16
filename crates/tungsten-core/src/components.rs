//! Engine-level game components introduced in M15 (see D-042).
//!
//! Four small, self-contained types that other engine subsystems and the
//! default sprite-extract path build on:
//!
//! - [`Transform`] — world-space position, rotation (radians, CCW positive),
//!   and per-axis scale. Rotation is applied around the quad centre by the
//!   sprite shader.
//! - [`Sprite`] — asset lookup + tint + z-order. The tint multiplies the
//!   sampled texel at draw time (`[255; 4]` = no tint). `z_order` is a stable
//!   ascending sort key used by the default extract.
//! - [`Visibility`] — explicit render gate. Required by the default sprite
//!   extract path: entities with `Transform + Sprite` but no `Visibility` are
//!   never emitted. No implicit fallback (D-042).
//! - [`Tag`] — a debug-friendly name for find-by-name lookups.
//!
//! Physics `Position` (see [`crate::physics::Position`]) stays separate per
//! `D-033`. Use [`sync_position_to_transform`] to opt in to copying
//! `Position.0` into `Transform.position` after the physics step; there is no
//! reverse sync.
//!
//! `Transform` is `Copy`; `Sprite` and `Tag` hold `String`s and are not.

use glam::Vec2;

use crate::ecs::World;
use crate::physics::Position;

/// World-space transform for a visual entity. Rotation is in radians, CCW
/// positive, applied around the quad centre at draw time; scale is per-axis
/// and multiplies the sprite's intrinsic pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Transform {
    /// Identity rotation, unit scale, with the given position.
    pub fn from_position(position: Vec2) -> Self {
        Self {
            position,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

/// Render description for a sprite entity. `asset_id` is resolved against
/// [`crate::AssetRegistry`] at extract time. `color` is an RGBA tint applied
/// at draw time (`[255; 4]` = no tint). `z_order` is a stable ascending sort
/// key for the default extract path; larger values render on top.
#[derive(Debug, Clone)]
pub struct Sprite {
    pub asset_id: String,
    pub color: [u8; 4],
    pub z_order: i32,
}

impl Sprite {
    /// Defaults to no tint (`[255; 4]`) and `z_order = 0`.
    pub fn new(asset_id: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
            color: [255; 4],
            z_order: 0,
        }
    }
}

/// Explicit render gate. Required by the default sprite extract: entities
/// with `Transform + Sprite` but no `Visibility` are never emitted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Visibility {
    pub visible: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Self { visible: true }
    }
}

/// Debug / find-by-name label for an entity. Not consulted by any render
/// path; examples and tools use it to locate specific entities.
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Opt-in system: copies `Position.0` into `Transform.position` for every
/// entity that carries both components. Does not touch rotation or scale.
///
/// Physics `Position` remains the source of truth per `D-033`; this is a
/// one-way sync meant to run after `physics_step` and before any sprite
/// extract stage that needs authoritative post-physics visuals. No reverse
/// sync.
pub fn sync_position_to_transform(world: &mut World) {
    let entities = world.query2_entities::<Position, Transform>();
    for entity in entities {
        let position = match world.get::<Position>(entity) {
            Some(p) => p.0,
            None => continue,
        };
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.position = position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_default_is_identity() {
        let t = Transform::default();
        assert_eq!(t.position, Vec2::ZERO);
        assert_eq!(t.rotation, 0.0);
        assert_eq!(t.scale, Vec2::ONE);
    }

    #[test]
    fn transform_from_position_sets_position_only() {
        let t = Transform::from_position(Vec2::new(7.0, -2.0));
        assert_eq!(t.position, Vec2::new(7.0, -2.0));
        assert_eq!(t.rotation, 0.0);
        assert_eq!(t.scale, Vec2::ONE);
    }

    #[test]
    fn visibility_default_is_visible() {
        assert!(Visibility::default().visible);
    }

    #[test]
    fn sprite_new_defaults_color_and_z_order() {
        let s = Sprite::new("player");
        assert_eq!(s.asset_id, "player");
        assert_eq!(s.color, [255; 4]);
        assert_eq!(s.z_order, 0);
    }

    #[test]
    fn tag_new_stores_name() {
        let t = Tag::new("hero");
        assert_eq!(t.name, "hero");
    }

    #[test]
    fn sync_position_to_transform_copies_position() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(3.0, 4.0)));
        world.insert(
            e,
            Transform {
                position: Vec2::ZERO,
                rotation: 1.5,
                scale: Vec2::splat(2.0),
            },
        );

        sync_position_to_transform(&mut world);

        let t = world.get::<Transform>(e).unwrap();
        assert_eq!(t.position, Vec2::new(3.0, 4.0));
        assert_eq!(t.rotation, 1.5);
        assert_eq!(t.scale, Vec2::splat(2.0));
    }

    #[test]
    fn sync_position_to_transform_skips_entities_missing_either() {
        let mut world = World::new();
        let only_position = world.spawn();
        world.insert(only_position, Position(Vec2::new(9.0, 9.0)));

        let only_transform = world.spawn();
        world.insert(only_transform, Transform::default());

        sync_position_to_transform(&mut world);

        assert!(world.get::<Transform>(only_position).is_none());
        assert_eq!(
            world.get::<Transform>(only_transform).unwrap().position,
            Vec2::ZERO
        );
    }

    #[test]
    fn sync_position_to_transform_does_not_touch_position() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(1.0, 2.0)));
        world.insert(e, Transform::default());

        sync_position_to_transform(&mut world);

        world.get_mut::<Transform>(e).unwrap().position = Vec2::splat(42.0);
        assert_eq!(world.get::<Position>(e).unwrap().0, Vec2::new(1.0, 2.0));
    }
}
