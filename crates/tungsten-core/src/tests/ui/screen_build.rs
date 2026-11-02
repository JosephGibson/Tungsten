use glam::Vec2;

use super::*;
use crate::display::ScaleFactor;
use crate::input::{ActionMap, InputState, MouseButton, RoutedInput};
use crate::time::Time;
use crate::ui::{
    FixedAdvanceMeasure, LayoutStats, ScreenFile, Theme, UiEvent, WidgetKind, ui_route_system,
};

const VIEWPORT: Vec2 = Vec2::new(800.0, 600.0);

/// What the test bindings read.
#[derive(Debug, Clone, Default)]
struct Values {
    text: String,
    on: bool,
    number: f32,
    tab: f32,
    shown: bool,
    enabled: bool,
    fill: f32,
    rows: Vec<UiRow>,
}

/// The nodes the `cards` mount was called with.
#[derive(Debug, Default)]
struct Mounted(Vec<WidgetId>);

fn values(world: &World) -> &Values {
    world.resource::<Values>()
}

fn text(world: &World) -> String {
    values(world).text.clone()
}

fn readout(world: &World) -> String {
    format!("{:.0}%", values(world).number * 100.0)
}

fn on(world: &World) -> UiValue {
    UiValue::Bool(values(world).on)
}

fn number(world: &World) -> UiValue {
    UiValue::Number(values(world).number)
}

fn tab(world: &World) -> UiValue {
    UiValue::Number(values(world).tab)
}

fn shown(world: &World) -> bool {
    values(world).shown
}

fn enabled(world: &World) -> bool {
    values(world).enabled
}

fn fill(world: &World) -> f32 {
    values(world).fill
}

fn rows(world: &World) -> Vec<UiRow> {
    values(world).rows.clone()
}

fn cards(world: &mut World, node: WidgetId) {
    world.resource_mut::<Mounted>().0.push(node);
    world.resource_mut::<UiTree>().label(node, "mounted");
}

/// A world holding `files` as `ui_screens`, a tree, and the test bindings.
fn world(files: &[(&str, &str)]) -> World {
    let mut world = World::new();
    world.insert_resource(UiTree::new());
    let mut registry = UiScreenRegistry::new();
    for (id, json) in files {
        let file = ScreenFile::from_json(json).unwrap_or_else(|e| panic!("{id}: {e}"));
        registry.insert(*id, file);
    }
    world.insert_resource(registry);
    world.insert_resource(Values {
        shown: true,
        enabled: true,
        ..Values::default()
    });
    world.insert_resource(Mounted::default());
    UiBindings::of(&mut world)
        .text("text", text)
        .text("readout", readout)
        .value("on", on)
        .value("number", number)
        .value("tab", tab)
        .visible("shown", shown)
        .enabled("enabled", enabled)
        .fill("fill", fill)
        .rows("rows", rows)
        .mount("cards", cards);
    world
}

/// A file whose screen card, 300 units wide, holds `children`.
fn screen(children: &str) -> String {
    format!(
        r#"{{"version": 1, "root": {{"kind": "screen", "width": 300, "children": [{children}]}}}}"#
    )
}

/// `world` with `children`'s screen built as `menu`.
fn built(children: &str) -> (World, BuiltScreen) {
    let mut world = world(&[("menu", &screen(children))]);
    let built = build(&mut world, "menu").expect("built");
    (world, built)
}

fn layout(world: &mut World) -> LayoutStats {
    let tree = world.resource_mut::<UiTree>();
    let stats = tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    tree.clear_paint_dirty();
    stats
}

fn tree(world: &World) -> &UiTree {
    world.resource::<UiTree>()
}

fn keyed(world: &World, root: WidgetId, key: &str) -> WidgetId {
    node(world, root, key).unwrap_or_else(|| panic!("no node keyed {key}"))
}

fn warned(world: &World) -> &HashSet<String> {
    &world.resource::<UiScreens>().warned
}

