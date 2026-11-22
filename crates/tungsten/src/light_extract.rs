//! M29 light extract.
//!
//! Pulls the `(Transform, Light)` pairs that can light the view out of the
//! world (`tungsten_render::lights_view`: a dark light, and a point light
//! whose radius ends short of the view, light no pixel of it), culls by
//! camera-AABB distance with directional-first retention, caps at
//! `LIGHT_CAP`, and packs into a `LightUbo` along with the optional
//! `AmbientLight` resource (defaults to `Vec3::ONE` per `D-061`).

use tungsten_core::{AmbientLight, CameraState, Entity, Light, Transform, World};
use tungsten_render::{LightUbo, cull_to_cap, lights_view, pack_lights};

/// Build the per-frame `LightUbo` from world state.
#[must_use]
pub fn extract_lights(
    world: &World,
    camera: &CameraState,
    viewport_w: f32,
    viewport_h: f32,
) -> LightUbo {
    let aabb = camera.visible_world_aabb(viewport_w, viewport_h);
    // Only the lights that can light the view are copied out.
    let entries: Vec<(glam::Vec2, Light)> = world
        .query::<(Entity, &Transform, &Light)>()
        .filter(|(_, t, l)| lights_view(aabb, t.position, l))
        .map(|(_, t, l)| (t.position, *l))
        .collect();
    let ambient = world
        .get_resource::<AmbientLight>()
        .copied()
        .unwrap_or_default()
        .0;
    let packed = cull_to_cap(aabb, &entries);
    pack_lights(&packed, ambient)
}

#[cfg(test)]
#[path = "tests/light_extract.rs"]
mod tests;
