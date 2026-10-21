//! Contact damage: an entity that hurts what its shape touches, at once
//! and again each interval while it touches: a hazard, a spiked enemy, an
//! aura.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Time, With, World, system,
};

use crate::TIME_EPSILON;
use crate::damage::kinds::{Conversion, ConversionDef, Penetration, PenetrationDef};
use crate::damage::pipeline::DamagePart;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags};
use crate::foundation::{non_negative, optional_vec2, positive};
use crate::lifecycle::ended::Despawning;
use crate::overlap::grid::{
    Blow, HitGrid, ensure_emitter, first_contact, spares, strike, sweep_of,
};
use crate::overlap::hit::{
    DamageDef, HitPayload, HitShape, Hurtbox, SavedPart, Struck, TagNames, after_grid, check_names,
    checked_offset, damage_field, damage_row, describe_hit_shape, ensure_grid, hit_fields,
    optional_offset, parts_total, payload_from_save, payload_to_save,
};

/// [`ContactDamagePlugin`]'s system, in the overlap phase.
pub const CONTACT_DAMAGE: &str = "contact_damage";

/// The scene component the plugin registers.
const CONTACT_DAMAGE_COMPONENT: &str = "contact_damage";

/// The default seconds between two hits on one target.
const DEFAULT_INTERVAL: f32 = 0.5;

/// Hurts the hurtboxes its shape touches, at once and then again each
/// interval of game time while they touch: a hazard, a spiked enemy, an
/// aura. [`ContactDamagePlugin`]'s system tests its shape, its own when it
/// has one, else its entity's [`Hurtbox`](crate::Hurtbox)'s, against every
/// other hurtbox its [`TagFilter`] passes, over the step's motion of both.
/// A target met is hit at once, then each `interval` of the game seconds
/// the item has stepped while it is still met; a target met by none of its
/// steps for a whole interval starts over. Each hit sends
/// [`Struck`](crate::Struck) and, when its damage is above 0, queues a
/// [`DamageRequest`](crate::DamageRequest) as a hitbox's does. An entity
/// with neither shape hits nothing, with one warning. Composing beats
/// options: an aura is contact damage with its own shape.
///
/// Typed contact damage ([`with_parts`](Self::with_parts)) keeps its parts
/// and splits each hit's damage among them, with its own [`Penetration`]
/// and [`Conversion`], as a typed [`Hitbox`](crate::Hitbox) does.
///
/// Scene name `contact_damage`:
/// `{ "damage": 1, "interval": 0.5, "filter": { "all": ["team.player"] }, "knockback": 420, "shape": { "circle": 28 } }`,
/// `interval` 0.5, `tags` none, `filter` everything and `knockback` 0 by
/// default; without `shape` it takes the hurtbox's shape and offset, and
/// an `offset` comes only with a `shape`. `damage`, `pen` and `convert`
/// take a hitbox's forms. It is data, without the times it keeps per
/// target, which name entities: a loaded one starts over unless the game
/// names its targets again by its own IDs through
/// [`set_touches`](Self::set_touches), from the [`touches`](Self::touches)
/// it saved.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::{ContactDamage, HitShape, TagFilter};
///
/// let aura = ContactDamage::new(1.0)
///     .with_interval(0.25)
///     .with_shape(HitShape::circle(28.0), Vec2::ZERO)
///     .with_filter(TagFilter::new().all(["team.enemy"]));
/// assert_eq!((aura.damage(), aura.interval()), (1.0, 0.25));
/// assert_eq!(aura.shape(), Some(HitShape::circle(28.0)));
/// let spikes: ContactDamage = serde_json::from_str(r#"{ "damage": 2 }"#).unwrap();
/// assert_eq!((spikes.interval(), spikes.shape()), (0.5, None), "the hurtbox's shape");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ContactDamageFields", into = "ContactDamageFields")]
pub struct ContactDamage {
    damage: f32,
    interval: f32,
    /// Its own shape and offset: an aura.
    own: Option<(HitShape, Vec2)>,
    tags: Tags,
    filter: TagFilter,
    knockback: f32,
    /// Typed contact damage's parts, penetration and conversion.
    typed: Option<Box<HitPayload>>,
    /// Game seconds the item has stepped: its targets' clock.
    clock: f64,
    /// Each target it timed, by index.
    timed: Vec<Timed>,
}

