//! The UI's debug actions (M46, w01 §14): `engine_toggle_ui_bounds` flips
//! the layout-bounds overlay and `engine_toggle_ui` hides the game layers,
//! both on the [`UiDebug`] resource the paint reads.

use tungsten_core::ui::UiDebug;
use tungsten_core::{ActionMap, InputState, World};

/// Flips [`UiDebug::show_bounds`] on a press of `engine_toggle_ui_bounds`
/// and [`UiDebug::hide_game_layers`] on one of `engine_toggle_ui`; a held
/// key flips once. Does nothing without the input, action map or `UiDebug`
/// resources.
pub fn ui_debug_toggle_system(world: &mut World) {
    let (bounds, hide) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        (
            actions.just_pressed(input, "engine_toggle_ui_bounds"),
            actions.just_pressed(input, "engine_toggle_ui"),
        )
    };
    if !bounds && !hide {
        return;
    }
    if let Some(debug) = world.get_resource_mut::<UiDebug>() {
        if bounds {
            debug.show_bounds = !debug.show_bounds;
        }
        if hide {
            debug.hide_game_layers = !debug.hide_game_layers;
        }
    }
}

#[cfg(test)]
#[path = "tests/ui_debug.rs"]
mod tests;
