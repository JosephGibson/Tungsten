//! Conditions (redo 8.1): a closed vocabulary that data writes where a
//! story item holds only when the World says so: an interactable's `when`,
//! a dialogue line's, a choice's and a quest objective's.

use serde::{Deserialize, Serialize};
use tungsten_core::{Entity, MissingAsset, World};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, describe_shape, round_trip,
};
use crate::foundation::stats::{Stats, check_stat};
use crate::foundation::tags::{TagFilter, Tags, valid_tag};
use crate::items::inventory::{check_base_ref, count_of};
use crate::react::counters::{Counters, Flags};
use crate::react::cues::Target;
use crate::story::quests::{QuestStatus, check_quest_ref, quests_of};

/// How deep conditions nest, the outermost counting as one.
pub(crate) const MAX_DEPTH: usize = 8;
/// How many conditions an `all` or `any` lists.
pub(crate) const MAX_LIST: usize = 32;

/// A test of the World that data writes as one object with one key, from a
/// closed set of forms, with no expressions:
///
/// | Form | Data | Holds when |
/// | --- | --- | --- |
/// | flag | `{ "flag": "nets_torn" }`, or `{ "flag": { "name": .., "target": "game" } }` | the target's [`Flags`] has it |
/// | counter | `{ "counter": { "name": "trust", "min": 3, "max": 9 } }`, one or both bounds, inclusive | the target's count, 0 when it holds none, lies in them |
/// | stat | `{ "stat": { "name": "vigour", "min": 20 } }`, none, one or both | the target has the stat and its [`Stats::get`] lies in them |
/// | tags | `{ "tags": { "all": [..], "any": [..], "none": [..] } }` | the target's [`Tags`] pass the filter |
/// | all, any | `{ "all": [..] }`, `{ "any": [..] }`, each at least one | every one, at least one |
/// | not | `{ "not": { .. } }` | the inner fails |
/// | quest | `{ "quest": { "id": "lamplight", "is": "active", "stage": "light" } }` | the target's quest is new (not taken), active (at `stage` when given), done or failed |
/// | item | `{ "item": { "id": "net", "min": 3, "max": 9 } }`, none, one or both, inclusive | the target holds that many of the base, bag and worn; with neither bound, at least 1 |
///
/// Each form but the lists takes a `target`: `self` (the default), `other`
/// or `game`, the words of the cue actions beside it; `game` reads the
/// World's own counters and flags, so a `stat`, `tags` or `item` form
/// refuses it.
/// A target that is absent or dead holds no form, so a `not` of it holds.
/// Conditions nest at most 8 deep and a list holds at most 32, refused at
/// parse past either. The load checks a `stat` form's stat and a `tags`
/// form's tags are declared; flag and counter names stay undeclared, as
/// the cue actions' are.
///
/// # Examples
///
/// ```
/// use tungsten_core::World;
/// use tungsten_kit::{Condition, Counters, Flags};
///
/// let when: Condition = serde_json::from_str(
///     r#"{ "all": [{ "flag": "lamp.promised" }, { "not": { "counter": { "name": "trust", "max": 2 } } }] }"#,
/// )
/// .unwrap();
/// let mut world = World::new();
/// let keeper = world.spawn_with((Flags::new().with("lamp.promised"), Counters::new().with("trust", 3)));
/// assert!(when.holds(&world, Some(keeper), None));
/// world.get_mut::<Counters>(keeper).unwrap().set("trust", 1);
/// assert!(!when.holds(&world, Some(keeper), None));
/// assert!(!when.holds(&world, None, None), "no subject holds no form");
/// assert_eq!(when, Condition::all([
///     Condition::flag("lamp.promised"),
///     Condition::not(Condition::counter("trust", None, Some(2))),
/// ]));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ConditionDef", into = "ConditionDef")]
pub struct Condition {
    form: Form,
}

/// What a condition tests.
#[derive(Debug, Clone, PartialEq)]
enum Form {
    Flag {
        name: String,
        target: Target,
    },
    Counter {
        name: String,
        min: Option<i64>,
        max: Option<i64>,
        target: Target,
    },
    Stat {
        name: String,
        min: Option<f32>,
        max: Option<f32>,
        target: Target,
    },
    Tags {
        filter: TagFilter,
        target: Target,
    },
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),
    /// A quest's status, or its stage while active: boxed, so the form
    /// keeps its size.
    Quest(Box<QuestConditionDef>),
    /// How many of an item base the target holds: boxed, as the quest
    /// form is.
    Item(Box<ItemConditionDef>),
}

