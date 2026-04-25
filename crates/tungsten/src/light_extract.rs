//! M29 light extract.
//!
//! Pulls `(Transform, Light)` out of the world, culls by camera-AABB
//! distance with directional-first retention, caps at `LIGHT_CAP`, and
//! packs into a `LightUbo` along with the optional `AmbientLight` resource
//! (defaults to `Vec3::ONE` per `D-061`).

use tungsten_core::{AmbientLight, CameraState, Light, Transform, World};
use tungsten_render::{cull_to_cap, pack_lights, LightUbo};

/// Build the per-frame `LightUbo` from world state.
#[must_use]
pub fn extract_lights(
    world: &World,
    camera: &CameraState,
    viewport_w: f32,
    viewport_h: f32,
) -> LightUbo {
    let aabb = camera.visible_world_aabb(viewport_w, viewport_h);
    let entries: Vec<(glam::Vec2, Light)> = world
        .query2::<Transform, Light>()
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
