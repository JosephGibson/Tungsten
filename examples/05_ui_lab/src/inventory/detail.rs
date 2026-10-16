//! The detail pane and the drop confirmation. The pane shows the focused
//! slot's item and its actions, changing its widgets in place as focus
//! moves, never rebuilding them; an action that does not apply to the item
//! is disabled, which takes its `disabled` look from the theme. The
//! confirmation is a modal on the popup layer that takes every key while
//! it shows, with Cancel focused, so a stray accept cancels (`D-149`).

use glam::Vec2;
use tungsten::core::ui::{
    Align, Anchor, Edges, LayoutStyle, Sizing, StyleOverrides, TextOverrides, UiLayer, UiTree,
    Visibility, WidgetId,
};

use super::items::{ItemDef, ItemKind};
use crate::theme;
use crate::widgets;

/// The pane's widgets.
#[derive(Debug, Clone, Copy)]
pub struct Pane {
    /// The pane's panel.
    pub root: WidgetId,
    /// The item's name, in its rarity's colour.
    pub name: WidgetId,
    /// Its kind and rarity.
    pub meta: WidgetId,
    /// Its one-sentence description.
    pub description: WidgetId,
    /// The stack's count, "×3", at the name's end.
    pub count: WidgetId,
    /// "Equipped", shown while the item is.
    pub badge: WidgetId,
    /// Use, Equip (Unequip while equipped) and Drop, in a row of equal
    /// widths centred under the pane.
    pub use_button: WidgetId,
    pub equip: WidgetId,
    pub drop: WidgetId,
}

/// What the pane shows: the item, its count and whether it is equipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    pub item: Option<(&'static ItemDef, u32)>,
    pub equipped: bool,
}

/// A raised panel: the pane's and the confirmation's surface, a background
/// override only, so the panel kind's border shows.
fn raised() -> StyleOverrides {
    StyleOverrides {
        background: Some(theme::RAISED),
        ..StyleOverrides::default()
    }
}

/// Builds the pane under `parent`, [`theme::PANE_WIDTH`] wide.
pub fn build(tree: &mut UiTree, parent: WidgetId) -> Pane {
    let mut style = LayoutStyle::column()
        .with_padding(theme::SECTION_GAP)
        .with_gap(theme::ROW_GAP)
        .with_size(Sizing::Px(theme::PANE_WIDTH), Sizing::Auto);
    style.align_items = Align::Stretch;
    let root = tree.panel(parent, style);
    tree.set_overrides(root, raised());
    // The name with the count at its end; the kind and rarity with the
    // badge at its end; the description: three lines, so the window fits a
    // 1280×720 window at an OS scale of 1.75 clear of the HUD.
    let line = |tree: &mut UiTree| {
        let mut style = LayoutStyle::row().with_gap(theme::ROW_GAP);
        style.align_items = Align::Center;
        let row = tree.panel(root, style);
        tree.set_overrides(row, widgets::transparent());
        row
    };
    let fill = LayoutStyle::default().with_size(Sizing::Fill, Sizing::Auto);
    let top = line(tree);
    let name = widgets::heading(tree, top, "");
    tree.set_layout_style(name, fill.clone());
    let count = tree.label(top, "");
    let muted = widgets::text(TextOverrides {
        color: Some(theme::MUTED),
        ..TextOverrides::default()
    });
    tree.set_overrides(
        count,
        widgets::text(widgets::value_text(TextOverrides {
            color: Some(theme::MUTED),
            ..TextOverrides::default()
        })),
    );
    let middle = line(tree);
    let meta = tree.label(middle, "");
    tree.set_layout_style(meta, fill);
    tree.set_overrides(meta, muted);
    let badge = tree.label(middle, "Equipped");
    tree.set_layout_style(
        badge,
        LayoutStyle {
            padding: Edges::symmetric(theme::SPACE[1], 0.0),
            ..LayoutStyle::default()
        },
    );
    tree.set_overrides(
        badge,
        StyleOverrides {
            background: Some(theme::BADGE),
            corner_radius: Some(4.0),
            text: TextOverrides {
                color: Some(theme::BADGE_TEXT),
                weight: Some(600),
                ..TextOverrides::default()
            },
            ..StyleOverrides::default()
        },
    );
    let description = tree.label(root, "");
    let actions = widgets::action_row(tree, root, &["Use", "Equip", "Drop"]);
    Pane {
        root,
        name,
        meta,
        description,
        count,
        badge,
        use_button: actions[0],
        equip: actions[1],
        drop: actions[2],
    }
}

/// Shows `shown` in `pane`: the item's lines and the actions that apply,
/// or an empty slot with every action disabled.
pub fn show(tree: &mut UiTree, pane: &Pane, shown: Shown) {
    let name_color = shown
        .item
        .map_or(theme::MUTED, |(item, _)| theme::rarity_color(item.rarity));
    tree.set_overrides(
        pane.name,
        StyleOverrides {
            text: TextOverrides {
                color: Some(name_color),
                ..widgets::heading_text()
            },
            ..StyleOverrides::default()
        },
    );
    if let Some((item, count)) = shown.item {
        tree.set_text(pane.name, item.name);
        tree.set_text(
            pane.meta,
            format!("{} \u{b7} {}", item.kind.name(), item.rarity.name()),
        );
        tree.set_text(pane.description, item.description);
        tree.set_text(pane.count, format!("\u{d7}{count}"));
        tree.set_enabled(pane.use_button, item.kind == ItemKind::Consumable);
        tree.set_enabled(pane.equip, item.kind.is_gear());
        tree.set_enabled(pane.drop, true);
    } else {
        tree.set_text(pane.name, "Empty slot");
        tree.set_text(pane.meta, "\u{2013}");
        tree.set_text(pane.description, "Nothing here.");
        tree.set_text(pane.count, "");
        for button in [pane.use_button, pane.equip, pane.drop] {
            tree.set_enabled(button, false);
        }
    }
    tree.set_text(pane.equip, if shown.equipped { "Unequip" } else { "Equip" });
    tree.set_visibility(
        pane.badge,
        if shown.equipped {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
}

/// The drop confirmation's widgets.
#[derive(Debug, Clone, Copy)]
pub struct Confirm {
    /// The modal root, hidden until a drop asks.
    pub root: WidgetId,
    /// "Drop <item>?".
    pub heading: WidgetId,
    /// Cancel, focused when it shows.
    pub cancel: WidgetId,
    /// Drop: removes the whole stack.
    pub drop: WidgetId,
}

/// The confirmation's width, in UI units: room for its text and its two
/// buttons.
const CONFIRM_WIDTH: f32 = 300.0;

/// Builds the confirmation: a raised modal root on the popup layer, centred
/// in the viewport, hidden.
pub fn build_confirm(tree: &mut UiTree) -> Confirm {
    let mut style = LayoutStyle::column()
        .with_padding(theme::WINDOW_PADDING)
        .with_gap(theme::ROW_GAP)
        .with_size(Sizing::Px(CONFIRM_WIDTH), Sizing::Auto)
        .anchored(Anchor::Center, Vec2::ZERO);
    style.align_items = Align::Stretch;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
    tree.set_overrides(root, raised());
    let heading = widgets::heading(tree, root, "Drop?");
    tree.label(
        root,
        "The whole stack is dropped. It cannot be picked up again.",
    );
    let actions = widgets::action_row(tree, root, &["Cancel", "Drop"]);
    tree.set_visibility(root, Visibility::Hidden);
    Confirm {
        root,
        heading,
        cancel: actions[0],
        drop: actions[1],
    }
}
