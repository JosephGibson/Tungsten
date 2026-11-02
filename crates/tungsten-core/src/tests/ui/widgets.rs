use glam::Vec2;

use super::*;
use crate::ui::{
    CursorIcon, Edges, FixedAdvanceMeasure, LayoutStyle, Rect, Sizing, SliderRange, Theme, UiKey,
    UiLayer, UiTree, Visibility, WidgetId,
};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);
const RED: [u8; 4] = [255, 0, 0, 255];

fn settle(tree: &mut UiTree) {
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    tree.clear_paint_dirty();
}

/// A tree with one full-viewport `Game` column to build under.
fn tree_and_root() -> (UiTree, WidgetId) {
    let mut tree = UiTree::new();
    let full = Sizing::Percent(100.0);
    let root = tree.add_root(UiLayer::Game, LayoutStyle::column().with_size(full, full));
    (tree, root)
}

#[test]
fn a_screen_is_a_modal_scrim_and_a_scroll_card() {
    let mut tree = UiTree::new();
    let (root, card) = screen(&mut tree, 300.0);
    assert!(tree.modal(root));
    assert_eq!(tree.layer(root), Some(UiLayer::GamePopup));
    let scrim = tree.theme().tokens.colors.scrim;
    let own = tree.overrides(root).expect("in the tree");
    assert_eq!((own.background, own.border_width), (Some(scrim), Some(0.0)));
    assert_eq!(tree.parent(card), Some(root));
    assert_eq!(tree.scroll(card), Some(0.0), "the card scrolls");
    let style = tree.layout_style(card).expect("in the tree");
    let spacing = tree.theme().tokens.spacing;
    assert_eq!(
        (style.padding, style.gap),
        (Edges::all(spacing.lg), spacing.sm)
    );
    settle(&mut tree);
    assert_eq!(
        tree.rect(root),
        Some(Rect::from_pos_size(Vec2::ZERO, VIEWPORT))
    );
    let rect = tree.rect(card).expect("laid out");
    assert_eq!(rect.width(), 300.0, "its design width");
    assert_eq!(rect.min.x, 250.0, "centred");
    tree.layout(Vec2::new(200.0, 600.0), &mut FixedAdvanceMeasure::default());
    assert_eq!(
        tree.rect(card).map(|rect| rect.width()),
        Some(200.0),
        "a narrower viewport narrows it"
    );
}

#[test]
fn rows_name_their_controls_by_caption() {
    let (mut tree, root) = tree_and_root();
    let save = button(&mut tree, root, "Save");
    let choice = choice_row(&mut tree, root, "Window", "Fullscreen");
    let (slider, readout) = slider_row(&mut tree, root, "Music", SliderRange::new(0.0, 1.0));
    let toggle = toggle_row(&mut tree, root, "Mute", true);
    for (control, caption) in [
        (save, "Save"),
        (choice, "Window"),
        (slider, "Music"),
        (toggle, "Mute"),
    ] {
        assert_eq!(tree.accessible_name(control), Some(caption));
        assert_eq!(tree.cursor(control), Some(CursorIcon::Pointer));
    }
    assert_eq!(
        tree.plain_text(choice),
        "Fullscreen",
        "the choice shows its value"
    );
    assert_eq!(tree.toggled(toggle), Some(true));
    assert_eq!(tree.slider_value(slider), Some(0.0), "at the range's start");
    let row = tree.parent(choice).expect("in its row");
    let caption = tree.children(row)[0];
    assert_eq!(tree.plain_text(caption), "Window");
    assert_eq!(
        tree.parent(readout),
        tree.parent(slider),
        "the readout beside its slider"
    );
    let mono = tree.theme().tokens.fonts.mono.clone();
    let text = &tree.overrides(readout).expect("in the tree").text;
    assert_eq!(text.family, Some(mono));
    assert!(tree.bare(row), "a row paints nothing");
}

#[test]
fn a_tab_rail_shows_one_bar() {
    let (mut tree, root) = tree_and_root();
    let (tabs, bars) = tab_rail(&mut tree, root, &["Display", "Audio", "Interface"]);
    assert_eq!((tabs.len(), bars.len()), (3, 3));
    for (tab, caption) in tabs.iter().zip(["Display", "Audio", "Interface"]) {
        assert_eq!(tree.accessible_name(*tab), Some(caption));
    }
    select_tab(&mut tree, &bars, 1);
    let shown: Vec<_> = bars.iter().map(|bar| tree.visibility(*bar)).collect();
    assert_eq!(
        shown,
        [
            Some(Visibility::Hidden),
            Some(Visibility::Visible),
            Some(Visibility::Hidden)
        ]
    );
    let accent = tree.theme().tokens.colors.accent;
    assert_eq!(
        tree.overrides(bars[1]).and_then(|own| own.background),
        Some(accent)
    );
}