impl Condition {
    fn of(form: Form) -> Self {
        Self { form }
    }

    /// Holds when the subject's [`Flags`] has `name`.
    #[must_use]
    pub fn flag(name: impl Into<String>) -> Self {
        Self::of(Form::Flag {
            name: name.into(),
            target: Target::Subject,
        })
    }

    /// Holds when the subject's count of `name`, 0 when it holds none, lies
    /// in the inclusive bounds given.
    ///
    /// # Panics
    ///
    /// When neither bound is given, or `min` exceeds `max`.
    #[must_use]
    #[track_caller]
    pub fn counter(name: impl Into<String>, min: Option<i64>, max: Option<i64>) -> Self {
        if let Err(error) = counter_bounds(min, max) {
            panic!("Condition::counter: {error}");
        }
        Self::of(Form::Counter {
            name: name.into(),
            min,
            max,
            target: Target::Subject,
        })
    }

    /// Holds when the subject has stat `name` and its value lies in the
    /// inclusive bounds given, if any.
    ///
    /// # Panics
    ///
    /// When a bound is not finite, or `min` exceeds `max`.
    #[must_use]
    #[track_caller]
    pub fn stat(name: impl Into<String>, min: Option<f32>, max: Option<f32>) -> Self {
        if let Err(error) = stat_bounds(min, max) {
            panic!("Condition::stat: {error}");
        }
        Self::of(Form::Stat {
            name: name.into(),
            min,
            max,
            target: Target::Subject,
        })
    }

    /// Holds when the subject holds a count of item base `id`, bag and
    /// worn, in the inclusive bounds given; at least 1 with neither.
    ///
    /// # Panics
    ///
    /// When `min` exceeds `max`.
    #[must_use]
    #[track_caller]
    pub fn item(id: impl Into<String>, min: Option<i64>, max: Option<i64>) -> Self {
        if let (Some(min), Some(max)) = (min, max) {
            assert!(
                min <= max,
                "Condition::item: `min` {min} exceeds `max` {max}"
            );
        }
        Self::of(Form::Item(Box::new(ItemConditionDef {
            id: id.into(),
            min,
            max,
            target: Target::Subject,
        })))
    }

    /// Holds when the subject's [`Tags`] pass `filter`.
    #[must_use]
    pub fn tags(filter: TagFilter) -> Self {
        Self::of(Form::Tags {
            filter,
            target: Target::Subject,
        })
    }

    /// Holds when every one of `conditions` does.
    ///
    /// # Panics
    ///
    /// When the list is empty or longer than 32, or nests past 8.
    #[must_use]
    #[track_caller]
    pub fn all(conditions: impl IntoIterator<Item = Condition>) -> Self {
        Self::checked(Form::All(conditions.into_iter().collect()), "all")
    }

    /// Holds when at least one of `conditions` does.
    ///
    /// # Panics
    ///
    /// When the list is empty or longer than 32, or nests past 8.
    #[must_use]
    #[track_caller]
    pub fn any(conditions: impl IntoIterator<Item = Condition>) -> Self {
        Self::checked(Form::Any(conditions.into_iter().collect()), "any")
    }

    /// Holds when `condition` fails.
    ///
    /// # Panics
    ///
    /// When it nests past 8.
    #[must_use]
    #[track_caller]
    #[allow(clippy::should_implement_trait)]
    pub fn not(condition: Condition) -> Self {
        Self::checked(Form::Not(Box::new(condition)), "not")
    }

    #[track_caller]
    fn checked(form: Form, method: &str) -> Self {
        let condition = Self::of(form);
        if let Err(error) = condition.check_shape() {
            panic!("Condition::{method}: {error}");
        }
        condition
    }

    /// The condition, reading the event's other entity instead: an
    /// interactable's interactor, a conversation's listener.
    ///
    /// # Panics
    ///
    /// When it is an `all`, `any` or `not`, which take no target.
    #[must_use]
    #[track_caller]
    pub fn for_other(self) -> Self {
        self.retarget(Target::Other, "for_other")
    }

