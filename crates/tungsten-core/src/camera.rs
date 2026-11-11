//! The camera (redo 7.18): rigs that frame the view, and the view each
//! frame draws.
//!
//! [`CameraState`] is the camera's output alone: [`update`], which the
//! engine's `camera_update` system runs, writes it each frame from the
//! winning [`CameraRig`] and never reads its position or zoom back, so a
//! game's write lasts only until the next update. A rig follows an entity
//! ([`FollowRig`]), stays fixed ([`FixedRig`]) or holds a point a game
//! moves ([`RigKind::Scripted`]), each with optional bounds, a view height,
//! a priority and a blend. The controller's base rig
//! ([`CameraController::rig`]) lies under the rigs entities carry: the
//! highest live rig wins, a tie going to the latest raised, and a switch
//! blends position and view height over the incoming rig's [`RigBlend`].
//! The shake is a layer after the winner's view.
//!
//! The default matrix keeps the pre-M10 y-down pixel space; the text
//! pipeline is screen-space and ignores the camera.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::camera::rh::proj::directx::orthographic;
use glam::{Mat4, Vec2, Vec3};

use crate::components::Transform;
use crate::debug_draw::DebugDraw;
use crate::ecs::{Entity, With, World};
use crate::physics::Velocity;
use crate::tween::Easing;

fn rotate_vec(vec: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(vec.x * cos - vec.y * sin, vec.x * sin + vec.y * cos)
}

fn viewport_size(viewport_w: f32, viewport_h: f32, zoom: f32) -> Vec2 {
    let zoom = zoom.max(f32::EPSILON);
    Vec2::new(viewport_w / zoom, viewport_h / zoom)
}

fn rotated_view_extents(view_size: Vec2, rotation: f32) -> (Vec2, Vec2) {
    let corners = [
        Vec2::ZERO,
        rotate_vec(Vec2::new(view_size.x, 0.0), rotation),
        rotate_vec(Vec2::new(0.0, view_size.y), rotation),
        rotate_vec(view_size, rotation),
    ];
    let mut min = corners[0];
    let mut max = corners[0];
    for corner in corners.into_iter().skip(1) {
        min = min.min(corner);
        max = max.max(corner);
    }
    (min, max)
}

#[track_caller]
fn finite(value: Vec2, what: &str) {
    assert!(value.is_finite(), "{what} must be finite, not {value}");
}

#[track_caller]
fn unit(value: f32, what: &str) {
    assert!(
        (0.0..=1.0).contains(&value),
        "{what} must lie in [0, 1], not {value}"
    );
}

#[track_caller]
fn positive(value: f32, what: &str) {
    assert!(
        value.is_finite() && value > 0.0,
        "{what} must be positive and finite, not {value}"
    );
}

#[track_caller]
fn at_least_zero(value: f32, what: &str) {
    assert!(
        value.is_finite() && value >= 0.0,
        "{what} must be zero or more and finite, not {value}"
    );
}

/// World-space camera clamp bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraBounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl CameraBounds {
    /// The box from `min` to `max`; an infinite edge leaves that side open.
    ///
    /// # Panics
    ///
    /// When a corner is NaN, or `min` passes `max` on an axis.
    #[track_caller]
    #[must_use]
    pub fn new(min: Vec2, max: Vec2) -> Self {
        assert!(
            !min.is_nan() && !max.is_nan() && min.cmple(max).all(),
            "camera bounds need min <= max with no NaN, not {min} to {max}"
        );
        Self { min, max }
    }

    /// Clamp top-left anchor so visible AABB stays within bounds.
    #[must_use]
    pub fn clamp_position(
        &self,
        position: Vec2,
        viewport_w: f32,
        viewport_h: f32,
        zoom: f32,
        rotation: f32,
    ) -> Vec2 {
        let view_size = viewport_size(viewport_w, viewport_h, zoom);
        let (offset_min, offset_max) = rotated_view_extents(view_size, rotation);
        let min_x = self.min.x - offset_min.x;
        let min_y = self.min.y - offset_min.y;
        let max_x = (self.max.x - offset_max.x).max(min_x);
        let max_y = (self.max.y - offset_max.y).max(min_y);
        // `max` and `min`, not `clamp`, whose assert fails on a NaN bound: a
        // NaN rotation leaves the axis unclamped, a NaN minimum clamps it from
        // above only, and a NaN maximum pins it to the minimum, as before.
        Vec2::new(
            position.x.max(min_x).min(max_x),
            position.y.max(min_y).min(max_y),
        )
    }

    fn centre(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
}

/// What a follow rig's lead runs ahead along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadFrom {
    /// The target's `Velocity`, else its drawn point's change over the frame.
    Velocity,
    /// The direction the World's [`CameraAim`] gives for the target; without
    /// one the lead stays at zero.
    Aim,
}

/// A follow rig's lead: an offset ahead of its target that eases toward
/// `velocity · time` ([`LeadFrom::Velocity`]) or `direction · max`
/// ([`LeadFrom::Aim`]), capped at `max` world units, at `smoothing` per
/// 1/60 s (`D-100`'s rate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigLead {
    pub from: LeadFrom,
    /// The longest lead, in world units.
    pub max: f32,
    /// Seconds of velocity ahead, for [`LeadFrom::Velocity`].
    pub time: f32,
    /// The share of the remaining way covered per 1/60 s: 0 never moves, 1
    /// snaps.
    pub smoothing: f32,
}

