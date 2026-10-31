use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

use crate::assets::material::MaterialUniformDefaults;
use crate::assets::particle::ParticleMesh;
use crate::audio::{AudioBus, MAX_VOICES};
use crate::physics::PhysicsMaterial;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read manifest '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid manifest '{path}': {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("sprite '{id}' references missing file: {path}")]
    MissingFile { id: String, path: String },
    #[error("sprite '{id}' references missing normal_map file: {path}")]
    MissingNormalMapFile { id: String, path: String },
    #[error("sprite '{id}' references missing emissive_mask file: {path}")]
    MissingEmissiveMaskFile { id: String, path: String },
    #[error("animation '{id}' references missing file: {path}")]
    MissingAnimationFile { id: String, path: String },
    #[error("font '{id}' references missing file: {path}")]
    MissingFontFile { id: String, path: String },
    #[error("sound '{id}' references missing file: {path}")]
    MissingSoundFile { id: String, path: String },
    #[error("tilemap '{id}' references missing file: {path}")]
    MissingTilemapFile { id: String, path: String },
    #[error("particle '{id}' references missing file: {path}")]
    MissingParticleFile { id: String, path: String },
    #[error("shader '{id}' references missing file: {path}")]
    MissingShaderFile { id: String, path: String },
    #[error("ui theme '{id}' references missing file: {path}")]
    MissingUiThemeFile { id: String, path: String },
    /// A `prefabs` entry names a file that does not exist (M51).
    #[error("prefab '{id}' references missing file: {path}")]
    MissingPrefabFile {
        /// The prefab's ID.
        id: String,
        /// The file the entry resolves to.
        path: String,
    },
    /// A `data` entry names a file that does not exist (M61).
    #[error("data `{kind}` entry '{id}' references missing file: {path}")]
    MissingDataFile {
        /// The entry's kind.
        kind: String,
        /// The entry's ID.
        id: String,
        /// The file the entry resolves to.
        path: String,
    },
    /// One file backs two `prefabs` or `data` entries, in one manifest or
    /// across roots; an edit to it could reload only one (M61). Of several,
    /// the smallest pair is named.
    #[error("'{path}' backs both {first} and {second}; a file backs one prefab or data entry")]
    SharedFile {
        /// The shared file.
        path: String,
        /// The smaller entry, as `prefabs.<id>` or `data.<kind>.<id>`.
        first: String,
        /// The other entry.
        second: String,
    },
    #[error("material '{id}' references unknown shader '{shader}'")]
    MaterialShaderMissing { id: String, shader: String },
    #[error("particle mesh '{id}' is invalid: {reason}")]
    InvalidParticleMesh { id: String, reason: String },
    #[error("duplicate asset ID '{id}' across manifests")]
    DuplicateId { id: String },
    #[error("font family '{family}' references unknown font '{face}'")]
    FontFamilyFaceMissing { family: String, face: String },
    #[error("font_fallback names unknown font family '{family}'")]
    UnknownFallbackFamily { family: String },
    /// A `physics_layers` entry's name is not `[a-z][a-z0-9_]*` or its
    /// layer is not 0 to 31.
    #[error("physics layer '{name}' is invalid: {reason}")]
    InvalidPhysicsLayer {
        /// The name the entry gives.
        name: String,
        /// What is wrong with it.
        reason: String,
    },
    /// Two `physics_layers` names give one layer, in one manifest or across
    /// them.
    #[error("physics layer {layer} is named both '{first}' and '{second}'")]
    PhysicsLayerTaken {
        /// The layer, 0 to 31.
        layer: u32,
        /// The name that took it first, or the smaller in one manifest.
        first: String,
        /// The other name.
        second: String,
    },
    /// A `physics_materials` entry's coefficient is out of range.
    #[error("physics material '{id}' is invalid: {reason}")]
    InvalidPhysicsMaterial {
        /// The material's name.
        id: String,
        /// What is wrong with it.
        reason: String,
    },
    /// An entry's path is absolute or climbs out of its manifest's folder
    /// with `..`; every load refuses it, so a watcher never watches outside.
    #[error("{id} names '{path}', which is absolute or leaves its manifest's folder")]
    PathEscapes {
        /// The entry, as [`ManifestError::OutsideFolder`] names it.
        id: String,
        /// The path as the manifest wrote it.
        path: String,
    },
    /// A root or an entry resolves outside the game's folder, which
    /// [`ResolvedManifest::load_for_check`] bounds every path by (M70).
    #[error("{id} resolves to '{path}', outside the game's folder")]
    OutsideFolder {
        /// The entry as `<section>.<id>` (`data.<kind>.<id>` for a data
        /// entry, `sprites.<id>.normal_map` for a sprite's sibling map), or
        /// `manifest` for a root.
        id: String,
        /// The canonical path outside the folder.
        path: String,
    },
}

/// An error [`ResolvedManifest::load_for_check`] found, with the manifest
/// to fix: the root that declares the entry it is about, or the root that
/// did not read or parse (M70).
#[derive(Debug)]
#[non_exhaustive]
pub struct ManifestIssue {
    /// The manifest file, as the roots name it.
    pub manifest: PathBuf,
    /// What is wrong, as `load_and_merge_many` would say it.
    pub error: ManifestError,
}

/// Where the loading bodies send an error. `load`, `load_and_merge_many`
/// and `merge` stop at the first; `load_for_check` keeps each one with the
/// manifest to fix and goes on, leaving the entry out.
struct Sink<'a> {
    every: Option<Every<'a>>,
}

