//! Facing: the direction an entity faces, from its motion, its intent or
//! its aim, snapped and held while it stops.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, PrevPosition, Velocity};
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Time, Transform, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHMUP, TOP_DOWN,
    describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{finite, non_negative, vec2};
use crate::intent::action_input::Intent;
use crate::intent::aim::Aim;

/// [`FacingPlugin`]'s system, in the present phase.
pub const FACING: &str = "facing";

/// The scene component the plugin registers.
const FACING_COMPONENT: &str = "facing";

/// The default speed below which a moving entity keeps facing, in world
/// pixels per second.
const DEFAULT_MIN_SPEED: f32 = 1.0;

/// Where a facing takes its direction from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FacingFrom {
    #[default]
    Motion,
    Intent,
    Aim,
}

/// How a facing snaps its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Snap {
    #[default]
    Free,
    Horizontal,
    Four,
    Eight,
}

/// The direction its entity faces, a unit vector each frame: which way a
/// character looks, a ship's heading, a gun's barrel. [`FacingPlugin`]'s
/// system takes it from one source:
///
/// - `motion`, the entity's velocity: its `Velocity`, else its last step's
///   move (`Position − PrevPosition`) over the step;
/// - `intent`, its [`Intent`](crate::Intent)'s movement;
/// - `aim`, its [`Aim`](crate::Aim)'s direction.
///
/// It holds while the speed is under its minimum or the source gives
/// nothing, so a stopped character keeps facing the way it went, then
/// snaps: `free` keeps the direction, `four` and `eight` round it to the
/// nearest of four or eight, and `horizontal` to left or right, keeping
/// the last side while it moves straight up or down. It starts facing
/// right.
///
/// With `turn`, it writes `Transform.rotation` as the direction's angle
/// plus its offset, so art drawn facing up turns with an offset of `π/2`; a
/// rotation tween or an oscillator on the same entity is hidden. It never
/// flips a sprite: a [`CharacterAnimator`](crate::CharacterAnimator)'s clip
/// sets show mirrored art. It reads no gameplay's state and gameplay reads
/// none of its, and it works on children.
///
/// Scene name `facing`:
/// `{ "from": "motion", "snap": "free", "turn": false, "offset": 0, "min_speed": 1 }`,
/// `from` one of `motion` (default), `intent` and `aim`, `snap` one of
/// `free` (default), `horizontal`, `four` and `eight`, `offset` in radians
/// and `min_speed` in world pixels per second, each with those defaults.
/// It is data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use std::f32::consts::FRAC_PI_2;
///
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition, Velocity};
/// use tungsten_core::{Plugin, Schedule, Stage, Transform, World};
/// use tungsten_kit::{Facing, FacingPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// FacingPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A ship drawn pointing up, turned to its heading in eight steps.
/// let ship = world.spawn_with((
///     Facing::new().eight_way().turning(FRAC_PI_2),
///     Transform::default(),
///     Velocity(Vec2::new(30.0, -40.0)),
/// ));
/// assert_eq!(world.get::<Facing>(ship).unwrap().direction(), Vec2::X, "it starts facing right");
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// let up_right = Vec2::new(1.0, -1.0).normalize();
/// assert!((world.get::<Facing>(ship).unwrap().direction() - up_right).length() < 1e-6);
/// assert!((world.get::<Transform>(ship).unwrap().rotation - FRAC_PI_2 / 2.0).abs() < 1e-6);
///
/// // Stopped, it keeps facing the way it went.
/// world.get_mut::<Velocity>(ship).unwrap().0 = Vec2::ZERO;
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert!((world.get::<Facing>(ship).unwrap().direction() - up_right).length() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FacingFields", into = "FacingFields")]
pub struct Facing {
    from: FacingFrom,
    snap: Snap,
    turn: bool,
    offset: f32,
    min_speed: f32,
    /// The unit direction at its last frame.
    direction: Vec2,
}

/// A facing as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FacingFields {
    #[serde(default)]
    from: FacingFrom,
    #[serde(default)]
    snap: Snap,
    #[serde(default)]
    turn: bool,
    #[serde(default)]
    offset: f32,
    #[serde(default = "default_min_speed")]
    min_speed: f32,
    #[serde(default = "right", with = "vec2")]
    direction: Vec2,
}

fn default_min_speed() -> f32 {
    DEFAULT_MIN_SPEED
}

fn right() -> Vec2 {
    Vec2::X
}

impl TryFrom<FacingFields> for Facing {
    type Error = String;

