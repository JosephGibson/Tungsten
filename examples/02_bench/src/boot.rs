//! `boot`: the cost of booting on a game's assets (redo 7.9). Before its run
//! it writes seeded assets under `target/bench-boot/<preset>/`: PNG sprites
//! of 16 to 96 px, eight-frame sheets, 16-bit mono WAV sounds, five in 117 a
//! minute long and the rest a quarter second, and a manifest naming them with
//! a boot group of the root manifest's `sans`, `green_circle` and
//! `blue_square`. The app then boots on the root manifest and that one, in
//! slices behind boot frames; its `startup:` line holds what the row
//! measures. The `bench:` counters, fixed by the seed, prove the workload.
//! A stamp of the configuration skips the writing when the files stand.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};
use tungsten::App;
use tungsten::asset_loader::LoadPlan;
use tungsten::core::assets::ResolvedManifest;
use tungsten::core::{AssetRegistry, Pcg32, Stage, World, system};

use crate::counters::{BenchCounters, FrameCounters, bench_counters_system};
use crate::knobs::{self, Bench, BenchConfig, Knob, Preset, Value};

pub(crate) static BENCH: Bench = Bench {
    name: "boot",
    workload_version: 1,
    warmup: 2,
    gpu_timing: false,
    knobs: KNOBS,
    presets: PRESETS,
    row,
    validate: knobs::no_cross_checks,
    derived,
    engine_config: knobs::keep_engine_config,
    configure,
};

const KNOBS: &[Knob] = &[
    Knob::int("sprites", 2_700, 1, 20_000)
        .scaled()
        .note("PNG sprites, 16 to 96 px; 2,700 is example 01's scale"),
    Knob::int("sheets", 40, 0, 1_000)
        .scaled()
        .note("Eight-frame sheets of 16 to 64 px frames"),
    Knob::int("sounds", 117, 0, 2_000)
        .scaled()
        .note("16-bit mono WAVs, five in 117 a minute long, the rest 0.25 s"),
    Knob::seed(),
];

const PRESETS: &[Preset] = &[
    Preset {
        name: "min",
        set: &[
            ("sprites", Value::Int(64)),
            ("sheets", Value::Int(4)),
            ("sounds", Value::Int(4)),
        ],
    },
    Preset {
        name: "default",
        set: &[],
    },
];

/// Frames in a sheet.
const SHEET_FRAMES: u32 = 8;
/// The sounds' sample rate.
const RATE: u32 = 44_100;

fn row(_cfg: &BenchConfig) -> &'static str {
    "boot"
}

fn derived(cfg: &BenchConfig) -> Json {
    json!({
        "frames": cfg.int("sheets") * i64::from(SHEET_FRAMES),
        "long_sounds": long_sounds(cfg),
    })
}

/// The sounds a minute long: five in 117, rounded down.
fn long_sounds(cfg: &BenchConfig) -> i64 {
    cfg.int("sounds") * 5 / 117
}

/// The root manifest, whose boot group's assets the row boots on.
pub(crate) fn root_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/manifest.json")
}

/// Where `cfg`'s assets go: `target/bench-boot/<preset>/`.
fn asset_dir(cfg: &BenchConfig) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/bench-boot")
        .join(cfg.preset)
}

/// Writes `cfg`'s assets and their manifest, unless the stamp says the
/// files stand for this configuration, and returns the manifest's path.
///
/// # Errors
///
/// A file that does not write.
pub(crate) fn write_assets(cfg: &BenchConfig) -> anyhow::Result<PathBuf> {
    let dir = asset_dir(cfg);
    let manifest = dir.join("manifest.json");
    let stamp = dir.join("stamp.json");
    let wanted = cfg.to_json().to_string();
    if std::fs::read_to_string(&stamp).is_ok_and(|written| written == wanted) && manifest.exists() {
        return Ok(manifest);
    }
    for sub in ["sprites", "sheets", "sounds"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }
    let mut rng = Pcg32::seeded(cfg.seed());
    let mut sprites = serde_json::Map::new();
    for index in 0..cfg.int("sprites") {
        let size = (
            rng.next_range(16.0, 97.0) as u32,
            rng.next_range(16.0, 97.0) as u32,
        );
        let path = format!("sprites/{index}.png");
        write_png(&dir.join(&path), size, colour(&mut rng))?;
        sprites.insert(format!("boot_sprite_{index}"), json!({ "path": path }));
    }
    let mut sheets = serde_json::Map::new();
    for index in 0..cfg.int("sheets") {
        let side = rng.next_range(16.0, 65.0) as u32;
        let path = format!("sheets/{index}.png");
        write_png(
            &dir.join(&path),
            (side * SHEET_FRAMES, side),
            colour(&mut rng),
        )?;
        sheets.insert(
            format!("boot_sheet_{index}"),
            json!({ "path": path, "frame": [side, side] }),
        );
    }
    let mut sounds = serde_json::Map::new();
    let long = long_sounds(cfg);
    for index in 0..cfg.int("sounds") {
        let seconds = if index < long { 60.0 } else { 0.25 };
        let hertz = rng.next_range(220.0, 880.0);
        let path = format!("sounds/{index}.wav");
        write_wav(&dir.join(&path), seconds, hertz)?;
        sounds.insert(format!("boot_sound_{index}"), json!({ "path": path }));
    }
    let text = json!({
        "sprites": sprites,
        "sheets": sheets,
        "sounds": sounds,
        "boot": { "logo": "green_circle", "bar": "blue_square", "font": "sans" },
    });
    std::fs::write(&manifest, serde_json::to_string_pretty(&text)?)?;
    std::fs::write(&stamp, wanted)?;
    Ok(manifest)
}

