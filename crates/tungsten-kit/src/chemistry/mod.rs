//! Chemistry (redo 7.10b, the ARPG design's §5.2): stimuli meet what
//! receives them through the `interactions` kind's rules, a saved, bounded
//! and capped queue and a fixed list of outputs. A landed hit stimulates by
//! each of its damage types; a status's end and a band's entry run their
//! own outputs.

pub(crate) mod outputs;
pub(crate) mod queue;
pub(crate) mod rules;

use std::rc::Rc;

use tungsten_core::{Entity, EventCursor, EventQueue, Plugin, Schedule, World, system};

use crate::damage::ailments::AILMENT_FEED;
use crate::damage::effects::definition;
use crate::damage::health::{DamagedPart, Health};
use crate::damage::pipeline::DAMAGE_RESOLVE;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape,
    round_trip,
};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::tags::{Tags, covers};
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::Despawning;

pub use outputs::{Output, Reaction};
pub use queue::Chemistry;
pub use rules::Interaction;

use outputs::{Handler, Handlers, run_outputs};
use queue::{CHEMISTRY_QUEUE, Entry, Settings, StatusList, Stimulus, queue_saved};
use rules::{INTERACTIONS, answered, any_rule, register_interactions, snapshot, winner};

/// [`ChemistryPlugin`]'s system, in the resolve phase after
/// [`DAMAGE_RESOLVE`](crate::DAMAGE_RESOLVE) and the ailment feed when they
/// are registered, before [`EFFECT_RESOLVE`](crate::EFFECT_RESOLVE), so the
/// statuses it applies land in its step.
pub const CHEMISTRY: &str = "chemistry";

/// Why an entry did nothing, for the trace.
const GONE: &str = "the receiver is gone or despawning";
const NO_RULE: &str = "no rule passes";

/// Registers chemistry: the `interactions` kind, the saved queue
/// ([`Chemistry`]) and [`CHEMISTRY`], which reads the step's landed parts
/// and runs the queue.
///
/// Each run: every non-periodic [`DamagedPart`] since the last is a
/// stimulus of each of its deepest types, its amount after the resistances,
/// credited to its source, queued when some rule names its type or an
/// ancestor of it, one entry per stimulus and receiver: a merge sums the
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
/// ends. With no `interactions` entry and an empty queue the run returns
/// before any query.
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
    /// Chemistry with a run's cap of 256, a bound of 16,384 entries and a
    /// depth limit of 8, and no handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cap: queue::CAP,
            bound: queue::BOUND,
            depth: queue::DEPTH,
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
        });
        world.register_saved_resource_with(CHEMISTRY_QUEUE, queue_saved());
        if schedule.contains(CHEMISTRY) {
            return;
        }
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

/// [`CHEMISTRY`]'s state between steps: its place in the parts queue and
/// its scratch lists, kept so a run allocates little once warm.
#[derive(Default)]
struct Runner {
    cursor: EventCursor<DamagedPart>,
    parts: Vec<DamagedPart>,
    rules: Vec<(String, Interaction)>,
    taken: Vec<Entry>,
    /// A part's deepest types that some rule answers.
    leaves: Vec<String>,
}

impl Runner {
    fn run(&mut self, world: &mut World) {
        let waiting = world
            .get_resource::<Chemistry>()
            .is_some_and(|chemistry| chemistry.pending() > 0);
        if !waiting && !any_rule(world) {
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
                chemistry.stimulate(&settings, part.entity, name, stimulus, run);
            }
        }
        self.parts = parts;

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
        let receiver = entry.receiver;
        if !world.is_alive(receiver) || world.has::<Despawning>(receiver) {
            trace(world, receiver, || TraceKind::Skipped {
                event: "interaction",
                action: "all",
                why: GONE,
            });
            return;
        }
        let dead = world.get::<Health>(receiver).is_some_and(Health::is_dead);
        let stimulus = world
            .resource::<Chemistry>()
            .name(entry.stimulus)
            .to_owned();
        let mut reaction = Reaction {
            receiver,
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
        let tags: Option<&Tags> = world.get::<Tags>(receiver);
        let Some((id, rule)) = winner(&self.rules, &reaction.stimulus, entry.raw, tags) else {
            trace(world, receiver, || TraceKind::Skipped {
                event: "interaction",
                action: "all",
                why: NO_RULE,
            });
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
    receiver: Entity,
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
