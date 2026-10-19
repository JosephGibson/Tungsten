//! Starts the game from its own folder, where `tungsten.json`, `input.json`
//! and `assets/` sit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use tungsten::App;
use tungsten::core::Config;
use tungsten_template_basic::game;
use tungsten_template_basic::shell::GameInfo;

fn main() -> anyhow::Result<()> {
    let config = Config::load("tungsten.json")?;
    let info = GameInfo {
        title: config.window.title.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![PathBuf::from(game::MANIFEST)]);
    if cfg!(debug_assertions) {
        // Edits under assets/ and to input.json apply while the game runs.
        app.enable_hot_reload(&[PathBuf::from("assets")], PathBuf::from(game::MANIFEST));
    }
    // Escape opens the pause menu; the shell's Quit ends the run.
    app.set_exit_on_escape(false);
    game::register(&mut app, info);
    app.run()
}
