//! The lab's screens through the headless harness (`D-125`): the paint
//! list's snapshot, with no GPU and no fonts, the inventory and its
//! confirmation driven by routed input (`D-149`), the "Settings" window's
//! controls (M55), the gameplay screens' states, the menu fixtures (M56)
//! and the inventory by every device (M59).

use example_05_ui_lab::hud::{LabHud, PlayHud};
use example_05_ui_lab::inventory::{self, Inventory};
use example_05_ui_lab::menus::{self, PauseMenu, PlayState, REBIND_ACTION, SettingsMenu};
use example_05_ui_lab::{budget, controls, labels, screens};
use glam::Vec2;
use tungsten::core::ui::{Rect, UiEvent, UiLayer, UiTree, Visibility, WidgetId, WidgetKind};
use tungsten::core::{
    ActionMap, AssetRegistry, AudioCommand, Binding, Config, DisplayMode, EventQueue,
    GamepadButton, GamepadId, InputState, KeyCode, MouseButton, ScaleFactor, Settings, UserFolder,
};
use tungsten::testing::UiHarness;
use tungsten::{App, StateStack, WindowSize};

/// The paint-path screen: the lab HUD and the inventory with its
/// confirmation open, driven by the inventory's system.
fn lab() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_system_named("ui_lab_inventory", inventory::inventory_system);
    let mut harness = UiHarness::new(app);
    let world = harness.world_mut();
    let mut tree = world.remove_resource::<UiTree>().expect("UiTree");
    let sprites = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry");
    let (lab_hud, inventory) = screens::build(&mut tree, sprites);
    world.insert_resource(tree);
    world.insert_resource(lab_hud);
    world.insert_resource(inventory);
    harness
}

fn inventory(harness: &UiHarness) -> &Inventory {
    harness.world().resource::<Inventory>()
}

#[test]
fn the_popup_over_the_window_opens_a_second_text_batch() {
    let mut harness = lab();
    assert_eq!(harness.step(2), 2);
    let dump = harness.dump_paint();
    assert!(dump.contains("-- batch 0\n"), "{dump}");
    assert!(dump.contains("-- batch 1\n"), "{dump}");
    assert!(
        !dump.contains("-- batch 2\n"),
        "the HUD and the window share a batch:\n{dump}"
    );
    assert!(
        !dump.contains("Image "),
        "unregistered sprites draw nothing:\n{dump}"
    );
    let tree = harness.tree();
    assert_eq!(tree.roots().len(), 3);
    let layers: Vec<UiLayer> = tree
        .roots()
        .iter()
        .filter_map(|root| tree.layer(*root))
        .collect();
    assert_eq!(
        layers,
        [UiLayer::Game, UiLayer::GamePopup, UiLayer::GamePopup]
    );
    let lab_hud = harness.world().resource::<LabHud>();
    assert_eq!(tree.kind(lab_hud.counter), Some(WidgetKind::Label));
    assert!(
        !tree.hit_testable(lab_hud.root),
        "HUD text is pointer-transparent"
    );
    assert_eq!(
        tree.focused(),
        Some(inventory(&harness).ui.confirm.cancel),
        "the confirmation's Cancel holds focus"
    );
}

