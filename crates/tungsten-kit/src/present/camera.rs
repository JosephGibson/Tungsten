//! The camera in data: `camera_target`, a marker the camera's base rig
//! follows, and `camera_rig`, a rig on its entity (redo 7.18).

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::camera::CameraAim;
use tungsten_core::ecs::bundle::BundleSink;
use tungsten_core::{
    Bundle, CameraBounds, CameraController, CameraRig, Easing, Entity, FixedRig, FollowRig,
    Inspectable, LeadFrom, Plugin, RigBlend, RigKind, RigLead, Schedule, SpawnContext, With, World,
    system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, SHOOTER, TOP_DOWN,
    describe, describe_shape, round_trip,
};
use crate::foundation::inspect::{register_rows, register_rows_with};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::valid_tag;
use crate::intent::aim::Aim;

/// [`CameraTargetPlugin`]'s system, in the present phase.
pub const CAMERA_TARGET: &str = "camera_target";

/// A marker: while an entity carries it, the camera's base rig follows the
/// lowest-indexed live one, the follow rig's shorthand. [`CameraTargetPlugin`]'s
/// system writes that entity into the base rig's follow `target` while the
/// base rig (`CameraController::rig`) is a follow whose target is none or
/// the one it set; a base rig the game made fixed or scripted, or a follow
/// of the game's own choosing, is left alone. Remove the marker, or despawn
/// the entity, and a target still the plugin's is cleared, so the base rig
/// holds where it stands.
///
/// Scene name `camera_target`: `"camera_target": null`.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{CameraController, Plugin, Schedule, Stage, Transform, World};
/// use tungsten_kit::{CameraTarget, CameraTargetPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(CameraController::default());
/// let mut schedule = Schedule::new();
/// CameraTargetPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let hero = world.spawn_with((Transform::from_position(Vec2::ZERO), CameraTarget));
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.resource::<CameraController>().rig.target(), Some(hero));
///
/// world.remove_component::<CameraTarget>(hero);
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert_eq!(world.resource::<CameraController>().rig.target(), None);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraTarget;

/// `components.camera_target` as a scene writes it: `null`.
#[derive(Serialize, Deserialize)]
struct CameraTargetDef;

/// Registers the `camera_target` scene component and [`CAMERA_TARGET`] in
/// the present phase, before `camera_update`, so the base rig follows the
/// target the same frame. Without a `CameraController` resource it does
/// nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct CameraTargetPlugin;

impl Plugin for CameraTargetPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<CameraTarget>(world, "CameraTarget");
        describe(
            world,
            ComponentDescription::new(
                CAMERA_TARGET,
                "The marker the camera's base rig follows: the lowest-indexed live one.",
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(RPG)
            .genre(SHOOTER)
            .sample("null")
            .roundtrip(round_trip::<CameraTargetDef>),
        );
        world.register_component_with(CAMERA_TARGET, |_: CameraTargetDef, _: &SpawnContext<'_>| {
            (CameraTarget,)
        });
        let mut following: Option<Entity> = None;
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(CAMERA_TARGET, move |world: &mut World| {
                camera_target(world, &mut following);
            })),
        );
    }
}

/// One frame: `following` is the target the plugin set last. With no
/// target in the World it clears its own and runs no query.
fn camera_target(world: &mut World, following: &mut Option<Entity>) {
    let target = if world.has_any::<CameraTarget>() {
        world
            .query_filtered::<Entity, With<CameraTarget>>()
            .min_by_key(|entity| entity.id())
    } else {
        None
    };
    let Some(controller) = world.get_resource_mut::<CameraController>() else {
        return;
    };
    let RigKind::Follow(follow) = &mut controller.rig.kind else {
        return;
    };
    if let Some(target) = target {
        if follow.target.is_none() || follow.target == *following {
            follow.target = Some(target);
            *following = Some(target);
        }
    } else {
        if following.is_some() && follow.target == *following {
            follow.target = None;
        }
        *following = None;
    }
}

