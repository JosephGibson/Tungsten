//! The `ui` scene's screens through the headless harness (`D-125`): the
//! engine rules the template's tests do not reach, kept from the UI lab
//! (`D-193`). No GPU, no fonts and no manifest.
//!
//! - A modal popup over a window opens a second text batch (`D-147`).
//! - A modal takes Escape and the click under it (`D-149`).
//! - The first key after a pointer press is the game's (`D-149`, `D-162`).
//! - Owned roots freeze and fade under a transition, and a popped state's
//!   root goes (`D-163`).
//! - The budget menu holds 100 nodes, and the prewarm hands its runs to the
//!   renderer once (`D-163`).

use example_03_fixtures::ui::hud::{LabHud, PlayHud};
use example_03_fixtures::ui::inventory::{self, Inventory};
use example_03_fixtures::ui::menus::{self, PauseMenu, PlayState};
use example_03_fixtures::ui::{budget, controls, labels, screens};
use tungsten::core::ui::{UiEvent, UiLayer, UiTree, Visibility, WidgetId, WidgetKind};
use tungsten::core::{AssetRegistry, Config, EventQueue, KeyCode};
use tungsten::testing::UiHarness;
use tungsten::{App, StateStack};

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
