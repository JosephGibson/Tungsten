//! The level's systems: its drawn clock, the camera's zoom, the glows and
//! each area's light; and the areas the view shows.

use glam::{Vec2, Vec3};
use tungsten::SceneEntity;
use tungsten::core::{
    AmbientLight, AssetRegistry, CameraRig, Collider, Entity, Light, LightKind, MaterialRegistry,
    Position, Shape, Sprite, Time, Transform, Visibility, World,
};
use tungsten::kit::{LocalOffset, spawn_child};

use super::components::{Area, AreaLight, Glow, GlowParts, Zoom};
use super::{AMBIENT, AMBIENT_INDOOR, AREA_FADE, LIGHT_EASE, MOONLIGHT_INTENSITY, TILE};
use crate::features::run::Run;
use crate::game::GAMEPLAY;
use crate::shell::options::GameOptions;

/// The halo's sprite and material, and the hot core's.
pub const HALO: &str = "ex10_halo";
pub const HALO_MATERIAL: &str = "ex10_soft_halo";
pub const CORE: &str = "ex10_flame_glow";
pub const CORE_MATERIAL: &str = "ex10_soft_flame";
/// Where the glows draw among the level's sprites: over the world's props,
/// under the hazards, the decks and the player, as their art expects.
pub const HALO_Z: i32 = -8;
pub const CORE_Z: i32 = -7;

/// `level_zoom`: the player's camera rig, which its prefab gives a view of
/// [`CAMERA_ROWS`](super::CAMERA_ROWS) rows of tiles in the window's height, takes the
/// player's [`Zoom`].
pub fn zoom(world: &mut World) {
    let Some(player) = world.get_resource::<Run>().map(|run| run.player) else {
        return;
    };
    let zoom = world.get_resource::<Zoom>().map_or(1.0, |zoom| zoom.0);
    if let Some(rig) = world.get_mut::<CameraRig>(player) {
        rig.zoom = zoom;
    }
}

/// The level's clock at the instant bodies are drawn: the engine's game
/// seconds ([`Time::elapsed`]), which a pause holds, less the step the
/// drawn bodies trail them by while the clock interpolates, or less the
/// part of a step past the last one when it does not, so the scene moves
/// in frames that run no step. The clouds drift and the glows and lights
/// pulse by it. 0 with no clock.
#[must_use]
pub fn drawn_time(world: &World) -> f32 {
    let Some(clock) = world.get_resource::<Time>() else {
        return 0.0;
    };
    let step = f64::from(clock.fixed_step());
    let behind = if clock.interpolate() {
        step
    } else {
        f64::from(clock.alpha()) * step
    };
    (clock.elapsed() - behind).max(0.0) as f32
}

/// Whether the Bloom option is on: the halos, bloom's look drawn as
/// sprites, show only then (B6).
#[must_use]
pub fn bloom_on(world: &World) -> bool {
    world
        .get_resource::<GameOptions>()
        .is_none_or(|options| options.bloom)
}

/// `level_glows`: each glow's halo, its core and its light, spawned as its
/// owner's children (the kit's) the first frame the glow is seen, each hung
/// from the owner's point by its offset, pulsing by the drawn clock; a part
/// leaves with its owner, as any child does. The halos hide while the Bloom
/// option is off.
pub fn glows(world: &mut World) {
    let time = drawn_time(world);
    let bloom = bloom_on(world);
    let owners: Vec<(Entity, Glow, Vec2, Option<GlowParts>)> = world
        .query::<(Entity, &Glow, &Transform, Option<&GlowParts>)>()
        .map(|(entity, glow, at, parts)| (entity, *glow, at.position, parts.copied()))
        .collect();
    for (owner, glow, at, parts) in owners {
        let parts = parts
            .filter(|parts| parts.spawned)
            .unwrap_or_else(|| spawn_parts(world, owner, &glow));
        let pulse = 1.0 + 0.04 * (time * 4.0 + parts.phase).sin();
        let centre = glow.centre(at);
        if let Some(halo) = parts.halo {
            place(
                world,
                halo,
                at,
                centre,
                glow.radius * pulse,
                glow.hidden || !bloom,
            );
        }
        if let Some(core) = parts.core {
            place(world, core, at, centre, glow.core, glow.hidden);
        }
        let flicker = glow.light + 0.13 * (time * 5.0 + parts.phase).sin();
        let Some(lamp) = parts.light else {
            continue;
        };
        hang(world, lamp, at, centre);
        if let Some(light) = world.get_mut::<Light>(lamp) {
            light.intensity = if glow.hidden { 0.0 } else { flicker };
        }
    }
}