/// [`CameraRigPlugin`]'s rig scene component.
pub const CAMERA_RIG: &str = "camera_rig";
/// [`CameraRigPlugin`]'s camera zone scene component.
pub const CAMERA_ZONE: &str = "camera_zone";

/// A camera zone: beside a [`Zone`](crate::Zone) on a sensor, its entity's
/// `CameraRig`, lowered at its spawn, raised through
/// `tungsten_core::camera::raise` when an entity the zone's filter passes
/// enters it empty, and lowered when the last inside leaves, the filter not
/// read again, or is despawned. A follow rig follows the first entity
/// inside. [`ZonePlugin`](crate::ZonePlugin)'s system keeps the list.
///
/// Scene name `camera_zone`: `{ "rig": { "fixed": { "rect": { "min": [0,
/// 0], "max": [640, 360] } }, "priority": 10, "blend": 0.4 } }`, the rig in
/// `camera_rig`'s form, its `live` refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CameraZone {
    inside: Vec<Entity>,
}

impl CameraZone {
    /// The entities inside, the first to enter first.
    #[must_use]
    pub fn inside(&self) -> &[Entity] {
        &self.inside
    }

    /// `entity` entered: whether the zone was empty, `None` when it was
    /// already inside.
    pub(crate) fn enter(&mut self, entity: Entity) -> Option<bool> {
        if self.inside.contains(&entity) {
            return None;
        }
        self.inside.push(entity);
        Some(self.inside.len() == 1)
    }

    /// `entity` left: whether the zone is now empty, `None` when it was not
    /// inside.
    pub(crate) fn leave(&mut self, entity: Entity) -> Option<bool> {
        let at = self.inside.iter().position(|&inside| inside == entity)?;
        self.inside.remove(at);
        Some(self.inside.is_empty())
    }
}

/// The name a rig takes in data, `camera_rig`'s `id`: a dotted name a
/// `camera` cue action ([`CueAction::raise_camera`](crate::CueAction::raise_camera))
/// raises and releases it by, kept beside the `CameraRig` so the rig stays
/// `Copy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraRigId(String);

impl CameraRigId {
    /// The ID `id`.
    ///
    /// # Panics
    ///
    /// When `id` is not a dotted name (`[a-z][a-z0-9_]*` segments joined
    /// by dots).
    #[must_use]
    #[track_caller]
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        assert!(
            valid_tag(&id),
            "CameraRigId::new: `{id}` is not a dotted name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        Self(id)
    }

