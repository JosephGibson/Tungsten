use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::assets::material::MaterialUniformDefaults;
use crate::assets::particle::ParticleMesh;

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

/// Raw manifest JSON.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawManifest {
    #[serde(default)]
    pub sprites: HashMap<String, SpriteEntry>,
    #[serde(default)]
    pub animations: HashMap<String, AnimationEntry>,
    #[serde(default)]
    pub fonts: HashMap<String, FontEntry>,
    #[serde(default)]
    pub sounds: HashMap<String, SoundEntry>,
    #[serde(default)]
    pub tilemaps: HashMap<String, TilemapEntry>,
    #[serde(default)]
    pub particles: HashMap<String, ParticleEntry>,
    #[serde(default)]
    pub shaders: HashMap<String, ShaderEntry>,
    #[serde(default)]
    pub materials: HashMap<String, MaterialEntry>,
    #[serde(default)]
    pub particle_meshes: HashMap<String, ParticleMeshEntry>,
    /// Font families: a family ID and the `fonts` face IDs it groups (`D-115`).
    #[serde(default)]
    pub font_families: HashMap<String, FontFamilyEntry>,
    /// Family IDs in order of preference for glyphs a style's family lacks.
    #[serde(default)]
    pub font_fallback: Vec<String>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SoundEntry {
    pub path: String,
    /// Default loop flag.
    #[serde(default)]
    pub looping: bool,
    /// Base volume before master volume.
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_volume() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct TilemapEntry {
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
}

#[derive(Debug, Clone)]
pub struct ResolvedSprite {
    pub path: PathBuf,
    pub filter: FilterMode,
    pub normal_path: Option<PathBuf>,
    pub emissive_path: Option<PathBuf>,
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

#[derive(Debug, Clone)]
pub struct ResolvedSound {
    pub path: PathBuf,
    pub looping: bool,
    pub volume: f32,
}

#[derive(Debug, Clone)]
pub struct ResolvedTilemap {
    pub path: PathBuf,
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
    /// every font family's faces and every family in the fallback chain. Of
    /// several failures of one kind, the smallest ID is reported, so the error
    /// does not depend on map order.
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
        self.validate_font_refs()
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

    /// Merge another manifest; duplicate IDs are fatal (D-017). A merged
    /// material's shader must already be in the graph or arrive with it, and
    /// so must a family's faces and a chain entry's family (`D-115`).
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
        append_fallback(&mut self.font_fallback, other.font_fallback);
        if check_refs {
            self.validate_font_refs()?;
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
