//! M23 particle configuration: JSON-backed, hot-reloadable, shared via
//! `Arc<ParticleConfig>` so in-flight particles and running emitters keep
//! their snapshot across a reload.
//!
//! The file schema is described in [`docs/plans/phase3-milestone-23-particle-system.md`].
//! Validation rules:
//! - Every float must be finite.
//! - `max_alive >= 1`; zero emitters would be inert-by-design but we forbid
//!   them outright so the per-emitter clamp stays `> 0` and doesn't need a
//!   special-case at the emit site.
//! - `lifetime.min >= 0`, `lifetime.max >= lifetime.min`, and `lifetime.max > 0`.
//! - Curves have at least one point, are sorted ascending in `t`, and cover
//!   the unit interval at the bounds — endpoint samples always return the
//!   first / last value.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use thiserror::Error;

use crate::rng::splitmix64;

/// Typed handle to a loaded asset, parametrised by the asset kind. The `u32`
/// is a dense index into the owning registry.
///
/// Post-M23 this type exists only for [`ParticleConfig`]; it is generic so
/// other registries can adopt it without a second wrapper type.
#[derive(Debug)]
pub struct AssetId<T> {
    index: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> AssetId<T> {
    pub fn new(index: u32) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    pub fn index(self) -> u32 {
        self.index
    }
}

impl<T> Clone for AssetId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for AssetId<T> {}

impl<T> PartialEq for AssetId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for AssetId<T> {}

impl<T> std::hash::Hash for AssetId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

/// Errors surfaced when parsing or validating a particle config file.
#[derive(Debug, Error)]
pub enum ParticleConfigError {
    #[error("failed to read particle config '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid particle config '{path}': {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("particle config '{path}' invalid: {reason}")]
    Validation { path: String, reason: String },
}

/// Inclusive scalar range used for per-particle spawn sampling.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Range {
    pub min: f32,
    pub max: f32,
}

impl Range {
    pub fn single(value: f32) -> Self {
        Self {
            min: value,
            max: value,
        }
    }
}

fn default_zero_range() -> Range {
    Range::single(0.0)
}

fn default_one_range() -> Range {
    Range::single(1.0)
}

fn default_drag_zero() -> f32 {
    0.0
}

fn default_gravity_zero() -> [f32; 2] {
    [0.0, 0.0]
}

fn default_white_rgba() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_blend() -> BlendMode {
    BlendMode::Alpha
}

/// Emission pattern. Drives how many particles spawn per tick.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EmissionKind {
    /// Emit `count` particles. If `once`, drains the emitter on first tick.
    Burst {
        count: u32,
        #[serde(default = "default_true")]
        once: bool,
    },
    /// Sustained emission at `rate_hz` particles per second.
    Continuous { rate_hz: f32 },
    /// Repeating burst every `interval_sec`; drains after `total_pulses` if set.
    Pulse {
        count_per_pulse: u32,
        interval_sec: f32,
        #[serde(default)]
        total_pulses: Option<u32>,
    },
}

fn default_true() -> bool {
    true
}

/// Initial-velocity sampler shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InitialVelocity {
    /// Random direction inside a cone around `direction` (±spread_deg/2).
    Cone {
        direction: [f32; 2],
        spread_deg: f32,
        speed: Range,
    },
    /// Random direction across the full circle.
    Radial { speed: Range },
    /// Exact velocity vector, magnitude = 1 (scaled by `speed`).
    Vector { direction: [f32; 2], speed: Range },
}

/// Blend mode hint. Applied CPU-side to the sampled RGBA before it hits the
/// shared sprite pipeline. `Premultiplied` multiplies `rgb *= a`, which
/// composes as additive-plus-over on a standard alpha-blended target —
/// giving glow-y sparks without a new pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlendMode {
    Alpha,
    Premultiplied,
}

/// Piecewise-linear curve over `t ∈ [0, 1]`. Points must be sorted by `t`
/// ascending; sampling outside the bound clamps to the endpoint.
#[derive(Debug, Clone)]
pub struct Curve<V: Copy> {
    pub points: Vec<(f32, V)>,
}