    /// The ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Registers the `camera_rig` scene component: a `CameraRig` on its
/// entity, a follow of the entity itself, with its `CameraRigId` beside it
/// when it names one; `camera_zone`, a [`CameraZone`] with its rig lowered;
/// the inspector's rows; and the World's `CameraAim` from the kit's
/// [`Aim`](crate::Aim), so a follow's `aim` lead reads the entity's aim. It
/// adds no system: the engine's camera update reads the rigs, and the
/// zone's system the zones. Rigs and zones are presentation, not saved: a
/// load starts them from their prefabs.
#[derive(Debug, Default, Clone, Copy)]
pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        register_rows_with(world, "CameraRig", rig_rows);
        describe_shape(
            world,
            ComponentDescription::new(CAMERA_BOX, "A world-space box, `min` to `max`.")
                .field(FieldDescription::new(
                    "min",
                    FieldType::Vec2,
                    "its least corner, in world units",
                ))
                .field(FieldDescription::new(
                    "max",
                    FieldType::Vec2,
                    "its greatest corner, at least `min` on each axis",
                ))
                .sample(r#"{ "min": [0, 0], "max": [1280, 720] }"#)
                .roundtrip(round_trip::<BoxDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                CAMERA_LEAD,
                "A follow's lead: the view runs ahead of its target along its velocity or its aim, capped and eased.",
            )
            .field(FieldDescription::new(
                "from",
                FieldType::Enum(&["velocity", "aim"]),
                "`velocity`: the target's `Velocity` times `time`, else its drawn point's change; `aim`: the target's `aim` direction times `max`",
            ))
            .field(FieldDescription::new(
                "max",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "the longest lead, in world units",
            ))
            .field(
                FieldDescription::new(
                    "time",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds of velocity ahead",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "smoothing",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Inclusive(1.0),
                    },
                    "the share of the way the lead covers per 1/60 s at any frame rate: 0 never moves, 1 snaps",
                )
                .default("0.1"),
            )
            .sample(r#"{ "from": "velocity", "max": 96 }"#)
            .roundtrip(round_trip::<LeadDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                CAMERA_FOLLOW,
                "A follow of the rig's own entity: once its drawn point, offset and led, leaves the dead zone, the view follows at its smoothing.",
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "added to the drawn point (its sprite's top-left): the anchor the view keeps",
                )
                .default("[0.0, 0.0]"),
            )
            .field(
                FieldDescription::new(
                    "dead_zone",
                    FieldType::Vec2,
                    "the box, in world units centred in the view and clamped to it, the anchor roams before the view moves; each side at least 0",
                )
                .default("[0.0, 0.0]"),
            )
            .field(
                FieldDescription::new(
                    "smoothing",
                    FieldType::Object("a number, or [x, y], each in [0, 1]"),
                    "the share of the way the view covers per 1/60 s at any frame rate, on both axes or each: 0 never moves, 1 snaps",
                )
                .default("1.0"),
            )
            .field(
                FieldDescription::new(
                    "lead",
                    FieldType::Shape(CAMERA_LEAD),
                    "a lead ahead of the target, added to the anchor before the dead zone judges it",
                )
                .default("null"),
            )
            .sample("{}")
            .roundtrip(round_trip::<FollowDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(CAMERA_FIXED, "Where a fixed rig holds the view.")
                .one_of()
                .field(FieldDescription::new(
                    "at",
                    FieldType::Vec2,
                    "the view's centre",
                ))
                .field(FieldDescription::new(
                    "corner",
                    FieldType::Vec2,
                    "the view's top-left",
                ))
                .field(FieldDescription::new(
                    "rect",
                    FieldType::Shape(CAMERA_BOX),
                    "a box the view centres on, at the rig's view height or else the least that shows it whole; wider and taller than 0",
                ))
                .sample(r#"{ "at": [640, 360] }"#)
                .roundtrip(round_trip::<FixedDef>),
        );
        describe(
            world,
            ComponentDescription::new(
                CAMERA_RIG,
                "A camera rig on its entity; exactly one of `follow`, `fixed` and `scripted`. The highest live rig wins, a tie going to the latest raised, and a switch blends over the incoming rig's `blend`.",
            )
            .field(FieldDescription::new(
                "follow",
                FieldType::Shape(CAMERA_FOLLOW),
                "follows the entity itself",
            ))
            .field(FieldDescription::new(
                "fixed",
                FieldType::Shape(CAMERA_FIXED),
                "holds a centre, a corner or a box",
            ))
            .field(FieldDescription::new(
                "scripted",
                FieldType::Vec2,
                "centres the view on a point a game moves",
            ))
            .field(
                FieldDescription::new(
                    "view_height",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "the world units the view shows top to bottom; null shows the window's height at a zoom of 1",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "zoom",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "a scale over the view: 2 shows half its height",
                )
                .default("1.0"),
            )
            .field(
                FieldDescription::new(
                    "bounds",
                    FieldType::Shape(CAMERA_BOX),
                    "the box the rig's view stays inside, at its own height",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "priority",
                    FieldType::I64 {
                        min: Some(i64::from(i32::MIN)),
                        max: Some(i64::from(i32::MAX)),
                    },
                    "the highest live rig wins; the engine's base rig sits at 0",
                )
                .default("0"),
            )
            .field(
                FieldDescription::new(
                    "blend",
                    FieldType::Object(
                        r#"seconds, or { "time": seconds, "ease": easing } (sine_in_out), each time at least 0"#,
                    ),
                    "how the view moves to this rig when it starts winning, on the game clock; 0 cuts",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "id",
                    FieldType::Text,
                    "a dotted name, kept beside the rig as its `CameraRigId`, that a `camera` cue action raises and releases it by",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "live",
                    FieldType::Bool,
                    "whether the rig starts as a candidate; a lowered one waits for a raise",
                )
                .default("true"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(RPG)
            .genre(SHOOTER)
            .sample(r#"{ "follow": {} }"#)
            .roundtrip(round_trip::<CameraRigDef>),
        );
        world.register_component_with(CAMERA_RIG, |def: CameraRigDef, _: &SpawnContext<'_>| {
            def.into_bundle()
        });
        register_rows_with(world, "CameraZone", |world: &World, entity: Entity| {
            world
                .get::<CameraZone>(entity)
                .map(|zone| vec![("inside", zone.inside.len().to_string())])
                .unwrap_or_default()
        });
        describe(
            world,
            ComponentDescription::new(
                CAMERA_ZONE,
                "A zone's camera: beside a `zone` on a sensor, its rig, lowered at its spawn, is raised while something the zone's filter passes is inside; a follow rig follows the first inside.",
            )
            .field(FieldDescription::new(
                "rig",
                FieldType::Object("a `camera_rig` form, its `live` refused"),
                "the rig the zone raises and lowers",
            ))
            .genre(PLATFORMER)
            .genre(RPG)
            .sample(r#"{ "rig": { "fixed": { "at": [640, 360] } } }"#)
            .roundtrip(round_trip::<CameraZoneDef>),
        );
        world.register_component_with(CAMERA_ZONE, |def: CameraZoneDef, _: &SpawnContext<'_>| {
            let mut rig = def.rig.into_bundle();
            rig.0.lower();
            (CameraZone::default(),).with(rig)
        });
        if !world.has_resource::<CameraAim>() {
            world.insert_resource(CameraAim(aim_of));
        }
    }
}

/// The shapes `camera_rig` names.
const CAMERA_BOX: &str = "camera_box";
const CAMERA_LEAD: &str = "camera_lead";
const CAMERA_FOLLOW: &str = "camera_follow";
const CAMERA_FIXED: &str = "camera_fixed";

/// The kit's aim source for an `aim` lead: the entity's [`Aim`] direction.
fn aim_of(world: &World, entity: Entity) -> Option<Vec2> {
    world.get::<Aim>(entity).and_then(Aim::direction)
}

/// The inspector's rows for a rig: its kind, priority, whether it is live,
/// its ID, its view and its bounds.
fn rig_rows(world: &World, entity: Entity) -> Vec<(&'static str, String)> {
    let Some(rig) = world.get::<CameraRig>(entity) else {
        return Vec::new();
    };
    let mut rows = vec![
        ("kind", rig.kind.name().to_string()),
        ("priority", rig.priority.to_string()),
        ("live", rig.is_live().to_string()),
    ];
    if let Some(id) = world.get::<CameraRigId>(entity) {
        rows.push(("id", id.as_str().to_string()));
    }
    let view = rig.view_height.map_or_else(
        || format!("the window's height \u{d7}{:.2}", rig.zoom),
        |height| format!("{height:.0} tall \u{d7}{:.2}", rig.zoom),
    );
    rows.push(("view", view));
    if let Some(bounds) = rig.bounds {
        rows.push((
            "bounds",
            format!(
                "{:.0}, {:.0} to {:.0}, {:.0}",
                bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y
            ),
        ));
    }
    rows
}

/// `[x, y]` as a scene writes it.
type Pair = [f32; 2];

fn finite(pair: Pair) -> bool {
    pair.iter().all(|value| value.is_finite())
}

/// A box as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoxFields {
    min: Pair,
    max: Pair,
}

/// A box, checked: finite, `min` at most `max` on each axis.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "BoxFields")]
struct BoxDef {
    min: Pair,
    max: Pair,
}

impl TryFrom<BoxFields> for BoxDef {
    type Error = String;

    fn try_from(fields: BoxFields) -> Result<Self, Self::Error> {
        let BoxFields { min, max } = fields;
        if !(finite(min) && finite(max)) {
            return Err(format!("the box {min:?} to {max:?} is not finite"));
        }
        if min[0] > max[0] || min[1] > max[1] {
            return Err(format!("the box's min {min:?} passes its max {max:?}"));
        }
        Ok(Self { min, max })
    }
}

impl BoxDef {
    fn bounds(self) -> CameraBounds {
        CameraBounds::new(self.min.into(), self.max.into())
    }
}

/// Where a lead comes from, as a scene writes it.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LeadFromDef {
    Velocity,
    Aim,
}

/// `follow.lead` as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeadFields {
    from: LeadFromDef,
    max: f32,
    #[serde(default)]
    time: f32,
    #[serde(default = "lead_smoothing")]
    smoothing: f32,
}