#[test]
fn a_menu_keys_its_buttons() {
    let mut tree = UiTree::new();
    let menu = Menu::new(
        &mut tree,
        "Paused",
        &[("resume", "Resume"), ("quit", "Quit to title")],
    );
    assert_eq!(tree.plain_text(menu.heading), "Paused");
    assert_eq!(tree.text_style(menu.heading), Some("heading"));
    assert_eq!(menu.buttons.len(), 2);
    assert_eq!(tree.key(menu.buttons[0]), Some(UiKey::new("resume")));
    assert_eq!(tree.key_name(menu.buttons[1]), Some("quit"));
    assert_eq!(
        tree.find_key(menu.root, UiKey::new("quit")),
        Some(menu.buttons[1])
    );
    assert_eq!(tree.accessible_name(menu.buttons[1]), Some("Quit to title"));
    assert_eq!(tree.parent(menu.buttons[0]), Some(menu.card));
    let width = tree
        .layout_style(menu.card)
        .and_then(|style| style.max_width);
    assert_eq!(width, Some(260.0), "the template's menu width");
}

#[test]
fn a_meter_set_twice_marks_once() {
    let (mut tree, root) = tree_and_root();
    let meter = meter(&mut tree, root, 100.0, 10.0);
    settle(&mut tree);
    meter.set(&mut tree, 0.5, RED);
    assert!(tree.is_layout_dirty() && tree.is_paint_dirty());
    settle(&mut tree);
    meter.set(&mut tree, 0.5, RED);
    assert!(
        !tree.is_layout_dirty() && !tree.is_paint_dirty(),
        "an equal share and colour mark nothing"
    );
    meter.set(&mut tree, 0.5, [0, 0, 255, 255]);
    assert!(
        !tree.is_layout_dirty() && tree.is_paint_dirty(),
        "a colour alone repaints"
    );
}

#[test]
fn a_meter_at_half_fills_half_its_track() {
    let (mut tree, root) = tree_and_root();
    let meter = meter(&mut tree, root, 100.0, 10.0);
    let width = |tree: &UiTree, id| tree.rect(id).map(|rect: Rect| rect.width());
    settle(&mut tree);
    assert_eq!(width(&tree, meter.fill), Some(100.0), "full at first");
    meter.set(&mut tree, 0.5, RED);
    settle(&mut tree);
    assert_eq!(width(&tree, meter.track), Some(100.0));
    assert_eq!(width(&tree, meter.fill), Some(50.0));
    assert_eq!(
        tree.overrides(meter.fill).and_then(|own| own.background),
        Some(RED)
    );
    for (share, filled) in [(f32::NAN, 0.0), (-1.0, 0.0), (2.0, 100.0), (0.25, 25.0)] {
        meter.set(&mut tree, share, RED);
        settle(&mut tree);
        assert_eq!(width(&tree, meter.fill), Some(filled), "{share}");
    }
}

#[test]
fn a_radial_fill_at_a_quarter_covers_the_top_right() {
    let (mut tree, root) = tree_and_root();
    let style = LayoutStyle::default().with_size(Sizing::Px(40.0), Sizing::Px(40.0));
    let id = radial(&mut tree, root, style);
    set_radial(&mut tree, id, 0.25, RED);
    let vertices = tree.mesh_vertices(id);
    assert_eq!(vertices.len(), 16 * 3, "a quarter of 64 triangles");
    assert!(vertices.iter().all(|vertex| vertex.color == RED));
    assert!(
        vertices
            .iter()
            .all(|vertex| vertex.position.x >= 0.5 - 1e-6 && vertex.position.y <= 0.5 + 1e-6),
        "inside the top right quarter"
    );
    let near = |a: Vec2, b: Vec2| a.distance(b) < 1e-5;
    assert!(
        near(vertices[1].position, Vec2::new(0.5, 0.0)),
        "from twelve o'clock"
    );
    assert!(
        near(vertices[vertices.len() - 1].position, Vec2::new(1.0, 0.5)),
        "to three, clockwise"
    );

    set_radial(&mut tree, id, 1.0, RED);
    let vertices = tree.mesh_vertices(id);
    assert_eq!(vertices.len(), 64 * 3, "the whole disc");
    assert!(
        near(vertices[vertices.len() - 1].position, Vec2::new(0.5, 0.0)),
        "round to twelve"
    );
    assert!(
        !set_radial_marks(&mut tree, id, 1.0),
        "an equal fill marks nothing"
    );
}

