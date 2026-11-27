//! Quests (redo 8.1, P8-2): the `quests` kind's stages, objectives on the
//! conditions and rewards, the progress a holder or the game keeps, saved,
//! and the system that completes a stage once its objectives hold.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::save;
use tungsten_core::{
    DataRegistry, Entity, EventQueue, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext,
    World, system,
};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, RPG, TOP_DOWN, describe, describe_kind,
    describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::order_of;
use crate::lifecycle::ended::KIT_DESPAWN;
use crate::motion::drive::centre_of;
use crate::react::counters::Counters;
use crate::react::cues::{CUES, CueAction, Target, check_list, run_list};
use crate::story::condition::Condition;

/// [`QuestsPlugin`]'s system, in the react phase.
pub const QUEST_TICK: &str = "quest_tick";

/// The data kind and the scene component.
pub(crate) const QUESTS: &str = "quests";
/// The restore hook's name.
const RESTORE_HOOK: &str = "kit.quests";

/// A quest: a `quests` entry, one a file. Its stages, by name, each show a
/// journal line and list objectives, each a [`Condition`] on the holder;
/// once every objective holds, the stage completes: the quest moves to its
/// `next`, or is done with none, then its `rewards`, cue actions, run on
/// the holder.
///
/// ```json
/// {
///   "title": "Lamplight",
///   "start": "light",
///   "stages": {
///     "light": { "journal": "Light the lamp by the steps.", "objectives": [{ "text": "Lamp lit", "when": { "flag": { "name": "lamp.lit", "target": "game" } } }], "next": "report" },
///     "report": { "journal": "Tell Tamsin it burns.", "rewards": [{ "counter": { "name": "coins", "add": 5, "target": "game" } }] }
///   }
/// }
/// ```
///
/// The parse holds `start` and each `next` to a stage, every stage to
/// reachable from `start` and the stages to no cycle. The load checks the
/// objectives' names and the rewards as a story list with no other entity,
/// since rewards run with none: a reward or an objective naming
/// `target: other` is refused. A stage with no objectives completes only
/// by a `quest` action's `advance`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Quest;
///
/// let lamplight: Quest = serde_json::from_str(
///     r#"{ "title": "Lamplight", "start": "light", "stages": { "light": { "journal": "Light it." } } }"#,
/// )
/// .unwrap();
/// assert_eq!((lamplight.title(), lamplight.start()), ("Lamplight", "light"));
/// assert_eq!(lamplight.journal("light"), Some("Light it."));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuestFields", into = "QuestFields")]
pub struct Quest {
    title: String,
    start: String,
    stages: BTreeMap<String, QuestStage>,
}

/// One stage of a quest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QuestStage {
    pub(crate) journal: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) objectives: Vec<Objective>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) next: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rewards: Vec<CueAction>,
}

/// One objective of a stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Objective {
    pub(crate) text: String,
    pub(crate) when: Condition,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestFields {
    title: String,
    start: String,
    stages: BTreeMap<String, QuestStage>,
}

impl From<Quest> for QuestFields {
    fn from(quest: Quest) -> Self {
        Self {
            title: quest.title,
            start: quest.start,
            stages: quest.stages,
        }
    }
}

impl TryFrom<QuestFields> for Quest {
    type Error = String;

    fn try_from(fields: QuestFields) -> Result<Self, Self::Error> {
        let stages = &fields.stages;
        if stages.is_empty() {
            return Err("stages: a quest has at least one stage".into());
        }
        if !stages.contains_key(&fields.start) {
            return Err(format!("start: unknown stage `{}`", fields.start));
        }
        for (name, stage) in stages {
            if let Some(next) = &stage.next
                && !stages.contains_key(next)
            {
                return Err(format!("stages.{name}.next: unknown stage `{next}`"));
            }
        }
        // Each stage has one `next` at most, so the stages reached from
        // `start` are a chain: walked once, a stage met twice is a cycle.
        let mut seen = BTreeSet::new();
        let mut at = Some(fields.start.as_str());
        while let Some(name) = at {
            if !seen.insert(name) {
                return Err(format!(
                    "stages.{name}: a cycle of stages, so the quest never ends"
                ));
            }
            at = stages[name].next.as_deref();
        }
        if let Some(name) = stages.keys().find(|name| !seen.contains(name.as_str())) {
            return Err(format!("stages.{name}: unreachable from `start`"));
        }
        Ok(Self {
            title: fields.title,
            start: fields.start,
            stages: fields.stages,
        })
    }
}

