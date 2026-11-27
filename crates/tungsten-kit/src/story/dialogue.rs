//! Speakers and conversations as data (redo 8.1, P8-1): the `speakers` and
//! `dialogue` kinds, held at parse to their graph's checks and at load to
//! the World's names.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use tungsten_core::save;
use tungsten_core::{AssetRegistry, DataRegistry, MissingAsset, Plugin, Schedule, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape,
    round_trip,
};
use crate::foundation::phases::ensure_phases;
use crate::react::cues::{CueAction, check_list};
use crate::story::condition::Condition;
use crate::story::runner::{DialogueEnded, DialogueStarted, end_on_restore};

/// The data kind of speakers.
pub(crate) const SPEAKERS: &str = "speakers";
/// The data kind of conversations.
pub(crate) const DIALOGUE: &str = "dialogue";

/// The mood a line without one shows.
pub(crate) const NEUTRAL: &str = "neutral";
/// The most nodes a conversation holds.
pub(crate) const MAX_NODES: usize = 256;
/// The most lines a node holds.
pub(crate) const MAX_LINES: usize = 32;
/// The most choices a node offers.
pub(crate) const MAX_CHOICES: usize = 8;
/// The most characters a line's or a choice's text holds.
pub(crate) const MAX_TEXT: usize = 2000;
/// The longest chain of nodes that may show nothing, so one call into a
/// conversation enters at most one more.
pub(crate) const MAX_JUMPS: usize = 63;

/// Who speaks a conversation's lines: a `speakers` entry, one a file.
///
/// `{ "name": "Old Tamsin", "portrait": { "neutral": "keeper_face", "worried": "keeper_worried" }, "pace": 40 }`:
/// `name` is shown; `portrait` is a sprite ID, or moods to sprite IDs with
/// `neutral` the mood of a line that names none, and optional; `pace` is
/// the characters a second a dialogue box reveals the speaker's lines at,
/// above 0, 40 by default. The load checks each portrait names a sprite
/// the registry knows.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Speaker;
///
/// let keeper: Speaker = serde_json::from_str(
///     r#"{ "name": "Old Tamsin", "portrait": { "neutral": "keeper_face", "worried": "keeper_worried" } }"#,
/// )
/// .unwrap();
/// assert_eq!((keeper.name(), keeper.pace()), ("Old Tamsin", 40.0));
/// assert_eq!(keeper.portrait(None), Some("keeper_face"));
/// assert_eq!(keeper.portrait(Some("worried")), Some("keeper_worried"));
/// assert_eq!(Speaker::new("Gull").portrait(None), None);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SpeakerFields", into = "SpeakerFields")]
pub struct Speaker {
    name: String,
    /// Mood to sprite, sorted by mood.
    portraits: BTreeMap<String, String>,
    pace: f32,
}

impl Speaker {
    /// A speaker shown as `name`, with no portrait, at 40 characters a
    /// second.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            portraits: BTreeMap::new(),
            pace: default_pace(),
        }
    }

    /// The speaker, showing sprite `sprite` for lines in `mood`.
    #[must_use]
    pub fn with_portrait(mut self, mood: impl Into<String>, sprite: impl Into<String>) -> Self {
        self.portraits.insert(mood.into(), sprite.into());
        self
    }

    /// The speaker, its lines revealed at `pace` characters a second.
    ///
    /// # Panics
    ///
    /// When `pace` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_pace(mut self, pace: f32) -> Self {
        assert!(
            pace.is_finite() && pace > 0.0,
            "Speaker::with_pace: {pace} is not finite and above 0"
        );
        self.pace = pace;
        self
    }

    /// The name shown.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Characters a second a box reveals its lines at.
    #[must_use]
    pub fn pace(&self) -> f32 {
        self.pace
    }

    /// The sprite for a line in `mood`, `neutral` when it names none; `None`
    /// when the speaker has none for it.
    #[must_use]
    pub fn portrait(&self, mood: Option<&str>) -> Option<&str> {
        self.portraits
            .get(mood.unwrap_or(NEUTRAL))
            .map(String::as_str)
    }

    /// Whether it has a portrait for `mood`.
    pub(crate) fn has_mood(&self, mood: &str) -> bool {
        self.portraits.contains_key(mood)
    }
}

