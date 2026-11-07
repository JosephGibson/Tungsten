//! Despawn outside: an entity, with its children, despawned once it leaves
//! a world-space box, the camera's view or the world's bounds, or killed
//! there.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Transform, World, system,
};

use crate::damage::health::{Health, kill};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHOOTER, TOP_DOWN,
    describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::lifecycle::ended::{end, ensure_kit_despawn};
use crate::motion::area::unshaken_view;

/// [`DespawnOutsidePlugin`]'s system, in the overlap phase.
pub const DESPAWN_OUTSIDE: &str = "despawn_outside";

/// The scene component the plugin registers.
const DESPAWN_OUTSIDE_COMPONENT: &str = "despawn_outside";

/// Despawns its entity, with its children, once it leaves a world-space
/// box or the camera's view: a bolt that left the arena, debris that fell
/// off the screen. [`DespawnOutsidePlugin`]'s system tests the entity's
/// `Position`, or its `Transform.position` when it has none, once a fixed
/// step after `physics_step` and the kit's hits, against the box or the
/// view the last frame's camera saw, and lists it for the kit's end-of-step
/// despawn ([`KIT_DESPAWN`](crate::KIT_DESPAWN)), which queues
/// [`despawn_recursive`](crate::despawn_recursive) after the step's spawns
/// from it for the frame's command flush. It tests a point, so a margin
/// stands for the entity's extent. A `world` rule tests against the
/// World's [`WorldBounds`] grown by its margin (redo 7.10g); a killing
/// rule ([`killing`](Self::killing)) kills an entity with health there
/// instead, once, and despawns one without.
///
/// Scene name `despawn_outside`: `{ "region": [[0, 0], [640, 360]] }`,
/// `{ "view": 16 }` or `{ "world": 32 }`, the margin, and `"kill": true`
/// beside any of them.
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
    /// Past its bounds, an entity with health is killed instead.
    kill: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bounds {
    Region {
        min: Vec2,
        max: Vec2,
    },
    View {
        margin: f32,
    },
    /// The [`WorldBounds`] grown by the margin.
    World {
        margin: f32,
    },
}

/// The world's box (redo 7.10g), a World resource a game or a level sets:
/// what a [`DespawnOutside::world`] rule leaves, its margin outside it, as
/// a fall past a level's bottom. Without it such a rule does nothing, as a
/// `view` rule without a camera.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::WorldBounds;
///
/// let level = WorldBounds::new(Vec2::ZERO, Vec2::new(3200.0, 720.0));
/// assert_eq!(level.max(), Vec2::new(3200.0, 720.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBounds {
    min: Vec2,
    max: Vec2,
}

impl WorldBounds {
    /// The closed box `min..=max` in world units.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite, or `min` lies above `max` on an
    /// axis.
    #[must_use]
    #[track_caller]
    pub fn new(min: Vec2, max: Vec2) -> Self {
        assert!(
            min.is_finite() && max.is_finite() && min.x <= max.x && min.y <= max.y,
            "WorldBounds::new: {min:?}..={max:?} is not a finite box"
        );
        Self { min, max }
    }

    /// Its least corner.
    #[must_use]
    pub fn min(&self) -> Vec2 {
        self.min
    }

    /// Its greatest corner.
    #[must_use]
    pub fn max(&self) -> Vec2 {
        self.max
    }
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
            kill: false,
        }
    }

    /// The box, when the rule is a region.
    pub(crate) fn region_bounds(&self) -> Option<(Vec2, Vec2)> {
        match self.bounds {
            Bounds::Region { min, max } => Some((min, max)),
            Bounds::View { .. } | Bounds::World { .. } => None,
        }
    }

    /// Despawns the entity when its position lies more than `margin` world
    /// units outside the camera's view without its shake
    /// (`CameraState::unshaken`'s visible box at the `WindowSize`), so a
    /// shake despawns nothing. Without either resource the entity stays.
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
            kill: false,
        }
    }

    /// Despawns the entity when its position lies more than `margin` world
    /// units outside the World's [`WorldBounds`]. Without the resource the
    /// entity stays.
    ///
    /// # Panics
    ///
    /// When `margin` is negative or not finite.
    #[must_use]
    #[track_caller]
    pub fn world(margin: f32) -> Self {
        assert!(
            margin.is_finite() && margin >= 0.0,
            "DespawnOutside::world: the margin {margin} is negative or not finite"
        );
        Self {
            bounds: Bounds::World { margin },
            kill: false,
        }
    }

    /// The rule, killing an entity with [`Health`](crate::Health) past its
    /// bounds instead of despawning it ([`kill`](crate::kill)), once, its
    /// death policy taking it from there; one without health is despawned.
    #[must_use]
    pub fn killing(mut self) -> Self {
        self.kill = true;
        self
    }

    /// Whether it kills an entity with health past its bounds.
    #[must_use]
    pub fn kills(&self) -> bool {
        self.kill
    }
}

