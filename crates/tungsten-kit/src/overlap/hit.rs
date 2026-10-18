//! Hit shapes: hitboxes that hit the hurtboxes their shapes meet over each
//! step's motion, each target once per activation, through the damage
//! pipeline; and the data forms the kit's overlap items share.

use glam::Vec2;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::physics::Shape;
use tungsten_core::{
    Entity, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, SystemDesc, With, Without,
    World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN,
    describe, describe_shape, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::non_negative;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared, valid_tag};
use crate::lifecycle::despawn::DESPAWN_OUTSIDE;
use crate::lifecycle::ended::Despawning;
use crate::motion::stay_in_view::STAY_IN_VIEW;
use crate::motion::wrap::WRAP;
use crate::overlap::grid::{
    Blow, HitGrid, Sweep, ensure_emitter, first_contact, hit_grid, spares, strike, sweep_of,
};
use crate::overlap::projectile::Projectile;

/// The overlap systems' grid of hurtboxes, rebuilt first in the overlap
/// phase each step; each of [`HitPlugin`],
/// [`ContactDamagePlugin`](crate::ContactDamagePlugin),
/// [`ProjectilePlugin`](crate::ProjectilePlugin) and
/// [`PickupPlugin`](crate::PickupPlugin) adds it when the schedule lacks it.
pub const HIT_GRID: &str = "hit_grid";

/// [`HitPlugin`]'s system that runs the hitboxes, in the overlap phase.
pub const HITBOX_TICK: &str = "hitbox_tick";

/// The scene components the plugin registers, and the shape they share.
const HITBOX: &str = "hitbox";
const HURTBOX: &str = "hurtbox";
const HIT_SHAPE: &str = "hit_shape";

/// The largest size a hit shape takes and the farthest its offset reaches,
/// in world pixels, as a `rigid_body`'s.
pub(crate) const MAX_EXTENT: f32 = 16_384.0;

/// A circle or a box, which each overlap item places at its entity's point
/// plus the item's offset. Its sizes are finite and in `(0, 16384]`, as a
/// `rigid_body`'s.
///
/// Data form, one field: `{ "circle": 8 }`, the radius, or
/// `{ "aabb": [6, 3] }`, the half extents.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::Shape;
/// use tungsten_kit::HitShape;
///
/// let blade = HitShape::aabb(Vec2::new(6.0, 3.0));
/// assert_eq!(blade.shape(), Shape::Aabb { half_extents: Vec2::new(6.0, 3.0) });
/// let read: HitShape = serde_json::from_str(r#"{ "circle": 8 }"#).unwrap();
/// assert_eq!(read, HitShape::circle(8.0));
/// assert!(serde_json::from_str::<HitShape>(r#"{ "circle": 0 }"#).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "HitShapeDef", into = "HitShapeDef")]
pub struct HitShape {
    shape: Shape,
}

/// A hit shape as data writes it: exactly one form, its sizes checked.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum HitShapeDef {
    Circle(#[serde(deserialize_with = "extent")] f32),
    Aabb(#[serde(with = "extents")] Vec2),
}

impl From<HitShapeDef> for HitShape {
    fn from(def: HitShapeDef) -> Self {
        match def {
            HitShapeDef::Circle(radius) => Self {
                shape: Shape::Circle { radius },
            },
            HitShapeDef::Aabb(half_extents) => Self {
                shape: Shape::Aabb { half_extents },
            },
        }
    }
}

impl From<HitShape> for HitShapeDef {
    fn from(shape: HitShape) -> Self {
        match shape.shape {
            Shape::Circle { radius } => HitShapeDef::Circle(radius),
            Shape::Aabb { half_extents } => HitShapeDef::Aabb(half_extents),
        }
    }
}

/// Whether `size` is finite and in `(0, 16384]`.
fn valid_extent(size: f32) -> bool {
    size.is_finite() && size > 0.0 && size <= MAX_EXTENT
}

/// A size in `(0, 16384]`, for a shape's `deserialize_with`.
fn extent<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if valid_extent(value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "must be finite and in (0, {MAX_EXTENT}] (got {value})"
        )))
    }
}

