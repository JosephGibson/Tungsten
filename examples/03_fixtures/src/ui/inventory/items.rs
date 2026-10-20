//! Item data as code: ten definitions and the starting contents. A game
//! keeps such tables in Rust, or loads them as an asset of its own; nothing
//! here names a file path (`D-046`). Each icon is a sprite in the lab's
//! manifest, drawn by `tools/generate.py`.

/// What an item is, which decides its actions: a consumable is used, gear
/// (a weapon, armour or a trinket) is equipped, anything is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// Used up one at a time.
    Consumable,
    /// Gear for the weapon slot.
    Weapon,
    /// Gear for the armour slot.
    Armour,
    /// Gear for the trinket slot.
    Trinket,
    /// Crafting stock: only dropped.
    Material,
}

impl ItemKind {
    /// The kind's name as the detail pane shows it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Consumable => "Consumable",
            Self::Weapon => "Weapon",
            Self::Armour => "Armour",
            Self::Trinket => "Trinket",
            Self::Material => "Material",
        }
    }

    /// Whether items of this kind are equipped.
    #[must_use]
    pub const fn is_gear(self) -> bool {
        matches!(self, Self::Weapon | Self::Armour | Self::Trinket)
    }

    /// The equipment slot of a gear kind: one item equipped per kind.
    #[must_use]
    pub const fn gear_slot(self) -> Option<usize> {
        match self {
            Self::Weapon => Some(0),
            Self::Armour => Some(1),
            Self::Trinket => Some(2),
            Self::Consumable | Self::Material => None,
        }
    }
}

/// How rare an item is; [`crate::ui::theme::rarity_color`] gives its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// The rarity's name as the detail pane shows it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Uncommon => "Uncommon",
            Self::Rare => "Rare",
            Self::Epic => "Epic",
            Self::Legendary => "Legendary",
        }
    }
}

/// One item definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemDef {
    pub name: &'static str,
    pub kind: ItemKind,
    pub rarity: Rarity,
    /// The most a slot holds.
    pub stack: u32,
    /// One short sentence for the detail pane, on one line.
    pub description: &'static str,
    /// The icon's sprite ID.
    pub icon: &'static str,
}

/// The ten item definitions.
pub const ITEMS: [ItemDef; 10] = [
    ItemDef {
        name: "Healing Draught",
        kind: ItemKind::Consumable,
        rarity: Rarity::Common,
        stack: 10,
        description: "Restores a little health.",
        icon: "ex05_item_draught",
    },
    ItemDef {
        name: "Iron Sword",
        kind: ItemKind::Weapon,
        rarity: Rarity::Common,
        stack: 1,
        description: "A plain blade that holds an edge.",
        icon: "ex05_item_sword",
    },
    ItemDef {
        name: "Leather Cap",
        kind: ItemKind::Armour,
        rarity: Rarity::Common,
        stack: 1,
        description: "Keeps the rain off, little else.",
        icon: "ex05_item_cap",
    },
    ItemDef {
        name: "Lantern",
        kind: ItemKind::Trinket,
        rarity: Rarity::Uncommon,
        stack: 1,
        description: "Lights a small circle in the dark.",
        icon: "ex05_item_lantern",
    },
    ItemDef {
        name: "Iron Ore",
        kind: ItemKind::Material,
        rarity: Rarity::Common,
        stack: 20,
        description: "Smelts into bars at any forge.",
        icon: "ex05_item_ore",
    },
    ItemDef {
        name: "Herb Bundle",
        kind: ItemKind::Material,
        rarity: Rarity::Uncommon,
        stack: 20,
        description: "Fresh leaves for draughts.",
        icon: "ex05_item_herb",
    },
    ItemDef {
        name: "Elixir",
        kind: ItemKind::Consumable,
        rarity: Rarity::Rare,
        stack: 5,
        description: "Restores health and focus at once.",
        icon: "ex05_item_elixir",
    },
    ItemDef {
        name: "Yew Longbow",
        kind: ItemKind::Weapon,
        rarity: Rarity::Rare,
        stack: 1,
        description: "Reaches what a sword cannot.",
        icon: "ex05_item_bow",
    },
    ItemDef {
        name: "Mithril Mail",
        kind: ItemKind::Armour,
        rarity: Rarity::Epic,
        stack: 1,
        description: "Light as cloth and hard as stone.",
        icon: "ex05_item_mail",
    },
    ItemDef {
        name: "Moonstone",
        kind: ItemKind::Trinket,
        rarity: Rarity::Legendary,
        stack: 1,
        description: "Glows faintly when danger is near.",
        icon: "ex05_item_moonstone",
    },
];

/// Slots in the grid: six rows of five.
pub const SLOTS: usize = 30;
/// Slots in a row.
pub const COLUMNS: usize = 5;

/// What the grid holds at the start: 12 of 30 slots, as (slot, item
/// index, count).
pub const START: [(usize, usize, u32); 12] = [
    (0, 0, 3),
    (1, 1, 1),
    (2, 2, 1),
    (3, 3, 1),
    (4, 4, 12),
    (5, 5, 7),
    (7, 6, 2),
    (10, 7, 1),
    (12, 8, 1),
    (15, 9, 1),
    (21, 4, 20),
    (27, 5, 3),
];
