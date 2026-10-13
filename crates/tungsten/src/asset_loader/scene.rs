//! Scene files (`D-046`): load with path context, spawn through the
//! `CommandBuffer`; and the umbrella's scene components (M48): `light` and
//! `parallax_layer`, which `App` registers beside the default extracts that
//! draw them, and `squash_stretch`, which `GameFeelPlugin` registers.

use std::path::Path;

use glam::{Vec2, Vec3};
use serde::Deserialize;
use tungsten_core::assets::{SceneData, SceneTransform};
use tungsten_core::{
    AssetRegistry, CommandBuffer, ComponentRegistry, Easing, Light, LightKind, ParallaxLayer,
    PreparedComponent, SpawnContext, Sprite, SpriteAssetId, SpriteSquashStretch, SquashTrigger,
    Tag, Transform, Visibility, World,
};

use crate::state::{SceneEntity, StateId};

/// `SceneData::load(path, world)`, with the path in the error: the entries'
/// components are checked against `world`'s `ComponentRegistry`.
pub fn load_scene(world: &World, path: &Path) -> anyhow::Result<SceneData> {
    SceneData::load(path, world)
        .map_err(|source| anyhow::anyhow!("Failed to load scene '{}': {source}", path.display()))
}

/// Spawn scene entities through `CommandBuffer`; D-046 leaves sprite names unresolved:
/// each is interned, so a sprite a later manifest reload registers draws from then on.
/// D-055 one `Tween` per entity — additional tween entries log `ERROR` and are dropped.
/// Each entry's registered components follow its fields, in name order, so the
/// entity lands in one archetype move and a component overrides a field of
/// the same type.
///
/// # Panics
///
/// When an entry names a sprite and the `AssetRegistry` resource is missing,
/// or the `CommandBuffer` resource is missing; and on a component this World's
/// registry rejects (data that skipped `SceneData::load`'s check against it),
/// before anything is queued or interned.
pub fn spawn_scene(world: &mut World, data: &SceneData, state_id: StateId) {
    let prepared = prepare_components(world, data);
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
    for ((entry, sprite_id), components) in data.entities.iter().zip(sprite_ids).zip(prepared) {
        let pending = buf.spawn();
        buf.insert_pending(pending, scene_transform(&entry.transform));
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
        for component in components {
            component.insert_pending(buf, pending);
        }
    }
}

/// The first pass: every entry's components, built with `&World` before the
/// spawn changes anything. A World without a `ComponentRegistry` finds every
/// component unknown, and still spawns a scene that names none.
fn prepare_components(world: &World, data: &SceneData) -> Vec<Vec<PreparedComponent>> {
    let empty = ComponentRegistry::new();
    let registry = world.get_resource::<ComponentRegistry>().unwrap_or(&empty);
    data.entities
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            if entry.components.is_empty() {
                return Vec::new();
            }
            let at = SpawnContext::new(world, scene_transform(&entry.transform));
            entry
                .components
                .iter()
                .map(|(name, value)| {
                    registry
                        .prepare(name, value, &at)
                        .unwrap_or_else(|err| panic!("scene entities[{i}]: {err}"))
                })
                .collect()
        })
        .collect()
}

fn scene_transform(transform: &SceneTransform) -> Transform {
    Transform {
        position: Vec2::from(transform.position),
        rotation: transform.rotation,
        scale: Vec2::from(transform.scale),
    }
}

/// Registers `light` and `parallax_layer`, the components the default
/// extracts draw; `App::with_plugins` calls it before the plugins build.
pub(crate) fn register_extract_components(world: &mut World) {
    world.register_component_with("light", |def: LightDef, _: &SpawnContext<'_>| (def.0,));
    world.register_component_with(
        "parallax_layer",
        |def: ParallaxDef, _: &SpawnContext<'_>| (def.0,),
    );
}

/// Registers `squash_stretch`, which `GameFeelPlugin`'s systems read.
pub(crate) fn register_squash_stretch(world: &mut World) {
    world.register_component_with("squash_stretch", |def: SquashDef, _: &SpawnContext<'_>| {
        (def.0,)
    });
}

