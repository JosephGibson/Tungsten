//! Keybindings: the listed actions, a capture, the device check, a swap,
//! the resets, the capture's timeout and the HUD's prompt after a rebind.

use std::path::Path;

use tungsten::core::{
    ActionMap, Binding, GamepadButton, GamepadId, InputState, KeyCode, UserFolder,
};
use tungsten::testing::UiHarness;
use tungsten_template_basic::game::BINDABLE_ACTIONS;
use tungsten_template_basic::sample::hud::Hud;
use tungsten_template_basic::shell::confirm::CONFIRM;
use tungsten_template_basic::shell::keybindings::{COLUMNS, KEYBINDINGS};
use tungsten_template_basic::shell::toast::Toasts;
use tungsten_template_basic::shell::{capturing, widgets};

use super::{find, harness, harness_with, named, press, shows, start, top, user_folder};

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

/// Presses `key` for a frame and releases it the next.
fn key(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

/// The shipped bindings for `action`: `input.json` over the defaults.
fn shipped(action: &str) -> Vec<Binding> {
    let map = ActionMap::load(Path::new("input.json")).unwrap();
    ActionMap::merged_with_defaults(map)
        .bindings(action)
        .to_vec()
}

#[test]
fn every_bindable_action_is_listed_and_no_ui_or_engine_action() {
    let mut harness = harness();
    keybindings(&mut harness);
    for (action, label) in BINDABLE_ACTIONS {
        assert!(!action.starts_with("ui_") && !action.starts_with("engine_"));
        for column in COLUMNS {
            assert!(
                named(&harness, &format!("{label} {column}")).is_some(),
                "{label} {column}"
            );
        }
    }
    let labels: Vec<&str> = BINDABLE_ACTIONS.iter().map(|(_, label)| *label).collect();
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
    let up = named(&harness, "Move up Primary").unwrap();
    assert_eq!(widgets::text_of(harness.tree(), up), "Up");
    let pad = named(&harness, "Move up Gamepad").unwrap();
    assert_eq!(widgets::text_of(harness.tree(), pad), "Left Stick Up");
}

#[test]
fn rebinding_primary_captures_a_key_and_survives_a_restart() {
    let dir = user_folder("rebind");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    keybindings(&mut harness);
    press(&mut harness, "Move up Primary");
    assert!(capturing(harness.world()));
    let cell = named(&harness, "Move up Primary").unwrap();
    assert!(widgets::text_of(harness.tree(), cell).starts_with("Press a key…"));
    key(&mut harness, KeyCode::KeyI);
    assert!(!capturing(harness.world()));
    assert_eq!(bindings(&harness, "move_up")[0], KEY(KeyCode::KeyI));
    assert!(
        bindings(&harness, "move_up").contains(&KEY(KeyCode::KeyW)),
        "the rest kept"
    );
    assert_eq!(widgets::text_of(harness.tree(), cell), "I");

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
    key(&mut harness, KeyCode::ArrowDown);
    super::settle(&mut harness);
    assert_eq!(top(&harness), Some(CONFIRM));
    assert!(shows(&harness, "Already used by Move down. Swap?"));
    press(&mut harness, "Swap");
    assert_eq!(top(&harness), Some(KEYBINDINGS));
    assert_eq!(bindings(&harness, "move_up")[0], KEY(KeyCode::ArrowDown));
    assert_eq!(bindings(&harness, "move_down")[0], KEY(KeyCode::ArrowUp));
    let down = named(&harness, "Move down Primary").unwrap();
    assert_eq!(widgets::text_of(harness.tree(), down), "Up");
}

#[test]
fn reset_one_and_reset_all_restore_input_json() {
    let mut harness = harness();
    keybindings(&mut harness);
    press(&mut harness, "Move up Primary");
    key(&mut harness, KeyCode::KeyI);
    press(&mut harness, "Pause Primary");
    key(&mut harness, KeyCode::KeyO);
    assert_ne!(bindings(&harness, "move_up"), shipped("move_up"));
    press(&mut harness, "Reset Move up");
    assert_eq!(bindings(&harness, "move_up"), shipped("move_up"));
    assert_ne!(
        bindings(&harness, "state_pause"),
        shipped("state_pause"),
        "one row only"
    );
    press(&mut harness, "Move left Alternate");
    key(&mut harness, KeyCode::KeyJ);
    press(&mut harness, "Reset all");
    for (action, _) in BINDABLE_ACTIONS {
        assert_eq!(bindings(&harness, action), shipped(action), "{action}");
    }
}

#[test]
fn a_capture_cancels_after_five_seconds() {
    let mut harness = harness();
    keybindings(&mut harness);
    let before = bindings(&harness, "state_pause");
    press(&mut harness, "Pause Gamepad");
    assert!(capturing(harness.world()));
    let cell = named(&harness, "Pause Gamepad").unwrap();
    assert!(widgets::text_of(harness.tree(), cell).starts_with("Press a button…"));
    harness.step(4 * 60);
    assert!(capturing(harness.world()), "still waiting at 4 s");
    harness.step(70);
    assert!(!capturing(harness.world()), "cancelled after 5 s");
    assert_eq!(widgets::text_of(harness.tree(), cell), "Start");
    assert_eq!(bindings(&harness, "state_pause"), before);
    assert!(
        find(&harness, "Back").is_some(),
        "the screen takes input again"
    );
}

#[test]
fn the_hud_prompt_follows_a_rebind() {
    let mut harness = harness();
    keybindings(&mut harness);
    press(&mut harness, "Pause Primary");
    key(&mut harness, KeyCode::KeyO);
    start(&mut harness);
    harness.step(1);
    let hud = *harness.world().resource::<Hud>();
    let prompt = widgets::text_of(harness.tree(), hud.prompt);
    assert!(prompt.contains("O Pause"), "{prompt:?}");
}
