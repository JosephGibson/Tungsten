//! The toys' components and resources.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::core::{Entity, Voice};

/// Every ball, small or bowling: its prefab names it `ball`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Ball;

/// A small glass marble, which burns, bursts on a hard impact and takes a
/// rainbow hue. Its prefab names it `small_ball`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct SmallBall {
    /// Game seconds until its next impact burst may show.
    #[serde(skip)]
    pub impact_cooldown: f32,
}

/// The heavy bowling ball: tossed, it lands low and rolls; a fireball
/// bursts it. Its prefab names it `bowling_ball`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct BowlingBall;

/// A small ball's rainbow tint: its hue in turns and how fast it turns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BallHue {
    pub hue: f32,
    pub speed: f32,
}

impl BallHue {
    /// A hue and a speed from `seed`, the ball's place in its spawn order.
    #[must_use]
    pub fn from_seed(seed: u32) -> Self {
        let a = hash_unit(seed);
        let b = hash_unit(seed ^ 0xa511_e9b3);
        let direction = if seed & 1 == 0 { 1.0 } else { -1.0 };
        Self {
            hue: a,
            speed: direction * (0.16 + b * 0.24),
        }
    }
}

/// A well-mixed value in `[0, 1]` from `x`.
#[must_use]
pub fn hash_unit(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x as f32) / (u32::MAX as f32)
}

/// An iron block: a heavy box that shoves balls aside, crushes what it
/// drives into fast and freezes under the cold spray. Its prefab names it
/// `iron_block`; its art is four quarters and their frost, its children.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct IronBlock;

/// One of the sixteen pieces a frozen block shatters into, each with one of
/// four broken-metal sprites. Its prefab names it `iron_scrap`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IronScrap {
    pub variant: usize,
}

/// A scrap's tumble as it flies off, on the level's clock: presentation.
#[derive(Debug, Clone, Copy)]
pub struct ScrapTumble {
    pub born_at: f32,
    pub initial_angle: f32,
    pub total_turn: f32,
}

impl ScrapTumble {
    /// Its turn at level time `time`.
    #[must_use]
    pub fn rotation(self, time: f32) -> f32 {
        let age = (time - self.born_at).max(0.0);
        self.initial_angle + self.total_turn * (1.0 - (-4.0 * age).exp())
    }
}

/// A black hole: it pulls bodies in reach, bends missiles and puts out
/// burning balls, for `left` game seconds. Its prefab names it
/// `black_hole`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlackHole {
    pub left: f32,
}

/// The fire missile, slot 1's spawn: the kit's projectile on its linear
/// motion, which 01 bends by the black holes and blasts on what it meets.
/// Its prefab names it `fire_missile`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct FireMissile;

/// A blast's shock ring growing to `size` pixels across over its life: a
/// fireball's, hot, a burst ball's, or a force push's, cool.
#[derive(Debug, Clone, Copy)]
pub struct ShockRing {
    pub age: f32,
    pub size: f32,
    pub kind: Ring,
}

/// What a shock ring belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    Fireball,
    Ball,
    Push,
}

/// A light left where a missile ended, fading out over `secs`: a blast's
/// flares and fades more slowly.
#[derive(Debug, Clone, Copy)]
pub struct Flare {
    pub age: f32,
    pub secs: f32,
    pub blast: bool,
    pub phase: f32,
}

/// A body's velocity before this step's physics, which impacts read: by
/// then the solver has stopped it on what it met.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PreviousVelocity(pub Vec2);

/// The small balls' placement: the part of a ball the last steps owed, the
/// next ball's place on the golden-angle ring, and the balls queued this
/// frame, which count against the cap before they are live.
#[derive(Debug, Clone, Copy, Default)]
pub struct BallSpawner {
    pub owed: f32,
    pub phase: u32,
    pub queued: u32,
    pub queued_frame: u64,
}

/// A spray's element: fire ignites small balls, cold frosts iron, and each
/// deals its damage type to the targets in its cone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Fire,
    Cold,
}

/// A held spray, while its spell holds: its caster, its element, where it
/// starts and stops this frame, its age, its particle emitters, its loop's
/// voice, whether this step's spell kept it, and the game seconds toward
/// its next pulse of damage on the targets in its cone.
#[derive(Debug, Clone, Copy)]
pub struct Spray {
    pub caster: Entity,
    pub element: Element,
    pub start: Vec2,
    pub end: Vec2,
    pub aim: Vec2,
    pub age: f32,
    pub emitters: [Option<Entity>; 3],
    pub voice: Option<Voice>,
    pub kept: bool,
    pub pulse: f32,
}

/// A spray emitter's aim, which its newborn particles turn to: the
/// engine's cones face +x.
#[derive(Debug, Clone, Copy)]
pub struct SprayEmitter {
    pub direction: Vec2,
}

/// A particle already turned to its spray's aim.
#[derive(Debug, Clone, Copy)]
pub struct Oriented;