/// `load_for_check`'s side of a [`Sink`].
struct Every<'a> {
    issues: &'a mut Vec<ManifestIssue>,
    /// The game's folder, canonical: every root and entry lies inside it.
    folder: PathBuf,
    /// The root being loaded, as the roots name it.
    root: PathBuf,
    /// The root that first declared each entry a cross reference names, by
    /// `<section>.<id>`.
    declared: HashMap<String, PathBuf>,
}

impl Sink<'_> {
    /// The first-error loaders' sink.
    fn first() -> Sink<'static> {
        Sink { every: None }
    }

    /// Reports `error` about the root being loaded: an `Err` that stops a
    /// first-error loader, or `Ok` once kept.
    fn report(&mut self, error: ManifestError) -> Result<(), ManifestError> {
        match &mut self.every {
            None => Err(error),
            Some(every) => {
                every.issues.push(ManifestIssue {
                    manifest: every.root.clone(),
                    error,
                });
                Ok(())
            }
        }
    }

    /// Reports `error` about the entry `key`, naming the root that declared
    /// it.
    fn report_entry(&mut self, key: &str, error: ManifestError) -> Result<(), ManifestError> {
        match &mut self.every {
            None => Err(error),
            Some(every) => {
                let manifest = every
                    .declared
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| every.root.clone());
                every.issues.push(ManifestIssue { manifest, error });
                Ok(())
            }
        }
    }

    /// The folder every path must lie inside, when checking.
    fn folder(&self) -> Option<&Path> {
        self.every.as_ref().map(|every| every.folder.as_path())
    }
}

/// Whether `rel` stays inside the folder it is relative to: not absolute,
/// and no `..` that climbs above where it starts.
fn stays_inside(rel: &Path) -> bool {
    let mut depth = 0_usize;
    for component in rel.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => match depth.checked_sub(1) {
                Some(up) => depth = up,
                None => return false,
            },
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Resolves `rel` against `base_dir` to its canonical file, or reports it
/// escaping its manifest's folder, missing (`missing` builds that error from
/// the path) or outside the sink's folder (`entry` names the entry) and
/// gives `None`.
fn resolve_file(
    base_dir: &Path,
    rel: &str,
    sink: &mut Sink<'_>,
    entry: impl FnOnce() -> String,
    missing: impl FnOnce(String) -> ManifestError,
) -> Result<Option<PathBuf>, ManifestError> {
    if !stays_inside(Path::new(rel)) {
        sink.report(ManifestError::PathEscapes {
            id: entry(),
            path: rel.to_string(),
        })?;
        return Ok(None);
    }
    let full_path = base_dir.join(rel);
    if !full_path.exists() {
        sink.report(missing(full_path.display().to_string()))?;
        return Ok(None);
    }
    let full_path = full_path.canonicalize().unwrap_or(full_path);
    if let Some(folder) = sink.folder()
        && !full_path.starts_with(folder)
    {
        sink.report(ManifestError::OutsideFolder {
            id: entry(),
            path: full_path.display().to_string(),
        })?;
        return Ok(None);
    }
    Ok(Some(full_path))
}

/// Raw manifest JSON. An ID repeated inside one section is a parse error
/// (`D-017`); `merge` refuses one repeated across manifests.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawManifest {
    #[serde(default, deserialize_with = "unique_ids")]
    pub sprites: HashMap<String, SpriteEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub animations: HashMap<String, AnimationEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub fonts: HashMap<String, FontEntry>,
    #[serde(default, deserialize_with = "sound_entries")]
    pub sounds: HashMap<String, SoundEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub tilemaps: HashMap<String, TilemapEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub particles: HashMap<String, ParticleEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub shaders: HashMap<String, ShaderEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub materials: HashMap<String, MaterialEntry>,
    #[serde(default, deserialize_with = "unique_ids")]
    pub particle_meshes: HashMap<String, ParticleMeshEntry>,
    /// Font families: a family ID and the `fonts` face IDs it groups (`D-115`).
    #[serde(default, deserialize_with = "unique_ids")]
    pub font_families: HashMap<String, FontFamilyEntry>,
    /// Family IDs in order of preference for glyphs a style's family lacks.
    #[serde(default)]
    pub font_fallback: Vec<String>,
    /// UI themes: a theme ID and the theme file it loads (M2, w01 §14).
    #[serde(default, deserialize_with = "unique_ids")]
    pub ui_themes: HashMap<String, UiThemeEntry>,
    /// Prefabs: a prefab ID and the prefab file it loads (M51).
    #[serde(default, deserialize_with = "unique_ids")]
    pub prefabs: HashMap<String, PrefabEntry>,
    /// Data entries by kind, then ID, each with the file it loads (M61). A
    /// kind, or an ID within a kind, repeated in one manifest is a parse
    /// error; a plugin registers each kind.
    #[serde(default, deserialize_with = "data_sections")]
    pub data: BTreeMap<String, BTreeMap<String, DataEntry>>,
    /// Physics layer names: a name, `[a-z][a-z0-9_]*`, and its layer, 0 to
    /// 31 (`D-178`). A name or a layer given twice, in one manifest or
    /// across them, fails the load.
    #[serde(default, deserialize_with = "unique_ids")]
    pub physics_layers: HashMap<String, u32>,
    /// Physics materials: a name and its contact coefficients; `materials`
    /// names the render's (`D-058`).
    #[serde(default, deserialize_with = "unique_ids")]
    pub physics_materials: HashMap<String, PhysicsMaterialEntry>,
}

/// A `physics_materials` entry: each coefficient optional, unknown fields
/// failing. `friction` finite and at least 0, `= 0`; `restitution` in
/// 0..=1, `= 0`; `rolling` finite and at least 0, `= 0`.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PhysicsMaterialEntry {
    /// Coulomb friction.
    pub friction: Option<f32>,
    /// Restitution.
    pub restitution: Option<f32>,
    /// Rolling resistance.
    pub rolling: Option<f32>,
}

