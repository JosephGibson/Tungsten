//! Cues: the kit's one way to react to events in data. An entity's `cues`
//! and the `cue_rules` data kind map an event to a list of actions, and
//! [`CuesOnEvent`] brings a game's own event in.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{
    AudioCommands, CommandBuffer, DataRef, DataRegistry, Entity, EventCursor, EventQueue,
    Inspectable, MissingAsset, Plugin, Schedule, ShakeEvent, SoundRegistry, SpawnContext,
    SquashEvent, SquashTrigger, Time, Transform, World, system,
};

use crate::damage::health::{Damaged, DamagedPart, start_flash};
use crate::damage::kinds::{DamageKinds, DamageTypeTable, check_type, label_style, merge_types};
use crate::damage::pipeline::Healed;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN,
    describe, describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::non_negative;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::rng::KitRng;
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared, valid_tag};
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::cooldown::CooldownElapsed;
use crate::lifecycle::ended::{Despawning, mark_despawning};
use crate::overlap::hit::Struck;
use crate::overlap::pickup::PickedUp;
use crate::overlap::projectile::ProjectileExpired;
use crate::overlap::zone::ZoneEntered;
use crate::present::labels::{
    LABEL_TICK, LabelDef, LabelStyle, LabelStyleDef, LiveLabels, float_label, label_tick,
};
use crate::react::counters::{Counters, Flags};
use crate::react::loot::{LOOT_TABLES, WeightedTable, register_loot_tables};
use crate::transform::hierarchy::despawn_recursive;

/// [`CuesPlugin`]'s system, in the react phase, last of the kit's items.
pub const CUES: &str = "cues";

/// The data kind of shared rules.
const CUE_RULES: &str = "cue_rules";

/// The kit's cue events, in the order the cue system reads them.
pub(crate) const DAMAGED_PART: &str = "damaged_part";
pub(crate) const DAMAGED: &str = "damaged";
pub(crate) const KILLED: &str = "killed";
pub(crate) const ZONE_ENTERED: &str = "zone_entered";
pub(crate) const COOLDOWN_ELAPSED: &str = "cooldown_elapsed";
pub(crate) const HIT: &str = "hit";
pub(crate) const HEALED: &str = "healed";
pub(crate) const PICKED_UP: &str = "picked_up";
pub(crate) const EXPIRED: &str = "expired";
const KIT_EVENTS: [&str; 9] = [
    DAMAGED_PART,
    DAMAGED,
    KILLED,
    ZONE_ENTERED,
    COOLDOWN_ELAPSED,
    HIT,
    HEALED,
    PICKED_UP,
    EXPIRED,
];

/// The squash triggers' data names, as a scene's `squash_stretch` writes
/// them.
const SQUASH_NAMES: &[&str] = &["on_land", "on_hit", "on_pickup", "manual"];

/// Whether `name` is an event name: `[a-z][a-z0-9_]*`.
fn valid_event(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// An entity's reactions: for each event about it, a list of
/// [`CueAction`]s that [`CuesPlugin`]'s system runs in order, in the fixed
/// step the event is read. The events are the kit's `damaged_part` (one per
/// part of a hit, before the hit's own), `damaged` and `killed` (after its
/// `damaged`; the hit's source is the other), `zone_entered`
/// (the zone is the subject, the entrant the other), `cooldown_elapsed`,
/// `hit` (the hitting entity is the subject, the target the other),
/// `healed` (the heal's source the other), `picked_up` (the pickup is the
/// subject, the collector the other) and `expired` (a projectile's end),
/// and each name a game's [`CuesOnEvent`] registers.
///
/// Scene name `cues`:
/// `{ "damaged": [{ "flash": {} }], "killed": [{ "prefab": "puff" }, { "despawn": {} }] }`;
/// the load checks each event is registered and each action's IDs. It is
/// data: it serializes as that object.
///
/// # Examples
///
/// ```
/// use tungsten_core::SquashTrigger;
/// use tungsten_kit::{CueAction, Cues};
///
/// let cues = Cues::new()
///     .on("damaged", [CueAction::flash(), CueAction::squash(SquashTrigger::OnHit)])
///     .on("killed", [CueAction::prefab("puff"), CueAction::despawn()]);
/// let names: Vec<&str> = cues.actions("killed").iter().map(CueAction::name).collect();
/// assert_eq!(names, ["prefab", "despawn"]);
/// assert!(cues.actions("zone_entered").is_empty());
/// let data = serde_json::to_value(&cues).unwrap();
/// assert_eq!(data["damaged"][1], serde_json::json!({ "squash": "on_hit" }));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    try_from = "BTreeMap<String, Vec<CueAction>>",
    into = "BTreeMap<String, Vec<CueAction>>"
)]
pub struct Cues {
    /// Sorted by event; shared, so the cue system's step takes a list
    /// without copying its actions.
    lists: Vec<(String, Arc<[CueAction]>)>,
}

impl TryFrom<BTreeMap<String, Vec<CueAction>>> for Cues {
    type Error = String;

    fn try_from(lists: BTreeMap<String, Vec<CueAction>>) -> Result<Self, Self::Error> {
        if let Some(event) = lists.keys().find(|event| !valid_event(event)) {
            return Err(format!(
                "`{event}` is not an event name (`[a-z][a-z0-9_]*`)"
            ));
        }
        Ok(Self {
            lists: lists
                .into_iter()
                .map(|(event, actions)| (event, actions.into()))
                .collect(),
        })
    }
}

impl From<Cues> for BTreeMap<String, Vec<CueAction>> {
    fn from(cues: Cues) -> Self {
        cues.lists
            .into_iter()
            .map(|(event, actions)| (event, actions.to_vec()))
            .collect()
    }
}

impl Cues {
    /// No reactions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The cues with `actions` after any `event` already has.
    ///
    /// # Panics
    ///
    /// When `event` is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn on(mut self, event: &str, actions: impl IntoIterator<Item = CueAction>) -> Self {
        assert!(
            valid_event(event),
            "Cues::on: `{event}` is not an event name (`[a-z][a-z0-9_]*`)"
        );
        match self
            .lists
            .binary_search_by(|(known, _)| known.as_str().cmp(event))
        {
            Ok(at) => {
                let list = &mut self.lists[at].1;
                *list = list.iter().cloned().chain(actions).collect();
            }
            Err(at) => self
                .lists
                .insert(at, (event.to_owned(), actions.into_iter().collect())),
        }
        self
    }

    /// The actions for `event`, in order; none when it has no list.
    #[must_use]
    pub fn actions(&self, event: &str) -> &[CueAction] {
        self.lists
            .binary_search_by(|(known, _)| known.as_str().cmp(event))
            .map_or(&[], |at| &self.lists[at].1)
    }

    /// The shared list for `event`, for the cue system.
    fn list(&self, event: &str) -> Option<Arc<[CueAction]>> {
        self.lists
            .binary_search_by(|(known, _)| known.as_str().cmp(event))
            .ok()
            .map(|at| Arc::clone(&self.lists[at].1))
    }

    /// Each event and its actions, by event.
    fn iter(&self) -> impl Iterator<Item = (&str, &[CueAction])> {
        self.lists
            .iter()
            .map(|(event, actions)| (event.as_str(), &actions[..]))
    }
}

