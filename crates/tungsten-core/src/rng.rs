//! In-tree PCG32 PRNG + SplitMix64 seed mixer.
//!
//! Rolled here instead of pulling in `rand` or `fastrand` because the only
//! consumer is the M23 particle system, and its deterministic-replay
//! requirement is satisfied by a 16-byte state machine. See `DECISIONS.md`.
//!
//! `Pcg32` is the XSH-RR 64/32 LCG from pcg-random.org: state advances with a
//! fixed multiplier, output is a permuted 32-bit lane. `SplitMix64` derives
//! an independent stream-selector (`inc`) from the input seed so consecutive
//! seed values still produce uncorrelated streams.
//!
//! Not cryptographic. Do not use for anything that needs unpredictability.
//!
//! Determinism: `Pcg32::seeded(seed)` is a pure function — the same seed
//! always produces the same stream, across platforms and across runs.

use glam::Vec2;

const PCG32_MULT: u64 = 6_364_136_223_846_793_005;

/// PCG32 (XSH-RR 64/32). Two 64-bit words of state: `state` is the LCG
/// accumulator, `inc` is the stream selector (always odd).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// Seed both words from a single `u64`. `inc` is derived via
    /// [`splitmix64`] and forced odd so two consecutive seed values still
    /// produce uncorrelated streams.
    pub fn seeded(seed: u64) -> Self {
        let inc = splitmix64(seed ^ 0xda3e_39cb_94b9_5bdb) | 1;
        let mut rng = Self { state: 0, inc };
        // Canonical pcg32 init: one advance, add seed, one advance.
        let _ = rng.next_u32();
        rng.state = rng.state.wrapping_add(splitmix64(seed));
        let _ = rng.next_u32();
        rng
    }

    /// Draw the next 32-bit output.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG32_MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform `f32` on `[0, 1)`.
    pub fn next_f32_unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Uniform `f32` on `[lo, hi)`. Returns `lo` if `lo == hi`.
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32_unit()
    }

    /// Uniform unit-length `Vec2` sampled from the full circle.
    pub fn next_unit_vec2(&mut self) -> Vec2 {
        let theta = self.next_f32_unit() * std::f32::consts::TAU;
        let (s, c) = theta.sin_cos();
        Vec2::new(c, s)
    }
}

/// SplitMix64 `finalise` mix, the canonical variant from Steele/Vigna 2014.
/// Also used to derive sequential seeds from a monotonic counter.
pub fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
#[path = "tests/rng.rs"]
mod tests;
