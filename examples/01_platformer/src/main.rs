//! Example 01 — Platformer
//!
//! Single project that exercises all engine features:
//!   ECS, physics (AABB + circles + tilemap collision), sprites, animation,
//!   audio, text, camera follow, input, and hot reload.
//!
//! Controls (defaults; see `input.json` at the workspace root to rebind):
//!   A / D or ←/→   move_left / move_right
//!   Space           jump (when grounded; plays a sound effect)
//!   LMB (hold)      spawn_ball at the cursor (one every 32 ms while held)
//!   RMB             spawn_black_hole at the cursor
//!   M               audio_toggle_music
//!   1 / 2 / 3       volume_preset_low / volume_preset_mid / volume_preset_high
//!   S or MMB        audio_stop_all
//!   = / - or wheel  zoom_in / zoom_out (50%–200% of base)
//!   F1              engine_toggle_physics_debug
//!   F2              engine_toggle_systems_overlay
//!   F3              engine_toggle_inspector
//!   F4              engine_toggle_hud
//!   F9              engine_toggle_vsync
//!   F11             engine_toggle_fullscreen
//!   Escape          engine_exit
//!
//! Cursor: the native OS cursor stays visible (winit default). A custom
//! crosshair sprite (`ex10_cursor`) is drawn in world space at the same
//! point so both layers are visible at once, demoing a sprite-cursor path
//! without having to hide the OS pointer.
//!
//! Two manifests are loaded at startup:
//!   • assets/manifest.json        — fonts, walk animation, sounds (shared root)
//!   • examples/01_platformer/assets/manifest.json — tilemap + tile sprites (local)
//!
//! Hot reload watches the local assets directory and the workspace-root
//! `input.json`; edit level.tmj or the action map while running and the
//! changes land within a frame.

mod extract;
mod setup;
mod state;
mod systems;

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;

use tungsten::core::Config;
use tungsten::App;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config)?;
    setup::configure_app(&mut app);
    app.run()
}
