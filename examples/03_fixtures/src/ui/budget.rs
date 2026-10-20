//! The prewarm budget's subject (`D-163`): a 100-widget menu that the
//! `menu` fixture shows from the first frame and the `menu-prewarm` and
//! `menu-cold` fixtures hide and show at fixed frames, prewarmed or not, so
//! a first open's glyph rasterization is measured against a warm one. Its
//! node count and its frames are a perf contract; keep them as they are.

use glam::Vec2;
use tungsten::core::ui::{
    Align, Anchor, CursorIcon, LayoutStyle, StyleOverrides, TextOverrides, UiLayer, UiTree,
    Visibility, WidgetId,
};

use crate::ui::widgets;

/// The menu fixtures' menu: a `World` resource.
#[derive(Debug)]
pub struct BudgetMenu {
    /// The root.
    pub root: WidgetId,
}

/// Item names for the menu's buttons: varied letters, so a first open
/// rasterizes a realistic set of glyphs.
const ITEMS: [&str; 46] = [
    "Longsword",
    "Quartz Ring",
    "Healing Draught",
    "Iron Buckler",
    "Lantern",
    "Map of Vexley",
    "Jade Amulet",
    "Rope (50 ft)",
    "Flint & Steel",
    "Bedroll",
    "Wizard's Hat",
    "Cloak of Mist",
    "Throwing Axe",
    "Spyglass",
    "Bag of Holding",
    "Zephyr Boots",
    "Kettle",
    "Quill & Ink",
    "Obsidian Dagger",
    "Warhammer",
    "Elixir No. 9",
    "Yew Longbow",
    "Arrows x24",
    "Gauntlets",
    "Tome of Frost",
    "Crowbar",
    "Hourglass",
    "Lockpicks",
    "Mirror Shard",
    "Pickaxe",
    "Silver Key",
    "Torch",
    "Waterskin",
    "Xylophone",
    "Brass Compass",
    "Dice Set",
    "Fishing Rod",
    "Grappling Hook",
    "Herb Pouch",
    "Ivory Comb",
    "Jester's Bell",
    "Knight's Crest",
    "Moonstone",
    "Nightshade",
    "Ox Horn",
    "Prism",
];

/// Builds the menu fixtures' 100-widget menu into `tree`: a centred root
/// holding a title, a row of four columns of 46 buttons and a footer. With
/// each button's label, the tree holds 1 + 1 + 1 + 4 + 92 + 1 = 100 nodes.
/// Hidden when `hidden`.
pub fn build_budget_menu(tree: &mut UiTree, hidden: bool) -> BudgetMenu {
    let mut style = LayoutStyle::column()
        .with_padding(16.0)
        .with_gap(10.0)
        .anchored(Anchor::Center, Vec2::ZERO);
    style.align_items = Align::Center;
    let root = tree.add_root(UiLayer::Game, style);
    let title = tree.label(root, "Inventory — 46 items");
    tree.set_overrides(
        title,
        StyleOverrides {
            text: TextOverrides {
                size: Some(26.0),
                line_height: Some(26.0 * 1.3),
                weight: Some(700),
                ..TextOverrides::default()
            },
            ..StyleOverrides::default()
        },
    );
    let row = tree.panel(root, LayoutStyle::row().with_gap(10.0));
    tree.set_overrides(row, widgets::transparent());
    let columns: Vec<WidgetId> = (0..4)
        .map(|_| {
            let column = tree.panel(row, LayoutStyle::column().with_gap(4.0));
            tree.set_overrides(column, widgets::transparent());
            column
        })
        .collect();
    // Plain buttons, not `widgets::button`'s padded ones: the menu's look
    // stays what the budget measured.
    for (i, item) in ITEMS.iter().enumerate() {
        let button = tree.button(columns[i % columns.len()], *item);
        tree.set_cursor(button, Some(CursorIcon::Pointer));
    }
    tree.label(root, "Quick-sort: weight, value, rarity — 9 kg free");
    if hidden {
        tree.set_visibility(root, Visibility::Hidden);
    }
    BudgetMenu { root }
}

/// The frames at which the `menu-prewarm` and `menu-cold` fixtures show,
/// hide and show the menu again.
pub const MENU_SHOW: u64 = 240;
/// See [`MENU_SHOW`].
pub const MENU_HIDE: u64 = 300;
/// See [`MENU_SHOW`].
pub const MENU_REOPEN: u64 = 360;
