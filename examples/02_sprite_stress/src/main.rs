//! Example 02 — Sprite Stress
//!
//! Two scene modes live under the same binary:
//!   - `baseline` (default): original M12 sine-wave sprite scene
//!   - `ecs-high-load`: 50k-entity ECS + render + camera stress scene
//!
//! Env vars:
//!   - `STRESS_SCENE=baseline|ecs-high-load`
//!   - `STRESS_COUNT=<n>` overrides the scene-specific default count
//!
//! Fixed capture rules (M12 baseline):
//!   Build mode:   release  (`cargo run -p example-02-sprite-stress --release`)
//!   Backend:      WGPU_BACKEND=vulkan  (Linux)
//!   Resolution:   1920 × 1080  (set through `config.display.resolution`)
//!   Frame window: 300 frames after 60-frame warm-up
//!   Present path: checked-in default auto no-vsync (`tungsten.json` keeps
//!                 `display.present_mode = "auto"` and this example keeps
//!                 `display.vsync = false`)
//!
//! Scene ownership:
//!   - [`baseline`]        → baseline.rs
//!   - [`ecs_high_load`]   → ecs_high_load.rs
//!   - Shared telemetry    → shared.rs
//!
//! Telemetry output: printed to stdout every 60 frames.
//! Baseline capture: pipe to `tee perf-runs/<timestamp>/sprite-stress.txt`

mod baseline;
mod ecs_high_load;
mod shared;

use tungsten::core::Config;
use tungsten::{App, InspectorState, PhysicsDebugOverlay, SystemTimingOverlay};

use crate::baseline::{configure_baseline_scene, DEFAULT_SPRITE_COUNT};
use crate::ecs_high_load::{configure_high_load_scene, DEFAULT_HIGH_LOAD_COUNT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StressScene {
    Baseline,
    EcsHighLoad,
}

impl StressScene {
    fn parse(raw: Option<&str>) -> anyhow::Result<Self> {
        match raw.unwrap_or("baseline") {
            "baseline" => Ok(Self::Baseline),
            "ecs-high-load" => Ok(Self::EcsHighLoad),
            other => Err(anyhow::anyhow!(
                "Unknown STRESS_SCENE '{other}'. Expected 'baseline' or 'ecs-high-load'"
            )),
        }
    }

    fn default_count(self) -> usize {
        match self {
            Self::Baseline => DEFAULT_SPRITE_COUNT,
            Self::EcsHighLoad => DEFAULT_HIGH_LOAD_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExampleOptions {
    scene: StressScene,
    count: usize,
}

impl ExampleOptions {
    fn from_env() -> anyhow::Result<Self> {
        let raw_scene = std::env::var("STRESS_SCENE").ok();
        let scene = StressScene::parse(raw_scene.as_deref())?;
        let raw_count = std::env::var("STRESS_COUNT").ok();
        let count = resolve_count(scene, raw_count.as_deref());
        Ok(Self { scene, count })
    }
}

fn resolve_count(scene: StressScene, raw_count: Option<&str>) -> usize {
    raw_count
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or_else(|| scene.default_count())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let options = ExampleOptions::from_env()?;

    let mut config = Config::load("tungsten.json")?;
    config.window.title = match options.scene {
        StressScene::Baseline => format!("Sprite Stress ({} sprites)", options.count),
        StressScene::EcsHighLoad => {
            format!("Sprite Stress ECS High Load ({} entities)", options.count)
        }
    };
    config.display.resolution = Some(tungsten::core::Resolution {
        width: 1920,
        height: 1080,
    });
    config.display.vsync = Some(false);

    let mut app = App::new(config)?;

    match options.scene {
        StressScene::Baseline => configure_baseline_scene(&mut app, options.count),
        StressScene::EcsHighLoad => configure_high_load_scene(&mut app, options.count),
    }

    apply_overlay_env(&mut app);

    app.run()
}

/// Flips matching overlay resources `.enabled = true` based on the
/// comma-separated `TUNGSTEN_OVERLAYS_ON` env var. Supported tokens:
/// `physics`, `systems`, `inspector`. Unknown tokens are ignored so perf
/// captures can tolerate typos without failing.
fn apply_overlay_env(app: &mut App) {
    let Ok(raw) = std::env::var("TUNGSTEN_OVERLAYS_ON") else {
        return;
    };
    let world = app.world_mut();
    for token in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
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

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
