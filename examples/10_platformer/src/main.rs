//! Example 10 — 2D Physics (M11)
//!
//! A tiny side-scroller platformer that exercises the hand-rolled
//! collision system added in M11:
//!
//!   • AABB player driven by WASD / arrows for horizontal movement and
//!     Space to jump. Jumps are gated on a `Grounded` flag fed from the
//!     collision-event stream.
//!   • Three bouncing circles with non-zero restitution that collide
//!     with the tilemap, the player, and each other.
//!   • Gravity overridden on `PhysicsConfig` at startup; the engine's
//!     default (`Vec2::ZERO`) is top-down-friendly.
//!   • A tilemap `.tmj` with a dedicated `collision` layer read by the
//!     physics step as static tile AABBs.
//!   • Camera follows the player horizontally and is clamped to level
//!     bounds so the scrolling half of M10 stays exercised.
//!
//! Run from the workspace root:
//!
//!     cargo run -p example-10-platformer

use std::path::PathBuf;

use glam::Vec2;
use tungsten::asset_loader;
use tungsten::core::{
    AssetRegistry, Camera2D, Config, InputState, KeyCode, ResolvedManifest, TilemapInstance,
    TilemapRegistry, World,
};
use tungsten::physics::{
    physics_step, BodyKind, Collider, CollisionEvents, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};
use tungsten::{extract_tilemaps, App, WindowSize};

const MANIFEST_PATH: &str = "examples/10_platformer/assets/manifest.json";
const ASSETS_DIR: &str = "examples/10_platformer/assets";
const TILE: f32 = 16.0;
const MAP_COLS: u32 = 48;
const MAP_ROWS: u32 = 18;

const PLAYER_HALF: Vec2 = Vec2::new(6.0, 7.0);
const PLAYER_MOVE_SPEED: f32 = 140.0;
const PLAYER_JUMP_IMPULSE: f32 = 320.0;
const GRAVITY_Y: f32 = 900.0;
const BALL_RADIUS: f32 = 6.0;
const BALL_RESTITUTION: f32 = 0.85;

/// Marker + state for the player entity.
#[derive(Debug, Clone, Copy, Default)]
struct Player {
    grounded: bool,
}

/// Marker for the bouncing circles.
#[derive(Debug, Clone, Copy)]
struct Ball;

/// Horizontal input, gravity-aware jump on the player. Runs BEFORE
/// `physics_step` so the velocity changes it makes are integrated in
/// the same frame.
fn player_input(world: &mut World) {
    let input = match world.get_resource::<InputState>() {
        Some(i) => i.clone(),
        None => return,
    };

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();

    for entity in player_entities {
        let mut dx = 0.0f32;
        if input.is_pressed(KeyCode::ArrowLeft) || input.is_pressed(KeyCode::KeyA) {
            dx -= 1.0;
        }
        if input.is_pressed(KeyCode::ArrowRight) || input.is_pressed(KeyCode::KeyD) {
            dx += 1.0;
        }

        let grounded = world
            .get::<Player>(entity)
            .map(|p| p.grounded)
            .unwrap_or(false);
        let want_jump = input.is_pressed(KeyCode::Space) && grounded;

        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0.x = dx * PLAYER_MOVE_SPEED;
            if want_jump {
                vel.0.y = -PLAYER_JUMP_IMPULSE;
            }
        }

        // Consume the grounded flag every frame; it gets re-set by
        // `ground_detection` after the physics step resolves contacts.
        if let Some(player) = world.get_mut::<Player>(entity) {
            player.grounded = false;
        }
    }
}

/// Scan `CollisionEvents` after `physics_step` and flag the player as
/// grounded whenever it received an upward-pointing contact (`normal.y
/// < -0.5`). This is intentionally game-code, not library-code: it
/// demonstrates how gameplay systems consume the event stream.
fn ground_detection(world: &mut World) {
    let events = match world.get_resource::<CollisionEvents>() {
        Some(e) => e.events.clone(),
        None => return,
    };

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();

    for entity in player_entities {
        let grounded = events.iter().any(|ev| ev.a == entity && ev.normal.y < -0.5);
        if let Some(player) = world.get_mut::<Player>(entity) {
            if grounded {
                player.grounded = true;
            }
        }
    }
}

