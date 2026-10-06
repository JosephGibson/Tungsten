//! Starts the game from its own folder, where `tungsten.json`, `input.json`
//! and `assets/` sit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use tungsten::App;
use tungsten::core::Config;
use tungsten_template_basic::game;

fn main() -> anyhow::Result<()> {
    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![PathBuf::from(game::MANIFEST)]);
    if cfg!(debug_assertions) {
        // Edits under assets/ and to input.json apply while the game runs.
        app.enable_hot_reload(&[PathBuf::from("assets")], PathBuf::from(game::MANIFEST));
    }
    game::register(&mut app);
    app.run()
}
