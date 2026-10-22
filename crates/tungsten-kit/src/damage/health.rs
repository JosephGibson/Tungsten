//! Health and damage: an invulnerability window counted on the fixed-step
//! clock, the `Damaged` event, a death policy and a hit flash on the
//! `damage_flash` material.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{
    Entity, Inspectable, MaterialAssetId, MaterialRegistry, Plugin, Schedule, SpawnContext, Sprite,
    Time, UniformOverrideBlock, World, system,
};

use crate::count_down;
use crate::damage::defense::{
    RESIST, Resist, ResistDef, SHIELD, SHIELD_TICK, Shield, ShieldDef, resist_description,
    shield_description, shield_tick,
};
use crate::damage::kinds::{DamageKinds, register_types};
use crate::damage::pipeline::{DAMAGE_RESOLVE, DamageRequest, Healed, Requests, Resolver};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN,
    describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::check_declared;
use crate::foundation::{finite_color, non_negative, optional_non_negative, positive};
use crate::lifecycle::ended::{end, ensure_kit_despawn};

/// [`HealthPlugin`]'s system that counts invulnerability windows and death
/// delays down, in the tick phase.
pub const HEALTH_TICK: &str = "health_tick";

/// [`HealthPlugin`]'s system that fades hit flashes, in the present phase.
pub const HIT_FLASH_TICK: &str = "hit_flash_tick";

/// The scene components the plugin registers.
const HEALTH: &str = "health";
const HIT_FLASH: &str = "hit_flash";

/// The manifest material a [`HitFlash`] draws through. Its shader mixes the
/// sprite toward `vec4[0]`'s colour, weighted by its alpha, by the amount
/// in `f32s[0]`, which the flash lowers from 1 to 0. This repository's root
/// manifest holds it: a game copies `assets/shaders/damage_flash.wgsl` and
/// the `damage_flash` shader and material entries of `assets/manifest.json`.
pub const HIT_FLASH_MATERIAL: &str = "damage_flash";

/// An entity's health, with an invulnerability window after each hit.
/// [`damage`] and the damage pipeline ([`DamageRequest`](crate::DamageRequest))
/// lower it, and [`heal`](Self::heal) and [`heal`](crate::heal()) raise it;
/// [`HealthPlugin`]'s [`HEALTH_TICK`] counts the window down by the fixed
/// step, so a hit sequence on the fixed-step clock lands the same hits
/// whatever the frame rate, and a paused game, which runs no step, holds
/// every window.
///
/// A dead entity stays until something despawns it, unless its death
/// policy ([`with_despawn_on_death`](Self::with_despawn_on_death)) says
/// otherwise: then a kill starts a delay that [`HEALTH_TICK`] counts from
/// the step after the kill, and once it has passed the kit's end-of-step
/// despawn takes the entity with its children, after the `killed` cues
/// have run. It is data: a game's save can hold it, a delay that runs
/// included.
///
/// Scene name `health`: `{ "max": 5, "invulnerability": 0.5, "despawn_on_death": 0.1 }`,
/// `invulnerability` 0 by default and `despawn_on_death` none, so the
/// entity is kept.
///
/// # Examples
///
/// ```
/// use tungsten_core::{EventQueue, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Damaged, Health, HealthPlugin, Hit, damage};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let slime = world.spawn_with((Health::new(3.0).with_invulnerability(0.5),));
/// assert_eq!(damage(&mut world, slime, 1.0), Hit::Hurt);
/// // Inside its window, the next hit does nothing.
/// assert_eq!(damage(&mut world, slime, 1.0), Hit::Ignored);
/// let sent: Vec<Damaged> = world.resource::<EventQueue<Damaged>>().iter().copied().collect();
/// assert_eq!(sent, [Damaged::new(slime, 1.0, false)]);
///
/// // Thirty steps of 1/60 s end the window.
/// for _ in 0..30 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// assert_eq!(damage(&mut world, slime, 5.0), Hit::Killed);
/// let health = world.get::<Health>(slime).unwrap();
/// assert!(health.is_dead() && health.current() == 0.0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "HealthFields")]
pub struct Health {
    current: f32,
    max: f32,
    invulnerability: f32,
    window_left: f32,
    /// Seconds from a kill to the despawn; `None` keeps the dead.
    #[serde(skip_serializing_if = "Option::is_none")]
    despawn_on_death: Option<f32>,
    /// The delay left since the kill, while it runs or once it has passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    death_left: Option<f32>,
}

