//! Chemistry (redo 7.10b, the ARPG design's §5.2): stimuli meet what
//! receives them through the `interactions` kind's rules, a saved, bounded
//! and capped queue and a fixed list of outputs. A landed hit stimulates by
//! each of its damage types, a status or an output by an emission to the
//! receivers in a radius, a body by its impact and a receiver by the light
//! at its point; a status's end and a band's entry run their own outputs.

#[path = "become.rs"]
pub(crate) mod becoming;
pub(crate) mod blast;
pub(crate) mod grid;
pub(crate) mod outputs;
pub(crate) mod queue;
pub(crate) mod rules;
pub(crate) mod stimuli;
pub(crate) mod tiles;

use std::rc::Rc;

use tungsten_core::physics::ContactBegan;
use tungsten_core::{Entity, EventCursor, EventQueue, Plugin, Schedule, Time, World, system};

use crate::damage::ailments::{AILMENT_FEED, AILMENT_TICK};
use crate::damage::effects::{EFFECT_TICK, definition};
use crate::damage::health::{DamagedPart, Health};
use crate::damage::pipeline::DAMAGE_RESOLVE;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape,
    round_trip,
};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::{STIMULUS_IMPACT, Tags, covers};
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::Despawning;

pub use outputs::{Output, Reaction, TileCell};
pub use queue::Chemistry;
pub use rules::Interaction;
pub use tiles::TileStatuses;

use becoming::{BECAME, Became};
use outputs::{Handler, Handlers, run_outputs};
use queue::{CHEMISTRY_QUEUE, Entry, Receiver, Settings, StatusList, Stimulus, queue_saved};
use rules::{INTERACTIONS, answered, answers, any_rule, register_interactions, snapshot, winner};
use stimuli::{Emission, LightSampler, Reach, impacts, spread};
use tiles::{TILE_STATUSES, tile_saved};

/// [`ChemistryPlugin`]'s system, in the resolve phase after
/// [`DAMAGE_RESOLVE`](crate::DAMAGE_RESOLVE) and the ailment feed when they
/// are registered, before [`EFFECT_RESOLVE`](crate::EFFECT_RESOLVE), so the
/// statuses it applies land in its step.
pub const CHEMISTRY: &str = "chemistry";

/// [`ChemistryPlugin`]'s second system, in the tick phase after
/// [`EFFECT_TICK`](crate::EFFECT_TICK) and the ailment tick when they are
/// registered: before the step's physics it registers the contact begins'
/// queue while a rule answers `stimulus.impact`, and it samples the light
/// on its period while one answers `stimulus.light`; idle as [`CHEMISTRY`]
/// is.
pub const CHEMISTRY_TICK: &str = "chemistry_tick";

/// Why an entry did nothing, for the trace.
const GONE: &str = "the receiver is gone or despawning";
const NO_RULE: &str = "no rule passes";