/// A seeded opaque colour.
fn colour(rng: &mut Pcg32) -> [u8; 4] {
    let mut channel = || rng.next_range(40.0, 240.0) as u8;
    [channel(), channel(), channel(), 255]
}

/// A `width` × `height` PNG of `fill` with a darker 1 px border.
fn write_png(path: &Path, (width, height): (u32, u32), fill: [u8; 4]) -> anyhow::Result<()> {
    let edge = [fill[0] / 2, fill[1] / 2, fill[2] / 2, 255];
    image::RgbaImage::from_fn(width, height, |x, y| {
        let border = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
        image::Rgba(if border { edge } else { fill })
    })
    .save(path)?;
    Ok(())
}

/// A 16-bit mono WAV of a `hertz` sine `seconds` long at a fifth of full
/// scale.
fn write_wav(path: &Path, seconds: f32, hertz: f32) -> std::io::Result<()> {
    let samples = (seconds * RATE as f32) as u32;
    let data = samples * 2;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    let step = std::f32::consts::TAU * hertz / RATE as f32;
    for index in 0..samples {
        let sample = ((index as f32 * step).sin() * f32::from(i16::MAX) * 0.2) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes)
}

/// The root manifest merged with `generated`, as the app boots on them.
///
/// # Errors
///
/// A manifest that does not load.
pub(crate) fn merged_manifest(generated: &Path) -> anyhow::Result<ResolvedManifest> {
    Ok(ResolvedManifest::load_and_merge_many(&[
        root_manifest(),
        generated.to_path_buf(),
    ])?)
}

/// The `bench:` counters, the same every frame: sprites, sheet frames and
/// sounds written, and the load plan's units.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct BootCounts {
    pub(crate) sprites: u64,
    pub(crate) frames: u64,
    pub(crate) sounds: u64,
    pub(crate) units: u64,
}

impl BootCounts {
    /// `cfg`'s counts, the units those of a plan of `manifest`.
    pub(crate) fn new(cfg: &BenchConfig, manifest: ResolvedManifest) -> Self {
        let mut scratch = World::new();
        scratch.insert_resource(AssetRegistry::new());
        let units = LoadPlan::new(manifest, &mut scratch).units().len() as u64;
        Self {
            sprites: cfg.int("sprites") as u64,
            frames: (cfg.int("sheets") * i64::from(SHEET_FRAMES)) as u64,
            sounds: cfg.int("sounds") as u64,
            units,
        }
    }
}

impl FrameCounters for BootCounts {
    fn write(&self, _world: &World, line: &mut String) {
        let _ = write!(
            line,
            " sprites={} frames={} sounds={} units={}",
            self.sprites, self.frames, self.sounds, self.units
        );
    }

    fn reset(&mut self) {}
}

fn configure(app: &mut App, cfg: &BenchConfig) {
    let generated = write_assets(cfg).expect("the boot row's assets write");
    let manifest = merged_manifest(&generated).expect("the boot row's manifests load");
    let counts = BootCounts::new(cfg, manifest);
    app.set_manifest_roots(vec![root_manifest(), generated]);
    app.world_mut().insert_resource(BenchCounters::new(counts));
    app.add_system_to(
        Stage::PreUpdate,
        system("bench_counters", bench_counters_system::<BootCounts>),
    );
}