/// Pin the camera to the player with a horizontal lead, clamped to the
/// level bounds so it doesn't show empty space past the walls.
fn camera_follow(world: &mut World) {
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 480,
            height: 288,
        });
    let map_w = (MAP_COLS as f32) * TILE;
    let map_h = (MAP_ROWS as f32) * TILE;
    let max_x = (map_w - window.width as f32).max(0.0);
    let max_y = (map_h - window.height as f32).max(0.0);

    let player_pos = world
        .query::<Player>()
        .next()
        .and_then(|(e, _)| world.get::<Position>(e).copied())
        .map(|p| p.0);

    let Some(player) = player_pos else { return };

    let target_x = player.x - window.width as f32 * 0.5;
    let target_y = player.y - window.height as f32 * 0.6;

    if let Some(camera) = world.get_resource_mut::<Camera2D>() {
        camera.position.x = target_x.clamp(0.0, max_x);
        camera.position.y = target_y.clamp(0.0, max_y);
    }
}

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let mut batches = extract_tilemaps(world);
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return batches;
    };

    // Player sprite (one per player entity).
    if let Some(player_asset) = assets.get_sprite("ex10_player") {
        let instances: Vec<SpriteInstance> = world
            .query::<Player>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .map(|p| SpriteInstance {
                position: [p.0.x - PLAYER_HALF.x, p.0.y - PLAYER_HALF.y],
                size: [PLAYER_HALF.x * 2.0, PLAYER_HALF.y * 2.0],
            })
            .collect();
        if !instances.is_empty() {
            batches.push(SpriteBatch {
                texture: player_asset.texture,
                filter: player_asset.filter,
                instances,
            });
        }
    }

    // Ball sprites.
    if let Some(ball_asset) = assets.get_sprite("ex10_ball") {
        let instances: Vec<SpriteInstance> = world
            .query::<Ball>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .map(|p| SpriteInstance {
                position: [p.0.x - BALL_RADIUS, p.0.y - BALL_RADIUS],
                size: [BALL_RADIUS * 2.0, BALL_RADIUS * 2.0],
            })
            .collect();
        if !instances.is_empty() {
            batches.push(SpriteBatch {
                texture: ball_asset.texture,
                filter: ball_asset.filter,
                instances,
            });
        }
    }

    batches
}

