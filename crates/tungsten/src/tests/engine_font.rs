use std::collections::HashMap;
use std::path::Path;

use tungsten_core::Config;
use tungsten_core::assets::ResolvedFontFamily;
use tungsten_core::ui::{WidgetId, WidgetKind};

use super::{
    ENGINE_FONT, ENGINE_FONT_ID, ENGINE_FONT_SEMIBOLD, ENGINE_FONT_SEMIBOLD_ID, with_engine_family,
};
use crate::testing::UiHarness;
use crate::{App, DebugHud, InspectorState, SystemTimingOverlay};

fn root_font_file(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/fonts/JetBrainsMono")
        .join(relative);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn engine_font_is_the_root_jetbrains_mono_regular_with_its_license() {
    assert_eq!(
        ENGINE_FONT,
        root_font_file("static/JetBrainsMono-Regular.ttf").as_slice()
    );
    let license = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/OFL.txt");
    assert_eq!(
        std::fs::read(license).expect("read the engine font's OFL.txt"),
        root_font_file("LICENSE.txt")
    );
}

#[test]
fn the_semibold_face_is_the_root_jetbrains_mono_semibold() {
    assert_eq!(
        ENGINE_FONT_SEMIBOLD,
        root_font_file("static/JetBrainsMono-SemiBold.ttf").as_slice()
    );
    assert_eq!(ENGINE_FONT_SEMIBOLD_ID, "engine_mono_semibold");
}

#[test]
fn the_engine_family_joins_the_manifests_families_unless_they_declare_it() {
    let mut families = HashMap::new();
    families.insert(
        "sans".to_string(),
        ResolvedFontFamily::new(vec!["sans".to_string()]),
    );
    let all = with_engine_family(&families);
    assert_eq!(all.len(), 2);
    assert_eq!(
        all[ENGINE_FONT_ID].faces,
        [ENGINE_FONT_ID, ENGINE_FONT_SEMIBOLD_ID]
    );
    assert_eq!(all["sans"].faces, ["sans"]);
    assert_eq!(
        with_engine_family(&HashMap::new())[ENGINE_FONT_ID].faces,
        [ENGINE_FONT_ID, ENGINE_FONT_SEMIBOLD_ID],
        "with no manifest at all"
    );
    families.insert(
        ENGINE_FONT_ID.to_string(),
        ResolvedFontFamily::new(vec!["house_mono".to_string()]),
    );
    assert_eq!(
        with_engine_family(&families)[ENGINE_FONT_ID].faces,
        ["house_mono"],
        "a manifest's own engine_mono family wins"
    );
}

#[test]
fn hud_systems_overlay_and_inspector_labels_name_the_engine_family() {
    let mut app = App::new(Config::default()).expect("App::new failed");
    // The systems overlay lists the previous frame's system timings.
    app.add_system(|_| {});
    let mut harness = UiHarness::new(app);
    {
        let world = harness.world_mut();
        world.resource_mut::<DebugHud>().enabled = true;
        world.resource_mut::<SystemTimingOverlay>().enabled = true;
        world.resource_mut::<InspectorState>().enabled = true;
    }
    harness.step(3);
    let tree = harness.tree();
    let mut labels = 0;
    let mut stack: Vec<WidgetId> = tree.roots().to_vec();
    while let Some(id) = stack.pop() {
        stack.extend_from_slice(tree.children(id));
        if tree.kind(id) == Some(WidgetKind::Label) {
            labels += 1;
            assert_eq!(
                tree.resolved_style(id).unwrap().text.family,
                ENGINE_FONT_ID,
                "{:?}",
                tree.text(id)
            );
        }
    }
    assert!(labels > 10, "{labels} labels");
    assert!(
        harness.harness().draw().unwrap().text.is_empty(),
        "no text section"
    );
}
