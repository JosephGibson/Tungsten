//! Example 01 — Platformer
//!
//! Single project that exercises all engine features:
//!   ECS, physics (AABB + circles + tilemap collision), sprites, animation,
//!   audio, text, camera follow, input, and hot reload.
//!
//! Controls:
//!   A / D or ←/→   horizontal movement
//!   Space           jump (when grounded; plays a sound effect)
//!   M               toggle background music
//!   1 / 2 / 3       master volume: 20% / 50% / 100%
//!   S               stop all sounds
//!   = / -           zoom in / zoom out (50%–200% of base)
//!   F4              toggle developer HUD
//!   F9              toggle vsync
//!   F11             toggle windowed / borderless fullscreen
//!
//! Two manifests are loaded at startup:
//!   • assets/manifest.json        — fonts, walk animation, sounds (shared root)
//!   • examples/01_platformer/assets/manifest.json — tilemap + tile sprites (local)
//!
//! Hot reload watches the local assets directory; edit level.tmj while running
//! and the map updates within a frame.

mod extract;
mod setup;
mod state;
mod systems;

#[cfg(test)]
mod tests;

use tungsten::core::Config;
use tungsten::App;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);
    setup::configure_app(&mut app);
    app.run()
}
