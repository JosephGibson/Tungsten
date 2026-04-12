use std::collections::HashMap;

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
}

/// Runtime asset registry, stored as a Resource in the World (D-014).
/// Maps string IDs to loaded asset data.
#[derive(Debug, Default)]
pub struct AssetRegistry {
    sprites: HashMap<String, SpriteAsset>,
    next_texture_handle: u32,
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
    ) -> TextureHandle {
        assert!(
            !self.sprites.contains_key(&id),
            "duplicate sprite ID '{id}' — each sprite must be registered exactly once"
        );
        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle += 1;
        self.sprites.insert(
            id,
            SpriteAsset {
                texture: handle,
                filter,
                width,
                height,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let mut reg = AssetRegistry::new();
        let h = reg.register_sprite("player_idle".into(), FilterMode::Nearest, 32, 32);
        let sprite = reg.get_sprite("player_idle").unwrap();
        assert_eq!(sprite.texture, h);
        assert_eq!(sprite.width, 32);
    }

    #[test]
    fn handles_are_unique() {
        let mut reg = AssetRegistry::new();
        let h1 = reg.register_sprite("a".into(), FilterMode::Nearest, 16, 16);
        let h2 = reg.register_sprite("b".into(), FilterMode::Linear, 32, 32);
        assert_ne!(h1, h2);
    }

    #[test]
    #[should_panic(expected = "duplicate sprite ID")]
    fn duplicate_sprite_id_panics() {
        let mut reg = AssetRegistry::new();
        reg.register_sprite("same".into(), FilterMode::Nearest, 16, 16);
        reg.register_sprite("same".into(), FilterMode::Nearest, 16, 16);
    }
}