    /// The condition, reading the World's own counters or flags instead.
    ///
    /// # Panics
    ///
    /// When it is not a `flag` or `counter`.
    #[must_use]
    #[track_caller]
    pub fn for_game(self) -> Self {
        self.retarget(Target::Game, "for_game")
    }

    #[track_caller]
    fn retarget(mut self, to: Target, method: &str) -> Self {
        match &mut self.form {
            Form::Flag { target, .. } | Form::Counter { target, .. } => *target = to,
            Form::Stat { target, .. } | Form::Tags { target, .. } if to == Target::Other => {
                *target = to;
            }
            Form::Quest(quest) => quest.target = to,
            Form::Item(item) if to == Target::Other => item.target = to,
            form => panic!(
                "Condition::{method}: a `{}` condition takes no such target",
                form.name()
            ),
        }
        self
    }

    /// Whether it holds now, `subject` and `other` the entities its
    /// `self` and `other` targets read; allocates nothing.
    #[must_use]
    pub fn holds(&self, world: &World, subject: Option<Entity>, other: Option<Entity>) -> bool {
        let entity = |target: Target| -> Option<Entity> {
            match target {
                Target::Subject => subject,
                Target::Other => other,
                Target::Game => None,
            }
            .filter(|&entity| world.is_alive(entity))
        };
        match &self.form {
            Form::Flag { name, target } => {
                let flags = if *target == Target::Game {
                    world.get_resource::<Flags>()
                } else {
                    entity(*target).and_then(|entity| world.get::<Flags>(entity))
                };
                flags.is_some_and(|flags| flags.has(name))
            }
            Form::Counter {
                name,
                min,
                max,
                target,
            } => {
                let count = if *target == Target::Game {
                    world
                        .get_resource::<Counters>()
                        .map(|counters| counters.get(name))
                } else {
                    entity(*target).map(|entity| {
                        world
                            .get::<Counters>(entity)
                            .map_or(0, |counters| counters.get(name))
                    })
                };
                count.is_some_and(|count| {
                    min.is_none_or(|min| count >= min) && max.is_none_or(|max| count <= max)
                })
            }
            Form::Stat {
                name,
                min,
                max,
                target,
            } => entity(*target)
                .and_then(|entity| world.get::<Stats>(entity))
                .and_then(|stats| stats.get(name))
                .is_some_and(|value| {
                    min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
                }),
            Form::Tags { filter, target } => {
                entity(*target).is_some_and(|entity| filter.matches(world.get::<Tags>(entity)))
            }
            Form::All(list) => list
                .iter()
                .all(|condition| condition.holds(world, subject, other)),
            Form::Any(list) => list
                .iter()
                .any(|condition| condition.holds(world, subject, other)),
            Form::Not(inner) => !inner.holds(world, subject, other),
            Form::Quest(quest) => {
                let holder = match quest.target {
                    Target::Game => None,
                    target => match entity(target) {
                        Some(entity) => Some(entity),
                        None => return false,
                    },
                };
                let state = quests_of(world, holder).and_then(|quests| quests.get(&quest.id));
                match (quest.is, state) {
                    (QuestIs::New, state) => state.is_none(),
                    (QuestIs::Active, Some(state)) => {
                        state.status() == QuestStatus::Active
                            && quest
                                .stage
                                .as_deref()
                                .is_none_or(|stage| stage == state.stage())
                    }
                    (QuestIs::Done, Some(state)) => state.status() == QuestStatus::Done,
                    (QuestIs::Failed, Some(state)) => state.status() == QuestStatus::Failed,
                    _ => false,
                }
            }
            Form::Item(item) => entity(item.target).is_some_and(|entity| {
                let count = i64::from(count_of(world, entity, &item.id));
                match (item.min, item.max) {
                    (None, None) => count >= 1,
                    (min, max) => {
                        min.is_none_or(|min| count >= min) && max.is_none_or(|max| count <= max)
                    }
                }
            }),
        }
    }