impl<V: Copy + Lerp> Curve<V> {
    pub fn sample(&self, t: f32) -> V {
        let pts = self.points.as_slice();
        if pts.is_empty() {
            panic!("Curve::sample called on empty curve — validate() should have caught this");
        }
        if t <= pts[0].0 {
            return pts[0].1;
        }
        if t >= pts[pts.len() - 1].0 {
            return pts[pts.len() - 1].1;
        }
        for win in pts.windows(2) {
            let (ta, va) = win[0];
            let (tb, vb) = win[1];
            if t <= tb {
                let span = (tb - ta).max(1.0e-6);
                let u = ((t - ta) / span).clamp(0.0, 1.0);
                return V::lerp(va, vb, u);
            }
        }
        pts[pts.len() - 1].1
    }
}

impl<'de, V: Copy + Deserialize<'de>> Deserialize<'de> for Curve<V> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw: Vec<(f32, V)> = Vec::deserialize(de)?;
        Ok(Curve { points: raw })
    }
}

/// Linear interpolation helper so the `Curve` sampler works for both scalars
/// and RGBA.
pub trait Lerp {
    fn lerp(a: Self, b: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl Lerp for [f32; 4] {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            a[3] + (b[3] - a[3]) * t,
        ]
    }
}

/// Full particle-system config, loaded from JSON and shared behind `Arc`.
#[derive(Debug, Clone, Deserialize)]
pub struct ParticleConfig {
    pub sprite: String,
    pub max_alive: u32,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default = "default_blend")]
    pub blend: BlendMode,
    pub emission: EmissionKind,
    pub lifetime: Range,
    pub initial_velocity: InitialVelocity,
    #[serde(default = "default_gravity_zero")]
    pub gravity: [f32; 2],
    #[serde(default = "default_drag_zero")]
    pub drag_per_sec: f32,
    #[serde(default = "default_zero_range")]
    pub angular_velocity: Range,
    #[serde(default = "default_one_range")]
    pub start_scale: Range,
    #[serde(default)]
    pub scale_over_life: Option<Curve<f32>>,
    #[serde(default)]
    pub color_over_life: Option<Curve<[f32; 4]>>,
    #[serde(default)]
    pub alpha_over_life: Option<Curve<f32>>,
    /// Per-emitter RGBA multiplier baked into each particle at spawn.
    #[serde(default = "default_white_rgba")]
    pub tint: [f32; 4],
}

impl ParticleConfig {
    /// Load and validate a particle config from disk. Parse errors and
    /// validation failures return typed errors the loader can surface.
    pub fn load(path: impl AsRef<Path>) -> Result<Arc<Self>, ParticleConfigError> {
        let path_ref = path.as_ref();
        let display = path_ref.display().to_string();
        let contents =
            std::fs::read_to_string(path_ref).map_err(|source| ParticleConfigError::Io {
                path: display.clone(),
                source,
            })?;
        let cfg: ParticleConfig =
            serde_json::from_str(&contents).map_err(|source| ParticleConfigError::Parse {
                path: display.clone(),
                source,
            })?;
        cfg.validate()
            .map_err(|reason| ParticleConfigError::Validation {
                path: display,
                reason,
            })?;
        Ok(Arc::new(cfg))
    }

