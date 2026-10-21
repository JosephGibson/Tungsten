//! The platformer controller: a side-view character on a kinematic body,
//! with a ground check, coyote time, a jump buffer, variable jump height and
//! air jumps.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Collider, Position, SpatialFilter, Velocity};
use tungsten_core::{
    Entity, Inspectable, PhysicsConfig, Plugin, Schedule, SpawnContext, World, system,
};

use crate::count_down;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHMUP, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{
    fraction, non_negative, optional_non_negative, optional_vec2, positive, vec2,
};
use crate::intent::action_input::Intent;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::slide::{SKIN, is_kinematic, move_and_slide, probe_ground, step_dt};

/// [`PlatformerControllerPlugin`]'s system, in the move phase.
pub const PLATFORMER_CONTROLLER: &str = "platformer_controller";

/// The scene component the plugin registers.
const PLATFORMER_CONTROLLER_COMPONENT: &str = "platformer_controller";

/// The slot of an [`Intent`] the controller reads as its jump.
const JUMP_SLOT: &str = "jump";

/// The default coyote time and jump buffer, in seconds.
const DEFAULT_GRACE: f32 = 0.1;

/// The default jump cut.
const DEFAULT_JUMP_CUT: f32 = 0.5;

/// A side-view character: it runs, jumps and falls on a kinematic body,
/// stops at walls, lands on floors and rides the kinematic body it stands
/// on. Its input is the entity's [`Intent`] when it has one (the
/// movement's `x` as the run, the `jump` slot as the jump, as
/// [`ActionInputPlugin`](crate::ActionInputPlugin) writes them), else what
/// the game sets through [`set_run`](Self::set_run),
/// [`press_jump`](Self::press_jump) and
/// [`set_jump_held`](Self::set_jump_held), which holds until changed;
/// [`PlatformerControllerPlugin`]'s system moves it once a fixed step,
/// before `physics_step`.
///
/// The entity needs a `Position`, a `Velocity`, a `Collider` and a
/// kinematic `RigidBody`, as `RigidBodyBundle::kinematic` spawns; any other
/// is skipped, with one warning. Each step the system:
///
/// 1. Moves nothing on the controller's first step, so the physics step
///    after it indexes the floor and walls it was spawned among: queries
///    find nothing spawned since the last step.
/// 2. Checks the ground unless it is rising: a floor within a pixel below
///    it grounds it and snaps it down onto the surface.
/// 3. Applies a pending [`launch`](Self::launch), which ungrounds it.
/// 4. Unless the input is locked, runs at `run · run_speed` and jumps on a
///    buffered press while grounded or within coyote time; past them, a
///    press with [air jumps](Self::with_air_jumps) left jumps again, one a
///    press, and being grounded refills them. While locked, presses are
///    dropped and the horizontal speed holds.
/// 5. Adds the [`push`](Self::push)es made since its last step.
/// 6. Cuts a rising jump by the jump cut once, when the jump is released.
/// 7. Applies gravity, `with_gravity`'s or the world's
///    `PhysicsConfig::gravity.y`, over the step's mean velocity, so a jump
///    follows the exact parabola at each step and peaks at
///    `jump_speed² / 2g`. A zero-gravity world needs `with_gravity`.
/// 8. Moves and slides against the last step's positions: a floor lands
///    it, a ceiling stops its rise and a wall its horizontal speed.
/// 9. On a kinematic ground, adds the ground's `Velocity` times the step,
///    uncast, unless it jumped, was launched or was pushed up.
/// 10. Writes `Velocity`, which its body moves by in the step, and ticks
///     the input lock, the jump buffer and coyote time on the game clock.
///
/// Its [`SpatialFilter`] says what blocks it, by default bodies and tiles,
/// its own body excluded. Dynamic bodies block it and hold it up as walls
/// and floors do; it never pushes one. Two controllers do not see each
/// other's motion within a step, a body spawned since the last step is not
/// seen until the next, and nothing pushes a kinematic body out of an
/// overlap, though a move may leave one.
///
/// It is data: a save holds its settings and its runtime state, the air
/// jumps left among them, but not its filter, which loads as
/// [`SpatialFilter::new`], or its first-step flag. A loaded controller
/// therefore takes its still first step, as a fresh one does, while the
/// physics index fills, and a game that set its own filter sets it again.
///
/// Scene name `platformer_controller`, on a prefab with a kinematic
/// `rigid_body`:
/// `{ "run_speed": 200, "jump_speed": 600, "gravity": 2000, "coyote_time": 0.1, "jump_buffer": 0.1, "jump_cut": 0.5, "air_jumps": 1 }`,
/// `gravity` optional (the world's by default) and the last four with
/// the defaults 0.1, 0.1, 0.5 and 0.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, PhysicsPlugin, Position, RigidBodyBundle};
/// use tungsten_core::{Bundle, PhysicsConfig, Plugin, Schedule, Stage, Time, World, system};
/// use tungsten_kit::{PLATFORMER_CONTROLLER, PlatformerController, PlatformerControllerPlugin};
///
/// /// The game's input, which a game reads from its action map.
/// #[derive(Clone, Copy, Default)]
/// struct Pad {
///     run: f32,
///     jump_pressed: bool,
///     jump_held: bool,
/// }
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(PhysicsConfig {
///     gravity: Vec2::new(0.0, 2000.0),
///     ..PhysicsConfig::default()
/// });
/// world.insert_resource(Pad::default());
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// PlatformerControllerPlugin.build(&mut schedule, &mut world);
/// // The game's input runs in `FixedUpdate`, before the controller.
/// schedule.add(
///     Stage::FixedUpdate,
///     system("pad_input", |world: &mut World| {
///         let pad = *world.resource::<Pad>();
///         for controller in world.query_mut::<&mut PlatformerController>() {
///             controller.set_run(pad.run);
///             controller.set_jump_held(pad.jump_held);
///             if pad.jump_pressed {
///                 controller.press_jump();
///             }
///         }
///         world.resource_mut::<Pad>().jump_pressed = false;
///     })
///     .before(PLATFORMER_CONTROLLER),
/// );
/// schedule.resolve().unwrap();
///
/// // A floor whose top is at y 100, and a hero standing on it.
/// world.spawn_with(RigidBodyBundle::r#static(
///     Position::new(0.0, 110.0),
///     Collider::aabb(Vec2::new(500.0, 10.0)),
/// ));
/// let hero = world.spawn_with(
///     RigidBodyBundle::kinematic(Position::new(0.0, 84.0), Collider::aabb(Vec2::new(8.0, 16.0)))
///         .with((PlatformerController::new(200.0, 600.0),)),
/// );
/// let mut frames = |world: &mut World, count: usize| {
///     for _ in 0..count {
///         world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///         schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
///     }
/// };
///
/// // Half a second running right: it stands still for its first step.
/// world.resource_mut::<Pad>().run = 1.0;
/// frames(&mut world, 30);
/// let at = world.get::<Position>(hero).unwrap().0;
/// assert!((at.x - 200.0 * 29.0 / 60.0).abs() < 0.01 && at.y == 84.0);
/// assert!(world.get::<PlatformerController>(hero).unwrap().grounded());
///
/// // A held jump rises toward its 90 px peak, then lands again.
/// *world.resource_mut::<Pad>() = Pad { run: 0.0, jump_pressed: true, jump_held: true };
/// frames(&mut world, 10);
/// assert!(world.get::<Position>(hero).unwrap().0.y < 20.0);
/// frames(&mut world, 60);
/// assert!(world.get::<PlatformerController>(hero).unwrap().grounded());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ControllerSave", into = "ControllerSave")]
#[allow(clippy::struct_excessive_bools)] // Input and step flags read at different points of a step; no two are a state machine.
pub struct PlatformerController {
    run_speed: f32,
    jump_speed: f32,
    gravity: Option<f32>,
    coyote_time: f32,
    jump_buffer: f32,
    jump_cut: f32,
    air_jumps: u32,
    /// Not saved: a load takes `new`'s.
    filter: SpatialFilter,
    /// The run input, in `[-1, 1]`.
    run: f32,
    jump_held: bool,
    /// A press not yet seen by a step.
    pressed: bool,
    launch: Option<Vec2>,
    push: Vec2,
    buffer_left: f32,
    coyote_left: f32,
    lock_left: f32,
    /// Its own velocity at the end of its last step.
    velocity: Vec2,
    grounded: bool,
    jumped: bool,
    /// The jump its last step started was an air jump.
    air_jumped: bool,
    /// The jump cut may still act on the current jump.
    cut_armed: bool,
    air_jumps_left: u32,
    /// Its first step has run. Not saved: a loaded controller takes its
    /// still first step.
    started: bool,
}

impl PlatformerController {
    /// A controller that runs at up to `run_speed` px/s and jumps at
    /// `jump_speed` px/s, with the world's gravity, 0.1 s of coyote time,
    /// a 0.1 s jump buffer and a jump cut of 0.5.
    ///
    /// # Panics
    ///
    /// When either speed is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(run_speed: f32, jump_speed: f32) -> Self {
        assert!(
            run_speed.is_finite() && run_speed > 0.0,
            "PlatformerController::new: run_speed must be finite and > 0 (got {run_speed})"
        );
        assert!(
            jump_speed.is_finite() && jump_speed > 0.0,
            "PlatformerController::new: jump_speed must be finite and > 0 (got {jump_speed})"
        );
        Self {
            run_speed,
            jump_speed,
            gravity: None,
            coyote_time: DEFAULT_GRACE,
            jump_buffer: DEFAULT_GRACE,
            jump_cut: DEFAULT_JUMP_CUT,
            air_jumps: 0,
            filter: SpatialFilter::new(),
            run: 0.0,
            jump_held: false,
            pressed: false,
            launch: None,
            push: Vec2::ZERO,
            buffer_left: 0.0,
            coyote_left: 0.0,
            lock_left: 0.0,
            velocity: Vec2::ZERO,
            grounded: false,
            jumped: false,
            air_jumped: false,
            cut_armed: false,
            air_jumps_left: 0,
            started: false,
        }
    }

    /// The controller with its own gravity, in px/s² with y pointing down,
    /// in place of the world's `PhysicsConfig::gravity.y`.
    ///
    /// # Panics
    ///
    /// When `gravity` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_gravity(mut self, gravity: f32) -> Self {
        assert!(
            gravity.is_finite() && gravity >= 0.0,
            "PlatformerController::with_gravity: gravity must be finite and >= 0 (got {gravity})"
        );
        self.gravity = Some(gravity);
        self
    }

    /// The controller with `seconds` of coyote time: a jump still starts
    /// that long after it walked off an edge.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_coyote_time(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "PlatformerController::with_coyote_time: seconds must be finite and >= 0 (got {seconds})"
        );
        self.coyote_time = seconds;
        self
    }

    /// The controller with a jump buffer of `seconds`: a press made that
    /// long before it can jump still jumps.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_jump_buffer(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "PlatformerController::with_jump_buffer: seconds must be finite and >= 0 (got {seconds})"
        );
        self.jump_buffer = seconds;
        self
    }

    /// The controller with jump cut `factor`: the rising speed a jump
    /// keeps when it is released early, once per jump; 1 keeps every jump
    /// full.
    ///
    /// # Panics
    ///
    /// When `factor` is outside `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn with_jump_cut(mut self, factor: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&factor),
            "PlatformerController::with_jump_cut: factor must be in [0, 1] (got {factor})"
        );
        self.jump_cut = factor;
        self
    }

    /// The controller with `jumps` air jumps: past the ground and coyote
    /// time, each press with one left jumps at `jump_speed` again, and
    /// being grounded refills them. 0, the default, has none.
    #[must_use]
    pub fn with_air_jumps(mut self, jumps: u32) -> Self {
        self.air_jumps = jumps;
        self.air_jumps_left = jumps;
        self
    }

    /// The controller blocked by what `filter` sees; its own body is always
    /// excluded, in place of the filter's exclusion.
    #[must_use]
    pub fn with_filter(mut self, filter: SpatialFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Sets the run speed, 0 standing it still: what a `move_speed` stat
    /// writes. One that is not finite and at least 0 is ignored.
    pub(crate) fn set_run_speed(&mut self, speed: f32) {
        if speed.is_finite() && speed >= 0.0 {
            self.run_speed = speed;
        }
    }

    /// Sets the run input, held until changed: -1 runs left at full speed,
    /// 1 right. It is clamped to `[-1, 1]`, and NaN counts as 0.
    pub fn set_run(&mut self, axis: f32) {
        self.run = if axis.is_nan() {
            0.0
        } else {
            axis.clamp(-1.0, 1.0)
        };
    }

    /// Presses jump: the next step jumps when it can, and the jump buffer
    /// keeps the press until then. It holds the jump as
    /// [`set_jump_held`](Self::set_jump_held)`(true)` does, so a game that
    /// never releases it jumps at full height.
    pub fn press_jump(&mut self) {
        self.pressed = true;
        self.buffer_left = self.jump_buffer;
        self.jump_held = true;
    }

    /// Sets whether jump is held, until changed: releasing it while a jump
    /// rises cuts the jump.
    pub fn set_jump_held(&mut self, held: bool) {
        self.jump_held = held;
    }

    /// Replaces its velocity at its next step and ungrounds it: an air
    /// jump, a knockback or a spring. The last call before the step wins.
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite.
    #[track_caller]
    pub fn launch(&mut self, velocity: Vec2) {
        assert!(
            velocity.is_finite(),
            "PlatformerController::launch: velocity must be finite (got {velocity})"
        );
        self.launch = Some(velocity);
    }

    /// Adds `velocity` to its velocity at its next step, after the input:
    /// a blast's kick or a pull. The pushes made before a step sum.
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite.
    #[track_caller]
    pub fn push(&mut self, velocity: Vec2) {
        assert!(
            velocity.is_finite(),
            "PlatformerController::push: velocity must be finite (got {velocity})"
        );
        self.push += velocity;
    }

    /// Locks the input for `seconds` of game time, or for the lock left
    /// when that is longer: the run input and presses do nothing meanwhile.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[track_caller]
    pub fn lock_input(&mut self, seconds: f32) {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "PlatformerController::lock_input: seconds must be finite and >= 0 (got {seconds})"
        );
        self.lock_left = self.lock_left.max(seconds);
    }

    /// It ended its last step on the ground.
    #[must_use]
    pub fn grounded(&self) -> bool {
        self.grounded
    }

    /// Its own velocity, which its next step starts from: what a speed
    /// limit caps and a clamp stops on an axis.
    pub(crate) fn velocity_mut(&mut self) -> &mut Vec2 {
        &mut self.velocity
    }

    /// A press would jump at its next step: it is grounded or within coyote
    /// time, and its input is not locked.
    #[must_use]
    pub fn can_jump(&self) -> bool {
        (self.grounded || self.coyote_left > 0.0) && self.lock_left <= 0.0
    }

    /// Its last step started a jump, from the ground or in the air.
    #[must_use]
    pub fn jumped(&self) -> bool {
        self.jumped
    }

    /// The air jumps it has left until it is grounded again.
    #[must_use]
    pub fn air_jumps_left(&self) -> u32 {
        self.air_jumps_left
    }

    /// Its last step started an air jump, which [`jumped`](Self::jumped)
    /// also reports.
    #[must_use]
    pub fn air_jumped(&self) -> bool {
        self.air_jumped
    }

    /// One step of `dt` for the body at `position` with `collider`: the
    /// velocity its body moves by in this step.
    fn step(
        &mut self,
        world: &World,
        entity: Entity,
        position: Vec2,
        collider: Collider,
        world_gravity: f32,
        dt: f32,
    ) -> Vec2 {
        self.jumped = false;
        self.air_jumped = false;
        if !self.started {
            self.started = true;
            return Vec2::ZERO;
        }
        let filter = self.filter.excluding(entity);
        let origin = position + collider.offset;
        let mut at = origin;

        // The ground check.
        self.grounded = false;
        let mut ground = None;
        if self.velocity.y >= 0.0
            && let Some(hit) = probe_ground(world, collider.shape, origin, &filter)
        {
            self.grounded = true;
            self.air_jumps_left = self.air_jumps;
            at.y += (hit.distance - SKIN).max(0.0);
            ground = hit.entity;
        }

        // A launch, then the input.
        let mut lifted = false;
        if let Some(velocity) = self.launch.take() {
            self.velocity = velocity;
            self.grounded = false;
            self.coyote_left = 0.0;
            self.cut_armed = false;
            lifted = true;
        }
        if self.lock_left > 0.0 {
            self.buffer_left = 0.0;
        } else {
            self.velocity.x = self.run * self.run_speed;
            let pressed = self.pressed || self.buffer_left > 0.0;
            if pressed && (self.grounded || self.coyote_left > 0.0) {
                self.velocity.y = -self.jump_speed;
                self.buffer_left = 0.0;
                self.coyote_left = 0.0;
                self.grounded = false;
                self.jumped = true;
                self.cut_armed = true;
                lifted = true;
            } else if pressed && self.air_jumps_left > 0 {
                self.air_jumps_left -= 1;
                self.velocity.y = -self.jump_speed;
                self.buffer_left = 0.0;
                self.jumped = true;
                self.air_jumped = true;
                self.cut_armed = true;
                lifted = true;
            }
        }
        self.pressed = false;

        // Pushes, the jump cut and gravity.
        if self.push != Vec2::ZERO {
            self.velocity += self.push;
            if self.push.y < 0.0 {
                lifted = true;
            }
            self.push = Vec2::ZERO;
        }
        if self.velocity.y < 0.0 {
            self.grounded = false;
        }
        if self.cut_armed {
            if self.velocity.y >= 0.0 {
                self.cut_armed = false;
            } else if !self.jump_held {
                self.velocity.y *= self.jump_cut;
                self.cut_armed = false;
            }
        }
        let gravity = self.gravity.unwrap_or(world_gravity);
        let mean_vy = if self.grounded {
            self.velocity.y = 0.0;
            0.0
        } else {
            let mean = self.velocity.y + gravity * dt * 0.5;
            self.velocity.y += gravity * dt;
            mean
        };

        // Move and slide, then the ground's carry.
        let slide = move_and_slide(
            world,
            collider.shape,
            at,
            Vec2::new(self.velocity.x, mean_vy) * dt,
            &filter,
        );
        at += slide.moved;
        if slide.floor && self.velocity.y >= 0.0 {
            self.grounded = true;
            self.air_jumps_left = self.air_jumps;
            self.velocity.y = 0.0;
            ground = slide.floor_body;
        }
        if slide.ceiling && self.velocity.y < 0.0 {
            self.velocity.y = 0.0;
        }
        if slide.wall {
            self.velocity.x = 0.0;
        }
        if self.grounded
            && !lifted
            && let Some(ground) = ground
            && is_kinematic(world, ground)
            && let Some(carry) = world.get::<Velocity>(ground)
        {
            at += carry.0 * dt;
        }

        // The timers, last, so the getters read the step's end.
        self.lock_left = count_down(self.lock_left, dt);
        self.buffer_left = count_down(self.buffer_left, dt);
        self.coyote_left = if self.grounded {
            self.coyote_time
        } else {
            count_down(self.coyote_left, dt)
        };
        (at - origin) / dt
    }
}

