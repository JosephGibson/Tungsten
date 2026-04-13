//! Example 09 — Tilemaps (M10)
//!
//! Demonstrates:
//!
//!   • Loading a multi-layer tilemap from a `.tmj` JSON file referenced
//!     by the manifest.
//!   • Camera scrolling with WASD / arrow keys — pan a viewport across
//!     a map larger than the window.
//!   • Per-layer render order (ground, then decorations on top).
//!   • Text HUD that stays screen-space while the world scrolls.
//!   • Hot reload: edit `assets/tilemaps/demo.tmj` while the example is
//!     running and the map updates within a frame.
//!
//! Run from the workspace root:
//!
//!     cargo run -p example-09-tilemap
//!
//! The map contains a non-rendering `collision` layer that is loaded
//! and validated but ignored by the renderer. M11 (2D physics) will
//! start consuming it.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::asset_loader;
use tungsten::core::{
    AssetRegistry, Camera2D, Config, DeltaTime, InputState, KeyCode, ResolvedManifest,
    TilemapInstance, TilemapRegistry, World,
};
use tungsten::render::{SpriteBatch, TextSection};
use tungsten::{extract_tilemaps, App, WindowSize};

const PAN_SPEED_PX_PER_SEC: f32 = 280.0;
const MANIFEST_PATH: &str = "examples/09_tilemap/assets/manifest.json";
const ASSETS_DIR: &str = "examples/09_tilemap/assets";

fn camera_system(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(0.0);
    let input = match world.get_resource::<InputState>() {
        Some(i) => i.clone(),
        None => return,
    };

    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
    if input.is_pressed(KeyCode::ArrowLeft) || input.is_pressed(KeyCode::KeyA) {
        dx -= 1.0;
    }
    if input.is_pressed(KeyCode::ArrowRight) || input.is_pressed(KeyCode::KeyD) {
        dx += 1.0;
    }
    if input.is_pressed(KeyCode::ArrowUp) || input.is_pressed(KeyCode::KeyW) {
        dy -= 1.0;
    }
    if input.is_pressed(KeyCode::ArrowDown) || input.is_pressed(KeyCode::KeyS) {
        dy += 1.0;
    }

    if dx == 0.0 && dy == 0.0 {
        return;
    }

    // Normalize diagonal movement so the camera doesn't move faster
    // along the diagonal than along an axis.
    let len = (dx * dx + dy * dy).sqrt();
    dx /= len;
    dy /= len;

    let step = PAN_SPEED_PX_PER_SEC * dt;

    if let Some(camera) = world.get_resource_mut::<Camera2D>() {
        camera.position.x += dx * step;
        camera.position.y += dy * step;

        // Clamp to roughly the map bounds so the player can't scroll
        // into empty space forever. Uses the demo tilemap size hardcoded
        // here — cheap enough and the map only exists in this example.
        const MAP_PX_W: f32 = 48.0 * 16.0;
        const MAP_PX_H: f32 = 30.0 * 16.0;
        const VIEW_W: f32 = 1280.0; // matches Config default; over-clamping is fine
        const VIEW_H: f32 = 720.0;
        let max_x: f32 = if MAP_PX_W > VIEW_W {
            MAP_PX_W - VIEW_W
        } else {
            0.0
        };
        let max_y: f32 = if MAP_PX_H > VIEW_H {
            MAP_PX_H - VIEW_H
        } else {
            0.0
        };
        camera.position.x = camera.position.x.clamp(0.0, max_x);
        camera.position.y = camera.position.y.clamp(0.0, max_y);
    }
}

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    // Only thing in this example is the tilemap, so the sprite batches
    // are exactly the tile batches. Mixing in entity sprites would just
    // be a second call + `.extend`.
    extract_tilemaps(world)
}

fn extract_text(world: &World) -> Vec<TextSection> {
    let ws = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    let camera = world
        .get_resource::<Camera2D>()
        .copied()
        .unwrap_or_default();

    let map_info = world
        .get_resource::<TilemapRegistry>()
        .and_then(|r| r.get("ex09_demo"))
        .map(|d| {
            format!(
                "{}x{} tiles @ {}px, {} layers",
                d.width,
                d.height,
                d.tile_width,
                d.layers.len()
            )
        })
        .unwrap_or_else(|| "<no map>".into());

    vec![
        TextSection {
            content: "Tilemap Demo (M10)".into(),
            font_id: "ex09_sans_bold".into(),
            font_size: 36.0,
            line_height: 44.0,
            color: [255, 255, 255, 255],
            position: [24.0, 18.0],
            bounds: None,
        },
        TextSection {
            content: format!(
                "WASD / arrows to pan    camera: ({:.0}, {:.0})    {}\n\
                 Edit assets/tilemaps/demo.tmj — hot reload is live.",
                camera.position.x, camera.position.y, map_info,
            ),
            font_id: "ex09_sans".into(),
            font_size: 16.0,
            line_height: 22.0,
            color: [210, 225, 255, 255],
            position: [24.0, 66.0],
            bounds: Some([ws.width as f32 - 48.0, 80.0]),
        },
    ]
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);

    // Watch the example-local assets directory. Manifest hot reload
    // works the same way as in example-08.
    app.enable_hot_reload(PathBuf::from(ASSETS_DIR), PathBuf::from(MANIFEST_PATH));

    // Spawn one tilemap entity anchored at world origin.
    {
        let world = app.world_mut();
        let e = world.spawn();
        world.insert(e, TilemapInstance::new("ex09_demo", Vec2::ZERO));
    }

    app.on_startup(|world, renderer| {
        let manifest = ResolvedManifest::load(MANIFEST_PATH).expect("Failed to load manifest");
        asset_loader::load_all(&manifest, world, renderer).expect("Failed to load assets");

        // Sanity check: the tilemap's tileset sprites are loaded.
        let registry = world.get_resource::<AssetRegistry>().unwrap();
        for id in [
            "ex09_grass",
            "ex09_dirt",
            "ex09_stone",
            "ex09_water",
            "ex09_flower",
        ] {
            assert!(registry.get_sprite(id).is_some(), "missing sprite '{id}'");
        }
    });

    app.add_system(camera_system);
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);

    app.run()
}