fn lead_smoothing() -> f32 {
    RigLead::SMOOTHING
}

/// A lead, checked: `max` and `time` at least 0, `smoothing` in [0, 1].
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "LeadFields")]
struct LeadDef {
    from: LeadFromDef,
    max: f32,
    time: f32,
    smoothing: f32,
}

impl TryFrom<LeadFields> for LeadDef {
    type Error = String;

    fn try_from(fields: LeadFields) -> Result<Self, Self::Error> {
        let LeadFields {
            from,
            max,
            time,
            smoothing,
        } = fields;
        if !(max.is_finite() && max >= 0.0) {
            return Err(format!("max: {max} is negative or not finite"));
        }
        if !(time.is_finite() && time >= 0.0) {
            return Err(format!("time: {time} is negative or not finite"));
        }
        if !(0.0..=1.0).contains(&smoothing) {
            return Err(format!("smoothing: {smoothing} lies outside [0, 1]"));
        }
        Ok(Self {
            from,
            max,
            time,
            smoothing,
        })
    }
}

impl LeadDef {
    fn lead(self) -> RigLead {
        RigLead {
            from: match self.from {
                LeadFromDef::Velocity => LeadFrom::Velocity,
                LeadFromDef::Aim => LeadFrom::Aim,
            },
            max: self.max,
            time: self.time,
            smoothing: self.smoothing,
        }
    }
}