#[test]
fn a_screen_builds_its_kinds_in_file_order() {
    let json = r#"{"version": 1, "root": {"kind": "screen", "width": 300, "children": [
          {"kind": "heading", "text": "Paused", "centred": true},
          {"kind": "label", "text": "one"},
          {"kind": "button", "key": "go", "text": "Go"},
          {"kind": "row", "children": [{"kind": "note", "text": "a"}, {"kind": "small", "text": "b"}]},
          {"kind": "toggle_row", "key": "t", "caption": "Toggle"}
        ], "over": [{"kind": "column", "key": "foot", "children": [{"kind": "small", "text": "v1"}]}]}}"#;
    let mut world = world(&[("menu", json)]);
    let built = build(&mut world, "menu").expect("built");
    let tree = tree(&world);
    assert!(tree.modal(built.root));
    assert_eq!(tree.layer(built.root), Some(UiLayer::GamePopup));
    assert_eq!(tree.parent(built.card), Some(built.root));
    let card = tree.children(built.card);
    assert_eq!(card.len(), 5);
    assert_eq!(tree.plain_text(card[0]), "Paused");
    assert_eq!(tree.text_style(card[0]), Some("heading"));
    assert_eq!(tree.plain_text(card[1]), "one");
    assert_eq!(tree.kind(card[2]), Some(WidgetKind::Button));
    assert_eq!(tree.plain_text(card[2]), "Go");
    let row = tree.children(card[3]);
    assert_eq!(
        (tree.plain_text(row[0]), tree.plain_text(row[1])),
        ("a".to_owned(), "b".to_owned())
    );
    assert_eq!(
        (tree.text_style(row[0]), tree.text_style(row[1])),
        (Some("note"), Some("small"))
    );
    let toggle = keyed(&world, built.root, "t");
    assert_eq!(tree.kind(toggle), Some(WidgetKind::Toggle));
    assert_eq!(
        tree.parent(toggle),
        Some(card[4]),
        "a row kind's node is its control, in its row"
    );
    assert_eq!(
        tree.children(built.root),
        [built.card, keyed(&world, built.root, "foot")],
        "over goes under the root, after the card"
    );

    let bare = r#"{"version": 1, "root": {"kind": "root", "layer": "game",
                   "children": [{"kind": "label", "text": "hud"}]}}"#;
    let mut world = self::world(&[("hud", bare)]);
    let built = build(&mut world, "hud").expect("built");
    let tree = self::tree(&world);
    assert_eq!(built.card, built.root, "a bare root holds its children");
    assert_eq!(tree.layer(built.root), Some(UiLayer::Game));
    assert!(!tree.modal(built.root));
    assert_eq!(tree.plain_text(tree.children(built.root)[0]), "hud");
}

#[test]
fn keys_styles_layouts_and_overrides_apply() {
    let json = r##"{"version": 1, "root": {"kind": "screen", "width": "menu_width",
          "card": {"layout": {"max_width": "none", "align_items": "center", "gap": 7},
                   "overrides": {"background": "#00000000"}},
          "children": [
            {"kind": "button", "key": "save", "text": "Save", "style": "note", "mono": true, "danger": true,
             "layout": {"width": "50%", "min_width": "control_width", "padding": {"x": 3, "top": "spacing.lg"},
                        "align_self": "end"},
             "overrides": {"background": "#ff0000", "border_width": 2, "text": {"letter_spacing": 0.25}}},
            {"kind": "column", "key": "foot",
             "layout": {"anchor": {"corner": "bottom_start", "offset": ["spacing.lg", 4]}, "direction": "row",
                        "justify_content": "space_between", "margin": 5}}
          ]}}"##;
    let mut world = world(&[("menu", json)]);
    let built = build(&mut world, "menu").expect("built");
    let tree = tree(&world);
    let theme = tree.theme();
    let spacing = theme.tokens.spacing;
    let card = tree.layout_style(built.card).unwrap();
    assert_eq!(
        (card.max_width, card.align_items, card.gap),
        (None, Align::Center, 7.0)
    );
    assert_eq!(tree.overrides(built.card).unwrap().background, Some([0; 4]));
    let save = keyed(&world, built.root, "save");
    assert_eq!(tree.key_name(save), Some("save"));
    assert_eq!(tree.text_style(save), Some("note"));
    let own = tree.overrides(save).unwrap();
    assert_eq!(own.background, Some([255, 0, 0, 255]));
    assert_eq!(own.border_width, Some(2.0));
    assert_eq!(own.text.letter_spacing, Some(0.25));
    assert_eq!(
        own.text.family.as_deref(),
        Some(theme.tokens.fonts.mono.as_str())
    );
    assert_eq!(own.text.color, Some(theme.tokens.colors.danger));
    let style = tree.layout_style(save).unwrap();
    assert_eq!(style.width, Sizing::Percent(50.0));
    assert_eq!(style.min_width, Some(170.0), "a size name's default");
    assert_eq!(
        style.padding,
        Edges {
            top: spacing.lg,
            end: 3.0,
            bottom: 5.0,
            start: 3.0
        },
        "the sides named over the button's own"
    );
    assert_eq!(style.align_self, Some(Align::End));
    let foot = tree
        .layout_style(keyed(&world, built.root, "foot"))
        .unwrap();
    assert_eq!(
        foot.placement,
        Placement::Anchored {
            anchor: Anchor::BottomStart,
            offset: Vec2::new(spacing.lg, 4.0)
        }
    );
    assert_eq!(foot.direction, Direction::Row);
    assert_eq!(foot.justify_content, Justify::SpaceBetween);
    assert_eq!(foot.margin, Edges::all(5.0));
}

