use glam::Vec2;
use tungsten::core::{AudioHandle, Entity};

pub(crate) const MANIFEST_ROOT: &str = "assets/manifest.json";
pub(crate) const MANIFEST_LOCAL: &str = "examples/01_platformer/assets/manifest.json";
pub(crate) const ASSETS_ROOT: &str = "assets";
pub(crate) const ASSETS_LOCAL: &str = "examples/01_platformer/assets";

pub(crate) const TILE: f32 = 16.0;
pub(crate) const MAP_COLS: u32 = 84;
pub(crate) const MAP_ROWS: u32 = 32;

pub(crate) const TEXT_UPDATE_INTERVAL: f32 = 0.25;

pub(crate) const PLAYER_HALF: Vec2 = Vec2::new(6.0, 7.0);
pub(crate) const PLAYER_SPAWN: Vec2 = Vec2::new(20.0 * TILE, 27.0 * TILE);
pub(crate) const PLAYER_MOVE_SPEED: f32 = 140.0;
pub(crate) const PLAYER_JUMP_IMPULSE: f32 = 320.0;
pub(crate) const GRAVITY_Y: f32 = 900.0;
pub(crate) const BALL_RADIUS: f32 = 6.0;
pub(crate) const BALL_RESTITUTION: f32 = 0.85;

pub(crate) const BALL_SPAWN_INTERVAL: f32 = 0.032;
/// Golden-angle spawn jitter prevents coincident-circle degenerate normals.
pub(crate) const BALL_SPAWN_JITTER: f32 = 1.0;

pub(crate) const BLACK_HOLE_RADIUS: f32 = 96.0;
pub(crate) const BLACK_HOLE_FORCE: f32 = 3000.0;
pub(crate) const BLACK_HOLE_LIFETIME: f32 = 2.0;
pub(crate) const BLACK_HOLE_VISUAL_DIAMETER: f32 = 45.0;

// Active physics bounds prevent runaway substep cost.
pub(crate) const WORLD_BOUNDS_MIN: Vec2 = Vec2::new(-TILE * 2.0, -TILE * 8.0);
pub(crate) const WORLD_BOUNDS_MAX: Vec2 = Vec2::new(
    (MAP_COLS as f32 + 2.0) * TILE,
    (MAP_ROWS as f32 + 8.0) * TILE,
);

pub(crate) struct AudioState {
    pub(crate) sfx_handle: AudioHandle,
    pub(crate) music_handle: AudioHandle,
    pub(crate) sfx_volume: f32,
    pub(crate) music_volume: f32,
    pub(crate) music_playing: bool,
    pub(crate) master_volume: f32,
}

pub(crate) struct TextDisplayState {
    pub(crate) fps: u32,
    pub(crate) contacts: usize,
    pub(crate) grounded: bool,
    pub(crate) music_on: bool,
    pub(crate) vol_pct: u32,
    pub(crate) zoom_pct: u32,
    pub(crate) timer: f32,
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

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Player {
    pub(crate) grounded: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Ball;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BlackHole {
    pub(crate) remaining: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BallSpawnState {
    pub(crate) accumulator: f32,
    pub(crate) spawn_phase: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ActiveBlackHole(pub(crate) Option<Entity>);

#[derive(Debug, Clone)]
pub(crate) struct CurrentSprite(pub(crate) String);
