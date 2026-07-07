use glam::Vec2;
use tungsten::core::{AudioHandle, Entity};

pub(crate) const MANIFEST_ROOT: &str = "assets/manifest.json";
pub(crate) const MANIFEST_LOCAL: &str = "examples/01_platformer/assets/manifest.json";
pub(crate) const ASSETS_ROOT: &str = "assets";
pub(crate) const ASSETS_LOCAL: &str = "examples/01_platformer/assets";

pub(crate) const TILE: f32 = 32.0;
pub(crate) const MAP_COLS: u32 = 84;
pub(crate) const MAP_ROWS: u32 = 32;

pub(crate) const TEXT_UPDATE_INTERVAL: f32 = 0.25;

pub(crate) const PLAYER_ANIMATION_ID: &str = "ex10_player_walk";
pub(crate) const PLAYER_START_SPRITE_ID: &str = "ex10_player_walk_0";
pub(crate) const BALL_ANIMATION_ID: &str = "ex10_ball_spin";
pub(crate) const BALL_START_SPRITE_ID: &str = "ex10_ball";

pub(crate) const PLAYER_HALF: Vec2 = Vec2::new(10.0, 14.0);
pub(crate) const PLAYER_SPAWN: Vec2 = Vec2::new(20.0 * TILE, 27.0 * TILE);
pub(crate) const PLAYER_MOVE_SPEED: f32 = 280.0;
pub(crate) const PLAYER_JUMP_IMPULSE: f32 = 640.0;
pub(crate) const GRAVITY_Y: f32 = 1800.0;
pub(crate) const BALL_RADIUS: f32 = 15.0;
pub(crate) const BALL_VISUAL_DIAMETER: f32 = TILE;
pub(crate) const BALL_RESTITUTION: f32 = 0.85;

pub(crate) const BALL_SPAWN_INTERVAL: f32 = 0.032;
/// Golden-angle spawn jitter prevents coincident-circle degenerate normals.
pub(crate) const BALL_SPAWN_JITTER: f32 = 2.0;

pub(crate) const BLACK_HOLE_RADIUS: f32 = 192.0;
pub(crate) const BLACK_HOLE_FORCE: f32 = 6000.0;
pub(crate) const BLACK_HOLE_LIFETIME: f32 = 2.0;
pub(crate) const BLACK_HOLE_VISUAL_DIAMETER: f32 = 90.0;

// Active physics bounds prevent runaway substep cost.
pub(crate) const WORLD_BOUNDS_MIN: Vec2 = Vec2::new(-TILE * 2.0, -TILE * 8.0);
pub(crate) const WORLD_BOUNDS_MAX: Vec2 = Vec2::new(
    (MAP_COLS as f32 + 2.0) * TILE,
    (MAP_ROWS as f32 + 8.0) * TILE,
);

pub(crate) struct AudioState {
    pub(crate) sfx_handle: AudioHandle,
    pub(crate) black_hole_sfx_handle: AudioHandle,
    pub(crate) music_handle: AudioHandle,
    pub(crate) sfx_volume: f32,
    pub(crate) black_hole_sfx_volume: f32,
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
            timer: TEXT_UPDATE_INTERVAL,
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
pub(crate) struct BallHue {
    pub(crate) hue: f32,
    pub(crate) speed: f32,
}

impl BallHue {
    pub(crate) fn from_seed(seed: u32) -> Self {
        let a = hash_unit(seed);
        let b = hash_unit(seed ^ 0xa511_e9b3);
        let direction = if seed & 1 == 0 { 1.0 } else { -1.0 };
        Self {
            hue: a,
            speed: direction * (0.16 + b * 0.24),
        }
    }
}

fn hash_unit(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x as f32) / (u32::MAX as f32)
}

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

/// M26: marks the player entity as rendered through the `damage_flash`
/// material. Present only when the material id resolved at setup time.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlayerMaterial {
    pub(crate) material_id: tungsten::core::MaterialAssetId,
}

/// M29 lighting fixture mode parsed from `TUNGSTEN_LIGHTING_FIXTURE`.
/// `On`: spawn warm + cool point lights and a directional, low ambient,
/// route the player through the lit pipeline. `Off`: keep the M28 baseline
/// (white ambient, custom material/unlit player path).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LightingFixtureMode {
    On,
    #[default]
    Off,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LightingFixture {
    pub(crate) mode: LightingFixtureMode,
}

/// Marker on entities created by the lighting fixture (so cleanup is easy).
#[derive(Debug, Clone, Copy)]
pub(crate) struct OrbitLight {
    pub(crate) phase: f32,
    pub(crate) speed: f32,
    pub(crate) radius: f32,
    pub(crate) cycle: CycleMode,
    /// Color held when `cycle = Pulse`; the system rewrites `Light.intensity`
    /// each frame and restores `Light.color` to this value so a Pulse light
    /// never drifts off its authored hue.
    pub(crate) base_color: glam::Vec3,
}

/// M29 fixture-side cycle mode for an orbiting light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `None` is the obvious default for future fixture variants.
pub(crate) enum CycleMode {
    /// Hold the authored color and intensity. Light only orbits.
    None,
    /// Hold the authored color, sin-pulse intensity around 1.0.
    Pulse,
    /// Hold intensity, rotate hue around the wheel using `phase` as the angle.
    Hue,
}
