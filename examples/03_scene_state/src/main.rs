//! Example 03 — Scene/State System (M20)
//!
//! Demonstrates the `StateStack` dispatcher driving a
//! `MainMenu → Gameplay → Pause → Gameplay` flow, scene-owned entity
//! auto-despawn via `SceneEntity { state_id }`, and the data-driven
//! `scene.json` loader.
//!
//! Controls:
//!   Enter     — menu: start gameplay (loads `scene.json`)
//!   P         — gameplay: pause; pause: resume
//!   Backspace — gameplay: return to menu
//!   F4        — toggle the debug HUD (the `state` row mirrors the top state id)
//!   Esc       — exit

mod states;

use glam::Vec2;

use tungsten::core::{Config, DeltaTime, ResolvedManifest, Tag, Transform, World};
use tungsten::render::TextSection;
use tungsten::{asset_loader, App, DebugHud, StateStack};

use crate::states::MainMenuState;

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_scene_state/assets/manifest.json";

pub(crate) const QUAD_ID: &str = "ex03_quad";
pub(crate) const SPRITE_HALF: f32 = 8.0;
pub(crate) const VIEW_CENTER: Vec2 = Vec2::new(640.0, 360.0);

#[derive(Default)]
pub(crate) struct GameplayClock(pub f32);

#[derive(Default)]
pub(crate) struct MenuClock(pub f32);

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Scene / State System — M20".to_string();

    let mut app = App::new(config)?;

    {
        let world = app.world_mut();
        world.insert_resource(GameplayClock::default());
        world.insert_resource(MenuClock::default());
    }

    app.on_startup(|world, renderer| {
        let root = ResolvedManifest::load(ROOT_MANIFEST).expect("Failed to load root manifest");
        asset_loader::load_fonts(&root, world, renderer).expect("Failed to load shared fonts");

        let local = ResolvedManifest::load(LOCAL_MANIFEST).expect("Failed to load local manifest");
        asset_loader::load_sprites(&local, world, renderer)
            .expect("Failed to load example 03 sprites");

        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = true;
        }

        world
            .get_resource_mut::<StateStack>()
            .expect("StateStack resource missing")
            .request_push(MainMenuState);
    });

    app.add_system_named("menu_idle_system", menu_idle_system);
    app.add_system_named("gameplay_orbit_system", gameplay_orbit_system);
    app.set_extract_text(state_driven_text);

    app.run()
}

fn active_id_is(world: &World, expected: &str) -> bool {
    world
        .get_resource::<StateStack>()
        .and_then(|stack| stack.active_id())
        .map(|id| id == expected)
        .unwrap_or(false)
}

fn menu_idle_system(world: &mut World) {
    if !active_id_is(world, "menu") {
        return;
    }

    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(1.0 / 60.0);

    if let Some(clock) = world.get_resource_mut::<MenuClock>() {
        clock.0 += dt;
    }

    let entities = world.query2_entities::<Tag, Transform>();
    for entity in entities {
        let is_decoration = world
            .get::<Tag>(entity)
            .map(|t| t.name == "menu_decoration")
            .unwrap_or(false);
        if !is_decoration {
            continue;
        }
        let Some(transform) = world.get::<Transform>(entity).copied() else {
            continue;
        };

        let half = Vec2::splat(transform.scale.x * SPRITE_HALF);
        let sprite_center = transform.position + half;
        let offset = sprite_center - VIEW_CENTER;
        let radius = offset.length();
        if radius <= f32::EPSILON {
            continue;
        }
        let angle = offset.y.atan2(offset.x) + dt * 0.35;
        let new_center = VIEW_CENTER + Vec2::new(angle.cos(), angle.sin()) * radius;

        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = new_center - half;
            t.rotation += dt * 1.4;
        }
    }
}

