//! Knob schema and resolution (`D-078`; `docs/perf/profiling-workflow.md`).
//!
//! Values resolve in order: preset, then `TUNGSTEN_BENCH_SCALE` on the
//! scalable knobs (integers round, everything clamps to its range), then
//! `TUNGSTEN_BENCH_SET` overrides, then validation. An unknown name or an
//! out-of-range value is fatal and names the knob and its range. The binary
//! owns the schema; `TUNGSTEN_BENCH_DESCRIBE=1` prints it for the runner.

use serde_json::{Map, Value as Json, json};
use tungsten::App;
use tungsten::core::Config;

/// Knob type and valid range.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Kind {
    Int { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    Choice(&'static [&'static str]),
    Seed,
}

/// One resolved knob value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Value {
    Int(i64),
    Float(f64),
    Choice(&'static str),
    Seed(u64),
}

impl Value {
    fn to_json(self) -> Json {
        match self {
            Self::Int(v) => json!(v),
            Self::Float(v) => json!(v),
            Self::Choice(v) => json!(v),
            Self::Seed(v) => json!(v),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Knob {
    pub(crate) name: &'static str,
    pub(crate) kind: Kind,
    pub(crate) default: Value,
    /// Multiplied by `TUNGSTEN_BENCH_SCALE`.
    pub(crate) scales: bool,
    pub(crate) note: &'static str,
}

impl Knob {
    pub(crate) const fn int(name: &'static str, default: i64, min: i64, max: i64) -> Self {
        Self::new(name, Kind::Int { min, max }, Value::Int(default))
    }

    pub(crate) const fn float(name: &'static str, default: f64, min: f64, max: f64) -> Self {
        Self::new(name, Kind::Float { min, max }, Value::Float(default))
    }

    pub(crate) const fn choice(
        name: &'static str,
        default: &'static str,
        choices: &'static [&'static str],
    ) -> Self {
        Self::new(name, Kind::Choice(choices), Value::Choice(default))
    }

    /// The `seed` knob every benchmark carries.
    pub(crate) const fn seed() -> Self {
        Self::new("seed", Kind::Seed, Value::Seed(1))
    }

    pub(crate) const fn scaled(mut self) -> Self {
        self.scales = true;
        self
    }

    pub(crate) const fn note(mut self, note: &'static str) -> Self {
        self.note = note;
        self
    }

    const fn new(name: &'static str, kind: Kind, default: Value) -> Self {
        Self {
            name,
            kind,
            default,
            scales: false,
            note: "",
        }
    }

    fn range_text(&self) -> String {
        match self.kind {
            Kind::Int { min, max } => format!("an integer in {min}..={max}"),
            Kind::Float { min, max } => format!("a number in {min}..={max}"),
            Kind::Choice(choices) => format!("one of {}", choices.join(", ")),
            Kind::Seed => "an unsigned 64-bit integer".to_string(),
        }
    }

    fn parse(&self, raw: &str) -> anyhow::Result<Value> {
        let invalid = || {
            anyhow::anyhow!(
                "knob '{}' must be {}, got '{raw}'",
                self.name,
                self.range_text()
            )
        };
        let value = match self.kind {
            Kind::Int { .. } => Value::Int(raw.parse().map_err(|_| invalid())?),
            Kind::Float { .. } => {
                let parsed: f64 = raw.parse().map_err(|_| invalid())?;
                if !parsed.is_finite() {
                    return Err(invalid());
                }
                Value::Float(parsed)
            }
            Kind::Choice(choices) => Value::Choice(
                choices
                    .iter()
                    .copied()
                    .find(|choice| *choice == raw)
                    .ok_or_else(invalid)?,
            ),
            Kind::Seed => Value::Seed(raw.parse().map_err(|_| invalid())?),
        };
        Ok(value)
    }

    fn check(&self, value: Value) -> anyhow::Result<()> {
        let ok = match (self.kind, value) {
            (Kind::Int { min, max }, Value::Int(v)) => (min..=max).contains(&v),
            (Kind::Float { min, max }, Value::Float(v)) => (min..=max).contains(&v),
            (Kind::Choice(choices), Value::Choice(v)) => choices.contains(&v),
            (Kind::Seed, Value::Seed(_)) => true,
            _ => false,
        };
        if ok {
            return Ok(());
        }
        let shown = match value {
            Value::Int(v) => v.to_string(),
            Value::Float(v) => v.to_string(),
            Value::Choice(v) => v.to_string(),
            Value::Seed(v) => v.to_string(),
        };
        anyhow::bail!(
            "knob '{}' must be {}, got {shown}",
            self.name,
            self.range_text()
        )
    }

    fn scale(&self, value: Value, scale: f64) -> Value {
        match (self.kind, value) {
            (Kind::Int { min, max }, Value::Int(v)) => {
                Value::Int(((v as f64 * scale).round() as i64).clamp(min, max))
            }
            (Kind::Float { min, max }, Value::Float(v)) => {
                Value::Float((v * scale).clamp(min, max))
            }
            _ => value,
        }
    }

    fn describe(&self) -> Json {
        let mut entry = Map::new();
        entry.insert("name".into(), json!(self.name));
        match self.kind {
            Kind::Int { min, max } => {
                entry.insert("type".into(), json!("int"));
                entry.insert("min".into(), json!(min));
                entry.insert("max".into(), json!(max));
            }
            Kind::Float { min, max } => {
                entry.insert("type".into(), json!("float"));
                entry.insert("min".into(), json!(min));
                entry.insert("max".into(), json!(max));
            }
            Kind::Choice(choices) => {
                entry.insert("type".into(), json!("choice"));
                entry.insert("choices".into(), json!(choices));
            }
            Kind::Seed => {
                entry.insert("type".into(), json!("u64"));
            }
        }
        entry.insert("default".into(), self.default.to_json());
        entry.insert("scales".into(), json!(self.scales));
        entry.insert("note".into(), json!(self.note));
        Json::Object(entry)
    }
}

/// Named knob set applied before scale and overrides.
#[derive(Debug)]
pub(crate) struct Preset {
    pub(crate) name: &'static str,
    pub(crate) set: &'static [(&'static str, Value)],
}

