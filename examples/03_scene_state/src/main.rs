//! Example 03: scene/state system.
//!
//! Flow: menu -> gameplay -> pause -> gameplay. Scene-owned despawn via `SceneEntity`.
//!
//! M31 (`D-093`): each state change runs behind a screen transition (fade,
//! pixelate, radial wipe, dissolve; see `states.rs`). The transition pass does
//! not cover screen-space text, so `state_driven_text` fades its sections with
//! `1 - StateStack::transition_cover()`.
//!
//! Env `TUNGSTEN_TRANSITION_FIXTURE={none|fade|wipe_radial|dissolve|pixelate}`
//! is for the smoke matrix and `tests/transition_regression.rs`: any value
//! turns the debug HUD off (its timing rows differ between runs), and an effect
//! name also requests menu -> gameplay at startup with that effect, 0.1 s per
//! phase, linear.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod states;

use std::path::PathBuf;

use glam::Vec2;

use tungsten::core::{Config, Entity, Tag, Time, Transform, With, World};
use tungsten::render::TextSection;
use tungsten::{App, DebugHud, StateStack, Transition, TransitionEffect};

use crate::states::{GameplayState, MainMenuState, fixture_effect};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_scene_state/assets/manifest.json";

pub(crate) const QUAD_ID: &str = "ex03_quad";
pub(crate) const SPRITE_HALF: f32 = 8.0;
pub(crate) const VIEW_CENTER: Vec2 = Vec2::new(640.0, 360.0);

/// Seconds per phase of the startup transition a fixture requests.
const FIXTURE_TRANSITION_SECS: f32 = 0.1;

/// `TUNGSTEN_TRANSITION_FIXTURE`, parsed.
#[derive(Clone, Copy)]
enum TransitionFixture {
    /// Not set: interactive run with the debug HUD.
    Off,
    /// `none`: no HUD, no startup transition.
    Still,
    /// An effect name: no HUD, menu -> gameplay behind that effect at startup.
    Effect(TransitionEffect),
}

fn transition_fixture() -> anyhow::Result<TransitionFixture> {
    let Ok(name) = std::env::var("TUNGSTEN_TRANSITION_FIXTURE") else {
        return Ok(TransitionFixture::Off);
    };
    if name == "none" {
        return Ok(TransitionFixture::Still);
    }
    match fixture_effect(&name) {
        Some(effect) => Ok(TransitionFixture::Effect(effect)),
        None => anyhow::bail!(
            "invalid TUNGSTEN_TRANSITION_FIXTURE='{name}': expected one of none, fade, wipe_radial, dissolve, pixelate"
        ),
    }
}

#[derive(Default)]
pub(crate) struct GameplayClock(pub f32);

#[derive(Default)]
pub(crate) struct MenuClock(pub f32);

fn main() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Scene / State System — M20".to_string();
    let fixture = transition_fixture()?;

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);

    {
        let world = app.world_mut();
        world.insert_resource(GameplayClock::default());
        world.insert_resource(MenuClock::default());
    }

    app.on_startup(move |world, _renderer| {
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = matches!(fixture, TransitionFixture::Off);
        }

        let stack = world
            .get_resource_mut::<StateStack>()
            .expect("StateStack resource missing");
        stack.request_push(MainMenuState);
        if let TransitionFixture::Effect(effect) = fixture {
            stack.request_replace_transition(
                GameplayState::default_scene(),
                Transition::new(effect, FIXTURE_TRANSITION_SECS),
            );
        }
    });

    configure(&mut app);

    app.run()
}

/// The example's systems, in `update` after the engine's dispatcher and
/// physics, and its text. `DefaultPlugins` runs the rest; the test below
/// pins the resolved order.
fn configure(app: &mut App) {
    app.add_system_named("menu_idle_system", menu_idle_system);
    app.add_system_named("gameplay_orbit_system", gameplay_orbit_system);
    app.set_extract_text(state_driven_text);
}

fn active_id_is(world: &World, expected: &str) -> bool {
    world
        .get_resource::<StateStack>()
        .and_then(StateStack::active_id)
        .is_some_and(|id| id == expected)
}