#[test]
fn each_binding_writes_its_node() {
    let mut world = world(&[(
        "menu",
        &screen(
            r#"{"kind": "label", "key": "label", "bind": {"text": "text", "visible": "shown", "enabled": "enabled"}},
               {"kind": "button", "key": "button", "bind": {"text": "text"}},
               {"kind": "choice_row", "key": "choice", "caption": "Mode", "bind": {"text": "text"}},
               {"kind": "toggle_row", "key": "toggle", "caption": "On", "bind": {"value": "on"}},
               {"kind": "slider_row", "key": "slider", "caption": "Level",
                "range": {"min": 0, "max": 1, "step": 0.25}, "readout": "readout", "bind": {"value": "number"}},
               {"kind": "meter", "key": "meter", "width": 100, "height": 8, "color": "danger", "bind": {"fill": "fill"}}"#,
        ),
    )]);
    *world.resource_mut::<Values>() = Values {
        text: "hello".into(),
        on: true,
        number: 0.75,
        fill: 0.5,
        shown: false,
        enabled: false,
        ..Values::default()
    };
    let built = build(&mut world, "menu").expect("built");
    let tree = tree(&world);
    let at = |key| keyed(&world, built.root, key);
    assert_eq!(tree.plain_text(at("label")), "hello");
    assert_eq!(tree.visibility(at("label")), Some(Visibility::Collapsed));
    assert!(!tree.enabled(at("label")));
    assert_eq!(tree.plain_text(at("button")), "hello");
    assert_eq!(tree.plain_text(at("choice")), "hello");
    assert_eq!(tree.toggled(at("toggle")), Some(true));
    assert_eq!(tree.slider_value(at("slider")), Some(0.75));
    let row = tree.parent(at("slider")).unwrap();
    assert_eq!(tree.plain_text(tree.children(row)[2]), "75%", "the readout");
    let fill = tree.children(at("meter"))[0];
    assert_eq!(
        tree.layout_style(fill).unwrap().width,
        Sizing::Percent(50.0)
    );
    assert_eq!(
        tree.overrides(fill).unwrap().background,
        Some(tree.theme().tokens.colors.danger)
    );
}

#[test]
fn an_equal_refresh_marks_nothing() {
    let (mut world, built) = built(
        r#"{"kind": "label", "bind": {"text": "text", "visible": "shown", "enabled": "enabled"}},
           {"kind": "toggle_row", "caption": "On", "bind": {"value": "on"}},
           {"kind": "slider_row", "caption": "Level", "range": {"min": 0, "max": 1, "step": 0.25},
            "readout": "readout", "bind": {"value": "number"}},
           {"kind": "meter", "width": 100, "height": 8, "bind": {"fill": "fill"}},
           {"kind": "tab_rail", "tabs": [{"key": "a", "caption": "A"}, {"key": "b", "caption": "B"}],
            "bind": {"value": "tab"}},
           {"kind": "repeat", "key": "slots", "rows": "rows", "row": {"kind": "label", "field": "name"}}"#,
    );
    world.resource_mut::<Values>().rows = vec![UiRow::new("a").with("name", "Alpha")];
    refresh(&mut world, built.root);
    layout(&mut world);
    for _ in 0..3 {
        refresh(&mut world, built.root);
        assert!(!tree(&world).is_paint_dirty());
        assert_eq!(layout(&mut world).roots_laid_out, 0);
    }
}

