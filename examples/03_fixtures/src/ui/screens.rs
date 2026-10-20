//! The paint list's batch partition (`D-147`): the lab HUD on the game
//! layer, the inventory window and its drop confirmation open over it, both
//! modal on the popup layer, so the confirmation's text paints over the
//! window's panels and opens a second text batch (w01 §6). The confirmation
//! takes every key while it shows (`D-149`): Cancel or Escape hides it, and
//! the pane's Drop shows it again. The `default`, `labels`, `hidden` and
//! `debug` fixtures build this screen; the interactive run pushes the same
//! inventory as a state ([`crate::ui::inventory::InventoryState`]).

use tungsten::core::AssetRegistry;
use tungsten::core::ui::UiTree;

use crate::ui::hud::{self, LabHud};
use crate::ui::inventory::{self, Inventory};

/// Builds the lab HUD and the inventory, its confirmation open over the
/// first slot's stack, into `tree`. Sprite IDs are interned through
/// `sprites`, so a headless test without the manifest builds the same tree
/// (its images draw nothing and warn once).
pub fn build(tree: &mut UiTree, sprites: &mut AssetRegistry) -> (LabHud, Inventory) {
    let lab_hud = hud::build_lab_hud(tree, sprites);
    let inventory = inventory::build(tree, sprites, true);
    (lab_hud, inventory)
}
