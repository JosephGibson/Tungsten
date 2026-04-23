//! In-tree deterministic PCG32 PRNG plus `SplitMix64` seed mixer.
//!
//! Not cryptographic.

use glam::Vec2;

const PCG32_MULT: u64 = 6_364_136_223_846_793_005;

/// PCG32 XSH-RR 64/32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// Seed deterministic stream from one `u64`.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        let inc = splitmix64(seed ^ 0xda3e_39cb_94b9_5bdb) | 1;
        let mut rng = Self { state: 0, inc };
        let _ = rng.next_u32();
        rng.state = rng.state.wrapping_add(splitmix64(seed));
        let _ = rng.next_u32();
        rng
    }

    /// Next 32-bit output.
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

    /// Uniform `f32` on `[lo, hi)`.
    pub fn next_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32_unit()
    }

    /// Uniform unit vector.
    pub fn next_unit_vec2(&mut self) -> Vec2 {
        let theta = self.next_f32_unit() * std::f32::consts::TAU;
        let (s, c) = theta.sin_cos();
        Vec2::new(c, s)
    }
}

/// `SplitMix64` finalizer.
#[must_use]
pub fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
#[path = "tests/rng.rs"]
mod tests;
