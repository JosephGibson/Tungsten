//! The top-down mover: a character on a kinematic body that moves in any
//! direction and slides along walls.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, PrevPosition, RigidBody, SpatialFilter, Velocity};
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, SHMUP, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{optional_positive, positive};
use crate::intent::action_input::Intent;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::platformer::kinematic_body;
use crate::motion::slide::{move_and_slide, step_dt};

/// [`TopDownMoverPlugin`]'s system, in the move phase.
pub const TOP_DOWN_MOVER: &str = "top_down_mover";

/// The scene component the plugin registers.
const TOP_DOWN_MOVER_COMPONENT: &str = "top_down_mover";

/// A top-down character: it moves at its speed in a direction, slides along
/// walls and stops at bodies. Its direction is the entity's [`Intent`]'s
/// movement when it has one (as [`ActionInputPlugin`](crate::ActionInputPlugin)
/// writes it), else what the game sets through
/// [`set_direction`](Self::set_direction), held until changed.
/// [`TopDownMoverPlugin`]'s system moves it once a fixed step, before
/// `physics_step`.
///
/// On a kinematic body (a `Position`, a `Velocity`, a `Collider` and a
/// kinematic `RigidBody`, as `RigidBodyBundle::kinematic` spawns) it casts
/// against the last step's positions and writes its `Velocity`, which the
/// body moves by in the step. Its first step moves nothing, so the physics
/// step after it indexes the walls it was spawned among. Its
/// [`SpatialFilter`] says what blocks it, by default bodies and tiles, its
/// own body excluded; it never pushes a dynamic body, and one that presses
/// into it sends a `CollisionEvent` whose `a` is that body, which a game
/// reads as contact damage.
///
/// Without a `RigidBody` it is bodiless: it moves its `Position` by its
/// direction, speed and the step, with no cast, so walls do not stop it,
/// and writes a `Velocity` when it has one; the physics sync draws it as a
/// body. Any other body is skipped, with one warning.
///
/// By default it reaches its speed at once and stops at once. With an
/// [acceleration](Self::with_acceleration) or a
/// [deceleration](Self::with_deceleration) it keeps a velocity instead
/// (`D-185`): each step the target is its direction times its speed; with
/// a direction the velocity moves toward the target by the acceleration
/// times the step, with none toward 0 by the deceleration times the step,
/// and a rate left unset jumps, as without either. On a kinematic body the
/// kept velocity is the one the slide moved by, so a wall absorbs it.
///
/// Scene name `top_down_mover`: `{ "speed": 240 }`, with optional
/// `"acceleration"` and `"deceleration"` in px/s², each above 0, which
/// spawns the mover with a `Position`, a `PrevPosition` and a `Velocity` at
/// the spawn position, so a prefab without a `rigid_body` moves bodiless
/// and one with a kinematic `rigid_body` keeps the body's.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, PhysicsPlugin, Position, RigidBodyBundle};
/// use tungsten_core::{Bundle, Plugin, Schedule, Stage, Time, World, system};
/// use tungsten_kit::{TOP_DOWN_MOVER, TopDownMover, TopDownMoverPlugin};
///
/// /// The game's input, which a game reads from its action map.
/// struct Stick(Vec2);
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(Stick(Vec2::new(1.0, 1.0)));
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// TopDownMoverPlugin.build(&mut schedule, &mut world);
/// // The game's input runs in `FixedUpdate`, before the mover.
/// schedule.add(
///     Stage::FixedUpdate,
///     system("stick_input", |world: &mut World| {
///         let stick = world.resource::<Stick>().0;
///         for mover in world.query_mut::<&mut TopDownMover>() {
///             mover.set_direction(stick);
///         }
///     })
///     .before(TOP_DOWN_MOVER),
/// );
/// schedule.resolve().unwrap();
///
/// // A wall whose left face is at x 100, and a ranger beside it heading
/// // down and right.
/// world.spawn_with(RigidBodyBundle::r#static(
///     Position::new(110.0, 0.0),
///     Collider::aabb(Vec2::new(10.0, 500.0)),
/// ));
/// let ranger = world.spawn_with(
///     RigidBodyBundle::kinematic(Position::new(90.0, 0.0), Collider::aabb(Vec2::splat(10.0)))
///         .with((TopDownMover::new(120.0),)),
/// );
/// for _ in 0..30 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
///
/// // The wall holds it; it slides down along the wall at its diagonal
/// // speed's vertical part.
/// let at = world.get::<Position>(ranger).unwrap().0;
/// assert!(at.x <= 90.0);
/// let down = 120.0 * std::f32::consts::FRAC_1_SQRT_2 * 29.0 / 60.0;
/// assert!((at.y - down).abs() < 0.01);
/// ```
#[derive(Debug, Clone)]
pub struct TopDownMover {
    speed: f32,
    direction: Vec2,
    filter: SpatialFilter,
    /// Its first step has run.
    started: bool,
    /// px/s² toward the target while it has a direction; `None` jumps.
    acceleration: Option<f32>,
    /// px/s² toward rest while it has none; `None` stops at once.
    deceleration: Option<f32>,
    /// The velocity it moved by at its last step, which the rates move.
    velocity: Vec2,
}