/// A follow's smoothing: one rate for both axes, or one each.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
enum SmoothingDef {
    Both(f32),
    Axes(Pair),
}

impl Default for SmoothingDef {
    fn default() -> Self {
        Self::Both(1.0)
    }
}

impl SmoothingDef {
    fn axes(self) -> Vec2 {
        match self {
            Self::Both(rate) => Vec2::splat(rate),
            Self::Axes(rates) => rates.into(),
        }
    }
}

/// `follow` as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowFields {
    #[serde(default)]
    offset: Pair,
    #[serde(default)]
    dead_zone: Pair,
    #[serde(default)]
    smoothing: SmoothingDef,
    lead: Option<LeadDef>,
}

/// A follow, checked: a finite offset, a dead zone of sides at least 0 and
/// rates in [0, 1].
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "FollowFields")]
struct FollowDef {
    offset: Pair,
    dead_zone: Pair,
    smoothing: SmoothingDef,
    lead: Option<LeadDef>,
}

impl TryFrom<FollowFields> for FollowDef {
    type Error = String;

    fn try_from(fields: FollowFields) -> Result<Self, Self::Error> {
        let FollowFields {
            offset,
            dead_zone,
            smoothing,
            lead,
        } = fields;
        if !finite(offset) {
            return Err(format!("offset: {offset:?} is not finite"));
        }
        if !(finite(dead_zone) && dead_zone.iter().all(|side| *side >= 0.0)) {
            return Err(format!(
                "dead_zone: {dead_zone:?} has a side below 0 or not finite"
            ));
        }
        let rates = smoothing.axes();
        if !((0.0..=1.0).contains(&rates.x) && (0.0..=1.0).contains(&rates.y)) {
            return Err(format!("smoothing: {rates} lies outside [0, 1]"));
        }
        Ok(Self {
            offset,
            dead_zone,
            smoothing,
            lead,
        })
    }
}

