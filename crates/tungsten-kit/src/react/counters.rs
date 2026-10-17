//! Counters and flags: named whole numbers and named switches, on an
//! entity or the game's own, that cues write and a game reads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tungsten_core::{Inspectable, Plugin, Schedule, SpawnContext, World};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::ensure_phases;
use crate::foundation::tags::valid_tag;

/// The scene components the plugin registers.
const COUNTERS: &str = "counters";
const FLAGS: &str = "flags";

/// The message of a name that fails [`valid_tag`].
fn name_error(name: &str) -> String {
    format!("`{name}` is not a counter or flag name (`[a-z][a-z0-9_]*` segments joined by dots)")
}

/// Named whole numbers: coins, kills, turns. A component, on an entity, and
/// a World resource, the game's own, which [`CountersPlugin`] inserts; a
/// cue's `counter` action adds to or sets either. A name follows the tag
/// rule, `[a-z][a-z0-9_]*` segments joined by dots, and needs no
/// declaration, since data writes it and the game reads it. A counter never
/// set reads 0, and adds saturate at the `i64` bounds.
///
/// Scene name `counters`: `{ "coins": 0 }`, the starting counts. It is
/// data: it serializes as that object.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Counters;
///
/// let mut counters = Counters::new().with("coins", 3);
/// assert_eq!(counters.add("coins", 2), 5);
/// assert_eq!(counters.get("keys"), 0, "never set");
/// counters.set("keys", 1);
/// assert_eq!(counters.iter().collect::<Vec<_>>(), [("coins", 5), ("keys", 1)]);
/// assert_eq!(
///     serde_json::to_value(&counters).unwrap(),
///     serde_json::json!({ "coins": 5, "keys": 1 })
/// );
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, i64>", into = "BTreeMap<String, i64>")]
pub struct Counters {
    /// Sorted by name.
    counts: Vec<(String, i64)>,
}

impl TryFrom<BTreeMap<String, i64>> for Counters {
    type Error = String;

    fn try_from(counts: BTreeMap<String, i64>) -> Result<Self, Self::Error> {
        if let Some(name) = counts.keys().find(|name| !valid_tag(name)) {
            return Err(name_error(name));
        }
        Ok(Self {
            counts: counts.into_iter().collect(),
        })
    }
}

impl From<Counters> for BTreeMap<String, i64> {
    fn from(counters: Counters) -> Self {
        counters.counts.into_iter().collect()
    }
}

impl Counters {
    /// No counts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The counters with `name` at `value`.
    ///
    /// # Panics
    ///
    /// When `name` is not a counter name.
    #[must_use]
    #[track_caller]
    pub fn with(mut self, name: &str, value: i64) -> Self {
        self.set(name, value);
        self
    }

    /// `name`'s count; 0 when never set.
    #[must_use]
    pub fn get(&self, name: &str) -> i64 {
        self.search(name).map_or(0, |at| self.counts[at].1)
    }

    /// Adds `by` to `name`, saturating at the `i64` bounds; the new count.
    ///
    /// # Panics
    ///
    /// When `name` is not a counter name.
    #[track_caller]
    pub fn add(&mut self, name: &str, by: i64) -> i64 {
        let slot = self.slot(name);
        *slot = slot.saturating_add(by);
        *slot
    }

    /// Sets `name` to `value`.
    ///
    /// # Panics
    ///
    /// When `name` is not a counter name.
    #[track_caller]
    pub fn set(&mut self, name: &str, value: i64) {
        *self.slot(name) = value;
    }

    /// Each count, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, i64)> {
        self.counts
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
    }

    fn search(&self, name: &str) -> Result<usize, usize> {
        self.counts
            .binary_search_by(|(known, _)| known.as_str().cmp(name))
    }

    #[track_caller]
    fn slot(&mut self, name: &str) -> &mut i64 {
        assert!(valid_tag(name), "Counters: {}", name_error(name));
        let at = match self.search(name) {
            Ok(at) => at,
            Err(at) => {
                self.counts.insert(at, (name.to_owned(), 0));
                at
            }
        };
        &mut self.counts[at].1
    }
}