/// Half extents written `[x, y]`, each in `(0, 16384]`.
mod extents {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer};

    use super::{MAX_EXTENT, Vec2, valid_extent};

    pub(crate) use crate::foundation::vec2::serialize;

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec2, D::Error> {
        let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
        if valid_extent(x) && valid_extent(y) {
            Ok(Vec2::new(x, y))
        } else {
            Err(D::Error::custom(format!(
                "each half extent must be finite and in (0, {MAX_EXTENT}] (got [{x}, {y}])"
            )))
        }
    }
}

/// Whether `offset` is finite and within ±16384 on each axis.
pub(crate) fn valid_offset(offset: Vec2) -> bool {
    offset.is_finite() && offset.abs().max_element() <= MAX_EXTENT
}

/// An offset written `[x, y]`, finite and within ±16384, for a
/// `#[serde(with = "offset")]` field.
pub(crate) mod offset {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer};

    use super::{MAX_EXTENT, Vec2, valid_offset};

    pub(crate) use crate::foundation::vec2::serialize;

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec2, D::Error> {
        let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
        let value = Vec2::new(x, y);
        if valid_offset(value) {
            Ok(value)
        } else {
            Err(D::Error::custom(format!(
                "must be finite and within ±{MAX_EXTENT} (got [{x}, {y}])"
            )))
        }
    }
}

/// An optional offset, as [`offset`] checks it, for a
/// `#[serde(with = "optional_offset")]` field.
pub(crate) mod optional_offset {
    use serde::{Deserialize, Deserializer};

    use super::Vec2;

    pub(crate) use crate::foundation::optional_vec2::serialize;

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec2>, D::Error> {
        #[derive(Deserialize)]
        #[serde(transparent)]
        struct Checked(#[serde(with = "super::offset")] Vec2);
        Ok(Option::<Checked>::deserialize(deserializer)?.map(|checked| checked.0))
    }
}

/// `offset` checked for a builder.
#[track_caller]
pub(crate) fn checked_offset(method: &str, offset: Vec2) -> Vec2 {
    assert!(
        valid_offset(offset),
        "{method}: offset must be finite and within ±{MAX_EXTENT} (got {offset})"
    );
    offset
}

impl HitShape {
    /// A circle of `radius` world pixels.
    ///
    /// # Panics
    ///
    /// When `radius` is not finite and in `(0, 16384]`.
    #[must_use]
    #[track_caller]
    pub fn circle(radius: f32) -> Self {
        assert!(
            valid_extent(radius),
            "HitShape::circle: radius must be finite and in (0, {MAX_EXTENT}] (got {radius})"
        );
        Self {
            shape: Shape::Circle { radius },
        }
    }

    /// A box of `half_extents` world pixels.
    ///
    /// # Panics
    ///
    /// When an axis is not finite and in `(0, 16384]`.
    #[must_use]
    #[track_caller]
    pub fn aabb(half_extents: Vec2) -> Self {
        assert!(
            valid_extent(half_extents.x) && valid_extent(half_extents.y),
            "HitShape::aabb: each half extent must be finite and in (0, {MAX_EXTENT}] (got {half_extents})"
        );
        Self {
            shape: Shape::Aabb { half_extents },
        }
    }

    /// The shape, as the physics queries take it.
    #[must_use]
    pub fn shape(&self) -> Shape {
        self.shape
    }

    /// The shape in a line: `circle 8` or `box 6 x 3`.
    pub(crate) fn summary(&self) -> String {
        match self.shape {
            Shape::Circle { radius } => format!("circle {radius}"),
            Shape::Aabb { half_extents } => format!("box {} x {}", half_extents.x, half_extents.y),
        }
    }
}

/// Tag names as a scene writes them, each checked, kept in the written
/// order so a load's error names the index as written.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>")]
pub(crate) struct TagNames(pub(crate) Vec<String>);

impl TryFrom<Vec<String>> for TagNames {
    type Error = String;

