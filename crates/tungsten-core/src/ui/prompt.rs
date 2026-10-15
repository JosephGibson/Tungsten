//! Binding-aware prompts (M56): a label's text built from a template whose
//! `{action:NAME}` placeholders show the binding the player would press,
//! for the device they last used. [`ui_route_system`](super::ui_route_system)
//! resolves them after each routing; a prompt whose device kind and
//! bindings are unchanged costs slice compares and no allocation.

use crate::input::{ActionMap, Binding};

use super::id::WidgetId;
use super::route::InputDevice;
use super::tree::UiTree;

/// What a placeholder shows for an action with no binding (Q8).
const UNBOUND: &str = "Unbound";
const OPEN: &str = "{action:";

/// One piece of a template.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    Literal(String),
    /// An action, and the bindings its label was last built from.
    Action {
        name: String,
        seen: Vec<Binding>,
    },
}

/// A label's prompt: its template's parts and the device kind of the last
/// resolution.
#[derive(Debug, Clone)]
pub(crate) struct Prompt {
    pub(crate) label: WidgetId,
    parts: Vec<Part>,
    /// Whether the last resolution read pad bindings; `None` before the
    /// first.
    pad: Option<bool>,
}

/// Splits `template` into literals and `{action:NAME}` placeholders; every
/// other brace is literal, as is a placeholder with an empty name or one
/// left open.
fn parse(template: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut rest = template;
    while let Some(at) = rest.find(OPEN) {
        let after = &rest[at + OPEN.len()..];
        let name = after.find('}').map(|end| &after[..end]);
        match name {
            Some(name) if !name.is_empty() && !name.contains('{') => {
                literal.push_str(&rest[..at]);
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(&mut literal)));
                }
                parts.push(Part::Action {
                    name: name.to_owned(),
                    seen: Vec::new(),
                });
                rest = &after[name.len() + 1..];
            }
            _ => {
                literal.push_str(&rest[..at + OPEN.len()]);
                rest = after;
            }
        }
    }
    literal.push_str(rest);
    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }
    parts
}

/// `action`'s bindings in `map`; none without a map.
fn bindings_of<'a>(map: Option<&'a ActionMap>, action: &str) -> &'a [Binding] {
    map.map_or(&[], |map| map.bindings(action))
}

/// The binding a prompt shows (Q8): the first of the device's kind (keys
/// and pointer: key, then mouse, then wheel; pad: button, then stick), else
/// the first of any kind.
fn chosen(bindings: &[Binding], pad: bool) -> Option<Binding> {
    let rank = |binding: &Binding| -> Option<u8> {
        match (binding, pad) {
            (Binding::Key { .. }, false) | (Binding::GamepadButton { .. }, true) => Some(0),
            (Binding::Mouse { .. }, false) | (Binding::GamepadAxis { .. }, true) => Some(1),
            (Binding::Scroll { .. }, false) => Some(2),
            _ => None,
        }
    };
    (0..3)
        .find_map(|want| bindings.iter().find(|b| rank(b) == Some(want)))
        .or_else(|| bindings.first())
        .copied()
}

impl Prompt {
    fn new(label: WidgetId, template: &str) -> Self {
        Self {
            label,
            parts: parse(template),
            pad: None,
        }
    }

    /// The text with every placeholder empty, until the first resolution.
    fn literal_text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                Part::Literal(text) => Some(text.as_str()),
                Part::Action { .. } => None,
            })
            .collect()
    }

    /// Whether the device kind or any action's bindings moved since the
    /// last resolution.
    fn stale(&self, map: Option<&ActionMap>, pad: bool) -> bool {
        self.pad != Some(pad)
            || self.parts.iter().any(|part| match part {
                Part::Action { name, seen } => bindings_of(map, name) != seen.as_slice(),
                Part::Literal(_) => false,
            })
    }

    /// Rebuilds the text against `map` and the device kind.
    fn resolve(&mut self, map: Option<&ActionMap>, pad: bool) -> String {
        self.pad = Some(pad);
        let mut text = String::new();
        for part in &mut self.parts {
            match part {
                Part::Literal(literal) => text.push_str(literal),
                Part::Action { name, seen } => {
                    let bindings = bindings_of(map, name);
                    seen.clear();
                    seen.extend_from_slice(bindings);
                    match chosen(bindings, pad) {
                        Some(binding) => text.push_str(&binding.label()),
                        None => text.push_str(UNBOUND),
                    }
                }
            }
        }
        text
    }
}

impl UiTree {
    /// Gives a label, or a button's label, a prompt (M56): `template`'s
    /// `{action:NAME}` placeholders show the binding of `NAME` for the
    /// device the player last used, every other brace literal. Keys and the
    /// pointer show a key, else a mouse button, else the wheel; a gamepad
    /// shows a pad button, else a stick direction; else the first binding
    /// of any kind, else `Unbound`. The routing resolves it after each
    /// routing, rebuilding the text when the device kind or the bindings
    /// changed; until the first, the label shows the literal text with each
    /// placeholder empty. [`Self::set_text`] on the label clears it.
    /// Returns `false`, setting nothing, when `id` holds no text.
    pub fn set_prompt(&mut self, id: WidgetId, template: &str) -> bool {
        let Some(target) = self.text_holder(id) else {
            return false;
        };
        let prompt = Prompt::new(target, template);
        let text = prompt.literal_text();
        self.prompts.retain(|prompt| prompt.label != target);
        self.prompts.push(prompt);
        self.replace_text(target, text.into());
        true
    }

    /// Whether `id`, a label or a button's label, has a prompt.
    #[must_use]
    pub fn has_prompt(&self, id: WidgetId) -> bool {
        self.text_holder(id)
            .is_some_and(|target| self.prompts.iter().any(|prompt| prompt.label == target))
    }

    /// Takes the prompts out, for a resolution that reads the action map
    /// while the tree is not borrowed.
    pub(crate) fn take_prompts(&mut self) -> Vec<Prompt> {
        std::mem::take(&mut self.prompts)
    }

    /// Puts back the prompts [`Self::take_prompts`] took, after the texts
    /// they changed, dropping those whose label is gone.
    pub(crate) fn restore_prompts(
        &mut self,
        mut prompts: Vec<Prompt>,
        texts: Vec<(WidgetId, String)>,
    ) {
        for (label, text) in texts {
            self.replace_text(label, text.into());
        }
        prompts.retain(|prompt| self.contains(prompt.label));
        prompts.append(&mut self.prompts);
        self.prompts = prompts;
    }
}

/// Resolves `prompts` against `map` for `device`, pushing each changed
/// label's new text onto `texts`.
pub(crate) fn resolve_prompts(
    prompts: &mut [Prompt],
    map: Option<&ActionMap>,
    device: InputDevice,
    texts: &mut Vec<(WidgetId, String)>,
) {
    let pad = device == InputDevice::Gamepad;
    for prompt in prompts {
        if prompt.stale(map, pad) {
            texts.push((prompt.label, prompt.resolve(map, pad)));
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/prompt.rs"]
mod tests;
