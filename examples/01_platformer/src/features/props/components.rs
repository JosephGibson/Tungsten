//! The props' components.

use serde::{Deserialize, Serialize};
use tungsten::core::Entity;

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

/// A crate's fire: the game seconds of fire it has left, kept once it is
/// put out; whether it burns; the save ID its fire credits; and the seconds
/// a douse keeps it from lighting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fuel {
    pub left: f32,
    pub lit: bool,
    pub lighter: Option<u64>,
    pub doused: f32,
}

/// A red powder keg on a fuse: the game seconds its lit fuse has left, the
/// save ID its blast credits, and the seconds a douse keeps it from
/// lighting. Its prefab names it `keg`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct Keg {
    #[serde(skip)]
    pub fuse: Option<f32>,
    #[serde(skip)]
    pub lighter: Option<u64>,
    #[serde(skip)]
    pub doused: f32,
}

/// A goblin's bomb, thrown lit: the game seconds its fuse has left, and the
/// save ID its blast credits, the goblin's. Its prefab names it
/// `goblin_bomb`; its throw lights it. Snuffed, it fizzles out at once, so
/// it keeps no state past its fuse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct Bomb {
    #[serde(skip)]
    pub fuse: Option<f32>,
    #[serde(skip)]
    pub lighter: Option<u64>,
}

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

/// One of a lit fuse's two emitters at its tip, its sparks or its smoke,
/// and the keg or bomb it burns on: presentation, made again after a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuseEmitter {
    pub prop: Entity,
}
