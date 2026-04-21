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
mod tests {
    use super::*;

    fn register(reg: &mut AssetRegistry, id: &str, filter: FilterMode, w: u32, h: u32, path: &str) {
        reg.register_sprite(
            id.to_string(),
            filter,
            w,
            h,
            PathBuf::from(path),
            TextureHandle(0),
            UvRect::FULL,
        );
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = AssetRegistry::new();
        register(
            &mut reg,
            "player_idle",
            FilterMode::Nearest,
            32,
            32,
            "dummy.png",
        );
        let sprite = reg.get_sprite("player_idle").unwrap();
        assert_eq!(sprite.atlas, TextureHandle(0));
        assert_eq!(sprite.uv, UvRect::FULL);
        assert_eq!(sprite.width, 32);
    }

    #[test]
    fn register_stores_filter_and_path() {
        let mut reg = AssetRegistry::new();
        register(&mut reg, "a", FilterMode::Nearest, 16, 16, "a.png");
        register(&mut reg, "b", FilterMode::Linear, 32, 32, "b.png");
        let a = reg.get_sprite("a").unwrap();
        let b = reg.get_sprite("b").unwrap();
        assert_eq!(a.filter, FilterMode::Nearest);
        assert_eq!(b.filter, FilterMode::Linear);
        assert_eq!(a.path, PathBuf::from("a.png"));
    }

    #[test]
    #[should_panic(expected = "duplicate sprite ID")]
    fn duplicate_sprite_id_panics() {
        let mut reg = AssetRegistry::new();
        register(&mut reg, "same", FilterMode::Nearest, 16, 16, "same.png");
        register(&mut reg, "same", FilterMode::Nearest, 16, 16, "same2.png");
    }

    #[test]
    fn sprite_id_for_path_reverse_lookup() {
        let mut reg = AssetRegistry::new();
        let path = "/assets/sprites/foo.png";
        register(&mut reg, "foo", FilterMode::Nearest, 32, 32, path);
        assert_eq!(reg.sprite_id_for_path(Path::new(path)), Some("foo"));
        assert_eq!(reg.sprite_id_for_path(Path::new("/other.png")), None);
    }

    #[test]
    fn update_sprite_entry_changes_stored_size() {
        let mut reg = AssetRegistry::new();
        register(&mut reg, "bar", FilterMode::Nearest, 16, 16, "bar.png");
        let new_uv = UvRect {
            min: [0.25, 0.25],
            max: [0.75, 0.75],
        };
        reg.update_sprite_entry("bar", TextureHandle(7), new_uv, 32, 64);
        let asset = reg.get_sprite("bar").unwrap();
        assert_eq!(asset.atlas, TextureHandle(7));
        assert_eq!(asset.uv, new_uv);
        assert_eq!(asset.width, 32);
        assert_eq!(asset.height, 64);
    }
}