impl Quest {
    /// The title shown.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The stage a quest starts at.
    #[must_use]
    pub fn start(&self) -> &str {
        &self.start
    }

    /// Stage `stage`'s journal line.
    #[must_use]
    pub fn journal(&self, stage: &str) -> Option<&str> {
        self.stages.get(stage).map(|stage| stage.journal.as_str())
    }

    /// Whether it has stage `stage`.
    #[must_use]
    pub fn has_stage(&self, stage: &str) -> bool {
        self.stages.contains_key(stage)
    }

    /// Stage `stage`'s objectives: each its text and its condition.
    pub fn objectives(&self, stage: &str) -> impl Iterator<Item = (&str, &Condition)> {
        self.stages
            .get(stage)
            .into_iter()
            .flat_map(|stage| stage.objectives.iter())
            .map(|objective| (objective.text.as_str(), &objective.when))
    }

    pub(crate) fn stage(&self, stage: &str) -> Option<&QuestStage> {
        self.stages.get(stage)
    }

    /// Stage `stage`'s objectives as a journal shows them for `holder`
    /// (`None`: the game's quests): each one's text, how far along it is and
    /// its goal. A lone `counter` form with a `min` gives its count, held to
    /// the goal, against that `min`; any other form 1 of 1 while it holds,
    /// else 0 of 1.
    pub fn progress<'a>(
        &'a self,
        world: &'a World,
        holder: Option<Entity>,
        stage: &str,
    ) -> impl Iterator<Item = (&'a str, i64, i64)> + 'a {
        self.stages
            .get(stage)
            .into_iter()
            .flat_map(|stage| stage.objectives.iter())
            .map(move |objective| {
                let (have, need) = match objective.when.counter_goal() {
                    Some((name, target, min)) => {
                        let counters = match (target, holder) {
                            (Target::Game, _) | (_, None) => world.get_resource::<Counters>(),
                            (_, Some(entity)) => world.get::<Counters>(entity),
                        };
                        let count = counters.map_or(0, |counters| counters.get(name));
                        (count.clamp(0, min.max(0)), min)
                    }
                    None => (i64::from(objective.when.holds(world, holder, None)), 1),
                };
                (objective.text.as_str(), have, need)
            })
    }
}

/// Where a quest stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    /// Under way at its stage.
    #[default]
    Active,
    /// Past its last stage.
    Done,
    /// Failed by a `quest` action.
    Failed,
}

impl QuestStatus {
    /// Its data name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// One quest's progress: its status and stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestState {
    #[serde(default)]
    status: QuestStatus,
    stage: String,
}

impl QuestState {
    /// Its status.
    #[must_use]
    pub fn status(&self) -> QuestStatus {
        self.status
    }

    /// Its stage: the one it is at, or was at when it ended.
    #[must_use]
    pub fn stage(&self) -> &str {
        &self.stage
    }
}

/// The quests a holder has taken, by `quests` ID: a component (`quests`,
/// on a holder such as the player) and the game's resource, as
/// [`Counters`](crate::Counters) are, saved both ways. A `quest` cue action
/// starts, advances, fails or moves one; [`QuestsPlugin`]'s system
/// completes an active quest's stage once its objectives hold. A restored
/// stage the data no longer holds restores at the quest's start, with a
/// warning.
///
/// Scene name `quests`: `{ "lamplight": { "stage": "light" } }`, each
/// entry's `status` `active` by default; the load checks each ID and stage.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{QuestStatus, Quests};
///
/// let quests: Quests =
///     serde_json::from_str(r#"{ "lamplight": { "stage": "light" } }"#).unwrap();
/// let lamp = quests.get("lamplight").unwrap();
/// assert_eq!((lamp.status(), lamp.stage()), (QuestStatus::Active, "light"));
/// assert!(quests.get("errand").is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Quests {
    quests: BTreeMap<String, QuestState>,
}

impl Quests {
    /// No quests.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Quest `id`'s progress, once taken.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&QuestState> {
        self.quests.get(id)
    }

    /// Each quest taken, by ID.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &QuestState)> {
        self.quests.iter().map(|(id, state)| (id.as_str(), state))
    }

    /// Whether any quest is active.
    #[must_use]
    pub fn any_active(&self) -> bool {
        self.quests
            .values()
            .any(|state| state.status == QuestStatus::Active)
    }
}