/// Registers chemistry: the `interactions` kind, the saved queue
/// ([`Chemistry`]), [`CHEMISTRY`], which reads the step's stimuli and runs
/// the queue, and [`CHEMISTRY_TICK`], which samples the light and turns on
/// the impact record.
///
/// Each run: every non-periodic [`DamagedPart`] since the last is a
/// stimulus of each of its deepest types, its amount after the resistances,
/// credited to its source; every contact begin, while a rule answers
/// `stimulus.impact`, one of that on both bodies by its closing speed,
/// credited to the other's [`Source`](crate::Source) entity, else the other
/// (a tile credits none); every emission since the last run, a status's
/// `emit` or an `emit` output's, one on each tagged receiver within its
/// radius but the emitter (a body measured to its surface, else its point,
/// through a grid of 64 px cells built once a run with radius work). Each
/// is queued when some rule names its name or an ancestor of it, one entry
/// per stimulus and receiver: a merge sums the
/// raw amounts, which a rule's `min` reads, and apart each hit's amount
/// times its status effect, which a `scale` build reads, and keeps the
/// largest status effect and damage-over-time base, the least depth, the
/// first credit and the run it was first queued for. Past the bound an add
/// is dropped, counted and warned of once a run. A run then takes at most
/// the cap of the entries queued for it or before, oldest first (by that
/// run, then depth, the receiver's save order and the stimulus), so fresh
/// work never passes a waiting entry, and decides each as it runs: nothing
/// for a receiver gone or despawning, or when no rule passes; else the most
/// specific rule's outputs ([`Interaction`]). A dead receiver still reacts,
/// its `build` and `apply` refused, and the kit's end-of-step despawn holds
/// it while it has an entry, taking no new one. An output's damage and
/// statuses carry the entry's depth plus one, and an entry past the depth
/// limit is dropped and counted ([`Chemistry::cut`]), so a cycle of rules
/// ends. With no `interactions` entry and an empty queue each system returns
/// before any query. Every `light_period` on the fixed clock, while a rule
/// answers `stimulus.light`, each receiver some such rule's `on` passes
/// takes a `stimulus.light` of the luma of the lights' sum at its point
/// ([`light_from`](tungsten_core::lighting::light_from)).
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{CHEMISTRY, ChemistryPlugin, EffectsPlugin, HealthPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// EffectsPlugin.build(&mut schedule, &mut world);
/// ChemistryPlugin::new()
///     .with_cap(64)
///     .with_handler("douse", |_world, reaction| assert_eq!(reaction.depth(), 0))
///     .build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert_eq!(schedule.stage_of(CHEMISTRY), Some(Stage::FixedUpdate));
/// ```
#[derive(Clone)]
pub struct ChemistryPlugin {
    cap: usize,
    bound: usize,
    depth: u8,
    light_period: f32,
    handlers: Vec<(String, Handler)>,
}

impl Default for ChemistryPlugin {
    /// [`new`](Self::new).
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ChemistryPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChemistryPlugin")
            .field("cap", &self.cap)
            .field("bound", &self.bound)
            .field("depth", &self.depth)
            .field("light_period", &self.light_period)
            .field(
                "handlers",
                &self
                    .handlers
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ChemistryPlugin {
    /// Chemistry with a run's cap of 256, a bound of 16,384 entries, a
    /// depth limit of 8 and the light sampled every 0.25 s, and no handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cap: queue::CAP,
            bound: queue::BOUND,
            depth: queue::DEPTH,
            light_period: queue::LIGHT_PERIOD,
            handlers: Vec::new(),
        }
    }

    /// At most `cap` entries a run.
    ///
    /// # Panics
    ///
    /// When `cap` is 0.
    #[must_use]
    #[track_caller]
    pub fn with_cap(mut self, cap: usize) -> Self {
        assert!(cap > 0, "ChemistryPlugin::with_cap: cap must be above 0");
        self.cap = cap;
        self
    }

    /// At most `bound` entries queued.
    ///
    /// # Panics
    ///
    /// When `bound` is 0.
    #[must_use]
    #[track_caller]
    pub fn with_bound(mut self, bound: usize) -> Self {
        assert!(
            bound > 0,
            "ChemistryPlugin::with_bound: bound must be above 0"
        );
        self.bound = bound;
        self
    }

    /// Entries at most `depth` reactions deep.
    #[must_use]
    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// The light sampled every `seconds` of the fixed clock.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_light_period(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "ChemistryPlugin::with_light_period: seconds must be finite and > 0 (got {seconds})"
        );
        self.light_period = seconds;
        self
    }

    /// The handler `name`, which a `handler` output runs with the World and
    /// the [`Reaction`]; the load refuses a rule naming a handler no plugin
    /// registered.
    #[must_use]
    pub fn with_handler(
        mut self,
        name: impl Into<String>,
        handler: impl Fn(&mut World, &Reaction) + 'static,
    ) -> Self {
        self.handlers.push((name.into(), Rc::new(handler)));
        self
    }
}

