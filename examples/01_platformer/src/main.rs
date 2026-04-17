//! Example 01 — Platformer
//!
//! Single project that exercises all engine features:
//!   ECS, physics (AABB + circles + tilemap collision), sprites, animation,
//!   audio, text, camera follow, input, and hot reload.
//!
//! Controls:
//!   A / D or ←/→   horizontal movement
//!   Space           jump (when grounded; plays a sound effect)
//!   M               toggle background music
//!   1 / 2 / 3       master volume: 20% / 50% / 100%
//!   S               stop all sounds
//!   = / -           zoom in / zoom out (50%–200% of base)
//!
//! Two manifests are loaded at startup:
//!   • assets/manifest.json        — fonts, walk animation, sounds (shared root)
//!   • examples/01_platformer/assets/manifest.json — tilemap + tile sprites (local)
//!
//! Hot reload watches the local assets directory; edit level.tmj while running
//! and the map updates within a frame.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::asset_loader;
use tungsten::core::{
    sync_position_to_transform, AnimationRegistry, AnimationState, AssetRegistry, AudioCommands,
    AudioHandle, CameraBounds, CameraController, CameraMode, CameraState, Config, DeltaTime,
    Entity, EventQueue, InputState, KeyCode, ResolvedManifest, SoundRegistry, TilemapInstance,
    TilemapRegistry, Transform, World,
};
use tungsten::physics::{
    physics_step, BodyKind, Collider, CollisionEvent, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};
use tungsten::{camera_update_system, extract_tilemaps, App, WindowSize};

const MANIFEST_ROOT: &str = "assets/manifest.json";
const MANIFEST_LOCAL: &str = "examples/01_platformer/assets/manifest.json";
const ASSETS_ROOT: &str = "assets";
const ASSETS_LOCAL: &str = "examples/01_platformer/assets";

const TILE: f32 = 16.0;
const MAP_COLS: u32 = 48;
const MAP_ROWS: u32 = 18;

/// How often the HUD values (FPS, contacts, etc.) are refreshed in seconds.
/// Values are cached between refreshes so they don't flicker every frame.
const TEXT_UPDATE_INTERVAL: f32 = 0.25;

const PLAYER_HALF: Vec2 = Vec2::new(6.0, 7.0);
const PLAYER_MOVE_SPEED: f32 = 140.0;
const PLAYER_JUMP_IMPULSE: f32 = 320.0;
const GRAVITY_Y: f32 = 900.0;
const BALL_RADIUS: f32 = 6.0;
const BALL_RESTITUTION: f32 = 0.85;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

struct AudioState {
    sfx_handle: AudioHandle,
    music_handle: AudioHandle,
    sfx_volume: f32,
    music_volume: f32,
    music_playing: bool,
    master_volume: f32,
}

/// Cached HUD values, updated at `TEXT_UPDATE_INTERVAL` by `update_text_display`.
/// Avoids rebuilding the text layout every frame and stops numbers from flickering.
struct TextDisplayState {
    fps: u32,
    contacts: usize,
    grounded: bool,
    music_on: bool,
    vol_pct: u32,
    zoom_pct: u32,
    timer: f32,
}

