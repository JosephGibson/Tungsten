//! The physics plugin: the snapshot and the step at the fixed stage, the
//! one-way, interpolating sync to `Transform` after update, the collision
//! and sensor event queues and the `rigid_body` scene component. A plugin
//! whose every need is met by core, so it lives here (the kit's shape,
//! `D-007`).

use glam::Vec2;
use serde::Deserialize;

use crate::component_registry::{MissingAsset, SpawnContext};
use crate::components::Transform;
use crate::ecs::World;
use crate::physics::{
    Collider, CollisionEvent, PhysicsBuffers, PhysicsLayers, PhysicsMaterials, Position,
    PrevPosition, RigidBody, RigidBodyBundle, SensorEntered, Shape, Spin, Velocity, physics_step,
};
use crate::schedule::{Plugin, Schedule, Stage, system};
use crate::time::Time;

/// `physics_prev_snapshot` in `FixedUpdate`, before `physics_step`.
pub const PHYSICS_PREV_SNAPSHOT: &str = "physics_prev_snapshot";
/// `physics_step` in `FixedUpdate`.
pub const PHYSICS_STEP: &str = "physics_step";
/// [`physics_sync`] in `PostUpdate`, before every other engine system there.
pub const PHYSICS_SYNC: &str = "physics_sync";

/// Registers [`PHYSICS_PREV_SNAPSHOT`] and [`PHYSICS_STEP`] after it in
/// `FixedUpdate`, [`PHYSICS_SYNC`] in `PostUpdate`,
/// `EventQueue<CollisionEvent>`, `EventQueue<SensorEntered>` and the
/// `rigid_body` scene component, which
/// spawns a [`RigidBodyBundle`] at the entry's position
/// ([`component_registry`](crate::component_registry)), its layer and
/// material names checked against the World's [`PhysicsLayers`] and
/// [`PhysicsMaterials`]. The snapshot is
/// registered first, so it heads the stage ahead of a game's fixed systems,
/// which `DefaultPlugins` builds before. `PhysicsConfig` stays the app's
/// resource, so a game edits it before or after adding the plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<CollisionEvent>();
        world.register_event::<SensorEntered>();
        world.register_component_with_assets(RIGID_BODY, check_rigid_body, spawn_rigid_body);
        schedule.add(
            Stage::FixedUpdate,
            system(PHYSICS_PREV_SNAPSHOT, physics_prev_snapshot),
        );
        schedule.add(
            Stage::FixedUpdate,
            system(PHYSICS_STEP, physics_step).after(PHYSICS_PREV_SNAPSHOT),
        );
        schedule.add(Stage::PostUpdate, system(PHYSICS_SYNC, physics_sync));
    }
}

/// Copies each body's `Position` into its [`PrevPosition`], and, once a
/// step has turned a spinner, each [`Spin`]'s angle into its `prev_angle`:
/// the history a fixed step starts from. `PhysicsPlugin` runs it first in `fixed_update` under
/// [`PHYSICS_PREV_SNAPSHOT`], so a step's history spans whatever moves the
/// body in that step, gameplay's writes and the solve.
pub fn physics_prev_snapshot(world: &mut World) {
    world
        .query_mut_slices::<(&mut PrevPosition, &Position)>()
        .for_each(|(_, (prevs, positions))| {
            prevs
                .iter_mut()
                .zip(positions)
                .for_each(|(prev, position)| prev.0 = position.0);
        });
    if spinning(world) {
        for spin in world.query_mut::<&mut Spin>() {
            spin.prev_angle = spin.angle;
        }
    }
}

/// Whether the last step turned a spinner, so `Spin`s need their history and
/// their drawing: a world without one never queries them.
fn spinning(world: &World) -> bool {
    world
        .get_resource::<PhysicsBuffers>()
        .is_some_and(PhysicsBuffers::has_spins)
}

