//! Example 05: the UI lab (M46, `D-147`, `D-148`): a HUD, a window and a
//! popup painted through the paint list, with the theme `ex05_default`
//! hot-reloaded from `assets/ui/themes/ex05_default.json`. The popup is a
//! modal the pointer and keys drive (M47, `D-149`).
//!
//! The interactive run floats damage numbers as world labels (M54, `D-161`)
//! and adds a "Settings" window of checkboxes and sliders that switches
//! them, sets their rate and shows a volume readout (M55).
//!
//! M56 makes the interactive run a `play` state owning the lab's roots,
//! with a HUD row (a live timer and a prompt). Dismiss the popup (Escape),
//! then `state_pause` (P, or a pad's Start) pushes the pause menu behind a
//! fade; its Settings pushes the settings screen (display mode, UI and text
//! scale, volume, a `jump` rebind, Back). A rebind persists every action
//! that differs from the shipped layers, Start on `state_pause` among them.
//!
//! Env for the smoke matrix, `tests/ui_regression.rs` and the perf sitting:
//! `TUNGSTEN_UI_LAB_FIXTURE={default|empty|hidden|labels|menus|menu|menu-prewarm|menu-cold}`
//! turns the debug HUD off and builds the lab (`default`), no UI at all
//! (`empty`), the lab with `UiDebug::hide_game_layers` set at startup
//! (`hidden`), the lab with a fixed set of world labels (`labels`), the
//! play state run through pause, settings, a cancelled capture and back by
//! a script within 150 frames (`menus`, smoke's row), the 100-widget menu
//! shown from the first frame (`menu`, the warmed-menu budget's subject),
//! or that menu hidden and shown at frame 240, hidden at 300 and shown
//! again at 360, prewarmed or not (`menu-prewarm`, `menu-cold`, the
//! first-open check); `TUNGSTEN_UI_LAB_POST={off|vignette}` appends a
//! vignette to the post stack. `TUNGSTEN_RENDER_POST_AA` works as
//! everywhere.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fmt::Write as _;
use std::path::PathBuf;

use example_05_ui_lab::menus::{self, FixtureClock, PlayState};
use example_05_ui_lab::screens::{self, THEME_ID, UiLab};
use tungsten::core::post::{PostPass, PostStack, VignetteParams};
use tungsten::core::ui::{UiDebug, UiThemeRegistry, UiTree, WidgetId};
use tungsten::core::{AssetRegistry, Config, World};
use tungsten::{App, DebugHud, StateStack};

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
    /// The lab with a fixed set of world labels.
    Labels,
    /// The play state, scripted through pause, settings and back (M56).
    Menus,
    /// The 100-widget menu, shown from the first frame (M56).
    Menu,
    /// That menu hidden and prewarmed, shown at frame 240 (M56).
    MenuPrewarm,
    /// That menu hidden, not prewarmed, shown at frame 240 (M56).
    MenuCold,
}

