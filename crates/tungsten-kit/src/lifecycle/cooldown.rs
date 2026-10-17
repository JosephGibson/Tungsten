//! Cooldowns: a repeating countdown on the game clock that sends an event
//! each time it elapses, with an optional random extra per cycle.

use serde::{Deserialize, Serialize};
use tungsten_core::{
    Entity, EventQueue, Inspectable, Plugin, Schedule, SpawnContext, Time, World, system,
};

use crate::TIME_EPSILON;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, SHMUP, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::rng::KitRng;
use crate::foundation::stats::STATS_APPLY;
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::{non_negative, positive};

/// [`CooldownPlugin`]'s system, in the tick phase, after
/// [`STATS_APPLY`](crate::STATS_APPLY) when it is registered.
pub const COOLDOWN_TICK: &str = "cooldown_tick";

/// The scene component the plugin registers.
const COOLDOWN: &str = "cooldown";

/// A repeating countdown on the game clock: a turret's fire, a bomb's fuse,
/// a turn timer. [`CooldownPlugin`]'s system counts it down once a fixed
/// step by the step times its rate (1 unless a `cooldown_rate` stat sets
/// it), so a pause holds it, and sends a [`CooldownElapsed`] each time it
/// reaches 0, then starts the next cycle. A step that fits several cycles
/// elapses once per whole cycle, at most
/// [`MAX_ELAPSES_PER_STEP`](Self::MAX_ELAPSES_PER_STEP) times: the rest of
/// that step's time is dropped and the next cycle starts whole.
///
/// With a jitter, each cycle lasts its seconds plus an extra drawn in
/// `[0, jitter]` from the World's [`KitRng`], the first at its first step
/// and each later one as the cycle before it elapses, before the step's
/// remaining time decides whether another fits. Jittered cooldowns draw in
/// `Entity::id` order, so a run under one seed repeats.
///
/// Scene name `cooldown`: `{ "seconds": 0.5, "jitter": 0.25, "count": 3 }`,
/// `jitter` 0 and `count` without end by default. It is data: a game's
/// save can hold it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{EventQueue, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Cooldown, CooldownElapsed, CooldownPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// CooldownPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A fuse that burns once after a quarter second.
/// let bomb = world.spawn_with((Cooldown::new(0.25).with_count(1),));
/// let mut elapsed = Vec::new();
/// for _ in 0..30 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     elapsed.extend(world.resource::<EventQueue<CooldownElapsed>>().iter_current().copied());
///     world.flush_events();
/// }
/// assert_eq!(elapsed, [CooldownElapsed::new(bomb)]);
/// assert_eq!(world.get::<Cooldown>(bomb).unwrap().remaining(), Some(0));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CooldownFields", into = "CooldownFields")]
pub struct Cooldown {
    seconds: f32,
    jitter: f32,
    /// The time left in the current cycle, its drawn extra included.
    left: f32,
    remaining: Option<u32>,
    rate: f32,
    /// The first cycle's extra is still to draw.
    fresh: bool,
}

/// A cooldown as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CooldownFields {
    seconds: f32,
    #[serde(default)]
    jitter: f32,
    left: f32,
    #[serde(default)]
    remaining: Option<u32>,
    #[serde(default = "one")]
    rate: f32,
    #[serde(default)]
    fresh: bool,
}

fn one() -> f32 {
    1.0
}

impl TryFrom<CooldownFields> for Cooldown {
    type Error = String;

    fn try_from(fields: CooldownFields) -> Result<Self, Self::Error> {
        if !(fields.seconds.is_finite() && fields.seconds > 0.0) {
            return Err(format!(
                "seconds: must be finite and > 0 (got {})",
                fields.seconds
            ));
        }
        for (field, value) in [
            ("jitter", fields.jitter),
            ("left", fields.left),
            ("rate", fields.rate),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            seconds: fields.seconds,
            jitter: fields.jitter,
            left: fields.left,
            remaining: fields.remaining,
            rate: fields.rate,
            fresh: fields.fresh,
        })
    }
}

impl From<Cooldown> for CooldownFields {
    fn from(cooldown: Cooldown) -> Self {
        Self {
            seconds: cooldown.seconds,
            jitter: cooldown.jitter,
            left: cooldown.left,
            remaining: cooldown.remaining,
            rate: cooldown.rate,
            fresh: cooldown.fresh,
        }
    }
}

