use tungsten_core::ui::{
    ScreenFile, Tooltip, UiAction, UiBindings, UiKey, UiLayer, UiScreenRegistry, UiTree, WidgetId,
    widgets,
};
use tungsten_core::{Config, KeyCode, World};

use super::*;
use crate::App;
use crate::screens::{ModalScreen, ScreensPlugin, pop};
use crate::state::{GameState, StateContext, StateId, StateStack};
use crate::testing::UiHarness;

/// What the test screens' bindings read and their actions write.
#[derive(Debug, Default)]
struct Board {
    text: String,
    heard: Vec<String>,
}

/// The tooltip's widget by key and the board's text: a tooltip screen's
/// line.
fn anchor_line(world: &World) -> String {
    let tree = world.resource::<UiTree>();
    let key = tooltip_anchor(world).and_then(|(widget, _)| tree.key_name(widget));
    format!("{}: {}", key.unwrap_or("?"), world.resource::<Board>().text)
}

/// Records the action's key at the front popup's control, by its key.
fn drop_item(world: &mut World, action: UiAction) {
    let tree = world.resource::<UiTree>();
    let anchor = popup_anchor(world)
        .and_then(|(widget, _)| tree.key_name(widget))
        .unwrap_or("?")
        .to_owned();
    let key = action.key.unwrap_or_default();
    world
        .resource_mut::<Board>()
        .heard
        .push(format!("{key}@{anchor}"));
}

/// A screen file whose card, 260 units wide, holds `children`.
fn file(children: &str) -> String {
    format!(
        r#"{{"version": 1, "root": {{"kind": "screen", "width": 260, "children": [{children}]}}}}"#
    )
}

/// A popup file: a modal bare root holding `children`.
fn popup_file(children: &str) -> String {
    format!(
        r#"{{"version": 1, "root": {{"kind": "root", "layer": "game_popup", "modal": true, "children": [{children}]}}}}"#
    )
}

/// The menu every test opens: a button without a tooltip, which takes
/// focus, one with a text tooltip,
/// a drag source with a tooltip and a drop target, a label whose tooltip
/// is a screen file, a button opening the cell menu, and a corner button
/// with a long tooltip and a popup.
const MENU: &str = r#"
    {"kind": "button", "key": "other", "text": "Other"},
    {"kind": "button", "key": "use", "text": "Use", "tooltip": "Drink it to heal"},
    {"kind": "button", "key": "cell", "text": "Cell", "drag": true, "tooltip": "A cell", "on": {"popup": "cell_menu"}},
    {"kind": "panel", "key": "bin", "drop": true, "layout": {"height": 30}},
    {"kind": "label", "key": "gem", "text": "Gem", "tooltip": {"screen": "item_tip"}},
    {"kind": "label", "key": "bad", "text": "Bad", "tooltip": {"screen": "cell_menu"}}"#;

/// A harness with the screens, `MENU` as `menu` (its card's `over`
/// holding `corner`, a button at the viewport's bottom end), the `cell_menu`
/// popup, the `item_tip` tooltip screen and the board's bindings.
fn harness() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_plugin(ScreensPlugin);
    let world = app.world_mut();
    let menu = file(MENU).replace(
        r#""children": ["#,
        r#""over": [{"kind": "button", "key": "corner", "text": "Corner",
            "tooltip": "A long tooltip whose words wrap across the panel at its widest and more",
            "on": {"popup": "cell_menu"},
            "layout": {"anchor": {"corner": "bottom_end", "offset": [0, 0]}}}],
          "children": ["#,
    );
    let files = [
        ("menu", menu),
        (
            "cell_menu",
            popup_file(
                r#"{"kind": "button", "key": "drop", "text": "Drop", "on": {"action": "drop_item"}},
                   {"kind": "button", "key": "close", "text": "Close", "on": "pop"}"#,
            ),
        ),
        (
            "item_tip",
            r#"{"version": 1, "root": {"kind": "root", "layer": "game_popup", "children": [
                {"kind": "label", "key": "line", "bind": {"text": "anchor_line"}}]}}"#
                .to_owned(),
        ),
        (
            "other_menu",
            file(r#"{"kind": "button", "key": "back", "text": "Back", "on": "pop"}"#),
        ),
    ];
    let mut registry = UiScreenRegistry::new();
    for (id, json) in files {
        let file = ScreenFile::from_json(&json).unwrap_or_else(|e| panic!("{id}: {e}"));
        registry.insert(id, file);
    }
    world.insert_resource(registry);
    world.insert_resource(Board::default());
    UiBindings::of(world)
        .text("anchor_line", anchor_line)
        .action("drop_item", drop_item);
    UiHarness::new(app)
}

