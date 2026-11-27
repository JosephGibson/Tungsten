//! The text field (redo 8.1, P8-5): a focusable box holding one label, its
//! value or its placeholder, edited by typed text and the editing keys
//! while it holds focus. Its state lives in a boxed
//! [`Control`](super::control::Control), so the tree's nodes keep their
//! size.

use crate::text::{StyledText, TextNodeId, TextSpan};

use super::control::Control;
use super::id::WidgetId;
use super::node::WidgetKind;
use super::style::LayoutStyle;
use super::tree::UiTree;

/// A field's state: its value, its caret as a character index, its bound,
/// its placeholder, the value it had when it took focus, and the scratch
/// text node layout measures the value before the caret through.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct FieldState {
    pub(crate) value: String,
    pub(crate) caret: usize,
    pub(crate) max: Option<usize>,
    pub(crate) placeholder: String,
    pub(crate) on_focus: Option<String>,
    pub(crate) scratch: Option<TextNodeId>,
    /// The width of the value before the caret, in UI units, from the last
    /// layout.
    pub(crate) caret_x: f32,
}

impl FieldState {
    fn chars(&self) -> usize {
        self.value.chars().count()
    }

    /// The byte offset of character index `at`.
    fn byte(&self, at: usize) -> usize {
        self.value
            .char_indices()
            .nth(at)
            .map_or(self.value.len(), |(byte, _)| byte)
    }

    /// The value before the caret.
    pub(crate) fn before_caret(&self) -> &str {
        &self.value[..self.byte(self.caret)]
    }
}

/// An editing key a focused field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKey {
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
}

