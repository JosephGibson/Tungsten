//! Stay in view: an entity's box clamped inside an area, its outward speed
//! stopped.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, Velocity};
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Transform, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, SHMUP, describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::vec2;
use crate::motion::area::{Area, check_size, unshaken_view};
use crate::motion::drive::{Skip, Warned};
use crate::motion::platformer::PlatformerController;
use crate::transform::hierarchy::Parent;

/// [`StayInViewPlugin`]'s system, in the overlap phase.
pub const STAY_IN_VIEW: &str = "stay_in_view";

/// The scene component the plugin registers.
const STAY_IN_VIEW_COMPONENT: &str = "stay_in_view";

/// Keeps its entity's box inside an area: a fighter that cannot fly off
/// the screen, a warden held in its room, a cursor kept on the board.
/// After the physics step, [`StayInViewPlugin`]'s system clamps the
/// entity's box (its point, its `Position` else its `Transform.position`,
/// to the point plus its size) inside the area, and on an axis it clamped
/// zeroes the outward part of its `Velocity` and of a
/// [`PlatformerController`](crate::PlatformerController)'s own velocity. A
/// clamp keeps `PrevPosition`, so the entity is drawn sliding into the
/// edge. It moves a body of any kind, or an entity's `Transform.position`
/// when it has no `Position`; a child is skipped, with one warning. A box
/// larger than the area sits on its top-left side.
///
/// The area is a world-space box, or the camera's view at the last frame
/// without its shake, so a shake never moves a clamped entity, shrunk by a
/// margin.
///
/// Scene name `stay_in_view`: `{ "view": 0 }` or
/// `{ "region": [[0, 0], [640, 480]] }`, exactly one, with `size` the
/// entity's box from its point in world pixels, `[0, 0]` by default. It is
/// data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition, Velocity};
/// use tungsten_core::{CameraState, Plugin, Schedule, Stage, Time, WindowSize, World};
/// use tungsten_kit::{StayInView, StayInViewPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(CameraState::new());
/// world.insert_resource(WindowSize { width: 640, height: 360 });
/// let mut schedule = Schedule::new();
/// StayInViewPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A 24 px fighter kept 8 px inside the camera's view, flying out of it.
/// let held = StayInView::view(8.0).with_size(Vec2::splat(24.0));
/// let fighter = world.spawn_with((
///     held,
///     Position::new(700.0, 100.0),
///     PrevPosition(Vec2::new(600.0, 100.0)),
///     Velocity(Vec2::new(300.0, -50.0)),
/// ));
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// // Its box's right side on the margin, its outward speed stopped.
/// assert_eq!(world.get::<Position>(fighter).unwrap().0, Vec2::new(608.0, 100.0));
/// assert_eq!(world.get::<Velocity>(fighter).unwrap().0, Vec2::new(0.0, -50.0));
///
/// let saved = serde_json::to_value(held).unwrap();
/// assert_eq!(saved, serde_json::json!({ "view": 8.0, "size": [24.0, 24.0] }));
/// assert_eq!(serde_json::from_value::<StayInView>(saved).unwrap(), held);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "StayInViewDef", into = "StayInViewDef")]
pub struct StayInView {
    area: Area,
    size: Vec2,
}

impl StayInView {
    /// A clamp inside the closed box `min..=max`, in world pixels, for a
    /// point.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite, or `min` lies above `max` on an
    /// axis.
    #[must_use]
    #[track_caller]
    pub fn region(min: Vec2, max: Vec2) -> Self {
        Self {
            area: Area::new_region(min, max, "StayInView::region"),
            size: Vec2::ZERO,
        }
    }

    /// A clamp inside the camera's view shrunk by `margin` world pixels,
    /// for a point.
    ///
    /// # Panics
    ///
    /// When `margin` is negative or not finite.
    #[must_use]
    #[track_caller]
    pub fn view(margin: f32) -> Self {
        Self {
            area: Area::new_view(margin, "StayInView::view"),
            size: Vec2::ZERO,
        }
    }

    /// The clamp, for a box of `size` world pixels from the entity's point.
    ///
    /// # Panics
    ///
    /// When `size` is not finite and at least 0 on each axis.
    #[must_use]
    #[track_caller]
    pub fn with_size(mut self, size: Vec2) -> Self {
        if let Err(error) = check_size(size) {
            panic!("StayInView::with_size: {error}");
        }
        self.size = size;
        self
    }

    /// The box, when its area is a region, for the debug drawing.
    pub(crate) fn region_bounds(&self) -> Option<(Vec2, Vec2)> {
        self.area.region()
    }

    /// `point` with its box clamped inside `min..=max`, a box larger than the
    /// area on its top-left side, and the side it was clamped on per axis:
    /// -1 at the low side, 1 at the high, 0 unclamped.
    fn clamp(&self, point: Vec2, min: Vec2, max: Vec2) -> (Vec2, Vec2) {
        let high = (max - self.size).max(min);
        let clamped = point.clamp(min, high);
        let side = Vec2::new(side(point.x, clamped.x), side(point.y, clamped.y));
        (clamped, side)
    }
}

/// The side a clamp moved `from` to `to` on one axis: -1 up to the low
/// side, 1 down to the high, 0 not at all.
fn side(from: f32, to: f32) -> f32 {
    if to > from {
        -1.0
    } else if to < from {
        1.0
    } else {
        0.0
    }
}