/// Named switches: `door.locked`, `has_key`. A component, on an entity, and
/// a World resource, the game's own, which [`CountersPlugin`] inserts; a
/// cue's `flag` action sets or clears one on either. Names follow
/// [`Counters`]' rule and need no declaration.
///
/// Scene name `flags`: `["door.locked"]`, the flags set at the start. It is
/// data: it serializes as that list.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Flags;
///
/// let mut flags = Flags::new().with("door.locked");
/// assert!(flags.has("door.locked") && !flags.has("has_key"));
/// assert!(flags.set("has_key", true), "it changed");
/// assert!(!flags.set("has_key", true), "already set");
/// assert!(flags.set("door.locked", false));
/// assert_eq!(flags.iter().collect::<Vec<_>>(), ["has_key"]);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Flags {
    /// Sorted.
    names: Vec<String>,
}

impl TryFrom<Vec<String>> for Flags {
    type Error = String;

    fn try_from(mut names: Vec<String>) -> Result<Self, Self::Error> {
        if let Some(name) = names.iter().find(|name| !valid_tag(name)) {
            return Err(name_error(name));
        }
        names.sort();
        names.dedup();
        Ok(Self { names })
    }
}

impl From<Flags> for Vec<String> {
    fn from(flags: Flags) -> Self {
        flags.names
    }
}

impl Flags {
    /// No flag set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The flags with `name` set.
    ///
    /// # Panics
    ///
    /// When `name` is not a flag name.
    #[must_use]
    #[track_caller]
    pub fn with(mut self, name: &str) -> Self {
        self.set(name, true);
        self
    }

    /// Whether `name` is set.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.search(name).is_ok()
    }

    /// Sets `name` when `on`, clears it otherwise; whether that changed it.
    ///
    /// # Panics
    ///
    /// When `name` is not a flag name.
    #[track_caller]
    pub fn set(&mut self, name: &str, on: bool) -> bool {
        assert!(valid_tag(name), "Flags: {}", name_error(name));
        match (self.search(name), on) {
            (Ok(_), true) | (Err(_), false) => false,
            (Err(at), true) => {
                self.names.insert(at, name.to_owned());
                true
            }
            (Ok(at), false) => {
                self.names.remove(at);
                true
            }
        }
    }

    /// The flags set, by name.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    fn search(&self, name: &str) -> Result<usize, usize> {
        self.names
            .binary_search_by(|known| known.as_str().cmp(name))
    }
}

/// Registers the `counters` and `flags` scene components and inserts the
/// World's own [`Counters`] and [`Flags`] when it has none: the game's,
/// which a cue writes with `"target": "game"`.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{Counters, CountersPlugin, Flags};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// CountersPlugin.build(&mut schedule, &mut world);
/// world.resource_mut::<Counters>().add("score", 10);
/// assert_eq!(world.resource::<Counters>().get("score"), 10);
/// assert!(!world.resource::<Flags>().has("boss.seen"));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct CountersPlugin;

impl Plugin for CountersPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Counters>(world, "Counters");
        register_rows::<Flags>(world, "Flags");
        describe(
            world,
            ComponentDescription::new(
                COUNTERS,
                "Named whole numbers on the entity, which cues add to or set.",
            )
            .field(FieldDescription::new(
                "counts",
                FieldType::Object("counter name → i64"),
                "the value itself: the starting counts; names are dotted, undeclared",
            ))
            .genre(TOP_DOWN)
            .genre(GRID)
            .sample(r#"{ "coins": 0 }"#)
            .roundtrip(round_trip::<Counters>),
        );
        describe(
            world,
            ComponentDescription::new(
                FLAGS,
                "Named switches on the entity, which cues set or clear.",
            )
            .field(FieldDescription::new(
                "names",
                FieldType::List(Box::new(FieldType::Text)),
                "the value itself: the flags set at the start; names are dotted, undeclared",
            ))
            .genre(PLATFORMER)
            .genre(GRID)
            .sample(r#"["door.locked"]"#)
            .roundtrip(round_trip::<Flags>),
        );
        if !world.has_resource::<Counters>() {
            world.insert_resource(Counters::new());
        }
        if !world.has_resource::<Flags>() {
            world.insert_resource(Flags::new());
        }
        world.register_component_with(COUNTERS, |counters: Counters, _: &SpawnContext<'_>| {
            (counters,)
        });
        world.register_component_with(FLAGS, |flags: Flags, _: &SpawnContext<'_>| (flags,));
    }
}

impl Inspectable for Counters {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        self.counts
            .iter()
            .map(|(name, value)| ("counter", format!("{name} = {value}")))
            .collect()
    }
}

impl Inspectable for Flags {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![("flags", self.names.join(", "))]
    }
}

#[cfg(test)]
#[path = "../tests/counters.rs"]
mod tests;
