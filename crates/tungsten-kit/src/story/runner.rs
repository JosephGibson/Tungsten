//! The conversation runner (redo 8.1, P8-1): calls, not a system, since a
//! dialogue box covers gameplay and the game clock holds under it; each
//! call walks the talk at once and runs its lists then.

use glam::Vec2;
use tungsten_core::{DataRegistry, Entity, EventQueue, World};

use crate::motion::drive::centre_of;
use crate::react::cues::run_list;
use crate::story::dialogue::{Conversation, MAX_JUMPS, Speaker};

/// The most nodes one call enters: past them the talk ends with a warning,
/// a bound the parse's jump check keeps checked data from reaching.
pub(crate) const MAX_ENTRIES: usize = MAX_JUMPS + 1;

/// The running conversation, a World resource the first [`talk`] inserts:
/// at most one [`Talk`] at a time. Each call walks it at once: a node's
/// `do` runs as it is entered, then its first line whose `when` holds
/// shows; past its lines, its choices whose `when` holds show, its `next`
/// or first holding `branch` entry is entered, or the talk ends. A list
/// runs at the call, so a flag or a counter it writes reads back in the
/// next condition while the game clock holds; a spawn or a despawn lands at
/// the frame's flush, and an effect at the next fixed step, once play
/// resumes. Not saved: a restore ends a running talk without its lists.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRegistry, Plugin, Schedule, World};
/// use tungsten_kit::{Conversation, Dialogue, DialoguePlugin, Speaker, talk};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// DialoguePlugin.build(&mut schedule, &mut world);
/// let chat: Conversation = serde_json::from_str(
///     r#"{ "speaker": "keeper", "start": "hi", "nodes": {
///         "hi": { "lines": ["Evening.", "Cold one."], "choices": [{ "text": "Goodnight." }] } } }"#,
/// )
/// .unwrap();
/// let data = world.resource_mut::<DataRegistry>();
/// data.insert("keeper", Speaker::new("Old Tamsin"));
/// data.insert("keeper.evening", chat);
/// let keeper = world.spawn();
/// let warden = world.spawn();
///
/// assert!(talk(&mut world, "keeper.evening", keeper, warden));
/// let line = |world: &World| {
///     let talk = world.resource::<Dialogue>().current().unwrap();
///     talk.line().map(|line| (line.name().to_owned(), line.text().to_owned()))
/// };
/// assert_eq!(line(&world), Some(("Old Tamsin".into(), "Evening.".into())));
/// assert!(Dialogue::advance(&mut world));
/// assert_eq!(line(&world).unwrap().1, "Cold one.");
/// assert!(Dialogue::advance(&mut world));
/// let talk = world.resource::<Dialogue>().current().unwrap();
/// assert_eq!(talk.choices().collect::<Vec<_>>(), [(0, "Goodnight.")]);
/// assert!(Dialogue::choose(&mut world, 0));
/// assert!(world.resource::<Dialogue>().current().is_none(), "no `next`: the end");
/// ```
#[derive(Debug, Default)]
pub struct Dialogue {
    talk: Option<Talk>,
}

/// One conversation under way: who speaks to whom, the node, the line
/// shown or the choices offered.
#[derive(Debug, Clone)]
pub struct Talk {
    id: String,
    conversation: Conversation,
    speaker: Entity,
    listener: Entity,
    node: String,
    /// The shown line's index in its node.
    line: Option<usize>,
    shown: Option<TalkLine>,
    /// The offered choices' indexes in their node.
    choices: Vec<usize>,
}

/// The line a talk shows: its speaker's ID and name, its text, the
/// portrait its mood picks and the speaker's reveal pace.
#[derive(Debug, Clone, PartialEq)]
pub struct TalkLine {
    speaker: String,
    name: String,
    text: String,
    portrait: Option<String>,
    pace: f32,
}

impl TalkLine {
    /// The speaker's `speakers` ID.
    #[must_use]
    pub fn speaker(&self) -> &str {
        &self.speaker
    }

