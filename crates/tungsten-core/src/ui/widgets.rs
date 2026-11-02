//! Tree-only builders (redo 7.17): the template's shell widgets, moved into
//! core with its names, so a game's screens and a data file's (7.20) build
//! alike. A modal screen (a scrim and its card), text in the theme's named
//! styles, buttons and setting rows each named by its caption, a tab rail,
//! a prompt row, a menu of keyed buttons, a meter and a radial fill. Each
//! builds through public [`UiTree`] calls and marks what they mark.
//!
//! Spacing comes from the theme's tokens, colours from its colour tokens,
//! and sizes from its `sizes`, each with the template's number where the
//! theme names none: `button_padding_x` 14 and `button_padding_y` 5,
//! `control_width` 170, `readout_width` 64, `slider_line` 18, `menu_width`
//! 260 and `tab_bar` 3.

use std::f32::consts::TAU;

use glam::Vec2;

use crate::text::TextAlign;

use super::control::SliderRange;
use super::id::WidgetId;
use super::node::{CursorIcon, UiLayer, UiVertex, Visibility};
use super::style::{Align, Edges, Justify, LayoutStyle, Sizing};
use super::theme::TextOverrides;
use super::tree::UiTree;

/// The theme's size `name`, else the template's number for it.
#[must_use]
pub fn size(tree: &UiTree, name: &str) -> f32 {
    let default = match name {
        "button_padding_x" => 14.0,
        "button_padding_y" => 5.0,
        "control_width" => 170.0,
        "readout_width" => 64.0,
        "slider_line" => 18.0,
        "menu_width" => 260.0,
        "tab_bar" => 3.0,
        _ => 0.0,
    };
    tree.theme().size(name, default)
}

/// A modal screen: a root on the popup layer filling the viewport with the
/// theme's `scrim` over the screen below, and centred in it the card, a
/// vertical scroll panel padded the theme's `spacing.lg` and gapped its
/// `spacing.sm`, `width` units wide or the viewport's width when narrower,
/// as tall as its content up to the viewport's height, past which it
/// scrolls. Returns the root and the card.
pub fn screen(tree: &mut UiTree, width: f32) -> (WidgetId, WidgetId) {
    let full = Sizing::Percent(100.0);
    let mut style = LayoutStyle::column().with_size(full, full);
    style.align_items = Align::Center;
    style.justify_content = Justify::Center;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
    let scrim = tree.theme().tokens.colors.scrim;
    fill(tree, root, scrim);
    let spacing = tree.theme().tokens.spacing;
    let mut style = LayoutStyle::column()
        .with_padding(spacing.lg)
        .with_gap(spacing.sm)
        .with_size(full, Sizing::Auto);
    style.max_width = Some(width);
    (root, tree.scroll_panel(root, style))
}

/// Fills `id` with `color`, with no border, its other overrides kept; a
/// clear fill leaves a layout-only panel.
pub fn fill(tree: &mut UiTree, id: WidgetId, color: [u8; 4]) {
    tree.edit_overrides(id, |own| {
        own.background = Some(color);
        own.border_width = Some(0.0);
    });
}

/// Restyles `id`'s text with `edit`, its other overrides kept.
pub fn restyle(tree: &mut UiTree, id: WidgetId, edit: impl FnOnce(&mut TextOverrides)) {
    tree.edit_overrides(id, |own| edit(&mut own.text));
}

/// A label under `parent` in the theme's text style `style`.
fn styled(tree: &mut UiTree, parent: WidgetId, caption: &str, style: &str) -> WidgetId {
    let label = tree.label(parent, caption);
    tree.set_text_style(label, Some(style));
    label
}

/// A centred label in the theme's text style `style`: a title, or game
/// over's line, which a style that shrinks keeps to one line.
pub fn display(tree: &mut UiTree, parent: WidgetId, caption: &str, style: &str) -> WidgetId {
    let label = styled(tree, parent, caption, style);
    restyle(tree, label, |text| text.align = Some(TextAlign::Center));
    label
}