/// One reaction to an event: spawn a prefab, roll a loot table, change a
/// counter or a flag, despawn, or present (a sound, a shake, a squash, a
/// flash, a label).
/// `counter`, `flag` and `despawn` act on the event's subject by default;
/// [`for_other`](Self::for_other) points them at the event's other entity
/// and [`for_game`](Self::for_game) the first two at the World's own
/// [`Counters`](crate::Counters) and [`Flags`](crate::Flags). The rest act
/// on the subject or at its position. Presentation actions change no
/// gameplay state.
///
/// Data form, one field per action:
///
/// | Action | Data |
/// | --- | --- |
/// | prefab | `{ "prefab": "puff" }`, spawned at the event's position from the subject |
/// | counter | `{ "counter": { "name": "kills", "add": 1, "target": "game" } }`, or `"set": 0`; `target` `self`, `other` or `game` |
/// | flag | `{ "flag": { "name": "has_key", "set": true, "target": "other" } }`; `set` true by default |
/// | despawn | `{ "despawn": {} }`, or `{ "despawn": { "target": "other" } }` |
/// | sound | `{ "sound": { "id": "blip", "min_interval": 0.1 } }`, at most once per interval of game time per sound |
/// | shake | `{ "shake": 0.3 }`, camera trauma added |
/// | squash | `{ "squash": "on_hit" }`, the subject's squash trigger |
/// | flash | `{ "flash": {} }`, the subject's [`HitFlash`](crate::HitFlash) |
/// | label | `{ "label": { "font": "sans" } }`, the event's amount as a [`LabelStyle`] label; on `damaged_part`, `{ "label": {} }` floats it in its type's style |
/// | roll | `{ "roll": "slime_drops" }`, a `loot_tables` entry or the [`WeightedTable`] in place: each prefab drawn spawned at the event's position from the subject |
///
/// # Examples
///
/// ```
/// use tungsten_kit::CueAction;
///
/// let score = CueAction::counter_add("score", 10).for_game();
/// assert_eq!(
///     serde_json::to_value(&score).unwrap(),
///     serde_json::json!({ "counter": { "name": "score", "add": 10, "target": "game" } })
/// );
/// let blip: CueAction =
///     serde_json::from_str(r#"{ "sound": { "id": "blip", "min_interval": 0.1 } }"#).unwrap();
/// assert_eq!(blip, CueAction::sound("blip").with_min_interval(0.1));
/// assert!(serde_json::from_str::<CueAction>(r#"{ "flash": {}, "shake": 1 }"#).is_err());
/// ```
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CueActionDef", into = "CueActionDef")]
pub struct CueAction {
    kind: ActionKind,
}

/// What an action does.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ActionKind {
    Prefab(String),
    Counter {
        name: String,
        op: CounterOp,
        target: Target,
    },
    Flag {
        name: String,
        on: bool,
        target: Target,
    },
    Despawn(Target),
    Sound {
        id: String,
        min_interval: f32,
    },
    Shake(f32),
    Squash(SquashTrigger),
    Flash,
    /// `None`: a `damaged_part`'s amount in its type's style.
    Label(Option<LabelStyle>),
    Roll(DataRef<WeightedTable>),
}

/// A counter action's change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterOp {
    Add(i64),
    Set(i64),
}

/// Whom a `counter`, `flag` or `despawn` action acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Target {
    /// The event's subject.
    #[default]
    #[serde(rename = "self")]
    Subject,
    /// The event's other entity.
    Other,
    /// The World's own counters and flags.
    Game,
}

impl CueAction {
    fn of(kind: ActionKind) -> Self {
        Self { kind }
    }

    /// Spawns prefab `id`, a manifest `prefabs` ID, at the event's
    /// position, from the subject, so it takes what the World's spawn
    /// inheritance gives from it.
    #[must_use]
    pub fn prefab(id: impl Into<String>) -> Self {
        Self::of(ActionKind::Prefab(id.into()))
    }

    /// Despawns the subject, with its children, after the step's spawns.
    #[must_use]
    pub fn despawn() -> Self {
        Self::of(ActionKind::Despawn(Target::Subject))
    }

    /// Adds `by` to the subject's counter `name`, inserting its
    /// [`Counters`](crate::Counters) when it has none.
    ///
    /// # Panics
    ///
    /// When `name` is not a counter name (`[a-z][a-z0-9_]*` segments joined
    /// by dots).
    #[must_use]
    #[track_caller]
    pub fn counter_add(name: &str, by: i64) -> Self {
        Self::of(ActionKind::Counter {
            name: checked_name("CueAction::counter_add", name),
            op: CounterOp::Add(by),
            target: Target::Subject,
        })
    }

    /// Sets the subject's counter `name` to `value`.
    ///
    /// # Panics
    ///
    /// As [`counter_add`](Self::counter_add).
    #[must_use]
    #[track_caller]
    pub fn counter_set(name: &str, value: i64) -> Self {
        Self::of(ActionKind::Counter {
            name: checked_name("CueAction::counter_set", name),
            op: CounterOp::Set(value),
            target: Target::Subject,
        })
    }

    /// Sets the subject's flag `name` when `on`, clears it otherwise,
    /// inserting its [`Flags`](crate::Flags) when it has none.
    ///
    /// # Panics
    ///
    /// When `name` is not a flag name.
    #[must_use]
    #[track_caller]
    pub fn flag(name: &str, on: bool) -> Self {
        Self::of(ActionKind::Flag {
            name: checked_name("CueAction::flag", name),
            on,
            target: Target::Subject,
        })
    }

    /// The action, on the event's other entity instead: a zone's entrant.
    ///
    /// # Panics
    ///
    /// When the action is not a `counter`, `flag` or `despawn`.
    #[must_use]
    #[track_caller]
    pub fn for_other(self) -> Self {
        self.retarget(Target::Other, "for_other")
    }

    /// The action, on the World's own counters or flags instead.
    ///
    /// # Panics
    ///
    /// When the action is not a `counter` or `flag`.
    #[must_use]
    #[track_caller]
    pub fn for_game(self) -> Self {
        self.retarget(Target::Game, "for_game")
    }

    #[track_caller]
    fn retarget(mut self, to: Target, method: &str) -> Self {
        let name = self.name();
        match &mut self.kind {
            ActionKind::Counter { target, .. } | ActionKind::Flag { target, .. } => *target = to,
            ActionKind::Despawn(target) if to == Target::Other => *target = to,
            _ => panic!("CueAction::{method}: a `{name}` action takes no target"),
        }
        self
    }

    /// Plays manifest sound `id` at its volume, never looping.
    #[must_use]
    pub fn sound(id: impl Into<String>) -> Self {
        Self::of(ActionKind::Sound {
            id: id.into(),
            min_interval: 0.0,
        })
    }

    /// The sound, played at most once per `seconds` of game time per sound
    /// ID.
    ///
    /// # Panics
    ///
    /// When the action is not a `sound`, or `seconds` is not finite and at
    /// least 0.
    #[must_use]
    #[track_caller]
    pub fn with_min_interval(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "CueAction::with_min_interval: seconds must be finite and >= 0 (got {seconds})"
        );
        let name = self.name();
        match &mut self.kind {
            ActionKind::Sound { min_interval, .. } => *min_interval = seconds,
            _ => panic!("CueAction::with_min_interval: a `{name}` action plays no sound"),
        }
        self
    }

    /// Adds `trauma` to the camera's shake.
    ///
    /// # Panics
    ///
    /// When `trauma` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn shake(trauma: f32) -> Self {
        assert!(
            trauma.is_finite() && trauma >= 0.0,
            "CueAction::shake: trauma must be finite and >= 0 (got {trauma})"
        );
        Self::of(ActionKind::Shake(trauma))
    }

    /// Arms the subject's squash for `trigger`.
    #[must_use]
    pub fn squash(trigger: SquashTrigger) -> Self {
        Self::of(ActionKind::Squash(trigger))
    }

    /// Starts the subject's [`HitFlash`](crate::HitFlash), as a hit does.
    #[must_use]
    pub fn flash() -> Self {
        Self::of(ActionKind::Flash)
    }

    /// Floats the event's amount at its position in `style`.
    #[must_use]
    pub fn label(style: LabelStyle) -> Self {
        Self::of(ActionKind::Label(Some(style)))
    }

    /// Floats a `damaged_part` occurrence's amount at its position in its
    /// part's type's style: the `label_style` of the deepest damage type
    /// among its kinds, the first by name at one depth, from that type's
    /// `damage_types` entry or its nearest ancestor's that sets one. A part
    /// whose types set none floats nothing, and the trace says so. Data form
    /// `{ "label": {} }`; the load refuses it on any other event.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_kit::{CueAction, Cues};
    ///
    /// let numbers = Cues::new().on("damaged_part", [CueAction::type_label()]);
    /// let data = serde_json::to_value(&numbers).unwrap();
    /// assert_eq!(data["damaged_part"][0], serde_json::json!({ "label": {} }));
    /// ```
    #[must_use]
    pub fn type_label() -> Self {
        Self::of(ActionKind::Label(None))
    }

    /// Rolls `table`, a `loot_tables` entry or a table in place: each of
    /// its rolls draws from the kit's [`KitRng`](crate::KitRng), in the cue
    /// system's order, and each prefab drawn spawns at the event's position
    /// from the subject, as [`prefab`](Self::prefab) spawns.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_core::DataRef;
    /// use tungsten_kit::{CueAction, WeightedTable};
    ///
    /// let drops = CueAction::roll(DataRef::by_id("slime_drops"));
    /// assert_eq!(serde_json::to_value(&drops).unwrap(), serde_json::json!({ "roll": "slime_drops" }));
    /// let chest = CueAction::roll(DataRef::inline(WeightedTable::new().with_entry("gold", 1.0)));
    /// assert_eq!(chest.name(), "roll");
    /// ```
    #[must_use]
    pub fn roll(table: DataRef<WeightedTable>) -> Self {
        Self::of(ActionKind::Roll(table))
    }

    /// The action's data name: `prefab`, `counter`, `flag`, `despawn`,
    /// `sound`, `shake`, `squash`, `flash`, `label` or `roll`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self.kind {
            ActionKind::Prefab(_) => "prefab",
            ActionKind::Counter { .. } => "counter",
            ActionKind::Flag { .. } => "flag",
            ActionKind::Despawn(_) => "despawn",
            ActionKind::Sound { .. } => "sound",
            ActionKind::Shake(_) => "shake",
            ActionKind::Squash(_) => "squash",
            ActionKind::Flash => "flash",
            ActionKind::Label(_) => "label",
            ActionKind::Roll(_) => "roll",
        }
    }
}