impl Plugin for ChemistryPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_interactions(world);
        describe_chemistry(world);
        if !world.has_resource::<Handlers>() {
            world.insert_resource(Handlers::default());
        }
        let handlers = world.resource_mut::<Handlers>();
        for (name, handler) in &self.handlers {
            handlers.0.retain(|(known, _)| known != name);
            handlers.0.push((name.clone(), Rc::clone(handler)));
        }
        if !world.has_resource::<Chemistry>() {
            world.insert_resource(Chemistry::default());
        }
        // Apart from the saved queue, so a load keeps them.
        world.insert_resource(Settings {
            cap: self.cap,
            bound: self.bound,
            depth: self.depth,
            light_period: self.light_period,
        });
        world.register_saved_resource_with(CHEMISTRY_QUEUE, queue_saved());
        if !world.has_resource::<TileStatuses>() {
            world.insert_resource(TileStatuses::default());
        }
        world.register_saved_resource_with(TILE_STATUSES, tile_saved());
        world.register_saved::<Became>(BECAME);
        if !world.has_resource::<blast::Blasts>() {
            world.insert_resource(blast::Blasts::default());
        }
        if schedule.contains(CHEMISTRY) {
            return;
        }
        let mut ticker = Ticker::default();
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick
                .join(system(CHEMISTRY_TICK, move |world: &mut World| {
                    ticker.run(world);
                }))
                .after_if_present(EFFECT_TICK)
                .after_if_present(AILMENT_TICK),
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Resolve.stage(),
            Phase::Resolve
                .join(system(CHEMISTRY, move |world: &mut World| {
                    runner.run(world);
                }))
                .after_if_present(DAMAGE_RESOLVE)
                .after_if_present(AILMENT_FEED),
        );
    }
}

/// Whether chemistry has work: an `interactions` entry, a queued entry, or
/// a blast queued outside a run, such as a projectile's end.
fn busy(world: &World) -> bool {
    let waiting = world
        .get_resource::<Chemistry>()
        .is_some_and(|chemistry| chemistry.pending() > 0);
    let blasting = world
        .get_resource::<blast::Blasts>()
        .is_some_and(|blasts| !blasts.0.is_empty());
    waiting || blasting || any_rule(world)
}

/// Whether [`CHEMISTRY_TICK`] has work: [`busy`], or a tile status or a
/// revert waiting.
fn ticking(world: &World) -> bool {
    busy(world)
        || world
            .get_resource::<TileStatuses>()
            .is_some_and(|statuses| !statuses.is_empty())
}

/// [`CHEMISTRY_TICK`]'s state between steps: the light sampler's lists and
/// the replacements due.
#[derive(Default)]
struct Ticker {
    light: LightSampler,
    due: Vec<(Entity, String)>,
}

impl Ticker {
    fn run(&mut self, world: &mut World) {
        if !ticking(world) {
            return;
        }
        // The contact record runs for a step that starts with the queue,
        // so a rule's first step counts.
        if !world.has_event::<ContactBegan>() && answers(world, STIMULUS_IMPACT) {
            world.register_event::<ContactBegan>();
        }
        let settings = Settings::of(world);
        let dt = world
            .get_resource::<Time>()
            .map_or(0.0, Time::delta)
            .max(0.0);
        tiles::tick(world, dt);
        becoming::tick(world, dt, &mut self.due);
        self.light.tick(world, &settings, dt);
    }
}

/// [`CHEMISTRY`]'s state between steps: its place in the parts queue and
/// its scratch lists, kept so a run allocates little once warm.
#[derive(Default)]
struct Runner {
    cursor: EventCursor<DamagedPart>,
    parts: Vec<DamagedPart>,
    begins_cursor: EventCursor<ContactBegan>,
    begins: Vec<ContactBegan>,
    emitted: Vec<Emission>,
    reach: Reach,
    rules: Vec<(String, Interaction)>,
    taken: Vec<Entry>,
    /// A part's deepest types that some rule answers.
    leaves: Vec<String>,
}

