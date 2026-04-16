//! Example 03 — Component Sprites (M15)
//!
//! Demonstrates the default sprite-extract path: no `set_extract_sprites`
//! call, just entities with `Transform + Sprite + Visibility` and the
//! engine renders them. Covers rotation, scale, tint, z-order, the
//! `Visibility` toggle, and `Tag`.
//!
//! Controls:
//!   V — toggle Visibility on the tagged entity (player quad)
//!   Esc — exit

use glam::Vec2;

use tungsten::core::{
    AssetRegistry, Camera2D, Config, DeltaTime, InputState, KeyCode, ResolvedManifest, Sprite, Tag,
    Transform, Visibility, World,
};
use tungsten::{asset_loader, App};

const MANIFEST_LOCAL: &str = "examples/03_component_sprites/assets/manifest.json";
const QUAD_ID: &str = "ex03_quad";

/// Marker for the spinning quad.
struct Spinner;

/// Marker for the pulsing (scaled) quad.
struct Pulser {
    time: f32,
}

/// Marker for the tint-cycling quad.
struct Tinter {
    time: f32,
}

/// Spins a `Transform.rotation` at 1.0 rad/s.
fn spin_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.dt)
        .unwrap_or(0.0);
    let entities = world.query2_entities::<Spinner, Transform>();
    for e in entities {
        if let Some(t) = world.get_mut::<Transform>(e) {
            t.rotation += dt;
        }
    }
}

/// Pulses `Transform.scale` as `1.0 + 0.25 * sin(time)`.
fn pulse_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.dt)
        .unwrap_or(0.0);
    let entities = world.query2_entities::<Pulser, Transform>();
    for e in entities {
        let time = {
            let p = world.get_mut::<Pulser>(e).unwrap();
            p.time += dt;
            p.time
        };
        if let Some(t) = world.get_mut::<Transform>(e) {
            let s = 1.0 + 0.25 * time.sin();
            t.scale = Vec2::splat(s);
        }
    }
}

/// Cycles `Sprite.color` through the RGB wheel.
fn tint_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.dt)
        .unwrap_or(0.0);
    let entities = world.query2_entities::<Tinter, Sprite>();
    for e in entities {
        let time = {
            let t = world.get_mut::<Tinter>(e).unwrap();
            t.time += dt;
            t.time
        };
        if let Some(sprite) = world.get_mut::<Sprite>(e) {
            let r = ((time * 0.9).sin() * 0.5 + 0.5) * 255.0;
            let g = ((time * 1.1 + 2.1).sin() * 0.5 + 0.5) * 255.0;
            let b = ((time * 1.3 + 4.2).sin() * 0.5 + 0.5) * 255.0;
            sprite.color = [r as u8, g as u8, b as u8, 255];
        }
    }
}

/// Toggles `Visibility.visible` on entities with `Tag { name == "player" }`
/// when `V` is pressed (edge-triggered).
fn visibility_toggle_system(world: &mut World) {
    let pressed = world
        .get_resource::<InputState>()
        .map(|i| i.just_pressed(KeyCode::KeyV))
        .unwrap_or(false);
    if !pressed {
        return;
    }

    let entities = world.query2_entities::<Tag, Visibility>();
    for e in entities {
        let is_player = world
            .get::<Tag>(e)
            .map(|t| t.name == "player")
            .unwrap_or(false);
        if !is_player {
            continue;
        }
        if let Some(v) = world.get_mut::<Visibility>(e) {
            v.visible = !v.visible;
            log::info!("visibility_toggle_system: player.visible = {}", v.visible);
        }
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json").unwrap_or_default();
    config.window.title = "Component Sprites".to_string();
    config.window.width = 800;
    config.window.height = 600;

    let mut app = App::new(config);

    app.on_startup(|world, renderer| {
        let manifest =
            ResolvedManifest::load(MANIFEST_LOCAL).expect("Failed to load local manifest");
        asset_loader::load_sprites(&manifest, world, renderer)
            .expect("Failed to load example 03 sprites");

        // Centre camera at the origin; world-space positions below are
        // relative to screen centre via the default Camera2D.
        if let Some(cam) = world.get_resource_mut::<Camera2D>() {
            cam.zoom = 3.0;
            cam.position = Vec2::ZERO;
        }

        // --- Entity 1: spinning quad, z = 0, tagged "player" for the V toggle.
        let spinner = world.spawn();
        world.insert(
            spinner,
            Transform {
                position: Vec2::new(-60.0, -8.0),
                rotation: 0.0,
                scale: Vec2::ONE,
            },
        );
        world.insert(spinner, Sprite::new(QUAD_ID));
        world.insert(spinner, Visibility::default());
        world.insert(spinner, Spinner);
        world.insert(spinner, Tag::new("player"));

        // --- Entity 2: pulsing (scaling) quad.
        let pulser = world.spawn();
        world.insert(
            pulser,
            Transform {
                position: Vec2::new(-8.0, -8.0),
                rotation: 0.0,
                scale: Vec2::ONE,
            },
        );
        world.insert(pulser, Sprite::new(QUAD_ID));
        world.insert(pulser, Visibility::default());
        world.insert(pulser, Pulser { time: 0.0 });

        // --- Entity 3: tint-cycling quad.
        let tinter = world.spawn();
        world.insert(
            tinter,
            Transform {
                position: Vec2::new(44.0, -8.0),
                rotation: 0.0,
                scale: Vec2::ONE,
            },
        );
        world.insert(tinter, Sprite::new(QUAD_ID));
        world.insert(tinter, Visibility::default());
        world.insert(tinter, Tinter { time: 0.0 });

        // --- Entities 4-6: z-order stack. Three overlapping tinted quads with
        // z_order ∈ {-1, 0, 1}. The one at z = 1 sits on top.
        for (i, (z, tint, offset)) in [
            (-1, [255, 60, 60, 255], Vec2::new(0.0, 0.0)),
            (0, [60, 255, 60, 255], Vec2::new(6.0, 6.0)),
            (1, [60, 60, 255, 255], Vec2::new(12.0, 12.0)),
        ]
        .into_iter()
        .enumerate()
        {
            let e = world.spawn();
            world.insert(
                e,
                Transform {
                    position: Vec2::new(-24.0, 40.0) + offset,
                    rotation: 0.0,
                    scale: Vec2::ONE,
                },
            );
            world.insert(
                e,
                Sprite {
                    asset_id: QUAD_ID.to_string(),
                    color: tint,
                    z_order: z,
                },
            );
            world.insert(e, Visibility::default());
            world.insert(e, Tag::new(format!("stack_{i}")));
        }

        let registry = world
            .get_resource::<AssetRegistry>()
            .expect("AssetRegistry resource missing");
        assert!(
            registry.get_sprite(QUAD_ID).is_some(),
            "missing sprite '{QUAD_ID}'"
        );

        let player_tag_count = world
            .query::<Tag>()
            .filter(|(_, t)| t.name == "player")
            .count();
        log::info!("startup: tagged 'player' entity count = {player_tag_count}");
    });

    app.add_system_named("spin_system", spin_system);
    app.add_system_named("pulse_system", pulse_system);
    app.add_system_named("tint_system", tint_system);
    app.add_system_named("visibility_toggle_system", visibility_toggle_system);
    // No set_extract_sprites — engine installs extract_sprites_default (D-042).

    app.run()
}