impl PhysicsMaterialEntry {
    /// The material, or why a coefficient is out of range.
    fn resolve(self) -> Result<PhysicsMaterial, String> {
        let at_least_zero = |field: &str, value: Option<f32>| match value {
            Some(value) if !(value.is_finite() && value >= 0.0) => Err(format!(
                "`{field}` must be finite and at least 0 (got {value})"
            )),
            value => Ok(value.unwrap_or(0.0)),
        };
        let restitution = match self.restitution {
            Some(value) if !(0.0..=1.0).contains(&value) => {
                return Err(format!("`restitution` must be in 0..=1 (got {value})"));
            }
            value => value.unwrap_or(0.0),
        };
        Ok(PhysicsMaterial {
            friction: at_least_zero("friction", self.friction)?,
            restitution,
            rolling: at_least_zero("rolling", self.rolling)?,
        })
    }
}

/// Whether `name` is `[a-z][a-z0-9_]*`.
fn valid_layer_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpriteEntry {
    pub path: String,
    #[serde(default = "default_filter")]
    pub filter: FilterMode,
    /// M29 sibling tangent-space normal map; same dimensions as `path`.
    #[serde(default)]
    pub normal_map: Option<String>,
    /// M29 sibling emissive mask; alpha or luma authored, decoded into RGB.
    #[serde(default)]
    pub emissive_mask: Option<String>,
    /// M2 nine-slice insets `[left, top, right, bottom]` in source pixels: a
    /// UI image of this sprite keeps the corners at source size and
    /// stretches the edges and the centre (Q5).
    #[serde(default)]
    pub nine_slice: Option<[u32; 4]>,
}

/// A `ui_themes` entry: the theme file's path, relative to the manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct UiThemeEntry {
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterMode {
    Nearest,
    Linear,
}

fn default_filter() -> FilterMode {
    FilterMode::Nearest
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimationEntry {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FontEntry {
    pub path: String,
}

/// A font family entry: the `fonts` IDs of its faces (`D-115`).
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct FontFamilyEntry {
    /// Face IDs from the `fonts` section; a style picks one by weight and
    /// italic.
    pub faces: Vec<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct SoundEntry {
    pub path: String,
    /// Default loop flag.
    #[serde(default)]
    pub looping: bool,
    /// Base volume before master volume: finite and at least 0.
    #[serde(default = "default_volume", deserialize_with = "sound_volume")]
    pub volume: f32,
    /// The bus it plays on, `"music"` or `"effects"`; effects by default.
    #[serde(default)]
    pub bus: AudioBus,
    /// The voices of it that play at once, 1 to [`MAX_VOICES`]; that many
    /// by default, so only the mixer's pool bounds a sound that names none.
    #[serde(default = "default_max_voices", deserialize_with = "sound_max_voices")]
    pub max_voices: u32,
}

/// The `sounds` section: entries by ID, refusing a repeated ID, with a bad
/// entry's error naming its ID.
fn sound_entries<'de, D>(deserializer: D) -> Result<HashMap<String, SoundEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Sounds;

    impl<'de> serde::de::Visitor<'de> for Sounds {
        type Value = HashMap<String, SoundEntry>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object of sounds by ID")
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut entries = HashMap::with_capacity(map.size_hint().unwrap_or(0));
            while let Some(id) = map.next_key::<String>()? {
                if entries.contains_key(&id) {
                    return Err(serde::de::Error::custom(format_args!(
                        "duplicate ID `{id}` in one section"
                    )));
                }
                // Read as a value first, so the entry's own error carries
                // the ID and the file's position once.
                let value: serde_json::Value = map.next_value()?;
                let entry = serde_json::from_value(value).map_err(|error| {
                    serde::de::Error::custom(format_args!("sound `{id}`: {error}"))
                })?;
                entries.insert(id, entry);
            }
            Ok(entries)
        }
    }

    deserializer.deserialize_map(Sounds)
}

/// A manifest section, entries by ID, refusing an ID it repeats: serde's
/// map would keep the last one (`D-017`).
fn unique_ids<'de, D, T>(deserializer: D) -> Result<HashMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Section<T>(std::marker::PhantomData<T>);

    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Section<T> {
        type Value = HashMap<String, T>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object of entries by ID")
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut entries = HashMap::with_capacity(map.size_hint().unwrap_or(0));
            while let Some(id) = map.next_key::<String>()? {
                if entries.contains_key(&id) {
                    return Err(serde::de::Error::custom(format_args!(
                        "duplicate ID `{id}` in one section"
                    )));
                }
                let entry = map.next_value()?;
                entries.insert(id, entry);
            }
            Ok(entries)
        }
    }

    deserializer.deserialize_map(Section(std::marker::PhantomData))
}

/// The `data` section: kinds, each an object of entries by ID, refusing a
/// kind or an ID within a kind that it repeats.
fn data_sections<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, BTreeMap<String, DataEntry>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Kinds;

    impl<'de> serde::de::Visitor<'de> for Kinds {
        type Value = BTreeMap<String, BTreeMap<String, DataEntry>>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object of data kinds")
        }

        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut kinds = BTreeMap::new();
            while let Some(kind) = map.next_key::<String>()? {
                if kinds.contains_key(&kind) {
                    return Err(serde::de::Error::custom(format_args!(
                        "duplicate data kind `{kind}` in one manifest"
                    )));
                }
                let DataSection(entries) = map.next_value()?;
                kinds.insert(kind, entries);
            }
            Ok(kinds)
        }
    }

    deserializer.deserialize_map(Kinds)
}