/// A quest's status or stage moved, sent by a `quest` action and by
/// [`QuestsPlugin`]'s system.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QuestChanged {
    /// The holder; `None` for the game's quests.
    pub holder: Option<Entity>,
    /// The quest's ID.
    pub quest: String,
    /// Its status now.
    pub status: QuestStatus,
    /// Its stage now.
    pub stage: String,
}

/// What a `quest` action does.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QuestOp {
    /// Starts it at its start; a quest taken once stays as it is.
    Start,
    /// Its stage completes, rewards and all.
    Advance,
    /// Fails it.
    Fail,
    /// Moves it straight to a stage, with no rewards; a quest not taken
    /// starts there.
    Stage(String),
}

/// Whose quests a `quest` action or condition reads.
pub(crate) type Holder = Option<Entity>;

/// The holder's quests: the entity's, else the game's.
pub(crate) fn quests_of(world: &World, holder: Holder) -> Option<&Quests> {
    match holder {
        Some(entity) => world.get::<Quests>(entity),
        None => world.get_resource::<Quests>(),
    }
}

fn quests_mut(world: &mut World, holder: Holder) -> &mut Quests {
    if let Some(entity) = holder {
        if !world.has::<Quests>(entity) {
            world.insert(entity, Quests::new());
        }
        world.get_mut::<Quests>(entity).expect("inserted")
    } else {
        if !world.has_resource::<Quests>() {
            world.insert_resource(Quests::new());
        }
        world.resource_mut::<Quests>()
    }
}

fn quest_data<'w>(world: &'w World, id: &str) -> Option<&'w Quest> {
    world.get_resource::<DataRegistry>()?.get::<Quest>(id)
}

/// Applies `op` to `holder`'s quest `id` at once; `false` when it changes
/// nothing (an unknown ID, a taken quest started again, an ended quest
/// advanced).
pub(crate) fn act(world: &mut World, holder: Holder, id: &str, op: &QuestOp) -> bool {
    let Some(quest) = quest_data(world, id).cloned() else {
        return false;
    };
    let current = quests_of(world, holder)
        .and_then(|quests| quests.get(id))
        .cloned();
    match (op, current) {
        (QuestOp::Start, None) => {
            set(
                world,
                holder,
                id,
                QuestStatus::Active,
                quest.start().to_owned(),
            );
            true
        }
        (QuestOp::Stage(stage), state)
            if quest.has_stage(stage)
                && state
                    .as_ref()
                    .is_none_or(|state| state.status == QuestStatus::Active) =>
        {
            set(world, holder, id, QuestStatus::Active, stage.clone());
            true
        }
        (QuestOp::Advance, Some(state)) if state.status == QuestStatus::Active => {
            complete(world, holder, id, &quest, &state.stage);
            true
        }
        (QuestOp::Fail, Some(state)) if state.status == QuestStatus::Active => {
            set(world, holder, id, QuestStatus::Failed, state.stage);
            true
        }
        _ => false,
    }
}

/// Writes the quest's state and sends [`QuestChanged`].
fn set(world: &mut World, holder: Holder, id: &str, status: QuestStatus, stage: String) {
    quests_mut(world, holder).quests.insert(
        id.to_owned(),
        QuestState {
            status,
            stage: stage.clone(),
        },
    );
    if let Some(queue) = world.get_resource_mut::<EventQueue<QuestChanged>>() {
        queue.send(QuestChanged {
            holder,
            quest: id.to_owned(),
            status,
            stage,
        });
    }
}

/// Completes `stage` of `holder`'s quest: the quest moves to its `next`,
/// or is done, first, then the stage's rewards run on the holder with no
/// other, so a reward that advances its own quest completes the next stage.
fn complete(world: &mut World, holder: Holder, id: &str, quest: &Quest, stage: &str) {
    let Some(done) = quest.stage(stage) else {
        return;
    };
    match &done.next {
        Some(next) => set(world, holder, id, QuestStatus::Active, next.clone()),
        None => set(world, holder, id, QuestStatus::Done, stage.to_owned()),
    }
    let at = holder
        .and_then(|entity| centre_of(world, entity))
        .unwrap_or(Vec2::ZERO);
    run_list(world, &done.rewards, holder, None, at);
}