impl RigLead {
    /// The smoothing a new lead eases at.
    pub const SMOOTHING: f32 = 0.1;

    /// A lead along the target's velocity, `time` seconds ahead and at most
    /// `max` world units.
    ///
    /// # Panics
    ///
    /// When `max` or `time` is negative or not finite.
    #[track_caller]
    #[must_use]
    pub fn velocity(max: f32, time: f32) -> Self {
        at_least_zero(max, "a lead's max");
        at_least_zero(time, "a lead's time");
        Self {
            from: LeadFrom::Velocity,
            max,
            time,
            smoothing: Self::SMOOTHING,
        }
    }

    /// A lead of `max` world units along the target's aim.
    ///
    /// # Panics
    ///
    /// When `max` is negative or not finite.
    #[track_caller]
    #[must_use]
    pub fn aim(max: f32) -> Self {
        at_least_zero(max, "a lead's max");
        Self {
            from: LeadFrom::Aim,
            max,
            time: 0.0,
            smoothing: Self::SMOOTHING,
        }
    }

    /// The lead easing at `smoothing` per 1/60 s.
    ///
    /// # Panics
    ///
    /// When `smoothing` lies outside [0, 1].
    #[track_caller]
    #[must_use]
    pub fn with_smoothing(mut self, smoothing: f32) -> Self {
        unit(smoothing, "a lead's smoothing");
        self.smoothing = smoothing;
        self
    }

    #[track_caller]
    fn check(self) {
        at_least_zero(self.max, "a lead's max");
        at_least_zero(self.time, "a lead's time");
        unit(self.smoothing, "a lead's smoothing");
    }
}

/// A rig that follows an entity's drawn point (its `Transform` position)
/// once that point leaves the dead zone, at its smoothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowRig {
    /// The entity followed. `None` follows the rig's own entity, and on the
    /// base rig follows nothing, which holds the view where it stands.
    pub target: Option<Entity>,
    /// Added to the target's drawn point: the anchor the view keeps.
    pub offset: Vec2,
    /// The box, in world units centred in the view and clamped to it, the
    /// anchor roams before the view moves.
    pub dead_zone: Vec2,
    /// Per axis, the share of the remaining way covered per 1/60 s at any
    /// frame rate: 0 never moves, 1 snaps (`D-100`).
    pub smoothing: Vec2,
    /// The lead ahead of the target, added to the anchor before the dead
    /// zone judges it.
    pub lead: Option<RigLead>,
}

impl Default for FollowRig {
    fn default() -> Self {
        Self {
            target: None,
            offset: Vec2::ZERO,
            dead_zone: Vec2::ZERO,
            smoothing: Vec2::ONE,
            lead: None,
        }
    }
}

impl FollowRig {
    /// A follow of `target`, snapping, with no dead zone, offset or lead.
    #[must_use]
    pub fn of(target: Entity) -> Self {
        Self {
            target: Some(target),
            ..Self::default()
        }
    }

    /// The follow with its anchor at the target's drawn point plus
    /// `offset`.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[track_caller]
    #[must_use]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        finite(offset, "a follow's offset");
        self.offset = offset;
        self
    }

    /// The follow with a dead zone of `size` world units.
    ///
    /// # Panics
    ///
    /// When a side is negative or not finite.
    #[track_caller]
    #[must_use]
    pub fn with_dead_zone(mut self, size: Vec2) -> Self {
        at_least_zero(size.x, "a dead zone's width");
        at_least_zero(size.y, "a dead zone's height");
        self.dead_zone = size;
        self
    }

    /// The follow covering `smoothing` of the way per 1/60 s on each axis.
    ///
    /// # Panics
    ///
    /// When an axis lies outside [0, 1].
    #[track_caller]
    #[must_use]
    pub fn with_smoothing(mut self, smoothing: Vec2) -> Self {
        unit(smoothing.x, "a follow's smoothing");
        unit(smoothing.y, "a follow's smoothing");
        self.smoothing = smoothing;
        self
    }

    /// The follow running `lead` ahead of its target.
    ///
    /// # Panics
    ///
    /// When the lead's values are out of range.
    #[track_caller]
    #[must_use]
    pub fn with_lead(mut self, lead: RigLead) -> Self {
        lead.check();
        self.lead = Some(lead);
        self
    }

    #[track_caller]
    fn check(&self) {
        finite(self.offset, "a follow's offset");
        at_least_zero(self.dead_zone.x, "a dead zone's width");
        at_least_zero(self.dead_zone.y, "a dead zone's height");
        unit(self.smoothing.x, "a follow's smoothing");
        unit(self.smoothing.y, "a follow's smoothing");
        if let Some(lead) = self.lead {
            lead.check();
        }
    }
}

/// Where a fixed rig holds the view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FixedRig {
    /// The view's centre.
    At(Vec2),
    /// The view's top-left.
    Corner(Vec2),
    /// A box the view centres on: at the rig's view height, else the least
    /// height that shows it whole at the window's aspect.
    Rect(CameraBounds),
}