    fn try_from(fields: FacingFields) -> Result<Self, Self::Error> {
        if !fields.offset.is_finite() {
            return Err(format!("offset: must be finite (got {})", fields.offset));
        }
        if !(fields.min_speed.is_finite() && fields.min_speed >= 0.0) {
            return Err(format!(
                "min_speed: must be finite and >= 0 (got {})",
                fields.min_speed
            ));
        }
        if !(fields.direction.is_finite() && (fields.direction.length() - 1.0).abs() <= 1e-3) {
            return Err(format!(
                "direction: must be a unit vector (got {})",
                fields.direction
            ));
        }
        Ok(Self {
            from: fields.from,
            snap: fields.snap,
            turn: fields.turn,
            offset: fields.offset,
            min_speed: fields.min_speed,
            direction: fields.direction,
        })
    }
}

impl From<Facing> for FacingFields {
    fn from(facing: Facing) -> Self {
        Self {
            from: facing.from,
            snap: facing.snap,
            turn: facing.turn,
            offset: facing.offset,
            min_speed: facing.min_speed,
            direction: facing.direction,
        }
    }
}

impl Default for Facing {
    fn default() -> Self {
        Self::new()
    }
}

impl Facing {
    /// A facing from the entity's motion, unsnapped, turning nothing, held
    /// under 1 world pixel per second, facing right.
    #[must_use]
    pub fn new() -> Self {
        Self {
            from: FacingFrom::Motion,
            snap: Snap::Free,
            turn: false,
            offset: 0.0,
            min_speed: DEFAULT_MIN_SPEED,
            direction: Vec2::X,
        }
    }

    /// The facing, from the entity's [`Intent`](crate::Intent)'s movement.
    #[must_use]
    pub fn from_intent(mut self) -> Self {
        self.from = FacingFrom::Intent;
        self
    }

    /// The facing, from the entity's [`Aim`](crate::Aim)'s direction.
    #[must_use]
    pub fn from_aim(mut self) -> Self {
        self.from = FacingFrom::Aim;
        self
    }

    /// The facing, snapped to left or right, the last side kept while the
    /// direction is straight up or down.
    #[must_use]
    pub fn horizontal(mut self) -> Self {
        self.snap = Snap::Horizontal;
        self
    }

    /// The facing, snapped to the nearest of right, down, left and up.
    #[must_use]
    pub fn four_way(mut self) -> Self {
        self.snap = Snap::Four;
        self
    }

    /// The facing, snapped to the nearest of eight directions, 45° apart.
    #[must_use]
    pub fn eight_way(mut self) -> Self {
        self.snap = Snap::Eight;
        self
    }

    /// The facing, writing `Transform.rotation` each frame as the
    /// direction's angle plus `offset` radians.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn turning(mut self, offset: f32) -> Self {
        assert!(
            offset.is_finite(),
            "Facing::turning: offset must be finite (got {offset})"
        );
        self.turn = true;
        self.offset = offset;
        self
    }

    /// The facing, held while the entity moves slower than `speed` world
    /// pixels per second.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_min_speed(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Facing::with_min_speed: speed must be finite and >= 0 (got {speed})"
        );
        self.min_speed = speed;
        self
    }

    /// The unit direction it faces, snapped: right until its first frame
    /// that gives one.
    #[must_use]
    pub fn direction(&self) -> Vec2 {
        self.direction
    }

    /// `input` snapped as the facing snaps, or `None` to hold: no input, or
    /// a motion under the minimum speed.
    fn snapped(&self, input: Vec2) -> Option<Vec2> {
        let floor = match self.from {
            FacingFrom::Motion => self.min_speed.max(f32::EPSILON),
            FacingFrom::Intent | FacingFrom::Aim => f32::EPSILON,
        };
        if !input.is_finite() || input.length() < floor {
            return None;
        }
        Some(match self.snap {
            Snap::Free => input.normalize(),
            Snap::Horizontal if input.x == 0.0 => Vec2::new(self.direction.x.signum(), 0.0),
            Snap::Horizontal => Vec2::new(input.x.signum(), 0.0),
            Snap::Four if input.x.abs() >= input.y.abs() => Vec2::new(input.x.signum(), 0.0),
            Snap::Four => Vec2::new(0.0, input.y.signum()),
            Snap::Eight => {
                let step = std::f32::consts::FRAC_PI_4;
                Vec2::from_angle((input.to_angle() / step).round() * step)
            }
        })
    }
}