/// Registers the `quests` kind and scene component, the game's [`Quests`]
/// resource, both saved at version 1, `EventQueue<QuestChanged>`, a
/// restore hook that moves a vanished stage to its quest's start, and
/// [`QUEST_TICK`] in the react phase after the cues and before the kit's
/// despawn, so a count a cue wrote this step counts: for each holder in the
/// kit's step order, then the game's, each active quest whose stage has
/// objectives that all hold completes that stage, one stage a quest a
/// step. It does nothing in a World with no holder and no active game
/// quest.
#[derive(Debug, Default, Clone, Copy)]
pub struct QuestsPlugin;

impl Plugin for QuestsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Quests>(world, "Quests");
        world.register_data_kind_with_assets(QUESTS, check_quest);
        world.register_saved::<Quests>(QUESTS);
        world.register_saved_resource::<Quests>(QUESTS);
        world.register_event::<QuestChanged>();
        world.register_component_with_assets(
            QUESTS,
            check_quests,
            |quests: Quests, _: &SpawnContext<'_>| (quests,),
        );
        save::on_restore(world, RESTORE_HOOK, restore_stages);
        describe_quests(world);
        let mut holders = Vec::new();
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(
                system(QUEST_TICK, move |world: &mut World| {
                    quest_tick(world, &mut holders);
                })
                .after_if_present(CUES)
                .before_if_present(KIT_DESPAWN),
            ),
        );
    }
}

/// The `quests` kind's check: each objective's names, each objective and
/// reward free of `target: other`, the rewards as a story list with no
/// other.
fn check_quest(quest: &Quest, world: &World) -> Result<(), MissingAsset> {
    for (name, stage) in &quest.stages {
        for (i, objective) in stage.objectives.iter().enumerate() {
            let at = format!("stages.{name}.objectives.{i}.when");
            if objective.when.reads_other() {
                return Err(MissingAsset::new(
                    at,
                    "condition with no other entity",
                    "other",
                ));
            }
            objective.when.check(world, &at)?;
        }
        check_list(
            world,
            &format!("stages.{name}.rewards"),
            &stage.rewards,
            false,
        )?;
    }
    Ok(())
}

/// The `quests` component's check: each ID a `quests` entry and each stage
/// one of its own.
fn check_quests(quests: &Quests, world: &World) -> Result<(), MissingAsset> {
    let Some(data) = world.get_resource::<DataRegistry>() else {
        return Ok(());
    };
    for (id, state) in &quests.quests {
        let Some(quest) = data.get::<Quest>(id) else {
            return Err(MissingAsset::new(id.as_str(), "quest", id.as_str()));
        };
        if !quest.has_stage(&state.stage) {
            return Err(MissingAsset::new(
                format!("{id}.stage"),
                "quest stage",
                state.stage.as_str(),
            ));
        }
    }
    Ok(())
}

/// The restore hook: a stage the data no longer holds moves to its quest's
/// start, with a warning.
fn restore_stages(world: &mut World) {
    let mut holders: Vec<Holder> = vec![None];
    if world.has_any::<Quests>() {
        holders.extend(
            world
                .query::<(Entity, &Quests)>()
                .map(|(entity, _)| Some(entity)),
        );
    }
    for holder in holders {
        let Some(quests) = quests_of(world, holder) else {
            continue;
        };
        let stale: Vec<(String, String)> = quests
            .iter()
            .filter_map(|(id, state)| {
                let quest = quest_data(world, id)?;
                (!quest.has_stage(state.stage())).then(|| (id.to_owned(), quest.start().to_owned()))
            })
            .collect();
        for (id, start) in stale {
            log::warn!(
                "quest `{id}`: its saved stage is gone from the data; it restores at `{start}`"
            );
            if let Some(state) = quests_mut(world, holder).quests.get_mut(&id) {
                state.stage = start;
            }
        }
    }
}

/// Each holder's active quests, then the game's, whose stage's objectives
/// all hold complete that stage; `holders` is the scratch list.
fn quest_tick(world: &mut World, holders: &mut Vec<(Entity, (bool, u64))>) {
    let any_holder = world.has_any::<Quests>();
    let game = world
        .get_resource::<Quests>()
        .is_some_and(Quests::any_active);
    if !any_holder && !game {
        return;
    }
    holders.clear();
    if any_holder {
        let order = order_of(world);
        holders.extend(
            world
                .query::<(Entity, &Quests)>()
                .map(|(entity, _)| (entity, order.key(entity))),
        );
        holders.sort_by_key(|&(_, key)| key);
    }
    for &(entity, _) in holders.iter() {
        tick_holder(world, Some(entity));
    }
    if game {
        tick_holder(world, None);
    }
}