#[test]
fn a_changed_binding_marks_only_its_node() {
    let (mut world, built) = built(
        r#"{"kind": "label", "key": "bound", "bind": {"text": "text"}},
           {"kind": "label", "key": "plain", "text": "still"},
           {"kind": "toggle_row", "key": "toggle", "caption": "On", "bind": {"value": "on"}}"#,
    );
    layout(&mut world);
    world.resource_mut::<Values>().text = "changed".into();
    refresh(&mut world, built.root);
    let tree = tree(&world);
    let at = |key| keyed(&world, built.root, key);
    assert!(tree.widget_dirty(at("bound")).unwrap().text);
    for other in ["plain", "toggle"] {
        let dirty = tree.widget_dirty(at(other)).unwrap();
        assert!(!dirty.text && !dirty.style, "{other}: {dirty:?}");
    }
    assert_eq!(tree.plain_text(at("bound")), "changed");
    assert_eq!(layout(&mut world).roots_laid_out, 1);
}

#[test]
fn a_repeat_rebuilds_only_when_its_rows_change() {
    let mut world = world(&[(
        "menu",
        &screen(
            r#"{"kind": "repeat", "key": "slots", "rows": "rows", "row": {"kind": "button", "field": "name"}}"#,
        ),
    )]);
    world.resource_mut::<Values>().rows = vec![
        UiRow::new("a").with("name", "Alpha"),
        UiRow::new("b").with("name", "Beta"),
    ];
    let built = build(&mut world, "menu").expect("built");
    let container = keyed(&world, built.root, "slots");
    let first = tree(&world).children(container).to_vec();
    let texts = |world: &World| -> Vec<String> {
        let tree = self::tree(world);
        tree.children(container)
            .iter()
            .map(|row| tree.plain_text(*row))
            .collect()
    };
    assert_eq!(texts(&world), ["Alpha", "Beta"]);
    layout(&mut world);
    refresh(&mut world, built.root);
    assert_eq!(
        tree(&world).children(container),
        first,
        "equal rows keep their nodes"
    );
    assert!(!tree(&world).is_paint_dirty());

    world.resource_mut::<Values>().rows[1] = UiRow::new("b").with("name", "Bravo");
    refresh(&mut world, built.root);
    let second = tree(&world).children(container).to_vec();
    assert_eq!(texts(&world), ["Alpha", "Bravo"]);
    assert!(
        first.iter().all(|row| !tree(&world).contains(*row)),
        "changed rows rebuild"
    );
    assert_eq!(second.len(), 2);
}

#[test]
fn a_mount_is_called_with_its_node() {
    let (mut world, built) = built(
        r#"{"kind": "mount", "key": "cards", "mount": "cards", "layout": {"direction": "row"}}"#,
    );
    let node = keyed(&world, built.root, "cards");
    assert_eq!(world.resource::<Mounted>().0, [node]);
    let tree = tree(&world);
    assert_eq!(tree.plain_text(tree.children(node)[0]), "mounted");
    assert_eq!(tree.layout_style(node).unwrap().direction, Direction::Row);
    refresh(&mut world, built.root);
    assert_eq!(
        world.resource::<Mounted>().0.len(),
        1,
        "a refresh calls no mount"
    );
}

#[test]
fn an_unknown_name_warns_and_is_skipped() {
    let (mut world, built) = built(
        r#"{"kind": "label", "key": "label", "bind": {"text": "nope", "visible": "shown"}},
           {"kind": "mount", "mount": "missing"},
           {"kind": "repeat", "key": "slots", "rows": "absent", "row": {"kind": "label", "field": "name"}}"#,
    );
    let tree = tree(&world);
    let label = keyed(&world, built.root, "label");
    assert_eq!(tree.plain_text(label), "", "built, its binding skipped");
    assert_eq!(tree.visibility(label), Some(Visibility::Visible));
    for once in [
        "text binding menu nope",
        "mount menu missing",
        "row provider menu absent",
    ] {
        assert!(
            warned(&world).contains(once),
            "{once}: {:?}",
            warned(&world)
        );
    }
    assert_eq!(world.resource::<Mounted>().0, []);
    assert_eq!(build(&mut world, "unknown"), None);
    assert!(warned(&world).contains("screen unknown"));
}