/// A heading in the theme's `heading` style; a menu's is centred, a card's
/// sits at the start.
pub fn heading(tree: &mut UiTree, parent: WidgetId, caption: &str, centred: bool) -> WidgetId {
    let label = styled(tree, parent, caption, "heading");
    if centred {
        restyle(tree, label, |text| text.align = Some(TextAlign::Center));
    }
    label
}

/// A label in the theme's `note` style, its muted colour.
pub fn note(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    styled(tree, parent, caption, "note")
}

/// Small muted print, the theme's `small` style.
pub fn small(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    styled(tree, parent, caption, "small")
}

/// Sets `id`'s text in the theme's mono family, its other text kept: the
/// shell's numbers.
pub fn mono(tree: &mut UiTree, id: WidgetId) {
    let family = tree.theme().tokens.fonts.mono.clone();
    restyle(tree, id, |text| text.family = Some(family));
}

/// Sets `id`'s text in the theme's danger colour: a destructive action.
pub fn danger(tree: &mut UiTree, id: WidgetId) {
    let color = tree.theme().tokens.colors.danger;
    restyle(tree, id, |text| text.color = Some(color));
}

/// A control's layout: padded by the theme's button padding, `width` wide.
fn padded(tree: &UiTree, width: Sizing) -> LayoutStyle {
    let (x, y) = (
        size(tree, "button_padding_x"),
        size(tree, "button_padding_y"),
    );
    let mut style = LayoutStyle::default().with_size(width, Sizing::Auto);
    style.padding = Edges::symmetric(x, y);
    style
}

/// A button holding `caption`, padded, with the pointer cursor and its
/// caption as its accessible name.
pub fn button(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let id = tree.button(parent, caption);
    let style = padded(tree, Sizing::Auto);
    tree.set_layout_style(id, style);
    name(tree, id, caption);
    id
}

/// Gives a control `caption` as its accessible name and the pointer cursor.
pub fn name(tree: &mut UiTree, control: WidgetId, caption: &str) {
    tree.set_accessible_name(control, Some(caption.to_owned()));
    tree.set_cursor(control, Some(CursorIcon::Pointer));
}

/// A layout-only group under `parent` ([`UiTree::group`]) whose children
/// flow along `style`'s direction, gapped the theme's `spacing.sm` and
/// `align`ed on it.
pub fn group(tree: &mut UiTree, parent: WidgetId, style: LayoutStyle, align: Align) -> WidgetId {
    let mut style = style.with_gap(tree.theme().tokens.spacing.sm);
    style.align_items = align;
    tree.group(parent, style)
}

/// A layout-only column under `parent` that stretches its children.
pub fn column(tree: &mut UiTree, parent: WidgetId) -> WidgetId {
    group(tree, parent, LayoutStyle::column(), Align::Stretch)
}

/// A layout-only row under `parent`, its children centred on it.
pub fn row(tree: &mut UiTree, parent: WidgetId) -> WidgetId {
    group(tree, parent, LayoutStyle::row(), Align::Center)
}

/// A setting row: a caption that takes the width the control the caller
/// adds after it leaves, wrapping. Returns the row.
pub fn caption_row(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let row = row(tree, parent);
    let label = tree.label(row, caption);
    set_width(tree, label, Sizing::Fill);
    row
}

/// `id` as wide as its content, at least `least` units.
fn at_least(tree: &mut UiTree, id: WidgetId, least: f32) {
    tree.edit_layout(id, |style| {
        (style.width, style.min_width) = (Sizing::Auto, Some(least));
    });
}

/// A setting row holding a button that shows `value` and cycles it when
/// activated, sharing the row with its caption, at least the theme's
/// `control_width` wide, its text wrapping; returns the button, named by
/// the caption.
pub fn choice_row(tree: &mut UiTree, parent: WidgetId, caption: &str, value: &str) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let choice = tree.button(row, value);
    let style = padded(tree, Sizing::Auto);
    tree.set_layout_style(choice, style);
    let least = size(tree, "control_width");
    at_least(tree, choice, least);
    set_width(tree, choice, Sizing::Fill);
    name(tree, choice, caption);
    choice
}

