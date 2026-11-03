//! The platformer controller: a side-view character on a kinematic body,
//! with a ground check, coyote time, a jump buffer, variable jump height and
//! air jumps.

use std::collections::HashSet;

use glam::Vec2;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::physics::{
    Aabb, BodyKind, Collider, Contact, Position, RigidBody, Shape, SpatialFilter, Velocity,
    aabb_vs_aabb, aabb_vs_circle, apply_impulse, cast_shape_where, circle_vs_circle, overlap,
    take_push, wake,
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
use crate::foundation::layers::{LayerRef, check_layers, layer_bits, layer_list};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::stats::Stats;
use crate::foundation::{
    fraction, non_negative, optional_non_negative, optional_positive, optional_vec2, positive, vec2,
};
use crate::intent::action_input::Intent;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::slide::{
    FLOOR_NORMAL_Y, GROUND_PROBE, SKIN, Slide, carry, climb, is_kinematic, move_and_slide_on,
    probe_ground_on, step_dt,
};
use crate::motion::top_down::TopDownMover;

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
/// 2. Is pushed out of each kinematic body its filter sees overlapping it
///    at the last step's positions, but its ground and a body a kit mover
///    drives (a platformer controller or a [`TopDownMover`]): along their
///    contact's normal by the overlap and the skin, cast against the rest
///    of the world, so a wall or a floor that cuts the push short leaves it
///    overlapped and [`crushed`](Self::crushed) for the step. It sees a
///    moving platform a step late, as every cast it makes does.
/// 3. Checks the ground unless it is rising: a floor within a pixel below
///    it grounds it and snaps it down onto the surface, and so does one
///    within `run_speed · dt · tan(slope limit)` when the controller ended
///    its last step grounded and the floor is a slope tile's face or it
///    stood on one, so it keeps to a slope and over its crest. A drop asked
///    for while it stands on a one-way edge starts there instead.
/// 4. Applies a pending [`launch`](Self::launch), which ungrounds it.
/// 5. Unless the input is locked, runs at `run · run_speed`, along a slope
///    tile's face when it stands on one, so it climbs and descends at that
///    speed, plus its slide down a face past its slide limit, and jumps on a
///    buffered press while grounded or within coyote time; past them, a
///    press with [air jumps](Self::with_air_jumps) left jumps again, one a
///    press, and being grounded refills them. With none left, a press jumps
///    again when the controller has an
///    [air-jump cost](Self::with_air_jump_cost) and the entity's
///    [`Stats`] base of its stat holds the amount, with no cap; below it
///    the jump buffer keeps the press. While locked, presses are dropped
///    and the horizontal speed holds.
/// 6. Adds the [`push`](Self::push)es made since its last step, velocity
///    given to it, where a [shove](Self::with_shove) moves what it walks
///    into; with a mass on its body (`RigidBody::with_mass`), the push the
///    bodies that hit it and the impulses it took gave since its last step,
///    which [`pushed`](Self::pushed) reads.
/// 7. Cuts a rising jump by the jump cut once, when the jump is released.
/// 8. Applies gravity, `with_gravity`'s or the world's
///    `PhysicsConfig::gravity.y`, over the step's mean velocity, so a jump
///    follows the exact parabola at each step and peaks at
///    `jump_speed² / 2g`. A zero-gravity world needs `with_gravity`.
/// 9. Moves and slides against the last step's positions: a floor lands
///    it, a ceiling stops its rise and a wall its horizontal speed; a slope
///    steeper than its slope limit is a wall it does not climb. A wall
///    met on the ground that its [step height](Self::with_step_height)
///    climbs is climbed instead, and a light dynamic body met as a wall
///    takes a [shove](Self::with_shove). Under locked input, with a mass
///    `M` on its body, a dynamic body of mass `m` it cannot shove takes the
///    inelastic impulse `M m / (M + m)` times the speed it closes at along
///    x, and the controller keeps the speed they share.
/// 10. On a kinematic ground, moves by the ground's `Velocity` times the
///     step, by a slide from where its own move ended that passes the
///     ground, so a wall or a ceiling stops the carry, unless it jumped, was
///     launched or was pushed up.
/// 11. Writes `Velocity`, which its body moves by in the step, or after a
///     climb its `Position` at the landing and a zero `Velocity`; writes a
///     shoved body's `Velocity.x` and wakes it; gives a ploughed body its
///     impulse; takes a paid air jump's
///     amount off its stat's base; and ticks the input lock, the jump
///     buffer and coyote time on the game clock.
///
/// Its [`SpatialFilter`] says what blocks it, by default bodies and tiles,
/// its own body excluded. [`drop_through`](Self::drop_through), or a jump
/// press with an [`Intent`]'s movement down, on a one-way edge drops it
/// through: its casts skip one-way tiles until its bottom is below the
/// edge's top face, or it lands on another floor or rises. Dynamic bodies block it and hold it up as walls
/// and floors do, unless it shoves or climbs them: a shoved body moves by
/// its own physics, a step behind, and one that cannot move holds it. Two
/// controllers do not see each other's motion within a step, a body spawned
/// since the last step is not seen until the next, and a moving platform
/// pushes it out of an overlap as step 2 says, while another controller, a
/// top-down mover and its ground do not, though a move may leave one.
///
/// It is data: a save holds its settings and its runtime state, the air
/// jumps left, the air-jump cost and a drop's edge among them, and of its
/// filter the layers it sees and skips, which load into
/// [`SpatialFilter::new`]'s, but not its first-step flag. A loaded
/// controller therefore takes its still first step, as a fresh one does,
/// while the physics index fills, and a game that set the rest of its filter
/// sets it again.
///
/// Scene name `platformer_controller`, on a prefab with a kinematic
/// `rigid_body`:
/// `{ "run_speed": 200, "jump_speed": 600, "gravity": 2000, "coyote_time": 0.1, "jump_buffer": 0.1, "jump_cut": 0.5, "air_jumps": 1, "shove_mass": 60, "shove_share": 0.5, "step_height": 16, "slope_limit": 50, "slide_limit": 40, "mask": [0, "ground"], "without": ["ghosts"] }`,
/// `gravity`, `shove_mass`, `slope_limit`, `slide_limit`, `mask` and
/// `without` optional (the world's gravity, no shove, the floor rule before
/// slopes, no slide, every layer and none skipped by default), and
/// `coyote_time`, `jump_buffer`, `jump_cut`, `air_jumps`, `shove_share` and
/// `step_height` with the defaults 0.1, 0.1, 0.5, 0, 0.5 and 0;
/// `shove_share` is read only with `shove_mass`. `mask` and `without` list
/// physics layers by number or by their manifest `physics_layers` name,
/// checked at load, and build its filter as
/// [`SpatialFilter::with_mask`] and [`SpatialFilter::without_layers`] do.
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
    /// The stat and amount a paid air jump spends; `None`, the default, has
    /// no paid jump.
    air_jump_cost: Option<(String, f32)>,
    /// `None`, the default, shoves nothing.
    shove: Option<Shove>,
    /// 0, the default, climbs nothing.
    step_height: f32,
    /// The steepest floor, in degrees; `None`, the default, keeps the floor
    /// rule before slopes, a normal's y at most −0.7.
    slope_limit: Option<f32>,
    /// The steepest slope face it stands on without sliding, in degrees;
    /// `None`, the default, is the slope limit.
    slide_limit: Option<f32>,
    /// Saved as the layers it sees and skips alone.
    filter: SpatialFilter,
    /// The run input, in `[-1, 1]`.
    run: f32,
    jump_held: bool,
    /// A press not yet seen by a step.
    pressed: bool,
    launch: Option<Vec2>,
    push: Vec2,
    /// The push its body's mass took at its last step.
    pushed: Vec2,
    /// Its last step's push out of a kinematic body was cut short.
    crushed: bool,
    /// A drop through a one-way edge asked for since its last step.
    drop_asked: bool,
    /// While a drop through a one-way edge lasts, that edge's top face's y.
    drop_edge: Option<f32>,
    /// Its speed along a slope face's tangent from sliding down it.
    slide_speed: f32,
    /// The normal of the floor it ended its last step on, and whether that
    /// is a slope tile's face; `None` off the ground. Saved on a slope face
    /// alone: a loaded controller on other ground reports none until a step
    /// after its still first one, as its ground.
    floor: Option<(Vec2, bool)>,
    buffer_left: f32,
    coyote_left: f32,
    lock_left: f32,
    /// Its own velocity at the end of its last step.
    velocity: Vec2,
    grounded: bool,
    jumped: bool,
    /// The jump its last step started was an air jump.
    air_jumped: bool,
    /// The air jump its last step started was paid.
    paid_air_jumped: bool,
    /// Its last step's fresh press was refused by the air-jump cost.
    air_jump_refused: bool,
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
            air_jump_cost: None,
            shove: None,
            step_height: 0.0,
            slope_limit: None,
            slide_limit: None,
            filter: SpatialFilter::new(),
            run: 0.0,
            jump_held: false,
            pressed: false,
            launch: None,
            push: Vec2::ZERO,
            pushed: Vec2::ZERO,
            crushed: false,
            drop_asked: false,
            drop_edge: None,
            slide_speed: 0.0,
            floor: None,
            buffer_left: 0.0,
            coyote_left: 0.0,
            lock_left: 0.0,
            velocity: Vec2::ZERO,
            grounded: false,
            jumped: false,
            air_jumped: false,
            paid_air_jumped: false,
            air_jump_refused: false,
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

    /// The controller with a paid air jump: past its free
    /// [air jumps](Self::with_air_jumps), a press in the air jumps at
    /// `jump_speed` again when the entity's [`Stats`] base of `stat` holds
    /// `amount`, which the step takes off it, with no cap. Below it the
    /// press is refused, and the jump buffer keeps it. Being grounded
    /// refills the free jumps alone.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*` or `amount` is not finite and
    /// at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_air_jump_cost(mut self, stat: &str, amount: f32) -> Self {
        self.set_air_jump_cost(Some((stat, amount)));
        self
    }

    /// Sets the [paid air jump](Self::with_air_jump_cost)'s stat and
    /// amount at run time, or takes it away with `None`.
    ///
    /// # Panics
    ///
    /// When the stat is not `[a-z][a-z0-9_]*` or the amount is not finite
    /// and at least 0.
    #[track_caller]
    pub fn set_air_jump_cost(&mut self, cost: Option<(&str, f32)>) {
        if let Some((stat, amount)) = cost {
            assert!(
                valid_stat(stat),
                "PlatformerController::set_air_jump_cost: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
            );
            assert!(
                amount.is_finite() && amount >= 0.0,
                "PlatformerController::set_air_jump_cost: amount must be finite and >= 0 (got {amount})"
            );
        }
        self.air_jump_cost = cost.map(|(stat, amount)| (stat.to_owned(), amount));
    }

    /// The stat and amount a paid air jump spends, when it has one.
    #[must_use]
    pub fn air_jump_cost(&self) -> Option<(&str, f32)> {
        self.air_jump_cost
            .as_ref()
            .map(|(stat, amount)| (stat.as_str(), *amount))
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

    /// The controller with a slope limit: a surface sloping at most
    /// `degrees` is a floor it stands on and walks, a steeper one a wall it
    /// does not climb. The default keeps the floor rule before slopes, a
    /// normal's y at most −0.7, about 45.6°.
    ///
    /// # Panics
    ///
    /// When `degrees` is not in (0, 90).
    #[must_use]
    #[track_caller]
    pub fn with_slope_limit(mut self, degrees: f32) -> Self {
        assert!(
            degrees > 0.0 && degrees < 90.0,
            "PlatformerController::with_slope_limit: degrees must be in (0, 90) (got {degrees})"
        );
        self.slope_limit = Some(degrees);
        self
    }

    /// The controller with a slide limit: standing on a slope tile's face
    /// steeper than `degrees` and within its slope limit, it slides down
    /// along the face at the speed gravity's share along it gives, its run
    /// added, until the floor is gentler. The default, the slope limit,
    /// never slides.
    ///
    /// # Panics
    ///
    /// When `degrees` is not in (0, 90).
    #[must_use]
    #[track_caller]
    pub fn with_slide_limit(mut self, degrees: f32) -> Self {
        assert!(
            degrees > 0.0 && degrees < 90.0,
            "PlatformerController::with_slide_limit: degrees must be in (0, 90) (got {degrees})"
        );
        self.slide_limit = Some(degrees);
        self
    }

    /// Drops through the one-way edge it stands on at its next step: its
    /// casts skip one-way tiles until its bottom is below the edge's top
    /// face, or it lands on another floor or rises, and a jump press made
    /// with it does not jump. Standing on anything else, it does nothing.
    pub fn drop_through(&mut self) {
        self.drop_asked = true;
    }

    /// The outward normal of the floor its last step ended on, `(0, -1)` on
    /// flat ground; `None` off the ground, and after a load off a slope face
    /// until a step past its still first one.
    #[must_use]
    pub fn floor_normal(&self) -> Option<Vec2> {
        self.floor.map(|(normal, _)| normal)
    }

    /// The push its last step took from the hits and impulses its body's
    /// mass kept ([`take_push`]), velocity it joined as a
    /// [`push`](Self::push) does; zero without a mass. A game turns a hard
    /// hit into a [`knock_back`](crate::knock_back) by it.
    #[must_use]
    pub fn pushed(&self) -> Vec2 {
        self.pushed
    }

    /// Its last step's push out of a kinematic body that overlapped it was
    /// cut short by a wall or a floor, so it stays overlapped: a deck coming
    /// down on it where it stands, a door closing on it against a wall.
    #[must_use]
    pub fn crushed(&self) -> bool {
        self.crushed
    }

    /// The floor rule: a surface whose normal's y is at most this is a
    /// floor.
    fn floor_y(&self) -> f32 {
        self.slope_limit
            .map_or(FLOOR_NORMAL_Y, |degrees| -degrees.to_radians().cos())
    }

    /// How far down its ground check snaps it onto a slope, past a pixel:
    /// a step's run at the slope limit's grade.
    fn snap_reach(&self, dt: f32) -> f32 {
        let cos = -self.floor_y();
        let grade = (1.0 - cos * cos).sqrt() / cos;
        GROUND_PROBE.max(self.run_speed * dt * grade)
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

    /// Its last step started a [paid air jump](Self::with_air_jump_cost),
    /// a buffered press's included, which [`air_jumped`](Self::air_jumped)
    /// also reports.
    #[must_use]
    pub fn paid_air_jumped(&self) -> bool {
        self.paid_air_jumped
    }

    /// Its last step refused a fresh press in the air for the
    /// [air-jump cost](Self::with_air_jump_cost): once a press, though the
    /// jump buffer keeps it.
    #[must_use]
    pub fn air_jump_refused(&self) -> bool {
        self.air_jump_refused
    }

    /// One step of `dt` for the body at `position` with `collider`: how its
    /// body moves in this step, and the body it shoves.
    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        world: &World,
        entity: Entity,
        position: Vec2,
        collider: Collider,
        world_gravity: f32,
        dt: f32,
        found: &mut Vec<Option<Entity>>,
    ) -> Moved {
        self.jumped = false;
        self.air_jumped = false;
        self.paid_air_jumped = false;
        self.air_jump_refused = false;
        if !self.started {
            self.started = true;
            return Moved {
                velocity: Vec2::ZERO,
                position: None,
                shove: None,
                plough: None,
            };
        }
        let base = self.filter.excluding(entity);
        let floor_y = self.floor_y();
        let origin = position + collider.offset;
        let mut at = self.push_out(world, collider.shape, origin, &base, found);

        // The ground check: a floor within a pixel, or within the snap reach
        // on a slope face or after one, grounded and not launched; a drop
        // asked for on a one-way edge starts there instead, and one that
        // lasts skips one-way tiles.
        let snaps = self.grounded && self.launch.is_none();
        let on_slope = self.floor.is_some_and(|(_, slope)| slope);
        let drop_asked = std::mem::take(&mut self.drop_asked);
        let mut filter = if self.drop_edge.is_some() {
            base.with_one_way(false)
        } else {
            base
        };
        self.grounded = false;
        self.floor = None;
        let mut ground = None;
        if self.velocity.y >= 0.0 {
            let reach = if snaps {
                self.snap_reach(dt)
            } else {
                GROUND_PROBE
            };
            let probe = |filter: &SpatialFilter| {
                probe_ground_on(world, collider.shape, at, filter, reach, floor_y)
            };
            let hit =
                probe(&filter).filter(|hit| hit.distance <= GROUND_PROBE || hit.slope || on_slope);
            if let Some(hit) = hit {
                let skipping = filter.with_one_way(false);
                let one_way = drop_asked
                    && self.drop_edge.is_none()
                    && probe(&skipping).is_none_or(|solid| solid.distance > hit.distance);
                if one_way {
                    // A drop takes no jump, nor coyote time after it.
                    self.drop_edge = Some(hit.point.y);
                    filter = skipping;
                    self.pressed = false;
                    self.buffer_left = 0.0;
                    self.coyote_left = 0.0;
                } else {
                    self.grounded = true;
                    self.air_jumps_left = self.air_jumps;
                    at.y += (hit.distance - SKIN).max(0.0);
                    ground = hit.entity;
                    self.floor = Some((hit.normal, hit.slope));
                }
            }
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
            } else if pressed && let Some((stat, amount)) = &self.air_jump_cost {
                let base = world
                    .get::<Stats>(entity)
                    .and_then(|stats| stats.base(stat));
                if base.is_some_and(|base| base >= *amount) {
                    self.velocity.y = -self.jump_speed;
                    self.buffer_left = 0.0;
                    self.jumped = true;
                    self.air_jumped = true;
                    self.paid_air_jumped = true;
                    self.cut_armed = true;
                    lifted = true;
                } else if self.pressed {
                    self.air_jump_refused = true;
                }
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

        // Move and slide, along a slope face it stands on, a climb or a shove
        // at a wall, then the ground's carry.
        let walking = self.grounded;
        let face = self
            .floor
            .filter(|&(_, slope)| slope && self.grounded)
            .map(|(normal, _)| normal);
        let delta = if let Some(normal) = face {
            let tangent = Vec2::new(-normal.y, normal.x);
            let steep = self
                .slide_limit
                .is_some_and(|degrees| normal.y > -degrees.to_radians().cos());
            self.slide_speed = if steep {
                self.slide_speed + gravity * tangent.y * dt
            } else {
                0.0
            };
            tangent * (self.velocity.x + self.slide_speed) * dt
        } else {
            self.slide_speed = 0.0;
            Vec2::new(self.velocity.x, mean_vy) * dt
        };
        let slide = move_and_slide_on(world, collider.shape, at, delta, &filter, floor_y, true);
        at += slide.moved;
        if slide.floor && self.velocity.y >= 0.0 {
            self.grounded = true;
            self.air_jumps_left = self.air_jumps;
            self.velocity.y = 0.0;
            ground = slide.floor_body;
            self.floor = Some((slide.floor_normal, slide.floor_slope));
        }
        if let Some(edge) = self.drop_edge {
            let bottom = at.y + collider.shape.half_extents().y;
            if bottom > edge || self.grounded || self.velocity.y < 0.0 {
                self.drop_edge = None;
            }
        }
        if slide.ceiling && self.velocity.y < 0.0 {
            self.velocity.y = 0.0;
        }
        let mut climbed = false;
        let mut shoved = None;
        let mut plough = None;
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
                plough = if shoved.is_none() {
                    self.plough_of(world, entity, &slide)
                } else {
                    None
                };
                self.velocity.x = plough.map_or(0.0, |(_, _, kept)| kept);
            }
        }
        if self.grounded
            && !lifted
            && let Some(ground) = ground
            && is_kinematic(world, ground)
            && let Some(moving) = world.get::<Velocity>(ground)
        {
            at = carry(world, collider.shape, at, moving.0 * dt, &filter, ground);
        }

        // The timers, last, so the getters read the step's end.
        self.ground = if self.grounded { ground } else { None };
        if !self.grounded {
            self.floor = None;
        }
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
                plough: None,
            }
        } else {
            Moved {
                velocity: (at - origin) / dt,
                position: None,
                shove: shoved,
                plough: plough.map(|(body, impulse, _)| (body, impulse)),
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
        // A box, and any later shape, reaches no further.
        let round = match shape {
            Shape::Circle { radius } => CLIMB_ROUND_REACH * radius,
            _ => 0.0,
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

    /// The plough of the first wall `slide` met under locked input, when it
    /// is a dynamic body and the controller's body `entity` has a mass: the
    /// body, its impulse along x and the controller's horizontal speed
    /// after, the inelastic share `M m / (M + m)` of their closing speed
    /// taken from the controller's `u` and given to the body's `w`.
    fn plough_of(
        &self,
        world: &World,
        entity: Entity,
        slide: &Slide,
    ) -> Option<(Entity, Vec2, f32)> {
        if self.lock_left <= 0.0 {
            return None;
        }
        let own = world.get::<RigidBody>(entity)?.inv_mass;
        let body = slide.wall_body?;
        let rigid = world.get::<RigidBody>(body)?;
        let other = world.get::<Velocity>(body)?.0;
        if rigid.kind != BodyKind::Dynamic
            || !(own.is_finite() && own > 0.0 && rigid.inv_mass.is_finite() && rigid.inv_mass > 0.0)
        {
            return None;
        }
        // Into the wall along x, the axis a wall stops.
        let into = -slide.wall_normal.x.signum();
        let (u, w) = (self.velocity.x * into, other.x * into);
        if u <= 0.0 {
            return None;
        }
        let impulse = (u - w).max(0.0) / (own + rigid.inv_mass);
        Some((
            body,
            Vec2::new(into * impulse, 0.0),
            into * (u - impulse * own),
        ))
    }

    /// `shape` at `at` pushed out of each kinematic body `filter` sees
    /// overlapping it at the last step's positions, in the query's order,
    /// but its ground and a body a kit mover drives: along their contact's
    /// normal by the overlap and the skin, cast with that body refused; a
    /// cast cut short stops the skin before what it met and crushes it.
    fn push_out(
        &mut self,
        world: &World,
        shape: Shape,
        mut at: Vec2,
        filter: &SpatialFilter,
        found: &mut Vec<Option<Entity>>,
    ) -> Vec2 {
        self.crushed = false;
        overlap(world, shape, at, filter, found);
        for &other in found.iter().flatten() {
            if Some(other) == self.ground
                || !is_kinematic(world, other)
                || world.has::<PlatformerController>(other)
                || world.has::<TopDownMover>(other)
            {
                continue;
            }
            let (Some(position), Some(collider)) =
                (world.get::<Position>(other), world.get::<Collider>(other))
            else {
                continue;
            };
            let Some(contact) =
                overlap_contact(shape, at, collider.shape, collider.center(position.0))
            else {
                continue;
            };
            if contact.penetration <= 0.0 {
                continue;
            }
            let reach = contact.penetration + SKIN;
            match cast_shape_where(world, shape, at, contact.normal, reach, filter, |hit| {
                hit.entity != Some(other)
            }) {
                Some(hit) => {
                    at += contact.normal * (hit.distance - SKIN).max(0.0);
                    self.crushed = true;
                }
                None => at += contact.normal * reach,
            }
        }
        at
    }
}

/// The contact that moves `shape` at `at` out of `other` at `center`, as
/// the physics' narrow phase finds it.
fn overlap_contact(shape: Shape, at: Vec2, other: Shape, center: Vec2) -> Option<Contact> {
    match (shape, other) {
        (
            Shape::Aabb { half_extents },
            Shape::Aabb {
                half_extents: theirs,
            },
        ) => aabb_vs_aabb(&Aabb::new(at, half_extents), &Aabb::new(center, theirs)),
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            aabb_vs_circle(&Aabb::new(at, half_extents), center, radius)
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            aabb_vs_circle(&Aabb::new(center, half_extents), at, radius).map(|contact| Contact {
                normal: -contact.normal,
                penetration: contact.penetration,
            })
        }
        (Shape::Circle { radius }, Shape::Circle { radius: theirs }) => {
            circle_vs_circle(at, radius, center, theirs)
        }
        _ => None,
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
    /// The body it ploughed and the impulse it gives it.
    plough: Option<(Entity, Vec2)>,
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
        world.register_saved::<PlatformerController>(PLATFORMER_CONTROLLER_COMPONENT);
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
            .field(
                FieldDescription::new(
                    "slope_limit",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Exclusive(90.0),
                    },
                    "degrees: the steepest floor it walks, a steeper slope a wall; the floor rule before slopes, about 45.6, when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "slide_limit",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Exclusive(90.0),
                    },
                    "degrees: on a slope face steeper than this it slides down; the slope limit, no slide, when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "mask",
                    layer_list(),
                    "the layers whose bodies and tiles block it; every layer when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "without",
                    layer_list(),
                    "the layers whose bodies and tiles it passes, beside mask",
                )
                .default("[]"),
            )
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample(r#"{ "run_speed": 200, "jump_speed": 600 }"#)
            .roundtrip(round_trip::<PlatformerControllerDef>),
        );
        world.register_component_with_assets(
            PLATFORMER_CONTROLLER_COMPONENT,
            |def: &PlatformerControllerDef, world: &World| {
                check_layers("mask", def.mask.as_deref().unwrap_or_default(), world)?;
                check_layers("without", &def.without, world)
            },
            |def: PlatformerControllerDef, at: &SpawnContext<'_>| {
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
                if let Some(degrees) = def.slope_limit {
                    controller = controller.with_slope_limit(degrees);
                }
                if let Some(degrees) = def.slide_limit {
                    controller = controller.with_slide_limit(degrees);
                }
                let mut filter = SpatialFilter::new();
                if let Some(mask) = &def.mask {
                    filter = filter.with_mask(layer_bits(mask, at.world()));
                }
                if !def.without.is_empty() {
                    filter = filter.without_layers(layer_bits(&def.without, at.world()));
                }
                (controller.with_filter(filter),)
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
    #[serde(default, deserialize_with = "optional_angle")]
    slope_limit: Option<f32>,
    #[serde(default, deserialize_with = "optional_angle")]
    slide_limit: Option<f32>,
    #[serde(default)]
    mask: Option<Vec<LayerRef>>,
    #[serde(default)]
    without: Vec<LayerRef>,
}

/// A controller as a save writes it: its settings and its runtime state,
/// of its filter the layers it sees and skips, and not the first-step flag.
/// The air-jump cost, the shove, the step height, the slope and slide
/// limits, the filter's layers, the paid-jump flags and the slope and drop
/// state are left out when off or at their defaults, so a save from before
/// them loads with each off and today's filter.
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
    air_jump_cost: Option<AirJumpCost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shove: Option<Shove>,
    #[serde(default, skip_serializing_if = "is_zero")]
    step_height: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slope_limit: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slide_limit: Option<f32>,
    #[serde(default = "all_layers", skip_serializing_if = "is_all_layers")]
    mask: u32,
    #[serde(default, skip_serializing_if = "is_no_layer")]
    without: u32,
    run: f32,
    jump_held: bool,
    pressed: bool,
    #[serde(with = "optional_vec2")]
    launch: Option<Vec2>,
    #[serde(with = "vec2")]
    push: Vec2,
    #[serde(default, with = "vec2", skip_serializing_if = "is_zero_vec")]
    pushed: Vec2,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    crushed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    drop_asked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    drop_edge: Option<f32>,
    #[serde(default, skip_serializing_if = "is_zero")]
    slide_speed: f32,
    #[serde(
        default,
        with = "optional_vec2",
        skip_serializing_if = "Option::is_none"
    )]
    slope_normal: Option<Vec2>,
    buffer_left: f32,
    coyote_left: f32,
    lock_left: f32,
    #[serde(with = "vec2")]
    velocity: Vec2,
    grounded: bool,
    jumped: bool,
    air_jumped: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    paid_air_jumped: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    air_jump_refused: bool,
    cut_armed: bool,
    air_jumps_left: u32,
}

