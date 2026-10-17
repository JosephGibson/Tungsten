//! Despawn outside: an entity, with its children, despawned once it leaves
//! a world-space box or the camera's view.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{
    CameraState, CommandBuffer, Entity, Inspectable, Plugin, Schedule, SpawnContext, Time,
    Transform, WindowSize, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::trace::{TraceKind, trace};
use crate::transform::hierarchy::despawn_recursive;

/// [`DespawnOutsidePlugin`]'s system, in the overlap phase.
pub const DESPAWN_OUTSIDE: &str = "despawn_outside";

/// The scene component the plugin registers.
const DESPAWN_OUTSIDE_COMPONENT: &str = "despawn_outside";

/// Despawns its entity, with its children, once it leaves a world-space
/// box or the camera's view: a bolt that left the arena, debris that fell
/// off the screen. [`DespawnOutsidePlugin`]'s system tests the entity's
/// `Position`, or its `Transform.position` when it has none, once a fixed
/// step after `physics_step`, against the box or the view the last frame's
/// camera saw, and queues [`despawn_recursive`] for the frame's command
/// flush. It tests a point, so a margin stands for the entity's extent.
///
/// Scene name `despawn_outside`: `{ "region": [[0, 0], [640, 360]] }` or
/// `{ "view": 16 }`, the margin.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{CommandBuffer, Plugin, Schedule, Stage, Transform, World};
/// use tungsten_kit::{DespawnOutside, DespawnOutsidePlugin};
///
/// let mut world = World::new();
/// world.insert_resource(CommandBuffer::new());
/// let mut schedule = Schedule::new();
/// DespawnOutsidePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let arena = DespawnOutside::region(Vec2::ZERO, Vec2::new(640.0, 360.0));
/// let inside = world.spawn_with((Transform::from_position(Vec2::new(320.0, 0.0)), arena));
/// let escaped = world.spawn_with((Transform::from_position(Vec2::new(700.0, 10.0)), arena));
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
/// world.flush_reusing(&mut commands);
/// assert!(world.is_alive(inside) && !world.is_alive(escaped));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DespawnOutside {
    bounds: Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bounds {
    Region { min: Vec2, max: Vec2 },
    View { margin: f32 },
}

impl DespawnOutside {
    /// Despawns the entity when its position leaves the closed box
    /// `min..=max` in world units.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite, or `min` lies above `max` on an
    /// axis.
    #[must_use]
    #[track_caller]
    pub fn region(min: Vec2, max: Vec2) -> Self {
        assert!(
            min.is_finite() && max.is_finite(),
            "DespawnOutside::region: {min:?}..={max:?} is not finite"
        );
        assert!(
            min.x <= max.x && min.y <= max.y,
            "DespawnOutside::region: {min:?} lies above {max:?} on an axis"
        );
        Self {
            bounds: Bounds::Region { min, max },
        }
    }

    /// The box, when the rule is a region.
    pub(crate) fn region_bounds(&self) -> Option<(Vec2, Vec2)> {
        match self.bounds {
            Bounds::Region { min, max } => Some((min, max)),
            Bounds::View { .. } => None,
        }
    }

    /// Despawns the entity when its position lies more than `margin` world
    /// units outside the camera's view: `CameraState::visible_world_aabb` at
    /// the `WindowSize`. Without either resource the entity stays.
    ///
    /// # Panics
    ///
    /// When `margin` is negative or not finite.
    #[must_use]
    #[track_caller]
    pub fn view(margin: f32) -> Self {
        assert!(
            margin.is_finite() && margin >= 0.0,
            "DespawnOutside::view: the margin {margin} is negative or not finite"
        );
        Self {
            bounds: Bounds::View { margin },
        }
    }
}

/// Registers the `despawn_outside` scene component and [`DESPAWN_OUTSIDE`]
/// in the overlap phase: each fixed step it queues the despawn of every
/// [`DespawnOutside`] entity, with its children, whose point has left its
/// box or the view. Without a `CommandBuffer` it queues nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct DespawnOutsidePlugin;

