//! The shell's screen files (`assets/ui/screens/`): each fits opened alone
//! as a data screen, and game over keeps its entrance through a reload.

use tungsten::WindowSize;
use tungsten::core::ScaleFactor;
use tungsten::core::ui::{ScreenFile, UiKey, UiScreenRegistry, Visibility, screen_build};
use tungsten::kit::{Hit, damage};
use tungsten::testing::UiHarness;
use tungsten_template_basic::shell::theme;

use super::{harness, player_entity, settle, start};

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

/// Game over's file with `from` written as `to`, reloaded into the open
/// screen as a save of it would be.
fn reload(harness: &mut UiHarness, from: &str, to: &str) {
    let text = std::fs::read_to_string("assets/ui/screens/game_over.json").unwrap();
    let file = ScreenFile::from_json(&text.replace(from, to)).expect("still a screen");
    let world = harness.world_mut();
    world
        .resource_mut::<UiScreenRegistry>()
        .insert("game_over", file);
    screen_build::reload(world, "game_over");
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