impl Cooldown {
    /// The most times one cooldown elapses in a step.
    pub const MAX_ELAPSES_PER_STEP: u32 = 16;

    /// A cooldown of `seconds` of game time a cycle, the first cycle
    /// starting now, without end.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "Cooldown::new: seconds must be finite and > 0 (got {seconds})"
        );
        Self {
            seconds,
            jitter: 0.0,
            left: seconds,
            remaining: None,
            rate: 1.0,
            fresh: true,
        }
    }

    /// Each cycle lasts up to `seconds` more, drawn from the World's
    /// [`KitRng`].
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_jitter(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "Cooldown::with_jitter: seconds must be finite and >= 0 (got {seconds})"
        );
        self.jitter = seconds;
        self
    }

    /// Stops after elapsing `count` times; 0 never elapses.
    #[must_use]
    pub fn with_count(mut self, count: u32) -> Self {
        self.remaining = Some(count);
        self
    }

    /// Starts the current cycle over, its extra drawn again at the next
    /// step; the elapses left stay as they are.
    pub fn restart(&mut self) {
        self.left = self.seconds;
        self.fresh = true;
    }

    /// The seconds of a cycle, before its extra.
    #[must_use]
    pub fn seconds(&self) -> f32 {
        self.seconds
    }

    /// The game seconds left in the current cycle at a rate of 1, its drawn
    /// extra included.
    #[must_use]
    pub fn left(&self) -> f32 {
        self.left
    }

    /// The elapses left; `None` without end.
    #[must_use]
    pub fn remaining(&self) -> Option<u32> {
        self.remaining
    }

    /// How fast it counts down: 1 on the game clock, or what a
    /// `cooldown_rate` stat sets.
    #[must_use]
    pub fn rate(&self) -> f32 {
        self.rate
    }

    /// Sets the rate: what a `cooldown_rate` stat writes. One that is not
    /// finite and at least 0 is ignored.
    pub(crate) fn set_rate(&mut self, rate: f32) {
        if rate.is_finite() && rate >= 0.0 {
            self.rate = rate;
        }
    }

    /// Whether the step needs a draw: a jittered cooldown at its first
    /// step, or one that reaches 0 within `budget`.
    fn draws_within(&self, budget: f32) -> bool {
        self.jitter > 0.0
            && self.remaining != Some(0)
            && (self.fresh || self.left - budget <= TIME_EPSILON)
    }

    /// Counts `budget` seconds down, `draw` giving each jittered cycle's
    /// extra; the elapses and whether the step reached
    /// [`MAX_ELAPSES_PER_STEP`](Self::MAX_ELAPSES_PER_STEP) with time left.
    fn advance(&mut self, budget: f32, draw: &mut dyn FnMut(f32) -> f32) -> (u32, bool) {
        if self.remaining == Some(0) {
            return (0, false);
        }
        let mut cycle = self.seconds;
        if std::mem::take(&mut self.fresh) && self.jitter > 0.0 {
            let extra = draw(self.jitter);
            self.left += extra;
            cycle += extra;
        }
        let mut left = self.left - budget;
        let mut elapses = 0;
        while left <= TIME_EPSILON {
            if elapses == Self::MAX_ELAPSES_PER_STEP {
                self.left = cycle;
                return (elapses, true);
            }
            elapses += 1;
            if let Some(remaining) = &mut self.remaining {
                *remaining -= 1;
                if *remaining == 0 {
                    self.left = 0.0;
                    return (elapses, false);
                }
            }
            cycle = self.seconds;
            if self.jitter > 0.0 {
                cycle += draw(self.jitter);
            }
            left += cycle;
        }
        self.left = left;
        (elapses, false)
    }
}

/// A [`Cooldown`] reached 0, sent once per elapse by [`CooldownPlugin`]'s
/// system. Build one with [`CooldownElapsed::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CooldownElapsed {
    /// The cooldown's entity.
    pub entity: Entity,
}