fn default_pace() -> f32 {
    40.0
}

/// A speaker as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeakerFields {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    portrait: Option<PortraitDef>,
    #[serde(default = "default_pace")]
    pace: f32,
}

/// A speaker's portrait: one sprite, its `neutral`, or moods to sprites.
#[derive(Serialize)]
#[serde(untagged)]
enum PortraitDef {
    One(String),
    Moods(BTreeMap<String, String>),
}

impl<'de> Deserialize<'de> for PortraitDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(sprite) = value.as_str() {
            return Ok(Self::One(sprite.to_owned()));
        }
        BTreeMap::<String, String>::deserialize(value)
            .map(Self::Moods)
            .map_err(|e| de::Error::custom(format!("a sprite ID, or moods to sprite IDs: {e}")))
    }
}

impl TryFrom<SpeakerFields> for Speaker {
    type Error = String;

    fn try_from(fields: SpeakerFields) -> Result<Self, Self::Error> {
        if !(fields.pace.is_finite() && fields.pace > 0.0) {
            return Err(format!("pace: {} is not finite and above 0", fields.pace));
        }
        let portraits = match fields.portrait {
            None => BTreeMap::new(),
            Some(PortraitDef::One(sprite)) => BTreeMap::from([(NEUTRAL.to_owned(), sprite)]),
            Some(PortraitDef::Moods(moods)) => {
                if let Some(mood) = moods.keys().find(|mood| !valid_name(mood)) {
                    return Err(format!(
                        "portrait.{mood}: not a mood name (`[a-z][a-z0-9_]*`)"
                    ));
                }
                moods
            }
        };
        Ok(Self {
            name: fields.name,
            portraits,
            pace: fields.pace,
        })
    }
}

impl From<Speaker> for SpeakerFields {
    fn from(speaker: Speaker) -> Self {
        let portrait = match speaker.portraits.len() {
            0 => None,
            1 if speaker.portraits.contains_key(NEUTRAL) => {
                speaker.portraits.into_values().next().map(PortraitDef::One)
            }
            _ => Some(PortraitDef::Moods(speaker.portraits)),
        };
        Self {
            name: speaker.name,
            portrait,
            pace: speaker.pace,
        }
    }
}

/// A conversation: a `dialogue` entry, one a file. Its nodes, by name, each
/// show lines, then offer choices, go on to a node, branch on conditions
/// or end the conversation; a runner walks it (`talk`).
///
/// ```json
/// {
///   "speaker": "keeper",
///   "start": "greet",
///   "nodes": {
///     "greet": { "lines": ["Evening, warden.", { "speaker": "warden", "text": "Evening." }], "next": "offer" },
///     "offer": {
///       "lines": [{ "text": "The lamp by the steps is out again.", "mood": "worried" }],
///       "choices": [
///         { "text": "I'll light it.", "do": [{ "flag": { "name": "lamp.promised", "target": "game" } }], "next": "thanks" },
///         { "text": "Not tonight.", "back": true }
///       ]
///     },
///     "thanks": { "lines": ["Bless you."] }
///   }
/// }
/// ```
///
/// `speaker` is its lines' speaker unless a line names its own; node names
/// are `[a-z][a-z0-9_]*`. A node has `lines`, `do` (cue actions run as it
/// is entered) and at most one exit: `choices`, `next` (a node) or `branch`
/// (a list of `{ "when", "next" }`, the first that holds); none ends the
/// conversation. A line is its text, or `{ "speaker", "text", "mood",
/// "when" }`, skipped when its `when` fails. A choice has `text`, `when`,
/// `do`, `next` (none ends) and `back`, the choice Cancel takes, one a
/// node at most.
///
/// The parse holds it to at most 256 nodes, 32 lines and 8 choices a node
/// and 2,000 characters a text; `start` and every target naming a node;
/// every node reachable from `start`; a choice without a `when` in every
/// list and a last branch entry without one, so a shown node always has a
/// way on; and the nodes that may show nothing, with no choices and no line
/// free of a `when`, joined by their exits in no cycle and no chain longer
/// than 63, so one call enters at most 64 nodes. Each error names its field
/// (`nodes.offer.choices.1.next`). The load checks each speaker names a
/// `speakers` entry and each mood one of its portraits, each condition's
/// names and each `do` list's actions as a cue's with an other entity and
/// no amount, refusing `ability`, `loop`, `stop` and `label`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Conversation;
///
/// let chat: Conversation = serde_json::from_str(
///     r#"{ "speaker": "keeper", "start": "hi", "nodes": { "hi": { "lines": ["Evening."] } } }"#,
/// )
/// .unwrap();
/// assert_eq!((chat.speaker(), chat.start()), ("keeper", "hi"));
/// let error = serde_json::from_str::<Conversation>(
///     r#"{ "speaker": "keeper", "start": "hi", "nodes": { "hi": { "next": "bye" } } }"#,
/// )
/// .unwrap_err();
/// assert!(error.to_string().contains("nodes.hi.next: unknown node `bye`"), "{error}");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ConversationFields", into = "ConversationFields")]
pub struct Conversation {
    speaker: String,
    start: String,
    nodes: BTreeMap<String, Node>,
}

