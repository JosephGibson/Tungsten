//! Exercises the engine's logger and crash report from outside (`D-119`),
//! one process per mode, for `tests/probe.rs` and the release probe in
//! `scripts/crash-report.py`. Each run takes its user folder from
//! `TUNGSTEN_USER_DIR`.
//!
//! - `panic` (the default): sets a hook that prints a marker, then panics
//!   two calls deep after `App::new`, so the engine's report chains to it.
//! - `log <level>`: sets `logging.level`, then logs one record at each level
//!   from warn to trace.
//! - `own-logger`: installs its own logger before `App::new`, then logs a
//!   marker through it.
//! - `bad-input`: `App::new` in a folder whose `input.json` is malformed.
//! - `bad-id`: `game.id` set in code to `../x`.

use std::error::Error;

use tungsten::App;
use tungsten::core::Config;

/// Printed by the hook the engine's hook replaces and then calls.
const HOOK_MARKER: &str = "tungsten-crash-probe: previous hook ran";

static PROBE_LOGGER: ProbeLogger = ProbeLogger;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map_or("panic", String::as_str) {
        "panic" => panic_mode(),
        "log" => log_mode(args.get(1).map_or("info", String::as_str)),
        "own-logger" => own_logger_mode(),
        "bad-input" => {
            App::new(probe_config())?;
            Ok(())
        }
        "bad-id" => {
            let mut config = probe_config();
            config.game.id = Some("../x".to_string());
            App::new(config)?;
            Ok(())
        }
        other => Err(format!(
            "unknown mode '{other}': expected panic, log <level>, own-logger, bad-input or bad-id"
        )
        .into()),
    }
}

/// The probe names its own folder; its version stands in for a game's.
fn probe_config() -> Config {
    let mut config = Config::default();
    config.game.id = Some("tungsten-crash-probe".to_string());
    config.game.version = Some(env!("CARGO_PKG_VERSION").to_string());
    config
}

fn panic_mode() -> Result<(), Box<dyn Error>> {
    std::panic::set_hook(Box::new(|_| eprintln!("{HOOK_MARKER}")));
    let _app = App::new(probe_config())?;
    outer();
    Ok(())
}

#[inline(never)]
fn outer() {
    inner();
}

#[inline(never)]
fn inner() {
    panic!("tungsten-crash-probe: deliberate panic");
}

fn log_mode(level: &str) -> Result<(), Box<dyn Error>> {
    let mut config = probe_config();
    config.logging.level = level.to_string();
    let _app = App::new(config)?;
    log::warn!("probe record at warn");
    log::info!("probe record at info");
    log::debug!("probe record at debug");
    log::trace!("probe record at trace");
    Ok(())
}

fn own_logger_mode() -> Result<(), Box<dyn Error>> {
    log::set_logger(&PROBE_LOGGER)?;
    log::set_max_level(log::LevelFilter::Info);
    let _app = App::new(probe_config())?;
    log::info!("own-logger marker");
    Ok(())
}

/// The game's own logger, which the engine leaves in place.
struct ProbeLogger;

impl log::Log for ProbeLogger {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        eprintln!("probe-logger: {}", record.args());
    }

    fn flush(&self) {}
}
