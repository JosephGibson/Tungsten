//! Builders over `UiTree` that every shell screen shares: a modal screen
//! (a scrim and its card), the run change's curtain, a heading, a note, a
//! button, a column, setting rows (a choice, a slider, a toggle), a tab row
//! and a prompt row. They compose the engine's widget kinds, so a game
//! changes a number here, not a widget. Every control takes its caption as
//! its accessible name, so a test finds it by name.

use tungsten::core::World;
use tungsten::core::text::TextAlign;
use tungsten::core::ui::{
    Align, CursorIcon, Edges, Justify, LayoutStyle, Sizing, SliderRange, StyleOverrides,
    TextOverrides, UiLayer, UiTree, Visibility, WidgetId,
};

use super::theme;

/// A modal screen: a root on the popup layer filling the viewport with the
/// theme's background at [`theme::SCRIM_ALPHA`], over the screen below,
/// and centred in it the card, a vertical scroll panel `width` units wide or
/// the viewport's width when narrower, as tall as its content up to the
/// viewport's height, past which it scrolls. Returns the root and the card.
pub fn screen(tree: &mut UiTree, width: f32) -> (WidgetId, WidgetId) {
    let full = Sizing::Percent(100.0);
    let mut style = LayoutStyle::column().with_size(full, full);
    style.align_items = Align::Center;
    style.justify_content = Justify::Center;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
    let [r, g, b, _] = tree.theme().tokens.colors.background;
    fill(tree, root, [r, g, b, theme::SCRIM_ALPHA]);
    let mut style = LayoutStyle::column()
        .with_padding(theme::PADDING)
        .with_gap(theme::GAP)
        .with_size(full, Sizing::Auto);
    style.max_width = Some(width);
    (root, tree.scroll_panel(root, style))
}

/// The run change's curtain: a modal root over every screen in the theme's
/// background, which the shell fades in and out.
pub fn curtain(tree: &mut UiTree) -> WidgetId {
    let full = Sizing::Percent(100.0);
    let root = tree.add_root(
        UiLayer::GamePopup,
        LayoutStyle::column().with_size(full, full),
    );
    tree.set_modal(root, true);
    let background = tree.theme().tokens.colors.background;
    fill(tree, root, background);
    root
}

/// Fills `id` with `color`, with no border; a clear fill leaves a
/// layout-only panel.
pub fn fill(tree: &mut UiTree, id: WidgetId, color: [u8; 4]) {
    let style = StyleOverrides {
        background: Some(color),
        border_width: Some(0.0),
        ..StyleOverrides::default()
    };
    tree.set_overrides(id, style);
}

/// Restyles `id`'s text with `edit`, its other overrides kept.
pub fn restyle(tree: &mut UiTree, id: WidgetId, edit: impl FnOnce(&mut TextOverrides)) {
    let mut style = tree.overrides(id).cloned().unwrap_or_default();
    edit(&mut style.text);
    tree.set_overrides(id, style);
}

/// A label under `parent` whose text `edit` restyles.
fn label(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    edit: impl FnOnce(&mut TextOverrides),
) -> WidgetId {
    let label = tree.label(parent, caption);
    restyle(tree, label, edit);
    label
}

/// A label in the heading family, `[size, line height]` large and centred.
pub fn display(tree: &mut UiTree, parent: WidgetId, caption: &str, size: [f32; 2]) -> WidgetId {
    let family = tree.theme().tokens.fonts.heading.clone();
    label(tree, parent, caption, |text| {
        text.family = Some(family);
        [text.size, text.line_height] = size.map(Some);
        text.align = Some(TextAlign::Center);
    })
}

/// The display labels whose size follows the text scale only up to a cap,
/// each (label, size, cap): a resource [`recap`] keeps current.
#[derive(Debug, Default)]
pub struct Capped {
    scale: f32,
    labels: Vec<(WidgetId, [f32; 2], f32)>,
}

/// Sizes `id` at `size` times the text scale, at most `cap` units.
fn fit(tree: &mut UiTree, id: WidgetId, size: [f32; 2], cap: f32) {
    let fit = (cap / tree.text_scale() / size[0]).min(1.0);
    restyle(tree, id, |text| {
        [text.size, text.line_height] = size.map(|each| Some(each * fit));
    });
}

/// Sizes display label `id` at `size` times the text scale, at most `cap`
/// units, so a long line keeps to one in the smallest viewport, and keeps
/// it so through text scale changes while it lives.
pub fn capped(world: &mut World, id: WidgetId, size: [f32; 2], cap: f32) {
    fit(world.resource_mut::<UiTree>(), id, size, cap);
    let capped = world.resource_mut::<Capped>();
    capped.labels.push((id, size, cap));
}

/// Sizes every capped label again when the text scale has moved, and drops
/// those gone with their screens.
pub fn recap(world: &mut World) {
    let mut capped = std::mem::take(world.resource_mut::<Capped>());
    let tree = world.resource_mut::<UiTree>();
    capped.labels.retain(|(id, ..)| tree.contains(*id));
    if capped.scale != tree.text_scale() {
        capped.scale = tree.text_scale();
        for &(id, size, cap) in &capped.labels {
            fit(tree, id, size, cap);
        }
    }
    *world.resource_mut::<Capped>() = capped;
}

