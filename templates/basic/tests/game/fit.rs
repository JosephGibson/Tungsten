//! Each screen at the smallest viewport a player gets, a 1280 × 720 window
//! at the OS scale 1.75 (731 × 411 UI units), and at 1920 × 1080: every
//! shown control, enabled or not, lies inside the viewport; a scroll panel
//! must fit, and its rows scroll inside it.

use tungsten::WindowSize;
use tungsten::core::UserFolder;
use tungsten::core::ui::Visibility;
use tungsten::kit::damage;
use tungsten::testing::UiHarness;

use super::{harness, harness_with, player_entity, press, settle, start, tap, user_folder};

const SIZES: [[u32; 2]; 2] = [[731, 411], [1920, 1080]];

/// Lays the tree out at `size` and checks every focusable widget's box.
fn assert_fits(harness: &mut UiHarness, size: [u32; 2], what: &str) {
    harness.world_mut().insert_resource(WindowSize {
        width: size[0],
        height: size[1],
    });
    harness.layout();
    let tree = harness.tree();
    let [width, height] = size.map(|side| side as f32);
    let mut checked = 0;
    let mut stack: Vec<_> = tree.roots().to_vec();
    while let Some(id) = stack.pop() {
        // A hidden or collapsed widget takes its subtree off the screen.
        if tree.visibility(id) != Some(Visibility::Visible) {
            continue;
        }
        // A scroll panel clips its rows: the panel must fit, they scroll.
        let scrolls = tree.scroll_max(id).is_some();
        if !scrolls {
            stack.extend(tree.children(id).iter().copied());
        }
        if !tree.focusable(id) && !scrolls {
            continue;
        }
        let rect = harness.rect(id).expect("a focusable widget has a box");
        assert!(
            rect.min.x >= 0.0 && rect.min.y >= 0.0 && rect.max.x <= width && rect.max.y <= height,
            "{what} at {size:?}: {:?} at {rect:?}",
            tree.accessible_name(id)
        );
        checked += 1;
    }
    assert!(checked > 0, "{what}: nothing focusable");
}

fn each_size(harness: &mut UiHarness, what: &str) {
    for size in SIZES {
        assert_fits(harness, size, what);
    }
}

#[test]
fn the_title_and_its_picker_fit() {
    let mut harness = harness();
    each_size(&mut harness, "title");
    press(&mut harness, "New Game");
    each_size(&mut harness, "difficulty picker");
}

#[test]
fn pause_fits() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    each_size(&mut harness, "pause");
}

#[test]
fn game_over_fits() {
    let mut harness = harness();
    start(&mut harness);
    let player = player_entity(&harness);
    damage(harness.world_mut(), player, 99.0);
    harness.step(1);
    settle(&mut harness);
    each_size(&mut harness, "game over");
}

#[test]
fn the_saves_screen_fits_in_both_modes() {
    let dir = user_folder("fit-saves");
    let mut harness = harness_with(UserFolder::At(dir.clone()));
    press(&mut harness, "Load");
    each_size(&mut harness, "load");
    tap(&mut harness, "ui_cancel");
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Save");
    each_size(&mut harness, "save");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_settings_tab_fits() {
    let mut harness = harness();
    start(&mut harness);
    tap(&mut harness, "state_pause");
    press(&mut harness, "Settings");
    for tab in tungsten_template_basic::shell::settings::TABS {
        press(&mut harness, tab);
        each_size(&mut harness, tab);
    }
}

#[test]
fn the_keybindings_screen_fits_and_its_list_scrolls_at_411_units() {
    let mut harness = harness();
    press(&mut harness, "Settings");
    press(&mut harness, "Controls");
    press(&mut harness, "Keybindings…");
    each_size(&mut harness, "keybindings");
    assert_fits(&mut harness, SIZES[0], "keybindings");
    let tree = harness.tree();
    let list = tree.roots().iter().rev().find_map(|root| {
        tree.children(*root)
            .iter()
            .copied()
            .find(|id| tree.scroll_max(*id).is_some())
    });
    let list = list.expect("the bindings list scrolls");
    assert!(
        tree.scroll_max(list).unwrap() > 0.0,
        "its rows overflow at 411 units"
    );
}