/// Registers the `despawn_outside` scene component, [`DESPAWN_OUTSIDE`] in
/// the overlap phase and the kit's end-of-step despawn when the schedule
/// lacks it: each fixed step the system lists every [`DespawnOutside`]
/// entity whose point has left its box or the view, and the despawn queues
/// it, with its children. Without a `CommandBuffer` nothing is queued.
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
                "Despawns the entity, with its children, once it leaves a world-space box, the camera's view or the world's bounds, or kills it there; exactly one of `region`, `view` and `world`.",
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
            .field(FieldDescription::new(
                "world",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "the margin outside the `WorldBounds` resource's box, which a game or a level sets; nothing without it",
            ))
            .field(
                FieldDescription::new(
                    "kill",
                    FieldType::Bool,
                    "past its bounds an entity with health is killed instead, once, its death policy taking it from there; one without health is despawned",
                )
                .default("false"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHOOTER)
            .sample(r#"{ "view": 16 }"#)
            .roundtrip(round_trip::<DespawnOutsideDef>),
        );
        world.register_component_with(
            DESPAWN_OUTSIDE_COMPONENT,
            |def: DespawnOutsideDef, _: &SpawnContext<'_>| (DespawnOutside::from(def),),
        );
        let mut gone: Vec<Entity> = Vec::new();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(DESPAWN_OUTSIDE, move |world: &mut World| {
                despawn_outside(world, &mut gone);
            })),
        );
        ensure_kit_despawn(schedule, world);
    }
}

/// `components.despawn_outside` as a scene writes it: exactly one of
/// `region`, `view` and `world`, and `kill`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DespawnOutsideFields {
    region: Option<[[f32; 2]; 2]>,
    view: Option<f32>,
    world: Option<f32>,
    #[serde(default)]
    kill: bool,
}

/// `components.despawn_outside`, checked: the one of the three it names.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "DespawnOutsideFields")]
struct DespawnOutsideDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<[[f32; 2]; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world: Option<f32>,
    kill: bool,
}

impl TryFrom<DespawnOutsideFields> for DespawnOutsideDef {
    type Error = String;

    fn try_from(fields: DespawnOutsideFields) -> Result<Self, Self::Error> {
        match (fields.region, fields.view, fields.world) {
            (Some([min, max]), None, None) => {
                let (min, max) = (Vec2::from(min), Vec2::from(max));
                if !(min.is_finite() && max.is_finite()) {
                    return Err(format!("region: {min:?}..={max:?} is not finite"));
                }
                if min.x > max.x || min.y > max.y {
                    return Err(format!("region: {min:?} lies above {max:?} on an axis"));
                }
            }
            (None, Some(margin), None) | (None, None, Some(margin)) => {
                if !(margin.is_finite() && margin >= 0.0) {
                    let field = if fields.view.is_some() {
                        "view"
                    } else {
                        "world"
                    };
                    return Err(format!(
                        "{field}: the margin {margin} is negative or not finite"
                    ));
                }
            }
            _ => return Err("exactly one of `region`, `view` and `world`".to_string()),
        }
        Ok(Self {
            region: fields.region,
            view: fields.view,
            world: fields.world,
            kill: fields.kill,
        })
    }
}

impl From<DespawnOutsideDef> for DespawnOutside {
    fn from(def: DespawnOutsideDef) -> Self {
        let rule = match (def.region, def.view, def.world) {
            (Some([min, max]), ..) => DespawnOutside::region(Vec2::from(min), Vec2::from(max)),
            (None, Some(margin), _) => DespawnOutside::view(margin),
            (None, None, Some(margin)) => DespawnOutside::world(margin),
            (None, None, None) => unreachable!("checked at deserialization"),
        };
        if def.kill { rule.killing() } else { rule }
    }
}

fn outside(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point.x < min.x || point.x > max.x || point.y < min.y || point.y > max.y
}

/// Lists each entity outside its bounds for the kit's despawn, through the
/// scratch list `gone`.
fn despawn_outside(world: &mut World, gone: &mut Vec<Entity>) {
    let view = unshaken_view(world);
    let bounds = world.get_resource::<WorldBounds>().copied();
    gone.clear();
    for (entity, rule, position, transform) in
        world.query::<(Entity, &DespawnOutside, Option<&Position>, &Transform)>()
    {
        let point = position.map_or(transform.position, |position| position.0);
        let left = match rule.bounds {
            Bounds::Region { min, max } => outside(point, min, max),
            Bounds::View { margin } => view.is_some_and(|(min, max)| {
                outside(point, min - Vec2::splat(margin), max + Vec2::splat(margin))
            }),
            Bounds::World { margin } => bounds.is_some_and(|bounds| {
                outside(
                    point,
                    bounds.min - Vec2::splat(margin),
                    bounds.max + Vec2::splat(margin),
                )
            }),
        };
        if left {
            gone.push(entity);
        }
    }
    for &entity in gone.iter() {
        // A killing rule kills the living once; the dead are its death
        // policy's, and the rest are despawned.
        if world
            .get::<DespawnOutside>(entity)
            .is_some_and(DespawnOutside::kills)
            && let Some(health) = world.get::<Health>(entity)
        {
            if !health.is_dead() {
                kill(world, entity);
            }
            continue;
        }
        end(world, entity, DESPAWN_OUTSIDE);
    }
}

impl Inspectable for DespawnOutside {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        match self.bounds {
            Bounds::Region { min, max } => {
                vec![("region", format!("{}..{}", point(min), point(max)))]
            }
            Bounds::View { margin } => vec![("view", format!("margin {margin}"))],
            Bounds::World { margin } => vec![("world", format!("margin {margin}"))],
        }
    }
}

#[cfg(test)]
#[path = "../tests/despawn.rs"]
mod tests;