/// A health as data writes it, before its values are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthFields {
    current: f32,
    max: f32,
    #[serde(default)]
    invulnerability: f32,
    #[serde(default)]
    window_left: f32,
    #[serde(default)]
    despawn_on_death: Option<f32>,
    #[serde(default)]
    death_left: Option<f32>,
}

impl TryFrom<HealthFields> for Health {
    type Error = String;

    fn try_from(fields: HealthFields) -> Result<Self, Self::Error> {
        let HealthFields {
            current,
            max,
            invulnerability,
            window_left,
            despawn_on_death,
            death_left,
        } = fields;
        if !(max.is_finite() && max > 0.0) {
            return Err(format!("max: must be finite and > 0 (got {max})"));
        }
        if !(current.is_finite() && (0.0..=max).contains(&current)) {
            return Err(format!("current: must be within 0..={max} (got {current})"));
        }
        for (field, value) in [
            ("invulnerability", Some(invulnerability)),
            ("window_left", Some(window_left)),
            ("despawn_on_death", despawn_on_death),
            ("death_left", death_left),
        ] {
            if let Some(value) = value
                && !(value.is_finite() && value >= 0.0)
            {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            current,
            max,
            invulnerability,
            window_left,
            despawn_on_death,
            death_left,
        })
    }
}

impl Health {
    /// Full health of `max`, with no invulnerability window.
    ///
    /// # Panics
    ///
    /// When `max` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(max: f32) -> Self {
        assert!(
            max.is_finite() && max > 0.0,
            "Health::new: max must be finite and > 0 (got {max})"
        );
        Self {
            current: max,
            max,
            invulnerability: 0.0,
            window_left: 0.0,
            despawn_on_death: None,
            death_left: None,
        }
    }

    /// The health with an invulnerability window of `seconds` of game time
    /// after each hit.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_invulnerability(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "Health::with_invulnerability: seconds must be finite and >= 0 (got {seconds})"
        );
        self.invulnerability = seconds;
        self
    }

    /// The health with a death policy: `seconds` of game time after a kill,
    /// counted from the step after it, the kit despawns the entity with its
    /// children, its `killed` cues having run first; 0 despawns it the step
    /// after the kill. Without one the dead stay until something despawns
    /// them.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_despawn_on_death(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "Health::with_despawn_on_death: seconds must be finite and >= 0 (got {seconds})"
        );
        self.despawn_on_death = Some(seconds);
        self
    }

    /// The death policy's delay, when it has one.
    #[must_use]
    pub fn despawn_on_death(&self) -> Option<f32> {
        self.despawn_on_death
    }

    /// The health left, from 0 to [`max`](Self::max).
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// The full health.
    #[must_use]
    pub fn max(&self) -> f32 {
        self.max
    }

    /// No health is left.
    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    /// A hit's window is still open, so [`damage`] does nothing; a periodic
    /// request ([`DamageRequest::with_periodic`](crate::DamageRequest::with_periodic))
    /// passes it.
    #[must_use]
    pub fn is_invulnerable(&self) -> bool {
        self.window_left > 0.0
    }

    /// Adds `amount`, up to [`max`](Self::max); the dead stay dead.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[track_caller]
    pub fn heal(&mut self, amount: f32) {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Health::heal: amount must be finite and >= 0 (got {amount})"
        );
        if !self.is_dead() {
            self.current = (self.current + amount).min(self.max);
        }
    }

    /// Takes `amount` off the health, to no less than 0, starts the window
    /// unless the hit is `periodic`, and on a kill starts the death policy's
    /// delay; whether it killed.
    pub(crate) fn hurt(&mut self, amount: f32, periodic: bool) -> bool {
        self.current = (self.current - amount).max(0.0);
        if !periodic {
            self.window_left = self.invulnerability;
        }
        let killed = self.is_dead();
        if killed && let Some(delay) = self.despawn_on_death {
            self.death_left = Some(delay);
        }
        killed
    }

    /// Adds up to `amount`, up to [`max`](Self::max); the health restored,
    /// 0 for the dead.
    pub(crate) fn restore(&mut self, amount: f32) -> f32 {
        if self.is_dead() {
            return 0.0;
        }
        let before = self.current;
        self.current = (self.current + amount).min(self.max);
        self.current - before
    }

    /// Sets the full health to `max`, the health left kept as the same
    /// fraction of it, so 5 of 5 at a max of 8 becomes 8 of 8 and the dead
    /// stay dead: what a `max_health` stat writes. A max that is not finite
    /// and above 0 is ignored.
    pub(crate) fn set_max(&mut self, max: f32) {
        if max == self.max || !(max.is_finite() && max > 0.0) {
            return;
        }
        self.current = (self.current / self.max * max).clamp(0.0, max);
        self.max = max;
    }
}

