use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::assets::material::MaterialUniformDefaults;
use crate::assets::particle::ParticleMesh;
use crate::audio::{AudioBus, MAX_VOICES};

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
        let result = Self::load_unvalidated(manifest_path.as_ref())?;
        result.validate_cross_refs()?;
        Ok(result)
    }

    /// Parse one manifest and resolve its paths, leaving references between
    /// entries unchecked.
    fn load_unvalidated(manifest_path: &Path) -> Result<Self, ManifestError> {
        let contents = std::fs::read_to_string(manifest_path).map_err(|e| ManifestError::Io {
            path: manifest_path.display().to_string(),
            source: e,
        })?;

        let raw: RawManifest =
            serde_json::from_str(&contents).map_err(|e| ManifestError::Parse {
                path: manifest_path.display().to_string(),
                source: e,
            })?;

        let base_dir = manifest_path.parent().unwrap_or(Path::new("."));

        let mut result = ResolvedManifest::default();

        for (id, entry) in raw.sprites {
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            let normal_path = match entry.normal_map {
                Some(rel) => {
                    let p = base_dir.join(&rel);
                    if !p.exists() {
                        return Err(ManifestError::MissingNormalMapFile {
                            id,
                            path: p.display().to_string(),
                        });
                    }
                    Some(p.canonicalize().unwrap_or(p))
                }
                None => None,
            };
            let emissive_path = match entry.emissive_mask {
                Some(rel) => {
                    let p = base_dir.join(&rel);
                    if !p.exists() {
                        return Err(ManifestError::MissingEmissiveMaskFile {
                            id,
                            path: p.display().to_string(),
                        });
                    }
                    Some(p.canonicalize().unwrap_or(p))
                }
                None => None,
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
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingAnimationFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            result
                .animations
                .insert(id, ResolvedAnimation { path: full_path });
        }

        for (id, entry) in raw.fonts {
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingFontFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
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
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingSoundFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
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
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingTilemapFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            result
                .tilemaps
                .insert(id, ResolvedTilemap { path: full_path });
        }

        for (id, entry) in raw.particles {
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingParticleFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            result
                .particles
                .insert(id, ResolvedParticle { path: full_path });
        }

        for (id, entry) in raw.shaders {
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingShaderFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            result
                .shaders
                .insert(id, ResolvedShader { path: full_path });
        }

        for (id, entry) in raw.ui_themes {
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingUiThemeFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            result
                .ui_themes
                .insert(id, ResolvedUiTheme { path: full_path });
        }

        for (id, entry) in raw.prefabs {
            let full_path = base_dir.join(&entry.path);
            if !full_path.exists() {
                return Err(ManifestError::MissingPrefabFile {
                    id,
                    path: full_path.display().to_string(),
                });
            }
            let full_path = full_path.canonicalize().unwrap_or(full_path);
            result
                .prefabs
                .insert(id, ResolvedPrefab { path: full_path });
        }

        for (kind, entries) in raw.data {
            let section = result.data.entry(kind.clone()).or_default();
            for (id, entry) in entries {
                let full_path = base_dir.join(&entry.path);
                if !full_path.exists() {
                    return Err(ManifestError::MissingDataFile {
                        kind,
                        id,
                        path: full_path.display().to_string(),
                    });
                }
                let full_path = full_path.canonicalize().unwrap_or(full_path);
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

        // Sorted, so the mesh an error names does not depend on map order.
        let mut meshes: Vec<(String, ParticleMeshEntry)> =
            raw.particle_meshes.into_iter().collect();
        meshes.sort_by(|a, b| a.0.cmp(&b.0));
        for (id, mesh) in meshes {
            if let Err(reason) = mesh.validate() {
                return Err(ManifestError::InvalidParticleMesh { id, reason });
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
        let mut merged = ResolvedManifest::default();
        for root in roots {
            let next = ResolvedManifest::load_unvalidated(root.as_ref())?;
            merged.merge_entries(next, false)?;
        }
        merged.validate_cross_refs()?;
        Ok(merged)
    }

    /// Every material's shader must be declared in this graph, and so must
    /// every font family's faces and every family in the fallback chain; and
    /// no file may back two prefab or data entries. Of several failures of
    /// one kind, the smallest ID is reported, so the error does not depend on
    /// map order.
    fn validate_cross_refs(&self) -> Result<(), ManifestError> {
        let missing = self
            .materials
            .iter()
            .filter(|(_, material)| !self.shaders.contains_key(&material.shader))
            .min_by(|a, b| a.0.cmp(b.0));
        if let Some((id, material)) = missing {
            return Err(ManifestError::MaterialShaderMissing {
                id: id.clone(),
                shader: material.shader.clone(),
            });
        }
        self.validate_font_refs()?;
        self.validate_shared_files()
    }

    /// Each file backs one prefab or data entry, since an edit routes to
    /// one (M61); of several shared files, the smallest pair of entries is
    /// reported.
    fn validate_shared_files(&self) -> Result<(), ManifestError> {
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
        let shared = holders
            .into_iter()
            .filter(|(_, names)| names.len() > 1)
            .map(|(path, mut names)| {
                names.sort();
                let second = names.swap_remove(1);
                let first = names.swap_remove(0);
                (first, second, path)
            })
            .min();
        match shared {
            Some((first, second, path)) => Err(ManifestError::SharedFile {
                path: path.display().to_string(),
                first,
                second,
            }),
            None => Ok(()),
        }
    }

    /// Every family face names a `fonts` entry and every chain entry a family
    /// (`D-115`); the smallest family ID, then face ID, is reported first.
    fn validate_font_refs(&self) -> Result<(), ManifestError> {
        let missing_face = self
            .font_families
            .iter()
            .flat_map(|(family, entry)| entry.faces.iter().map(move |face| (family, face)))
            .filter(|(_, face)| !self.fonts.contains_key(*face))
            .min();
        if let Some((family, face)) = missing_face {
            return Err(ManifestError::FontFamilyFaceMissing {
                family: family.clone(),
                face: face.clone(),
            });
        }
        let unknown = self
            .font_fallback
            .iter()
            .filter(|family| !self.font_families.contains_key(*family))
            .min();
        match unknown {
            Some(family) => Err(ManifestError::UnknownFallbackFamily {
                family: family.clone(),
            }),
            None => Ok(()),
        }
    }

    /// Merge another manifest; duplicate IDs are fatal (D-017), a data ID
    /// within its kind. A merged material's shader must already be in the
    /// graph or arrive with it, and so must a family's faces and a chain
    /// entry's family (`D-115`); no file may back two prefab or data entries.
    pub fn merge(&mut self, other: ResolvedManifest) -> Result<(), ManifestError> {
        self.merge_entries(other, true)
    }

    /// `merge`, with the reference checks optional: a caller that merges
    /// several roots checks them once at the end instead.
    fn merge_entries(
        &mut self,
        other: ResolvedManifest,
        check_refs: bool,
    ) -> Result<(), ManifestError> {
        for (id, sprite) in other.sprites {
            if self.sprites.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.sprites.insert(id, sprite);
        }
        for (id, anim) in other.animations {
            if self.animations.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.animations.insert(id, anim);
        }
        for (id, font) in other.fonts {
            if self.fonts.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.fonts.insert(id, font);
        }
        for (id, sound) in other.sounds {
            if self.sounds.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.sounds.insert(id, sound);
        }
        for (id, tilemap) in other.tilemaps {
            if self.tilemaps.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.tilemaps.insert(id, tilemap);
        }
        for (id, particle) in other.particles {
            if self.particles.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.particles.insert(id, particle);
        }
        for (id, shader) in other.shaders {
            if self.shaders.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.shaders.insert(id, shader);
        }
        for (id, material) in other.materials {
            if self.materials.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            // Re-validate cross-ref: when a material merges in from a sibling
            // manifest, the shader may live in the merged graph we just built.
            if check_refs && !self.shaders.contains_key(&material.shader) {
                return Err(ManifestError::MaterialShaderMissing {
                    id,
                    shader: material.shader,
                });
            }
            self.materials.insert(id, material);
        }
        for (id, mesh) in other.particle_meshes {
            if self.particle_meshes.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.particle_meshes.insert(id, mesh);
        }
        for (id, family) in other.font_families {
            if self.font_families.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.font_families.insert(id, family);
        }
        for (id, theme) in other.ui_themes {
            if self.ui_themes.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.ui_themes.insert(id, theme);
        }
        for (id, prefab) in other.prefabs {
            if self.prefabs.contains_key(&id) {
                return Err(ManifestError::DuplicateId { id });
            }
            self.prefabs.insert(id, prefab);
        }
        for (kind, entries) in other.data {
            let section = self.data.entry(kind.clone()).or_default();
            for (id, entry) in entries {
                if section.contains_key(&id) {
                    return Err(ManifestError::DuplicateId {
                        id: format!("data.{kind}.{id}"),
                    });
                }
                section.insert(id, entry);
            }
        }
        append_fallback(&mut self.font_fallback, other.font_fallback);
        if check_refs {
            self.validate_font_refs()?;
            self.validate_shared_files()?;
        }
        Ok(())
    }
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
