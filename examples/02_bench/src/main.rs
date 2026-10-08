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
//! In an interactive run (no `TUNGSTEN_SMOKE_FRAMES`) Tab relaunches the
//! binary on the next benchmark.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

use tungsten::core::{Config, InputState, KeyCode, PluginSet, Resolution, World};
use tungsten::{
    App, DebugPlugin, DisplayPlugin, InspectorState, ParticlesPlugin, PhysicsDebugOverlay,
    StatePlugin, SystemTimingOverlay, TweensPlugin,
};

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
    let interactive = interactive();
    let hint = if interactive {
        " - Tab: next benchmark"
    } else {
        ""
    };
    config.window.title = format!("Tungsten bench: {} ({}){hint}", cfg.row(), cfg.preset);
    config.display.resolution = Some(Resolution {
        width: VIEWPORT.x as u32,
        height: VIEWPORT.y as u32,
    });
    config.display.vsync = Some(false);
    (cfg.bench.engine_config)(&mut config, &cfg);
    // No `DefaultPlugins`: each row wires the engine systems it measures
    // itself, in its own order. These are the engine stages every row ran
    // before the schedule, so the frames stay comparable.
    let plugins = PluginSet::new()
        .with(DebugPlugin)
        .with(DisplayPlugin)
        .with(StatePlugin)
        .with(ParticlesPlugin)
        .with(TweensPlugin);
    let mut app = App::with_plugins(config, plugins)?;
    (cfg.bench.configure)(&mut app, &cfg);
    // Captures run under `TUNGSTEN_SMOKE_FRAMES`; the extra system would add a
    // row to their `systems:` lines.
    if interactive {
        app.add_system_named("bench_cycle", cycle_system(cfg.bench.name, cfg.preset));
    }
    counters::log_config(&bench_config);
    apply_overlay_env(&mut app);
    app.run()
}

/// Whether this is a window run rather than a pinned-dt smoke or capture run.
fn interactive() -> bool {
    std::env::var("TUNGSTEN_SMOKE_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .is_none_or(|frames| frames == 0)
}

/// Tab starts the next benchmark in `BENCHES` and closes this window.
fn cycle_system(current: &'static str, preset: &'static str) -> impl FnMut(&mut World) + 'static {
    move |world| {
        let pressed = world
            .get_resource::<InputState>()
            .is_some_and(|input| input.just_pressed(KeyCode::Tab));
        if !pressed {
            return;
        }
        let next = knobs::next_bench(BENCHES, current);
        match relaunch(next, preset) {
            Ok(()) => std::process::exit(0),
            Err(err) => log::error!("cannot start '{}': {err}", next.name),
        }
    }
}

/// Spawn this binary on `next`. The preset carries over when `next` has it;
/// `TUNGSTEN_BENCH_SET` names knobs of the current benchmark, so it is dropped.
fn relaunch(next: &Bench, preset: &str) -> std::io::Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .env("TUNGSTEN_BENCH", next.name)
        .env_remove("TUNGSTEN_BENCH_SET");
    if next
        .presets
        .iter()
        .any(|candidate| candidate.name == preset)
    {
        command.env("TUNGSTEN_BENCH_PRESET", preset);
    } else {
        command.env_remove("TUNGSTEN_BENCH_PRESET");
    }
    command.spawn().map(drop)
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