/// One data kind's entries by ID, refusing a repeated ID.
struct DataSection(BTreeMap<String, DataEntry>);

impl<'de> Deserialize<'de> for DataSection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries: HashMap<String, DataEntry> = unique_ids(deserializer)?;
        Ok(Self(entries.into_iter().collect()))
    }
}

/// A sound's `volume`, which the mixer multiplies by: finite and at least
/// 0 (`1e39` narrows to an infinite `f32`).
fn sound_volume<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let volume = f32::deserialize(deserializer)?;
    if volume.is_finite() && volume >= 0.0 {
        Ok(volume)
    } else {
        Err(serde::de::Error::custom(format!(
            "sound volume {volume} is not a finite number of at least 0"
        )))
    }
}

fn default_volume() -> f32 {
    1.0
}

/// A sound's `max_voices`: 1 to [`MAX_VOICES`].
fn sound_max_voices<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let limit = u64::deserialize(deserializer)?;
    if (1..=u64::from(MAX_VOICES)).contains(&limit) {
        Ok(limit as u32)
    } else {
        Err(serde::de::Error::custom(format!(
            "max_voices {limit} is outside 1..={MAX_VOICES}"
        )))
    }
}

fn default_max_voices() -> u32 {
    MAX_VOICES
}

#[derive(Debug, Clone, Deserialize)]
pub struct TilemapEntry {
    pub path: String,
}

/// A `prefabs` entry: the prefab file's path, relative to the manifest.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct PrefabEntry {
    /// The prefab file, relative to the manifest.
    pub path: String,
}

/// A `data` entry: the data file's path, relative to the manifest (M61).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct DataEntry {
    /// The data file, relative to the manifest.
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParticleEntry {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShaderEntry {
    pub path: String,
}

/// M26 material entry: a WGSL shader id plus an authored `UniformOverrideBlock`
/// default payload. Material serialisation lives inside the manifest — there
/// is no standalone per-material JSON file in M26.
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialEntry {
    /// Stable shader id (must exist under `shaders`).
    pub shader: String,
    #[serde(default)]
    pub uniform_defaults: MaterialUniformDefaults,
}

/// M31 particle mesh entry (`D-093`): inline geometry, like `materials`; there
/// is no standalone mesh file.
pub type ParticleMeshEntry = ParticleMesh;

/// D-052 loaded merged manifest resource.
#[derive(Debug, Clone, Default)]
pub struct LoadedManifest(pub ResolvedManifest);

impl LoadedManifest {
    #[must_use]
    pub fn new(manifest: ResolvedManifest) -> Self {
        Self(manifest)
    }

    #[must_use]
    pub fn as_resolved(&self) -> &ResolvedManifest {
        &self.0
    }
}

/// Manifest with resolved paths.
#[derive(Debug, Clone, Default)]
pub struct ResolvedManifest {
    pub sprites: HashMap<String, ResolvedSprite>,
    pub animations: HashMap<String, ResolvedAnimation>,
    pub fonts: HashMap<String, ResolvedFont>,
    pub sounds: HashMap<String, ResolvedSound>,
    pub tilemaps: HashMap<String, ResolvedTilemap>,
    pub particles: HashMap<String, ResolvedParticle>,
    pub shaders: HashMap<String, ResolvedShader>,
    pub materials: HashMap<String, ResolvedMaterial>,
    pub particle_meshes: HashMap<String, ResolvedParticleMesh>,
    /// Font families by family ID (`D-115`).
    pub font_families: HashMap<String, ResolvedFontFamily>,
    /// The fallback chain: family IDs in order of preference, each once.
    pub font_fallback: Vec<String>,
    /// UI themes by ID (M2).
    pub ui_themes: HashMap<String, ResolvedUiTheme>,
    /// Prefabs by ID (M51).
    pub prefabs: HashMap<String, ResolvedPrefab>,
    /// Data entries by kind, then ID (M61).
    pub data: BTreeMap<String, BTreeMap<String, ResolvedDataEntry>>,
    /// Physics layer names and their layers, 0 to 31 (`D-178`).
    pub physics_layers: HashMap<String, u32>,
    /// Physics materials by name.
    pub physics_materials: HashMap<String, PhysicsMaterial>,
}

#[derive(Debug, Clone)]
pub struct ResolvedSprite {
    pub path: PathBuf,
    pub filter: FilterMode,
    pub normal_path: Option<PathBuf>,
    pub emissive_path: Option<PathBuf>,
    /// Nine-slice insets `[left, top, right, bottom]` in source pixels.
    pub nine_slice: Option<[u32; 4]>,
}

/// A UI theme: the theme file's resolved path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedUiTheme {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedAnimation {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedFont {
    pub path: PathBuf,
}

/// A font family: the `fonts` IDs of its faces, as the manifest lists them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFontFamily {
    /// Face IDs from the `fonts` section.
    pub faces: Vec<String>,
}

