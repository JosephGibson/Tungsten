use glam::Vec2;

use super::*;
use crate::text::{TextStyle, TextWrap};
use crate::ui::theme::{ResolvedStyle, StyleOverrides, TextOverrides, Theme, WidgetState};
use crate::ui::tree::StyleStats;
use crate::ui::{FixedAdvanceMeasure, SliderRange, UiLayer, UiTree, WidgetId, WidgetKind};

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
        Some(WidgetState::Disabled),
        "a widget under a disabled one applies its disabled variant (M56)"
    );

    tree.set_enabled(button, true);
    tree.resolve_styles();
    assert_eq!(
        resolved(&tree, button).background,
        tree.theme().kinds.button.normal.background
    );
}

#[test]
fn hover_and_press_layer_over_visible_focus_and_disabled_applies_alone() {
    let (mut tree, _root, button, _label) = clean_button();
    let theme = tree.theme().clone();
    let kind = &theme.kinds.button;
    let focus_border = theme.tokens.colors.focus;
    let hover_fill = kind.hovered.background.unwrap();
    let press_fill = kind.pressed.background.unwrap();

    tree.set_hovered(Some(button));
    assert_eq!(tree.hovered(), Some(button));
    assert!(!tree.is_paint_dirty(), "hover marks the style, not paint");
    let stats = tree.resolve_styles();
    assert_eq!(stats.resolved, 1);
    assert!(tree.is_paint_dirty(), "the new fill marks paint");
    assert!(!tree.is_layout_dirty());
    assert_eq!(tree.widget_state(button), Some(WidgetState::Hovered));
    assert_eq!(resolved(&tree, button).background, hover_fill);

    assert!(tree.focus(button));
    tree.resolve_styles();
    assert_eq!(
        tree.widget_state(button),
        Some(WidgetState::Hovered),
        "hover is above focus"
    );
    let style = resolved(&tree, button);
    assert_eq!(style.border, focus_border, "the focus border stays");
    assert_eq!(style.border_width, 2.0);
    assert_eq!(style.background, hover_fill, "the hover fill over it");

    tree.set_pressed(Some(button));
    tree.resolve_styles();
    assert_eq!(tree.widget_state(button), Some(WidgetState::Pressed));
    let style = resolved(&tree, button);
    assert_eq!(style.background, press_fill, "pressed wins the fill");
    assert_eq!(style.border, focus_border);

    assert!(tree.focus_visible());
    tree.set_focus_visible(false);
    tree.resolve_styles();
    assert_eq!(tree.focused(), Some(button), "hidden focus is still focus");
    let style = resolved(&tree, button);
    assert_eq!(
        style.border, kind.normal.border,
        "a hidden focus drops the focused variant"
    );
    assert_eq!(style.border_width, 1.0);
    assert_eq!(style.background, press_fill);
    tree.set_pressed(None);
    tree.set_hovered(None);
    tree.resolve_styles();
    assert_eq!(tree.widget_state(button), Some(WidgetState::Normal));
    assert_eq!(resolved(&tree, button).background, kind.normal.background);
    tree.set_focus_visible(true);
    tree.resolve_styles();
    assert_eq!(tree.widget_state(button), Some(WidgetState::Focused));
    assert_eq!(resolved(&tree, button).border, focus_border);

    tree.set_hovered(Some(button));
    tree.set_pressed(Some(button));
    tree.set_enabled(button, false);
    assert_eq!(
        (tree.focused(), tree.hovered(), tree.pressed()),
        (None, None, None),
        "disabling releases all three"
    );
    tree.set_hovered(Some(button));
    tree.set_pressed(Some(button));
    tree.resolve_styles();
    assert_eq!(tree.widget_state(button), Some(WidgetState::Disabled));
    let style = resolved(&tree, button);
    assert_eq!(
        style.background,
        kind.disabled.background.unwrap(),
        "disabled applies alone"
    );
    assert_eq!(style.border, kind.normal.border);
    assert_eq!(style.text.color, theme.tokens.colors.disabled_text);

    // `theme::resolve` keeps applying the one state it is given.
    let base = theme.base_text();
    let hovered = crate::ui::theme::resolve(
        &theme,
        crate::ui::WidgetKind::Button,
        WidgetState::Hovered,
        &StyleOverrides::default(),
        &base,
    );
    assert_eq!(hovered.background, hover_fill);
    assert_eq!(hovered.border, kind.normal.border);
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

    // The background only: resolved with the label under it (M56), no
    // text change, no layout dirt.
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
            resolved: 2,
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
        !tree.is_layout_dirty(),
        "a colour change alone repaints and re-sets no node (M56)"
    );
    assert!(tree.is_paint_dirty());
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

