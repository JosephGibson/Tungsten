//! The spells' components: the player's spellbook, and the spells and the
//! elements their slots show.

use serde::{Deserialize, Serialize};

use crate::features::toys::components::Element;

/// The player's spellbook, which its prefab names `spellbook`: the slot
/// selected, 0 for the first (key 1) to 9 for the tenth (key 0).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Spellbook {
    pub slot: usize,
}

/// A spell of the bar. Each casts the player's ability of [`Spell::ability`]'s
/// name, whose data holds its cost, cooldown, charges and payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spell {
    FireMissile,
    Flamethrower,
    IceSpray,
    ForcePush,
    BlackHole,
    IronBlock,
    BowlingBall,
    SmallBalls,
    Lightning,
    Summon,
}

impl Spell {
    /// Its ability's name in the player's `abilities`.
    #[must_use]
    pub const fn ability(self) -> &'static str {
        match self {
            Spell::FireMissile => "fire_missile",
            Spell::Flamethrower => "flamethrower",
            Spell::IceSpray => "ice_spray",
            Spell::ForcePush => "force_push",
            Spell::BlackHole => "black_hole",
            Spell::IronBlock => "iron_block",
            Spell::BowlingBall => "bowling_ball",
            Spell::SmallBalls => "small_balls",
            Spell::Lightning => "lightning",
            Spell::Summon => "summon",
        }
    }

    /// Its icon's sprite ID: the summon's first type's, which the bar
    /// follows to the type selected.
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Spell::FireMissile => "ex10_icon_fire_missile",
            Spell::Flamethrower => "ex10_icon_flamethrower",
            Spell::IceSpray => "ex10_icon_ice_spray",
            Spell::ForcePush => "ex10_icon_force_push",
            Spell::BlackHole => "ex10_icon_black_hole",
            Spell::IronBlock => "ex10_icon_iron_block",
            Spell::BowlingBall => "ex10_icon_bowling_ball",
            Spell::SmallBalls => "ex10_icon_small_balls",
            Spell::Lightning => "ex10_icon_lightning",
            Spell::Summon => super::SUMMONS[0].icon,
        }
    }

    /// The element its slot's border shows.
    #[must_use]
    pub const fn school(self) -> School {
        match self {
            Spell::FireMissile | Spell::Flamethrower => School::Fire,
            Spell::IceSpray => School::Cold,
            Spell::ForcePush | Spell::BlackHole | Spell::Summon => School::Mana,
            Spell::IronBlock | Spell::BowlingBall | Spell::SmallBalls => School::Physical,
            Spell::Lightning => School::Shock,
        }
    }

    /// Whether `cast` holds it, a use each fixed step, rather than
    /// pressing it once.
    #[must_use]
    pub const fn held(self) -> bool {
        matches!(
            self,
            Spell::Flamethrower | Spell::IceSpray | Spell::SmallBalls
        )
    }

    /// The spray it holds, for the two sprays.
    #[must_use]
    pub const fn spray(self) -> Option<Element> {
        match self {
            Spell::Flamethrower => Some(Element::Fire),
            Spell::IceSpray => Some(Element::Cold),
            _ => None,
        }
    }
}

/// A spell's element, as its slot's border shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum School {
    Fire,
    Cold,
    Mana,
    Physical,
    Shock,
}

impl School {
    /// Its border colour, from the palette: ember orange, ice cyan, indigo,
    /// bone white and violet.
    #[must_use]
    pub const fn color(self) -> [u8; 4] {
        match self {
            School::Fire => [246, 124, 40, 255],
            School::Cold => [176, 236, 244, 255],
            School::Mana => [132, 128, 236, 255],
            School::Physical => [242, 234, 200, 255],
            School::Shock => [184, 140, 255, 255],
        }
    }
}

/// The type the summon slot places, by its place in [`SUMMONS`]: a World
/// resource a run's start inserts, saved.
///
/// [`SUMMONS`]: super::SUMMONS
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summon {
    pub selected: usize,
}