    /// The speaker's name, shown; its ID when no entry holds it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The line's text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The portrait sprite the line's mood picks, if the speaker has one.
    #[must_use]
    pub fn portrait(&self) -> Option<&str> {
        self.portrait.as_deref()
    }

    /// Characters a second a box reveals it at.
    #[must_use]
    pub fn pace(&self) -> f32 {
        self.pace
    }
}

impl Talk {
    /// The conversation's `dialogue` ID, `inline` for one written in place.
    #[must_use]
    pub fn conversation(&self) -> &str {
        &self.id
    }

    /// Who speaks: a condition's and a list's subject.
    #[must_use]
    pub fn speaker(&self) -> Entity {
        self.speaker
    }

    /// Who listens: a condition's and a list's other entity.
    #[must_use]
    pub fn listener(&self) -> Entity {
        self.listener
    }

    /// The node it is at.
    #[must_use]
    pub fn node(&self) -> &str {
        &self.node
    }

    /// The line shown; `None` while choices are offered.
    #[must_use]
    pub fn line(&self) -> Option<&TalkLine> {
        self.shown.as_ref()
    }

    /// The choices offered, each its index in its node and its text; none
    /// while a line shows.
    pub fn choices(&self) -> impl Iterator<Item = (usize, &str)> {
        let node = self.conversation.node(&self.node);
        self.choices.iter().filter_map(move |&index| {
            node.and_then(|node| node.choices.get(index))
                .map(|choice| (index, choice.text.as_str()))
        })
    }

    /// Whether choices are offered.
    #[must_use]
    pub fn choosing(&self) -> bool {
        !self.choices.is_empty()
    }

    /// The node's choice that Cancel takes, when it is offered.
    #[must_use]
    pub fn back(&self) -> Option<usize> {
        let node = self.conversation.node(&self.node)?;
        self.choices
            .iter()
            .copied()
            .find(|&index| node.choices.get(index).is_some_and(|choice| choice.back))
    }
}

/// A conversation began, sent by [`talk`] and the `dialogue` cue action;
/// a plain event, no cue event, since a cue under a box would run only
/// once it closed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DialogueStarted {
    /// Who speaks.
    pub speaker: Entity,
    /// Who listens.
    pub listener: Entity,
    /// The `dialogue` ID, `inline` for one written in place.
    pub conversation: String,
}

/// A conversation ended: its last exit, [`Dialogue::end`], a gone speaker
/// or listener, the entry bound or a restore.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DialogueEnded {
    /// Who spoke.
    pub speaker: Entity,
    /// Who listened.
    pub listener: Entity,
    /// The `dialogue` ID, `inline` for one written in place.
    pub conversation: String,
}

/// Starts `dialogue` entry `id` with `speaker` speaking to `listener`:
/// refused, returning `false`, while a talk runs, for an unknown ID or a
/// dead entity; else it enters the conversation's `start` at once.
pub fn talk(world: &mut World, id: &str, speaker: Entity, listener: Entity) -> bool {
    let Some(conversation) = world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<Conversation>(id))
        .cloned()
    else {
        log::warn!("talk: no `dialogue` entry `{id}`");
        return false;
    };
    start_talk(world, id.to_owned(), conversation, speaker, listener)
}

/// Whether a talk runs.
pub(crate) fn talking(world: &World) -> bool {
    world
        .get_resource::<Dialogue>()
        .is_some_and(|dialogue| dialogue.talk.is_some())
}

/// Starts `conversation`, known as `id`, as [`talk`] does.
pub(crate) fn start_talk(
    world: &mut World,
    id: String,
    conversation: Conversation,
    speaker: Entity,
    listener: Entity,
) -> bool {
    if talking(world) || !world.is_alive(speaker) || !world.is_alive(listener) {
        return false;
    }
    let start = conversation.start().to_owned();
    if !world.has_resource::<Dialogue>() {
        world.insert_resource(Dialogue::default());
    }
    send(
        world,
        DialogueStarted {
            speaker,
            listener,
            conversation: id.clone(),
        },
    );
    world.resource_mut::<Dialogue>().talk = Some(Talk {
        id,
        conversation,
        speaker,
        listener,
        node: start.clone(),
        line: None,
        shown: None,
        choices: Vec::new(),
    });
    walk(world, Walk::Enter(start));
    true
}