/// What a rig does each frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RigKind {
    /// Follows an entity's drawn point.
    Follow(FollowRig),
    /// Holds a point, a corner or a box.
    Fixed(FixedRig),
    /// Centres the view on a point a game, or a timeline, moves through
    /// [`CameraRig::set_point`].
    Scripted(Vec2),
}

impl RigKind {
    /// `follow`, `fixed` or `scripted`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Follow(_) => "follow",
            Self::Fixed(_) => "fixed",
            Self::Scripted(_) => "scripted",
        }
    }

    fn tag(&self) -> u8 {
        match self {
            Self::Follow(_) => 0,
            Self::Fixed(_) => 1,
            Self::Scripted(_) => 2,
        }
    }

    #[track_caller]
    fn check(&self) {
        match self {
            Self::Follow(follow) => follow.check(),
            Self::Fixed(FixedRig::At(point) | FixedRig::Corner(point)) | Self::Scripted(point) => {
                finite(*point, "a rig's point");
            }
            Self::Fixed(FixedRig::Rect(rect)) => {
                finite(rect.min, "a rig's rect");
                finite(rect.max, "a rig's rect");
                assert!(
                    rect.min.cmplt(rect.max).all(),
                    "a rig's rect needs min < max on both axes, not {} to {}",
                    rect.min,
                    rect.max
                );
            }
        }
    }
}

/// How the view moves to a rig that starts winning: over `time` seconds of
/// the game clock, eased; a time of 0 cuts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigBlend {
    pub time: f32,
    pub ease: Easing,
}

impl Default for RigBlend {
    fn default() -> Self {
        Self::CUT
    }
}

impl RigBlend {
    /// No blend: the view cuts to the rig.
    pub const CUT: Self = Self {
        time: 0.0,
        ease: Easing::SineInOut,
    };

    /// A blend over `time` seconds, `sine_in_out`.
    ///
    /// # Panics
    ///
    /// When `time` is negative or not finite.
    #[track_caller]
    #[must_use]
    pub fn over(time: f32) -> Self {
        at_least_zero(time, "a blend's time");
        Self { time, ..Self::CUT }
    }

    /// The blend eased by `ease`.
    #[must_use]
    pub fn with_ease(mut self, ease: Easing) -> Self {
        self.ease = ease;
        self
    }
}

/// A camera rig: a component an entity carries, and the type of the
/// controller's base rig ([`CameraController::rig`]).
///
/// Each frame the highest live rig wins, a tie going to the latest raised
/// and then the lower entity index; the base rig lies under the rest, at
/// its own priority, raised first. A rig live from its spawn, or raised by
/// [`raise`](Self::raise), takes its stamp at the next update; [`raise`]
/// and [`lower`] stamp at once, so raises made in one step keep their
/// order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraRig {
    pub kind: RigKind,
    /// The box the rig's view stays inside, at its own height.
    pub bounds: Option<CameraBounds>,
    /// The world units the view shows top to bottom; `None` shows the
    /// window's height at a zoom of 1 (a rect rig: the least that shows its
    /// box whole).
    pub view_height: Option<f32>,
    /// A scale over the view: 2 shows half the view height.
    pub zoom: f32,
    pub priority: i32,
    /// How the view moves to this rig when it starts winning.
    pub blend: RigBlend,
    live: bool,
    stamp: u64,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::follow(FollowRig::default())
    }
}

impl CameraRig {
    /// A follow rig, live, at priority 0, cutting in.
    #[must_use]
    pub fn follow(follow: FollowRig) -> Self {
        Self {
            kind: RigKind::Follow(follow),
            bounds: None,
            view_height: None,
            zoom: 1.0,
            priority: 0,
            blend: RigBlend::CUT,
            live: true,
            stamp: 0,
        }
    }

    /// A fixed rig, live, at priority 0, cutting in.
    ///
    /// # Panics
    ///
    /// When its point is not finite, or its box is empty or not finite.
    #[track_caller]
    #[must_use]
    pub fn fixed(fixed: FixedRig) -> Self {
        let kind = RigKind::Fixed(fixed);
        kind.check();
        Self {
            kind,
            ..Self::default()
        }
    }

    /// A scripted rig centred on `centre`, live, at priority 0, cutting in.
    ///
    /// # Panics
    ///
    /// When `centre` is not finite.
    #[track_caller]
    #[must_use]
    pub fn scripted(centre: Vec2) -> Self {
        finite(centre, "a rig's point");
        Self {
            kind: RigKind::Scripted(centre),
            ..Self::default()
        }
    }

    /// The rig kept inside `bounds`.
    ///
    /// # Panics
    ///
    /// When a corner is NaN or `min` passes `max`.
    #[track_caller]
    #[must_use]
    pub fn with_bounds(mut self, bounds: CameraBounds) -> Self {
        self.bounds = Some(CameraBounds::new(bounds.min, bounds.max));
        self
    }

    /// The rig showing `height` world units top to bottom.
    ///
    /// # Panics
    ///
    /// When `height` is not positive and finite.
    #[track_caller]
    #[must_use]
    pub fn with_view_height(mut self, height: f32) -> Self {
        positive(height, "a view height");
        self.view_height = Some(height);
        self
    }