/// What [`damage`] or a [`DamageRequest`](crate::DamageRequest) did. A
/// match outside this crate needs a wildcard arm, since later outcomes are
/// additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hit {
    /// Nothing: an amount of 0, a target that is gone or being despawned,
    /// one without [`Health`], a dead one, one inside its invulnerability
    /// window, or a hit whose every part dropped: of 0, or resisted to 0 or
    /// by an immunity of its [`Resist`](crate::Resist).
    Ignored,
    /// Health fell and some is left.
    Hurt,
    /// Health fell to 0.
    Killed,
    /// The [`Shield`](crate::Shield) took it all: health did not fall.
    Absorbed,
}

/// A hit the damage pipeline applied, sent once [`HealthPlugin`] has
/// registered its queue, after its parts' [`DamagedPart`]s: damage numbers,
/// a hurt sound, a score. Build one with [`Damaged::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Damaged {
    /// The entity hit.
    pub entity: Entity,
    /// The parts' total after the target's resistances, which may exceed
    /// the health it took: a shield may have taken some.
    pub amount: f32,
    /// The hit left no health.
    pub killed: bool,
    /// The entity the hit came from, when the request named one: the
    /// `damaged` and `killed` cue events' other entity.
    pub source: Option<Entity>,
    /// Where the hit landed, when the request said: where a cue spawns or
    /// floats its label, else at the entity's point.
    pub position: Option<Vec2>,
    /// The request was a critical hit.
    pub crit: bool,
    /// The request was periodic: an effect's payload, which passed the
    /// window and started none.
    pub periodic: bool,
    /// The damage types of every part that landed, together; none for an
    /// untyped hit.
    pub kinds: DamageKinds,
}

impl Damaged {
    /// An event with these fields, no source, no position, no crit, not
    /// periodic and no kind, as [`damage`] sends one.
    #[must_use]
    pub fn new(entity: Entity, amount: f32, killed: bool) -> Self {
        Self {
            entity,
            amount,
            killed,
            source: None,
            position: None,
            crit: false,
            periodic: false,
            kinds: DamageKinds::NONE,
        }
    }

    /// The event, periodic when `periodic`.
    #[must_use]
    pub fn with_periodic(mut self, periodic: bool) -> Self {
        self.periodic = periodic;
        self
    }

    /// The event, of the damage types `kinds`.
    #[must_use]
    pub fn with_kinds(mut self, kinds: DamageKinds) -> Self {
        self.kinds = kinds;
        self
    }

    /// The event, from `source`.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The event, landing at `position`.
    #[must_use]
    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = Some(position);
        self
    }

    /// The event, a critical hit when `crit`.
    #[must_use]
    pub fn with_crit(mut self, crit: bool) -> Self {
        self.crit = crit;
        self
    }
}

