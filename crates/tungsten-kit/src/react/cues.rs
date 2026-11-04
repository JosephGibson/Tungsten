//! Cues: the kit's one way to react to events in data. An entity's `cues`
//! and the `cue_rules` data kind map an event to a list of actions, and
//! [`CuesOnEvent`] brings a game's own event in.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use glam::Vec2;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{
    AudioCommands, AudioHandle, CommandBuffer, DataRef, DataRegistry, Entity, EventCursor,
    EventQueue, Inspectable, MissingAsset, Plugin, Schedule, ShakeEvent, SoundRegistry,
    SpawnContext, SquashEvent, SquashTrigger, Time, Transform, Voice, World, system,
};

use crate::damage::effects::{EFFECTS, Effect, EffectApplied, EffectExpired, EffectRequest};
use crate::damage::health::{Damaged, DamagedPart, start_flash};
use crate::damage::kinds::{DamageKinds, DamageTypeTable, check_type, label_style, merge_types};
use crate::damage::pipeline::Healed;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, SHOOTER, TOP_DOWN,
    describe, describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::rng::{KIT_RNG, KitRng};
use crate::foundation::tags::{TagFilter, TagFilterDef, Tags, check_declared, valid_tag};
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::{non_negative, optional_positive};
use crate::intent::ability::{Abilities, AbilityUsed, Used, use_ability};
use crate::lifecycle::cooldown::CooldownElapsed;
use crate::lifecycle::ended::{Despawning, mark_despawning};
use crate::motion::drive::prefab_centre;
use crate::overlap::hit::Struck;
use crate::overlap::pickup::PickedUp;
use crate::overlap::projectile::ProjectileExpired;
use crate::overlap::zone::ZoneEntered;
use crate::present::labels::{
    CritLabel, LABEL_TICK, LabelDef, LabelStyle, LabelStyleDef, LiveLabels, PartLabel, TickLabel,
    float_label, label_tick,
};
use crate::react::counters::{Counters, Flags};
use crate::react::level::LevelUp;
use crate::react::loot::{LOOT_TABLES, WeightedTable, register_loot_tables};
use crate::react::world_cues::{WORLD_CUES, WorldCue, WorldCueActions};
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
pub(crate) const EFFECT_APPLIED: &str = "effect_applied";
pub(crate) const EFFECT_EXPIRED: &str = "effect_expired";
pub(crate) const ABILITY_USED: &str = "ability_used";
pub(crate) const LEVEL_UP: &str = "level_up";
pub(crate) const JUMPED: &str = "jumped";
pub(crate) const AIR_JUMPED: &str = "air_jumped";
pub(crate) const JUMP_REFUSED: &str = "jump_refused";
pub(crate) const LANDED: &str = "landed";
pub(crate) const ABILITY_REFUSED: &str = "ability_refused";
pub(crate) const ABILITY_READY: &str = "ability_ready";
pub(crate) const ABILITY_HELD: &str = "ability_held";
pub(crate) const ABILITY_RELEASED: &str = "ability_released";
/// A [`WorldCue`]'s occurrence: no subject, so no entity or rule names it.
const WORLD_CUE: &str = "world_cue";
/// The cue events about an ability, whose occurrences carry its name.
const ABILITY_EVENTS: [&str; 5] = [
    ABILITY_USED,
    ABILITY_REFUSED,
    ABILITY_READY,
    ABILITY_HELD,
    ABILITY_RELEASED,
];
const KIT_EVENTS: [&str; 21] = [
    DAMAGED_PART,
    DAMAGED,
    KILLED,
    ZONE_ENTERED,
    COOLDOWN_ELAPSED,
    HIT,
    HEALED,
    PICKED_UP,
    EXPIRED,
    EFFECT_APPLIED,
    EFFECT_EXPIRED,
    ABILITY_USED,
    LEVEL_UP,
    JUMPED,
    AIR_JUMPED,
    JUMP_REFUSED,
    LANDED,
    ABILITY_REFUSED,
    ABILITY_READY,
    ABILITY_HELD,
    ABILITY_RELEASED,
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
/// subject, the collector the other), `expired` (a projectile's end),
/// `effect_applied` (the effect's source the other, the stacks the amount),
/// `effect_expired`, `ability_used` (the ability's target the other),
/// `level_up` (the level the amount), a
/// [`PlatformerController`](crate::PlatformerController)'s `jumped` (a
/// ground or coyote jump), `air_jumped` (the stat amount it paid the
/// amount, 0 for a free one), `jump_refused` (a press in the air refused
/// for its cost) and `landed` (the speed it fell at the amount, px/s), each
/// at its feet, an ability's `ability_refused` (a use of an owned ability
/// refused, with its reason), `ability_ready` (the step its counting makes
/// it ready), `ability_held` and `ability_released` (a held trigger's slot
/// going down and up), each about its user, and each name a game's
/// [`CuesOnEvent`] registers.
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
/// counter or a flag, despawn, apply an effect, use or refill an ability,
/// or present (a sound, a shake, a squash, a flash, a label).
/// `counter`, `flag`, `despawn`, `effect` and `ability` act on the event's
/// subject by default; [`for_other`](Self::for_other) points them at the event's
/// other entity and [`for_game`](Self::for_game) the first two at the
/// World's own [`Counters`](crate::Counters) and [`Flags`](crate::Flags).
/// The rest act on the subject or at its position. Presentation actions
/// change no gameplay state.
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
/// | shake | `{ "shake": 0.3 }`, camera trauma added, or `{ "shake": { "trauma": 0.3, "near": 100, "far": 500 } }`, whole within `near` of the [`CueListener`] and falling to none at `far` |
/// | squash | `{ "squash": "on_hit" }`, the subject's squash trigger |
/// | flash | `{ "flash": {} }`, the subject's [`HitFlash`](crate::HitFlash) |
/// | label | `{ "label": { "font": "sans" } }`, the event's amount as a [`LabelStyle`] label; on `damaged_part`, `{ "label": {} }` floats it in its type's style, `merge`, `ticks` and `crit` grouping a target's numbers ([`type_label`](CueAction::type_label)) |
/// | roll | `{ "roll": "slime_drops" }`, a `loot_tables` entry or the [`WeightedTable`] in place: each prefab drawn spawned at the event's position from the subject |
/// | effect | `{ "effect": "burning" }`, or `{ "effect": { "id": "burning", "target": "other" } }`, an `effects` entry or the [`Effect`] in place under `id`, queued on the subject from the event's other entity, or on the other from the subject |
/// | ability | `{ "ability": "wand" }`, or `{ "ability": { "name": "wand", "op": "refill", "target": "other" } }`, the named ability of the subject or the other used now (`op` `use`), or made ready with every charge (`refill`) |
/// | loop | `{ "loop": { "id": "crackle", "volume": 1.0, "fade": 0.1 } }`, the subject wants the sound looping: one voice per sound ID at its loudest want by the [`CueListener`] |
/// | stop | `{ "stop": { "id": "crackle", "fade": 0.1 } }`, the subject's want of the loop ends; the last one's voice fades out |
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
    /// A prefab, its centre on the position when `centred`.
    Prefab {
        id: String,
        centred: bool,
    },
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
    /// Trauma, whole within the falloff's near of the listener and none
    /// from its far.
    Shake {
        trauma: f32,
        falloff: Option<(f32, f32)>,
    },
    Squash(SquashTrigger),
    Flash,
    /// `None`: a `damaged_part`'s amount in its type's style.
    Label(Option<LabelStyle>),
    /// A `damaged_part`'s amount in its type's style, grouped.
    PartLabel(Box<PartLabel>),
    /// A table's draws, each prefab's centre on the position when
    /// `centred`.
    Roll {
        table: DataRef<WeightedTable>,
        centred: bool,
    },
    Effect {
        effect: DataRef<Effect>,
        target: Target,
    },
    Ability {
        name: String,
        refill: bool,
        target: Target,
    },
    /// The subject wants sound `id` looping at `volume`, its voice rising
    /// over `fade` when it starts.
    Loop {
        id: String,
        volume: f32,
        fade: f32,
    },
    /// The subject's want of sound `id` ends, its voice fading over `fade`
    /// when it was the last.
    Stop {
        id: String,
        fade: f32,
    },
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
        Self::of(ActionKind::Prefab {
            id: id.into(),
            centred: false,
        })
    }

    /// The `prefab` or `roll` action, placing each prefab's centre (its
    /// `rigid_body`'s offset or its sprite's drawn centre) on the event's
    /// position rather than its point. Data form
    /// `{ "prefab": { "id": "puff", "centred": true } }` or
    /// `{ "roll": { "table": "slime_drops", "centred": true } }`.
    ///
    /// # Panics
    ///
    /// When the action is neither.
    #[must_use]
    #[track_caller]
    pub fn centred(mut self) -> Self {
        match &mut self.kind {
            ActionKind::Prefab { centred, .. } | ActionKind::Roll { centred, .. } => {
                *centred = true;
            }
            _ => panic!(
                "CueAction::centred: a `{}` action spawns nothing",
                self.name()
            ),
        }
        self
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
    /// When the action is not a `counter`, `flag`, `despawn`, `effect` or
    /// `ability`.
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
            ActionKind::Despawn(target)
            | ActionKind::Effect { target, .. }
            | ActionKind::Ability { target, .. }
                if to == Target::Other =>
            {
                *target = to;
            }
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
        Self::of(ActionKind::Shake {
            trauma,
            falloff: None,
        })
    }

    /// The shake, falling off from the World's [`CueListener`]: whole within
    /// `near` of it, falling linearly to none at `far`; whole without a
    /// listener. Data form
    /// `{ "shake": { "trauma": 0.8, "near": 100, "far": 500 } }`.
    ///
    /// ```
    /// use tungsten_kit::CueAction;
    ///
    /// let blast = CueAction::shake(0.75).with_falloff(100.0, 500.0);
    /// assert_eq!(
    ///     serde_json::to_value(&blast).unwrap(),
    ///     serde_json::json!({ "shake": { "trauma": 0.75, "near": 100.0, "far": 500.0 } })
    /// );
    /// // The number form stays a number.
    /// assert_eq!(serde_json::to_value(CueAction::shake(0.5)).unwrap(), serde_json::json!({ "shake": 0.5 }));
    /// ```
    ///
    /// # Panics
    ///
    /// When the action is not a `shake`, or unless `0 <= near < far`, both
    /// finite.
    #[must_use]
    #[track_caller]
    pub fn with_falloff(mut self, near: f32, far: f32) -> Self {
        assert!(
            near.is_finite() && far.is_finite() && 0.0 <= near && near < far,
            "CueAction::with_falloff: needs finite distances with 0 <= near < far (got {near} and {far})"
        );
        let name = self.name();
        match &mut self.kind {
            ActionKind::Shake { falloff, .. } => *falloff = Some((near, far)),
            _ => panic!("CueAction::with_falloff: a `{name}` action shakes nothing"),
        }
        self
    }

    /// The subject wants manifest sound `id` looping, at its volume: the kit
    /// plays one voice per looping sound ID, rising over 0.1 s as its first
    /// want starts it, at the loudest of its wants, each its volume times
    /// the [`CueListener`]'s gain at its wanter, panned toward that one,
    /// and fading out over its last want's [`stop_loop`](Self::stop_loop)
    /// fade, or over 0.1 s once its wanters are gone or despawning. Wants
    /// are unsaved. Data form `{ "loop": { "id": "crackle" } }`, with
    /// `volume` and `fade`.
    ///
    /// ```
    /// use tungsten_kit::{CueAction, Cues};
    ///
    /// let fire = Cues::new()
    ///     .on("effect_applied", [CueAction::looping("crackle").with_volume(0.5)])
    ///     .on("effect_expired", [CueAction::stop_loop("crackle").with_fade(0.5)]);
    /// let data = serde_json::to_value(&fire).unwrap();
    /// assert_eq!(data["effect_applied"][0]["loop"]["volume"], serde_json::json!(0.5));
    /// let back: Cues = serde_json::from_value(data.clone()).unwrap();
    /// assert_eq!(back, fire);
    /// assert_eq!(data["effect_expired"][0], serde_json::json!({ "stop": { "id": "crackle", "fade": 0.5 } }));
    /// ```
    #[must_use]
    pub fn looping(id: impl Into<String>) -> Self {
        Self::of(ActionKind::Loop {
            id: id.into(),
            volume: 1.0,
            fade: LOOP_FADE,
        })
    }

    /// Ends the subject's want of looping sound `id`; the sound's last want
    /// fades its voice out over 0.1 s.
    #[must_use]
    pub fn stop_loop(id: impl Into<String>) -> Self {
        Self::of(ActionKind::Stop {
            id: id.into(),
            fade: LOOP_FADE,
        })
    }

    /// The loop, at `volume` times its sound's.
    ///
    /// # Panics
    ///
    /// When the action is not a `loop`, or `volume` is not finite and at
    /// least 0.
    #[must_use]
    #[track_caller]
    pub fn with_volume(mut self, volume: f32) -> Self {
        assert!(
            volume.is_finite() && volume >= 0.0,
            "CueAction::with_volume: volume must be finite and >= 0 (got {volume})"
        );
        let name = self.name();
        match &mut self.kind {
            ActionKind::Loop { volume: at, .. } => *at = volume,
            _ => panic!("CueAction::with_volume: a `{name}` action loops nothing"),
        }
        self
    }

    /// The loop's voice rising over `seconds` as it starts, or the stop's
    /// fading over them.
    ///
    /// # Panics
    ///
    /// When the action is not a `loop` or a `stop`, or `seconds` is not
    /// finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_fade(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "CueAction::with_fade: seconds must be finite and >= 0 (got {seconds})"
        );
        let name = self.name();
        match &mut self.kind {
            ActionKind::Loop { fade, .. } | ActionKind::Stop { fade, .. } => *fade = seconds,
            _ => panic!("CueAction::with_fade: a `{name}` action fades nothing"),
        }
        self
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
    /// Three fields group a target's numbers, each optional: `merge`, the
    /// seconds a target's parts of a type sum over, its first plain part
    /// floating at once and the rest within the window as one sum when it
    /// closes; `ticks`, `{ "size", "kind" }`, periodic parts whose kinds pass
    /// `kind` (every periodic part by default) summed on their own over the
    /// window at that size; and `crit`, `{ "size", "color" }`, a step's crit
    /// parts on a target floated at once as one sum in the first part's
    /// type's style at that size and colour. Open sums are presentation,
    /// unsaved, aged on the fixed step; without the fields each part floats
    /// alone.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_kit::{CueAction, Cues};
    ///
    /// let numbers = Cues::new().on("damaged_part", [CueAction::type_label()]);
    /// let data = serde_json::to_value(&numbers).unwrap();
    /// assert_eq!(data["damaged_part"][0], serde_json::json!({ "label": {} }));
    ///
    /// let grouped = serde_json::json!({ "label": {
    ///     "merge": 0.25,
    ///     "ticks": { "size": 10.0, "kind": { "any": ["elemental"] } },
    ///     "crit": { "size": 24.0, "color": [1.0, 0.75, 0.0, 1.0] }
    /// } });
    /// let action: CueAction = serde_json::from_value(grouped.clone()).unwrap();
    /// assert_eq!((action.name(), serde_json::to_value(&action).unwrap()), ("label", grouped));
    /// assert!(serde_json::from_value::<CueAction>(serde_json::json!({
    ///     "label": { "font": "sans", "merge": 0.25 }
    /// }))
    /// .is_err(), "a named style groups nothing");
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
        Self::of(ActionKind::Roll {
            table,
            centred: false,
        })
    }

    /// Queues `effect`, an `effects` entry or an effect in place, on the
    /// subject, from the event's other entity, for the next
    /// [`EFFECT_RESOLVE`](crate::EFFECT_RESOLVE);
    /// [`for_other`](Self::for_other) queues it on the other entity from
    /// the subject. Data form `{ "effect": "burning" }` or
    /// `{ "effect": { "id": "burning", "target": "other" } }`; the load
    /// checks the effect.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_core::DataRef;
    /// use tungsten_kit::{CueAction, Effect};
    ///
    /// let slow = CueAction::effect(DataRef::by_id("slowed")).for_other();
    /// assert_eq!(
    ///     serde_json::to_value(&slow).unwrap(),
    ///     serde_json::json!({ "effect": { "id": "slowed", "target": "other" } })
    /// );
    /// let short: CueAction = serde_json::from_str(r#"{ "effect": "slowed" }"#).unwrap();
    /// assert_eq!(short, CueAction::effect(DataRef::by_id("slowed")));
    /// let inline = CueAction::effect(DataRef::inline(Effect::new().with_duration(1.0)));
    /// assert_eq!(inline.name(), "effect");
    /// ```
    #[must_use]
    pub fn effect(effect: DataRef<Effect>) -> Self {
        Self::of(ActionKind::Effect {
            effect,
            target: Target::Subject,
        })
    }

    /// Uses the subject's ability `name` now, as [`use_ability`] uses it;
    /// [`for_other`](Self::for_other) uses the other entity's. Data form
    /// `{ "ability": "wand" }` or
    /// `{ "ability": { "name": "wand", "op": "use", "target": "other" } }`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_kit::CueAction;
    ///
    /// let swing = CueAction::ability("sword");
    /// assert_eq!(
    ///     serde_json::to_value(&swing).unwrap(),
    ///     serde_json::json!({ "ability": { "name": "sword", "op": "use", "target": "self" } })
    /// );
    /// let short: CueAction = serde_json::from_str(r#"{ "ability": "sword" }"#).unwrap();
    /// assert_eq!(short, swing);
    /// let reload = CueAction::refill_ability("volley").for_other();
    /// assert_eq!(reload.name(), "ability");
    /// ```
    #[must_use]
    pub fn ability(name: impl Into<String>) -> Self {
        Self::of(ActionKind::Ability {
            name: name.into(),
            refill: false,
            target: Target::Subject,
        })
    }

    /// Makes the subject's ability `name` ready now, every charge back and
    /// its cooldown cleared ([`Abilities::refill`]); an ammo pickup's
    /// `picked_up` cue refills its collector's with
    /// [`for_other`](Self::for_other). Data form
    /// `{ "ability": { "name": "volley", "op": "refill" } }`.
    #[must_use]
    pub fn refill_ability(name: impl Into<String>) -> Self {
        Self::of(ActionKind::Ability {
            name: name.into(),
            refill: true,
            target: Target::Subject,
        })
    }

    /// The action's data name: `prefab`, `counter`, `flag`, `despawn`,
    /// `sound`, `shake`, `squash`, `flash`, `label`, `roll`, `effect`,
    /// `ability`, `loop` or `stop`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self.kind {
            ActionKind::Prefab { .. } => "prefab",
            ActionKind::Counter { .. } => "counter",
            ActionKind::Flag { .. } => "flag",
            ActionKind::Despawn(_) => "despawn",
            ActionKind::Sound { .. } => "sound",
            ActionKind::Shake { .. } => "shake",
            ActionKind::Squash(_) => "squash",
            ActionKind::Flash => "flash",
            ActionKind::Label(_) | ActionKind::PartLabel(_) => "label",
            ActionKind::Roll { .. } => "roll",
            ActionKind::Effect { .. } => "effect",
            ActionKind::Ability { .. } => "ability",
            ActionKind::Loop { .. } => "loop",
            ActionKind::Stop { .. } => "stop",
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
    Prefab(PrefabActionDef),
    Counter(CounterDef),
    Flag(FlagDef),
    Despawn(DespawnDef),
    Sound(SoundDef),
    Shake(ShakeDef),
    Squash(SquashName),
    Flash(FlashDef),
    Label(LabelDef),
    Roll(RollActionDef),
    Effect(EffectActionDef),
    Ability(AbilityActionDef),
    Loop(LoopDef),
    Stop(StopDef),
}