/// `harness` with the menu opened and its slide in finished.
fn menu() -> UiHarness {
    let mut ui = harness();
    assert!(crate::screens::open(ui.world_mut(), "menu"));
    ui.step(12);
    ui
}

fn keyed(ui: &UiHarness, key: &str) -> WidgetId {
    ui.find_key(UiKey::new(key))
        .unwrap_or_else(|| panic!("no widget keyed {key}"))
}

/// The default delay, in real seconds.
fn delay() -> f32 {
    TooltipSettings::default().delay
}

/// Hovers `widget` past the delay; the tooltip's root, placed.
fn shown_on(ui: &mut UiHarness, widget: WidgetId) -> WidgetId {
    ui.hover(widget);
    ui.step(1);
    ui.wait(delay());
    ui.step(2);
    let root = shown_tooltip(ui.world()).expect("a tooltip shows");
    assert_eq!(tooltip_anchor(ui.world()).map(|(at, _)| at), Some(widget));
    root
}

/// The text of the first label under `root`, depth first.
fn text_under(ui: &UiHarness, root: WidgetId) -> Option<String> {
    let tree = ui.tree();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if tree.text(id).is_some() {
            return Some(tree.plain_text(id));
        }
        stack.extend(tree.children(id).iter().rev());
    }
    None
}

fn viewport(ui: &UiHarness) -> glam::Vec2 {
    ui.tree()
        .layer_viewport(UiLayer::GamePopup)
        .expect("laid out")
}

fn top(ui: &UiHarness) -> Option<StateId> {
    ui.world().resource::<StateStack>().active_id()
}

fn tap(ui: &mut UiHarness, key: KeyCode) {
    ui.key_down(key);
    ui.step(1);
    ui.key_up(key);
    ui.step(1);
}

#[test]
fn a_tooltip_shows_after_its_delay_beside_its_widget() {
    let mut ui = menu();
    let used = keyed(&ui, "use");
    ui.hover(used);
    ui.step(1);
    ui.wait(delay() - 0.05);
    assert_eq!(shown_tooltip(ui.world()), None, "not before its delay");
    ui.wait(0.05);
    ui.step(2);
    let root = shown_tooltip(ui.world()).expect("shown after its delay");
    assert_eq!(
        tooltip_anchor(ui.world()),
        Some((used, Some(UiKey::new("use"))))
    );
    assert_eq!(text_under(&ui, root).as_deref(), Some("Drink it to heal"));
    let (tip, at) = (ui.rect(root).unwrap(), ui.rect(used).unwrap());
    assert_eq!(tip.min.x, at.min.x, "under its widget's start");
    assert_eq!(tip.min.y, at.max.y + 4.0, "below it, past the gap");
    assert!(tip.width() <= TOOLTIP_WIDTH);
    let tree = ui.tree();
    assert_eq!(tree.layer(root), Some(UiLayer::GamePopup));
    assert!(!tree.modal(root));
    assert_eq!(tree.hovered(), Some(used), "it takes no hits");
    let centre = (tip.min + tip.max) * 0.5;
    assert_ne!(
        tree.hit_test(centre).and_then(|hit| tree.root_of(hit)),
        Some(root)
    );
}

#[test]
fn leaving_pressing_or_dragging_hides_it() {
    let mut ui = menu();
    let [used, other, cell, bin] = ["use", "other", "cell", "bin"].map(|key| keyed(&ui, key));

    // Leaving: onto a widget with none.
    let root = shown_on(&mut ui, used);
    ui.hover(other);
    ui.step(1);
    assert_eq!(shown_tooltip(ui.world()), None);
    assert!(!ui.tree().contains(root), "its root removed");

    // A press hides it, and it stays hidden over the widget pressed.
    shown_on(&mut ui, used);
    ui.pointer_down();
    ui.step(1);
    assert_eq!(shown_tooltip(ui.world()), None, "a press");
    ui.pointer_up();
    ui.step(1);
    ui.wait(delay() * 2.0);
    assert_eq!(shown_tooltip(ui.world()), None, "quiet until it leaves");
    ui.hover(other);
    ui.step(1);
    shown_on(&mut ui, used);

    // A drag hides it until its drop.
    shown_on(&mut ui, cell);
    ui.drag(cell, bin);
    assert_eq!(shown_tooltip(ui.world()), None, "a drag");
    ui.step(1);
    ui.wait(delay() * 2.0);
    assert_eq!(
        shown_tooltip(ui.world()),
        None,
        "the drop's target has none"
    );
}

