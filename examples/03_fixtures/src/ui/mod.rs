//! The `ui` scene: fixture code for the UI overlay's paint path, world
//! labels, the gameplay screens, the debug views and the prewarm budget
//! menu. The template's shell has replaced it as the UI reference
//! (`D-193`).
//!
//! - [`theme`]: every look and timing value in one place, and the theme
//!   asset's apply (`D-148`).
//! - [`widgets`]: small builders over `UiTree` that the screens share.
//! - [`hud`]: pointer-transparent HUD rows written without allocating.
//! - [`labels`]: world-anchored text (`D-161`): a fixed set and damage
//!   numbers.
//! - [`controls`]: toggles and sliders read through `UiEvent::ValueChanged`
//!   (`D-162`).
//! - [`menus`]: state-owned screens with fades, prompts, rebinding and the
//!   settings apply (`D-163`, `D-164`, `D-159`).
//! - [`inventory`]: a slot grid, a detail pane and a drop confirmation
//!   driven by every device, with a modal screen's hotkeys.
//! - [`budget`]: the 100-widget menu the prewarm budget measures.
//! - [`screens`]: the paint list's batch partition, the inventory under its
//!   modal confirmation (`D-147`, `D-149`).
//! - [`fixtures`]: driving the UI from scripts for smoke, pixel tests and
//!   perf.
//!
//! [`run`] wires them into an `App`; the modules let the headless tests
//! build the same trees.
//!
//! [`run`] picks the manifests, the theme, the states and the systems by
//! `TUNGSTEN_UI_LAB_FIXTURE` ([`fixtures`]). With no fixture the scene runs
//! interactively: the play state over the lab HUD and the controls window,
//! with damage numbers and the debug HUD. `inventory_open` (I, or a pad's
//! North) pushes the inventory, and `state_pause` (P, or a pad's Start) the
//! pause menu, each behind a fade; the pause menu's Settings pushes the
//! settings screen (display mode, UI and text scale, the Volume, Music and
//! Effects sliders, a `jump` rebind, Back; an Effects change plays a blip)
//! and its Inventory the inventory. The play state's music fades in over a
//! second, and `audio_toggle_music` (M) crossfades it into a new voice over
//! two seconds.
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

pub mod budget;
pub mod controls;
pub mod fixtures;
pub mod hud;
pub mod inventory;
pub mod labels;
pub mod menus;
pub mod screens;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;

use tungsten::core::post::{PostPass, PostStack, VignetteParams};
use tungsten::core::ui::{UiDebug, UiTree};
use tungsten::core::{AssetRegistry, Config, World};
use tungsten::{App, DebugHud, StateStack};

use self::fixtures::{Fixture, FixtureClock, Post};
use self::labels::DamageNumbers;
use self::menus::PlayState;

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_fixtures/assets/manifest.json";

/// Runs the `ui` scene with the fixture `TUNGSTEN_UI_LAB_FIXTURE` names.
pub fn run() -> anyhow::Result<()> {
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
                menus::add_music_binding(world);
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