impl Default for TextDisplayState {
    fn default() -> Self {
        Self {
            fps: 0,
            contacts: 0,
            grounded: false,
            music_on: false,
            vol_pct: 50,
            zoom_pct: 100,
            timer: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Player marker + grounded flag. Reset to false each frame by `player_input`;
/// re-set to true by `ground_detection` after the physics step resolves contacts.
#[derive(Debug, Clone, Copy, Default)]
struct Player {
    grounded: bool,
}

/// Marker for the bouncing circle entities.
#[derive(Debug, Clone, Copy)]
struct Ball;

/// Current sprite frame driven by `AnimationState` — updated by `animation_system`.
#[derive(Debug, Clone)]
struct CurrentSprite(String);

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Horizontal movement + jump. Plays the jump SFX when the player leaves the
/// ground. Runs BEFORE `physics_step` so velocity changes are integrated in
/// the same frame.
fn player_input(world: &mut World) {
    let (pressed_left, pressed_right, pressed_space);
    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        pressed_left = input.is_pressed(KeyCode::ArrowLeft) || input.is_pressed(KeyCode::KeyA);
        pressed_right = input.is_pressed(KeyCode::ArrowRight) || input.is_pressed(KeyCode::KeyD);
        pressed_space = input.is_pressed(KeyCode::Space);
    }

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    let mut did_jump = false;

    for entity in player_entities {
        let mut dx = 0.0f32;
        if pressed_left {
            dx -= 1.0;
        }
        if pressed_right {
            dx += 1.0;
        }

        let grounded = world
            .get::<Player>(entity)
            .map(|p| p.grounded)
            .unwrap_or(false);
        let want_jump = pressed_space && grounded;

        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0.x = dx * PLAYER_MOVE_SPEED;
            if want_jump {
                vel.0.y = -PLAYER_JUMP_IMPULSE;
                did_jump = true;
            }
        }

        // Consume the grounded flag; `ground_detection` will re-set it below.
        if let Some(player) = world.get_mut::<Player>(entity) {
            player.grounded = false;
        }
    }

    if did_jump {
        let handle_and_vol = world
            .get_resource::<AudioState>()
            .map(|s| (s.sfx_handle, s.sfx_volume));
        if let Some((handle, vol)) = handle_and_vol {
            if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
                cmds.play_with(handle, vol, false);
            }
        }
    }
}

/// Music toggle (M), volume steps (1/2/3), stop-all (S).
fn audio_input_system(world: &mut World) {
    let (just_m, just_1, just_2, just_3, just_s);
    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        just_m = input.just_pressed(KeyCode::KeyM);
        just_1 = input.just_pressed(KeyCode::Digit1);
        just_2 = input.just_pressed(KeyCode::Digit2);
        just_3 = input.just_pressed(KeyCode::Digit3);
        just_s = input.just_pressed(KeyCode::KeyS);
    }

    if world.get_resource::<AudioState>().is_none() {
        return;
    }

    {
        let state = world.get_resource_mut::<AudioState>().unwrap();
        if just_1 {
            state.master_volume = 0.2;
        }
        if just_2 {
            state.master_volume = 0.5;
        }
        if just_3 {
            state.master_volume = 1.0;
        }
        if just_m {
            state.music_playing = !state.music_playing;
        }
        if just_s {
            state.music_playing = false;
        }
    }

    let (music_handle, music_volume, music_playing, master_volume) = {
        let state = world.get_resource::<AudioState>().unwrap();
        (
            state.music_handle,
            state.music_volume,
            state.music_playing,
            state.master_volume,
        )
    };

    let cmds = world.get_resource_mut::<AudioCommands>().unwrap();
    if just_1 || just_2 || just_3 {
        cmds.set_master_volume(master_volume);
    }
    if just_s {
        cmds.stop_all();
    }
    if just_m {
        if music_playing {
            cmds.play_with(music_handle, music_volume, true);
        } else {
            cmds.stop(music_handle);
        }
    }
}

/// = / - to zoom in or out. Adjusts `CameraController.zoom_multiplier` in 25% steps,
/// clamped to [0.5, 2.0] relative to the base window-height zoom.
fn camera_zoom_input_system(world: &mut World) {
    let (just_equal, just_minus);
    {
        let input = match world.get_resource::<InputState>() {
            Some(i) => i,
            None => return,
        };
        just_equal = input.just_pressed(KeyCode::Equal);
        just_minus = input.just_pressed(KeyCode::Minus);
    }
    if !just_equal && !just_minus {
        return;
    }
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        if just_equal {
            controller.zoom_multiplier = (controller.zoom_multiplier + 0.25).min(2.0);
        }
        if just_minus {
            controller.zoom_multiplier = (controller.zoom_multiplier - 0.25).max(0.5);
        }
    }
}

/// Advances `AnimationState` and writes the resulting sprite ID into `CurrentSprite`.
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

