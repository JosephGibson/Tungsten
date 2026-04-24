use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

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
    #[error("duplicate asset ID '{id}' across manifests")]
    DuplicateId { id: String },
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpriteEntry {
    pub path: String,
    #[serde(default = "default_filter")]
    pub filter: FilterMode,
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
}

#[derive(Debug, Clone)]
pub struct ResolvedSprite {
    pub path: PathBuf,
    pub filter: FilterMode,
}

#[derive(Debug, Clone)]
pub struct ResolvedAnimation {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedFont {
    pub path: PathBuf,
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

impl ResolvedManifest {
    /// Load manifest and resolve paths relative to its parent.
    pub fn load(manifest_path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let manifest_path = manifest_path.as_ref();
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
            result.sprites.insert(
                id,
                ResolvedSprite {
                    path: full_path,
                    filter: entry.filter,
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

        Ok(result)
    }

    /// Load ordered roots into one graph; duplicate IDs are fatal (D-017).
    pub fn load_and_merge_many(
        roots: &[impl AsRef<Path>],
    ) -> Result<ResolvedManifest, ManifestError> {
        let mut merged = ResolvedManifest::default();
        for root in roots {
            let next = ResolvedManifest::load(root)?;
            merged.merge(next)?;
        }
        Ok(merged)
    }

    /// Merge another manifest; duplicate IDs are fatal (D-017).
    pub fn merge(&mut self, other: ResolvedManifest) -> Result<(), ManifestError> {
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
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/assets/manifest.rs"]
mod tests;