impl Plugin for DespawnOutsidePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<DespawnOutside>(world, "DespawnOutside");
        describe(
            world,
            ComponentDescription::new(
                DESPAWN_OUTSIDE_COMPONENT,
                "Despawns the entity, with its children, once it leaves a world-space box or the camera's view; exactly one of `region` and `view`.",
            )
            .field(FieldDescription::new(
                "region",
                FieldType::Object("[[min_x, min_y], [max_x, max_y]]"),
                "the closed box, in world pixels",
            ))
            .field(FieldDescription::new(
                "view",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "the margin outside the last frame's camera view",
            ))
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample(r#"{ "view": 16 }"#)
            .roundtrip(round_trip::<DespawnOutsideDef>),
        );
        world.register_component_with(
            DESPAWN_OUTSIDE_COMPONENT,
            |def: DespawnOutsideDef, _: &SpawnContext<'_>| (DespawnOutside::from(def),),
        );
        let mut traced = Traced::default();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(DESPAWN_OUTSIDE, move |world: &mut World| {
                despawn_outside(world, &mut traced);
            })),
        );
    }
}

/// `components.despawn_outside` as a scene writes it: exactly one of
/// `region` and `view`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DespawnOutsideFields {
    region: Option<[[f32; 2]; 2]>,
    view: Option<f32>,
}

/// `components.despawn_outside`, checked: the one of the two it names.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "DespawnOutsideFields")]
struct DespawnOutsideDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<[[f32; 2]; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<f32>,
}

impl TryFrom<DespawnOutsideFields> for DespawnOutsideDef {
    type Error = String;

    fn try_from(fields: DespawnOutsideFields) -> Result<Self, Self::Error> {
        match (fields.region, fields.view) {
            (Some([min, max]), None) => {
                let (min, max) = (Vec2::from(min), Vec2::from(max));
                if !(min.is_finite() && max.is_finite()) {
                    return Err(format!("region: {min:?}..={max:?} is not finite"));
                }
                if min.x > max.x || min.y > max.y {
                    return Err(format!("region: {min:?} lies above {max:?} on an axis"));
                }
            }
            (None, Some(margin)) => {
                if !(margin.is_finite() && margin >= 0.0) {
                    return Err(format!(
                        "view: the margin {margin} is negative or not finite"
                    ));
                }
            }
            _ => return Err("exactly one of `region` and `view`".to_string()),
        }
        Ok(Self {
            region: fields.region,
            view: fields.view,
        })
    }
}

impl From<DespawnOutsideDef> for DespawnOutside {
    fn from(def: DespawnOutsideDef) -> Self {
        match (def.region, def.view) {
            (Some([min, max]), _) => DespawnOutside::region(Vec2::from(min), Vec2::from(max)),
            (None, Some(margin)) => DespawnOutside::view(margin),
            (None, None) => unreachable!("checked at deserialization"),
        }
    }
}

fn outside(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point.x < min.x || point.x > max.x || point.y < min.y || point.y > max.y
}

/// The entities the trace already has a despawn for this frame: the system
/// queues a despawn each step until the flush, and traces it once.
#[derive(Default)]
struct Traced {
    frame: u64,
    entities: Vec<Entity>,
}

fn despawn_outside(world: &mut World, traced: &mut Traced) {
    let view = match (
        world.get_resource::<CameraState>(),
        world.get_resource::<WindowSize>(),
    ) {
        (Some(camera), Some(size)) => {
            Some(camera.visible_world_aabb(size.width as f32, size.height as f32))
        }
        _ => None,
    };
    let mut gone: Vec<Entity> = Vec::new();
    for (entity, rule, position, transform) in
        world.query::<(Entity, &DespawnOutside, Option<&Position>, &Transform)>()
    {
        let point = position.map_or(transform.position, |position| position.0);
        let left = match rule.bounds {
            Bounds::Region { min, max } => outside(point, min, max),
            Bounds::View { margin } => view.is_some_and(|(min, max)| {
                outside(point, min - Vec2::splat(margin), max + Vec2::splat(margin))
            }),
        };
        if left {
            gone.push(entity);
        }
    }
    if gone.is_empty() {
        return;
    }
    let frame = world.get_resource::<Time>().map_or(0, Time::frame);
    if traced.frame != frame {
        traced.frame = frame;
        traced.entities.clear();
    }
    for &entity in &gone {
        if !traced.entities.contains(&entity) {
            traced.entities.push(entity);
            trace(world, entity, || TraceKind::Despawn {
                by: DESPAWN_OUTSIDE,
            });
        }
    }
    let Some(commands) = world.get_resource_mut::<CommandBuffer>() else {
        return;
    };
    for entity in gone {
        despawn_recursive(commands, entity);
    }
}

impl Inspectable for DespawnOutside {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        match self.bounds {
            Bounds::Region { min, max } => {
                vec![("region", format!("{}..{}", point(min), point(max)))]
            }
            Bounds::View { margin } => vec![("view", format!("margin {margin}"))],
        }
    }
}

#[cfg(test)]
#[path = "../tests/despawn.rs"]
mod tests;
