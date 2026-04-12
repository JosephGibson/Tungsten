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
    #[error("duplicate asset ID '{id}' across manifests")]
    DuplicateId { id: String },
}

/// Raw manifest as deserialized from JSON.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawManifest {
    #[serde(default)]
    pub sprites: HashMap<String, SpriteEntry>,
    #[serde(default)]
    pub animations: HashMap<String, AnimationEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpriteEntry {
    pub path: String,
    #[serde(default = "default_filter")]
    pub filter: FilterMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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

/// A fully resolved manifest with absolute paths.
#[derive(Debug, Clone, Default)]
pub struct ResolvedManifest {
    pub sprites: HashMap<String, ResolvedSprite>,
    pub animations: HashMap<String, ResolvedAnimation>,
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

impl ResolvedManifest {
    /// Load and resolve a single manifest file. Paths are resolved relative
    /// to the manifest's parent directory.
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
            result
                .animations
                .insert(id, ResolvedAnimation { path: full_path });
        }

        Ok(result)
    }

    /// Merge another manifest into this one. Duplicate IDs are fatal (D-017).
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
        Ok(())
    }
}