/// Registers the `platformer_controller` scene component and
/// [`PLATFORMER_CONTROLLER`] in the move phase, after [`PATH_FOLLOW`] when
/// it is registered. Its system moves every [`PlatformerController`] by the
/// step, `Time::delta` bounded by `PhysicsConfig::max_step_dt` as the
/// physics step takes it; a step of 0 moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformerControllerPlugin;

impl Plugin for PlatformerControllerPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<PlatformerController>(world, "PlatformerController");
        describe(
            world,
            ComponentDescription::new(
                PLATFORMER_CONTROLLER_COMPONENT,
                "A side-view character on a kinematic body: it runs, jumps and falls, stops at walls, lands on floors and rides moving platforms.",
            )
            .field(FieldDescription::new(
                "run_speed",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels per second at full run",
            ))
            .field(FieldDescription::new(
                "jump_speed",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the jump's upward speed; it peaks at jump_speed² / 2g",
            ))
            .field(
                FieldDescription::new(
                    "gravity",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "its own gravity, y down; the world's when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "coyote_time",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds after leaving an edge a jump still starts",
                )
                .default("0.1"),
            )
            .field(
                FieldDescription::new(
                    "jump_buffer",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds a press before it can jump is kept",
                )
                .default("0.1"),
            )
            .field(
                FieldDescription::new(
                    "jump_cut",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Inclusive(1.0),
                    },
                    "the rising speed kept when the jump is released early",
                )
                .default("0.5"),
            )
            .field(
                FieldDescription::new(
                    "air_jumps",
                    FieldType::U32 {
                        min: None,
                        max: None,
                    },
                    "jumps a press starts in the air, past coyote time; being grounded refills them",
                )
                .default("0"),
            )
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample(r#"{ "run_speed": 200, "jump_speed": 600 }"#)
            .roundtrip(round_trip::<PlatformerControllerDef>),
        );
        world.register_component_with(
            PLATFORMER_CONTROLLER_COMPONENT,
            |def: PlatformerControllerDef, _: &SpawnContext<'_>| {
                let controller = PlatformerController::new(def.run_speed, def.jump_speed)
                    .with_coyote_time(def.coyote_time)
                    .with_jump_buffer(def.jump_buffer)
                    .with_jump_cut(def.jump_cut)
                    .with_air_jumps(def.air_jumps);
                (def.gravity.map_or_else(
                    || controller.clone(),
                    |gravity| controller.clone().with_gravity(gravity),
                ),)
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(PLATFORMER_CONTROLLER, move |world: &mut World| {
                    runner.run(world);
                }))
                .after_if_present(PATH_FOLLOW),
        );
    }
}

