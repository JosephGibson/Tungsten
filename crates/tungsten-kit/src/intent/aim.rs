//! Aim: the direction an entity aims in, from its intent, the cursor, its
//! movement or a target.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Velocity;
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{finite_vec2, optional_vec2, vec2};
use crate::intent::action_input::{ACTION_INPUT, Intent};
use crate::intent::cursor::cursor_to_world;
use crate::motion::drive::point_of;
use crate::motion::target::{Measure, Resolver, TARGET_SHAPE, Target, TargetDef, describe_target};

/// [`AimPlugin`]'s system, in the intent phase.
pub const AIM: &str = "aim";

/// The scene component the plugin registers.
const AIM_COMPONENT: &str = "aim";

/// Where an aim takes its direction from.
#[derive(Debug, Clone, PartialEq)]
enum AimFrom {
    Intent,
    Cursor,
    Movement,
    Target(Target),
}

/// The name of an aim's source, as data writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FromName {
    #[default]
    Intent,
    Cursor,
    Movement,
    Target,
}

/// The direction its entity aims in, a unit vector each fixed step: a
/// turret's barrel, a hero's crosshair, a sentry's watch. Its readers are
/// [`Facing`](crate::Facing), and later projectiles and abilities.
/// [`AimPlugin`]'s system takes it from one source:
///
/// - `intent`, the entity's [`Intent`](crate::Intent)'s aim, as an
///   [`ActionInput`](crate::ActionInput)'s aim actions or a game's AI write
///   it;
/// - `cursor`, toward the world point under the cursor
///   ([`cursor_to_world`](crate::cursor_to_world)) from the entity's point
///   plus its offset;
/// - `movement`, the intent's movement, else its velocity's direction;
/// - `target`, toward a [`Target`]'s point from its point plus its offset.
///
/// It keeps its last direction while the source gives none, so a released
/// stick keeps aiming and a sentry whose target walked out of range holds;
/// [`direction`](Self::direction) is `None` until the first.
///
/// Scene name `aim`:
/// `{ "from": "target", "target": { "nearest": { "all": ["team.player"], "range": 320 } }, "offset": [0, 0] }`,
/// `from` one of `intent` (default), `cursor`, `movement` and `target`,
/// `target` needed with `from: target` and unused otherwise, and `offset`
/// the aim's origin from the entity's point, `[0, 0]` by default. It is
/// data: a game's save can hold it, without the entity it aimed at, which
/// the next step resolves again.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::Position;
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Aim, AimPlugin, TagFilter, Tags, Target};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// AimPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A sentry that watches the nearest hero within 320 px, from its centre.
/// let hero_tag = TagFilter::new().all(["team.player"]);
/// let watch = Aim::at(Target::nearest(hero_tag).with_range(320.0)).with_offset(Vec2::new(16.0, 16.0));
/// let sentry = world.spawn_with((watch, Position::new(0.0, 0.0)));
/// let hero = world.spawn_with((Tags::new(["team.player"]), Position::new(216.0, 16.0)));
/// assert_eq!(world.get::<Aim>(sentry).unwrap().direction(), None, "none before its first step");
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// assert_eq!(world.get::<Aim>(sentry).unwrap().direction(), Some(Vec2::X));
/// assert_eq!(world.get::<Aim>(sentry).unwrap().target_entity(), Some(hero));
///
/// // Out of range, it keeps watching where the hero went.
/// world.get_mut::<Position>(hero).unwrap().0 = Vec2::new(16.0, 900.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// let aim = world.get::<Aim>(sentry).unwrap();
/// assert_eq!((aim.direction(), aim.target_entity()), (Some(Vec2::X), None));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AimFields", into = "AimFields")]
pub struct Aim {
    from: AimFrom,
    offset: Vec2,
    /// The unit direction at its last step that gave one.
    direction: Option<Vec2>,
    /// The entity it aimed at at its last step.
    target_entity: Option<Entity>,
    /// Its origin and its target's point at its last step, for the debug
    /// drawing.
    drawn: Option<(Vec2, Option<Vec2>)>,
}

/// An aim as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AimFields {
    #[serde(default)]
    from: FromName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<Target>,
    #[serde(default, with = "vec2")]
    offset: Vec2,
    #[serde(
        default,
        with = "optional_vec2",
        skip_serializing_if = "Option::is_none"
    )]
    direction: Option<Vec2>,
}