    fn try_from(names: Vec<String>) -> Result<Self, Self::Error> {
        match names.iter().find(|name| !valid_tag(name)) {
            Some(name) => Err(format!(
                "`{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            )),
            None => Ok(Self(names)),
        }
    }
}

/// The load's check of an item's `tags` and `filter`: each name declared,
/// named by its field as written.
pub(crate) fn check_names(
    world: &World,
    tags: Option<&TagNames>,
    filter: &TagFilterDef,
) -> Result<(), MissingAsset> {
    for (i, name) in tags.map_or(&[][..], |tags| &tags.0[..]).iter().enumerate() {
        check_declared(world, &format!("tags.{i}"), name)?;
    }
    for (field, name) in filter.names() {
        check_declared(world, &format!("filter.{field}"), name)?;
    }
    Ok(())
}

/// `system` in the overlap phase after [`HIT_GRID`], and before
/// `despawn_outside`, `wrap` and `stay_in_view` when each is registered, so
/// it reads the step's motion before a wrap rewrites it.
pub(crate) fn after_grid(system: SystemDesc) -> SystemDesc {
    before_corrections(Phase::Overlap.join(system).after(HIT_GRID))
}

fn before_corrections(system: SystemDesc) -> SystemDesc {
    system
        .before_if_present(DESPAWN_OUTSIDE)
        .before_if_present(WRAP)
        .before_if_present(STAY_IN_VIEW)
}

/// Adds [`HIT_GRID`] when the schedule lacks it: every overlap plugin
/// calls it before its own systems.
pub(crate) fn ensure_grid(schedule: &mut Schedule) {
    if schedule.contains(HIT_GRID) {
        return;
    }
    schedule.add(
        Phase::Overlap.stage(),
        before_corrections(Phase::Overlap.join(system(HIT_GRID, hit_grid))),
    );
}

/// Hits the hurtboxes its shape meets over each step, each target once per
/// activation: a sword's swing, a bolt's head, a stomp. [`HitPlugin`]'s
/// system tests its shape, at its entity's point plus its offset, against
/// every [`Hurtbox`] that its [`TagFilter`] passes, over the step's motion
/// of both, so a fast blow skips nothing. Each hit sends [`Struck`] and,
/// when its damage is above 0, queues a
/// [`DamageRequest`](crate::DamageRequest) of its damage and tags, from
/// its entity's [`Source`](crate::Source) or the entity itself, pushing the
/// target away by its knockback. A hitbox of damage 0 is a trigger: its
/// `hit` cues run and nothing is hurt. It never hits its own entity, the
/// entity that spawned it or its spawn chain's origin. An activation lasts
/// until [`rearm`](Self::rearm); a [`Projectile`](crate::Projectile)'s
/// hitbox is the projectile's to run.
///
/// Scene name `hitbox`:
/// `{ "shape": { "aabb": [6, 3] }, "offset": [8, 0], "damage": 2, "tags": ["damage.fire"], "filter": { "all": ["team.enemy"] }, "knockback": 300 }`,
/// `offset` `[0, 0]`, `tags` none, `filter` everything and `knockback` 0 by
/// default; the load checks its tags. It is data, without the targets it
/// has hit: a loaded one starts a fresh activation.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::{HitShape, Hitbox, TagFilter, Tags};
///
/// let sword = Hitbox::new(HitShape::aabb(Vec2::new(6.0, 3.0)), 2.0)
///     .with_offset(Vec2::new(8.0, 0.0))
///     .with_tags(Tags::new(["damage.slash"]))
///     .with_filter(TagFilter::new().all(["team.enemy"]))
///     .with_knockback(300.0);
/// assert_eq!((sword.damage(), sword.knockback()), (2.0, 300.0));
/// assert!(sword.tags().has("damage"));
/// let saved = serde_json::to_value(&sword).unwrap();
/// assert_eq!(saved["shape"], serde_json::json!({ "aabb": [6.0, 3.0] }));
/// assert_eq!(serde_json::from_value::<Hitbox>(saved).unwrap(), sword);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "HitboxFields", into = "HitboxFields")]
pub struct Hitbox {
    shape: HitShape,
    offset: Vec2,
    damage: f32,
    tags: Tags,
    filter: TagFilter,
    knockback: f32,
    /// The targets hit in this activation, by index.
    hit: Vec<Entity>,
}

/// A hitbox as a save writes it, before its values are checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HitboxFields {
    shape: HitShape,
    #[serde(default, with = "offset")]
    offset: Vec2,
    damage: f32,
    #[serde(default)]
    tags: Tags,
    #[serde(default)]
    filter: TagFilter,
    #[serde(default)]
    knockback: f32,
}

impl TryFrom<HitboxFields> for Hitbox {
    type Error = String;