/// `holder`'s active quests whose objectives all hold complete their stage.
fn tick_holder(world: &mut World, holder: Holder) {
    let ready: Vec<(String, String)> = {
        let Some(quests) = quests_of(world, holder) else {
            return;
        };
        quests
            .iter()
            .filter(|(_, state)| state.status() == QuestStatus::Active)
            .filter_map(|(id, state)| {
                let stage = quest_data(world, id)?.stage(state.stage())?;
                let met = !stage.objectives.is_empty()
                    && stage
                        .objectives
                        .iter()
                        .all(|objective| objective.when.holds(world, holder, None));
                met.then(|| (id.to_owned(), state.stage().to_owned()))
            })
            .collect()
    };
    for (id, stage) in ready {
        if let Some(quest) = quest_data(world, &id).cloned() {
            complete(world, holder, &id, &quest, &stage);
        }
    }
}

impl Inspectable for Quests {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let summary: Vec<String> = self
            .quests
            .iter()
            .map(|(id, state)| format!("{id} {} at {}", state.status.name(), state.stage))
            .collect();
        vec![(
            "quests",
            if summary.is_empty() {
                "none".to_owned()
            } else {
                summary.join(", ")
            },
        )]
    }
}

/// The `quests` component and kind, and their shapes.
fn describe_quests(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            QUESTS,
            "The quests the entity has taken, by `quests` ID: each one's status and stage.",
        )
        .field(FieldDescription::new(
            "progress",
            FieldType::Object("quest ID → { status, stage }"),
            "the value itself: each quest taken, `status` `active`, `done` or `failed` (active by default) and its `stage`; the load checks each",
        ))
        .genre(TOP_DOWN)
        .genre(RPG)
        .sample(r#"{ "lamplight": { "stage": "light" } }"#)
        .roundtrip(round_trip::<Quests>),
    );
    describe_kind(
        world,
        QUESTS,
        "A `quest`: stages of a journal line, objectives on conditions and rewards a holder takes.",
    );
    let shapes = [
        ComponentDescription::new("quest", "A `quests` entry: stages walked from `start`, none in a cycle.")
            .field(FieldDescription::new("title", FieldType::Text, "the title shown"))
            .field(FieldDescription::new(
                "start",
                FieldType::Text,
                "the stage a start begins at",
            ))
            .field(FieldDescription::new(
                "stages",
                FieldType::Object("{ stage name: quest_stage }"),
                "each reachable from `start` through the `next`s, which form no cycle",
            ))
            .sample(r#"{ "title": "Lamplight", "start": "light", "stages": { "light": { "journal": "Light it." } } }"#)
            .roundtrip(round_trip::<Quest>),
        ComponentDescription::new(
            "quest_stage",
            "A stage: completed once its objectives all hold, then its `next` and its rewards.",
        )
        .field(FieldDescription::new(
            "journal",
            FieldType::Text,
            "its line in the journal",
        ))
        .field(
            FieldDescription::new(
                "objectives",
                FieldType::List(Box::new(FieldType::Shape("quest_objective"))),
                "each holding completes the stage; none completes it only by `advance`",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "next",
                FieldType::Text,
                "the stage after it; none: the quest is done",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "rewards",
                FieldType::List(Box::new(FieldType::Shape("cue_action"))),
                "run on the holder, with no other, as the stage completes; no `ability`, `loop`, `stop`, `label` or `target: other`",
            )
            .default("[]"),
        )
        .sample(r#"{ "journal": "Light it." }"#)
        .roundtrip(stage_round_trip),
        ComponentDescription::new("quest_objective", "An objective: its line and its condition.")
            .field(FieldDescription::new(
                "text",
                FieldType::Text,
                "its line; a lone `counter` with a `min` shows its count against it",
            ))
            .field(FieldDescription::new(
                "when",
                FieldType::Shape("condition"),
                "met while it holds, the holder the subject, with no other",
            ))
            .sample(r#"{ "text": "Lamp lit", "when": { "flag": "lamp.lit" } }"#)
            .roundtrip(round_trip::<Objective>),
        ComponentDescription::new(
            "quest_action",
            "A quest of the subject, the other or the game started, advanced, failed or moved.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Text,
            "the `quests` entry; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "op",
                FieldType::Enum(&["start", "advance", "fail"]),
                "started (a quest taken stays), its stage completed with its rewards, or failed",
            )
            .default(r#""start""#),
        )
        .field(
            FieldDescription::new(
                "stage",
                FieldType::Text,
                "in `op`'s place: moved straight to this stage, no rewards; a quest not taken starts there",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "target",
                FieldType::Enum(&["self", "other", "game"]),
                "the subject's quests, the other entity's or the game's",
            )
            .default(r#""self""#),
        )
        .sample(r#"{ "id": "lamplight" }"#)
        .roundtrip(round_trip::<QuestActionDef>),
        ComponentDescription::new(
            "quest_condition",
            "A quest's status, or its stage while active.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Text,
            "the `quests` entry; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "is",
                FieldType::Enum(&["new", "active", "done", "failed"]),
                "not taken, under way, done or failed",
            )
            .default(r#""active""#),
        )
        .field(
            FieldDescription::new(
                "stage",
                FieldType::Text,
                "active at this stage; the load checks it",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "target",
                FieldType::Enum(&["self", "other", "game"]),
                "the subject's quests, the other entity's or the game's",
            )
            .default(r#""self""#),
        )
        .sample(r#"{ "id": "lamplight" }"#)
        .roundtrip(round_trip::<crate::story::condition::QuestConditionDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

/// The `quest_stage` round trip: a list left out, which the description
/// writes as its default `[]`.
fn stage_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut out = round_trip::<QuestStage>(value)?;
    if let Some(object) = out.as_object_mut() {
        for list in ["objectives", "rewards"] {
            object
                .entry(list)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        }
    }
    Ok(out)
}

/// A `quest` action as data writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "QuestActionFields", into = "QuestActionFields")]
pub(crate) struct QuestActionDef {
    pub(crate) id: String,
    pub(crate) op: QuestOp,
    pub(crate) target: Target,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestActionFields {
    id: String,
    #[serde(default)]
    op: OpName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage: Option<String>,
    #[serde(default)]
    target: Target,
}

/// A `quest` action's `op`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OpName {
    #[default]
    Start,
    Advance,
    Fail,
}

impl TryFrom<QuestActionFields> for QuestActionDef {
    type Error = String;

    fn try_from(fields: QuestActionFields) -> Result<Self, Self::Error> {
        let op = match (fields.stage, fields.op) {
            (Some(_), OpName::Advance | OpName::Fail) => {
                return Err("`stage` takes `op`'s place: give one of them".into());
            }
            (Some(stage), OpName::Start) => QuestOp::Stage(stage),
            (None, OpName::Start) => QuestOp::Start,
            (None, OpName::Advance) => QuestOp::Advance,
            (None, OpName::Fail) => QuestOp::Fail,
        };
        Ok(Self {
            id: fields.id,
            op,
            target: fields.target,
        })
    }
}

impl From<QuestActionDef> for QuestActionFields {
    fn from(def: QuestActionDef) -> Self {
        let (op, stage) = match def.op {
            QuestOp::Start => (OpName::Start, None),
            QuestOp::Advance => (OpName::Advance, None),
            QuestOp::Fail => (OpName::Fail, None),
            QuestOp::Stage(stage) => (OpName::Start, Some(stage)),
        };
        Self {
            id: def.id,
            op,
            stage,
            target: def.target,
        }
    }
}

/// The load's check of a quest ID and, when given, its stage.
pub(crate) fn check_quest_ref(
    world: &World,
    field: &str,
    id: &str,
    stage: Option<&str>,
) -> Result<(), MissingAsset> {
    let Some(data) = world.get_resource::<DataRegistry>() else {
        return Ok(());
    };
    let Some(quest) = data.get::<Quest>(id) else {
        return Err(MissingAsset::new(format!("{field}.id"), "quest", id));
    };
    match stage {
        Some(stage) if !quest.has_stage(stage) => Err(MissingAsset::new(
            format!("{field}.stage"),
            "quest stage",
            stage,
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "../tests/quests.rs"]
mod tests;
