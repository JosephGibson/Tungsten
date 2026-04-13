use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::manifest::FilterMode;

/// Opaque handle to a GPU texture. The actual wgpu texture lives in
/// tungsten-render's texture pool, keyed by this handle. Core never
/// sees wgpu types (D-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u32);

/// Metadata about a loaded sprite.
#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub texture: TextureHandle,
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
    next_texture_handle: u32,
    path_to_sprite_id: HashMap<PathBuf, String>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a sprite and allocate an opaque texture handle.
    /// Called by the asset loading pipeline during startup.
    ///
    /// # Panics
    /// Panics if a sprite with the same `id` is already registered (D-017).
    pub fn register_sprite(
        &mut self,
        id: String,
        filter: FilterMode,
        width: u32,
        height: u32,
        path: PathBuf,
    ) -> TextureHandle {
        assert!(
            !self.sprites.contains_key(&id),
            "duplicate sprite ID '{id}' — each sprite must be registered exactly once"
        );
        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle += 1;
        self.path_to_sprite_id.insert(path.clone(), id.clone());
        self.sprites.insert(
            id,
            SpriteAsset {
                texture: handle,
                filter,
                width,
                height,
                path,
            },
        );
        handle
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

    /// Update the stored dimensions of a sprite after a hot-reload where
    /// the image dimensions changed.
    pub fn update_sprite_dimensions(&mut self, id: &str, width: u32, height: u32) {
        if let Some(asset) = self.sprites.get_mut(id) {
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

    #[test]
    fn register_and_lookup() {
        let mut reg = AssetRegistry::new();
        let h = reg.register_sprite(
            "player_idle".into(),
            FilterMode::Nearest,
            32,
            32,
            PathBuf::from("dummy.png"),
        );
        let sprite = reg.get_sprite("player_idle").unwrap();
        assert_eq!(sprite.texture, h);
        assert_eq!(sprite.width, 32);
    }

    #[test]
    fn handles_are_unique() {
        let mut reg = AssetRegistry::new();
        let h1 = reg.register_sprite(
            "a".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("a.png"),
        );
        let h2 = reg.register_sprite(
            "b".into(),
            FilterMode::Linear,
            32,
            32,
            PathBuf::from("b.png"),
        );
        assert_ne!(h1, h2);
    }

    #[test]
    #[should_panic(expected = "duplicate sprite ID")]
    fn duplicate_sprite_id_panics() {
        let mut reg = AssetRegistry::new();
        reg.register_sprite(
            "same".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("same.png"),
        );
        reg.register_sprite(
            "same".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("same2.png"),
        );
    }

    #[test]
    fn sprite_id_for_path_reverse_lookup() {
        let mut reg = AssetRegistry::new();
        let path = PathBuf::from("/assets/sprites/foo.png");
        reg.register_sprite("foo".into(), FilterMode::Nearest, 32, 32, path.clone());
        assert_eq!(reg.sprite_id_for_path(&path), Some("foo"));
        assert_eq!(reg.sprite_id_for_path(Path::new("/other.png")), None);
    }

    #[test]
    fn update_sprite_dimensions_changes_stored_size() {
        let mut reg = AssetRegistry::new();
        reg.register_sprite(
            "bar".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("bar.png"),
        );
        reg.update_sprite_dimensions("bar", 32, 64);
        let asset = reg.get_sprite("bar").unwrap();
        assert_eq!(asset.width, 32);
        assert_eq!(asset.height, 64);
    }
}
