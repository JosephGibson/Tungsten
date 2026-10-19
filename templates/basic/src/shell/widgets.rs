//! Builders over `UiTree` that every shell screen shares: a modal screen, a
//! heading, a note, a button, a column, setting rows (a choice, a slider, a
//! toggle), a tab row and a prompt row. They compose the engine's widget
//! kinds, so a game changes a number here, not a widget. Every control
//! takes its caption as its accessible name, so a test finds it by name.

use tungsten::core::ui::{
    Align, Anchor, CursorIcon, Edges, LayoutStyle, Sizing, SliderRange, StyleOverrides,
    TextOverrides, UiLayer, UiTree, Visibility, WidgetId,
};

use super::theme;

/// A modal screen root on the popup layer, `width` units wide, centred: a
/// column that stretches its children to one width.
pub fn screen(tree: &mut UiTree, width: f32) -> WidgetId {
    let mut style = LayoutStyle::column()
        .with_padding(theme::PADDING)
        .with_gap(theme::GAP)
        .with_size(Sizing::Px(width), Sizing::Auto)
        .anchored(Anchor::Center, Default::default());
    style.align_items = Align::Stretch;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
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

/// A label under `parent` whose text `edit` restyles.
fn label(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    edit: impl FnOnce(&mut TextOverrides),
) -> WidgetId {
    let label = tree.label(parent, caption);
    let mut style = StyleOverrides::default();
    edit(&mut style.text);
    tree.set_overrides(label, style);
    label
}

/// A label `[size, line height]` large.
pub fn sized_label(tree: &mut UiTree, parent: WidgetId, caption: &str, size: [f32; 2]) -> WidgetId {
    label(tree, parent, caption, |text| {
        text.size = Some(size[0]);
        text.line_height = Some(size[1]);
    })
}

/// A heading: larger text, never heavier.
pub fn heading(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    sized_label(
        tree,
        parent,
        caption,
        [theme::HEADING_SIZE, theme::HEADING_LINE],
    )
}

/// A label in the theme's muted text colour.
pub fn note(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let color = tree.theme().tokens.colors.text_muted;
    label(tree, parent, caption, |text| text.color = Some(color))
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

/// A setting row: a caption [`theme::CAPTION_WIDTH`] wide, for the control
/// the caller adds after it. Returns the row.
pub fn caption_row(tree: &mut UiTree, parent: WidgetId, caption: &str) -> WidgetId {
    let row = row(tree, parent);
    let label = tree.label(row, caption);
    set_width(tree, label, Sizing::Px(theme::CAPTION_WIDTH));
    row
}

/// A setting row holding a button that shows `value` and cycles it when
/// activated; returns the button.
pub fn choice_row(tree: &mut UiTree, parent: WidgetId, caption: &str, value: &str) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let choice = tree.button(row, value);
    tree.set_layout_style(choice, padded(Sizing::Px(theme::CONTROL_WIDTH)));
    name(tree, choice, caption);
    choice
}

/// A setting row holding a slider at the range's start; returns it.
pub fn slider_row(
    tree: &mut UiTree,
    parent: WidgetId,
    caption: &str,
    range: SliderRange,
) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let slider = tree.slider(row, range, range.min());
    set_width(tree, slider, Sizing::Px(theme::CONTROL_WIDTH));
    name(tree, slider, caption);
    slider
}

/// A setting row holding a toggle; returns the toggle.
pub fn toggle_row(tree: &mut UiTree, parent: WidgetId, caption: &str, on: bool) -> WidgetId {
    let row = caption_row(tree, parent, caption);
    let toggle = tree.toggle(row, on);
    name(tree, toggle, caption);
    toggle
}

/// A row of tab buttons, each with a 3-unit accent bar under its caption
/// that [`select_tab`] shows on the selected one; returns the buttons and
/// their bars.
pub fn tab_row(
    tree: &mut UiTree,
    parent: WidgetId,
    captions: &[&str],
) -> (Vec<WidgetId>, Vec<WidgetId>) {
    let row = row(tree, parent);
    let accent = tree.theme().tokens.colors.accent;
    let bar = LayoutStyle::default().with_size(Sizing::Fill, Sizing::Px(3.0));
    let tab = |caption: &&str| {
        let tab = button(tree, row, caption);
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