/// A target contact damage has hit: its last hit and the last step that met
/// it, on the item's clock.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Timed {
    target: Entity,
    hit: f64,
    met: f64,
}

/// One target a [`ContactDamage`] touches: the game seconds since it last
/// hurt it and since they last met, which a game's save holds beside its
/// own ID for the target.
///
/// # Examples
///
/// ```
/// use tungsten_core::World;
/// use tungsten_kit::{ContactDamage, ContactTouch};
///
/// let mut world = World::new();
/// let hero = world.spawn();
/// // Hurt 0.2 s ago and met this step: due again in 0.3 s.
/// let mut spikes = ContactDamage::new(1.0).with_interval(0.5);
/// spikes.set_touches([ContactTouch::new(hero, 0.2, 0.0)]);
/// let touch = spikes.touches().next().unwrap();
/// assert_eq!((touch.target(), touch.since_hit(), touch.since_met()), (hero, 0.2, 0.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactTouch {
    target: Entity,
    since_hit: f32,
    since_met: f32,
}

impl ContactTouch {
    /// `target`, hurt `since_hit` seconds ago and met `since_met` seconds
    /// ago.
    ///
    /// # Panics
    ///
    /// When either seconds is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(target: Entity, since_hit: f32, since_met: f32) -> Self {
        assert!(
            since_hit.is_finite() && since_hit >= 0.0,
            "ContactTouch::new: since_hit must be finite and >= 0 (got {since_hit})"
        );
        assert!(
            since_met.is_finite() && since_met >= 0.0,
            "ContactTouch::new: since_met must be finite and >= 0 (got {since_met})"
        );
        Self {
            target,
            since_hit,
            since_met,
        }
    }

    /// The target it touches.
    #[must_use]
    pub fn target(&self) -> Entity {
        self.target
    }

    /// The game seconds since it last hurt the target.
    #[must_use]
    pub fn since_hit(&self) -> f32 {
        self.since_hit
    }

    /// The game seconds since a step last met the target.
    #[must_use]
    pub fn since_met(&self) -> f32 {
        self.since_met
    }
}

/// A contact damage as a save writes it, before its values are checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContactDamageFields {
    damage: f32,
    #[serde(default = "default_interval")]
    interval: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<HitShape>,
    #[serde(
        default,
        with = "optional_offset",
        skip_serializing_if = "Option::is_none"
    )]
    offset: Option<Vec2>,
    #[serde(default)]
    tags: Tags,
    #[serde(default)]
    filter: TagFilter,
    #[serde(default)]
    knockback: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parts: Vec<SavedPart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pen: Option<Penetration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    convert: Option<Conversion>,
}

fn default_interval() -> f32 {
    DEFAULT_INTERVAL
}

/// The pairing rule: an offset comes only with a shape.
fn own_shape(
    shape: Option<HitShape>,
    offset: Option<Vec2>,
) -> Result<Option<(HitShape, Vec2)>, String> {
    match (shape, offset) {
        (Some(shape), offset) => Ok(Some((shape, offset.unwrap_or(Vec2::ZERO)))),
        (None, None) => Ok(None),
        (None, Some(_)) => Err("an `offset` comes only with its own `shape`".to_string()),
    }
}

impl TryFrom<ContactDamageFields> for ContactDamage {
    type Error = String;

