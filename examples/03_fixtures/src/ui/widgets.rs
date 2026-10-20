//! Small builder functions over `UiTree` that every screen shares: a modal
//! screen root, a corner window, a heading, a button, a row of a caption
//! and its control (a slider, a toggle or any widget), a prompt row. They
//! compose the core's widget kinds (`D-125`) rather than adding new ones,
//! so a game copies them and changes a number, not a widget. Their sizes
//! come from [`crate::ui::theme`], which is what keeps every screen aligned: a
//! menu column's buttons stretch to one width, setting rows share one
//! caption column and one control column, and a row of action buttons
//! takes equal widths, centred.

use glam::Vec2;
use tungsten::core::text::TextWrap;
use tungsten::core::ui::{
    Align, Anchor, CursorIcon, Edges, Justify, LayoutStyle, Sizing, SliderRange, StyleOverrides,
    TextOverrides, UiLayer, UiTree, WidgetId,
};

use crate::ui::theme;

/// No fill and no border: a layout-only panel.
#[must_use]
pub fn transparent() -> StyleOverrides {
    StyleOverrides {
        background: Some([0, 0, 0, 0]),
        border_width: Some(0.0),
        ..StyleOverrides::default()
    }
}

/// Style overrides that change only the text.
#[must_use]
pub fn text(overrides: TextOverrides) -> StyleOverrides {
    StyleOverrides {
        text: overrides,
        ..StyleOverrides::default()
    }
}

/// A live value's text: one line that never wraps, so a value that grows
/// (a counter's next digit) cannot push its end onto a hidden line.
#[must_use]
pub fn value_text(overrides: TextOverrides) -> TextOverrides {
    TextOverrides {
        wrap: Some(TextWrap::None),
        ..overrides
    }
}

/// A heading's text: [`theme::HEADING_SIZE`] on [`theme::HEADING_LINE`],
/// bold.
#[must_use]
pub fn heading_text() -> TextOverrides {
    TextOverrides {
        size: Some(theme::HEADING_SIZE),
        line_height: Some(theme::HEADING_LINE),
        weight: Some(700),
        ..TextOverrides::default()
    }
}

/// A heading label under `parent`.
pub fn heading(tree: &mut UiTree, parent: WidgetId, text: &str) -> WidgetId {
    let label = tree.label(parent, text);
    tree.set_overrides(
        label,
        StyleOverrides {
            text: heading_text(),
            ..StyleOverrides::default()
        },
    );
    label
}

/// A button's layout: its padding and least width.
#[must_use]
pub fn button_style() -> LayoutStyle {
    LayoutStyle {
        padding: Edges::symmetric(theme::BUTTON_PADDING.x, theme::BUTTON_PADDING.y),
        min_width: Some(theme::BUTTON_MIN_WIDTH),
        ..LayoutStyle::default()
    }
}

/// A button holding `text`, padded, at least [`theme::BUTTON_MIN_WIDTH`]
/// wide, with the pointer cursor; the theme centres its text.
pub fn button(tree: &mut UiTree, parent: WidgetId, text: &str) -> WidgetId {
    let id = tree.button(parent, text);
    tree.set_layout_style(id, button_style());
    tree.set_cursor(id, Some(CursorIcon::Pointer));
    id
}

/// A row of action buttons under `parent`, each exactly
/// [`theme::BUTTON_MIN_WIDTH`] wide, [`theme::ROW_GAP`] apart and centred
/// as a group.
pub fn action_row(tree: &mut UiTree, parent: WidgetId, labels: &[&str]) -> Vec<WidgetId> {
    let mut style = LayoutStyle::row().with_gap(theme::ROW_GAP);
    style.justify_content = Justify::Center;
    let row = tree.panel(parent, style);
    tree.set_overrides(row, transparent());
    labels
        .iter()
        .map(|label| {
            let id = button(tree, row, label);
            tree.set_layout_style(
                id,
                LayoutStyle {
                    width: Sizing::Px(theme::BUTTON_MIN_WIDTH),
                    ..button_style()
                },
            );
            id
        })
        .collect()
}