impl Conversation {
    /// The speaker of its lines that name none: a `speakers` ID.
    #[must_use]
    pub fn speaker(&self) -> &str {
        &self.speaker
    }

    /// The node a talk enters first.
    #[must_use]
    pub fn start(&self) -> &str {
        &self.start
    }

    /// Its node names, in order.
    pub fn node_names(&self) -> impl Iterator<Item = &str> {
        self.nodes.keys().map(String::as_str)
    }

    /// Node `name`.
    pub(crate) fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.get(name)
    }
}

#[cfg(test)]
impl Conversation {
    /// A conversation built past the parse's checks, for a test of the
    /// runner's own bound.
    pub(crate) fn unchecked(speaker: &str, start: &str, nodes: BTreeMap<String, Node>) -> Self {
        Self {
            speaker: speaker.to_owned(),
            start: start.to_owned(),
            nodes,
        }
    }
}

/// One node of a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Node {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) lines: Vec<Line>,
    #[serde(default, rename = "do", skip_serializing_if = "Vec::is_empty")]
    pub(crate) actions: Vec<CueAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) choices: Vec<Choice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) next: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) branch: Vec<BranchEntry>,
}

impl Node {
    /// Whether entering it may show nothing: no choices, and no line free
    /// of a `when`.
    pub(crate) fn may_show_nothing(&self) -> bool {
        self.choices.is_empty() && self.lines.iter().all(|line| line.when.is_some())
    }

    /// The nodes its `next` and `branch` name, which a node that shows
    /// nothing passes on to.
    fn jumps(&self) -> impl Iterator<Item = &str> {
        self.next
            .iter()
            .map(String::as_str)
            .chain(self.branch.iter().map(|entry| entry.next.as_str()))
    }
}

/// One line of a node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    pub(crate) speaker: Option<String>,
    pub(crate) text: String,
    pub(crate) mood: Option<String>,
    pub(crate) when: Option<Condition>,
}

/// A line's object form.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    speaker: Option<String>,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mood: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    when: Option<Condition>,
}

impl Serialize for Line {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.speaker.is_none() && self.mood.is_none() && self.when.is_none() {
            self.text.serialize(serializer)
        } else {
            LineFields {
                speaker: self.speaker.clone(),
                text: self.text.clone(),
                mood: self.mood.clone(),
                when: self.when.clone(),
            }
            .serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Line {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(text) = value.as_str() {
            return Ok(Self {
                speaker: None,
                text: text.to_owned(),
                mood: None,
                when: None,
            });
        }
        let fields = LineFields::deserialize(value).map_err(de::Error::custom)?;
        Ok(Self {
            speaker: fields.speaker,
            text: fields.text,
            mood: fields.mood,
            when: fields.when,
        })
    }
}

/// One choice of a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Choice {
    pub(crate) text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) when: Option<Condition>,
    #[serde(default, rename = "do", skip_serializing_if = "Vec::is_empty")]
    pub(crate) actions: Vec<CueAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) next: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) back: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(value: &bool) -> bool {
    !*value
}