    fn try_from(fields: ContactDamageFields) -> Result<Self, Self::Error> {
        for (field, value) in [("damage", fields.damage), ("knockback", fields.knockback)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if !(fields.interval.is_finite() && fields.interval > 0.0) {
            return Err(format!(
                "interval: must be finite and > 0 (got {})",
                fields.interval
            ));
        }
        Ok(Self {
            damage: fields.damage,
            interval: fields.interval,
            own: own_shape(fields.shape, fields.offset)?,
            tags: fields.tags,
            filter: fields.filter,
            knockback: fields.knockback,
            typed: payload_from_save(fields.parts, fields.pen, fields.convert)?,
            clock: 0.0,
            timed: Vec::new(),
        })
    }
}

impl From<ContactDamage> for ContactDamageFields {
    fn from(contact: ContactDamage) -> Self {
        let (parts, pen, convert) = payload_to_save(contact.typed);
        Self {
            damage: contact.damage,
            interval: contact.interval,
            shape: contact.own.map(|(shape, _)| shape),
            offset: contact.own.map(|(_, offset)| offset),
            tags: contact.tags,
            filter: contact.filter,
            knockback: contact.knockback,
            parts,
            pen,
            convert,
        }
    }
}

impl ContactDamage {
    /// Contact damage of `damage` every 0.5 s, through its entity's
    /// hurtbox's shape and offset, of no damage tag, hitting everything and
    /// pushing nothing.
    ///
    /// # Panics
    ///
    /// When `damage` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(damage: f32) -> Self {
        assert!(
            damage.is_finite() && damage >= 0.0,
            "ContactDamage::new: damage must be finite and >= 0 (got {damage})"
        );
        Self {
            damage,
            interval: DEFAULT_INTERVAL,
            own: None,
            tags: Tags::default(),
            filter: TagFilter::new(),
            knockback: 0.0,
            typed: None,
            clock: 0.0,
            timed: Vec::new(),
        }
    }

    /// The contact damage, typed: `parts` as its base parts, which no stat
    /// writes, and their sum as its damage, split among them at each hit by
    /// their proportions; a part without types takes its
    /// [`with_tags`](Self::with_tags).
    ///
    /// # Panics
    ///
    /// When there is no part, or every part is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_kit::{ContactDamage, DamagePart, Penetration, Tags};
    ///
    /// let lava = ContactDamage::new(0.0)
    ///     .with_parts([DamagePart::new(3.0).with_tags(Tags::new(["elemental.fire"]))])
    ///     .with_penetration(Penetration::new().with_points("elemental", 0.5));
    /// assert_eq!((lava.damage(), lava.parts().len()), (3.0, 1));
    /// assert!(lava.conversion().is_none());
    /// ```
    #[must_use]
    #[track_caller]
    pub fn with_parts(mut self, parts: impl IntoIterator<Item = DamagePart>) -> Self {
        let parts: Vec<DamagePart> = parts.into_iter().collect();
        self.damage = parts_total("ContactDamage::with_parts", &parts);
        self.typed.get_or_insert_default().parts = parts;
        self
    }

    /// The contact damage, with its own `penetration`, before its source's.
    #[must_use]
    pub fn with_penetration(mut self, penetration: Penetration) -> Self {
        self.typed.get_or_insert_default().penetration = Some(penetration);
        self
    }

    /// The contact damage, with its own `conversion`, before its source's.
    #[must_use]
    pub fn with_conversion(mut self, conversion: Conversion) -> Self {
        self.typed.get_or_insert_default().conversion = Some(conversion);
        self
    }

    /// Its base parts; empty for contact damage of one number.
    #[must_use]
    pub fn parts(&self) -> &[DamagePart] {
        self.typed.as_ref().map_or(&[], |typed| &typed.parts)
    }

    /// Its own penetration, when it has one.
    #[must_use]
    pub fn penetration(&self) -> Option<&Penetration> {
        self.typed.as_ref()?.penetration.as_ref()
    }

    /// Its own conversion, when it has one.
    #[must_use]
    pub fn conversion(&self) -> Option<&Conversion> {
        self.typed.as_ref()?.conversion.as_ref()
    }