#[test]
fn images_are_transparent_like_labels_and_resolve_under_their_parent() {
    let theme = Theme::default();
    let image = theme.kind(crate::ui::WidgetKind::Image);
    assert_eq!(image, &theme.kinds.image);
    assert_eq!(image.normal.background, [0, 0, 0, 0]);
    assert_eq!(image.normal.border_width, 0.0);
    assert!(image.normal.text.is_empty());
    assert_eq!(
        image.variant(WidgetState::Focused),
        Some(&StyleOverrides::default())
    );

    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::default());
    let image = tree.image(
        root,
        crate::assets::SpriteAssetId::new(0),
        Vec2::new(8.0, 8.0),
        LayoutStyle::default(),
    );
    settle(&mut tree);
    let style = resolved(&tree, image);
    assert_eq!(style.background, [0, 0, 0, 0]);
    assert_eq!(style.text, theme.base_text(), "text inherits from the root");
}

#[test]
fn toggles_and_sliders_have_built_in_looks_from_the_tokens() {
    let theme = Theme::default();
    let tokens = &theme.tokens;
    let toggle = theme.kind(WidgetKind::Toggle);
    assert_eq!(toggle, &theme.kinds.toggle);
    assert_eq!(toggle.normal.background, tokens.colors.surface);
    assert_eq!(toggle.normal.border, tokens.colors.border);
    assert_eq!(toggle.normal.border_width, 1.0);
    assert_eq!(toggle.normal.corner_radius, tokens.radii.sm);
    assert_eq!(toggle.normal.text.color, Some(tokens.colors.accent));
    assert_eq!(toggle.focused.border, Some(tokens.colors.focus));
    assert_eq!(toggle.focused.border_width, Some(2.0));
    assert_eq!(
        toggle.disabled.text.color,
        Some(tokens.colors.disabled_text)
    );
    assert_eq!(toggle.disabled.background, None);
    assert_eq!(toggle.hovered.background, Some([56, 57, 64, 255]));
    assert_eq!(toggle.pressed.background, Some([16, 17, 24, 255]));

    let slider = theme.kind(WidgetKind::Slider);
    assert_eq!(slider, &theme.kinds.slider);
    assert_eq!(slider.normal.background, tokens.colors.border, "the track");
    assert_eq!(slider.normal.border, tokens.colors.border);
    assert_eq!(slider.normal.border_width, 0.0);
    assert_eq!(slider.normal.corner_radius, tokens.radii.sm);
    assert_eq!(slider.normal.text.color, Some(tokens.colors.accent));
    assert_eq!(slider.focused.border, Some(tokens.colors.focus));
    assert_eq!(slider.focused.border_width, Some(2.0));
    assert_eq!(
        slider.disabled.text.color,
        Some(tokens.colors.disabled_text)
    );
    assert_eq!(slider.hovered.text.color, Some([110, 180, 255, 255]));
    assert_eq!(slider.hovered.background, None);
    assert_eq!(slider.pressed.text.color, Some([54, 124, 223, 255]));
}

#[test]
fn a_controls_colour_only_change_repaints_and_a_line_height_change_relayouts() {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Game, LayoutStyle::row());
    let toggle = tree.toggle(root, true);
    let slider = tree.slider(root, SliderRange::new(0.0, 1.0), 0.5);
    settle(&mut tree);
    let theme = Theme::default();

    // Hovering a slider moves its text colour alone: paint, no layout.
    tree.set_hovered(Some(slider));
    let stats = tree.resolve_styles();
    assert_eq!(
        stats,
        StyleStats {
            resolved: 1,
            text_changed: 1,
            measurement_changed: 0,
        }
    );
    assert!(!tree.is_layout_dirty(), "a control holds no text node");
    assert!(tree.is_paint_dirty());
    assert_eq!(
        resolved(&tree, slider).text.color,
        theme.kinds.slider.hovered.text.color.expect("set")
    );
    settle(&mut tree);

    // Disabling a toggle moves its text colour alone, too.
    tree.set_enabled(toggle, false);
    tree.clear_paint_dirty();
    let stats = tree.resolve_styles();
    assert_eq!(stats.text_changed, 1);
    assert_eq!(stats.measurement_changed, 0);
    assert!(!tree.is_layout_dirty());
    assert!(tree.is_paint_dirty());
    assert_eq!(
        resolved(&tree, toggle).text.color,
        theme.tokens.colors.disabled_text
    );
    settle(&mut tree);

    // A line height sizes a control: its root relayouts.
    tree.set_overrides(
        toggle,
        StyleOverrides {
            text: TextOverrides {
                line_height: Some(30.0),
                ..TextOverrides::default()
            },
            ..StyleOverrides::default()
        },
    );
    let stats = tree.resolve_styles();
    assert_eq!(stats.measurement_changed, 1);
    assert!(tree.is_layout_dirty());
    settle(&mut tree);
    assert_eq!(
        tree.rect(toggle).expect("laid out").size(),
        Vec2::new(30.0, 30.0)
    );
}
