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
    #[serde(default)]
    pub fonts: HashMap<String, FontEntry>,
    #[serde(default)]
    pub sounds: HashMap<String, SoundEntry>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct FontEntry {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundEntry {
    pub path: String,
    /// Whether this sound loops by default. Can be overridden at play time.
    #[serde(default)]
    pub looping: bool,
    /// Base volume (0.0–1.0). Multiplied by the master volume at mix time.
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_volume() -> f32 {
    1.0
}

/// A fully resolved manifest with absolute paths.
#[derive(Debug, Clone, Default)]
pub struct ResolvedManifest {
    pub sprites: HashMap<String, ResolvedSprite>,
    pub animations: HashMap<String, ResolvedAnimation>,
    pub fonts: HashMap<String, ResolvedFont>,
    pub sounds: HashMap<String, ResolvedSound>,
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
            result.sounds.insert(
                id,
                ResolvedSound {
                    path: full_path,
                    looping: entry.looping,
                    volume: entry.volume,
                },
            );
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_manifest(dir: &Path, content: &str) -> PathBuf {
        let path = dir.join("manifest.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn write_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::File::create(&path).unwrap();
        path
    }

    #[test]
    fn load_empty_manifest() {
        let tmp = tempdir();
        let path = write_manifest(&tmp, r#"{"sprites": {}, "animations": {}}"#);
        let m = ResolvedManifest::load(&path).unwrap();
        assert!(m.sprites.is_empty());
        assert!(m.animations.is_empty());
        assert!(m.fonts.is_empty());
    }

    #[test]
    fn load_manifest_with_sprites() {
        let tmp = tempdir();
        write_file(&tmp, "hero.png");
        let path = write_manifest(
            &tmp,
            r#"{"sprites": {"hero": {"path": "hero.png", "filter": "nearest"}}}"#,
        );
        let m = ResolvedManifest::load(&path).unwrap();
        assert!(m.sprites.contains_key("hero"));
        assert_eq!(m.sprites["hero"].filter, FilterMode::Nearest);
    }

    #[test]
    fn load_manifest_missing_sprite_file() {
        let tmp = tempdir();
        let path = write_manifest(&tmp, r#"{"sprites": {"hero": {"path": "missing.png"}}}"#);
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::MissingFile { .. }));
    }

    #[test]
    fn load_manifest_missing_animation_file() {
        let tmp = tempdir();
        let path = write_manifest(&tmp, r#"{"animations": {"walk": {"path": "walk.json"}}}"#);
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::MissingAnimationFile { .. }));
    }

    #[test]
    fn load_manifest_invalid_json() {
        let tmp = tempdir();
        let path = write_manifest(&tmp, "NOT JSON!");
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }));
    }

    #[test]
    fn load_manifest_nonexistent_file() {
        let err = ResolvedManifest::load("/nonexistent/manifest.json").unwrap_err();
        assert!(matches!(err, ManifestError::Io { .. }));
    }

    #[test]
    fn merge_success() {
        let mut a = ResolvedManifest::default();
        a.sprites.insert(
            "hero".into(),
            ResolvedSprite {
                path: "hero.png".into(),
                filter: FilterMode::Nearest,
            },
        );

        let mut b = ResolvedManifest::default();
        b.sprites.insert(
            "enemy".into(),
            ResolvedSprite {
                path: "enemy.png".into(),
                filter: FilterMode::Linear,
            },
        );

        a.merge(b).unwrap();
        assert!(a.sprites.contains_key("hero"));
        assert!(a.sprites.contains_key("enemy"));
    }

    #[test]
    fn merge_duplicate_sprite_is_error() {
        let mut a = ResolvedManifest::default();
        a.sprites.insert(
            "hero".into(),
            ResolvedSprite {
                path: "hero.png".into(),
                filter: FilterMode::Nearest,
            },
        );

        let mut b = ResolvedManifest::default();
        b.sprites.insert(
            "hero".into(),
            ResolvedSprite {
                path: "hero2.png".into(),
                filter: FilterMode::Nearest,
            },
        );

        let err = a.merge(b).unwrap_err();
        assert!(matches!(err, ManifestError::DuplicateId { id } if id == "hero"));
    }

    #[test]
    fn merge_duplicate_animation_is_error() {
        let mut a = ResolvedManifest::default();
        a.animations.insert(
            "walk".into(),
            ResolvedAnimation {
                path: "walk.json".into(),
            },
        );

        let mut b = ResolvedManifest::default();
        b.animations.insert(
            "walk".into(),
            ResolvedAnimation {
                path: "walk2.json".into(),
            },
        );

        let err = a.merge(b).unwrap_err();
        assert!(matches!(err, ManifestError::DuplicateId { id } if id == "walk"));
    }

    #[test]
    fn load_manifest_with_fonts() {
        let tmp = tempdir();
        write_file(&tmp, "sans.ttf");
        let path = write_manifest(&tmp, r#"{"fonts": {"sans": {"path": "sans.ttf"}}}"#);
        let m = ResolvedManifest::load(&path).unwrap();
        assert!(m.fonts.contains_key("sans"));
    }

    #[test]
    fn load_manifest_missing_font_file() {
        let tmp = tempdir();
        let path = write_manifest(&tmp, r#"{"fonts": {"sans": {"path": "missing.ttf"}}}"#);
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::MissingFontFile { .. }));
    }

    #[test]
    fn merge_duplicate_font_is_error() {
        let mut a = ResolvedManifest::default();
        a.fonts.insert(
            "sans".into(),
            ResolvedFont {
                path: "sans.ttf".into(),
            },
        );

        let mut b = ResolvedManifest::default();
        b.fonts.insert(
            "sans".into(),
            ResolvedFont {
                path: "sans2.ttf".into(),
            },
        );

        let err = a.merge(b).unwrap_err();
        assert!(matches!(err, ManifestError::DuplicateId { id } if id == "sans"));
    }

    #[test]
    fn load_manifest_with_sounds() {
        let tmp = tempdir();
        write_file(&tmp, "blip.ogg");
        let path = write_manifest(
            &tmp,
            r#"{"sounds": {"sfx_blip": {"path": "blip.ogg", "looping": false, "volume": 0.8}}}"#,
        );
        let m = ResolvedManifest::load(&path).unwrap();
        assert!(m.sounds.contains_key("sfx_blip"));
        let s = &m.sounds["sfx_blip"];
        assert!(!s.looping);
        assert!((s.volume - 0.8).abs() < 1e-6);
    }

    #[test]
    fn sound_defaults_looping_false_volume_one() {
        let tmp = tempdir();
        write_file(&tmp, "blip.ogg");
        let path = write_manifest(&tmp, r#"{"sounds": {"sfx_blip": {"path": "blip.ogg"}}}"#);
        let m = ResolvedManifest::load(&path).unwrap();
        let s = &m.sounds["sfx_blip"];
        assert!(!s.looping);
        assert!((s.volume - 1.0).abs() < 1e-6);
    }

    #[test]
    fn load_manifest_missing_sound_file() {
        let tmp = tempdir();
        let path = write_manifest(&tmp, r#"{"sounds": {"sfx_blip": {"path": "missing.ogg"}}}"#);
        let err = ResolvedManifest::load(&path).unwrap_err();
        assert!(matches!(err, ManifestError::MissingSoundFile { .. }));
    }

    #[test]
    fn merge_duplicate_sound_is_error() {
        let mut a = ResolvedManifest::default();
        a.sounds.insert(
            "sfx_blip".into(),
            ResolvedSound {
                path: "blip.ogg".into(),
                looping: false,
                volume: 1.0,
            },
        );

        let mut b = ResolvedManifest::default();
        b.sounds.insert(
            "sfx_blip".into(),
            ResolvedSound {
                path: "blip2.ogg".into(),
                looping: false,
                volume: 1.0,
            },
        );

        let err = a.merge(b).unwrap_err();
        assert!(matches!(err, ManifestError::DuplicateId { id } if id == "sfx_blip"));
    }

    #[test]
    fn default_filter_is_nearest() {
        let tmp = tempdir();
        write_file(&tmp, "hero.png");
        let path = write_manifest(&tmp, r#"{"sprites": {"hero": {"path": "hero.png"}}}"#);
        let m = ResolvedManifest::load(&path).unwrap();
        assert_eq!(m.sprites["hero"].filter, FilterMode::Nearest);
    }

    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tempdir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_test_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