/// Hangs the part `entity` at `point` from its owner, drawn at `at`: its
/// [`LocalOffset`], which the kit's propagation holds it at, and its point
/// for this frame, since the glows run after the frame's propagation (the
/// player's lantern sets its glow's offset by the frame its animator shows).
fn hang(world: &mut World, entity: Entity, at: Vec2, point: Vec2) {
    if let Some(local) = world.get_mut::<LocalOffset>(entity) {
        local.0 = point - at;
    }
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = point;
    }
}

/// A glow's parts for `owner`, its children (the kit's `spawn_child`),
/// owned by gameplay, phased by its save ID's low bits, which keep the
/// clock's in an `f32` at an authored ID's size, or by its column when it
/// has none; recorded in the owner's [`GlowParts`].
fn spawn_parts(world: &mut World, owner: Entity, glow: &Glow) -> GlowParts {
    let phase = crate::shared::save_id(world, owner).map_or_else(
        || {
            world
                .get::<Transform>(owner)
                .map_or(0.0, |transform| transform.position.x / TILE)
        },
        |id| (id.0 % 1024) as f32,
    );
    let [r, g, b, _] = glow.color.map(|c| f32::from(c) / 255.0);
    let offset = Vec2::from(glow.offset);
    let light = (glow.light > 0.0).then(|| {
        let mut light = Light::point(Vec3::new(r, g, b), glow.reach());
        light.intensity = glow.light;
        spawn_child(
            world,
            owner,
            offset,
            (light, SceneEntity { state_id: GAMEPLAY }),
        )
    });
    let halo = (glow.radius > 0.0).then(|| {
        let corner = offset - Vec2::splat(glow.radius);
        spawn_sprite(
            world,
            owner,
            corner,
            HALO,
            HALO_MATERIAL,
            HALO_Z,
            glow.color,
        )
    });
    let core = (glow.core > 0.0).then(|| {
        let corner = offset - Vec2::splat(glow.core);
        spawn_sprite(
            world,
            owner,
            corner,
            CORE,
            CORE_MATERIAL,
            CORE_Z,
            glow.color,
        )
    });
    let parts = GlowParts {
        halo,
        core,
        light,
        phase,
        spawned: true,
    };
    // A prefab's glow came with its parts' place; one built in code takes
    // it here.
    match world.get_mut::<GlowParts>(owner) {
        Some(place) => *place = parts,
        None => world.insert(owner, parts),
    }
    parts
}

fn spawn_sprite(
    world: &mut World,
    owner: Entity,
    offset: Vec2,
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
    spawn_child(
        world,
        owner,
        offset,
        (
            sprite,
            Visibility { visible: false },
            SceneEntity { state_id: GAMEPLAY },
        ),
    )
}

/// Draws the part `entity` of the owner drawn at `at` as a disc of `radius`
/// about `centre`, its sprite scaled from the art's size.
fn place(world: &mut World, entity: Entity, at: Vec2, centre: Vec2, radius: f32, hidden: bool) {
    let width = world
        .get::<Sprite>(entity)
        .and_then(|sprite| world.resource::<AssetRegistry>().sprite(sprite.asset_id))
        .map_or(1.0, |asset| asset.width.max(1) as f32);
    hang(world, entity, at, centre - Vec2::splat(radius));
    if let Some(transform) = world.get_mut::<Transform>(entity) {
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
