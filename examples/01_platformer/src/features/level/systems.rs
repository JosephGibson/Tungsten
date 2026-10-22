//! The level's systems: its clock, the camera's zoom, the glows and each
//! area's light; and the areas the view shows.

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    AmbientLight, AssetRegistry, CameraController, Collider, Entity, Light, LightKind,
    MaterialRegistry, Position, Shape, Sprite, Time, Transform, Visibility, WindowSize, World,
};

use super::components::{Area, AreaLight, Glow, GlowPart, GlowParts, SceneTime, Zoom};
use super::{
    AMBIENT, AMBIENT_INDOOR, AREA_FADE, CAMERA_ROWS, LIGHT_EASE, MOONLIGHT_INTENSITY, TILE,
};
use crate::features::run::Run;
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

/// The area the run's player last entered, as the run names it.
#[must_use]
pub fn current_area(world: &World) -> Option<Area> {
    let name = world.get_resource::<Run>()?.area.clone()?;
    world
        .query::<&Area>()
        .find(|area| area.name == name)
        .cloned()
}

/// `level_light`, in the present phase: an indoor area's ambient
/// ([`AMBIENT_INDOOR`]) with the moon out, an outdoor one's midnight
/// ([`AMBIENT`]) with the moon at [`MOONLIGHT_INTENSITY`], eased over
/// [`LIGHT_EASE`] real seconds; the run's first frame snaps. Presentation.
pub fn light(world: &mut World) {
    let Some(state) = world.get_resource::<AreaLight>().copied() else {
        return;
    };
    let indoor = current_area(world).is_some_and(|area| area.indoor);
    let (ambient, moon) = if indoor {
        (AMBIENT_INDOOR, 0.0)
    } else {
        (AMBIENT, MOONLIGHT_INTENSITY)
    };
    // The share of the full ease this frame covers; all of it at the first.
    let share = if state.lit {
        world.resource::<Time>().real_delta() / LIGHT_EASE
    } else {
        f32::INFINITY
    };
    let toward =
        |from: f32, to: f32, span: f32| from + (to - from).clamp(-span * share, span * share);
    let span = (AMBIENT - AMBIENT_INDOOR).abs();
    if let Some(light) = world.get_resource_mut::<AmbientLight>() {
        let now = light.0;
        light.0 = Vec3::new(
            toward(now.x, ambient.x, span.x),
            toward(now.y, ambient.y, span.y),
            toward(now.z, ambient.z, span.z),
        );
    }
    for light in world.query_mut::<&mut Light>() {
        if matches!(light.kind, LightKind::Directional { .. }) {
            light.intensity = toward(light.intensity, moon, MOONLIGHT_INTENSITY);
        }
    }
    world.insert_resource(AreaLight { lit: true });
}

/// How much the view centred at `x` shows each area: 1 inside its columns,
/// crossfading to its neighbour's over [`AREA_FADE`] each side of their
/// border; west to east, the shares summing to 1.
#[must_use]
pub fn area_shares(world: &World, x: f32) -> Vec<(Area, f32)> {
    let mut spans: Vec<(f32, f32, Area)> = world
        .query::<(&Area, &Position, &Collider)>()
        .filter_map(|(area, at, collider)| {
            let Shape::Aabb { half_extents } = collider.shape else {
                return None;
            };
            let centre = at.0.x + collider.offset.x;
            Some((
                centre - half_extents.x,
                centre + half_extents.x,
                area.clone(),
            ))
        })
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let last = spans.len().saturating_sub(1);
    spans
        .into_iter()
        .enumerate()
        .map(|(k, (left, right, area))| {
            let rise = if k == 0 {
                1.0
            } else {
                ((x - left + AREA_FADE) / (2.0 * AREA_FADE)).clamp(0.0, 1.0)
            };
            let fall = if k == last {
                1.0
            } else {
                ((right + AREA_FADE - x) / (2.0 * AREA_FADE)).clamp(0.0, 1.0)
            };
            (area, rise * fall)
        })
        .filter(|(_, share)| *share > 0.0)
        .collect()
}