    /// The load's check, its errors naming `field` and the path inside it
    /// (`when.all.1.stat.name`): a `stat` form's stat and a `tags` form's
    /// tags declared.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        match &self.form {
            Form::Flag { .. } | Form::Counter { .. } => Ok(()),
            Form::Stat { name, .. } => check_stat(world, &format!("{field}.stat.name"), name),
            Form::Tags { filter, .. } => filter.check(world, &format!("{field}.tags")),
            Form::All(list) | Form::Any(list) => {
                let form = self.form.name();
                for (i, condition) in list.iter().enumerate() {
                    condition.check(world, &format!("{field}.{form}.{i}"))?;
                }
                Ok(())
            }
            Form::Not(inner) => inner.check(world, &format!("{field}.not")),
            Form::Quest(quest) => check_quest_ref(
                world,
                &format!("{field}.quest"),
                &quest.id,
                quest.stage.as_deref(),
            ),
            Form::Item(item) => check_base_ref(world, &format!("{field}.item.id"), &item.id),
        }
    }

    /// Whether any form reads the event's other entity: a check for lists
    /// and objectives that run with none.
    pub(crate) fn reads_other(&self) -> bool {
        match &self.form {
            Form::Flag { target, .. }
            | Form::Counter { target, .. }
            | Form::Stat { target, .. }
            | Form::Tags { target, .. } => *target == Target::Other,
            Form::Quest(quest) => quest.target == Target::Other,
            Form::Item(item) => item.target == Target::Other,
            Form::All(list) | Form::Any(list) => list.iter().any(Condition::reads_other),
            Form::Not(inner) => inner.reads_other(),
        }
    }

    /// The lone `counter` form's name, target and least bound, when it is
    /// one with a `min`: a journal shows its count against it.
    pub(crate) fn counter_goal(&self) -> Option<(&str, Target, i64)> {
        match &self.form {
            Form::Counter {
                name,
                min: Some(min),
                target,
                ..
            } => Some((name, *target, *min)),
            _ => None,
        }
    }

    /// How deep it nests, itself counting as one.
    fn depth(&self) -> usize {
        match &self.form {
            Form::All(list) | Form::Any(list) => {
                1 + list.iter().map(Condition::depth).max().unwrap_or(0)
            }
            Form::Not(inner) => 1 + inner.depth(),
            _ => 1,
        }
    }

    /// The bounds' check: a list of 1 to 32, at most 8 deep.
    fn check_shape(&self) -> Result<(), String> {
        if let Form::All(list) | Form::Any(list) = &self.form {
            if list.is_empty() {
                return Err(format!("an `{}` lists at least one", self.form.name()));
            }
            if list.len() > MAX_LIST {
                return Err(format!(
                    "an `{}` lists at most {MAX_LIST}, not {}",
                    self.form.name(),
                    list.len()
                ));
            }
        }
        let depth = self.depth();
        if depth > MAX_DEPTH {
            return Err(format!(
                "conditions nest at most {MAX_DEPTH} deep, not {depth}"
            ));
        }
        Ok(())
    }
}

impl Form {
    /// Its data key.
    fn name(&self) -> &'static str {
        match self {
            Form::Flag { .. } => "flag",
            Form::Counter { .. } => "counter",
            Form::Stat { .. } => "stat",
            Form::Tags { .. } => "tags",
            Form::All(_) => "all",
            Form::Any(_) => "any",
            Form::Not(_) => "not",
            Form::Quest(_) => "quest",
            Form::Item(_) => "item",
        }
    }
}

/// A counter form's bounds: one or both, `min` no more than `max`.
fn counter_bounds(min: Option<i64>, max: Option<i64>) -> Result<(), String> {
    match (min, max) {
        (None, None) => Err("a counter condition takes `min`, `max` or both".into()),
        (Some(min), Some(max)) if min > max => Err(format!("`min` {min} exceeds `max` {max}")),
        _ => Ok(()),
    }
}

/// A stat form's bounds: finite, `min` no more than `max`.
fn stat_bounds(min: Option<f32>, max: Option<f32>) -> Result<(), String> {
    for (field, bound) in [("min", min), ("max", max)] {
        if bound.is_some_and(|bound| !bound.is_finite()) {
            return Err(format!("`{field}` must be finite"));
        }
    }
    match (min, max) {
        (Some(min), Some(max)) if min > max => Err(format!("`min` {min} exceeds `max` {max}")),
        _ => Ok(()),
    }
}

