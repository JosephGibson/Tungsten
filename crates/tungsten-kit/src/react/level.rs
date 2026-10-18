//! Levels: a counter climbed along a curve, each level up sending an event
//! and drawing offers from a weighted table that a choice applies as an
//! effect or an ability; the `offers` data kind and the `level` scene
//! component.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use tungsten_core::{
    DataRef, DataRegistry, Entity, EventQueue, Inspectable, MissingAsset, Plugin, Schedule,
    SpawnContext, World, system,
};

use crate::damage::effects::{EFFECTS, Effect, EffectRequest};
use crate::foundation::curve::{CURVES, Curve, register_curves};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, TOP_DOWN, describe,
    describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::rng::KitRng;
use crate::foundation::tags::valid_tag;
use crate::foundation::trace::{TraceKind, trace};
use crate::intent::ability::{ABILITIES, Abilities, Ability};
use crate::lifecycle::cooldown::Cooldown;
use crate::react::counters::Counters;
use crate::react::cues::CUES;

/// [`LevelPlugin`]'s system that climbs levels, draws offers and applies
/// choices, in the react phase before [`CUES`](crate::CUES) when it is
/// registered.
pub const LEVEL_TICK: &str = "level_tick";

/// The data kind of offers, and its shapes.
pub(crate) const OFFERS: &str = "offers";
const OFFER: &str = "offer";
const OFFER_GIVES: &str = "offer_gives";
/// The scene component.
const LEVEL: &str = "level";

/// The most levels one entity climbs in a step, as a cooldown bounds its
/// elapses; the rest carry to the next steps.
const MAX_LEVELS_PER_STEP: u32 = Cooldown::MAX_ELAPSES_PER_STEP;

/// What an offer gives: an effect, or an ability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Gives {
    Effect(DataRef<Effect>),
    Ability(DataRef<Ability>),
}

/// One offer: what it gives and its weight among the table's.
///
/// Data form: `{ "weight": 2, "gives": { "effect": "swift" } }` or
/// `{ "weight": 1, "gives": { "ability": "wand" } }`, each an entry's ID or
/// the definition in place.
///
/// # Examples
///
/// ```
/// use tungsten_core::DataRef;
/// use tungsten_kit::Offers;
///
/// let offers = Offers::new(2)
///     .with_effect(2.0, DataRef::by_id("swift"))
///     .with_ability(1.0, DataRef::by_id("wand"));
/// let [swift, wand] = offers.entries() else { panic!() };
/// assert_eq!((swift.weight(), swift.effect().and_then(|e| e.id())), (2.0, Some("swift")));
/// assert!(swift.ability().is_none() && wand.ability().is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OfferFields")]
pub struct Offer {
    weight: f32,
    gives: Gives,
}

/// An offer as data writes it, before its weight is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OfferFields {
    weight: f32,
    gives: Gives,
}

impl TryFrom<OfferFields> for Offer {
    type Error = String;

    fn try_from(fields: OfferFields) -> Result<Self, Self::Error> {
        if !(fields.weight.is_finite() && fields.weight > 0.0) {
            return Err(format!(
                "weight: must be finite and > 0 (got {})",
                fields.weight
            ));
        }
        Ok(Self {
            weight: fields.weight,
            gives: fields.gives,
        })
    }
}

impl Offer {
    /// Its share of the draws.
    #[must_use]
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// The effect it gives, when it gives one.
    #[must_use]
    pub fn effect(&self) -> Option<&DataRef<Effect>> {
        match &self.gives {
            Gives::Effect(effect) => Some(effect),
            Gives::Ability(_) => None,
        }
    }

    /// The ability it gives, when it gives one.
    #[must_use]
    pub fn ability(&self) -> Option<&DataRef<Ability>> {
        match &self.gives {
            Gives::Ability(ability) => Some(ability),
            Gives::Effect(_) => None,
        }
    }

    /// The load's check of what it gives, under `field`.
    fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        match &self.gives {
            Gives::Effect(effect) => effect.check(world, &format!("{field}.gives.effect")),
            Gives::Ability(ability) => ability.check(world, &format!("{field}.gives.ability")),
        }
    }

    /// What it gives, in a line.
    fn summary(&self) -> String {
        match &self.gives {
            Gives::Effect(effect) => format!("effect {}", effect.id().unwrap_or("inline")),
            Gives::Ability(ability) => format!("ability {}", ability.id().unwrap_or("inline")),
        }
    }
}