impl Dialogue {
    /// The running talk.
    #[must_use]
    pub fn current(&self) -> Option<&Talk> {
        self.talk.as_ref()
    }

    /// Shows the next line whose `when` holds, else goes past the node's
    /// lines: its choices, its `next` or `branch`, or the end. `false`,
    /// doing nothing, with no talk or while choices are offered; a talk
    /// whose speaker or listener is gone ends instead.
    pub fn advance(world: &mut World) -> bool {
        if !live(world) {
            return false;
        }
        let Some(talk) = current(world) else {
            return false;
        };
        if talk.choosing() {
            return false;
        }
        let from = talk.line.map_or(0, |line| line + 1);
        if !show_line(world, from) {
            walk(world, Walk::PastLines);
        }
        true
    }

    /// Takes the offered choice at `index` in its node: its `do`, then its
    /// `next`, or the end. `false`, doing nothing, with no talk or for a
    /// choice not offered; a talk whose speaker or listener is gone ends
    /// instead.
    pub fn choose(world: &mut World, index: usize) -> bool {
        if !live(world) {
            return false;
        }
        let Some(talk) = current(world) else {
            return false;
        };
        if !talk.choices.contains(&index) {
            return false;
        }
        let Some(choice) = talk
            .conversation
            .node(&talk.node)
            .and_then(|node| node.choices.get(index))
            .cloned()
        else {
            return false;
        };
        let (speaker, listener) = (talk.speaker, talk.listener);
        if let Some(talk) = talk_mut(world) {
            talk.choices.clear();
        }
        run_list(
            world,
            &choice.actions,
            Some(speaker),
            Some(listener),
            point(world, speaker),
        );
        match choice.next {
            Some(next) => walk(world, Walk::Enter(next)),
            None => finish(world),
        }
        true
    }

    /// Ends the running talk at once, without its lists.
    pub fn end(world: &mut World) {
        finish(world);
    }
}

/// Where a walk goes on from.
enum Walk {
    /// Into a node.
    Enter(String),
    /// Past the current node's lines.
    PastLines,
}

/// The running talk.
fn current(world: &World) -> Option<&Talk> {
    world.get_resource::<Dialogue>()?.talk.as_ref()
}

fn talk_mut(world: &mut World) -> Option<&mut Talk> {
    world.get_resource_mut::<Dialogue>()?.talk.as_mut()
}

/// Where `entity`'s lists happen: its centre, else the origin.
fn point(world: &World, entity: Entity) -> Vec2 {
    centre_of(world, entity).unwrap_or(Vec2::ZERO)
}

/// Whether the talk's speaker and listener live; a talk whose either is
/// gone ends here.
fn live(world: &mut World) -> bool {
    let Some(talk) = current(world) else {
        return false;
    };
    if world.is_alive(talk.speaker) && world.is_alive(talk.listener) {
        return true;
    }
    finish(world);
    false
}

