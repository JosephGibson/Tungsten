//! The level editor's prompt: one line of text, the smallest field that
//! lets the editor rename an entry, set an object's override and link a
//! place, which have no gesture.
//!
//! It is not a text widget. While one is open the editor reads the frame's
//! raw key presses as characters on a US layout, [`typed`], and no other
//! editor key or button acts: Enter commits, Escape cancels, Backspace
//! takes the last character back. It types what an ID, a path and a small
//! JSON value need: letters, digits, space and `_ - . , = [ ] ' "`. A
//! layout that puts those elsewhere types the US key's character (known
//! issues), and the `editor` script action's `prompt` takes any text.

use tungsten_core::input::KeyCode;

/// What a prompt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ask {
    /// A new ID for the pick: a `rename`.
    Rename,
    /// `<path> = <value>` for the picked object: a `set` of `set.<path>`,
    /// its value JSON when it reads as JSON and text otherwise; an empty
    /// value removes the override.
    Set,
    /// `<room>.<place>` for the picked point or area: a `link`; empty
    /// unlinks.
    Link,
}

impl Ask {
    /// Every prompt, in the panel's order.
    pub(crate) const ALL: [Self; 3] = [Self::Rename, Self::Set, Self::Link];

    /// Its name in a script.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Set => "set",
            Self::Link => "link",
        }
    }

    /// The prompt `name` names.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ask| ask.name() == name)
    }
}

/// A prompt under way: what it asks, for which entry of which room, and
/// the text typed so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Prompt {
    pub(crate) ask: Ask,
    pub(crate) room: String,
    pub(crate) id: String,
    pub(crate) text: String,
}

impl Prompt {
    /// The line the panel shows for it: what is asked, the text, a caret,
    /// and how it ends.
    pub(crate) fn line(&self) -> String {
        let Self { ask, id, text, .. } = self;
        let asks = match ask {
            Ask::Rename => format!("Rename `{id}` to"),
            Ask::Set => format!("Set on `{id}`, as path = value"),
            Ask::Link => format!("Link `{id}` to, as room.place"),
        };
        format!("{asks}: {text}_  (Enter commits, Esc cancels)")
    }
}

/// What a key press does to an open prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Typed {
    Char(char),
    Backspace,
    Commit,
    Cancel,
    /// A key the prompt has no character for: nothing.
    Nothing,
}

/// The keys core's `KeyCode` has no name for, by their physical key.
const PERIOD: KeyCode = KeyCode::Other(winit::keyboard::KeyCode::Period as u32);
const COMMA: KeyCode = KeyCode::Other(winit::keyboard::KeyCode::Comma as u32);
const QUOTE: KeyCode = KeyCode::Other(winit::keyboard::KeyCode::Quote as u32);

/// What `code` does to a prompt, with Shift held or not, on a US layout.
pub(crate) fn typed(code: KeyCode, shift: bool) -> Typed {
    let plain = |c: char| {
        if shift {
            Typed::Nothing
        } else {
            Typed::Char(c)
        }
    };
    match code {
        KeyCode::Enter => Typed::Commit,
        KeyCode::Escape => Typed::Cancel,
        KeyCode::Backspace => Typed::Backspace,
        KeyCode::Space => Typed::Char(' '),
        KeyCode::Minus => Typed::Char(if shift { '_' } else { '-' }),
        KeyCode::Equal => plain('='),
        KeyCode::BracketLeft => plain('['),
        KeyCode::BracketRight => plain(']'),
        PERIOD => plain('.'),
        COMMA => plain(','),
        QUOTE => Typed::Char(if shift { '"' } else { '\'' }),
        _ => {
            // `KeyA` to `KeyZ` and `Digit0` to `Digit9`, by their names.
            let name = format!("{code:?}");
            let one = |prefix: &str| {
                let mut rest = name.strip_prefix(prefix)?.chars();
                rest.next().filter(|_| rest.next().is_none())
            };
            match (one("Key"), one("Digit")) {
                (Some(letter), _) if shift => Typed::Char(letter),
                (Some(letter), _) => Typed::Char(letter.to_ascii_lowercase()),
                (_, Some(digit)) => plain(digit),
                _ => Typed::Nothing,
            }
        }
    }
}