#[test]
fn a_token_colour_resolves_against_the_trees_theme() {
    let mut world = world(&[(
        "menu",
        &screen(
            r##"{"kind": "panel", "key": "panel",
                 "overrides": {"background": "accent", "border": "#102030", "text": {"color": "danger"}}},
                {"kind": "meter", "key": "meter", "width": 10, "height": 2, "color": "accent", "bind": {"fill": "fill"}}"##,
        ),
    )]);
    let mut theme = Theme::default();
    theme.tokens.colors.accent = [1, 2, 3, 255];
    theme.tokens.colors.danger = [4, 5, 6, 255];
    world.resource_mut::<UiTree>().set_theme(theme);
    let built = build(&mut world, "menu").expect("built");
    let tree = tree(&world);
    let own = tree.overrides(keyed(&world, built.root, "panel")).unwrap();
    assert_eq!(own.background, Some([1, 2, 3, 255]));
    assert_eq!(own.border, Some([0x10, 0x20, 0x30, 255]));
    assert_eq!(own.text.color, Some([4, 5, 6, 255]));
    let fill = tree.children(keyed(&world, built.root, "meter"))[0];
    assert_eq!(
        tree.overrides(fill).unwrap().background,
        Some([1, 2, 3, 255])
    );
}

/// One routed frame: raw input, the clock, the routing, the layout.
fn frame(world: &mut World, feed: impl FnOnce(&mut InputState)) {
    world.flush_events();
    let input = world.resource_mut::<InputState>();
    input.begin_frame();
    feed(input);
    world.resource_mut::<Time>().advance_frame(1.0 / 32.0);
    ui_route_system(world);
    layout(world);
}

#[test]
fn a_refresh_leaves_the_captured_control_alone() {
    let (mut world, built) = built(
        r#"{"kind": "slider_row", "key": "slider", "caption": "Level",
            "range": {"min": 0, "max": 1, "step": 0.25}, "bind": {"value": "number"}}"#,
    );
    world.insert_resource(InputState::new());
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(Time::new());
    world.insert_resource(ScaleFactor::default());
    world.insert_resource(UiRouter::default());
    world.insert_resource(RoutedInput::default());
    world.register_event::<UiEvent>();
    layout(&mut world);
    let slider = keyed(&world, built.root, "slider");
    let rect = tree(&world).rect(slider).expect("laid out");
    let left = MouseButton::Left;
    let y = f32::midpoint(rect.min.y, rect.max.y);
    frame(&mut world, |input| {
        input.update_cursor_position(rect.max.x - 1.0, y);
        input.mouse_down(left);
    });
    assert_eq!(tree(&world).slider_value(slider), Some(1.0), "the press");
    // The drag leaves the slider: its pressed look drops, the capture holds.
    frame(&mut world, |input| {
        input.update_cursor_position(rect.max.x + 200.0, y + 200.0);
    });
    assert_eq!(world.resource::<UiRouter>().captured(), Some(slider));
    assert_ne!(tree(&world).pressed(), Some(slider));
    for _ in 0..2 {
        refresh(&mut world, built.root);
        assert_eq!(
            tree(&world).slider_value(slider),
            Some(1.0),
            "the binding's 0 is not written over the drag"
        );
        frame(&mut world, |_| {});
    }
    frame(&mut world, |input| input.mouse_up(left));
    assert_eq!(world.resource::<UiRouter>().captured(), None);
    refresh(&mut world, built.root);
    assert_eq!(
        tree(&world).slider_value(slider),
        Some(0.0),
        "released, the binding writes again"
    );
}