    fn try_from(fields: HitboxFields) -> Result<Self, Self::Error> {
        for (field, value) in [("damage", fields.damage), ("knockback", fields.knockback)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            shape: fields.shape,
            offset: fields.offset,
            damage: fields.damage,
            tags: fields.tags,
            filter: fields.filter,
            knockback: fields.knockback,
            hit: Vec::new(),
        })
    }
}

impl From<Hitbox> for HitboxFields {
    fn from(hitbox: Hitbox) -> Self {
        Self {
            shape: hitbox.shape,
            offset: hitbox.offset,
            damage: hitbox.damage,
            tags: hitbox.tags,
            filter: hitbox.filter,
            knockback: hitbox.knockback,
        }
    }
}

impl Hitbox {
    /// A hitbox of `shape` dealing `damage`, at its entity's point, of no
    /// damage tag, hitting everything and pushing nothing.
    ///
    /// # Panics
    ///
    /// When `damage` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(shape: HitShape, damage: f32) -> Self {
        assert!(
            damage.is_finite() && damage >= 0.0,
            "Hitbox::new: damage must be finite and >= 0 (got {damage})"
        );
        Self {
            shape,
            offset: Vec2::ZERO,
            damage,
            tags: Tags::default(),
            filter: TagFilter::new(),
            knockback: 0.0,
            hit: Vec::new(),
        }
    }

    /// The hitbox, its shape at `offset` world pixels from its entity's
    /// point.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite and within ±16384 on each axis.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = checked_offset("Hitbox::with_offset", offset);
        self
    }

    /// The hitbox, its damage of the kinds `tags` (`damage.fire`), which a
    /// [`Resist`](crate::Resist) reads.
    #[must_use]
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    /// The hitbox, hitting only the hurtboxes whose entity's [`Tags`] pass
    /// `filter`.
    #[must_use]
    pub fn with_filter(mut self, filter: TagFilter) -> Self {
        self.filter = filter;
        self
    }

    /// The hitbox, pushing each target it hurts away from it at `speed`
    /// world pixels per second, through its
    /// [`Knockback`](crate::Knockback).
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_knockback(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "Hitbox::with_knockback: speed must be finite and >= 0 (got {speed})"
        );
        self.knockback = speed;
        self
    }

    /// Its shape.
    #[must_use]
    pub fn shape(&self) -> HitShape {
        self.shape
    }

    /// Its shape's offset from its entity's point.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.offset
    }

    /// Each hit's damage, which a `damage` stat writes.
    #[must_use]
    pub fn damage(&self) -> f32 {
        self.damage
    }

    /// Its damage tags.
    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
    }

    /// What it hits.
    #[must_use]
    pub fn filter(&self) -> &TagFilter {
        &self.filter
    }

    /// The push it gives, in world pixels per second.
    #[must_use]
    pub fn knockback(&self) -> f32 {
        self.knockback
    }

    /// Whether it has hit `target` in this activation.
    #[must_use]
    pub fn has_hit(&self, target: Entity) -> bool {
        self.hit.contains(&target)
    }

    /// Records a hit on `target` in this activation.
    pub(crate) fn mark_hit(&mut self, target: Entity) {
        self.hit.push(target);
    }

    /// Starts a new activation: every target may be hit once more.
    pub fn rearm(&mut self) {
        self.hit.clear();
    }

    /// Sets the damage a `damage` stat gives; one that is not finite and at
    /// least 0 is ignored.
    pub(crate) fn set_damage(&mut self, damage: f32) {
        if damage.is_finite() && damage >= 0.0 {
            self.damage = damage;
        }
    }
}

