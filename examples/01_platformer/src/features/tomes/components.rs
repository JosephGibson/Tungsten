//! The tomes' components: a book's kind, the book on the map and the
//! tomes the player holds.

use serde::{Deserialize, Serialize};

use crate::features::audio::components::Sound;

/// One of the three tomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TomeKind {
    /// The demonic pact: a shield for half the maximum health.
    Pact,
    /// The tome of bounding: air jumps past the double jump, for mana.
    Bounding,
    /// The tome of arcane power: spells twice as strong and twice as dear.
    Arcane,
}

impl TomeKind {
    /// Every tome, in the HUD's art order.
    pub const ALL: [TomeKind; 3] = [TomeKind::Pact, TomeKind::Bounding, TomeKind::Arcane];

    /// Its place in [`ALL`](Self::ALL).
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            TomeKind::Pact => 0,
            TomeKind::Bounding => 1,
            TomeKind::Arcane => 2,
        }
    }

    /// Its book's prefab.
    #[must_use]
    pub const fn prefab(self) -> &'static str {
        match self {
            TomeKind::Pact => "tome_pact",
            TomeKind::Bounding => "tome_bounding",
            TomeKind::Arcane => "tome_arcane",
        }
    }

    /// Its entry's ID in the `tomes` data.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            TomeKind::Pact => "pact",
            TomeKind::Bounding => "bounding",
            TomeKind::Arcane => "arcane",
        }
    }

    /// Its pickup sound.
    #[must_use]
    pub const fn sound(self) -> Sound {
        match self {
            TomeKind::Pact => Sound::TomePactTaken,
            TomeKind::Bounding => Sound::TomeBoundingTaken,
            TomeKind::Arcane => Sound::TomeArcaneTaken,
        }
    }
}

/// A tome's texts and art, its entry in the `tomes` data
/// (`data/tomes/<id>.json`): its name, what it gives and what it costs on
/// the panel, its HUD tile, and the clip its book comes apart in and the
/// sparks that follow it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TomeData {
    pub name: String,
    pub gives: String,
    pub costs: String,
    pub icon: String,
    pub take_clip: String,
    pub take_sparks: String,
}

/// A book on the map, its prefab's `tome`: `{ "kind": "pact" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tome {
    pub kind: TomeKind,
}

/// The tomes the player holds, in the order taken: on the player from its
/// first take, saved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tomes(pub Vec<TomeKind>);

impl Tomes {
    /// Whether it holds `kind`.
    #[must_use]
    pub fn holds(&self, kind: TomeKind) -> bool {
        self.0.contains(&kind)
    }
}