/// One entry of a node's `branch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BranchEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) when: Option<Condition>,
    pub(crate) next: String,
}

/// A conversation as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversationFields {
    speaker: String,
    start: String,
    nodes: BTreeMap<String, Node>,
}

impl From<Conversation> for ConversationFields {
    fn from(conversation: Conversation) -> Self {
        Self {
            speaker: conversation.speaker,
            start: conversation.start,
            nodes: conversation.nodes,
        }
    }
}

/// Whether `name` is a node or mood name: `[a-z][a-z0-9_]*`.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `text` within [`MAX_TEXT`] characters, else the error at `field`.
fn check_text(field: &str, text: &str) -> Result<(), String> {
    let count = text.chars().count();
    if count > MAX_TEXT {
        return Err(format!("{field}: {count} characters, past {MAX_TEXT}"));
    }
    Ok(())
}

impl TryFrom<ConversationFields> for Conversation {
    type Error = String;

    fn try_from(fields: ConversationFields) -> Result<Self, Self::Error> {
        let nodes = &fields.nodes;
        if nodes.is_empty() {
            return Err("nodes: a conversation has at least one node".into());
        }
        if nodes.len() > MAX_NODES {
            return Err(format!("nodes: {} nodes, past {MAX_NODES}", nodes.len()));
        }
        let known = |field: &str, name: &str| -> Result<(), String> {
            if nodes.contains_key(name) {
                Ok(())
            } else {
                Err(format!("{field}: unknown node `{name}`"))
            }
        };
        known("start", &fields.start)?;
        for (name, node) in nodes {
            let at = format!("nodes.{name}");
            if !valid_name(name) {
                return Err(format!("{at}: not a node name (`[a-z][a-z0-9_]*`)"));
            }
            check_node(&at, node, &known)?;
        }
        check_reachable(&fields.start, nodes)?;
        check_jumps(nodes)?;
        Ok(Self {
            speaker: fields.speaker,
            start: fields.start,
            nodes: fields.nodes,
        })
    }
}

