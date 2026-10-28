//! The spells' components: the player's spellbook, and the spells and the
//! elements their slots show.

use serde::{Deserialize, Serialize};

use crate::features::player::Gesture;
use crate::features::toys::components::Element;

/// The player's spellbook, which its prefab names `spellbook`: the slot
/// selected, 0 for the first (key 1) to 10 for the eleventh (the minus
/// key).
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
    ToxicFlask,
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
            Spell::ToxicFlask => "toxic_flask",
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
            Spell::ToxicFlask => "ex10_icon_toxic_flask",
            Spell::Summon => super::SUMMONS[0].icon,
        }
    }

    /// The gesture the wizard casts it with: a toss, a bolt of the staff,
    /// a push of the hand, a conjuring strike, or, held, the staff levelled
    /// along the aim.
    #[must_use]
    pub const fn gesture(self) -> Gesture {
        match self {
            Spell::BowlingBall | Spell::ToxicFlask => Gesture::Toss,
            Spell::FireMissile | Spell::Lightning => Gesture::Bolt,
            Spell::ForcePush | Spell::BlackHole => Gesture::Push,
            Spell::IronBlock | Spell::Summon => Gesture::Conjure,
            Spell::Flamethrower | Spell::IceSpray | Spell::SmallBalls => Gesture::Pour,
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

/// An element and its colour, as the bar's borders showed them before the
/// icon family drew each element into its tile (01 (h)); the damage
/// numbers' tests read the colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum School {
    Fire,
    Cold,
    Mana,
    Physical,
    Shock,
    Poison,
}

impl School {
    /// Its border colour, from the palette: ember orange, ice cyan, indigo,
    /// bone white, violet and poison green.
    #[must_use]
    pub const fn color(self) -> [u8; 4] {
        match self {
            School::Fire => [246, 124, 40, 255],
            School::Cold => [176, 236, 244, 255],
            School::Mana => [132, 128, 236, 255],
            School::Physical => [242, 234, 200, 255],
            School::Shock => [184, 140, 255, 255],
            School::Poison => [132, 214, 64, 255],
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

/// A held spell's refusals since the cast was pressed: a World resource a
/// run's start inserts, both cleared at the release. `mana`: its ability
/// refused it for mana, and it stays off, so a spray at empty mana does not
/// restart each step the regeneration pays for (I2). `said`: a refusal was
/// said, so a held spell says one a press.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Refused {
    pub mana: bool,
    pub said: bool,
}

/// Why a cast was refused, which `spells::systems::refuse_cast` says: too
/// little mana (`Used::NoCost`), a cooldown or no charge (`Used::NotReady`
/// or `NoCharge`), or no room for its toy (at its cap, a summon or a block
/// that does not fit, no cursor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NoMana,
    NotReady,
    NoRoom,
}

impl Refusal {
    /// The refusal a use's outcome says, none for a use or no ability.
    #[must_use]
    pub fn of(used: tungsten::kit::Used) -> Option<Self> {
        use tungsten::kit::Used;
        match used {
            Used::NoCost => Some(Refusal::NoMana),
            Used::NotReady | Used::NoCharge => Some(Refusal::NotReady),
            Used::NoTarget => Some(Refusal::NoRoom),
            _ => None,
        }
    }
}