/// A `shake` action as data writes it: the trauma alone, or an object of
/// `trauma` and the falloff's `near` and `far`; written back as the number
/// without a falloff.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ShakeDef {
    Trauma(#[serde(deserialize_with = "non_negative")] f32),
    Falloff(ShakeObject),
}

/// The object form of a `shake` action: `near` 0 by default; `far`, when
/// set, above `near`.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ShakeFields")]
pub(crate) struct ShakeObject {
    trauma: f32,
    near: f32,
    far: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShakeFields {
    #[serde(deserialize_with = "non_negative")]
    trauma: f32,
    #[serde(default, deserialize_with = "non_negative")]
    near: f32,
    #[serde(default, deserialize_with = "optional_positive")]
    far: Option<f32>,
}

impl TryFrom<ShakeFields> for ShakeObject {
    type Error = String;

    fn try_from(fields: ShakeFields) -> Result<Self, Self::Error> {
        if let Some(far) = fields.far
            && fields.near >= far
        {
            return Err(format!("near {} must be below far {far}", fields.near));
        }
        Ok(Self {
            trauma: fields.trauma,
            near: fields.near,
            far: fields.far,
        })
    }
}

/// A `loop` action as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoopDef {
    id: String,
    #[serde(default = "full", deserialize_with = "non_negative")]
    volume: f32,
    #[serde(default = "loop_fade", deserialize_with = "non_negative")]
    fade: f32,
}

/// A `stop` action as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StopDef {
    id: String,
    #[serde(default = "loop_fade", deserialize_with = "non_negative")]
    fade: f32,
}

fn full() -> f32 {
    1.0
}

fn loop_fade() -> f32 {
    LOOP_FADE
}

/// Seconds a loop's voice rises over as it starts, and falls over as it
/// stops, by default.
pub(crate) const LOOP_FADE: f32 = 0.1;

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

/// Whom a `despawn` or an `effect` action acts on: the subject or the
/// other entity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DespawnTarget {
    #[default]
    #[serde(rename = "self")]
    Subject,
    Other,
}

impl From<DespawnTarget> for Target {
    fn from(target: DespawnTarget) -> Self {
        match target {
            DespawnTarget::Subject => Target::Subject,
            DespawnTarget::Other => Target::Other,
        }
    }
}

impl From<Target> for DespawnTarget {
    /// `Game` reads as the subject; no action of this target takes it.
    fn from(target: Target) -> Self {
        match target {
            Target::Other => DespawnTarget::Other,
            Target::Subject | Target::Game => DespawnTarget::Subject,
        }
    }
}

/// An `effect` action as data writes it: the effect's ID alone, or an
/// object of `id` (the ID or the effect in place) and `target`; written
/// back as the object.
#[derive(Clone, Serialize)]
#[serde(into = "EffectActionObject")]
pub(crate) struct EffectActionDef {
    id: DataRef<Effect>,
    target: DespawnTarget,
}

/// The object form of an `effect` action.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectActionObject {
    id: DataRef<Effect>,
    #[serde(default)]
    target: DespawnTarget,
}

impl From<EffectActionDef> for EffectActionObject {
    fn from(def: EffectActionDef) -> Self {
        Self {
            id: def.id,
            target: def.target,
        }
    }
}

/// An `ability` action's operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AbilityOp {
    #[default]
    Use,
    Refill,
}

/// An `ability` action as data writes it: the ability's name alone, or an
/// object of `name`, `op` and `target`; written back as the object.
#[derive(Clone, Serialize)]
#[serde(into = "AbilityActionObject")]
pub(crate) struct AbilityActionDef {
    name: String,
    op: AbilityOp,
    target: DespawnTarget,
}

/// The object form of an `ability` action.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AbilityActionObject {
    name: String,
    #[serde(default)]
    op: AbilityOp,
    #[serde(default)]
    target: DespawnTarget,
}

impl From<AbilityActionDef> for AbilityActionObject {
    fn from(def: AbilityActionDef) -> Self {
        Self {
            name: def.name,
            op: def.op,
            target: def.target,
        }
    }
}

impl<'de> Deserialize<'de> for AbilityActionDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AbilityActionVisitor;

        impl<'de> Visitor<'de> for AbilityActionVisitor {
            type Value = AbilityActionDef;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an ability's name, or an object of `name`, `op` and `target`")
            }

            fn visit_str<E: de::Error>(self, name: &str) -> Result<Self::Value, E> {
                Ok(AbilityActionDef {
                    name: name.to_owned(),
                    op: AbilityOp::Use,
                    target: DespawnTarget::Subject,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let object =
                    AbilityActionObject::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(AbilityActionDef {
                    name: object.name,
                    op: object.op,
                    target: object.target,
                })
            }
        }

        deserializer.deserialize_any(AbilityActionVisitor)
    }
}

impl<'de> Deserialize<'de> for EffectActionDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EffectActionVisitor;

        impl<'de> Visitor<'de> for EffectActionVisitor {
            type Value = EffectActionDef;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an effect's ID, or an object of `id` and `target`")
            }

            fn visit_str<E: de::Error>(self, id: &str) -> Result<Self::Value, E> {
                Ok(EffectActionDef {
                    id: DataRef::by_id(id),
                    target: DespawnTarget::Subject,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let object =
                    EffectActionObject::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(EffectActionDef {
                    id: object.id,
                    target: object.target,
                })
            }
        }

        deserializer.deserialize_any(EffectActionVisitor)
    }
}

/// A `prefab` action as data writes it: the prefab's ID alone, or an
/// object of `id` and `centred`; written back as the ID unless centred.
#[derive(Clone)]
pub(crate) struct PrefabActionDef {
    id: String,
    centred: bool,
}

/// The object form of a `prefab` action.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefabActionObject {
    id: String,
    #[serde(default)]
    centred: bool,
}

impl Serialize for PrefabActionDef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.centred {
            PrefabActionObject {
                id: self.id.clone(),
                centred: true,
            }
            .serialize(serializer)
        } else {
            self.id.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for PrefabActionDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(id) = value.as_str() {
            return Ok(Self {
                id: id.to_owned(),
                centred: false,
            });
        }
        let object = PrefabActionObject::deserialize(value).map_err(de::Error::custom)?;
        Ok(Self {
            id: object.id,
            centred: object.centred,
        })
    }
}

/// A `roll` action as data writes it: the table's ID or the table in
/// place, or an object of `table` and `centred`; written back as the table
/// unless centred.
#[derive(Clone)]
pub(crate) struct RollActionDef {
    table: DataRef<WeightedTable>,
    centred: bool,
}

/// The object form of a `roll` action.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollActionObject {
    table: DataRef<WeightedTable>,
    #[serde(default)]
    centred: bool,
}

