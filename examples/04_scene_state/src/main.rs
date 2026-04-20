//! Example 04 — Scene/State System (M20)
//!
//! Demonstrates the `StateStack` dispatcher driving a
//! `MainMenu -> Gameplay -> Pause -> Gameplay` flow, scene-owned entity
//! auto-despawn via `SceneEntity { state_id }`, and a data-driven
//! `scene.json` loader.
//!
//! Controls:
//!   Enter     — from menu: replace with gameplay, loading scene.json
//!   P         — in gameplay: push pause overlay; in pause: pop back
//!   Backspace — in gameplay: replace with menu
//!   F4        — toggle the debug HUD (the `state` row mirrors the top state id)
//!   Esc       — exit

mod states;

use tungsten::core::{Config, ResolvedManifest};
use tungsten::{asset_loader, App, DebugHud, StateStack};

use crate::states::MainMenuState;

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/04_scene_state/assets/manifest.json";

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Scene / State System".to_string();

    let mut app = App::new(config)?;

    app.on_startup(|world, renderer| {
        let root = ResolvedManifest::load(ROOT_MANIFEST).expect("Failed to load root manifest");
        asset_loader::load_fonts(&root, world, renderer).expect("Failed to load shared fonts");

        let local = ResolvedManifest::load(LOCAL_MANIFEST).expect("Failed to load local manifest");
        asset_loader::load_sprites(&local, world, renderer)
            .expect("Failed to load example 04 sprites");

        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = true;
        }

        world
            .get_resource_mut::<StateStack>()
            .expect("StateStack resource missing")
            .request_push(MainMenuState);
    });

    app.run()
}