/// Draws each body: writes `Transform.position` as
/// `prev + (cur - prev) * alpha` ([`Time::alpha`]) for an entity with a
/// [`PrevPosition`] while [`Time::interpolate`] is on, and as its `Position`
/// otherwise and in a world without `Time`; and, once a step has turned a
/// spinner, `Transform.rotation` from a [`Spin`] the same way, its
/// `prev_angle` to its `angle`. Never writes `Position` or `Spin` (`D-033`).
pub fn physics_sync(world: &mut World) {
    let alpha = world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha);
    // The slice form with `for_each` at both levels keeps the row loop in
    // registers (`D-135`); whether the history column exists is per
    // archetype.
    world
        .query_mut_slices::<(&mut Transform, &Position, Option<&PrevPosition>)>()
        .for_each(|(_, (transforms, positions, prevs))| match (alpha, prevs) {
            (Some(alpha), Some(prevs)) => transforms.iter_mut().zip(positions).zip(prevs).for_each(
                |((transform, position), prev)| {
                    transform.position = prev.0 + (position.0 - prev.0) * alpha;
                },
            ),
            _ => transforms
                .iter_mut()
                .zip(positions)
                .for_each(|(transform, position)| transform.position = position.0),
        });
    if spinning(world) {
        for (transform, spin) in world.query_mut::<(&mut Transform, &Spin)>() {
            transform.rotation = match alpha {
                Some(alpha) => spin.prev_angle + (spin.angle - spin.prev_angle) * alpha,
                None => spin.angle,
            };
        }
    }
}

/// The scene component [`PhysicsPlugin`] registers.
const RIGID_BODY: &str = "rigid_body";

/// The largest collider size and offset a scene names, in pixels: with the
/// transform's ±2^24, a scene's bodies stay inside the broadphase's `i32`
/// cells.
const MAX_BODY_EXTENT: f32 = 16_384.0;

/// `rigid_body.kind`.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SceneBodyKind {
    Dynamic,
    Static,
    Kinematic,
    Sensor,
}

/// `rigid_body.shape`.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SceneShape {
    Aabb { half_extents: [f32; 2] },
    Circle { radius: f32 },
}

/// One entry of `rigid_body.layers` or `rigid_body.mask`: a layer number,
/// 0 to 31, or a physics layer's name ([`PhysicsLayers`]).
enum SceneLayer {
    Number(u32),
    Name(String),
}

impl<'de> Deserialize<'de> for SceneLayer {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entry;

        impl serde::de::Visitor<'_> for Entry {
            type Value = SceneLayer;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a layer number 0 to 31 or a physics layer name")
            }

            fn visit_u64<E: serde::de::Error>(self, layer: u64) -> Result<SceneLayer, E> {
                if layer < u64::from(u32::BITS) {
                    Ok(SceneLayer::Number(layer as u32))
                } else {
                    Err(E::custom(format_args!(
                        "a layer must be 0 to 31 (got {layer})"
                    )))
                }
            }

            fn visit_i64<E: serde::de::Error>(self, layer: i64) -> Result<SceneLayer, E> {
                match u64::try_from(layer) {
                    Ok(layer) => self.visit_u64(layer),
                    Err(_) => Err(E::invalid_value(
                        serde::de::Unexpected::Signed(layer),
                        &self,
                    )),
                }
            }

            fn visit_str<E: serde::de::Error>(self, name: &str) -> Result<SceneLayer, E> {
                Ok(SceneLayer::Name(name.to_owned()))
            }
        }

        deserializer.deserialize_any(Entry)
    }
}

/// `rigid_body.layers` or `rigid_body.mask` as written: the bits of its
/// numbers, and its names with their places in the list, which resolve at
/// spawn.
struct SceneLayers {
    bits: u32,
    names: Vec<(usize, String)>,
}

impl<'de> Deserialize<'de> for SceneLayers {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct List;

        impl<'de> serde::de::Visitor<'de> for List {
            type Value = SceneLayers;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a sequence")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<SceneLayers, A::Error> {
                // The numbers take the buffer a list of `u32` would, sized as
                // serde sizes one, so a list of numbers allocates as it did
                // before names were read.
                const MAX_PREALLOC_BYTES: usize = 1024 * 1024;
                let capacity = seq
                    .size_hint()
                    .unwrap_or(0)
                    .min(MAX_PREALLOC_BYTES / size_of::<u32>());
                let mut numbers: Vec<u32> = Vec::with_capacity(capacity);
                let mut names = Vec::new();
                let mut at = 0;
                while let Some(entry) = seq.next_element::<SceneLayer>()? {
                    match entry {
                        SceneLayer::Number(layer) => numbers.push(layer),
                        SceneLayer::Name(name) => names.push((at, name)),
                    }
                    at += 1;
                }
                let bits = numbers.iter().fold(0, |bits, layer| bits | 1 << layer);
                Ok(SceneLayers { bits, names })
            }
        }

        deserializer.deserialize_seq(List)
    }
}