impl fmt::Debug for CueAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

/// `name` checked as a counter or flag name.
#[track_caller]
fn checked_name(method: &str, name: &str) -> String {
    assert!(
        valid_tag(name),
        "{method}: `{name}` is not a counter or flag name (`[a-z][a-z0-9_]*` segments joined by dots)"
    );
    name.to_owned()
}

/// An action as data writes it: an object with exactly one field.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CueActionDef {
    Prefab(String),
    Counter(CounterDef),
    Flag(FlagDef),
    Despawn(DespawnDef),
    Sound(SoundDef),
    Shake(#[serde(deserialize_with = "non_negative")] f32),
    Squash(SquashName),
    Flash(FlashDef),
    Label(LabelDef),
    Roll(DataRef<WeightedTable>),
}

/// A `counter` action as data writes it: exactly one of `add` and `set`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "CounterFields")]
pub(crate) struct CounterDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    add: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    set: Option<i64>,
    target: Target,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterFields {
    name: String,
    #[serde(default)]
    add: Option<i64>,
    #[serde(default)]
    set: Option<i64>,
    #[serde(default)]
    target: Target,
}

impl TryFrom<CounterFields> for CounterDef {
    type Error = String;

    fn try_from(fields: CounterFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.name) {
            return Err(format!(
                "name: `{}` is not a counter name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.name
            ));
        }
        if fields.add.is_some() == fields.set.is_some() {
            return Err("exactly one of `add` and `set`".to_string());
        }
        Ok(Self {
            name: fields.name,
            add: fields.add,
            set: fields.set,
            target: fields.target,
        })
    }
}

/// A `flag` action as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "FlagFields")]
pub(crate) struct FlagDef {
    name: String,
    set: bool,
    target: Target,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlagFields {
    name: String,
    #[serde(default = "yes")]
    set: bool,
    #[serde(default)]
    target: Target,
}

fn yes() -> bool {
    true
}

impl TryFrom<FlagFields> for FlagDef {
    type Error = String;

    fn try_from(fields: FlagFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.name) {
            return Err(format!(
                "name: `{}` is not a flag name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.name
            ));
        }
        Ok(Self {
            name: fields.name,
            set: fields.set,
            target: fields.target,
        })
    }
}

/// A `despawn` action as data writes it: its subject, or the other.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DespawnDef {
    #[serde(default)]
    target: DespawnTarget,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DespawnTarget {
    #[default]
    #[serde(rename = "self")]
    Subject,
    Other,
}

/// A `sound` action as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SoundDef {
    id: String,
    #[serde(default, deserialize_with = "non_negative")]
    min_interval: f32,
}

/// A `flash` action as data writes it: no fields.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FlashDef {}

/// A squash trigger's data name.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SquashName {
    OnLand,
    OnHit,
    OnPickup,
    Manual,
}

impl From<SquashName> for SquashTrigger {
    fn from(name: SquashName) -> Self {
        match name {
            SquashName::OnLand => SquashTrigger::OnLand,
            SquashName::OnHit => SquashTrigger::OnHit,
            SquashName::OnPickup => SquashTrigger::OnPickup,
            SquashName::Manual => SquashTrigger::Manual,
        }
    }
}

impl From<SquashTrigger> for SquashName {
    fn from(trigger: SquashTrigger) -> Self {
        match trigger {
            SquashTrigger::OnLand => SquashName::OnLand,
            SquashTrigger::OnHit => SquashName::OnHit,
            SquashTrigger::OnPickup => SquashName::OnPickup,
            SquashTrigger::Manual => SquashName::Manual,
        }
    }
}

impl TryFrom<CueActionDef> for CueAction {
    type Error = String;

    fn try_from(def: CueActionDef) -> Result<Self, Self::Error> {
        let kind = match def {
            CueActionDef::Prefab(id) => ActionKind::Prefab(id),
            CueActionDef::Counter(def) => ActionKind::Counter {
                name: def.name,
                op: match (def.add, def.set) {
                    (Some(by), _) => CounterOp::Add(by),
                    (None, Some(value)) => CounterOp::Set(value),
                    (None, None) => unreachable!("checked at deserialization"),
                },
                target: def.target,
            },
            CueActionDef::Flag(def) => ActionKind::Flag {
                name: def.name,
                on: def.set,
                target: def.target,
            },
            CueActionDef::Despawn(def) => ActionKind::Despawn(match def.target {
                DespawnTarget::Subject => Target::Subject,
                DespawnTarget::Other => Target::Other,
            }),
            CueActionDef::Sound(def) => ActionKind::Sound {
                id: def.id,
                min_interval: def.min_interval,
            },
            CueActionDef::Shake(trauma) => ActionKind::Shake(trauma),
            CueActionDef::Squash(name) => ActionKind::Squash(name.into()),
            CueActionDef::Flash(FlashDef {}) => ActionKind::Flash,
            CueActionDef::Label(LabelDef(style)) => ActionKind::Label(style),
            CueActionDef::Roll(table) => ActionKind::Roll(table),
        };
        Ok(Self { kind })
    }
}

impl From<CueAction> for CueActionDef {
    fn from(action: CueAction) -> Self {
        match action.kind {
            ActionKind::Prefab(id) => CueActionDef::Prefab(id),
            ActionKind::Counter { name, op, target } => CueActionDef::Counter(CounterDef {
                name,
                add: match op {
                    CounterOp::Add(by) => Some(by),
                    CounterOp::Set(_) => None,
                },
                set: match op {
                    CounterOp::Set(value) => Some(value),
                    CounterOp::Add(_) => None,
                },
                target,
            }),
            ActionKind::Flag { name, on, target } => CueActionDef::Flag(FlagDef {
                name,
                set: on,
                target,
            }),
            ActionKind::Despawn(target) => CueActionDef::Despawn(DespawnDef {
                target: if target == Target::Other {
                    DespawnTarget::Other
                } else {
                    DespawnTarget::Subject
                },
            }),
            ActionKind::Sound { id, min_interval } => {
                CueActionDef::Sound(SoundDef { id, min_interval })
            }
            ActionKind::Shake(trauma) => CueActionDef::Shake(trauma),
            ActionKind::Squash(trigger) => CueActionDef::Squash(trigger.into()),
            ActionKind::Flash => CueActionDef::Flash(FlashDef {}),
            ActionKind::Label(style) => CueActionDef::Label(LabelDef(style)),
            ActionKind::Roll(table) => CueActionDef::Roll(table),
        }
    }
}