/// `components.platformer_controller` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformerControllerDef {
    #[serde(deserialize_with = "positive")]
    run_speed: f32,
    #[serde(deserialize_with = "positive")]
    jump_speed: f32,
    #[serde(default, deserialize_with = "optional_non_negative")]
    gravity: Option<f32>,
    #[serde(default = "default_grace", deserialize_with = "non_negative")]
    coyote_time: f32,
    #[serde(default = "default_grace", deserialize_with = "non_negative")]
    jump_buffer: f32,
    #[serde(default = "default_jump_cut", deserialize_with = "fraction")]
    jump_cut: f32,
    #[serde(default)]
    air_jumps: u32,
}

/// A controller as a save writes it: its settings and its runtime state,
/// without the filter or the first-step flag.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // The controller's own flags, saved as they are.
struct ControllerSave {
    run_speed: f32,
    jump_speed: f32,
    gravity: Option<f32>,
    coyote_time: f32,
    jump_buffer: f32,
    jump_cut: f32,
    air_jumps: u32,
    run: f32,
    jump_held: bool,
    pressed: bool,
    #[serde(with = "optional_vec2")]
    launch: Option<Vec2>,
    #[serde(with = "vec2")]
    push: Vec2,
    buffer_left: f32,
    coyote_left: f32,
    lock_left: f32,
    #[serde(with = "vec2")]
    velocity: Vec2,
    grounded: bool,
    jumped: bool,
    air_jumped: bool,
    cut_armed: bool,
    air_jumps_left: u32,
}