/// A modal screen root on the popup layer, `width` units wide, centred in
/// the viewport: a column that stretches its children, so a menu's buttons
/// share its width (`D-149`).
pub fn screen_root(tree: &mut UiTree, width: f32) -> WidgetId {
    let mut style = LayoutStyle::column()
        .with_padding(theme::WINDOW_PADDING)
        .with_gap(theme::ROW_GAP)
        .with_size(Sizing::Px(width), Sizing::Auto)
        .anchored(Anchor::Center, Vec2::ZERO);
    style.align_items = Align::Stretch;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
    root
}

/// A window on the game layer at `anchor`, [`theme::SCREEN_MARGIN`] in from
/// it, `width` units wide: a column that stretches its children.
pub fn window(tree: &mut UiTree, anchor: Anchor, width: f32) -> WidgetId {
    let mut style = LayoutStyle::column()
        .with_padding(theme::WINDOW_PADDING)
        .with_gap(theme::ROW_GAP)
        .with_size(Sizing::Px(width), Sizing::Auto)
        .anchored(anchor, Vec2::splat(theme::SCREEN_MARGIN));
    style.align_items = Align::Stretch;
    tree.add_root(UiLayer::Game, style)
}

/// The width of a window or screen whose setting rows end at the control
/// column, `extra` units past it.
#[must_use]
pub fn setting_width(extra: f32) -> f32 {
    2.0 * theme::WINDOW_PADDING
        + theme::CAPTION_WIDTH
        + theme::ROW_GAP
        + theme::CONTROL_WIDTH
        + extra
}

/// A setting row under `parent`: a caption [`theme::CAPTION_WIDTH`] wide,
/// then the control the caller adds after it, both centred on the row.
/// Returns the row and the caption.
pub fn caption_row(tree: &mut UiTree, parent: WidgetId, caption: &str) -> (WidgetId, WidgetId) {
    let mut style = LayoutStyle::row().with_gap(theme::ROW_GAP);
    style.align_items = Align::Center;
    let row = tree.panel(parent, style);
    tree.set_overrides(row, transparent());
    let label = tree.label(row, caption);
    tree.set_layout_style(
        label,
        LayoutStyle::default().with_size(Sizing::Px(theme::CAPTION_WIDTH), Sizing::Auto),
    );
    (row, label)
}

/// Gives a control its caption as its accessible name and the pointer
/// cursor.
pub fn name_control(tree: &mut UiTree, control: WidgetId, caption: &str) {
    tree.set_accessible_name(control, Some(caption.to_owned()));
    tree.set_cursor(control, Some(CursorIcon::Pointer));
}

/// A setting row holding a slider [`theme::CONTROL_WIDTH`] wide after its
/// caption; returns the row and the slider.
pub fn slider_row(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    range: SliderRange,
    value: f32,
) -> (WidgetId, WidgetId) {
    let (row, _) = caption_row(tree, parent, caption);
    let slider = tree.slider(row, range, value);
    tree.set_layout_style(
        slider,
        LayoutStyle::default().with_size(Sizing::Px(theme::CONTROL_WIDTH), Sizing::Auto),
    );
    name_control(tree, slider, caption);
    (row, slider)
}

/// A setting row holding a toggle after its caption; returns the row and
/// the toggle.
pub fn toggle_row(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    on: bool,
) -> (WidgetId, WidgetId) {
    let (row, _) = caption_row(tree, parent, caption);
    let toggle = tree.toggle(row, on);
    name_control(tree, toggle, caption);
    (row, toggle)
}

/// A prompt label under `parent` naming actions' bindings for the last
/// device through `template` (`D-163`); it fills its row.
pub fn prompt_row(tree: &mut UiTree, parent: WidgetId, template: &str) -> WidgetId {
    let prompt = tree.label(parent, "");
    tree.set_overrides(
        prompt,
        text(TextOverrides {
            color: Some(theme::MUTED),
            ..TextOverrides::default()
        }),
    );
    tree.set_prompt(prompt, template);
    prompt
}