/// One part of a hit the damage pipeline applied, sent once
/// [`HealthPlugin`] has registered its queue, before the hit's
/// [`Damaged`]: the cue event `damaged_part`, whose subject is the entity
/// hit and whose other is the source. An untyped part sends one too, with
/// [`DamageKinds::NONE`]. Build one with [`DamagedPart::new`].
///
/// # Examples
///
/// ```
/// use tungsten_core::{EventQueue, Plugin, Schedule, World};
/// use tungsten_kit::{DamagePart, DamageRequest, DamagedPart, Health, HealthPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// let slime = world.spawn_with((Health::new(10.0),));
/// DamageRequest::from_parts(slime, [DamagePart::new(2.0), DamagePart::new(0.0)]).apply(&mut world);
/// let parts: Vec<DamagedPart> = world.resource::<EventQueue<DamagedPart>>().iter().copied().collect();
/// assert_eq!(parts, [DamagedPart::new(slime, 2.0)], "a part of 0 drops");
/// assert!(parts[0].kinds.is_empty());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct DamagedPart {
    /// The entity hit.
    pub entity: Entity,
    /// The part's amount after the target's resistances.
    pub amount: f32,
    /// The part's damage types.
    pub kinds: DamageKinds,
    /// The entity the hit came from, when the request named one.
    pub source: Option<Entity>,
    /// Where the hit landed, when the request said.
    pub position: Option<Vec2>,
    /// The request was periodic: an effect's payload.
    pub periodic: bool,
    /// The part's hit was a critical one
    /// ([`DamageRequest::with_crit`](crate::DamageRequest::with_crit)).
    pub crit: bool,
}

impl DamagedPart {
    /// An event with these fields, no kind, no source, no position, not
    /// periodic and no crit.
    #[must_use]
    pub fn new(entity: Entity, amount: f32) -> Self {
        Self {
            entity,
            amount,
            kinds: DamageKinds::NONE,
            source: None,
            position: None,
            periodic: false,
            crit: false,
        }
    }

    /// The event, periodic when `periodic`.
    #[must_use]
    pub fn with_periodic(mut self, periodic: bool) -> Self {
        self.periodic = periodic;
        self
    }

    /// The event, a critical hit's part when `crit`.
    #[must_use]
    pub fn with_crit(mut self, crit: bool) -> Self {
        self.crit = crit;
        self
    }

    /// The event, of the damage types `kinds`.
    #[must_use]
    pub fn with_kinds(mut self, kinds: DamageKinds) -> Self {
        self.kinds = kinds;
        self
    }

    /// The event, from `source`.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The event, landing at `position`.
    #[must_use]
    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = Some(position);
        self
    }
}

/// One part of a hit that the target's immunity dropped: a type its
/// [`Resist`](crate::Resist) holds at a factor of 0, a modifier's 0
/// included. Sent once [`HealthPlugin`] has registered its queue, in part
/// order, whether the hit's other parts land or the hit is ignored, so a
/// game can still react to the type that struck (fire on a frozen
/// fire-proof creature). A part of 0, a part a factor above 0 brings to
/// nothing, and a target that takes no hits (gone, dead or, unless the
/// request is periodic, inside its window) send none; nothing else of the
/// hit happens for it, and it is no cue event. Build one with
/// [`ImmunePart::new`].
///
/// # Examples
///
/// ```
/// use tungsten_core::{EventQueue, Plugin, Schedule, World};
/// use tungsten_kit::{
///     DamagePart, DamageRequest, DamagedPart, Health, HealthPlugin, Hit, ImmunePart, Resist,
///     Tags,
/// };
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// let ember = world.spawn_with((Health::new(10.0), Resist::new().with_factor("fire", 0.0)));
/// let fire = DamagePart::new(3.0).with_tags(Tags::new(["fire"]));
/// assert_eq!(DamageRequest::from_parts(ember, [fire]).apply(&mut world), Hit::Ignored);
/// let immune: Vec<ImmunePart> = world.resource::<EventQueue<ImmunePart>>().iter().copied().collect();
/// assert_eq!(immune.len(), 1);
/// assert_eq!((immune[0].entity, immune[0].amount), (ember, 3.0), "its amount before resistances");
/// assert!(world.resource::<EventQueue<DamagedPart>>().iter().next().is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ImmunePart {
    /// The entity struck.
    pub entity: Entity,
    /// The part's amount before the target's resistances.
    pub amount: f32,
    /// The part's damage types.
    pub kinds: DamageKinds,
    /// The entity the hit came from, when the request named one.
    pub source: Option<Entity>,
    /// Where the hit landed, when the request said.
    pub position: Option<Vec2>,
    /// The request was periodic: an effect's payload.
    pub periodic: bool,
    /// The part's hit was a critical one
    /// ([`DamageRequest::with_crit`](crate::DamageRequest::with_crit)).
    pub crit: bool,
}