/// `components.rigid_body` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RigidBodyFields {
    kind: SceneBodyKind,
    shape: SceneShape,
    #[serde(default)]
    offset: [f32; 2],
    mass: Option<f32>,
    /// The entry's own coefficients, over its material's; each 0 without
    /// either.
    restitution: Option<f32>,
    friction: Option<f32>,
    rolling: Option<f32>,
    /// A physics material's name ([`PhysicsMaterials`]).
    material: Option<String>,
    velocity: Option<[f32; 2]>,
    /// The layers the body is in; `[0]` when missing.
    layers: Option<SceneLayers>,
    /// The layers it meets; every layer when missing.
    mask: Option<SceneLayers>,
    /// A dynamic body's share of gravity; 1 when missing.
    gravity_scale: Option<f32>,
    /// A dynamic circle that rolls, spawned with a `Spin`.
    #[serde(default)]
    rolls: bool,
}

/// `components.rigid_body`, checked: the bundle's parts beside the entry's
/// position, and the names that resolve against the World at spawn.
#[derive(Deserialize)]
#[serde(try_from = "RigidBodyFields")]
struct RigidBodyDef {
    velocity: Option<Velocity>,
    collider: Collider,
    /// The body with its numbered layers and its own coefficients.
    body: RigidBody,
    /// It rolls, so its spawn puts a `Spin`.
    rolls: bool,
    /// The layer names of `layers` and of `mask`, with their places.
    layer_names: Vec<(usize, String)>,
    mask_names: Vec<(usize, String)>,
    material: Option<String>,
    /// The entry's own coefficients, which its material's do not replace.
    restitution: Option<f32>,
    friction: Option<f32>,
    rolling: Option<f32>,
}

impl TryFrom<RigidBodyFields> for RigidBodyDef {
    type Error = String;

