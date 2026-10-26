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

    /// Its name on the panel.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            TomeKind::Pact => "The Demonic Pact",
            TomeKind::Bounding => "The Tome of Bounding",
            TomeKind::Arcane => "The Tome of Arcane Power",
        }
    }

    /// What it gives, on the panel.
    #[must_use]
    pub const fn gives(self) -> &'static str {
        match self {
            TomeKind::Pact => "A shield of 50 over your health, back 3 s after a hit",
            TomeKind::Bounding => "Jump again and again in the air",
            TomeKind::Arcane => "Spells deal twice the damage and build twice the ailments",
        }
    }

    /// What it costs, on the panel.
    #[must_use]
    pub const fn costs(self) -> &'static str {
        match self {
            TomeKind::Pact => "Half your maximum health, for good",
            TomeKind::Bounding => "25 mana for each jump past the second",
            TomeKind::Arcane => "Spells cost twice the mana",
        }
    }

    /// Its HUD tile.
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            TomeKind::Pact => "ex10_icon_tome_pact",
            TomeKind::Bounding => "ex10_icon_tome_bounding",
            TomeKind::Arcane => "ex10_icon_tome_arcane",
        }
    }

    /// The clip its book comes apart in, and the sparks that follow it.
    #[must_use]
    pub const fn take_clip(self) -> &'static str {
        match self {
            TomeKind::Pact => "ex10_tome_pact_take",
            TomeKind::Bounding => "ex10_tome_bounding_take",
            TomeKind::Arcane => "ex10_tome_arcane_take",
        }
    }

    #[must_use]
    pub const fn take_sparks(self) -> &'static str {
        match self {
            TomeKind::Pact => "ex10_tome_pact_take_sparks",
            TomeKind::Bounding => "ex10_tome_bounding_take_sparks",
            TomeKind::Arcane => "ex10_tome_arcane_take_sparks",
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

/// A book on the map, its prefab's `tome`: `{ "kind": "pact" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tome {
    pub kind: TomeKind,
}

/// The tomes the player holds, in the order taken: on the player from its
/// first take, saved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tomes(pub Vec<TomeKind>);

impl Tomes {
    /// Whether it holds `kind`.
    #[must_use]
    pub fn holds(&self, kind: TomeKind) -> bool {
        self.0.contains(&kind)
    }
}
