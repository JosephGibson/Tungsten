//! Wiring a UI game: the manifests, the theme, the states and the systems,
//! chosen by `TUNGSTEN_UI_LAB_FIXTURE` ([`fixtures`]). With no fixture the
//! lab runs interactively: the play state over the lab HUD and the
//! controls window, with damage numbers and the debug HUD. `inventory_open`
//! (I, or a pad's North) pushes the inventory, and `state_pause` (P, or a
//! pad's Start) the pause menu, each behind a fade; the pause menu's
//! Settings pushes the settings screen (display mode, UI and text scale,
//! volume, a `jump` rebind, Back) and its Inventory the inventory.
//!
//! Env: `TUNGSTEN_UI_LAB_FIXTURE={default|empty|hidden|labels|menus|menu|menu-prewarm|menu-cold|debug|display}`
//! picks a fixture ([`fixtures`] lists them): `default` builds the lab
//! without the debug HUD, `empty` no UI at all, `hidden` the lab with
//! `UiDebug::hide_game_layers` set at startup, `labels` the lab under a
//! fixed set of world labels, `menus` the play state scripted through its
//! screens within 150 frames (smoke's row), `menu` the 100-widget menu from
//! the first frame, `menu-prewarm` and `menu-cold` that menu shown at frame
//! 240, hidden at 300 and shown at 360, prewarmed or not, `debug` the lab
//! under every debug view (smoke's row), and `display` the settings screen
//! through a resize to 1600×900, a volume step and the display mode cycled
//! back to windowed, printing the engine's view of the window at each
//! change (not a smoke row, since it flips the screen;
//! `TUNGSTEN_UI_LAB_MAXIMIZE_WAIT=1` waits up to 600 frames for an outside
//! maximize before the volume step). `TUNGSTEN_UI_LAB_POST={off|vignette}`
//! appends a vignette to the post stack; `TUNGSTEN_UI_LAB_BOUNDS=1` shows
//! the layout bounds from the first frame (F5 toggles them);
//! `TUNGSTEN_RENDER_POST_AA` works as everywhere.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use example_05_ui_lab::fixtures::{self, Fixture, FixtureClock, Post};
use example_05_ui_lab::labels::{self, DamageNumbers};
use example_05_ui_lab::menus::{self, PlayState};
use example_05_ui_lab::{budget, controls, hud, inventory, screens, theme};
use tungsten::core::post::{PostPass, PostStack, VignetteParams};
use tungsten::core::ui::{UiDebug, UiTree};
use tungsten::core::{AssetRegistry, Config, World};
use tungsten::{App, DebugHud, StateStack};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/05_ui_lab/assets/manifest.json";

fn main() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "UI Lab — M56".to_string();
    let fixture = Fixture::from_env()?;
    let post = Post::from_env()?;
    let bounds = fixtures::bounds_from_env()?;

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);

    app.on_startup(move |world, _renderer| {
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = matches!(fixture, Fixture::Off | Fixture::Debug);
        }
        if post == Post::Vignette
            && let Some(stack) = world.get_resource_mut::<PostStack>()
        {
            stack.0.push(PostPass::Vignette(VignetteParams::default()));
        }
        if let Some(debug) = world.get_resource_mut::<UiDebug>() {
            debug.hide_game_layers |= fixture == Fixture::Hidden;
            debug.show_bounds |= bounds;
        }
        match fixture {
            Fixture::Empty => {}
            Fixture::Menu | Fixture::MenuPrewarm | Fixture::MenuCold => {
                build_menu(world, fixture);
            }
            Fixture::Off | Fixture::Menus | Fixture::Display => {
                let roots = menus::build_play_screen(world, fixture == Fixture::Off);
                menus::add_pause_pad_binding(world);
                inventory::add_bindings(world);
                world
                    .resource_mut::<StateStack>()
                    .request_push(PlayState::new(roots));
            }
            Fixture::Default | Fixture::Hidden | Fixture::Labels => {
                build_paint_path(world);
            }
            Fixture::Debug => {
                build_paint_path(world);
                fixtures::open_debug_views(world);
            }
        }
    });
    app.world_mut().insert_resource(hud::CounterText::default());
    app.add_system_named("ui_lab_counter", hud::counter_system);
    if matches!(
        fixture,
        Fixture::Default | Fixture::Hidden | Fixture::Labels | Fixture::Debug
    ) {
        app.add_system_named("ui_lab_inventory", inventory::inventory_system);
    }
    // The other fixtures keep their schedule.
    match fixture {
        Fixture::Labels => app.add_system_named("ui_lab_labels", labels::fixture_labels_system),
        Fixture::Menus => {
            app.world_mut().insert_resource(FixtureClock::default());
            app.add_system_named("ui_lab_menus_script", fixtures::menus_script_system);
        }
        Fixture::MenuPrewarm | Fixture::MenuCold => {
            app.world_mut().insert_resource(FixtureClock::default());
            app.add_system_named("ui_lab_menu_open", fixtures::menu_open_system);
        }
        Fixture::Debug => app.add_system_named("ui_lab_debug", fixtures::debug_fixture_system),
        Fixture::Display => {
            let maximize_wait =
                std::env::var("TUNGSTEN_UI_LAB_MAXIMIZE_WAIT").as_deref() == Ok("1");
            app.world_mut()
                .insert_resource(fixtures::DisplayScript::new(maximize_wait));
            app.add_system_named("ui_lab_display_script", fixtures::display_script_system);
        }
        Fixture::Off => {
            app.world_mut().insert_resource(DamageNumbers::default());
            app.add_system_named("ui_lab_controls", controls::controls_system);
            app.add_system_named("ui_lab_damage_numbers", labels::damage_numbers_system);
        }
        _ => {}
    }

    app.run()
}

/// Applies the lab's theme and builds the paint-path screen: the lab HUD
/// and the inventory with its confirmation open.
fn build_paint_path(world: &mut World) {
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return;
    };
    theme::apply(world, &mut tree);
    let (lab_hud, inventory) = {
        let sprites = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        screens::build(&mut tree, sprites)
    };
    world.insert_resource(tree);
    world.insert_resource(lab_hud);
    world.insert_resource(inventory);
    inventory::add_bindings(world);
}

/// Applies the lab's theme and builds the budget menu: shown for `menu`,
/// hidden for the other two, and prewarmed for `menu-prewarm`.
fn build_menu(world: &mut World, fixture: Fixture) {
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return;
    };
    theme::apply(world, &mut tree);
    let menu = budget::build_budget_menu(&mut tree, fixture != Fixture::Menu);
    if fixture == Fixture::MenuPrewarm {
        tree.prewarm(menu.root);
    }
    world.insert_resource(tree);
    world.insert_resource(menu);
}