/// Sets `id`'s fill to `share` in red on a clean tree; whether it marked paint.
fn set_radial_marks(tree: &mut UiTree, id: WidgetId, share: f32) -> bool {
    settle(tree);
    set_radial(tree, id, share, RED);
    tree.is_paint_dirty()
}

#[test]
fn a_radial_fill_at_zero_draws_nothing() {
    let (mut tree, root) = tree_and_root();
    let style = LayoutStyle::default().with_size(Sizing::Px(40.0), Sizing::Px(40.0));
    let id = radial(&mut tree, root, style);
    set_radial(&mut tree, id, 0.5, RED);
    for share in [0.0, -0.5, f32::NAN] {
        set_radial(&mut tree, id, share, RED);
        assert!(tree.mesh_vertices(id).is_empty(), "{share}");
    }
    let own = tree.overrides(id).expect("in the tree");
    assert_eq!(
        (own.background, own.border_width),
        (Some([0; 4]), Some(0.0)),
        "its box paints nothing"
    );
}

#[test]
fn builders_follow_the_theme_sizes() {
    // The template's numbers without sizes in the theme.
    let (mut tree, root) = tree_and_root();
    let padding = |tree: &UiTree, id| tree.layout_style(id).map(|style| style.padding);
    let plain = button(&mut tree, root, "Plain");
    assert_eq!(padding(&tree, plain), Some(Edges::symmetric(14.0, 5.0)));
    let (slider, readout) = slider_row(&mut tree, root, "Volume", SliderRange::new(0.0, 1.0));
    assert_eq!(
        tree.layout_style(slider).map(|style| style.width),
        Some(Sizing::Px(170.0))
    );
    assert_eq!(
        tree.overrides(slider).and_then(|own| own.text.line_height),
        Some(18.0)
    );
    assert_eq!(
        tree.layout_style(readout).and_then(|style| style.min_width),
        Some(64.0)
    );

    let mut theme = Theme::default();
    theme.tokens.spacing.lg = 10.0;
    theme.tokens.spacing.sm = 4.0;
    for (name, value) in [
        ("button_padding_x", 20.0),
        ("button_padding_y", 7.0),
        ("control_width", 120.0),
        ("readout_width", 50.0),
        ("slider_line", 24.0),
        ("menu_width", 300.0),
        ("tab_bar", 5.0),
    ] {
        theme.names.sizes.insert(name.to_owned(), value);
    }
    let mut tree = UiTree::new();
    tree.set_theme(theme);
    let menu = Menu::new(&mut tree, "Menu", &[("go", "Go")]);
    let style = tree.layout_style(menu.card).cloned().expect("in the tree");
    assert_eq!(
        (style.max_width, style.padding, style.gap),
        (Some(300.0), Edges::all(10.0), 4.0)
    );
    assert_eq!(
        padding(&tree, menu.buttons[0]),
        Some(Edges::symmetric(20.0, 7.0))
    );
    let choice = choice_row(&mut tree, menu.card, "Mode", "A");
    assert_eq!(
        tree.layout_style(choice).and_then(|style| style.min_width),
        Some(120.0)
    );
    let (slider, readout) = slider_row(&mut tree, menu.card, "Volume", SliderRange::new(0.0, 1.0));
    assert_eq!(
        tree.layout_style(slider).map(|style| style.width),
        Some(Sizing::Px(120.0))
    );
    assert_eq!(
        tree.overrides(slider).and_then(|own| own.text.line_height),
        Some(24.0)
    );
    assert_eq!(
        tree.layout_style(readout).and_then(|style| style.min_width),
        Some(50.0)
    );
    let (_, bars) = tab_rail(&mut tree, menu.card, &["One"]);
    assert_eq!(
        tree.layout_style(bars[0]).map(|style| style.height),
        Some(Sizing::Px(5.0))
    );
}