/// Scans `EventQueue<CollisionEvent>` and flags the player as grounded on an upward contact.
fn ground_detection(world: &mut World) {
    let events: Vec<CollisionEvent> = match world.get_resource::<EventQueue<CollisionEvent>>() {
        Some(queue) => queue.iter().copied().collect(),
        None => return,
    };
    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    for entity in player_entities {
        if events.iter().any(|ev| ev.a == entity && ev.normal.y < -0.5) {
            if let Some(player) = world.get_mut::<Player>(entity) {
                player.grounded = true;
            }
        }
    }
}

/// Refreshes `TextDisplayState` at `TEXT_UPDATE_INTERVAL` so HUD values
/// update at a readable rate instead of every frame.
fn update_text_display(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(0.0);

    let timer = world
        .get_resource::<TextDisplayState>()
        .map(|s| s.timer)
        .unwrap_or(0.0);
    let new_timer = timer + dt;

    if new_timer < TEXT_UPDATE_INTERVAL {
        if let Some(state) = world.get_resource_mut::<TextDisplayState>() {
            state.timer = new_timer;
        }
        return;
    }

    let fps = if dt > 0.0 {
        (1.0 / dt).round() as u32
    } else {
        0
    };
    let contacts = world
        .get_resource::<EventQueue<CollisionEvent>>()
        .map(|queue| queue.len())
        .unwrap_or(0);
    let grounded = world
        .query::<Player>()
        .next()
        .map(|(_, p)| p.grounded)
        .unwrap_or(false);
    let (music_on, vol_pct) = world
        .get_resource::<AudioState>()
        .map(|s| (s.music_playing, (s.master_volume * 100.0).round() as u32))
        .unwrap_or((false, 0));
    let zoom_pct = world
        .get_resource::<CameraController>()
        .map(|controller| (controller.zoom_multiplier * 100.0).round() as u32)
        .unwrap_or(100);

    if let Some(state) = world.get_resource_mut::<TextDisplayState>() {
        state.fps = fps;
        state.contacts = contacts;
        state.grounded = grounded;
        state.music_on = music_on;
        state.vol_pct = vol_pct;
        state.zoom_pct = zoom_pct;
        state.timer = new_timer - TEXT_UPDATE_INTERVAL;
    }
}

/// Recomputes the platformer's base zoom from the current window height.
/// `camera_update_system` multiplies this by `CameraController.zoom_multiplier`
/// before producing the authoritative `CameraState` for the frame.
fn platformer_camera_base_zoom(world: &mut World) {
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1920,
            height: 1080,
        });
    let map_h = (MAP_ROWS as f32) * TILE;
    let base_zoom = (window.height as f32 / map_h).max(f32::EPSILON);
    if let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.zoom = base_zoom;
    }
}

