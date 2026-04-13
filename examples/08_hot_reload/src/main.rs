//! Example 08 — Hot Reload
//!
//! Demonstrates live asset reloading. While this example is running:
//!
//!   • Edit `assets/sprites/red_square.png` — the sprite updates within a few frames.
//!   • Edit `assets/animations/walk.json` — the walk cycle updates live.
//!   • Edit `assets/fonts/Inter/static/Inter-Regular.ttf` — the text label changes font.
//!   • Add/remove entries in `assets/manifest.json` — new assets load immediately;
//!     removed entries log a warning and stay stale.
//!
//! No restart needed for any of the above.

use std::path::PathBuf;

use tungsten::asset_loader;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, Config, DeltaTime, ResolvedManifest, World,
};
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};
use tungsten::{App, WindowSize};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Position {
    x: f32,
    y: f32,
}

/// The sprite ID currently displayed for an animated entity.
#[derive(Debug, Clone)]
struct CurrentSprite(String);

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn animation_system(world: &mut World) {
    let dt_ms = world.get_resource::<DeltaTime>().unwrap().seconds() * 1000.0;

    let anim_registry = match world.get_resource::<AnimationRegistry>() {
        Some(r) => r.clone(),
        None => return,
    };

    let entities = world.query_entities::<AnimationState>();
    for entity in entities {
        let mut state = world.get::<AnimationState>(entity).unwrap().clone();
        let new_sprite = state.advance(dt_ms, &anim_registry);
        *world.get_mut::<AnimationState>(entity).unwrap() = state;

        if let Some(sprite_id) = new_sprite {
            if let Some(cs) = world.get_mut::<CurrentSprite>(entity) {
                cs.0 = sprite_id;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Extract functions
// ---------------------------------------------------------------------------

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let registry = match world.get_resource::<AssetRegistry>() {
        Some(r) => r,
        None => return vec![],
    };

    let mut batches: std::collections::HashMap<String, SpriteBatch> =
        std::collections::HashMap::new();

    for (entity, current_sprite) in world.query::<CurrentSprite>() {
        let pos = match world.get::<Position>(entity) {
            Some(p) => p,
            None => continue,
        };
        let asset = match registry.get_sprite(&current_sprite.0) {
            Some(a) => a,
            None => continue,
        };

        const SCALE: f32 = 4.0;
        let batch = batches
            .entry(current_sprite.0.clone())
            .or_insert_with(|| SpriteBatch {
                texture: asset.texture,
                filter: asset.filter,
                instances: Vec::new(),
            });
        batch.instances.push(SpriteInstance {
            position: [pos.x, pos.y],
            size: [asset.width as f32 * SCALE, asset.height as f32 * SCALE],
        });
    }

    batches.into_values().collect()
}

fn extract_text(world: &World) -> Vec<TextSection> {
    let ws = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });

    vec![
        TextSection {
            content: "Hot Reload Demo".into(),
            font_id: "sans_bold".into(),
            font_size: 40.0,
            line_height: 48.0,
            color: [255, 255, 255, 255],
            position: [30.0, 20.0],
            bounds: None,
        },
        TextSection {
            content: format!(
                "Edit these files while running (no restart):\n\
                 \n\
                 assets/sprites/red_square.png   → static sprite (top-right)\n\
                 assets/animations/walk.json     → walk cycle (bottom row)\n\
                 assets/fonts/Inter/static/Inter-Regular.ttf  → this text\n\
                 assets/manifest.json            → add/remove entries"
            ),
            font_id: "sans".into(),
            font_size: 16.0,
            line_height: 24.0,
            color: [200, 220, 255, 255],
            position: [30.0, 80.0],
            bounds: Some([ws.width as f32 - 60.0, 200.0]),
        },
    ]
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);

    // Enable hot reload — watches assets/ and reacts to manifest.json changes.
    app.enable_hot_reload(
        PathBuf::from("assets"),
        PathBuf::from("assets/manifest.json"),
    );

    // Spawn a static sprite in the upper-right area.
    {
        let world = app.world_mut();
        let e = world.spawn();
        world.insert(e, Position { x: 900.0, y: 80.0 });
        world.insert(e, CurrentSprite("red_square".into()));
    }

    // Spawn a row of animated walk-cycle entities.
    {
        let world = app.world_mut();
        for i in 0..6 {
            let e = world.spawn();
            world.insert(
                e,
                Position {
                    x: 80.0 + i as f32 * 160.0,
                    y: 380.0,
                },
            );
            world.insert(e, AnimationState::new("walk"));
            world.insert(e, CurrentSprite("walk_0".into()));
        }
    }

    app.on_startup(|world, renderer| {
        let manifest =
            ResolvedManifest::load("assets/manifest.json").expect("Failed to load manifest");
        asset_loader::load_all(&manifest, world, renderer).expect("Failed to load assets");
    });

    app.add_system(animation_system);
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);

    app.run()
}