/// A node's own checks: its bounds, its one exit, its texts and targets,
/// its choices' `back` and a way on.
fn check_node(
    at: &str,
    node: &Node,
    known: &impl Fn(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    if node.lines.len() > MAX_LINES {
        return Err(format!(
            "{at}.lines: {} lines, past {MAX_LINES}",
            node.lines.len()
        ));
    }
    if node.choices.len() > MAX_CHOICES {
        return Err(format!(
            "{at}.choices: {} choices, past {MAX_CHOICES}",
            node.choices.len()
        ));
    }
    let exits = usize::from(!node.choices.is_empty())
        + usize::from(node.next.is_some())
        + usize::from(!node.branch.is_empty());
    if exits > 1 {
        return Err(format!(
            "{at}: at most one of `choices`, `next` and `branch`"
        ));
    }
    for (i, line) in node.lines.iter().enumerate() {
        check_text(&format!("{at}.lines.{i}.text"), &line.text)?;
        if let Some(mood) = &line.mood
            && !valid_name(mood)
        {
            return Err(format!(
                "{at}.lines.{i}.mood: not a mood name (`[a-z][a-z0-9_]*`)"
            ));
        }
    }
    if let Some(next) = &node.next {
        known(&format!("{at}.next"), next)?;
    }
    for (i, choice) in node.choices.iter().enumerate() {
        check_text(&format!("{at}.choices.{i}.text"), &choice.text)?;
        if let Some(next) = &choice.next {
            known(&format!("{at}.choices.{i}.next"), next)?;
        }
        if choice.back && node.choices[..i].iter().any(|earlier| earlier.back) {
            return Err(format!(
                "{at}.choices.{i}.back: a node has one `back` choice at most"
            ));
        }
    }
    if !node.choices.is_empty() && node.choices.iter().all(|choice| choice.when.is_some()) {
        return Err(format!(
            "{at}.choices: every choice has a `when`, so the node may offer none; leave one without"
        ));
    }
    for (i, entry) in node.branch.iter().enumerate() {
        known(&format!("{at}.branch.{i}.next"), &entry.next)?;
    }
    if node.branch.last().is_some_and(|last| last.when.is_some()) {
        let last = node.branch.len() - 1;
        return Err(format!(
            "{at}.branch.{last}.when: the last entry takes no `when`, so the branch always goes on"
        ));
    }
    Ok(())
}

/// Every node reachable from `start` through `next`, `branch` and the
/// choices' `next`.
fn check_reachable(start: &str, nodes: &BTreeMap<String, Node>) -> Result<(), String> {
    let mut seen: BTreeSet<&str> = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(name) = stack.pop() {
        let node = &nodes[name];
        let targets = node.jumps().chain(
            node.choices
                .iter()
                .filter_map(|choice| choice.next.as_deref()),
        );
        for target in targets {
            if seen.insert(target) {
                stack.push(target);
            }
        }
    }
    match nodes.keys().find(|name| !seen.contains(name.as_str())) {
        Some(name) => Err(format!("nodes.{name}: unreachable from `start`")),
        None => Ok(()),
    }
}

/// The nodes that may show nothing, joined by their `next` and `branch`, in
/// no cycle and no chain of more than [`MAX_JUMPS`].
fn check_jumps(nodes: &BTreeMap<String, Node>) -> Result<(), String> {
    /// A node's state in the walk: on the path, or its longest chain.
    enum Mark {
        Open,
        Done(usize),
    }
    fn chain<'a>(
        name: &'a str,
        nodes: &'a BTreeMap<String, Node>,
        marks: &mut BTreeMap<&'a str, Mark>,
    ) -> Result<usize, String> {
        let node = &nodes[name];
        if !node.may_show_nothing() {
            return Ok(0);
        }
        match marks.get(name) {
            Some(Mark::Done(length)) => return Ok(*length),
            Some(Mark::Open) => {
                return Err(format!(
                    "nodes.{name}: a cycle of nodes that may show nothing, which would never stop"
                ));
            }
            None => {}
        }
        marks.insert(name, Mark::Open);
        let mut longest = 0;
        for target in node.jumps() {
            longest = longest.max(chain(target, nodes, marks)?);
        }
        let length = longest + 1;
        if length > MAX_JUMPS {
            return Err(format!(
                "nodes.{name}: a chain of {length} nodes that may show nothing, past {MAX_JUMPS}"
            ));
        }
        marks.insert(name, Mark::Done(length));
        Ok(length)
    }
    let mut marks = BTreeMap::new();
    for name in nodes.keys() {
        chain(name, nodes, &mut marks)?;
    }
    Ok(())
}

/// The registry's speaker `id`, else the [`MissingAsset`] of kind `speaker`
/// at `field`. A World with no data registry passes.
fn speaker_at<'w>(
    world: &'w World,
    field: &str,
    id: &str,
) -> Result<Option<&'w Speaker>, MissingAsset> {
    let Some(data) = world.get_resource::<DataRegistry>() else {
        return Ok(None);
    };
    data.get::<Speaker>(id)
        .map(Some)
        .ok_or_else(|| MissingAsset::new(field, "speaker", id))
}

/// The `speakers` kind's check: each portrait a sprite the registry knows,
/// loaded or interned.
fn check_speaker(speaker: &Speaker, world: &World) -> Result<(), MissingAsset> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Ok(());
    };
    for (mood, sprite) in &speaker.portraits {
        if assets.sprite_id(sprite).is_none() {
            return Err(MissingAsset::new(
                format!("portrait.{mood}"),
                "sprite",
                sprite.as_str(),
            ));
        }
    }
    Ok(())
}