impl ImmunePart {
    /// An event with these fields, no kind, no source, no position, not
    /// periodic and no crit.
    #[must_use]
    pub fn new(entity: Entity, amount: f32) -> Self {
        Self {
            entity,
            amount,
            kinds: DamageKinds::NONE,
            source: None,
            position: None,
            periodic: false,
            crit: false,
        }
    }

    /// The event, periodic when `periodic`.
    #[must_use]
    pub fn with_periodic(mut self, periodic: bool) -> Self {
        self.periodic = periodic;
        self
    }

    /// The event, a critical hit's part when `crit`.
    #[must_use]
    pub fn with_crit(mut self, crit: bool) -> Self {
        self.crit = crit;
        self
    }

    /// The event, of the damage types `kinds`.
    #[must_use]
    pub fn with_kinds(mut self, kinds: DamageKinds) -> Self {
        self.kinds = kinds;
        self
    }

    /// The event, from `source`.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The event, landing at `position`.
    #[must_use]
    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = Some(position);
        self
    }
}

/// Damages `target` by `amount` at once, so the caller knows the outcome
/// in the same step: `DamageRequest::new(target, amount).apply(world)`,
/// one untyped part with no source, tags, position, knockback or crit. An amount of 0, a
/// target that is gone or being despawned, one without [`Health`], a dead
/// one or one inside its invulnerability window is [`Hit::Ignored`].
/// Otherwise its [`Resist`](crate::Resist) and [`Shield`](crate::Shield)
/// apply, its health falls by the rest, to no less than 0, its window
/// starts, its [`HitFlash`] starts when it has one and a `Sprite`, and a
/// [`DamagedPart`] and a [`Damaged`] are sent when [`HealthPlugin`] has
/// registered their queues. The World's [`KitTrace`](crate::KitTrace)
/// records the part, the hit, and the kill.
/// See [`Health`] for an example.
///
/// # Panics
///
/// When `amount` is not finite and at least 0.
#[track_caller]
pub fn damage(world: &mut World, target: Entity, amount: f32) -> Hit {
    DamageRequest::new(target, amount).apply(world)
}