impl TopDownMover {
    /// A mover at up to `speed` px/s, standing still.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed > 0.0,
            "TopDownMover::new: speed must be finite and > 0 (got {speed})"
        );
        Self {
            speed,
            direction: Vec2::ZERO,
            filter: SpatialFilter::new(),
            started: false,
            acceleration: None,
            deceleration: None,
            velocity: Vec2::ZERO,
        }
    }

    /// The mover reaching its speed at `rate` px/s² instead of at once,
    /// turning and slowing toward a lower speed at it too, while it has a
    /// direction (`D-185`).
    ///
    /// # Panics
    ///
    /// When `rate` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_acceleration(mut self, rate: f32) -> Self {
        assert!(
            rate.is_finite() && rate > 0.0,
            "TopDownMover::with_acceleration: rate must be finite and > 0 (got {rate})"
        );
        self.acceleration = Some(rate);
        self
    }

    /// The mover slowing to a stop at `rate` px/s² instead of at once, once
    /// it has no direction (`D-185`).
    ///
    /// # Panics
    ///
    /// When `rate` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_deceleration(mut self, rate: f32) -> Self {
        assert!(
            rate.is_finite() && rate > 0.0,
            "TopDownMover::with_deceleration: rate must be finite and > 0 (got {rate})"
        );
        self.deceleration = Some(rate);
        self
    }

    /// The mover blocked by what `filter` sees; its own body is always
    /// excluded, in place of the filter's exclusion.
    #[must_use]
    pub fn with_filter(mut self, filter: SpatialFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Sets the direction it moves in, held until changed: its length is
    /// the fraction of its speed, so a direction longer than 1 is
    /// normalized, and one that is not finite stops it.
    pub fn set_direction(&mut self, direction: Vec2) {
        self.direction = if !direction.is_finite() {
            Vec2::ZERO
        } else if direction.length_squared() > 1.0 {
            direction.normalize()
        } else {
            direction
        };
    }

    /// The direction it moves in: what the game set, or, on an entity with
    /// an [`Intent`], the intent's movement at its last step.
    #[must_use]
    pub fn direction(&self) -> Vec2 {
        self.direction
    }

    /// Sets the speed, 0 standing it still: what a `move_speed` stat
    /// writes. One that is not finite and at least 0 is ignored.
    pub(crate) fn set_speed(&mut self, speed: f32) {
        if speed.is_finite() && speed >= 0.0 {
            self.speed = speed;
        }
    }
}

/// Registers the `top_down_mover` scene component and [`TOP_DOWN_MOVER`] in
/// the move phase, after [`PATH_FOLLOW`] when it is registered. Its system
/// moves every [`TopDownMover`] by the step, `Time::delta` bounded by
/// `PhysicsConfig::max_step_dt` as the physics step takes it; a step of 0
/// moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct TopDownMoverPlugin;

impl Plugin for TopDownMoverPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<TopDownMover>(world, "TopDownMover");
        describe(
            world,
            ComponentDescription::new(
                TOP_DOWN_MOVER_COMPONENT,
                "A top-down character: it moves at its speed in its intent's direction, sliding along walls on a kinematic body, or freely without one.",
            )
            .field(FieldDescription::new(
                "speed",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels per second at full tilt",
            ))
            .field(
                FieldDescription::new(
                    "acceleration",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second squared toward full tilt while moving; null reaches it at once",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "deceleration",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second squared toward rest once stopped; null stops at once",
                )
                .default("null"),
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "speed": 240 }"#)
            .roundtrip(round_trip::<TopDownMoverDef>),
        );
        world.register_component_with(
            TOP_DOWN_MOVER_COMPONENT,
            |def: TopDownMoverDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                let mut mover = TopDownMover::new(def.speed);
                mover.acceleration = def.acceleration;
                mover.deceleration = def.deceleration;
                (
                    mover,
                    Position(spawn),
                    PrevPosition(spawn),
                    Velocity(Vec2::ZERO),
                )
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(TOP_DOWN_MOVER, move |world: &mut World| {
                    runner.run(world);
                }))
                .after_if_present(PATH_FOLLOW),
        );
    }
}