/// Runner-side check over every measured frame; an unmet guard invalidates
/// the capture.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Guard {
    /// A `physics:` field never exceeds `max`.
    PhysicsMax { field: &'static str, max: i64 },
    /// A `bench:` counter never drops below `min`.
    CounterMin { counter: &'static str, min: i64 },
    /// A `bench:` counter never exceeds `max`.
    CounterMax { counter: &'static str, max: i64 },
    /// A `bench:` counter keeps its first measured value.
    CounterConst { counter: &'static str },
    /// Two `bench:` counters are equal in every measured frame.
    CounterEq {
        counter: &'static str,
        other: &'static str,
    },
    /// A `bench:` counter stays within `tolerance` (a share) of its median
    /// over the measured frames, so the band follows the knobs.
    CounterBand {
        counter: &'static str,
        tolerance: f64,
    },
}

impl Guard {
    fn describe(self) -> Json {
        match self {
            Self::PhysicsMax { field, max } => {
                json!({"kind": "physics_max", "field": field, "max": max})
            }
            Self::CounterMin { counter, min } => {
                json!({"kind": "counter_min", "counter": counter, "min": min})
            }
            Self::CounterMax { counter, max } => {
                json!({"kind": "counter_max", "counter": counter, "max": max})
            }
            Self::CounterConst { counter } => json!({"kind": "counter_const", "counter": counter}),
            Self::CounterEq { counter, other } => {
                json!({"kind": "counter_eq", "counter": counter, "other": other})
            }
            Self::CounterBand { counter, tolerance } => {
                json!({"kind": "counter_band", "counter": counter, "tolerance": tolerance})
            }
        }
    }
}

/// A tracked row: the preset that selects it, the metrics it is judged on
/// (`stage.<frame key>` or `system.<name>` with their statistics), its guards
/// and its `bench:` counters in line order.
#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) name: &'static str,
    pub(crate) preset: &'static str,
    pub(crate) owned: &'static [(&'static str, &'static [&'static str])],
    pub(crate) guards: &'static [Guard],
    pub(crate) counters: &'static [&'static str],
    /// The limiting stage capacity search expects at the budget:
    /// `system.<name>`, `update` (any system), `stage.flush`,
    /// `stage.particles`, `stage.extract`, `stage.render_encode`, `present`
    /// (acquire plus submit/present) or `stage.unattributed`.
    pub(crate) bottleneck: &'static str,
    /// Knobs whose resolved values capacity search reports as the row's key
    /// counts; the scaled ones also bound its `scale` axis.
    pub(crate) key_knobs: &'static [&'static str],
    /// Caveat shown with the row by `describe`; empty when there is none.
    pub(crate) note: &'static str,
}