impl Serialize for RollActionDef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.centred {
            RollActionObject {
                table: self.table.clone(),
                centred: true,
            }
            .serialize(serializer)
        } else {
            self.table.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for RollActionDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("table").is_some() {
            let object = RollActionObject::deserialize(value).map_err(de::Error::custom)?;
            return Ok(Self {
                table: object.table,
                centred: object.centred,
            });
        }
        let table = DataRef::<WeightedTable>::deserialize(value).map_err(de::Error::custom)?;
        Ok(Self {
            table,
            centred: false,
        })
    }
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
            CueActionDef::Prefab(def) => ActionKind::Prefab {
                id: def.id,
                centred: def.centred,
            },
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
            CueActionDef::Despawn(def) => ActionKind::Despawn(def.target.into()),
            CueActionDef::Sound(def) => ActionKind::Sound {
                id: def.id,
                min_interval: def.min_interval,
            },
            CueActionDef::Shake(ShakeDef::Trauma(trauma)) => ActionKind::Shake {
                trauma,
                falloff: None,
            },
            CueActionDef::Shake(ShakeDef::Falloff(shake)) => ActionKind::Shake {
                trauma: shake.trauma,
                falloff: shake.far.map(|far| (shake.near, far)),
            },
            CueActionDef::Squash(name) => ActionKind::Squash(name.into()),
            CueActionDef::Flash(FlashDef {}) => ActionKind::Flash,
            CueActionDef::Label(LabelDef(style, None)) => ActionKind::Label(style),
            CueActionDef::Label(LabelDef(_, Some(part))) => ActionKind::PartLabel(part),
            CueActionDef::Roll(def) => ActionKind::Roll {
                table: def.table,
                centred: def.centred,
            },
            CueActionDef::Effect(def) => ActionKind::Effect {
                effect: def.id,
                target: def.target.into(),
            },
            CueActionDef::Ability(def) => ActionKind::Ability {
                name: def.name,
                refill: def.op == AbilityOp::Refill,
                target: def.target.into(),
            },
            CueActionDef::Loop(def) => ActionKind::Loop {
                id: def.id,
                volume: def.volume,
                fade: def.fade,
            },
            CueActionDef::Stop(def) => ActionKind::Stop {
                id: def.id,
                fade: def.fade,
            },
        };
        Ok(Self { kind })
    }
}

impl From<CueAction> for CueActionDef {
    fn from(action: CueAction) -> Self {
        match action.kind {
            ActionKind::Prefab { id, centred } => {
                CueActionDef::Prefab(PrefabActionDef { id, centred })
            }
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
                target: target.into(),
            }),
            ActionKind::Sound { id, min_interval } => {
                CueActionDef::Sound(SoundDef { id, min_interval })
            }
            ActionKind::Shake {
                trauma,
                falloff: None,
            } => CueActionDef::Shake(ShakeDef::Trauma(trauma)),
            ActionKind::Shake {
                trauma,
                falloff: Some((near, far)),
            } => CueActionDef::Shake(ShakeDef::Falloff(ShakeObject {
                trauma,
                near,
                far: Some(far),
            })),
            ActionKind::Squash(trigger) => CueActionDef::Squash(trigger.into()),
            ActionKind::Flash => CueActionDef::Flash(FlashDef {}),
            ActionKind::Label(style) => CueActionDef::Label(LabelDef(style, None)),
            ActionKind::PartLabel(part) => CueActionDef::Label(LabelDef(None, Some(part))),
            ActionKind::Roll { table, centred } => {
                CueActionDef::Roll(RollActionDef { table, centred })
            }
            ActionKind::Effect { effect, target } => CueActionDef::Effect(EffectActionDef {
                id: effect,
                target: target.into(),
            }),
            ActionKind::Ability {
                name,
                refill,
                target,
            } => CueActionDef::Ability(AbilityActionDef {
                name,
                op: if refill {
                    AbilityOp::Refill
                } else {
                    AbilityOp::Use
                },
                target: target.into(),
            }),
            ActionKind::Loop { id, volume, fade } => {
                CueActionDef::Loop(LoopDef { id, volume, fade })
            }
            ActionKind::Stop { id, fade } => CueActionDef::Stop(StopDef { id, fade }),
        }
    }
}

/// One occurrence of a game's event, as its [`CuesOnEvent`] map gives it:
/// the subject, where it happened, and the amount and the other entity the
/// actions may read. Build one with [`CueEvent::new`], or with
/// [`CueEvent::of`] at the subject's point.
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
/// // A contact's speed, at the struck body's point as the cue system reads it.
/// let impact = CueEvent::of(coin).with_amount(420.0);
/// assert!(!impact.position.is_finite());
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct CueEvent {
    /// The entity the event is about, whose cues run.
    pub entity: Entity,
    /// Where it happened: a `prefab` spawns there, a `label` floats there;
    /// not finite, the subject's point when the cue system reads it.
    pub position: Vec2,
    /// What a `label` shows.
    pub amount: Option<f32>,
    /// Whom a `target: other` action acts on.
    pub other: Option<Entity>,
    /// The cue event it is, one of its [`CuesOnEvent`]'s names; `None`
    /// for its only one.
    pub name: Option<&'static str>,
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
            name: None,
        }
    }

    /// An occurrence about `entity` at its point as the cue system reads it
    /// when it runs: its `Position`, else its `Transform`'s, else the
    /// origin. The position it holds is not finite, which the cue system
    /// reads so from any occurrence.
    #[must_use]
    pub fn of(entity: Entity) -> Self {
        Self::new(entity, Vec2::NAN)
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

    /// The occurrence, as the cue event `name`: one of the names its
    /// [`CuesOnEvent::with_names`] brings in, whose cues it runs.
    #[must_use]
    pub fn named(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }
}

/// Where cue sounds are heard from, an opt-in World resource. With one in
/// the World, a `sound` action plays at its volume times
/// [`gain`](Self::gain) at the occurrence's position, and skips a cue past
/// the listener's hearing without marking its `min_interval`, so a nearer
/// one may still play; without one, every cue sound plays at its volume.
/// The game moves it, to its player or its camera, each step before
/// [`CUES`], or has it [follow](Self::following) an entity. With a
/// [pan width](Self::with_pan), a cue sound plays panned toward its side;
/// a `loop` plays by it, and a `shake` with a falloff falls off from it.
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::CueListener;
///
/// let mut listener = CueListener::new(100.0, 300.0);
/// assert_eq!(listener.gain(Vec2::new(50.0, 0.0)), 1.0);
/// assert_eq!(listener.gain(Vec2::new(200.0, 0.0)), 0.25);
/// assert_eq!(listener.gain(Vec2::new(300.0, 0.0)), 0.0);
/// listener.set_position(Vec2::new(300.0, 0.0));
/// assert_eq!(listener.gain(Vec2::new(300.0, 0.0)), 1.0);
/// let panned = listener.with_pan(200.0);
/// assert_eq!(panned.pan(Vec2::new(200.0, 0.0)), -0.5);
/// assert_eq!(panned.pan(Vec2::new(900.0, 0.0)), 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CueListener {
    position: Vec2,
    full: f32,
    silent: f32,
    /// The entity the cue system moves it to, each step.
    follows: Option<Entity>,
    /// The distance along x at which a sound plays wholly to one side.
    pan: Option<f32>,
}

impl CueListener {
    /// A listener at the origin that hears a cue at its full volume within
    /// `full` of it and nothing from `silent`.
    ///
    /// # Panics
    ///
    /// Unless both are finite and `0 <= full < silent`.
    #[must_use]
    pub fn new(full: f32, silent: f32) -> Self {
        assert!(
            full.is_finite() && silent.is_finite() && 0.0 <= full && full < silent,
            "a cue listener needs finite distances with 0 <= full < silent, got {full} and {silent}"
        );
        Self {
            position: Vec2::ZERO,
            full,
            silent,
            follows: None,
            pan: None,
        }
    }

    /// The listener, following `entity`: at each run's start, before any
    /// occurrence, the cue system moves it to the entity's point, its
    /// `Position`, else its `Transform`'s; an entity gone, or without
    /// either, leaves it where it was.
    #[must_use]
    pub fn following(mut self, entity: Entity) -> Self {
        self.follows = Some(entity);
        self
    }