/// What a level up offers: `count` distinct entries drawn by weight without
/// replacement from the kit's random stream, fewer when the entries are
/// fewer. The `offers` kind's entry, or offers in place in a `level`.
///
/// Data form:
/// `{ "count": 3, "entries": [{ "weight": 2, "gives": { "effect": "swift" } }, { "weight": 1, "gives": { "ability": "wand" } }] }`,
/// `count` at least 1; the load checks each effect and ability.
///
/// # Examples
///
/// ```
/// use tungsten_core::DataRef;
/// use tungsten_kit::{KitRng, Offers};
///
/// let offers = Offers::new(2)
///     .with_effect(1.0, DataRef::by_id("a"))
///     .with_effect(1.0, DataRef::by_id("b"))
///     .with_effect(1.0, DataRef::by_id("c"));
/// assert_eq!((offers.count(), offers.entries().len()), (2, 3));
/// let mut rng = KitRng::seeded(3);
/// let drawn = offers.draw(&mut rng);
/// assert_eq!(drawn.len(), 2);
/// assert_ne!(drawn[0], drawn[1], "distinct");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OffersFields")]
pub struct Offers {
    count: u32,
    entries: Vec<Offer>,
}

/// Offers as data writes them, before the count is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OffersFields {
    count: u32,
    #[serde(default)]
    entries: Vec<Offer>,
}

impl TryFrom<OffersFields> for Offers {
    type Error = String;

    fn try_from(fields: OffersFields) -> Result<Self, Self::Error> {
        if fields.count < 1 {
            return Err("count: must be at least 1".to_string());
        }
        Ok(Self {
            count: fields.count,
            entries: fields.entries,
        })
    }
}

impl Offers {
    /// Offers of `count` draws and no entries.
    ///
    /// # Panics
    ///
    /// When `count` is 0.
    #[must_use]
    #[track_caller]
    pub fn new(count: u32) -> Self {
        assert!(count >= 1, "Offers::new: count must be at least 1");
        Self {
            count,
            entries: Vec::new(),
        }
    }

    /// The offers, with `effect` drawn by `weight`.
    ///
    /// # Panics
    ///
    /// When `weight` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_effect(self, weight: f32, effect: DataRef<Effect>) -> Self {
        self.push(weight, Gives::Effect(effect), "with_effect")
    }

    /// The offers, with `ability` drawn by `weight`.
    ///
    /// # Panics
    ///
    /// When `weight` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_ability(self, weight: f32, ability: DataRef<Ability>) -> Self {
        self.push(weight, Gives::Ability(ability), "with_ability")
    }

    #[track_caller]
    fn push(mut self, weight: f32, gives: Gives, method: &str) -> Self {
        assert!(
            weight.is_finite() && weight > 0.0,
            "Offers::{method}: weight must be finite and > 0 (got {weight})"
        );
        self.entries.push(Offer { weight, gives });
        self
    }

    /// How many distinct entries a level up draws.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// The entries, in order.
    #[must_use]
    pub fn entries(&self) -> &[Offer] {
        &self.entries
    }

    /// `count` distinct entries by weight without replacement, one number
    /// from `rng` per draw; every entry when there are no more than
    /// `count`.
    #[must_use]
    pub fn draw(&self, rng: &mut KitRng) -> Vec<Offer> {
        let mut pool: Vec<&Offer> = self.entries.iter().collect();
        let mut drawn = Vec::with_capacity((self.count as usize).min(pool.len()));
        while drawn.len() < self.count as usize && !pool.is_empty() {
            let total: f64 = pool.iter().map(|offer| f64::from(offer.weight)).sum();
            let mut left = f64::from(rng.unit()) * total;
            let mut at = pool.len() - 1;
            for (i, offer) in pool.iter().enumerate() {
                let weight = f64::from(offer.weight);
                if left < weight {
                    at = i;
                    break;
                }
                left -= weight;
            }
            drawn.push(pool.remove(at).clone());
        }
        drawn
    }

    /// The load's check: each entry's effect or ability.
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        for (i, offer) in self.entries.iter().enumerate() {
            offer.check(world, &format!("entries.{i}"))?;
        }
        Ok(())
    }
}