/// One occurrence of a game's event, as its [`CuesOnEvent`] map gives it:
/// the subject, where it happened, and the amount and the other entity the
/// actions may read. Build one with [`CueEvent::new`].
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::World;
/// use tungsten_kit::CueEvent;
///
/// let mut world = World::new();
/// let (coin, hero) = (world.spawn(), world.spawn());
/// let event = CueEvent::new(coin, Vec2::new(10.0, 4.0)).with_amount(5.0).with_other(hero);
/// assert_eq!((event.amount, event.other), (Some(5.0), Some(hero)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct CueEvent {
    /// The entity the event is about, whose cues run.
    pub entity: Entity,
    /// Where it happened: a `prefab` spawns there, a `label` floats there.
    pub position: Vec2,
    /// What a `label` shows.
    pub amount: Option<f32>,
    /// Whom a `target: other` action acts on.
    pub other: Option<Entity>,
}

impl CueEvent {
    /// An occurrence about `entity` at `position`, with no amount and no
    /// other entity.
    #[must_use]
    pub fn new(entity: Entity, position: Vec2) -> Self {
        Self {
            entity,
            position,
            amount: None,
            other: None,
        }
    }

    /// The occurrence, with `amount`.
    #[must_use]
    pub fn with_amount(mut self, amount: f32) -> Self {
        self.amount = Some(amount);
        self
    }

    /// The occurrence, with `other`.
    #[must_use]
    pub fn with_other(mut self, other: Entity) -> Self {
        self.other = Some(other);
        self
    }
}

/// One event the cue system runs actions for, from any source.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Occurrence {
    pub(crate) event: &'static str,
    pub(crate) subject: Entity,
    pub(crate) position: Vec2,
    pub(crate) amount: Option<f32>,
    pub(crate) other: Option<Entity>,
    /// A damage event's kinds: a part's, or a hit's parts' together.
    pub(crate) kinds: DamageKinds,
}

/// What a registered cue event may carry, for the load's checks.
#[derive(Debug, Clone, Copy)]
struct EventInfo {
    name: &'static str,
    amount: bool,
    other: bool,
    /// Its occurrences carry damage kinds, which a rule's `kind` reads.
    kinds: bool,
}

/// The cue events a load may name, a World resource: the kit's and each
/// [`CuesOnEvent`]'s.
#[derive(Debug)]
struct CueEventNames(Vec<EventInfo>);

impl Default for CueEventNames {
    fn default() -> Self {
        Self(
            [
                (DAMAGED_PART, true, true, true),
                (DAMAGED, true, true, true),
                (KILLED, true, true, true),
                (ZONE_ENTERED, false, true, false),
                (COOLDOWN_ELAPSED, false, false, false),
                (HIT, true, true, false),
                (HEALED, true, true, false),
                (PICKED_UP, false, true, false),
                (EXPIRED, false, false, false),
            ]
            .into_iter()
            .map(|(name, amount, other, kinds)| EventInfo {
                name,
                amount,
                other,
                kinds,
            })
            .collect(),
        )
    }
}

impl CueEventNames {
    fn get(&self, name: &str) -> Option<EventInfo> {
        self.0.iter().copied().find(|info| info.name == name)
    }
}

/// A game event's reader, kept in [`CueSources`].
trait CueSource {
    /// Appends the occurrences sent since the last read.
    fn read(&mut self, world: &World, out: &mut Vec<Occurrence>);
}

/// The game events' readers, a World resource, in build order.
#[derive(Default)]
struct CueSources(Vec<Box<dyn CueSource>>);

/// Brings a game's event `E` to cues: [`CuesPlugin`]'s system reads its
/// queue once per event, after the kit's, and runs the cues registered
/// under `name` for each occurrence its `map` gives; `None` skips the
/// event. The plugin registers `EventQueue<E>` (again is a no-op) and the
/// name, so `cues` and `cue_rules` may use it. A game event may carry an
/// amount and another entity, so the load lets a `label` or a
/// `target: other` action name it; an occurrence without them skips those
/// actions, and the trace says so.
///
/// It takes a game type, so it stays out of
/// [`KitPlugins::set()`](crate::KitPlugins::set); a game adds it after the
/// set and before the manifest loads.
///
/// # Panics
///
/// At build, when `name` is not `[a-z][a-z0-9_]*`, is one of the kit's
/// events or was registered before.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{Entity, Plugin, Schedule, World};
/// use tungsten_kit::{CueEvent, CuesOnEvent, CuesPlugin};
///
/// /// The game's chest event.
/// struct Opened {
///     chest: Entity,
///     by: Entity,
///     at: Vec2,
/// }
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// CuesPlugin.build(&mut schedule, &mut world);
/// CuesOnEvent::<Opened>::new("opened", |e| {
///     Some(CueEvent::new(e.chest, e.at).with_other(e.by))
/// })
/// .build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert!(world.has_event::<Opened>());
/// ```
pub struct CuesOnEvent<E> {
    name: &'static str,
    map: fn(&E) -> Option<CueEvent>,
}

impl<E: 'static> CuesOnEvent<E> {
    /// Event `E` as the cue event `name`, each occurrence given by `map`.
    #[must_use]
    pub fn new(name: &'static str, map: fn(&E) -> Option<CueEvent>) -> Self {
        Self { name, map }
    }
}

impl<E> Clone for CuesOnEvent<E> {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            map: self.map,
        }
    }
}

impl<E> fmt::Debug for CuesOnEvent<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CuesOnEvent")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// A game event's reader: its name, its map and its place in the queue.
struct GameSource<E> {
    name: &'static str,
    map: fn(&E) -> Option<CueEvent>,
    cursor: EventCursor<E>,
}

impl<E: 'static> CueSource for GameSource<E> {
    fn read(&mut self, world: &World, out: &mut Vec<Occurrence>) {
        let Some(queue) = world.get_resource::<EventQueue<E>>() else {
            return;
        };
        out.extend(
            self.cursor
                .read(queue)
                .filter_map(self.map)
                .map(|event| Occurrence {
                    event: self.name,
                    subject: event.entity,
                    position: event.position,
                    amount: event.amount,
                    other: event.other,
                    kinds: DamageKinds::NONE,
                }),
        );
    }
}

impl<E: 'static> Plugin for CuesOnEvent<E> {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        let name = self.name;
        assert!(
            valid_event(name),
            "CuesOnEvent: `{name}` is not an event name (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            !KIT_EVENTS.contains(&name),
            "CuesOnEvent: `{name}` is one of the kit's cue events"
        );
        if !world.has_resource::<CueEventNames>() {
            world.insert_resource(CueEventNames::default());
        }
        let names = world.resource_mut::<CueEventNames>();
        assert!(
            names.get(name).is_none(),
            "CuesOnEvent: the cue event `{name}` is already registered"
        );
        names.0.push(EventInfo {
            name,
            amount: true,
            other: true,
            kinds: false,
        });
        world.register_event::<E>();
        if !world.has_resource::<CueSources>() {
            world.insert_resource(CueSources::default());
        }
        world
            .resource_mut::<CueSources>()
            .0
            .push(Box::new(GameSource {
                name,
                map: self.map,
                cursor: EventCursor::new(),
            }));
    }
}

/// A rule of the `cue_rules` kind as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CueRuleDef {
    on: String,
    #[serde(default)]
    filter: TagFilterDef,
    #[serde(default)]
    kind: KindFilterDef,
    #[serde(rename = "do")]
    actions: Vec<CueAction>,
}

/// A rule's filter over an occurrence's damage kinds as data writes it:
/// declared damage types or their ancestors in `all`, `any` and `none`,
/// each checked as a tag name and kept as written, so a load's error names
/// the field.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "KindFields")]
struct KindFilterDef {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    all: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    any: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    none: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct KindFields {
    all: Vec<String>,
    any: Vec<String>,
    none: Vec<String>,
}

impl TryFrom<KindFields> for KindFilterDef {
    type Error = String;

    fn try_from(fields: KindFields) -> Result<Self, Self::Error> {
        for (field, names) in [
            ("all", &fields.all),
            ("any", &fields.any),
            ("none", &fields.none),
        ] {
            if let Some(name) = names.iter().find(|name| !valid_tag(name)) {
                return Err(format!(
                    "{field}: `{name}` is not a damage type (`[a-z][a-z0-9_]*` segments joined by dots)"
                ));
            }
        }
        Ok(Self {
            all: fields.all,
            any: fields.any,
            none: fields.none,
        })
    }
}

impl KindFilterDef {
    /// Every name the filter uses, with the field that names it as written
    /// (`any.0`), for a load's check.
    fn names(&self) -> impl Iterator<Item = (String, &str)> {
        [("all", &self.all), ("any", &self.any), ("none", &self.none)]
            .into_iter()
            .flat_map(|(field, names)| {
                names
                    .iter()
                    .enumerate()
                    .map(move |(i, name)| (format!("{field}.{i}"), name.as_str()))
            })
    }