/// What hitboxes, contact damage and pickups meet: a circle or a box at
/// its entity's point plus its offset. A hurtbox whose entity is dead by
/// its [`Health`](crate::Health), or being despawned by the kit, is met by
/// nothing.
///
/// Scene name `hurtbox`: `{ "shape": { "circle": 8 }, "offset": [0, 0] }`,
/// `offset` `[0, 0]` by default. It is data.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::{HitShape, Hurtbox};
///
/// let body = Hurtbox::new(HitShape::circle(8.0)).with_offset(Vec2::new(0.0, -4.0));
/// assert_eq!(body.offset(), Vec2::new(0.0, -4.0));
/// let read: Hurtbox = serde_json::from_str(r#"{ "shape": { "circle": 8 }, "offset": [0, -4] }"#).unwrap();
/// assert_eq!(read, body);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "HurtboxDef", into = "HurtboxDef")]
pub struct Hurtbox {
    shape: HitShape,
    offset: Vec2,
}

impl Hurtbox {
    /// A hurtbox of `shape` at its entity's point.
    #[must_use]
    pub fn new(shape: HitShape) -> Self {
        Self {
            shape,
            offset: Vec2::ZERO,
        }
    }

    /// The hurtbox, at `offset` world pixels from its entity's point.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite and within ±16384 on each axis.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = checked_offset("Hurtbox::with_offset", offset);
        self
    }

    /// Its shape.
    #[must_use]
    pub fn shape(&self) -> HitShape {
        self.shape
    }

    /// Its offset from its entity's point.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.offset
    }
}

/// `components.hurtbox` as a scene writes it, and a save.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HurtboxDef {
    shape: HitShape,
    #[serde(default, with = "offset")]
    offset: Vec2,
}

impl From<HurtboxDef> for Hurtbox {
    fn from(def: HurtboxDef) -> Self {
        Self {
            shape: def.shape,
            offset: def.offset,
        }
    }
}

impl From<Hurtbox> for HurtboxDef {
    fn from(hurtbox: Hurtbox) -> Self {
        Self {
            shape: hurtbox.shape,
            offset: hurtbox.offset,
        }
    }
}

/// A hitbox, a projectile or a contact damage met a target: the cue event
/// `hit`, whose subject is the hitting entity and whose other is the
/// target. Sent once per hit, whatever its damage. Build one with
/// [`Struck::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Struck {
    /// The hitting entity: a hitbox's, a contact damage's or a
    /// projectile's.
    pub entity: Entity,
    /// The entity whose hurtbox it met.
    pub target: Entity,
    /// The hitting shape's centre where they met.
    pub position: Vec2,
    /// The damage the hit carried, before the target's resistances; 0 for a
    /// trigger.
    pub amount: f32,
}

impl Struck {
    /// An event with these fields, as the overlap items send one.
    #[must_use]
    pub fn new(entity: Entity, target: Entity, position: Vec2, amount: f32) -> Self {
        Self {
            entity,
            target,
            position,
            amount,
        }
    }
}

/// `components.hitbox` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HitboxDef {
    shape: HitShape,
    #[serde(default, with = "offset")]
    offset: Vec2,
    #[serde(deserialize_with = "non_negative")]
    damage: f32,
    #[serde(default)]
    tags: TagNames,
    #[serde(default)]
    filter: TagFilterDef,
    #[serde(default, deserialize_with = "non_negative")]
    knockback: f32,
}

impl From<HitboxDef> for Hitbox {
    fn from(def: HitboxDef) -> Self {
        Hitbox::new(def.shape, def.damage)
            .with_offset(def.offset)
            .with_tags(Tags::new(def.tags.0))
            .with_filter(def.filter.into())
            .with_knockback(def.knockback)
    }
}