/// Whose counter a level reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CounterTarget {
    #[default]
    #[serde(rename = "self")]
    Itself,
    Game,
}

/// A level climbed along a curve: the curve maps a level to the total
/// count that reaches it, read from the entity's [`Counters`] or the
/// game's, and its last point's `x`, floored, is the top level.
/// [`LevelPlugin`]'s [`LEVEL_TICK`] raises the level while the count
/// reaches the next one, at most 16 a step, sending [`LevelUp`] each time
/// and drawing the [`Offers`]' `count` distinct entries, which replace any
/// pending; a game shows them and [`choose`](Self::choose)s one, which the
/// next tick applies: an effect as an [`EffectRequest`] on the entity,
/// queued, or an ability added to the entity's [`Abilities`] (inserted if
/// absent), or, when the entity owns that name, that entry refilled with
/// its cooldown cleared. It is data: a save holds the level, the pending
/// offers and the choice.
///
/// Scene name `level`:
/// `{ "counter": "xp", "target": "self", "curve": "hero_xp", "offers": "hero_offers" }`,
/// `target` `self` by default or `game`, `curve` a `curves` entry or a
/// curve in place, `offers` optional, an `offers` entry or offers in place;
/// the load checks both.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRef, EventQueue, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Counters, Curve, Level, LevelPlugin, LevelUp};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// LevelPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let xp = Curve::new([(1.0, 0.0), (2.0, 10.0), (3.0, 25.0)]);
/// let hero = world.spawn_with((
///     Level::new("xp", DataRef::inline(xp)),
///     Counters::new().with("xp", 12),
/// ));
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// let level = world.get::<Level>(hero).unwrap();
/// assert_eq!((level.level(), level.top()), (2, 3));
/// let sent: Vec<u32> = world.resource::<EventQueue<LevelUp>>().iter().map(|e| e.level).collect();
/// assert_eq!(sent, [2]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LevelFields", into = "LevelFields")]
pub struct Level {
    counter: String,
    target: CounterTarget,
    curve: DataRef<Curve>,
    offers: Option<DataRef<Offers>>,
    current: u32,
    pending: Vec<Offer>,
    chosen: Option<u32>,
}

/// A level as data and a save write it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelFields {
    counter: String,
    #[serde(default)]
    target: CounterTarget,
    curve: DataRef<Curve>,
    #[serde(default)]
    offers: Option<DataRef<Offers>>,
    #[serde(default = "one")]
    level: u32,
    #[serde(default)]
    pending: Vec<Offer>,
    #[serde(default)]
    chosen: Option<u32>,
}

fn one() -> u32 {
    1
}

impl TryFrom<LevelFields> for Level {
    type Error = String;

    fn try_from(fields: LevelFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.counter) {
            return Err(format!(
                "counter: `{}` is not a counter name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.counter
            ));
        }
        if fields.level < 1 {
            return Err("level: must be at least 1".to_string());
        }
        if let Some(chosen) = fields.chosen
            && chosen as usize >= fields.pending.len()
        {
            return Err(format!(
                "chosen: {chosen} is past the {} pending offers",
                fields.pending.len()
            ));
        }
        Ok(Self {
            counter: fields.counter,
            target: fields.target,
            curve: fields.curve,
            offers: fields.offers,
            current: fields.level,
            pending: fields.pending,
            chosen: fields.chosen,
        })
    }
}

impl From<Level> for LevelFields {
    fn from(level: Level) -> Self {
        Self {
            counter: level.counter,
            target: level.target,
            curve: level.curve,
            offers: level.offers,
            level: level.current,
            pending: level.pending,
            chosen: level.chosen,
        }
    }
}

impl Level {
    /// Level 1, climbing the entity's own counter `counter` along `curve`,
    /// with no offers.
    ///
    /// # Panics
    ///
    /// When `counter` is not a counter name (`[a-z][a-z0-9_]*` segments
    /// joined by dots).
    #[must_use]
    #[track_caller]
    pub fn new(counter: &str, curve: DataRef<Curve>) -> Self {
        assert!(
            valid_tag(counter),
            "Level::new: `{counter}` is not a counter name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        Self {
            counter: counter.to_owned(),
            target: CounterTarget::Itself,
            curve,
            offers: None,
            current: 1,
            pending: Vec::new(),
            chosen: None,
        }
    }

