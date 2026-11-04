//! Named world cues: a `world_cues` entry is a list of cue actions a game
//! plays by name at a point, with no subject.

use std::sync::Arc;

use glam::Vec2;
use serde::Deserialize;
use tungsten_core::{EventQueue, World};

use crate::react::cues::CueAction;

/// The data kind of named world cues.
pub(crate) const WORLD_CUES: &str = "world_cues";

/// A named world cue played at a point: a thunderclap, a crate's break.
/// [`play`](Self::play) queues it; [`CuesPlugin`](crate::CuesPlugin)'s
/// system plays each after every other source in the step, in send order,
/// running the `world_cues` entry `name`'s actions at `at` with no subject:
/// `prefab` and `roll` spawn there with no source, `sound` and `shake` play
/// by the [`CueListener`](crate::CueListener), a `label` with a font floats
/// the amount, and `counter` and `flag` act on the game's own with
/// `target: game`. The load refuses the rest: `despawn`, `squash`, `flash`,
/// `effect`, `ability`, `loop`, `stop`, a style-less label, and a counter
/// or flag on a subject or the other. An unknown name plays nothing and
/// warns once; the trace writes a world cue under `Entity::PLACEHOLDER`.
///
/// The first play registers its queue, so a World that plays none holds
/// none.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{EventQueue, World};
/// use tungsten_kit::WorldCue;
///
/// let mut world = World::new();
/// assert!(!world.has_event::<WorldCue>());
/// WorldCue::new("thunder", Vec2::new(320.0, 40.0)).with_amount(3.0).play(&mut world);
/// let queued: Vec<&WorldCue> = world.resource::<EventQueue<WorldCue>>().iter().collect();
/// assert_eq!((queued[0].name.as_str(), queued[0].amount), ("thunder", Some(3.0)));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WorldCue {
    /// The `world_cues` entry it plays.
    pub name: String,
    /// Where it plays: a `prefab` spawns there, a `sound` and a `shake` are
    /// heard from there.
    pub at: Vec2,
    /// What a `label` floats.
    pub amount: Option<f32>,
}

impl WorldCue {
    /// The world cue `name` at `at`, with no amount.
    #[must_use]
    pub fn new(name: impl Into<String>, at: Vec2) -> Self {
        Self {
            name: name.into(),
            at,
            amount: None,
        }
    }

    /// The cue, with `amount`.
    #[must_use]
    pub fn with_amount(mut self, amount: f32) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Queues the cue for the cue system's next run, registering its queue
    /// at the first play.
    pub fn play(self, world: &mut World) {
        if !world.has_event::<WorldCue>() {
            world.register_event::<WorldCue>();
        }
        world.resource_mut::<EventQueue<WorldCue>>().send(self);
    }
}

/// A `world_cues` entry: its actions, in order, shared so a play takes them
/// without copying.
#[derive(Debug, Clone, Deserialize)]
#[serde(from = "Vec<CueAction>")]
pub(crate) struct WorldCueActions(pub(crate) Arc<[CueAction]>);

impl From<Vec<CueAction>> for WorldCueActions {
    fn from(actions: Vec<CueAction>) -> Self {
        Self(actions.into())
    }
}

#[cfg(test)]
#[path = "../tests/world_cues.rs"]
mod tests;