/// One benchmark: its schema plus the hooks `main` dispatches to.
pub(crate) struct Bench {
    pub(crate) name: &'static str,
    /// Bumped by any bench change that alters the work; engine changes never do.
    pub(crate) workload_version: u32,
    pub(crate) warmup: u32,
    /// Whether captures add a `TUNGSTEN_GPU_TIMING=1` diagnostic run by default.
    pub(crate) gpu_timing: bool,
    pub(crate) knobs: &'static [Knob],
    pub(crate) presets: &'static [Preset],
    pub(crate) rows: &'static [Row],
    /// Row a resolved config measures.
    pub(crate) row: fn(&BenchConfig) -> &'static str,
    /// Cross-knob checks after every knob is in range.
    pub(crate) validate: fn(&BenchConfig) -> anyhow::Result<()>,
    /// The `derived` object of `bench-config`; must not need a window.
    pub(crate) derived: fn(&BenchConfig) -> Json,
    /// Engine config edits before the window opens; `main` has already set
    /// the 1920x1080 viewport and vsync off.
    pub(crate) engine_config: fn(&mut Config, &BenchConfig),
    pub(crate) configure: fn(&mut App, &BenchConfig),
}

/// `Bench::engine_config` for benchmarks that keep `main`'s engine config.
pub(crate) fn keep_engine_config(_config: &mut Config, _cfg: &BenchConfig) {}

/// `Bench::validate` for benchmarks without cross-knob checks.
#[allow(clippy::unnecessary_wraps)] // Signature fixed by `Bench::validate`.
pub(crate) fn no_cross_checks(_cfg: &BenchConfig) -> anyhow::Result<()> {
    Ok(())
}

/// Sets engine config `field` to knob `knob`'s `value` unless environment
/// variable `var` already set it: the environment wins, so the smoke matrix
/// can pin render settings, and `bench` logs a warning when it does.
pub(crate) fn unless_env<T: std::fmt::Debug>(
    bench: &str,
    field: &mut T,
    var: &str,
    knob: &str,
    value: T,
) {
    if std::env::var_os(var).is_some() {
        log::warn!("{bench}: {var} overrides knob '{knob}' ({value:?}); using {field:?}");
    } else {
        *field = value;
    }
}

impl std::fmt::Debug for Bench {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bench")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Bench {
    fn knob_index(&self, name: &str) -> anyhow::Result<usize> {
        self.knobs
            .iter()
            .position(|knob| knob.name == name)
            .ok_or_else(|| {
                let names: Vec<_> = self.knobs.iter().map(|knob| knob.name).collect();
                anyhow::anyhow!(
                    "unknown knob '{name}' for {}; knobs: {}",
                    self.name,
                    names.join(", ")
                )
            })
    }

    fn describe(&self) -> Json {
        let presets: Vec<Json> = self
            .presets
            .iter()
            .map(|preset| {
                let set: Map<String, Json> = preset
                    .set
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), value.to_json()))
                    .collect();
                json!({"name": preset.name, "set": set})
            })
            .collect();
        let rows: Vec<Json> = self
            .rows
            .iter()
            .map(|row| {
                let owned: Vec<Json> = row
                    .owned
                    .iter()
                    .map(|(metric, stats)| json!({"metric": metric, "stats": stats}))
                    .collect();
                let guards: Vec<Json> = row.guards.iter().map(|guard| guard.describe()).collect();
                json!({
                    "name": row.name,
                    "preset": row.preset,
                    "owned": owned,
                    "guards": guards,
                    "counters": row.counters,
                    "bottleneck": row.bottleneck,
                    "key_knobs": row.key_knobs,
                    "note": row.note,
                })
            })
            .collect();
        json!({
            "name": self.name,
            "workload_version": self.workload_version,
            "warmup": self.warmup,
            "gpu_timing": self.gpu_timing,
            "knobs": self.knobs.iter().map(Knob::describe).collect::<Vec<_>>(),
            "presets": presets,
            "rows": rows,
        })
    }
}

/// A fully resolved and validated benchmark configuration.
#[derive(Debug)]
pub(crate) struct BenchConfig {
    pub(crate) bench: &'static Bench,
    pub(crate) preset: &'static str,
    pub(crate) scale: f64,
    values: Vec<Value>,
}

impl BenchConfig {
    fn value(&self, name: &str) -> Value {
        let index = self
            .bench
            .knobs
            .iter()
            .position(|knob| knob.name == name)
            .unwrap_or_else(|| panic!("{} has no knob '{name}'", self.bench.name));
        self.values[index]
    }

    pub(crate) fn int(&self, name: &str) -> i64 {
        match self.value(name) {
            Value::Int(v) => v,
            other => panic!("knob '{name}' is not an integer: {other:?}"),
        }
    }

    pub(crate) fn float(&self, name: &str) -> f64 {
        match self.value(name) {
            Value::Float(v) => v,
            other => panic!("knob '{name}' is not a number: {other:?}"),
        }
    }