impl Runner {
    fn run(&mut self, world: &mut World) {
        if !busy(world) {
            return;
        }
        snapshot(world, &mut self.rules);
        let settings = Settings::of(world);
        // A queue a load restored meets this World's settings first.
        if world
            .get_resource::<Chemistry>()
            .is_some_and(Chemistry::unchecked)
        {
            let mut chemistry = world.remove_resource::<Chemistry>().unwrap();
            chemistry.enforce(world, &settings);
            world.insert_resource(chemistry);
        }
        let Some(run) = world
            .get_resource::<Chemistry>()
            .map(|chemistry| chemistry.run)
        else {
            return;
        };
        // The step's landed parts, each a stimulus of its deepest types.
        self.parts.clear();
        if let Some(queue) = world.get_resource::<EventQueue<DamagedPart>>() {
            self.parts.extend(self.cursor.read(queue).copied());
        }
        let parts = std::mem::take(&mut self.parts);
        for part in &parts {
            if part.periodic || part.kinds.is_empty() {
                continue;
            }
            self.leaves.clear();
            {
                let names = part.kinds.names(world);
                for (i, name) in names.iter().enumerate() {
                    let deeper = names
                        .iter()
                        .enumerate()
                        .any(|(j, other)| j != i && covers(other, name));
                    if !deeper && answered(&self.rules, name) {
                        self.leaves.push((*name).to_owned());
                    }
                }
            }
            let stimulus = Stimulus {
                amount: part.amount,
                status_effect: part.status_effect,
                dot_base: part.dot_base,
                depth: part.depth,
                credit: part.source,
            };
            let chemistry = world.resource_mut::<Chemistry>();
            for name in &self.leaves {
                let receiver = Receiver::Entity(part.entity);
                chemistry.stimulate(&settings, receiver, name, stimulus, run);
            }
        }
        self.parts = parts;

        // The step's contact begins, while a rule answers impacts.
        self.begins.clear();
        if let Some(queue) = world.get_resource::<EventQueue<ContactBegan>>() {
            self.begins.extend(self.begins_cursor.read(queue).copied());
        }
        if !self.begins.is_empty() && answered(&self.rules, STIMULUS_IMPACT) {
            let impact = world.resource_mut::<Chemistry>().intern(STIMULUS_IMPACT);
            impacts(
                world,
                &settings,
                &self.begins,
                impact,
                &mut self.reach.names,
                run,
            );
        }

        // The emissions since the last run, through the grid and the cells.
        self.reach.clear();
        std::mem::swap(
            &mut self.emitted,
            &mut world.resource_mut::<Chemistry>().emitted,
        );
        if !self.emitted.is_empty() {
            let rules = &self.rules;
            spread(
                world,
                &settings,
                &mut self.emitted,
                &mut self.reach,
                |name| answered(rules, name),
                run,
            );
        }

        // The run: at most the cap, oldest first.
        let mut taken = std::mem::take(&mut self.taken);
        {
            let mut chemistry = world.remove_resource::<Chemistry>().unwrap();
            chemistry.take(world, settings.cap, run, &mut taken);
            chemistry.run = run + 1;
            world.insert_resource(chemistry);
        }
        let count = taken.len();
        for entry in taken.drain(..) {
            self.react(world, entry);
        }
        self.taken = taken;
        // The blasts the reactions set off, once they are done: their
        // stimuli for the next run.
        if world
            .get_resource::<blast::Blasts>()
            .is_some_and(|blasts| !blasts.0.is_empty())
        {
            let mut blasts = world.remove_resource::<blast::Blasts>().unwrap();
            blast::detonate(world, &settings, &mut blasts.0, &mut self.reach, run + 1);
            world.insert_resource(blasts);
        }
        let chemistry = world.resource_mut::<Chemistry>();
        chemistry.count_ran(count);
        let warn = std::mem::take(&mut chemistry.dropped_now);
        if warn {
            log::warn!(
                "chemistry: the queue is at its bound of {}; {} adds dropped so far",
                settings.bound,
                chemistry.dropped()
            );
        }
        // A held receiver gone or despawned holds no more.
        let mut gone = Vec::new();
        if let Some(chemistry) = world.get_resource::<Chemistry>() {
            gone.extend(
                chemistry
                    .held()
                    .iter()
                    .copied()
                    .filter(|&entity| !world.is_alive(entity) || world.has::<Despawning>(entity)),
            );
        }
        if !gone.is_empty() {
            world
                .resource_mut::<Chemistry>()
                .release_held(|entity| !gone.contains(&entity));
        }
    }

