use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::atlas::UvRect;
use super::manifest::FilterMode;

/// GPU texture handle; core never sees `wgpu` types (D-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// Loaded sprite metadata; atlas handle may be shared.
#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub atlas: TextureHandle,
    pub uv: UvRect,
    pub filter: FilterMode,
    pub width: u32,
    pub height: u32,
    /// Source PNG path for hot reload.
    pub path: PathBuf,
    /// M29 sibling normal-map source PNG path (if registered with one).
    pub normal_path: Option<PathBuf>,
    /// M29 sibling emissive-mask source PNG path (if registered with one).
    pub emissive_path: Option<PathBuf>,
    /// M29 lit atlas marker. `Some(handle)` means the sprite shares its packed
    /// rect with a parallel normal/emissive bundle uploaded under `handle`
    /// (typically equal to `atlas` since lit pages reuse the albedo handle).
    pub lit_atlas: Option<TextureHandle>,
}

/// D-014 runtime asset registry resource.
#[derive(Debug, Default)]
pub struct AssetRegistry {
    sprites: HashMap<String, SpriteAsset>,
    path_to_sprite_id: HashMap<PathBuf, String>,
}

impl AssetRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register sprite with renderer-owned atlas handle and UV rect.
    ///
    /// # Panics
    /// Panics on duplicate sprite ID (D-017).
    #[allow(clippy::too_many_arguments)] // stable M22/M29 surface; see D-048
    pub fn register_sprite(
        &mut self,
        id: String,
        filter: FilterMode,
        width: u32,
        height: u32,
        path: PathBuf,
        atlas: TextureHandle,
        uv: UvRect,
        normal_path: Option<PathBuf>,
        emissive_path: Option<PathBuf>,
        lit_atlas: Option<TextureHandle>,
    ) {
        assert!(
            !self.sprites.contains_key(&id),
            "duplicate sprite ID '{id}' — each sprite must be registered exactly once"
        );
        self.path_to_sprite_id.insert(path.clone(), id.clone());
        if let Some(np) = &normal_path {
            self.path_to_sprite_id.insert(np.clone(), id.clone());
        }
        if let Some(ep) = &emissive_path {
            self.path_to_sprite_id.insert(ep.clone(), id.clone());
        }
        self.sprites.insert(
            id,
            SpriteAsset {
                atlas,
                uv,
                filter,
                width,
                height,
                path,
                normal_path,
                emissive_path,
                lit_atlas,
            },
        );
    }

    #[must_use]
    pub fn get_sprite(&self, id: &str) -> Option<&SpriteAsset> {
        self.sprites.get(id)
    }

    pub fn sprite_ids(&self) -> impl Iterator<Item = &str> {
        self.sprites.keys().map(String::as_str)
    }

    /// Sprite ID for source path.
    #[must_use]
    pub fn sprite_id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_sprite_id.get(path).map(String::as_str)
    }

    /// Update atlas/UV/dimensions after hot reload.
    pub fn update_sprite_entry(
        &mut self,
        id: &str,
        atlas: TextureHandle,
        uv: UvRect,
        width: u32,
        height: u32,
    ) {
        if let Some(asset) = self.sprites.get_mut(id) {
            asset.atlas = atlas;
            asset.uv = uv;
            asset.width = width;
            asset.height = height;
        }
    }

    /// Update the M29 lit atlas marker after an atlas (re)build.
    pub fn update_sprite_lit_atlas(&mut self, id: &str, lit_atlas: Option<TextureHandle>) {
        if let Some(asset) = self.sprites.get_mut(id) {
            asset.lit_atlas = lit_atlas;
        }
    }
}

/// Font path reverse-lookup registry.
#[derive(Debug, Default)]
pub struct FontRegistry {
    path_to_id: HashMap<PathBuf, String>,
}

impl FontRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: String, path: PathBuf) {
        self.path_to_id.insert(path, id);
    }

    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }

    #[must_use]
    pub fn contains_id(&self, id: &str) -> bool {
        self.path_to_id.values().any(|v| v == id)
    }
}

#[cfg(test)]
#[path = "../tests/assets/registry.rs"]
mod tests;