/// The `dialogue` kind's check: each speaker and mood, each condition's
/// names and each `do` list, at its field.
fn check_conversation(conversation: &Conversation, world: &World) -> Result<(), MissingAsset> {
    speaker_at(world, "speaker", &conversation.speaker)?;
    for (name, node) in &conversation.nodes {
        let at = format!("nodes.{name}");
        for (i, line) in node.lines.iter().enumerate() {
            let line_at = format!("{at}.lines.{i}");
            let speaker = match &line.speaker {
                Some(id) => speaker_at(world, &format!("{line_at}.speaker"), id)?,
                None => speaker_at(world, "speaker", &conversation.speaker)?,
            };
            if let (Some(mood), Some(speaker)) = (&line.mood, speaker)
                && !speaker.has_mood(mood)
            {
                return Err(MissingAsset::new(
                    format!("{line_at}.mood"),
                    "speaker's portrait mood",
                    mood.as_str(),
                ));
            }
            if let Some(when) = &line.when {
                when.check(world, &format!("{line_at}.when"))?;
            }
        }
        check_list(world, &format!("{at}.do"), &node.actions, true)?;
        for (i, choice) in node.choices.iter().enumerate() {
            let choice_at = format!("{at}.choices.{i}");
            if let Some(when) = &choice.when {
                when.check(world, &format!("{choice_at}.when"))?;
            }
            check_list(world, &format!("{choice_at}.do"), &choice.actions, true)?;
        }
        for (i, entry) in node.branch.iter().enumerate() {
            if let Some(when) = &entry.when {
                when.check(world, &format!("{at}.branch.{i}.when"))?;
            }
        }
    }
    Ok(())
}

/// Registers the `speakers` and `dialogue` data kinds with their checks and
/// descriptions, `EventQueue<DialogueStarted>` and
/// `EventQueue<DialogueEnded>`, and a restore hook ([`save::on_restore`])
/// that ends a running talk. The runner that walks a conversation,
/// [`talk`](crate::talk) and [`Dialogue`](crate::Dialogue), is calls, not a
/// system.
#[derive(Debug, Default, Clone, Copy)]
pub struct DialoguePlugin;

/// The restore hook's name.
const RESTORE_HOOK: &str = "kit.dialogue";

impl Plugin for DialoguePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        world.register_data_kind_with_assets(SPEAKERS, check_speaker);
        world.register_data_kind_with_assets(DIALOGUE, check_conversation);
        world.register_event::<DialogueStarted>();
        world.register_event::<DialogueEnded>();
        save::on_restore(world, RESTORE_HOOK, end_on_restore);
        describe_dialogue(world);
    }
}

/// The node, line, choice and branch round trips: a list left out, which
/// the descriptions write as its default `[]`.
fn with_lists(
    value: serde_json::Value,
    lists: &[&str],
    out: fn(serde_json::Value) -> Result<serde_json::Value, String>,
) -> Result<serde_json::Value, String> {
    let mut out = out(value)?;
    if let Some(object) = out.as_object_mut() {
        for list in lists {
            object
                .entry(*list)
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        }
    }
    Ok(out)
}

fn node_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    with_lists(
        value,
        &["lines", "do", "choices", "branch"],
        round_trip::<Node>,
    )
}

fn choice_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut out = with_lists(value, &["do"], round_trip::<Choice>)?;
    if let Some(object) = out.as_object_mut() {
        object
            .entry("back")
            .or_insert(serde_json::Value::Bool(false));
    }
    Ok(out)
}

/// A line's round trip, through its object form so the description's
/// fields read back.
fn line_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let line: Line = serde_json::from_value(value).map_err(|e| e.to_string())?;
    serde_json::to_value(LineFields {
        speaker: line.speaker,
        text: line.text,
        mood: line.mood,
        when: line.when,
    })
    .map_err(|e| e.to_string())
}

