//! Health and damage: an invulnerability window counted on the fixed-step
//! clock, the `Damaged` event and a hit flash on the `damage_flash`
//! material.

use serde::{Deserialize, Serialize};
use tungsten_core::{
    Entity, EventQueue, Inspectable, MaterialAssetId, MaterialRegistry, Plugin, Schedule,
    SpawnContext, Sprite, Time, UniformOverrideBlock, World, system,
};

use crate::count_down;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN,
    describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{finite_color, non_negative, positive};

/// [`HealthPlugin`]'s system that counts invulnerability windows down, in
/// the tick phase.
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
/// [`damage`] lowers it and [`heal`](Self::heal) raises it;
/// [`HealthPlugin`]'s [`HEALTH_TICK`] counts the window down by the fixed
/// step, so a hit sequence on the fixed-step clock lands the same hits
/// whatever the frame rate, and a paused game, which runs no step, holds
/// every window. It is data: a game's save can hold it.
///
/// Scene name `health`: `{ "max": 5, "invulnerability": 0.5 }`,
/// `invulnerability` 0 by default.
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
}

impl TryFrom<HealthFields> for Health {
    type Error = String;

    fn try_from(fields: HealthFields) -> Result<Self, Self::Error> {
        let HealthFields {
            current,
            max,
            invulnerability,
            window_left,
        } = fields;
        if !(max.is_finite() && max > 0.0) {
            return Err(format!("max: must be finite and > 0 (got {max})"));
        }
        if !(current.is_finite() && (0.0..=max).contains(&current)) {
            return Err(format!("current: must be within 0..={max} (got {current})"));
        }
        for (field, value) in [
            ("invulnerability", invulnerability),
            ("window_left", window_left),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            current,
            max,
            invulnerability,
            window_left,
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

    /// A hit's window is still open, so [`damage`] does nothing.
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
}

/// What [`damage`] did. A match outside this crate needs a wildcard arm,
/// since later outcomes are additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hit {
    /// Nothing: an amount of 0, a target without [`Health`], a dead one or
    /// one inside its invulnerability window.
    Ignored,
    /// Health fell and some is left.
    Hurt,
    /// Health fell to 0.
    Killed,
}

/// A hit [`damage`] applied, sent once [`HealthPlugin`] has registered its
/// queue: damage numbers, a hurt sound, a score. Build one with
/// [`Damaged::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Damaged {
    /// The entity hit.
    pub entity: Entity,
    /// The amount passed to [`damage`], which may exceed the health it
    /// took.
    pub amount: f32,
    /// The hit left no health.
    pub killed: bool,
}

impl Damaged {
    /// An event with these fields, as [`damage`] sends one.
    #[must_use]
    pub fn new(entity: Entity, amount: f32, killed: bool) -> Self {
        Self {
            entity,
            amount,
            killed,
        }
    }
}

/// Damages `target` by `amount` at once, so the caller knows the outcome
/// in the same step. An amount of 0, a target without [`Health`], a dead
/// one or one inside its invulnerability window is [`Hit::Ignored`].
/// Otherwise its health falls by `amount`, to no less than 0, its window
/// starts, its [`HitFlash`] starts when it has one and a `Sprite`, and a
/// [`Damaged`] is sent when [`HealthPlugin`] has registered its queue.
/// See [`Health`] for an example.
///
/// # Panics
///
/// When `amount` is not finite and at least 0.
#[track_caller]
pub fn damage(world: &mut World, target: Entity, amount: f32) -> Hit {
    assert!(
        amount.is_finite() && amount >= 0.0,
        "damage: amount must be finite and >= 0 (got {amount})"
    );
    if amount == 0.0 {
        return Hit::Ignored;
    }
    let Some(health) = world.get_mut::<Health>(target) else {
        return Hit::Ignored;
    };
    if health.is_dead() || health.is_invulnerable() {
        return Hit::Ignored;
    }
    health.current = (health.current - amount).max(0.0);
    health.window_left = health.invulnerability;
    let killed = health.is_dead();
    start_flash(world, target);
    if world.has_event::<Damaged>() {
        world
            .resource_mut::<EventQueue<Damaged>>()
            .send(Damaged::new(target, amount, killed));
    }
    if killed { Hit::Killed } else { Hit::Hurt }
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

/// Starts the flash on `target` when it has a [`HitFlash`] and a `Sprite`.
fn start_flash(world: &mut World, target: Entity) {
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

/// Registers `EventQueue<Damaged>`, the `health` and `hit_flash` scene
/// components, [`HEALTH_TICK`] in the tick phase and [`HIT_FLASH_TICK`] in
/// the present phase, which runs after `tween_tick`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Health>(world, "Health");
        register_rows::<HitFlash>(world, "HitFlash");
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
            .event("Damaged")
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
        world.register_event::<Damaged>();
        world.register_component_with(HEALTH, |def: HealthDef, _: &SpawnContext<'_>| {
            (Health::new(def.max).with_invulnerability(def.invulnerability),)
        });
        world.register_component_with(HIT_FLASH, |def: HitFlashDef, _: &SpawnContext<'_>| {
            (HitFlash::new(def.color, def.seconds),)
        });
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(system(HEALTH_TICK, health_tick)),
        );
        let mut flashes = Flashes::default();
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(HIT_FLASH_TICK, move |world: &mut World| {
                flashes.run(world);
            })),
        );
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

fn health_tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    for health in world.query_mut::<&mut Health>() {
        health.window_left = count_down(health.window_left, dt);
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
        vec![
            ("hp", format!("{} / {}", self.current, self.max)),
            ("window", format!("{:.2} s", self.window_left)),
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
