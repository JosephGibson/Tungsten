use tungsten::asset_loader;
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, Config, DeltaTime, ResolvedManifest, World,
};
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::App;

#[derive(Debug, Clone)]
struct Position {
    x: f32,
    y: f32,
}

/// The sprite currently displayed for this entity. Updated by the animation system.
#[derive(Debug, Clone)]
struct CurrentSprite(String);

fn animation_system(world: &mut World) {
    let dt = world.get_resource::<DeltaTime>().unwrap().seconds();
    let dt_ms = dt * 1000.0;

    // Clone the registry so we can mutate components freely.
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

        let sprite_asset = match registry.get_sprite(&current_sprite.0) {
            Some(a) => a,
            None => continue,
        };

        let scale = 4.0;

        let batch = batches
            .entry(current_sprite.0.clone())
            .or_insert_with(|| SpriteBatch {
                texture: sprite_asset.texture,
                filter: sprite_asset.filter,
                instances: Vec::new(),
            });

        batch.instances.push(SpriteInstance {
            position: [pos.x, pos.y],
            size: [
                sprite_asset.width as f32 * scale,
                sprite_asset.height as f32 * scale,
            ],
        });
    }

    batches.into_values().collect()
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);

    let world = app.world_mut();

    // Animated entities displaying a walk cycle at different positions
    for i in 0..6 {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: 100.0 + i as f32 * 150.0,
                y: 300.0,
            },
        );
        world.insert(e, AnimationState::new("walk"));
        world.insert(e, CurrentSprite("walk_0".into()));
    }

    // Static sprite for comparison
    let static_sprite = world.spawn();
    world.insert(static_sprite, Position { x: 500.0, y: 100.0 });
    world.insert(static_sprite, CurrentSprite("red_square".into()));

    app.on_startup(|world, renderer| {
        let manifest =
            ResolvedManifest::load("assets/manifest.json").expect("Failed to load manifest");
        asset_loader::load_all(&manifest, world, renderer).expect("Failed to load assets");
    });

    app.add_system(animation_system);
    app.set_extract_sprites(extract_sprites);

    app.run()
}
