//! The lab's screens through the headless harness (`D-125`): the paint
//! list's snapshot, with no GPU and no fonts, the popup driven by routed
//! input (`D-149`), and the "Settings" window's controls (M55).

use example_05_ui_lab::screens;
use tungsten::App;
use tungsten::core::ui::{UiEvent, UiLayer, UiTree, Visibility, WidgetKind};
use tungsten::core::{AssetRegistry, Config, EventQueue, KeyCode};
use tungsten::testing::UiHarness;

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
