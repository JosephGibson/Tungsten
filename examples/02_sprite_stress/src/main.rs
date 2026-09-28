//! Example 02: sprite stress.
//!
//! Modes: `baseline` default, `ecs-high-load`, `physics-stress`, `render-features`. Env:
//! `STRESS_SCENE`, `STRESS_COUNT`, `STRESS_PHYSICS_SLEEP` (`0`/`off` disables
//! island sleeping in `physics-stress` for solver-throughput captures).
//! `STRESS_ECS_DENSITY=preserve` scales ECS world area with count; default `fixed`.
//! Perf capture: release, Vulkan, 1920x1080, 300 frames after 60-frame warm-up.

mod baseline;
mod ecs_high_load;
mod physics_stress;
mod render_features;
mod shared;

use tungsten::core::Config;
use tungsten::{App, InspectorState, PhysicsDebugOverlay, SystemTimingOverlay};

use crate::baseline::{DEFAULT_SPRITE_COUNT, configure_baseline_scene};
use crate::ecs_high_load::{DEFAULT_HIGH_LOAD_COUNT, configure_high_load_scene};
use crate::physics_stress::{DEFAULT_PHYSICS_STRESS_COUNT, configure_physics_stress_scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StressScene {
    Baseline,
    EcsHighLoad,
    PhysicsStress,
    RenderFeatures,
}

impl StressScene {
    fn parse(raw: Option<&str>) -> anyhow::Result<Self> {
        match raw.unwrap_or("baseline") {
            "baseline" => Ok(Self::Baseline),
            "ecs-high-load" => Ok(Self::EcsHighLoad),
            "physics-stress" => Ok(Self::PhysicsStress),
            "render-features" => Ok(Self::RenderFeatures),
            other => Err(anyhow::anyhow!(
                "Unknown STRESS_SCENE '{other}'. Expected 'baseline', 'ecs-high-load', 'physics-stress', or 'render-features'"
            )),
        }
    }

    fn default_count(self) -> usize {
        match self {
            Self::Baseline => DEFAULT_SPRITE_COUNT,
            Self::EcsHighLoad => DEFAULT_HIGH_LOAD_COUNT,
            Self::PhysicsStress => DEFAULT_PHYSICS_STRESS_COUNT,
            Self::RenderFeatures => render_features::DEFAULT_RENDER_FEATURES_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExampleOptions {
    scene: StressScene,
    count: usize,
    physics_sleep: bool,
    preserve_density: bool,
}

impl ExampleOptions {
    fn from_env() -> anyhow::Result<Self> {
        let raw_scene = std::env::var("STRESS_SCENE").ok();
        let scene = StressScene::parse(raw_scene.as_deref())?;
        let raw_count = std::env::var("STRESS_COUNT").ok();
        let count = resolve_count(scene, raw_count.as_deref());
        let raw_sleep = std::env::var("STRESS_PHYSICS_SLEEP").ok();
        let physics_sleep = parse_physics_sleep(raw_sleep.as_deref())?;
        let raw_density = std::env::var("STRESS_ECS_DENSITY").ok();
        let preserve_density = parse_ecs_density(raw_density.as_deref())?;
        Ok(Self {
            scene,
            count,
            physics_sleep,
            preserve_density,
        })
    }
}

/// `STRESS_PHYSICS_SLEEP`: unset, `1` or `on` keeps sleeping (the canonical
/// scene); `0` or `off` disables it.
fn parse_physics_sleep(raw: Option<&str>) -> anyhow::Result<bool> {
    match raw.unwrap_or("1") {
        "1" | "on" => Ok(true),
        "0" | "off" => Ok(false),
        other => Err(anyhow::anyhow!(
            "Unknown STRESS_PHYSICS_SLEEP '{other}'. Expected '1', 'on', '0', or 'off'"
        )),
    }
}

/// Fixed world is the canonical 50k workload; preserve scales world area by count.
fn parse_ecs_density(raw: Option<&str>) -> anyhow::Result<bool> {
    match raw.unwrap_or("fixed") {
        "fixed" => Ok(false),
        "preserve" => Ok(true),
        other => Err(anyhow::anyhow!(
            "Unknown STRESS_ECS_DENSITY '{other}'. Expected 'fixed' or 'preserve'"
        )),
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
        StressScene::RenderFeatures => format!("Render Features ({} sprites)", options.count),
        StressScene::Baseline => format!("Sprite Stress ({} sprites)", options.count),
        StressScene::EcsHighLoad => {
            format!("Sprite Stress ECS High Load ({} entities)", options.count)
        }
        StressScene::PhysicsStress => {
            format!("Physics Stress ({} bodies)", options.count)
        }
    };
    config.display.resolution = Some(tungsten::core::Resolution {
        width: 1920,
        height: 1080,
    });
    config.display.vsync = Some(false);

    if options.scene == StressScene::RenderFeatures {
        config.render.post_aa = tungsten::core::config::PostAaMode::SmaaHigh;
    }
    let mut app = App::new(config)?;

    match options.scene {
        StressScene::RenderFeatures => {
            render_features::configure_render_features_scene(&mut app, options.count);
        }
        StressScene::Baseline => configure_baseline_scene(&mut app, options.count),
        StressScene::EcsHighLoad => {
            configure_high_load_scene(&mut app, options.count, options.preserve_density);
        }
        StressScene::PhysicsStress => {
            configure_physics_stress_scene(&mut app, options.count, options.physics_sleep);
        }
    }

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

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
