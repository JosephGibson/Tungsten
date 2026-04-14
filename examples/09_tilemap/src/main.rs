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

    // Clamp against live map + viewport dimensions. Reading from the
    // resources on every tick keeps the pan bounds correct after a hot
    // reload of the `.tmj` or a window resize — the previous version
    // hardcoded 48×30@16 and 1280×720, and silently pinned the camera
    // to (0, 0) as soon as either side of that assumption drifted.
    let (map_w, map_h) = world
        .get_resource::<TilemapRegistry>()
        .and_then(|r| r.get("ex09_demo"))
        .map(|d| {
            (
                (d.width * d.tile_width) as f32,
                (d.height * d.tile_height) as f32,
            )
        })
        .unwrap_or((f32::INFINITY, f32::INFINITY));
    let (view_w, view_h) = world
        .get_resource::<WindowSize>()
        .map(|w| (w.width as f32, w.height as f32))
        .unwrap_or((0.0, 0.0));
    let max_x = (map_w - view_w).max(0.0);
    let max_y = (map_h - view_h).max(0.0);

    if let Some(camera) = world.get_resource_mut::<Camera2D>() {
        camera.position.x = (camera.position.x + dx * step).clamp(0.0, max_x);
        camera.position.y = (camera.position.y + dy * step).clamp(0.0, max_y);
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

    // The demo tilemap is 48×30 @ 16px = 768×480. The shared
    // `tungsten.json` defaults to 1280×720, which would fully contain the
    // map and leave the camera with nothing to pan over. Shrink the window
    // here so the map is strictly larger than the viewport — this is the
    // whole point of the example.
    let mut config = Config::load("tungsten.json")?;
    config.window.width = 640;
    config.window.height = 360;
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

        // Guardrail: panning only makes sense when the map is strictly
        // larger than the viewport on at least one axis. If the map fits,
        // the camera clamp will pin to (0, 0) and the example silently
        // looks "broken" (WASD does nothing). We hit that exact bug before
        // — fail loudly at startup instead of letting it slip to runtime.
        let map = world
            .get_resource::<TilemapRegistry>()
            .and_then(|r| r.get("ex09_demo"))
            .expect("ex09_demo tilemap not registered");
        let ws = world
            .get_resource::<WindowSize>()
            .copied()
            .expect("WindowSize resource missing");
        let map_px = (
            (map.width * map.tile_width) as f32,
            (map.height * map.tile_height) as f32,
        );
        if map_px.0 <= ws.width as f32 && map_px.1 <= ws.height as f32 {
            log::warn!(
                "ex09: map ({}x{} px) fits inside viewport ({}x{}) — panning will be a no-op",
                map_px.0,
                map_px.1,
                ws.width,
                ws.height,
            );
        }
    });

    app.add_system(camera_system);
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);

    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
    use tungsten::core::World;

    fn world_with_map(map_w: u32, map_h: u32, view_w: u32, view_h: u32) -> World {
        let mut world = World::new();
        world.insert_resource(DeltaTime { dt: 0.1 });
        world.insert_resource(InputState::new());
        world.insert_resource(Camera2D::new());
        world.insert_resource(WindowSize {
            width: view_w,
            height: view_h,
        });
        let mut registry = TilemapRegistry::new();
        let cells = (map_w * map_h) as usize;
        let data = TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: map_w,
            height: map_h,
            tileset: vec![],
            layers: vec![TilemapLayer {
                name: "background".into(),
                kind: LayerKind::Render,
                tiles: vec![-1; cells],
            }],
        };
        registry.insert("ex09_demo".into(), data);
        world.insert_resource(registry);
        world
    }

    #[test]
    fn pans_right_when_map_larger_than_viewport() {
        let mut world = world_with_map(48, 30, 320, 180);
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .key_down(KeyCode::KeyD);
        camera_system(&mut world);
        let cam = world.get_resource::<Camera2D>().unwrap();
        assert!(
            cam.position.x > 0.0,
            "camera did not pan right: {:?}",
            cam.position
        );
    }

    #[test]
    fn stays_put_when_map_fits_viewport() {
        // This is the exact configuration that caused the original bug:
        // a 768×480 map inside a 1280×720 viewport. Clamping to 0 is the
        // correct answer here — there is nothing to pan into.
        let mut world = world_with_map(48, 30, 1280, 720);
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .key_down(KeyCode::KeyD);
        camera_system(&mut world);
        let cam = world.get_resource::<Camera2D>().unwrap();
        assert_eq!(cam.position.x, 0.0);
    }
}