    /// Decides `entry` as it runs.
    fn react(&mut self, world: &mut World, entry: Entry) {
        let (dead, cell_tags) = match &entry.receiver {
            Receiver::Entity(receiver) => {
                let receiver = *receiver;
                if !world.is_alive(receiver) || world.has::<Despawning>(receiver) {
                    trace(world, receiver, || TraceKind::Skipped {
                        event: "interaction",
                        action: "all",
                        why: GONE,
                    });
                    return;
                }
                (
                    world.get::<Health>(receiver).is_some_and(Health::is_dead),
                    None,
                )
            }
            Receiver::Cell(cell) => {
                if !tiles::placed(world, cell) {
                    return;
                }
                (false, Some(tiles::tags_of(world, cell)))
            }
        };
        let stimulus = world
            .resource::<Chemistry>()
            .name(entry.stimulus)
            .to_owned();
        let mut reaction = Reaction {
            receiver: entry.receiver.clone(),
            stimulus,
            rule: String::new(),
            raw: entry.raw,
            weighted: entry.weighted,
            status_effect: entry.status_effect,
            dot_base: entry.dot_base,
            depth: entry.depth,
            credit: entry.credit,
        };
        if let Some(list) = entry.status {
            // A status's own list: its `on_end`, or a band's `enter`.
            let outputs = definition(world, &list.effect).and_then(|effect| match list.band {
                None => Some(effect.on_end().to_vec()),
                Some(band) => effect.band_enter(band as usize).map(<[Output]>::to_vec),
            });
            let Some(outputs) = outputs else {
                return;
            };
            reaction.rule.clone_from(&reaction.stimulus);
            let label = reaction.rule.clone();
            run_outputs(world, &reaction, &outputs, &label, dead);
            return;
        }
        let tags: Option<&Tags> = match (&cell_tags, &entry.receiver) {
            (Some(tags), _) => Some(tags),
            (None, Receiver::Entity(receiver)) => world.get::<Tags>(*receiver),
            (None, Receiver::Cell(_)) => None,
        };
        let Some((id, rule)) = winner(&self.rules, &reaction.stimulus, entry.raw, tags) else {
            if let Some(receiver) = entry.receiver.entity() {
                trace(world, receiver, || TraceKind::Skipped {
                    event: "interaction",
                    action: "all",
                    why: NO_RULE,
                });
            }
            return;
        };
        reaction.rule.clone_from(id);
        run_outputs(world, &reaction, rule.outputs(), id, dead);
    }
}

/// Whether the kit's despawn holds `entity` for chemistry: it has an entry
/// waiting. It then takes no new one.
pub(crate) fn holds(world: &mut World, entity: Entity) -> bool {
    match world.get_resource_mut::<Chemistry>() {
        Some(chemistry) if chemistry.holds(entity) => {
            chemistry.hold(entity);
            true
        }
        _ => false,
    }
}

/// Queues a status's own outputs on `receiver`: its `on_end`, or band
/// `band`'s `enter`, `depth` deep, for the next run. A World without
/// chemistry queues nothing.
pub(crate) fn queue_status_list(
    world: &mut World,
    receiver: Receiver,
    list: StatusList,
    depth: u8,
    credit: Option<Entity>,
) {
    let settings = Settings::of(world);
    if let Some(chemistry) = world.get_resource_mut::<Chemistry>() {
        let run = chemistry.run;
        chemistry.queue_list(&settings, receiver, list, depth, credit, run);
    }
}