impl ResolvedFontFamily {
    /// A family of these face IDs.
    #[must_use]
    pub fn new(faces: Vec<String>) -> Self {
        Self { faces }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ResolvedSound {
    pub path: PathBuf,
    pub looping: bool,
    pub volume: f32,
    /// The bus it plays on.
    pub bus: AudioBus,
    /// The voices of it that play at once, 1 to [`MAX_VOICES`].
    pub max_voices: u32,
}

#[derive(Debug, Clone)]
pub struct ResolvedTilemap {
    pub path: PathBuf,
}

/// A prefab: the prefab file's resolved path.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ResolvedPrefab {
    /// The prefab file, canonical when it resolves.
    pub path: PathBuf,
}

impl ResolvedPrefab {
    /// A prefab loaded from `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

/// A data entry: the data file's resolved path (M61).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ResolvedDataEntry {
    /// The data file, canonical when it resolves.
    pub path: PathBuf,
}

impl ResolvedDataEntry {
    /// A data entry loaded from `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedParticle {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedShader {
    pub path: PathBuf,
}

/// Resolved material: shader resolution is deferred to asset load (ID
/// allocation is a runtime, not a parse-time, concern).
#[derive(Debug, Clone)]
pub struct ResolvedMaterial {
    /// Manifest path that this entry was parsed from; fed back into the
    /// hot-reload routing to locate the source manifest after an edit.
    pub source_manifest: PathBuf,
    /// Name of the shader entry this material targets; resolved to a
    /// `ShaderAssetId` by `tungsten::asset_loader::load_materials`.
    pub shader: String,
    pub uniform_defaults: MaterialUniformDefaults,
}

/// Resolved particle mesh: validated geometry plus the manifest it came from.
#[derive(Debug, Clone)]
pub struct ResolvedParticleMesh {
    /// Manifest path that this entry was parsed from.
    pub source_manifest: PathBuf,
    pub mesh: ParticleMesh,
}

impl ResolvedManifest {
    /// Load one manifest, resolve paths relative to its parent and check that
    /// every material's shader is declared in the same file. Roots that refer
    /// to each other load through [`Self::load_and_merge_many`].
    pub fn load(manifest_path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let mut sink = Sink::first();
        let result = Self::load_unvalidated(manifest_path.as_ref(), &mut sink)?;
        result.validate_cross_refs(&mut sink)?;
        Ok(result)
    }

    /// Parse one manifest and resolve its paths, leaving references between
    /// entries unchecked. A checking sink takes a root that does not read or
    /// parse as an empty manifest, and leaves out each entry it reports.
    fn load_unvalidated(manifest_path: &Path, sink: &mut Sink<'_>) -> Result<Self, ManifestError> {
        let contents = match std::fs::read_to_string(manifest_path) {
            Ok(contents) => contents,
            Err(e) => {
                sink.report(ManifestError::Io {
                    path: manifest_path.display().to_string(),
                    source: e,
                })?;
                return Ok(Self::default());
            }
        };

        let raw: RawManifest = match serde_json::from_str(&contents) {
            Ok(raw) => raw,
            Err(e) => {
                sink.report(ManifestError::Parse {
                    path: manifest_path.display().to_string(),
                    source: e,
                })?;
                return Ok(Self::default());
            }
        };

        let base_dir = manifest_path.parent().unwrap_or(Path::new("."));

        let mut result = ResolvedManifest::default();

        for (id, entry) in raw.sprites {
            let full_path = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("sprites.{id}"),
                |path| ManifestError::MissingFile {
                    id: id.clone(),
                    path,
                },
            )?;
            let normal_path = match &entry.normal_map {
                Some(rel) => Some(resolve_file(
                    base_dir,
                    rel,
                    sink,
                    || format!("sprites.{id}.normal_map"),
                    |path| ManifestError::MissingNormalMapFile {
                        id: id.clone(),
                        path,
                    },
                )?),
                None => None,
            };
            let emissive_path = match &entry.emissive_mask {
                Some(rel) => Some(resolve_file(
                    base_dir,
                    rel,
                    sink,
                    || format!("sprites.{id}.emissive_mask"),
                    |path| ManifestError::MissingEmissiveMaskFile {
                        id: id.clone(),
                        path,
                    },
                )?),
                None => None,
            };
            // A sprite with a sibling that did not resolve is left out.
            let (Some(full_path), Some(normal_path), Some(emissive_path)) = (
                full_path,
                normal_path.map_or(Some(None), |p| p.map(Some)),
                emissive_path.map_or(Some(None), |p| p.map(Some)),
            ) else {
                continue;
            };
            result.sprites.insert(
                id,
                ResolvedSprite {
                    path: full_path,
                    filter: entry.filter,
                    normal_path,
                    emissive_path,
                    nine_slice: entry.nine_slice,
                },
            );
        }

        for (id, entry) in raw.animations {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("animations.{id}"),
                |path| ManifestError::MissingAnimationFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result
                .animations
                .insert(id, ResolvedAnimation { path: full_path });
        }

        for (id, entry) in raw.fonts {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("fonts.{id}"),
                |path| ManifestError::MissingFontFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result.fonts.insert(id, ResolvedFont { path: full_path });
        }

        // Faces are checked against the merged graph, as material shaders are.
        for (id, entry) in raw.font_families {
            result
                .font_families
                .insert(id, ResolvedFontFamily { faces: entry.faces });
        }
        append_fallback(&mut result.font_fallback, raw.font_fallback);

        for (id, entry) in raw.sounds {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("sounds.{id}"),
                |path| ManifestError::MissingSoundFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result.sounds.insert(
                id,
                ResolvedSound {
                    path: full_path,
                    looping: entry.looping,
                    volume: entry.volume,
                    bus: entry.bus,
                    max_voices: entry.max_voices,
                },
            );
        }

        for (id, entry) in raw.tilemaps {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("tilemaps.{id}"),
                |path| ManifestError::MissingTilemapFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result
                .tilemaps
                .insert(id, ResolvedTilemap { path: full_path });
        }

        for (id, entry) in raw.particles {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("particles.{id}"),
                |path| ManifestError::MissingParticleFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result
                .particles
                .insert(id, ResolvedParticle { path: full_path });
        }

