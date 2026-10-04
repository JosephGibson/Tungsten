use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::atlas::UvRect;
use super::manifest::FilterMode;
use super::particle::AssetId;

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

/// Dense sprite handle, minted by [`AssetRegistry`]; stable for the registry's life.
///
/// An ID means something only in the registry that minted it: another registry
/// reads its own slot at that index, or `None` past its end.
pub type SpriteAssetId = AssetId<SpriteAsset>;

/// D-014 runtime asset registry resource.
///
/// Sprite names are interned to dense [`SpriteAssetId`]s. Interning is
/// append-only, and an interned name reads `None` until it is registered.
#[derive(Debug, Default)]
pub struct AssetRegistry {
    /// Indexed by `SpriteAssetId::index`; `None` until registered.
    sprites: Vec<Option<SpriteAsset>>,
    sprite_names: Vec<String>,
    sprite_id_by_name: HashMap<String, SpriteAssetId>,
    path_to_sprite_id: HashMap<PathBuf, SpriteAssetId>,
}

impl AssetRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns `name`: the same name gets the same ID, registered or not.
    ///
    /// An interned name draws nothing until [`Self::register_sprite`] fills its slot.
    pub fn intern_sprite(&mut self, name: &str) -> SpriteAssetId {
        match self.sprite_id(name) {
            Some(id) => id,
            None => self.intern_new_sprite(name.to_owned()),
        }
    }

    fn intern_new_sprite(&mut self, name: String) -> SpriteAssetId {
        let id = SpriteAssetId::new(self.sprites.len() as u32);
        self.sprites.push(None);
        self.sprite_names.push(name.clone());
        self.sprite_id_by_name.insert(name, id);
        id
    }

    /// Interns `names` in sorted order, so a name set gets the same IDs every run.
    ///
    /// Names already interned keep their IDs; duplicates in `names` are ignored.
    pub fn intern_sprites<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        let mut names: Vec<&str> = names.into_iter().collect();
        names.sort_unstable();
        names.dedup();
        for name in names {
            self.intern_sprite(name);
        }
    }

    /// ID of an interned name; never interns.
    #[must_use]
    pub fn sprite_id(&self, name: &str) -> Option<SpriteAssetId> {
        self.sprite_id_by_name.get(name).copied()
    }

    /// Registered sprite for `id`, by index. `None` for an ID past the end or
    /// one interned and not yet registered. `id` must come from this registry.
    #[must_use]
    pub fn sprite(&self, id: SpriteAssetId) -> Option<&SpriteAsset> {
        self.sprites.get(id.index() as usize)?.as_ref()
    }

    /// Name interned as `id`; `None` for an ID past the end.
    #[must_use]
    pub fn sprite_name(&self, id: SpriteAssetId) -> Option<&str> {
        self.sprite_names
            .get(id.index() as usize)
            .map(String::as_str)
    }

    /// Register sprite with renderer-owned atlas handle and UV rect, filling the
    /// slot `id` is interned to (interning it if needed), and return its ID.
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
    ) -> SpriteAssetId {
        assert!(
            self.get_sprite(&id).is_none(),
            "duplicate sprite ID '{id}' — each sprite must be registered exactly once"
        );
        let sprite_id = match self.sprite_id(&id) {
            Some(sprite_id) => sprite_id,
            None => self.intern_new_sprite(id),
        };
        self.path_to_sprite_id.insert(path.clone(), sprite_id);
        if let Some(np) = &normal_path {
            self.path_to_sprite_id.insert(np.clone(), sprite_id);
        }
        if let Some(ep) = &emissive_path {
            self.path_to_sprite_id.insert(ep.clone(), sprite_id);
        }
        self.sprites[sprite_id.index() as usize] = Some(SpriteAsset {
            atlas,
            uv,
            filter,
            width,
            height,
            path,
            normal_path,
            emissive_path,
            lit_atlas,
        });
        sprite_id
    }

    #[must_use]
    pub fn get_sprite(&self, id: &str) -> Option<&SpriteAsset> {
        self.sprite(self.sprite_id(id)?)
    }

    fn get_sprite_mut(&mut self, id: &str) -> Option<&mut SpriteAsset> {
        let index = self.sprite_id(id)?.index() as usize;
        self.sprites[index].as_mut()
    }

    /// Names of the registered sprites, in ID order; interned-only names are left out.
    pub fn sprite_names(&self) -> impl Iterator<Item = &str> {
        self.sprite_names
            .iter()
            .zip(&self.sprites)
            .filter(|(_, asset)| asset.is_some())
            .map(|(name, _)| name.as_str())
    }

    /// Sprite name for a source path (albedo, normal or emissive).
    #[must_use]
    pub fn sprite_name_for_path(&self, path: &Path) -> Option<&str> {
        self.sprite_name(*self.path_to_sprite_id.get(path)?)
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
        if let Some(asset) = self.get_sprite_mut(id) {
            asset.atlas = atlas;
            asset.uv = uv;
            asset.width = width;
            asset.height = height;
        }
    }

    /// Update the M29 lit atlas marker after an atlas (re)build.
    pub fn update_sprite_lit_atlas(&mut self, id: &str, lit_atlas: Option<TextureHandle>) {
        if let Some(asset) = self.get_sprite_mut(id) {
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
