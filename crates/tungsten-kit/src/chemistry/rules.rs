//! The `interactions` kind: a rule matching a stimulus against what
//! receives it, by its tags and the stimulus's amount, to a list of
//! outputs.

use serde::{Deserialize, Serialize};
use tungsten_core::{DataRegistry, MissingAsset, World};

use crate::chemistry::outputs::{Output, check_outputs};
use crate::damage::kinds::check_type;
use crate::foundation::tags::{TagFilter, Tags, check_declared, covers, valid_tag};

/// The data kind of chemistry's rules.
pub(crate) const INTERACTIONS: &str = "interactions";

/// One chemistry rule, an entry of the `interactions` kind: when a stimulus
/// named `stimulus` (or one under it) of at least `min` reaches a receiver
/// whose tags pass `on`, its `outputs` run on the receiver, in order. For a
/// stimulus and a receiver the most specific passing rule runs alone: the
/// deepest stimulus name, then the most dotted segments in `on`'s `all`,
/// then the lowest entry ID.
///
/// A stimulus is a damage type, whose landed parts stimulate by their
/// amount after the resistances, or a declared tag a game names, such as
/// `heat`; the kit declares `stimulus.impact`, `stimulus.light` and
/// `stimulus.blast` for its own.
///
/// Data form:
///
/// ```json
/// {
///   "stimulus": "elemental.fire", "on": { "all": ["material.wood"] }, "min": 0.5,
///   "outputs": [{ "build": { "status": "burning", "scale": 1 } }]
/// }
/// ```
///
/// `on` passes everything when left out and `min` is 0; the load checks the
/// stimulus (a damage type or a declared tag), the filter's tags and every
/// status, prefab and handler the outputs name.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Interaction;
///
/// let rule: Interaction = serde_json::from_value(serde_json::json!({
///     "stimulus": "heat", "on": { "all": ["material.wood"] },
///     "outputs": [{ "remove": "frozen" }]
/// }))
/// .unwrap();
/// assert_eq!((rule.stimulus(), rule.min(), rule.outputs().len()), ("heat", 0.0, 1));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "InteractionDef", into = "InteractionDef")]
pub struct Interaction {
    stimulus: String,
    on: TagFilter,
    min: f32,
    outputs: Vec<Output>,
}

impl Interaction {
    /// The stimulus it answers, and every one under it.
    #[must_use]
    pub fn stimulus(&self) -> &str {
        &self.stimulus
    }

    /// The filter a receiver's tags pass.
    #[must_use]
    pub fn on(&self) -> &TagFilter {
        &self.on
    }

    /// The least amount it answers.
    #[must_use]
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Its outputs, in order.
    #[must_use]
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// Whether it answers `stimulus` of `amount` on a receiver with `tags`.
    pub(crate) fn passes(&self, stimulus: &str, amount: f64, tags: Option<&Tags>) -> bool {
        covers(stimulus, &self.stimulus) && amount >= f64::from(self.min) && self.on.matches(tags)
    }

    /// How specific it is, the larger the more: its stimulus's depth, then
    /// the dotted segments of its filter's `all`.
    pub(crate) fn specificity(&self) -> (usize, usize) {
        (self.stimulus.split('.').count(), self.on.all_segments())
    }

    /// The load's check: the stimulus, the filter and the outputs.
    pub(crate) fn check(&self, world: &World) -> Result<(), MissingAsset> {
        check_stimulus(world, "stimulus", &self.stimulus)?;
        self.on.check(world, "on")?;
        check_outputs(world, "outputs", &self.outputs)
    }
}

/// The load's check of a stimulus name at `field`: a damage type, else a
/// declared tag.
pub(crate) fn check_stimulus(world: &World, field: &str, name: &str) -> Result<(), MissingAsset> {
    if check_type(world, field, name, true).is_ok() {
        return Ok(());
    }
    check_declared(world, field, name)
}

/// A rule as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractionDef {
    stimulus: String,
    #[serde(default, skip_serializing_if = "TagFilter::is_everything")]
    on: TagFilter,
    #[serde(default, deserialize_with = "crate::foundation::non_negative")]
    min: f32,
    outputs: Vec<Output>,
}

impl TryFrom<InteractionDef> for Interaction {
    type Error = String;

    fn try_from(def: InteractionDef) -> Result<Self, Self::Error> {
        if !valid_tag(&def.stimulus) {
            return Err(format!(
                "stimulus: `{}` is not a stimulus name (`[a-z][a-z0-9_]*` segments joined by dots)",
                def.stimulus
            ));
        }
        Ok(Self {
            stimulus: def.stimulus,
            on: def.on,
            min: def.min,
            outputs: def.outputs,
        })
    }
}

impl From<Interaction> for InteractionDef {
    fn from(rule: Interaction) -> Self {
        Self {
            stimulus: rule.stimulus,
            on: rule.on,
            min: rule.min,
            outputs: rule.outputs,
        }
    }
}

/// Registers the `interactions` kind with its load check.
pub(crate) fn register_interactions(world: &mut World) {
    world.register_data_kind_with_assets(INTERACTIONS, |rule: &Interaction, world: &World| {
        rule.check(world)
    });
}

/// The World's rules, by ID: a run's snapshot, so its outputs run while the
/// World changes.
pub(crate) fn snapshot(world: &World, into: &mut Vec<(String, Interaction)>) {
    into.clear();
    let Some(data) = world.get_resource::<DataRegistry>() else {
        return;
    };
    for id in data.ids(INTERACTIONS) {
        if let Some(rule) = data.get::<Interaction>(id) {
            into.push((id.to_owned(), rule.clone()));
        }
    }
}

/// Whether the World holds an `interactions` entry.
pub(crate) fn any_rule(world: &World) -> bool {
    world
        .get_resource::<DataRegistry>()
        .is_some_and(|data| data.ids(INTERACTIONS).next().is_some())
}

/// Whether some rule of the World's answers `stimulus`: it names it or an
/// ancestor of it. It reads the rules in place, cloning none.
pub(crate) fn answers(world: &World, stimulus: &str) -> bool {
    rules_of(world).any(|rule| covers(stimulus, &rule.stimulus))
}

/// The World's rules in place, in ID order.
pub(crate) fn rules_of(world: &World) -> impl Iterator<Item = &Interaction> + '_ {
    world
        .get_resource::<DataRegistry>()
        .into_iter()
        .flat_map(|data| {
            data.ids(INTERACTIONS)
                .filter_map(move |id| data.get::<Interaction>(id))
        })
}

/// Whether some rule of `rules` answers `stimulus`: it names it or an
/// ancestor of it.
pub(crate) fn answered(rules: &[(String, Interaction)], stimulus: &str) -> bool {
    rules
        .iter()
        .any(|(_, rule)| covers(stimulus, &rule.stimulus))
}

/// The rule of `rules` that runs for `stimulus` of `amount` on a receiver
/// with `tags`: the most specific passing one, the lowest ID among equals
/// (`rules` is sorted by ID).
pub(crate) fn winner<'r>(
    rules: &'r [(String, Interaction)],
    stimulus: &str,
    amount: f64,
    tags: Option<&Tags>,
) -> Option<&'r (String, Interaction)> {
    let mut best: Option<&(String, Interaction)> = None;
    for entry in rules {
        if !entry.1.passes(stimulus, amount, tags) {
            continue;
        }
        if best.is_none_or(|(_, held)| entry.1.specificity() > held.specificity()) {
            best = Some(entry);
        }
    }
    best
}