impl FollowDef {
    fn follow(self) -> FollowRig {
        let follow = FollowRig::default()
            .with_offset(self.offset.into())
            .with_dead_zone(self.dead_zone.into())
            .with_smoothing(self.smoothing.axes());
        match self.lead {
            Some(lead) => follow.with_lead(lead.lead()),
            None => follow,
        }
    }
}

/// `fixed` as a scene writes it: exactly one of `at`, `corner` and `rect`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixedFields {
    at: Option<Pair>,
    corner: Option<Pair>,
    rect: Option<BoxDef>,
}

/// `fixed`, checked: the one form it names, a point finite and a box wider
/// and taller than 0.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "FixedFields")]
struct FixedDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    at: Option<Pair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    corner: Option<Pair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rect: Option<BoxDef>,
}

impl TryFrom<FixedFields> for FixedDef {
    type Error = String;

    fn try_from(fields: FixedFields) -> Result<Self, Self::Error> {
        match (fields.at, fields.corner, fields.rect) {
            (Some(point), None, None) | (None, Some(point), None) => {
                if !finite(point) {
                    return Err(format!("the point {point:?} is not finite"));
                }
            }
            (None, None, Some(rect)) => {
                if rect.min[0] >= rect.max[0] || rect.min[1] >= rect.max[1] {
                    return Err(format!(
                        "rect: {:?} to {:?} is not wider and taller than 0",
                        rect.min, rect.max
                    ));
                }
            }
            _ => return Err("exactly one of `at`, `corner` and `rect`".to_string()),
        }
        Ok(Self {
            at: fields.at,
            corner: fields.corner,
            rect: fields.rect,
        })
    }
}

impl FixedDef {
    fn fixed(self) -> FixedRig {
        match (self.at, self.corner, self.rect) {
            (Some(centre), ..) => FixedRig::At(centre.into()),
            (None, Some(corner), _) => FixedRig::Corner(corner.into()),
            (None, None, Some(rect)) => FixedRig::Rect(rect.bounds()),
            (None, None, None) => unreachable!("checked at deserialization"),
        }
    }
}

/// A blend as a scene writes it: seconds, or seconds and an easing.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
enum BlendDef {
    Seconds(f32),
    Eased(EasedBlend),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EasedBlend {
    time: f32,
    #[serde(default = "sine_in_out")]
    ease: Easing,
}

fn sine_in_out() -> Easing {
    RigBlend::CUT.ease
}

impl Default for BlendDef {
    fn default() -> Self {
        Self::Seconds(0.0)
    }
}

impl BlendDef {
    fn blend(self) -> RigBlend {
        match self {
            Self::Seconds(time) => RigBlend::over(time),
            Self::Eased(EasedBlend { time, ease }) => RigBlend::over(time).with_ease(ease),
        }
    }

    fn time(self) -> f32 {
        match self {
            Self::Seconds(time) | Self::Eased(EasedBlend { time, .. }) => time,
        }
    }
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// `components.camera_rig` as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraRigFields {
    follow: Option<FollowDef>,
    fixed: Option<FixedDef>,
    scripted: Option<Pair>,
    view_height: Option<f32>,
    #[serde(default = "one")]
    zoom: f32,
    bounds: Option<BoxDef>,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    blend: BlendDef,
    id: Option<String>,
    #[serde(default = "yes")]
    live: bool,
}

/// `components.camera_rig`, checked: the one kind it names, each value in
/// range.
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "CameraRigFields")]
struct CameraRigDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    follow: Option<FollowDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fixed: Option<FixedDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scripted: Option<Pair>,
    view_height: Option<f32>,
    zoom: f32,
    bounds: Option<BoxDef>,
    priority: i32,
    blend: BlendDef,
    id: Option<String>,
    live: bool,
}

impl TryFrom<CameraRigFields> for CameraRigDef {
    type Error = String;