    /// The rig's view scaled by `zoom`.
    ///
    /// # Panics
    ///
    /// When `zoom` is not positive and finite.
    #[track_caller]
    #[must_use]
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        positive(zoom, "a rig's zoom");
        self.zoom = zoom;
        self
    }

    /// The rig at `priority`.
    #[must_use]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// The rig blending in by `blend`.
    ///
    /// # Panics
    ///
    /// When the blend's time is negative or not finite.
    #[track_caller]
    #[must_use]
    pub fn with_blend(mut self, blend: RigBlend) -> Self {
        at_least_zero(blend.time, "a blend's time");
        self.blend = blend;
        self
    }

    /// The rig lowered, as a zone's rig starts: no candidate until raised.
    #[must_use]
    pub fn lowered(mut self) -> Self {
        self.lower();
        self
    }

    /// Raises the rig: live, stamped at the next update. [`raise`] stamps
    /// at once instead.
    pub fn raise(&mut self) {
        self.live = true;
        self.stamp = 0;
    }

    /// Lowers the rig, which loses its stamp: no candidate until raised.
    pub fn lower(&mut self) {
        self.live = false;
        self.stamp = 0;
    }

    /// Whether the rig is a candidate.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Moves a scripted rig's point, or a fixed rig's centre or corner; a
    /// follow or a rect rig is left.
    ///
    /// # Panics
    ///
    /// When `point` is not finite.
    #[track_caller]
    pub fn set_point(&mut self, point: Vec2) {
        finite(point, "a rig's point");
        match &mut self.kind {
            RigKind::Scripted(at) | RigKind::Fixed(FixedRig::At(at) | FixedRig::Corner(at)) => {
                *at = point;
            }
            RigKind::Follow(_) | RigKind::Fixed(FixedRig::Rect(_)) => {}
        }
    }

    /// The entity a follow rig names; `None` for another kind or a follow
    /// of its own entity.
    #[must_use]
    pub fn target(&self) -> Option<Entity> {
        match self.kind {
            RigKind::Follow(follow) => follow.target,
            RigKind::Fixed(_) | RigKind::Scripted(_) => None,
        }
    }

    /// The zoom this rig draws at in a window `window.y` pixels tall.
    fn zoom_at(&self, window: Vec2) -> f32 {
        let scale = sane(self.zoom, 1.0);
        let height = self.view_height.map(|height| sane(height, window.y));
        let height = match (height, self.kind) {
            (Some(height), _) => Some(height),
            (None, RigKind::Fixed(FixedRig::Rect(rect))) => {
                let size = rect.max - rect.min;
                let aspect = window.y / window.x.max(1.0);
                Some(sane(size.y.max(size.x * aspect), window.y))
            }
            (None, _) => None,
        };
        let zoom = match height {
            Some(height) => (window.y / height) * scale,
            None => scale,
        };
        zoom.max(f32::EPSILON)
    }
}

/// `value` when positive and finite, else `fallback`: a rig's public field
/// a game wrote out of range draws as its default.
fn sane(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

/// Raises `entity`'s rig now, stamped from the controller, so raises made
/// in one step keep their order and the latest wins a tie. `false` when
/// the entity holds no rig.
pub fn raise(world: &mut World, entity: Entity) -> bool {
    if !world.has::<CameraRig>(entity) {
        return false;
    }
    let stamp = world
        .get_resource_mut::<CameraController>()
        .map_or(0, CameraController::next_stamp);
    let Some(rig) = world.get_mut::<CameraRig>(entity) else {
        return false;
    };
    rig.live = true;
    rig.stamp = stamp;
    true
}

/// Lowers `entity`'s rig, which loses its stamp. `false` when the entity
/// holds no rig.
pub fn lower(world: &mut World, entity: Entity) -> bool {
    let Some(rig) = world.get_mut::<CameraRig>(entity) else {
        return false;
    };
    rig.lower();
    true
}

/// The aim an [`LeadFrom::Aim`] lead reads: the direction `entity` aims in,
/// if any. The kit inserts one from its `Aim`; without one an aim lead
/// stays at zero.
#[derive(Debug, Clone, Copy)]
pub struct CameraAim(pub fn(&World, Entity) -> Option<Vec2>);

/// What the camera's last update drew and judged, for an overlay or a
/// probe.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CameraFrame {
    /// The drawn view's top-left, without the shake, in world units.
    pub corner: Vec2,
    /// The drawn view's size in world units.
    pub size: Vec2,
    /// The winner's dead zone, as a world box, while it follows a target.
    pub dead_zone: Option<CameraBounds>,
    /// The winner's bounds.
    pub bounds: Option<CameraBounds>,
    /// The point the winner keeps: its target's drawn point plus its
    /// offset.
    pub anchor: Option<Vec2>,
    /// The anchor plus the lead, while the winner leads.
    pub lead: Option<Vec2>,
    /// The running blend's share of its time, in [0, 1); `None` with none.
    pub blend: Option<f32>,
}

/// A view: its top-left and its size, in world units, without the shake.
#[derive(Debug, Clone, Copy, PartialEq)]
struct View {
    corner: Vec2,
    size: Vec2,
}

impl View {
    fn centre(self) -> Vec2 {
        self.corner + self.size * 0.5
    }
}

/// A running blend: the view it started from, by centre and height, and
/// its progress.
#[derive(Debug, Clone, Copy)]
struct Blend {
    centre: Vec2,
    height: f32,
    elapsed: f32,
    time: f32,
    ease: Easing,
}