/// A setting row holding a slider at the range's start, the theme's
/// `control_width` wide and `slider_line` tall, named by the caption, and
/// its value's readout in the mono family, at least `readout_width` wide,
/// which the caller writes; returns both.
pub fn slider_row(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    range: SliderRange,
) -> (WidgetId, WidgetId) {
    let row = caption_row(tree, parent, caption);
    let slider = tree.slider(row, range, range.min());
    let width = size(tree, "control_width");
    set_width(tree, slider, Sizing::Px(width));
    let line = size(tree, "slider_line");
    restyle(tree, slider, |text| text.line_height = Some(line));
    name(tree, slider, caption);
    let readout = tree.label(row, "");
    let least = size(tree, "readout_width");
    at_least(tree, readout, least);
    mono(tree, readout);
    restyle(tree, readout, |text| text.align = Some(TextAlign::End));
    (slider, readout)
}

/// A setting row holding a toggle named by the caption; returns the toggle.
pub fn toggle_row(tree: &mut UiTree, parent: WidgetId, caption: &str, on: bool) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let toggle = tree.toggle(row, on);
    name(tree, toggle, caption);
    toggle
}

/// A column of tab buttons, each with a `tab_bar`-unit accent bar under its
/// caption that [`select_tab`] shows on the selected one; returns the
/// buttons and their bars.
pub fn tab_rail(
    tree: &mut UiTree,
    parent: WidgetId,
    captions: &[&str],
) -> (Vec<WidgetId>, Vec<WidgetId>) {
    let rail = column(tree, parent);
    let accent = tree.theme().tokens.colors.accent;
    let bar = LayoutStyle::default().with_size(Sizing::Fill, Sizing::Px(size(tree, "tab_bar")));
    let tab = |caption: &&str| {
        let tab = button(tree, rail, caption);
        let bar = tree.panel(tab, bar.clone());
        fill(tree, bar, accent);
        (tab, bar)
    };
    captions.iter().map(tab).unzip()
}

/// Shows the bar of `selected` and hides the others'.
pub fn select_tab(tree: &mut UiTree, bars: &[WidgetId], selected: usize) {
    for (at, bar) in bars.iter().enumerate() {
        let shown = [Visibility::Hidden, Visibility::Visible][usize::from(at == selected)];
        tree.set_visibility(*bar, shown);
    }
}

/// A muted label naming actions' bindings for the last device through
/// `template`, as `{action:state_pause} Pause`.
pub fn prompt_row(tree: &mut UiTree, parent: WidgetId, template: &str) -> WidgetId {
    let prompt = note(tree, parent, "");
    tree.set_prompt(prompt, template);
    prompt
}

/// Sets `id`'s width, its other layout kept.
pub fn set_width(tree: &mut UiTree, id: WidgetId, width: Sizing) {
    tree.edit_layout(id, |style| style.width = width);
}

/// Focuses the first of `candidates` that can take focus.
pub fn focus_first(tree: &mut UiTree, candidates: impl IntoIterator<Item = WidgetId>) {
    if let Some(id) = candidates.into_iter().find(|id| tree.can_focus(*id)) {
        tree.focus(id);
    }
}

/// Shows `id`, or takes it out of the layout.
pub fn show(tree: &mut UiTree, id: WidgetId, shown: bool) {
    let shown = [Visibility::Collapsed, Visibility::Visible][usize::from(shown)];
    tree.set_visibility(id, shown);
}