impl TryFrom<AimFields> for Aim {
    type Error = String;

    fn try_from(fields: AimFields) -> Result<Self, Self::Error> {
        let from = match (fields.from, fields.target) {
            (FromName::Intent, _) => AimFrom::Intent,
            (FromName::Cursor, _) => AimFrom::Cursor,
            (FromName::Movement, _) => AimFrom::Movement,
            (FromName::Target, Some(target)) => AimFrom::Target(target),
            (FromName::Target, None) => return Err("`from: target` needs a `target`".into()),
        };
        if !fields.offset.is_finite() {
            return Err(format!("offset: must be finite (got {})", fields.offset));
        }
        if let Some(direction) = fields.direction
            && !(direction.is_finite() && (direction.length() - 1.0).abs() <= 1e-3)
        {
            return Err(format!(
                "direction: must be a unit vector (got {direction})"
            ));
        }
        Ok(Self {
            from,
            offset: fields.offset,
            direction: fields.direction,
            target_entity: None,
            drawn: None,
        })
    }
}

impl From<Aim> for AimFields {
    fn from(aim: Aim) -> Self {
        let (from, target) = match aim.from {
            AimFrom::Intent => (FromName::Intent, None),
            AimFrom::Cursor => (FromName::Cursor, None),
            AimFrom::Movement => (FromName::Movement, None),
            AimFrom::Target(target) => (FromName::Target, Some(target)),
        };
        Self {
            from,
            target,
            offset: aim.offset,
            direction: aim.direction,
        }
    }
}

impl Aim {
    fn new(from: AimFrom) -> Self {
        Self {
            from,
            offset: Vec2::ZERO,
            direction: None,
            target_entity: None,
            drawn: None,
        }
    }

    /// An aim along the entity's [`Intent`](crate::Intent)'s aim.
    #[must_use]
    pub fn intent() -> Self {
        Self::new(AimFrom::Intent)
    }

    /// An aim toward the world point under the cursor.
    #[must_use]
    pub fn cursor() -> Self {
        Self::new(AimFrom::Cursor)
    }

    /// An aim along the entity's [`Intent`](crate::Intent)'s movement, else
    /// its velocity.
    #[must_use]
    pub fn movement() -> Self {
        Self::new(AimFrom::Movement)
    }

    /// An aim toward `target`'s point.
    #[must_use]
    pub fn at(target: Target) -> Self {
        Self::new(AimFrom::Target(target))
    }

    /// The aim, from `offset` world pixels from the entity's point, such as
    /// a sprite's centre or a barrel's tip.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "Aim::with_offset: offset must be finite (got {offset})"
        );
        self.offset = offset;
        self
    }

    /// The unit direction it aims in: its last step's, or the last one its
    /// source gave; `None` until the first.
    #[must_use]
    pub fn direction(&self) -> Option<Vec2> {
        self.direction
    }

    /// The entity it aimed at at its last step, with `from: target`:
    /// `None` before its first step, while a point is its target, or while
    /// no target resolves.
    #[must_use]
    pub fn target_entity(&self) -> Option<Entity> {
        self.target_entity
    }

    /// Its ray at its last step, for the debug drawing: its origin and its
    /// end, at its target's point or 32 world pixels along its direction.
    pub(crate) fn ray(&self) -> Option<(Vec2, Vec2)> {
        let (origin, at) = self.drawn?;
        let end = at.or_else(|| self.direction.map(|direction| origin + direction * 32.0))?;
        Some((origin, end))
    }
}

/// Registers the `aim` scene component and [`AIM`] in the intent phase,
/// after [`ACTION_INPUT`](crate::ACTION_INPUT) when it is registered, so it
/// reads the step's intent. Its system sets every [`Aim`]'s direction once
/// a fixed step.
#[derive(Debug, Default, Clone, Copy)]
pub struct AimPlugin;