    /// Validate the parsed struct. Returns a human-readable reason string on
    /// failure; the caller wraps it in the appropriate `ParticleConfigError`
    /// variant with the source path attached.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_alive == 0 {
            return Err("max_alive must be >= 1".into());
        }
        check_finite_range(&self.lifetime, "lifetime")?;
        if self.lifetime.max <= 0.0 {
            return Err("lifetime.max must be > 0".into());
        }
        if self.lifetime.min < 0.0 {
            return Err("lifetime.min must be >= 0".into());
        }
        if !self.drag_per_sec.is_finite() || self.drag_per_sec < 0.0 {
            return Err("drag_per_sec must be finite and >= 0".into());
        }
        check_finite_range(&self.angular_velocity, "angular_velocity")?;
        check_finite_range(&self.start_scale, "start_scale")?;
        if self.start_scale.min < 0.0 {
            return Err("start_scale.min must be >= 0".into());
        }
        for (i, g) in self.gravity.iter().enumerate() {
            if !g.is_finite() {
                return Err(format!("gravity[{i}] must be finite"));
            }
        }
        for (i, c) in self.tint.iter().enumerate() {
            if !c.is_finite() {
                return Err(format!("tint[{i}] must be finite"));
            }
        }
        match &self.emission {
            EmissionKind::Burst { count, .. } => {
                if *count == 0 {
                    return Err("emission.burst.count must be >= 1".into());
                }
            }
            EmissionKind::Continuous { rate_hz } => {
                if !rate_hz.is_finite() || *rate_hz < 0.0 {
                    return Err("emission.continuous.rate_hz must be finite and >= 0".into());
                }
            }
            EmissionKind::Pulse {
                count_per_pulse,
                interval_sec,
                ..
            } => {
                if *count_per_pulse == 0 {
                    return Err("emission.pulse.count_per_pulse must be >= 1".into());
                }
                if !interval_sec.is_finite() || *interval_sec <= 0.0 {
                    return Err("emission.pulse.interval_sec must be finite and > 0".into());
                }
            }
        }
        match &self.initial_velocity {
            InitialVelocity::Cone {
                direction,
                spread_deg,
                speed,
            } => {
                check_finite_vec2(direction, "initial_velocity.cone.direction")?;
                if !spread_deg.is_finite() {
                    return Err("initial_velocity.cone.spread_deg must be finite".into());
                }
                check_finite_range(speed, "initial_velocity.cone.speed")?;
            }
            InitialVelocity::Radial { speed } => {
                check_finite_range(speed, "initial_velocity.radial.speed")?;
            }
            InitialVelocity::Vector { direction, speed } => {
                check_finite_vec2(direction, "initial_velocity.vector.direction")?;
                check_finite_range(speed, "initial_velocity.vector.speed")?;
            }
        }
        if let Some(c) = &self.scale_over_life {
            validate_scalar_curve(c, "scale_over_life")?;
        }
        if let Some(c) = &self.alpha_over_life {
            validate_scalar_curve(c, "alpha_over_life")?;
        }
        if let Some(c) = &self.color_over_life {
            validate_rgba_curve(c, "color_over_life")?;
        }
        Ok(())
    }
}

fn check_finite_range(r: &Range, field: &str) -> Result<(), String> {
    if !r.min.is_finite() || !r.max.is_finite() {
        return Err(format!("{field}.min/max must be finite"));
    }
    if r.max < r.min {
        return Err(format!("{field}.max ({}) < {field}.min ({})", r.max, r.min));
    }
    Ok(())
}

fn check_finite_vec2(v: &[f32; 2], field: &str) -> Result<(), String> {
    if !v[0].is_finite() || !v[1].is_finite() {
        return Err(format!("{field} must be finite"));
    }
    Ok(())
}

fn validate_scalar_curve(c: &Curve<f32>, field: &str) -> Result<(), String> {
    if c.points.is_empty() {
        return Err(format!("{field} curve must have at least one point"));
    }
    let mut last_t = f32::NEG_INFINITY;
    for (i, (t, v)) in c.points.iter().enumerate() {
        if !t.is_finite() || !v.is_finite() {
            return Err(format!("{field}[{i}] has non-finite value"));
        }
        if *t < last_t {
            return Err(format!("{field} curve must be sorted by t (index {i})"));
        }
        last_t = *t;
    }
    Ok(())
}

/// Global cap on concurrent particles across all emitters. Default `10_000`.
/// Queried in the emit system and clamped against the live count tracked by
/// [`ParticleActive`]. Stored as a World resource (`D-014`).
#[derive(Debug, Clone, Copy)]
pub struct ParticleBudget {
    pub global_cap: u32,
}

impl ParticleBudget {
    pub const DEFAULT_GLOBAL_CAP: u32 = 10_000;
}

impl Default for ParticleBudget {
    fn default() -> Self {
        Self {
            global_cap: Self::DEFAULT_GLOBAL_CAP,
        }
    }
}

/// Live particle count across all emitters + orphaned particles. Written by
/// `particle_count_refresh_system`, read by `particle_emit_system` to enforce
/// [`ParticleBudget::global_cap`]. Stored as a World resource.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParticleActive {
    pub count: u32,
}