    fn try_from(fields: CameraRigFields) -> Result<Self, Self::Error> {
        let kinds = [
            fields.follow.is_some(),
            fields.fixed.is_some(),
            fields.scripted.is_some(),
        ];
        if kinds.iter().filter(|&&named| named).count() != 1 {
            return Err("exactly one of `follow`, `fixed` and `scripted`".to_string());
        }
        if let Some(point) = fields.scripted
            && !finite(point)
        {
            return Err(format!("scripted: {point:?} is not finite"));
        }
        if let Some(height) = fields.view_height
            && !(height.is_finite() && height > 0.0)
        {
            return Err(format!("view_height: {height} is not above 0 and finite"));
        }
        if !(fields.zoom.is_finite() && fields.zoom > 0.0) {
            return Err(format!("zoom: {} is not above 0 and finite", fields.zoom));
        }
        let time = fields.blend.time();
        if !(time.is_finite() && time >= 0.0) {
            return Err(format!("blend: {time} s is negative or not finite"));
        }
        if let Some(id) = &fields.id
            && !valid_tag(id)
        {
            return Err(format!(
                "id: `{id}` is not a dotted name (`[a-z][a-z0-9_]*` segments joined by dots)"
            ));
        }
        Ok(Self {
            follow: fields.follow,
            fixed: fields.fixed,
            scripted: fields.scripted,
            view_height: fields.view_height,
            zoom: fields.zoom,
            bounds: fields.bounds,
            priority: fields.priority,
            blend: fields.blend,
            id: fields.id,
            live: fields.live,
        })
    }
}

impl CameraRigDef {
    /// The rig, a follow of its own entity, and its ID when it names one.
    fn rig(&self) -> CameraRig {
        let mut rig = match (self.follow, self.fixed, self.scripted) {
            (Some(follow), ..) => CameraRig::follow(follow.follow()),
            (None, Some(fixed), _) => CameraRig::fixed(fixed.fixed()),
            (None, None, Some(point)) => CameraRig::scripted(point.into()),
            (None, None, None) => unreachable!("checked at deserialization"),
        };
        if let Some(height) = self.view_height {
            rig = rig.with_view_height(height);
        }
        if let Some(bounds) = self.bounds {
            rig = rig.with_bounds(bounds.bounds());
        }
        rig = rig
            .with_zoom(self.zoom)
            .with_priority(self.priority)
            .with_blend(self.blend.blend());
        if !self.live {
            rig.lower();
        }
        rig
    }

    fn into_bundle(self) -> RigBundle {
        RigBundle(self.rig(), self.id.map(CameraRigId))
    }
}

/// `components.camera_zone` as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraZoneFields {
    rig: serde_json::Value,
}

/// `components.camera_zone`, checked: its rig's form, with no `live`.
#[derive(Deserialize)]
#[serde(try_from = "CameraZoneFields")]
struct CameraZoneDef {
    rig: CameraRigDef,
}

impl TryFrom<CameraZoneFields> for CameraZoneDef {
    type Error = String;

    fn try_from(fields: CameraZoneFields) -> Result<Self, Self::Error> {
        if fields.rig.get("live").is_some() {
            return Err("rig.live: a zone's rig is raised while something is inside".to_string());
        }
        let rig = serde_json::from_value(fields.rig).map_err(|e| format!("rig: {e}"))?;
        Ok(Self { rig })
    }
}

impl Serialize for CameraZoneDef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut rig = serde_json::to_value(&self.rig).map_err(serde::ser::Error::custom)?;
        if let Some(fields) = rig.as_object_mut() {
            fields.remove("live");
        }
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("rig", &rig)?;
        map.end()
    }
}

/// A rig and its ID, when it has one.
struct RigBundle(CameraRig, Option<CameraRigId>);

impl Bundle for RigBundle {
    fn put<S: BundleSink>(self, sink: &mut S) {
        sink.put(self.0);
        if let Some(id) = self.1 {
            sink.put(id);
        }
    }
}

impl Inspectable for CameraTarget {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("camera", "follows".to_string())]
    }
}

#[cfg(test)]
#[path = "../tests/camera.rs"]
mod tests;
