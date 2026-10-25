//! The platformer controller: a side-view character on a kinematic body,
//! with a ground check, coyote time, a jump buffer, variable jump height and
//! air jumps.

use std::collections::HashSet;

use glam::Vec2;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::physics::{
    BodyKind, Collider, Position, RigidBody, Shape, SpatialFilter, Velocity, wake,
};
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
    fraction, non_negative, optional_non_negative, optional_positive, optional_vec2, positive, vec2,
};
use crate::intent::action_input::Intent;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::slide::{
    SKIN, Slide, climb, is_kinematic, move_and_slide, probe_ground, step_dt,
};

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

/// The default share of its mass that a body it shoves may weigh.
const DEFAULT_SHOVE_SHARE: f32 = 0.5;

/// The least a climb goes across, as a share of the step height. A cast's
/// contact turns floor-like 0.286 of the two shapes' summed radii past side
/// contact, which for a round step no taller than the height is 0.143 of it.
const CLIMB_REACH: f32 = 0.15;

/// What a round controller's climb adds across, as a share of its radius,
/// whose own rounding adds 0.286 of it to that contact.
const CLIMB_ROUND_REACH: f32 = 0.29;

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
/// 5. Adds the [`push`](Self::push)es made since its last step, velocity
///    given to it, where a [shove](Self::with_shove) moves what it walks
///    into.
/// 6. Cuts a rising jump by the jump cut once, when the jump is released.
/// 7. Applies gravity, `with_gravity`'s or the world's
///    `PhysicsConfig::gravity.y`, over the step's mean velocity, so a jump
///    follows the exact parabola at each step and peaks at
///    `jump_speed² / 2g`. A zero-gravity world needs `with_gravity`.
/// 8. Moves and slides against the last step's positions: a floor lands
///    it, a ceiling stops its rise and a wall its horizontal speed. A wall
///    met on the ground that its [step height](Self::with_step_height)
///    climbs is climbed instead, and a light dynamic body met as a wall
///    takes a [shove](Self::with_shove).
/// 9. On a kinematic ground, adds the ground's `Velocity` times the step,
///    uncast, unless it jumped, was launched or was pushed up.
/// 10. Writes `Velocity`, which its body moves by in the step, or after a
///     climb its `Position` at the landing and a zero `Velocity`; writes a
///     shoved body's `Velocity.x` and wakes it; and ticks the input lock,
///     the jump buffer and coyote time on the game clock.
///
/// Its [`SpatialFilter`] says what blocks it, by default bodies and tiles,
/// its own body excluded. Dynamic bodies block it and hold it up as walls
/// and floors do, unless it shoves or climbs them: a shoved body moves by
/// its own physics, a step behind, and one that cannot move holds it. Two
/// controllers do not see each other's motion within a step, a body spawned
/// since the last step is not seen until the next, and nothing pushes a
/// kinematic body out of an overlap, though a move may leave one.
///
/// It is data: a save holds its settings and its runtime state, the air
/// jumps left among them, but not its filter, which loads as
/// [`SpatialFilter::new`], or its first-step flag. A loaded controller
/// therefore takes its still first step, as a fresh one does, while the
/// physics index fills, and a game that set its own filter sets it again.
///
/// Scene name `platformer_controller`, on a prefab with a kinematic
/// `rigid_body`:
/// `{ "run_speed": 200, "jump_speed": 600, "gravity": 2000, "coyote_time": 0.1, "jump_buffer": 0.1, "jump_cut": 0.5, "air_jumps": 1, "shove_mass": 60, "shove_share": 0.5, "step_height": 16 }`,
/// `gravity` and `shove_mass` optional (the world's gravity and no shove by
/// default), and `coyote_time`, `jump_buffer`, `jump_cut`, `air_jumps`,
/// `shove_share` and `step_height` with the defaults 0.1, 0.1, 0.5, 0, 0.5
/// and 0; `shove_share` is read only with `shove_mass`.
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
    /// `None`, the default, shoves nothing.
    shove: Option<Shove>,
    /// 0, the default, climbs nothing.
    step_height: f32,
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
    /// The body its last step ended standing on. Not saved: a loaded
    /// controller reports none until a step after its still first one.
    ground: Option<Entity>,
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
            shove: None,
            step_height: 0.0,
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
            ground: None,
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

    /// The controller with a shove: of the dynamic bodies its horizontal
    /// move meets as walls, one of mass `m` up to `share` of `mass` (its
    /// own mass for shoves) takes the controller's horizontal velocity
    /// less `m / mass` of it, unless it already moves that way faster, and
    /// is woken. The controller stops at the body that step, as at a wall,
    /// then follows at the body's own pace, so it slows by `m / mass`; a
    /// body that cannot move holds it as before. A heavier body, a
    /// kinematic or static one and a tile block it as before.
    ///
    /// # Panics
    ///
    /// When `mass` is not finite and above 0, or `share` is not in (0, 1].
    #[must_use]
    #[track_caller]
    pub fn with_shove(mut self, mass: f32, share: f32) -> Self {
        assert!(
            mass.is_finite() && mass > 0.0,
            "PlatformerController::with_shove: mass must be finite and > 0 (got {mass})"
        );
        assert!(
            share > 0.0 && share <= 1.0,
            "PlatformerController::with_shove: share must be in (0, 1] (got {share})"
        );
        self.shove = Some(Shove { mass, share });
        self
    }

    /// The controller with a step height: on the ground, a horizontal move
    /// that meets a wall goes up to `height` px, across and back down in
    /// the same step when that lands it on a floor farther on, so it
    /// climbs a ledge or a body no taller than `height` with room above. A
    /// climb goes at least 0.15 of `height` across, and 0.29 of its radius
    /// more for a circle collider, so it reaches a round top's walkable
    /// part, and moves the body by its position rather than its velocity,
    /// so it shoves nothing it climbs. A kinematic body is never a step,
    /// and neither is a floor on one. 0, the default, climbs nothing.
    ///
    /// # Panics
    ///
    /// When `height` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_step_height(mut self, height: f32) -> Self {
        assert!(
            height.is_finite() && height >= 0.0,
            "PlatformerController::with_step_height: height must be finite and >= 0 (got {height})"
        );
        self.step_height = height;
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

    /// The body it ended its last step standing on, a moving deck, a slab
    /// or another body, as its ground probe or its landing met it; `None`
    /// on tiles, in the air, and after a load until a step past its still
    /// first one. A game reads it after the step, as it reads
    /// [`grounded`](Self::grounded): a lift that starts when stood on.
    #[must_use]
    pub fn ground(&self) -> Option<Entity> {
        self.ground
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

    /// One step of `dt` for the body at `position` with `collider`: how its
    /// body moves in this step, and the body it shoves.
    fn step(
        &mut self,
        world: &World,
        entity: Entity,
        position: Vec2,
        collider: Collider,
        world_gravity: f32,
        dt: f32,
    ) -> Moved {
        self.jumped = false;
        self.air_jumped = false;
        if !self.started {
            self.started = true;
            return Moved {
                velocity: Vec2::ZERO,
                position: None,
                shove: None,
            };
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

        // Move and slide, a climb or a shove at a wall, then the ground's
        // carry.
        let walking = self.grounded;
        let delta = Vec2::new(self.velocity.x, mean_vy) * dt;
        let slide = move_and_slide(world, collider.shape, at, delta, &filter);
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
        let mut climbed = false;
        let mut shoved = None;
        if slide.wall {
            let landing = if walking {
                self.climb(
                    world,
                    collider.shape,
                    at,
                    delta.x - slide.moved.x,
                    &slide,
                    &filter,
                )
            } else {
                None
            };
            if let Some((landing, floor)) = landing {
                at = landing;
                ground = floor;
                climbed = true;
            } else {
                shoved = self.shove_of(world, &slide);
                self.velocity.x = 0.0;
            }
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
        self.ground = if self.grounded { ground } else { None };
        self.lock_left = count_down(self.lock_left, dt);
        self.buffer_left = count_down(self.buffer_left, dt);
        self.coyote_left = if self.grounded {
            self.coyote_time
        } else {
            count_down(self.coyote_left, dt)
        };
        if climbed {
            Moved {
                velocity: Vec2::ZERO,
                position: Some(at - collider.offset),
                shove: None,
            }
        } else {
            Moved {
                velocity: (at - origin) / dt,
                position: None,
                shove: shoved,
            }
        }
    }

    /// Where a climb over the wall `slide` met lands its shape from `at`,
    /// the move's end, with `rest` px of its horizontal move left: with a
    /// step height, moving, at a wall that is not a kinematic body.
    fn climb(
        &self,
        world: &World,
        shape: Shape,
        at: Vec2,
        rest: f32,
        slide: &Slide,
        filter: &SpatialFilter,
    ) -> Option<(Vec2, Option<Entity>)> {
        if self.step_height <= 0.0
            || self.velocity.x == 0.0
            || slide
                .wall_body
                .is_some_and(|body| is_kinematic(world, body))
        {
            return None;
        }
        let round = match shape {
            Shape::Circle { radius } => CLIMB_ROUND_REACH * radius,
            Shape::Aabb { .. } => 0.0,
        };
        let across = rest.abs().max(CLIMB_REACH * self.step_height + round);
        climb(
            world,
            shape,
            at,
            self.step_height,
            across.copysign(self.velocity.x),
            filter,
        )
    }

    /// The body its move shoves and the horizontal speed it gives it: the
    /// first wall `slide` met, when that is a dynamic body light enough and
    /// the move went into it.
    fn shove_of(&self, world: &World, slide: &Slide) -> Option<(Entity, f32)> {
        let shove = self.shove?;
        let body = slide.wall_body?;
        if self.velocity.x * slide.wall_normal.x >= 0.0 || !world.has::<Velocity>(body) {
            return None;
        }
        let rigid = world.get::<RigidBody>(body)?;
        if rigid.kind != BodyKind::Dynamic || rigid.inv_mass <= 0.0 {
            return None;
        }
        let mass = rigid.inv_mass.recip();
        let speed = self.velocity.x * (1.0 - mass / shove.mass);
        (mass <= shove.share * shove.mass && speed != 0.0).then_some((body, speed))
    }
}

/// A controller's shove: the mass it shoves as, and the share of it a body
/// it moves may weigh.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Shove {
    mass: f32,
    share: f32,
}

/// What a controller's step writes.
struct Moved {
    /// The velocity its body moves by in the step.
    velocity: Vec2,
    /// Where a climb set its body, which then moves by a zero velocity.
    position: Option<Vec2>,
    /// The body it shoved and the horizontal speed the shove gives it.
    shove: Option<(Entity, f32)>,
}

/// Gives `body` a shove's horizontal `speed`, unless it already moves that
/// way faster, as another shover's may have, and wakes it.
fn shove(world: &mut World, body: Entity, speed: f32) {
    let Some(velocity) = world.get_mut::<Velocity>(body) else {
        return;
    };
    if velocity.0.x * speed.signum() < speed.abs() {
        velocity.0.x = speed;
    }
    wake(world, body);
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
            .field(
                FieldDescription::new(
                    "shove_mass",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "the mass it shoves as: a dynamic body it walks into of up to shove_share of it takes its run less the body's share of it; none when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "shove_share",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Inclusive(1.0),
                    },
                    "the share of shove_mass a body it shoves may weigh; read only with shove_mass",
                )
                .default("0.5"),
            )
            .field(
                FieldDescription::new(
                    "step_height",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels of ledge or body it climbs on the ground, with room above; 0 climbs nothing",
                )
                .default("0.0"),
            )
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample(r#"{ "run_speed": 200, "jump_speed": 600 }"#)
            .roundtrip(round_trip::<PlatformerControllerDef>),
        );
        world.register_component_with(
            PLATFORMER_CONTROLLER_COMPONENT,
            |def: PlatformerControllerDef, _: &SpawnContext<'_>| {
                let mut controller = PlatformerController::new(def.run_speed, def.jump_speed)
                    .with_coyote_time(def.coyote_time)
                    .with_jump_buffer(def.jump_buffer)
                    .with_jump_cut(def.jump_cut)
                    .with_air_jumps(def.air_jumps)
                    .with_step_height(def.step_height);
                if let Some(gravity) = def.gravity {
                    controller = controller.with_gravity(gravity);
                }
                if let Some(mass) = def.shove_mass {
                    controller = controller.with_shove(mass, def.shove_share);
                }
                (controller,)
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
    #[serde(default, deserialize_with = "optional_positive")]
    shove_mass: Option<f32>,
    #[serde(default = "default_shove_share", deserialize_with = "share")]
    shove_share: f32,
    #[serde(default, deserialize_with = "non_negative")]
    step_height: f32,
}

/// A controller as a save writes it: its settings and its runtime state,
/// without the filter or the first-step flag. The shove and the step height
/// are left out when off, so a save from before them loads with both off.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shove: Option<Shove>,
    #[serde(default, skip_serializing_if = "is_zero")]
    step_height: f32,
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
        if let Some(shove) = save.shove {
            if !(shove.mass.is_finite() && shove.mass > 0.0) {
                return Err(format!(
                    "shove.mass: must be finite and > 0 (got {})",
                    shove.mass
                ));
            }
            if !(shove.share > 0.0 && shove.share <= 1.0) {
                return Err(format!(
                    "shove.share: must be in (0, 1] (got {})",
                    shove.share
                ));
            }
        }
        if !at_least_0(save.step_height) {
            return Err(format!(
                "step_height: must be finite and >= 0 (got {})",
                save.step_height
            ));
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
            shove: save.shove,
            step_height: save.step_height,
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
            ground: None,
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
            shove: controller.shove,
            step_height: controller.step_height,
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

fn default_shove_share() -> f32 {
    DEFAULT_SHOVE_SHARE
}

/// A number in `(0, 1]`, for `shove_share`'s `deserialize_with`.
fn share<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if value > 0.0 && value <= 1.0 {
        Ok(value)
    } else {
        Err(D::Error::custom(format!("must be in (0, 1] (got {value})")))
    }
}

// The reference is serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &f32) -> bool {
    *value == 0.0
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
            let moved = controller.step(world, entity, position, collider, gravity, dt);
            *world.get_mut::<PlatformerController>(entity).unwrap() = controller;
            world.get_mut::<Velocity>(entity).unwrap().0 = moved.velocity;
            if let Some(at) = moved.position {
                world.get_mut::<Position>(entity).unwrap().0 = at;
            }
            if let Some((body, speed)) = moved.shove {
                shove(world, body, speed);
            }
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