/// The description of `hit_shape`, which the hit items' fields name.
pub(crate) fn describe_hit_shape(world: &mut World) {
    describe_shape(
        world,
        ComponentDescription::new(
            HIT_SHAPE,
            "A circle or a box, placed at its entity's point plus its item's offset.",
        )
        .one_of()
        .field(FieldDescription::new(
            "circle",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Inclusive(MAX_EXTENT),
            },
            "the radius, in world pixels",
        ))
        .field(FieldDescription::new(
            "aabb",
            FieldType::Vec2,
            "the half extents [x, y], each in (0, 16384]",
        ))
        .sample(r#"{ "circle": 8 }"#)
        .roundtrip(round_trip::<HitShapeDef>),
    );
}

/// The `shape` and `offset` fields of a hit item.
fn shape_fields(description: ComponentDescription, doc: &'static str) -> ComponentDescription {
    description
        .field(FieldDescription::new(
            "shape",
            FieldType::Shape(HIT_SHAPE),
            doc,
        ))
        .field(
            FieldDescription::new(
                "offset",
                FieldType::Vec2,
                "from the entity's point, each axis within ±16384",
            )
            .default("[0.0, 0.0]"),
        )
}

/// The `tags`, `filter` and `knockback` fields of a hitting item.
pub(crate) fn hit_fields(description: ComponentDescription) -> ComponentDescription {
    description
        .field(
            FieldDescription::new(
                "tags",
                FieldType::List(Box::new(FieldType::Tag)),
                "the damage's kinds, such as `damage.fire`, which `resist` reads; the load checks them",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "filter",
                FieldType::Filter,
                "the hurtboxes it hits, by their entity's tags; the load checks its tags",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "knockback",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels per second, pushing a target it hurts away from it",
            )
            .default("0.0"),
        )
}

/// The `shape` and `offset` fields of a pickup, as a hurtbox's.
pub(crate) fn pickup_shape_fields(description: ComponentDescription) -> ComponentDescription {
    shape_fields(
        description,
        "its circle or box, which a collector's hurtbox meets",
    )
}

/// Registers `EventQueue<Struck>`, the `hitbox` and `hurtbox` scene
/// components with the `hit_shape` they share, and in the overlap phase,
/// after `physics_step`, [`HIT_GRID`] when the schedule lacks it and then
/// [`HITBOX_TICK`], both before `despawn_outside`, `wrap` and
/// `stay_in_view` when each is registered, so they read the step's motion
/// before a wrap rewrites it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{HIT_GRID, HITBOX_TICK, HitPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HitPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
/// let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
/// assert!(at("kit_overlap") < at(HIT_GRID) && at(HIT_GRID) < at(HITBOX_TICK));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct HitPlugin;

impl Plugin for HitPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Hitbox>(world, "Hitbox");
        register_rows::<Hurtbox>(world, "Hurtbox");
        describe_hit_shape(world);
        describe(
            world,
            hit_fields(
                shape_fields(
                    ComponentDescription::new(
                        HITBOX,
                        "Hits the hurtboxes its shape meets over each step, each target once per activation, through the damage pipeline.",
                    ),
                    "its circle or box",
                )
                .field(FieldDescription::new(
                    "damage",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "each hit's damage, which a `damage` stat writes; 0 is a trigger whose `hit` cues run",
                )),
            )
            .event("Struck")
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(PLATFORMER)
            .genre(GRID)
            .sample(r#"{ "shape": { "circle": 4 }, "damage": 1 }"#)
            .roundtrip(round_trip::<HitboxDef>),
        );
        describe(
            world,
            shape_fields(
                ComponentDescription::new(
                    HURTBOX,
                    "What hitboxes, contact damage and pickups meet: a circle or a box at the entity's point.",
                ),
                "its circle or box",
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(PLATFORMER)
            .genre(GRID)
            .sample(r#"{ "shape": { "circle": 8 } }"#)
            .roundtrip(round_trip::<HurtboxDef>),
        );
        world.register_event::<Struck>();
        world.register_component_with_assets(
            HITBOX,
            |def: &HitboxDef, world: &World| check_names(world, Some(&def.tags), &def.filter),
            |def: HitboxDef, _: &SpawnContext<'_>| (Hitbox::from(def),),
        );
        world.register_component_with(HURTBOX, |def: HurtboxDef, _: &SpawnContext<'_>| {
            (Hurtbox::from(def),)
        });
        ensure_emitter(world);
        ensure_grid(schedule);
        let mut runner = HitboxRunner::default();
        schedule.add(
            Phase::Overlap.stage(),
            after_grid(system(HITBOX_TICK, move |world: &mut World| {
                runner.run(world);
            })),
        );
    }
}

