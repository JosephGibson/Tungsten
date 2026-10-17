//! Wrap: an entity that leaves an area past one side comes back in from
//! the other.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, PrevPosition};
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Transform, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::vec2;
use crate::motion::area::{Area, check_size, unshaken_view};
use crate::motion::drive::{Skip, Warned};
use crate::transform::hierarchy::Parent;

/// [`WrapPlugin`]'s system, in the overlap phase.
pub const WRAP: &str = "wrap";

/// The scene component the plugin registers.
const WRAP_COMPONENT: &str = "wrap";

/// The axes a wrap moves its entity on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Axes {
    #[default]
    Both,
    X,
    Y,
}

/// Brings its entity back in from the far side once it leaves an area past
/// one side: an asteroid round the arena, a rover through a side tunnel, a
/// screen-wrapping ship. After the physics step, [`WrapPlugin`]'s system
/// tests the entity's box (its point, its `Position` else its
/// `Transform.position`, to the point plus its size) against the area: once
/// the box has wholly left past one side, the entity moves by the area's
/// extent plus its own size on that axis, so it comes back in from the
/// other side just outside it, and slides in. A wrap is a teleport: it
/// writes `PrevPosition` with `Position`, so the physics sync draws the
/// entity at the far side, never between. It moves a body of any kind, or
/// an entity's `Transform.position` when it has no `Position`; a child is
/// skipped, with one warning.
///
/// The area is a world-space box, or the camera's view at the last frame
/// without its shake, so a shake never wraps an entity, grown by a margin.
///
/// Scene name `wrap`: `{ "region": [[0, 0], [1280, 720]] }` or
/// `{ "view": 0 }`, exactly one, as `despawn_outside` takes them, with
/// `size` the entity's box from its point in world pixels, `[0, 0]` by
/// default, and `axes` one of `both` (default), `x` and `y`. It is data: a
/// game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Wrap, WrapPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// WrapPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // An asteroid 32 px across that wraps a 1280 × 720 arena side to side,
/// // just past the right side.
/// let wrap = Wrap::region(Vec2::ZERO, Vec2::new(1280.0, 720.0))
///     .with_size(Vec2::splat(32.0))
///     .only_x();
/// let rock = world.spawn_with((wrap, Position::new(1281.0, 300.0), PrevPosition(Vec2::new(1279.0, 300.0))));
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// // In again from the left, just outside it, and drawn from there.
/// assert_eq!(world.get::<Position>(rock).unwrap().0, Vec2::new(-31.0, 300.0));
/// assert_eq!(world.get::<PrevPosition>(rock).unwrap().0, Vec2::new(-33.0, 300.0));
///
/// let saved = serde_json::to_value(wrap).unwrap();
/// assert_eq!(saved["axes"], "x");
/// assert_eq!(serde_json::from_value::<Wrap>(saved).unwrap(), wrap);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "WrapDef", into = "WrapDef")]
pub struct Wrap {
    area: Area,
    size: Vec2,
    axes: Axes,
}

impl Wrap {
    /// A wrap through the sides of the closed box `min..=max`, in world
    /// pixels, on both axes, for a point.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite, or `min` lies above `max` on an
    /// axis.
    #[must_use]
    #[track_caller]
    pub fn region(min: Vec2, max: Vec2) -> Self {
        Self {
            area: Area::new_region(min, max, "Wrap::region"),
            size: Vec2::ZERO,
            axes: Axes::Both,
        }
    }

    /// A wrap through the sides of the camera's view grown by `margin`
    /// world pixels, on both axes, for a point.
    ///
    /// # Panics
    ///
    /// When `margin` is negative or not finite.
    #[must_use]
    #[track_caller]
    pub fn view(margin: f32) -> Self {
        Self {
            area: Area::new_view(margin, "Wrap::view"),
            size: Vec2::ZERO,
            axes: Axes::Both,
        }
    }

    /// The wrap, for a box of `size` world pixels from the entity's point.
    ///
    /// # Panics
    ///
    /// When `size` is not finite and at least 0 on each axis.
    #[must_use]
    #[track_caller]
    pub fn with_size(mut self, size: Vec2) -> Self {
        if let Err(error) = check_size(size) {
            panic!("Wrap::with_size: {error}");
        }
        self.size = size;
        self
    }

    /// The wrap, through the left and right sides only.
    #[must_use]
    pub fn only_x(mut self) -> Self {
        self.axes = Axes::X;
        self
    }

    /// The wrap, through the top and bottom sides only.
    #[must_use]
    pub fn only_y(mut self) -> Self {
        self.axes = Axes::Y;
        self
    }

    /// The box, when its area is a region, for the debug drawing.
    pub(crate) fn region_bounds(&self) -> Option<(Vec2, Vec2)> {
        self.area.region()
    }

