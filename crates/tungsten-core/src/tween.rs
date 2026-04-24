//! M24 tween primitives. D-054 closed easings, D-055 one `Tween` per entity,
//! D-056 completion via `EventQueue` + removal via `CommandBuffer`.
//!
//! M26 extends `TweenChannel` with uniform-slot variants that write into
//! `UniformOverrideBlock`. The block is a 256-byte GPU-ready payload shared
//! between material UBOs (M26) and MSDF outline/glow (M32).

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

use crate::ecs::Entity;

/// `apply(t)` is pre-clamped `[0, 1]`; Back/Bounce overshoot intentionally.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Easing {
    #[default]
    Linear,
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    QuartIn,
    QuartOut,
    QuartInOut,
    SineIn,
    SineOut,
    SineInOut,
    ExpoIn,
    ExpoOut,
    ExpoInOut,
    BackIn,
    BackOut,
    BackInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
}

impl Easing {
    #[inline]
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,

            Self::QuadIn => t * t,
            Self::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }

            Self::CubicIn => t * t * t,
            Self::CubicOut => {
                let u = 1.0 - t;
                1.0 - u * u * u
            }
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
                }
            }

            Self::QuartIn => t * t * t * t,
            Self::QuartOut => {
                let u = 1.0 - t;
                1.0 - u * u * u * u
            }
            Self::QuartInOut => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) * 0.5
                }
            }

            Self::SineIn => 1.0 - ((t * std::f32::consts::FRAC_PI_2).cos()),
            Self::SineOut => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::SineInOut => -0.5 * ((std::f32::consts::PI * t).cos() - 1.0),

            Self::ExpoIn => {
                if t <= 0.0 {
                    0.0
                } else {
                    (10.0 * (t - 1.0)).exp2()
                }
            }
            Self::ExpoOut => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - (-10.0 * t).exp2()
                }
            }
            Self::ExpoInOut => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else if t < 0.5 {
                    0.5 * (20.0 * t - 10.0).exp2()
                } else {
                    1.0 - 0.5 * (-20.0 * t + 10.0).exp2()
                }
            }

            Self::BackIn => {
                let c1 = 1.701_58_f32;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Self::BackOut => {
                let c1 = 1.701_58_f32;
                let c3 = c1 + 1.0;
                let u = t - 1.0;
                1.0 + c3 * u * u * u + c1 * u * u
            }
            Self::BackInOut => {
                let c1 = 1.701_58_f32;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    let u = 2.0 * t;
                    (u * u * ((c2 + 1.0) * u - c2)) * 0.5
                } else {
                    let u = 2.0 * t - 2.0;
                    (u * u * ((c2 + 1.0) * u + c2) + 2.0) * 0.5
                }
            }

            Self::BounceIn => 1.0 - bounce_out(1.0 - t),
            Self::BounceOut => bounce_out(t),
            Self::BounceInOut => {
                if t < 0.5 {
                    (1.0 - bounce_out(1.0 - 2.0 * t)) * 0.5
                } else {
                    (1.0 + bounce_out(2.0 * t - 1.0)) * 0.5
                }
            }
        }
    }
}

// Robert Penner reference constants.
#[inline]
fn bounce_out(t: f32) -> f32 {
    let n1 = 7.5625_f32;
    let d1 = 2.75_f32;
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let u = t - 1.5 / d1;
        n1 * u * u + 0.75
    } else if t < 2.5 / d1 {
        let u = t - 2.25 / d1;
        n1 * u * u + 0.9375
    } else {
        let u = t - 2.625 / d1;
        n1 * u * u + 0.984_375
    }
}

/// M26 4x `vec4<f32>` slot in `UniformOverrideBlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Vec4Slot {
    V0,
    V1,
    V2,
    V3,
}

/// M26 4x `f32` scalar slot in `UniformOverrideBlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarSlot {
    F0,
    F1,
    F2,
    F3,
}

/// M26 4x `i32` slot in `UniformOverrideBlock` (stepped, not lerped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntSlot {
    I0,
    I1,
    I2,
    I3,
}

impl Vec4Slot {
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::V0 => 0,
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3,
        }
    }
}

impl ScalarSlot {
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::F0 => 0,
            Self::F1 => 1,
            Self::F2 => 2,
            Self::F3 => 3,
        }
    }
}

impl IntSlot {
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::I0 => 0,
            Self::I1 => 1,
            Self::I2 => 2,
            Self::I3 => 3,
        }
    }
}