/// Walks the talk from `step` until it shows a line, offers choices or
/// ends, entering at most [`MAX_ENTRIES`] nodes.
fn walk(world: &mut World, mut step: Walk) {
    let mut entered = 0;
    loop {
        match step {
            Walk::Enter(name) => {
                if entered == MAX_ENTRIES {
                    let id = current(world).map_or("", |talk| talk.id.as_str());
                    log::warn!(
                        "dialogue `{id}`: one call entered {MAX_ENTRIES} nodes that showed \\
                         nothing; the talk ends"
                    );
                    finish(world);
                    return;
                }
                entered += 1;
                let Some(talk) = talk_mut(world) else {
                    return;
                };
                talk.node = name;
                talk.line = None;
                talk.shown = None;
                talk.choices.clear();
                let (speaker, listener) = (talk.speaker, talk.listener);
                let actions = talk
                    .conversation
                    .node(&talk.node)
                    .map(|node| node.actions.clone())
                    .unwrap_or_default();
                run_list(
                    world,
                    &actions,
                    Some(speaker),
                    Some(listener),
                    point(world, speaker),
                );
                if current(world).is_none() || show_line(world, 0) {
                    return;
                }
                step = Walk::PastLines;
            }
            Walk::PastLines => {
                let Some(talk) = current(world) else {
                    return;
                };
                let (speaker, listener) = (Some(talk.speaker), Some(talk.listener));
                let Some(node) = talk.conversation.node(&talk.node) else {
                    finish(world);
                    return;
                };
                if !node.choices.is_empty() {
                    let offered: Vec<usize> = node
                        .choices
                        .iter()
                        .enumerate()
                        .filter(|(_, choice)| {
                            choice
                                .when
                                .as_ref()
                                .is_none_or(|when| when.holds(world, speaker, listener))
                        })
                        .map(|(index, _)| index)
                        .collect();
                    if let Some(talk) = talk_mut(world) {
                        talk.line = None;
                        talk.shown = None;
                        talk.choices = offered;
                    }
                    return;
                }
                let next = node.next.clone().or_else(|| {
                    node.branch
                        .iter()
                        .find(|entry| {
                            entry
                                .when
                                .as_ref()
                                .is_none_or(|when| when.holds(world, speaker, listener))
                        })
                        .map(|entry| entry.next.clone())
                });
                let Some(next) = next else {
                    finish(world);
                    return;
                };
                step = Walk::Enter(next);
            }
        }
    }
}

/// Shows the current node's first line from index `from` whose `when`
/// holds; `false` when none is left.
fn show_line(world: &mut World, from: usize) -> bool {
    let Some(talk) = current(world) else {
        return false;
    };
    let (speaker, listener) = (Some(talk.speaker), Some(talk.listener));
    let Some(node) = talk.conversation.node(&talk.node) else {
        return false;
    };
    let Some((index, line)) = node.lines.iter().enumerate().skip(from).find(|(_, line)| {
        line.when
            .as_ref()
            .is_none_or(|when| when.holds(world, speaker, listener))
    }) else {
        return false;
    };
    let id = line
        .speaker
        .clone()
        .unwrap_or_else(|| talk.conversation.speaker().to_owned());
    let entry = world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<Speaker>(&id));
    let shown = TalkLine {
        name: entry.map_or_else(|| id.clone(), |entry| entry.name().to_owned()),
        text: line.text.clone(),
        portrait: entry
            .and_then(|entry| entry.portrait(line.mood.as_deref()))
            .map(str::to_owned),
        pace: entry.map_or(40.0, Speaker::pace),
        speaker: id,
    };
    if let Some(talk) = talk_mut(world) {
        talk.line = Some(index);
        talk.shown = Some(shown);
        talk.choices.clear();
    }
    true
}

/// Ends the running talk, sending [`DialogueEnded`].
fn finish(world: &mut World) {
    let Some(talk) = world
        .get_resource_mut::<Dialogue>()
        .and_then(|dialogue| dialogue.talk.take())
    else {
        return;
    };
    send(
        world,
        DialogueEnded {
            speaker: talk.speaker,
            listener: talk.listener,
            conversation: talk.id,
        },
    );
}

/// Sends `event` when its queue is registered.
fn send<E: 'static>(world: &mut World, event: E) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<E>>() {
        queue.send(event);
    }
}

/// The restore hook: a restore ends a running talk without its lists, so
/// a box over it closes.
pub(crate) fn end_on_restore(world: &mut World) {
    finish(world);
}

#[cfg(test)]
#[path = "../tests/runner.rs"]
mod tests;
