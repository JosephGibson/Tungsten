//! Keybindings: the listed actions, a capture, the device check, a swap,
//! the resets, the capture's timeout and the HUD's prompt after a rebind.

use tungsten::core::{
    ActionMap, Binding, GamepadButton, GamepadId, InputState, KeyCode, UserFolder,
};
use tungsten::testing::UiHarness;
use tungsten_template_basic::sample::hud::Hud;
use tungsten_template_basic::shell::capturing;
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::keybindings::{COLUMNS, KEYBINDINGS};
use tungsten_template_basic::shell::toast::Toasts;

use super::{harness, harness_with, press, start, top, user_folder};

const KEY: fn(KeyCode) -> Binding = |code| Binding::Key { code };

/// Opens Keybindings from the title through Settings' Controls tab.
fn keybindings(harness: &mut UiHarness) {
    press(harness, "Settings");
    press(harness, "Controls");
    press(harness, "Keybindings…");
    assert_eq!(top(harness), Some(KEYBINDINGS));
}

fn bindings(harness: &UiHarness, action: &str) -> Vec<Binding> {
    harness
        .world()
        .resource::<ActionMap>()
        .bindings(action)
        .to_vec()
}

/// The shipped bindings for `action`: `input.json` over the defaults,
/// under the player's layer.
fn shipped(harness: &UiHarness, action: &str) -> Vec<Binding> {
    let map = harness.world().resource::<ActionMap>();
    map.shipped(action).to_vec()
}

/// The actions `input.json`'s `labels` list, with their labels, in order.
fn labelled(harness: &UiHarness) -> Vec<(String, String)> {
    harness.world().resource::<ActionMap>().labelled().to_vec()
}

#[test]
fn every_bindable_action_is_listed_and_no_ui_or_engine_action() {
    let mut harness = harness();
    keybindings(&mut harness);
    let listed = labelled(&harness);
    for (action, label) in &listed {
        assert!(!action.starts_with("ui_") && !action.starts_with("engine_"));
        for column in COLUMNS {
            assert!(
                harness.named(&format!("{label} {column}")).is_some(),
                "{label} {column}"
            );
        }
    }
    let labels: Vec<&str> = listed.iter().map(|(_, label)| label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Move up",
            "Move down",
            "Move left",
            "Move right",
            "Pause",
            "Shoot"
        ]
    );
    // The rows follow `labels`' order down the card.
    let tops: Vec<f32> = labels
        .iter()
        .map(|label| {
            let cell = harness.named(&format!("{label} Primary")).unwrap();
            harness.rect(cell).unwrap().min.y
        })
        .collect();
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
    let up = harness.named("Move up Primary").unwrap();
    assert_eq!(harness.tree().plain_text(up), "Up");
    let pad = harness.named("Move up Gamepad").unwrap();
    assert_eq!(harness.tree().plain_text(pad), "Left Stick Up");
}

#[test]
fn rebinding_primary_captures_a_key_and_survives_a_restart() {
    let dir = user_folder("rebind");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    keybindings(&mut harness);
    press(&mut harness, "Move up Primary");
    assert!(capturing(harness.world()));
    let cell = harness.named("Move up Primary").unwrap();
    assert!(harness.tree().plain_text(cell).starts_with("Press a key…"));
    harness.tap_key(KeyCode::KeyI);
    assert!(!capturing(harness.world()));
    assert_eq!(bindings(&harness, "move_up")[0], KEY(KeyCode::KeyI));
    assert!(
        bindings(&harness, "move_up").contains(&KEY(KeyCode::KeyW)),
        "the rest kept"
    );
    assert_eq!(harness.tree().plain_text(cell), "I");

    let restarted = harness_with(UserFolder::At(dir.clone()));
    assert_eq!(bindings(&restarted, "move_up")[0], KEY(KeyCode::KeyI));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_key_column_refuses_a_pad_press() {
    let mut harness = harness();
    keybindings(&mut harness);
    let before = bindings(&harness, "move_up");
    press(&mut harness, "Move up Primary");
    let pad = GamepadId::new(0);
    let input = harness.world_mut().resource_mut::<InputState>();
    input.gamepad_button_down(pad, GamepadButton::South);
    harness.step(1);
    let input = harness.world_mut().resource_mut::<InputState>();
    input.gamepad_button_up(pad, GamepadButton::South);
    harness.step(1);
    assert_eq!(
        bindings(&harness, "move_up"),
        before,
        "the old binding kept"
    );
    let toasts = harness.world().resource::<Toasts>().texts(harness.tree());
    assert_eq!(toasts, ["Primary takes keys"]);
}

#[test]
fn a_conflict_offers_swap_and_swap_exchanges_them() {
    let mut harness = harness();
    keybindings(&mut harness);
    press(&mut harness, "Move up Primary");
    harness.tap_key(KeyCode::ArrowDown);
    super::settle(&mut harness);
    assert_eq!(top(&harness), Some(CONFIRM));
    assert!(harness.shows("Already used by Move down. Swap?"));
    press(&mut harness, "Swap");
    assert_eq!(top(&harness), Some(KEYBINDINGS));
    assert_eq!(bindings(&harness, "move_up")[0], KEY(KeyCode::ArrowDown));
    assert_eq!(bindings(&harness, "move_down")[0], KEY(KeyCode::ArrowUp));
    let down = harness.named("Move down Primary").unwrap();
    assert_eq!(harness.tree().plain_text(down), "Up");
}

#[test]
fn reset_one_and_reset_all_restore_input_json() {
    let mut harness = harness();
    keybindings(&mut harness);
    press(&mut harness, "Move up Primary");
    harness.tap_key(KeyCode::KeyI);
    press(&mut harness, "Pause Primary");
    harness.tap_key(KeyCode::KeyO);
    assert_ne!(bindings(&harness, "move_up"), shipped(&harness, "move_up"));
    press(&mut harness, "Reset Move up");
    assert_eq!(bindings(&harness, "move_up"), shipped(&harness, "move_up"));
    assert_ne!(
        bindings(&harness, "state_pause"),
        shipped(&harness, "state_pause"),
        "one row only"
    );
    press(&mut harness, "Move left Alternate");
    harness.tap_key(KeyCode::KeyJ);
    press(&mut harness, "Reset all");
    for (action, _) in &labelled(&harness) {
        assert_eq!(
            bindings(&harness, action),
            shipped(&harness, action),
            "{action}"
        );
    }
}

#[test]
fn a_capture_cancels_after_five_seconds() {
    let mut harness = harness();
    keybindings(&mut harness);
    let before = bindings(&harness, "state_pause");
    press(&mut harness, "Pause Gamepad");
    assert!(capturing(harness.world()));
    let cell = harness.named("Pause Gamepad").unwrap();
    assert!(
        harness
            .tree()
            .plain_text(cell)
            .starts_with("Press a button…")
    );
    harness.step(4 * 60);
    assert!(capturing(harness.world()), "still waiting at 4 s");
    harness.step(70);
    assert!(!capturing(harness.world()), "cancelled after 5 s");
    assert_eq!(harness.tree().plain_text(cell), "Start");
    assert_eq!(bindings(&harness, "state_pause"), before);
    assert!(
        harness.find("Back").is_some(),
        "the screen takes input again"
    );
}

#[test]
fn the_hud_prompt_follows_a_rebind() {
    let mut harness = harness();
    keybindings(&mut harness);
    press(&mut harness, "Pause Primary");
    harness.tap_key(KeyCode::KeyO);
    start(&mut harness);
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let prompt = harness.tree().plain_text(hud.prompt);
    assert!(prompt.contains("O Pause"), "{prompt:?}");
}