    /// The level, reading the game's own [`Counters`] in place of the
    /// entity's.
    #[must_use]
    pub fn with_game_counter(mut self) -> Self {
        self.target = CounterTarget::Game;
        self
    }

    /// The level, drawing `offers` at each level up.
    #[must_use]
    pub fn with_offers(mut self, offers: DataRef<Offers>) -> Self {
        self.offers = Some(offers);
        self
    }

    /// The counter it reads.
    #[must_use]
    pub fn counter(&self) -> &str {
        &self.counter
    }

    /// Whether it reads the game's counters.
    #[must_use]
    pub fn reads_game(&self) -> bool {
        self.target == CounterTarget::Game
    }

    /// The curve.
    #[must_use]
    pub fn curve(&self) -> &DataRef<Curve> {
        &self.curve
    }

    /// The level now, from 1.
    #[must_use]
    pub fn level(&self) -> u32 {
        self.current
    }

    /// The top level: the curve's last `x`, floored, at least 1; 1 for a
    /// curve named by ID, whose entry the tick reads from the World's data.
    #[must_use]
    pub fn top(&self) -> u32 {
        self.curve.as_inline().map_or(1, top_level)
    }

    /// The offers the last level up drew, until one is chosen or the next
    /// level up replaces them.
    #[must_use]
    pub fn offers(&self) -> &[Offer] {
        &self.pending
    }

    /// Chooses the pending offer at `index`, which the next
    /// [`LEVEL_TICK`] applies; whether there is one.
    pub fn choose(&mut self, index: usize) -> bool {
        if index < self.pending.len() {
            self.chosen = u32::try_from(index).ok();
            self.chosen.is_some()
        } else {
            false
        }
    }

    /// The choice made, when one waits.
    #[must_use]
    pub fn chosen(&self) -> Option<usize> {
        self.chosen.map(|index| index as usize)
    }
}

/// `components.level` as a scene writes it: a [`Level`] at level 1 with
/// nothing pending.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelDef {
    counter: String,
    #[serde(default)]
    target: CounterTarget,
    curve: DataRef<Curve>,
    #[serde(default)]
    offers: Option<DataRef<Offers>>,
}

impl LevelDef {
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        if !valid_tag(&self.counter) {
            return Err(MissingAsset::new(
                "counter",
                "counter name",
                self.counter.as_str(),
            ));
        }
        self.curve.check(world, "curve")?;
        if let Some(offers) = &self.offers {
            offers.check(world, "offers")?;
        }
        Ok(())
    }
}

/// A level rose, sent once [`LevelPlugin`] has registered its queue: the
/// cue event `level_up`, whose amount is the level. Build one with
/// [`LevelUp::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct LevelUp {
    /// The entity whose level rose.
    pub entity: Entity,
    /// The level reached.
    pub level: u32,
}

impl LevelUp {
    /// An event with these fields.
    #[must_use]
    pub fn new(entity: Entity, level: u32) -> Self {
        Self { entity, level }
    }
}

/// The top level of `curve`: its last `x`, floored, at least 1 and at most
/// 65535.
fn top_level(curve: &Curve) -> u32 {
    let last = curve.points().last().map_or(1.0, |point| point.0);
    (last.floor().max(1.0).min(f32::from(u16::MAX)) as u32).max(1)
}

/// `curve`'s definition: the one in place, or the entry the World's data
/// holds now.
fn curve_of<'a>(world: &'a World, curve: &'a DataRef<Curve>) -> Option<&'a Curve> {
    match world.get_resource::<DataRegistry>() {
        Some(data) => curve.resolve(data),
        None => curve.as_inline(),
    }
}

/// `offers`' definition, as [`curve_of`] finds a curve's.
fn offers_of<'a>(world: &'a World, offers: &'a DataRef<Offers>) -> Option<&'a Offers> {
    match world.get_resource::<DataRegistry>() {
        Some(data) => offers.resolve(data),
        None => offers.as_inline(),
    }
}

/// Why a choice did not apply, for the trace.
const NO_EFFECTS: &str = "the World has no EffectsPlugin queue";

/// [`LEVEL_TICK`]'s state between steps.
#[derive(Default)]
struct Ticker {
    entities: Vec<Entity>,
}