/// The descriptions of the two kinds and their shapes.
fn describe_dialogue(world: &mut World) {
    describe_kind(
        world,
        SPEAKERS,
        "A `speaker`: the name a conversation shows, its portraits by mood and its reveal pace.",
    );
    describe_kind(
        world,
        DIALOGUE,
        "A `conversation`: nodes of lines, choices and branches that a talk walks.",
    );
    let shapes = [
        ComponentDescription::new("speaker", "Who speaks: a `speakers` entry.")
            .field(FieldDescription::new("name", FieldType::Text, "the name shown"))
            .field(
                FieldDescription::new(
                    "portrait",
                    FieldType::Object("a sprite ID, or { mood: sprite ID }"),
                    "the sprite a line in each mood shows, `neutral` for a line naming none; the load checks each",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "pace",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "characters a second a box reveals its lines at",
                )
                .default("40.0"),
            )
            .sample(r#"{ "name": "Old Tamsin" }"#)
            .roundtrip(round_trip::<Speaker>),
        ComponentDescription::new(
            "conversation",
            "A `dialogue` entry: nodes by name, walked from `start`; lists run at once, story-safe actions only.",
        )
        .field(FieldDescription::new(
            "speaker",
            FieldType::Text,
            "the `speakers` entry of its lines that name none; the load checks it",
        ))
        .field(FieldDescription::new(
            "start",
            FieldType::Text,
            "the node a talk enters first",
        ))
        .field(FieldDescription::new(
            "nodes",
            FieldType::Object("{ node name: dialogue_node }"),
            "1 to 256, `[a-z][a-z0-9_]*`, each reachable from `start`; the nodes that may show nothing chain at most 63 and never cycle",
        ))
        .sample(r#"{ "speaker": "keeper", "start": "hi", "nodes": { "hi": { "lines": ["Evening."] } } }"#)
        .roundtrip(round_trip::<Conversation>),
        ComponentDescription::new(
            "dialogue_node",
            "A node: its lines, then at most one exit; none ends the talk.",
        )
        .field(
            FieldDescription::new(
                "lines",
                FieldType::List(Box::new(FieldType::Shape("dialogue_line"))),
                "at most 32, each shown in turn, skipped when its `when` fails",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "do",
                FieldType::List(Box::new(FieldType::Shape("cue_action"))),
                "run as the node is entered, the speaker the subject, the listener the other; no `ability`, `loop`, `stop` or `label`",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "choices",
                FieldType::List(Box::new(FieldType::Shape("dialogue_choice"))),
                "at most 8, offered after the lines while their `when` holds; one without a `when`",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new("next", FieldType::Text, "the node entered after the lines")
                .default("null"),
        )
        .field(
            FieldDescription::new(
                "branch",
                FieldType::List(Box::new(FieldType::Shape("dialogue_branch"))),
                "after the lines, the first entry that holds; the last without a `when`",
            )
            .default("[]"),
        )
        .sample("{}")
        .roundtrip(node_round_trip),
        ComponentDescription::new(
            "dialogue_line",
            "A line: its text alone, or this object.",
        )
        .field(
            FieldDescription::new(
                "speaker",
                FieldType::Text,
                "its `speakers` entry, else the conversation's",
            )
            .default("null"),
        )
        .field(FieldDescription::new(
            "text",
            FieldType::Text,
            "at most 2,000 characters",
        ))
        .field(
            FieldDescription::new(
                "mood",
                FieldType::Text,
                "the speaker's portrait for it; `neutral` when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "when",
                FieldType::Shape("condition"),
                "shown only while it holds, the speaker the subject, the listener the other",
            )
            .default("null"),
        )
        .sample(r#"{ "text": "Evening." }"#)
        .roundtrip(line_round_trip),
        ComponentDescription::new("dialogue_choice", "A choice a node offers.")
            .field(FieldDescription::new(
                "text",
                FieldType::Text,
                "at most 2,000 characters",
            ))
            .field(
                FieldDescription::new(
                    "when",
                    FieldType::Shape("condition"),
                    "offered only while it holds",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "do",
                    FieldType::List(Box::new(FieldType::Shape("cue_action"))),
                    "run as it is taken, before its `next`",
                )
                .default("[]"),
            )
            .field(
                FieldDescription::new("next", FieldType::Text, "the node it enters; none ends")
                    .default("null"),
            )
            .field(
                FieldDescription::new(
                    "back",
                    FieldType::Bool,
                    "the choice Cancel takes; one a node",
                )
                .default("false"),
            )
            .sample(r#"{ "text": "Not tonight." }"#)
            .roundtrip(choice_round_trip),
        ComponentDescription::new("dialogue_branch", "A branch entry: the first that holds is entered.")
            .field(
                FieldDescription::new(
                    "when",
                    FieldType::Shape("condition"),
                    "taken when it holds; the last entry takes none",
                )
                .default("null"),
            )
            .field(FieldDescription::new("next", FieldType::Text, "the node entered"))
            .sample(r#"{ "next": "thanks" }"#)
            .roundtrip(round_trip::<BranchEntry>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

#[cfg(test)]
#[path = "../tests/dialogue.rs"]
mod tests;