/// A condition as data writes it: one key.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ConditionDef {
    Flag(FlagConditionDef),
    Counter(CounterConditionDef),
    Stat(StatConditionDef),
    Tags(TagsConditionDef),
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),
    Quest(QuestConditionDef),
    Item(ItemConditionDef),
}

/// An item form's object: a base, inclusive bounds and an entity's bag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ItemConditionFields", into = "ItemConditionFields")]
pub(crate) struct ItemConditionDef {
    id: String,
    min: Option<i64>,
    max: Option<i64>,
    target: Target,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemConditionFields {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max: Option<i64>,
    #[serde(default)]
    target: Target,
}

impl TryFrom<ItemConditionFields> for ItemConditionDef {
    type Error = String;

    fn try_from(fields: ItemConditionFields) -> Result<Self, Self::Error> {
        if fields.id.is_empty() {
            return Err("item: `id` names an `item_bases` entry".into());
        }
        if fields.target == Target::Game {
            return Err("item: a bag is an entity's; `target` is `self` or `other`".into());
        }
        if let (Some(min), Some(max)) = (fields.min, fields.max)
            && min > max
        {
            return Err(format!("item: `min` {min} exceeds `max` {max}"));
        }
        Ok(Self {
            id: fields.id,
            min: fields.min,
            max: fields.max,
            target: fields.target,
        })
    }
}

impl From<ItemConditionDef> for ItemConditionFields {
    fn from(def: ItemConditionDef) -> Self {
        Self {
            id: def.id,
            min: def.min,
            max: def.max,
            target: def.target,
        }
    }
}

/// A quest form's object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QuestConditionDef {
    id: String,
    #[serde(default)]
    is: QuestIs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stage: Option<String>,
    #[serde(default)]
    target: Target,
}

/// What a quest form asks of its quest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QuestIs {
    /// Not taken.
    New,
    /// Under way, at `stage` when given.
    #[default]
    Active,
    /// Done.
    Done,
    /// Failed.
    Failed,
}

/// A flag form: its name alone, or the object.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum FlagConditionDef {
    Name(String),
    Object(FlagConditionFields),
}

/// A flag form's object.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FlagConditionFields {
    name: String,
    #[serde(default)]
    target: Target,
}

/// A counter form's object.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "CounterConditionFields")]
pub(crate) struct CounterConditionDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<i64>,
    target: Target,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterConditionFields {
    name: String,
    #[serde(default)]
    min: Option<i64>,
    #[serde(default)]
    max: Option<i64>,
    #[serde(default)]
    target: Target,
}

impl TryFrom<CounterConditionFields> for CounterConditionDef {
    type Error = String;

    fn try_from(fields: CounterConditionFields) -> Result<Self, Self::Error> {
        counter_bounds(fields.min, fields.max)?;
        Ok(Self {
            name: fields.name,
            min: fields.min,
            max: fields.max,
            target: fields.target,
        })
    }
}

/// An entity's target, as a `stat` or `tags` form writes it: never the
/// game's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EntityTarget {
    #[default]
    #[serde(rename = "self")]
    Subject,
    Other,
}

impl From<EntityTarget> for Target {
    fn from(target: EntityTarget) -> Self {
        match target {
            EntityTarget::Subject => Target::Subject,
            EntityTarget::Other => Target::Other,
        }
    }
}

impl From<Target> for EntityTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Other => EntityTarget::Other,
            Target::Subject | Target::Game => EntityTarget::Subject,
        }
    }
}

/// A stat form's object.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "StatConditionFields")]
pub(crate) struct StatConditionDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<f32>,
    target: EntityTarget,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatConditionFields {
    name: String,
    #[serde(default)]
    min: Option<f32>,
    #[serde(default)]
    max: Option<f32>,
    #[serde(default)]
    target: EntityTarget,
}

impl TryFrom<StatConditionFields> for StatConditionDef {
    type Error = String;

    fn try_from(fields: StatConditionFields) -> Result<Self, Self::Error> {
        stat_bounds(fields.min, fields.max)?;
        Ok(Self {
            name: fields.name,
            min: fields.min,
            max: fields.max,
            target: fields.target,
        })
    }
}