    /// The contact damage, hitting a target it still touches again each
    /// `seconds` of game time.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_interval(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "ContactDamage::with_interval: seconds must be finite and > 0 (got {seconds})"
        );
        self.interval = seconds;
        self
    }

    /// The contact damage, through its own `shape` at `offset` world pixels
    /// from its entity's point instead of the hurtbox's: an aura.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite and within ±16384 on each axis.
    #[must_use]
    #[track_caller]
    pub fn with_shape(mut self, shape: HitShape, offset: Vec2) -> Self {
        self.own = Some((shape, checked_offset("ContactDamage::with_shape", offset)));
        self
    }

    /// The contact damage, of the damage types `tags`: typed contact
    /// damage's untyped parts'.
    #[must_use]
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    /// The contact damage, hitting only the hurtboxes whose entity's
    /// [`Tags`] pass `filter`.
    #[must_use]
    pub fn with_filter(mut self, filter: TagFilter) -> Self {
        self.filter = filter;
        self
    }

    /// The contact damage, pushing each target it hurts away from it at
    /// `speed` world pixels per second.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_knockback(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "ContactDamage::with_knockback: speed must be finite and >= 0 (got {speed})"
        );
        self.knockback = speed;
        self
    }

    /// Each hit's damage, which a `damage` stat writes: one number, or typed
    /// contact damage's total.
    #[must_use]
    pub fn damage(&self) -> f32 {
        self.damage
    }

    /// The game seconds between two hits on a target it still touches.
    #[must_use]
    pub fn interval(&self) -> f32 {
        self.interval
    }

    /// Its own shape, when it has one; else the hurtbox's applies.
    #[must_use]
    pub fn shape(&self) -> Option<HitShape> {
        self.own.map(|(shape, _)| shape)
    }

    /// Its own shape's offset from its entity's point; `[0, 0]` without
    /// its own shape, whose hurtbox's offset applies.
    #[must_use]
    pub fn offset(&self) -> Vec2 {
        self.own.map_or(Vec2::ZERO, |(_, offset)| offset)
    }

    /// Its damage types.
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

    /// The targets it touches, in entity order, each with its seconds since
    /// its last hit and since they last met: what a game's save holds by
    /// its own IDs.
    pub fn touches(&self) -> impl Iterator<Item = ContactTouch> + '_ {
        let clock = self.clock;
        self.timed.iter().map(move |timed| ContactTouch {
            target: timed.target,
            since_hit: seconds_since(clock, timed.hit),
            since_met: seconds_since(clock, timed.met),
        })
    }

    /// Replaces the targets it touches, each once, the first of a target
    /// named twice kept: after a load, each is next due an interval after
    /// its last hit and forgotten an interval after it last met, as before
    /// the save.
    pub fn set_touches(&mut self, touches: impl IntoIterator<Item = ContactTouch>) {
        let clock = self.clock;
        self.timed.clear();
        for touch in touches {
            if self.timed.iter().any(|timed| timed.target == touch.target) {
                continue;
            }
            self.timed.push(Timed {
                target: touch.target,
                hit: clock - f64::from(touch.since_hit),
                met: clock - f64::from(touch.since_met),
            });
        }
        // `due` finds a target by its index.
        self.timed.sort_by_key(|timed| timed.target.id());
    }

    /// Sets the damage a `damage` stat gives; one that is not finite and at
    /// least 0 is ignored.
    pub(crate) fn set_damage(&mut self, damage: f32) {
        if damage.is_finite() && damage >= 0.0 {
            self.damage = damage;
        }
    }
}

/// The seconds from `then` to `clock`, as an `f32` of at least 0.
fn seconds_since(clock: f64, then: f64) -> f32 {
    ((clock - then).max(0.0)).min(f64::from(f32::MAX)) as f32
}

/// `components.contact_damage` as a scene writes it, before the pairing
/// rules.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContactDamageScene {
    damage: DamageDef,
    #[serde(default = "default_interval", deserialize_with = "positive")]
    interval: f32,
    #[serde(default)]
    shape: Option<HitShape>,
    #[serde(default, with = "optional_offset")]
    offset: Option<Vec2>,
    #[serde(default)]
    tags: TagNames,
    #[serde(default)]
    pen: PenetrationDef,
    #[serde(default)]
    convert: ConversionDef,
    #[serde(default)]
    filter: TagFilterDef,
    #[serde(default, deserialize_with = "non_negative")]
    knockback: f32,
}

/// `components.contact_damage`, checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "ContactDamageScene")]
struct ContactDamageDef {
    damage: DamageDef,
    interval: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    shape: Option<HitShape>,
    #[serde(with = "optional_vec2", skip_serializing_if = "Option::is_none")]
    offset: Option<Vec2>,
    tags: TagNames,
    pen: PenetrationDef,
    convert: ConversionDef,
    filter: TagFilterDef,
    knockback: f32,
}

impl TryFrom<ContactDamageScene> for ContactDamageDef {
    type Error = String;