/// Monotonic world-scoped RNG seed source. Each emitter without an explicit
/// `seed` or `seed_override` derives its seed via SplitMix64 over `next` and
/// then we post-increment, so identical scene setups produce identical
/// particle streams across runs (plan: "deterministic replay").
#[derive(Debug, Clone, Copy, Default)]
pub struct WorldRngSeed {
    pub next: u64,
}

impl WorldRngSeed {
    pub fn new(start: u64) -> Self {
        Self { next: start }
    }

    /// Mint a fresh per-emitter seed. Mixes via SplitMix64 so consecutive
    /// counter values don't produce correlated emitter streams.
    pub fn derive_seed(&mut self) -> u64 {
        let n = self.next;
        self.next = self.next.wrapping_add(1);
        splitmix64(n)
    }
}

/// Runtime registry of loaded particle configs, stored as a Resource in the
/// `World`. Keeps `Arc`s so in-flight particles keep their original config
/// across a hot reload — the emitter reads the current `Arc` at emit time,
/// spawned particles capture the snapshot and are unaffected by later swaps.
#[derive(Debug, Default)]
pub struct ParticleConfigRegistry {
    by_id: HashMap<AssetId<ParticleConfig>, Arc<ParticleConfig>>,
    id_by_name: HashMap<String, AssetId<ParticleConfig>>,
    name_by_id: HashMap<AssetId<ParticleConfig>, String>,
    id_by_path: HashMap<PathBuf, AssetId<ParticleConfig>>,
    path_by_id: HashMap<AssetId<ParticleConfig>, PathBuf>,
    next_index: u32,
}

impl ParticleConfigRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a newly loaded config, minting a fresh [`AssetId`]. Duplicate
    /// string names or paths overwrite the existing entry — the caller is
    /// expected to surface that as a manifest-level error before calling.
    pub fn register(
        &mut self,
        name: String,
        path: PathBuf,
        config: Arc<ParticleConfig>,
    ) -> AssetId<ParticleConfig> {
        let id = AssetId::<ParticleConfig>::new(self.next_index);
        self.next_index += 1;
        self.id_by_name.insert(name.clone(), id);
        self.name_by_id.insert(id, name);
        self.id_by_path.insert(path.clone(), id);
        self.path_by_id.insert(id, path);
        self.by_id.insert(id, config);
        id
    }

    /// Replace an existing config's `Arc` in place. Used by hot-reload so the
    /// `AssetId` stays stable across reloads and existing emitters keep working.
    pub fn replace(&mut self, id: AssetId<ParticleConfig>, config: Arc<ParticleConfig>) {
        self.by_id.insert(id, config);
    }

    pub fn get(&self, id: AssetId<ParticleConfig>) -> Option<&Arc<ParticleConfig>> {
        self.by_id.get(&id)
    }

    pub fn id_for_name(&self, name: &str) -> Option<AssetId<ParticleConfig>> {
        self.id_by_name.get(name).copied()
    }

    pub fn name_for_id(&self, id: AssetId<ParticleConfig>) -> Option<&str> {
        self.name_by_id.get(&id).map(|s| s.as_str())
    }

    pub fn id_for_path(&self, path: &Path) -> Option<AssetId<ParticleConfig>> {
        self.id_by_path.get(path).copied()
    }

    pub fn path_for_id(&self, id: AssetId<ParticleConfig>) -> Option<&Path> {
        self.path_by_id.get(&id).map(|p| p.as_path())
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Iterate every registered particle's string name. Ordering matches the
    /// underlying `HashMap` and is not stable across runs.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.id_by_name.keys().map(|s| s.as_str())
    }
}

fn validate_rgba_curve(c: &Curve<[f32; 4]>, field: &str) -> Result<(), String> {
    if c.points.is_empty() {
        return Err(format!("{field} curve must have at least one point"));
    }
    let mut last_t = f32::NEG_INFINITY;
    for (i, (t, rgba)) in c.points.iter().enumerate() {
        if !t.is_finite() {
            return Err(format!("{field}[{i}].t non-finite"));
        }
        for (k, v) in rgba.iter().enumerate() {
            if !v.is_finite() {
                return Err(format!("{field}[{i}].rgba[{k}] non-finite"));
            }
        }
        if *t < last_t {
            return Err(format!("{field} curve must be sorted by t (index {i})"));
        }
        last_t = *t;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/assets/particle.rs"]
mod tests;
