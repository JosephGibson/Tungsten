use std::path::Path;

use tungsten_core::Config;

use super::{ENGINE_FONT, ENGINE_FONT_ID};
use crate::testing::Harness;
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
fn hud_systems_overlay_and_inspector_draw_with_the_engine_font() {
    for overlay in ["HUD", "systems overlay", "inspector"] {
        let mut app = App::new(Config::default()).expect("App::new failed");
        // The systems overlay lists the previous frame's system timings.
        app.add_system(|_| {});
        let mut harness = Harness::new(app);
        let world = harness.world_mut();
        match overlay {
            "HUD" => world.get_resource_mut::<DebugHud>().unwrap().enabled = true,
            "systems overlay" => {
                world
                    .get_resource_mut::<SystemTimingOverlay>()
                    .unwrap()
                    .enabled = true;
            }
            _ => world.get_resource_mut::<InspectorState>().unwrap().enabled = true,
        }
        harness.step(2);
        let text = &harness.draw().expect("a completed frame").text;
        assert!(!text.is_empty(), "{overlay}: no section");
        for section in text {
            assert_eq!(
                section.font_id, ENGINE_FONT_ID,
                "{overlay}: {}",
                section.content
            );
        }
    }
}