#[test]
fn repeat_rows_and_their_controls_take_qualified_keys() {
    let mut world = world(&[(
        "menu",
        &screen(
            r#"{"kind": "repeat", "key": "slots", "rows": "rows",
                "row": {"kind": "button", "key": "ignored", "field": "name", "on": {"action": "open"},
                        "children": [{"kind": "button", "key": "delete", "text": "x", "on": {"action": "delete"}}]}}"#,
        ),
    )]);
    world.resource_mut::<Values>().rows = vec![
        UiRow::new("a").with("name", "Alpha"),
        UiRow::new("b").with("name", "Beta"),
    ];
    let built = build(&mut world, "menu").expect("built");
    for row in ["a", "b"] {
        let node = keyed(&world, built.root, &format!("slots/{row}"));
        let delete = keyed(&world, built.root, &format!("slots/{row}/delete"));
        assert_eq!(tree(&world).parent(delete), Some(node));
        assert_eq!(
            action(&world, built.root, node),
            Some(KeyedAction {
                action: ScreenAction::Named("open".into()),
                key: Some("slots".into()),
                row: Some(row.into()),
                on_commit: false,
                value_bound: false,
            }),
            "a row's node acts under the repeat's key"
        );
        assert_eq!(
            action(&world, built.root, delete),
            Some(KeyedAction {
                action: ScreenAction::Named("delete".into()),
                key: Some("delete".into()),
                row: Some(row.into()),
                on_commit: false,
                value_bound: false,
            })
        );
    }
    assert_eq!(node(&world, built.root, "delete"), None);
    assert_eq!(node(&world, built.root, "ignored"), None);
    let listed = actions(&world, built.root);
    assert_eq!(listed.len(), 4, "two rows, two controls each");
    assert!(listed.iter().all(|(_, action)| action.row.is_some()));
}

#[test]
fn a_repeated_or_slashed_row_key_warns_and_builds_unkeyed() {
    let mut world = world(&[(
        "menu",
        &screen(
            r#"{"kind": "repeat", "key": "slots", "rows": "rows",
                "row": {"kind": "button", "field": "name", "on": {"action": "open"}}}"#,
        ),
    )]);
    world.resource_mut::<Values>().rows = vec![
        UiRow::new("a").with("name", "one"),
        UiRow::new("a").with("name", "two"),
        UiRow::new("b/c").with("name", "three"),
    ];
    let built = build(&mut world, "menu").expect("built");
    let container = keyed(&world, built.root, "slots");
    let rows = tree(&world).children(container).to_vec();
    assert_eq!(rows.len(), 3, "every row builds");
    assert_eq!(tree(&world).key_name(rows[0]), Some("slots/a"));
    for unkeyed in &rows[1..] {
        assert_eq!(tree(&world).key(*unkeyed), None);
        assert_eq!(action(&world, built.root, *unkeyed), None, "and actionless");
    }
    assert!(action(&world, built.root, rows[0]).is_some());
    for once in ["row menu slots a", "row menu slots b/c"] {
        assert!(warned(&world).contains(once), "{once}");
    }
}

#[test]
fn a_tab_rails_value_selects_its_bar() {
    let (mut world, built) = built(
        r#"{"kind": "tab_rail", "key": "rail", "bind": {"value": "tab"},
            "tabs": [{"key": "display", "caption": "Display"}, {"key": "audio", "caption": "Audio"},
                     {"key": "video", "caption": "Video"}]}"#,
    );
    let tabs = ["display", "audio", "video"].map(|key| keyed(&world, built.root, key));
    let rail = keyed(&world, built.root, "rail");
    assert!(
        tabs.iter()
            .all(|tab| tree(&world).parent(*tab) == Some(rail))
    );
    let bars = |world: &World| {
        let tree = self::tree(world);
        tabs.map(|tab| tree.visibility(*tree.children(tab).last().unwrap()))
    };
    let (shown, hidden) = (Some(Visibility::Visible), Some(Visibility::Hidden));
    assert_eq!(bars(&world), [shown, hidden, hidden], "tab 0 at first");
    world.resource_mut::<Values>().tab = 2.0;
    refresh(&mut world, built.root);
    assert_eq!(bars(&world), [hidden, hidden, shown]);
    assert_eq!(tree(&world).plain_text(tabs[1]), "Audio");
}

/// Replaces `id`'s file in the world's registry, as a reload's load does.
fn replace(world: &mut World, id: &str, json: &str) {
    let file = ScreenFile::from_json(json).unwrap_or_else(|e| panic!("{json}: {e}"));
    world.resource_mut::<UiScreenRegistry>().insert(id, file);
}

