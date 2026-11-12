//! The props' components.

use serde::{Deserialize, Serialize};

/// What a prop is: a pine crate, a powder keg, a goblin's bomb or rock, a
/// broken crate's plank, or a rubble stone of its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropKind {
    Crate,
    Keg,
    Bomb,
    Rock,
    Plank,
    Stone(StoneSize),
}

/// A rubble stone's size (e01hollow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoneSize {
    Small,
    Medium,
    Large,
}

/// A crate or keg the map sets down for a fresh run, at its markers: none
/// of the summon's, so its caps leave it alone. Saved, as `map_prop`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapProp;

impl StoneSize {
    /// Every size, smallest first.
    pub const ALL: [StoneSize; 3] = [StoneSize::Small, StoneSize::Medium, StoneSize::Large];

    /// Its name in its prefab's and its art's IDs.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            StoneSize::Small => "small",
            StoneSize::Medium => "medium",
            StoneSize::Large => "large",
        }
    }
}

/// A pine crate, which catches fire, burns down and falls in: its art's
/// variant, by its save ID's parity. Its prefab names it `crate`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Crate {
    #[serde(skip)]
    pub variant: usize,
}

/// A plank of a broken crate (e01sfx), which burns as a crate does and
/// falls in quietly: its art's variant, its row in the crate. Its prefab
/// names it `plank`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Plank {
    #[serde(skip)]
    pub variant: usize,
}

/// [`Plank`] in a save: its row, which its prefab leaves out.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlankForm {
    pub variant: usize,
}

impl From<&Plank> for PlankForm {
    fn from(plank: &Plank) -> Self {
        Self {
            variant: plank.variant,
        }
    }
}

impl From<PlankForm> for Plank {
    fn from(form: PlankForm) -> Self {
        Self {
            variant: form.variant,
        }
    }
}

/// A crate's or a plank's fire, as a break passes it on (7.13a): the game
/// seconds of fire it has left, its `fuel` stat, kept once it is put out;
/// whether it burns, its `prop_burn`; the save ID its fire credits, the
/// burn's source's; and the seconds a douse keeps it from lighting, its
/// `prop_doused`'s.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropFire {
    pub left: f32,
    pub lit: bool,
    pub lighter: Option<u64>,
    pub doused: f32,
}

/// A red powder keg on a fuse, whose fuse is its `lit` status, sourced to
/// whoever lit it, and whose douse its `prop_doused`. Its prefab names it
/// `keg`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Keg;

/// A goblin's bomb, thrown lit: its fuse is its `lit` status, sourced to
/// its goblin. Its prefab names it `goblin_bomb`. Snuffed, it fizzles out
/// at once, so it keeps no state past its fuse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Bomb;

/// A goblin's rock, a stone that bounces and rolls once thrown and stays
/// where it comes to rest: its art's variant, by its save ID's parity. Its
/// prefab names it `rock`; its throw's credit is a `Throws` entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Rock {
    #[serde(skip)]
    pub variant: usize,
}

/// A rubble stone of the hollow's basalt (e01hollow), which the map sets
/// down in heaps: pushed, thrown and blasted as a goblin's rock is, it
/// slides as a crate does and never burns or freezes. Its size, its
/// prefab's, and its art's variant, by its save ID's parity. Its prefab
/// names it `stone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Stone {
    pub size: StoneSize,
    #[serde(skip)]
    pub variant: usize,
}
