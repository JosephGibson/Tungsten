//! Each screen at the smallest viewport a player gets, a 1280 × 720 window
//! at the OS scale 1.75 (731 × 411 UI units), and at a 1920 × 1080 window
//! (1280 × 720 units): every shown control, enabled or not, lies inside the
//! viewport; a scroll panel must fit, and its rows scroll inside it. Then
//! a matrix of six windows and scale extremes, box by box.

use tungsten::WindowSize;
use tungsten::core::text::TextNodeStore;
use tungsten::core::ui::Visibility;
use tungsten::core::{ScaleFactor, Settings, UserFolder};
use tungsten::kit::damage;
use tungsten::testing::UiHarness;
use tungsten_template_basic::shell::widgets;

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

/// The matrix: the window, the OS, UI and text scales, and the
/// viewport in UI units they give under the shell's policy (a 720-pixel
/// reference height under a 731 × 411 floor).
const MATRIX: [([u32; 2], [f32; 3], [f32; 2]); 6] = [
    ([1280, 720], [1.75, 1.0, 1.0], [731.4, 411.4]),
    ([1280, 720], [1.75, 2.0, 2.0], [731.0, 411.2]),
    ([1280, 720], [1.0, 0.5, 0.5], [2560.0, 1440.0]),
    ([3840, 2160], [1.0, 1.0, 1.0], [1280.0, 720.0]),
    ([3840, 2160], [1.0, 2.0, 2.0], [731.0, 411.2]),
    ([1920, 1080], [1.0, 0.5, 2.0], [2560.0, 1440.0]),
];

/// Sets the window and the scales, the UI and text scales as a player
/// does, through `Settings`, and runs the frame that takes them.
fn set_scales(harness: &mut UiHarness, window: [u32; 2], [os, ui, text]: [f32; 3]) {
    let world = harness.world_mut();
    world.insert_resource(WindowSize {
        width: window[0],
        height: window[1],
    });
    world.resource_mut::<ScaleFactor>().set_os(os);
    let settings = world.resource_mut::<Settings>();
    settings.set_ui_scale(Some(ui));
    settings.set_text_scale(Some(text));
    tungsten::apply_settings(world);
    harness.step(1);
    harness.layout();
}

/// The fit rules on the shown screens: every root inside the viewport, every
/// visible widget's box inside its parent's, horizontally always and
/// vertically unless a scroll panel above it scrolls it, and the title's
/// label, when shown, one line.
fn assert_rules(harness: &mut UiHarness, what: &str) {
    harness.layout();
    let tree = harness.tree();
    let window = *harness.world().resource::<WindowSize>();
    let scale = harness.world().resource::<ScaleFactor>().get();
    let viewport = [window.width as f32 / scale, window.height as f32 / scale];
    let inside = |inner: f32, outer: f32| inner >= outer - 0.01;
    let mut checked = 0;
    for &root in tree.roots() {
        if tree.visibility(root) != Some(Visibility::Visible) {
            continue;
        }
        let rect = harness.rect(root).expect("a laid-out root");
        assert!(
            inside(rect.min.x, 0.0)
                && inside(rect.min.y, 0.0)
                && inside(viewport[0], rect.max.x)
                && inside(viewport[1], rect.max.y),
            "{what}: root {root:?} at {rect:?} outside {viewport:?}"
        );
        let mut stack = vec![(root, false)];
        while let Some((id, scrolled)) = stack.pop() {
            let parent = harness.rect(id).unwrap();
            let scrolls = scrolled || tree.scroll_max(id).is_some();
            for &child in tree.children(id) {
                if tree.visibility(child) != Some(Visibility::Visible) {
                    continue;
                }
                let rect = harness.rect(child).unwrap();
                let across = inside(rect.min.x, parent.min.x) && inside(parent.max.x, rect.max.x);
                let down = scrolls
                    || (inside(rect.min.y, parent.min.y) && inside(parent.max.y, rect.max.y));
                assert!(
                    across && down,
                    "{what}: {:?} ({:?}) at {rect:?} outside its parent at {parent:?}",
                    tree.accessible_name(child),
                    widgets::text_of(tree, child)
                );
                checked += 1;
                stack.push((child, scrolls));
            }
        }
    }
    assert!(checked > 0, "{what}: nothing shown");
    let title = tree.roots().iter().find_map(|root| {
        let card = *tree.children(*root).first()?;
        let label = *tree.children(card).first()?;
        (widgets::text_of(tree, label) == super::info().title).then_some(label)
    });
    if let Some(title) = title.filter(|_| tree.visibility(tree.roots()[0]).is_some()) {
        let node = tree.text_node(title).unwrap();
        let lines = harness
            .text()
            .committed(node)
            .map(|metrics| metrics.line_count);
        assert_eq!(lines, Some(1), "{what}: the title on one line");
    }
}