        for (id, entry) in raw.shaders {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("shaders.{id}"),
                |path| ManifestError::MissingShaderFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result
                .shaders
                .insert(id, ResolvedShader { path: full_path });
        }

        for (id, entry) in raw.ui_themes {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("ui_themes.{id}"),
                |path| ManifestError::MissingUiThemeFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result
                .ui_themes
                .insert(id, ResolvedUiTheme { path: full_path });
        }

        for (id, entry) in raw.prefabs {
            let Some(full_path) = resolve_file(
                base_dir,
                &entry.path,
                sink,
                || format!("prefabs.{id}"),
                |path| ManifestError::MissingPrefabFile {
                    id: id.clone(),
                    path,
                },
            )?
            else {
                continue;
            };
            result
                .prefabs
                .insert(id, ResolvedPrefab { path: full_path });
        }

        for (kind, entries) in raw.data {
            let section = result.data.entry(kind.clone()).or_default();
            for (id, entry) in entries {
                let Some(full_path) = resolve_file(
                    base_dir,
                    &entry.path,
                    sink,
                    || format!("data.{kind}.{id}"),
                    |path| ManifestError::MissingDataFile {
                        kind: kind.clone(),
                        id: id.clone(),
                        path,
                    },
                )?
                else {
                    continue;
                };
                section.insert(id, ResolvedDataEntry { path: full_path });
            }
        }

        // The material -> shader reference is checked by the caller, on this
        // file alone (`load`) or on the merged graph (`load_and_merge_many`).
        let source_manifest = manifest_path
            .canonicalize()
            .unwrap_or_else(|_| manifest_path.to_path_buf());
        for (id, entry) in raw.materials {
            result.materials.insert(
                id,
                ResolvedMaterial {
                    source_manifest: source_manifest.clone(),
                    shader: entry.shader,
                    uniform_defaults: entry.uniform_defaults,
                },
            );
        }

        // Sorted, so the layer or material an error names does not depend
        // on map order; a layer two names give keeps the smaller name.
        let mut layers: Vec<(String, u32)> = raw.physics_layers.into_iter().collect();
        layers.sort();
        for (name, layer) in layers {
            let reason = if !valid_layer_name(&name) {
                Some("a name must match [a-z][a-z0-9_]*".to_owned())
            } else if layer >= u32::BITS {
                Some(format!("a layer must be 0 to 31 (got {layer})"))
            } else {
                None
            };
            if let Some(reason) = reason {
                sink.report(ManifestError::InvalidPhysicsLayer { name, reason })?;
                continue;
            }
            if let Some((first, _)) = result.physics_layers.iter().find(|&(_, &l)| l == layer) {
                sink.report(ManifestError::PhysicsLayerTaken {
                    layer,
                    first: first.clone(),
                    second: name,
                })?;
                continue;
            }
            result.physics_layers.insert(name, layer);
        }
        let mut materials: Vec<(String, PhysicsMaterialEntry)> =
            raw.physics_materials.into_iter().collect();
        materials.sort_by(|a, b| a.0.cmp(&b.0));
        for (id, entry) in materials {
            match entry.resolve() {
                Ok(material) => {
                    result.physics_materials.insert(id, material);
                }
                Err(reason) => sink.report(ManifestError::InvalidPhysicsMaterial { id, reason })?,
            }
        }

        // Sorted, so the mesh an error names does not depend on map order.
        let mut meshes: Vec<(String, ParticleMeshEntry)> =
            raw.particle_meshes.into_iter().collect();
        meshes.sort_by(|a, b| a.0.cmp(&b.0));
        for (id, mesh) in meshes {
            if let Err(reason) = mesh.validate() {
                sink.report(ManifestError::InvalidParticleMesh { id, reason })?;
                continue;
            }
            result.particle_meshes.insert(
                id,
                ResolvedParticleMesh {
                    source_manifest: source_manifest.clone(),
                    mesh,
                },
            );
        }

        Ok(result)
    }

    /// Load ordered roots into one graph; duplicate IDs are fatal (D-017).
    /// References between entries are checked once, on the merged graph, so a
    /// material may name a shader that another root declares, whatever the
    /// order of the roots (D-089).
    pub fn load_and_merge_many(
        roots: &[impl AsRef<Path>],
    ) -> Result<ResolvedManifest, ManifestError> {
        let mut sink = Sink::first();
        let mut merged = ResolvedManifest::default();
        for root in roots {
            let next = ResolvedManifest::load_unvalidated(root.as_ref(), &mut sink)?;
            merged.merge_entries(next, false, &mut sink)?;
        }
        merged.validate_cross_refs(&mut sink)?;
        Ok(merged)
    }

