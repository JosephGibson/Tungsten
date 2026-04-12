use tungsten::core::Config;
use tungsten::App;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    log::info!(
        "Loaded config: {}x{} '{}'",
        config.window.width,
        config.window.height,
        config.window.title,
    );

    let app = App::new(config);
    app.run()
}