    /// The listener, panning: a cue sound `dx` along x from it plays at pan
    /// `dx / width`, clamped to −1 (left) to 1 (right), through
    /// `AudioCommands::play_panned`; without it, centred, as before.
    ///
    /// # Panics
    ///
    /// Unless `width` is finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_pan(mut self, width: f32) -> Self {
        assert!(
            width.is_finite() && width > 0.0,
            "CueListener::with_pan: width must be finite and > 0 (got {width})"
        );
        self.pan = Some(width);
        self
    }

    /// The entity it follows, if any.
    #[must_use]
    pub fn follows(&self) -> Option<Entity> {
        self.follows
    }

    /// Follows `entity`, or stops following with `None`.
    pub fn set_following(&mut self, entity: Option<Entity>) {
        self.follows = entity;
    }

    /// Its pan width, if it pans.
    #[must_use]
    pub fn pan_width(&self) -> Option<f32> {
        self.pan
    }

    /// The pan a sound at `at` plays at: `dx / width` clamped to −1 to 1;
    /// 0 without a pan width or at a point that is not finite.
    #[must_use]
    pub fn pan(&self, at: Vec2) -> f32 {
        let Some(width) = self.pan else {
            return 0.0;
        };
        let pan = (at.x - self.position.x) / width;
        if pan.is_finite() {
            pan.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    /// The listener, at `position`.
    #[must_use]
    pub fn at(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    /// Where the listener hears from.
    #[must_use]
    pub fn position(&self) -> Vec2 {
        self.position
    }

    /// Moves the listener to `position`.
    pub fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }

    /// The distance within which a cue plays at its full volume.
    #[must_use]
    pub fn full(&self) -> f32 {
        self.full
    }

    /// The distance from which a cue is not heard.
    #[must_use]
    pub fn silent(&self) -> f32 {
        self.silent
    }

    /// The share of its volume a cue at `at` plays at: 1 within
    /// [`full`](Self::full) of the listener, 0 from [`silent`](Self::silent)
    /// or at a point that is not finite, and `(1 − t)²` between, `t` the
    /// distance's share of the way from `full` to `silent`.
    #[must_use]
    pub fn gain(&self, at: Vec2) -> f32 {
        let distance = self.position.distance(at);
        if distance <= self.full {
            return 1.0;
        }
        if distance.is_nan() || distance >= self.silent {
            return 0.0;
        }
        let t = (distance - self.full) / (self.silent - self.full);
        (1.0 - t) * (1.0 - t)
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
    /// Its flags, packed: [`PERIODIC`], [`CRIT`], [`FRESH`], [`ENDED`] and
    /// [`ABSORBED`], and an `ability_refused` occurrence's reason in the top
    /// three bits ([`reason_of`]).
    pub(crate) bits: u8,
    /// An effect event's entry, or an ability event's ability: an index
    /// into the step's effect IDs, or [`NO_EFFECT`].
    pub(crate) effect: u16,
}

/// A damage occurrence's bit: a periodic request's.
pub(crate) const PERIODIC: u8 = 1;
/// A damage occurrence's bit: a crit's.
pub(crate) const CRIT: u8 = 1 << 1;
/// An `effect_applied` occurrence's bit: a new instance or an instant
/// effect's landing.
pub(crate) const FRESH: u8 = 1 << 2;
/// An `effect_expired` occurrence's bit: ended before its time.
pub(crate) const ENDED: u8 = 1 << 3;
/// A `damaged` occurrence's bit: the shield took the hit whole.
pub(crate) const ABSORBED: u8 = 1 << 4;
/// An occurrence's effect when it names no entry.
pub(crate) const NO_EFFECT: u16 = u16::MAX;
/// Where an `ability_refused` occurrence's reason starts in its bits.
const REASON_SHIFT: u8 = 5;

/// A refused use's reason code, which an `ability_refused` occurrence
/// carries in its top three bits and a rule's `reason` reads: 1 to 4 for
/// [`Used::NotReady`], [`Used::NoCharge`], [`Used::NoCost`] and
/// [`Used::NoTarget`]; 0 for a use, or for no owned ability.
pub(crate) fn reason_of(used: Used) -> u8 {
    match used {
        Used::NotReady => 1,
        Used::NoCharge => 2,
        Used::NoCost => 3,
        Used::NoTarget => 4,
        _ => 0,
    }
}

impl Occurrence {
    /// An occurrence of `event` about `subject` at `position`, with nothing
    /// else.
    fn new(event: &'static str, subject: Entity, position: Vec2) -> Self {
        Self {
            event,
            subject,
            position,
            amount: None,
            other: None,
            kinds: DamageKinds::NONE,
            bits: 0,
            effect: NO_EFFECT,
        }
    }

    /// Whether it carries `bit`.
    pub(crate) fn has(&self, bit: u8) -> bool {
        self.bits & bit != 0
    }

    /// An `ability_refused` occurrence's reason code; 0 for any other.
    fn reason(&self) -> u8 {
        self.bits >> REASON_SHIFT
    }
}

/// A damage occurrence's bits.
fn damage_bits(periodic: bool, crit: bool) -> u8 {
    (u8::from(periodic) * PERIODIC) | (u8::from(crit) * CRIT)
}

/// `id`'s index among the step's first `count` effect IDs, reusing their
/// strings, added when new; [`NO_EFFECT`] for none.
fn effect_index(ids: &mut Vec<String>, count: &mut usize, id: Option<&str>) -> u16 {
    let Some(id) = id else {
        return NO_EFFECT;
    };
    if let Some(at) = ids[..*count].iter().position(|known| known == id) {
        return at as u16;
    }
    if *count >= usize::from(NO_EFFECT) {
        return NO_EFFECT;
    }
    if *count < ids.len() {
        ids[*count].clear();
        ids[*count].push_str(id);
    } else {
        ids.push(id.to_owned());
    }
    *count += 1;
    (*count - 1) as u16
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
                (EFFECT_APPLIED, true, true, false),
                (EFFECT_EXPIRED, false, false, false),
                (ABILITY_USED, false, true, false),
                (LEVEL_UP, true, false, false),
                (JUMPED, false, false, false),
                (AIR_JUMPED, true, false, false),
                (JUMP_REFUSED, false, false, false),
                (LANDED, true, false, false),
                (ABILITY_REFUSED, false, false, false),
                (ABILITY_READY, false, false, false),
                (ABILITY_HELD, false, false, false),
                (ABILITY_RELEASED, false, false, false),
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

/// One of a platformer controller's cue events, as its system pushes it:
/// the event, the controller's entity, its feet and the amount.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ControllerCue {
    pub(crate) event: &'static str,
    pub(crate) entity: Entity,
    pub(crate) at: Vec2,
    pub(crate) amount: Option<f32>,
}

/// The step's controller cue events, a World resource [`CuesPlugin`]
/// inserts with its capacity reserved, which the cue system drains each
/// step: no event queue, so a controller's jumps send no event.
#[derive(Debug)]
pub(crate) struct ControllerCues(pub(crate) Vec<ControllerCue>);

/// The controller cue events a step holds before the list grows.
const CONTROLLER_CUES: usize = 32;

/// One of an ability's cue events, as its system pushes it: the event, the
/// user, its point, the entry's index in its [`Abilities`], which names the
/// ability when the cue system reads it, and a refusal's [reason](reason_of).
#[derive(Debug, Clone, Copy)]
pub(crate) struct AbilityCue {
    pub(crate) event: &'static str,
    pub(crate) entity: Entity,
    pub(crate) at: Vec2,
    pub(crate) entry: u16,
    pub(crate) reason: u8,
}

/// The step's ability cue events, a World resource [`CuesPlugin`] inserts
/// with its capacity reserved, which the cue system drains each step: no
/// event queue, so an ability's refusals, readiness and holds send no
/// event. Its pushers ask [`cue_named`] first.
#[derive(Debug)]
pub(crate) struct AbilityCues(pub(crate) Vec<AbilityCue>);

/// The ability cue events a step holds before the list grows.
const ABILITY_CUES: usize = 32;

/// Pushes `cue` for the cue system's next read, when the World has the
/// list; its caller asked [`cue_named`] first.
pub(crate) fn push_ability_cue(world: &mut World, cue: AbilityCue) {
    if let Some(cues) = world.get_resource_mut::<AbilityCues>() {
        cues.0.push(cue);
    }
}

/// Whether the cue event `event` about `entity` has anything to run: the
/// entity's own [`Cues`] list an action for it, or a loaded `cue_rules`
/// rule is on it, whatever its filters, which the cue system reads later as
/// for any occurrence. A platformer controller asks before it pushes one of
/// its events, so a World that names none does no cue work for its
/// controllers. It allocates nothing.
pub(crate) fn cue_named(world: &World, entity: Entity, event: &str) -> bool {
    world
        .get::<Cues>(entity)
        .is_some_and(|cues| !cues.actions(event).is_empty())
        || world.get_resource::<DataRegistry>().is_some_and(|data| {
            data.ids(CUE_RULES).any(|id| {
                data.get::<CueRules>(id)
                    .is_some_and(|rules| rules.0.iter().any(|rule| rule.on == event))
            })
        })
}

/// Whether a loaded `cue_rules` rule on `ability_used` filters on an
/// ability's name, so the cue system names its occurrences.
fn filters_ability(world: &World) -> bool {
    world.get_resource::<DataRegistry>().is_some_and(|data| {
        data.ids(CUE_RULES).any(|id| {
            data.get::<CueRules>(id).is_some_and(|rules| {
                rules
                    .0
                    .iter()
                    .any(|rule| rule.on == ABILITY_USED && rule.ability.is_some())
            })
        })
    })
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
/// name, so `cues` and `cue_rules` may use it.
/// [`with_names`](Self::with_names) brings `E` in under several names,
/// each occurrence running the cues of the name it carries
/// ([`CueEvent::named`]); one that names none of them, or no name, is
/// skipped and logged once per name. Under [`new`](Self::new), an
/// occurrence's own name, when set, must be `name`. A game event may carry an
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
/// At build, when a name is not `[a-z][a-z0-9_]*`, is one of the kit's
/// events or was registered before, or `with_names` names none.
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
    names: Vec<&'static str>,
    /// Registered by `new`: an occurrence without a name is its one.
    single: bool,
    map: fn(&E) -> Option<CueEvent>,
}

impl<E: 'static> CuesOnEvent<E> {
    /// Event `E` as the cue event `name`, each occurrence given by `map`.
    #[must_use]
    pub fn new(name: &'static str, map: fn(&E) -> Option<CueEvent>) -> Self {
        Self {
            names: vec![name],
            single: true,
            map,
        }
    }

    /// Event `E` as each of the cue events `names`, each occurrence given
    /// by `map` under the name it carries.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_core::{Entity, Plugin, Schedule, World};
    /// use tungsten_kit::{CueEvent, CuesOnEvent, CuesPlugin};
    ///
    /// /// The game's chest event: opened, or emptied by its last take.
    /// struct Chest {
    ///     chest: Entity,
    ///     emptied: bool,
    /// }
    ///
    /// let mut world = World::new();
    /// let mut schedule = Schedule::new();
    /// CuesPlugin.build(&mut schedule, &mut world);
    /// CuesOnEvent::<Chest>::with_names(&["opened", "emptied"], |e| {
    ///     Some(CueEvent::of(e.chest).named(if e.emptied { "emptied" } else { "opened" }))
    /// })
    /// .build(&mut schedule, &mut world);
    /// assert!(world.has_event::<Chest>());
    /// ```
    #[must_use]
    pub fn with_names(names: &[&'static str], map: fn(&E) -> Option<CueEvent>) -> Self {
        Self {
            names: names.to_vec(),
            single: false,
            map,
        }
    }
}

impl<E> Clone for CuesOnEvent<E> {
    fn clone(&self) -> Self {
        Self {
            names: self.names.clone(),
            single: self.single,
            map: self.map,
        }
    }
}

impl<E> fmt::Debug for CuesOnEvent<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = f.debug_struct("CuesOnEvent");
        if self.single {
            out.field("name", &self.names[0]);
        } else {
            out.field("names", &self.names);
        }
        out.finish_non_exhaustive()
    }
}

/// A game event's reader: its names, its map, its place in the queue and
/// the names it warned of.
struct GameSource<E> {
    names: Vec<&'static str>,
    single: bool,
    map: fn(&E) -> Option<CueEvent>,
    cursor: EventCursor<E>,
    /// The names an occurrence carried that are not the source's, logged
    /// once each; "" for none.
    warned: HashSet<&'static str>,
}

impl<E: 'static> CueSource for GameSource<E> {
    fn read(&mut self, world: &World, out: &mut Vec<Occurrence>) {
        let Some(queue) = world.get_resource::<EventQueue<E>>() else {
            return;
        };
        let Self {
            names,
            single,
            map,
            cursor,
            warned,
        } = self;
        for event in cursor.read(queue).filter_map(*map) {
            let name = match event.name {
                None if *single => Some(names[0]),
                Some(name) => names.iter().copied().find(|&known| known == name),
                None => None,
            };
            let Some(name) = name else {
                let carried = event.name.unwrap_or("");
                if warned.insert(carried) {
                    log::warn!(
                        "CuesOnEvent {names:?}: an occurrence named `{carried}`, none of its names; skipped"
                    );
                }
                continue;
            };
            let position = if event.position.is_finite() {
                event.position
            } else {
                point_of(world, event.entity)
            };
            out.push(Occurrence {
                amount: event.amount,
                other: event.other,
                ..Occurrence::new(name, event.entity, position)
            });
        }
    }
}

impl<E: 'static> Plugin for CuesOnEvent<E> {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        assert!(!self.names.is_empty(), "CuesOnEvent::with_names: no names");
        if !world.has_resource::<CueEventNames>() {
            world.insert_resource(CueEventNames::default());
        }
        for &name in &self.names {
            assert!(
                valid_event(name),
                "CuesOnEvent: `{name}` is not an event name (`[a-z][a-z0-9_]*`)"
            );
            assert!(
                !KIT_EVENTS.contains(&name),
                "CuesOnEvent: `{name}` is one of the kit's cue events"
            );
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
        }
        world.register_event::<E>();
        if !world.has_resource::<CueSources>() {
            world.insert_resource(CueSources::default());
        }
        world
            .resource_mut::<CueSources>()
            .0
            .push(Box::new(GameSource {
                names: self.names.clone(),
                single: self.single,
                map: self.map,
                cursor: EventCursor::new(),
                warned: HashSet::new(),
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
    #[serde(default)]
    periodic: Option<bool>,
    #[serde(default)]
    amount: Option<AmountRange>,
    #[serde(default)]
    flags: FlagFilterDef,
    #[serde(default)]
    effect: Option<String>,
    #[serde(default)]
    fresh: Option<bool>,
    #[serde(default)]
    ended: Option<bool>,
    #[serde(default)]
    absorbed: Option<bool>,
    #[serde(default)]
    ability: Option<String>,
    #[serde(default)]
    reason: Option<ReasonDef>,
    #[serde(default, deserialize_with = "non_negative")]
    min_interval: f32,
    #[serde(default, deserialize_with = "positive_cap")]
    cap: Option<u32>,
    #[serde(rename = "do")]
    actions: Vec<CueAction>,
}

/// A rule's `reason`, as data writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReasonDef {
    NotReady,
    NoCharge,
    NoCost,
    NoTarget,
}

impl ReasonDef {
    /// Its code, as [`reason_of`] gives it.
    fn code(self) -> u8 {
        reason_of(match self {
            ReasonDef::NotReady => Used::NotReady,
            ReasonDef::NoCharge => Used::NoCharge,
            ReasonDef::NoCost => Used::NoCost,
            ReasonDef::NoTarget => Used::NoTarget,
        })
    }
}

/// A rule's `cap`: at least 1 when set.
fn positive_cap<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    match Option::<u32>::deserialize(deserializer)? {
        Some(0) => Err(de::Error::custom("a cap of at least 1")),
        cap => Ok(cap),
    }
}

/// A rule's `amount`: the inclusive range an occurrence's amount must fall
/// in, each end optional.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AmountFields")]
pub(crate) struct AmountRange {
    min: Option<f32>,
    max: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AmountFields {
    #[serde(default)]
    min: Option<f32>,
    #[serde(default)]
    max: Option<f32>,
}

impl TryFrom<AmountFields> for AmountRange {
    type Error = String;

    fn try_from(fields: AmountFields) -> Result<Self, Self::Error> {
        for (field, value) in [("min", fields.min), ("max", fields.max)] {
            if value.is_some_and(|value| !value.is_finite()) {
                return Err(format!("{field}: not a finite number"));
            }
        }
        if let (Some(min), Some(max)) = (fields.min, fields.max)
            && min > max
        {
            return Err(format!("min {min} is above max {max}"));
        }
        Ok(Self {
            min: fields.min,
            max: fields.max,
        })
    }
}

impl AmountRange {
    /// Whether `amount` passes: an amount within the range; none fails.
    fn passes(self, amount: Option<f32>) -> bool {
        amount.is_some_and(|amount| {
            self.min.is_none_or(|min| amount >= min) && self.max.is_none_or(|max| amount <= max)
        })
    }
}

/// A rule's filter over the World's own [`Flags`] as data writes it: flag
/// names in `all`, `any` and `none`, each checked as a tag name and not
/// declared, as flags are not.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "KindFields")]
pub(crate) struct FlagFilterDef {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    all: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    any: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    none: Vec<String>,
}

impl TryFrom<KindFields> for FlagFilterDef {
    type Error = String;