    /// Whether it names no type, so every occurrence passes.
    fn is_everything(&self) -> bool {
        self.all.is_empty() && self.any.is_empty() && self.none.is_empty()
    }

    /// Whether an occurrence of `kinds` passes, each name's kinds from
    /// `mask`, read as a tag filter reads tags: every `all`, one `any` when
    /// it names any, no `none`.
    fn matches(&self, kinds: DamageKinds, mut mask: impl FnMut(&str) -> DamageKinds) -> bool {
        let mut has = |name: &String| kinds.intersects(mask(name));
        self.all.iter().all(&mut has)
            && (self.any.is_empty() || self.any.iter().any(&mut has))
            && !self.none.iter().any(&mut has)
    }
}

/// One shared rule: on event `on`, for a subject whose tags pass `filter`
/// and an occurrence whose damage kinds pass `kind`, the actions in order.
struct CueRule {
    on: String,
    filter: TagFilter,
    /// The filter's names with the fields that name them, for the load's
    /// check.
    names: Vec<(String, String)>,
    kind: KindFilterDef,
    actions: Arc<[CueAction]>,
}

/// A `cue_rules` entry: its rules, in file order.
#[derive(Deserialize)]
#[serde(try_from = "Vec<CueRuleDef>")]
struct CueRules(Vec<CueRule>);

impl TryFrom<Vec<CueRuleDef>> for CueRules {
    type Error = String;

