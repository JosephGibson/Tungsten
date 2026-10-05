use glam::Vec2;

use super::*;
use crate::text::{TextStyle, TextWrap};
use crate::ui::theme::{ResolvedStyle, StyleOverrides, TextOverrides, Theme, WidgetState};
use crate::ui::tree::StyleStats;
use crate::ui::{FixedAdvanceMeasure, UiLayer, UiTree, WidgetId};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

/// A root panel holding a button, resolved and clean.
fn clean_button() -> (UiTree, WidgetId, WidgetId, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let button = tree.button(root, "Start");
    let label = tree.children(button)[0];
    settle(&mut tree);
    (tree, root, button, label)
}

fn settle(tree: &mut UiTree) {
    let mut text = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut text);
    tree.clear_paint_dirty();
    assert!(!tree.is_layout_dirty());
    assert!(!tree.is_paint_dirty());
}

fn resolved(tree: &UiTree, id: WidgetId) -> &ResolvedStyle {
    tree.resolved_style(id).expect("resolved")
}

fn disabled_variant(text: TextOverrides, background: Option<[u8; 4]>) -> Theme {
    let mut theme = Theme::default();
    theme.kinds.button.disabled = StyleOverrides {
        background,
        text,
        ..StyleOverrides::default()
    };
    theme
}

#[test]
fn the_default_theme_is_dark_with_the_manifest_families() {
    let theme = Theme::default();
    assert_eq!(theme.tokens.fonts.body, "sans");
    assert_eq!(theme.tokens.fonts.heading, "sans");
    assert_eq!(theme.tokens.fonts.mono, "mono");
    assert_eq!(theme.tokens.colors.surface[3], 255);
    assert!(theme.tokens.colors.surface[0] < 64, "a dark surface");
    assert!(theme.tokens.colors.text[0] > 192, "light text");
    let base = theme.base_text();
    assert_eq!(base.family, "sans");
    assert_eq!(base.size, theme.tokens.text.size);
    assert_eq!(base.line_height, theme.tokens.text.line_height);
    assert_eq!(base.weight, 400);
    assert_eq!(base.color, theme.tokens.colors.text);
    assert!(theme.kinds.label.normal.text.is_empty());
    assert_eq!(
        theme.kinds.button.normal.text.color,
        Some(theme.tokens.colors.accent_text)
    );
}

#[test]
fn a_button_resolves_its_normal_visual_when_enabled_and_unfocused() {
    let (tree, root, button, label) = clean_button();
    let theme = tree.theme();
    let style = resolved(&tree, button);
    assert_eq!(style.background, theme.kinds.button.normal.background);
    assert_eq!(style.border, theme.kinds.button.normal.border);
    assert_eq!(style.border_width, 1.0);
    assert_eq!(style.corner_radius, theme.tokens.radii.md);
    assert_eq!(style.text.color, theme.tokens.colors.accent_text);
    assert_eq!(style.text.weight, 600);
    assert_eq!(
        resolved(&tree, root).background,
        theme.kinds.panel.normal.background
    );
    assert_eq!(resolved(&tree, root).text, theme.base_text());
    assert_eq!(tree.widget_state(button), Some(WidgetState::Normal));
    assert_eq!(tree.widget_state(label), Some(WidgetState::Normal));
    assert_eq!(tree.theme_epoch().get(), 0);
}

#[test]
fn the_focused_then_the_disabled_variant_apply_by_state() {
    let (mut tree, _root, button, _label) = clean_button();
    let focus_border = tree.theme().tokens.colors.focus;
    assert!(tree.focus(button));
    assert_eq!(tree.focused(), Some(button));
    assert!(tree.is_paint_dirty());
    assert!(!tree.is_layout_dirty());
    let stats = tree.resolve_styles();
    assert_eq!(
        stats.resolved, 1,
        "the button alone: its text did not change"
    );
    assert_eq!(tree.widget_state(button), Some(WidgetState::Focused));
    let style = resolved(&tree, button);
    assert_eq!(style.border, focus_border);
    assert_eq!(style.border_width, 2.0);

    tree.set_enabled(button, false);
    assert_eq!(tree.focused(), None, "a disabled widget holds no focus");
    tree.resolve_styles();
    assert_eq!(tree.widget_state(button), Some(WidgetState::Disabled));
    let theme = tree.theme();
    let style = resolved(&tree, button);
    assert_eq!(
        style.background,
        theme.kinds.button.disabled.background.unwrap()
    );
    assert_eq!(
        style.border, theme.kinds.button.normal.border,
        "the focused border is gone"
    );
    assert_eq!(style.text.color, theme.tokens.colors.disabled_text);
    assert_eq!(
        tree.widget_state(tree.children(button)[0]),
        Some(WidgetState::Normal)
    );

    tree.set_enabled(button, true);
    tree.resolve_styles();
    assert_eq!(
        resolved(&tree, button).background,
        tree.theme().kinds.button.normal.background
    );
}