fn unit() -> f32 {
    1.0
}

fn finite2([x, y]: [f32; 2]) -> bool {
    x.is_finite() && y.is_finite()
}

/// `components.light` as written.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum LightFields {
    Point {
        radius: f32,
        color: [f32; 3],
        #[serde(default = "unit")]
        intensity: f32,
    },
    Directional {
        angle: f32,
        color: [f32; 3],
        #[serde(default = "unit")]
        intensity: f32,
    },
}

/// `components.light`, checked.
#[derive(Deserialize)]
#[serde(try_from = "LightFields")]
struct LightDef(Light);

impl TryFrom<LightFields> for LightDef {
    type Error = String;

    fn try_from(fields: LightFields) -> Result<Self, String> {
        let (kind, color, intensity) = match fields {
            LightFields::Point {
                radius,
                color,
                intensity,
            } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return Err(format!("`radius` must be finite and > 0 (got {radius})"));
                }
                (LightKind::Point { radius }, color, intensity)
            }
            LightFields::Directional {
                angle,
                color,
                intensity,
            } => {
                if !angle.is_finite() {
                    return Err(format!("`angle` must be finite (got {angle})"));
                }
                (LightKind::Directional { angle }, color, intensity)
            }
        };
        if !color.iter().all(|c| c.is_finite() && *c >= 0.0) {
            let [r, g, b] = color;
            return Err(format!(
                "`color` channels must be finite and >= 0 (got [{r}, {g}, {b}])"
            ));
        }
        if !(intensity.is_finite() && intensity >= 0.0) {
            return Err(format!(
                "`intensity` must be finite and >= 0 (got {intensity})"
            ));
        }
        Ok(Self(Light {
            kind,
            color: Vec3::from(color),
            intensity,
        }))
    }
}

/// `components.parallax_layer` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParallaxFields {
    scroll_factor: [f32; 2],
}

/// `components.parallax_layer`, checked.
#[derive(Deserialize)]
#[serde(try_from = "ParallaxFields")]
struct ParallaxDef(ParallaxLayer);

impl TryFrom<ParallaxFields> for ParallaxDef {
    type Error = String;

    fn try_from(fields: ParallaxFields) -> Result<Self, String> {
        let [x, y] = fields.scroll_factor;
        if !finite2(fields.scroll_factor) {
            return Err(format!("`scroll_factor` must be finite (got [{x}, {y}])"));
        }
        Ok(Self(ParallaxLayer::new(Vec2::new(x, y))))
    }
}

/// `squash_stretch.on`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SceneSquashTrigger {
    OnLand,
    OnHit,
    OnPickup,
    Manual,
}

/// `components.squash_stretch` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SquashFields {
    on: SceneSquashTrigger,
    amount: [f32; 2],
    duration: f32,
    #[serde(default)]
    easing: Easing,
}

/// `components.squash_stretch`, checked.
#[derive(Deserialize)]
#[serde(try_from = "SquashFields")]
struct SquashDef(SpriteSquashStretch);

impl TryFrom<SquashFields> for SquashDef {
    type Error = String;

    fn try_from(fields: SquashFields) -> Result<Self, String> {
        let [x, y] = fields.amount;
        if !(finite2(fields.amount) && x > 0.0 && y > 0.0) {
            return Err(format!(
                "`amount` must be finite and > 0 on both axes (got [{x}, {y}])"
            ));
        }
        if !(fields.duration.is_finite() && fields.duration > 0.0) {
            return Err(format!(
                "`duration` must be finite and > 0 (got {})",
                fields.duration
            ));
        }
        let on = match fields.on {
            SceneSquashTrigger::OnLand => SquashTrigger::OnLand,
            SceneSquashTrigger::OnHit => SquashTrigger::OnHit,
            SceneSquashTrigger::OnPickup => SquashTrigger::OnPickup,
            SceneSquashTrigger::Manual => SquashTrigger::Manual,
        };
        Ok(Self(SpriteSquashStretch {
            on,
            amount: Vec2::new(x, y),
            duration: fields.duration,
            easing: fields.easing,
        }))
    }
}