#[test]
fn a_screen_tooltip_reads_its_anchor() {
    let mut ui = menu();
    ui.world_mut().resource_mut::<Board>().text = "rare".to_owned();
    let gem = keyed(&ui, "gem");
    let root = shown_on(&mut ui, gem);
    assert_eq!(text_under(&ui, root).as_deref(), Some("gem: rare"));
    let tree = ui.tree();
    assert!(!tree.modal(root) && !tree.hit_testable(root));
    ui.world_mut().resource_mut::<Board>().text = "worn".to_owned();
    ui.step(1);
    assert_eq!(
        text_under(&ui, root).as_deref(),
        Some("gem: worn"),
        "it refreshes each frame"
    );

    // A screen file whose root is a popup's shows nothing, warned once.
    let bad = keyed(&ui, "bad");
    ui.hover(bad);
    ui.step(1);
    ui.wait(delay() * 2.0);
    assert_eq!(shown_tooltip(ui.world()), None);
    let tooltips = ui.world().resource::<Tooltips>();
    assert_eq!(tooltips.warned, ["cell_menu"]);
}

/// A Rust-built modal screen: two buttons, the second given a tooltip.
struct Shelf {
    screen: Option<ModalScreen>,
}

impl GameState for Shelf {
    fn id(&self) -> StateId {
        "shelf"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let screen = ModalScreen::open(ctx.world, 260.0);
        let tree = ctx.world.resource_mut::<UiTree>();
        let first = widgets::button(tree, screen.card, "First");
        tree.set_key(first, "first");
        let lamp = widgets::button(tree, screen.card, "Lamp");
        tree.set_key(lamp, "lamp");
        set_tooltip(ctx.world, lamp, Tooltip::Text("Lights the hall".to_owned()));
        screen.resume(ctx.world);
        self.screen = Some(screen);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        if let Some(mut screen) = self.screen {
            screen.update(world);
            self.screen = Some(screen);
        }
    }
}

#[test]
fn a_focused_widget_shows_its_tooltip() {
    let mut ui = harness();
    assert!(crate::screens::push(ui.world_mut(), Shelf { screen: None }));
    ui.step(12);
    let lamp = keyed(&ui, "lamp");
    assert_ne!(ui.tree().focused(), Some(lamp));
    // The pointer rests over the scrim, on no widget with a tooltip.
    ui.pointer_to(2.0, 2.0);
    ui.step(1);
    tap(&mut ui, KeyCode::ArrowDown);
    assert_eq!(ui.tree().focused(), Some(lamp), "focus moved by key");
    assert!(ui.tree().focus_visible());
    ui.wait(delay());
    ui.step(2);
    let root = shown_tooltip(ui.world()).expect("the focused widget's");
    assert_eq!(text_under(&ui, root).as_deref(), Some("Lights the hall"));
    assert_eq!(tooltip_anchor(ui.world()).map(|(at, _)| at), Some(lamp));
    tap(&mut ui, KeyCode::ArrowUp);
    assert_eq!(shown_tooltip(ui.world()), None, "focus left it");
}

#[test]
fn a_tooltip_or_popup_stays_inside_the_viewport() {
    let mut ui = menu();
    let corner = keyed(&ui, "corner");
    let view = viewport(&ui);
    let at = ui.rect(corner).unwrap();
    assert_eq!(at.max, view, "the corner button sits at the bottom end");
    let root = shown_on(&mut ui, corner);
    let tip = ui.rect(root).unwrap();
    assert_eq!(tip.max.y, at.min.y - 4.0, "above it: no room below");
    assert_eq!(tip.max.x, view.x, "shifted in at the end");
    assert!(tip.min.x >= 0.0 && tip.width() <= TOOLTIP_WIDTH);

    // A popup beside the corner, placed the same way.
    ui.click(corner);
    ui.step(4);
    assert_eq!(top(&ui), Some("cell_menu"));
    let popup = ui.front_modal().expect("the popup");
    let rect = ui.rect(popup).unwrap();
    assert_eq!(rect.max.y, at.min.y - 4.0, "above its control");
    assert!(rect.min.x >= 0.0 && rect.max.x <= view.x && rect.min.y >= 0.0);
}

