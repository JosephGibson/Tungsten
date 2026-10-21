//! The level's systems: its clock, the camera's zoom and the glows.

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    AssetRegistry, CameraController, Entity, Light, MaterialRegistry, Sprite, Time, Transform,
    Visibility, WindowSize, World,
};

use super::components::{Glow, GlowPart, GlowParts, SceneTime, Zoom};
use super::{CAMERA_ROWS, TILE};
use crate::game::GAMEPLAY;
use crate::shared::SaveId;

/// The halo's sprite and material, and the hot core's.
pub const HALO: &str = "ex10_halo";
pub const HALO_MATERIAL: &str = "ex10_soft_halo";
pub const CORE: &str = "ex10_flame_glow";
pub const CORE_MATERIAL: &str = "ex10_soft_flame";
/// Where the glows draw among the level's sprites: over the world's props,
/// under the hazards, the decks and the player, as their art expects.
pub const HALO_Z: i32 = -8;
pub const CORE_Z: i32 = -7;

/// `level_zoom`: the camera's zoom shows [`CAMERA_ROWS`] rows of tiles in
/// the window's height, times the player's [`Zoom`].
pub fn zoom(world: &mut World) {
    let height = world
        .get_resource::<WindowSize>()
        .map_or(720, |size| size.height);
    let base = (height as f32 / (CAMERA_ROWS * TILE)).max(f32::EPSILON);
    let zoom = world.get_resource::<Zoom>().map_or(1.0, |zoom| zoom.0);
    world.resource_mut::<CameraController>().zoom_multiplier = base * zoom;
}

/// `level_clock`: the level's clock advances by each step.
pub fn clock(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if let Some(time) = world.get_resource_mut::<SceneTime>() {
        time.0 += dt;
    }
}

/// The level's clock at the instant bodies are drawn: [`SceneTime`] less
/// the part of a step the drawn bodies trail the last step by, while the
/// clock interpolates, so the scene moves in frames that run no step.
#[must_use]
pub fn drawn_time(world: &World) -> f32 {
    let time = world.get_resource::<SceneTime>().map_or(0.0, |t| t.0);
    match world
        .get_resource::<Time>()
        .filter(|clock| clock.interpolate())
    {
        Some(clock) => (time - (1.0 - clock.alpha()) * clock.fixed_step()).max(0.0),
        None => time,
    }
}

/// `level_glows`: each glow's halo, its core and its light, spawned the
/// first frame the glow is seen and moved each frame to its owner's drawn
/// point, pulsing by the level's clock; a part whose owner has gone goes.
pub fn glows(world: &mut World) {
    let time = drawn_time(world);
    let owners: Vec<(Entity, Glow, Vec2, Option<GlowParts>)> = world
        .query::<(Entity, &Glow, &Transform, Option<&GlowParts>)>()
        .map(|(entity, glow, at, parts)| (entity, *glow, at.position, parts.copied()))
        .collect();
    for (owner, glow, at, parts) in owners {
        let parts = parts.unwrap_or_else(|| spawn_parts(world, owner, &glow));
        let pulse = 1.0 + 0.04 * (time * 4.0 + parts.phase).sin();
        let centre = glow.centre(at);
        if let Some(halo) = parts.halo {
            place(world, halo, centre, glow.radius * pulse, glow.hidden);
        }
        if let Some(core) = parts.core {
            place(world, core, centre, glow.core, glow.hidden);
        }
        let flicker = glow.light + 0.13 * (time * 5.0 + parts.phase).sin();
        if let Some(transform) = world.get_mut::<Transform>(parts.light) {
            transform.position = centre;
        }
        if let Some(light) = world.get_mut::<Light>(parts.light) {
            light.intensity = if glow.hidden { 0.0 } else { flicker };
        }
    }
    let orphans: Vec<Entity> = world
        .query::<(Entity, &GlowPart)>()
        .filter(|(_, part)| !world.has::<Glow>(part.owner))
        .map(|(entity, _)| entity)
        .collect();
    for entity in orphans {
        world.despawn(entity);
    }
}

/// A glow's parts for `owner`, owned by gameplay, phased by its save ID.
fn spawn_parts(world: &mut World, owner: Entity, glow: &Glow) -> GlowParts {
    let phase = world.get::<SaveId>(owner).map_or(0.0, |id| id.0 as f32);
    let [r, g, b, _] = glow.color.map(|c| f32::from(c) / 255.0);
    let mut light = Light::point(Vec3::new(r, g, b), glow.reach());
    light.intensity = glow.light;
    let light = world.spawn_with((
        Transform::from_position(glow.centre(Vec2::ZERO)),
        light,
        GlowPart { owner },
        SceneEntity { state_id: GAMEPLAY },
    ));
    let halo = (glow.radius > 0.0)
        .then(|| spawn_sprite(world, owner, HALO, HALO_MATERIAL, HALO_Z, glow.color));
    let core = (glow.core > 0.0)
        .then(|| spawn_sprite(world, owner, CORE, CORE_MATERIAL, CORE_Z, glow.color));
    let parts = GlowParts {
        halo,
        core,
        light,
        phase,
    };
    world.insert(owner, parts);
    parts
}

fn spawn_sprite(
    world: &mut World,
    owner: Entity,
    asset: &str,
    material: &str,
    z_order: i32,
    color: [u8; 4],
) -> Entity {
    let asset_id = world.resource_mut::<AssetRegistry>().intern_sprite(asset);
    let material_id = world
        .get_resource::<MaterialRegistry>()
        .and_then(|materials| materials.get(material));
    let sprite = Sprite {
        asset_id,
        color,
        z_order,
        material_id,
    };
    world.spawn_with((
        Transform::default(),
        sprite,
        Visibility { visible: false },
        GlowPart { owner },
        SceneEntity { state_id: GAMEPLAY },
    ))
}

/// Draws the part `entity` as a disc of `radius` about `centre`, its
/// sprite scaled from the art's size.
fn place(world: &mut World, entity: Entity, centre: Vec2, radius: f32, hidden: bool) {
    let width = world
        .get::<Sprite>(entity)
        .and_then(|sprite| world.resource::<AssetRegistry>().sprite(sprite.asset_id))
        .map_or(1.0, |asset| asset.width.max(1) as f32);
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = centre - Vec2::splat(radius);
        transform.scale = Vec2::splat(2.0 * radius / width);
    }
    if let Some(visibility) = world.get_mut::<Visibility>(entity) {
        visibility.visible = !hidden;
    }
}
