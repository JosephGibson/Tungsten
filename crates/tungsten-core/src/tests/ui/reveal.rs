use glam::Vec2;

use super::*;
use crate::AssetRegistry;
use crate::text::{StyledText, TextOutline, TextSpan};
use crate::ui::{
    FixedAdvanceMeasure, LayoutStyle, NamedStyle, PaintCommand, Theme, UiDebug, UiLayer,
};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);

/// A tree whose label `line` is outlined and holds a coloured span, and
/// whose `button` holds a caption: (tree, line, button).
fn tree() -> (UiTree, WidgetId, WidgetId) {
    let mut theme = Theme::default();
    theme.names.text_styles.insert(
        "ringed".to_owned(),
        NamedStyle {
            outline: Some(TextOutline::new([0, 0, 0, 255], 1.0)),
            ..NamedStyle::default()
        },
    );
    let mut tree = UiTree::new();
    tree.set_theme(theme);
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let line = tree.label(root, "");
    let mut text = StyledText::from("Evening, ");
    text.spans
        .push(TextSpan::new("warden").with_color([255, 200, 0, 255]));
    tree.set_text(line, text);
    tree.set_text_style(line, Some("ringed"));
    let button = tree.button(root, "Go on");
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    (tree, line, button)
}

fn painted(tree: &mut UiTree) -> PaintList {
    let mut list = PaintList::default();
    tree.paint(1.0, &UiDebug::default(), &AssetRegistry::new(), &mut list);
    list
}

fn text_node(tree: &UiTree, id: WidgetId) -> TextNodeId {
    let holder = tree.text_holder(id).unwrap();
    tree.node(holder).unwrap().text_id.unwrap()
}

#[test]
fn a_reveal_writes_its_count_for_the_label_and_its_copies() {
    let (mut tree, line, button) = tree();
    let mut world = World::new();
    let reveals = UiReveals::of(&mut world);
    reveals.set(line, Some(4));
    reveals.set(button, Some(2));
    let mut list = painted(&mut tree);
    assert!(list.reveals.is_empty(), "the paint writes none itself");
    reveals.write(&tree, &mut list);
    let base = text_node(&tree, line);
    let caption = text_node(&tree, button);
    assert_eq!(list.reveals, [(base, 4), (caption, 2)]);

    // The line's outline copies draw its plain view; each command's base
    // finds its count.
    let looked_up: Vec<(bool, Option<u32>)> = list
        .commands
        .iter()
        .filter_map(|command| match *command {
            PaintCommand::Text { node, .. } if node.base() == base => Some((
                node.is_plain(),
                list.reveals
                    .iter()
                    .find(|(id, _)| *id == node.base())
                    .map(|&(_, count)| count),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(looked_up.len(), 9, "eight copies and the text");
    assert!(looked_up[..8].iter().all(|&(plain, _)| plain));
    assert!(looked_up.iter().all(|&(_, count)| count == Some(4)));

    // A count back to all, or a label gone, leaves the list.
    reveals.set(line, None);
    tree.remove(button);
    reveals.write(&tree, &mut list);
    assert!(list.reveals.is_empty());
    assert_eq!(
        reveals.get(button),
        Some(2),
        "kept until the game clears it"
    );
}

#[test]
fn a_reveal_marks_no_layout_and_leaves_the_tree_its_size() {
    let (mut tree, line, _) = tree();
    let _ = painted(&mut tree);
    tree.clear_paint_dirty();
    let mut world = World::new();
    let reveals = UiReveals::of(&mut world);
    let before = reveals.revision();
    for count in 0..20 {
        reveals.set(line, Some(count));
    }
    assert_eq!(reveals.revision(), before + 20);
    reveals.set(line, Some(19));
    assert_eq!(
        reveals.revision(),
        before + 20,
        "an equal count changes nothing"
    );
    assert!(!tree.is_layout_dirty() && !tree.is_paint_dirty());
    // The counts live beside the tree: its types keep their sizes, which
    // the app's re-boxed tree and the views' guards pin.
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        (
            std::mem::size_of::<UiTree>(),
            std::mem::size_of::<crate::ui::node::Node>()
        ),
        (UI_TREE_SIZE, NODE_SIZE)
    );
}

/// `UiTree`'s and `Node`'s sizes at 0.120, on 64-bit targets.
const UI_TREE_SIZE: usize = 3568;
const NODE_SIZE: usize = 736;