    fn try_from(rules: Vec<CueRuleDef>) -> Result<Self, Self::Error> {
        let mut out = Vec::with_capacity(rules.len());
        for (i, rule) in rules.into_iter().enumerate() {
            if !valid_event(&rule.on) {
                return Err(format!(
                    "{i}.on: `{}` is not an event name (`[a-z][a-z0-9_]*`)",
                    rule.on
                ));
            }
            let names = rule
                .filter
                .names()
                .map(|(field, name)| (field, name.to_owned()))
                .collect();
            out.push(CueRule {
                on: rule.on,
                filter: rule.filter.into(),
                names,
                kind: rule.kind,
                actions: rule.actions.into(),
            });
        }
        Ok(Self(out))
    }
}

/// The `cue_rules` kind's check: each rule's event, its tag filter's
/// tags, its `kind` filter on an event with damage kinds, each of its names
/// a declared damage type or an ancestor of one, and its actions.
fn check_rule_list(rules: &CueRules, world: &World) -> Result<(), MissingAsset> {
    for (i, rule) in rules.0.iter().enumerate() {
        let event = event_info(world, &format!("{i}.on"), &rule.on)?;
        for (field, name) in &rule.names {
            check_declared(world, &format!("{i}.filter.{field}"), name)?;
        }
        if !rule.kind.is_everything() && !event.kinds {
            return Err(MissingAsset::new(
                format!("{i}.kind"),
                "cue event with damage kinds",
                event.name,
            ));
        }
        for (field, name) in rule.kind.names() {
            check_type(world, &format!("{i}.kind.{field}"), name, true)?;
        }
        check_actions(world, &format!("{i}.do"), event, &rule.actions)?;
    }
    Ok(())
}

/// Checks `actions`, run for `event`, against the World: each prefab,
/// sound and loot table ID and an inline table's prefabs, a `label` on an
/// event with an amount and a style-less one on `damaged_part` only, and a
/// `target: other` on an event with another entity; `field` names the
/// list.
fn check_actions(
    world: &World,
    field: &str,
    event: EventInfo,
    actions: &[CueAction],
) -> Result<(), MissingAsset> {
    for (i, action) in actions.iter().enumerate() {
        let at = format!("{field}.{i}.{}", action.name());
        match &action.kind {
            ActionKind::Prefab(id) => world.check_prefab_id(&at, id)?,
            ActionKind::Roll(table) => table.check(world, &at)?,
            ActionKind::Sound { id, .. } => {
                if let Some(sounds) = world.get_resource::<SoundRegistry>()
                    && sounds.get_by_id(id).is_none()
                {
                    return Err(MissingAsset::new(format!("{at}.id"), "sound", id.as_str()));
                }
            }
            ActionKind::Label(_) if !event.amount => {
                return Err(MissingAsset::new(
                    at,
                    "cue event with an amount",
                    event.name,
                ));
            }
            ActionKind::Label(None) if event.name != DAMAGED_PART => {
                return Err(MissingAsset::new(
                    at,
                    "cue event with a damage type's label style",
                    event.name,
                ));
            }
            ActionKind::Counter {
                target: Target::Other,
                ..
            }
            | ActionKind::Flag {
                target: Target::Other,
                ..
            }
            | ActionKind::Despawn(Target::Other)
                if !event.other =>
            {
                return Err(MissingAsset::new(
                    format!("{at}.target"),
                    "cue event with an other entity",
                    event.name,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// The registered cue event `name`, else the [`MissingAsset`] at `field`.
fn event_info(world: &World, field: &str, name: &str) -> Result<EventInfo, MissingAsset> {
    world
        .get_resource::<CueEventNames>()
        .and_then(|names| names.get(name))
        .ok_or_else(|| MissingAsset::new(field, "cue event", name))
}

/// Registers the `cues` scene component, the `cue_rules` and `loot_tables`
/// data kinds, the kit's cue events, a [`KitRng`](crate::KitRng) when the
/// World has none, [`CUES`] in the react phase, last of the kit's items,
/// and [`LABEL_TICK`] in the present phase.
///
/// Each fixed step, the cue system reads its sources, each once through an
/// `EventCursor`: [`DamagedPart`] (`damaged_part`), [`Damaged`] (a
/// `damaged` occurrence, then a `killed` one for a kill), [`ZoneEntered`],
/// [`CooldownElapsed`],
/// [`Struck`](crate::Struck) (`hit`), [`Healed`](crate::Healed),
/// [`PickedUp`](crate::PickedUp) (`picked_up`),
/// [`ProjectileExpired`](crate::ProjectileExpired) (`expired`), then each
/// [`CuesOnEvent`] in build order, each queue in send order. It runs every
/// occurrence's entity cues, then every occurrence's matching rules (rules
/// in entry-ID order, then file order); each list in its written order, so
/// in each pass every `damaged_part` runs before the step's `damaged` and
/// `killed`. A rule matches on its event, its tag filter over the subject's
/// [`Tags`](crate::Tags) and its `kind` filter over the occurrence's
/// [`DamageKinds`](crate::DamageKinds). `counter` and `flag` apply at once, `prefab` and
/// `roll` spawns queue as they are met, a `roll` drawing from the
/// [`KitRng`](crate::KitRng) in this order, and the step's despawns queue
/// after its spawns, each target marked with its descendants so a frame's
/// second step reacts to them no more; an occurrence whose subject is dead
/// or so marked is skipped.
///
/// A `cue_rules` entry is a list of rules,
/// `[{ "on": "killed", "filter": { "all": ["team.enemy"] }, "do": [{ "counter": { "name": "kills", "add": 1, "target": "game" } }] }]`,
/// `filter` everything by default. On `damaged`, `killed` and
/// `damaged_part`, a rule's `kind`, `{ "all": [..], "any": [..], "none": [..] }`
/// of declared damage types or their ancestors, read as a tag filter reads
/// tags, passes the occurrences whose damage kinds match it (a kill's are
/// its hit's); everything by default, and an untyped occurrence passes only
/// a `kind` without `all` and `any`. The load checks every event name, tag,
/// damage type, prefab, sound and loot table ID it names, and refuses a
/// `kind` on another event.
///
/// The vocabulary stops there: no conditions, variables or sequences, and
/// no filter but the rule's tag filter and, on the damage events, its kind
/// filter; a `roll` draws only from the table it names. A reaction that
/// needs more is a game's Rust.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tungsten_core::{ComponentRegistry, Plugin, Schedule, World};
/// use tungsten_kit::CuesPlugin;
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// CuesPlugin.build(&mut schedule, &mut world);
/// let components = world.resource::<ComponentRegistry>();
/// let flash = json!({ "damaged": [{ "flash": {} }] });
/// assert!(components.check_in("cues", &flash, &world).is_ok());
/// let typo = json!({ "dmaged": [{ "flash": {} }] });
/// let error = components.check_in("cues", &typo, &world).unwrap_err();
/// assert!(error.to_string().contains("unknown cue event `dmaged`"));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct CuesPlugin;

impl Plugin for CuesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Cues>(world, "Cues");
        describe_cues(world);
        if !world.has_resource::<CueEventNames>() {
            world.insert_resource(CueEventNames::default());
        }
        if !world.has_resource::<CueSources>() {
            world.insert_resource(CueSources::default());
        }
        if !world.has_resource::<LiveLabels>() {
            world.insert_resource(LiveLabels::default());
        }
        if !world.has_resource::<KitRng>() {
            world.insert_resource(KitRng::default());
        }
        register_loot_tables(world);
        world.register_data_kind_with_assets(CUE_RULES, check_rule_list);
        world.register_component_with_assets(
            CUES,
            |cues: &Cues, world: &World| {
                for (event, actions) in cues.iter() {
                    let info = event_info(world, event, event)?;
                    check_actions(world, event, info, actions)?;
                }
                Ok(())
            },
            |cues: Cues, _: &SpawnContext<'_>| (cues,),
        );
        let mut reactor = Reactor::default();
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(system(CUES, move |world: &mut World| reactor.run(world))),
        );
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(LABEL_TICK, label_tick)),
        );
    }
}

/// Why an action or an occurrence did not run, for the trace.
const GONE: &str = "the subject is gone";
const DESPAWNING: &str = "the subject is despawning";
const NO_OTHER: &str = "the event has no other entity, or it is gone";
const NO_AMOUNT: &str = "the event has no amount";
const NO_COMMANDS: &str = "the World has no CommandBuffer";
const NO_QUEUE: &str = "the World has no queue for it";
const NO_SOUND: &str = "the World has no sound or audio for it";
const TOO_SOON: &str = "the sound played within its min_interval";
const NO_TABLE: &str = "the World has no such loot table";
const NO_STYLE: &str = "the part's types set no label style";

/// The cue system's state between steps: its place in each kit queue, the
/// step's scratch lists, and each cue sound's last play on its clock.
#[derive(Default)]
struct Reactor {
    parts: EventCursor<DamagedPart>,
    damaged: EventCursor<Damaged>,
    zones: EventCursor<ZoneEntered>,
    cooldowns: EventCursor<CooldownElapsed>,
    struck: EventCursor<Struck>,
    healed: EventCursor<Healed>,
    picked: EventCursor<PickedUp>,
    expired: EventCursor<ProjectileExpired>,
    occurrences: Vec<Occurrence>,
    /// The rules that match one occurrence, with their names.
    matched: Vec<(String, Arc<[CueAction]>)>,
    despawns: Vec<Entity>,
    /// A roll's drawn prefabs, and the despawn mark's walk.
    drawn: Vec<String>,
    stack: Vec<Entity>,
    /// Game seconds the system has stepped through: the cue sounds' clock.
    clock: f64,
    /// Each sound ID's last play on `clock`.
    played: HashMap<String, f64>,
    /// The prefab IDs whose failure was logged.
    warned: HashSet<String>,
    /// Each `kind` filter name's kinds, kept until the type table grows.
    masks: HashMap<String, DamageKinds>,
    /// The type table's bits when `masks` was last cleared.
    masked: usize,
}

impl Reactor {
    fn run(&mut self, world: &mut World) {
        self.clock += f64::from(world.get_resource::<Time>().map_or(0.0, Time::delta));
        self.occurrences.clear();
        self.read(world);
        if self.occurrences.is_empty() {
            return;
        }
        merge_types(world);
        let types = world
            .get_resource::<DamageTypeTable>()
            .map_or(0, DamageTypeTable::len);
        if types != self.masked {
            self.masks.clear();
            self.masked = types;
        }
        let mut occurrences = std::mem::take(&mut self.occurrences);
        occurrences.retain(|occurrence| {
            let why = if !world.is_alive(occurrence.subject) {
                GONE
            } else if world.has::<Despawning>(occurrence.subject) {
                DESPAWNING
            } else {
                return true;
            };
            trace(world, occurrence.subject, || TraceKind::Skipped {
                event: occurrence.event,
                action: "all",
                why,
            });
            false
        });
        for occurrence in &occurrences {
            let Some(list) = world
                .get::<Cues>(occurrence.subject)
                .and_then(|cues| cues.list(occurrence.event))
            else {
                continue;
            };
            for action in list.iter() {
                self.apply(world, occurrence, action, None);
            }
        }
        for occurrence in &occurrences {
            self.match_rules(world, occurrence);
            let matched = std::mem::take(&mut self.matched);
            for (rule, list) in &matched {
                for action in list.iter() {
                    self.apply(world, occurrence, action, Some(rule));
                }
            }
            self.matched = matched;
            self.matched.clear();
        }
        self.occurrences = occurrences;
        for entity in self.despawns.drain(..) {
            if let Some(commands) = world.get_resource_mut::<CommandBuffer>() {
                despawn_recursive(commands, entity);
            }
            trace(world, entity, || TraceKind::Despawn { by: CUES });
        }
    }

    /// The rules for `occurrence`'s event whose tag filter its subject
    /// passes and whose kind filter its kinds pass, in entry-ID order, then
    /// file order.
    fn match_rules(&mut self, world: &World, occurrence: &Occurrence) {
        let Some(data) = world.get_resource::<DataRegistry>() else {
            return;
        };
        let table = world.get_resource::<DamageTypeTable>();
        let tags = world.get::<Tags>(occurrence.subject);
        let Self { matched, masks, .. } = self;
        let mut mask = |name: &str| -> DamageKinds {
            if let Some(&kinds) = masks.get(name) {
                return kinds;
            }
            let kinds = table.map_or(DamageKinds::NONE, |table| table.mask_of(name));
            masks.insert(name.to_owned(), kinds);
            kinds
        };
        for id in data.ids(CUE_RULES) {
            let Some(rules) = data.get::<CueRules>(id) else {
                continue;
            };
            for (i, rule) in rules.0.iter().enumerate() {
                if rule.on == occurrence.event
                    && rule.filter.matches(tags)
                    && (rule.kind.is_everything() || rule.kind.matches(occurrence.kinds, &mut mask))
                {
                    matched.push((format!("{id}.{i}"), Arc::clone(&rule.actions)));
                }
            }
        }
    }

    /// Runs `action` for `occurrence`, from rule `rule` or the subject's
    /// own cues, and traces it.
    fn apply(
        &mut self,
        world: &mut World,
        occurrence: &Occurrence,
        action: &CueAction,
        rule: Option<&str>,
    ) {
        let subject = occurrence.subject;
        let skip = |world: &mut World, why: &'static str| {
            trace(world, subject, || TraceKind::Skipped {
                event: occurrence.event,
                action: action.name(),
                why,
            });
        };
        let target = |target: Target, world: &World| -> Option<Entity> {
            match target {
                Target::Subject => Some(subject),
                Target::Other => occurrence.other.filter(|&other| world.is_alive(other)),
                Target::Game => None,
            }
        };
        match &action.kind {
            ActionKind::Prefab(id) => {
                if !world.has_resource::<CommandBuffer>() {
                    return skip(world, NO_COMMANDS);
                }
                if !self.spawn(world, subject, id, occurrence.position) {
                    return;
                }
            }
            ActionKind::Roll(table) => {
                if !world.has_resource::<CommandBuffer>() {
                    return skip(world, NO_COMMANDS);
                }
                if !self.draw(world, table) {
                    return skip(world, NO_TABLE);
                }
                let drawn = std::mem::take(&mut self.drawn);
                for id in &drawn {
                    self.spawn(world, subject, id, occurrence.position);
                }
                self.drawn = drawn;
            }
            ActionKind::Counter {
                name,
                op,
                target: to,
            } => {
                let counters = if *to == Target::Game {
                    if !world.has_resource::<Counters>() {
                        world.insert_resource(Counters::new());
                    }
                    world.resource_mut::<Counters>()
                } else {
                    let Some(entity) = target(*to, world) else {
                        return skip(world, NO_OTHER);
                    };
                    if !world.has::<Counters>(entity) {
                        world.insert(entity, Counters::new());
                    }
                    world.get_mut::<Counters>(entity).unwrap()
                };
                match *op {
                    CounterOp::Add(by) => {
                        counters.add(name, by);
                    }
                    CounterOp::Set(value) => counters.set(name, value),
                }
            }
            ActionKind::Flag {
                name,
                on,
                target: to,
            } => {
                let flags = if *to == Target::Game {
                    if !world.has_resource::<Flags>() {
                        world.insert_resource(Flags::new());
                    }
                    world.resource_mut::<Flags>()
                } else {
                    let Some(entity) = target(*to, world) else {
                        return skip(world, NO_OTHER);
                    };
                    if !world.has::<Flags>(entity) {
                        world.insert(entity, Flags::new());
                    }
                    world.get_mut::<Flags>(entity).unwrap()
                };
                flags.set(name, *on);
            }
            ActionKind::Despawn(to) => {
                let Some(entity) = target(*to, world) else {
                    return skip(world, NO_OTHER);
                };
                if !world.has_resource::<CommandBuffer>() {
                    return skip(world, NO_COMMANDS);
                }
                if !world.has::<Despawning>(entity) {
                    mark_despawning(world, entity, &mut self.stack);
                    self.despawns.push(entity);
                }
            }
            ActionKind::Sound { id, min_interval } => {
                let interval = f64::from(*min_interval);
                if interval > 0.0
                    && let Some(&last) = self.played.get(id)
                    && self.clock - last + SOUND_SLACK < interval
                {
                    return skip(world, TOO_SOON);
                }
                let Some((handle, volume)) = world
                    .get_resource::<SoundRegistry>()
                    .and_then(|sounds| sounds.get_by_id(id).map(|h| (h, sounds.get_volume(h))))
                else {
                    return skip(world, NO_SOUND);
                };
                let Some(commands) = world.get_resource_mut::<AudioCommands>() else {
                    return skip(world, NO_SOUND);
                };
                commands.play_with(handle, volume, false);
                self.played.insert(id.clone(), self.clock);
            }
            ActionKind::Shake(trauma) => {
                let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>() else {
                    return skip(world, NO_QUEUE);
                };
                queue.send(ShakeEvent {
                    trauma_add: *trauma,
                });
            }
            ActionKind::Squash(trigger) => {
                let Some(queue) = world.get_resource_mut::<EventQueue<SquashEvent>>() else {
                    return skip(world, NO_QUEUE);
                };
                queue.send(SquashEvent {
                    entity: subject,
                    trigger: *trigger,
                });
            }
            ActionKind::Flash => start_flash(world, subject),
            ActionKind::Label(Some(style)) => {
                let Some(amount) = occurrence.amount else {
                    return skip(world, NO_AMOUNT);
                };
                float_label(world, style, occurrence.position, amount);
            }
            ActionKind::Label(None) => {
                let Some(amount) = occurrence.amount else {
                    return skip(world, NO_AMOUNT);
                };
                let Some(style) = label_style(world, occurrence.kinds) else {
                    return skip(world, NO_STYLE);
                };
                float_label(world, &style, occurrence.position, amount);
            }
        }
        trace(world, subject, || TraceKind::Cue {
            event: occurrence.event,
            action: action.name(),
            rule: rule.map(str::to_owned),
        });
    }

    /// Queues prefab `id` at `position` from `subject` and traces the
    /// spawn, or traces and logs once why it spawns nothing; whether it
    /// spawned. The World holds a `CommandBuffer`.
    fn spawn(&mut self, world: &mut World, subject: Entity, id: &str, position: Vec2) -> bool {
        match world.prepare_prefab(id, position) {
            Ok(prepared) => {
                world
                    .resource_mut::<CommandBuffer>()
                    .spawn_prefab_from(subject, prepared);
                trace(world, subject, || TraceKind::Spawn {
                    prefab: id.to_owned(),
                    by: CUES,
                });
                true
            }
            Err(err) => {
                if self.warned.insert(id.to_owned()) {
                    log::error!("cues: prefab `{id}` spawns nothing: {err}");
                }
                trace(world, subject, || TraceKind::SpawnFailed {
                    prefab: id.to_owned(),
                    by: CUES,
                    error: err.to_string(),
                });
                false
            }
        }
    }

    /// Draws `table`'s rolls from the World's [`KitRng`] into `drawn`;
    /// false when the table is an ID the World's data lacks.
    fn draw(&mut self, world: &mut World, table: &DataRef<WeightedTable>) -> bool {
        self.drawn.clear();
        let mut rng = world.remove_resource::<KitRng>().unwrap_or_default();
        let found = match world.get_resource::<DataRegistry>() {
            Some(data) => table.resolve(data),
            None => table.as_inline(),
        };
        let resolved = found.is_some();
        if let Some(table) = found {
            for _ in 0..table.rolls() {
                if let Some(id) = table.draw(&mut rng) {
                    self.drawn.push(id.to_owned());
                }
            }
        }
        world.insert_resource(rng);
        resolved
    }

    /// The step's occurrences, from every source in order.
    fn read(&mut self, world: &mut World) {
        let Self {
            parts,
            damaged,
            zones,
            cooldowns,
            struck,
            healed,
            picked,
            expired,
            occurrences,
            ..
        } = self;
        if let Some(queue) = world.get_resource::<EventQueue<DamagedPart>>() {
            for event in parts.read(queue) {
                occurrences.push(Occurrence {
                    event: DAMAGED_PART,
                    subject: event.entity,
                    position: event
                        .position
                        .unwrap_or_else(|| point_of(world, event.entity)),
                    amount: Some(event.amount),
                    other: event.source,
                    kinds: event.kinds,
                });
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<Damaged>>() {
            for event in damaged.read(queue) {
                let position = event
                    .position
                    .unwrap_or_else(|| point_of(world, event.entity));
                let amount = Some(event.amount);
                occurrences.push(Occurrence {
                    event: DAMAGED,
                    subject: event.entity,
                    position,
                    amount,
                    other: event.source,
                    kinds: event.kinds,
                });
                if event.killed {
                    occurrences.push(Occurrence {
                        event: KILLED,
                        subject: event.entity,
                        position,
                        amount,
                        other: event.source,
                        kinds: event.kinds,
                    });
                }
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<ZoneEntered>>() {
            occurrences.extend(zones.read(queue).map(|event| Occurrence {
                event: ZONE_ENTERED,
                subject: event.zone,
                position: event.position,
                amount: None,
                other: Some(event.entity),
                kinds: DamageKinds::NONE,
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<CooldownElapsed>>() {
            occurrences.extend(cooldowns.read(queue).map(|event| Occurrence {
                event: COOLDOWN_ELAPSED,
                subject: event.entity,
                position: point_of(world, event.entity),
                amount: None,
                other: None,
                kinds: DamageKinds::NONE,
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<Struck>>() {
            occurrences.extend(struck.read(queue).map(|event| Occurrence {
                event: HIT,
                subject: event.entity,
                position: event.position,
                amount: Some(event.amount),
                other: Some(event.target),
                kinds: DamageKinds::NONE,
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<Healed>>() {
            occurrences.extend(healed.read(queue).map(|event| Occurrence {
                event: HEALED,
                subject: event.entity,
                position: point_of(world, event.entity),
                amount: Some(event.amount),
                other: event.source,
                kinds: DamageKinds::NONE,
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<PickedUp>>() {
            occurrences.extend(picked.read(queue).map(|event| Occurrence {
                event: PICKED_UP,
                subject: event.entity,
                position: event.position,
                amount: None,
                other: Some(event.by),
                kinds: DamageKinds::NONE,
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<ProjectileExpired>>() {
            occurrences.extend(expired.read(queue).map(|event| Occurrence {
                event: EXPIRED,
                subject: event.entity,
                position: event.position,
                amount: None,
                other: None,
                kinds: DamageKinds::NONE,
            }));
        }
        if let Some(mut sources) = world.remove_resource::<CueSources>() {
            for source in &mut sources.0 {
                source.read(world, occurrences);
            }
            world.insert_resource(sources);
        }
    }
}

/// Where `entity` is: its `Position`, else its `Transform.position`, else
/// the origin.
fn point_of(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).map_or_else(
        || {
            world
                .get::<Transform>(entity)
                .map_or(Vec2::ZERO, |transform| transform.position)
        },
        |position| position.0,
    )
}

/// The `cues` round trip: an event left out has no actions, which the
/// description writes as its default `[]`.
fn cues_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut out = round_trip::<Cues>(value)?;
    if let Some(object) = out.as_object_mut() {
        for event in KIT_EVENTS {
            object
                .entry(event)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        }
    }
    Ok(out)
}

/// The descriptions of `cues`, its shapes and the `cue_rules` kind.
fn describe_cues(world: &mut World) {
    let target = |values: &'static [&'static str], doc: &'static str| {
        FieldDescription::new("target", FieldType::Enum(values), doc).default(r#""self""#)
    };
    let mut cues = ComponentDescription::new(
        CUES,
        "The entity's reactions: for each event about it, actions run in order.",
    );
    for event in KIT_EVENTS {
        cues = cues.field(
            FieldDescription::new(
                event,
                FieldType::List(Box::new(FieldType::Shape("cue_action"))),
                "the actions for this kit event; a game's `CuesOnEvent` names more",
            )
            .default("[]"),
        );
    }
    describe(
        world,
        cues.event("ShakeEvent")
            .event("SquashEvent")
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample("{}")
            .roundtrip(cues_round_trip),
    );
    let shapes = [
        ComponentDescription::new("cue_action", "One reaction to an event.")
            .one_of()
            .field(FieldDescription::new(
                "prefab",
                FieldType::Prefab,
                "spawned at the event's position from the subject; the load checks the ID",
            ))
            .field(FieldDescription::new(
                "counter",
                FieldType::Shape("counter_action"),
                "a counter added to or set",
            ))
            .field(FieldDescription::new(
                "flag",
                FieldType::Shape("flag_action"),
                "a flag set or cleared",
            ))
            .field(FieldDescription::new(
                "despawn",
                FieldType::Shape("despawn_action"),
                "the subject or the other despawned with its children, after the step's spawns",
            ))
            .field(FieldDescription::new(
                "sound",
                FieldType::Shape("sound_action"),
                "a manifest sound played at its volume",
            ))
            .field(FieldDescription::new(
                "shake",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "camera trauma added",
            ))
            .field(FieldDescription::new(
                "squash",
                FieldType::Enum(SQUASH_NAMES),
                "the subject's squash armed for this trigger",
            ))
            .field(FieldDescription::new(
                "flash",
                FieldType::Shape("flash_action"),
                "the subject's hit flash started",
            ))
            .field(FieldDescription::new(
                "label",
                FieldType::Shape("label_style"),
                "the event's amount floated at its position; on `damaged_part`, `{}` floats it in its type's `label_style`",
            ))
            .field(FieldDescription::new(
                "roll",
                FieldType::Data(LOOT_TABLES),
                "a weighted table drawn from the kit's random stream, each prefab drawn spawned at the event's position from the subject; the load checks its IDs",
            ))
            .sample(r#"{ "despawn": {} }"#)
            .roundtrip(round_trip::<CueActionDef>),
        ComponentDescription::new("counter_action", "A counter added to or set: exactly one of `add` and `set`.")
            .field(FieldDescription::new(
                "name",
                FieldType::Text,
                "the counter: dotted, undeclared",
            ))
            .field(
                FieldDescription::new(
                    "add",
                    FieldType::I64 {
                        min: None,
                        max: None,
                    },
                    "added, saturating",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "set",
                    FieldType::I64 {
                        min: None,
                        max: None,
                    },
                    "the new count",
                )
                .default("null"),
            )
            .field(target(
                &["self", "other", "game"],
                "the subject's counters, the other entity's or the game's",
            ))
            .sample(r#"{ "name": "kills", "add": 1 }"#)
            .roundtrip(round_trip::<CounterDef>),
        ComponentDescription::new("flag_action", "A flag set or cleared.")
            .field(FieldDescription::new(
                "name",
                FieldType::Text,
                "the flag: dotted, undeclared",
            ))
            .field(
                FieldDescription::new("set", FieldType::Bool, "set when true, cleared when false")
                    .default("true"),
            )
            .field(target(
                &["self", "other", "game"],
                "the subject's flags, the other entity's or the game's",
            ))
            .sample(r#"{ "name": "has_key" }"#)
            .roundtrip(round_trip::<FlagDef>),
        ComponentDescription::new("despawn_action", "An entity despawned with its children.")
            .field(target(&["self", "other"], "the subject or the other entity"))
            .sample("{}")
            .roundtrip(round_trip::<DespawnDef>),
        ComponentDescription::new("sound_action", "A manifest sound played at its volume, never looping.")
            .field(FieldDescription::new(
                "id",
                FieldType::Text,
                "the manifest sound; the load checks the ID",
            ))
            .field(
                FieldDescription::new(
                    "min_interval",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "game seconds before this sound plays again from a cue",
                )
                .default("0.0"),
            )
            .sample(r#"{ "id": "blip" }"#)
            .roundtrip(round_trip::<SoundDef>),
        ComponentDescription::new("flash_action", "The subject's hit flash started, as a hit starts it.")
            .sample("{}")
            .roundtrip(round_trip::<FlashDef>),
        ComponentDescription::new(
            "label_style",
            "The event's amount as a timed world label: it rises, pops from its scale to 1 over its first fifth and fades over its second half.",
        )
        .field(FieldDescription::new(
            "font",
            FieldType::Text,
            "a manifest font face; the load does not check it",
        ))
        .field(
            FieldDescription::new(
                "size",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "UI units tall",
            )
            .default("16.0"),
        )
        .field(
            FieldDescription::new("color", FieldType::Color, "linear, encoded to sRGB when drawn")
                .default("[1.0, 1.0, 1.0, 1.0]"),
        )
        .field(
            FieldDescription::new(
                "seconds",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "game seconds it lasts",
            )
            .default("0.8"),
        )
        .field(
            FieldDescription::new(
                "rise",
                FieldType::F32 {
                    min: Bound::Open,
                    max: Bound::Open,
                },
                "UI units it rises over its seconds",
            )
            .default("32.0"),
        )
        .field(
            FieldDescription::new(
                "pop",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the scale it starts at",
            )
            .default("1.5"),
        )
        .sample(r#"{ "font": "sans" }"#)
        .roundtrip(round_trip::<LabelStyleDef>),
        ComponentDescription::new(
            "cue_rule",
            "A shared reaction: on an event, for each subject whose tags pass the filter, the actions in order.",
        )
        .field(FieldDescription::new(
            "on",
            FieldType::Text,
            "the cue event; the load checks it is registered",
        ))
        .field(
            FieldDescription::new(
                "filter",
                FieldType::Filter,
                "the subjects it runs for; the load checks its tags",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "kind",
                FieldType::Filter,
                "the occurrences it runs for by their damage kinds, each a declared damage type or an ancestor of one; `damaged`, `killed` and `damaged_part` only",
            )
            .default("{}"),
        )
        .field(FieldDescription::new(
            "do",
            FieldType::List(Box::new(FieldType::Shape("cue_action"))),
            "the actions, in order",
        ))
        .sample(r#"{ "on": "killed", "do": [] }"#)
        .roundtrip(round_trip::<CueRuleDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
    describe_kind(
        world,
        CUE_RULES,
        "A list of `cue_rule`: shared reactions to an event, run after the entity's own cues.",
    );
}

/// Seconds a cue sound's interval may end early by and still pass, against
/// the clock's rounding.
const SOUND_SLACK: f64 = 1e-6;

/// The event names a [`Cues`]' lists hold, with their action counts.
impl Inspectable for Cues {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let lists: Vec<String> = self
            .lists
            .iter()
            .map(|(event, actions)| format!("{event}: {}", actions.len()))
            .collect();
        vec![("cues", lists.join(", "))]
    }
}

#[cfg(test)]
#[path = "../tests/cues.rs"]
mod tests;