/// What the update keeps between frames.
#[derive(Debug, Clone, Copy, Default)]
struct Runtime {
    /// Whether an update has picked a winner yet.
    started: bool,
    /// The last winner, `None` for the base rig, and its kind.
    winner: Option<Entity>,
    kind: u8,
    /// The winner's own view corner, unshaken and unblended, which a follow
    /// steps from.
    corner: Vec2,
    /// The winner's lead, and its target's drawn point last frame.
    lead: Vec2,
    last_point: Option<(Entity, Vec2)>,
    /// The last drawn view, unshaken.
    last: Option<View>,
    blend: Option<Blend>,
    /// The last stamp handed out.
    stamps: u64,
    /// A cut the next update takes: the winner's view centre.
    cut: Option<Vec2>,
    frame: CameraFrame,
}

/// The camera's settings and runtime: the base rig, the shake, and what
/// the update keeps between frames.
#[derive(Debug, Clone, Copy)]
pub struct CameraController {
    /// The base rig: live under the entities' rigs, at its own priority,
    /// raised before any of them. By default a follow with no target, which
    /// holds the view, so a default World draws its top-left at the origin
    /// at a zoom of 1.
    pub rig: CameraRig,
    pub shake_amplitude: Vec2,
    pub shake_frequency_hz: f32,
    /// Deterministic shake phase.
    pub shake_phase: f32,
    /// M30 trauma level, `0.0..=1.0`. Clamped on write by `add_trauma`;
    /// `shake_tick_system` decays it. `0.0` leaves `shake_offset` at the
    /// pre-M30 sine output.
    pub shake_trauma: f32,
    /// Trauma units shed per second.
    pub shake_decay: f32,
    /// Peak trauma contribution in pixels, reached at `shake_trauma == 1.0`.
    pub shake_max_offset: Vec2,
    /// Scales the whole shake offset: the share a game's Screen shake
    /// option leaves, 1 by default.
    pub shake_scale: f32,
    /// Whether the update draws the winner's shapes into the World's
    /// `DebugDraw`: its dead zone and bounds as boxes, its anchor and led
    /// point as crosses, at the frame's own view. Off by default; the kit's
    /// debug overlay sets it each frame.
    pub draw_shapes: bool,
    runtime: Runtime,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            rig: CameraRig::default(),
            shake_amplitude: Vec2::ZERO,
            shake_frequency_hz: 0.0,
            shake_phase: 0.0,
            shake_trauma: 0.0,
            shake_decay: 1.0,
            shake_max_offset: Vec2::ZERO,
            shake_scale: 1.0,
            draw_shapes: false,
            runtime: Runtime::default(),
        }
    }
}

impl CameraController {
    /// The rig the last update drew: `None` for the base rig.
    #[must_use]
    pub fn winner(&self) -> Option<Entity> {
        self.runtime.winner
    }

    /// What the last update drew and judged.
    #[must_use]
    pub fn frame(&self) -> CameraFrame {
        self.runtime.frame
    }

    /// A cut the next update takes: any blend ends and the winner starts
    /// its view centred on `centre`, at its own height, as a run's start
    /// puts its camera on the play space.
    ///
    /// # Panics
    ///
    /// When `centre` is not finite.
    #[track_caller]
    pub fn cut_to(&mut self, centre: Vec2) {
        finite(centre, "a cut's centre");
        self.runtime.cut = Some(centre);
    }

    fn next_stamp(&mut self) -> u64 {
        self.runtime.stamps += 1;
        self.runtime.stamps
    }

    /// Additive trauma request (M30), saturating at `1.0` rather than wrapping.
    /// Negative amounts are floored at `0.0`.
    pub fn add_trauma(&mut self, amount: f32) {
        self.shake_trauma = (self.shake_trauma + amount).clamp(0.0, 1.0);
    }

    /// Shake offset for the current phase: sine carrier, trauma-squared
    /// envelope (M30, `D-073`), times [`shake_scale`](Self::shake_scale).
    ///
    /// Trauma adds to `shake_amplitude` rather than replacing it, so a
    /// controller with `shake_trauma == 0.0` reproduces the pre-M30 offset
    /// exactly.
    #[must_use]
    pub fn shake_offset(&self) -> Vec2 {
        let trauma = self.shake_trauma.clamp(0.0, 1.0);
        let effective = self.shake_amplitude + self.shake_max_offset * trauma * trauma;
        Vec2::new(
            effective.x * self.shake_phase.sin(),
            effective.y * (self.shake_phase + FRAC_PI_2).sin(),
        ) * self.shake_scale
    }
}

/// The view a follow wants: `current` moved so `target` sits inside the
/// dead zone of `dead_zone` centred in a view of `viewport_size`.
fn solve_follow_position(
    current_position: Vec2,
    target_position: Vec2,
    viewport_size: Vec2,
    dead_zone: Vec2,
) -> Vec2 {
    let mut next = current_position;
    let dead_zone_offset = (viewport_size - dead_zone) * 0.5;
    let dead_zone_min = current_position + dead_zone_offset;
    let dead_zone_max = dead_zone_min + dead_zone;

    if target_position.x < dead_zone_min.x {
        next.x = target_position.x - dead_zone_offset.x;
    } else if target_position.x > dead_zone_max.x {
        next.x = target_position.x - dead_zone_offset.x - dead_zone.x;
    }

    if target_position.y < dead_zone_min.y {
        next.y = target_position.y - dead_zone_offset.y;
    } else if target_position.y > dead_zone_max.y {
        next.y = target_position.y - dead_zone_offset.y - dead_zone.y;
    }

    next
}