    fn try_from(fields: RigidBodyFields) -> Result<Self, String> {
        let size = |v: f32| v > 0.0 && v <= MAX_BODY_EXTENT;
        let shape = match fields.shape {
            SceneShape::Aabb {
                half_extents: [x, y],
            } => {
                if !(size(x) && size(y)) {
                    return Err(format!(
                        "`shape.half_extents` must be in (0, {MAX_BODY_EXTENT}] (got [{x}, {y}])"
                    ));
                }
                Shape::Aabb {
                    half_extents: Vec2::new(x, y),
                }
            }
            SceneShape::Circle { radius } => {
                if !size(radius) {
                    return Err(format!(
                        "`shape.radius` must be in (0, {MAX_BODY_EXTENT}] (got {radius})"
                    ));
                }
                Shape::Circle { radius }
            }
        };
        let [ox, oy] = fields.offset;
        if !(ox.abs() <= MAX_BODY_EXTENT && oy.abs() <= MAX_BODY_EXTENT) {
            return Err(format!(
                "`offset` must be within ±{MAX_BODY_EXTENT} (got [{ox}, {oy}])"
            ));
        }
        if let Some(restitution) = fields.restitution
            && !(0.0..=1.0).contains(&restitution)
        {
            return Err(format!(
                "`restitution` must be in 0..=1 (got {restitution})"
            ));
        }
        for (field, value) in [("friction", fields.friction), ("rolling", fields.rolling)] {
            if let Some(value) = value
                && !(value.is_finite() && value >= 0.0)
            {
                return Err(format!(
                    "`{field}` must be finite and at least 0 (got {value})"
                ));
            }
        }
        let velocity = match fields.velocity {
            Some([x, y]) if !(x.is_finite() && y.is_finite()) => {
                return Err(format!("`velocity` must be finite (got [{x}, {y}])"));
            }
            velocity => velocity.map(|[x, y]| Velocity::new(x, y)),
        };
        if fields.mass.is_some() && !matches!(fields.kind, SceneBodyKind::Dynamic) {
            return Err("`mass` is for dynamic bodies only".to_owned());
        }
        let gravity_scale = match fields.gravity_scale {
            Some(_) if !matches!(fields.kind, SceneBodyKind::Dynamic) => {
                return Err("`gravity_scale` is for dynamic bodies only".to_owned());
            }
            Some(scale) if !scale.is_finite() => {
                return Err(format!("`gravity_scale` must be finite (got {scale})"));
            }
            scale => scale.unwrap_or(1.0),
        };
        if fields.rolls
            && !(matches!(fields.kind, SceneBodyKind::Dynamic)
                && matches!(shape, Shape::Circle { .. }))
        {
            return Err("`rolls` is for dynamic circles only".to_owned());
        }
        // `restitution`, `friction`, `rolling` and `material` are accepted on
        // every kind and do nothing on a sensor, which has no contacts.
        let (body, velocity) = match fields.kind {
            SceneBodyKind::Dynamic => {
                let mass = fields.mass.unwrap_or(1.0);
                if !(mass.is_finite() && mass > 0.0 && (1.0 / mass).is_finite()) {
                    return Err(format!(
                        "`mass` must be finite and > 0 with a finite inverse (got {mass})"
                    ));
                }
                (
                    RigidBody::dynamic().with_mass(mass),
                    Some(velocity.unwrap_or_default()),
                )
            }
            SceneBodyKind::Static => (RigidBody::r#static(), velocity),
            SceneBodyKind::Kinematic => {
                (RigidBody::kinematic(), Some(velocity.unwrap_or_default()))
            }
            SceneBodyKind::Sensor => (RigidBody::sensor(), Some(velocity.unwrap_or_default())),
        };
        let (layers, layer_names) = fields
            .layers
            .map_or((RigidBody::DEFAULT_LAYERS, Vec::new()), |list| {
                (list.bits, list.names)
            });
        let (mask, mask_names) = fields
            .mask
            .map_or((RigidBody::ALL_LAYERS, Vec::new()), |list| {
                (list.bits, list.names)
            });
        Ok(Self {
            velocity,
            collider: Collider {
                shape,
                offset: Vec2::new(ox, oy),
            },
            body: body
                .with_restitution(fields.restitution.unwrap_or(0.0))
                .with_friction(fields.friction.unwrap_or(0.0))
                .with_rolling(fields.rolling.unwrap_or(0.0))
                .with_layers(layers)
                .with_mask(mask)
                .with_gravity_scale(gravity_scale),
            rolls: fields.rolls,
            layer_names,
            mask_names,
            material: fields.material,
            restitution: fields.restitution,
            friction: fields.friction,
            rolling: fields.rolling,
        })
    }
}

/// `rigid_body`'s World check: each layer name in the World's
/// [`PhysicsLayers`] and the material in its [`PhysicsMaterials`].
fn check_rigid_body(def: &RigidBodyDef, world: &World) -> Result<(), MissingAsset> {
    let layers = world.get_resource::<PhysicsLayers>();
    for (field, names) in [("layers", &def.layer_names), ("mask", &def.mask_names)] {
        for (at, name) in names {
            if layers.and_then(|layers| layers.bit(name)).is_none() {
                return Err(MissingAsset::new(
                    format!("{field}[{at}]"),
                    "physics layer",
                    name,
                ));
            }
        }
    }
    if let Some(name) = &def.material
        && world
            .get_resource::<PhysicsMaterials>()
            .and_then(|materials| materials.get(name))
            .is_none()
    {
        return Err(MissingAsset::new("material", "physics material", name));
    }
    Ok(())
}

/// `rigid_body`'s spawn: a [`RigidBodyBundle`] at the entry's position, so
/// `Position` and `PrevPosition` start there; the transform's scale does not
/// size the collider. Its layer names add their bits, and its material's
/// coefficients stand where the entry gives none of its own.
fn spawn_rigid_body(def: RigidBodyDef, at: &SpawnContext<'_>) -> RigidBodyBundle {
    let RigidBodyDef {
        velocity,
        collider,
        mut body,
        rolls,
        layer_names,
        mask_names,
        material,
        restitution,
        friction,
        rolling,
    } = def;
    if !(layer_names.is_empty() && mask_names.is_empty()) {
        let layers = at
            .world()
            .get_resource::<PhysicsLayers>()
            .expect("the check found the physics layers");
        let bits = |names: &[(usize, String)]| {
            layers
                .bits(names.iter().map(|(_, name)| name.as_str()))
                .expect("the check found every layer name")
        };
        body.layers |= bits(&layer_names);
        body.mask |= bits(&mask_names);
    }
    if let Some(name) = material {
        let material = at
            .world()
            .get_resource::<PhysicsMaterials>()
            .and_then(|materials| materials.get(&name))
            .expect("the check found the physics material");
        body = body.with_material(material);
        if let Some(restitution) = restitution {
            body = body.with_restitution(restitution);
        }
        if let Some(friction) = friction {
            body = body.with_friction(friction);
        }
        if let Some(rolling) = rolling {
            body = body.with_rolling(rolling);
        }
    }
    body.rolls = rolls;
    RigidBodyBundle {
        position: Position(at.transform().position),
        velocity,
        collider,
        body,
    }
}

#[cfg(test)]
#[path = "../tests/physics/plugin.rs"]
mod tests;
