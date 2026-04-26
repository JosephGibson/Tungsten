//! Example 01: platformer feature kitchen sink.
//!
//! Manifests: shared root plus example-local tilemap/sprites. Hot reload: assets + `input.json`.

mod extract;
mod setup;
mod state;
mod systems;

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;

use tungsten::core::{Config, DisplayMode, Resolution};
use tungsten::App;

const STARTUP_WINDOW_WIDTH: u32 = 1920;
const STARTUP_WINDOW_HEIGHT: u32 = 1080;

fn apply_example_window_defaults(config: &mut Config) {
    config.window.width = STARTUP_WINDOW_WIDTH;
    config.window.height = STARTUP_WINDOW_HEIGHT;
    if std::env::var_os("TUNGSTEN_DISPLAY_RESOLUTION").is_none() {
        config.display.resolution = Some(Resolution {
            width: STARTUP_WINDOW_WIDTH,
            height: STARTUP_WINDOW_HEIGHT,
        });
    }
    if std::env::var_os("TUNGSTEN_DISPLAY_MODE").is_none() {
        config.display.display_mode = Some(DisplayMode::Windowed);
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    apply_example_window_defaults(&mut config);
    let mut app = App::new(config)?;
    setup::configure_app(&mut app);
    app.run()
}
