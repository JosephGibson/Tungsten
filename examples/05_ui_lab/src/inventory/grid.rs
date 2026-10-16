//! The slot grid: a scroll panel of rows of focusable slots, laid out as
//! rows of a row panel, since layout has no grid (`D-124`). A slot is an
//! interactive, focusable panel that overrides only its background; its
//! border and focus ring come from the theme's `panel` kind, whose state
//! variants set only the border (`D-148`, `D-149`). Its icon, count and
//! rarity stripe take no hits, so a click lands on the slot. Directional
//! focus moves by the slots' layout boxes and scrolls the panel to the
//! focused slot.

use glam::Vec2;
use tungsten::core::AssetRegistry;
use tungsten::core::ui::{
    Anchor, CursorIcon, Edges, LayoutStyle, Sizing, StyleOverrides, TextOverrides, UiTree,
    Visibility, WidgetId,
};

use super::Stack;
use super::items::{COLUMNS, ITEMS, SLOTS};
use crate::theme;
use crate::widgets;

/// One slot's widgets.
#[derive(Debug, Clone, Copy)]
pub struct SlotUi {
    /// The focusable panel.
    pub root: WidgetId,
    /// The item's icon; `None` for a slot that started empty.
    pub icon: Option<WidgetId>,
    /// The stack's count, at the bottom end corner.
    pub count: WidgetId,
    /// The rarity stripe along the bottom edge.
    pub stripe: WidgetId,
}

/// The grid's widgets.
#[derive(Debug, Clone)]
pub struct Grid {
    /// The scroll panel.
    pub panel: WidgetId,
    /// The slots in order, row by row.
    pub slots: Vec<SlotUi>,
}

/// Rows the panel shows at once.
pub const VISIBLE_ROWS: f32 = 3.0;

/// The grid's width and visible height, in UI units: the slots, their gaps
/// and the room the scroll thumb paints in.
#[must_use]
pub fn size() -> Vec2 {
    let columns = COLUMNS as f32;
    Vec2::new(
        columns * theme::SLOT + (columns - 1.0) * theme::SLOT_GAP + 2.0 * theme::SLOT_GAP,
        VISIBLE_ROWS * theme::SLOT + (VISIBLE_ROWS - 1.0) * theme::SLOT_GAP,
    )
}

/// Builds the grid under `parent` for `contents`, each filled slot with its
/// item's icon, count and stripe.
pub fn build(
    tree: &mut UiTree,
    parent: WidgetId,
    sprites: &mut AssetRegistry,
    contents: &[Option<Stack>; SLOTS],
) -> Grid {
    let size = size();
    let panel = tree.scroll_panel(
        parent,
        LayoutStyle {
            // Room for the scroll thumb at the end, and as much at the
            // start, so the slots sit centred in the panel.
            padding: Edges {
                start: theme::SLOT_GAP,
                end: theme::SLOT_GAP,
                ..Edges::default()
            },
            gap: theme::SLOT_GAP,
            ..LayoutStyle::column().with_size(Sizing::Px(size.x), Sizing::Px(size.y))
        },
    );
    tree.set_overrides(panel, widgets::transparent());
    let mut slots = Vec::with_capacity(SLOTS);
    for chunk in contents.chunks(COLUMNS) {
        let row = tree.panel(panel, LayoutStyle::row().with_gap(theme::SLOT_GAP));
        tree.set_overrides(row, widgets::transparent());
        for stack in chunk {
            slots.push(build_slot(tree, row, sprites, *stack));
        }
    }
    Grid { panel, slots }
}

fn build_slot(
    tree: &mut UiTree,
    row: WidgetId,
    sprites: &mut AssetRegistry,
    stack: Option<Stack>,
) -> SlotUi {
    let root = tree.panel(
        row,
        LayoutStyle::default().with_size(Sizing::Px(theme::SLOT), Sizing::Px(theme::SLOT)),
    );
    tree.set_focusable(root, true);
    tree.set_interactive(root, true);
    tree.set_cursor(root, Some(CursorIcon::Pointer));
    tree.set_overrides(
        root,
        StyleOverrides {
            background: Some(theme::RAISED),
            ..StyleOverrides::default()
        },
    );
    let icon = stack.map(|stack| {
        let sprite = sprites.intern_sprite(ITEMS[stack.item].icon);
        let icon = tree.image(
            root,
            sprite,
            Vec2::splat(theme::ICON),
            LayoutStyle::default()
                .with_size(Sizing::Px(theme::ICON), Sizing::Px(theme::ICON))
                .anchored(Anchor::Center, Vec2::ZERO),
        );
        tree.set_hit_testable(icon, false);
        icon
    });
    // A label with no text: a fill and no border, so nothing overrides a
    // border and the slot's focus ring stays the theme's.
    let stripe = tree.label(root, "");
    tree.set_layout_style(
        stripe,
        LayoutStyle::default()
            .with_size(
                Sizing::Px(theme::SLOT - 2.0 * theme::STRIPE_INSET),
                Sizing::Px(theme::STRIPE),
            )
            .anchored(Anchor::BottomStart, Vec2::splat(theme::STRIPE_INSET)),
    );
    tree.set_hit_testable(stripe, false);
    let count = tree.label(root, "");
    tree.set_layout_style(
        count,
        LayoutStyle::default().anchored(
            Anchor::BottomEnd,
            Vec2::new(theme::STRIPE_INSET, theme::STRIPE_INSET + theme::STRIPE),
        ),
    );
    tree.set_hit_testable(count, false);
    tree.set_overrides(
        count,
        widgets::text(widgets::value_text(TextOverrides {
            family: Some("mono".to_owned()),
            size: Some(12.0),
            line_height: Some(14.0),
            ..TextOverrides::default()
        })),
    );
    let slot = SlotUi {
        root,
        icon,
        count,
        stripe,
    };
    show_slot(tree, &slot, stack);
    slot
}

/// Shows `stack` in `slot`: its name as the slot's accessible name, the
/// count above 1, the stripe in its rarity's colour, or an empty slot.
pub fn show_slot(tree: &mut UiTree, slot: &SlotUi, stack: Option<Stack>) {
    let shown = |filled: bool| {
        if filled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        }
    };
    if let Some(icon) = slot.icon {
        tree.set_visibility(icon, shown(stack.is_some()));
    }
    tree.set_visibility(slot.stripe, shown(stack.is_some()));
    if let Some(stack) = stack {
        let item = &ITEMS[stack.item];
        tree.set_accessible_name(slot.root, Some(item.name.to_owned()));
        tree.set_overrides(
            slot.stripe,
            StyleOverrides {
                background: Some(theme::rarity_color(item.rarity)),
                corner_radius: Some(1.0),
                ..StyleOverrides::default()
            },
        );
        if stack.count > 1 {
            tree.set_text(slot.count, stack.count.to_string());
        } else {
            tree.set_text(slot.count, "");
        }
    } else {
        tree.set_accessible_name(slot.root, Some("Empty slot".to_owned()));
        tree.set_text(slot.count, "");
    }
}