    /// Loads `roots` as [`Self::load_and_merge_many`] does, past every error,
    /// for `tungsten check`: a root that does not read or parse, or lies
    /// outside `folder`, is one issue and adds nothing; an entry whose file is
    /// missing or resolves outside `folder` (both canonicalized), a particle
    /// mesh that fails its check and an ID an earlier root declared are an
    /// issue each and are left out, the first declaration kept; then every
    /// material shader, family face and fallback family the graph lacks, and
    /// every file that backs two entries, is an issue. Returns the graph of
    /// what resolved and the issues, root by root, then the graph's sorted
    /// by ID. A root's own issues are sorted by their message.
    ///
    /// ```
    /// use tungsten_core::assets::{ManifestError, ResolvedManifest};
    ///
    /// let dir = std::env::temp_dir().join(format!("tungsten_check_doc_{}", std::process::id()));
    /// std::fs::create_dir_all(&dir).unwrap();
    /// std::fs::write(dir.join("hero.png"), b"").unwrap();
    /// let root = dir.join("manifest.json");
    /// std::fs::write(
    ///     &root,
    ///     r#"{ "sprites": { "hero": { "path": "hero.png" }, "gem": { "path": "gem.png" } },
    ///          "materials": { "glow": { "shader": "glow" } } }"#,
    /// )
    /// .unwrap();
    ///
    /// let (graph, issues) = ResolvedManifest::load_for_check(&[&root], &dir);
    /// assert!(graph.sprites.contains_key("hero"));
    /// assert_eq!(issues.len(), 2);
    /// assert!(issues.iter().all(|issue| issue.manifest == root));
    /// assert!(matches!(issues[0].error, ManifestError::MissingFile { .. }));
    /// assert!(matches!(issues[1].error, ManifestError::MaterialShaderMissing { .. }));
    /// std::fs::remove_dir_all(&dir).unwrap();
    /// ```
    #[must_use]
    pub fn load_for_check(roots: &[impl AsRef<Path>], folder: &Path) -> (Self, Vec<ManifestIssue>) {
        let mut issues = Vec::new();
        let mut sink = Sink {
            every: Some(Every {
                issues: &mut issues,
                folder: folder
                    .canonicalize()
                    .unwrap_or_else(|_| folder.to_path_buf()),
                root: PathBuf::new(),
                declared: HashMap::new(),
            }),
        };
        let mut merged = ResolvedManifest::default();
        // A checking sink never stops a body, so each `Err` arm is unused.
        for root in roots {
            let root = root.as_ref();
            let Some(every) = sink.every.as_mut() else {
                break;
            };
            every.root = root.to_path_buf();
            let first = every.issues.len();
            if let Ok(canonical) = root.canonicalize()
                && !canonical.starts_with(&every.folder)
            {
                every.issues.push(ManifestIssue {
                    manifest: root.to_path_buf(),
                    error: ManifestError::OutsideFolder {
                        id: "manifest".to_owned(),
                        path: canonical.display().to_string(),
                    },
                });
                continue;
            }
            let Ok(next) = Self::load_unvalidated(root, &mut sink) else {
                continue;
            };
            if let Some(every) = sink.every.as_mut() {
                next.declare(&mut every.declared, root);
            }
            let _ = merged.merge_entries(next, false, &mut sink);
            if let Some(every) = sink.every.as_mut() {
                every.issues[first..].sort_by_cached_key(|issue| issue.error.to_string());
            }
        }
        let _ = merged.validate_cross_refs(&mut sink);
        drop(sink);
        (merged, issues)
    }

    /// Records `root` as the declaring root of each entry a cross reference
    /// can name, unless an earlier root declared it.
    fn declare(&self, declared: &mut HashMap<String, PathBuf>, root: &Path) {
        let keys = self
            .materials
            .keys()
            .map(|id| format!("materials.{id}"))
            .chain(
                self.font_families
                    .keys()
                    .map(|id| format!("font_families.{id}")),
            )
            .chain(
                self.font_fallback
                    .iter()
                    .map(|family| format!("font_fallback.{family}")),
            )
            .chain(self.prefabs.keys().map(|id| format!("prefabs.{id}")))
            .chain(self.data.iter().flat_map(|(kind, entries)| {
                entries.keys().map(move |id| format!("data.{kind}.{id}"))
            }));
        for key in keys {
            declared.entry(key).or_insert_with(|| root.to_path_buf());
        }
    }

    /// Every material's shader must be declared in this graph, and so must
    /// every font family's faces and every family in the fallback chain; and
    /// no file may back two prefab or data entries. Failures go to the sink
    /// by kind, each kind sorted by ID, so the first does not depend on map
    /// order.
    fn validate_cross_refs(&self, sink: &mut Sink<'_>) -> Result<(), ManifestError> {
        let mut missing: Vec<(&String, &ResolvedMaterial)> = self
            .materials
            .iter()
            .filter(|(_, material)| !self.shaders.contains_key(&material.shader))
            .collect();
        missing.sort_by(|a, b| a.0.cmp(b.0));
        for (id, material) in missing {
            sink.report_entry(
                &format!("materials.{id}"),
                ManifestError::MaterialShaderMissing {
                    id: id.clone(),
                    shader: material.shader.clone(),
                },
            )?;
        }
        self.validate_font_refs(sink)?;
        self.validate_shared_files(sink)
    }

    /// Each file backs one prefab or data entry, since an edit routes to
    /// one (M61); each shared file is reported with its smallest pair of
    /// entries, the files in that pair's order.
    fn validate_shared_files(&self, sink: &mut Sink<'_>) -> Result<(), ManifestError> {
        let mut holders: HashMap<&Path, Vec<String>> = HashMap::new();
        for (id, prefab) in &self.prefabs {
            holders
                .entry(prefab.path.as_path())
                .or_default()
                .push(format!("prefabs.{id}"));
        }
        for (kind, entries) in &self.data {
            for (id, entry) in entries {
                holders
                    .entry(entry.path.as_path())
                    .or_default()
                    .push(format!("data.{kind}.{id}"));
            }
        }
        let mut shared: Vec<(String, String, &Path)> = holders
            .into_iter()
            .filter(|(_, names)| names.len() > 1)
            .map(|(path, mut names)| {
                names.sort();
                let second = names.swap_remove(1);
                let first = names.swap_remove(0);
                (first, second, path)
            })
            .collect();
        shared.sort();
        for (first, second, path) in shared {
            sink.report_entry(
                &second.clone(),
                ManifestError::SharedFile {
                    path: path.display().to_string(),
                    first,
                    second,
                },
            )?;
        }
        Ok(())
    }