fn fixture() -> anyhow::Result<Fixture> {
    let Ok(name) = std::env::var("TUNGSTEN_UI_LAB_FIXTURE") else {
        return Ok(Fixture::Off);
    };
    match name.as_str() {
        "default" => Ok(Fixture::Default),
        "empty" => Ok(Fixture::Empty),
        "hidden" => Ok(Fixture::Hidden),
        "labels" => Ok(Fixture::Labels),
        "menus" => Ok(Fixture::Menus),
        "menu" => Ok(Fixture::Menu),
        "menu-prewarm" => Ok(Fixture::MenuPrewarm),
        "menu-cold" => Ok(Fixture::MenuCold),
        other => anyhow::bail!(
            "invalid TUNGSTEN_UI_LAB_FIXTURE='{other}': expected one of default, empty, hidden, labels, menus, menu, menu-prewarm, menu-cold"
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
    config.window.title = "UI Lab — M56".to_string();
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
        match fixture {
            Fixture::Empty => {}
            Fixture::Menu | Fixture::MenuPrewarm | Fixture::MenuCold => {
                build_menu(world, fixture);
            }
            Fixture::Off | Fixture::Menus => {
                let roots = build_ui(world, fixture == Fixture::Off);
                menus::add_pause_pad_binding(world);
                world
                    .resource_mut::<StateStack>()
                    .request_push(PlayState::new(roots));
            }
            Fixture::Default | Fixture::Hidden | Fixture::Labels => {
                build_ui(world, false);
            }
        }
    });
    app.world_mut().insert_resource(CounterText::default());
    app.add_system_named("ui_lab_counter", counter_system);
    app.add_system_named("ui_lab_popup", screens::popup_system);
    // The other fixtures keep their schedule.
    match fixture {
        Fixture::Labels => app.add_system_named("ui_lab_labels", screens::fixture_labels_system),
        Fixture::Menus => {
            app.world_mut().insert_resource(FixtureClock::default());
            app.add_system_named("ui_lab_menus_script", menus::menus_script_system);
        }
        Fixture::MenuPrewarm | Fixture::MenuCold => {
            app.world_mut().insert_resource(FixtureClock::default());
            app.add_system_named("ui_lab_menu_open", menus::menu_open_system);
        }
        Fixture::Off => {
            app.world_mut()
                .insert_resource(screens::DamageNumbers::default());
            app.add_system_named("ui_lab_controls", screens::controls_system);
            app.add_system_named("ui_lab_damage_numbers", screens::damage_numbers_system);
        }
        _ => {}
    }

    app.run()
}

/// Applies the lab's theme from the registry to `tree`.
fn apply_theme(world: &World, tree: &mut UiTree) {
    let theme = world
        .get_resource::<UiThemeRegistry>()
        .and_then(|registry| registry.get(THEME_ID).cloned());
    match theme {
        Some(theme) => tree.set_theme_asset(THEME_ID, theme),
        None => println!("ui theme '{THEME_ID}' is not loaded; the built-in theme draws"),
    }
}

/// Applies the lab's theme and builds the screens, with the "Settings"
/// window when `controls` is set; returns the roots built.
fn build_ui(world: &mut World, controls: bool) -> Vec<WidgetId> {
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return Vec::new();
    };
    apply_theme(world, &mut tree);
    let lab = {
        let sprites = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        screens::build(&mut tree, sprites)
    };
    let mut roots = vec![lab.hud, lab.window, lab.popup];
    if controls {
        let controls = screens::build_controls(&mut tree);
        roots.push(controls.window);
        world.insert_resource(controls);
    }
    world.insert_resource(tree);
    world.insert_resource(lab);
    roots
}

/// Applies the lab's theme and builds the menu fixtures' menu: shown for
/// `menu`, hidden for the other two, and prewarmed for `menu-prewarm`.
fn build_menu(world: &mut World, fixture: Fixture) {
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return;
    };
    apply_theme(world, &mut tree);
    let menu = menus::build_budget_menu(&mut tree, fixture != Fixture::Menu);
    if fixture == Fixture::MenuPrewarm {
        tree.prewarm(menu.root);
    }
    world.insert_resource(tree);
    world.insert_resource(menu);
}

/// The counter's text, retained so the counter writes without allocating.
#[derive(Default)]
struct CounterText(String);

/// Writes the frame count into the HUD's counter with `write!` into a
/// retained `String`; an unchanged text costs a compare
/// (`UiTree::set_plain_text`, M56).
fn counter_system(world: &mut World) {
    let Some(lab) = world.get_resource_mut::<UiLab>() else {
        return;
    };
    lab.frames += 1;
    let (frames, counter) = (lab.frames, lab.counter);
    let Some(mut text) = world.remove_resource::<CounterText>() else {
        return;
    };
    text.0.clear();
    let _ = write!(text.0, "frame {frames}");
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        tree.set_plain_text(counter, &text.0);
    }
    world.insert_resource(text);
}
