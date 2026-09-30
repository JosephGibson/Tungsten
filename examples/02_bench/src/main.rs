//! Example 02: benchmark suite. One binary runs one benchmark per launch
//! (`docs/perf/benchmarks.md`); captures go through
//! `just perf run <bench>` (`scripts/bench.py`).
//!
//! Env: `TUNGSTEN_BENCH` (default `physics`), `TUNGSTEN_BENCH_PRESET`
//! (default `default`), `TUNGSTEN_BENCH_SCALE` (multiplies the scalable
//! knobs) and `TUNGSTEN_BENCH_SET=knob=value,...` (applied last).
//! `TUNGSTEN_BENCH_DESCRIBE=1` prints every benchmark's schema as JSON and
//! `=config` the resolved configuration, both before a window opens.
//! `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` enables overlays.

mod churn;
mod counters;
mod ecs;
mod r#gen;
mod gpu;
mod integrated;
mod knobs;
mod particles;
mod physics;
mod view;

use tungsten::core::{Config, Resolution};
use tungsten::{App, InspectorState, PhysicsDebugOverlay, SystemTimingOverlay};

use crate::knobs::Bench;
use crate::view::VIEWPORT;

const BENCHES: &[&Bench] = &[
    &physics::BENCH,
    &ecs::BENCH,
    &churn::BENCH,
    &gpu::BENCH,
    &particles::BENCH,
    &integrated::BENCH,
];

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let describe = std::env::var("TUNGSTEN_BENCH_DESCRIBE").ok();
    if describe.as_deref() == Some("1") {
        println!("{}", knobs::describe(BENCHES));
        return Ok(());
    }
    let cfg = knobs::from_env(BENCHES)?;
    let bench_config = cfg.to_json();
    match describe.as_deref() {
        None => {}
        Some("config") => {
            println!("{bench_config}");
            return Ok(());
        }
        Some(other) => {
            anyhow::bail!("TUNGSTEN_BENCH_DESCRIBE must be '1' or 'config', got '{other}'")
        }
    }

    let mut config = Config::load("tungsten.json")?;
    config.window.title = format!("Tungsten bench: {} ({})", cfg.row(), cfg.preset);
    config.display.resolution = Some(Resolution {
        width: VIEWPORT.x as u32,
        height: VIEWPORT.y as u32,
    });
    config.display.vsync = Some(false);
    (cfg.bench.engine_config)(&mut config, &cfg);
    let mut app = App::new(config)?;
    (cfg.bench.configure)(&mut app, &cfg);
    counters::log_config(&bench_config);
    apply_overlay_env(&mut app);
    app.run()
}

/// Enable overlays listed in `TUNGSTEN_OVERLAYS_ON`.
fn apply_overlay_env(app: &mut App) {
    let Ok(raw) = std::env::var("TUNGSTEN_OVERLAYS_ON") else {
        return;
    };
    let world = app.world_mut();
    for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match token {
            "physics" => {
                if let Some(overlay) = world.get_resource_mut::<PhysicsDebugOverlay>() {
                    overlay.enabled = true;
                }
            }
            "systems" => {
                if let Some(overlay) = world.get_resource_mut::<SystemTimingOverlay>() {
                    overlay.enabled = true;
                }
            }
            "inspector" => {
                if let Some(state) = world.get_resource_mut::<InspectorState>() {
                    state.enabled = true;
                }
            }
            other => {
                log::warn!("TUNGSTEN_OVERLAYS_ON: ignoring unknown token '{other}'");
            }
        }
    }
}