#[test]
fn escape_hides_the_popup_and_the_windows_drop_shows_it_again() {
    let mut harness = lab();
    assert_eq!(harness.step(1), 1);
    let ui = inventory(&harness).ui.clone();
    let (popup, drop, cancel) = (ui.confirm.root, ui.pane.drop, ui.confirm.cancel);
    assert_eq!(harness.tree().focused(), Some(cancel));

    harness.click(drop);
    harness.step(1);
    assert_eq!(
        harness.tree().visibility(popup),
        Some(Visibility::Visible),
        "the window's Drop under the modal popup changes nothing"
    );
    assert_eq!(harness.tree().focused(), Some(cancel));
    assert_eq!(
        harness.tree().hovered(),
        None,
        "the modal blocks the window"
    );
    assert!(
        !harness
            .world()
            .get_resource::<EventQueue<UiEvent>>()
            .expect("UiEvent queue")
            .iter()
            .any(|event| matches!(event, UiEvent::Activated { widget, .. } if *widget == drop)),
        "Drop was not activated"
    );

    harness.key_down(KeyCode::Escape);
    harness.step(1);
    harness.key_up(KeyCode::Escape);
    assert_eq!(
        harness.tree().visibility(popup),
        Some(Visibility::Hidden),
        "Escape hides the popup"
    );
    assert_eq!(
        harness.tree().focused(),
        Some(ui.grid.slots[0].root),
        "and focus goes back to its slot"
    );
    assert!(!harness.harness().exit_requested(), "the popup took it");
    assert!(inventory(&harness).contents[0].is_some(), "nothing dropped");

    harness.step(1);
    harness.click(drop);
    harness.step(1);
    assert_eq!(
        harness.tree().visibility(popup),
        Some(Visibility::Visible),
        "the same click with the popup hidden shows it"
    );
    assert_eq!(harness.tree().focused(), Some(cancel), "on Cancel");
}

/// The "Settings" window alone, with its system and the damage numbers it
/// drives.
fn controls_lab() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.world_mut()
        .insert_resource(labels::DamageNumbers::default());
    app.add_system_named("ui_lab_controls", controls::controls_system);
    let mut harness = UiHarness::new(app);
    let controls = controls::build_controls(harness.tree_mut());
    harness.world_mut().insert_resource(controls);
    harness.layout();
    harness
}

#[test]
fn the_settings_volume_and_damage_switch_follow_the_pointer_and_keys() {
    let mut harness = controls_lab();
    let controls = harness.world().resource::<controls::UiControls>();
    let (volume, readout, damage) = (controls.volume, controls.volume_readout, controls.damage);
    let shown = |harness: &UiHarness| {
        harness
            .tree()
            .accessible_name(readout)
            .map(str::to_owned)
            .unwrap_or_default()
    };
    assert_eq!(shown(&harness), "80%");

    let rect = harness.rect(volume).expect("laid out");
    harness.pointer_to(rect.min.x + 1.0, f32::midpoint(rect.min.y, rect.max.y));
    harness.pointer_down();
    harness.pointer_up();
    harness.step(1);
    assert_eq!(shown(&harness), "0%", "a click at the slider's start");

    harness.key_down(KeyCode::ArrowRight);
    harness.step(1);
    harness.key_up(KeyCode::ArrowRight);
    assert_eq!(
        shown(&harness),
        "0%",
        "the first key after the pointer is the game's (D-149)"
    );
    harness.step(1);
    harness.key_down(KeyCode::ArrowRight);
    harness.step(1);
    harness.key_up(KeyCode::ArrowRight);
    assert_eq!(shown(&harness), "5%");

    let numbers = |harness: &UiHarness| {
        harness
            .world()
            .resource::<labels::DamageNumbers>()
            .enabled()
    };
    assert!(numbers(&harness));
    harness.click(damage);
    harness.step(1);
    assert!(
        !numbers(&harness),
        "the toggle turns the damage numbers off"
    );
}

// ---- M56: the gameplay screens ----

/// The play state on an empty lab, entered.
fn play() -> UiHarness {
    let app = App::new(Config::default()).expect("App::new failed");
    let mut harness = UiHarness::new(app);
    menus::add_pause_pad_binding(harness.world_mut());
    inventory::add_bindings(harness.world_mut());
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(PlayState::new(Vec::new()));
    harness.step(1);
    harness
}

fn transitioning(harness: &UiHarness) -> bool {
    harness.world().resource::<StateStack>().is_transitioning()
}

fn top(harness: &UiHarness) -> &'static str {
    harness
        .world()
        .resource::<StateStack>()
        .active_id()
        .unwrap_or("")
}

/// Steps until the transition ends.
fn settle(harness: &mut UiHarness) {
    let mut frames = 0;
    while transitioning(harness) {
        harness.step(1);
        frames += 1;
        assert!(frames < 200, "the transition never ended");
    }
}