impl TryFrom<ControllerSave> for PlatformerController {
    type Error = String;

    fn try_from(save: ControllerSave) -> Result<Self, Self::Error> {
        let at_least_0 = |v: f32| v.is_finite() && v >= 0.0;
        for (field, value) in [
            ("run_speed", save.run_speed),
            ("coyote_time", save.coyote_time),
            ("jump_buffer", save.jump_buffer),
            ("buffer_left", save.buffer_left),
            ("coyote_left", save.coyote_left),
            ("lock_left", save.lock_left),
        ] {
            if !at_least_0(value) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if !(save.jump_speed.is_finite() && save.jump_speed > 0.0) {
            return Err(format!(
                "jump_speed: must be finite and > 0 (got {})",
                save.jump_speed
            ));
        }
        if let Some(gravity) = save.gravity
            && !at_least_0(gravity)
        {
            return Err(format!("gravity: must be finite and >= 0 (got {gravity})"));
        }
        for (field, value, range) in [
            ("jump_cut", save.jump_cut, 0.0..=1.0),
            ("run", save.run, -1.0..=1.0),
        ] {
            if !range.contains(&value) {
                return Err(format!("{field}: must be in {range:?} (got {value})"));
            }
        }
        for (field, value) in [
            ("launch", save.launch.unwrap_or(Vec2::ZERO)),
            ("push", save.push),
            ("velocity", save.velocity),
        ] {
            if !value.is_finite() {
                return Err(format!("{field}: must be finite (got {value})"));
            }
        }
        if save.air_jumps_left > save.air_jumps {
            return Err(format!(
                "air_jumps_left: must be at most air_jumps, {} (got {})",
                save.air_jumps, save.air_jumps_left
            ));
        }
        Ok(Self {
            run_speed: save.run_speed,
            jump_speed: save.jump_speed,
            gravity: save.gravity,
            coyote_time: save.coyote_time,
            jump_buffer: save.jump_buffer,
            jump_cut: save.jump_cut,
            air_jumps: save.air_jumps,
            filter: SpatialFilter::new(),
            run: save.run,
            jump_held: save.jump_held,
            pressed: save.pressed,
            launch: save.launch,
            push: save.push,
            buffer_left: save.buffer_left,
            coyote_left: save.coyote_left,
            lock_left: save.lock_left,
            velocity: save.velocity,
            grounded: save.grounded,
            jumped: save.jumped,
            air_jumped: save.air_jumped,
            cut_armed: save.cut_armed,
            air_jumps_left: save.air_jumps_left,
            started: false,
        })
    }
}

impl From<PlatformerController> for ControllerSave {
    fn from(controller: PlatformerController) -> Self {
        Self {
            run_speed: controller.run_speed,
            jump_speed: controller.jump_speed,
            gravity: controller.gravity,
            coyote_time: controller.coyote_time,
            jump_buffer: controller.jump_buffer,
            jump_cut: controller.jump_cut,
            air_jumps: controller.air_jumps,
            run: controller.run,
            jump_held: controller.jump_held,
            pressed: controller.pressed,
            launch: controller.launch,
            push: controller.push,
            buffer_left: controller.buffer_left,
            coyote_left: controller.coyote_left,
            lock_left: controller.lock_left,
            velocity: controller.velocity,
            grounded: controller.grounded,
            jumped: controller.jumped,
            air_jumped: controller.air_jumped,
            cut_armed: controller.cut_armed,
            air_jumps_left: controller.air_jumps_left,
        }
    }
}

fn default_grace() -> f32 {
    DEFAULT_GRACE
}

fn default_jump_cut() -> f32 {
    DEFAULT_JUMP_CUT
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's controllers, kept so a step allocates nothing once warm.
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
        let gravity = world
            .get_resource::<PhysicsConfig>()
            .map_or(0.0, |config| config.gravity.y);
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &PlatformerController)>()
                .map(|(entity, _)| entity),
        );
        for &entity in &self.entities {
            let Some((position, collider)) = kinematic_body(world, entity) else {
                if self.warned.insert(entity) {
                    log::warn!(
                        "PlatformerController on {entity:?}: needs a kinematic body with a \
                         Position, a Velocity and a Collider; skipped"
                    );
                }
                continue;
            };
            let mut controller = world.get::<PlatformerController>(entity).unwrap().clone();
            if let Some(intent) = world.get::<Intent>(entity) {
                controller.set_run(intent.movement().x);
                controller.set_jump_held(intent.held(JUMP_SLOT));
                if intent.pressed(JUMP_SLOT) {
                    controller.press_jump();
                }
            }
            let velocity = controller.step(world, entity, position, collider, gravity, dt);
            *world.get_mut::<PlatformerController>(entity).unwrap() = controller;
            world.get_mut::<Velocity>(entity).unwrap().0 = velocity;
        }
    }
}

/// The position and collider of `entity` when it is a kinematic body with
/// a `Velocity`.
pub(crate) fn kinematic_body(world: &World, entity: Entity) -> Option<(Vec2, Collider)> {
    if !is_kinematic(world, entity) || !world.has::<Velocity>(entity) {
        return None;
    }
    Some((
        world.get::<Position>(entity)?.0,
        *world.get::<Collider>(entity)?,
    ))
}

impl Inspectable for PlatformerController {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("run", format!("{:.2} of {:.1}", self.run, self.run_speed)),
            (
                "jump",
                format!("{:.1}, held {}", self.jump_speed, self.jump_held),
            ),
            ("velocity", point(self.velocity)),
            ("grounded", self.grounded.to_string()),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/platformer.rs"]
mod tests;