/// A paid air jump's cost as a save writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AirJumpCost {
    stat: String,
    amount: f32,
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
            ("pushed", save.pushed),
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
        if let Some(cost) = &save.air_jump_cost {
            if !valid_stat(&cost.stat) {
                return Err(format!(
                    "air_jump_cost.stat: `{}` is not a stat name (`[a-z][a-z0-9_]*`)",
                    cost.stat
                ));
            }
            if !at_least_0(cost.amount) {
                return Err(format!(
                    "air_jump_cost.amount: must be finite and >= 0 (got {})",
                    cost.amount
                ));
            }
        }
        for (field, limit) in [
            ("slope_limit", save.slope_limit),
            ("slide_limit", save.slide_limit),
        ] {
            if let Some(degrees) = limit
                && !(degrees > 0.0 && degrees < 90.0)
            {
                return Err(format!("{field}: must be in (0, 90) (got {degrees})"));
            }
        }
        for (field, value) in [
            ("drop_edge", save.drop_edge.unwrap_or(0.0)),
            ("slide_speed", save.slide_speed),
        ] {
            if !value.is_finite() {
                return Err(format!("{field}: must be finite (got {value})"));
            }
        }
        if let Some(normal) = save.slope_normal
            && !normal.is_finite()
        {
            return Err(format!("slope_normal: must be finite (got {normal})"));
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
            air_jump_cost: save.air_jump_cost.map(|cost| (cost.stat, cost.amount)),
            shove: save.shove,
            step_height: save.step_height,
            slope_limit: save.slope_limit,
            slide_limit: save.slide_limit,
            filter: SpatialFilter::new()
                .with_mask(save.mask)
                .without_layers(save.without),
            run: save.run,
            jump_held: save.jump_held,
            pressed: save.pressed,
            launch: save.launch,
            push: save.push,
            pushed: save.pushed,
            crushed: save.crushed,
            drop_asked: save.drop_asked,
            drop_edge: save.drop_edge,
            slide_speed: save.slide_speed,
            floor: save.slope_normal.map(|normal| (normal, true)),
            buffer_left: save.buffer_left,
            coyote_left: save.coyote_left,
            lock_left: save.lock_left,
            velocity: save.velocity,
            grounded: save.grounded,
            jumped: save.jumped,
            air_jumped: save.air_jumped,
            paid_air_jumped: save.paid_air_jumped,
            air_jump_refused: save.air_jump_refused,
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
            air_jump_cost: controller
                .air_jump_cost
                .map(|(stat, amount)| AirJumpCost { stat, amount }),
            shove: controller.shove,
            step_height: controller.step_height,
            slope_limit: controller.slope_limit,
            slide_limit: controller.slide_limit,
            mask: controller.filter.mask(),
            without: controller.filter.without(),
            run: controller.run,
            jump_held: controller.jump_held,
            pressed: controller.pressed,
            launch: controller.launch,
            push: controller.push,
            pushed: controller.pushed,
            crushed: controller.crushed,
            drop_asked: controller.drop_asked,
            drop_edge: controller.drop_edge,
            slide_speed: controller.slide_speed,
            slope_normal: controller
                .floor
                .filter(|&(_, slope)| slope)
                .map(|(normal, _)| normal),
            buffer_left: controller.buffer_left,
            coyote_left: controller.coyote_left,
            lock_left: controller.lock_left,
            velocity: controller.velocity,
            grounded: controller.grounded,
            jumped: controller.jumped,
            air_jumped: controller.air_jumped,
            paid_air_jumped: controller.paid_air_jumped,
            air_jump_refused: controller.air_jump_refused,
            cut_armed: controller.cut_armed,
            air_jumps_left: controller.air_jumps_left,
        }
    }
}