/// A sprite that flashes a colour when [`damage`] hits its entity, drawn
/// through the [`HIT_FLASH_MATERIAL`] material, which fades it out over its
/// seconds of game time.
///
/// At a hit on an entity with a `HitFlash` and a `Sprite`, the flash keeps
/// the sprite's `material_id` and the entity's [`UniformOverrideBlock`], or
/// that it had none, once per flash; a hit while it flashes starts it over.
/// It then sets the material to `damage_flash`'s and writes the colour into
/// `vec4[0]` and 1 into `f32s[0]` of the block, inserting one if absent.
/// [`HIT_FLASH_TICK`] lowers `f32s[0]` linearly to 0, leaving the first
/// frame at full strength, then restores the kept material and block and
/// removes a block the flash inserted, so the material's own defaults
/// apply again. While it flashes, the flash owns the sprite's material and
/// block: a change to either then is lost at the restore, and a game
/// `Tween` on the block loses to the flash, which runs after
/// `tween_tick`.
///
/// Without `damage_flash` in the `MaterialRegistry` nothing flashes and
/// the tick warns once. A lit sprite draws without its material, so it
/// does not flash, and a flashing sprite is a batch of its own.
///
/// Scene name `hit_flash`: `{ "color": [1, 1, 1, 1], "seconds": 0.1 }`,
/// both optional with those defaults.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use tungsten_core::{
///     MaterialRegistry, MaterialUniformDefaults, Plugin, Schedule, Sprite, SpriteAssetId, Stage,
///     Time, UniformOverrideBlock, World,
/// };
/// use tungsten_kit::{HIT_FLASH_MATERIAL, Health, HealthPlugin, HitFlash, damage};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut materials = MaterialRegistry::new();
/// let flash = materials.allocate(
///     HIT_FLASH_MATERIAL,
///     PathBuf::from("assets/manifest.json"),
///     "damage_flash".to_string(),
///     MaterialUniformDefaults::default(),
/// );
/// world.insert_resource(materials);
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let bat = world.spawn_with((
///     Health::new(2.0),
///     HitFlash::new([1.0, 1.0, 1.0, 1.0], 0.2),
///     Sprite::new(SpriteAssetId::new(0)),
/// ));
/// damage(&mut world, bat, 1.0);
/// assert_eq!(world.get::<Sprite>(bat).unwrap().material_id, Some(flash));
///
/// // Frames of 0.1 s: full, half, then over, with the sprite as it was.
/// let mut amounts = Vec::new();
/// for _ in 0..3 {
///     world.resource_mut::<Time>().advance_frame(0.1);
///     schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
///     amounts.push(world.get::<UniformOverrideBlock>(bat).map(|block| block.f32s[0]));
/// }
/// assert_eq!(amounts, [Some(1.0), Some(0.5), None]);
/// assert_eq!(world.get::<Sprite>(bat).unwrap().material_id, None);
/// assert!(!world.get::<HitFlash>(bat).unwrap().is_active());
/// ```
#[derive(Debug, Clone)]
pub struct HitFlash {
    color: [f32; 4],
    seconds: f32,
    left: f32,
    /// The first tick leaves the flash at full strength.
    fresh: bool,
    /// What the flash replaced, while it runs.
    kept: Option<Kept>,
    /// A hit found no `damage_flash` material, for the tick to warn about.
    missing: bool,
}

/// The sprite's own material and block, kept while it flashes.
#[derive(Debug, Clone, Copy)]
struct Kept {
    flash: MaterialAssetId,
    material: Option<MaterialAssetId>,
    /// `None` when the entity had no block.
    block: Option<UniformOverrideBlock>,
}

impl HitFlash {
    /// A flash of `color`, linear RGBA whose alpha weighs the mix, fading
    /// over `seconds` of game time.
    ///
    /// # Panics
    ///
    /// When a colour channel is not finite, or `seconds` is not finite and
    /// above 0.
    #[must_use]
    #[track_caller]
    pub fn new(color: [f32; 4], seconds: f32) -> Self {
        assert!(
            color.iter().all(|channel| channel.is_finite()),
            "HitFlash::new: color must be finite (got {color:?})"
        );
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "HitFlash::new: seconds must be finite and > 0 (got {seconds})"
        );
        Self {
            color,
            seconds,
            left: 0.0,
            fresh: false,
            kept: None,
            missing: false,
        }
    }

    /// It is flashing.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.kept.is_some()
    }
}

/// Starts the flash on `target` when it has a [`HitFlash`] and a `Sprite`:
/// a hit's, or a cue's `flash` action.
pub(crate) fn start_flash(world: &mut World, target: Entity) {
    let Some(material_id) = world.get::<Sprite>(target).map(|sprite| sprite.material_id) else {
        return;
    };
    if !world.has::<HitFlash>(target) {
        return;
    }
    let Some(flash_material) = world
        .get_resource::<MaterialRegistry>()
        .and_then(|materials| materials.get(HIT_FLASH_MATERIAL))
    else {
        world.get_mut::<HitFlash>(target).unwrap().missing = true;
        return;
    };
    let block = world.get::<UniformOverrideBlock>(target).copied();
    let flash = world.get_mut::<HitFlash>(target).unwrap();
    if flash.kept.is_none() {
        flash.kept = Some(Kept {
            flash: flash_material,
            material: material_id,
            block,
        });
    }
    flash.left = flash.seconds;
    flash.fresh = true;
    let color = flash.color;
    world.get_mut::<Sprite>(target).unwrap().material_id = Some(flash_material);
    let mut block = block.unwrap_or_default();
    block.vec4[0] = color;
    block.f32s[0] = 1.0;
    world.insert(target, block);
}

