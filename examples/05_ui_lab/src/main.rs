//! Example 05: the UI lab (M46, `D-147`, `D-148`): a HUD, a window and a
//! popup painted through the paint list, with the theme `ex05_default`
//! hot-reloaded from `assets/ui/themes/ex05_default.json`.
//!
//! Env for the smoke matrix and `tests/ui_regression.rs`:
//! `TUNGSTEN_UI_LAB_FIXTURE={default|empty|hidden}` turns the debug HUD off
//! and builds the lab (`default`), no UI at all (`empty`) or the lab with
//! `UiDebug::hide_game_layers` set at startup (`hidden`);
//! `TUNGSTEN_UI_LAB_POST={off|vignette}` appends a vignette to the post
//! stack. `TUNGSTEN_RENDER_POST_AA` works as everywhere.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use example_05_ui_lab::screens::{self, THEME_ID, UiLab};
use tungsten::core::post::{PostPass, PostStack, VignetteParams};
use tungsten::core::ui::{UiDebug, UiThemeRegistry, UiTree};
use tungsten::core::{AssetRegistry, Config, World};
use tungsten::{App, DebugHud};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/05_ui_lab/assets/manifest.json";

/// `TUNGSTEN_UI_LAB_FIXTURE`, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fixture {
    /// Not set: interactive run with the debug HUD.
    Off,
    /// The lab, no debug HUD.
    Default,
    /// No roots at all.
    Empty,
    /// The lab with the game layers hidden at startup.
    Hidden,
}

fn fixture() -> anyhow::Result<Fixture> {
    let Ok(name) = std::env::var("TUNGSTEN_UI_LAB_FIXTURE") else {
        return Ok(Fixture::Off);
    };
    match name.as_str() {
        "default" => Ok(Fixture::Default),
        "empty" => Ok(Fixture::Empty),
        "hidden" => Ok(Fixture::Hidden),
        other => anyhow::bail!(
            "invalid TUNGSTEN_UI_LAB_FIXTURE='{other}': expected one of default, empty, hidden"
        ),
    }
}

/// `TUNGSTEN_UI_LAB_POST`, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Post {
    Off,
    Vignette,
}

fn post() -> anyhow::Result<Post> {
    match std::env::var("TUNGSTEN_UI_LAB_POST").as_deref() {
        Err(_) | Ok("off") => Ok(Post::Off),
        Ok("vignette") => Ok(Post::Vignette),
        Ok(other) => {
            anyhow::bail!("invalid TUNGSTEN_UI_LAB_POST='{other}': expected one of off, vignette")
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "UI Lab — M46".to_string();
    let fixture = fixture()?;
    let post = post()?;

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);

    app.on_startup(move |world, _renderer| {
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = fixture == Fixture::Off;
        }
        if post == Post::Vignette
            && let Some(stack) = world.get_resource_mut::<PostStack>()
        {
            stack.0.push(PostPass::Vignette(VignetteParams::default()));
        }
        if fixture == Fixture::Hidden
            && let Some(debug) = world.get_resource_mut::<UiDebug>()
        {
            debug.hide_game_layers = true;
        }
        if fixture != Fixture::Empty {
            build_ui(world);
        }
    });
    app.add_system_named("ui_lab_counter", counter_system);

    app.run()
}

/// Applies the lab's theme from the registry and builds the screens.
fn build_ui(world: &mut World) {
    let theme = world
        .get_resource::<UiThemeRegistry>()
        .and_then(|registry| registry.get(THEME_ID).cloned());
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return;
    };
    match theme {
        Some(theme) => tree.set_theme_asset(THEME_ID, theme),
        None => println!("ui theme '{THEME_ID}' is not loaded; the built-in theme draws"),
    }
    let lab = {
        let sprites = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        screens::build(&mut tree, sprites)
    };
    world.insert_resource(tree);
    world.insert_resource(lab);
}

/// Writes the frame count into the HUD's counter; an unchanged text costs
/// a compare (`UiTree::set_text`).
fn counter_system(world: &mut World) {
    let Some(lab) = world.get_resource_mut::<UiLab>() else {
        return;
    };
    lab.frames += 1;
    let (frames, counter) = (lab.frames, lab.counter);
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        tree.set_text(counter, format!("frame {frames}"));
    }
}