/// Whether `name` is a stat name, as `Stats` reads one.
fn valid_stat(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
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

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_vec(value: &Vec2) -> bool {
    *value == Vec2::ZERO
}

/// An optional angle in degrees in `(0, 90)`, for a scene field's
/// `deserialize_with`.
fn optional_angle<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f32>, D::Error> {
    let value = Option::<f32>::deserialize(deserializer)?;
    match value {
        Some(degrees) if !(degrees > 0.0 && degrees < 90.0) => Err(D::Error::custom(format!(
            "must be in (0, 90) (got {degrees})"
        ))),
        value => Ok(value),
    }
}

fn all_layers() -> u32 {
    RigidBody::ALL_LAYERS
}

// The references are serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_all_layers(mask: &u32) -> bool {
    *mask == RigidBody::ALL_LAYERS
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_no_layer(layers: &u32) -> bool {
    *layers == 0
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's controllers, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    /// The bodies a controller overlaps, kept likewise.
    found: Vec<Option<Entity>>,
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
            // The cost moves across rather than cloning, so a step allocates
            // nothing.
            let stored = world.get_mut::<PlatformerController>(entity).unwrap();
            let cost = stored.air_jump_cost.take();
            let mut controller = stored.clone();
            controller.air_jump_cost = cost;
            // What hits and impulses gave its body's mass joins it as a push.
            let taken = take_push(world, entity);
            if taken != Vec2::ZERO {
                controller.push(taken);
            }
            controller.pushed = taken;
            if let Some(intent) = world.get::<Intent>(entity) {
                controller.set_run(intent.movement().x);
                controller.set_jump_held(intent.held(JUMP_SLOT));
                if intent.pressed(JUMP_SLOT) {
                    controller.press_jump();
                    // Down with a jump drops through a one-way edge.
                    if intent.movement().y > 0.5 {
                        controller.drop_through();
                    }
                }
            }
            let moved = controller.step(
                world,
                entity,
                position,
                collider,
                gravity,
                dt,
                &mut self.found,
            );
            if controller.paid_air_jumped
                && let Some((stat, amount)) = &controller.air_jump_cost
                && let Some(stats) = world.get_mut::<Stats>(entity)
                && let Some(base) = stats.base(stat)
            {
                stats.set_base(stat, base - amount);
            }
            *world.get_mut::<PlatformerController>(entity).unwrap() = controller;
            world.get_mut::<Velocity>(entity).unwrap().0 = moved.velocity;
            if let Some(at) = moved.position {
                world.get_mut::<Position>(entity).unwrap().0 = at;
            }
            if let Some((body, speed)) = moved.shove {
                shove(world, body, speed);
            }
            if let Some((body, impulse)) = moved.plough {
                apply_impulse(world, body, impulse);
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