impl UiTree {
    /// A text field under `parent` holding `value`, its caret at its end: a
    /// focusable, interactive box with the button's look holding one
    /// label, which shows the value, or the placeholder in the theme's muted
    /// text colour while the value is empty. While it holds focus, typed
    /// text inserts at the caret, Backspace, Delete, Left, Right, Home and
    /// End edit and move, Enter commits and Escape restores the value it had
    /// on focus and lets focus go; its keys never reach the game. Panics
    /// when `parent` is not in the tree.
    pub fn text_field(&mut self, parent: WidgetId, value: impl Into<String>) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::TextField, LayoutStyle::default());
        self.label(id, "");
        let value = value.into();
        let caret = value.chars().count();
        if let Some(node) = self.node_mut(id) {
            node.control = Some(Control::Field(Box::new(FieldState {
                value,
                caret,
                ..FieldState::default()
            })));
        }
        self.show_field(id);
        id
    }

    /// Replaces a field's value, its caret at the end, held to its bound;
    /// `false`, changing nothing, for an equal value or a widget that is
    /// not a field.
    pub fn set_field_value(&mut self, id: WidgetId, value: impl Into<String>) -> bool {
        let value = value.into();
        let Some(field) = self.field_mut(id) else {
            return false;
        };
        let value = match field.max {
            Some(max) => value.chars().take(max).collect(),
            None => value,
        };
        if field.value == value {
            return false;
        }
        field.caret = value.chars().count();
        field.value = value;
        self.show_field(id);
        true
    }

    /// A field's value; `None` for a widget that is not one.
    #[must_use]
    pub fn field_value(&self, id: WidgetId) -> Option<&str> {
        self.field(id).map(|field| field.value.as_str())
    }

    /// A field's caret, a character index into its value.
    #[must_use]
    pub fn field_caret(&self, id: WidgetId) -> Option<usize> {
        self.field(id).map(|field| field.caret)
    }

    /// Bounds a field's value to `max` characters, cutting a longer one;
    /// `None` takes off the bound.
    pub fn set_field_max(&mut self, id: WidgetId, max: Option<usize>) {
        let Some(field) = self.field_mut(id) else {
            return;
        };
        field.max = max;
        if let Some(max) = max
            && field.chars() > max
        {
            let cut: String = field.value.chars().take(max).collect();
            field.value = cut;
            field.caret = field.caret.min(max);
            self.show_field(id);
        }
    }

    /// The text a field shows while its value is empty.
    pub fn set_placeholder(&mut self, id: WidgetId, text: impl Into<String>) {
        let text = text.into();
        let Some(field) = self.field_mut(id) else {
            return;
        };
        if field.placeholder == text {
            return;
        }
        field.placeholder = text;
        self.show_field(id);
    }

    pub(crate) fn field(&self, id: WidgetId) -> Option<&FieldState> {
        match self.node(id)?.control.as_ref()? {
            Control::Field(field) => Some(field),
            _ => None,
        }
    }

    pub(crate) fn field_mut(&mut self, id: WidgetId) -> Option<&mut FieldState> {
        match self.node_mut(id)?.control.as_mut()? {
            Control::Field(field) => Some(field),
            _ => None,
        }
    }

    /// Inserts `ch` at a field's caret, past it; `false` at its bound.
    pub(crate) fn field_insert(&mut self, id: WidgetId, ch: char) -> bool {
        let Some(field) = self.field_mut(id) else {
            return false;
        };
        if field.max.is_some_and(|max| field.chars() >= max) {
            return false;
        }
        let at = field.byte(field.caret);
        field.value.insert(at, ch);
        field.caret += 1;
        self.show_field(id);
        true
    }

    /// Applies an editing `key`; whether the value changed. A caret move
    /// alone lays the field's root out again, so the caret follows.
    pub(crate) fn field_key(&mut self, id: WidgetId, key: FieldKey) -> bool {
        let Some(field) = self.field_mut(id) else {
            return false;
        };
        let chars = field.chars();
        let before = field.caret;
        let mut edited = false;
        match key {
            FieldKey::Backspace if field.caret > 0 => {
                let at = field.byte(field.caret - 1);
                field.value.remove(at);
                field.caret -= 1;
                edited = true;
            }
            FieldKey::Delete if field.caret < chars => {
                let at = field.byte(field.caret);
                field.value.remove(at);
                edited = true;
            }
            FieldKey::Left => field.caret = field.caret.saturating_sub(1),
            FieldKey::Right => field.caret = (field.caret + 1).min(chars),
            FieldKey::Home => field.caret = 0,
            FieldKey::End => field.caret = chars,
            FieldKey::Backspace | FieldKey::Delete => {}
        }
        let moved = field.caret != before;
        if edited {
            self.show_field(id);
        } else if moved {
            self.mark_layout_dirty(id);
        }
        edited
    }

    /// A field took focus (`true`) or let it go: it keeps its value as it
    /// was then, for Escape, its caret at the end; or forgets it.
    pub(crate) fn field_focus(&mut self, id: WidgetId, gained: bool) {
        let Some(field) = self.field_mut(id) else {
            return;
        };
        if gained {
            field.on_focus = Some(field.value.clone());
            let end = field.chars();
            if field.caret != end {
                field.caret = end;
                self.mark_layout_dirty(id);
            }
        } else {
            field.on_focus = None;
        }
        self.mark_paint_dirty();
    }

    /// Escape: the value back as it was on focus; whether it changed.
    pub(crate) fn field_restore(&mut self, id: WidgetId) -> bool {
        let Some(field) = self.field_mut(id) else {
            return false;
        };
        let Some(before) = field.on_focus.clone() else {
            return false;
        };
        if field.value == before {
            return false;
        }
        field.caret = before.chars().count();
        field.value = before;
        self.show_field(id);
        true
    }

    /// Enter: the value committed, kept as the one Escape restores.
    pub(crate) fn field_commit(&mut self, id: WidgetId) {
        if let Some(field) = self.field_mut(id) {
            field.on_focus = Some(field.value.clone());
        }
    }

    /// Shows a field's value in its label, or its placeholder in the muted
    /// text colour while the value is empty.
    fn show_field(&mut self, id: WidgetId) {
        let Some(field) = self.field(id) else {
            return;
        };
        let text = if field.value.is_empty() && !field.placeholder.is_empty() {
            let muted = self.theme().tokens.colors.text_muted;
            StyledText {
                spans: vec![TextSpan::new(field.placeholder.clone()).with_color(muted)],
            }
        } else {
            StyledText::from(field.value.as_str())
        };
        let Some(label) = self
            .node(id)
            .and_then(|node| node.children.first().copied())
        else {
            return;
        };
        self.replace_text(label, text);
        self.mark_layout_dirty(id);
    }
}

#[cfg(test)]
#[path = "../tests/ui/field.rs"]
mod tests;