fn tap(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

/// Presses P and steps through the fade into the pause menu.
fn pause(harness: &mut UiHarness) -> (WidgetId, WidgetId, WidgetId) {
    tap(harness, KeyCode::KeyP);
    settle(harness);
    let menu = harness.world().resource::<PauseMenu>();
    (menu.root, menu.resume, menu.settings)
}

/// Pauses, focuses Settings, presses Enter and steps into the settings
/// screen.
fn settings(harness: &mut UiHarness) {
    let (_, _, settings) = pause(harness);
    assert!(harness.focus(settings));
    tap(harness, KeyCode::Enter);
    settle(harness);
    assert_eq!(top(harness), "settings");
}

fn menu(harness: &UiHarness) -> &SettingsMenu {
    harness.world().resource::<SettingsMenu>()
}

#[test]
fn state_pause_pushes_the_pause_menu_behind_a_fade_frozen_until_it_ends() {
    let mut harness = play();
    let hud = harness.world().resource::<PlayHud>().root;
    harness.key_down(KeyCode::KeyP);
    harness.step(1);
    harness.key_up(KeyCode::KeyP);
    assert!(transitioning(&harness));
    let mut frames = 0;
    while !harness.world().has_resource::<PauseMenu>() {
        assert!(
            harness.tree().frozen(hud),
            "the play HUD freezes for the fade"
        );
        harness.step(1);
        frames += 1;
        assert!(frames < 100);
    }
    let menu = harness.world().resource::<PauseMenu>();
    let (root, resume) = (menu.root, menu.resume);
    harness.step(1);
    let tree = harness.tree();
    assert!(tree.frozen(root), "frozen while it fades in");
    let fade = tree.fade(root).unwrap();
    assert!(fade > 0.0 && fade < 1.0, "faded: {fade}");
    settle(&mut harness);
    let tree = harness.tree();
    assert!(!tree.frozen(root));
    assert_eq!(tree.fade(root), Some(1.0));
    assert_eq!(tree.focused(), Some(resume), "Resume focused");
    assert!(
        tree.frozen(hud),
        "the covered play HUD stays shown and frozen"
    );
    assert_eq!(tree.visibility(hud), Some(Visibility::Visible));
}

#[test]
fn resume_and_ui_cancel_pop_the_pause_menu_and_its_root_goes() {
    let mut harness = play();
    for key in [KeyCode::Enter, KeyCode::Escape] {
        let (root, resume, _) = pause(&mut harness);
        assert_eq!(harness.tree().focused(), Some(resume));
        tap(&mut harness, key);
        assert!(transitioning(&harness), "{key:?} pops behind a fade");
        settle(&mut harness);
        assert_eq!(top(&harness), "play", "{key:?}");
        assert!(!harness.tree().contains(root), "{key:?}: its root is gone");
        assert!(!harness.world().has_resource::<PauseMenu>());
        assert!(!harness.harness().exit_requested());
        let hud = harness.world().resource::<PlayHud>().root;
        assert!(!harness.tree().frozen(hud), "play takes input again");
    }
}

/// A play harness whose user folder is a fresh temp folder named `name`.
fn play_at(name: &str) -> UiHarness {
    let dir = std::env::temp_dir().join(format!("ui_lab_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp folder");
    let mut config = Config::default();
    config.game.user_folder = UserFolder::At(dir);
    let mut harness = UiHarness::new(App::new(config).expect("App::new failed"));
    menus::add_pause_pad_binding(harness.world_mut());
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(PlayState::new(Vec::new()));
    harness.step(1);
    harness
}

#[test]
fn the_display_mode_button_cycles_the_three_modes_into_settings() {
    let mut harness = play_at("display_modes");
    settings(&mut harness);
    let path = harness
        .world()
        .resource::<Settings>()
        .path()
        .expect("a user folder")
        .to_path_buf();
    let (mut seen, mut saved) = (Vec::new(), Vec::new());
    for _ in 0..3 {
        let button = menu(&harness).display_mode;
        assert_eq!(harness.tree().focused(), Some(button));
        tap(&mut harness, KeyCode::Enter);
        seen.push(
            harness
                .world()
                .resource::<Settings>()
                .display_mode()
                .expect("set"),
        );
        assert_eq!(menu(&harness).display_requests, 1);
        // Back saves and pops; the file holds the mode; the screen reopens.
        let back = menu(&harness).back;
        assert!(harness.focus(back));
        tap(&mut harness, KeyCode::Enter);
        settle(&mut harness);
        assert_eq!(top(&harness), "pause");
        saved.push(Settings::load(&path).display_mode().expect("saved"));
        let reopen = harness.world().resource::<PauseMenu>().settings;
        assert!(harness.focus(reopen));
        tap(&mut harness, KeyCode::Enter);
        settle(&mut harness);
        assert_eq!(top(&harness), "settings");
    }
    let cycle = [
        DisplayMode::BorderlessFullscreen,
        DisplayMode::ExclusiveFullscreen,
        DisplayMode::Windowed,
    ];
    assert_eq!(seen, cycle);
    assert_eq!(saved, cycle);
    assert_eq!(
        harness.tree().accessible_name(menu(&harness).display_mode),
        Some("Display: windowed")
    );
}

#[test]
fn the_ui_scale_applies_at_the_pointers_release_and_not_before() {
    let mut harness = play();
    settings(&mut harness);
    let slider = menu(&harness).ui_scale;
    let rect = harness.rect(slider).expect("laid out");
    let scale = harness.world().resource::<ScaleFactor>().get();
    let y = f32::midpoint(rect.min.y, rect.max.y) * scale;
    harness.pointer_to(rect.min.x * scale + 1.0, y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(rect.max.x * scale + 40.0, y);
    harness.step(1);
    assert_eq!(
        harness.tree().slider_value(slider),
        Some(2.0),
        "the slider follows the pointer"
    );
    assert_eq!(
        harness.world().resource::<ScaleFactor>().ui_scale(),
        1.0,
        "not before the release"
    );
    harness.pointer_up();
    harness.step(1);
    assert_eq!(harness.world().resource::<ScaleFactor>().ui_scale(), 2.0);
    assert_eq!(harness.world().resource::<Settings>().ui_scale(), Some(2.0));
}

#[test]
fn the_text_scale_applies_on_a_key_step() {
    let mut harness = play();
    settings(&mut harness);
    let slider = menu(&harness).text_scale;
    assert!(harness.focus(slider));
    tap(&mut harness, KeyCode::ArrowRight);
    assert_eq!(harness.tree().text_scale(), 1.25);
    assert_eq!(
        harness.world().resource::<Settings>().text_scale(),
        Some(1.25)
    );
}

#[test]
fn the_volume_reaches_the_audio_commands() {
    let mut harness = play();
    settings(&mut harness);
    let slider = menu(&harness).volume;
    assert!(harness.focus(slider));
    harness.key_down(KeyCode::ArrowLeft);
    harness.step(1);
    harness.key_up(KeyCode::ArrowLeft);
    assert!(
        harness
            .harness()
            .audio()
            .iter()
            .any(|command| matches!(command, AudioCommand::SetMasterVolume(v) if (*v - 0.95).abs() < 1e-6)),
        "{:?}",
        harness.harness().audio()
    );
}

#[test]
fn the_rebind_captures_a_key_and_keeps_the_pads_binding() {
    let mut harness = play();
    let south = Binding::GamepadButton {
        button: GamepadButton::South,
    };
    harness
        .world_mut()
        .resource_mut::<ActionMap>()
        .replace_bindings(
            REBIND_ACTION,
            vec![
                Binding::Key {
                    code: KeyCode::Space,
                },
                Binding::Mouse {
                    button: MouseButton::Left,
                },
                south,
            ],
        );
    settings(&mut harness);
    let rebind = menu(&harness).rebind;
    assert_eq!(harness.tree().accessible_name(rebind), Some("Space"));
    assert!(harness.focus(rebind));
    tap(&mut harness, KeyCode::Enter);
    assert!(
        harness
            .world()
            .resource::<tungsten::core::ui::UiRouter>()
            .capturing()
            .is_some()
    );
    harness.key_down(KeyCode::KeyK);
    harness.step(1);
    harness.key_up(KeyCode::KeyK);
    assert_eq!(
        harness
            .world()
            .resource::<ActionMap>()
            .bindings(REBIND_ACTION),
        [
            Binding::Key {
                code: KeyCode::KeyK
            },
            south
        ]
    );
    harness.step(1);
    assert_eq!(
        harness.tree().accessible_name(rebind),
        Some("K"),
        "after the next routing"
    );
    assert_eq!(
        top(&harness),
        "settings",
        "the capture took K from the game"
    );
}

/// Every node under `id`, itself included.
fn count(tree: &UiTree, id: WidgetId) -> usize {
    1 + tree
        .children(id)
        .iter()
        .map(|child| count(tree, *child))
        .sum::<usize>()
}

#[test]
fn the_menu_fixture_holds_exactly_100_nodes() {
    let mut tree = UiTree::new();
    budget::build_budget_menu(&mut tree, false);
    let nodes: usize = tree.roots().iter().map(|root| count(&tree, *root)).sum();
    assert_eq!(nodes, 100);
}

#[test]
fn the_prewarm_fixtures_first_frame_hands_the_menus_runs_to_the_renderer() {
    let app = App::new(Config::default()).expect("App::new failed");
    let mut harness = UiHarness::new(app);
    let menu = budget::build_budget_menu(harness.tree_mut(), true);
    harness.tree_mut().prewarm(menu.root);
    harness.layout();
    harness.step(1);
    let draw = harness.harness().draw().expect("a frame ran");
    let prewarm = draw.ui_prewarm.as_ref().expect("the first frame's runs");
    assert_eq!(prewarm.commands.len(), 48, "title, 46 buttons, footer");
    assert!(
        prewarm
            .commands
            .iter()
            .all(tungsten::core::ui::PaintCommand::is_text)
    );
    assert!(draw.ui.is_empty(), "hidden until frame 240");
    harness.step(1);
    assert!(harness.harness().draw().unwrap().ui_prewarm.is_none());
}

// ---- M59: the inventory ----

/// Presses and releases `button` on pad 0, a frame each.
fn pad_tap(harness: &mut UiHarness, button: GamepadButton) {
    let pad = GamepadId::new(0);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .gamepad_button_down(pad, button);
    harness.step(1);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .gamepad_button_up(pad, button);
    harness.step(1);
}

/// Presses I in play and steps through the fade into the inventory.
fn open_inventory(harness: &mut UiHarness) {
    tap(harness, KeyCode::KeyI);
    settle(harness);
    assert_eq!(top(harness), "inventory");
}

fn shown_text(harness: &UiHarness, label: WidgetId) -> String {
    harness
        .tree()
        .accessible_name(label)
        .map(str::to_owned)
        .unwrap_or_default()
}

#[test]
fn the_grid_scrolls_to_the_focused_slot_and_the_pane_follows() {
    let mut harness = play();
    open_inventory(&mut harness);
    let ui = inventory(&harness).ui.clone();
    assert_eq!(harness.tree().focused(), Some(ui.grid.slots[0].root));
    assert_eq!(shown_text(&harness, ui.pane.name), "Healing Draught");
    assert_eq!(harness.tree().scroll(ui.grid.panel), Some(0.0));
    for _ in 0..3 {
        tap(&mut harness, KeyCode::ArrowDown);
    }
    assert_eq!(harness.tree().focused(), Some(ui.grid.slots[15].root));
    harness.step(1);
    assert!(
        harness
            .tree()
            .scroll(ui.grid.panel)
            .is_some_and(|offset| offset > 0.0),
        "the fourth row scrolls into view: {:?}",
        harness.tree().scroll(ui.grid.panel)
    );
    assert_eq!(shown_text(&harness, ui.pane.name), "Moonstone");
    tap(&mut harness, KeyCode::ArrowRight);
    assert_eq!(harness.tree().focused(), Some(ui.grid.slots[16].root));
    assert_eq!(shown_text(&harness, ui.pane.name), "Empty slot");
    assert!(!harness.tree().enabled(ui.pane.drop));
}

#[test]
fn use_equip_and_drop_by_pointer_keys_and_pad() {
    let mut harness = play();
    // U during the fade-in changes nothing: the screen is not live yet.
    tap(&mut harness, KeyCode::KeyI);
    let mut frames = 0;
    while !harness.world().has_resource::<Inventory>() {
        harness.step(1);
        frames += 1;
        assert!(frames < 100);
    }
    assert!(harness.world().resource::<StateStack>().is_transitioning());
    tap(&mut harness, KeyCode::KeyU);
    settle(&mut harness);
    assert_eq!(
        inventory(&harness).contents[0].map(|stack| stack.count),
        Some(3)
    );
    let ui = inventory(&harness).ui.clone();

    // A click on Use takes one; E equips the focused gear and shows the badge.
    harness.click(ui.pane.use_button);
    harness.step(1);
    assert_eq!(
        inventory(&harness).contents[0].map(|stack| stack.count),
        Some(2)
    );
    assert!(harness.focus(ui.grid.slots[1].root));
    tap(&mut harness, KeyCode::KeyE);
    assert_eq!(inventory(&harness).equipped[0], Some(1));
    assert_eq!(
        harness.tree().visibility(ui.pane.badge),
        Some(Visibility::Visible)
    );
    assert_eq!(shown_text(&harness, ui.pane.equip), "Unequip");

    // The pad's Select asks with Cancel focused; U does nothing meanwhile,
    // and South there cancels.
    assert!(harness.focus(ui.grid.slots[0].root));
    pad_tap(&mut harness, GamepadButton::Select);
    assert_eq!(
        harness.tree().visibility(ui.confirm.root),
        Some(Visibility::Visible)
    );
    assert_eq!(harness.tree().focused(), Some(ui.confirm.cancel));
    tap(&mut harness, KeyCode::KeyU);
    assert_eq!(
        inventory(&harness).contents[0].map(|stack| stack.count),
        Some(2)
    );
    pad_tap(&mut harness, GamepadButton::South);
    assert_eq!(
        harness.tree().visibility(ui.confirm.root),
        Some(Visibility::Hidden)
    );
    assert!(inventory(&harness).contents[0].is_some());

    // Select, Right, South drops the stack and keeps focus on the slot.
    pad_tap(&mut harness, GamepadButton::Select);
    pad_tap(&mut harness, GamepadButton::DPadRight);
    assert_eq!(harness.tree().focused(), Some(ui.confirm.drop));
    pad_tap(&mut harness, GamepadButton::South);
    assert_eq!(inventory(&harness).contents[0], None);
    assert_eq!(harness.tree().focused(), Some(ui.grid.slots[0].root));
    assert_eq!(shown_text(&harness, ui.pane.name), "Empty slot");

    // Escape cancels a third drop and leaves the inventory open.
    assert!(harness.focus(ui.grid.slots[4].root));
    tap(&mut harness, KeyCode::Delete);
    assert_eq!(
        harness.tree().visibility(ui.confirm.root),
        Some(Visibility::Visible)
    );
    tap(&mut harness, KeyCode::Escape);
    assert_eq!(
        harness.tree().visibility(ui.confirm.root),
        Some(Visibility::Hidden)
    );
    assert!(inventory(&harness).contents[4].is_some());
    assert_eq!(top(&harness), "inventory");
    assert!(!harness.harness().exit_requested());
}

// ---- M59: alignment ----

/// Within half a unit.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn laid(harness: &UiHarness, id: WidgetId) -> Rect {
    harness
        .rect(id)
        .unwrap_or_else(|| panic!("{id:?} has no box"))
}

fn centre(rect: Rect) -> Vec2 {
    (rect.min + rect.max) * 0.5
}

/// R1: `root` centred in `viewport`.
fn assert_centred(harness: &UiHarness, root: WidgetId, viewport: Vec2, what: &str) {
    let at = centre(laid(harness, root));
    let mid = viewport * 0.5;
    assert!(
        near(at.x, mid.x) && near(at.y, mid.y),
        "R1: {what} centred at {at} in {viewport}"
    );
}

/// R2: `buttons` share one width and the column's centre x.
fn assert_menu_column(harness: &UiHarness, column: WidgetId, buttons: &[WidgetId], what: &str) {
    let column = laid(harness, column);
    for button in buttons {
        let rect = laid(harness, *button);
        assert!(
            near(rect.width(), laid(harness, buttons[0]).width())
                && near(centre(rect).x, centre(column).x),
            "R2: {what}: {rect:?} in {column:?}"
        );
    }
}

/// R3: each control's row puts its caption and control on one centre y,
/// the captions share a left edge and a width, the controls a left edge;
/// `sliders` share a width.
fn assert_setting_rows(
    harness: &UiHarness,
    controls: &[WidgetId],
    sliders: &[WidgetId],
    what: &str,
) {
    let tree = harness.tree();
    let parts: Vec<(Rect, Rect)> = controls
        .iter()
        .map(|control| {
            let row = tree.parent(*control).expect("a row");
            let caption = tree.children(row)[0];
            (laid(harness, caption), laid(harness, *control))
        })
        .collect();
    let (first_caption, first_control) = parts[0];
    for (caption, control) in &parts {
        assert!(
            near(caption.min.x, first_caption.min.x)
                && near(caption.width(), first_caption.width()),
            "R3: {what}: caption {caption:?} against {first_caption:?}"
        );
        assert!(
            near(control.min.x, first_control.min.x),
            "R3: {what}: control {control:?} against {first_control:?}"
        );
        assert!(
            near(centre(*caption).y, centre(*control).y),
            "R3: {what}: caption {caption:?} and control {control:?} off one centre"
        );
    }
    for slider in sliders {
        assert!(
            near(
                laid(harness, *slider).width(),
                laid(harness, sliders[0]).width()
            ),
            "R3: {what}: slider widths"
        );
    }
}

/// R4: `buttons` have equal widths and gaps, centred under `under`.
fn assert_action_row(harness: &UiHarness, buttons: &[WidgetId], under: WidgetId, what: &str) {
    let rects: Vec<Rect> = buttons.iter().map(|id| laid(harness, *id)).collect();
    for pair in rects.windows(2) {
        assert!(
            near(pair[0].width(), pair[1].width()),
            "R4: {what}: widths {rects:?}"
        );
        assert!(
            near(
                pair[1].min.x - pair[0].max.x,
                rects[1].min.x - rects[0].max.x
            ),
            "R4: {what}: gaps {rects:?}"
        );
    }
    let group = f32::midpoint(rects[0].min.x, rects[rects.len() - 1].max.x);
    assert!(
        near(group, centre(laid(harness, under)).x),
        "R4: {what}: group centre {group} under {:?}",
        laid(harness, under)
    );
}

fn disjoint(a: Rect, b: Rect) -> bool {
    a.max.x <= b.min.x || b.max.x <= a.min.x || a.max.y <= b.min.y || b.max.y <= a.min.y
}

#[test]
fn every_screen_is_centred_and_aligned_at_three_sizes() {
    for (width, height) in [(1280_u32, 720_u32), (1920, 1080), (2560, 1440)] {
        let mut harness = UiHarness::new(App::new(Config::default()).expect("App::new failed"));
        let world = harness.world_mut();
        *world.resource_mut::<WindowSize>() = WindowSize { width, height };
        world.resource_mut::<ScaleFactor>().set_os(1.75);
        menus::add_pause_pad_binding(world);
        inventory::add_bindings(world);
        let roots = menus::build_play_screen(world, true);
        world
            .resource_mut::<StateStack>()
            .request_push(PlayState::new(roots));
        harness.step(2);
        let viewport = Vec2::new(width as f32, height as f32) / 1.75;
        let at = format!("{width}x{height} at 1.75");

        // The play screen: R6, R7, R3 on the controls window.
        let lab_hud = laid(&harness, harness.world().resource::<LabHud>().root);
        let play_hud = laid(&harness, harness.world().resource::<PlayHud>().root);
        let controls = harness.world().resource::<controls::UiControls>();
        let (window, damage, spawn_rate, volume, locked) = (
            controls.window,
            controls.damage,
            controls.spawn_rate,
            controls.volume,
            controls.locked,
        );
        assert!(
            near(lab_hud.min.x, 16.0) && near(lab_hud.min.y, 16.0),
            "R6: {at}: {lab_hud:?}"
        );
        assert!(
            near(play_hud.min.x, 16.0) && near(play_hud.max.y, viewport.y - 16.0),
            "R6: {at}: {play_hud:?}"
        );
        let window_rect = laid(&harness, window);
        for (a, b) in [
            (lab_hud, window_rect),
            (lab_hud, play_hud),
            (window_rect, play_hud),
        ] {
            assert!(disjoint(a, b), "R7: {at}: {a:?} and {b:?} overlap");
        }
        assert_setting_rows(
            &harness,
            &[damage, spawn_rate, volume, locked],
            &[spawn_rate, volume],
            &format!("controls {at}"),
        );

        // The pause menu: R1, R2.
        tap(&mut harness, KeyCode::KeyP);
        settle(&mut harness);
        let pause = harness.world().resource::<PauseMenu>();
        let (root, resume, settings_button, inventory_button) =
            (pause.root, pause.resume, pause.settings, pause.inventory);
        assert_centred(&harness, root, viewport, &format!("pause {at}"));
        assert_menu_column(
            &harness,
            root,
            &[resume, settings_button, inventory_button],
            &format!("pause {at}"),
        );

        // The settings screen: R1, R2, R3.
        assert!(harness.focus(settings_button));
        tap(&mut harness, KeyCode::Enter);
        settle(&mut harness);
        assert_eq!(top(&harness), "settings");
        let menu = menu(&harness);
        let (root, display_mode, back, ui_scale, text_scale, volume, rebind) = (
            menu.root,
            menu.display_mode,
            menu.back,
            menu.ui_scale,
            menu.text_scale,
            menu.volume,
            menu.rebind,
        );
        assert_centred(&harness, root, viewport, &format!("settings {at}"));
        assert_menu_column(
            &harness,
            root,
            &[display_mode, back],
            &format!("settings {at}"),
        );
        assert_setting_rows(
            &harness,
            &[ui_scale, text_scale, volume, rebind],
            &[ui_scale, text_scale, volume],
            &format!("settings {at}"),
        );

        // The inventory: R1, R4, R5.
        tap(&mut harness, KeyCode::Escape);
        settle(&mut harness);
        assert!(harness.focus(inventory_button));
        tap(&mut harness, KeyCode::Enter);
        settle(&mut harness);
        assert_eq!(top(&harness), "inventory");
        let ui = inventory(&harness).ui.clone();
        assert_centred(&harness, ui.root, viewport, &format!("inventory {at}"));
        assert_action_row(
            &harness,
            &[ui.pane.use_button, ui.pane.equip, ui.pane.drop],
            ui.pane.root,
            &format!("pane {at}"),
        );
        let slots: Vec<Rect> = ui
            .grid
            .slots
            .iter()
            .map(|slot| laid(&harness, slot.root))
            .collect();
        let pitch = Vec2::new(
            slots[1].min.x - slots[0].min.x,
            slots[5].min.y - slots[0].min.y,
        );
        for (i, slot) in slots.iter().enumerate() {
            let expected = slots[0].min + pitch * Vec2::new((i % 5) as f32, (i / 5) as f32);
            assert!(
                near(slot.min.x, expected.x) && near(slot.min.y, expected.y),
                "R5: {at}: slot {i} at {:?}, expected {expected}",
                slot.min
            );
        }
        assert!(
            near(
                f32::midpoint(slots[0].min.x, slots[4].max.x),
                centre(laid(&harness, ui.grid.panel)).x
            ),
            "R5: {at}: the grid off its panel's centre"
        );

        // The confirmation: R1, R4.
        tap(&mut harness, KeyCode::Delete);
        harness.step(1);
        assert_centred(
            &harness,
            ui.confirm.root,
            viewport,
            &format!("confirm {at}"),
        );
        assert_action_row(
            &harness,
            &[ui.confirm.cancel, ui.confirm.drop],
            ui.confirm.root,
            &format!("confirm {at}"),
        );
    }
}