    fn try_from(scene: ContactDamageScene) -> Result<Self, Self::Error> {
        own_shape(scene.shape, scene.offset)?;
        scene.damage.beside(&scene.tags)?;
        Ok(Self {
            damage: scene.damage,
            interval: scene.interval,
            shape: scene.shape,
            offset: scene.offset,
            tags: scene.tags,
            pen: scene.pen,
            convert: scene.convert,
            filter: scene.filter,
            knockback: scene.knockback,
        })
    }
}

impl From<ContactDamageDef> for ContactDamage {
    fn from(def: ContactDamageDef) -> Self {
        let mut contact = ContactDamage::new(def.damage.total())
            .with_interval(def.interval)
            .with_tags(Tags::new(def.tags.0))
            .with_filter(def.filter.into())
            .with_knockback(def.knockback);
        if let Some(shape) = def.shape {
            contact = contact.with_shape(shape, def.offset.unwrap_or(Vec2::ZERO));
        }
        if let Some(parts) = def.damage.parts() {
            contact = contact.with_parts(parts);
        }
        if !def.pen.is_empty() {
            contact = contact.with_penetration(def.pen.into());
        }
        if !def.convert.is_empty() {
            contact = contact.with_conversion(def.convert.into());
        }
        contact
    }
}

/// Registers `EventQueue<Struck>`, the `contact_damage` scene component
/// and, in the overlap phase, the hit grid when the schedule lacks it and
/// then [`CONTACT_DAMAGE`], before `despawn_outside`, `wrap` and
/// `stay_in_view` when each is registered.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{CONTACT_DAMAGE, ContactDamagePlugin, HIT_GRID};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ContactDamagePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert!(schedule.contains(HIT_GRID) && schedule.contains(CONTACT_DAMAGE));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct ContactDamagePlugin;

impl Plugin for ContactDamagePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<ContactDamage>(world, "ContactDamage");
        describe_hit_shape(world);
        describe(
            world,
            hit_fields(
                ComponentDescription::new(
                    CONTACT_DAMAGE_COMPONENT,
                    "Hurts the hurtboxes its shape touches, at once and again each interval while they touch: a hazard, an aura.",
                )
                .field(damage_field())
                .field(
                    FieldDescription::new(
                        "interval",
                        FieldType::F32 {
                            min: Bound::Exclusive(0.0),
                            max: Bound::Open,
                        },
                        "game seconds between two hits on a target it still touches",
                    )
                    .default("0.5"),
                )
                .field(
                    FieldDescription::new(
                        "shape",
                        FieldType::Shape("hit_shape"),
                        "its own circle or box, an aura; the entity's hurtbox's shape and offset when left out",
                    )
                    .default("null"),
                )
                .field(
                    FieldDescription::new(
                        "offset",
                        FieldType::Vec2,
                        "its own shape's offset from the entity's point, with `shape` only; [0, 0] when left out",
                    )
                    .default("null"),
                ),
            )
            .event("Struck")
            .genre(TOP_DOWN)
            .genre(PLATFORMER)
            .genre(GRID)
            .sample(r#"{ "damage": 1 }"#)
            .roundtrip(round_trip::<ContactDamageDef>),
        );
        world.register_event::<Struck>();
        world.register_component_with_assets(
            CONTACT_DAMAGE_COMPONENT,
            |def: &ContactDamageDef, world: &World| {
                check_names(world, Some(&def.tags), &def.filter)?;
                def.damage.check(world)?;
                def.pen.check(world, "pen.")?;
                def.convert.check(world, "convert.")
            },
            |def: ContactDamageDef, _: &SpawnContext<'_>| (ContactDamage::from(def),),
        );
        ensure_emitter(world);
        ensure_grid(schedule);
        let mut runner = ContactRunner::default();
        schedule.add(
            Phase::Overlap.stage(),
            after_grid(system(CONTACT_DAMAGE, move |world: &mut World| {
                runner.run(world);
            })),
        );
    }
}

/// [`CONTACT_DAMAGE`]'s state between steps: the step's items, a query's
/// candidates and an item's targets met, kept so a step allocates nothing
/// once warm, and the items warned about.
#[derive(Default)]
struct ContactRunner {
    hitters: Vec<Entity>,
    candidates: Vec<u32>,
    /// (t, grid entry), in sweep order once sorted.
    met: Vec<(f32, u32)>,
    warned: HashSet<Entity>,
}