/// A heading in the heading family, larger, never heavier; a menu's is
/// centred, a card's sits at the start.
pub fn heading(tree: &mut UiTree, parent: WidgetId, caption: &str, centred: bool) -> WidgetId {
    let size = [theme::HEADING_SIZE, theme::HEADING_LINE];
    let heading = display(tree, parent, caption, size);
    restyle(tree, heading, |text| {
        text.align = centred.then_some(TextAlign::Center);
    });
    heading
}

/// A label in the theme's muted text colour.
pub fn note(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let color = tree.theme().tokens.colors.text_muted;
    label(tree, parent, caption, |text| text.color = Some(color))
}

/// Small muted print, [`theme::SMALL`].
pub fn small(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let note = note(tree, parent, caption);
    restyle(tree, note, |text| {
        [text.size, text.line_height] = theme::SMALL.map(Some);
    });
    note
}

/// Sets `id`'s text in the theme's mono family: the shell's numbers.
pub fn mono(tree: &mut UiTree, id: WidgetId) {
    let family = tree.theme().tokens.fonts.mono.clone();
    restyle(tree, id, |text| text.family = Some(family));
}

/// Sets `id`'s text in the theme's danger colour: a destructive action.
pub fn danger(tree: &mut UiTree, id: WidgetId) {
    let color = tree.theme().tokens.colors.danger;
    restyle(tree, id, |text| text.color = Some(color));
}

/// A control's layout: padded, `width` wide.
fn padded(width: Sizing) -> LayoutStyle {
    let [x, y] = theme::BUTTON_PADDING;
    let mut style = LayoutStyle::default().with_size(width, Sizing::Auto);
    style.padding = Edges::symmetric(x, y);
    style
}

/// A button holding `caption`, padded, with the pointer cursor and its
/// caption as its accessible name.
pub fn button(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let id = tree.button(parent, caption);
    tree.set_layout_style(id, padded(Sizing::Auto));
    name(tree, id, caption);
    id
}

/// Gives a control `caption` as its accessible name and the pointer cursor.
pub fn name(tree: &mut UiTree, control: WidgetId, caption: &str) {
    tree.set_accessible_name(control, Some(caption.to_owned()));
    tree.set_cursor(control, Some(CursorIcon::Pointer));
}

/// A layout-only panel under `parent` whose children flow along `style`'s
/// direction, `align`ed on it.
fn group(tree: &mut UiTree, parent: WidgetId, style: LayoutStyle, align: Align) -> WidgetId {
    let mut style = style.with_gap(theme::GAP);
    style.align_items = align;
    let id = tree.panel(parent, style);
    fill(tree, id, [0; 4]);
    id
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
    let mut style = tree.layout_style(id).cloned().unwrap_or_default();
    (style.width, style.min_width) = (Sizing::Auto, Some(least));
    tree.set_layout_style(id, style);
}

/// A setting row holding a button that shows `value` and cycles it when
/// activated, sharing the row with its caption, at least
/// [`theme::CONTROL_WIDTH`] wide, its text wrapping; returns the button.
pub fn choice_row(tree: &mut UiTree, parent: WidgetId, caption: &str, value: &str) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let choice = tree.button(row, value);
    tree.set_layout_style(choice, padded(Sizing::Auto));
    at_least(tree, choice, theme::CONTROL_WIDTH);
    set_width(tree, choice, Sizing::Fill);
    name(tree, choice, caption);
    choice
}

/// A setting row holding a slider at the range's start, [`theme::SLIDER_LINE`]
/// tall, and its value's readout in the mono family, which the caller
/// writes; returns both.
pub fn slider_row(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    range: SliderRange,
) -> (WidgetId, WidgetId) {
    let row = caption_row(tree, parent, caption);
    let slider = tree.slider(row, range, range.min());
    set_width(tree, slider, Sizing::Px(theme::CONTROL_WIDTH));
    restyle(tree, slider, |text| {
        text.line_height = Some(theme::SLIDER_LINE)
    });
    name(tree, slider, caption);
    let readout = tree.label(row, "");
    at_least(tree, readout, theme::READOUT_WIDTH);
    mono(tree, readout);
    restyle(tree, readout, |text| text.align = Some(TextAlign::End));
    (slider, readout)
}

/// A setting row holding a toggle; returns the toggle.
pub fn toggle_row(tree: &mut UiTree, parent: WidgetId, caption: &str, on: bool) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let toggle = tree.toggle(row, on);
    name(tree, toggle, caption);
    toggle
}

/// A column of tab buttons, each with a 3-unit accent bar under its caption
/// that [`select_tab`] shows on the selected one; returns the buttons and
/// their bars.
pub fn tab_rail(
    tree: &mut UiTree,
    parent: WidgetId,
    captions: &[&str],
) -> (Vec<WidgetId>, Vec<WidgetId>) {
    let rail = column(tree, parent);
    let accent = tree.theme().tokens.colors.accent;
    let bar = LayoutStyle::default().with_size(Sizing::Fill, Sizing::Px(3.0));
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
    let mut style = tree.layout_style(id).cloned().unwrap_or_default();
    style.width = width;
    tree.set_layout_style(id, style);
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

/// A label's or a button's text, its spans joined.
#[must_use]
pub fn text_of(tree: &UiTree, id: WidgetId) -> String {
    let spans = tree.text(id).map(|styled| styled.spans.as_slice());
    spans
        .unwrap_or_default()
        .iter()
        .map(|span| span.text.as_str())
        .collect()
}