/// `components.top_down_mover` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TopDownMoverDef {
    #[serde(deserialize_with = "positive")]
    speed: f32,
    #[serde(
        default,
        deserialize_with = "optional_positive",
        skip_serializing_if = "Option::is_none"
    )]
    acceleration: Option<f32>,
    #[serde(
        default,
        deserialize_with = "optional_positive",
        skip_serializing_if = "Option::is_none"
    )]
    deceleration: Option<f32>,
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's movers, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    /// The entities whose body was warned about.
    warned: HashSet<Entity>,
}

impl Runner {
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &TopDownMover)>()
                .map(|(entity, _)| entity),
        );
        let Self { entities, warned } = self;
        for &entity in entities.iter() {
            let (own_direction, started, filter, mover) = {
                let mover = world.get::<TopDownMover>(entity).unwrap();
                (
                    mover.direction,
                    mover.started,
                    mover.filter.excluding(entity),
                    (
                        mover.speed,
                        mover.acceleration,
                        mover.deceleration,
                        mover.velocity,
                    ),
                )
            };
            let direction = world
                .get::<Intent>(entity)
                .map_or(own_direction, Intent::movement);
            let wanted = step_velocity(mover, direction, dt);
            let velocity = if world.has::<RigidBody>(entity) {
                let Some((position, collider)) = kinematic_body(world, entity) else {
                    warn(warned, entity);
                    continue;
                };
                let velocity = if started {
                    let slide = move_and_slide(
                        world,
                        collider.shape,
                        position + collider.offset,
                        wanted * dt,
                        &filter,
                    );
                    slide.moved / dt
                } else {
                    Vec2::ZERO
                };
                world.get_mut::<Velocity>(entity).unwrap().0 = velocity;
                velocity
            } else {
                // Bodiless: the position moves, with no cast.
                let Some(position) = world.get_mut::<Position>(entity) else {
                    warn(warned, entity);
                    continue;
                };
                position.0 += wanted * dt;
                if let Some(body_velocity) = world.get_mut::<Velocity>(entity) {
                    body_velocity.0 = wanted;
                }
                wanted
            };
            let mover = world.get_mut::<TopDownMover>(entity).unwrap();
            mover.direction = direction;
            mover.velocity = velocity;
            mover.started = true;
        }
    }
}

/// The velocity a mover of (speed, acceleration, deceleration, kept
/// velocity) moves by this step: the target, direction times speed, or the
/// kept velocity moved toward it at the rate that applies (`D-185`).
fn step_velocity(
    (speed, acceleration, deceleration, kept): (f32, Option<f32>, Option<f32>, Vec2),
    direction: Vec2,
    dt: f32,
) -> Vec2 {
    let target = direction * speed;
    let rate = if direction == Vec2::ZERO {
        deceleration
    } else {
        acceleration
    };
    let Some(rate) = rate else {
        return target;
    };
    let gap = target - kept;
    let reach = rate * dt;
    if gap.length_squared() <= reach * reach {
        target
    } else {
        kept + gap.normalize() * reach
    }
}

/// Warns once per entity that no path moves.
fn warn(warned: &mut HashSet<Entity>, entity: Entity) {
    if warned.insert(entity) {
        log::warn!(
            "TopDownMover on {entity:?}: needs a kinematic body with a Position, a Velocity and \
             a Collider, or a Position and no body; skipped"
        );
    }
}

impl Inspectable for TopDownMover {
    /// `speed` and `direction`, then `acceleration`, `deceleration` and
    /// `velocity` only when a rate is set, so a mover without one keeps
    /// the rows (and the kit hash) it had before `D-185`.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![
            ("speed", format!("{:.1}", self.speed)),
            ("direction", point(self.direction)),
        ];
        if self.acceleration.is_some() || self.deceleration.is_some() {
            let rate =
                |rate: Option<f32>| rate.map_or_else(|| "none".to_string(), |r| format!("{r:.1}"));
            rows.push(("acceleration", rate(self.acceleration)));
            rows.push(("deceleration", rate(self.deceleration)));
            rows.push(("velocity", point(self.velocity)));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/top_down.rs"]
mod tests;