impl ContactRunner {
    /// Runs every contact damage, in entity order, against the step's grid.
    fn run(&mut self, world: &mut World) {
        let dt = f64::from(world.get_resource::<Time>().map_or(0.0, Time::delta));
        let Some(grid) = world.remove_resource::<HitGrid>() else {
            return;
        };
        self.hitters.clear();
        self.hitters
            .extend(world.query_filtered::<Entity, With<ContactDamage>>());
        self.hitters.sort_by_key(|entity| entity.id());
        for i in 0..self.hitters.len() {
            let hitter = self.hitters[i];
            if world.has::<Despawning>(hitter) {
                continue;
            }
            world.get_mut::<ContactDamage>(hitter).unwrap().clock += dt;
            self.touch(world, &grid, hitter);
        }
        world.insert_resource(grid);
    }

    /// Meets `hitter`'s shape against the grid, hits each target due, and
    /// forgets each it has not met for a whole interval.
    fn touch(&mut self, world: &mut World, grid: &HitGrid, hitter: Entity) {
        self.met.clear();
        let contact = world.get::<ContactDamage>(hitter).unwrap();
        let shape = contact.own.or_else(|| {
            world
                .get::<Hurtbox>(hitter)
                .map(|hurtbox| (hurtbox.shape(), hurtbox.offset()))
        });
        let Some((shape, offset)) = shape else {
            if self.warned.insert(hitter) {
                log::warn!(
                    "ContactDamage on {hitter:?}: no shape of its own and no Hurtbox; it hits nothing"
                );
            }
            return;
        };
        let Some(sweep) = sweep_of(world, hitter, shape.shape(), offset) else {
            return;
        };
        let (min, max) = sweep.bounds();
        grid.candidates(min, max, &mut self.candidates);
        for &index in &self.candidates {
            let entry = &grid.entries()[index as usize];
            if spares(world, hitter, entry.entity)
                || !contact
                    .filter
                    .matches(world.get::<crate::Tags>(entry.entity))
            {
                continue;
            }
            if let Some(t) = first_contact(&sweep, &entry.sweep) {
                self.met.push((t, index));
            }
        }
        self.met
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let (damage, tags, knockback, typed) = (
            contact.damage,
            contact.tags.clone(),
            contact.knockback,
            contact.typed.clone(),
        );
        for &(t, index) in &self.met {
            let entry = grid.entries()[index as usize];
            let contact = world.get_mut::<ContactDamage>(hitter).unwrap();
            if !contact.due(entry.entity) {
                continue;
            }
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
                    typed: typed.as_deref(),
                    knockback,
                },
            );
        }
        world.get_mut::<ContactDamage>(hitter).unwrap().forget();
    }
}

impl ContactDamage {
    /// Records that `target` met the item this step; whether it is due a
    /// hit: new, or an interval past its last hit, which it then records.
    fn due(&mut self, target: Entity) -> bool {
        let clock = self.clock;
        let interval = f64::from(self.interval);
        let at = self
            .timed
            .partition_point(|timed| timed.target.id() < target.id());
        let known = self.timed[at..]
            .iter_mut()
            .take_while(|timed| timed.target.id() == target.id())
            .find(|timed| timed.target == target);
        if let Some(timed) = known {
            timed.met = clock;
            let due = clock - timed.hit + f64::from(TIME_EPSILON) >= interval;
            if due {
                timed.hit = clock;
            }
            due
        } else {
            self.timed.insert(
                at,
                Timed {
                    target,
                    hit: clock,
                    met: clock,
                },
            );
            true
        }
    }

    /// Drops each target no step has met for a whole interval.
    fn forget(&mut self) {
        let (clock, interval) = (self.clock, f64::from(self.interval));
        self.timed
            .retain(|timed| clock - timed.met + f64::from(TIME_EPSILON) < interval);
    }
}

impl Inspectable for ContactDamage {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let shape = match self.own {
            Some((shape, offset)) => format!("{} at {}", shape.summary(), point(offset)),
            None => "the hurtbox's".to_string(),
        };
        vec![
            ("contact", shape),
            (
                "damage",
                format!(
                    "{} every {:.2} s",
                    damage_row(self.damage, self.parts()),
                    self.interval
                ),
            ),
            ("timed", format!("{} targets", self.timed.len())),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/contact.rs"]
mod tests;