/// The descriptions of the `interactions` kind and its shapes.
fn describe_chemistry(world: &mut World) {
    let amount = || FieldType::F32 {
        min: Bound::Inclusive(0.0),
        max: Bound::Open,
    };
    let shapes = [
        ComponentDescription::new(
            "interaction",
            "A chemistry rule: when a stimulus reaches a receiver whose tags pass `on`, its outputs run on the receiver.",
        )
        .field(FieldDescription::new(
            "stimulus",
            FieldType::Text,
            "a damage type or a declared tag, and every name under it; the deepest rule wins",
        ))
        .field(
            FieldDescription::new(
                "on",
                FieldType::Filter,
                "the receiver's tags, its statuses' grants and bands among them; more dotted segments in `all` win; the load checks its tags",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "min",
                amount(),
                "the least amount it answers: the merged hits' raw sum",
            )
            .default("0.0"),
        )
        .field(FieldDescription::new(
            "outputs",
            FieldType::List(Box::new(FieldType::Shape("interaction_output"))),
            "run on the receiver in order",
        ))
        .sample(r#"{ "stimulus": "heat", "outputs": [] }"#)
        .roundtrip(round_trip::<Interaction>),
        ComponentDescription::new(
            "interaction_output",
            "One thing a rule, a status's end or a band's entry does to its receiver.",
        )
        .one_of()
        .field(FieldDescription::new(
            "build",
            FieldType::Shape("build_output"),
            "builds a status's intensity",
        ))
        .field(FieldDescription::new(
            "apply",
            FieldType::Shape("apply_output"),
            "applies a status at the entry's largest status effect",
        ))
        .field(FieldDescription::new(
            "remove",
            FieldType::Data(crate::damage::effects::EFFECTS),
            "ends the status's instance; the load checks it",
        ))
        .field(FieldDescription::new(
            "consume",
            FieldType::Shape("consume_output"),
            "takes intensity off a status; at 0 it has run out",
        ))
        .field(FieldDescription::new(
            "damage",
            FieldType::Damage,
            "queued on the receiver from the credit for the next step's resolve, one reaction deeper",
        ))
        .field(FieldDescription::new(
            "spawn",
            FieldType::Prefab,
            "a prefab at the receiver's point, from it",
        ))
        .field(FieldDescription::new(
            "emit",
            FieldType::Shape("emit_output"),
            "a stimulus sent once from the receiver's point to every receiver in a radius but itself, one reaction deeper",
        ))
        .field(FieldDescription::new(
            "set_tile",
            FieldType::Shape("set_tile_output"),
            "on a tile cell, a sprite written through the registry, which every placement of the map shows",
        ))
        .field(FieldDescription::new(
            "become",
            FieldType::Shape("become_output"),
            "the receiver replaced by a prefab at its point under its save identity, its statuses not carried",
        ))
        .field(FieldDescription::new(
            "blast",
            FieldType::Shape("blast_output"),
            "hurts, pushes and sets off what lies within a radius of the receiver's point, by a linear falloff, after the run's reactions",
        ))
        .field(FieldDescription::new(
            "handler",
            FieldType::Text,
            "a game's Rust handler by name; the load refuses one no plugin registered",
        ))
        .sample(r#"{ "remove": "burning" }"#)
        .roundtrip(round_trip::<Output>),
        ComponentDescription::new("build_output", "A status built by a fixed amount or by the entry's hits.")
            .field(FieldDescription::new(
                "status",
                FieldType::Data(crate::damage::effects::EFFECTS),
                "the status; the load checks it",
            ))
            .field(
                FieldDescription::new(
                    "amount",
                    amount(),
                    "a fixed build, at the entry's largest status effect",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "scale",
                    amount(),
                    "times the entry's hits, each at its own status effect; 1 when neither is given",
                )
                .default("null"),
            )
            .sample(r#"{ "status": "burning" }"#)
            .roundtrip(round_trip::<outputs::BuildDef>),
        ComponentDescription::new("apply_output", "A status applied, with a damage-over-time rate and lines.")
            .field(FieldDescription::new(
                "status",
                FieldType::Data(crate::damage::effects::EFFECTS),
                "the status; the load checks it",
            ))
            .field(
                FieldDescription::new(
                    "dot",
                    amount(),
                    "its damage over time a second: this times the hit's base, the drawn amount before the factors times the `delivery.dot` sums and the status effect",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "lines",
                    FieldType::List(Box::new(FieldType::Shape("stat_modifier"))),
                    "lines on the instance, beside the status's own",
                )
                .default("null"),
            )
            .sample(r#"{ "status": "burning" }"#)
            .roundtrip(round_trip::<outputs::ApplyDef>),
        ComponentDescription::new("consume_output", "Intensity taken off a status.")
            .field(FieldDescription::new(
                "status",
                FieldType::Data(crate::damage::effects::EFFECTS),
                "the status; the load checks it",
            ))
            .field(FieldDescription::new(
                "amount",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "taken off; at 0 it has run out",
            ))
            .sample(r#"{ "status": "burning", "amount": 1 }"#)
            .roundtrip(round_trip::<outputs::ConsumeDef>),
        ComponentDescription::new(
            "emit_output",
            "A stimulus sent once to every receiver within a radius of the receiver but itself.",
        )
        .field(FieldDescription::new(
            "stimulus",
            FieldType::Text,
            "a damage type or a declared tag; the load checks it",
        ))
        .field(FieldDescription::new(
            "amount",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "the stimulus's amount",
        ))
        .field(FieldDescription::new(
            "radius",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "px from the receiver's point to another's surface, or to its point without a body",
        ))
        .sample(r#"{ "stimulus": "heat", "amount": 1, "radius": 40 }"#)
        .roundtrip(round_trip::<outputs::EmitDef>),
        ComponentDescription::new(
            "set_tile_output",
            "A tile cell written: the first tileset entry with a sprite, and the old tile back after a while.",
        )
        .field(FieldDescription::new(
            "tile",
            FieldType::Text,
            "the sprite ID; a map whose tileset lacks it changes nothing",
        ))
        .field(
            FieldDescription::new(
                "revert",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds until the old tile comes back, kept per map cell and saved; a later write on the cell since drops it",
            )
            .default("null"),
        )
        .sample(r#"{ "tile": "ash" }"#)
        .roundtrip(round_trip::<outputs::SetTileDef>),
        ComponentDescription::new(
            "become_output",
            "The receiver replaced by another prefab under its save identity, and turned back after a while.",
        )
        .field(FieldDescription::new(
            "prefab",
            FieldType::Prefab,
            "what it becomes, at its point; it keeps the receiver's source",
        ))
        .field(
            FieldDescription::new(
                "revert",
                FieldType::Shape("become_revert"),
                "turns it into `to` after `after` seconds, under the same identity; saved",
            )
            .default("null"),
        )
        .sample(r#"{ "prefab": "lamp_doused" }"#)
        .roundtrip(round_trip::<becoming::BecomeDef>),
        ComponentDescription::new("become_revert", "A replacement turned back after a while.")
            .field(FieldDescription::new(
                "after",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "game seconds until it turns back",
            ))
            .field(FieldDescription::new(
                "to",
                FieldType::Prefab,
                "the prefab it turns into; data names it, since a named entity's save holds no prefab",
            ))
            .sample(r#"{ "after": 3, "to": "lamp" }"#)
            .roundtrip(round_trip::<becoming::RevertDef>),
        ComponentDescription::new(
            "blast_output",
            "A blast at the receiver's point or a cell's centre, each effect falling off linearly to 0 at its radius.",
        )
        .field(FieldDescription::new(
            "radius",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "px to a target's surface, or to its point without a body",
        ))
        .field(
            FieldDescription::new(
                "damage",
                FieldType::Damage,
                "at its centre, on each entity with health but the receiver, from the credit, at the next step's resolve",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "context",
                FieldType::List(Box::new(FieldType::Text)),
                "the tags its hits carry as their context, such as a game's `hit.blast`",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new("push", amount(), "a knockback at its centre, in px/s, away from it")
                .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "impulse",
                amount(),
                "a physics impulse at its centre on dynamic bodies and kinematic ones with a mass",
            )
            .default("0.0"),
        )
        .sample(r#"{ "radius": 60 }"#)
        .roundtrip(round_trip::<blast::BlastDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
    describe_kind(
        world,
        INTERACTIONS,
        "An `interaction`: a chemistry rule from a stimulus to outputs on what receives it.",
    );
}

#[cfg(test)]
#[path = "../tests/chemistry.rs"]
mod tests;