/// This frame's blend toward the desired position for smoothing `s`, the
/// fraction of the distance covered per 1/60 s: `1 - (1 - s)^(dt·60)`, so the
/// camera converges at the same rate at any frame rate. 0 never moves and 1
/// snaps, whatever `dt` (B5, `D-100`).
fn smoothing_blend(s: f32, dt: f32) -> f32 {
    if s >= 1.0 {
        1.0
    } else if s <= 0.0 || s.is_nan() {
        0.0
    } else {
        1.0 - (1.0 - s).powf(dt * 60.0)
    }
}

/// `from.lerp(to, s)` per axis in glam's arithmetic, `a·(1 − s) + b·s`, so
/// equal rates draw what one `lerp` drew.
fn lerp_axes(from: Vec2, to: Vec2, s: Vec2) -> Vec2 {
    Vec2::new(
        from.x * (1.0 - s.x) + to.x * s.x,
        from.y * (1.0 - s.y) + to.y * s.y,
    )
}

/// The winning rig, `None` for the base, and a copy of it, stamping the
/// rigs that went live since the last update in query order.
fn pick(world: &mut World, controller: &mut CameraController) -> (Option<Entity>, CameraRig) {
    if !world.has_any::<CameraRig>() {
        return (None, controller.rig);
    }
    let issued = controller.runtime.stamps;
    let mut fresh = 0;
    let mut best = (None::<Entity>, controller.rig, 0_u64);
    for entity in world.query_filtered::<Entity, With<CameraRig>>() {
        let Some(rig) = world.get::<CameraRig>(entity) else {
            continue;
        };
        if !rig.live {
            continue;
        }
        let stamp = if rig.stamp == 0 {
            fresh += 1;
            issued + fresh
        } else {
            rig.stamp
        };
        let (key, best_key) = ((rig.priority, stamp), (best.1.priority, best.2));
        let beats = key > best_key
            || (key == best_key && best.0.is_some_and(|other| entity.id() < other.id()));
        if beats {
            best = (Some(entity), *rig, stamp);
        }
    }
    if fresh > 0 {
        let mut next = issued;
        for rig in world.query_mut::<&mut CameraRig>() {
            if rig.live && rig.stamp == 0 {
                next += 1;
                rig.stamp = next;
            }
        }
        controller.runtime.stamps = next;
        best.1.stamp = best.2;
    }
    (best.0, best.1)
}

/// The window, the rig's zoom, the game's rotation and the frame's `dt`
/// a rig's view is evaluated at.
#[derive(Debug, Clone, Copy)]
struct Eval {
    window: Vec2,
    zoom: f32,
    rotation: f32,
    dt: f32,
}

/// The winner's own view this frame: its follow stepped from where it
/// stood, or its fixed or scripted point, clamped to its bounds at its own
/// height. Fills the frame's dead zone, anchor and lead.
fn rig_view(
    world: &World,
    runtime: &mut Runtime,
    rig: &CameraRig,
    own: Option<Entity>,
    at: Eval,
) -> View {
    let Eval {
        window,
        zoom,
        rotation,
        dt,
    } = at;
    let size = window / zoom;
    let (corner, dead_zone) = match rig.kind {
        RigKind::Follow(follow) => follow_step(world, runtime, &follow, own, size, dt),
        RigKind::Fixed(FixedRig::At(centre)) | RigKind::Scripted(centre) => {
            (centre - size * 0.5, None)
        }
        RigKind::Fixed(FixedRig::Corner(corner)) => (corner, None),
        RigKind::Fixed(FixedRig::Rect(rect)) => (rect.centre() - size * 0.5, None),
    };
    let corner = match rig.bounds {
        Some(bounds) => bounds.clamp_position(corner, window.x, window.y, zoom, rotation),
        None => corner,
    };
    runtime.frame.dead_zone = dead_zone.map(|zone| {
        let min = corner + (size - zone) * 0.5;
        CameraBounds {
            min,
            max: min + zone,
        }
    });
    View { corner, size }
}

/// A follow's corner this frame, stepped from where its view stood toward
/// its target's anchor (its drawn point, offset and led) by `D-100`'s rate
/// on each axis, and the dead zone it judged by; one with no target holds.
fn follow_step(
    world: &World,
    runtime: &mut Runtime,
    follow: &FollowRig,
    own: Option<Entity>,
    size: Vec2,
    dt: f32,
) -> (Vec2, Option<Vec2>) {
    let target = follow.target.or(own);
    let Some((target, point)) =
        target.and_then(|t| world.get::<Transform>(t).map(|at| (t, at.position)))
    else {
        runtime.last_point = None;
        return (runtime.corner, None);
    };
    step_lead(world, runtime, follow.lead, target, point, dt);
    let anchor = point + follow.offset;
    let led = anchor + runtime.lead;
    let zone = follow.dead_zone.clamp(Vec2::ZERO, size);
    let desired = solve_follow_position(runtime.corner, led, size, zone);
    let rate = Vec2::new(
        smoothing_blend(follow.smoothing.x, dt),
        smoothing_blend(follow.smoothing.y, dt),
    );
    runtime.frame.anchor = Some(anchor);
    runtime.frame.lead = follow.lead.map(|_| led);
    runtime.last_point = Some((target, point));
    (lerp_axes(runtime.corner, desired, rate), Some(zone))
}

