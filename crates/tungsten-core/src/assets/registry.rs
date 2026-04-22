use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::atlas::UvRect;
use super::manifest::FilterMode;

/// Opaque handle to a GPU texture. The actual wgpu texture lives in
/// tungsten-render's texture pool, keyed by this handle. Core never
/// sees wgpu types (D-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// Metadata about a loaded sprite.
///
/// Post-M22 the `atlas` handle may be shared between many sprites; every
/// sprite also carries a `uv` rect that locates it on its atlas page. Pre-M22
/// one-sprite-per-texture callers remain correct by using `UvRect::FULL`.
#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub atlas: TextureHandle,
    pub uv: UvRect,
    pub filter: FilterMode,
    pub width: u32,
    pub height: u32,
    /// Absolute path to the source PNG, used for hot-reload reverse lookup.
    pub path: PathBuf,
}

/// Runtime asset registry, stored as a Resource in the World (D-014).
/// Maps string IDs to loaded asset data.
#[derive(Debug, Default)]
pub struct AssetRegistry {
    sprites: HashMap<String, SpriteAsset>,
    path_to_sprite_id: HashMap<PathBuf, String>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a sprite with a pre-allocated atlas handle and UV rect.
    /// Post-M22 handle authority lives with the renderer's texture pool —
    /// callers pass in the `atlas` handle produced by
    /// `Renderer::allocate_texture_handle`.
    ///
    /// # Panics
    /// Panics if a sprite with the same `id` is already registered (D-017).
    #[allow(clippy::too_many_arguments)] // stable M22 surface; see D-048
    pub fn register_sprite(
        &mut self,
        id: String,
        filter: FilterMode,
        width: u32,
        height: u32,
        path: PathBuf,
        atlas: TextureHandle,
        uv: UvRect,
    ) {
        assert!(
            !self.sprites.contains_key(&id),
            "duplicate sprite ID '{id}' — each sprite must be registered exactly once"
        );
        self.path_to_sprite_id.insert(path.clone(), id.clone());
        self.sprites.insert(
            id,
            SpriteAsset {
                atlas,
                uv,
                filter,
                width,
                height,
                path,
            },
        );
    }

    pub fn get_sprite(&self, id: &str) -> Option<&SpriteAsset> {
        self.sprites.get(id)
    }

    pub fn sprite_ids(&self) -> impl Iterator<Item = &str> {
        self.sprites.keys().map(|s| s.as_str())
    }

    /// Reverse-lookup: find the sprite ID registered for a given file path.
    pub fn sprite_id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_sprite_id.get(path).map(|s| s.as_str())
    }

    /// Update the stored atlas binding and/or dimensions after a hot-reload.
    /// Used by both the in-place (`atlas`/`uv` unchanged) and rebuild paths.
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
}

/// Tracks loaded font IDs and their file paths for hot-reload reverse-lookup.
#[derive(Debug, Default)]
pub struct FontRegistry {
    path_to_id: HashMap<PathBuf, String>,
}

impl FontRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, id: String, path: PathBuf) {
        self.path_to_id.insert(path, id);
    }

    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(|s| s.as_str())
    }

    pub fn contains_id(&self, id: &str) -> bool {
        self.path_to_id.values().any(|v| v == id)
    }
}

#[cfg(test)]
#[path = "../tests/assets/registry.rs"]
mod tests;