/// A menu: a [`screen`] at the theme's `menu_width`, a centred heading and
/// one [`button`] per item, keyed by the item's key and named by its
/// caption, so a handler reads `EventQueue::activated_key`.
#[derive(Debug, Clone)]
pub struct Menu {
    /// The screen's root.
    pub root: WidgetId,
    /// Its card, which holds the heading and the buttons.
    pub card: WidgetId,
    /// The heading.
    pub heading: WidgetId,
    /// The buttons, in the items' order.
    pub buttons: Vec<WidgetId>,
}

impl Menu {
    /// Builds the menu: `heading`, then a button per `(key, caption)` of
    /// `items`.
    pub fn new(tree: &mut UiTree, heading: &str, items: &[(&str, &str)]) -> Self {
        let width = size(tree, "menu_width");
        let (root, card) = screen(tree, width);
        let heading = self::heading(tree, card, heading, true);
        let buttons = items
            .iter()
            .map(|(key, caption)| {
                let button = button(tree, card, caption);
                tree.set_key(button, key);
                button
            })
            .collect();
        Self {
            root,
            card,
            heading,
            buttons,
        }
    }
}

/// A bar: a panel track and a fill whose width is a share of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    /// The track, in the panel's kind style.
    pub track: WidgetId,
    /// The fill.
    pub fill: WidgetId,
}

/// A meter under `parent`, `width` by `height` units, full in the theme's
/// accent.
pub fn meter(tree: &mut UiTree, parent: WidgetId, width: f32, height: f32) -> Meter {
    let track = tree.panel(
        parent,
        LayoutStyle::row().with_size(Sizing::Px(width), Sizing::Px(height)),
    );
    let bar = tree.panel(
        track,
        LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Fill),
    );
    let accent = tree.theme().tokens.colors.accent;
    fill(tree, bar, accent);
    Meter { track, fill: bar }
}

impl Meter {
    /// Fills `share` of the track, clamped to 0..=1 (NaN is 0), in `color`;
    /// an equal share and colour mark nothing.
    pub fn set(&self, tree: &mut UiTree, share: f32, color: [u8; 4]) {
        let share = if share.is_nan() {
            0.0
        } else {
            share.clamp(0.0, 1.0)
        };
        tree.edit_layout(self.fill, |style| {
            style.width = Sizing::Percent(share * 100.0);
        });
        fill(tree, self.fill, color);
    }
}

/// The most triangles a radial fill takes: a whole disc.
const RADIAL_TRIANGLES: usize = 64;

/// A radial fill under `parent` laid out by `style`: a mesh whose own box
/// paints nothing, which [`set_radial`] fills as a clock wipe.
pub fn radial(tree: &mut UiTree, parent: WidgetId, style: LayoutStyle) -> WidgetId {
    let id = tree.mesh(parent, style);
    fill(tree, id, [0; 4]);
    id
}

/// Fills `share` of radial fill `id`, clamped to 0..=1 (NaN is 0), in
/// `color`: a wipe from twelve o'clock clockwise over the disc its box
/// holds, in at most 64 triangles; none at 0, the whole disc at 1. An equal
/// share and colour mark nothing.
pub fn set_radial(tree: &mut UiTree, id: WidgetId, share: f32, color: [u8; 4]) {
    let share = if share.is_nan() {
        0.0
    } else {
        share.clamp(0.0, 1.0)
    };
    let triangles = (share * RADIAL_TRIANGLES as f32).ceil() as usize;
    let centre = Vec2::splat(0.5);
    let at = |turn: f32| {
        let angle = turn * TAU;
        centre + Vec2::new(angle.sin(), -angle.cos()) * 0.5
    };
    let mut vertices = Vec::with_capacity(triangles * 3);
    for step in 0..triangles {
        let from = share * step as f32 / triangles as f32;
        let to = share * (step + 1) as f32 / triangles as f32;
        vertices.extend([
            UiVertex::new(centre, color),
            UiVertex::new(at(from), color),
            UiVertex::new(at(to), color),
        ]);
    }
    tree.set_mesh(id, &vertices);
}

#[cfg(test)]
#[path = "../tests/ui/widgets.rs"]
mod tests;
