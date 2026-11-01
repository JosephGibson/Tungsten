//! Starts the game from its own folder, where `tungsten.json`, `input.json`
//! and `assets/` sit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    // `tungsten.json`'s `assets` section names the manifest; in a debug
    // build, edits under assets/ and to input.json apply while it runs.
    let mut app = App::new(config)?;
    // Escape opens the pause menu; the shell's Quit ends the run.
    app.set_exit_on_escape(false);
    game::register(&mut app, info);
    app.run()
}
