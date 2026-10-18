//! Loot tables: weighted draws of prefabs, the `loot_tables` data kind
//! that the `roll` cue action names.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use tungsten_core::{MissingAsset, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape,
    round_trip,
};
use crate::foundation::positive;
use crate::foundation::rng::KitRng;

/// The data kind of loot tables.
pub(crate) const LOOT_TABLES: &str = "loot_tables";

/// The most rolls a table draws.
const MAX_ROLLS: u32 = 64;

/// A weighted table: each of its rolls draws one entry, each entry's chance
/// its weight over the table's total, and an entry is a prefab or nothing.
/// The `roll` cue action ([`CueAction::roll`](crate::CueAction::roll))
/// draws one from the kit's random stream and spawns each prefab drawn:
/// what an enemy drops, what a chest holds.
///
/// Data form, an entry of the `loot_tables` kind or a `roll` action's table
/// in place:
/// `{ "rolls": 1, "entries": [{ "prefab": "gem", "weight": 3 }, { "weight": 1 }] }`,
/// `rolls` 1 by default and in `1..=64`, each weight finite and above 0, an
/// entry without `prefab` drawing nothing; the load checks each prefab ID.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{KitRng, WeightedTable};
///
/// let drops = WeightedTable::new().with_entry("gem", 3.0).with_nothing(1.0);
/// let mut rng = KitRng::seeded(7);
/// let gems = (0..1000).filter(|_| drops.draw(&mut rng) == Some("gem")).count();
/// assert!((700..800).contains(&gems), "about three in four: {gems}");
///
/// let read: WeightedTable = serde_json::from_str(
///     r#"{ "rolls": 2, "entries": [{ "prefab": "gem", "weight": 3 }, { "weight": 1 }] }"#,
/// )
/// .unwrap();
/// assert_eq!(read, drops.with_rolls(2));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "WeightedTableFields", into = "WeightedTableFields")]
pub struct WeightedTable {
    rolls: u32,
    entries: Vec<Entry>,
}

/// One entry: a prefab, or nothing, and its weight.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    prefab: Option<String>,
    weight: f32,
}

impl Default for WeightedTable {
    /// [`new`](Self::new): one roll, no entries.
    fn default() -> Self {
        Self::new()
    }
}

impl WeightedTable {
    /// A table of one roll and no entries, which draws nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rolls: 1,
            entries: Vec::new(),
        }
    }

    /// The table, with the prefab `prefab` drawn by `weight`.
    ///
    /// # Panics
    ///
    /// When `weight` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_entry(self, prefab: impl Into<String>, weight: f32) -> Self {
        self.push(Some(prefab.into()), weight, "with_entry")
    }

    /// The table, with nothing drawn by `weight`.
    ///
    /// # Panics
    ///
    /// When `weight` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_nothing(self, weight: f32) -> Self {
        self.push(None, weight, "with_nothing")
    }

    #[track_caller]
    fn push(mut self, prefab: Option<String>, weight: f32, method: &str) -> Self {
        assert!(
            weight.is_finite() && weight > 0.0,
            "WeightedTable::{method}: weight must be finite and > 0 (got {weight})"
        );
        self.entries.push(Entry { prefab, weight });
        self
    }

    /// The table, drawing `rolls` entries each time it is rolled.
    ///
    /// # Panics
    ///
    /// When `rolls` is outside `1..=64`.
    #[must_use]
    #[track_caller]
    pub fn with_rolls(mut self, rolls: u32) -> Self {
        assert!(
            (1..=MAX_ROLLS).contains(&rolls),
            "WeightedTable::with_rolls: rolls must be in 1..={MAX_ROLLS} (got {rolls})"
        );
        self.rolls = rolls;
        self
    }

    /// How many entries a roll of the table draws.
    #[must_use]
    pub fn rolls(&self) -> u32 {
        self.rolls
    }

    /// One draw: the prefab of the entry `rng` picks by weight, or `None`
    /// for an entry of nothing. A table with no entries draws nothing and
    /// takes no number from the stream; otherwise each draw takes one.
    pub fn draw(&self, rng: &mut KitRng) -> Option<&str> {
        let total: f64 = self
            .entries
            .iter()
            .map(|entry| f64::from(entry.weight))
            .sum();
        if total <= 0.0 {
            return None;
        }
        let mut left = f64::from(rng.unit()) * total;
        for entry in &self.entries {
            let weight = f64::from(entry.weight);
            if left < weight {
                return entry.prefab.as_deref();
            }
            left -= weight;
        }
        // Rounding can leave a sliver past the last entry.
        self.entries
            .last()
            .and_then(|entry| entry.prefab.as_deref())
    }

    /// The load's check: each entry's prefab ID, named by its field as
    /// written (`entries.1.prefab`).
    pub(crate) fn check(&self, world: &World) -> Result<(), MissingAsset> {
        for (i, entry) in self.entries.iter().enumerate() {
            if let Some(prefab) = &entry.prefab {
                world.check_prefab_id(&format!("entries.{i}.prefab"), prefab)?;
            }
        }
        Ok(())
    }
}