fn configure_platformer_camera(world: &mut World, player: Entity) {
    let map_bounds = CameraBounds {
        min: Vec2::ZERO,
        max: Vec2::new(MAP_COLS as f32 * TILE, MAP_ROWS as f32 * TILE),
    };
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.mode = CameraMode::Follow(player);
        controller.dead_zone_size = Vec2::ZERO;
        controller.smoothing_factor = 1.0;
        controller.bounds = Some(map_bounds);
        controller.zoom_multiplier = 1.0;
        controller.shake_amplitude = Vec2::ZERO;
        controller.shake_frequency_hz = 0.0;
        controller.shake_phase = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Extract functions
// ---------------------------------------------------------------------------

fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let mut batches = extract_tilemaps(world);
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return batches;
    };

    // Player — sprite frame driven by CurrentSprite / AnimationState.
    // Rendered at 1:1 world-pixel scale (camera zoom handles the screen
    // upscale). Sprite is bottom-aligned to the physics AABB so the player
    // visually stands on surfaces rather than sinking into them.
    let mut player_batches: std::collections::HashMap<String, SpriteBatch> =
        std::collections::HashMap::new();
    for (entity, cs) in world.query::<CurrentSprite>() {
        let Some(pos) = world.get::<Position>(entity).copied() else {
            continue;
        };
        let Some(asset) = assets.get_sprite(&cs.0) else {
            continue;
        };
        let sprite_w = asset.width as f32;
        let sprite_h = asset.height as f32;
        let batch = player_batches
            .entry(cs.0.clone())
            .or_insert_with(|| SpriteBatch {
                texture: asset.texture,
                filter: asset.filter,
                instances: Vec::new(),
            });
        batch.instances.push(SpriteInstance {
            // Centre horizontally on physics centre; align sprite bottom with
            // physics AABB bottom so the character stands on the ground.
            position: [pos.0.x - sprite_w * 0.5, pos.0.y + PLAYER_HALF.y - sprite_h],
            size: [sprite_w, sprite_h],
            rotation: 0.0,
            color: [255; 4],
        });
    }
    batches.extend(player_batches.into_values());

    // Bouncing balls.
    if let Some(ball_asset) = assets.get_sprite("ex10_ball") {
        let instances: Vec<SpriteInstance> = world
            .query::<Ball>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .map(|p| SpriteInstance {
                position: [p.0.x - BALL_RADIUS, p.0.y - BALL_RADIUS],
                size: [BALL_RADIUS * 2.0, BALL_RADIUS * 2.0],
                rotation: 0.0,
                color: [255; 4],
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

/// Renders `section` with a solid dark outline by drawing the same text at
/// eight pixel offsets in a dark colour first, then the original on top.
/// No engine changes needed — just extra TextSections in draw order.
fn text_outlined(section: TextSection) -> impl Iterator<Item = TextSection> {
    const STROKE: f32 = 2.0;
    const OUTLINE: [u8; 4] = [0, 0, 0, 210];
    let offsets: &[[f32; 2]] = &[
        [-STROKE, 0.0],
        [STROKE, 0.0],
        [0.0, -STROKE],
        [0.0, STROKE],
        [-STROKE, -STROKE],
        [STROKE, -STROKE],
        [-STROKE, STROKE],
        [STROKE, STROKE],
    ];
    let shadows: Vec<TextSection> = offsets
        .iter()
        .map(|&[dx, dy]| TextSection {
            content: section.content.clone(),
            font_id: section.font_id.clone(),
            font_size: section.font_size,
            line_height: section.line_height,
            color: OUTLINE,
            position: [section.position[0] + dx, section.position[1] + dy],
            bounds: section.bounds,
        })
        .collect();
    shadows.into_iter().chain(std::iter::once(section))
}

fn extract_text(world: &World) -> Vec<TextSection> {
    let disp = world
        .get_resource::<TextDisplayState>()
        .map(|s| {
            (
                s.fps, s.contacts, s.grounded, s.music_on, s.vol_pct, s.zoom_pct,
            )
        })
        .unwrap_or((0, 0, false, false, 50, 100));
    let (fps, contacts, grounded, music_on, vol_pct, zoom_pct) = disp;

    let mut sections = Vec::new();

    sections.extend(text_outlined(TextSection {
        content: "Tungsten Platformer".into(),
        font_id: "sans_bold".into(),
        font_size: 36.0,
        line_height: 44.0,
        color: [255, 255, 255, 230],
        position: [16.0, 14.0],
        bounds: None,
    }));

    sections.extend(text_outlined(TextSection {
        content: format!(
            "A/D move  Space jump  M music  1/2/3 vol  S stop  =/- zoom\n\
             grounded:{:<4} contacts:{:<3} music:{:<4} vol:{}%  zoom:{}%  FPS:{}",
            if grounded { "yes" } else { "no" },
            contacts,
            if music_on { "on" } else { "off" },
            vol_pct,
            zoom_pct,
            fps,
        ),
        font_id: "mono".into(),
        font_size: 24.0,
        line_height: 32.0,
        color: [200, 220, 255, 210],
        position: [16.0, 70.0],
        bounds: None,
    }));

    sections
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut config = Config::load("tungsten.json")?;
    config.window.width = 1920;
    config.window.height = 1080;
    let mut app = App::new(config);

    // Hot reload watches both the shared root assets dir (walk sprites,
    // animation, fonts) and the local example dir (tilemap, tile sprites).
    app.enable_hot_reload(
        &[PathBuf::from(ASSETS_ROOT), PathBuf::from(ASSETS_LOCAL)],
        PathBuf::from(MANIFEST_LOCAL),
    );

    {
        let world = app.world_mut();

        if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
            cfg.gravity = Vec2::new(0.0, GRAVITY_Y);
            cfg.broadphase_cell_size = 32.0;
        }
        world.insert_resource(TextDisplayState::default());

        // Tilemap — provides the static ground, platforms, and collision layer.
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

        // Player — spawn past the camera dead-zone (x > viewport_w/2 = 256) so
        // the camera is visibly scrolled from the first frame. At x = 320 the
        // camera starts at position 320 - 256 = 64, putting the player centred
        // on screen. Moving left scrolls the background right; moving right
        // scrolls it left (until the right edge clamp at max_x = 256).
        let player = world.spawn();
        let player_start = Vec2::new(20.0 * TILE, 13.0 * TILE);
        world.insert(player, Player::default());
        world.insert(player, Position(player_start));
        world.insert(player, Transform::from_position(player_start));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic().with_restitution(0.0));
        world.insert(player, AnimationState::new("walk"));
        world.insert(player, CurrentSprite("walk_0".into()));
        configure_platformer_camera(world, player);

        // Bouncing balls spread across the level: (col, row, initial vx).
        let ball_spawns: &[(f32, f32, f32)] = &[
            (6.0, 3.0, 70.0),
            (10.0, 5.0, -50.0),
            (15.0, 3.0, 90.0),
            (20.0, 5.0, -80.0),
            (25.0, 3.0, 55.0),
            (30.0, 5.0, -65.0),
            (35.0, 3.0, 75.0),
            (40.0, 5.0, -45.0),
        ];
        for &(col, row, vx) in ball_spawns {
            let ball = world.spawn();
            world.insert(ball, Ball);
            world.insert(ball, Position(Vec2::new(col * TILE, row * TILE)));
            world.insert(ball, Velocity(Vec2::new(vx, 0.0)));
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
        // Root manifest: fonts (sans, sans_bold, mono), walk animation + sprites, sounds.
        let root = ResolvedManifest::load(MANIFEST_ROOT).expect("Failed to load root manifest");
        asset_loader::load_all(&root, world, renderer).expect("Failed to load root assets");

        // Local manifest: tile sprites + tilemap only. Call individual loaders rather than
        // load_all to avoid overwriting the SoundRegistry/AnimationRegistry/FontRegistry
        // that were just populated from the root manifest (those registries are replaced on
        // every load_all call).
        let local = ResolvedManifest::load(MANIFEST_LOCAL).expect("Failed to load local manifest");
        asset_loader::load_sprites(&local, world, renderer).expect("Failed to load local sprites");
        asset_loader::load_tilemaps(&local, world).expect("Failed to load local tilemaps");

        // Verify required assets.
        let registry = world.get_resource::<AssetRegistry>().unwrap();
        for id in [
            "ex10_ground",
            "ex10_platform",
            "ex10_sky",
            "ex10_ball",
            "walk_0",
        ] {
            assert!(registry.get_sprite(id).is_some(), "missing sprite '{id}'");
        }
        let tilemaps = world.get_resource::<TilemapRegistry>().unwrap();
        assert!(
            tilemaps.get("ex10_level").is_some(),
            "missing tilemap 'ex10_level'"
        );

        // Resolve audio handles and stash them in a resource.
        let (sfx_handle, music_handle, sfx_volume, music_volume) = {
            let reg = world
                .get_resource::<SoundRegistry>()
                .expect("SoundRegistry missing");
            let sfx = reg.get_by_id("sfx_blip").expect("sfx_blip not found");
            let music = reg.get_by_id("music_main").expect("music_main not found");
            (sfx, music, reg.get_volume(sfx), reg.get_volume(music))
        };
        world.insert_resource(AudioState {
            sfx_handle,
            music_handle,
            sfx_volume,
            music_volume,
            music_playing: false,
            master_volume: 0.5,
        });
        if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
            cmds.set_master_volume(0.5);
        }
    });

    // Ordering: text-display-cache → input → audio → animation → physics →
    // post-physics state sync → shared camera update.
    app.add_system(update_text_display);
    app.add_system(player_input);
    app.add_system(audio_input_system);
    app.add_system(camera_zoom_input_system);
    app.add_system(animation_system);
    app.add_system(physics_step);
    app.add_system(ground_detection);
    app.add_system(sync_position_to_transform);
    app.add_system(platformer_camera_base_zoom);
    app.add_system(camera_update_system);
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);

    app.run()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
    use tungsten::core::DeltaTime;

    fn seed_world() -> World {
        let mut world = World::new();
        world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
        world.insert_resource(InputState::new());
        world.insert_resource(EventQueue::<CollisionEvent>::new());
        world.insert_resource(PhysicsConfig {
            gravity: Vec2::new(0.0, GRAVITY_Y),
            ..PhysicsConfig::default()
        });
        world.insert_resource(TilemapRegistry::new());
        world.insert_resource(CameraState::new());
        world.insert_resource(CameraController::default());
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

    // --- Input / ECS ---

    #[test]
    fn player_moves_right_on_d() {
        let mut world = seed_world();
        let player = world.spawn();
        world.insert(player, Player::default());
        world.insert(player, Position(Vec2::new(100.0, 100.0)));
        world.insert(player, Transform::from_position(Vec2::new(100.0, 100.0)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic());
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .key_down(KeyCode::KeyD);

        player_input(&mut world);

        let vel = world.get::<Velocity>(player).unwrap().0;
        assert!(vel.x > 0.0, "velocity.x did not increase: {:?}", vel);
    }

    // --- Physics ---

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
        world.insert(player, Position(Vec2::new(40.0, 8.0)));
        world.insert(player, Transform::from_position(Vec2::new(40.0, 8.0)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic());

        for _ in 0..20 {
            player_input(&mut world);
            physics_step(&mut world);
            ground_detection(&mut world);
            world
                .get_resource_mut::<EventQueue<CollisionEvent>>()
                .unwrap()
                .flush();
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
        world.insert(player, Player { grounded: false });
        world.insert(player, Position(Vec2::new(40.0, 40.0)));
        world.insert(player, Transform::from_position(Vec2::new(40.0, 40.0)));
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

    // --- Camera ---

    #[test]
    fn shared_camera_tracks_player() {
        let mut world = seed_world();
        let player = world.spawn();
        world.insert(player, Player::default());
        world.insert(player, Position(Vec2::new(300.0, 100.0)));
        world.insert(player, Transform::from_position(Vec2::new(300.0, 100.0)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic());
        configure_platformer_camera(&mut world, player);

        sync_position_to_transform(&mut world);
        platformer_camera_base_zoom(&mut world);
        camera_update_system(&mut world);

        let cam = world.get_resource::<CameraState>().unwrap();
        assert!(
            cam.position.x > 0.0,
            "camera did not follow player: {:?}",
            cam.position
        );
    }

    #[test]
    fn camera_clamped_at_right_boundary() {
        let mut world = seed_world();
        let player = world.spawn();
        world.insert(player, Player::default());
        // Place the player far past the right edge of the map.
        world.insert(player, Position(Vec2::new(9999.0, 100.0)));
        world.insert(player, Transform::from_position(Vec2::new(9999.0, 100.0)));
        world.insert(player, Velocity(Vec2::ZERO));
        world.insert(player, Collider::aabb(PLAYER_HALF));
        world.insert(player, RigidBody::dynamic());
        configure_platformer_camera(&mut world, player);

        sync_position_to_transform(&mut world);
        platformer_camera_base_zoom(&mut world);
        camera_update_system(&mut world);

        let cam = world.get_resource::<CameraState>().unwrap();
        // The shared camera path derives zoom from window.height / map_h.
        // seed_world uses 480x288, map_h = 288, so zoom = 1.0 and viewport_w = 480.
        let zoom = 288.0 / (MAP_ROWS as f32 * TILE);
        let max_x = (MAP_COLS as f32 * TILE - 480.0 / zoom).max(0.0);
        assert!(
            cam.position.x <= max_x,
            "camera not clamped: {} > {}",
            cam.position.x,
            max_x
        );
    }
}