#[test]
fn a_reload_keeps_the_root_and_focus_by_key() {
    let (mut world, built) = built(
        r#"{"kind": "button", "key": "a", "text": "A"}, {"kind": "button", "key": "b", "text": "B"}"#,
    );
    let b = keyed(&world, built.root, "b");
    assert!(world.resource_mut::<UiTree>().focus(b));
    replace(
        &mut world,
        "menu",
        &screen(
            r#"{"kind": "label", "text": "new"}, {"kind": "button", "key": "b", "text": "Bee"},
               {"kind": "button", "key": "a", "text": "A"}"#,
        ),
    );
    reload(&mut world, "menu");
    let view = tree(&world);
    assert!(
        view.contains(built.root) && view.contains(built.card),
        "kept"
    );
    assert!(!view.contains(b), "its nodes rebuilt");
    assert_eq!(view.children(built.card).len(), 3);
    let new_b = keyed(&world, built.root, "b");
    assert_eq!(view.focused(), Some(new_b), "focus on the old key");
    assert_eq!(view.plain_text(new_b), "Bee");

    // A key the new file drops: the root's first that can take focus.
    replace(
        &mut world,
        "menu",
        &screen(r#"{"kind": "label", "text": "x"}, {"kind": "button", "key": "c", "text": "C"}"#),
    );
    reload(&mut world, "menu");
    let c = keyed(&world, built.root, "c");
    assert_eq!(tree(&world).focused(), Some(c));

    // Focus under another root stays there.
    let other = world
        .resource_mut::<UiTree>()
        .add_root(UiLayer::Game, LayoutStyle::default());
    let elsewhere = world.resource_mut::<UiTree>().button(other, "elsewhere");
    assert!(world.resource_mut::<UiTree>().focus(elsewhere));
    reload(&mut world, "menu");
    assert_eq!(tree(&world).focused(), Some(elsewhere));
}

#[test]
fn a_reload_calls_mounts_again_and_raises_the_revision() {
    let (mut world, built) = built(
        r#"{"kind": "mount", "key": "cards", "mount": "cards"},
           {"kind": "label", "key": "label", "bind": {"text": "text"}}"#,
    );
    assert_eq!(revision(&world, built.root), Some(0));
    let first = world.resource::<Mounted>().0.clone();
    world.resource_mut::<Values>().text = "bound".into();
    replace(
        &mut world,
        "menu",
        &screen(
            r#"{"kind": "label", "text": "before"},
               {"kind": "mount", "key": "cards", "mount": "cards"},
               {"kind": "label", "key": "label", "bind": {"text": "text"}}"#,
        ),
    );
    reload(&mut world, "menu");
    assert_eq!(revision(&world, built.root), Some(1));
    let mounted = &world.resource::<Mounted>().0;
    assert_eq!(mounted.len(), 2, "called again");
    assert_eq!(mounted[1], keyed(&world, built.root, "cards"));
    assert_ne!(mounted[1], first[0]);
    assert_eq!(
        tree(&world).plain_text(keyed(&world, built.root, "label")),
        "bound",
        "the bindings refreshed"
    );
    reload(&mut world, "menu");
    assert_eq!(revision(&world, built.root), Some(2));
    assert_eq!(revision(&world, WidgetId::new(999, 0)), None);
}

#[test]
fn a_changed_layer_waits_for_the_next_build() {
    let popup = r#"{"version": 1, "root": {"kind": "root", "layer": "game_popup", "modal": true,
                    "children": [{"kind": "label", "key": "label", "text": "x"}]}}"#;
    let mut world = world(&[("menu", popup)]);
    let built = build(&mut world, "menu").expect("built");
    let label = keyed(&world, built.root, "label");
    replace(
        &mut world,
        "menu",
        r#"{"version": 1, "root": {"kind": "root", "layer": "game", "modal": true,
            "children": [{"kind": "label", "key": "label", "text": "y"}]}}"#,
    );
    reload(&mut world, "menu");
    assert_eq!(revision(&world, built.root), Some(0), "not rebuilt");
    assert!(tree(&world).contains(label));
    assert!(warned(&world).contains("shape menu"));
    let again = build(&mut world, "menu").expect("built");
    assert_eq!(tree(&world).layer(again.root), Some(UiLayer::Game));
    assert_eq!(
        tree(&world).plain_text(keyed(&world, again.root, "label")),
        "y"
    );
}
