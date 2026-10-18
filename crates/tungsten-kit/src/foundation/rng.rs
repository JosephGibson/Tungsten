//! The kit's random stream: one seeded generator that every kit draw
//! takes from, in a fixed order, so a run repeats under its seed.

use tungsten_core::Pcg32;

/// The kit's random stream, a World resource: a seeded PCG32 that kit
/// systems draw from in a fixed order (a cooldown's jitter, in entity
/// order; a cue's `roll`, in the cue system's order), so a run under one
/// seed repeats. It is the kit's own, apart from the particles'
/// `WorldRngSeed`, so a draw here moves no emitter's seed.
/// [`CooldownPlugin`](crate::CooldownPlugin) and
/// [`CuesPlugin`](crate::CuesPlugin) insert one at
/// [`DEFAULT_SEED`](Self::DEFAULT_SEED) when the World has none; a game
/// reseeds it per run, as from a run seed in a save.
///
/// # Examples
///
/// ```
/// use tungsten_kit::KitRng;
///
/// let mut a = KitRng::seeded(7);
/// let mut b = KitRng::seeded(7);
/// let draws: Vec<f32> = (0..3).map(|_| a.unit()).collect();
/// assert_eq!(draws, (0..3).map(|_| b.unit()).collect::<Vec<_>>(), "one seed, one stream");
/// assert!(draws.iter().all(|u| (0.0..1.0).contains(u)));
/// assert_eq!(a.draws(), 3);
///
/// let roll = a.range(2.0, 4.0);
/// assert!((2.0..=4.0).contains(&roll));
/// assert!(a.index(6) < 6);
///
/// a.reseed(7);
/// assert_eq!((a.seed(), a.draws()), (7, 0));
/// assert_eq!(a.unit(), draws[0], "the stream starts over");
/// ```
#[derive(Debug, Clone)]
pub struct KitRng {
    rng: Pcg32,
    seed: u64,
    draws: u64,
}

impl Default for KitRng {
    /// [`seeded`](Self::seeded) at [`DEFAULT_SEED`](Self::DEFAULT_SEED).
    fn default() -> Self {
        Self::seeded(Self::DEFAULT_SEED)
    }
}

impl KitRng {
    /// The seed the kit starts from when the game gives none.
    pub const DEFAULT_SEED: u64 = 0x6b69_745f_726e_6721;

    /// A stream seeded with `seed`.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self {
            rng: Pcg32::seeded(seed),
            seed,
            draws: 0,
        }
    }

    /// Starts the stream over from `seed`.
    pub fn reseed(&mut self, seed: u64) {
        *self = Self::seeded(seed);
    }

    /// The seed the stream started from.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// How many 32-bit draws it has made since its seed: its place in the
    /// stream.
    #[must_use]
    pub fn draws(&self) -> u64 {
        self.draws
    }

    /// The next 32 bits.
    pub fn next_u32(&mut self) -> u32 {
        self.draws += 1;
        self.rng.next_u32()
    }

    /// A uniform number in `[0, 1)`, from one draw.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1_u32 << 24) as f32
    }

    /// A uniform number in `[lo, hi]`, from one draw, computed in `f64` so
    /// a span past `f32::MAX` stays finite; the result can round to `hi`
    /// for the largest draws.
    ///
    /// # Panics
    ///
    /// When `lo` or `hi` is not finite or `lo` is above `hi`.
    #[track_caller]
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        assert!(
            lo.is_finite() && hi.is_finite() && lo <= hi,
            "KitRng::range: {lo}..{hi} is not a finite, ordered range"
        );
        let unit = f64::from(self.unit());
        let (lo64, hi64) = (f64::from(lo), f64::from(hi));
        ((lo64 + (hi64 - lo64) * unit) as f32).clamp(lo, hi)
    }

    /// A uniform index below `len`, from one draw.
    ///
    /// # Panics
    ///
    /// When `len` is 0 or above `u32::MAX`.
    #[track_caller]
    pub fn index(&mut self, len: usize) -> usize {
        let Ok(len) = u32::try_from(len) else {
            panic!("KitRng::index: {len} is above u32::MAX");
        };
        assert!(len > 0, "KitRng::index: no index below 0");
        let wide = u64::from(self.next_u32()) * u64::from(len);
        // The high half of a 32 × 32 product is below `len`.
        (wide >> 32) as usize
    }
}

#[cfg(test)]
#[path = "../tests/rng.rs"]
mod tests;