fn gameplay_orbit_system(world: &mut World) {
    if !active_id_is(world, "gameplay") {
        return;
    }

    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(1.0 / 60.0);

    let elapsed = if let Some(clock) = world.get_resource_mut::<GameplayClock>() {
        clock.0 += dt;
        clock.0
    } else {
        0.0
    };

    let entities = world.query2_entities::<Tag, Transform>();
    for entity in entities {
        let Some(tag_name) = world.get::<Tag>(entity).map(|t| t.name.clone()) else {
            continue;
        };

        if tag_name == "hub" {
            let pulse = 3.0 + (elapsed * 1.8).sin() * 0.35;
            if let Some(t) = world.get_mut::<Transform>(entity) {
                t.scale = Vec2::splat(pulse);
                let half = Vec2::splat(pulse * SPRITE_HALF);
                t.position = VIEW_CENTER - half;
                t.rotation += dt * 0.6;
            }
            continue;
        }

        let (omega, spin) = match tag_name.as_str() {
            "ring_a" => (0.55, 1.5),
            "ring_b" => (-0.34, -1.0),
            "ring_c" => (0.19, 0.55),
            _ => continue,
        };

        let Some(transform) = world.get::<Transform>(entity).copied() else {
            continue;
        };
        let old_half = Vec2::splat(transform.scale.x * SPRITE_HALF);
        let sprite_center = transform.position + old_half;
        let offset = sprite_center - VIEW_CENTER;
        let radius = offset.length();
        if radius <= f32::EPSILON {
            continue;
        }
        let angle = offset.y.atan2(offset.x) + dt * omega;
        let new_center = VIEW_CENTER + Vec2::new(angle.cos(), angle.sin()) * radius;

        let shimmer = 1.5 + (elapsed * 1.2 + radius * 0.018).sin() * 0.18;
        let new_half = Vec2::splat(shimmer * SPRITE_HALF);

        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.scale = Vec2::splat(shimmer);
            t.position = new_center - new_half;
            t.rotation += dt * spin;
        }
    }
}

fn state_driven_text(world: &World) -> Vec<TextSection> {
    let Some(stack) = world.get_resource::<StateStack>() else {
        return Vec::new();
    };
    match stack.active_id() {
        Some("menu") => menu_text(world),
        Some("gameplay") => gameplay_text(world),
        Some("pause") => pause_text(world),
        _ => Vec::new(),
    }
}

fn menu_text(world: &World) -> Vec<TextSection> {
    let elapsed = world
        .get_resource::<MenuClock>()
        .map(|c| c.0)
        .unwrap_or(0.0);
    let prompt_alpha = (((elapsed * 2.0).sin() * 0.5 + 0.5) * 130.0 + 125.0) as u8;

    vec![
        TextSection {
            content: "TUNGSTEN".into(),
            font_id: "sans_bold".into(),
            font_size: 96.0,
            line_height: 104.0,
            color: [240, 244, 255, 255],
            position: [430.0, 150.0],
            bounds: None,
        },
        TextSection {
            content: "Scene / State System · Milestone 20".into(),
            font_id: "sans".into(),
            font_size: 24.0,
            line_height: 28.0,
            color: [180, 210, 255, 240],
            position: [430.0, 260.0],
            bounds: None,
        },
        TextSection {
            content: "Press Enter to launch Gameplay".into(),
            font_id: "sans_bold".into(),
            font_size: 30.0,
            line_height: 34.0,
            color: [255, 255, 255, prompt_alpha],
            position: [430.0, 520.0],
            bounds: None,
        },
        TextSection {
            content: "F4 toggles HUD   ·   Esc exits".into(),
            font_id: "mono".into(),
            font_size: 16.0,
            line_height: 20.0,
            color: [160, 170, 200, 200],
            position: [490.0, 568.0],
            bounds: None,
        },
    ]
}

fn gameplay_text(world: &World) -> Vec<TextSection> {
    let elapsed = world
        .get_resource::<GameplayClock>()
        .map(|c| c.0)
        .unwrap_or(0.0);

    vec![
        TextSection {
            content: "Gameplay · scene.json spawned 25 entities".into(),
            font_id: "sans_bold".into(),
            font_size: 22.0,
            line_height: 26.0,
            color: [230, 240, 255, 240],
            position: [16.0, 14.0],
            bounds: None,
        },
        TextSection {
            content: format!(
                "t = {:6.2}s   ·   P pauses   ·   Backspace returns to menu",
                elapsed
            ),
            font_id: "mono".into(),
            font_size: 16.0,
            line_height: 20.0,
            color: [180, 200, 230, 220],
            position: [16.0, 44.0],
            bounds: None,
        },
    ]
}

fn pause_text(_world: &World) -> Vec<TextSection> {
    vec![
        TextSection {
            content: "PAUSED".into(),
            font_id: "sans_bold".into(),
            font_size: 96.0,
            line_height: 104.0,
            color: [255, 255, 255, 255],
            position: [450.0, 308.0],
            bounds: None,
        },
        TextSection {
            content: "Press P to resume   ·   Backspace returns to menu".into(),
            font_id: "sans".into(),
            font_size: 22.0,
            line_height: 26.0,
            color: [220, 225, 240, 240],
            position: [370.0, 430.0],
            bounds: None,
        },
    ]
}
