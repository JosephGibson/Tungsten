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

use tungsten::core::Config;
use tungsten::App;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config)?;
    setup::configure_app(&mut app);
    app.run()
}
