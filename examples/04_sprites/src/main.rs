use tungsten::asset_loader;
use tungsten::core::{AssetRegistry, Config, DeltaTime, ResolvedManifest, World};
use tungsten::render::{SpriteBatch, SpriteInstance};
use tungsten::App;

#[derive(Debug, Clone)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
struct SpriteId(String);

#[derive(Debug, Clone)]
struct Velocity {
    dx: f32,
    dy: f32,
}

fn movement_system(world: &mut World) {
    let dt = world.get_resource::<DeltaTime>().unwrap().seconds();
    let entities = world.query_entities::<Velocity>();
    for entity in entities {
        let vel = world.get::<Velocity>(entity).unwrap().clone();
        let pos = world.get_mut::<Position>(entity).unwrap();
        pos.x += vel.dx * dt;
        pos.y += vel.dy * dt;
    }
}

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let registry = match world.get_resource::<AssetRegistry>() {
        Some(r) => r,
        None => return vec![],
    };

    let mut batches: std::collections::HashMap<String, SpriteBatch> =
        std::collections::HashMap::new();

    for (entity, sprite_id) in world.query::<SpriteId>() {
        let pos = match world.get::<Position>(entity) {
            Some(p) => p,
            None => continue,
        };

        let sprite_asset = match registry.get_sprite(&sprite_id.0) {
            Some(a) => a,
            None => continue,
        };

        let batch = batches
            .entry(sprite_id.0.clone())
            .or_insert_with(|| SpriteBatch {
                texture: sprite_asset.texture,
                filter: sprite_asset.filter,
                instances: Vec::new(),
            });

        batch.instances.push(SpriteInstance {
            position: [pos.x, pos.y],
            size: [sprite_asset.width as f32, sprite_asset.height as f32],
        });
    }

    batches.into_values().collect()
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::load("tungsten.json")?;
    let mut app = App::new(config);

    // Spawn sprite entities (asset IDs, not file paths — per D-009)
    let world = app.world_mut();

    let e1 = world.spawn();
    world.insert(e1, Position { x: 100.0, y: 100.0 });
    world.insert(e1, SpriteId("red_square".into()));

    let e2 = world.spawn();
    world.insert(e2, Position { x: 300.0, y: 200.0 });
    world.insert(e2, SpriteId("blue_square".into()));
    world.insert(e2, Velocity { dx: 80.0, dy: 50.0 });

    let e3 = world.spawn();
    world.insert(e3, Position { x: 500.0, y: 300.0 });
    world.insert(e3, SpriteId("green_circle".into()));

    // Multiple instances of the same sprite
    for i in 0..5 {
        let e = world.spawn();
        world.insert(
            e,
            Position {
                x: 200.0 + i as f32 * 50.0,
                y: 450.0,
            },
        );
        world.insert(e, SpriteId("red_square".into()));
    }

    // Load assets after renderer is ready
    app.on_startup(|world, renderer| {
        let manifest =
            ResolvedManifest::load("assets/manifest.json").expect("Failed to load manifest");
        asset_loader::load_sprites(&manifest, world, renderer)
            .expect("Failed to load sprite assets");
    });

    app.add_system(movement_system);
    app.set_extract_sprites(extract_sprites);

    app.run()
}
