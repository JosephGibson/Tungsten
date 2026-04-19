use glam::Vec2;
use tungsten::core::AudioHandle;

pub(crate) const MANIFEST_ROOT: &str = "assets/manifest.json";
pub(crate) const MANIFEST_LOCAL: &str = "examples/01_platformer/assets/manifest.json";
pub(crate) const ASSETS_ROOT: &str = "assets";
pub(crate) const ASSETS_LOCAL: &str = "examples/01_platformer/assets";

pub(crate) const TILE: f32 = 16.0;
pub(crate) const MAP_COLS: u32 = 48;
pub(crate) const MAP_ROWS: u32 = 18;

/// How often the HUD values (FPS, contacts, etc.) are refreshed in seconds.
/// Values are cached between refreshes so they don't flicker every frame.
pub(crate) const TEXT_UPDATE_INTERVAL: f32 = 0.25;

pub(crate) const PLAYER_HALF: Vec2 = Vec2::new(6.0, 7.0);
pub(crate) const PLAYER_SPAWN: Vec2 = Vec2::new(20.0 * TILE, 13.0 * TILE);
pub(crate) const PLAYER_MOVE_SPEED: f32 = 140.0;
pub(crate) const PLAYER_JUMP_IMPULSE: f32 = 320.0;
pub(crate) const GRAVITY_Y: f32 = 900.0;
pub(crate) const BALL_RADIUS: f32 = 6.0;
pub(crate) const BALL_RESTITUTION: f32 = 0.85;

// Active physics region. Anything outside gets culled (balls despawn,
// player resets to `PLAYER_SPAWN`) so runaway velocities can't inflate
// `compute_substeps` and drag the whole simulation to the 8× substep cap.
// Margins are generous — normal bounces stay well inside.
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

/// Cached HUD values, updated at `TEXT_UPDATE_INTERVAL` by `update_text_display`.
/// Avoids rebuilding the text layout every frame and stops numbers from flickering.
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

/// Player marker + grounded flag. Reset to false each frame by `player_input`;
/// re-set to true by `ground_detection` after the physics step resolves contacts.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Player {
    pub(crate) grounded: bool,
}

/// Marker for the bouncing circle entities.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ball;

/// Current sprite frame driven by `AnimationState` — updated by `animation_system`.
#[derive(Debug, Clone)]
pub(crate) struct CurrentSprite(pub(crate) String);
