//! The shell's screen files (`assets/ui/screens/`): each fits opened alone
//! as a data screen, and game over keeps its entrance through a reload.

use tungsten::WindowSize;
use tungsten::core::ScaleFactor;
use tungsten::core::ui::{ScreenFile, UiKey, UiScreenRegistry, Visibility, screen_build};
use tungsten::kit::{Hit, damage};
use tungsten::testing::UiHarness;
use tungsten_template_basic::shell::theme;

use tungsten_template_basic::shell::settings::{SETTINGS, SettingsView, TABS};

use super::{harness, player_entity, press, settle, start, tap};

/// Each file, opened alone over the title as a data screen, fits at the
/// smallest window a player gets and at a 1920 × 1080 one, at the UI
/// scale 1 and the shell's 75 %.
#[test]
fn every_screen_file_fits() {
    let ids: Vec<String> = harness()
        .world()
        .resource::<UiScreenRegistry>()
        .ids()
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert!(!ids.is_empty());
    for id in ids {
        let mut harness = harness();
        assert!(tungsten::screens::open(harness.world_mut(), &id), "{id}");
        harness.step(12);
        for [width, height] in [[731, 411], [1920, 1080]] {
            harness
                .world_mut()
                .insert_resource(WindowSize { width, height });
            for ui in [1.0, theme::UI_SCALE] {
                let world = harness.world_mut();
                world.resource_mut::<ScaleFactor>().set_ui_scale(ui);
                harness.assert_fits(&format!("{id} at {width}x{height}, UI scale {ui}"));
            }
        }
    }
}

/// Screen file `id` with `from` written as `to`, reloaded into its open
/// screens as a save of it would be.
fn reload_file(harness: &mut UiHarness, id: &str, from: &str, to: &str) {
    let text = std::fs::read_to_string(format!("assets/ui/screens/{id}.json")).unwrap();
    let file = ScreenFile::from_json(&text.replace(from, to)).expect("still a screen");
    let world = harness.world_mut();
    world.resource_mut::<UiScreenRegistry>().insert(id, file);
    screen_build::reload(world, id);
}

fn reload(harness: &mut UiHarness, from: &str, to: &str) {
    reload_file(harness, "game_over", from, to);
}

/// The visibility of game over's card, shown or hidden.
fn card(harness: &UiHarness) -> Option<Visibility> {
    let tree = harness.tree();
    tree.visibility(tree.find_key(harness.front_modal()?, UiKey::new("card"))?)
}

#[test]
fn a_game_over_reload_during_its_entrance_still_reveals_its_buttons() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    assert_eq!(damage(harness.world_mut(), player, 99.0), Hit::Killed);
    harness.step(30);
    assert_eq!(
        card(&harness),
        Some(Visibility::Hidden),
        "the entrance runs"
    );
    reload(&mut harness, "Retry", "Try Again");
    harness.step(1);
    assert_eq!(
        card(&harness),
        Some(Visibility::Hidden),
        "the new card waits"
    );
    settle(&mut harness);
    assert_eq!(card(&harness), Some(Visibility::Visible));
    let again = harness.find("Try Again").expect("the reloaded button");
    assert_eq!(harness.tree().focused(), Some(again));

    // After the entrance a reload keeps the buttons shown.
    reload(&mut harness, "Retry", "Retry once more");
    harness.step(1);
    assert_eq!(card(&harness), Some(Visibility::Visible));
    assert!(harness.find("Retry once more").is_some());
}

/// The settings screen's tab buttons, in the rail's order, and their bars'
/// visibility.
fn tabs(harness: &UiHarness) -> Vec<(String, Option<Visibility>)> {
    let tree = harness.tree();
    let first = harness.find_key(UiKey::new("display")).expect("the tabs");
    let rail = tree.parent(first).unwrap();
    let bar =
        |tab: &tungsten::core::ui::WidgetId| tree.visibility(*tree.children(*tab).last().unwrap());
    tree.children(rail)
        .iter()
        .map(|tab| (tree.plain_text(*tab), bar(tab)))
        .collect()
}

#[test]
fn the_settings_files_tabs_are_the_shells_tabs() {
    let mut harness = harness();
    press(&mut harness, "Settings");
    let captions: Vec<String> = tabs(&harness)
        .into_iter()
        .map(|(caption, _)| caption)
        .collect();
    assert_eq!(captions, TABS);
}

#[test]
fn a_settings_reload_keeps_its_tab_and_focus() {
    let mut harness = harness();
    press(&mut harness, "Settings");
    tap(&mut harness, "ui_next");
    let master = harness
        .find_key(UiKey::new("master"))
        .expect("the Audio tab");
    assert!(harness.focus(master));
    reload_file(&mut harness, "settings", "\"Master\"", "\"Main\"");
    harness.step(1);
    let reloaded = harness
        .find_key(UiKey::new("master"))
        .expect("the Audio tab still");
    assert_ne!(reloaded, master, "rebuilt");
    assert_eq!(harness.tree().focused(), Some(reloaded), "focus by its key");
    assert_eq!(harness.tree().accessible_name(reloaded), Some("Main"));
    assert!(
        harness.find_key(UiKey::new("window_mode")).is_none(),
        "Display hides"
    );
    let bars: Vec<_> = tabs(&harness).into_iter().map(|(_, bar)| bar).collect();
    assert_eq!(bars[1], Some(Visibility::Visible), "the Audio bar shows");
}

#[test]
fn a_settings_file_opens_alone_on_its_first_tab() {
    let mut harness = harness();
    assert!(tungsten::screens::open(harness.world_mut(), "settings"));
    harness.step(12);
    assert!(
        !harness.world().has_resource::<SettingsView>(),
        "no SettingsState entered"
    );
    let stack = harness.world().resource::<tungsten::StateStack>();
    assert_eq!(
        stack.active_id(),
        Some(SETTINGS),
        "a data screen, by the file's ID"
    );
    assert!(harness.find_key(UiKey::new("window_mode")).is_some());
    for hidden in ["master", "ui_scale", "keybindings", "difficulty"] {
        assert!(harness.find_key(UiKey::new(hidden)).is_none(), "{hidden}");
    }
    let bars: Vec<_> = tabs(&harness).into_iter().map(|(_, bar)| bar).collect();
    let (shown, hidden) = (Some(Visibility::Visible), Some(Visibility::Hidden));
    assert_eq!(bars, [shown, hidden, hidden, hidden, hidden]);
}
