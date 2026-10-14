//! The physics plugin: the snapshot and the step at the fixed stage, the
//! one-way, interpolating sync to `Transform` after update, the collision
//! and sensor event queues and the `rigid_body` scene component. A plugin
//! whose every need is met by core, so it lives here (the kit's shape,
//! `D-007`).

use glam::Vec2;
use serde::Deserialize;

use crate::component_registry::SpawnContext;
use crate::components::Transform;
use crate::ecs::World;
use crate::physics::{
    Collider, CollisionEvent, Position, PrevPosition, RigidBody, RigidBodyBundle, SensorEntered,
    Shape, Velocity, physics_step,
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
/// ([`component_registry`](crate::component_registry)). The snapshot is
/// registered first, so it heads the stage ahead of a game's fixed systems,
/// which `DefaultPlugins` builds before. `PhysicsConfig` stays the app's
/// resource, so a game edits it before or after adding the plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.register_event::<CollisionEvent>();
        world.register_event::<SensorEntered>();
        world.register_component_with(RIGID_BODY, spawn_rigid_body);
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

/// Copies each body's `Position` into its [`PrevPosition`]: the history a
/// fixed step starts from. `PhysicsPlugin` runs it first in `fixed_update`
/// under [`PHYSICS_PREV_SNAPSHOT`], so a step's history spans whatever moves
/// the body in that step, gameplay's writes and the solve.
pub fn physics_prev_snapshot(world: &mut World) {
    world
        .query_mut_slices::<(&mut PrevPosition, &Position)>()
        .for_each(|(_, (prevs, positions))| {
            prevs
                .iter_mut()
                .zip(positions)
                .for_each(|(prev, position)| prev.0 = position.0);
        });
}

/// Draws each body: writes `Transform.position` as
/// `prev + (cur - prev) * alpha` ([`Time::alpha`]) for an entity with a
/// [`PrevPosition`] while [`Time::interpolate`] is on, and as its `Position`
/// otherwise and in a world without `Time`. Never writes `Position`
/// (`D-033`).
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

/// `components.rigid_body` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RigidBodyFields {
    kind: SceneBodyKind,
    shape: SceneShape,
    #[serde(default)]
    offset: [f32; 2],
    mass: Option<f32>,
    #[serde(default)]
    restitution: f32,
    velocity: Option<[f32; 2]>,
}

/// `components.rigid_body`, checked: the bundle's parts beside the entry's
/// position.
#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "RigidBodyFields")]
struct RigidBodyDef {
    velocity: Option<Velocity>,
    collider: Collider,
    body: RigidBody,
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
        if !(0.0..=1.0).contains(&fields.restitution) {
            return Err(format!(
                "`restitution` must be in 0..=1 (got {})",
                fields.restitution
            ));
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
        // `restitution` is accepted on every kind and does nothing on a
        // sensor, which has no contacts.
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
        Ok(Self {
            velocity,
            collider: Collider {
                shape,
                offset: Vec2::new(ox, oy),
            },
            body: body.with_restitution(fields.restitution),
        })
    }
}

/// `rigid_body`'s spawn: a [`RigidBodyBundle`] at the entry's position, so
/// `Position` and `PrevPosition` start there; the transform's scale does not
/// size the collider.
fn spawn_rigid_body(def: RigidBodyDef, at: &SpawnContext<'_>) -> RigidBodyBundle {
    RigidBodyBundle {
        position: Position(at.transform().position),
        velocity: def.velocity,
        collider: def.collider,
        body: def.body,
    }
}

#[cfg(test)]
#[path = "../tests/physics/plugin.rs"]
mod tests;