    pub(crate) fn choice(&self, name: &str) -> &'static str {
        match self.value(name) {
            Value::Choice(v) => v,
            other => panic!("knob '{name}' is not a choice: {other:?}"),
        }
    }

    pub(crate) fn seed(&self) -> u64 {
        match self.value("seed") {
            Value::Seed(v) => v,
            other => panic!("knob 'seed' is not a seed: {other:?}"),
        }
    }

    pub(crate) fn row(&self) -> &'static str {
        (self.bench.row)(self)
    }

    /// The object `bench-config:` logs and `TUNGSTEN_BENCH_DESCRIBE=config` prints.
    pub(crate) fn to_json(&self) -> Json {
        let knobs: Map<String, Json> = self
            .bench
            .knobs
            .iter()
            .zip(&self.values)
            .map(|(knob, value)| (knob.name.to_string(), value.to_json()))
            .collect();
        json!({
            "bench": self.bench.name,
            "row": self.row(),
            "workload_version": self.bench.workload_version,
            "preset": self.preset,
            "scale": self.scale,
            "seed": self.seed(),
            "knobs": knobs,
            "derived": (self.bench.derived)(self),
        })
    }
}

/// Resolve `bench` from the raw `TUNGSTEN_BENCH_PRESET`, `_SCALE` and `_SET` values.
pub(crate) fn resolve(
    bench: &'static Bench,
    preset: Option<&str>,
    scale: Option<&str>,
    set: Option<&str>,
) -> anyhow::Result<BenchConfig> {
    let preset_name = preset.unwrap_or("default");
    let preset = bench
        .presets
        .iter()
        .find(|candidate| candidate.name == preset_name)
        .ok_or_else(|| {
            let names: Vec<_> = bench.presets.iter().map(|preset| preset.name).collect();
            anyhow::anyhow!(
                "unknown preset '{preset_name}' for {}; presets: {}",
                bench.name,
                names.join(", ")
            )
        })?;
    let mut values: Vec<Value> = bench.knobs.iter().map(|knob| knob.default).collect();
    for (name, value) in preset.set {
        values[bench.knob_index(name)?] = *value;
    }

    let scale = match scale {
        None => 1.0,
        Some(raw) => raw
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .ok_or_else(|| {
                anyhow::anyhow!("TUNGSTEN_BENCH_SCALE must be a positive number, got '{raw}'")
            })?,
    };
    for (knob, value) in bench.knobs.iter().zip(values.iter_mut()) {
        if knob.scales {
            *value = knob.scale(*value, scale);
        }
    }

    for entry in set
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (name, raw) = entry.split_once('=').ok_or_else(|| {
            anyhow::anyhow!("TUNGSTEN_BENCH_SET entry '{entry}' is not knob=value")
        })?;
        let index = bench.knob_index(name.trim())?;
        values[index] = bench.knobs[index].parse(raw.trim())?;
    }

    for (knob, value) in bench.knobs.iter().zip(&values) {
        knob.check(*value)?;
    }
    let config = BenchConfig {
        bench,
        preset: preset.name,
        scale,
        values,
    };
    (bench.validate)(&config)?;
    Ok(config)
}

/// Pick the benchmark named by `TUNGSTEN_BENCH` (default `physics`).
pub(crate) fn select(
    benches: &[&'static Bench],
    name: Option<&str>,
) -> anyhow::Result<&'static Bench> {
    let name = name.unwrap_or("physics");
    benches
        .iter()
        .copied()
        .find(|bench| bench.name == name)
        .ok_or_else(|| {
            let names: Vec<_> = benches.iter().map(|bench| bench.name).collect();
            anyhow::anyhow!(
                "unknown benchmark '{name}' in TUNGSTEN_BENCH; benchmarks: {}",
                names.join(", ")
            )
        })
}

/// Resolve the configuration from the `TUNGSTEN_BENCH*` environment.
pub(crate) fn from_env(benches: &[&'static Bench]) -> anyhow::Result<BenchConfig> {
    let var = |name: &str| std::env::var(name).ok();
    let bench = select(benches, var("TUNGSTEN_BENCH").as_deref())?;
    resolve(
        bench,
        var("TUNGSTEN_BENCH_PRESET").as_deref(),
        var("TUNGSTEN_BENCH_SCALE").as_deref(),
        var("TUNGSTEN_BENCH_SET").as_deref(),
    )
}

/// The `TUNGSTEN_BENCH_DESCRIBE=1` schema of every benchmark.
pub(crate) fn describe(benches: &[&'static Bench]) -> Json {
    json!({
        "schema": 1,
        "benches": benches.iter().map(|bench| bench.describe()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
#[path = "tests/knobs.rs"]
mod tests;
