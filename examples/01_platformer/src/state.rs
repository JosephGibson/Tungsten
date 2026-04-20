use glam::Vec2;
use tungsten::core::{AudioHandle, Entity};

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

// --- Ball-spawn-hold tunables ---
/// Seconds between ball spawns while `spawn_ball` is held. Driven by a
/// fixed accumulator over frame `DeltaTime`, not wall-clock.
pub(crate) const BALL_SPAWN_INTERVAL: f32 = 0.032;
/// Sub-pixel offset applied to each spawned ball via a golden-angle spiral.
/// Why: `circle_vs_circle` degenerates when two circles are exactly
/// coincident and falls back to a fixed `(-1, 0)` normal, so a pile of
/// balls spawned at one point drifts systematically southeast every
/// substep. Breaking coincidence at spawn time avoids the degenerate path
/// entirely while staying well under `BALL_RADIUS` so the jitter is
/// visually invisible.
pub(crate) const BALL_SPAWN_JITTER: f32 = 1.0;

// --- Black-hole tunables ---
/// Pull radius in world pixels. Entities outside are untouched.
pub(crate) const BLACK_HOLE_RADIUS: f32 = 96.0;
/// Peak inward acceleration in px/s² at the black hole's centre.
/// Falls off linearly with distance to zero at `BLACK_HOLE_RADIUS`.
pub(crate) const BLACK_HOLE_FORCE: f32 = 3000.0;
/// Seconds the black hole stays active after the cursor is released.
/// Refreshed every frame while Mouse2 is held so the hole persists for
/// the full duration of the drag.
pub(crate) const BLACK_HOLE_LIFETIME: f32 = 2.0;
/// Visual diameter of the purple blob sprite in world pixels.
pub(crate) const BLACK_HOLE_VISUAL_DIAMETER: f32 = 45.0;

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

/// Black-hole attractor. Despawned by `black_hole_lifetime_system` once
/// `remaining` reaches zero.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlackHole {
    pub(crate) remaining: f32,
}

/// Fixed-accumulator state for ball-spawn-hold. Advanced by `DeltaTime`
/// each frame; every `BALL_SPAWN_INTERVAL` drained spawns one ball.
/// `spawn_phase` indexes into the golden-angle jitter spiral so
/// consecutive spawns are never coincident.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BallSpawnState {
    pub(crate) accumulator: f32,
    pub(crate) spawn_phase: u32,
}

/// Entity of the black hole currently being dragged by the held Mouse2
/// button, if any. Cleared on release so subsequent holes fade with their
/// remaining lifetime instead of being dragged around.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ActiveBlackHole(pub(crate) Option<Entity>);

/// Current sprite frame driven by `AnimationState` — updated by `animation_system`.
#[derive(Debug, Clone)]
pub(crate) struct CurrentSprite(pub(crate) String);