impl Ticker {
    /// Per entity in index order: the choice applied, then the level
    /// climbed while the count reaches the next, at most 16 times.
    fn run(&mut self, world: &mut World) {
        self.entities.clear();
        self.entities
            .extend(world.query::<(Entity, &Level)>().map(|(entity, _)| entity));
        self.entities.sort_by_key(|entity| entity.id());
        let entities = std::mem::take(&mut self.entities);
        for &entity in &entities {
            tick_entity(world, entity);
        }
        self.entities = entities;
    }
}

fn tick_entity(world: &mut World, entity: Entity) {
    let Some(level) = world.get_mut::<Level>(entity) else {
        return;
    };
    // The choice made since the last tick.
    if let Some(index) = level.chosen.take() {
        let offer = level.pending.get(index as usize).cloned();
        level.pending.clear();
        if let Some(offer) = offer {
            apply(world, entity, &offer);
        }
    }
    let (counter, target, curve_ref, offers_ref, current) = {
        let level = world.get::<Level>(entity).unwrap();
        (
            level.counter.clone(),
            level.target,
            level.curve.clone(),
            level.offers.clone(),
            level.current,
        )
    };
    let count = match target {
        CounterTarget::Itself => world
            .get::<Counters>(entity)
            .map_or(0, |counters| counters.get(&counter)),
        CounterTarget::Game => world
            .get_resource::<Counters>()
            .map_or(0, |counters| counters.get(&counter)),
    };
    let (reached, top) = {
        let Some(curve) = curve_of(world, &curve_ref) else {
            return;
        };
        let top = top_level(curve);
        let mut reached = current;
        let mut ups = 0;
        while reached < top && ups < MAX_LEVELS_PER_STEP {
            let needed = f64::from(curve.at((reached + 1) as f32));
            if (count as f64) < needed {
                break;
            }
            reached += 1;
            ups += 1;
        }
        (reached, top)
    };
    debug_assert!(reached <= top);
    if reached == current {
        return;
    }
    let mut rng = world.remove_resource::<KitRng>().unwrap_or_default();
    let drawn = {
        let offers = offers_ref
            .as_ref()
            .and_then(|offers| offers_of(world, offers));
        offers.map(|offers| {
            let mut drawn = Vec::new();
            for _ in current..reached {
                drawn = offers.draw(&mut rng);
            }
            drawn
        })
    };
    world.insert_resource(rng);
    let level = world.get_mut::<Level>(entity).unwrap();
    level.current = reached;
    if let Some(drawn) = drawn {
        level.pending = drawn;
        level.chosen = None;
    }
    for reached in current + 1..=reached {
        if let Some(queue) = world.get_resource_mut::<EventQueue<LevelUp>>() {
            queue.send(LevelUp {
                entity,
                level: reached,
            });
        }
        trace(world, entity, || TraceKind::Event {
            name: "level_up",
            other: None,
            amount: Some(reached as f32),
        });
    }
}

/// Applies a chosen `offer` to `entity`: an effect queued, or an ability
/// added or refilled.
fn apply(world: &mut World, entity: Entity, offer: &Offer) {
    match &offer.gives {
        Gives::Effect(effect) => {
            if !EffectRequest::new(entity, effect.clone()).queue(world) {
                trace(world, entity, || TraceKind::Skipped {
                    event: "level_up",
                    action: "effect",
                    why: NO_EFFECTS,
                });
            }
        }
        Gives::Ability(ability) => {
            if !world.has::<Abilities>(entity) {
                world.insert(entity, Abilities::new());
            }
            let abilities = world.get_mut::<Abilities>(entity).unwrap();
            let owned = ability
                .id()
                .is_some_and(|id| abilities.names().any(|name| name == id));
            if owned {
                abilities.refill(ability.id().unwrap_or_default());
            } else {
                abilities.add(ability.clone());
            }
        }
    }
}