/// [`HITBOX_TICK`]'s state between steps: the step's hitboxes, a query's
/// candidates and a hitbox's hits, kept so a step allocates nothing once
/// warm.
#[derive(Default)]
struct HitboxRunner {
    hitters: Vec<Entity>,
    candidates: Vec<u32>,
    /// (t, grid entry), in sweep order once sorted.
    hits: Vec<(f32, u32)>,
}

impl HitboxRunner {
    /// Runs every hitbox but a projectile's, in entity order, against the
    /// step's grid.
    fn run(&mut self, world: &mut World) {
        let Some(grid) = world.remove_resource::<HitGrid>() else {
            return;
        };
        self.hitters.clear();
        self.hitters
            .extend(world.query_filtered::<Entity, (With<Hitbox>, Without<Projectile>)>());
        self.hitters.sort_by_key(|entity| entity.id());
        for i in 0..self.hitters.len() {
            let hitter = self.hitters[i];
            if let Some(sweep) = self.collect(world, &grid, hitter) {
                self.land(world, &grid, hitter, &sweep);
            }
        }
        world.insert_resource(grid);
    }

    /// `hitter`'s sweep, with the entries it meets this step in `hits`, in
    /// sweep order: (t, target index); `None` for a hitter being despawned
    /// or unplaced.
    fn collect(&mut self, world: &World, grid: &HitGrid, hitter: Entity) -> Option<Sweep> {
        self.hits.clear();
        if world.has::<Despawning>(hitter) {
            return None;
        }
        let hitbox = world.get::<Hitbox>(hitter)?;
        let sweep = sweep_of(world, hitter, hitbox.shape.shape(), hitbox.offset)?;
        let (min, max) = sweep.bounds();
        grid.candidates(min, max, &mut self.candidates);
        for &index in &self.candidates {
            let entry = &grid.entries()[index as usize];
            if hitbox.has_hit(entry.entity)
                || spares(world, hitter, entry.entity)
                || !hitbox.filter.matches(world.get::<Tags>(entry.entity))
            {
                continue;
            }
            if let Some(t) = first_contact(&sweep, &entry.sweep) {
                self.hits.push((t, index));
            }
        }
        self.hits
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Some(sweep)
    }

    /// Lands `hits` in order, each marked so the activation hits it once.
    fn land(&mut self, world: &mut World, grid: &HitGrid, hitter: Entity, sweep: &Sweep) {
        if self.hits.is_empty() {
            return;
        }
        let (damage, tags, knockback) = {
            let hitbox = world.get::<Hitbox>(hitter).unwrap();
            (hitbox.damage, hitbox.tags.clone(), hitbox.knockback)
        };
        for &(t, index) in &self.hits {
            let entry = grid.entries()[index as usize];
            world
                .get_mut::<Hitbox>(hitter)
                .unwrap()
                .mark_hit(entry.entity);
            strike(
                world,
                &Blow {
                    hitter,
                    target: entry.entity,
                    hitter_at: sweep.at(t),
                    target_at: entry.sweep.at(t),
                    relative: (sweep.end - sweep.start) - (entry.sweep.end - entry.sweep.start),
                    damage,
                    tags: &tags,
                    knockback,
                },
            );
        }
    }
}

impl Inspectable for Hitbox {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "hitbox",
                format!("{} at {}", self.shape.summary(), point(self.offset)),
            ),
            ("damage", format!("{}", self.damage)),
            ("hit", format!("{} targets", self.hit.len())),
        ]
    }
}

impl Inspectable for Hurtbox {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![(
            "hurtbox",
            format!("{} at {}", self.shape.summary(), point(self.offset)),
        )]
    }
}

#[cfg(test)]
#[path = "../tests/hit.rs"]
mod tests;