/// Registers the damage pipeline: `EventQueue<Damaged>`,
/// `EventQueue<DamagedPart>`, `EventQueue<ImmunePart>` and
/// `EventQueue<Healed>`, the request queue,
/// the `damage_types` data kind with the World's type table, the `health`,
/// `hit_flash`, `resist`, `shield`, `penetration` and `conversion` scene
/// components, [`HEALTH_TICK`] and
/// [`SHIELD_TICK`] in the tick phase, [`DAMAGE_RESOLVE`] in the resolve
/// phase, [`HIT_FLASH_TICK`] in the present phase, which runs after
/// `tween_tick`, and the kit's end-of-step despawn when the schedule lacks
/// it, for the death policy.
#[derive(Debug, Default, Clone, Copy)]
pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Health>(world, "Health");
        register_rows::<HitFlash>(world, "HitFlash");
        register_rows::<Resist>(world, "Resist");
        register_rows::<Shield>(world, "Shield");
        describe(
            world,
            ComponentDescription::new(
                HEALTH,
                "Health with an invulnerability window after each hit, which `damage` lowers.",
            )
            .field(FieldDescription::new(
                "max",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the full health, which it starts at",
            ))
            .field(
                FieldDescription::new(
                    "invulnerability",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds of game time a hit is ignored after one lands",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "despawn_on_death",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "game seconds from a kill, counted from the step after it, until the kit despawns the entity after its `killed` cues; kept when left out",
                )
                .default("null"),
            )
            .event("Damaged")
            .event("DamagedPart")
            .event("ImmunePart")
            .event("Healed")
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "max": 5 }"#)
            .roundtrip(round_trip::<HealthDef>),
        );
        describe(
            world,
            ComponentDescription::new(
                HIT_FLASH,
                "A flash of colour on the sprite when `damage` hits the entity, through the `damage_flash` material.",
            )
            .field(
                FieldDescription::new(
                    "color",
                    FieldType::Color,
                    "the flash's colour; its alpha weighs the mix",
                )
                .default("[1.0, 1.0, 1.0, 1.0]"),
            )
            .field(
                FieldDescription::new(
                    "seconds",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "how long the flash fades over",
                )
                .default("0.1"),
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample("{}")
            .roundtrip(round_trip::<HitFlashDef>),
        );
        describe(world, resist_description());
        describe(world, shield_description());
        register_types(world);
        world.register_event::<Damaged>();
        world.register_event::<DamagedPart>();
        world.register_event::<ImmunePart>();
        world.register_event::<Healed>();
        if !world.has_resource::<Requests>() {
            world.insert_resource(Requests::default());
        }
        world.register_component_with(HEALTH, |def: HealthDef, _: &SpawnContext<'_>| {
            let health = Health::new(def.max).with_invulnerability(def.invulnerability);
            match def.despawn_on_death {
                Some(seconds) => (health.with_despawn_on_death(seconds),),
                None => (health,),
            }
        });
        world.register_component_with(HIT_FLASH, |def: HitFlashDef, _: &SpawnContext<'_>| {
            (HitFlash::new(def.color, def.seconds),)
        });
        world.register_component_with_assets(
            RESIST,
            |def: &ResistDef, world: &World| {
                for tag in def.0.keys() {
                    check_declared(world, tag, tag)?;
                }
                Ok(())
            },
            |def: ResistDef, _: &SpawnContext<'_>| (Resist::from(def),),
        );
        world.register_component_with(SHIELD, |def: ShieldDef, _: &SpawnContext<'_>| {
            (Shield::new(def.max).with_recharge(def.recharge, def.delay),)
        });
        let mut dying: Vec<Entity> = Vec::new();
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(system(HEALTH_TICK, move |world: &mut World| {
                health_tick(world, &mut dying);
            })),
        );
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(system(SHIELD_TICK, shield_tick)),
        );
        let mut resolver = Resolver::default();
        schedule.add(
            Phase::Resolve.stage(),
            Phase::Resolve.join(system(DAMAGE_RESOLVE, move |world: &mut World| {
                resolver.run(world);
            })),
        );
        let mut flashes = Flashes::default();
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(HIT_FLASH_TICK, move |world: &mut World| {
                flashes.run(world);
            })),
        );
        ensure_kit_despawn(schedule, world);
    }
}

/// `components.health` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthDef {
    #[serde(deserialize_with = "positive")]
    max: f32,
    #[serde(default, deserialize_with = "non_negative")]
    invulnerability: f32,
    #[serde(
        default,
        deserialize_with = "optional_non_negative",
        skip_serializing_if = "Option::is_none"
    )]
    despawn_on_death: Option<f32>,
}

