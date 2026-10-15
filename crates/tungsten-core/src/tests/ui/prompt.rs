use super::*;
use crate::input::{
    AxisDirection, GamepadAxis, GamepadButton, KeyCode, MouseButton, ScrollDirection,
};
use crate::ui::{FixedAdvanceMeasure, LayoutStyle, UiLayer};

const KEY_K: Binding = Binding::Key {
    code: KeyCode::KeyK,
};
const SPACE: Binding = Binding::Key {
    code: KeyCode::Space,
};
const LEFT: Binding = Binding::Mouse {
    button: MouseButton::Left,
};
const WHEEL: Binding = Binding::Scroll {
    direction: ScrollDirection::Up,
};
const PAD_A: Binding = Binding::GamepadButton {
    button: GamepadButton::South,
};
const STICK_UP: Binding = Binding::GamepadAxis {
    axis: GamepadAxis::LeftStickY,
    direction: AxisDirection::Positive,
};

fn map(bindings: &[(&str, Vec<Binding>)]) -> ActionMap {
    let mut map = ActionMap::new();
    for (action, list) in bindings {
        map.replace_bindings(*action, list.clone());
    }
    map
}

/// A tree with one label holding `template`'s prompt: (tree, label).
fn labelled(template: &str) -> (UiTree, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let label = tree.label(root, "");
    assert!(tree.set_prompt(label, template));
    (tree, label)
}

fn text(tree: &UiTree, id: WidgetId) -> String {
    tree.accessible_name(id).unwrap_or_default().to_owned()
}

/// Resolves the tree's prompts as the routing does; returns how many
/// labels changed.
fn resolve(tree: &mut UiTree, map: &ActionMap, device: InputDevice) -> usize {
    let mut prompts = tree.take_prompts();
    let mut texts = Vec::new();
    resolve_prompts(&mut prompts, Some(map), device, &mut texts);
    let changed = texts.len();
    tree.restore_prompts(prompts, texts);
    changed
}

#[test]
fn templates_split_into_literals_and_placeholders() {
    assert_eq!(
        parse("Press {action:jump} or {action:dash}!"),
        [
            Part::Literal("Press ".into()),
            Part::Action {
                name: "jump".into(),
                seen: Vec::new()
            },
            Part::Literal(" or ".into()),
            Part::Action {
                name: "dash".into(),
                seen: Vec::new()
            },
            Part::Literal("!".into()),
        ]
    );
    for literal in ["{jump}", "{action:}", "{action:jump", "{{action:x{y}", "}{"] {
        assert_eq!(
            parse(literal),
            [Part::Literal(literal.into())],
            "{literal} stays literal"
        );
    }
    let (mut tree, label) = labelled("Hold {curly} {action:nothing_here}.");
    assert_eq!(
        text(&tree, label),
        "Hold {curly} .",
        "placeholders empty first"
    );
    resolve(&mut tree, &map(&[]), InputDevice::Keyboard);
    assert_eq!(text(&tree, label), "Hold {curly} Unbound.");
}

#[test]
fn the_binding_follows_the_device_kind_then_falls_back() {
    let all = [STICK_UP, PAD_A, WHEEL, LEFT, KEY_K];
    assert_eq!(chosen(&all, false), Some(KEY_K));
    assert_eq!(chosen(&all, true), Some(PAD_A));
    assert_eq!(chosen(&[STICK_UP, WHEEL, LEFT], false), Some(LEFT));
    assert_eq!(chosen(&[STICK_UP, WHEEL], false), Some(WHEEL));
    assert_eq!(chosen(&[KEY_K, STICK_UP], true), Some(STICK_UP));
    assert_eq!(
        chosen(&[WHEEL, KEY_K], true),
        Some(WHEEL),
        "none of the kind: the first of any"
    );
    assert_eq!(chosen(&[], true), None);

    let (mut tree, label) = labelled("{action:jump}");
    let bindings = map(&[("jump", vec![PAD_A, SPACE])]);
    resolve(&mut tree, &bindings, InputDevice::Pointer);
    assert_eq!(text(&tree, label), "Space", "the pointer reads keys");
    resolve(&mut tree, &bindings, InputDevice::Gamepad);
    assert_eq!(text(&tree, label), "A");
}

#[test]
fn a_prompt_rebuilds_when_its_bindings_or_the_device_change_and_not_otherwise() {
    let mut store = FixedAdvanceMeasure::default();
    let (mut tree, label) = labelled("Jump: {action:jump}");
    let mut bindings = map(&[("jump", vec![SPACE, PAD_A])]);
    assert_eq!(resolve(&mut tree, &bindings, InputDevice::Keyboard), 1);
    assert_eq!(text(&tree, label), "Jump: Space");
    tree.layout(glam::Vec2::new(400.0, 300.0), &mut store);
    tree.clear_paint_dirty();
    for device in [InputDevice::Keyboard, InputDevice::Pointer] {
        assert_eq!(resolve(&mut tree, &bindings, device), 0, "{device:?}");
    }
    assert!(!tree.is_layout_dirty() && !tree.is_paint_dirty());

    bindings.replace_bindings("jump", vec![KEY_K, PAD_A]);
    assert_eq!(resolve(&mut tree, &bindings, InputDevice::Keyboard), 1);
    assert_eq!(text(&tree, label), "Jump: K");
    // A whole new map with equal bindings, as a reload replaces it.
    let reloaded = map(&[("jump", vec![KEY_K, PAD_A])]);
    assert_eq!(resolve(&mut tree, &reloaded, InputDevice::Keyboard), 0);
    assert_eq!(resolve(&mut tree, &reloaded, InputDevice::Gamepad), 1);
    assert_eq!(text(&tree, label), "Jump: A");

    // `set_text` clears the prompt; removal drops it.
    tree.set_text(label, "plain");
    assert!(!tree.has_prompt(label));
    assert_eq!(resolve(&mut tree, &reloaded, InputDevice::Keyboard), 0);
    assert_eq!(text(&tree, label), "plain");
    let root = tree.roots()[0];
    let button = tree.button(root, "x");
    assert!(tree.set_prompt(button, "{action:jump}"));
    assert!(tree.has_prompt(button));
    tree.remove(button);
    assert!(tree.prompts.is_empty());
    assert!(
        !tree.set_prompt(root, "{action:jump}"),
        "a panel holds no text"
    );
}