#[test]
fn a_node_override_beats_the_variant() {
    let (mut tree, _root, button, _label) = clean_button();
    tree.set_overrides(
        button,
        StyleOverrides {
            border: Some([1, 2, 3, 4]),
            corner_radius: Some(9.0),
            ..StyleOverrides::default()
        },
    );
    assert!(tree.is_paint_dirty());
    assert!(tree.focus(button));
    let stats = tree.resolve_styles();
    assert_eq!(stats.resolved, 1);
    let style = resolved(&tree, button);
    assert_eq!(
        style.border,
        [1, 2, 3, 4],
        "the node's border beats the focused variant's"
    );
    assert_eq!(
        style.border_width, 2.0,
        "what the node leaves alone, the variant sets"
    );
    assert_eq!(style.corner_radius, 9.0);
    assert_eq!(tree.overrides(button).unwrap().corner_radius, Some(9.0));

    // Equal overrides mark nothing.
    settle(&mut tree);
    let same = tree.overrides(button).unwrap().clone();
    tree.set_overrides(button, same);
    assert!(!tree.is_paint_dirty());
    assert_eq!(tree.resolve_styles().resolved, 0);
}

#[test]
fn text_inherits_and_non_text_does_not() {
    let (mut tree, root, button, label) = clean_button();
    let theme = tree.theme().clone();
    let label_style = resolved(&tree, label);
    assert_eq!(
        label_style.text.color, theme.tokens.colors.accent_text,
        "the button's colour"
    );
    assert_eq!(label_style.text.weight, 600, "the button's weight");
    assert_eq!(
        label_style.text.family, theme.tokens.fonts.body,
        "the theme's base family"
    );
    assert_eq!(
        label_style.background,
        [0, 0, 0, 0],
        "the label's own background, not the panel's"
    );
    assert_eq!(label_style.border_width, 0.0);
    assert_eq!(resolved(&tree, button).text.size, theme.tokens.text.size);

    // A panel override on text passes to a label under it; its background stays its own.
    let sibling = tree.label(root, "hud");
    tree.set_overrides(
        root,
        StyleOverrides {
            background: Some([9, 9, 9, 255]),
            text: TextOverrides {
                size: Some(32.0),
                line_height: Some(40.0),
                italic: Some(true),
                ..TextOverrides::default()
            },
            ..StyleOverrides::default()
        },
    );
    let stats = tree.resolve_styles();
    assert_eq!(
        stats.resolved, 4,
        "the root, the new label, the button and its label"
    );
    assert_eq!(stats.text_changed, 4);
    assert_eq!(
        stats.measurement_changed, 4,
        "the size reaches every node; the new label resolves for the first time"
    );
    let sibling_style = resolved(&tree, sibling);
    assert_eq!(sibling_style.text.size, 32.0);
    assert_eq!(sibling_style.text.line_height, 40.0);
    assert!(sibling_style.text.italic);
    assert_eq!(sibling_style.text.color, theme.tokens.colors.text);
    assert_eq!(sibling_style.background, [0, 0, 0, 0]);
    let label_style = resolved(&tree, label);
    assert_eq!(label_style.text.size, 32.0, "two levels down");
    assert_eq!(
        label_style.text.color, theme.tokens.colors.accent_text,
        "the button's colour still"
    );
    assert_eq!(
        resolved(&tree, button).background,
        theme.kinds.button.normal.background
    );
    assert!(tree.is_layout_dirty(), "a text size change relayouts");
}

#[test]
fn a_nodes_own_text_override_beats_the_inherited_one() {
    let (mut tree, _root, button, label) = clean_button();
    tree.set_overrides(
        label,
        StyleOverrides {
            text: TextOverrides {
                color: Some([7, 7, 7, 255]),
                weight: Some(300),
                wrap: Some(TextWrap::None),
                ..TextOverrides::default()
            },
            ..StyleOverrides::default()
        },
    );
    let stats = tree.resolve_styles();
    assert_eq!(stats.resolved, 1);
    assert_eq!(stats.text_changed, 1);
    assert_eq!(stats.measurement_changed, 1, "weight and wrap measure");
    let style = resolved(&tree, label);
    assert_eq!(style.text.color, [7, 7, 7, 255]);
    assert_eq!(style.text.weight, 300);
    assert_eq!(style.text.layout.wrap, TextWrap::None);
    assert_eq!(
        resolved(&tree, button).text.weight,
        600,
        "the parent keeps its own"
    );
}