/// Eases the winner's lead toward its goal; a pause holds it.
fn step_lead(
    world: &World,
    runtime: &mut Runtime,
    lead: Option<RigLead>,
    target: Entity,
    point: Vec2,
    dt: f32,
) {
    let Some(lead) = lead else {
        runtime.lead = Vec2::ZERO;
        return;
    };
    if dt <= 0.0 {
        return;
    }
    let goal = match lead.from {
        LeadFrom::Velocity => world
            .get::<Velocity>(target)
            .map(|velocity| velocity.0)
            .or_else(|| {
                runtime
                    .last_point
                    .filter(|&(entity, _)| entity == target)
                    .map(|(_, last)| (point - last) / dt)
            })
            .map_or(Vec2::ZERO, |velocity| velocity * lead.time),
        LeadFrom::Aim => world
            .get_resource::<CameraAim>()
            .and_then(|aim| (aim.0)(world, target))
            .map_or(Vec2::ZERO, |direction| direction * lead.max),
    };
    let goal = if goal.is_finite() {
        goal.clamp_length_max(lead.max)
    } else {
        Vec2::ZERO
    };
    runtime.lead = runtime.lead.lerp(goal, smoothing_blend(lead.smoothing, dt));
}

/// One frame of the camera, as the engine's `camera_update` system runs it
/// with the window's size in physical pixels and the game clock's `dt`:
/// the winning rig's view, blended when the winner changed, then the
/// shake, written into [`CameraState`] with the game's rotation kept.
/// Returns the state it wrote; `None`, writing nothing, without a
/// `CameraState` or a `CameraController`.
///
/// A rig starts its view at the last one drawn when it wins or its kind
/// changes (that view's corner when the heights match, else its centre),
/// and at a pending [`CameraController::cut_to`]'s centre, winning or not.
/// When the winner changes, the incoming rig's blend moves the last drawn
/// view to the rig's live one, its centre linearly and its height
/// geometrically by the eased share of the blend's time, so the view is a
/// function of the time since the switch; a pause holds that share.
pub fn update(world: &mut World, window: Vec2, dt: f32) -> Option<CameraState> {
    let rotation = world.get_resource::<CameraState>()?.rotation;
    let mut controller = *world.get_resource::<CameraController>()?;
    let (winner, rig) = pick(world, &mut controller);
    let runtime = &mut controller.runtime;
    let kind = rig.kind.tag();
    let won = !runtime.started || winner != runtime.winner;
    let zoom = rig.zoom_at(window);
    let size = window / zoom;
    let cut = runtime.cut.take();
    if let Some(centre) = cut {
        runtime.corner = centre - size * 0.5;
    } else if won || kind != runtime.kind {
        runtime.corner = match runtime.last {
            Some(last) if last.size.y == size.y => last.corner,
            Some(last) => last.centre() - size * 0.5,
            None => Vec2::ZERO,
        };
    }
    if cut.is_some() || won || kind != runtime.kind {
        runtime.lead = Vec2::ZERO;
        runtime.last_point = None;
    }
    runtime.frame = CameraFrame {
        bounds: rig.bounds,
        ..CameraFrame::default()
    };
    let at = Eval {
        window,
        zoom,
        rotation,
        dt,
    };
    let live = rig_view(world, runtime, &rig, winner, at);
    runtime.corner = live.corner;
    if cut.is_some() {
        runtime.blend = None;
    } else if won {
        runtime.blend = runtime
            .last
            .filter(|_| rig.blend.time > 0.0)
            .map(|last| Blend {
                centre: last.centre(),
                height: last.size.y,
                elapsed: 0.0,
                time: rig.blend.time,
                ease: rig.blend.ease,
            });
    }
    let (mut drawn, mut drawn_zoom) = (live, zoom);
    if let Some(mut blend) = runtime.blend.take() {
        blend.elapsed += dt.max(0.0);
        if blend.elapsed < blend.time && blend.height > 0.0 && live.size.y > 0.0 {
            let share = blend.elapsed / blend.time;
            let s = blend.ease.apply(share.clamp(0.0, 1.0));
            let centre = blend.centre + (live.centre() - blend.centre) * s;
            let height = blend.height * (live.size.y / blend.height).powf(s);
            drawn_zoom = (window.y / height).max(f32::EPSILON);
            let size = window / drawn_zoom;
            drawn = View {
                corner: centre - size * 0.5,
                size,
            };
            runtime.frame.blend = Some(share);
            runtime.blend = Some(blend);
        }
    }
    runtime.frame.corner = drawn.corner;
    runtime.frame.size = drawn.size;
    runtime.last = Some(drawn);
    runtime.winner = winner;
    runtime.kind = kind;
    runtime.started = true;

    let shake = controller.shake_offset();
    let state = CameraState {
        position: drawn.corner + shake,
        zoom: drawn_zoom,
        rotation,
        shake,
    };
    controller.shake_phase = (controller.shake_phase
        + controller.shake_frequency_hz.max(0.0) * TAU * dt)
        .rem_euclid(TAU);
    if controller.draw_shapes {
        draw_frame(world, &controller.runtime.frame);
    }
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        *camera = state;
    }
    if let Some(camera) = world.get_resource_mut::<CameraController>() {
        *camera = controller;
    }
    Some(state)
}