/// `components.hit_flash` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HitFlashDef {
    #[serde(default = "default_flash_color", deserialize_with = "finite_color")]
    color: [f32; 4],
    #[serde(default = "default_flash_seconds", deserialize_with = "positive")]
    seconds: f32,
}

fn default_flash_color() -> [f32; 4] {
    [1.0; 4]
}

fn default_flash_seconds() -> f32 {
    0.1
}

/// Counts every window down by the step and every death delay that runs,
/// listing for the kit's despawn each entity whose delay has passed, the
/// step after the kill for a delay of 0; `dying` is the scratch list.
fn health_tick(world: &mut World, dying: &mut Vec<Entity>) {
    let dt = world
        .get_resource::<Time>()
        .map_or(0.0, Time::delta)
        .max(0.0);
    dying.clear();
    for (entity, health) in world.query_mut::<(Entity, &mut Health)>() {
        if dt > 0.0 {
            health.window_left = count_down(health.window_left, dt);
        }
        if let Some(left) = health.death_left {
            let left = count_down(left, dt);
            health.death_left = Some(left);
            if left <= 0.0 {
                dying.push(entity);
            }
        }
    }
    // Listed each step until the flush; the despawn marks it the first
    // time and skips it after.
    for &entity in dying.iter() {
        end(world, entity, HEALTH);
    }
}

/// The flash tick's state between frames.
#[derive(Default)]
struct Flashes {
    warned: bool,
    /// Blocks the restore removes, kept so a frame allocates nothing once
    /// warm.
    remove: Vec<Entity>,
    /// Blocks the restore puts back after the game removed them.
    insert: Vec<(Entity, UniformOverrideBlock)>,
}

impl Flashes {
    fn run(&mut self, world: &mut World) {
        let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
        let mut missing = false;
        for (entity, flash, sprite, block) in world.query_mut::<(
            Entity,
            &mut HitFlash,
            Option<&mut Sprite>,
            Option<&mut UniformOverrideBlock>,
        )>() {
            missing |= std::mem::take(&mut flash.missing);
            let Some(kept) = flash.kept else {
                continue;
            };
            if std::mem::take(&mut flash.fresh) {
                // The hit's own frame shows the full flash.
            } else {
                flash.left = count_down(flash.left, dt);
            }
            if flash.left > 0.0 {
                if let Some(sprite) = sprite {
                    sprite.material_id = Some(kept.flash);
                }
                if let Some(block) = block {
                    block.vec4[0] = flash.color;
                    block.f32s[0] = flash.left / flash.seconds;
                }
                continue;
            }
            flash.kept = None;
            if let Some(sprite) = sprite {
                sprite.material_id = kept.material;
            }
            match (kept.block, block) {
                (Some(own), Some(block)) => *block = own,
                (Some(own), None) => self.insert.push((entity, own)),
                (None, Some(_)) => self.remove.push(entity),
                (None, None) => {}
            }
        }
        for entity in self.remove.drain(..) {
            world.remove_component::<UniformOverrideBlock>(entity);
        }
        for (entity, block) in self.insert.drain(..) {
            world.insert(entity, block);
        }
        if missing && !self.warned {
            self.warned = true;
            log::warn!(
                "HitFlash: no `{HIT_FLASH_MATERIAL}` material in the MaterialRegistry; \
                 nothing flashes"
            );
        }
    }
}

impl Inspectable for Health {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let death = match (self.despawn_on_death, self.death_left) {
            (None, _) => "kept".to_string(),
            (Some(_), Some(left)) => format!("despawns in {left:.2} s"),
            (Some(delay), None) => format!("despawns {delay:.2} s after a kill"),
        };
        vec![
            ("hp", format!("{} / {}", self.current, self.max)),
            ("window", format!("{:.2} s", self.window_left)),
            ("death", death),
        ]
    }
}

impl Inspectable for HitFlash {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let state = if self.is_active() {
            format!("{:.2} of {:.2} s", self.left, self.seconds)
        } else {
            format!("off, {:.2} s", self.seconds)
        };
        vec![("flash", state)]
    }
}

#[cfg(test)]
#[path = "../tests/health.rs"]
mod tests;