fn extract_text(world: &World) -> Vec<TextSection> {
    let events = world
        .get_resource::<CollisionEvents>()
        .map(|e| e.len())
        .unwrap_or(0);
    let grounded = world
        .query::<Player>()
        .next()
        .map(|(_, p)| p.grounded)
        .unwrap_or(false);

    vec![
        TextSection {
            content: "Physics Playground (M11)".into(),
            font_id: "ex10_sans_bold".into(),
            font_size: 22.0,
            line_height: 26.0,
            color: [255, 255, 255, 255],
            position: [16.0, 12.0],
            bounds: None,
        },
        TextSection {
            content: format!(
                "A/D to move, Space to jump    grounded: {}    contacts: {}\n\
                 Bouncing circles show circle↔AABB and circle↔tile.",
                if grounded { "yes" } else { "no" },
                events,
            ),
            font_id: "ex10_sans".into(),
            font_size: 13.0,
            line_height: 18.0,
            color: [220, 230, 255, 255],
            position: [16.0, 42.0],
            bounds: None,
        },
    ]
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    config.window.width = 480;
    config.window.height = 288;
    let mut app = App::new(config);

    app.enable_hot_reload(PathBuf::from(ASSETS_DIR), PathBuf::from(MANIFEST_PATH));

    // Override physics for a side-scrolling gravity world.
    {
        let world = app.world_mut();
        if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
            cfg.gravity = Vec2::new(0.0, GRAVITY_Y);
            cfg.broadphase_cell_size = 32.0;
        }

        // Tilemap at world origin — provides the static ground + walls.
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

        // Player: spawn on top of the ground a few tiles in.
        let player = world.spawn();
        world.insert(player, Player::default());
        world.insert(player, Position(Vec2::new(3.0 * TILE + 8.0, 13.0 * TILE)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic().with_restitution(0.0));

        // Three bouncy balls at different spots on the level.
        for (i, spawn_x) in [8.0, 20.0, 32.0].iter().enumerate() {
            let ball = world.spawn();
            world.insert(ball, Ball);
            world.insert(
                ball,
                Position(Vec2::new(spawn_x * TILE, 3.0 * TILE + i as f32 * 8.0)),
            );
            world.insert(ball, Velocity(Vec2::new(60.0 - i as f32 * 45.0, 0.0)));
            world.insert(ball, Collider::circle(BALL_RADIUS));
            world.insert(
                ball,
                RigidBody {
                    kind: BodyKind::Dynamic,
                    inv_mass: 1.0,
                    restitution: BALL_RESTITUTION,
                },
            );
        }
    }

    app.on_startup(|world, renderer| {
        let manifest = ResolvedManifest::load(MANIFEST_PATH).expect("Failed to load manifest");
        asset_loader::load_all(&manifest, world, renderer).expect("Failed to load assets");

        let registry = world.get_resource::<AssetRegistry>().unwrap();
        for id in [
            "ex10_ground",
            "ex10_platform",
            "ex10_sky",
            "ex10_player",
            "ex10_ball",
        ] {
            assert!(registry.get_sprite(id).is_some(), "missing sprite '{id}'");
        }
        let tilemaps = world.get_resource::<TilemapRegistry>().unwrap();
        assert!(tilemaps.get("ex10_level").is_some(), "missing tilemap");
    });

    // Ordering matters here: input sets intents; physics resolves them;
    // ground_detection reads post-step events; camera reads final pos.
    app.add_system(player_input);
    app.add_system(physics_step);
    app.add_system(ground_detection);
    app.add_system(camera_follow);
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);

    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
    use tungsten::core::DeltaTime;

    fn seed_world() -> World {
        let mut world = World::new();
        world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
        world.insert_resource(InputState::new());
        world.insert_resource(CollisionEvents::new());
        world.insert_resource(PhysicsConfig {
            gravity: Vec2::new(0.0, GRAVITY_Y),
            ..PhysicsConfig::default()
        });
        world.insert_resource(TilemapRegistry::new());
        world.insert_resource(Camera2D::new());
        world.insert_resource(WindowSize {
            width: 480,
            height: 288,
        });
        world
    }

    fn solid_floor(width: u32) -> TilemapData {
        let mut tiles = vec![-1i32; (width as usize) * 2];
        for x in 0..width as usize {
            tiles[width as usize + x] = 0;
        }
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width,
            height: 2,
            tileset: vec!["ex10_ground".into()],
            layers: vec![TilemapLayer {
                name: "collision".into(),
                kind: LayerKind::Collision,
                tiles,
            }],
        }
    }

    #[test]
    fn player_becomes_grounded_after_falling_onto_tilemap() {
        let mut world = seed_world();
        world
            .get_resource_mut::<TilemapRegistry>()
            .unwrap()
            .insert("ex10_level".into(), solid_floor(8));
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

        let player = world.spawn();
        world.insert(player, Player::default());
        // Start just above the floor so a single 60Hz step lands it.
        world.insert(player, Position(Vec2::new(40.0, 8.0)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic());

        // Run a few frames so gravity pulls the player onto the floor.
        for _ in 0..20 {
            player_input(&mut world);
            physics_step(&mut world);
            ground_detection(&mut world);
        }

        let p = world.get::<Player>(player).unwrap();
        assert!(p.grounded, "player did not become grounded");
    }

    #[test]
    fn jump_impulse_only_applies_when_grounded() {
        let mut world = seed_world();
        world
            .get_resource_mut::<TilemapRegistry>()
            .unwrap()
            .insert("ex10_level".into(), solid_floor(8));
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

        let player = world.spawn();
        world.insert(
            player,
            Player {
                grounded: false, // Airborne — space should do nothing.
            },
        );
        world.insert(player, Position(Vec2::new(40.0, 40.0)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic());

        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .key_down(KeyCode::Space);

        player_input(&mut world);

        let vel = world.get::<Velocity>(player).unwrap().0;
        assert!(
            vel.y >= 0.0,
            "jump fired while airborne — should be gated: {:?}",
            vel
        );
    }
}
