//! The top-down mover: a character on a kinematic body that moves in any
//! direction and slides along walls.

use std::collections::HashSet;

use glam::Vec2;
use tungsten_core::physics::{PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, SpatialFilter, Velocity};
use tungsten_core::{Entity, Plugin, Schedule, Stage, World, system};

use crate::path::PATH_FOLLOW;
use crate::platformer::kinematic_body;
use crate::slide::{move_and_slide, step_dt};

/// [`TopDownMoverPlugin`]'s system, in `FixedUpdate`.
pub const TOP_DOWN_MOVER: &str = "top_down_mover";

/// A top-down character: it moves at its speed in the direction the game
/// sets, held until changed, slides along walls and stops at bodies.
/// [`TopDownMoverPlugin`]'s system moves it once a fixed step, before
/// `physics_step`, against the last step's positions, and writes its
/// `Velocity`, which its body moves by in the step.
///
/// The entity needs a `Position`, a `Velocity`, a `Collider` and a
/// kinematic `RigidBody`, as `RigidBodyBundle::kinematic` spawns; any other
/// is skipped, with one warning. Its first step moves nothing, so the
/// physics step after it indexes the walls it was spawned among. Its
/// [`SpatialFilter`] says what blocks it, by default bodies and tiles, its
/// own body excluded; it never pushes a dynamic body, and one that presses
/// into it sends a `CollisionEvent` whose `a` is that body, which a game
/// reads as contact damage.
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
        }
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

    /// The direction it moves in.
    #[must_use]
    pub fn direction(&self) -> Vec2 {
        self.direction
    }
}

/// Registers [`TOP_DOWN_MOVER`] in `FixedUpdate`: after the history
/// snapshot and [`PATH_FOLLOW`], and before `physics_step`, each when
/// registered. Its system moves every [`TopDownMover`] by the step,
/// `Time::delta` bounded by `PhysicsConfig::max_step_dt` as the physics
/// step takes it; a step of 0 moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct TopDownMoverPlugin;

impl Plugin for TopDownMoverPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        let mut runner = Runner::default();
        schedule.add(
            Stage::FixedUpdate,
            system(TOP_DOWN_MOVER, move |world: &mut World| runner.run(world))
                .after_if_present(PHYSICS_PREV_SNAPSHOT)
                .after_if_present(PATH_FOLLOW)
                .before_if_present(PHYSICS_STEP),
        );
    }
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
        for &entity in &self.entities {
            let Some((position, collider)) = kinematic_body(world, entity) else {
                if self.warned.insert(entity) {
                    log::warn!(
                        "TopDownMover on {entity:?}: needs a kinematic body with a Position, a \
                         Velocity and a Collider; skipped"
                    );
                }
                continue;
            };
            let mover = world.get::<TopDownMover>(entity).unwrap();
            let velocity = if mover.started {
                let filter = mover.filter.excluding(entity);
                let slide = move_and_slide(
                    world,
                    collider.shape,
                    position + collider.offset,
                    mover.direction * mover.speed * dt,
                    &filter,
                );
                slide.moved / dt
            } else {
                Vec2::ZERO
            };
            world.get_mut::<TopDownMover>(entity).unwrap().started = true;
            world.get_mut::<Velocity>(entity).unwrap().0 = velocity;
        }
    }
}

#[cfg(test)]
#[path = "tests/top_down.rs"]
mod tests;