/// A tags form's object: a tag filter's lists and a target.
#[derive(Serialize, Deserialize)]
pub(crate) struct TagsConditionDef {
    #[serde(flatten)]
    filter: TagFilter,
    #[serde(default)]
    target: EntityTarget,
}

impl TryFrom<ConditionDef> for Condition {
    type Error = String;

    fn try_from(def: ConditionDef) -> Result<Self, Self::Error> {
        let form = match def {
            ConditionDef::Flag(FlagConditionDef::Name(name)) => Form::Flag {
                name,
                target: Target::Subject,
            },
            ConditionDef::Flag(FlagConditionDef::Object(fields)) => Form::Flag {
                name: fields.name,
                target: fields.target,
            },
            ConditionDef::Counter(def) => Form::Counter {
                name: def.name,
                min: def.min,
                max: def.max,
                target: def.target,
            },
            ConditionDef::Stat(def) => Form::Stat {
                name: def.name,
                min: def.min,
                max: def.max,
                target: def.target.into(),
            },
            ConditionDef::Tags(def) => Form::Tags {
                filter: def.filter,
                target: def.target.into(),
            },
            ConditionDef::All(list) => Form::All(list),
            ConditionDef::Any(list) => Form::Any(list),
            ConditionDef::Not(inner) => Form::Not(inner),
            ConditionDef::Quest(quest) => {
                if quest.stage.is_some() && quest.is != QuestIs::Active {
                    return Err(
                        "quest: a `stage` reads an active quest; `is` must be `active`".into(),
                    );
                }
                Form::Quest(Box::new(quest))
            }
            ConditionDef::Item(item) => Form::Item(Box::new(item)),
        };
        if let Form::Flag { name, .. } | Form::Counter { name, .. } = &form
            && !valid_tag(name)
        {
            return Err(format!(
                "{}: `{name}` is not a {} name (`[a-z][a-z0-9_]*` segments joined by dots)",
                form.name(),
                form.name()
            ));
        }
        let condition = Self::of(form);
        condition.check_shape()?;
        Ok(condition)
    }
}

impl From<Condition> for ConditionDef {
    fn from(condition: Condition) -> Self {
        match condition.form {
            Form::Flag {
                name,
                target: Target::Subject,
            } => ConditionDef::Flag(FlagConditionDef::Name(name)),
            Form::Flag { name, target } => {
                ConditionDef::Flag(FlagConditionDef::Object(FlagConditionFields {
                    name,
                    target,
                }))
            }
            Form::Counter {
                name,
                min,
                max,
                target,
            } => ConditionDef::Counter(CounterConditionDef {
                name,
                min,
                max,
                target,
            }),
            Form::Stat {
                name,
                min,
                max,
                target,
            } => ConditionDef::Stat(StatConditionDef {
                name,
                min,
                max,
                target: target.into(),
            }),
            Form::Tags { filter, target } => ConditionDef::Tags(TagsConditionDef {
                filter,
                target: target.into(),
            }),
            Form::All(list) => ConditionDef::All(list),
            Form::Any(list) => ConditionDef::Any(list),
            Form::Not(inner) => ConditionDef::Not(inner),
            Form::Quest(quest) => ConditionDef::Quest(*quest),
            Form::Item(item) => ConditionDef::Item(*item),
        }
    }
}

/// The described target of a form: its words and whom they read.
fn target_field(values: &'static [&'static str], doc: &'static str) -> FieldDescription {
    FieldDescription::new("target", FieldType::Enum(values), doc).default(r#""self""#)
}