/// M26 entity-local animation surface. 256 bytes, `std140`-friendly layout:
/// 4 `vec4<f32>` slots (64 bytes), 4 `f32` scalars (16 bytes), 4 `i32` slots
/// (16 bytes), then 160 bytes of reserved tail. Shared between material UBOs
/// and the future M32 MSDF outline/glow surface; writing through slot enums
/// keeps layout decisions out of call sites.
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct UniformOverrideBlock {
    pub vec4: [[f32; 4]; 4],
    pub f32s: [f32; 4],
    pub i32s: [i32; 4],
    /// Reserved zero-padding to reach a fixed 256-byte binding. Laid out as
    /// `vec4<u32>`-sized rows so it remains `bytemuck::Pod`-compatible. Do not
    /// read; future slot growth should rename pieces out of this tail.
    _reserved: [[u32; 4]; 10],
}

impl Default for UniformOverrideBlock {
    fn default() -> Self {
        Self {
            vec4: [[0.0; 4]; 4],
            f32s: [0.0; 4],
            i32s: [0; 4],
            _reserved: [[0; 4]; 10],
        }
    }
}

impl UniformOverrideBlock {
    /// Construct a block from the authored/public slots while preserving the
    /// zeroed reserved tail.
    #[must_use]
    pub fn from_slots(vec4: [[f32; 4]; 4], f32s: [f32; 4], i32s: [i32; 4]) -> Self {
        Self {
            vec4,
            f32s,
            i32s,
            ..Self::default()
        }
    }

    /// GPU-ready 256-byte payload.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut out = [0u8; 256];
        out.copy_from_slice(bytemuck::bytes_of(self));
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TweenChannel {
    PositionX {
        from: f32,
        to: f32,
    },
    PositionY {
        from: f32,
        to: f32,
    },
    Rotation {
        from: f32,
        to: f32,
    },
    ScaleX {
        from: f32,
        to: f32,
    },
    ScaleY {
        from: f32,
        to: f32,
    },
    ColorR {
        from: u8,
        to: u8,
    },
    ColorG {
        from: u8,
        to: u8,
    },
    ColorB {
        from: u8,
        to: u8,
    },
    ColorA {
        from: u8,
        to: u8,
    },
    /// Drives one lane (`0..=3`) of a `UniformOverrideBlock.vec4[slot]` slot.
    UniformVec4Lane {
        slot: Vec4Slot,
        lane: u8,
        from: f32,
        to: f32,
    },
    UniformScalar {
        slot: ScalarSlot,
        from: f32,
        to: f32,
    },
    /// Integer step: `to` once `k >= 0.5`, else `from`. Not linearly interpolated.
    UniformInt {
        slot: IntSlot,
        from: i32,
        to: i32,
    },
}

/// `Loop` and `PingPong` never emit `TweenComplete`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TweenRepeat {
    #[default]
    Once,
    Loop,
    PingPong,
    Times(u32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TweenDirection {
    #[default]
    Forward,
    Backward,
}

/// `pending_remove` latches the tick so a completed tween cannot re-fire before
/// the command flush drops it.
#[derive(Debug, Clone)]
pub struct Tween {
    pub channels: Vec<TweenChannel>,
    pub easing: Easing,
    pub duration: f32,
    pub elapsed: f32,
    pub repeat: TweenRepeat,
    pub direction: TweenDirection,
    pub completed_cycles: u32,
    pub on_complete_tag: Option<String>,
    pub pending_remove: bool,
}

impl Tween {
    /// Clamps non-finite / ≤ 0 durations to `f32::EPSILON`; debug-asserts in debug builds.
    #[must_use]
    pub fn new(duration: f32, easing: Easing) -> Self {
        debug_assert!(
            duration.is_finite() && duration > 0.0,
            "Tween::new requires duration > 0 (got {duration})"
        );
        let duration = if duration.is_finite() && duration > 0.0 {
            duration
        } else {
            f32::EPSILON
        };
        Self {
            channels: Vec::new(),
            easing,
            duration,
            elapsed: 0.0,
            repeat: TweenRepeat::Once,
            direction: TweenDirection::Forward,
            completed_cycles: 0,
            on_complete_tag: None,
            pending_remove: false,
        }
    }

    #[must_use]
    pub fn with_channel(mut self, channel: TweenChannel) -> Self {
        self.channels.push(channel);
        self
    }

    #[must_use]
    pub fn with_repeat(mut self, repeat: TweenRepeat) -> Self {
        self.repeat = repeat;
        self
    }

    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.on_complete_tag = Some(tag.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct TweenComplete {
    pub entity: Entity,
    pub tag: Option<String>,
}

#[inline]
#[must_use]
pub fn lerp_u8(from: u8, to: u8, t: f32) -> u8 {
    let from = f32::from(from);
    let to = f32::from(to);
    (from + (to - from) * t).round().clamp(0.0, 255.0) as u8
}

#[inline]
#[must_use]
pub fn lerp_f32(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[cfg(test)]
#[path = "tests/tween.rs"]
mod tests;