#[test]
fn set_theme_bumps_the_epoch_and_re_resolves_everything() {
    let (mut tree, root, button, label) = clean_button();
    let epoch = tree.theme_epoch();
    let mut theme = tree.theme().clone();
    theme.tokens.colors.text = [1, 1, 1, 255];
    theme.kinds.panel.normal.background = [2, 2, 2, 255];
    tree.set_theme(theme.clone());
    assert_eq!(tree.theme_epoch(), epoch.next());
    assert!(tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    assert_eq!(tree.theme(), &theme);
    let stats = tree.resolve_styles();
    assert_eq!(stats.resolved, 3);
    assert_eq!(resolved(&tree, root).background, [2, 2, 2, 255]);
    assert_eq!(resolved(&tree, root).text.color, [1, 1, 1, 255]);
    assert_eq!(
        resolved(&tree, button).text.color,
        theme.tokens.colors.accent_text
    );
    assert_eq!(
        resolved(&tree, label).text.color,
        theme.tokens.colors.accent_text
    );
}

#[test]
fn an_unchanged_tree_re_resolves_nothing() {
    let (mut tree, root, button, _label) = clean_button();
    assert_eq!(tree.resolve_styles(), StyleStats::default());
    // Changes that touch no style resolve nothing either.
    tree.set_role(button, crate::ui::Role::Dialog);
    tree.set_accessible_name(root, Some("x".to_owned()));
    tree.set_hit_testable(button, false);
    assert!(tree.set_text(button, "Go"));
    assert_eq!(tree.resolve_styles(), StyleStats::default());
    // A second root with no change stays untouched when the first changes.
    let other = tree.add_root(UiLayer::Debug, LayoutStyle::default());
    tree.label(other, "fps");
    settle(&mut tree);
    tree.set_enabled(button, false);
    let stats = tree.resolve_styles();
    assert_eq!(stats.resolved, 2, "the button and its label");
}

#[test]
fn a_disabled_variant_reports_what_it_changes_on_a_clean_tree() {
    // A larger text size: a measurement change, on the button and its label.
    let (mut tree, _root, button, _label) = clean_button();
    tree.set_theme(disabled_variant(
        TextOverrides {
            size: Some(24.0),
            ..TextOverrides::default()
        },
        None,
    ));
    settle(&mut tree);
    tree.set_enabled(button, false);
    let stats = tree.resolve_styles();
    assert_eq!(
        stats,
        StyleStats {
            resolved: 2,
            text_changed: 2,
            measurement_changed: 2,
        }
    );
    assert!(tree.is_layout_dirty());

    // The background only: resolved, no text change, no layout dirt.
    let (mut tree, _root, button, _label) = clean_button();
    tree.set_theme(disabled_variant(
        TextOverrides::default(),
        Some([5, 5, 5, 255]),
    ));
    settle(&mut tree);
    tree.set_enabled(button, false);
    let stats = tree.resolve_styles();
    assert_eq!(
        stats,
        StyleStats {
            resolved: 1,
            text_changed: 0,
            measurement_changed: 0,
        }
    );
    assert!(!tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    assert_eq!(resolved(&tree, button).background, [5, 5, 5, 255]);

    // The text colour: a text-style change on both, no measurement change.
    let (mut tree, _root, button, label) = clean_button();
    tree.set_theme(disabled_variant(
        TextOverrides {
            color: Some([6, 6, 6, 255]),
            ..TextOverrides::default()
        },
        None,
    ));
    settle(&mut tree);
    tree.set_enabled(button, false);
    let stats = tree.resolve_styles();
    assert_eq!(
        stats,
        StyleStats {
            resolved: 2,
            text_changed: 2,
            measurement_changed: 0,
        }
    );
    assert!(
        tree.is_layout_dirty(),
        "a colour change re-sets the node, so its root relayouts (Q10)"
    );
    assert_eq!(resolved(&tree, label).text.color, [6, 6, 6, 255]);
}

#[test]
fn resolution_runs_inside_layout() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let label = tree.label(root, "x");
    assert_eq!(
        tree.resolved_style(label),
        None,
        "nothing resolved before the first layout"
    );
    let mut text = FixedAdvanceMeasure::default();
    tree.layout(VIEWPORT, &mut text);
    assert_eq!(resolved(&tree, label).text, Theme::default().base_text());
    assert_eq!(tree.resolve_styles(), StyleStats::default());
    assert_eq!(
        resolved(&tree, root).text,
        TextStyle::new("sans", 16.0, 20.0).with_color([235, 235, 240, 255])
    );
}