fn menu_idle_system(world: &mut World) {
    if !active_id_is(world, "menu") {
        return;
    }

    let dt = world.get_resource::<Time>().map_or(1.0 / 60.0, Time::delta);

    if let Some(clock) = world.get_resource_mut::<MenuClock>() {
        clock.0 += dt;
    }

    let entities = world
        .query_filtered::<Entity, (With<Tag>, With<Transform>)>()
        .collect::<Vec<_>>();
    for entity in entities {
        let is_decoration = world
            .get::<Tag>(entity)
            .is_some_and(|t| t.name == "menu_decoration");
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

    let dt = world.get_resource::<Time>().map_or(1.0 / 60.0, Time::delta);

    let elapsed = if let Some(clock) = world.get_resource_mut::<GameplayClock>() {
        clock.0 += dt;
        clock.0
    } else {
        0.0
    };

    let entities = world
        .query_filtered::<Entity, (With<Tag>, With<Transform>)>()
        .collect::<Vec<_>>();
    for entity in entities {
        let Some(tag_name) = world.get::<Tag>(entity).map(|t| t.name.clone()) else {
            continue;
        };

        if tag_name == "hub" {
            if let Some(t) = world.get_mut::<Transform>(entity) {
                let half = Vec2::splat(t.scale.x * SPRITE_HALF);
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
    let mut sections = match stack.active_id() {
        Some("menu") => menu_text(world),
        Some("gameplay") => gameplay_text(world),
        Some("pause") => pause_text(world),
        _ => Vec::new(),
    };

    // Text draws after the post stack, so the transition pass does not cover
    // it: fade it by the same amount here.
    let cover = stack.transition_cover();
    if cover > 0.0 {
        for section in &mut sections {
            section.color[3] = (f32::from(section.color[3]) * (1.0 - cover)) as u8;
        }
    }
    sections
}

fn menu_text(world: &World) -> Vec<TextSection> {
    let elapsed = world.get_resource::<MenuClock>().map_or(0.0, |c| c.0);
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
            ..Default::default()
        },
        TextSection {
            content: "Scene / State System · Milestone 20".into(),
            font_id: "sans".into(),
            font_size: 24.0,
            line_height: 28.0,
            color: [180, 210, 255, 240],
            position: [430.0, 260.0],
            bounds: None,
            ..Default::default()
        },
        TextSection {
            content: "Press Enter to launch Gameplay".into(),
            font_id: "sans_bold".into(),
            font_size: 30.0,
            line_height: 34.0,
            color: [255, 255, 255, prompt_alpha],
            position: [430.0, 520.0],
            bounds: None,
            ..Default::default()
        },
        TextSection {
            content: "F4 toggles HUD   ·   Esc exits".into(),
            font_id: "mono".into(),
            font_size: 16.0,
            line_height: 20.0,
            color: [160, 170, 200, 200],
            position: [490.0, 568.0],
            bounds: None,
            ..Default::default()
        },
    ]
}

fn gameplay_text(world: &World) -> Vec<TextSection> {
    let elapsed = world.get_resource::<GameplayClock>().map_or(0.0, |c| c.0);

    vec![
        TextSection {
            content: "Gameplay · scene.json spawned 25 entities".into(),
            font_id: "sans_bold".into(),
            font_size: 22.0,
            line_height: 26.0,
            color: [230, 240, 255, 240],
            position: [16.0, 14.0],
            bounds: None,
            ..Default::default()
        },
        TextSection {
            content: format!("t = {elapsed:6.2}s   ·   P pauses   ·   Backspace returns to menu"),
            font_id: "mono".into(),
            font_size: 16.0,
            line_height: 20.0,
            color: [180, 200, 230, 220],
            position: [16.0, 44.0],
            bounds: None,
            ..Default::default()
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
            ..Default::default()
        },
        TextSection {
            content: "Press P to resume   ·   Backspace returns to menu".into(),
            font_id: "sans".into(),
            font_size: 22.0,
            line_height: 26.0,
            color: [220, 225, 240, 240],
            position: [370.0, 430.0],
            bounds: None,
            ..Default::default()
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The resolved schedule of the example as `main` builds it (M38).
    const SCHEDULE: &str = "\
startup: -
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, inspector_pick, hud_toggle, display_input, state_dispatcher
fixed_update: physics_step
update: menu_idle_system, gameplay_orbit_system
post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update
";

    #[test]
    fn the_schedule_matches_the_snapshot() {
        let mut app = App::new(Config::default()).expect("App::new failed");
        configure(&mut app);
        app.resolve_schedule().expect("the schedule resolves");
        assert_eq!(app.schedule().resolved_text(), SCHEDULE);
    }
}