    fn try_from(fields: KindFields) -> Result<Self, Self::Error> {
        for (field, names) in [
            ("all", &fields.all),
            ("any", &fields.any),
            ("none", &fields.none),
        ] {
            if let Some(name) = names.iter().find(|name| !valid_tag(name)) {
                return Err(format!(
                    "{field}: `{name}` is not a flag name (`[a-z][a-z0-9_]*` segments joined by dots)"
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

impl FlagFilterDef {
    /// Whether it names no flag, so every occurrence passes.
    fn is_everything(&self) -> bool {
        self.all.is_empty() && self.any.is_empty() && self.none.is_empty()
    }

    /// Whether the game's `flags` pass, read as a tag filter reads tags:
    /// every `all`, one `any` when it names any, no `none`; no flags set
    /// without them.
    fn matches(&self, flags: Option<&Flags>) -> bool {
        let has = |name: &String| flags.is_some_and(|flags| flags.has(name));
        self.all.iter().all(has)
            && (self.any.is_empty() || self.any.iter().any(has))
            && !self.none.iter().any(has)
    }
}

/// A rule's filter over an occurrence's damage kinds as data writes it:
/// declared damage types or their ancestors in `all`, `any` and `none`,
/// each checked as a tag name and kept as written, so a load's error names
/// the field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "KindFields")]
pub(crate) struct KindFilterDef {
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
    pub(crate) fn names(&self) -> impl Iterator<Item = (String, &str)> {
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
    pub(crate) fn is_everything(&self) -> bool {
        self.all.is_empty() && self.any.is_empty() && self.none.is_empty()
    }

    /// Whether an occurrence of `kinds` passes, each name's kinds from
    /// `mask`, read as a tag filter reads tags: every `all`, one `any` when
    /// it names any, no `none`.
    pub(crate) fn matches(
        &self,
        kinds: DamageKinds,
        mut mask: impl FnMut(&str) -> DamageKinds,
    ) -> bool {
        let mut has = |name: &String| kinds.intersects(mask(name));
        self.all.iter().all(&mut has)
            && (self.any.is_empty() || self.any.iter().any(&mut has))
            && !self.none.iter().any(&mut has)
    }
}

/// One shared rule: on event `on`, for a subject whose tags pass `filter`
/// and an occurrence whose damage kinds pass `kind` and the rest of its
/// filters, the actions in order, held by its limits.
struct CueRule {
    on: String,
    filter: TagFilter,
    /// The filter's names with the fields that name them, for the load's
    /// check.
    names: Vec<(String, String)>,
    kind: KindFilterDef,
    periodic: Option<bool>,
    amount: Option<AmountRange>,
    flags: FlagFilterDef,
    effect: Option<String>,
    fresh: Option<bool>,
    ended: Option<bool>,
    absorbed: Option<bool>,
    /// An ability's name, on the ability events.
    ability: Option<String>,
    /// A refusal's reason code, on `ability_refused`.
    reason: Option<u8>,
    /// Game seconds per subject between runs; 0 for none.
    min_interval: f32,
    /// Runs a step at most.
    cap: Option<u32>,
    actions: Arc<[CueAction]>,
}

impl CueRule {
    /// Whether `occurrence` passes the filters past the event, tag and kind
    /// filters, the game's flags from `flags` and the step's effect IDs
    /// from `effect_ids`.
    fn passes(
        &self,
        occurrence: &Occurrence,
        flags: Option<&Flags>,
        effect_ids: &[String],
    ) -> bool {
        let bit = |want: Option<bool>, bit: u8| want.is_none_or(|want| occurrence.has(bit) == want);
        bit(self.periodic, PERIODIC)
            && bit(self.fresh, FRESH)
            && bit(self.ended, ENDED)
            && bit(self.absorbed, ABSORBED)
            && self
                .amount
                .is_none_or(|range| range.passes(occurrence.amount))
            && self.effect.as_ref().is_none_or(|id| {
                effect_ids
                    .get(usize::from(occurrence.effect))
                    .is_some_and(|known| known == id)
            })
            && self.ability.as_ref().is_none_or(|name| {
                effect_ids
                    .get(usize::from(occurrence.effect))
                    .is_some_and(|known| known == name)
            })
            && self.reason.is_none_or(|code| occurrence.reason() == code)
            && (self.flags.is_everything() || self.flags.matches(flags))
    }
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
            if rule.ability.as_deref() == Some("") {
                return Err(format!("{i}.ability: an ability's name, not empty"));
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
                periodic: rule.periodic,
                amount: rule.amount,
                flags: rule.flags,
                effect: rule.effect,
                fresh: rule.fresh,
                ended: rule.ended,
                absorbed: rule.absorbed,
                ability: rule.ability,
                reason: rule.reason.map(ReasonDef::code),
                min_interval: rule.min_interval,
                cap: rule.cap,
                actions: rule.actions.into(),
            });
        }
        Ok(Self(out))
    }
}

/// The `cue_rules` kind's check: each rule's event, its tag filter's
/// tags, its `kind` and `periodic` filters on an event with damage kinds,
/// each `kind` name a declared damage type or an ancestor of one, its
/// `amount` on an event with one, its `effect` on an effect event and an
/// `effects` entry, `fresh` on `effect_applied`, `ended` on
/// `effect_expired`, and its actions.
fn check_rule_list(rules: &CueRules, world: &World) -> Result<(), MissingAsset> {
    for (i, rule) in rules.0.iter().enumerate() {
        let event = event_info(world, &format!("{i}.on"), &rule.on)?;
        for (field, name) in &rule.names {
            check_declared(world, &format!("{i}.filter.{field}"), name)?;
        }
        let refuse = |field: &str, needs: &'static str| {
            Err(MissingAsset::new(format!("{i}.{field}"), needs, event.name))
        };
        if !rule.kind.is_everything() && !event.kinds {
            return refuse("kind", "cue event with damage kinds");
        }
        if rule.periodic.is_some() && !event.kinds {
            return refuse("periodic", "cue event with damage kinds");
        }
        if rule.amount.is_some() && !event.amount {
            return refuse("amount", "cue event with an amount");
        }
        if let Some(id) = &rule.effect {
            if event.name != EFFECT_APPLIED && event.name != EFFECT_EXPIRED {
                return refuse("effect", "cue event with an effect");
            }
            DataRef::<Effect>::by_id(id.as_str()).check(world, &format!("{i}.effect"))?;
        }
        if rule.fresh.is_some() && event.name != EFFECT_APPLIED {
            return refuse("fresh", "cue event with a fresh instance");
        }
        if rule.ended.is_some() && event.name != EFFECT_EXPIRED {
            return refuse("ended", "cue event with an ended instance");
        }
        if rule.absorbed.is_some() && event.name != DAMAGED {
            return refuse("absorbed", "cue event with a shield's take");
        }
        if rule.ability.is_some() && !ABILITY_EVENTS.contains(&event.name) {
            return refuse("ability", "cue event about an ability");
        }
        if rule.reason.is_some() && event.name != ABILITY_REFUSED {
            return refuse("reason", "cue event with a refusal's reason");
        }
        for (field, name) in rule.kind.names() {
            check_type(world, &format!("{i}.kind.{field}"), name, true)?;
        }
        check_actions(world, &format!("{i}.do"), event, &rule.actions)?;
    }
    Ok(())
}

/// The `world_cues` kind's check: each action one a cue with no subject
/// runs, `prefab`, `roll`, `sound`, `shake`, a `label` with a font, and a
/// `counter` or `flag` with `target: game`, and its IDs.
pub(crate) fn check_world_cue(cue: &WorldCueActions, world: &World) -> Result<(), MissingAsset> {
    for (i, action) in cue.0.iter().enumerate() {
        let at = format!("{i}.{}", action.name());
        let refused = match &action.kind {
            ActionKind::Prefab { .. }
            | ActionKind::Roll { .. }
            | ActionKind::Sound { .. }
            | ActionKind::Shake { .. }
            | ActionKind::Label(Some(_))
            | ActionKind::Counter {
                target: Target::Game,
                ..
            }
            | ActionKind::Flag {
                target: Target::Game,
                ..
            } => None,
            ActionKind::Counter { .. } | ActionKind::Flag { .. } => Some(format!("{at}.target")),
            _ => Some(at),
        };
        if let Some(field) = refused {
            return Err(MissingAsset::new(
                field,
                "world cue action (prefab, roll, sound, shake, a styled label, a game counter or flag)",
                action.name(),
            ));
        }
    }
    let event = EventInfo {
        name: WORLD_CUE,
        amount: true,
        other: false,
        kinds: false,
    };
    check_actions(world, "", event, &cue.0).map_err(|mut missing| {
        missing.field = missing.field.trim_start_matches('.').to_owned();
        missing
    })
}

/// Checks `actions`, run for `event`, against the World: each prefab,
/// sound, loot table and effect ID and an inline table's prefabs or
/// effect's fields, a `label` on an event with an amount and a style-less
/// one on `damaged_part` only, and a `target: other` on an event with
/// another entity; `field` names the list.
fn check_actions(
    world: &World,
    field: &str,
    event: EventInfo,
    actions: &[CueAction],
) -> Result<(), MissingAsset> {
    for (i, action) in actions.iter().enumerate() {
        let at = format!("{field}.{i}.{}", action.name());
        match &action.kind {
            ActionKind::Prefab { id, .. } => world.check_prefab_id(&at, id)?,
            ActionKind::Roll { table, .. } => table.check(world, &at)?,
            ActionKind::Effect { effect, target } => {
                effect.check(world, &at)?;
                if *target == Target::Other && !event.other {
                    return Err(MissingAsset::new(
                        format!("{at}.target"),
                        "cue event with an other entity",
                        event.name,
                    ));
                }
            }
            ActionKind::Sound { id, .. }
            | ActionKind::Loop { id, .. }
            | ActionKind::Stop { id, .. } => {
                if let Some(sounds) = world.get_resource::<SoundRegistry>()
                    && sounds.get_by_id(id).is_none()
                {
                    return Err(MissingAsset::new(format!("{at}.id"), "sound", id.as_str()));
                }
            }
            ActionKind::Label(_) | ActionKind::PartLabel(_) if !event.amount => {
                return Err(MissingAsset::new(
                    at,
                    "cue event with an amount",
                    event.name,
                ));
            }
            ActionKind::Label(None) | ActionKind::PartLabel(_) if event.name != DAMAGED_PART => {
                return Err(MissingAsset::new(
                    at,
                    "cue event with a damage type's label style",
                    event.name,
                ));
            }
            ActionKind::PartLabel(part) => {
                if let Some(ticks) = &part.ticks {
                    for (field, name) in ticks.kind.names() {
                        check_type(world, &format!("{at}.ticks.kind.{field}"), name, true)?;
                    }
                }
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
            | ActionKind::Ability {
                target: Target::Other,
                ..
            } if !event.other => {
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

/// Registers the `cues` scene component, the `cue_rules`, `world_cues` and
/// `loot_tables` data kinds, the kit's cue events, a [`KitRng`](crate::KitRng) when the
/// World has none, [`CUES`] in the react phase, last of the kit's items,
/// and [`LABEL_TICK`] in the present phase.
///
/// Each fixed step, the cue system reads its sources, each once through an
/// `EventCursor`: [`DamagedPart`] (`damaged_part`), [`Damaged`] (a
/// `damaged` occurrence, then a `killed` one for a kill), [`ZoneEntered`],
/// [`CooldownElapsed`],
/// [`Struck`](crate::Struck) (`hit`), [`Healed`](crate::Healed),
/// [`PickedUp`](crate::PickedUp) (`picked_up`),
/// [`ProjectileExpired`](crate::ProjectileExpired) (`expired`),
/// [`EffectApplied`](crate::EffectApplied) (`effect_applied`),
/// [`EffectExpired`](crate::EffectExpired) (`effect_expired`),
/// [`AbilityUsed`](crate::AbilityUsed) (`ability_used`),
/// [`LevelUp`](crate::LevelUp) (`level_up`), then the
/// [`PlatformerController`](crate::PlatformerController)s' `jumped`,
/// `air_jumped`, `jump_refused` and `landed` from a list the controller's
/// system fills, no event queue, then the abilities' `ability_refused`,
/// `ability_ready`, `ability_held` and `ability_released` from a list the
/// abilities fill, each pushed only when named, then each [`CuesOnEvent`] in build order,
/// each queue in send order, and last the step's [`WorldCue`]s. It runs
/// every
/// occurrence's entity cues, then every occurrence's matching rules (rules
/// in entry-ID order, then file order); each list in its written order, so
/// in each pass every `damaged_part` runs before the step's `damaged` and
/// `killed`. A rule matches on its event, its tag filter over the subject's
/// [`Tags`](crate::Tags) and its `kind` filter over the occurrence's
/// [`DamageKinds`](crate::DamageKinds). `counter` and `flag` apply at once, `prefab` and
/// `roll` spawns queue as they are met, a `roll` drawing from the
/// [`KitRng`](crate::KitRng) in this order, an `effect` queues for the
/// next step's effect resolve, an `ability` uses or refills at once, and
/// the step's despawns queue after its spawns, each target marked with its descendants so a frame's
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
/// a `kind` without `all` and `any`.
///
/// A rule's other filters, each optional, run after those: `periodic` on
/// the damage events, `true` passing a periodic request's tick and `false`
/// a blow; `amount`, `{ "min", "max" }`, inclusive, over the occurrence's
/// amount on an event with one, an occurrence without one failing it;
/// `flags`, `{ "all", "any", "none" }` over the World's own
/// [`Flags`](crate::Flags), read as a tag filter reads tags; `effect`, an
/// `effects` entry ID, on `effect_applied` and `effect_expired`; `fresh` on
/// `effect_applied`, `true` passing a new instance or an instant effect's
/// landing and `false` a stack or a refresh; and `ended` on
/// `effect_expired`, `false` passing an instance that ran out and `true`
/// one ended early; `absorbed` on `damaged`, `true` passing a hit the
/// [`Shield`](crate::Shield) took whole and `false` one that hurt; `ability`,
/// an ability's name, on `ability_used` and the four ability events; and
/// `reason` on `ability_refused`, one of `not_ready`, `no_charge`, `no_cost`
/// and `no_target`. Its limits come last, so an occurrence a filter turns
/// away spends neither: `min_interval`, the game seconds a rule waits for a
/// subject once it ran for it, and `cap`, its runs a step at most. A held
/// rule is traced as skipped; the limits are the cue system's, unsaved. The
/// load checks every event name, tag, damage type, effect, prefab, sound
/// and loot table ID it names, and refuses a filter on an event that
/// cannot carry it.
///
/// The vocabulary stops there: no conditions, variables or sequences, and
/// no filter but these; a `roll` draws only from the table it names. An
/// entity's own `cues` take none of them. A reaction that needs more is a
/// game's Rust.
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
        if !world.has_resource::<ControllerCues>() {
            world.insert_resource(ControllerCues(Vec::with_capacity(CONTROLLER_CUES)));
        }
        if !world.has_resource::<AbilityCues>() {
            world.insert_resource(AbilityCues(Vec::with_capacity(ABILITY_CUES)));
        }
        if !world.has_resource::<KitRng>() {
            world.insert_resource(KitRng::default());
        }
        world.register_saved_resource::<KitRng>(KIT_RNG);
        register_loot_tables(world);
        world.register_data_kind_with_assets(CUE_RULES, check_rule_list);
        world.register_data_kind_with_assets(WORLD_CUES, check_world_cue);
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

/// Who wants a looping sound: an entity's cue loop, or a status look's
/// instance on its holder by serial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wanter {
    Cue(Entity),
    Look(Entity, u64),
}

/// One want of a looping sound: whose place it is heard from, and its
/// volume as a share of the sound's.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LoopWant {
    pub(crate) place: Entity,
    pub(crate) volume: f32,
}

/// A looping sound's voice and its wants.
#[derive(Debug)]
struct LoopSound {
    id: String,
    voice: Option<Voice>,
    wants: Vec<(Wanter, LoopWant)>,
    /// The volume and pan last sent to its voice.
    volume: f32,
    pan: f32,
    /// Seconds its voice rises over as it starts, and fades over as it
    /// stops.
    fade_in: f32,
    fade_out: f32,
}

/// The kit's looping sounds, a World resource the first want inserts: per
/// sound ID its wants by wanter, apart from its one voice. Unsaved: a load
/// into a new run finds the old wanters gone, and their voices fade.
#[derive(Debug, Default)]
pub(crate) struct CueLoops {
    sounds: Vec<LoopSound>,
}

impl CueLoops {
    /// `wanter` wants sound `id` looping at `volume`, heard from `place`;
    /// a want it held is replaced. A sound's first want starts its voice
    /// over `fade`.
    pub(crate) fn want(&mut self, id: &str, wanter: Wanter, want: LoopWant, fade: f32) {
        let at = self
            .sounds
            .iter()
            .position(|sound| sound.id == id)
            .unwrap_or_else(|| {
                self.sounds.push(LoopSound {
                    id: id.to_owned(),
                    voice: None,
                    wants: Vec::new(),
                    volume: 0.0,
                    pan: 0.0,
                    fade_in: fade,
                    fade_out: LOOP_FADE,
                });
                self.sounds.len() - 1
            });
        let wants = &mut self.sounds[at].wants;
        match wants.iter_mut().find(|(known, _)| *known == wanter) {
            Some((_, held)) => *held = want,
            None => wants.push((wanter, want)),
        }
    }

    /// `wanter`'s want of sound `id` ends; the sound's last fades its voice
    /// out over `fade`.
    pub(crate) fn let_go(&mut self, id: &str, wanter: Wanter, fade: f32) {
        if let Some(sound) = self.sounds.iter_mut().find(|sound| sound.id == id) {
            let before = sound.wants.len();
            sound.wants.retain(|(known, _)| *known != wanter);
            if sound.wants.len() < before {
                sound.fade_out = fade;
            }
        }
    }

    /// Whether it holds no sound.
    fn is_idle(&self) -> bool {
        self.sounds.is_empty()
    }
}

/// What the loops' pass decided for one sound.
#[derive(Debug, Clone, Copy)]
struct LoopPlan {
    handle: Option<AudioHandle>,
    /// Its voice's volume: the sound's times the loudest want's, its gain
    /// at its place counted; and the pan toward that want.
    volume: f32,
    pan: f32,
    /// No want held on.
    silent: bool,
}

/// A loop's change of volume or pan that sends a command.
const LOOP_STEP: f32 = 1.0 / 64.0;

/// Seconds a loop's voice moves to a new volume or pan over.
const LOOP_RAMP: f32 = 0.05;

/// Why an action or an occurrence did not run, for the trace.
const GONE: &str = "the subject is gone";
const DESPAWNING: &str = "the subject is despawning";
const NO_OTHER: &str = "the event has no other entity, or it is gone";
const NO_AMOUNT: &str = "the event has no amount";
const NO_COMMANDS: &str = "the World has no CommandBuffer";
const NO_QUEUE: &str = "the World has no queue for it";
const NO_SOUND: &str = "the World has no sound or audio for it";
const TOO_SOON: &str = "the sound played within its min_interval";
const UNHEARD: &str = "the cue's position is past the listener's hearing";
const FAR: &str = "the cue's position is past the shake's far";
const NO_TABLE: &str = "the World has no such loot table";
const NO_WORLD_CUE: &str = "the World has no such world cue";
const NO_STYLE: &str = "the part's types set no label style";
const NO_EFFECTS: &str = "the World has no EffectsPlugin queue";
const NO_ABILITY: &str = "the entity owns no such ability";
const NOT_USED: &str = "the ability was not ready, had no charge, no cost or no target";
const WAITING: &str = "a rule ran for the subject within its min_interval";
const CAPPED: &str = "a rule ran its cap this step";

/// Game seconds between sweeps of the rules' intervals.
const SWEEP_EVERY: f64 = 1.0;

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
    applied: EventCursor<EffectApplied>,
    effects_expired: EventCursor<EffectExpired>,
    abilities_used: EventCursor<AbilityUsed>,
    level_ups: EventCursor<LevelUp>,
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
    /// The part labels' open sums, by label, target, kinds and track.
    sums: HashMap<(usize, Entity, DamageKinds, bool), OpenSum>,
    /// The step's crit sums, floated at its end in the order they opened.
    crits: Vec<OpenSum>,
    /// How many sums have opened: their close order.
    opened: u64,
    /// The step's effect IDs, which its occurrences index; the first
    /// `effect_count` are the step's, the rest kept for their strings.
    effect_ids: Vec<String>,
    effect_count: usize,
    /// Each rule with an interval, by name: when it may run again for each
    /// subject, on `clock`.
    waits: HashMap<String, HashMap<Entity, f64>>,
    /// When `waits` was last swept, on `clock`.
    swept: f64,
    /// The step's runs of each capped rule, by name.
    runs: HashMap<String, u32>,
    /// Why each rule its limits held for one occurrence did not run.
    held: Vec<&'static str>,
    /// The loops' pass's plans, one a sound, kept so a step allocates
    /// nothing once warm.
    plans: Vec<LoopPlan>,
    /// Its place in the world cues' queue, and the unknown names it warned
    /// of.
    world_cues: EventCursor<WorldCue>,
    unknown_cues: HashSet<String>,
}

/// A part label's sum while it is open: its label and target, the sum and
/// how many parts made it, where it floats, the seconds left, its style and
/// when it opened.
struct OpenSum {
    label: usize,
    subject: Entity,
    sum: f32,
    parts: u32,
    position: Vec2,
    left: f32,
    style: LabelStyle,
    order: u64,
}

/// Seconds a part label's window may end early by and still close, against
/// the steps' rounding.
const SUM_SLACK: f32 = 1e-4;

impl Reactor {
    fn run(&mut self, world: &mut World) {
        follow_listener(world);
        self.react(world);
        self.play_world_cues(world);
        self.loops(world);
    }

    /// Plays the step's [`WorldCue`]s, after every other source, in send
    /// order: each named entry's actions at its point with no subject.
    fn play_world_cues(&mut self, world: &mut World) {
        let Some(queue) = world.get_resource::<EventQueue<WorldCue>>() else {
            return;
        };
        let cues: Vec<WorldCue> = self.world_cues.read(queue).cloned().collect();
        for cue in cues {
            let actions = world
                .get_resource::<DataRegistry>()
                .and_then(|data| data.get::<WorldCueActions>(&cue.name))
                .map(|actions| Arc::clone(&actions.0));
            let Some(actions) = actions else {
                if self.unknown_cues.insert(cue.name.clone()) {
                    log::warn!(
                        "world cue `{}`: no `world_cues` entry; it plays nothing",
                        cue.name
                    );
                }
                trace(world, Entity::PLACEHOLDER, || TraceKind::Skipped {
                    event: WORLD_CUE,
                    action: "all",
                    why: NO_WORLD_CUE,
                });
                continue;
            };
            let occurrence = Occurrence {
                amount: cue.amount,
                ..Occurrence::new(WORLD_CUE, Entity::PLACEHOLDER, cue.at)
            };
            for action in actions.iter() {
                self.apply(world, &occurrence, action, Some(&cue.name));
            }
        }
    }

    /// Reads the step's occurrences and runs their cues and rules.
    fn react(&mut self, world: &mut World) {
        let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
        self.clock += f64::from(dt);
        self.close_sums(world, dt);
        self.sweep(world);
        self.runs.clear();
        self.occurrences.clear();
        self.effect_count = 0;
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
            for why in self.held.drain(..) {
                trace(world, occurrence.subject, || TraceKind::Skipped {
                    event: occurrence.event,
                    action: "all",
                    why,
                });
            }
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
        for open in self.crits.drain(..) {
            float_label(world, &open.style, open.position, open.sum);
        }
        for entity in self.despawns.drain(..) {
            if let Some(commands) = world.get_resource_mut::<CommandBuffer>() {
                despawn_recursive(commands, entity);
            }
            trace(world, entity, || TraceKind::Despawn { by: CUES });
        }
    }

    /// The rules for `occurrence`'s event whose tag filter its subject
    /// passes, whose kind filter its kinds pass and whose other filters it
    /// passes, in entry-ID order, then file order; then each rule's limits,
    /// a held rule's reason in `held`.
    fn match_rules(&mut self, world: &World, occurrence: &Occurrence) {
        let Some(data) = world.get_resource::<DataRegistry>() else {
            return;
        };
        let table = world.get_resource::<DamageTypeTable>();
        let tags = world.get::<Tags>(occurrence.subject);
        let flags = world.get_resource::<Flags>();
        let Self {
            matched,
            masks,
            effect_ids,
            effect_count,
            waits,
            runs,
            held,
            clock,
            ..
        } = self;
        let effect_ids = &effect_ids[..*effect_count];
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
                if !(rule.on == occurrence.event
                    && rule.filter.matches(tags)
                    && (rule.kind.is_everything()
                        || rule.kind.matches(occurrence.kinds, &mut mask))
                    && rule.passes(occurrence, flags, effect_ids))
                {
                    continue;
                }
                let name = format!("{id}.{i}");
                let interval = f64::from(rule.min_interval);
                if interval > 0.0
                    && waits
                        .get(&name)
                        .and_then(|subjects| subjects.get(&occurrence.subject))
                        .is_some_and(|&until| *clock + SOUND_SLACK < until)
                {
                    held.push(WAITING);
                    continue;
                }
                if let Some(cap) = rule.cap {
                    let ran = runs.entry(name.clone()).or_insert(0);
                    if *ran >= cap {
                        held.push(CAPPED);
                        continue;
                    }
                    *ran += 1;
                }
                if interval > 0.0 {
                    waits
                        .entry(name.clone())
                        .or_default()
                        .insert(occurrence.subject, *clock + interval);
                }
                matched.push((name, Arc::clone(&rule.actions)));
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
            ActionKind::Prefab { id, centred } => {
                if !world.has_resource::<CommandBuffer>() {
                    return skip(world, NO_COMMANDS);
                }
                let at = placed(world, id, occurrence.position, *centred);
                if !self.spawn(world, subject, id, at) {
                    return;
                }
            }
            ActionKind::Roll { table, centred } => {
                if !world.has_resource::<CommandBuffer>() {
                    return skip(world, NO_COMMANDS);
                }
                if !self.draw(world, table) {
                    return skip(world, NO_TABLE);
                }
                let drawn = std::mem::take(&mut self.drawn);
                for id in &drawn {
                    let at = placed(world, id, occurrence.position, *centred);
                    self.spawn(world, subject, id, at);
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
                let listener = world.get_resource::<CueListener>();
                let gain = listener.map_or(1.0, |listener| listener.gain(occurrence.position));
                let pan = listener
                    .filter(|listener| listener.pan.is_some())
                    .map(|listener| listener.pan(occurrence.position));
                if gain <= 0.0 {
                    return skip(world, UNHEARD);
                }
                let Some(commands) = world.get_resource_mut::<AudioCommands>() else {
                    return skip(world, NO_SOUND);
                };
                match pan {
                    Some(pan) => commands.play_panned(handle, volume * gain, false, pan),
                    None => commands.play_with(handle, volume * gain, false),
                };
                self.played.insert(id.clone(), self.clock);
            }
            ActionKind::Shake { trauma, falloff } => {
                let share = match (falloff, world.get_resource::<CueListener>()) {
                    (Some((near, far)), Some(listener)) => {
                        falloff_share(listener.position.distance(occurrence.position), *near, *far)
                    }
                    _ => 1.0,
                };
                if share <= 0.0 {
                    return skip(world, FAR);
                }
                let Some(queue) = world.get_resource_mut::<EventQueue<ShakeEvent>>() else {
                    return skip(world, NO_QUEUE);
                };
                queue.send(ShakeEvent {
                    trauma_add: trauma * share,
                });
            }
            ActionKind::Loop { id, volume, fade } => {
                if !world.has_resource::<CueLoops>() {
                    world.insert_resource(CueLoops::default());
                }
                let want = LoopWant {
                    place: subject,
                    volume: *volume,
                };
                world
                    .resource_mut::<CueLoops>()
                    .want(id, Wanter::Cue(subject), want, *fade);
            }
            ActionKind::Stop { id, fade } => {
                if let Some(loops) = world.get_resource_mut::<CueLoops>() {
                    loops.let_go(id, Wanter::Cue(subject), *fade);
                }
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
            ActionKind::PartLabel(label) => {
                let Some(amount) = occurrence.amount else {
                    return skip(world, NO_AMOUNT);
                };
                let Some(style) = label_style(world, occurrence.kinds) else {
                    return skip(world, NO_STYLE);
                };
                self.part_label(world, occurrence, label, style, amount);
            }
            ActionKind::Effect { effect, target: to } => {
                let Some(entity) = target(*to, world) else {
                    return skip(world, NO_OTHER);
                };
                let source = if *to == Target::Other {
                    Some(subject)
                } else {
                    target(Target::Other, world)
                };
                let mut request = EffectRequest::new(entity, effect.clone());
                if let Some(source) = source {
                    request = request.with_source(source);
                }
                if !request.queue(world) {
                    return skip(world, NO_EFFECTS);
                }
            }
            ActionKind::Ability {
                name,
                refill,
                target: to,
            } => {
                let Some(entity) = target(*to, world) else {
                    return skip(world, NO_OTHER);
                };
                if *refill {
                    let refilled = world
                        .get_mut::<Abilities>(entity)
                        .is_some_and(|abilities| abilities.refill(name));
                    if !refilled {
                        return skip(world, NO_ABILITY);
                    }
                } else {
                    match use_ability(world, entity, name) {
                        Used::Used => {}
                        Used::NoAbility => return skip(world, NO_ABILITY),
                        _ => return skip(world, NOT_USED),
                    }
                }
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
                let commands = world.resource_mut::<CommandBuffer>();
                // A world cue's spawns have no source.
                if subject == Entity::PLACEHOLDER {
                    commands.spawn_prefab(prepared);
                } else {
                    commands.spawn_prefab_from(subject, prepared);
                }
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

    /// Keeps each looping sound's one voice: started with its first want,
    /// at the loudest of its wants, each its volume times the listener's
    /// gain at its place, panned toward that one, a command past a change
    /// of 1/64 and always one to or from 0; a want whose place is gone or
    /// despawning lets go, and a sound with no want left fades its voice
    /// out. With no loop it returns at once.
    fn loops(&mut self, world: &mut World) {
        if world
            .get_resource::<CueLoops>()
            .is_none_or(CueLoops::is_idle)
            || !world.has_resource::<AudioCommands>()
        {
            return;
        }
        let mut loops = std::mem::take(world.resource_mut::<CueLoops>());
        // The wants whose place is gone or despawning let go, as over a
        // stop of the default fade.
        for sound in &mut loops.sounds {
            let before = sound.wants.len();
            sound.wants.retain(|(_, want)| {
                world.is_alive(want.place) && !world.has::<Despawning>(want.place)
            });
            if sound.wants.len() < before && sound.wants.is_empty() {
                sound.fade_out = LOOP_FADE;
            }
        }
        let listener = world.get_resource::<CueListener>();
        let sounds = world.get_resource::<SoundRegistry>();
        self.plans.clear();
        for sound in &loops.sounds {
            let handle = sounds.and_then(|sounds| sounds.get_by_id(&sound.id));
            let base = match (sounds, handle) {
                (Some(sounds), Some(handle)) => sounds.get_volume(handle),
                _ => 0.0,
            };
            let mut loudest = (0.0_f32, 0.0_f32);
            for (_, want) in &sound.wants {
                let at = point_of(world, want.place);
                let level = want.volume * listener.map_or(1.0, |listener| listener.gain(at));
                if level > loudest.0 || loudest == (0.0, 0.0) {
                    loudest = (level, listener.map_or(0.0, |listener| listener.pan(at)));
                }
            }
            self.plans.push(LoopPlan {
                handle,
                volume: base * loudest.0,
                pan: loudest.1,
                silent: sound.wants.is_empty(),
            });
        }
        let commands = world.resource_mut::<AudioCommands>();
        for (sound, plan) in loops.sounds.iter_mut().zip(&self.plans) {
            match (sound.voice, plan.silent) {
                (Some(voice), true) => {
                    commands.stop_voice(voice, sound.fade_out);
                    sound.voice = None;
                }
                (None, true) => {}
                (None, false) => {
                    let Some(handle) = plan.handle else {
                        continue;
                    };
                    let voice = commands.play_faded(handle, plan.volume, true, sound.fade_in);
                    if plan.pan != 0.0 {
                        commands.set_voice_pan(voice, plan.pan, 0.0);
                    }
                    sound.voice = Some(voice);
                    sound.volume = plan.volume;
                    sound.pan = plan.pan;
                }
                (Some(voice), false) => {
                    if moved(sound.volume, plan.volume) {
                        commands.set_voice_volume(voice, plan.volume, LOOP_RAMP);
                        sound.volume = plan.volume;
                    }
                    if moved(sound.pan, plan.pan) {
                        commands.set_voice_pan(voice, plan.pan, LOOP_RAMP);
                        sound.pan = plan.pan;
                    }
                }
            }
        }
        loops
            .sounds
            .retain(|sound| sound.voice.is_some() || !sound.wants.is_empty());
        *world.resource_mut::<CueLoops>() = loops;
    }

    /// Drops the rules' waits that have passed or whose subject is gone, at
    /// most once a game second.
    fn sweep(&mut self, world: &World) {
        if self.waits.is_empty() || self.clock - self.swept < SWEEP_EVERY {
            return;
        }
        self.swept = self.clock;
        let clock = self.clock;
        self.waits.retain(|_, subjects| {
            subjects.retain(|&subject, &mut until| {
                until > clock + SOUND_SLACK && world.is_alive(subject)
            });
            !subjects.is_empty()
        });
    }

    /// Ages the part labels' open sums by `dt` and floats each that closes
    /// with a part in it, in the order they opened.
    fn close_sums(&mut self, world: &mut World, dt: f32) {
        if self.sums.is_empty() {
            return;
        }
        let mut closed: Vec<OpenSum> = self
            .sums
            .extract_if(|_, open| {
                open.left -= dt;
                open.left <= SUM_SLACK
            })
            .map(|(_, open)| open)
            .collect();
        closed.sort_by_key(|open| open.order);
        for open in closed.iter().filter(|open| open.parts > 0) {
            float_label(world, &open.style, open.position, open.sum);
        }
    }

    /// A part label's number for `occurrence`'s part of `amount` in its
    /// type's `style`: a crit into its target's crit sum for the step; a
    /// tick into its own sum at the tick size; a plain part at once when it
    /// opens a window, else into the window's sum; each alone without one.
    fn part_label(
        &mut self,
        world: &mut World,
        occurrence: &Occurrence,
        label: &PartLabel,
        style: LabelStyle,
        amount: f32,
    ) {
        let id = std::ptr::from_ref(label) as usize;
        let subject = occurrence.subject;
        if occurrence.has(CRIT)
            && let Some(crit) = label.crit
        {
            match self
                .crits
                .iter_mut()
                .find(|open| open.label == id && open.subject == subject)
            {
                Some(open) => {
                    open.sum += amount;
                    open.parts += 1;
                }
                None => self.crits.push(OpenSum {
                    label: id,
                    subject,
                    sum: amount,
                    parts: 1,
                    position: occurrence.position,
                    left: 0.0,
                    style: style.with_size(crit.size).with_color(crit.color),
                    order: 0,
                }),
            }
            return;
        }
        let tick = occurrence.has(PERIODIC)
            && label.ticks.as_ref().is_some_and(|ticks| {
                ticks.kind.is_everything() || self.passes(world, &ticks.kind, occurrence.kinds)
            });
        let style = match &label.ticks {
            Some(ticks) if tick => style.with_size(ticks.size),
            _ => style,
        };
        let Some(window) = label.merge else {
            float_label(world, &style, occurrence.position, amount);
            return;
        };
        let key = (id, subject, occurrence.kinds, tick);
        if let Some(open) = self.sums.get_mut(&key) {
            open.sum += amount;
            open.parts += 1;
            open.position = occurrence.position;
            return;
        }
        // A window opens: a plain part floats at once, a tick waits for
        // the close.
        let (sum, parts) = if tick {
            (amount, 1)
        } else {
            float_label(world, &style, occurrence.position, amount);
            (0.0, 0)
        };
        self.opened += 1;
        self.sums.insert(
            key,
            OpenSum {
                label: id,
                subject,
                sum,
                parts,
                position: occurrence.position,
                left: window,
                style,
                order: self.opened,
            },
        );
    }

    /// Whether `kinds` pass `filter`, each name's kinds from the type table
    /// through the reactor's cache.
    fn passes(&mut self, world: &World, filter: &KindFilterDef, kinds: DamageKinds) -> bool {
        let table = world.get_resource::<DamageTypeTable>();
        let masks = &mut self.masks;
        filter.matches(kinds, |name| {
            if let Some(&kinds) = masks.get(name) {
                return kinds;
            }
            let kinds = table.map_or(DamageKinds::NONE, |table| table.mask_of(name));
            masks.insert(name.to_owned(), kinds);
            kinds
        })
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
            applied,
            effects_expired,
            abilities_used,
            level_ups,
            occurrences,
            effect_ids,
            effect_count,
            ..
        } = self;
        if let Some(queue) = world.get_resource::<EventQueue<DamagedPart>>() {
            for event in parts.read(queue) {
                let position = event
                    .position
                    .unwrap_or_else(|| point_of(world, event.entity));
                occurrences.push(Occurrence {
                    amount: Some(event.amount),
                    other: event.source,
                    kinds: event.kinds,
                    bits: damage_bits(event.periodic, event.crit),
                    ..Occurrence::new(DAMAGED_PART, event.entity, position)
                });
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<Damaged>>() {
            for event in damaged.read(queue) {
                let position = event
                    .position
                    .unwrap_or_else(|| point_of(world, event.entity));
                let hit = Occurrence {
                    amount: Some(event.amount),
                    other: event.source,
                    kinds: event.kinds,
                    bits: damage_bits(event.periodic, event.crit)
                        | (u8::from(event.is_absorbed()) * ABSORBED),
                    ..Occurrence::new(DAMAGED, event.entity, position)
                };
                occurrences.push(hit);
                if event.killed {
                    occurrences.push(Occurrence {
                        event: KILLED,
                        ..hit
                    });
                }
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<ZoneEntered>>() {
            occurrences.extend(zones.read(queue).map(|event| Occurrence {
                other: Some(event.entity),
                ..Occurrence::new(ZONE_ENTERED, event.zone, event.position)
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<CooldownElapsed>>() {
            occurrences.extend(cooldowns.read(queue).map(|event| {
                Occurrence::new(
                    COOLDOWN_ELAPSED,
                    event.entity,
                    point_of(world, event.entity),
                )
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<Struck>>() {
            occurrences.extend(struck.read(queue).map(|event| Occurrence {
                amount: Some(event.amount),
                other: Some(event.target),
                ..Occurrence::new(HIT, event.entity, event.position)
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<Healed>>() {
            occurrences.extend(healed.read(queue).map(|event| Occurrence {
                amount: Some(event.amount),
                other: event.source,
                ..Occurrence::new(HEALED, event.entity, point_of(world, event.entity))
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<PickedUp>>() {
            occurrences.extend(picked.read(queue).map(|event| Occurrence {
                other: Some(event.by),
                ..Occurrence::new(PICKED_UP, event.entity, event.position)
            }));
        }
        if let Some(queue) = world.get_resource::<EventQueue<ProjectileExpired>>() {
            occurrences.extend(
                expired
                    .read(queue)
                    .map(|event| Occurrence::new(EXPIRED, event.entity, event.position)),
            );
        }
        if let Some(queue) = world.get_resource::<EventQueue<EffectApplied>>() {
            for event in applied.read(queue) {
                occurrences.push(Occurrence {
                    amount: Some(event.stacks as f32),
                    other: event.source,
                    bits: u8::from(event.fresh) * FRESH,
                    effect: effect_index(effect_ids, effect_count, event.id.as_deref()),
                    ..Occurrence::new(EFFECT_APPLIED, event.entity, point_of(world, event.entity))
                });
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<EffectExpired>>() {
            for event in effects_expired.read(queue) {
                occurrences.push(Occurrence {
                    bits: u8::from(event.ended) * ENDED,
                    effect: effect_index(effect_ids, effect_count, event.id.as_deref()),
                    ..Occurrence::new(EFFECT_EXPIRED, event.entity, point_of(world, event.entity))
                });
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<AbilityUsed>>() {
            // The name only for a rule that filters on one, so a World with
            // none indexes nothing.
            let mut by_name = None;
            for event in abilities_used.read(queue) {
                let named = *by_name.get_or_insert_with(|| filters_ability(world));
                occurrences.push(Occurrence {
                    other: event.target,
                    effect: if named {
                        effect_index(effect_ids, effect_count, Some(&event.name))
                    } else {
                        NO_EFFECT
                    },
                    ..Occurrence::new(ABILITY_USED, event.entity, event.position)
                });
            }
        }
        if let Some(queue) = world.get_resource::<EventQueue<LevelUp>>() {
            occurrences.extend(level_ups.read(queue).map(|event| Occurrence {
                amount: Some(event.level as f32),
                ..Occurrence::new(LEVEL_UP, event.entity, point_of(world, event.entity))
            }));
        }
        if let Some(cues) = world.get_resource_mut::<ControllerCues>() {
            occurrences.extend(cues.0.drain(..).map(|cue| Occurrence {
                amount: cue.amount,
                ..Occurrence::new(cue.event, cue.entity, cue.at)
            }));
        }
        if let Some(cues) = world.get_resource_mut::<AbilityCues>()
            && !cues.0.is_empty()
        {
            let mut cues = std::mem::take(&mut cues.0);
            for cue in cues.drain(..) {
                let name = world
                    .get::<Abilities>(cue.entity)
                    .and_then(|abilities| abilities.name_at(usize::from(cue.entry)));
                occurrences.push(Occurrence {
                    bits: cue.reason << REASON_SHIFT,
                    effect: effect_index(effect_ids, effect_count, name),
                    ..Occurrence::new(cue.event, cue.entity, cue.at)
                });
            }
            if let Some(list) = world.get_resource_mut::<AbilityCues>() {
                list.0 = cues;
            }
        }
        if let Some(mut sources) = world.remove_resource::<CueSources>() {
            for source in &mut sources.0 {
                source.read(world, occurrences);
            }
            world.insert_resource(sources);
        }
    }
}

/// Where prefab `id` spawns for a cue at `position`: there, or with
/// `centred` its point placed so its centre lands there.
fn placed(world: &World, id: &str, position: Vec2, centred: bool) -> Vec2 {
    if centred {
        position - prefab_centre(world, id)
    } else {
        position
    }
}

/// Moves the World's [`CueListener`] to the entity it follows, when that
/// entity is alive with a point.
fn follow_listener(world: &mut World) {
    let Some(entity) = world
        .get_resource::<CueListener>()
        .and_then(|listener| listener.follows)
    else {
        return;
    };
    if !world.is_alive(entity) || !(world.has::<Position>(entity) || world.has::<Transform>(entity))
    {
        return;
    }
    let at = point_of(world, entity);
    world.resource_mut::<CueListener>().position = at;
}

/// A shake's share `distance` from the listener: whole within `near`,
/// falling linearly to none at `far`.
fn falloff_share(distance: f32, near: f32, far: f32) -> f32 {
    if distance <= near {
        1.0
    } else if distance >= far || distance.is_nan() {
        0.0
    } else {
        1.0 - (distance - near) / (far - near)
    }
}

/// Whether a loop's volume or pan moved from `sent` to `now` by enough to
/// send: past 1/64, or to or from 0.
fn moved(sent: f32, now: f32) -> bool {
    (sent - now).abs() > LOOP_STEP || ((sent == 0.0) != (now == 0.0))
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
            .genre(RPG)
            .genre(SHOOTER)
            .sample("{}")
            .roundtrip(cues_round_trip),
    );
    let shapes = [
        ComponentDescription::new("cue_action", "One reaction to an event.")
            .one_of()
            .field(FieldDescription::new(
                "prefab",
                FieldType::Shape("prefab_action"),
                "spawned at the event's position from the subject, or its centre there; an ID alone reads as `{ \"id\": .. }`",
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
                FieldType::Shape("shake_action"),
                "camera trauma added, falling off from the cue listener with `near` and `far`; a number alone reads as `{ \"trauma\": .. }`",
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
                FieldType::Shape("label_action"),
                "the event's amount floated at its position in a `label_style`; on `damaged_part`, without a font, in its type's, grouped by `merge`, `ticks` and `crit`",
            ))
            .field(FieldDescription::new(
                "roll",
                FieldType::Shape("roll_action"),
                "a weighted table drawn from the kit's random stream, each prefab drawn spawned at the event's position from the subject, or its centre there; a table alone reads as `{ \"table\": .. }`",
            ))
            .field(FieldDescription::new(
                "effect",
                FieldType::Shape("effect_action"),
                "an effect queued on the subject from the event's other entity, or on the other from the subject; an ID alone reads as `{ \"id\": .. }`",
            ))
            .field(FieldDescription::new(
                "ability",
                FieldType::Shape("ability_action"),
                "the subject's or the other's ability used now, or made ready; a name alone reads as `{ \"name\": .. }`",
            ))
            .field(FieldDescription::new(
                "loop",
                FieldType::Shape("loop_action"),
                "the subject wants a manifest sound looping: one voice per sound at its loudest want by the cue listener",
            ))
            .field(FieldDescription::new(
                "stop",
                FieldType::Shape("stop_action"),
                "the subject's want of a looping sound ends; the last one's voice fades out",
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
        ComponentDescription::new(
            "effect_action",
            "An effect applied at the next effect resolve, from the event's other entity on the subject or from the subject on the other.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Data(EFFECTS),
            "an `effects` entry or the effect in place; the load checks it",
        ))
        .field(target(&["self", "other"], "the subject or the other entity"))
        .sample(r#"{ "id": "burning" }"#)
        .roundtrip(round_trip::<EffectActionDef>),
        ComponentDescription::new(
            "ability_action",
            "An ability of the subject or the other entity used now, as `use_ability` uses it, or made ready with every charge.",
        )
        .field(FieldDescription::new(
            "name",
            FieldType::Text,
            "the ability's name in the entity's `abilities`: an entry's ID or `#<index>`",
        ))
        .field(
            FieldDescription::new(
                "op",
                FieldType::Enum(&["use", "refill"]),
                "used now, or every charge back and the cooldown cleared",
            )
            .default(r#""use""#),
        )
        .field(target(&["self", "other"], "the subject or the other entity"))
        .sample(r#"{ "name": "wand" }"#)
        .roundtrip(round_trip::<AbilityActionDef>),
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
        ComponentDescription::new(
            "shake_action",
            "Camera trauma added: whole within `near` of the cue listener, falling linearly to none at `far`; whole without a listener or a `far`.",
        )
        .field(FieldDescription::new(
            "trauma",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Open,
            },
            "camera trauma added",
        ))
        .field(
            FieldDescription::new(
                "near",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels from the listener within which it is whole",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "far",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels from the listener at which it is none; above `near`",
            )
            .default("null"),
        )
        .sample(r#"{ "trauma": 0.3 }"#)
        .roundtrip(round_trip::<ShakeObject>),
        ComponentDescription::new(
            "loop_action",
            "A manifest sound the subject wants looping: one voice per sound ID, at the loudest of its wants, each its volume times the cue listener's gain at its wanter, panned toward that one; unsaved.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Text,
            "the manifest sound; the load checks the ID",
        ))
        .field(
            FieldDescription::new(
                "volume",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "the want's share of the sound's volume",
            )
            .default("1.0"),
        )
        .field(
            FieldDescription::new(
                "fade",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "seconds the voice rises over when this want starts it",
            )
            .default("0.1"),
        )
        .sample(r#"{ "id": "crackle" }"#)
        .roundtrip(round_trip::<LoopDef>),
        ComponentDescription::new(
            "stop_action",
            "The subject's want of a looping sound ended; the sound's last want fades its voice out.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Text,
            "the manifest sound; the load checks the ID",
        ))
        .field(
            FieldDescription::new(
                "fade",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "seconds the voice fades out over when this was its last want",
            )
            .default("0.1"),
        )
        .sample(r#"{ "id": "crackle" }"#)
        .roundtrip(round_trip::<StopDef>),
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
            "label_action",
            "A `label` action: a `label_style` that names its font, or on `damaged_part` none, its part's type's style, which the grouping fields take.",
        )
        .field(
            FieldDescription::new(
                "font",
                FieldType::Text,
                "a manifest font face, and with it a `label_style`'s other fields; without it, a `damaged_part`'s type's style",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "merge",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds a target's parts of a type sum over: the first plain part floats at once, the rest as one sum when the window closes; without it each part floats alone",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "ticks",
                FieldType::Shape("label_ticks"),
                "periodic parts summed on their own over the window",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "crit",
                FieldType::Shape("label_crit"),
                "a step's crit parts on a target floated at once as one sum",
            )
            .default("null"),
        )
        .sample("{}")
        .roundtrip(round_trip::<LabelDef>),
        ComponentDescription::new(
            "label_ticks",
            "A type label's periodic parts: summed on their own over its window at this size.",
        )
        .field(FieldDescription::new(
            "size",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "UI units tall",
        ))
        .field(
            FieldDescription::new(
                "kind",
                FieldType::Filter,
                "the periodic parts it takes by their damage kinds, each a declared damage type or an ancestor of one",
            )
            .default("{}"),
        )
        .sample(r#"{ "size": 10 }"#)
        .roundtrip(round_trip::<TickLabel>),
        ComponentDescription::new(
            "label_crit",
            "A type label's crit style: a step's crit parts on a target as one sum, in the first part's type's style at this size and colour.",
        )
        .field(FieldDescription::new(
            "size",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "UI units tall",
        ))
        .field(FieldDescription::new(
            "color",
            FieldType::Color,
            "linear, encoded to sRGB when drawn",
        ))
        .sample(r#"{ "size": 24, "color": [1.0, 0.7, 0.0, 1.0] }"#)
        .roundtrip(round_trip::<CritLabel>),
        ComponentDescription::new(
            "prefab_action",
            "A prefab spawned from the subject at the event's position, its point or its centre there.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Prefab,
            "the manifest prefab; the load checks the ID",
        ))
        .field(
            FieldDescription::new(
                "centred",
                FieldType::Bool,
                "its centre, its `rigid_body`'s offset or its sprite's drawn centre, on the position rather than its point",
            )
            .default("false"),
        )
        .sample(r#"{ "id": "puff" }"#)
        .roundtrip(round_trip::<PrefabActionObject>),
        ComponentDescription::new(
            "roll_action",
            "A weighted table drawn, each prefab drawn spawned from the subject at the event's position, its point or its centre there.",
        )
        .field(FieldDescription::new(
            "table",
            FieldType::Data(LOOT_TABLES),
            "the table: a `loot_tables` entry's ID or a table in place; the load checks its IDs",
        ))
        .field(
            FieldDescription::new(
                "centred",
                FieldType::Bool,
                "each prefab's centre, its `rigid_body`'s offset or its sprite's drawn centre, on the position rather than its point",
            )
            .default("false"),
        )
        .sample(r#"{ "table": "drops" }"#)
        .roundtrip(round_trip::<RollActionObject>),
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
        .field(
            FieldDescription::new(
                "periodic",
                FieldType::Bool,
                "`true` passes a periodic request's tick, `false` a blow; `damaged`, `killed` and `damaged_part` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "amount",
                FieldType::Shape("cue_amount"),
                "the range the occurrence's amount falls in; an event with an amount only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "flags",
                FieldType::Filter,
                "the game's own flags, read as a tag filter reads tags; none set without them",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "effect",
                FieldType::Text,
                "an `effects` entry ID, which the load checks; `effect_applied` and `effect_expired` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "fresh",
                FieldType::Bool,
                "`true` passes a new instance or an instant effect's landing, `false` a stack or a refresh; `effect_applied` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "ended",
                FieldType::Bool,
                "`false` passes an instance that ran out, `true` one ended early; `effect_expired` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "absorbed",
                FieldType::Bool,
                "`true` passes a hit the shield took whole, `false` one that hurt; `damaged` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "ability",
                FieldType::Text,
                "an ability's name in its user's `abilities`; `ability_used`, `ability_refused`, `ability_ready`, `ability_held` and `ability_released` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "reason",
                FieldType::Enum(&["not_ready", "no_charge", "no_cost", "no_target"]),
                "why the use was refused; `ability_refused` only",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "min_interval",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "game seconds the rule waits for a subject once it ran for it",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "cap",
                FieldType::U32 {
                    min: Some(1),
                    max: None,
                },
                "the rule's runs a step at most",
            )
            .default("null"),
        )
        .field(FieldDescription::new(
            "do",
            FieldType::List(Box::new(FieldType::Shape("cue_action"))),
            "the actions, in order",
        ))
        .sample(r#"{ "on": "killed", "do": [] }"#)
        .roundtrip(round_trip::<CueRuleDef>),
        ComponentDescription::new(
            "cue_amount",
            "A rule's range over an occurrence's amount, each end inclusive and optional.",
        )
        .field(
            FieldDescription::new(
                "min",
                FieldType::F32 {
                    min: Bound::Open,
                    max: Bound::Open,
                },
                "the least amount that passes",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "max",
                FieldType::F32 {
                    min: Bound::Open,
                    max: Bound::Open,
                },
                "the greatest amount that passes",
            )
            .default("null"),
        )
        .sample("{}")
        .roundtrip(round_trip::<AmountRange>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
    describe_kind(
        world,
        CUE_RULES,
        "A list of `cue_rule`: shared reactions to an event, run after the entity's own cues.",
    );
    describe_kind(
        world,
        WORLD_CUES,
        "A list of `cue_action` a game plays by name at a point, with no subject: `prefab`, `roll`, `sound`, `shake`, a `label` with a font, and a `counter` or `flag` with `target: game`.",
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