#[test]
fn every_screen_fits_at_the_scale_extremes() {
    for (window, scales, expected) in MATRIX {
        let dir = user_folder("fit-extremes");
        let mut harness = harness_with(UserFolder::At(dir.clone()));
        set_scales(&mut harness, window, scales);
        let viewport = {
            let scale = harness.world().resource::<ScaleFactor>().get();
            [window[0] as f32 / scale, window[1] as f32 / scale]
        };
        let entry = format!("{window:?} at {scales:?}");
        assert!(
            (viewport[0] - expected[0]).abs() < 0.5 && (viewport[1] - expected[1]).abs() < 0.5,
            "{entry}: {viewport:?}"
        );
        let check = |harness: &mut UiHarness, what: &str| {
            assert_rules(harness, &format!("{what}, {entry}"));
        };
        check(&mut harness, "title");
        press(&mut harness, "New Game");
        check(&mut harness, "difficulty picker");
        press(&mut harness, "Back");
        press(&mut harness, "Load");
        check(&mut harness, "load");
        tap(&mut harness, "ui_cancel");
        press(&mut harness, "Settings");
        for tab in tungsten_template_basic::shell::settings::TABS {
            press(&mut harness, tab);
            check(&mut harness, tab);
        }
        press(&mut harness, "Controls");
        press(&mut harness, "Keybindings…");
        check(&mut harness, "keybindings");
        tap(&mut harness, "ui_cancel");
        press(&mut harness, "Back");
        start(&mut harness);
        tap(&mut harness, "state_pause");
        check(&mut harness, "pause");
        press(&mut harness, "Save");
        check(&mut harness, "save");
        press(&mut harness, "Slot 1");
        press(&mut harness, "Slot 1");
        check(&mut harness, "a confirm");
        press(&mut harness, "Cancel");
        tap(&mut harness, "ui_cancel");
        press(&mut harness, "Resume");
        let player = player_entity(&harness);
        damage(harness.world_mut(), player, 99.0);
        harness.step(1);
        settle(&mut harness);
        check(&mut harness, "game over");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_keybindings_card_scrolls_only_when_it_must() {
    let mut harness = harness();
    set_scales(&mut harness, [1280, 720], [1.75, 1.0, 1.0]);
    press(&mut harness, "Settings");
    press(&mut harness, "Controls");
    press(&mut harness, "Keybindings…");
    let card = |harness: &UiHarness| {
        let tree = harness.tree();
        let root = tree
            .roots()
            .iter()
            .rev()
            .copied()
            .find(|root| tree.modal(*root));
        *tree.children(root.unwrap()).first().unwrap()
    };
    harness.layout();
    let fits = harness.tree().scroll_max(card(&harness));
    assert_eq!(fits, Some(0.0), "every row shows at 731 × 411");
    set_scales(&mut harness, [1280, 720], [1.75, 1.0, 2.0]);
    let scrolls = harness.tree().scroll_max(card(&harness)).unwrap();
    assert!(scrolls > 0.0, "the card scrolls at text 200 %");
}