    /// How far a box at `point` moves to come back in through `min..=max`:
    /// on each axis it wraps through, the area's extent plus its size,
    /// toward the side it left, once it has wholly left.
    fn shift(&self, point: Vec2, min: Vec2, max: Vec2) -> Vec2 {
        let span = max - min + self.size;
        let mut shift = Vec2::ZERO;
        if self.axes != Axes::Y {
            if point.x > max.x {
                shift.x = -span.x;
            } else if point.x + self.size.x < min.x {
                shift.x = span.x;
            }
        }
        if self.axes != Axes::X {
            if point.y > max.y {
                shift.y = -span.y;
            } else if point.y + self.size.y < min.y {
                shift.y = span.y;
            }
        }
        shift
    }
}

/// Registers the `wrap` scene component and [`WRAP`] in the overlap phase:
/// each fixed step after `physics_step`, it brings back every [`Wrap`]
/// entity whose box has left its area.
#[derive(Debug, Default, Clone, Copy)]
pub struct WrapPlugin;

impl Plugin for WrapPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Wrap>(world, "Wrap");
        describe(
            world,
            ComponentDescription::new(
                WRAP_COMPONENT,
                "Brings the entity back in from the far side once its box leaves an area; exactly one of `region` and `view`.",
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
                    "the margin outside the last frame's camera view, without its shake",
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
            .field(
                FieldDescription::new(
                    "axes",
                    FieldType::Enum(&["both", "x", "y"]),
                    "the sides it wraps through",
                )
                .default("\"both\""),
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample(r#"{ "view": 0 }"#)
            .roundtrip(round_trip::<WrapDef>),
        );
        world.register_component_with(WRAP_COMPONENT, |def: WrapDef, _: &SpawnContext<'_>| {
            (Wrap::from(def),)
        });
        let mut runner = Runner::default();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(WRAP, move |world: &mut World| runner.run(world))),
        );
    }
}

/// `components.wrap` as a scene and a save write it, before its area and
/// size are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrapFields {
    #[serde(default)]
    region: Option<[[f32; 2]; 2]>,
    #[serde(default)]
    view: Option<f32>,
    #[serde(default, with = "vec2")]
    size: Vec2,
    #[serde(default)]
    axes: Axes,
}

/// `components.wrap`, checked: exactly one area and a size of at least 0.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "WrapFields")]
struct WrapDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<[[f32; 2]; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<f32>,
    #[serde(with = "vec2")]
    size: Vec2,
    axes: Axes,
}

impl TryFrom<WrapFields> for WrapDef {
    type Error = String;

    fn try_from(fields: WrapFields) -> Result<Self, Self::Error> {
        Area::from_fields(fields.region, fields.view)?;
        check_size(fields.size)?;
        Ok(Self {
            region: fields.region,
            view: fields.view,
            size: fields.size,
            axes: fields.axes,
        })
    }
}

impl From<WrapDef> for Wrap {
    fn from(def: WrapDef) -> Self {
        Self {
            area: Area::from_fields(def.region, def.view).expect("checked at deserialization"),
            size: def.size,
            axes: def.axes,
        }
    }
}

impl From<Wrap> for WrapDef {
    fn from(wrap: Wrap) -> Self {
        let (region, view) = wrap.area.fields();
        Self {
            region,
            view,
            size: wrap.size,
            axes: wrap.axes,
        }
    }
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's wraps and their shifts, `None` for a child, kept so a
    /// step allocates nothing once warm.
    moves: Vec<(Entity, Option<Vec2>)>,
    warned: Warned,
}

impl Runner {
    /// Brings back every wrapped entity whose box has left its area: a
    /// teleport, `PrevPosition` moved with `Position`.
    fn run(&mut self, world: &mut World) {
        let view = unshaken_view(world);
        self.moves.clear();
        for (entity, wrap, position, transform, parent) in world.query::<(
            Entity,
            &Wrap,
            Option<&Position>,
            Option<&Transform>,
            Option<&Parent>,
        )>() {
            if parent.is_some() {
                self.moves.push((entity, None));
                continue;
            }
            let Some(point) = position.map(|p| p.0).or(transform.map(|t| t.position)) else {
                continue;
            };
            let Some((min, max)) = wrap.area.bounds(view, true) else {
                continue;
            };
            let shift = wrap.shift(point, min, max);
            if shift != Vec2::ZERO {
                self.moves.push((entity, Some(shift)));
            }
        }
        for &(entity, shift) in &self.moves {
            let Some(shift) = shift else {
                self.warned.skip("Wrap", entity, Skip::Child);
                continue;
            };
            if let Some(position) = world.get_mut::<Position>(entity) {
                position.0 += shift;
                if let Some(prev) = world.get_mut::<PrevPosition>(entity) {
                    prev.0 += shift;
                }
            } else if let Some(transform) = world.get_mut::<Transform>(entity) {
                transform.position += shift;
            }
        }
    }
}

impl Inspectable for Wrap {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let axes = match self.axes {
            Axes::Both => "both",
            Axes::X => "x",
            Axes::Y => "y",
        };
        vec![
            ("area", self.area.summary()),
            ("size", point(self.size)),
            ("axes", axes.to_string()),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/wrap.rs"]
mod tests;