/// Registers the `facing` scene component and [`FACING`] in the present
/// phase. Its system sets every [`Facing`]'s direction once a frame, and
/// its entity's rotation when it turns.
#[derive(Debug, Default, Clone, Copy)]
pub struct FacingPlugin;

impl Plugin for FacingPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Facing>(world, "Facing");
        describe(
            world,
            ComponentDescription::new(
                FACING_COMPONENT,
                "The direction the entity faces, from its motion, intent or aim, snapped and held while it stops; it turns the entity when asked.",
            )
            .field(
                FieldDescription::new(
                    "from",
                    FieldType::Enum(&["motion", "intent", "aim"]),
                    "the velocity, the intent's movement or the aim's direction",
                )
                .default("\"motion\""),
            )
            .field(
                FieldDescription::new(
                    "snap",
                    FieldType::Enum(&["free", "horizontal", "four", "eight"]),
                    "unsnapped, left or right, or the nearest of four or eight",
                )
                .default("\"free\""),
            )
            .field(
                FieldDescription::new(
                    "turn",
                    FieldType::Bool,
                    "writes the entity's rotation as the direction's angle plus `offset`",
                )
                .default("false"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::F32 {
                        min: Bound::Open,
                        max: Bound::Open,
                    },
                    "radians added to the angle it turns to: π/2 for art drawn facing up",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "min_speed",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second under which it keeps facing",
                )
                .default("1.0"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample("{}")
            .roundtrip(round_trip::<FacingDef>),
        );
        world.register_component_with(FACING_COMPONENT, |def: FacingDef, _: &SpawnContext<'_>| {
            let mut facing = Facing::new().with_min_speed(def.min_speed);
            facing.from = def.from;
            facing.snap = def.snap;
            if def.turn {
                facing = facing.turning(def.offset);
            } else {
                facing.offset = def.offset;
            }
            (facing,)
        });
        let mut entities = Vec::new();
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(FACING, move |world: &mut World| {
                facing(world, &mut entities);
            })),
        );
    }
}

/// `components.facing` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FacingDef {
    #[serde(default)]
    from: FacingFrom,
    #[serde(default)]
    snap: Snap,
    #[serde(default)]
    turn: bool,
    #[serde(default, deserialize_with = "finite")]
    offset: f32,
    #[serde(default = "default_min_speed", deserialize_with = "non_negative")]
    min_speed: f32,
}

/// Sets every facing's direction from its source, and its entity's
/// rotation when it turns; `entities` is the frame's scratch list.
fn facing(world: &mut World, entities: &mut Vec<Entity>) {
    let step = world.get_resource::<Time>().map_or(0.0, Time::fixed_step);
    entities.clear();
    entities.extend(world.query::<(Entity, &Facing)>().map(|(entity, _)| entity));
    for &entity in entities.iter() {
        let facing = *world.get::<Facing>(entity).unwrap();
        let input = match facing.from {
            FacingFrom::Motion => motion_of(world, entity, step),
            FacingFrom::Intent => world.get::<Intent>(entity).map(Intent::movement),
            FacingFrom::Aim => world.get::<Aim>(entity).and_then(Aim::direction),
        };
        let direction = input
            .and_then(|input| facing.snapped(input))
            .unwrap_or(facing.direction);
        world.get_mut::<Facing>(entity).unwrap().direction = direction;
        if facing.turn
            && let Some(transform) = world.get_mut::<Transform>(entity)
        {
            transform.rotation = direction.to_angle() + facing.offset;
        }
    }
}

/// An entity's velocity: its `Velocity`, else its last step's move over the
/// step.
fn motion_of(world: &World, entity: Entity, step: f32) -> Option<Vec2> {
    if let Some(velocity) = world.get::<Velocity>(entity) {
        return Some(velocity.0);
    }
    let moved = world.get::<Position>(entity)?.0 - world.get::<PrevPosition>(entity)?.0;
    (step > 0.0).then(|| moved / step)
}

impl Inspectable for Facing {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let from = match self.from {
            FacingFrom::Motion => "motion",
            FacingFrom::Intent => "intent",
            FacingFrom::Aim => "aim",
        };
        let snap = match self.snap {
            Snap::Free => "free",
            Snap::Horizontal => "horizontal",
            Snap::Four => "four",
            Snap::Eight => "eight",
        };
        let turn = if self.turn {
            format!(", turning +{:.2} rad", self.offset)
        } else {
            String::new()
        };
        vec![
            ("source", format!("{from}, snap {snap}{turn}")),
            ("direction", point(self.direction)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/facing.rs"]
mod tests;