/// A table as data writes it, its numbers checked.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightedTableFields {
    #[serde(default = "one_roll", deserialize_with = "rolls")]
    rolls: u32,
    entries: Vec<WeightedEntryFields>,
}

/// An entry as data writes it.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightedEntryFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefab: Option<String>,
    #[serde(deserialize_with = "positive")]
    weight: f32,
}

fn one_roll() -> u32 {
    1
}

/// A roll count in `1..=64`, for `deserialize_with`.
fn rolls<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = u32::deserialize(deserializer)?;
    if (1..=MAX_ROLLS).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "must be in 1..={MAX_ROLLS} (got {value})"
        )))
    }
}

impl From<WeightedTableFields> for WeightedTable {
    fn from(fields: WeightedTableFields) -> Self {
        Self {
            rolls: fields.rolls,
            entries: fields
                .entries
                .into_iter()
                .map(|entry| Entry {
                    prefab: entry.prefab,
                    weight: entry.weight,
                })
                .collect(),
        }
    }
}

impl From<WeightedTable> for WeightedTableFields {
    fn from(table: WeightedTable) -> Self {
        Self {
            rolls: table.rolls,
            entries: table
                .entries
                .into_iter()
                .map(|entry| WeightedEntryFields {
                    prefab: entry.prefab,
                    weight: entry.weight,
                })
                .collect(),
        }
    }
}

/// Registers the `loot_tables` kind with its check of every prefab, and
/// its descriptions: the kind, `weighted_table` and `weighted_entry`.
pub(crate) fn register_loot_tables(world: &mut World) {
    world.register_data_kind_with_assets(LOOT_TABLES, |table: &WeightedTable, world: &World| {
        table.check(world)
    });
    describe_shape(
        world,
        ComponentDescription::new(
            "weighted_table",
            "Rolls that each draw one entry by weight: a prefab, or nothing.",
        )
        .field(
            FieldDescription::new(
                "rolls",
                FieldType::U32 {
                    min: Some(1),
                    max: Some(MAX_ROLLS),
                },
                "the entries one roll of the table draws",
            )
            .default("1"),
        )
        .field(FieldDescription::new(
            "entries",
            FieldType::List(Box::new(FieldType::Shape("weighted_entry"))),
            "what a draw may give, each by its weight over the total",
        ))
        .sample(r#"{ "entries": [] }"#)
        .roundtrip(round_trip::<WeightedTableFields>),
    );
    describe_shape(
        world,
        ComponentDescription::new("weighted_entry", "A prefab, or nothing, and its weight.")
            .field(
                FieldDescription::new(
                    "prefab",
                    FieldType::Prefab,
                    "spawned when drawn; the load checks the ID; nothing is drawn when left out",
                )
                .default("null"),
            )
            .field(FieldDescription::new(
                "weight",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "its share of the draws",
            ))
            .sample(r#"{ "weight": 1 }"#)
            .roundtrip(round_trip::<WeightedEntryFields>),
    );
    describe_kind(
        world,
        LOOT_TABLES,
        "A `weighted_table`: what a `roll` cue action draws and spawns.",
    );
}

#[cfg(test)]
#[path = "../tests/loot.rs"]
mod tests;
