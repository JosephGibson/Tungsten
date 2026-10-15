//! The lab's screens through the headless harness (`D-125`): the paint
//! list's snapshot, with no GPU and no fonts, the popup driven by routed
//! input (`D-149`), the "Settings" window's controls (M55), and the
//! gameplay screens' states and the menu fixtures (M56).

use example_05_ui_lab::menus::{self, PauseMenu, PlayHud, PlayState, REBIND_ACTION, SettingsMenu};
use example_05_ui_lab::screens;
use tungsten::core::ui::{UiEvent, UiLayer, UiTree, Visibility, WidgetId, WidgetKind};
use tungsten::core::{
    ActionMap, AssetRegistry, AudioCommand, Binding, Config, DisplayMode, EventQueue,
    GamepadButton, KeyCode, MouseButton, ScaleFactor, Settings,
};
use tungsten::testing::UiHarness;
use tungsten::{App, StateStack};

fn lab() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_system_named("ui_lab_popup", screens::popup_system);
    let mut harness = UiHarness::new(app);
    let world = harness.world_mut();
    let mut tree = world.remove_resource::<UiTree>().expect("UiTree");
    let sprites = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry");
    let lab = screens::build(&mut tree, sprites);
    world.insert_resource(tree);
    world.insert_resource(lab);
    harness
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
    assert_eq!(layers, [UiLayer::Game, UiLayer::Game, UiLayer::GamePopup]);
    let lab = harness
        .world()
        .get_resource::<screens::UiLab>()
        .expect("UiLab");
    assert_eq!(tree.kind(lab.counter), Some(WidgetKind::Label));
    assert!(
        !tree.hit_testable(lab.hud),
        "HUD text is pointer-transparent"
    );
    assert!(
        tree.focused().is_some(),
        "the popup's confirm button holds focus"
    );
}

#[test]
fn escape_hides_the_popup_and_the_windows_drop_shows_it_again() {
    let mut harness = lab();
    assert_eq!(harness.step(1), 1);
    let lab = harness
        .world()
        .get_resource::<screens::UiLab>()
        .expect("UiLab");
    let (popup, drop, drop_it) = (lab.popup, lab.drop, lab.drop_it);
    assert_eq!(harness.tree().focused(), Some(drop_it));

    harness.click(drop);
    harness.step(1);
    assert_eq!(
        harness.tree().visibility(popup),
        Some(Visibility::Visible),
        "the window's Drop under the modal popup changes nothing"
    );
    assert_eq!(harness.tree().focused(), Some(drop_it));
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
    assert_eq!(harness.tree().focused(), None, "and drops its focus");
    assert!(!harness.harness().exit_requested(), "the popup took it");

    harness.step(1);
    harness.click(drop);
    harness.step(1);
    assert_eq!(
        harness.tree().visibility(popup),
        Some(Visibility::Visible),
        "the same click with the popup hidden shows it"
    );
    assert_eq!(harness.tree().focused(), Some(drop_it), "on Drop it");
}

/// The "Settings" window alone, with its system and the damage numbers it
/// drives.
fn controls_lab() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.world_mut()
        .insert_resource(screens::DamageNumbers::default());
    app.add_system_named("ui_lab_controls", screens::controls_system);
    let mut harness = UiHarness::new(app);
    let controls = screens::build_controls(harness.tree_mut());
    harness.world_mut().insert_resource(controls);
    harness.layout();
    harness
}

#[test]
fn the_settings_volume_and_damage_switch_follow_the_pointer_and_keys() {
    let mut harness = controls_lab();
    let controls = harness.world().resource::<screens::UiControls>();
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
            .resource::<screens::DamageNumbers>()
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

#[test]
fn the_display_mode_button_cycles_the_three_modes_into_settings() {
    let mut harness = play();
    settings(&mut harness);
    let button = menu(&harness).display_mode;
    assert_eq!(harness.tree().focused(), Some(button));
    let mut seen = Vec::new();
    for _ in 0..3 {
        tap(&mut harness, KeyCode::Enter);
        seen.push(
            harness
                .world()
                .resource::<Settings>()
                .display_mode()
                .expect("set"),
        );
    }
    assert_eq!(
        seen,
        [
            DisplayMode::BorderlessFullscreen,
            DisplayMode::ExclusiveFullscreen,
            DisplayMode::Windowed
        ]
    );
    assert_eq!(menu(&harness).display_requests, 3);
    assert_eq!(
        harness.tree().accessible_name(button),
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
    menus::build_budget_menu(&mut tree, false);
    let nodes: usize = tree.roots().iter().map(|root| count(&tree, *root)).sum();
    assert_eq!(nodes, 100);
}

#[test]
fn the_prewarm_fixtures_first_frame_hands_the_menus_runs_to_the_renderer() {
    let app = App::new(Config::default()).expect("App::new failed");
    let mut harness = UiHarness::new(app);
    let menu = menus::build_budget_menu(harness.tree_mut(), true);
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