/// The camera's debug shapes: its dead zone, anchor and led point, and its
/// bounds.
const SHAPE_COLOR: [f32; 4] = [1.0, 1.0, 0.3, 1.0];
const BOUNDS_COLOR: [f32; 4] = [1.0, 0.5, 0.1, 1.0];
/// A cross's half size and the lines' thickness, in world units.
const CROSS: f32 = 6.0;
const THICKNESS: f32 = 1.0;

/// Draws `frame`'s dead zone and its finite bounds as boxes and its anchor
/// and led point as crosses into the World's `DebugDraw`, when it has one.
fn draw_frame(world: &mut World, frame: &CameraFrame) {
    let Some(draw) = world.get_resource_mut::<DebugDraw>() else {
        return;
    };
    if let Some(zone) = frame.dead_zone {
        draw.draw_aabb(zone.min, zone.max, SHAPE_COLOR, THICKNESS);
    }
    if let Some(bounds) = frame.bounds
        && bounds.min.is_finite()
        && bounds.max.is_finite()
    {
        draw.draw_aabb(bounds.min, bounds.max, BOUNDS_COLOR, THICKNESS);
    }
    for at in [frame.anchor, frame.lead].into_iter().flatten() {
        draw.draw_line(
            at - Vec2::new(CROSS, 0.0),
            at + Vec2::new(CROSS, 0.0),
            SHAPE_COLOR,
            THICKNESS,
        );
        draw.draw_line(
            at - Vec2::new(0.0, CROSS),
            at + Vec2::new(0.0, CROSS),
            SHAPE_COLOR,
            THICKNESS,
        );
    }
}

/// The camera's output: the view the frame draws, which only the camera
/// update writes, each frame from the winning rig; a write lasts until the
/// next update (one that runs no update, such as a benchmark row, keeps
/// it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraState {
    /// Top-left world position, the shake included; preserves pre-M10
    /// pixel ortho.
    pub position: Vec2,
    /// Uniform zoom, applied around top-left.
    pub zoom: f32,
    /// Rotation around top-left anchor: the game's, which the update keeps
    /// and its bounds clamp reads.
    pub rotation: f32,
    /// The shake offset the update added to `position`.
    pub shake: Vec2,
}

impl CameraState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            shake: Vec2::ZERO,
        }
    }

    /// The view less its shake: what a wrap, a clamp to the view or a
    /// despawn outside it judges, so a shake moves none of them.
    #[must_use]
    pub fn unshaken(&self) -> Self {
        Self {
            position: self.position - self.shake,
            shake: Vec2::ZERO,
            ..*self
        }
    }

    /// View-projection matrix for physical viewport size.
    #[must_use]
    pub fn view_projection(&self, viewport_w: f32, viewport_h: f32) -> Mat4 {
        let zoom = self.zoom.max(f32::EPSILON);
        if self.rotation == 0.0 {
            let view_w = viewport_w / zoom;
            let view_h = viewport_h / zoom;
            let left = self.position.x;
            let right = self.position.x + view_w;
            let top = self.position.y;
            let bottom = self.position.y + view_h;
            // WebGPU NDC (right-handed, Z in [0, 1], Y-up): the same matrix the
            // deprecated `Mat4::orthographic_rh` produced.
            return orthographic(left, right, bottom, top, -1.0, 1.0);
        }

        let ortho = orthographic(0.0, viewport_w, viewport_h, 0.0, -1.0, 1.0);
        let scale = Mat4::from_scale(Vec3::new(zoom, zoom, 1.0));
        let rotate = Mat4::from_rotation_z(-self.rotation);
        let translate = Mat4::from_translation(Vec3::new(-self.position.x, -self.position.y, 0.0));
        ortho * scale * rotate * translate
    }

    /// Visible world AABB; rotation returns conservative bounding box.
    #[must_use]
    pub fn visible_world_aabb(&self, viewport_w: f32, viewport_h: f32) -> (Vec2, Vec2) {
        let view_size = viewport_size(viewport_w, viewport_h, self.zoom);
        let (offset_min, offset_max) = rotated_view_extents(view_size, self.rotation);
        (self.position + offset_min, self.position + offset_max)
    }

    /// The world point drawn at `screen` (physical pixels from the window's
    /// top-left): the inverse of [`view_projection`](Self::view_projection),
    /// rotation included. The viewport's size does not enter it, since the
    /// camera is anchored at the top-left.
    #[must_use]
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        let zoom = self.zoom.max(f32::EPSILON);
        self.position + rotate_vec(screen / zoom, self.rotation)
    }

    /// The screen point, in physical pixels from the window's top-left, at
    /// which `world` is drawn: the inverse of
    /// [`screen_to_world`](Self::screen_to_world), rotation included.
    #[must_use]
    pub fn world_to_screen(&self, world: Vec2) -> Vec2 {
        let zoom = self.zoom.max(f32::EPSILON);
        rotate_vec(world - self.position, -self.rotation) * zoom
    }
}

impl Default for CameraState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/camera.rs"]
mod tests;
