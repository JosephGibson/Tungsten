//! The lab's screens through the headless harness (`D-125`): the paint
//! list's snapshot, with no GPU and no fonts.

use example_05_ui_lab::screens;
use tungsten::App;
use tungsten::core::ui::{UiLayer, UiTree, WidgetKind};
use tungsten::core::{AssetRegistry, Config};
use tungsten::testing::UiHarness;

fn lab() -> UiHarness {
    let mut harness = UiHarness::new(App::new(Config::default()).expect("App::new failed"));
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