/// Registers the `offers` and `curves` data kinds, the `level` scene
/// component, `EventQueue<LevelUp>` and [`LEVEL_TICK`] in the react phase
/// before [`CUES`](crate::CUES) when it is registered, so a level up's cues
/// run in its step.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{LEVEL_TICK, LevelPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// LevelPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert_eq!(schedule.stage_of(LEVEL_TICK), Some(Stage::FixedUpdate));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Level>(world, "Level");
        register_curves(world);
        describe_level(world);
        world.register_event::<LevelUp>();
        if !world.has_resource::<KitRng>() {
            world.insert_resource(KitRng::default());
        }
        world.register_data_kind_with_assets(OFFERS, |offers: &Offers, world: &World| {
            offers.check(world)
        });
        world.register_component_with_assets(
            LEVEL,
            |def: &LevelDef, world: &World| def.check(world),
            |def: LevelDef, _: &SpawnContext<'_>| {
                let mut level = Level::new(&def.counter, def.curve);
                if def.target == CounterTarget::Game {
                    level = level.with_game_counter();
                }
                if let Some(offers) = def.offers {
                    level = level.with_offers(offers);
                }
                (level,)
            },
        );
        let mut ticker = Ticker::default();
        schedule.add(
            Phase::React.stage(),
            Phase::React
                .join(system(LEVEL_TICK, move |world: &mut World| {
                    ticker.run(world);
                }))
                .before_if_present(CUES),
        );
    }
}

/// The descriptions of `level`, the offers shapes and the `offers` kind.
fn describe_level(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            LEVEL,
            "A level climbed along a curve by a counter, each level up sending an event and drawing offers.",
        )
        .field(FieldDescription::new(
            "counter",
            FieldType::Text,
            "the counter it reads: dotted, undeclared",
        ))
        .field(
            FieldDescription::new(
                "target",
                FieldType::Enum(&["self", "game"]),
                "the entity's counters, or the game's",
            )
            .default(r#""self""#),
        )
        .field(FieldDescription::new(
            "curve",
            FieldType::Data(CURVES),
            "level to the total count that reaches it; its last `x`, floored, is the top level; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "offers",
                FieldType::Data(OFFERS),
                "drawn at each level up; the load checks it",
            )
            .default("null"),
        )
        .event("LevelUp")
        .genre(TOP_DOWN)
        .genre(GRID)
        .sample(r#"{ "counter": "xp", "curve": "hero_xp" }"#)
        .roundtrip(round_trip::<LevelDef>),
    );
    let shapes = [
        ComponentDescription::new(
            OFFERS,
            "What a level up offers: distinct entries drawn by weight without replacement.",
        )
        .field(FieldDescription::new(
            "count",
            FieldType::U32 {
                min: Some(1),
                max: None,
            },
            "how many entries a level up draws; every one when there are no more",
        ))
        .field(
            FieldDescription::new(
                "entries",
                FieldType::List(Box::new(FieldType::Shape(OFFER))),
                "what a draw may give, each by its weight over the rest",
            )
            .default("[]"),
        )
        .sample(r#"{ "count": 1 }"#)
        .roundtrip(round_trip::<Offers>),
        ComponentDescription::new(OFFER, "One offer: what it gives and its weight.")
            .field(FieldDescription::new(
                "weight",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "its share of the draws",
            ))
            .field(FieldDescription::new(
                "gives",
                FieldType::Shape(OFFER_GIVES),
                "an effect queued on the entity, or an ability added to it; the load checks it",
            ))
            .sample(r#"{ "weight": 1, "gives": { "effect": "swift" } }"#)
            .roundtrip(round_trip::<Offer>),
        ComponentDescription::new(OFFER_GIVES, "What a chosen offer gives.")
            .one_of()
            .field(FieldDescription::new(
                "effect",
                FieldType::Data(EFFECTS),
                "queued on the entity with no source at the next level tick",
            ))
            .field(FieldDescription::new(
                "ability",
                FieldType::Data(ABILITIES),
                "added to the entity's `abilities`, or, when it owns the name, that entry refilled with its cooldown cleared",
            ))
            .sample(r#"{ "effect": "swift" }"#)
            .roundtrip(round_trip::<Gives>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
    describe_kind(
        world,
        OFFERS,
        "An `offers`: what a `level` draws at each level up.",
    );
}

impl Inspectable for Level {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut line = format!("{} of {}, by {}", self.current, self.top(), self.counter);
        if self.target == CounterTarget::Game {
            line.push_str(" (game)");
        }
        let mut rows = vec![("level", line)];
        if !self.pending.is_empty() {
            let mut offers = String::new();
            for (i, offer) in self.pending.iter().enumerate() {
                if i > 0 {
                    offers.push_str(", ");
                }
                let _ = write!(offers, "{}", offer.summary());
                if self.chosen == Some(i as u32) {
                    offers.push_str(" (chosen)");
                }
            }
            rows.push(("offers", offers));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/level.rs"]
mod tests;