#[test]
fn a_popup_opens_beside_its_control_and_closes_on_a_press_outside_or_cancel() {
    let mut ui = menu();
    let cell = keyed(&ui, "cell");
    ui.click(cell);
    ui.step(2);
    assert_eq!(top(&ui), Some("cell_menu"), "pushed by the click");
    let tree = ui.tree();
    let popup = tree
        .roots()
        .iter()
        .rev()
        .copied()
        .find(|root| tree.modal(*root))
        .expect("the popup's root");
    assert_eq!(
        ui.tree().visibility(popup),
        Some(tungsten_core::ui::Visibility::Hidden),
        "hidden until placed"
    );
    ui.step(12);
    assert_eq!(
        popup_anchor(ui.world()),
        Some((cell, Some(UiKey::new("cell"))))
    );
    let (rect, at) = (ui.rect(popup).unwrap(), ui.rect(cell).unwrap());
    assert_eq!(
        rect.min,
        glam::Vec2::new(at.min.x, at.max.y + 4.0),
        "below it"
    );

    // Its actions run as a data screen's, reading the anchor.
    let drop = keyed(&ui, "drop");
    ui.click(drop);
    ui.step(1);
    assert_eq!(ui.world().resource::<Board>().heard, ["drop@cell"]);
    assert_eq!(top(&ui), Some("cell_menu"), "a press inside keeps it");

    // A press outside pops it, reaching nothing below.
    ui.pointer_to(1.0, 1.0);
    ui.pointer_down();
    ui.step(1);
    ui.pointer_up();
    ui.step(12);
    assert_eq!(top(&ui), Some("menu"));
    assert_eq!(popup_anchor(ui.world()), None);
    assert!(!ui.tree().contains(popup));

    // Cancel pops it too.
    ui.click(cell);
    ui.step(12);
    assert_eq!(top(&ui), Some("cell_menu"));
    tap(&mut ui, KeyCode::Escape);
    ui.step(12);
    assert_eq!(top(&ui), Some("menu"));
    assert_eq!(popup_anchor(ui.world()), None);

    // A file that is not a popup's does not open.
    assert!(!open(ui.world_mut(), "menu", cell), "a screen's root");
    assert!(!open(ui.world_mut(), "item_tip", cell), "a tooltip's root");
    assert!(!open(ui.world_mut(), "nope", cell), "no file");
}

#[test]
fn a_data_screen_and_an_open_dialogue_box_show_tooltips() {
    let mut ui = menu();
    let used = keyed(&ui, "use");
    shown_on(&mut ui, used);

    #[cfg(feature = "kit")]
    {
        use crate::kit::{KitPlugins, talk};
        use crate::story::StoryUiPlugin;
        let fixture =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/story");
        let mut app = App::new(Config::default()).expect("App::new failed");
        app.add_plugins(KitPlugins::set());
        app.add_plugin(ScreensPlugin);
        app.add_plugin(StoryUiPlugin);
        app.world_mut()
            .resource_mut::<tungsten_core::AssetRegistry>()
            .intern_sprites(["keeper_face", "keeper_worried"]);
        let mut ui = UiHarness::new(app);
        ui.harness_mut()
            .load_manifest(fixture.join("manifest.json"))
            .expect("the story fixture loads");
        let json = std::fs::read_to_string(fixture.join("ui/screens/dialogue.json"))
            .expect("the box's file");
        let tipped = json.replacen(
            r#""key": "speaker","#,
            r#""key": "speaker", "tooltip": "Who speaks","#,
            1,
        );
        assert_ne!(tipped, json, "the speaker takes a tooltip");
        let file = ScreenFile::from_json(&tipped).expect("the box's file reads");
        ui.world_mut()
            .resource_mut::<UiScreenRegistry>()
            .insert("dialogue", file);
        let world = ui.world_mut();
        let (keeper, warden) = (world.spawn(), world.spawn());
        assert!(talk(ui.world_mut(), "keeper.evening", keeper, warden));
        ui.step(12);
        let speaker = keyed(&ui, "speaker");
        let root = shown_on(&mut ui, speaker);
        assert_eq!(text_under(&ui, root).as_deref(), Some("Who speaks"));
    }
}

#[test]
fn closing_or_covering_the_screen_takes_its_tooltip() {
    let mut ui = menu();
    let used = keyed(&ui, "use");

    // Covered by another screen.
    let root = shown_on(&mut ui, used);
    assert!(crate::screens::open(ui.world_mut(), "other_menu"));
    ui.step(1);
    assert_eq!(top(&ui), Some("other_menu"));
    assert_eq!(shown_tooltip(ui.world()), None);
    let tree = ui.tree();
    assert!(
        !tree.contains(root)
            || tree.visibility(root) != Some(tungsten_core::ui::Visibility::Visible),
        "not shown while covered"
    );
    ui.step(12);
    tap(&mut ui, KeyCode::Escape);
    ui.step(12);
    assert_eq!(top(&ui), Some("menu"));

    // Closed: gone with the screen's slide out, the root with its state.
    let root = shown_on(&mut ui, used);
    assert!(pop(ui.world_mut()));
    ui.step(1);
    assert_eq!(shown_tooltip(ui.world()), None, "while it slides out");
    assert!(!ui.tree().contains(root));
    ui.step(12);
    assert_eq!(top(&ui), None);
}