/// `velocity` without its part toward `side`: on an axis clamped at the
/// high side its positive part, at the low side its negative.
fn stop_outward(velocity: Vec2, side: Vec2) -> Vec2 {
    let axis = |v: f32, side: f32| if v * side > 0.0 { 0.0 } else { v };
    Vec2::new(axis(velocity.x, side.x), axis(velocity.y, side.y))
}

/// Registers the `stay_in_view` scene component and [`STAY_IN_VIEW`] in the
/// overlap phase: each fixed step after `physics_step`, it clamps every
/// [`StayInView`] entity's box inside its area.
#[derive(Debug, Default, Clone, Copy)]
pub struct StayInViewPlugin;

impl Plugin for StayInViewPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<StayInView>(world, "StayInView");
        describe(
            world,
            ComponentDescription::new(
                STAY_IN_VIEW_COMPONENT,
                "Clamps the entity's box inside an area and stops its outward speed; exactly one of `region` and `view`.",
            )
            .field(
                FieldDescription::new(
                    "region",
                    FieldType::Object("[[min_x, min_y], [max_x, max_y]]"),
                    "the closed box, in world pixels",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "view",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "the margin inside the last frame's camera view, without its shake",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "size",
                    FieldType::Vec2,
                    "the entity's box from its point, in world pixels",
                )
                .default("[0.0, 0.0]"),
            )
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "view": 0 }"#)
            .roundtrip(round_trip::<StayInViewDef>),
        );
        world.register_component_with(
            STAY_IN_VIEW_COMPONENT,
            |def: StayInViewDef, _: &SpawnContext<'_>| (StayInView::from(def),),
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(STAY_IN_VIEW, move |world: &mut World| {
                runner.run(world);
            })),
        );
    }
}

/// `components.stay_in_view` as a scene and a save write it, before its
/// area and size are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StayInViewFields {
    #[serde(default)]
    region: Option<[[f32; 2]; 2]>,
    #[serde(default)]
    view: Option<f32>,
    #[serde(default, with = "vec2")]
    size: Vec2,
}

/// `components.stay_in_view`, checked: exactly one area and a size of at
/// least 0.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "StayInViewFields")]
struct StayInViewDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<[[f32; 2]; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<f32>,
    #[serde(with = "vec2")]
    size: Vec2,
}

impl TryFrom<StayInViewFields> for StayInViewDef {
    type Error = String;

    fn try_from(fields: StayInViewFields) -> Result<Self, Self::Error> {
        Area::from_fields(fields.region, fields.view)?;
        check_size(fields.size)?;
        Ok(Self {
            region: fields.region,
            view: fields.view,
            size: fields.size,
        })
    }
}

impl From<StayInViewDef> for StayInView {
    fn from(def: StayInViewDef) -> Self {
        Self {
            area: Area::from_fields(def.region, def.view).expect("checked at deserialization"),
            size: def.size,
        }
    }
}

impl From<StayInView> for StayInViewDef {
    fn from(held: StayInView) -> Self {
        let (region, view) = held.area.fields();
        Self {
            region,
            view,
            size: held.size,
        }
    }
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's clamps: the entity, its clamped point and the sides per
    /// axis, `None` for a child; kept so a step allocates nothing once warm.
    clamps: Vec<(Entity, Option<(Vec2, Vec2)>)>,
    warned: Warned,
}

impl Runner {
    /// Clamps every held entity's box inside its area and stops its
    /// outward speed on each clamped axis.
    fn run(&mut self, world: &mut World) {
        let view = unshaken_view(world);
        self.clamps.clear();
        for (entity, held, position, transform, parent) in world.query::<(
            Entity,
            &StayInView,
            Option<&Position>,
            Option<&Transform>,
            Option<&Parent>,
        )>() {
            if parent.is_some() {
                self.clamps.push((entity, None));
                continue;
            }
            let Some(point) = position.map(|p| p.0).or(transform.map(|t| t.position)) else {
                continue;
            };
            let Some((min, max)) = held.area.bounds(view, false) else {
                continue;
            };
            let (clamped, side) = held.clamp(point, min, max);
            if side != Vec2::ZERO {
                self.clamps.push((entity, Some((clamped, side))));
            }
        }
        for &(entity, clamp) in &self.clamps {
            let Some((clamped, side)) = clamp else {
                self.warned.skip("StayInView", entity, Skip::Child);
                continue;
            };
            if let Some(position) = world.get_mut::<Position>(entity) {
                position.0 = clamped;
            } else if let Some(transform) = world.get_mut::<Transform>(entity) {
                transform.position = clamped;
            }
            if let Some(velocity) = world.get_mut::<Velocity>(entity) {
                velocity.0 = stop_outward(velocity.0, side);
            }
            if let Some(controller) = world.get_mut::<PlatformerController>(entity) {
                let velocity = controller.velocity_mut();
                *velocity = stop_outward(*velocity, side);
            }
        }
    }
}

impl Inspectable for StayInView {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("area", self.area.summary()), ("size", point(self.size))]
    }
}

#[cfg(test)]
#[path = "../tests/stay_in_view.rs"]
mod tests;