impl CooldownElapsed {
    /// An event about `entity`, as the cooldown's system sends one.
    #[must_use]
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

/// Registers `EventQueue<CooldownElapsed>`, the `cooldown` scene component
/// and [`COOLDOWN_TICK`] in the tick phase, after
/// [`STATS_APPLY`](crate::STATS_APPLY) when it is registered, so a
/// `cooldown_rate` stat applies in its own step, and inserts a [`KitRng`]
/// when the World has none.
#[derive(Debug, Default, Clone, Copy)]
pub struct CooldownPlugin;

impl Plugin for CooldownPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Cooldown>(world, "Cooldown");
        describe(
            world,
            ComponentDescription::new(
                COOLDOWN,
                "A repeating countdown on the game clock that sends an event each time it elapses.",
            )
            .field(FieldDescription::new(
                "seconds",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds of game time a cycle",
            ))
            .field(
                FieldDescription::new(
                    "jitter",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "up to this many seconds more each cycle, drawn from the kit's random stream",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "count",
                    FieldType::U32 {
                        min: None,
                        max: None,
                    },
                    "how many times it elapses in all; without end when left out",
                )
                .default("null"),
            )
            .event("CooldownElapsed")
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "seconds": 0.5 }"#)
            .roundtrip(round_trip::<CooldownDef>),
        );
        if !world.has_resource::<KitRng>() {
            world.insert_resource(KitRng::default());
        }
        world.register_event::<CooldownElapsed>();
        world.register_component_with(COOLDOWN, |def: CooldownDef, _: &SpawnContext<'_>| {
            let cooldown = Cooldown::new(def.seconds).with_jitter(def.jitter);
            (def.count
                .map_or(cooldown.clone(), |count| cooldown.with_count(count)),)
        });
        let mut ticker = Ticker::default();
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(
                system(COOLDOWN_TICK, move |world: &mut World| ticker.run(world))
                    .after_if_present(STATS_APPLY),
            ),
        );
    }
}

/// `components.cooldown` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CooldownDef {
    #[serde(deserialize_with = "positive")]
    seconds: f32,
    #[serde(default, deserialize_with = "non_negative")]
    jitter: f32,
    #[serde(default)]
    count: Option<u32>,
}

/// The system's state between steps, kept so a step allocates nothing once
/// warm.
#[derive(Default)]
struct Ticker {
    /// The jittered cooldowns that draw this step.
    jittered: Vec<Entity>,
    /// Each cooldown that elapsed: its entity, the elapses and whether the
    /// step capped it.
    elapsed: Vec<(Entity, u32, bool)>,
}

impl Ticker {
    fn run(&mut self, world: &mut World) {
        let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
        if dt <= 0.0 {
            return;
        }
        self.jittered.clear();
        self.elapsed.clear();
        for (entity, cooldown) in world.query_mut::<(Entity, &mut Cooldown)>() {
            let budget = dt * cooldown.rate;
            if cooldown.draws_within(budget) {
                self.jittered.push(entity);
                continue;
            }
            let (elapses, capped) = cooldown.advance(budget, &mut |_| 0.0);
            if elapses > 0 || capped {
                self.elapsed.push((entity, elapses, capped));
            }
        }
        if !self.jittered.is_empty() {
            self.jittered.sort_by_key(|entity| entity.id());
            let mut rng = world.remove_resource::<KitRng>().unwrap_or_default();
            for &entity in &self.jittered {
                let cooldown = world.get_mut::<Cooldown>(entity).unwrap();
                let budget = dt * cooldown.rate;
                let (elapses, capped) =
                    cooldown.advance(budget, &mut |jitter| rng.range(0.0, jitter));
                if elapses > 0 || capped {
                    self.elapsed.push((entity, elapses, capped));
                }
            }
            world.insert_resource(rng);
        }
        for &(entity, elapses, capped) in &self.elapsed {
            if let Some(queue) = world.get_resource_mut::<EventQueue<CooldownElapsed>>() {
                for _ in 0..elapses {
                    queue.send(CooldownElapsed::new(entity));
                }
            }
            for _ in 0..elapses {
                trace(world, entity, || TraceKind::Event {
                    name: "cooldown_elapsed",
                    other: None,
                    amount: None,
                });
            }
            if capped {
                trace(world, entity, || TraceKind::Skipped {
                    event: "cooldown_elapsed",
                    action: "catch_up",
                    why: "16 elapses a step at most; the rest of the step dropped",
                });
            }
        }
    }
}

impl Inspectable for Cooldown {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("next", format!("{:.2} s", self.left)),
            (
                "remaining",
                self.remaining
                    .map_or_else(|| "unbounded".to_string(), |n| n.to_string()),
            ),
            ("rate", format!("{:.2}", self.rate)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/cooldown.rs"]
mod tests;