impl Plugin for AimPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Aim>(world, "Aim");
        describe_target(world);
        describe(
            world,
            ComponentDescription::new(
                AIM_COMPONENT,
                "The direction the entity aims in, from its intent, the cursor, its movement or a target, kept while its source gives none.",
            )
            .field(
                FieldDescription::new(
                    "from",
                    FieldType::Enum(&["intent", "cursor", "movement", "target"]),
                    "the intent's aim, the cursor's world point, the movement or a target",
                )
                .default("\"intent\""),
            )
            .field(
                FieldDescription::new(
                    "target",
                    FieldType::Shape(TARGET_SHAPE),
                    "what it aims at with `from: target`, unused otherwise; the load checks its tags",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "the aim's origin from the entity's point, in world pixels",
                )
                .default("[0.0, 0.0]"),
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample(r#"{ "from": "target", "target": { "nearest": { "all": ["team.player"] } } }"#)
            .roundtrip(round_trip::<AimDef>),
        );
        world.register_component_with_assets(
            AIM_COMPONENT,
            |def: &AimDef, world: &World| {
                def.target
                    .as_ref()
                    .map_or(Ok(()), |target| target.check(world, "target"))
            },
            |def: AimDef, _: &SpawnContext<'_>| {
                let aim = match (def.from, def.target) {
                    (FromName::Intent, _) => Aim::intent(),
                    (FromName::Cursor, _) => Aim::cursor(),
                    (FromName::Movement, _) => Aim::movement(),
                    (FromName::Target, target) => {
                        Aim::at(target.expect("checked at deserialization").into())
                    }
                };
                (aim.with_offset(def.offset),)
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Intent.stage(),
            Phase::Intent
                .join(system(AIM, move |world: &mut World| runner.run(world)))
                .after_if_present(ACTION_INPUT),
        );
    }
}

/// `components.aim` as a scene writes it, before its target is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AimDefFields {
    #[serde(default)]
    from: FromName,
    #[serde(default)]
    target: Option<TargetDef>,
    #[serde(default, with = "finite_vec2")]
    offset: Vec2,
}

/// `components.aim`, checked: a target with `from: target`.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "AimDefFields")]
struct AimDef {
    from: FromName,
    target: Option<TargetDef>,
    #[serde(with = "vec2")]
    offset: Vec2,
}

impl TryFrom<AimDefFields> for AimDef {
    type Error = String;

    fn try_from(fields: AimDefFields) -> Result<Self, Self::Error> {
        if fields.from == FromName::Target && fields.target.is_none() {
            return Err("`from: target` needs a `target`".to_string());
        }
        Ok(Self {
            from: fields.from,
            target: fields.target,
            offset: fields.offset,
        })
    }
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's aims, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    resolver: Resolver,
}

impl Runner {
    /// Sets every aim's direction from its source, keeping the last one
    /// while the source gives none.
    fn run(&mut self, world: &mut World) {
        self.resolver.begin();
        self.entities.clear();
        self.entities
            .extend(world.query::<(Entity, &Aim)>().map(|(entity, _)| entity));
        let cursor = cursor_to_world(world);
        for &entity in &self.entities {
            let Some(point) = point_of(world, entity) else {
                continue;
            };
            let aim = world.get::<Aim>(entity).unwrap();
            let origin = point + aim.offset;
            let mut target = None;
            let direction = match &aim.from {
                AimFrom::Intent => world
                    .get::<Intent>(entity)
                    .and_then(Intent::aim)
                    .and_then(Vec2::try_normalize),
                AimFrom::Cursor => cursor.and_then(|at| (at - origin).try_normalize()),
                AimFrom::Movement => world
                    .get::<Intent>(entity)
                    .and_then(|intent| intent.movement().try_normalize())
                    .or_else(|| {
                        world
                            .get::<Velocity>(entity)
                            .and_then(|velocity| velocity.0.try_normalize())
                    }),
                AimFrom::Target(goal) => {
                    target = self
                        .resolver
                        .resolve(world, goal, entity, origin, Measure::Now);
                    target.and_then(|(_, at)| (at - origin).try_normalize())
                }
            };
            let aim = world.get_mut::<Aim>(entity).unwrap();
            if direction.is_some() {
                aim.direction = direction;
            }
            aim.target_entity = target.and_then(|(entity, _)| entity);
            aim.drawn = Some((origin, target.map(|(_, at)| at)));
        }
    }
}

impl Inspectable for Aim {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let source = match &self.from {
            AimFrom::Intent => "intent".to_string(),
            AimFrom::Cursor => "cursor".to_string(),
            AimFrom::Movement => "movement".to_string(),
            AimFrom::Target(target) => target.summary(),
        };
        let direction = self.direction.map_or_else(|| "none".to_string(), point);
        let target = self
            .target_entity
            .map_or_else(|| "none".to_string(), |entity| format!("{entity:?}"));
        vec![
            ("source", source),
            ("direction", direction),
            ("target", target),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/aim.rs"]
mod tests;