/// The shapes of a condition and its forms, described once.
pub(crate) fn describe_conditions(world: &mut World) {
    let shapes = [
        ComponentDescription::new(
            "condition",
            "A test of the World: one form, no expressions; a target absent or dead holds none.",
        )
        .one_of()
        .field(FieldDescription::new(
            "flag",
            FieldType::Shape("flag_condition"),
            "the target has the flag; a name alone reads as `{ \"name\": .. }`",
        ))
        .field(FieldDescription::new(
            "counter",
            FieldType::Shape("counter_condition"),
            "the target's count, 0 when it holds none, lies in the bounds",
        ))
        .field(FieldDescription::new(
            "stat",
            FieldType::Shape("stat_condition"),
            "the target has the stat and it lies in the bounds; the load checks the stat",
        ))
        .field(FieldDescription::new(
            "tags",
            FieldType::Shape("tags_condition"),
            "the target's tags pass the filter; the load checks its tags",
        ))
        .field(FieldDescription::new(
            "all",
            FieldType::List(Box::new(FieldType::Shape("condition"))),
            "every one holds: 1 to 32, nested at most 8 deep",
        ))
        .field(FieldDescription::new(
            "any",
            FieldType::List(Box::new(FieldType::Shape("condition"))),
            "at least one holds: 1 to 32, nested at most 8 deep",
        ))
        .field(FieldDescription::new(
            "not",
            FieldType::Shape("condition"),
            "the inner fails",
        ))
        .field(FieldDescription::new(
            "quest",
            FieldType::Shape("quest_condition"),
            "the target's quest has the status, or the stage while active; the load checks both",
        ))
        .field(FieldDescription::new(
            "item",
            FieldType::Shape("item_condition"),
            "the target holds that many of the base, bag and worn; the load checks the base",
        ))
        .sample(r#"{ "flag": "door.open" }"#)
        .roundtrip(round_trip::<Condition>),
        ComponentDescription::new("flag_condition", "A flag the target has.")
            .field(FieldDescription::new(
                "name",
                FieldType::Text,
                "the flag: dotted, undeclared",
            ))
            .field(target_field(
                &["self", "other", "game"],
                "the subject's flags, the other entity's or the game's",
            ))
            .sample(r#"{ "name": "door.open" }"#)
            .roundtrip(round_trip::<FlagConditionFields>),
        ComponentDescription::new(
            "counter_condition",
            "A count within inclusive bounds: one or both of `min` and `max`.",
        )
        .field(FieldDescription::new(
            "name",
            FieldType::Text,
            "the counter: dotted, undeclared",
        ))
        .field(
            FieldDescription::new(
                "min",
                FieldType::I64 {
                    min: None,
                    max: None,
                },
                "the least count that holds",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "max",
                FieldType::I64 {
                    min: None,
                    max: None,
                },
                "the greatest count that holds",
            )
            .default("null"),
        )
        .field(target_field(
            &["self", "other", "game"],
            "the subject's counters, the other entity's or the game's",
        ))
        .sample(r#"{ "name": "trust", "min": 3 }"#)
        .roundtrip(round_trip::<CounterConditionDef>),
        ComponentDescription::new(
            "stat_condition",
            "A stat the target has, within inclusive finite bounds when given.",
        )
        .field(FieldDescription::new(
            "name",
            FieldType::Text,
            "the stat: well known or declared; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "min",
                FieldType::F32 {
                    min: crate::Bound::Open,
                    max: crate::Bound::Open,
                },
                "the least value that holds",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "max",
                FieldType::F32 {
                    min: crate::Bound::Open,
                    max: crate::Bound::Open,
                },
                "the greatest value that holds",
            )
            .default("null"),
        )
        .field(target_field(
            &["self", "other"],
            "the subject or the other entity",
        ))
        .sample(r#"{ "name": "vigour" }"#)
        .roundtrip(round_trip::<StatConditionDef>),
        ComponentDescription::new("tags_condition", "A tag filter the target's tags pass.")
            .field(
                FieldDescription::new(
                    "all",
                    FieldType::List(Box::new(FieldType::Tag)),
                    "every one of these",
                )
                .default("[]"),
            )
            .field(
                FieldDescription::new(
                    "any",
                    FieldType::List(Box::new(FieldType::Tag)),
                    "at least one of these, when any are given",
                )
                .default("[]"),
            )
            .field(
                FieldDescription::new(
                    "none",
                    FieldType::List(Box::new(FieldType::Tag)),
                    "none of these",
                )
                .default("[]"),
            )
            .field(target_field(
                &["self", "other"],
                "the subject or the other entity",
            ))
            .sample(r#"{ "all": ["team.player"] }"#)
            .roundtrip(tags_round_trip),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

/// The `tags_condition` round trip: an empty list left out, which the
/// description writes as its default `[]`.
fn tags_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut out = round_trip::<TagsConditionDef>(value)?;
    if let Some(object) = out.as_object_mut() {
        for list in ["all", "any", "none"] {
            object
                .entry(list)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "../tests/condition.rs"]
mod tests;