    /// Every family face names a `fonts` entry and every chain entry a family
    /// (`D-115`); faces by family ID, then face ID, then chain entries by ID.
    fn validate_font_refs(&self, sink: &mut Sink<'_>) -> Result<(), ManifestError> {
        let mut missing_faces: Vec<(&String, &String)> = self
            .font_families
            .iter()
            .flat_map(|(family, entry)| entry.faces.iter().map(move |face| (family, face)))
            .filter(|(_, face)| !self.fonts.contains_key(*face))
            .collect();
        missing_faces.sort();
        for (family, face) in missing_faces {
            sink.report_entry(
                &format!("font_families.{family}"),
                ManifestError::FontFamilyFaceMissing {
                    family: family.clone(),
                    face: face.clone(),
                },
            )?;
        }
        let mut unknown: Vec<&String> = self
            .font_fallback
            .iter()
            .filter(|family| !self.font_families.contains_key(*family))
            .collect();
        unknown.sort();
        for family in unknown {
            sink.report_entry(
                &format!("font_fallback.{family}"),
                ManifestError::UnknownFallbackFamily {
                    family: family.clone(),
                },
            )?;
        }
        Ok(())
    }

    /// Merge another manifest; duplicate IDs are fatal (D-017), a data ID
    /// within its kind. A merged material's shader must already be in the
    /// graph or arrive with it, and so must a family's faces and a chain
    /// entry's family (`D-115`); no file may back two prefab or data entries.
    pub fn merge(&mut self, other: ResolvedManifest) -> Result<(), ManifestError> {
        self.merge_entries(other, true, &mut Sink::first())
    }

    /// `merge`, with the reference checks optional: a caller that merges
    /// several roots checks them once at the end instead. A checking sink
    /// keeps the first declaration of a repeated ID.
    fn merge_entries(
        &mut self,
        other: ResolvedManifest,
        check_refs: bool,
        sink: &mut Sink<'_>,
    ) -> Result<(), ManifestError> {
        merge_section(&mut self.sprites, other.sprites, sink)?;
        merge_section(&mut self.animations, other.animations, sink)?;
        merge_section(&mut self.fonts, other.fonts, sink)?;
        merge_section(&mut self.sounds, other.sounds, sink)?;
        merge_section(&mut self.tilemaps, other.tilemaps, sink)?;
        merge_section(&mut self.particles, other.particles, sink)?;
        merge_section(&mut self.shaders, other.shaders, sink)?;
        for (id, material) in other.materials {
            if self.materials.contains_key(&id) {
                sink.report(ManifestError::DuplicateId { id })?;
                continue;
            }
            // Re-validate cross-ref: when a material merges in from a sibling
            // manifest, the shader may live in the merged graph we just built.
            if check_refs && !self.shaders.contains_key(&material.shader) {
                sink.report(ManifestError::MaterialShaderMissing {
                    id,
                    shader: material.shader,
                })?;
                continue;
            }
            self.materials.insert(id, material);
        }
        merge_section(&mut self.particle_meshes, other.particle_meshes, sink)?;
        merge_section(&mut self.font_families, other.font_families, sink)?;
        merge_section(&mut self.ui_themes, other.ui_themes, sink)?;
        merge_section(&mut self.prefabs, other.prefabs, sink)?;
        for (kind, entries) in other.data {
            let section = self.data.entry(kind.clone()).or_default();
            for (id, entry) in entries {
                if section.contains_key(&id) {
                    sink.report(ManifestError::DuplicateId {
                        id: format!("data.{kind}.{id}"),
                    })?;
                    continue;
                }
                section.insert(id, entry);
            }
        }
        append_fallback(&mut self.font_fallback, other.font_fallback);
        // Sorted, so the name an error gives does not depend on map order.
        let mut layers: Vec<(String, u32)> = other.physics_layers.into_iter().collect();
        layers.sort();
        for (name, layer) in layers {
            if self.physics_layers.contains_key(&name) {
                sink.report(ManifestError::DuplicateId {
                    id: format!("physics_layers.{name}"),
                })?;
                continue;
            }
            if let Some((first, _)) = self.physics_layers.iter().find(|&(_, &l)| l == layer) {
                sink.report(ManifestError::PhysicsLayerTaken {
                    layer,
                    first: first.clone(),
                    second: name,
                })?;
                continue;
            }
            self.physics_layers.insert(name, layer);
        }
        for (id, material) in other.physics_materials {
            if self.physics_materials.contains_key(&id) {
                sink.report(ManifestError::DuplicateId {
                    id: format!("physics_materials.{id}"),
                })?;
                continue;
            }
            self.physics_materials.insert(id, material);
        }
        if check_refs {
            self.validate_font_refs(sink)?;
            self.validate_shared_files(sink)?;
        }
        Ok(())
    }
}

/// Moves `other`'s entries into `section`; an ID it already holds is a
/// [`ManifestError::DuplicateId`] (D-017), the first kept.
fn merge_section<T>(
    section: &mut HashMap<String, T>,
    other: HashMap<String, T>,
    sink: &mut Sink<'_>,
) -> Result<(), ManifestError> {
    for (id, entry) in other {
        if section.contains_key(&id) {
            sink.report(ManifestError::DuplicateId { id })?;
            continue;
        }
        section.insert(id, entry);
    }
    Ok(())
}

/// Appends `more` to the chain in order, skipping families already in it, so
/// each keeps its first position (`D-115`).
fn append_fallback(chain: &mut Vec<String>, more: Vec<String>) {
    for family in more {
        if !chain.contains(&family) {
            chain.push(family);
        }
    }
}

#[cfg(test)]
#[path = "../tests/assets/manifest.rs"]
mod tests;
